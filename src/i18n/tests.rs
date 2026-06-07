//! i18n tests - unit tests for internationalization
use super::manager::I18nManager;
use super::options::{InitOptions, InitReport};
use super::types::{ReloadEvent, Translation, TranslationFile};
use crossbeam_channel::unbounded;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;
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
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello World"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");
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
            map.insert(
                "hello".to_string(),
                Translation { context: None, message: "Hello".to_string(), plural: None },
            );
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
            map.insert(
                "hello".to_string(),
                Translation { context: None, message: "Hello Updated".to_string(), plural: None },
            );
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
            map.insert(
                "test".to_string(),
                Translation { context: None, message: "Test".to_string(), plural: None },
            );
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
            map.insert(
                "test".to_string(),
                Translation { context: None, message: "Test Updated".to_string(), plural: None },
            );
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
