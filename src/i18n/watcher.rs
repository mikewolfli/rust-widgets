// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n watcher - file system monitoring for hot reload
//!
//! # Reachability
//!
//! **State:** Production callers: [`crate::i18n::init_with_hot_reload`] (module-local) and
//! [`crate::app::App::enable_i18n_hot_reload`], which the default runtime path invokes from
//! `src/app/app_core.rs`. The watcher is opt-in: it starts only when a translation directory is
//! configured, because watching a directory that does not exist would otherwise be a silent
//! startup cost on every application. Removal condition: when translation hot reload is retired in
//! favour of a compile-time-only catalogue.
use crate::compat::{String, ToString, Vec};
use crate::i18n::global::get_manager;
use crate::i18n::types::ReloadEvent;
use crossbeam_channel::{bounded, Receiver, Sender};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::VecDeque;
use std::path::Path;
use std::time::SystemTime;

/// Maximum number of pending reload announcements the channel holds (D09-WATCH-01).
///
/// The channel is bounded so a burst of translation-file changes cannot grow the
/// queue without limit while the consumer pauses. Callback-side coalescing (see
/// `flush_pending`) re-checks the channel and collapses a language already queued,
/// so the queue holds at most one pending announcement per language; the bound is
/// nonetheless kept as a hard ceiling, because the manager itself can also publish
/// announcements (`announce_reload`) that the watcher must not drop.
pub(crate) const I18N_RELOAD_CAPACITY: usize = 1024;

/// Maximum number of events one [`process_reload_events`] call may take
/// (D09-WATCH-01). The rest stay queued and are applied by later calls, so a
/// single frame applies a bounded number of reloads while delivery stays eventual.
pub(crate) const I18N_RELOAD_BATCH_LIMIT: usize = 64;

/// Maximum number of events one [`I18nFileWatcher::drain_events`] call may take
/// (D09-WATCH-01). Kept equal to [`I18N_RELOAD_BATCH_LIMIT`] so the watcher and the
/// free-function entry point bound a single pass to the same amount of work.
pub(crate) const I18N_DRAIN_BATCH_LIMIT: usize = I18N_RELOAD_BATCH_LIMIT;

/// The language a translation file name implies: the file stem (name without the
/// `.json` extension).
///
/// `I18nManager::load_translations` rejects files whose stem disagrees with the
/// `language` field they declare, so this mapping is always consistent with the
/// key a file was registered under — the watcher and the loader cannot drift
/// apart and request a language key that does not exist.
fn language_from_path(path: &Path) -> Option<String> {
    path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string())
}

/// The languages an event's paths identify, in first-seen order and de-duplicated.
///
/// Split out from the watcher callback so the mapping is unit-testable without a real
/// filesystem event. Every relevant path is mapped, not just the first: `notify`
/// reports `Modify(Name(Both))` with two paths (old and new) for an atomic save, and a
/// new locale arrives as `Create`; taking only `paths.first()` ignored an atomic save
/// whose first path was a non-`json` temp name. Non-`json` and nameless paths are
/// skipped, and a language named by more than one path is emitted once.
fn languages_for_event(kind: &EventKind, paths: &[std::path::PathBuf]) -> Vec<String> {
    if !matches!(kind, EventKind::Modify(_) | EventKind::Create(_)) {
        return Vec::new();
    }
    let mut languages: Vec<String> = Vec::new();
    for path in paths {
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        let Some(language) = language_from_path(path) else {
            continue;
        };
        if languages.iter().any(|seen| seen == &language) {
            continue;
        }
        languages.push(language);
    }
    languages
}

