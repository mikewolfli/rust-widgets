//! i18n module - internationalization support with hot reload

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;
use crossbeam_channel::{Sender, Receiver, unbounded};
use notify::{Watcher, RecursiveMode, Event, EventKind};

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

/// i18n manager with hot reload support
pub struct I18nManager {
    translations: HashMap<String, TranslationFile>,
    current_language: String,
    translation_paths: HashMap<String, PathBuf>,
    file_modification_times: HashMap<String, SystemTime>,
    hot_reload_enabled: bool,
    reload_sender: Option<Sender<ReloadEvent>>,
}

/// Reload event notification
#[derive(Debug, Clone)]
pub enum ReloadEvent {
    /// Translation file was reloaded
    TranslationReloaded {
        language: String,
        timestamp: SystemTime,
    },
    /// Error during reload
    ReloadError {
        language: String,
        error: String,
    },
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
            let mut file = File::open(path)
                .map_err(|e| format!("Failed to open file: {}", e))?;
            
            let mut content = String::new();
            file.read_to_string(&mut content)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            
            let translation_file: TranslationFile = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;
            
            self.translations.insert(language.to_string(), translation_file);
            
            if let Some(modified) = File::open(path).ok().and_then(|f| f.metadata().ok()).and_then(|m| m.modified().ok()) {
                self.file_modification_times.insert(language.to_string(), modified);
            }
            
