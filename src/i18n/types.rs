//! i18n types - data structures for internationalization
use crate::compat::HashMap;
use serde::{Deserialize, Serialize};
/// Translation entry
#[derive(Debug, Serialize, Deserialize)]
pub struct Translation {
    pub context: Option<String>,
    pub message: String,
    pub plural: Option<HashMap<u32, String>>,
}
/// Translation file
#[derive(Debug, Serialize, Deserialize)]
pub struct TranslationFile {
    pub language: String,
    pub translations: HashMap<String, Translation>,
}
/// Reload event notification
#[derive(Debug, Clone)]
pub enum ReloadEvent {
    /// Translation file was reloaded
    TranslationReloaded { language: String, timestamp: std::time::SystemTime },
    /// Error during reload
    ReloadError { language: String, error: String },
}