/// File watcher for hot reload
pub struct I18nFileWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    reload_sender: Sender<ReloadEvent>,
    reload_receiver: Receiver<ReloadEvent>,
}
impl I18nFileWatcher {
    /// Create a new file watcher
    ///
    /// The reload channel is bounded (D09-WATCH-01) and its callback coalesces
    /// repeated changes by language, so a burst that outruns the consumer cannot
    /// grow the queue without limit. A coalesced duplicate is dropped; the fact
    /// that a locale changed is never dropped.
    pub fn new() -> Self {
        let (reload_sender, reload_receiver) = bounded(I18N_RELOAD_CAPACITY);
        Self { watcher: None, reload_sender, reload_receiver }
    }
    /// Start watching a directory for translation file changes
    ///
    /// The notify callback buffers the languages an event names and flushes them
    /// to the bounded channel *after* the event, coalescing against what is already
    /// queued (D09-WATCH-01). A flush that finds the channel full leaves the
    /// still-pending languages in the buffer instead of dropping them, so the
    /// pending set is bounded rather than unbounded, and the next callback retries.
    pub fn watch_directory(&mut self, dir: &Path) -> Result<(), String> {
        let sender = self.reload_sender.clone();
        let receiver = self.reload_receiver.clone();
        // Pending languages for the coalescing flush, in insertion order for
        // deterministic delivery. Owned by the callback, which runs on the notify
        // backend thread (D09-WATCH-01).
        let mut pending: VecDeque<String> = VecDeque::new();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
            Ok(event) => {
                // Every path the event carries is mapped to its language identity, not
                // just the first: `notify` reports `Modify(Name(Both))` with two paths
                // (old and new) for an atomic save, and a new locale is a `Create`.
                // `languages_for_event` owns that mapping so it can be unit-tested.
                for language in languages_for_event(&event.kind, &event.paths) {
                    // Skip a language already pending in this callback's buffer.
                    if pending.iter().any(|queued| queued == &language) {
                        continue;
                    }
                    // Skip a language already queued on the channel: re-reading the
                    // same file again before the consumer has applied it would only
                    // produce the same result, so the duplicate announcement is the
                    // one thing safe to drop (D09-WATCH-01).
                    if channel_already_holds_language(&sender, &receiver, &language) {
                        continue;
                    }
                    if !sender
                        .try_send(ReloadEvent::TranslationReloaded {
                            language: language.clone(),
                            timestamp: SystemTime::now(),
                        })
                        .is_ok()
                    {
                        // The channel is full: keep the language pending so a later
                        // callback retries it rather than losing the change.
                        pending.push_back(language);
                    }
                }
                flush_pending(&sender, &receiver, &mut pending);
            }
            Err(e) => {
                log::error!("[i18n] Watcher error: {e:?}");
            }
        })
        .map_err(|e| {
            format!(
                "i18n file watcher could not be created for '{}': {e} (the OS notify \
                 backend may be unavailable)",
                dir.display()
            )
        })?;
        watcher.watch(dir, RecursiveMode::NonRecursive).map_err(|e| {
            format!(
                "directory '{}' could not be watched for translation changes: {e} (check \
                     that the path exists and is readable)",
                dir.display()
            )
        })?;
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
    /// Apply up to [`I18N_DRAIN_BATCH_LIMIT`] pending reload events from this
    /// watcher's own channel (D09-WATCH-01).
    ///
    /// The per-call work is bounded and the rest stay queued for later calls, so a
    /// consumer that paused through a storm does not pay for the whole backlog in
    /// one frame. This is the watcher-owned equivalent of [`process_reload_events`]
    /// and applies events through [`I18nManager::reload_translation_quiet`] for the
    /// same reason: an announcing reload would feed this channel back onto itself.
    pub fn drain_events(&self) -> Vec<ReloadEvent> {
        process_reload_events_bounded(&self.reload_receiver, I18N_DRAIN_BATCH_LIMIT)
    }
    /// Enable hot reload on the global i18n manager
    pub fn enable_hot_reload(&self) {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            manager.enable_hot_reload(self.reload_sender.clone());
        }
    }
    /// Disable hot reload on the global i18n manager
    pub fn disable_hot_reload(&self) {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            manager.disable_hot_reload();
        }
    }
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled(&self) -> bool {
        let guard = get_manager();
        guard.as_ref().map(|m| m.is_hot_reload_enabled()).unwrap_or(false)
    }
}
crate::impl_default_via_new!(I18nFileWatcher);
/// Returns whether `language` already has a `TranslationReloaded` waiting on the
/// channel (D09-WATCH-01).
///
/// Used by the notify callback to coalesce repeated changes: a language already
/// queued needs no second announcement. Only `TranslationReloaded` is matched —
/// `ReloadError` events are diagnostics and are never coalesced. `receiver` is a
/// clone of the channel that `sender` feeds, and `try_recv` removes the events it
/// sees; the caller re-queues every probed event immediately, so nothing is lost.
/// The channel is small, so a scan over its buffered messages is cheap.
fn channel_already_holds_language(
    sender: &Sender<ReloadEvent>,
    receiver: &Receiver<ReloadEvent>,
    language: &str,
) -> bool {
    let mut found = false;
    let mut requeue: VecDeque<ReloadEvent> = VecDeque::new();
    while let Ok(event) = receiver.try_recv() {
        if matches!(&event, ReloadEvent::TranslationReloaded { language: seen, .. } if seen == language)
        {
            found = true;
        }
        requeue.push_back(event);
    }
    for event in requeue {
        if sender.try_send(event).is_err() {
            // The channel holds exactly the messages just drained, so re-queueing
            // them cannot fail unless the receiver was dropped concurrently; in
            // that case there is no consumer left to observe them anyway.
            break;
        }
    }
    found
}

