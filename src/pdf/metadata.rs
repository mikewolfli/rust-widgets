//! PDF metadata, security, and pagination support.
pub struct PdfMetadata {
    /// Document title.
    pub title: String,
    /// Document author.
    pub author: String,
    /// Document subject.
    pub subject: String,
    /// Document keywords.
    pub keywords: Vec<String>,
    /// Application creating the source document.
    pub creator: String,
    /// PDF producer implementation name.
    pub producer: String,
    /// Creation timestamp string.
    pub creation_date: Option<String>,
    /// Last modification timestamp string.
    pub modification_date: Option<String>,
}
impl Default for PdfMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            subject: String::new(),
            keywords: Vec::new(),
            creator: "Rust Widgets PDF".to_string(),
            producer: "Rust Widgets PDF Library".to_string(),
            creation_date: None,
            modification_date: None,
        }
    }
}
