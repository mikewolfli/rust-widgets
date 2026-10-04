// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

    // A language with no file of its own falls back to the embedded `en` catalogue
    // rather than echoing the key. `hello` is present there (the shipped
    // `language/en.json` is compiled in), so the English string is served — which is the
    // documented purpose of that embedded file. A key absent from `en` too still echoes
    // itself; `i18n_missing_language_falls_back_to_english` covers both halves against a
    // controlled catalogue.
    manager.set_language("de");
    assert_eq!(manager.translate("hello"), "Hello");
    assert_eq!(manager.translate("no_such_key_anywhere"), "no_such_key_anywhere");
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
    // A key whose entry carries no context is "unambiguous on its own" (per
    // `Translation::context`), so a context-qualified lookup must still resolve it.
    // This used to return the raw key, which discarded a perfectly good translation
    // for every entry a catalogue did not restate the context on — and
    // `tr!("key", "ctx", n)` is exactly that lookup.
    assert_eq!(
        manager.translate_with_context("plain", Some("anything"), 1),
        "No context",
        "an entry without a context satisfies any context"
    );
    // A `Some(other)` entry still belongs to another context and must not be served.
    assert_eq!(manager.translate_with_context("greeting", Some("casual"), 1), "greeting");
}

/// A missing key in the active language falls back to `en` before echoing the key.
///
/// `language/en.json` is embedded at build time and documented as "a compile-time
/// fallback", but nothing consulted it: `translate_with_context` went straight from
/// "no entry in the current language" to returning the key, so a partially translated
/// locale showed raw identifiers where a source string was available.
#[test]
fn i18n_missing_language_falls_back_to_english() {
    let mut manager = I18nManager::new();
    let temp_dir = TempDir::new().unwrap();
    let en_path = temp_dir.path().join("en.json");
    let fr_path = temp_dir.path().join("fr.json");
    // `en` covers both keys; `fr` covers only one of them.
    fs::write(
        &en_path,
        r#"{"language":"en","translations":{
             "greeting":{"message":"Hello"},
             "farewell":{"message":"Goodbye"}}}"#,
    )
    .unwrap();
    fs::write(&fr_path, r#"{"language":"fr","translations":{"greeting":{"message":"Bonjour"}}}"#)
        .unwrap();
    manager.load_translations(en_path.to_str().unwrap()).unwrap();
    manager.load_translations(fr_path.to_str().unwrap()).unwrap();
    manager.set_language("fr");

    assert_eq!(manager.translate("greeting"), "Bonjour", "the active language wins");
    assert_eq!(
        manager.translate("farewell"),
        "Goodbye",
        "a gap in the active language must fall back to `en`, not echo the key"
    );
    // A key missing from `en` too has nothing to fall back to.
    assert_eq!(manager.translate("nowhere"), "nowhere");
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

    // No sleep is needed: change detection pairs mtime with the file length and
    // a content hash, so it does not depend on the filesystem clock advancing
    // between the two writes (which a fixed sleep used to paper over).

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
    let _lock = crate::i18n::global::global_i18n_test_lock();
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
    // The embedded English fallback plus the one file-provided `de` catalogue.
    assert_eq!(report.translations_count, 2);
    assert!(report.errors.is_empty());

    assert_eq!(global::translate("hallo"), "Guten Tag");
    assert_eq!(global::translate("missing"), "missing");

    // Reset global state
    global::init();
}

