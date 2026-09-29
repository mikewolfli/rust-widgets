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
use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::time::SystemTime;
/// File watcher for hot reload
pub struct I18nFileWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    reload_sender: Sender<ReloadEvent>,
    reload_receiver: Receiver<ReloadEvent>,
}
impl I18nFileWatcher {
    /// Create a new file watcher
    pub fn new() -> Self {
        let (reload_sender, reload_receiver) = unbounded();
        Self { watcher: None, reload_sender, reload_receiver }
    }
    /// Start watching a directory for translation file changes
    pub fn watch_directory(&mut self, dir: &Path) -> Result<(), String> {
        let sender = self.reload_sender.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if let Some(path) = event.paths.first() {
                        if path.extension().is_some_and(|ext| ext == "json") {
                            if let Some(lang) = path.file_stem().and_then(|s| s.to_str()) {
                                if let Err(e) = sender.send(ReloadEvent::TranslationReloaded {
                                    language: lang.to_string(),
                                    timestamp: SystemTime::now(),
                                }) {
                                    log::error!("[i18n] Watcher send failed: {e:?}");
                                }
                            }
                        }
                    }
                }
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
/// Drains every event already queued on `receiver`, re-reads the translation
/// file for each announced language, and returns the events actually applied.
/// A language whose reload failed is reported as [`ReloadEvent::ReloadError`]
/// carrying the underlying reason, so a caller can surface it rather than
/// discovering a silently stale catalogue.
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
    let mut events = Vec::new();
    let mut languages_seen: Vec<String> = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        match &event {
            ReloadEvent::TranslationReloaded { language, .. } => {
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
            ReloadEvent::ReloadError { .. } => {
                events.push(event);
            }
        }
    }
    events
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
/// edited on disk appear without a restart. It is deliberately cheap when hot
/// reload is off — one mutex lock and a `None` check — because the platform loops
/// call it on every tick whether or not a catalogue directory was configured.
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
