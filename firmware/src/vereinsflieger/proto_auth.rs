use super::AccessToken;
use crate::json::{self, FromJsonObject, ToJson};
use alloc::string::String;
use alloc::vec::Vec;
use embedded_io_async::{BufRead, Write};

/// `auth/accesstoken` response
#[derive(Debug, Default)]
pub struct AccessTokenResponse {
    pub accesstoken: AccessToken,
    // pub URL: String,
    // pub httpstatuscode: u16,
}

impl FromJsonObject for AccessTokenResponse {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "accesstoken" => self.accesstoken = json.read().await?,
            _ => json.skip_any().await?,
        }
        Ok(())
    }
}

/// `auth/signin` request
#[derive(Debug)]
pub struct SignInRequest<'a> {
    pub accesstoken: &'a AccessToken,
    pub username: &'a str,
    pub password_md5: &'a str,
    pub appkey: &'a str,
    pub cid: Option<u32>,
    pub auth_secret: Option<&'a str>,
}

impl ToJson for SignInRequest<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        let mut object = json.write_object().await?;
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

/// `auth/signin` response
#[derive(Debug, Default)]
pub struct SignInResponse {
    // pub httpstatuscode: u16,
}

impl FromJsonObject for SignInResponse {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        _key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        json.skip_any().await
    }
}

/// `auth/getuser` request
#[derive(Debug)]
pub struct UserInformationRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

impl ToJson for UserInformationRequest<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write_object()
            .await?
            .field("accesstoken", self.accesstoken)
            .await?
            .finish()
            .await
    }
}

/// `auth/getuser` response
#[derive(Debug, Default)]
pub struct UserInformationResponse {
    pub uid: u32,
    pub firstname: String,
    pub lastname: String,
    pub memberid: u32,
    pub status: String,
    // pub cid: u32, // undocumented
    pub roles: Vec<String>,
    pub email: String,
    // pub httpstatuscode: u16,
}

impl FromJsonObject for UserInformationResponse {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "uid" => self.uid = json.read_any().await?.try_into()?,
            "firstname" => self.firstname = json.read().await?,
            "lastname" => self.lastname = json.read().await?,
            "memberid" => self.memberid = json.read_any().await?.try_into()?,
            "status" => self.status = json.read().await?,
            "roles" => self.roles = json.read().await?,
            "email" => self.email = json.read().await?,
            _ => json.skip_any().await?,
        }
        Ok(())
    }
}
