use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Article id
/// Equivalent to the Vereinsflieger `articleid` attribute
#[allow(clippy::module_name_repetitions)]
pub type ArticleId = String;

/// Article information
#[derive(Debug, Clone, PartialEq)]
pub struct Article {
    // pub id: ArticleId,
    pub name: String,
    pub price: f32,
}

/// Article lookup table
/// Provides a look up of article information (name and price) by index (0 = 1st article). The
/// list of article ids is given on initialization (from static system configuration), while
/// article information is fetched later from Vereinsflieger.
#[derive(Debug)]
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
    pub fn update(&mut self, id: &ArticleId, name: String, price: f32) {
        if self.ids.contains(id) {
            self.articles.insert(id.clone(), Article { name, price });
        }
    }

    /// Number of ids
    pub fn count_ids(&self) -> usize {
        self.ids.len()
    }

    /// Number of articles
    pub fn count(&self) -> usize {
        self.articles.len()
    }

    /// Iterate over articles in order given on initialization
    pub fn iter(&self) -> impl Iterator<Item = (usize, &ArticleId, &Article)> {
        self.ids
            .iter()
            .enumerate()
            .filter_map(|(idx, id)| self.get(id).map(|article| (idx, id, article)))
    }

    /// Look up id of article at given index
    pub fn id(&self, index: usize) -> Option<&ArticleId> {
        self.ids.get(index)
    }

    /// Look up article by article id
    pub fn get(&self, id: &ArticleId) -> Option<&Article> {
        self.articles.get(id)
    }
}
