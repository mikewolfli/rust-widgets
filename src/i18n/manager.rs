//! i18n manager - core internationalization management
use crate::i18n::types::{ReloadEvent, TranslationFile};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;
/// i18n manager with hot reload support
pub struct I18nManager {
    translations: HashMap<String, TranslationFile>,
    current_language: String,
    translation_paths: HashMap<String, PathBuf>,
    file_modification_times: HashMap<String, SystemTime>,
    hot_reload_enabled: bool,
    reload_sender: Option<Sender<ReloadEvent>>,
}
impl I18nManager {
    /// Create a new i18n manager
    pub fn new() -> Self {
        Self {
            translations: HashMap::new(),
            current_language: "en".to_string(),
            translation_paths: HashMap::new(),
            file_modification_times: HashMap::new(),
            hot_reload_enabled: false,
            reload_sender: None,
        }
    }
    /// Enable hot reload functionality
    pub fn enable_hot_reload(&mut self, sender: Sender<ReloadEvent>) {
        self.hot_reload_enabled = true;
        self.reload_sender = Some(sender);
    }
    /// Disable hot reload functionality
    pub fn disable_hot_reload(&mut self) {
        self.hot_reload_enabled = false;
        self.reload_sender = None;
    }
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled(&self) -> bool {
        self.hot_reload_enabled
    }
    /// Reload a specific translation file
    pub fn reload_translation(&mut self, language: &str) -> Result<(), String> {
        if let Some(path) = self.translation_paths.get(language) {
            let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
            let mut content = String::new();
            file.read_to_string(&mut content).map_err(|e| format!("Failed to read file: {}", e))?;
            let translation_file: TranslationFile = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;
            self.translations.insert(language.to_string(), translation_file);
            if let Some(modified) = File::open(path)
                .ok()
                .and_then(|f| f.metadata().ok())
                .and_then(|m| m.modified().ok())
            {
                self.file_modification_times.insert(language.to_string(), modified);
            }
            if let Some(ref sender) = self.reload_sender {
                if let Err(e) = sender.send(ReloadEvent::TranslationReloaded {
                    language: language.to_string(),
                    timestamp: SystemTime::now(),
                }) {
                    log::error!("[i18n] Failed to send reload event: {:?}", e);
                }
            }
            Ok(())
        } else {
            Err(format!("Translation file path not found for language: {}", language))
        }
    }
    /// Check and reload all modified translation files
    pub fn check_and_reload(&mut self) -> Vec<ReloadEvent> {
        let mut events = Vec::new();
        if !self.hot_reload_enabled {
            return events;
        }
        let mut languages_to_reload: Vec<String> = Vec::new();
        for (language, path) in self.translation_paths.iter() {
            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    if let Some(last_modified) = self.file_modification_times.get(language) {
                        if modified > *last_modified {
                            languages_to_reload.push(language.clone());
                        }
                    }
                }
            }
        }
        for language in languages_to_reload {
            match self.reload_translation(&language) {
                Ok(()) => {
                    events.push(ReloadEvent::TranslationReloaded {
                        language,
                        timestamp: SystemTime::now(),
                    });
                }
                Err(e) => {
                    events.push(ReloadEvent::ReloadError { language, error: e });
                }
            }
        }
        events
    }
    /// Load translations from file
    pub fn load_translations(&mut self, path: &str) -> Result<(), std::io::Error> {
        let path_buf = PathBuf::from(path);
        let mut file = File::open(&path_buf)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let translation_file: TranslationFile = serde_json::from_str(&content)?;
        let language = translation_file.language.clone();
        self.translations.insert(language.clone(), translation_file);
        self.translation_paths.insert(language.clone(), path_buf);
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                self.file_modification_times.insert(language, modified);
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
    /// Return all unique translation keys across all loaded languages.
    pub fn audit_keys(&self) -> Vec<String> {
        let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for file in self.translations.values() {
            for key in file.translations.keys() {
                keys.insert(key.clone());
            }
        }
        keys.into_iter().collect()
    }

    /// Return loaded translation file count.
    pub fn translation_count(&self) -> usize {
        self.translations.len()
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
impl Default for I18nManager {
    fn default() -> Self {
        Self::new()
    }
}