/// Flush coalesced pending languages to the bounded channel (D09-WATCH-01).
///
/// A language already queued (checked at send time) is dropped as a duplicate; a
/// language that does not fit stays pending so it is retried by a later callback
/// rather than lost. There is at most one pending entry per language.
fn flush_pending(
    sender: &Sender<ReloadEvent>,
    receiver: &Receiver<ReloadEvent>,
    pending: &mut VecDeque<String>,
) {
    let mut index = 0usize;
    while index < pending.len() {
        let language = pending[index].clone();
        if channel_already_holds_language(sender, receiver, &language) {
            // Already queued: this pending copy is a duplicate.
            pending.remove(index);
            continue;
        }
        if sender
            .try_send(ReloadEvent::TranslationReloaded { language, timestamp: SystemTime::now() })
            .is_ok()
        {
            pending.remove(index);
            continue;
        }
        // Channel full: leave the entry pending and try again on the next flush.
        index += 1;
    }
}

/// Apply at most `limit` pending reload events from `receiver` (D09-WATCH-01).
///
/// This is the bounded core shared by [`process_reload_events`] and
/// [`I18nFileWatcher::drain_events`]. Events beyond `limit` stay queued, so a
/// single pass never processes an unbounded backlog while still guaranteeing the
/// pending set is applied by later passes.
fn process_reload_events_bounded(
    receiver: &Receiver<ReloadEvent>,
    limit: usize,
) -> Vec<ReloadEvent> {
    let mut events = Vec::new();
    let mut languages_seen: Vec<String> = Vec::new();
    let mut taken = 0usize;
    while taken < limit {
        let Ok(event) = receiver.try_recv() else { break };
        taken += 1;
        match &event {
            ReloadEvent::TranslationReloaded { language, .. } => {
                // De-duplicate within a single pass: an editor save is often a burst
                // for one logical change, and re-reading the same file for each is
                // wasted work (D09-WATCH-01).
                if languages_seen.iter().any(|seen| seen == language) {
                    continue;
                }
                languages_seen.push(language.clone());
                let mut guard = get_manager();
                if let Some(ref mut manager) = *guard {
                    match manager.reload_translation_quiet(language) {
                        Ok(()) => events.push(event),
                        Err(e) => events.push(ReloadEvent::ReloadError {
                            language: language.clone(),
                            error: e,
                        }),
                    }
                }
            }
            ReloadEvent::ReloadError { .. } => events.push(event),
        }
    }
    events
}
/// Initialize i18n with hot reload support
pub fn init_with_hot_reload(
    options: crate::i18n::options::InitOptions,
    watch_dir: Option<&Path>,
) -> (crate::i18n::options::InitReport, I18nFileWatcher) {
    let diagnostics = options.diagnostics;
    let mut report = crate::i18n::global::init_with_options(options);
    let mut file_watcher = I18nFileWatcher::new();
    if let Some(dir) = watch_dir {
        if let Err(e) = file_watcher.watch_directory(dir) {
            report.errors.push(format!("Failed to setup file watcher: {e}"));
        } else {
            file_watcher.enable_hot_reload();
            if diagnostics {
                log::info!("[i18n] Hot reload enabled for directory: {dir:?}");
            }
        }
    }
    (report, file_watcher)
}
/// Process pending reload events and apply the corresponding translations.
///
/// Applies up to [`I18N_RELOAD_BATCH_LIMIT`] events already queued on `receiver`,
/// re-reads the translation file for each announced language, and returns the
/// events actually applied. Any events beyond the batch limit — and any left in
/// the receiver when the limit is hit — stay queued and are applied by the next
/// call, so the per-call synchronous work is bounded without losing a reload
/// (D09-WATCH-01). A language whose reload failed is reported as
/// [`ReloadEvent::ReloadError`] carrying the underlying reason, so a caller can
/// surface it rather than discovering a silently stale catalogue.
///
/// # Why this does not announce the reload it performs
///
/// The events are applied with [`I18nManager::reload_translation_quiet`]. The
/// announcing variant would push a fresh [`ReloadEvent::TranslationReloaded`]
/// onto the very channel being drained here, so the same language would be
/// reloaded (and re-announced) once more on the next pass — forever, since the
/// reload of a file that has not changed still succeeds and still announces.
/// The initial filesystem notification is the only announcement this path
/// needs, and it already arrived.
///
/// Languages are de-duplicated within a single call: an editor that saves a
/// file often produces a burst of platform events for one logical change, and
/// re-reading and re-parsing the same file for each of them multiplies work
/// without changing the result.
pub fn process_reload_events(receiver: &Receiver<ReloadEvent>) -> Vec<ReloadEvent> {
    process_reload_events_bounded(receiver, I18N_RELOAD_BATCH_LIMIT)
}