#[test]
fn tr_macro_basic() {
    let _lock = crate::i18n::global::global_i18n_test_lock();
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

/// A content change must be detected even when the filesystem does **not**
/// advance the file's mtime between writes.
///
/// Coarse-grained timestamps (1 s on some Linux/network mounts) make two writes
/// share an mtime. The fingerprint pairs length with a content hash, so this
/// test pins that behaviour down deterministically by rewriting with
/// equal-length, different-content payloads and then *back-dating* the mtime to
/// the value recorded at load time.
#[test]
fn test_i18n_reload_detects_change_with_identical_mtime() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");

    // Both payloads must be the SAME byte length so that length alone cannot
    // distinguish them; only the content hash can.
    let v1 = r#"{"language":"en","translations":{"k":{"message":"aaaa"}}}"#;
    let v2 = r#"{"language":"en","translations":{"k":{"message":"bbbb"}}}"#;
    assert_eq!(v1.len(), v2.len(), "the fixture must be equal-length by design");

    fs::write(&file_path, v1).unwrap();
    let mut manager = I18nManager::new();
    let (sender, _receiver) = unbounded();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    assert_eq!(manager.translate("k"), "aaaa");

    // Capture the mtime the manager recorded, then write the equal-length update
    // and restore that exact mtime so mtime and length are both unchanged.
    let recorded = fs::metadata(&file_path).unwrap().modified().unwrap();
    fs::write(&file_path, v2).unwrap();
    let f = fs::File::options().write(true).open(&file_path).unwrap();
    f.set_modified(recorded).unwrap();
    drop(f);

    let after = fs::metadata(&file_path).unwrap();
    assert_eq!(after.len(), v1.len() as u64, "length must be unchanged");
    assert_eq!(after.modified().unwrap(), recorded, "mtime must be unchanged");

    // With mtime and length identical, only the content hash can detect the edit.
    let events = manager.check_and_reload();
    assert!(
        !events.is_empty(),
        "an equal-length, equal-mtime content change must still trigger a reload"
    );
    assert_eq!(manager.translate("k"), "bbbb");
}

/// The bug this pins: `process_reload_events` used to apply each announced reload
/// through the *announcing* `reload_translation`, so the applied reload pushed a
/// fresh `TranslationReloaded` onto the very channel being drained. The
/// re-announcement was picked up by the same `while let Ok` loop, so one queued
/// event caused the file to be read and parsed twice.
///
/// The contract this pins is therefore the count the caller can observe:
/// **one queued event yields exactly one returned event**, and a following drain
/// finds nothing left over.
///
/// `process_reload_events` reads the **global** manager (that is what the frame
/// pump uses), so this drives the global one under the shared test lock.
#[test]
fn process_reload_events_does_not_feed_its_own_channel() {
    use super::watcher::process_reload_events;
    let _lock = crate::i18n::global::global_i18n_test_lock();

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"one"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let report = global::init_with_options(InitOptions {
        language: "en".to_string(),
        preload_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    });
    assert_eq!(report.files_loaded, 1);
    global::get_manager().as_mut().unwrap().enable_hot_reload(sender.clone());
    assert_eq!(global::translate("k"), "one");

    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"two"}}}"#).unwrap();

    // Exactly one watcher notification for one file change.
    sender
        .send(ReloadEvent::TranslationReloaded {
            language: "en".to_string(),
            timestamp: SystemTime::now(),
        })
        .unwrap();

    let applied = process_reload_events(&receiver);
    assert_eq!(applied.len(), 1, "the single announced reload must be applied");
    assert_eq!(global::translate("k"), "two", "the edit must reach the catalogue");

    // The channel must be empty: applying the reload must not have re-announced
    // it. A second drain therefore sees nothing and reloads nothing.
    let second = process_reload_events(&receiver);
    assert!(second.is_empty(), "applying a reload must not queue another reload; got {second:?}");

    global::init();
}

/// A burst of platform events for one logical save must collapse to one reload.
#[test]
fn process_reload_events_deduplicates_a_burst_for_one_language() {
    use super::watcher::process_reload_events;
    let _lock = crate::i18n::global::global_i18n_test_lock();

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let _report = global::init_with_options(InitOptions {
        language: "en".to_string(),
        preload_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    });
    global::get_manager().as_mut().unwrap().enable_hot_reload(sender.clone());

    // Editors commonly emit several events for one save.
    for _ in 0..3 {
        sender
            .send(ReloadEvent::TranslationReloaded {
                language: "en".to_string(),
                timestamp: SystemTime::now(),
            })
            .unwrap();
    }

    let applied = process_reload_events(&receiver);
    assert_eq!(applied.len(), 1, "three events for one language are one reload");
    assert_eq!(global::translate("k"), "v");

    global::init();
}

