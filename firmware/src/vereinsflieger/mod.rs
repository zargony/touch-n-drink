mod proto_auth;

use crate::http::{self, Http};
use crate::wifi::Wifi;
use alloc::string::String;
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
}

impl<'a> fmt::Debug for Vereinsflieger<'a> {
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
        }
    }

    /// Connect to API server
    #[allow(dead_code)]
    pub async fn connect(&mut self) -> Result<Connection, Error> {
        let connection = self.http.connect(BASE_URL).await?;

        Connection::sign_in(
            connection,
            self.username,
            self.password_md5,
            self.appkey,
            self.cid,
        )
        .await
    }
}

/// Vereinsflieger API client connection
pub struct Connection<'a> {
    connection: http::Connection<'a>,
    accesstoken: AccessToken,
}

impl<'a> fmt::Debug for Connection<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("connection", &self.connection)
            .field("accesstoken", &"<redacted>")
            .finish()
    }
}

impl<'a> Connection<'a> {
    /// Fetch information about authenticated user
    #[allow(dead_code)]
    pub async fn get_user_information(&mut self) -> Result<(), Error> {
        use proto_auth::{UserInformationRequest, UserInformationResponse};

        let response: UserInformationResponse = self
            .connection
            .post(
                "auth/getuser",
                &UserInformationRequest {
                    accesstoken: &self.accesstoken,
                },
            )
            .await
            .unwrap();
        debug!("Vereinsflieger: Got user information: {:?}", response);
        Ok(())
    }
}

impl<'a> Connection<'a> {
    /// Fetch access token, sign in to API server, return connection for API requests
    async fn sign_in(
        mut connection: http::Connection<'a>,
        username: &str,
        password_md5: &str,
        appkey: &str,
        cid: Option<u32>,
    ) -> Result<Self, Error> {
        use proto_auth::{AccessTokenResponse, SignInRequest, SignInResponse};

        // Fetch access token
        let response: AccessTokenResponse = connection.get("auth/accesstoken").await?;
        let accesstoken = response.accesstoken;
        // debug!("Vereinsflieger: Got access token {}", accesstoken);
        debug!(
            "Vereinsflieger: Got access token (length {})",
            accesstoken.len()
        );

        // Use access token and credentials to sign in
        let response: Result<SignInResponse, _> = connection
            .post(
                "auth/signin",
                &SignInRequest {
                    accesstoken: &accesstoken,
                    username,
                    password_md5,
                    appkey,
                    cid,
                    auth_secret: None,
                },
            )
            .await;
        match response {
            Ok(_) => {
                info!("Vereinsflieger: Signed in as {}", username);
                Ok(Self {
                    connection,
                    accesstoken,
                })
            }
            Err(err) => {
                warn!("Vereinsflieger: Sign in failed: {}", err);
                Err(err.into())
            }
        }
    }
}
