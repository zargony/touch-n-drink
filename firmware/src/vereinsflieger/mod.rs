mod proto_articles;
mod proto_auth;
mod proto_sale;
mod proto_user;

pub use proto_articles::Article;
pub use proto_user::User;

use crate::reader::{self, StreamingJsonObjectReader};
use crate::time;
use alloc::string::String;
use alloc::vec;
use chrono::{DateTime, NaiveDate};
use derive_more::{Display, From};
use embassy_time::{Duration, Instant, with_deadline, with_timeout};
use embedded_nal_async::{Dns, TcpConnect};
use log::{debug, info, warn};
use reqwless::client::{HttpClient, HttpConnection, HttpResource};
use reqwless::headers::ContentType;
use reqwless::request::RequestBuilder;
use reqwless::response::{Response, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Vereinsflieger API base URL
const BASE_URL: &str = "https://www.vereinsflieger.de/interface/rest";

/// How long to wait for a server response
const TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait to finish streaming a server's response
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum size of response headers from server
const MAX_RESPONSE_HEADER_SIZE: usize = 2048;

/// Vereinsflieger API error
#[derive(Debug, Display, From)]
pub enum Error {
    /// Network error
    #[from]
    #[display("Network: {_0}")]
    Network(reqwless::Error),
    /// Request could not be built
    #[display("Malformed request")]
    MalformedRequest(serde_json::Error),
    /// Request failed
    #[display("Request failed ({})", _0.0)]
    RequestFailed(StatusCode),
    /// Response could not be parsed
    #[display("Malformed response")]
    MalformedResponse(serde_json::Error),
    /// Streaming response could not be parsed
    #[display("Malformed response stream")]
    MalformedResponseStream(reader::Error<reqwless::Error>),
    /// Timeout waiting for response
    #[from(embassy_time::TimeoutError)]
    #[display("Timeout")]
    Timeout,
    /// Not logged in
    #[display("Not logged in")]
    NotLoggedIn,
}

impl From<reader::Error<reqwless::Error>> for Error {
    fn from(err: reader::Error<reqwless::Error>) -> Self {
        match err {
            reader::Error::Read(err) => Self::Network(err),
            err => Self::MalformedResponseStream(err),
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

    /// Connect to API server, check existing access token (if any) or fetch a new one and sign
    /// in. Returns connection for authenticated API requests.
    pub async fn connect<'vf, 'conn, T: TcpConnect, D: Dns>(
        &'vf mut self,
        http: &'conn mut HttpClient<'_, T, D>,
    ) -> Result<Connection<'vf, 'conn, T>, Error> {
        // Connect to API server
        let resource = with_timeout(TIMEOUT, http.resource(BASE_URL)).await??;
        debug!("Vereinsflieger: Connected {BASE_URL}");

        // Check whether the current access token (if any) is signed in. If not, forget it.
        // Note: juggling with a temporary connection, as self can't be modified while borrowed
        let mut connection: Connection<T> = Connection::new(self, resource);
        let is_signed_in = connection.is_signed_in().await?;
        let resource = connection.into_resource();
        if !is_signed_in {
            self.accesstoken = None;
        }

        // Without an access token, fetch a new access token and sign in
        // Note: juggling with a temporary connection, as self can't be modified while borrowed
        let connection = if self.accesstoken.is_none() {
            // Fetch a new access token
            let mut connection: Connection<T> = Connection::new(self, resource);
            let accesstoken = connection.get_new_accesstoken().await?;
            let resource = connection.into_resource();
            self.accesstoken = Some(accesstoken);

            // Use credentials to sign in
            let mut connection: Connection<T> = Connection::new(self, resource);
            connection
                .sign_in(self.username, self.password_md5, self.appkey, self.cid)
                .await?;
            connection
        } else {
            Connection::new(self, resource)
        };

        Ok(connection)
    }
}

impl Vereinsflieger<'_> {
    /// Vereinsflieger API accesstoken
    fn accesstoken(&self) -> Result<&AccessToken, Error> {
        self.accesstoken.as_ref().ok_or(Error::NotLoggedIn)
    }
}

/// Vereinsflieger API client connection
pub struct Connection<'vf, 'conn, T: TcpConnect + 'conn> {
    vereinsflieger: &'vf Vereinsflieger<'vf>,
    resource: HttpResource<'conn, T::Connection<'conn>>,
}

impl<T: TcpConnect> Connection<'_, '_, T> {
    /// Fetch information about authenticated user
    pub async fn get_user_information(&mut self) -> Result<(), Error> {
        use proto_auth::{UserInformationRequest, UserInformationResponse};

        let response: UserInformationResponse = self
            .http_json_post(
                "auth/getuser",
                &UserInformationRequest {
                    accesstoken: self.vereinsflieger.accesstoken()?,
                },
            )
            .await?;
        debug!("Vereinsflieger: Got user information: {response:?}");
        Ok(())
    }

    /// Fetch list of articles and call closure with each
    pub async fn get_articles<F>(&mut self, mut f: F) -> Result<usize, Error>
    where
        F: AsyncFnMut(&Article),
    {
        use proto_articles::ArticleListRequest;

        /// Helper to distinguish article and numeric status code in response
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        pub enum ArticleOrNumber {
            Article(Article),
            #[expect(dead_code)]
            Number(u16),
        }

        let mut total_articles = 0;
        self.http_json_post_fn::<_, ArticleOrNumber, _>(
            "articles/list",
            &ArticleListRequest {
                accesstoken: self.vereinsflieger.accesstoken()?,
            },
            async |_key, element| {
                match element {
                    ArticleOrNumber::Article(article) => {
                        total_articles += 1;
                        f(article).await;
                    }
                    ArticleOrNumber::Number(_) => (),
                }
                Ok(())
            },
        )
        .await?;
        debug!("Vereinsflieger: Got {total_articles} articles");
        Ok(total_articles)
    }

    /// Fetch list of users and call closure with each
    pub async fn get_users<F>(&mut self, mut f: F) -> Result<usize, Error>
    where
        F: AsyncFnMut(&User),
    {
        use proto_user::UserListRequest;

        /// Helper to distinguish user and numeric status code in response
        #[derive(Debug, Deserialize)]
        #[serde(untagged)]
        pub enum UserOrNumber {
            User(User),
            #[expect(dead_code)]
            Number(u16),
        }

        let mut total_users = 0;
        self.http_json_post_fn::<_, UserOrNumber, _>(
            "user/list",
            &UserListRequest {
                accesstoken: self.vereinsflieger.accesstoken()?,
            },
            async |_key, element| {
                match element {
                    UserOrNumber::User(user) => {
                        total_users += 1;
                        f(user).await;
                    }
                    UserOrNumber::Number(_) => (),
                }
                Ok(())
            },
        )
        .await?;
        debug!("Vereinsflieger: Got {total_users} users");
        Ok(total_users)
    }

    /// Store a purchase
    pub async fn purchase(
        &mut self,
        date: NaiveDate,
        article_id: &str,
        amount: f32,
        user_id: u32,
        total_price: f32,
    ) -> Result<(), Error> {
        use proto_sale::{SaleAddRequest, SaleAddResponse};

        debug!(
            "Vereinsflieger: Purchasing {amount}x {article_id}, {total_price:.02} EUR for user {user_id}"
        );

        let _response: SaleAddResponse = self
            .http_json_post(
                "sale/add",
                &SaleAddRequest {
                    accesstoken: self.vereinsflieger.accesstoken()?,
                    bookingdate: date,
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

impl<'vf, 'conn, T: TcpConnect> Connection<'vf, 'conn, T> {
    /// Create new connection from given client and http resource handle
    fn new(
        vereinsflieger: &'vf Vereinsflieger<'vf>,
        resource: HttpResource<'conn, T::Connection<'conn>>,
    ) -> Self {
        Self {
            vereinsflieger,
            resource,
        }
    }

    /// Consume connection and return the http resource handle
    fn into_resource(self) -> HttpResource<'conn, T::Connection<'conn>> {
        self.resource
    }
}

impl<T: TcpConnect> Connection<'_, '_, T> {
    /// Check whether the current access token is valid and signed in
    async fn is_signed_in(&mut self) -> Result<bool, Error> {
        match self.get_user_information().await {
            Ok(()) => {
                debug!("Vereinsflieger: Access token valid");
                Ok(true)
            }
            Err(Error::RequestFailed(status)) if status.0 == 401 => {
                debug!("Vereinsflieger: Access token expired");
                Ok(false)
            }
            Err(Error::NotLoggedIn) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Sign in using the given credentials
    async fn sign_in(
        &mut self,
        username: &str,
        password_md5: &str,
        appkey: &str,
        cid: Option<u32>,
    ) -> Result<(), Error> {
        use proto_auth::{SignInRequest, SignInResponse};

        match self
            .http_json_post::<_, SignInResponse>(
                "auth/signin",
                &SignInRequest {
                    accesstoken: self.vereinsflieger.accesstoken()?,
                    username,
                    password_md5,
                    appkey,
                    cid,
                    auth_secret: None,
                },
            )
            .await
        {
            Ok(_response) => info!("Vereinsflieger: Signed in as {username}"),
            Err(err) => {
                warn!("Vereinsflieger: Sign in failed: {err}");
                return Err(err);
            }
        }
        Ok(())
    }

    // Fetch a new access token
    async fn get_new_accesstoken(&mut self) -> Result<AccessToken, Error> {
        use proto_auth::AccessTokenResponse;

        let response: AccessTokenResponse = self.http_json_get("auth/accesstoken").await?;
        let accesstoken = response.accesstoken;
        // debug!("Vereinsflieger: Got access token {accesstoken}");
        debug!(
            "Vereinsflieger: Got access token (length {})",
            accesstoken.len()
        );
        Ok(accesstoken)
    }

    /// Send HTTP GET request, deserialize JSON response
    async fn http_json_get<R: DeserializeOwned>(&mut self, path: &str) -> Result<R, Error> {
        let deadline = Instant::now() + TIMEOUT;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();

        let request = self
            .resource
            .get(path)
            .headers(&[("Accept", "application/json")]);

        debug!("Vereinsflieger: GET {BASE_URL}/{path}");
        let response = with_deadline(deadline, request.send(&mut rx_buf)).await??;

        debug!("Vereinsflieger: HTTP status {}", response.status.0);
        if !response.status.is_successful() {
            return Err(Error::RequestFailed(response.status));
        }

        let body = with_deadline(deadline, response.body().read_to_end()).await??;

        serde_json::from_slice(body).map_err(Error::MalformedResponse)
    }

    /// Serialize data to JSON, send HTTP POST request, deserialize JSON response
    async fn http_json_post<D: Serialize, R: DeserializeOwned>(
        &mut self,
        path: &str,
        data: &D,
    ) -> Result<R, Error> {
        let deadline = Instant::now() + TIMEOUT;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();

        self.http_post_fn(&mut rx_buf, path, data, async |response| {
            with_deadline(deadline, async {
                let body = response.body().read_to_end().await?;
                serde_json::from_slice(body).map_err(Error::MalformedResponse)
            })
            .await?
        })
        .await
    }

    /// Serialize data to JSON, send HTTP POST request, deserialize JSON response stream and call
    /// closure with elements from response stream
    async fn http_json_post_fn<D: Serialize, R: DeserializeOwned, F>(
        &mut self,
        path: &str,
        data: &D,
        mut f: F,
    ) -> Result<(), Error>
    where
        F: AsyncFnMut(&str, &R) -> Result<(), Error>,
    {
        let deadline = Instant::now() + FETCH_TIMEOUT;
        let mut rx_buf = vec![0; MAX_RESPONSE_HEADER_SIZE].into_boxed_slice();

        self.http_post_fn(&mut rx_buf, path, data, async |response| {
            with_deadline(deadline, async {
                let reader = response.body().reader();
                let mut json: StreamingJsonObjectReader<_, R> =
                    StreamingJsonObjectReader::new(reader);
                while let Some((key, element)) = json.next().await? {
                    with_deadline(deadline, f(&key, &element)).await??;
                }
                Ok(())
            })
            .await?
        })
        .await
    }

    /// Serialize data to JSON, send HTTP POST request and call closure with response object
    async fn http_post_fn<D: Serialize, F, R>(
        &mut self,
        rx_buf: &mut [u8],
        path: &str,
        data: &D,
        f: F,
    ) -> Result<R, Error>
    where
        F: AsyncFnOnce(Response<'_, '_, HttpConnection<'_, T::Connection<'_>>>) -> Result<R, Error>,
    {
        let body = serde_json::to_vec(data)
            .map_err(Error::MalformedRequest)?
            .into_boxed_slice();

        let request = self
            .resource
            .post(path)
            .content_type(ContentType::ApplicationJson)
            .headers(&[("Accept", "application/json")])
            .body(body.as_ref());

        debug!(
            "Vereinsflieger: POST {BASE_URL}/{path} ({} bytes)",
            body.len()
        );
        let response = with_timeout(TIMEOUT, request.send(rx_buf)).await??;

        // Extract current date and time from response
        let time = response
            .headers()
            .find_map(|(k, v)| (k.eq_ignore_ascii_case("date")).then_some(v))
            .and_then(|v| str::from_utf8(v).ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok());
        if let Some(time) = time {
            time::set(&time);
        }

        debug!("Vereinsflieger: HTTP status {}", response.status.0);
        if !response.status.is_successful() {
            return Err(Error::RequestFailed(response.status));
        }

        f(response).await
    }
}
