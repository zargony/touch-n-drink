mod proto_articles;
mod proto_auth;
mod proto_sale;
mod proto_user;

use crate::article::{ArticleId, Articles};
use crate::http::{self, Http};
use crate::time;
use crate::user::{UserId, Users};
use crate::wifi::Wifi;
use alloc::format;
use alloc::string::String;
use core::cell::RefCell;
use core::fmt;
use log::{debug, info, warn};

/// Vereinsflieger API base URL
const BASE_URL: &str = "https://www.vereinsflieger.de/interface/rest";

/// Vereinsflieger API error
#[derive(Debug)]
pub enum Error {
    /// HTTP error (network, protocol, (de)serializing)
    Http(http::Error),
}

impl From<http::Error> for Error {
    fn from(err: http::Error) -> Self {
        Self::Http(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(_err) => write!(f, "API error"),
        }
    }
}

/// Access token
type AccessToken = String;

/// Vereinsflieger API client
pub struct Vereinsflieger<'a> {
    http: Http<'a>,
    username: &'a str,
    password_md5: &'a str,
    appkey: &'a str,
    cid: Option<u32>,
    accesstoken: Option<AccessToken>,
}

impl fmt::Debug for Vereinsflieger<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vereinsflieger")
            .field("http", &self.http)
            .field("username", &self.username)
            .field("password_md5", &"<redacted>")
            .field("appkey", &"<redacted>")
            .field("cid", &self.cid)
            .finish()
    }
}

impl<'a> Vereinsflieger<'a> {
    /// Create new Vereinsflieger API client using the given TCP and DNS sockets
    pub fn new(
        wifi: &'a Wifi,
        seed: u64,
        resources: &'a mut http::Resources,
        username: &'a str,
        password_md5: &'a str,
        appkey: &'a str,
        cid: Option<u32>,
    ) -> Self {
        let http = Http::new(wifi, seed, resources);

        Self {
            http,
            username,
            password_md5,
            appkey,
            cid,
            accesstoken: None,
        }
    }

    /// Connect to API server
    pub async fn connect(&mut self) -> Result<Connection, Error> {
        Connection::new(self).await
    }
}

/// Vereinsflieger API client connection
pub struct Connection<'a> {
    connection: http::Connection<'a>,
    accesstoken: &'a AccessToken,
}

impl fmt::Debug for Connection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("connection", &self.connection)
            .field("accesstoken", &"<redacted>")
            .finish()
    }
}

impl Connection<'_> {
    /// Fetch information about authenticated user
    #[allow(dead_code)]
    pub async fn get_user_information(&mut self) -> Result<(), Error> {
        use proto_auth::{UserInformationRequest, UserInformationResponse};

        let response: UserInformationResponse = self
            .connection
            .post(
                "auth/getuser",
                &UserInformationRequest {
                    accesstoken: self.accesstoken,
                },
            )
            .await?;
        debug!("Vereinsflieger: Got user information: {:?}", response);
        Ok(())
    }

    /// Fetch list of articles and update article lookup table
    pub async fn refresh_articles<const N: usize>(
        &mut self,
        articles: &mut Articles<N>,
    ) -> Result<(), Error> {
        use proto_articles::{ArticleListRequest, ArticleListResponse};

        debug!("Vereinsflieger: Refreshing articles...");
        let request_body = http::Connection::prepare_body(&ArticleListRequest {
            accesstoken: self.accesstoken,
        })
        .await?;
        let mut rx_buf = [0; 4096];
        let mut json = self
            .connection
            .post_json("articles/list", &request_body, &mut rx_buf)
            .await?;

        articles.clear();
        let articles = RefCell::new(articles);

        let response: ArticleListResponse<N> = json
            .read_object_with_context(&articles)
            .await
            .map_err(http::Error::MalformedResponse)?;
        info!(
            "Vereinsflieger: Refreshed {} of {} articles",
            articles.borrow().count(),
            response.total_articles
        );

        // Discard remaining body (needed to make the next pipelined request work)
        json.discard_to_end()
            .await
            .map_err(http::Error::MalformedResponse)?;

        Ok(())
    }

    /// Fetch list of users and update user lookup table
    pub async fn refresh_users(&mut self, users: &mut Users) -> Result<(), Error> {
        use proto_user::{UserListRequest, UserListResponse};

        debug!("Vereinsflieger: Refreshing users...");
        let request_body = http::Connection::prepare_body(&UserListRequest {
            accesstoken: self.accesstoken,
        })
        .await?;
        let mut rx_buf = [0; 4096];
        let mut json = self
            .connection
            .post_json("user/list", &request_body, &mut rx_buf)
            .await?;

        users.clear();
        let users = RefCell::new(users);

        let response: UserListResponse = json
            .read_object_with_context(&users)
            .await
            .map_err(http::Error::MalformedResponse)?;
        info!(
            "Vereinsflieger: Refreshed {} of {} users",
            users.borrow().count(),
            response.total_users
        );

        // Discard remaining body (needed to make the next pipelined request work)
        json.discard_to_end()
            .await
            .map_err(http::Error::MalformedResponse)?;

        Ok(())
    }

    /// Store a purchase
    pub async fn purchase(
        &mut self,
        article_id: &ArticleId,
        amount: f32,
        user_id: UserId,
        total_price: f32,
    ) -> Result<(), Error> {
        use proto_sale::{SaleAddRequest, SaleAddResponse};

        debug!(
            "Vereinsflieger: Purchasing {}x {}, {:.02} EUR for user {}",
            amount, article_id, total_price, user_id
        );

        let _response: SaleAddResponse = self
            .connection
            .post(
                "sale/add",
                &SaleAddRequest {
                    accesstoken: self.accesstoken,
                    bookingdate: &Self::today(),
                    articleid: article_id,
                    amount,
                    memberid: Some(user_id),
                    totalprice: Some(total_price),
                    comment: None,
                },
            )
            .await?;
        debug!("Vereinsflieger: Purchase successful");
        Ok(())
    }
}

