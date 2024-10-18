use crate::http::{self, Http};
use crate::json::{self, FromJsonObject, ToJson};
use crate::wifi::Wifi;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use embedded_io_async::{BufRead, Write};
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

/// Access token response
#[derive(Debug, Default)]
struct AccessTokenResponse {
    accesstoken: AccessToken,
    // URL: String,
    // httpstatuscode: String,
}

impl FromJsonObject for AccessTokenResponse {
    type Context = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        reader: &mut json::Reader<R>,
        _context: &Self::Context,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "accesstoken" => self.accesstoken = reader.read().await?,
            _ => _ = reader.read_any().await?,
        }
        Ok(())
    }
}

/// Sign in request
#[derive(Debug)]
struct SignInRequest<'a> {
    accesstoken: &'a str,
    username: &'a str,
    password_md5: &'a str,
    appkey: &'a str,
    cid: Option<u32>,
    auth_secret: Option<&'a str>,
}

impl<'a> ToJson for SignInRequest<'a> {
    async fn to_json<W: Write>(
        &self,
        writer: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        let mut object = writer.write_object().await?;
        let mut object = object
            .field("accesstoken", self.accesstoken)
            .await?
            .field("username", self.username)
            .await?
            .field("password", self.password_md5)
            .await?
            .field("appkey", self.appkey)
            .await?;
        if let Some(cid) = self.cid {
            object = object.field("cid", f64::from(cid)).await?;
        }
        if let Some(auth_secret) = self.auth_secret {
            object = object.field("auth_secret", auth_secret).await?;
        }
        object.finish().await
    }
}

/// Sign in response
#[derive(Debug, Default)]
struct SignInResponse {
    // httpstatuscode: String,
}

impl FromJsonObject for SignInResponse {
    type Context = ();

    async fn read_next<R: BufRead>(
        &mut self,
        _key: String,
        reader: &mut json::Reader<R>,
        _context: &Self::Context,
    ) -> Result<(), json::Error<R::Error>> {
        _ = reader.read_any().await?;
        Ok(())
    }
}

/// User information request
#[derive(Debug)]
struct UserInformationRequest<'a> {
    accesstoken: &'a str,
}

impl<'a> ToJson for UserInformationRequest<'a> {
    async fn to_json<W: Write>(
        &self,
        writer: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        writer
            .write_object()
            .await?
            .field("accesstoken", self.accesstoken)
            .await?
            .finish()
            .await
    }
}

/// User information response
#[derive(Debug, Default)]
struct UserInformationResponse {
    uid: u32,
    firstname: String,
    lastname: String,
    memberid: u32,
    status: String,
    cid: u32,
    roles: Vec<String>,
    email: String,
    // httpstatuscode: String,
}

impl FromJsonObject for UserInformationResponse {
    type Context = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        reader: &mut json::Reader<R>,
        _context: &Self::Context,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "uid" => self.uid = reader.read_any().await?.try_into()?,
            "firstname" => self.firstname = reader.read().await?,
            "lastname" => self.lastname = reader.read().await?,
            "memberid" => self.memberid = reader.read_any().await?.try_into()?,
            "status" => self.status = reader.read().await?,
            "cid" => self.cid = reader.read_any().await?.try_into()?,
            "roles" => self.roles = reader.read().await?,
            "email" => self.email = reader.read().await?,
            _ => _ = reader.read_any().await?,
        }
        Ok(())
    }
}

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
