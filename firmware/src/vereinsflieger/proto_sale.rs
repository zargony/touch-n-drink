use super::AccessToken;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// `sale/add` request
#[derive(Debug, Serialize)]
pub struct SaleAddRequest<'a> {
    pub accesstoken: &'a AccessToken,
    pub bookingdate: &'a str, // "yyyy-mm-dd"
    pub articleid: &'a str,
    pub amount: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memberid: Option<u32>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub callsign: Option<&'a str>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub salestax: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totalprice: Option<f32>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub counter: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<&'a str>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub ccid: Option<&'a str>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub caid2: Option<u32>,
}

/// `sale/add` response
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SaleAddResponse {
    // pub createtime: String, // "yyyy-mm-dd hh:mm:ss"
    // pub modifytime: String, // "yyyy-mm-dd hh:mm:ss"
    // pub bookingdate: String, // "yyyy-mm-dd"
    // pub callsign: String,
    // pub comment: String,
    // pub username: String,
    // #[serde_as(as = "DisplayFromStr")]
    // pub uid: u32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub memberid: u32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub amount: f32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub netvalue: f32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub salestax: f32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub totalprice: f32,
    // #[serde_as(as = "DisplayFromStr")]
    // pub supid: u32,
    // pub articleid: String,
    // #[serde_as(as = "DisplayFromStr")]
    // pub caid2: u32,
    // pub httpstatuscode: u16,
}
