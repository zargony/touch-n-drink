use super::AccessToken;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

/// `auth/accesstoken` response
#[derive(Debug, Deserialize)]
pub struct AccessTokenResponse {
    pub accesstoken: AccessToken,
    // pub httpstatuscode: u16,
}

/// `auth/signin` request
#[derive(Debug, Serialize)]
pub struct SignInRequest<'a> {
    pub accesstoken: &'a AccessToken,
    pub username: &'a str,
    #[serde(rename = "password")]
    pub password_md5: &'a str,
    pub appkey: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_secret: Option<&'a str>,
}

/// `auth/signin` response
#[derive(Debug, Deserialize)]
pub struct SignInResponse {
    // pub httpstatuscode: u16,
}

/// `auth/getuser` request
#[derive(Debug, Serialize)]
pub struct UserInformationRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

/// `auth/getuser` response
#[serde_as]
#[derive(Debug, Deserialize)]
#[expect(dead_code)]
pub struct UserInformationResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub uid: u32,
    pub firstname: String,
    pub lastname: String,
    #[serde_as(as = "DisplayFromStr")]
    pub memberid: u32,
    pub status: String,
    // #[serde_as(as = "DisplayFromStr")]
    // pub cid: u32, // undocumented
    pub roles: Vec<String>,
    pub email: String,
    // pub httpstatuscode: u16,
}
