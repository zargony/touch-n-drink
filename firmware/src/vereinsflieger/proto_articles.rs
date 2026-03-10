use super::AccessToken;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

/// `articles/list` request
#[derive(Debug, Serialize)]
pub struct ArticleListRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

// /// `articles/list` response
// #[derive(Debug, Deserialize)]
// #[serde(transparent)]
// pub struct ArticleListResponse {
//     pub articles: BTreeMap<String, Article>,
//     // pub httpstatuscode: u16,
// }

/// Article
#[derive(Debug, Deserialize)]
pub struct Article {
    pub articleid: String,
    pub designation: String,
    // pub unittype: String,
    pub prices: Vec<ArticlePrice>,
}

impl Article {
    /// Get today's price
    pub fn price(&self) -> Option<f32> {
        // TODO: Get a current date and do a real price selection based on validity dates.
        // For now, we make sure to end up with the last entry valid until 9999-12-31, if any, or
        // any last entry otherwise.
        let price = self
            .prices
            .iter()
            .rev()
            .find(|p| p.validto == "9999-12-31")
            .or(self.prices.last());
        price.map(|p| p.unitprice)
    }
}

/// Article price
#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ArticlePrice {
    // pub validfrom: String, // "yyyy-mm-dd"
    pub validto: String, // "yyyy-mm-dd"
    // #[serde_as(as = "DisplayFromStr")]
    // pub salestax: f32,
    #[serde_as(as = "DisplayFromStr")]
    pub unitprice: f32,
}
