use crate::vereinsflieger;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use chrono::NaiveDate;
use core::borrow::Borrow;
use log::warn;

/// Article id
/// Equivalent to the Vereinsflieger `articleid` attribute
pub type ArticleId = String;

/// Article information
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Article {
    pub name: String,
    pub price: f32,
}

/// Article lookup table
/// Provides a look up of article information (name and price) by index (0 = 1st article). The
/// list of article ids is given on initialization (from static system configuration), while
/// article information is fetched later from Vereinsflieger.
#[derive(Debug)]
#[must_use]
pub struct Articles {
    /// Look up index to article id
    ids: Vec<ArticleId>,
    /// Look up article id to article information
    articles: BTreeMap<ArticleId, Article>,
}

impl Articles {
    /// Create new article lookup table
    pub fn new(ids: Vec<ArticleId>) -> Self {
        Self {
            ids,
            articles: BTreeMap::new(),
        }
    }

    /// Clear all article information
    pub fn clear(&mut self) {
        self.articles.clear();
    }

    /// Update article with given article id. Ignores article ids not in list.
    pub fn update_article(&mut self, id: &ArticleId, name: &str, price: f32) {
        // Trim generic prefix from article name
        let name = name
            // TODO: get prefixes from system configuration
            .trim_start_matches("Getränke ")
            .trim_start_matches("Getränk ")
            .trim_start()
            .to_string();
        if self.ids.contains(id) {
            self.articles.insert(id.clone(), Article { name, price });
        }
    }

    /// Update article using the given Vereinsflieger article with price on given date
    pub fn update_vereinsflieger_article(
        &mut self,
        article: &vereinsflieger::Article,
        date: NaiveDate,
    ) {
        if let Some(price) = article.price_valid_on(date) {
            self.update_article(&article.articleid, &article.designation, price);
        } else {
            warn!(
                "Ignoring article with no valid price ({}): {}",
                article.articleid, article.designation
            );
        }
    }

    /// Number of articles available
    #[must_use]
    pub fn count(&self) -> usize {
        self.articles.len()
    }

    /// Iterate over articles in order given on initialization
    pub fn iter(&self) -> impl Iterator<Item = (&ArticleId, &Article)> {
        self.ids
            .iter()
            .filter_map(|id| self.get(id).map(|article| (id, article)))
    }

    /// Look up article by article id
    #[must_use]
    pub fn get<Q>(&self, id: &Q) -> Option<&Article>
    where
        ArticleId: Borrow<Q> + Ord,
        Q: Ord + ?Sized,
    {
        self.articles.get(id)
    }

    /// Look up article at given index
    #[must_use]
    pub fn get_by_index(&self, index: usize) -> Option<(&ArticleId, &Article)> {
        self.iter().nth(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn smoke() {
        let mut articles = Articles::new(vec!["1111".into(), "2222".into()]);
        articles.update_article(&"1111".into(), "Cold Drink", 11.11);
        articles.update_article(&"2222".into(), "Hot Drink", 22.22);
        assert_eq!(articles.count(), 2);
        assert_eq!(articles.get("1111").unwrap().name, "Cold Drink");
        assert_eq!(articles.get("1111").unwrap().price, 11.11);
        assert_eq!(articles.get("2222").unwrap().name, "Hot Drink");
        assert_eq!(articles.get("2222").unwrap().price, 22.22);
        assert_eq!(articles.get_by_index(0).unwrap().0, "1111");
        assert_eq!(articles.get_by_index(0).unwrap().1.name, "Cold Drink");
        assert_eq!(articles.get_by_index(1).unwrap().0, "2222");
        assert_eq!(articles.get_by_index(1).unwrap().1.name, "Hot Drink");
    }

    #[test]
    fn no_articles() {
        let mut articles = Articles::new(vec![]);
        articles.update_article(&"1111".into(), "Cold Drink", 11.11);
        articles.update_article(&"2222".into(), "Hot Drink", 22.22);
        assert_eq!(articles.count(), 0);
        assert_eq!(articles.get_by_index(0), None);
    }

    #[test]
    fn ignore_missing_and_unwanted() {
        let mut articles = Articles::new(vec!["9999".into(), "2222".into()]);
        articles.update_article(&"1111".into(), "Cold Drink", 11.11);
        articles.update_article(&"2222".into(), "Hot Drink", 22.22);
        assert_eq!(articles.count(), 1);
        assert_eq!(articles.get("1111"), None);
        assert_eq!(articles.get_by_index(0).unwrap().0, "2222");
        assert_eq!(articles.get_by_index(1), None);
    }
}
