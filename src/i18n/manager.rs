// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n manager - core internationalization management
use crate::compat::{HashMap, String, ToString, Vec};
use crate::i18n::types::{KeyAudit, LanguageChanged, ReloadEvent, TranslationFile};
use crate::signal::Signal;
use crossbeam_channel::Sender;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;
/// Fingerprint of a translation file used to detect changes.
///
/// The modification time is deliberately not part of the fingerprint: a
/// filesystem clock can stay put (coarse granularity), move forward, or move
/// backward (backup restore, sync tooling that preserves an old mtime), and
/// none of those is proof of a content change. Byte length plus a content hash
/// is the actual proof — equal length + equal hash means "unchanged", anything
/// else means "reload".
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    /// Stable content digest; `0` means "not computed" (unreadable file).
    hash: u64,
}

impl FileFingerprint {
    /// Reads the fingerprint of `path`, computing a content hash.
    fn read(path: &std::path::Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        let len = metadata.len();
        let hash = std::fs::read(path).map(|bytes| fnv1a(&bytes)).unwrap_or(0);
        Some(Self { len, hash })
    }

    /// True when `self` differs from `previous` in length or content.
    ///
    /// Forward, equal, and backward mtimes are handled identically: the mtime is
    /// never consulted, so an older timestamp cannot hide a genuine edit and an
    /// unchanged file (same length and hash) never triggers a reload.
    fn is_newer_than(&self, previous: &Self) -> bool {
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
    /// The keys a catalogue must provide, when the host declared them.
    ///
    /// Empty means "no declared contract", and every load is accepted unchanged — the historical
    /// behaviour. Non-empty turns a file load into a checked operation: a catalogue that lacks a
    /// required key is **refused** rather than installed, because a partially loaded catalogue
    /// paints the missing labels as raw identifiers, which reads as a broken application instead
    /// of as the packaging error it is.
    required_keys: Vec<String>,
    /// Languages tried after the active language and its BCP-47 parents miss.
    ///
    /// Defaults to `["en"]`, preserving the historical single-level fallback. A host can inject
    /// its own chain with [`Self::set_fallback_languages`]; the chain is consulted in order and
    /// stops at the first catalogue that has the key.
    fallback_languages: Vec<String>,
    /// Emitted whenever the active language changes, and by [`Self::retranslate`].
    language_changed: Signal<LanguageChanged>,
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
            required_keys: Vec::new(),
            fallback_languages: Vec::from(["en".to_string()]),
            language_changed: Signal::new(),
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
            // A reload must apply the same identity check the initial load does: the file
            // name (which the watcher uses to decide *which* language changed) must agree
            // with the `language` field the file declares. Without it, an edit that changed
            // the declared language — or a file swapped under a mismatched name — was
            // inserted under the caller's key regardless, so the catalogue silently held a
            // language under the wrong key and the mismatch the loader rejects on first load
            // went unreported on every reload. Reject and keep the previous translations and
            // fingerprint untouched.
            let stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                format!(
                    "translation file '{}' for language \"{language}\" has no usable file name; \
                     it must be named '<language>.json'",
                    path.display()
                )
            })?;
            if stem != translation_file.language {
                return Err(format!(
                    "translation file '{}' is named '{stem}.json' but declares language \"{}\"; \
                     refusing to reload it under language \"{language}\" (the watcher identifies a \
                     language by file name)",
                    path.display(),
                    translation_file.language
                ));
            }
            // The declared key contract applies to a reload exactly as it does to the first
            // load: an edit that drops a key must be refused, otherwise the running application
            // silently starts painting that label as a raw identifier until restart.
            if !self.required_keys.is_empty() {
                let audit = audit_against(&translation_file, &self.required_keys);
                if !audit.missing.is_empty() {
                    return Err(format!(
                        "translation file '{}' is missing {} required key(s): {}; keeping the \
                         previously loaded catalogue",
                        path.display(),
                        audit.missing.len(),
                        audit.missing.join(", ")
                    ));
                }
            }
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

        // The hot-reload watcher derives a language from the file *name* (its
        // stem), so a file whose name disagrees with the `language` it declares
        // would be loaded under one key and re-requested under another on every
        // change — the catalogue would never reload. Reject that mismatch up
        // front rather than let it silently break hot reload.
        let stem = path_buf.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "translation file '{path}' has no usable file name; name it \
                         '<language>.json'"
                ),
            )
        })?;
        if stem != language {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "translation file '{path}' is named '{stem}.json' but declares language \
                     \"{language}\"; rename it to '{language}.json' (the hot-reload watcher \
                     identifies a language by file name)"
                ),
            ));
        }

        // A declared key contract turns this load into a checked one. The check happens before
        // anything is stored, so a refused file leaves the previous catalogue in place — a
        // partially installed catalogue is worse than none, because it looks loaded.
        if !self.required_keys.is_empty() {
            let audit = audit_against(&translation_file, &self.required_keys);
            if !audit.missing.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "translation file '{path}' is missing {} required key(s): {}",
                        audit.missing.len(),
                        audit.missing.join(", ")
                    ),
                ));
            }
        }

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

    /// Inject translations **and** register the file that originated them for hot reload.
    ///
    /// # Why this exists next to [`Self::inject_translations`]
    ///
    /// Embedded data is generated into a file at build time by some hosts, and those hosts still
    /// want an edit of that file to reload without restarting. Plain `inject_translations` has no
    /// path, so `reload_translation` reported "no translation file is registered" — the embedded
    /// catalogue was a dead end even though a file was sitting on disk. This entry point records
    /// the path and its fingerprint, so the same watcher path as [`Self::load_translations`]
    /// applies. The file name must match `language`, because that is how the watcher identifies a
    /// changed catalogue.
    pub fn inject_translations_with_path(
        &mut self,
        language: String,
        file: TranslationFile,
        path: impl Into<PathBuf>,
    ) -> Result<(), String> {
        let path = path.into();
        let stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            format!(
                "translation file '{}' has no usable file name; it must be named \
                 '<language>.json' so hot reload can identify it",
                path.display()
            )
        })?;
        if stem != language {
            return Err(format!(
                "translation file '{}' is named '{stem}.json' but is being injected for language \
                 \"{language}\"; inject it as '{language}.json' or rename the file",
                path.display()
            ));
        }
        // Apply the same declared-key contract the file loader applies: an injected catalogue is
        // still a catalogue the running application will paint.
        if !self.required_keys.is_empty() {
            let audit = audit_against(&file, &self.required_keys);
            if !audit.missing.is_empty() {
                return Err(format!(
                    "translation file '{}' is missing {} required key(s): {}; keeping the \
                     previously loaded catalogue",
                    path.display(),
                    audit.missing.len(),
                    audit.missing.join(", ")
                ));
            }
        }
        self.translations.insert(language.clone(), file);
        self.register_watch_path(&language, path)
    }

    /// Registers an existing catalogue's source path for hot reload.
    ///
    /// This is the smaller half of [`Self::inject_translations_with_path`] for a caller that has
    /// already injected the catalogue and only wants the watcher to track a file. The path's file
    /// stem must equal `language`; the fingerprint is recorded immediately so an edit made after
    /// this call is detected on the next [`Self::check_and_reload`].
    pub fn register_watch_path(
        &mut self,
        language: &str,
        path: impl Into<PathBuf>,
    ) -> Result<(), String> {
        let path = path.into();
        let stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
            format!(
                "translation file '{}' has no usable file name; it must be named '<language>.json'",
                path.display()
            )
        })?;
        if stem != language {
            return Err(format!(
                "translation file '{}' is named '{stem}.json' but is registered for language \
                 \"{language}\"; the watcher identifies a language by file name",
                path.display()
            ));
        }
        if let Some(fingerprint) = FileFingerprint::read(&path) {
            self.file_fingerprints.insert(language.to_string(), fingerprint);
        }
        self.translation_paths.insert(language.to_string(), path);
        Ok(())
    }

    /// Set current language. Emits [`LanguageChanged`] when it actually changes.
    pub fn set_language(&mut self, language: &str) {
        if self.current_language == language {
            return;
        }
        let previous = core::mem::replace(&mut self.current_language, language.to_string());
        self.language_changed
            .emit(LanguageChanged { previous, current: self.current_language.clone() });
    }

    /// Updates the language without emitting [`LanguageChanged`].
    ///
    /// The global [`crate::i18n::set_language`] uses this while it holds the process-wide i18n
    /// mutex, then emits the signal after releasing it. Emitting with that mutex held would make a
    /// slot that calls `translate` deadlock on the non-reentrant lock.
    pub(crate) fn set_language_quiet(&mut self, language: &str) {
        self.current_language = language.to_string();
    }

    /// Emits a [`LanguageChanged`] event for the current language.
    ///
    /// This is the retranslation hook: the language did not change, but every label built from
    /// `tr!` needs to be resolved again (for example after a catalogue was reloaded while the
    /// active language stayed the same). Consumers subscribed through
    /// [`Self::on_language_changed`] re-resolve their text on this event.
    pub fn retranslate(&self) {
        self.language_changed.emit(LanguageChanged {
            previous: self.current_language.clone(),
            current: self.current_language.clone(),
        });
    }

    /// The signal emitted when the active language changes or a retranslation is requested.
    pub fn on_language_changed(&self) -> &Signal<LanguageChanged> {
        &self.language_changed
    }

    /// Get current language
    pub fn current_language(&self) -> &String {
        &self.current_language
    }

    /// Replaces the fallback chain consulted when the active language has no entry.
    ///
    /// Each entry is tried **exactly** and in order, after the active tag and its BCP-47 parents.
    /// Duplicates and empty strings are dropped so a host that appends its current language does
    /// not pay for a second lookup. An empty chain is legal and means "no cross-language fallback"
    /// — the key itself is then the last resort.
    pub fn set_fallback_languages(&mut self, languages: Vec<String>) {
        let mut seen: Vec<String> = Vec::new();
        for language in languages {
            if language.is_empty() || seen.contains(&language) {
                continue;
            }
            seen.push(language);
        }
        self.fallback_languages = seen;
    }

    /// The configured fallback chain, in lookup order.
    pub fn fallback_languages(&self) -> &[String] {
        &self.fallback_languages
    }

    /// The keys one catalogue provides, sorted, or `None` when that language is not loaded.
    ///
    /// This is the per-language view [`Self::audit_keys`] deliberately is not: the audit is the
    /// union across every loaded catalogue, which is the right shape for "what keys exist" but
    /// useless for answering "what does this one locale provide?". Returning `Option` keeps "no
    /// such catalogue" distinct from "a catalogue with no keys".
    pub fn keys_for(&self, language: &str) -> Option<Vec<String>> {
        let file = self.translations.get(language)?;
        let mut keys: Vec<String> = file.translations.keys().cloned().collect();
        keys.sort();
        Some(keys)
    }

    /// One loaded catalogue, by language, with no union fallback.
    pub fn catalogue_for(&self, language: &str) -> Option<&TranslationFile> {
        self.translations.get(language)
    }

    /// The languages that have a loaded catalogue, sorted.
    pub fn languages(&self) -> Vec<&str> {
        let mut languages: Vec<&str> = self.translations.keys().map(String::as_str).collect();
        languages.sort();
        languages
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

    /// Declares the keys every catalogue must provide.
    ///
    /// # What it changes
    ///
    /// Once set, [`Self::load_translations`] **refuses** a file that lacks any of them, so an
    /// incomplete catalogue fails at load time instead of painting raw keys at run time. A
    /// catalogue is also checked in the other direction by [`Self::audit_against_required`], which
    /// reports stale entries that nothing asks for.
    ///
    /// The list is sorted and de-duplicated, so two hosts that declare the same keys in different
    /// orders get the same audit. An empty list restores the unchecked behaviour.
    ///
    /// [`Self::inject_translations`] is **not** checked: it returns no error channel, and an
    /// embedded catalogue compiled into the binary is validated by the build rather than at run
    /// time. A host that wants the same guarantee for an embedded file can call
    /// [`Self::audit_against_required`] after injecting it.
    pub fn set_required_keys(&mut self, keys: Vec<String>) {
        let mut keys = keys;
        keys.sort();
        keys.dedup();
        self.required_keys = keys;
    }

    /// The declared key contract, sorted; empty when none was declared.
    pub fn required_keys(&self) -> &[String] {
        &self.required_keys
    }

    /// Compares one loaded catalogue against the declared key contract.
    ///
    /// `None` when `language` is not loaded or no contract was declared — the honest "there is
    /// nothing to compare" answer rather than an audit that reads as a pass.
    pub fn audit_against_required(&self, language: &str) -> Option<KeyAudit> {
        let file = self.translations.get(language)?;
        if self.required_keys.is_empty() {
            return None;
        }
        Some(audit_against(file, &self.required_keys))
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
    /// context is documented as "unambiguous on its own" (see `Translation::context`), which is
    /// exactly the case where qualifying the lookup adds nothing. It used to return the raw key
    /// instead — so a catalogue that did not restate the context on every entry lost all of its
    /// translations the moment a caller used the `tr!("key", "ctx", n)` spelling, which is the
    /// only reason the macro variant exists.
    ///
    /// A `Some(ctx)` lookup still rejects an entry with a *different* `Some(other)`: that entry
    /// belongs to another context and would be the wrong message.
    pub fn translate_with_context(&self, key: &str, context: Option<&str>, count: u32) -> String {
        let mut out = String::new();
        self.translate_with_context_into(key, context, count, &mut out);
        out
    }

    /// Translate and fill `{name}` placeholders in one call.
    ///
    /// # Why this is one call and not two
    ///
    /// A message such as `"{count} files selected"` needs a lookup and then a substitution. Exposing
    /// only the lookup left hosts writing their own filler (the reason the consuming host still
    /// carries one), and a second filler is a second answer to "what does a placeholder look like"
    /// — two implementations that drift (principle #101). The substitution rule is documented on
    /// [`interpolate_into`], which this delegates to, so there is exactly one definition.
    ///
    /// `args` are `(name, value)` pairs matched **exactly** (no prefix matching): `{name}` is
    /// replaced only by an argument literally named `name`. A placeholder with no matching argument
    /// is left verbatim, so a missing value is visible in the output instead of silently vanishing.
    pub fn translate_with_args(
        &self,
        key: &str,
        context: Option<&str>,
        count: u32,
        args: &[(&str, &str)],
    ) -> String {
        let mut out = String::new();
        self.translate_with_args_into(key, context, count, args, &mut out);
        out
    }

    /// [`Self::translate_with_args`], appending to `out`.
    ///
    /// One scratch buffer is allocated for the resolved message; the substituted text goes into
    /// the caller's buffer, so a draw loop still reuses its own allocation for the result.
    pub fn translate_with_args_into(
        &self,
        key: &str,
        context: Option<&str>,
        count: u32,
        args: &[(&str, &str)],
        out: &mut String,
    ) {
        if args.is_empty() {
            // No placeholders to fill: stay on the zero-allocation path.
            self.translate_with_context_into(key, context, count, out);
            return;
        }
        let mut message = String::new();
        self.translate_with_context_into(key, context, count, &mut message);
        interpolate_into(&message, args, out);
    }

    /// Translate and fill placeholders whose values carry a type.
    ///
    /// # Why this exists next to [`Self::translate_with_args`]
    ///
    /// String arguments make every numeric placeholder a formatting decision the caller has
    /// already made — and the caller usually made it with `to_string()`, which is the C locale.
    /// A German user then reads `1,234.5` where `1.234,5` is correct. Typed arguments keep the
    /// number a number until this crate formats it for the active language, and the plain string
    /// path is left untouched, so existing callers see exactly the behaviour they had.
    ///
    /// `count` still selects the plural form; it is not used to format a placeholder (the
    /// `{count}` placeholder, if used, is an explicit argument so the caller chooses its type).
    pub fn translate_with_typed_args(
        &self,
        key: &str,
        context: Option<&str>,
        count: u32,
        args: &[(&str, PlaceholderArgument<'_>)],
    ) -> String {
        let mut out = String::new();
        self.translate_with_typed_args_into(key, context, count, args, &mut out);
        out
    }

    /// [`Self::translate_with_typed_args`], appending to `out`.
    pub fn translate_with_typed_args_into(
        &self,
        key: &str,
        context: Option<&str>,
        count: u32,
        args: &[(&str, PlaceholderArgument<'_>)],
        out: &mut String,
    ) {
        if args.is_empty() {
            self.translate_with_context_into(key, context, count, out);
            return;
        }
        let mut message = String::new();
        self.translate_with_context_into(key, context, count, &mut message);
        interpolate_typed_into(&message, args, Some(self.current_language.as_str()), out);
    }

    /// Zero-allocation counterpart of [`Self::translate`]: appends the resolved message to `out`.
    pub fn translate_into(&self, key: &str, out: &mut String) {
        self.translate_with_context_into(key, None, 1, out);
    }

    /// Zero-allocation counterpart of [`Self::translate_with_context`].
    ///
    /// `out` receives the active-language entry, else the `en` fallback entry, else the key
    /// itself — exactly the precedence of `translate_with_context`. The buffer is appended to,
    /// never cleared, so a caller can compose several translations in one `String`.
    pub fn translate_with_context_into(
        &self,
        key: &str,
        context: Option<&str>,
        count: u32,
        out: &mut String,
    ) {
        // The common case is a hit in the active language, so it costs one lookup and no
        // allocation. Only a miss builds the parent chain.
        if self.lookup_into(&self.current_language, key, context, count, out) {
            return;
        }

        // Walk the BCP-47 parent chain: `zh-Hans-CN` -> `zh-Hans` -> `zh`. A catalogue keyed by a
        // more specific tag wins; a region-only variant therefore still finds its language's text
        // instead of jumping straight to the fallback. `skip(1)` drops the tag already tried above.
        for candidate in bcp47_lookup_chain(&self.current_language).skip(1) {
            if self.lookup_into(candidate, key, context, count, out) {
                return;
            }
        }

        // Then the host-injected chain, in order. `en` is the default, preserving the historical
        // single-level fallback. A candidate that was already tried (the active language itself,
        // or one of its parents) simply misses again; the map lookup is cheap and keeping the
        // chain explicit is clearer than maintaining a skip set on the hot path.
        for fallback in &self.fallback_languages {
            if self.lookup_into(fallback, key, context, count, out) {
                return;
            }
        }

        // Last resort: the key itself. Documented behaviour, and the caller can see the
        // miss because the returned text equals the key.
        out.push_str(key);
    }

    /// Resolves `key` inside one language and appends it to `out` when found.
    fn lookup_into(
        &self,
        language: &str,
        key: &str,
        context: Option<&str>,
        count: u32,
        out: &mut String,
    ) -> bool {
        let Some(translation) =
            self.translations.get(language).and_then(|file| file.translations.get(key))
        else {
            return false;
        };
        if let Some(ctx) = context {
            // `None` on the entry means "unambiguous", so it satisfies any context. A
            // `Some(other)` belongs to another context and must not be served here.
            if let Some(trans_ctx) = &translation.context {
                if trans_ctx != ctx {
                    return false;
                }
            }
        }
        // Plural forms are keyed by CLDR category number, not by the raw count (W4-17). The
        // category this quantity selects is what the file's `plural` map is asked for; a file that
        // provides no form for that category falls through to the singular rather than reporting a
        // miss.
        let category = plural_category(language, count);
        if let Some(plural_form) =
            translation.plural.as_ref().and_then(|plural| plural.get(&category))
        {
            out.push_str(plural_form);
            return true;
        }
        out.push_str(&translation.message);
        true
    }
}
crate::impl_default_via_new!(I18nManager);

/// Numeric plural-category keys, in CLDR's canonical order (W4-17).
///
/// A translation's `plural` map is keyed by these numbers, not by the quantity itself: `1` is the
/// singular form and `5` is the catch-all form regardless of the count that selected it. The full
/// order is `zero = 0`, `one = 1`, `two = 2`, `few = 3`, `many = 4`, `other = 5`; the rules below
/// select only the categories they actually distinguish.
mod plural {
    pub const ONE: u32 = 1;
    pub const FEW: u32 = 3;
    pub const MANY: u32 = 4;
    pub const OTHER: u32 = 5;
}

/// Maps a quantity to its CLDR plural category number for `language` (W4-17).
///
/// # Scope
///
/// This is the **minimal** rule set the crate needs; it is not a full CLDR table. It covers:
///
/// - languages without grammatical number (`zh`, `ja`, `ko`, `th`, `vi`, `id`, `ms`, `lo`, `my`)
///   always select [`plural::OTHER`];
/// - French and Portuguese select [`plural::ONE`] for 0 and 1 and [`plural::OTHER`] above that;
/// - the East-Slavic family (`ru`, `uk`, `be`, `sr`, `hr`, `bs`) uses the integer one/few/many
///   rules;
/// - everything else, including English and German, selects [`plural::ONE`] for exactly 1 and
///   [`plural::OTHER`] otherwise.
///
/// Categories a language does not use (`two` for English, for example) are never selected, and a
/// file may omit them. Languages with dual or paucal forms (`ar`, `he`, `cy`, ...) fall back to
/// the default one/other rule rather than pretending to be correct; a host that needs them
/// supplies pre-formatted text through [`PlaceholderArgument::Text`]. Extending the table is
/// local to this function.
fn plural_category(language: &str, count: u32) -> u32 {
    let primary = language.split(['-', '_']).next().unwrap_or("");
    let is_one_of = |tags: &[&str]| tags.iter().any(|tag| primary.eq_ignore_ascii_case(tag));

    if is_one_of(&["zh", "ja", "ko", "th", "vi", "id", "ms", "lo", "my"]) {
        return plural::OTHER;
    }
    if is_one_of(&["fr", "pt"]) {
        return if count <= 1 { plural::ONE } else { plural::OTHER };
    }
    if is_one_of(&["ru", "uk", "be", "sr", "hr", "bs"]) {
        let mod10 = count % 10;
        let mod100 = count % 100;
        return if mod10 == 1 && mod100 != 11 {
            plural::ONE
        } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
            plural::FEW
        } else {
            plural::MANY
        };
    }
    if count == 1 {
        plural::ONE
    } else {
        plural::OTHER
    }
}

/// The ordered lookup candidates for a BCP-47 language tag.
///
/// `zh-Hans-CN` yields `zh-Hans-CN`, `zh-Hans`, `zh`; a bare `en` yields just `en`. Both `-` and
/// `_` are accepted as separators so a POSIX locale (`pt_BR`) is not treated as one opaque tag.
/// Private because it exists to serve [`I18nManager::translate_with_context_into`], whose
/// documented behaviour is the contract; a caller that wants to resolve keys itself uses
/// [`I18nManager::catalogue_for`].
///
/// The candidates are prefixes of the tag itself, so they are borrowed slices rather than owned
/// `String`s: a label whose key is missing from the active language walks this chain on **every
/// paint**, and building a `Vec<String>` there allocated once per frame per label.
fn bcp47_lookup_chain(tag: &str) -> impl Iterator<Item = &str> {
    let mut current = tag;
    let mut done = tag.is_empty();
    core::iter::from_fn(move || {
        if done {
            return None;
        }
        let candidate = current;
        match current.rfind(['-', '_']) {
            Some(index) => current = &current[..index],
            None => done = true,
        }
        Some(candidate)
    })
}

/// Appends `message` to `out` with every `{name}` placeholder replaced by its argument.
///
/// # The rule, in full
///
/// * A placeholder is `{` + a name + `}`, where the name is one or more ASCII alphanumerics or
///   `_`. Anything else — `{ }`, `{}`, `{a-b}`, an unterminated `{` — is copied through literally.
/// * A matching argument's value is inserted **once per occurrence**, and arguments are matched
///   exactly (not as prefixes), so `{count}` never picks up `{count_total}`.
/// * A placeholder with no matching argument is left verbatim. This is deliberate: silently
///   substituting an empty string would make a missing value indistinguishable from an empty one,
///   and the raw `{name}` in a label is a defect a translator or a test can see.
/// * The first argument with a given name wins; a duplicate later in the list is ignored rather
///   than reported, because a caller that passes the same name twice has already lost which value
///   it meant.
/// * There is no escape syntax: `{{` is a literal `{` followed by the rest of the text, and a
///   message that legitimately contains `{name}` with no argument keeps it as written.
///
/// # Why a free function
///
/// Filling is a property of the message, not of the manager. A host that already holds a resolved
/// string (from a cached translation, a name table, or a plural rule of its own) can fill it
/// without a manager, which is what lets one filler serve both the lookup path and the direct
/// path instead of each having its own.
pub fn interpolate_into(message: &str, args: &[(&str, &str)], out: &mut String) {
    interpolate_with(
        message,
        |name, value| {
            if let Some((_, text)) = args.iter().find(|(arg, _)| *arg == name) {
                value.push_str(text);
                true
            } else {
                false
            }
        },
        out,
    );
}

/// One placeholder value with its type preserved until formatting.
///
/// # Why a typed value rather than a pre-formatted string
///
/// A locale-aware number cannot be produced by `to_string()`: it needs a decimal separator and
/// grouping rules that depend on the active language. Keeping the number typed here lets
/// [`interpolate_typed_into`] apply those rules at the one point where the locale is known, and
/// leaves [`interpolate_into`]'s string contract unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaceholderArgument<'a> {
    /// Text inserted verbatim.
    Text(&'a str),
    /// A whole number, grouped according to the locale.
    Integer(i64),
    /// A finite decimal with a fixed number of fractional digits.
    Decimal {
        /// The value to format.
        value: f64,
        /// Fractional digits to print. Clamped by the formatter to a sane maximum.
        decimals: u32,
    },
}

/// The separators one locale uses for ordinary decimal numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberFormat {
    /// The character between the integer and fractional parts.
    pub decimal_separator: char,
    /// The character between digit groups of three.
    pub grouping_separator: char,
}

impl NumberFormat {
    /// The C-locale format: `.` decimal, `,` grouping.
    pub const C_LOCALE: Self = Self { decimal_separator: '.', grouping_separator: ',' };
}

/// The number format implied by a BCP-47 language tag.
///
/// The table is intentionally small and explicit: it covers the languages whose conventions a
/// host is likely to need, and everything else keeps the C-locale shape. That is a documented
/// boundary rather than a claim of full CLDR coverage — a host with an unusual locale supplies
/// its own pre-formatted text through [`PlaceholderArgument::Text`].
pub fn number_format_for(locale: Option<&str>) -> NumberFormat {
    let Some(locale) = locale else {
        return NumberFormat::C_LOCALE;
    };
    let tag = locale.split('.').next().unwrap_or(locale);
    let language = tag.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    const COMMA_DECIMAL: [&str; 27] = [
        "bg", "ca", "cs", "da", "de", "el", "es", "fi", "fr", "hr", "hu", "id", "it", "nb", "nl",
        "nn", "pl", "pt", "ro", "ru", "sk", "sl", "sr", "sv", "tr", "uk", "vi",
    ];
    const SPACE_GROUPING: [&str; 14] =
        ["ca", "cs", "da", "fi", "fr", "hu", "nb", "nn", "pl", "ru", "sk", "sv", "uk", "vi"];
    if COMMA_DECIMAL.contains(&language.as_str()) {
        NumberFormat {
            decimal_separator: ',',
            // The narrow no-break space is what French, Russian and Nordic locales use between
            // groups. A plain space would let a line break split the number.
            grouping_separator: if SPACE_GROUPING.contains(&language.as_str()) {
                '\u{202F}'
            } else {
                '.'
            },
        }
    } else {
        NumberFormat::C_LOCALE
    }
}

/// Formats a whole number with the locale's grouping separator.
pub fn format_integer(value: i64, locale: Option<&str>) -> String {
    let mut out = String::new();
    format_integer_into(value, number_format_for(locale), &mut out);
    out
}

/// Formats a fixed-point decimal with the locale's separators.
///
pub fn format_number(value: f64, decimals: u32, locale: Option<&str>) -> String {
    let mut out = String::new();
    format_number_into(value, decimals, number_format_for(locale), &mut out);
    out
}

fn format_integer_into(value: i64, format: NumberFormat, out: &mut String) {
    if value < 0 {
        out.push('-');
    }
    // `i64::MIN` has no positive `i64`, so the magnitude is taken in `i128`.
    let digits = (value as i128).abs().to_string();
    push_grouped(&digits, format.grouping_separator, out);
}

fn format_number_into(value: f64, decimals: u32, format: NumberFormat, out: &mut String) {
    if !value.is_finite() {
        // `NaN`/`inf` have no locale-specific spelling here; the Rust spelling is the honest
        // fallback rather than a fabricated word.
        out.push_str(&value.to_string());
        return;
    }
    let decimals = decimals.min(12) as usize;
    let magnitude = value.abs();
    let text = format!("{:.*}", decimals, magnitude);
    let (integer, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    if value.is_sign_negative() && magnitude != 0.0 {
        out.push('-');
    }
    push_grouped(integer, format.grouping_separator, out);
    if decimals > 0 {
        out.push(format.decimal_separator);
        out.push_str(fraction);
    }
}

/// Appends `digits` with a separator every three characters from the right.
fn push_grouped(digits: &str, separator: char, out: &mut String) {
    let length = digits.len();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (length - index).is_multiple_of(3) {
            out.push(separator);
        }
        out.push(digit);
    }
}

/// Appends `message` to `out`, resolving each placeholder from a typed argument.
///
/// The placeholder grammar is exactly [`interpolate_into`]'s — this function shares its scanner,
/// so it cannot drift from the string path. Values are formatted for `locale`: whole numbers are
/// grouped, decimals use the locale's decimal separator, and text is inserted verbatim. A
/// placeholder with no matching argument is left as written, for the same reason the string
/// filler leaves it: a visible `{name}` is a defect a test can see, whereas an empty substitution
/// is indistinguishable from a real empty value.
pub fn interpolate_typed_into(
    message: &str,
    args: &[(&str, PlaceholderArgument<'_>)],
    locale: Option<&str>,
    out: &mut String,
) {
    let format = number_format_for(locale);
    interpolate_with(
        message,
        |name, value| {
            let Some((_, argument)) = args.iter().find(|(arg, _)| *arg == name) else {
                return false;
            };
            match argument {
                PlaceholderArgument::Text(text) => value.push_str(text),
                PlaceholderArgument::Integer(number) => format_integer_into(*number, format, value),
                PlaceholderArgument::Decimal { value: number, decimals } => {
                    format_number_into(*number, *decimals, format, value)
                }
            }
            true
        },
        out,
    );
}

/// The shared placeholder scanner behind both interpolation entry points.
///
/// `resolve` writes the replacement into the second argument and returns whether the placeholder
/// was recognised. Splitting it out is what makes the typed and untyped paths agree on the
/// grammar rather than each carrying its own copy.
fn interpolate_with<F>(message: &str, mut resolve: F, out: &mut String)
where
    F: FnMut(&str, &mut String) -> bool,
{
    let bytes = message.as_bytes();
    let mut copied = 0usize;
    let mut index = 0usize;
    let mut value = String::new();
    while index < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }
        // Find the closing brace before any nested `{`; anything else is not a placeholder.
        let mut end = index + 1;
        while end < bytes.len() && bytes[end] != b'}' && bytes[end] != b'{' {
            end += 1;
        }
        if end >= bytes.len() || bytes[end] != b'}' {
            index += 1;
            continue;
        }
        let name = &message[index + 1..end];
        let is_placeholder = !name.is_empty()
            && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if !is_placeholder {
            index += 1;
            continue;
        }
        value.clear();
        if resolve(name, &mut value) {
            out.push_str(&message[copied..index]);
            out.push_str(&value);
            copied = end + 1;
        }
        // No value: leave the braces in place, which the `copied..index` run keeps intact.
        index = end + 1;
    }
    out.push_str(&message[copied..]);
}

/// Both directions of the comparison between one catalogue and a required key set.
///
/// Sorted on both sides so an audit is stable regardless of the map's iteration order (a
/// `HashMap` on desktop, a `BTreeMap` under mini).
fn audit_against(file: &TranslationFile, required: &[String]) -> KeyAudit {
    let mut missing: Vec<String> = required
        .iter()
        .filter(|key| !file.translations.contains_key(key.as_str()))
        .cloned()
        .collect();
    missing.sort();
    missing.dedup();

    let mut unexpected: Vec<String> = file
        .translations
        .keys()
        .filter(|key| !required.iter().any(|required| required == *key))
        .cloned()
        .collect();
    unexpected.sort();
    unexpected.dedup();

    KeyAudit { missing, unexpected }
}
