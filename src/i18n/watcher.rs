//! i18n watcher - file system monitoring for hot reload
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
        Self {
            watcher: None,
            reload_sender,
            reload_receiver,
        }
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
                                let _ = sender.send(ReloadEvent::TranslationReloaded {
                                    language: lang.to_string(),
                                    timestamp: SystemTime::now(),
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[i18n] Watcher error: {:?}", e);
            }
        })
        .map_err(|e| format!("Failed to create watcher: {}", e))?;
        watcher
            .watch(dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;
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
    pub fn disable_hot_reload() {
        let mut guard = get_manager();
        if let Some(ref mut manager) = *guard {
            manager.disable_hot_reload();
        }
    }
    /// Check if hot reload is enabled
    pub fn is_hot_reload_enabled() -> bool {
        let guard = get_manager();
        guard
            .as_ref()
            .map(|m| m.is_hot_reload_enabled())
            .unwrap_or(false)
    }
}
impl Default for I18nFileWatcher {
    fn default() -> Self {
        Self::new()
    }
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
            report
                .errors
                .push(format!("Failed to setup file watcher: {}", e));
        } else {
            file_watcher.enable_hot_reload();
            if diagnostics {
                eprintln!("[i18n] Hot reload enabled for directory: {:?}", dir);
            }
        }
    }
    (report, file_watcher)
}
/// Process reload events and update translations
pub fn process_reload_events(receiver: &Receiver<ReloadEvent>) -> Vec<ReloadEvent> {
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        match &event {
            ReloadEvent::TranslationReloaded { language, .. } => {
                let mut guard = get_manager();
                if let Some(ref mut manager) = *guard {
                    if let Err(e) = manager.reload_translation(language) {
                        events.push(ReloadEvent::ReloadError {
                            language: language.clone(),
                            error: e,
                        });
                    } else {
                        events.push(event);
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
