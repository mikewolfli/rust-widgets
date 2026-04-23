//! i18n options - configuration and initialization types
/// Initialization options for i18n system
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Language code (e.g., "en", "zh", "ja")
    pub language: String,
    /// Directory containing translation files
    pub preload_dir: Option<String>,
    /// Enable diagnostics output
    pub diagnostics: bool,
}
impl Default for InitOptions {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            preload_dir: None,
            diagnostics: false,
        }
    }
}
/// Initialization report
#[derive(Debug, Clone)]
pub struct InitReport {
    /// Number of translation files loaded
    pub files_loaded: usize,
    /// Total number of translations
    pub translations_count: usize,
    /// Any errors that occurred during initialization
    pub errors: Vec<String>,
}
impl InitReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            files_loaded: 0,
            translations_count: 0,
            errors: Vec::new(),
        }
    }
}
impl Default for InitReport {
    fn default() -> Self {
        Self::new()
    }
}
