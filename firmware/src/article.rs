use alloc::string::String;

/// Article id
#[allow(clippy::module_name_repetitions)]
pub type ArticleId = u32;

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
pub struct Articles<const N: usize> {
    ids: [ArticleId; N],
    articles: [Option<Article>; N],
}

impl<const N: usize> Articles<N> {
    /// Create new article lookup table
    pub fn new(ids: [ArticleId; N]) -> Self {
        Self {
            ids,
            articles: [const { None }; N],
        }
    }

    /// Clear all article information
    pub fn clear(&mut self) {
        self.articles = [const { None }; N];
    }

    /// Update article with given article id. Ignores article ids not in list.
    pub fn update(&mut self, id: ArticleId, name: String, price: f32) {
        if let Some(idx) = self.ids.iter().position(|i| *i == id) {
            self.articles[idx] = Some(Article { name, price });
        }
    }

    /// Number of articles
    pub fn count(&self) -> usize {
        self.articles.iter().filter(|a| a.is_some()).count()
    }

    /// Look up id of article at given index
    #[allow(dead_code)]
    pub fn id(&self, index: usize) -> Option<ArticleId> {
        self.ids.get(index).copied()
    }

    /// Look up article information at given index
    pub fn get(&self, index: usize) -> Option<&Article> {
        match self.articles.get(index) {
            Some(Some(article)) => Some(article),
            _ => None,
        }
    }
}
