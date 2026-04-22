//! i18n global - global functions and static instances

use crate::i18n::manager::I18nManager;
use crate::i18n::options::{InitOptions, InitReport};
use std::sync::Mutex;

/// Global i18n manager instance using Mutex for thread-safe access
static GLOBAL_I18N: Mutex<Option<I18nManager>> = Mutex::new(None);

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
                            report
                                .errors
                                .push(format!("Failed to load {:?}: {}", path, e));
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

/// Check and reload all modified translation files
pub fn check_and_reload_all() -> Vec<crate::i18n::types::ReloadEvent> {
    let mut guard = get_manager();
    if let Some(ref mut manager) = *guard {
        manager.check_and_reload()
    } else {
        Vec::new()
    }
}
