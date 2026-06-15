//! i18n tests - unit tests for internationalization
use super::global;
use super::manager::I18nManager;
use super::options::{InitOptions, InitReport};
use super::types::{ReloadEvent, Translation, TranslationFile};
use crate::compat::HashMap;
use crossbeam_channel::unbounded;
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

// ── R9.3 i18n comprehensive tests ──

#[test]
fn i18n_manager_translate_exact() {
    // Basic translation lookup with loaded translations
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello World"},
            "goodbye": {"message": "Goodbye"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");
    assert_eq!(manager.translate("hello"), "Hello World");
    assert_eq!(manager.translate("goodbye"), "Goodbye");
}

#[test]
fn i18n_manager_translate_fallback() {
    // Missing key returns the key itself
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
    assert_eq!(manager.translate("nonexistent"), "nonexistent");
    assert_eq!(manager.translate(""), "");
}

#[test]
fn i18n_manager_set_language() {
    // Switching language changes the translation output
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let en_path = temp_dir.path().join("en.json");
    let fr_path = temp_dir.path().join("fr.json");
    let en_json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello"}
        }
    }"#;
    let fr_json = r#"{
        "language": "fr",
        "translations": {
            "hello": {"message": "Bonjour"}
        }
    }"#;
    fs::write(&en_path, en_json).unwrap();
    fs::write(&fr_path, fr_json).unwrap();
    manager.load_translations(en_path.to_str().unwrap()).unwrap();
    manager.load_translations(fr_path.to_str().unwrap()).unwrap();

    manager.set_language("en");
    assert_eq!(manager.translate("hello"), "Hello");
    assert_eq!(manager.current_language(), "en");

    manager.set_language("fr");
    assert_eq!(manager.translate("hello"), "Bonjour");
    assert_eq!(manager.current_language(), "fr");

    // Language without loaded translations falls back to key
    manager.set_language("de");
    assert_eq!(manager.translate("hello"), "hello");
}

#[test]
fn i18n_manager_context_matching() {
    // Context matching works correctly
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "greeting": {
                "message": "Hello",
                "context": "formal"
            },
            "casual_greeting": {
                "message": "Hey",
                "context": "casual"
            },
            "plain": {
                "message": "No context"
            }
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");

    // Matching context returns the message
    assert_eq!(manager.translate_with_context("greeting", Some("formal"), 1), "Hello");
    // Non-matching context returns the key
    assert_eq!(manager.translate_with_context("greeting", Some("casual"), 1), "greeting");
    // Key without context, no context requested returns message
    assert_eq!(manager.translate_with_context("plain", None, 1), "No context");
    // Key without context, context requested returns key
    assert_eq!(manager.translate_with_context("plain", Some("anything"), 1), "plain");
}

#[test]
fn i18n_manager_plural_forms() {
    // Plural resolution works correctly
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "item": {
                "message": "items",
                "plural": {
                    "1": "item",
                    "2": "items",
                    "5": "many items"
                }
            },
            "simple": {
                "message": "simple"
            }
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");

    // Exact plural match
    assert_eq!(manager.translate_with_context("item", None, 1), "item");
    assert_eq!(manager.translate_with_context("item", None, 2), "items");
    assert_eq!(manager.translate_with_context("item", None, 5), "many items");
    // Plural count without a match falls back to message
    assert_eq!(manager.translate_with_context("item", None, 3), "items");
    assert_eq!(manager.translate_with_context("item", None, 0), "items");
    // Key without plural always returns message
    assert_eq!(manager.translate_with_context("simple", None, 1), "simple");
    assert_eq!(manager.translate_with_context("simple", None, 99), "simple");
}

#[test]
fn i18n_manager_load_translations() {
    // File loading works using temp files
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("fr.json");
    let json = r#"{
        "language": "fr",
        "translations": {
            "hello": {"message": "bonjour"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();

    let result = manager.load_translations(file_path.to_str().unwrap());
    assert!(result.is_ok());
    assert_eq!(manager.translation_count(), 1);

    manager.set_language("fr");
    assert_eq!(manager.translate("hello"), "bonjour");

    // Loading a non-existent file returns an error
    let bad_result = manager.load_translations("/nonexistent/path.json");
    assert!(bad_result.is_err());
}

#[test]
fn i18n_manager_hot_reload() {
    // Hot reload enable/disable works
    let (sender, _receiver) = unbounded();
    let mut manager = I18nManager::new();

    assert!(!manager.is_hot_reload_enabled());

    manager.enable_hot_reload(sender);
    assert!(manager.is_hot_reload_enabled());

    manager.disable_hot_reload();
    assert!(!manager.is_hot_reload_enabled());

    // Re-enabling with a new sender works
    let (sender2, _receiver2) = unbounded();
    manager.enable_hot_reload(sender2);
    assert!(manager.is_hot_reload_enabled());

    manager.disable_hot_reload();
    assert!(!manager.is_hot_reload_enabled());
}

#[test]
fn i18n_manager_check_and_reload() {
    // Reload tracking works with modified files
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let initial_json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello"}
        }
    }"#;
    fs::write(&file_path, initial_json).unwrap();

    let (sender, receiver) = unbounded();
    let mut manager = I18nManager::new();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");
    assert_eq!(manager.translate("hello"), "Hello");

    // Give the file system time to register a different modification time
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Update the file
    let updated_json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello Updated"}
        }
    }"#;
    fs::write(&file_path, updated_json).unwrap();

    // check_and_reload should detect the change
    let events = manager.check_and_reload();
    assert!(!events.is_empty(), "Expected reload events after file modification");
    assert_eq!(manager.translate("hello"), "Hello Updated");

    // Draining reload events from channel
    while receiver.try_recv().is_ok() {}

    // Second check without changes should return empty
    let no_events = manager.check_and_reload();
    assert!(
        no_events.is_empty(),
        "Expected no events when file hasn't changed, got {:?}",
        no_events
    );
}

