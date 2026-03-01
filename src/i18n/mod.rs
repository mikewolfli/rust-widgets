//! Internationalization support.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{File, read_dir};
use std::io::Read;
use std::sync::Mutex;

/// Translation entry for one key (optionally context and plural forms).
#[derive(Debug, Serialize, Deserialize)]
pub struct Translation {
    /// Optional translation context namespace.
    pub context: Option<String>,
    /// Base translated message.
    pub message: String,
    /// Optional plural forms keyed by count category.
    pub plural: Option<HashMap<u32, String>>,
}

/// Translation file grouped by language code.
#[derive(Debug, Serialize, Deserialize)]
pub struct TranslationFile {
    /// Language code (for example `en` or `zh-CN`).
    pub language: String,
    /// Translation entries keyed by message id.
    pub translations: HashMap<String, Translation>,
}

/// Runtime translation manager with language fallback.
pub struct I18nManager {
    translations: HashMap<String, TranslationFile>,
    current_language: String,
    default_language: String,
}

/// Startup options used to initialize i18n behavior deterministically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOptions {
    /// Preferred language used for both current/default language slots.
    pub language: String,
    /// Optional directory containing language JSON files to preload.
    pub preload_dir: Option<String>,
    /// Emit diagnostics to stderr.
    pub diagnostics: bool,
}

impl InitOptions {
    /// Build options from process environment.
    pub fn from_env() -> Self {
        let language = detect_language_from_env().unwrap_or_else(|| "en".to_string());
        let preload_dir = env::var("RUST_WIDGETS_I18N_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let diagnostics = env_flag_enabled("RUST_WIDGETS_I18N_DIAGNOSTICS");
        Self {
            language,
            preload_dir,
            diagnostics,
        }
    }
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

/// Result summary returned by i18n initialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    /// Effective language set for current/default slots.
    pub language: String,
    /// Optional preload directory used.
    pub preload_dir: Option<String>,
    /// Number of translation files loaded during init.
    pub loaded_languages: usize,
    /// Optional preload error if loading failed.
    pub preload_error: Option<String>,
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

// Global i18n manager instance used by top-level helper functions.
lazy_static::lazy_static! {
    /// Global i18n manager instance used by top-level helpers.
    pub static ref I18N_MANAGER: Mutex<I18nManager> = Mutex::new(I18nManager::new());
}

/// Initialize i18n system with deterministic defaults and environment-based preload.
///
/// Behavior:
/// - Resolves language from `RUST_WIDGETS_I18N_LANG`, then `LC_ALL`, then `LANG`, falling back to `en`.
/// - Applies resolved language to both current and default language.
/// - If `RUST_WIDGETS_I18N_DIR` is set, attempts to preload all `*.json` translation files.
/// - If `RUST_WIDGETS_I18N_DIAGNOSTICS` is truthy (`1/true/yes/on`), prints initialization diagnostics.
pub fn init() {
    let _ = init_with_options(InitOptions::from_env());
}

/// Initialize i18n with explicit options and return initialization report.
pub fn init_with_options(options: InitOptions) -> InitReport {
    let mut manager = I18N_MANAGER.lock().unwrap();
    manager.set_default_language(&options.language);
    manager.set_language(&options.language);

    let mut preload_error = None;
    if let Some(dir) = &options.preload_dir {
        if let Err(error) = manager.load_translations_from_dir(dir) {
            preload_error = Some(error.to_string());
        }
    }

    let report = InitReport {
        language: options.language,
        preload_dir: options.preload_dir,
        loaded_languages: manager.translations.len(),
        preload_error,
    };

    if options.diagnostics {
        emit_init_diagnostics(&report);
    }

    report
}

fn emit_init_diagnostics(report: &InitReport) {
    match &report.preload_dir {
        Some(dir) => {
            eprintln!(
                "[rust_widgets::i18n] init language={} preload_dir={} loaded_languages={} preload_error={}",
                report.language,
                dir,
                report.loaded_languages,
                report.preload_error.as_deref().unwrap_or("none")
            );
        }
        None => {
            eprintln!(
                "[rust_widgets::i18n] init language={} preload_dir=none loaded_languages={} preload_error={}",
                report.language,
                report.loaded_languages,
                report.preload_error.as_deref().unwrap_or("none")
            );
        }
    }
}

fn detect_language_from_env() -> Option<String> {
    ["RUST_WIDGETS_I18N_LANG", "LC_ALL", "LANG"]
        .iter()
        .filter_map(|key| env::var(key).ok())
        .find_map(|value| normalize_language_tag(&value))
}

fn env_flag_enabled(key: &str) -> bool {
    env::var(key)
        .ok()
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn normalize_language_tag(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_encoding = trimmed.split('.').next().unwrap_or(trimmed);
    let without_modifier = without_encoding.split('@').next().unwrap_or(without_encoding);
    let normalized = without_modifier.replace('_', "-");

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_language_tag_removes_encoding_and_modifier() {
        assert_eq!(normalize_language_tag("zh_CN.UTF-8"), Some("zh-CN".to_string()));
        assert_eq!(normalize_language_tag("en_US@POSIX"), Some("en-US".to_string()));
        assert_eq!(normalize_language_tag("  fr-FR  "), Some("fr-FR".to_string()));
        assert_eq!(normalize_language_tag(""), None);
    }

    #[test]
    fn init_with_options_applies_language_without_preload() {
        let report = init_with_options(InitOptions {
            language: "de-DE".to_string(),
            preload_dir: None,
            diagnostics: false,
        });

        assert_eq!(report.language, "de-DE");
        assert_eq!(report.preload_dir, None);
    }

    #[test]
    fn init_with_options_reports_preload_errors_deterministically() {
        let report = init_with_options(InitOptions {
            language: "en".to_string(),
            preload_dir: Some("/path/that/does/not/exist".to_string()),
            diagnostics: false,
        });

        assert_eq!(report.language, "en");
        assert_eq!(report.preload_dir, Some("/path/that/does/not/exist".to_string()));
        assert!(report.preload_error.is_some());
    }
}
