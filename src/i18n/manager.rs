// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n manager - core internationalization management
use crate::compat::{HashMap, String, ToString, Vec};
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
    /// Reload a specific translation file, re-reading it from disk.
    ///
    /// On success this also publishes a [`ReloadEvent::TranslationReloaded`] on
    /// the hot-reload channel, so a consumer that asked for a reload explicitly
    /// (for example a settings dialog changing the language) still observes the
    /// outcome through the same channel as a watcher-driven reload.
    ///
    /// A reload that happens *in response to* an event already on that channel
    /// must use [`I18nManager::reload_translation_quiet`] instead: announcing
    /// again from inside the drain loop feeds the announcement back into the
    /// channel it is being drained from, so the same language is reloaded (and
    /// re-announced) once more on every pass.
    pub fn reload_translation(&mut self, language: &str) -> Result<(), String> {
        self.reload_translation_quiet(language)?;
        self.announce_reload(language);
        Ok(())
    }

    /// Re-read a translation file from disk without announcing it on the channel.
    ///
    /// This is the reentrant-safe half of [`I18nManager::reload_translation`],
    /// separated so [`crate::i18n::process_reload_events`] can apply a reload
    /// that a watcher already announced without pushing a duplicate event back
    /// into that watcher's channel.
    ///
    /// Returns the same error as [`I18nManager::reload_translation`] when the
    /// language has no registered file or the file cannot be read and parsed.
    pub fn reload_translation_quiet(&mut self, language: &str) -> Result<(), String> {
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
            Ok(())
        } else {
            Err(format!(
                "no translation file is registered for language \"{language}\"; call \
                 `load_translation` for it first (known languages: {:?})",
                self.translation_paths.keys().collect::<Vec<_>>()
            ))
        }
    }

    /// Publish a [`ReloadEvent::TranslationReloaded`] on the hot-reload channel.
    ///
    /// A no-op when hot reload is disabled (no sender attached), so a reload
    /// performed before `enable_hot_reload` does not fabricate a listener that
    /// does not exist. A failed send is logged rather than propagated: the
    /// translation data itself was already swapped in successfully, and turning
    /// a notification failure into a reload failure would misreport state that
    /// has in fact changed.
    fn announce_reload(&self, language: &str) {
        if let Some(ref sender) = self.reload_sender {
            if let Err(e) = sender.send(ReloadEvent::TranslationReloaded {
                language: language.to_string(),
                timestamp: SystemTime::now(),
            }) {
                log::error!("[i18n] Failed to send reload event: {e:?}");
            }
        }
    }
    /// Check every registered translation file for on-disk changes and reload
    /// the ones that changed, returning the events to report to a caller.
    ///
    /// The reloads go through [`I18nManager::reload_translation_quiet`], not
    /// [`I18nManager::reload_translation`]: the returned `Vec` **is** this
    /// method's notification channel, so also publishing each success on the
    /// hot-reload channel would deliver the same reload twice — once here and
    /// once as a queued event a caller still has to drain. A caller that only
    /// wants the channel populated with no return value is served by
    /// [`crate::i18n::check_and_reload_all`], which discards this vector, so
    /// dropping the duplicate send loses no notification.
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
            match self.reload_translation_quiet(&language) {
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
    ///
    /// # Context matching
    ///
    /// A `Some(ctx)` lookup accepts an entry whose own `context` is `None`: an entry without a
    /// context is documented as "unambiguous on its own" (see [`Translation::context`]), which is
    /// exactly the case where qualifying the lookup adds nothing. It used to return the raw key
    /// instead — so a catalogue that did not restate the context on every entry lost all of its
    /// translations the moment a caller used the `tr!("key", "ctx", n)` spelling, which is the
    /// only reason the macro variant exists.
    ///
    /// A `Some(ctx)` lookup still rejects an entry with a *different* `Some(other)`: that entry
    /// belongs to another context and would be the wrong message.
    pub fn translate_with_context(&self, key: &str, context: Option<&str>, count: u32) -> String {
        if let Some(resolved) = self.lookup_in(&self.current_language, key, context, count) {
            return resolved;
        }
        // No entry in the active language. Fall back to `en`, which is embedded at build
        // time (`language/en.json`), before giving up and echoing the key: a user of a
        // partially translated locale is better served by the source string than by a
        // raw identifier. `en` is skipped when it *is* the active language, so this
        // cannot loop or repeat the same miss.
        const FALLBACK_LANGUAGE: &str = "en";
        if self.current_language != FALLBACK_LANGUAGE {
            if let Some(resolved) = self.lookup_in(FALLBACK_LANGUAGE, key, context, count) {
                return resolved;
            }
        }
        // Last resort: the key itself. Documented behaviour, and the caller can see the
        // miss because the returned text equals the key.
        key.to_string()
    }

    /// Resolve `key` inside one language, honouring context and plural rules.
    ///
    /// Returns `None` when the language has no such key, or when the entry it has belongs to a
    /// different context.
    fn lookup_in(
        &self,
        language: &str,
        key: &str,
        context: Option<&str>,
        count: u32,
    ) -> Option<String> {
        let translation = self.translations.get(language)?.translations.get(key)?;
        if let Some(ctx) = context {
            // `None` on the entry means "unambiguous", so it satisfies any context. A
            // `Some(other)` belongs to another context and must not be served here.
            if let Some(trans_ctx) = &translation.context {
                if trans_ctx != ctx {
                    return None;
                }
            }
        }
        // A plural entry whose category is absent has no string for this count; fall
        // through to the singular rather than reporting a miss.
        if let Some(plural) = &translation.plural {
            if let Some(plural_form) = plural.get(&count) {
                return Some(plural_form.clone());
            }
        }
        Some(translation.message.clone())
    }
}
crate::impl_default_via_new!(I18nManager);