/// Two languages announced in one drain must each be applied once. This is the
/// case the per-call de-duplication cannot hide: with the announcing reload the
/// file for `en` would be re-announced while `de` is still queued, and the
/// returned count would exceed the number of distinct languages announced.
#[test]
fn process_reload_events_applies_each_language_once() {
    use super::watcher::process_reload_events;
    let _lock = crate::i18n::global::global_i18n_test_lock();

    let temp_dir = TempDir::new().unwrap();
    let en_path = temp_dir.path().join("en.json");
    let de_path = temp_dir.path().join("de.json");
    fs::write(&en_path, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();
    fs::write(&de_path, r#"{"language":"de","translations":{"k":{"message":"w"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let report = global::init_with_options(InitOptions {
        language: "en".to_string(),
        preload_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    });
    assert_eq!(report.files_loaded, 2);
    global::get_manager().as_mut().unwrap().enable_hot_reload(sender.clone());

    for language in ["en", "de"] {
        sender
            .send(ReloadEvent::TranslationReloaded {
                language: language.to_string(),
                timestamp: SystemTime::now(),
            })
            .unwrap();
    }

    let applied = process_reload_events(&receiver);
    assert_eq!(
        applied.len(),
        2,
        "two distinct languages announced must yield exactly two applications"
    );
    // Nothing left over: neither reload may have re-announced itself.
    assert!(process_reload_events(&receiver).is_empty());

    global::init();
}

/// The property the quiet path uniquely guarantees: applying a reload that a
/// watcher already announced must leave **nothing new on the channel**.
///
/// This is asserted at the manager level, where the de-duplicating guard in
/// `process_reload_events` cannot hide it. The announcing variant fails this:
/// the reload pushes a `TranslationReloaded` back onto the channel, which is
/// exactly what turns one announcement into an endless supply of them once the
/// drain loop has no de-duplicating guard (verified by removing that guard:
/// the loop does not terminate).
#[test]
fn quiet_reload_leaves_nothing_on_the_channel() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let mut manager = I18nManager::new();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();

    // Drain is what a watcher-driven apply does. It must not queue anything.
    manager.reload_translation_quiet("en").unwrap();
    assert!(
        receiver.try_recv().is_err(),
        "a watcher-driven apply must not re-announce the reload it just applied"
    );

    // The announcing variant, by contrast, does queue -- which is why the drain
    // loop must never call it.
    manager.reload_translation("en").unwrap();
    assert!(
        receiver.try_recv().is_ok(),
        "an explicit reload is expected to be observable on the channel"
    );
}

/// `reload_translation` must still announce, so an explicit reload (a language
/// switch driven by the app) is observable on the channel.
#[test]
fn reload_translation_still_announces_on_the_channel() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let mut manager = I18nManager::new();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();

    manager.reload_translation("en").unwrap();
    assert!(
        matches!(
            receiver.try_recv(),
            Ok(ReloadEvent::TranslationReloaded { ref language, .. }) if language == "en"
        ),
        "an explicit reload must still be announced"
    );
}

/// `reload_translation_quiet` must NOT announce: that is the whole point of it.
#[test]
fn reload_translation_quiet_does_not_announce() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");
    fs::write(&file_path, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();

    let (sender, receiver) = unbounded();
    let mut manager = I18nManager::new();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();

    manager.reload_translation_quiet("en").unwrap();
    assert!(receiver.try_recv().is_err(), "the quiet path must not announce");
}

/// The global pump is a no-op (and must not panic) when hot reload was never
/// enabled — the state every frame is in unless the deployer opted in.
#[test]
fn pump_hot_reload_is_a_noop_when_disabled() {
    assert_eq!(super::watcher::pump_hot_reload(), 0);
}

// ── S-31: `init_with_options` must provide the embedded English fallback and
// truthful error reporting ──

/// Default options (no preload directory) must still load the embedded English
/// catalogue, so the English fallback works from this entry point too.
#[test]
fn init_with_options_loads_embedded_english_fallback() {
    let _lock = crate::i18n::global::global_i18n_test_lock();
    let report = global::init_with_options(InitOptions::default());
    assert!(report.translations_count >= 1, "embedded English must be loaded, got {report:?}");
    assert!(report.errors.is_empty(), "no errors expected, got {:?}", report.errors);
    assert_eq!(global::translate("common.button.ok"), "OK");
    global::init();
}

