mod proto_articles;
mod proto_auth;
mod proto_sale;
mod proto_user;

use crate::article::{ArticleId, Articles};
use crate::http::{self, Http};
use crate::nfc::Uid;
use crate::time;
use crate::user::{UserId, Users};
use alloc::format;
use alloc::string::String;
use core::fmt;
use core::str::FromStr;
use embassy_time::{Duration, with_timeout};
use log::{debug, info, warn};
use serde::Deserialize;

/// Vereinsflieger API base URL
const BASE_URL: &str = "https://www.vereinsflieger.de/interface/rest";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait to finish streaming a server's response
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Vereinsflieger API error
#[derive(Debug)]
pub enum Error {
    /// Failed to fetch user information
    FetchUserInformation(http::Error),
    /// Failed to fetch articles
    FetchArticles(http::Error),
    /// Failed to fetch users
    FetchUsers(http::Error),
    /// Failed to purchase
    Purchase(http::Error),
    /// Failed to connect to API server
    Connect(http::Error),
    /// Failed to sign in to API server
    SignIn(http::Error),
    /// Timeout waiting for response from API server
    Timeout,
}

impl From<embassy_time::TimeoutError> for Error {
    fn from(_err: embassy_time::TimeoutError) -> Self {
        Self::Timeout
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FetchUserInformation(err) => write!(f, "Fetch user info failed ({err})"),
            Self::FetchArticles(err) => write!(f, "Fetch articles failed ({err})"),
            Self::FetchUsers(err) => write!(f, "Fetch users failed ({err})"),
            Self::Purchase(err) => write!(f, "Purchase failed ({err})"),
            Self::Connect(err) => write!(f, "Connect failed ({err})"),
            Self::SignIn(err) => write!(f, "Sign in failed ({err})"),
            Self::Timeout => write!(f, "Timeout"),
        }
    }
}

/// Access token
type AccessToken = String;

/// Vereinsflieger API client
pub struct Vereinsflieger<'a> {
    username: &'a str,
    password_md5: &'a str,
    appkey: &'a str,
    cid: Option<u32>,
    accesstoken: Option<AccessToken>,
}

impl fmt::Debug for Vereinsflieger<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vereinsflieger")
            .field("username", &self.username)
            .field("password_md5", &"<redacted>")
            .field("appkey", &"<redacted>")
            .field("cid", &self.cid)
            .finish()
    }
}

impl<'a> Vereinsflieger<'a> {
    /// Create new Vereinsflieger API client using the given credentials
    pub fn new(
        username: &'a str,
        password_md5: &'a str,
        appkey: &'a str,
        cid: Option<u32>,
    ) -> Self {
        Self {
            username,
            password_md5,
            appkey,
            cid,
            accesstoken: None,
        }
    }

    /// Connect to API server
    pub async fn connect<'conn>(
        &'conn mut self,
        http: &'conn mut Http<'_>,
    ) -> Result<Connection<'conn>, Error> {
        Connection::new(self, http).await
    }
}

/// Vereinsflieger API client connection
pub struct Connection<'a> {
    http: http::Connection<'a>,
    accesstoken: &'a AccessToken,
}

impl fmt::Debug for Connection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("http", &self.http)
            .field("accesstoken", &"<redacted>")
            .finish()
    }
}

