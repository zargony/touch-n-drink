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
    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        reader: &mut json::Reader<R>,
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
    password: &'a str,
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
            .field("password", self.password)
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
    async fn read_next<R: BufRead>(
        &mut self,
        _key: String,
        reader: &mut json::Reader<R>,
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
    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        reader: &mut json::Reader<R>,
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
#[derive(Debug)]
pub struct Vereinsflieger<'a> {
    http: Http<'a>,
    accesstoken: Option<AccessToken>,
    username: &'a str,
    password: &'a str,
    appkey: &'a str,
    cid: Option<u32>,
}

impl<'a> Vereinsflieger<'a> {
    /// Create new Vereinsflieger API client using the given TCP and DNS sockets
    pub fn new(
        wifi: &'a Wifi,
        seed: u64,
        resources: &'a mut http::Resources,
        username: &'a str,
        password: &'a str,
        appkey: &'a str,
        cid: Option<u32>,
    ) -> Self {
        let http = Http::new(wifi, seed, resources, BASE_URL);

        Self {
            http,
            accesstoken: None,
            username,
            password,
            appkey,
            cid,
        }
    }

    /// Fetch information about authenticated user
    #[allow(dead_code)]
    pub async fn get_user_information(&mut self) -> Result<(), Error> {
        self.ensure_authenticated().await?;
        let accesstoken = self.accesstoken()?.clone();

        let response: UserInformationResponse = self
            .http
            .post(
                "auth/getuser",
                &UserInformationRequest {
                    accesstoken: &accesstoken,
                },
            )
            .await
            .unwrap();
        debug!("Vereinsflieger: Got user information: {:?}", response);
        Ok(())
    }
}

impl<'a> Vereinsflieger<'a> {
    /// Sign in to API server, returns new access token
    async fn sign_in(
        &mut self,
        username: &str,
        password: &str,
        appkey: &str,
        cid: Option<u32>,
    ) -> Result<AccessToken, Error> {
        // Fetch access token
        let response: AccessTokenResponse = self.http.get("auth/accesstoken").await?;
        let accesstoken = response.accesstoken;
        debug!("Vereinsflieger: Got access token {}", accesstoken);

        // Use access token and credentials to sign in
        let signin_response: Result<SignInResponse, _> = self
            .http
            .post(
                "auth/signin",
                &SignInRequest {
                    accesstoken: &accesstoken,
                    username,
                    password,
                    appkey,
                    cid,
                    auth_secret: None,
                },
            )
            .await;
        match signin_response {
            Ok(_) => {
                info!("Vereinsflieger: Signed in as {}", username);
                Ok(accesstoken)
            }
            Err(err) => {
                warn!("Vereinsflieger: Sign in failed: {}", err);
                Err(err.into())
            }
        }
    }

    /// Sign in to API server if we don't have an access token yet
    async fn ensure_authenticated(&mut self) -> Result<(), Error> {
        if self.accesstoken.is_none() {
            let accesstoken = self
                .sign_in(self.username, self.password, self.appkey, self.cid)
                .await?;
            self.accesstoken = Some(accesstoken);
        }
        Ok(())
    }

    /// Current accesstoken (if any)
    fn accesstoken(&self) -> Result<&AccessToken, Error> {
        match self.accesstoken {
            Some(ref accesstoken) => Ok(accesstoken),
            None => Err(Error::Http(http::Error::Unauthorized)),
        }
    }
}