impl<'a> Connection<'a> {
    /// Connect to API server, check existing access token (if any) or fetch a new one and sign
    /// in. Return connection for authenticated API requests.
    async fn new(vf: &'a mut Vereinsflieger<'_>) -> Result<Self, Error> {
        // Connect to API server
        let mut connection = vf.http.connect(BASE_URL).await?;

        // If exist, check validity of access token
        if let Some(ref accesstoken) = vf.accesstoken {
            use proto_auth::{UserInformationRequest, UserInformationResponse};

            let response: Result<UserInformationResponse, _> = connection
                .post("auth/getuser", &UserInformationRequest { accesstoken })
                .await;
            match response {
                Ok(_userinfo) => debug!("Vereinsflieger: Access token valid"),
                Err(http::Error::Unauthorized) => {
                    debug!("Vereinsflieger: Access token expired");
                    vf.accesstoken = None;
                }
                Err(err) => return Err(err.into()),
            }
        }

        // Without an access token, fetch a new access token and sign in
        if vf.accesstoken.is_none() {
            use proto_auth::{AccessTokenResponse, SignInRequest, SignInResponse};

            // Fetch a new access token
            let response: AccessTokenResponse = connection.get("auth/accesstoken").await?;
            let accesstoken = response.accesstoken;
            // debug!("Vereinsflieger: Got access token {}", accesstoken);
            debug!(
                "Vereinsflieger: Got access token (length {})",
                accesstoken.len()
            );

            // Use credentials to sign in
            let response: Result<SignInResponse, _> = connection
                .post(
                    "auth/signin",
                    &SignInRequest {
                        accesstoken: &accesstoken,
                        username: vf.username,
                        password_md5: vf.password_md5,
                        appkey: vf.appkey,
                        cid: vf.cid,
                        auth_secret: None,
                    },
                )
                .await;
            match response {
                Ok(_signin) => {
                    vf.accesstoken = Some(accesstoken);
                    info!("Vereinsflieger: Signed in as {}", vf.username);
                }
                Err(err) => {
                    warn!("Vereinsflieger: Sign in failed: {}", err);
                    return Err(err.into());
                }
            }
        }

        match vf.accesstoken {
            Some(ref accesstoken) => Ok(Self {
                connection,
                accesstoken,
            }),
            // Actually unreachable
            None => Err(http::Error::Unauthorized.into()),
        }
    }

    /// Helper function to get today's date as "yyyy-mm-dd" string
    fn today() -> String {
        if let Some(now) = time::now() {
            format!("{}", now.format("%Y-%m-%d"))
        } else {
            String::new()
        }
    }
}