/// A directory with one good and one malformed file must report the bad file,
/// and a missing directory must be reported rather than silently ignored.
#[test]
fn init_with_options_reports_missing_dir_and_bad_file() {
    let _lock = crate::i18n::global::global_i18n_test_lock();
    let temp_dir = TempDir::new().unwrap();
    fs::write(
        temp_dir.path().join("en.json"),
        r#"{"language":"en","translations":{"k":{"message":"v"}}}"#,
    )
    .unwrap();
    fs::write(temp_dir.path().join("fr.json"), "not-json").unwrap();

    let report = global::init_with_options(InitOptions {
        language: "en".to_string(),
        preload_dir: Some(temp_dir.path().to_str().unwrap().to_string()),
        diagnostics: false,
    });
    assert_eq!(report.files_loaded, 1, "the good file must load");
    assert!(
        !report.errors.is_empty(),
        "the malformed file must be reported, got {:?}",
        report.errors
    );
    assert_eq!(global::translate("k"), "v");

    let missing = global::init_with_options(InitOptions {
        language: "en".to_string(),
        preload_dir: Some("/nonexistent/i18n-dir-xyz".to_string()),
        diagnostics: false,
    });
    assert!(
        missing.errors.iter().any(|e| e.contains("could not be read")),
        "a missing directory must be reported, got {:?}",
        missing.errors
    );
    global::init();
}

// ── S-32: a backward mtime must not hide a content change ──

/// A content change whose mtime moves *backward* (backup restore, clock sync)
/// must still trigger a reload. The old code returned `false` as soon as the
/// mtime was older, never reaching the hash comparison.
#[test]
fn test_i18n_reload_detects_change_with_backward_mtime() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("en.json");

    let v1 = r#"{"language":"en","translations":{"k":{"message":"aaaa"}}}"#;
    let v2 = r#"{"language":"en","translations":{"k":{"message":"bbbb"}}}"#;
    assert_eq!(v1.len(), v2.len(), "the fixture must be equal-length by design");

    fs::write(&file_path, v1).unwrap();
    let mut manager = I18nManager::new();
    let (sender, _receiver) = unbounded();
    manager.enable_hot_reload(sender);
    manager.load_translations(file_path.to_str().unwrap()).unwrap();
    assert_eq!(manager.translate("k"), "aaaa");

    // Rewrite with different content and set an *older* mtime than the one the
    // manager recorded at load.
    let recorded = fs::metadata(&file_path).unwrap().modified().unwrap();
    fs::write(&file_path, v2).unwrap();
    let older = recorded - std::time::Duration::from_secs(60);
    let f = fs::File::options().write(true).open(&file_path).unwrap();
    f.set_modified(older).unwrap();
    drop(f);

    let events = manager.check_and_reload();
    assert!(
        !events.is_empty(),
        "a backward-mtime content change must still reload, got {events:?}"
    );
    assert_eq!(manager.translate("k"), "bbbb");
}

// ── S-91: reject a file whose name disagrees with its declared language ──

/// The watcher derives a language from the file name, so `load_translations`
/// must reject a file whose name disagrees with its `language` field rather than
/// load it under a key the watcher will never request.
#[test]
fn load_translations_rejects_file_name_language_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let mismatched = temp_dir.path().join("catalogue.json");
    fs::write(&mismatched, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();

    let mut manager = I18nManager::new();
    assert!(
        manager.load_translations(mismatched.to_str().unwrap()).is_err(),
        "a file whose name disagrees with its declared language must be rejected"
    );

    // A matching name is accepted and registered under the declared language.
    let ok = temp_dir.path().join("en.json");
    fs::write(&ok, r#"{"language":"en","translations":{"k":{"message":"v"}}}"#).unwrap();
    manager.load_translations(ok.to_str().unwrap()).unwrap();
    assert_eq!(manager.translate("k"), "v");
}