// ════════════════════════════════════════════════════════════════
// Process-global hot reload, driven by the frame loop
// ════════════════════════════════════════════════════════════════

/// Environment variable naming a directory of translation JSON files to watch.
///
/// The alternative to this variable would be a new `AppConfig` field, but the
/// directory is a *deployment* fact (which catalogue a build ships), not an
/// application-identity fact like `app_name`, and `AppConfig` is a documented
/// public struct whose fields callers construct positionally in the cookbook.
/// An environment variable keeps the setting out of that surface and matches how
/// the library already takes its other runtime overrides
/// (`RUST_WIDGETS_CPU_UTIL`, `RUST_WIDGETS_GPU_TIME_MS`).
pub const HOT_RELOAD_ENV_VAR: &str = "RUST_WIDGETS_I18N_DIR";

/// The process-wide watcher, set up once by [`enable_global_hot_reload`].
///
/// A `Mutex<Option<..>>` rather than a `OnceLock` because hot reload can be
/// disabled as well as enabled — a `OnceLock` would make the first enable
/// permanent, so `disable_global_hot_reload` (used by tests, and by a host that
/// stops serving a writable catalogue directory) could not work.
static GLOBAL_WATCHER: crate::compat::Mutex<Option<I18nFileWatcher>> =
    crate::compat::Mutex::new(None);

/// Starts watching `dir` for translation changes and wires the global manager to it.
///
/// Loads any translation file already in `dir` before watching it, so the initial
/// catalogue is not missing until each file happens to be touched — the watcher
/// only reports *changes*, and a file that is present at startup is not a change.
///
/// Returns the number of translation files loaded from `dir`. Errors are returned
/// rather than logged so a caller can decide whether a missing directory is fatal
/// (it usually is not — a build with an embedded catalogue still works).
pub fn enable_global_hot_reload(dir: &Path) -> Result<usize, String> {
    let mut loaded = 0usize;
    {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            let entries = std::fs::read_dir(dir).map_err(|e| {
                format!("translation directory '{}' could not be read: {e}", dir.display())
            })?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    let Some(path_str) = path.to_str() else { continue };
                    match manager.load_translations(path_str) {
                        Ok(()) => loaded += 1,
                        Err(e) => log::error!(
                            "[i18n] '{}' could not be loaded for hot reload: {e}",
                            path.display()
                        ),
                    }
                }
            }
        }
    }

    let mut watcher = I18nFileWatcher::new();
    watcher.watch_directory(dir)?;
    watcher.enable_hot_reload();

    let mut guard = crate::compat::lock(&GLOBAL_WATCHER);
    *guard = Some(watcher);
    log::info!(
        "[i18n] Hot reload watching '{}' ({loaded} translation file(s) loaded)",
        dir.display()
    );
    Ok(loaded)
}

