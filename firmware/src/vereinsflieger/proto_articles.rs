use super::AccessToken;
use crate::article::Articles;
use crate::json::{self, FromJsonObject, ToJson};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::str::FromStr;
use embedded_io_async::{BufRead, Write};
use log::warn;

/// `articles/list` request
#[derive(Debug)]
pub struct ArticleListRequest<'a> {
    pub accesstoken: &'a AccessToken,
}

impl<'a> ToJson for ArticleListRequest<'a> {
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

/// `articles/list` response
#[derive(Debug, Default)]
pub struct ArticleListResponse<const N: usize> {
    // pub *: Article,
    // pub httpstatuscode: u16,
    //
    /// Total number of articles
    pub total_articles: u32,
}

impl<const N: usize> FromJsonObject for ArticleListResponse<N> {
    // Mutable reference to article lookup table
    type Context<'ctx> = RefCell<&'ctx mut Articles<N>>;

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match u32::from_str(&key) {
            Ok(_key) => {
                let article: Article = json.read().await?;
                self.total_articles += 1;
                if let Some(price) = article.price() {
                    // Instead of reading all articles to a vector, this deserialization stores
                    // articles directly to the article lookup table and only keeps the articles
                    // needed, which heavily reduces memory consumption.
                    let mut articles = context.borrow_mut();
                    articles.update(&article.articleid, article.designation, price);
                } else {
                    warn!(
                        "Ignoring article with no valid price ({}): {}",
                        article.articleid, article.designation
                    );
                }
            }
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}

/// Article
#[derive(Debug, Default)]
pub struct Article {
    pub articleid: String,
    pub designation: String,
    pub unittype: String,
    pub prices: Vec<ArticlePrice>,
}

impl FromJsonObject for Article {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "articleid" => self.articleid = json.read().await?,
            "designation" => self.designation = json.read().await?,
            "unittype" => self.unittype = json.read().await?,
            "prices" => self.prices = json.read().await?,
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
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
#[derive(Debug, Default)]
pub struct ArticlePrice {
    pub validfrom: String, // "yyyy-mm-dd"
    pub validto: String,   // "yyyy-mm-dd"
    pub salestax: f32,
    pub unitprice: f32,
}

impl FromJsonObject for ArticlePrice {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "validfrom" => self.validfrom = json.read().await?,
            "validto" => self.validto = json.read().await?,
            "salestax" => self.salestax = json.read_any().await?.try_into()?,
            "unitprice" => self.unitprice = json.read_any().await?.try_into()?,
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}
