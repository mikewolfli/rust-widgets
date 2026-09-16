// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n manager - core internationalization management
use crate::compat::HashMap;
use crate::i18n::types::{ReloadEvent, TranslationFile};
use crossbeam_channel::Sender;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;
/// Fingerprint of a translation file used to detect changes.
///
/// `mtime` alone is not sufficient: filesystems with coarse timestamp
/// granularity (e.g. 1 s on some Linux/network mounts) report the *same* mtime
/// for two writes within one tick, so a strict `modified > last_modified`
/// comparison silently misses the update and the reload never fires. Pairing the
/// timestamp with the byte length makes same-tick edits observable, and a
/// content hash catches same-tick edits that happen to keep the same length.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    modified: Option<SystemTime>,
    len: u64,
    /// Stable content digest; `0` means "not computed".
    hash: u64,
}

impl FileFingerprint {
    /// Reads the fingerprint of `path`, computing a content hash.
    fn read(path: &std::path::Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        let modified = metadata.modified().ok();
        let len = metadata.len();
        // Hashing is only needed to disambiguate equal (mtime, len) pairs; doing
        // it unconditionally keeps the logic simple and the files are tiny.
        let hash = std::fs::read(path).map(|bytes| fnv1a(&bytes)).unwrap_or(0);
        Some(Self { modified, len, hash })
    }

    /// True when `self` is newer than `previous`.
    ///
    /// Ordered by mtime first, then length, then content hash, so a change is
    /// detected even when the filesystem clock did not advance.
    fn is_newer_than(&self, previous: &Self) -> bool {
        if let (Some(now), Some(before)) = (self.modified, previous.modified) {
            if now != before {
                return now > before;
            }
        }
        // Same (or unavailable) timestamp: fall back to size, then content.
        if self.len != previous.len {
            return true;
        }
        self.hash != previous.hash
    }
}

/// 64-bit FNV-1a hash — small, dependency-free, and adequate for change
/// detection (this is not a security boundary).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// i18n manager with hot reload support
pub struct I18nManager {
    translations: HashMap<String, TranslationFile>,
    current_language: String,
    translation_paths: HashMap<String, PathBuf>,
    file_fingerprints: HashMap<String, FileFingerprint>,
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
            file_fingerprints: HashMap::new(),
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
            let mut file = File::open(path).map_err(|e| {
                format!(
                    "translation file '{}' for language \"{language}\" could not be opened: {e}",
                    path.display()
                )
            })?;
            let mut content = String::new();
            file.read_to_string(&mut content).map_err(|e| {
                format!(
                    "translation file '{}' for language \"{language}\" could not be read: {e}",
                    path.display()
                )
            })?;
            let translation_file: TranslationFile =
                serde_json::from_str(&content).map_err(|e| {
                    format!(
                        "translation file '{}' for language \"{language}\" is not valid \
                         TranslationFile JSON: {e}",
                        path.display()
                    )
                })?;
            self.translations.insert(language.to_string(), translation_file);
            if let Some(fingerprint) = FileFingerprint::read(path) {
                self.file_fingerprints.insert(language.to_string(), fingerprint);
            }
            if let Some(ref sender) = self.reload_sender {
                if let Err(e) = sender.send(ReloadEvent::TranslationReloaded {
                    language: language.to_string(),
                    timestamp: SystemTime::now(),
                }) {
                    log::error!("[i18n] Failed to send reload event: {e:?}");
                }
            }
            Ok(())
        } else {
            Err(format!(
                "no translation file is registered for language \"{language}\"; call \
                 `load_translation` for it first (known languages: {:?})",
                self.translation_paths.keys().collect::<Vec<_>>()
            ))
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
            let Some(fingerprint) = FileFingerprint::read(path) else {
                continue;
            };
            if let Some(previous) = self.file_fingerprints.get(language) {
                if fingerprint.is_newer_than(previous) {
                    languages_to_reload.push(language.clone());
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
        self.translation_paths.insert(language.clone(), path_buf.clone());
        if let Some(fingerprint) = FileFingerprint::read(&path_buf) {
            self.file_fingerprints.insert(language, fingerprint);
        }
        Ok(())
    }
    /// Inject translations directly (from embedded data, no file I/O).
    /// This bypasses file path tracking and hot reload for the embedded locale.
    pub fn inject_translations(&mut self, language: String, file: TranslationFile) {
        self.translations.insert(language, file);
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
crate::impl_default_via_new!(I18nManager);