            if let Some(ref sender) = self.reload_sender {
                let _ = sender.send(ReloadEvent::TranslationReloaded {
                    language: language.to_string(),
                    timestamp: SystemTime::now(),
                });
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
                    events.push(ReloadEvent::ReloadError {
                        language,
                        error: e,
                    });
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

impl Default for InitReport {
    fn default() -> Self {
        Self::new()
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
    
    let diagnostics = options.diagnostics;
    
    // Load translations from directory if specified
    if let Some(dir) = options.preload_dir {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    match manager.load_translations(path.to_str().unwrap_or("")) {
                        Ok(()) => {
                            report.files_loaded += 1;
                            if diagnostics {
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

/// File watcher for hot reload
pub struct I18nFileWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    reload_sender: Sender<ReloadEvent>,
    reload_receiver: Receiver<ReloadEvent>,
}

impl I18nFileWatcher {
    /// Create a new file watcher
    pub fn new() -> Self {
        let (reload_sender, reload_receiver) = unbounded();
        Self {
            watcher: None,
            reload_sender,
            reload_receiver,
        }
    }
    
    /// Start watching a directory for translation file changes
    pub fn watch_directory(&mut self, dir: &Path) -> Result<(), String> {
        let sender = self.reload_sender.clone();
        
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        if let Some(path) = event.paths.first() {
                            if path.extension().is_some_and(|ext| ext == "json") {
                                if let Some(lang) = path.file_stem().and_then(|s| s.to_str()) {
                                    let _ = sender.send(ReloadEvent::TranslationReloaded {
                                        language: lang.to_string(),
                                        timestamp: SystemTime::now(),
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[i18n] Watcher error: {:?}", e);
                }
            }
        }).map_err(|e| format!("Failed to create watcher: {}", e))?;
        
        watcher.watch(dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;
        
        self.watcher = Some(watcher);
        Ok(())
    }
    
    /// Get the reload event receiver
    pub fn receiver(&self) -> &Receiver<ReloadEvent> {
        &self.reload_receiver
    }
    
    /// Get the reload event sender for use with I18nManager
    pub fn sender(&self) -> &Sender<ReloadEvent> {
        &self.reload_sender
    }
    
    /// Enable hot reload on the global i18n manager
    pub fn enable_hot_reload(&self) {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            manager.enable_hot_reload(self.reload_sender.clone());
        }
    }
    
    /// Disable hot reload on the global i18n manager
    pub fn disable_hot_reload() {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            manager.disable_hot_reload();
        }
    }
    
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled() -> bool {
        let guard = get_manager();
        guard.as_ref().map(|m| m.is_hot_reload_enabled()).unwrap_or(false)
    }
}

impl Default for I18nFileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize i18n with hot reload support
pub fn init_with_hot_reload(options: InitOptions, watch_dir: Option<&Path>) -> (InitReport, I18nFileWatcher) {
    let diagnostics = options.diagnostics;
    let mut report = init_with_options(options);
    let mut file_watcher = I18nFileWatcher::new();
    
    if let Some(dir) = watch_dir {
        if let Err(e) = file_watcher.watch_directory(dir) {
            report.errors.push(format!("Failed to setup file watcher: {}", e));
        } else {
            file_watcher.enable_hot_reload();
            if diagnostics {
                eprintln!("[i18n] Hot reload enabled for directory: {:?}", dir);
            }
        }
    }
    
    (report, file_watcher)
}

/// Process reload events and update translations
pub fn process_reload_events(receiver: &Receiver<ReloadEvent>) -> Vec<ReloadEvent> {
    let mut events = Vec::new();
    
    while let Ok(event) = receiver.try_recv() {
        match &event {
            ReloadEvent::TranslationReloaded { language, .. } => {
                let mut guard = get_manager();
                if let Some(ref mut manager) = *guard {
                    if let Err(e) = manager.reload_translation(language) {
                        events.push(ReloadEvent::ReloadError {
                            language: language.clone(),
                            error: e,
                        });
                    } else {
                        events.push(event);
                    }
                }
            }
            ReloadEvent::ReloadError { .. } => {
                events.push(event);
            }
        }
    }
    
    events
}

/// Check and reload all modified translation files
pub fn check_and_reload_all() -> Vec<ReloadEvent> {
    let mut guard = get_manager();
    if let Some(ref mut manager) = *guard {
        manager.check_and_reload()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_i18n_manager_basic() {
        let mut manager = I18nManager::new();
        manager.set_language("en");
        
        assert_eq!(manager.current_language(), "en");
        assert_eq!(manager.translate("hello"), "hello");
    }

    #[test]
    fn test_i18n_manager_with_translations() {
        let mut manager = I18nManager::new();
        
        let translation = Translation {
            context: None,
            message: "Hello World".to_string(),
            plural: None,
        };
        
        let mut translations = HashMap::new();
        translations.insert("hello".to_string(), translation);
        
        let translation_file = TranslationFile {
            language: "en".to_string(),
            translations,
        };
        
        manager.translations.insert("en".to_string(), translation_file);
        
        assert_eq!(manager.translate("hello"), "Hello World");
    }

    #[test]
    fn test_i18n_manager_hot_reload() {
        let (sender, _receiver) = unbounded();
        let mut manager = I18nManager::new();
        manager.enable_hot_reload(sender);
        
        assert!(manager.is_hot_reload_enabled());
        
        manager.disable_hot_reload();
        assert!(!manager.is_hot_reload_enabled());
    }

    #[test]
    fn test_i18n_manager_reload_translation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("en.json");
        
        let translation_file = TranslationFile {
            language: "en".to_string(),
            translations: {
                let mut map = HashMap::new();
                map.insert("hello".to_string(), Translation {
                    context: None,
                    message: "Hello".to_string(),
                    plural: None,
                });
                map
            },
        };
        
        fs::write(&file_path, serde_json::to_string(&translation_file).unwrap()).unwrap();
        
        let mut manager = I18nManager::new();
        manager.load_translations(file_path.to_str().unwrap()).unwrap();
        
        assert_eq!(manager.translate("hello"), "Hello");
        
        let updated_file = TranslationFile {
            language: "en".to_string(),
            translations: {
                let mut map = HashMap::new();
                map.insert("hello".to_string(), Translation {
                    context: None,
                    message: "Hello Updated".to_string(),
                    plural: None,
                });
                map
            },
        };
        
        fs::write(&file_path, serde_json::to_string(&updated_file).unwrap()).unwrap();
        
        let result = manager.reload_translation("en");
        assert!(result.is_ok());
        assert_eq!(manager.translate("hello"), "Hello Updated");
    }

    #[test]
    fn test_i18n_manager_check_and_reload() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("en.json");
        
        let translation_file = TranslationFile {
            language: "en".to_string(),
            translations: {
                let mut map = HashMap::new();
                map.insert("test".to_string(), Translation {
                    context: None,
                    message: "Test".to_string(),
                    plural: None,
                });
                map
            },
        };
        
        fs::write(&file_path, serde_json::to_string(&translation_file).unwrap()).unwrap();
        
        let (sender, _receiver) = unbounded();
        let mut manager = I18nManager::new();
        manager.enable_hot_reload(sender);
        manager.load_translations(file_path.to_str().unwrap()).unwrap();
        
        assert_eq!(manager.translate("test"), "Test");
        
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let updated_file = TranslationFile {
            language: "en".to_string(),
            translations: {
                let mut map = HashMap::new();
                map.insert("test".to_string(), Translation {
                    context: None,
                    message: "Test Updated".to_string(),
                    plural: None,
                });
                map
            },
        };
        
        fs::write(&file_path, serde_json::to_string(&updated_file).unwrap()).unwrap();
        
        let events = manager.check_and_reload();
        assert!(!events.is_empty());
        assert_eq!(manager.translate("test"), "Test Updated");
    }

    #[test]
    fn test_init_options() {
        let options = InitOptions {
            language: "zh".to_string(),
            preload_dir: Some("/path/to/translations".to_string()),
            diagnostics: true,
        };
        
        assert_eq!(options.language, "zh");
        assert_eq!(options.preload_dir, Some("/path/to/translations".to_string()));
        assert!(options.diagnostics);
    }

    #[test]
    fn test_init_options_default() {
        let options = InitOptions::default();
        
        assert_eq!(options.language, "en");
        assert_eq!(options.preload_dir, None);
        assert!(!options.diagnostics);
    }

    #[test]
    fn test_init_report() {
        let report = InitReport::new();
        
        assert_eq!(report.files_loaded, 0);
        assert_eq!(report.translations_count, 0);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_reload_event() {
        let event = ReloadEvent::TranslationReloaded {
            language: "en".to_string(),
            timestamp: SystemTime::now(),
        };
        
        match event {
            ReloadEvent::TranslationReloaded { language, .. } => {
                assert_eq!(language, "en");
            }
            _ => panic!("Expected TranslationReloaded event"),
        }
    }

    #[test]
    fn test_reload_error_event() {
        let event = ReloadEvent::ReloadError {
            language: "en".to_string(),
            error: "Test error".to_string(),
        };
        
        match event {
            ReloadEvent::ReloadError { language, error } => {
                assert_eq!(language, "en");
                assert_eq!(error, "Test error");
            }
            _ => panic!("Expected ReloadError event"),
        }
    }

    #[test]
    fn test_file_watcher() {
        let watcher = I18nFileWatcher::new();
        
        assert!(watcher.watcher.is_none());
        assert_eq!(watcher.receiver().try_recv().is_err(), true);
    }

    #[test]
    fn test_file_watcher_default() {
        let watcher = I18nFileWatcher::default();
        
        assert!(watcher.watcher.is_none());
    }
}