impl Connection<'_> {
    /// Fetch information about authenticated user
    #[allow(dead_code)]
    pub async fn get_user_information(&mut self) -> Result<(), Error> {
        use proto_auth::{UserInformationRequest, UserInformationResponse};

        let response: UserInformationResponse = with_timeout(
            TIMEOUT,
            self.http.post(
                "auth/getuser",
                &UserInformationRequest {
                    accesstoken: self.accesstoken,
                },
            ),
        )
        .await?
        .map_err(Error::FetchUserInformation)?;
        debug!("Vereinsflieger: Got user information: {response:?}");
        Ok(())
    }

    /// Fetch list of articles and update article lookup table
    pub async fn refresh_articles(&mut self, articles: &mut Articles) -> Result<(), Error> {
        use proto_articles::{Article, ArticleListRequest};

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum ArticleOrStatus {
            Article(Article),
            #[allow(dead_code)]
            Status(u16),
        }

        debug!("Vereinsflieger: Refreshing articles...");
        articles.clear();

        let mut total_articles: usize = 0;
        with_timeout(
            FETCH_TIMEOUT,
            self.http.post_fn(
                "articles/list",
                &ArticleListRequest {
                    accesstoken: self.accesstoken,
                },
                |_key, article_or_status: ArticleOrStatus| {
                    if let ArticleOrStatus::Article(article) = article_or_status {
                        total_articles += 1;
                        if let Some(price) = article.price() {
                            articles.update(&article.articleid, &article.designation, price);
                        } else {
                            warn!(
                                "Ignoring article with no valid price ({}): {}",
                                article.articleid, article.designation
                            );
                        }
                    }
                },
            ),
        )
        .await?
        .map_err(Error::FetchArticles)?;

        info!(
            "Vereinsflieger: Refreshed {} of {} articles",
            articles.count(),
            total_articles
        );
        Ok(())
    }

    /// Fetch list of users and update user lookup table
    pub async fn refresh_users(&mut self, users: &mut Users) -> Result<(), Error> {
        use proto_user::{User, UserListRequest};

        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        enum UserOrStatus {
            User(User),
            #[allow(dead_code)]
            Status(u16),
        }

        debug!("Vereinsflieger: Refreshing users...");
        users.clear();

        let mut total_users: usize = 0;
        with_timeout(
            FETCH_TIMEOUT,
            self.http.post_fn(
                "user/list",
                &UserListRequest {
                    accesstoken: self.accesstoken,
                },
                |_key, user_or_status: UserOrStatus| {
                    if let UserOrStatus::User(user) = user_or_status {
                        total_users += 1;
                        if !user.is_retired() {
                            let keys = user.keys_named_with_prefix("NFC Transponder");
                            if !keys.is_empty() {
                                for key in keys {
                                    if let Ok(uid) = Uid::from_str(key) {
                                        users.update_uid(uid, user.memberid);
                                    } else {
                                        warn!(
                                            "Ignoring user key with invalid NFC uid ({}): {}",
                                            user.memberid, key
                                        );
                                    }
                                }
                                users.update_user(user.memberid, user.firstname);
                            }
                        }
                    }
                },
            ),
        )
        .await?
        .map_err(Error::FetchUsers)?;

        info!(
            "Vereinsflieger: Refreshed {} of {} users",
            users.count(),
            total_users
        );
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
            "Vereinsflieger: Purchasing {amount}x {article_id}, {total_price:.02} EUR for user {user_id}"
        );

        let _response: SaleAddResponse = with_timeout(
            TIMEOUT,
            self.http.post(
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
            ),
        )
        .await?
        .map_err(Error::Purchase)?;
        debug!("Vereinsflieger: Purchase successful");
        Ok(())
    }
}

impl<'a> Connection<'a> {
    /// Connect to API server, check existing access token (if any) or fetch a new one and sign
    /// in. Return connection for authenticated API requests.
    async fn new(vf: &'a mut Vereinsflieger<'_>, http: &'a mut Http<'_>) -> Result<Self, Error> {
        // Connect to API server
        let mut connection = with_timeout(TIMEOUT, http.connect(BASE_URL))
            .await?
            .map_err(Error::Connect)?;

        // If exist, check validity of access token
        if let Some(ref accesstoken) = vf.accesstoken {
            use proto_auth::{UserInformationRequest, UserInformationResponse};

            let response: Result<UserInformationResponse, _> = with_timeout(
                TIMEOUT,
                connection.post("auth/getuser", &UserInformationRequest { accesstoken }),
            )
            .await?;
            match response {
                Ok(_userinfo) => debug!("Vereinsflieger: Access token valid"),
                Err(http::Error::Unauthorized) => {
                    debug!("Vereinsflieger: Access token expired");
                    vf.accesstoken = None;
                }
                Err(err) => return Err(Error::Connect(err)),
            }
        }

        // Without an access token, fetch a new access token and sign in
        if vf.accesstoken.is_none() {
            use proto_auth::{AccessTokenResponse, SignInRequest, SignInResponse};

            // Fetch a new access token
            let response: AccessTokenResponse =
                with_timeout(TIMEOUT, connection.get("auth/accesstoken"))
                    .await?
                    .map_err(Error::SignIn)?;
            let accesstoken = response.accesstoken;
            // debug!("Vereinsflieger: Got access token {accesstoken}");
            debug!(
                "Vereinsflieger: Got access token (length {})",
                accesstoken.len()
            );

            // Use credentials to sign in
            let response: Result<SignInResponse, _> = with_timeout(
                TIMEOUT,
                connection.post(
                    "auth/signin",
                    &SignInRequest {
                        accesstoken: &accesstoken,
                        username: vf.username,
                        password_md5: vf.password_md5,
                        appkey: vf.appkey,
                        cid: vf.cid,
                        auth_secret: None,
                    },
                ),
            )
            .await?;
            match response {
                Ok(_signin) => {
                    vf.accesstoken = Some(accesstoken);
                    info!("Vereinsflieger: Signed in as {}", vf.username);
                }
                Err(err) => {
                    warn!("Vereinsflieger: Sign in failed: {err}");
                    return Err(Error::SignIn(err));
                }
            }
        }

        match vf.accesstoken {
            Some(ref accesstoken) => Ok(Self {
                http: connection,
                accesstoken,
            }),
            // Actually unreachable
            None => Err(Error::SignIn(http::Error::Unauthorized)),
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