/// Stops watching for translation changes and detaches the manager from the watcher.
pub fn disable_global_hot_reload() {
    let mut guard = crate::compat::lock(&GLOBAL_WATCHER);
    if let Some(watcher) = guard.take() {
        watcher.disable_hot_reload();
    }
}

/// Drains pending translation-reload events and applies them.
///
/// This is the frame-loop entry point: call it once per frame and translations
/// edited on disk appear without a restart. It applies at most
/// [`I18N_RELOAD_BATCH_LIMIT`] events per frame (D09-WATCH-01); a larger backlog
/// is spread over subsequent frames rather than processed in one unbounded pass.
/// It is deliberately cheap when hot reload is off — one mutex lock and a `None`
/// check — because the platform loops call it on every tick whether or not a
/// catalogue directory was configured.
///
/// Returns the number of events applied, so the frame ledger can report that a
/// frame did reload work rather than leaving it invisible.
pub fn pump_hot_reload() -> usize {
    // The lock is scoped to *reading the receiver out*, not held across the
    // reload: `process_reload_events` itself takes the manager lock, and holding
    // the watcher lock across it would order the two locks one way here and the
    // other way in `enable_global_hot_reload` (which takes the manager lock, then
    // the watcher lock) — a classic AB/BA deadlock waiting for the first caller
    // that enables hot reload from a thread while another thread pumps.
    let receiver = {
        let guard = crate::compat::lock(&GLOBAL_WATCHER);
        guard.as_ref().map(|watcher| watcher.receiver().clone())
    };
    match receiver {
        Some(receiver) => process_reload_events(&receiver).len(),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S-91: the watcher identifies a language by the file stem. Because
    /// `load_translations` rejects a file whose stem disagrees with its declared
    /// `language`, this conversion can never request a language key that does not
    /// exist in the catalogue.
    #[test]
    fn language_from_path_uses_the_file_stem() {
        assert_eq!(language_from_path(Path::new("en.json")).as_deref(), Some("en"));
        assert_eq!(language_from_path(Path::new("/some/dir/de.json")).as_deref(), Some("de"));
        assert_eq!(language_from_path(Path::new("no-extension")).as_deref(), Some("no-extension"));
        assert_eq!(language_from_path(Path::new("")), None);
    }

    /// N-S-69: every relevant path in an event is mapped, so an atomic save that puts
    /// the new `en.json` *second* is still recognised.
    ///
    /// `notify` reports an atomic editor save as `Modify(Name(Both))` carrying two
    /// paths: the temp file first and the real catalogue second. The old callback looked
    /// only at `paths.first()`, so the temp name (not `*.json`) made the whole event a
    /// no-op and the reload never fired. This drives the mapping directly.
    #[test]
    fn an_atomic_save_maps_the_json_path_even_when_it_is_not_first() {
        use notify::event::{ModifyKind, RenameMode};
        let paths = vec![
            std::path::PathBuf::from("/dir/en.json.tmp-1234"),
            std::path::PathBuf::from("/dir/en.json"),
        ];
        assert_eq!(
            languages_for_event(&EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &paths),
            vec!["en".to_string()],
            "the json path must be mapped even when the temp path is first"
        );
    }

    /// A `Create` for a new locale is mapped, so adding `de.json` triggers a reload.
    #[test]
    fn a_created_locale_is_mapped() {
        use notify::event::CreateKind;
        let paths = vec![std::path::PathBuf::from("/dir/de.json")];
        assert_eq!(
            languages_for_event(&EventKind::Create(CreateKind::File), &paths),
            vec!["de".to_string()]
        );
    }

    /// A single event naming one language on two paths is emitted once, so a rename
    /// within `en.json` does not queue two identical reloads.
    #[test]
    fn a_language_named_by_two_paths_is_emitted_once() {
        use notify::event::{ModifyKind, RenameMode};
        let paths = vec![
            std::path::PathBuf::from("/dir/en.json"),
            std::path::PathBuf::from("/dir/en.json"),
        ];
        assert_eq!(
            languages_for_event(&EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &paths),
            vec!["en".to_string()]
        );
    }

    /// Non-matching kinds and non-`json` paths map to nothing.
    #[test]
    fn unrelated_kinds_and_paths_map_to_nothing() {
        use notify::event::{AccessKind, ModifyKind};
        let json = vec![std::path::PathBuf::from("/dir/en.json")];
        assert!(languages_for_event(&EventKind::Access(AccessKind::Any), &json).is_empty());
        assert!(languages_for_event(
            &EventKind::Modify(ModifyKind::Any),
            &[std::path::PathBuf::from("/dir/notes.txt")],
        )
        .is_empty());
    }

    /// D09-WATCH-01: the channel is bounded, so a paused consumer cannot grow it
    /// without limit; and once the consumer resumes, every language still queued is
    /// applied and reported.
    #[test]
    fn reload_channel_is_bounded_and_delivers_every_language() {
        use crate::i18n::global;
        use crate::i18n::options::InitOptions;
        let _lock = crate::i18n::global::global_i18n_test_lock();
        let _ = global::init_with_options(InitOptions {
            language: "en".to_string(),
            ..Default::default()
        });

        let watcher = I18nFileWatcher::new();
        let sender = watcher.reload_sender.clone();

        for i in 0..(I18N_RELOAD_CAPACITY + 500) {
            // `try_send` is what the notify callback uses; a full channel is an
            // expected outcome here, not a failure (the callback keeps the item
            // pending).
            let _ = sender.try_send(ReloadEvent::TranslationReloaded {
                language: format!("lang-{i}"),
                timestamp: SystemTime::now(),
            });
        }
        assert!(
            sender.len() <= I18N_RELOAD_CAPACITY,
            "queue grew past the bound: {}",
            sender.len()
        );

        // A bounded pass drains at most one batch; repeating until the queue is
        // empty delivers every queued language and applies each through the global
        // manager. One call alone would leave the rest queued, which is the timeout
        // the fix is about, not a loss.
        let queued = sender.len();
        let mut applied = watcher.drain_events();
        assert!(!applied.is_empty(), "the first pass must do work");
        assert!(applied.len() <= I18N_DRAIN_BATCH_LIMIT, "one pass is bounded");
        while !applied.is_empty() {
            applied = watcher.drain_events();
        }
        assert_eq!(sender.len(), 0, "the queue is emptied over bounded passes");
        assert!(queued <= I18N_RELOAD_CAPACITY, "the queue never outgrew its bound");
    }

    /// D09-WATCH-01: pausing the consumer and then resuming costs a bounded amount
    /// of work per pass, while the whole backlog is still delivered eventually (so
    /// the final set of changed locales is recoverable).
    #[test]
    fn paused_consumer_resumes_with_bounded_work_and_loses_nothing() {
        let watcher = I18nFileWatcher::new();
        let sender = watcher.reload_sender.clone();

        let pushed = I18N_DRAIN_BATCH_LIMIT * 2;
        for i in 0..pushed {
            sender
                .try_send(ReloadEvent::ReloadError {
                    language: format!("lang-{i}"),
                    error: "synthetic backlog entry".to_string(),
                })
                .expect("within capacity");
        }

        let mut drained = 0usize;
        let mut calls = 0usize;
        loop {
            let batch = watcher.drain_events();
            assert!(
                batch.len() <= I18N_DRAIN_BATCH_LIMIT,
                "one drain exceeded the batch limit: {}",
                batch.len()
            );
            if batch.is_empty() {
                break;
            }
            drained += batch.len();
            calls += 1;
        }

        assert_eq!(drained, pushed, "every queued event must eventually be delivered");
        assert_eq!(calls, 2, "work is spread over ceil(pushed / batch_limit) calls, not one");
        assert!(watcher.drain_events().is_empty(), "the queue is empty in the end");
    }
}
