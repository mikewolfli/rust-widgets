//! i18n global - global functions and static instances
use crate::compat::{Mutex, MutexGuard};
use crate::i18n::manager::I18nManager;
use crate::i18n::options::{InitOptions, InitReport};
use crate::i18n::types::TranslationFile;
/// Embedded English translations as a compile-time fallback.
const EMBEDDED_EN_JSON: &str = include_str!("../../language/en.json");
/// Global i18n manager instance using Mutex for thread-safe access
pub(crate) static GLOBAL_I18N: Mutex<Option<I18nManager>> = Mutex::new(None);
/// Initialize the i18n system, loading the embedded English translations.
pub fn init() {
    let mut guard = GLOBAL_I18N.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut manager = I18nManager::new();
    // Load embedded en.json as the default locale
    match serde_json::from_str::<TranslationFile>(EMBEDDED_EN_JSON) {
        Ok(translation_file) => {
            let language = translation_file.language.clone();
            manager.set_language(&language);
            manager.inject_translations(language, translation_file);
            log::info!(
                "[i18n] Loaded embedded English translations ({} translation files)",
                manager.translation_count()
            );
        }
        Err(e) => {
            log::error!("[i18n] Failed to parse embedded en.json: {e}");
        }
    }
    *guard = Some(manager);
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
                    let path_str = match path.to_str() {
                        Some(p) => p,
                        None => {
                            report.errors.push(format!("Non-UTF-8 path: {path:?}"));
                            continue;
                        }
                    };
                    match manager.load_translations(path_str) {
                        Ok(()) => {
                            report.files_loaded += 1;
                            if diagnostics {
                                log::info!("[i18n] Loaded translations from: {path:?}");
                            }
                        }
                        Err(e) => {
                            report.errors.push(format!("Failed to load {path:?}: {e}"));
                        }
                    }
                }
            }
        }
    }
    report.translations_count = manager.translation_count();
    let mut guard = GLOBAL_I18N.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = Some(manager);
    report
}
/// Translate a key to the current language
pub fn translate(key: &str) -> String {
    let mut guard = GLOBAL_I18N.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(ref mut manager) = *guard {
        manager.translate(key)
    } else {
        key.to_string()
    }
}
/// Translate a key with optional context and plural count
pub fn translate_with_context(key: &str, context: Option<&str>, count: u32) -> String {
    let mut guard = GLOBAL_I18N.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(ref mut manager) = *guard {
        manager.translate_with_context(key, context, count)
    } else {
        key.to_string()
    }
}
/// Get the global i18n manager
pub fn get_manager() -> MutexGuard<'static, Option<I18nManager>> {
    GLOBAL_I18N.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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

/// Serializes tests that mutate the process-global i18n state.
///
/// `GLOBAL_I18N` is a process-wide static, and Rust runs `#[test]` functions on
/// parallel threads. Tests that call `init()` / `init_with_options()` /
/// `reset` race with each other; holding this lock keeps them sequential.
#[cfg(test)]
pub(crate) fn global_i18n_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_init_loads_english() {
        let _lock = global_i18n_test_lock();
        init();
        let count = {
            let mgr = get_manager();
            mgr.as_ref().map(|m| m.translation_count()).unwrap_or(0)
        };
        assert!(count > 0, "init should load English translations");
        // Reset global state so other tests don't see loaded translations
        // (drop the manager guard first, then acquire the lock again for reset)
        *GLOBAL_I18N.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}
