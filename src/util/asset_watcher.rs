//! Generic file-system asset watcher.
//!
//! Watches directories for file changes and delivers events through a
//! crossbeam channel. Extends the pattern proven by `I18nFileWatcher`.

use crossbeam_channel::{Receiver, Sender};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Events produced by the `AssetWatcher`.
#[derive(Debug, Clone)]
pub enum AssetEvent {
    /// A file was created or modified.
    FileChanged {
        path: PathBuf,
        /// The OS-level event kind.
        kind: EventKind,
        /// Monotonic timestamp.
        timestamp: std::time::SystemTime,
    },
    /// Watcher encountered an error.
    WatcherError { path: Option<PathBuf>, error: String },
}

/// Callback type for filtering which file changes should produce events.
pub type FileFilter = Arc<dyn Fn(&Path) -> bool + Send + Sync>;

/// A generic file-system asset watcher.
///
/// Uses `notify::RecommendedWatcher` to monitor directories and sends
/// filtered `AssetEvent` values through a crossbeam channel.
pub struct AssetWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    sender: Sender<AssetEvent>,
    receiver: Receiver<AssetEvent>,
}

impl AssetWatcher {
    /// Create a new `AssetWatcher` with no watched directories yet.
    pub fn new() -> Self {
        let (sender, receiver) = crossbeam_channel::unbounded();
        Self { watcher: None, sender, receiver }
    }

    /// Start watching `directory` (non-recursively by default).
    /// Only files passing `filter` will produce events.
    pub fn watch<F>(&mut self, directory: &Path, recursive: bool, filter: F) -> Result<(), String>
    where
        F: Fn(&Path) -> bool + Send + Sync + 'static,
    {
        let filter = Arc::new(filter);
        let sender = self.sender.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Only interested in create/modify events
                    let is_modify = matches!(event.kind, EventKind::Modify(_));
                    let is_create = matches!(event.kind, EventKind::Create(_));
                    if !is_modify && !is_create {
                        return;
                    }
                    for path in &event.paths {
                        if filter(path) {
                            let _ = sender.send(AssetEvent::FileChanged {
                                path: path.clone(),
                                kind: event.kind,
                                timestamp: std::time::SystemTime::now(),
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ =
                        sender.send(AssetEvent::WatcherError { path: None, error: e.to_string() });
                }
            }
        })
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;

        let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        watcher
            .watch(directory, mode)
            .map_err(|e| format!("Failed to watch directory '{}': {e}", directory.display()))?;

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Get the receiver for polling events from an event loop.
    pub fn receiver(&self) -> &Receiver<AssetEvent> {
        &self.receiver
    }

    /// Drain all pending events and return them.
    pub fn drain(&self) -> Vec<AssetEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Default for AssetWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn asset_watcher_create_drain_no_panics() {
        let watcher = AssetWatcher::new();
        let events = watcher.drain();
        assert!(events.is_empty());
    }

    #[test]
    fn asset_watcher_watch_temp_dir_delivers_events() {
        let dir = std::env::temp_dir().join(format!("asset_watcher_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut watcher = AssetWatcher::new();
        watcher.watch(&dir, false, |p| p.extension().map_or(false, |e| e == "txt")).unwrap();

        // Create a .txt file
        let test_file = dir.join("test.txt");
        fs::write(&test_file, b"hello").unwrap();

        // Give notify time to deliver the event
        std::thread::sleep(Duration::from_millis(200));

        let events = watcher.drain();
        // Canonicalize to handle macOS /var -> /private/var symlink
        let canonical = std::fs::canonicalize(&test_file).unwrap_or_else(|_| test_file.clone());
        let matched = events
            .iter()
            .any(|e| matches!(e, AssetEvent::FileChanged { path, .. } if *path == canonical || path.ends_with("test.txt")));
        assert!(matched, "Expected FileChanged event for test.txt, got {events:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn asset_watcher_filter_blocks_unmatched() {
        let dir = std::env::temp_dir().join(format!("asset_watcher_filter_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut watcher = AssetWatcher::new();
        watcher.watch(&dir, false, |p| p.extension().map_or(false, |e| e == "json")).unwrap();

        // Create .txt file (should NOT produce event)
        let txt_file = dir.join("ignored.txt");
        fs::write(&txt_file, b"ignored").unwrap();

        std::thread::sleep(Duration::from_millis(200));

        let events = watcher.drain();
        // Canonicalize to handle macOS /var -> /private/var symlink
        let canonical = std::fs::canonicalize(&txt_file).unwrap_or_else(|_| txt_file.clone());
        let matched = events
            .iter()
            .any(|e| matches!(e, AssetEvent::FileChanged { path, .. } if *path == canonical || path.ends_with("ignored.txt")));
        assert!(!matched, "Filtered file should not produce event, got {events:?}");

        let _ = fs::remove_dir_all(&dir);
    }
}
