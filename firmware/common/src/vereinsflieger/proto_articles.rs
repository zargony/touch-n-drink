use super::AccessToken;
use alloc::string::String;
use alloc::vec::Vec;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

/// `articles/list` request
#[derive(Debug, Serialize)]
pub struct ArticleListRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

// /// `articles/list` response
// #[derive(Debug, Deserialize)]
// pub struct ArticleListResponse {
//     #[serde(flatten)]
//     pub articles: BTreeMap<String, Article>,
//     // pub httpstatuscode: u16,
// }

/// Article
#[derive(Debug, Deserialize)]
#[must_use]
pub struct Article {
    pub articleid: String,
    pub designation: String,
    // pub unittype: String,
    pub prices: Vec<ArticlePrice>,
}

impl Article {
    /// Get price valid on given date
    #[must_use]
    pub fn price_valid_on(&self, date: NaiveDate) -> Option<f32> {
        self.prices
            .iter()
            .rev()
            .find(|p| date >= p.validfrom && date <= p.validto)
            .map(|p| p.unitprice)
    }
}

/// Article price
#[serde_as]
#[derive(Debug, Deserialize)]
#[must_use]
pub struct ArticlePrice {
    #[serde_as(as = "DisplayFromStr")]
    pub validfrom: NaiveDate, // "yyyy-mm-dd"
    #[serde_as(as = "DisplayFromStr")]
    pub validto: NaiveDate, // "yyyy-mm-dd"
    // #[serde_as(as = "DisplayFromStr")]
    // pub salestax: f32,
    #[serde_as(as = "DisplayFromStr")]
    pub unitprice: f32,
}
