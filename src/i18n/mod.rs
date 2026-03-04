//! i18n module - internationalization support

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
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
}

impl I18nManager {
    /// Create a new i18n manager
    pub fn new() -> Self {
        Self {
            translations: HashMap::new(),
            current_language: "en".to_string(),
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
    
    /// Set current language
    pub fn set_language(&mut self, language: &str) {
        self.current_language = language.to_string();
    }
    
    /// Get current language
    pub fn current_language(&self) -> &String {
        &self.current_language
    }
    
    /// Translate a message
    pub fn translate(&self, key: &str) -> String {
        self.translate_with_context(key, None, 1)
    }
    
    /// Translate a message with context
    pub fn translate_with_context(&self, key: &str, context: Option<&str>, count: u32) -> String {
        if let Some(translation_file) = self.translations.get(&self.current_language) {
            if let Some(translation) = translation_file.translations.get(key) {
                // Check context
                if let Some(ctx) = context {
                    if let Some(trans_ctx) = &translation.context {
                        if trans_ctx != ctx {
                            return key.to_string();
                        }
                    } else {
                        return key.to_string();
                    }
                }
                
                // Check plural
                if let Some(plural) = &translation.plural {
                    if let Some(plural_form) = plural.get(&count) {
                        return plural_form.clone();
                    }
                }
                
                return translation.message.clone();
            }
        }
        
        // Fallback to key if translation not found
        key.to_string()
    }
}

/// tr! macro for translation
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::I18nManager::new().translate($key)
    };
    ($key:expr, $count:expr) => {
        $crate::i18n::I18nManager::new().translate_with_context($key, None, $count)
    };
    ($key:expr, $context:expr, $count:expr) => {
        $crate::i18n::I18nManager::new().translate_with_context($key, Some($context), $count)
    };
}

/// Global i18n manager instance using Mutex for thread-safe access
static GLOBAL_I18N: Mutex<Option<I18nManager>> = Mutex::new(None);

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

/// Initialize the i18n system
pub fn init() {
    let mut guard = GLOBAL_I18N.lock().expect("i18n lock poisoned");
    *guard = Some(I18nManager::new());
}

/// Initialize the i18n system with options
pub fn init_with_options(options: InitOptions) -> InitReport {
    let mut report = InitReport::new();
    let mut manager = I18nManager::new();
    manager.set_language(&options.language);
    
    // Load translations from directory if specified
    if let Some(dir) = options.preload_dir {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    match manager.load_translations(path.to_str().unwrap_or("")) {
                        Ok(()) => {
                            report.files_loaded += 1;
                            if options.diagnostics {
                                eprintln!("[i18n] Loaded translations from: {:?}", path);
                            }
                        }
                        Err(e) => {
                            report.errors.push(format!("Failed to load {:?}: {}", path, e));
                        }
                    }
                }
            }
        }
    }
    
    report.translations_count = manager.translations.len();
    let mut guard = GLOBAL_I18N.lock().expect("i18n lock poisoned");
    *guard = Some(manager);
    
    report
}

/// Translate a key to the current language
pub fn translate(key: &str) -> String {
    let mut guard = GLOBAL_I18N.lock().expect("i18n lock poisoned");
    if let Some(ref mut manager) = *guard {
        manager.translate(key)
    } else {
        key.to_string()
    }
}

/// Get the global i18n manager
pub fn get_manager() -> std::sync::MutexGuard<'static, Option<I18nManager>> {
    GLOBAL_I18N.lock().expect("i18n lock poisoned")
}
