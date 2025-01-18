use super::AccessToken;
use crate::json::{self, FromJsonObject, ToJson};
use alloc::string::{String, ToString};
use embedded_io_async::{BufRead, Write};

/// `sale/add` request
#[derive(Debug)]
pub struct SaleAddRequest<'a> {
    pub accesstoken: &'a AccessToken,
    pub bookingdate: &'a str, // "yyyy-mm-dd"
    pub articleid: &'a str,
    pub amount: f32,
    pub memberid: Option<u32>,
    // pub callsign: Option<&'a str>,
    // pub salestax: Option<f32>,
    pub totalprice: Option<f32>,
    // pub counter: Option<f32>,
    pub comment: Option<&'a str>,
    // pub ccid: Option<&'a str>,
    // pub caid2: Option<u32>,
}

impl ToJson for SaleAddRequest<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        let mut object = json.write_object().await?;
        let mut object = object
            .field("accesstoken", self.accesstoken)
            .await?
            .field("bookingdate", self.bookingdate)
            .await?
            .field("articleid", self.articleid)
            .await?
            .field("amount", self.amount)
            .await?;
        if let Some(memberid) = self.memberid {
            object = object.field("memberid", memberid.to_string()).await?;
        }
        if let Some(totalprice) = self.totalprice {
            object = object.field("totalprice", totalprice.to_string()).await?;
        }
        if let Some(comment) = self.comment {
            object = object.field("comment", comment).await?;
        }
        object.finish().await
    }
}

/// `sale/add` response
#[derive(Debug, Default)]
pub struct SaleAddResponse {
    // pub createtime: String, // "yyyy-mm-dd hh:mm:ss"
    // pub modifytime: String, // "yyyy-mm-dd hh:mm:ss"
    // pub bookingdate: String, // "yyyy-mm-dd"
    // pub callsign: String,
    // pub comment: String,
    // pub username: String,
    // pub uid: u32,
    // pub memberid: u32,
    // pub amount: f32,
    // pub netvalue: f32,
    // pub salestax: f32,
    // pub totalprice: f32,
    // pub supid: u32,
    // pub articleid: String,
    // pub caid2: u32,
    // pub httpstatuscode: u16,
}

impl FromJsonObject for SaleAddResponse {
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
