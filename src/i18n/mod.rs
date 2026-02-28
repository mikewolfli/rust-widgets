//! Internationalization support.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::Read;
use std::sync::Mutex;

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

/// i18n manager
pub struct I18nManager {
    translations: HashMap<String, TranslationFile>,
    current_language: String,
    default_language: String,
}

impl I18nManager {
    /// Create a new i18n manager
    pub fn new() -> Self {
        Self {
            translations: HashMap::new(),
            current_language: "en".to_string(),
            default_language: "en".to_string(),
        }
    }
    
    /// Load translations from file
    pub fn load_translations(&mut self, path: &str) -> Result<(), std::io::Error> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        
        let translation_file: TranslationFile = serde_json::from_str(&content)?;
        self.translations.insert(translation_file.language.clone(), translation_file);
        
        Ok(())
    }
    
    /// Load translations from directory
    pub fn load_translations_from_dir(&mut self, dir_path: &str) -> Result<(), std::io::Error> {
        let dir = read_dir(dir_path)?;
        
        for entry in dir {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().unwrap_or_default() == "json" {
                self.load_translations(path.to_str().unwrap())?;
            }
        }
        
        Ok(())
    }
    
    /// Set current language
    pub fn set_language(&mut self, language: &str) {
        self.current_language = language.to_string();
    }
    
    /// Get current language
    pub fn current_language(&self) -> &String {
        &self.current_language
    }
    
    /// Set default language
    pub fn set_default_language(&mut self, language: &str) {
        self.default_language = language.to_string();
    }
    
    /// Get default language
    pub fn default_language(&self) -> &String {
        &self.default_language
    }
    
    /// Get supported languages
    pub fn supported_languages(&self) -> Vec<&String> {
        self.translations.keys().collect()
    }
    
    /// Translate a message
    pub fn translate(&self, key: &str) -> String {
        self.translate_with_context(key, None, 1)
    }
    
    /// Translate a message with context
    pub fn translate_with_context(&self, key: &str, context: Option<&str>, count: u32) -> String {
        // Try current language first
        if let Some(translation) = self.get_translation(self.current_language(), key, context) {
            return self.get_plural_form(translation, count);
        }
        
        // Fallback to default language
        if self.current_language() != self.default_language() {
            if let Some(translation) = self.get_translation(self.default_language(), key, context) {
                return self.get_plural_form(translation, count);
            }
        }
        
        // Fallback to key if translation not found
        key.to_string()
    }
    
    /// Get translation for a specific language
    fn get_translation(&self, language: &str, key: &str, context: Option<&str>) -> Option<&Translation> {
        if let Some(translation_file) = self.translations.get(language) {
            if let Some(ctx) = context {
                let ctx_key = format!("{}::{}", ctx, key);
                if let Some(translation) = translation_file.translations.get(&ctx_key) {
                    return Some(translation);
                }
            }

            if let Some(translation) = translation_file.translations.get(key) {
                return Some(translation);
            }
        }
        None
    }
    
    /// Get plural form based on count
    fn get_plural_form(&self, translation: &Translation, count: u32) -> String {
        if let Some(plural) = &translation.plural {
            if let Some(plural_form) = plural.get(&count) {
                return plural_form.clone();
            }
            if count > 1 {
                if let Some(default_plural) = plural.get(&2) {
                    return default_plural.clone();
                }
            }
        }
        translation.message.clone()
    }
}

// Global i18n manager instance
lazy_static::lazy_static! {
    pub static ref I18N_MANAGER: Mutex<I18nManager> = Mutex::new(I18nManager::new());
}

/// Initialize i18n system
pub fn init() {
    // Default initialization
}

/// Load translations from file
pub fn load_translations(path: &str) -> Result<(), std::io::Error> {
    I18N_MANAGER.lock().unwrap().load_translations(path)
}

/// Load translations from directory
pub fn load_translations_from_dir(dir_path: &str) -> Result<(), std::io::Error> {
    I18N_MANAGER.lock().unwrap().load_translations_from_dir(dir_path)
}

/// Set current language
pub fn set_language(language: &str) {
    I18N_MANAGER.lock().unwrap().set_language(language);
}

/// Get current language
pub fn current_language() -> String {
    I18N_MANAGER.lock().unwrap().current_language().clone()
}

/// Set default language
pub fn set_default_language(language: &str) {
    I18N_MANAGER.lock().unwrap().set_default_language(language);
}

/// Get supported languages
pub fn supported_languages() -> Vec<String> {
    I18N_MANAGER.lock().unwrap().supported_languages().iter().map(|&s| s.clone()).collect()
}

/// Translate a message
pub fn translate(key: &str) -> String {
    I18N_MANAGER.lock().unwrap().translate(key)
}

/// Translate a message with context
pub fn translate_with_context(key: &str, context: Option<&str>, count: u32) -> String {
    I18N_MANAGER.lock().unwrap().translate_with_context(key, context, count)
}

/// Translation macro with optional count and context.
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $count:expr) => {
        $crate::i18n::translate_with_context($key, None, $count)
    };
    ($key:expr, $context:expr, $count:expr) => {
        $crate::i18n::translate_with_context($key, Some($context), $count)
    };
}