#[test]
fn i18n_global_init() {
    // Global init works (uses separate test to avoid state pollution)
    // Ensure global is reset by calling init()
    global::init();
    {
        let guard = global::get_manager();
        assert!(guard.is_some());
    }

    // Translate through global before any translations loaded
    assert_eq!(global::translate("hello"), "hello");

    // Init with options
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("de.json");
    let json = r#"{
        "language": "de",
        "translations": {
            "hallo": {"message": "Guten Tag"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();

    let options = InitOptions {
        language: "de".to_string(),
        preload_dir: Some(dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    };
    let report = global::init_with_options(options);
    assert_eq!(report.files_loaded, 1);
    assert_eq!(report.translations_count, 1);
    assert!(report.errors.is_empty());

    assert_eq!(global::translate("hallo"), "Guten Tag");
    assert_eq!(global::translate("missing"), "missing");

    // Reset global state
    global::init();
}

#[test]
fn tr_macro_basic() {
    // tr! macro works through the global i18n system
    // Setup global with a known translation
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "greeting": {"message": "Hello"},
            "item_count": {
                "message": "items",
                "plural": {"1": "item", "2": "items"}
            },
            "formal_greet": {
                "message": "Good day",
                "context": "formal"
            }
        }
    }"#;
    fs::write(&file_path, json).unwrap();

    let options = InitOptions {
        language: "en".to_string(),
        preload_dir: Some(dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    };
    let _report = global::init_with_options(options);

    // Basic tr!($key)
    assert_eq!(crate::tr!("greeting"), "Hello");
    // Fallback for missing key
    assert_eq!(crate::tr!("unknown"), "unknown");
    // tr!($key, $count) - plural
    assert_eq!(crate::tr!("item_count", 1), "item");
    assert_eq!(crate::tr!("item_count", 2), "items");
    // tr!($key, $context, $count) - context
    assert_eq!(crate::tr!("formal_greet", "formal", 1), "Good day");
    // Non-matching context returns key
    assert_eq!(crate::tr!("formal_greet", "casual", 1), "formal_greet");

    // Reset global state
    global::init();
}

#[test]
fn test_i18n_manager_translate_with_context_count_only() {
    // Count without context still works properly
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "item": {
                "message": "items",
                "plural": {
                    "1": "1 item",
                    "2": "{count} items"
                }
            }
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");
    assert_eq!(manager.translate_with_context("item", None, 1), "1 item");
    assert_eq!(manager.translate_with_context("item", None, 2), "{count} items");
    assert_eq!(manager.translate_with_context("item", None, 0), "items");
    assert_eq!(manager.translate_with_context("item", None, 100), "items");
}

#[test]
fn test_i18n_manager_audit_keys() {
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "hello": {"message": "Hello"},
            "goodbye": {"message": "Goodbye"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    let keys = manager.audit_keys();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"hello".to_string()));
    assert!(keys.contains(&"goodbye".to_string()));
}

#[test]
fn test_i18n_manager_multiple_languages_key_audit() {
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let en_path = temp_dir.path().join("en.json");
    let fr_path = temp_dir.path().join("fr.json");
    fs::write(&en_path, r#"{"language":"en","translations":{"a":{"message":"A"}}}"#).unwrap();
    fs::write(&fr_path, r#"{"language":"fr","translations":{"b":{"message":"B"}}}"#).unwrap();
    manager.load_translations(en_path.to_str().unwrap()).unwrap();
    manager.load_translations(fr_path.to_str().unwrap()).unwrap();
    let keys = manager.audit_keys();
    assert_eq!(keys.len(), 2);
}

#[test]
fn test_i18n_manager_empty_key() {
    let mut manager = I18nManager::new();
    manager.set_language("en");
    assert_eq!(manager.translate(""), "");
    assert_eq!(manager.translate_with_context("", None, 1), "");
    assert_eq!(manager.translate_with_context("", Some("ctx"), 1), "");
}

#[test]
fn test_i18n_manager_special_chars_in_key() {
    let mut manager = I18nManager::new();
    manager.set_language("en");
    // Keys with special characters that might cause issues
    assert_eq!(manager.translate("hello.world"), "hello.world");
    assert_eq!(manager.translate("hello world"), "hello world");
    assert_eq!(manager.translate("hello\nworld"), "hello\nworld");
    assert_eq!(manager.translate("hello\tworld"), "hello\tworld");
}

#[test]
fn test_i18n_manager_unicode() {
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    let json = r#"{
        "language": "en",
        "translations": {
            "问候": {"message": "Hello"},
            "emoji_test": {"message": "😀 🎉"}
        }
    }"#;
    fs::write(&file_path, json).unwrap();
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    manager.set_language("en");
    assert_eq!(manager.translate("问候"), "Hello");
    assert_eq!(manager.translate("emoji_test"), "😀 🎉");
}
