// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n manager - core internationalization management
use crate::compat::{HashMap, String, ToString, Vec};
use crate::i18n::types::{KeyAudit, ReloadEvent, TranslationFile};
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
        if self.lookup_into(&self.current_language, key, context, count, out) {
            return;
        }
        // No entry in the active language. Fall back to `en`, which is embedded at build
        // time (`language/en.json`), before giving up and echoing the key: a user of a
        // partially translated locale is better served by the source string than by a
        // raw identifier. `en` is skipped when it *is* the active language, so this
        // cannot loop or repeat the same miss.
        const FALLBACK_LANGUAGE: &str = "en";
        if self.current_language != FALLBACK_LANGUAGE
            && self.lookup_into(FALLBACK_LANGUAGE, key, context, count, out)
        {
            return;
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
        // A plural entry whose category is absent has no string for this count; fall
        // through to the singular rather than reporting a miss.
        if let Some(plural_form) = translation.plural.as_ref().and_then(|plural| plural.get(&count))
        {
            out.push_str(plural_form);
            return true;
        }
        out.push_str(&translation.message);
        true
    }
}
crate::impl_default_via_new!(I18nManager);

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
    let bytes = message.as_bytes();
    let mut copied = 0usize;
    let mut index = 0usize;
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
        if let Some((_, value)) = args.iter().find(|(arg, _)| *arg == name) {
            out.push_str(&message[copied..index]);
            out.push_str(value);
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
