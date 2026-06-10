//! Generic asset file watcher with predicate-based filtering.
//!
//! Provides a `notify`-based watcher that can be configured to watch
//! a directory and filter file change events through a user-supplied predicate.
//!
//! # Example
//!
//! ```rust
//! use rust_widgets::asset::{AssetEvent, AssetWatcher};
//!
//! let mut watcher = AssetWatcher::new();
//! let result = watcher.watch_directory(
//!     std::path::Path::new("/some/dir"),
//!     |path| path.extension().is_some_and(|ext| ext == "png"),
//! );
//! ```

use crossbeam_channel::{unbounded, Receiver, Sender};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};

/// Events produced by the `AssetWatcher`.
#[derive(Debug, Clone)]
pub enum AssetEvent {
    /// A watched file was modified or created.
    FileChanged { path: PathBuf },
    /// The underlying watcher reported an error for a given path.
    WatchError { path: PathBuf, error: String },
}

/// A generic file watcher that monitors a directory for file changes matching
/// a user-supplied predicate.
///
/// The watcher uses `notify` under the hood and communicates file-change
/// events via a `crossbeam_channel` so consumers can poll for events without
/// blocking indefinitely.
pub struct AssetWatcher {
    watcher: Option<notify::RecommendedWatcher>,
    sender: Sender<AssetEvent>,
    receiver: Receiver<AssetEvent>,
}

impl AssetWatcher {
    /// Create a new `AssetWatcher` with no active watch.
    pub fn new() -> Self {
        let (sender, receiver) = unbounded();
        Self { watcher: None, sender, receiver }
    }

    /// Start watching `dir` for file changes filtered by `filter`.
    ///
    /// The `filter` predicate is called with the absolute path of each
    /// changed file. When it returns `true`, a `FileChanged` event is emitted.
    ///
    /// # Errors
    ///
    /// Returns an error if the `notify` watcher cannot be created or if
    /// `dir` does not exist or is not readable.
    pub fn watch_directory<P>(&mut self, dir: &Path, filter: P) -> Result<(), String>
    where
        P: Fn(&Path) -> bool + Send + 'static,
    {
        let sender = self.sender.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if let Some(path) = event.paths.first() {
                        if filter(path) {
                            if let Err(e) =
                                sender.send(AssetEvent::FileChanged { path: path.clone() })
                            {
                                log::error!("[asset] Watcher send failed: {:?}", e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("[asset] Watcher error: {:?}", e);
            }
        })
        .map_err(|e| format!("Failed to create asset watcher: {}", e))?;

        watcher
            .watch(dir, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Drain all currently buffered events from the channel.
    pub fn poll_events(&self) -> Vec<AssetEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get a reference to the underlying event receiver.
    pub fn receiver(&self) -> &Receiver<AssetEvent> {
        &self.receiver
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn asset_watcher_create_and_drop() {
        // Creating and dropping a watcher must not panic.
        let watcher = AssetWatcher::new();
        drop(watcher);
        let watcher = AssetWatcher::default();
        drop(watcher);
    }

    #[test]
    fn asset_watcher_channel_roundtrip() {
        let watcher = AssetWatcher::new();
        // Send an event via the sender and receive it via poll_events.
        watcher
            .sender
            .send(AssetEvent::FileChanged { path: PathBuf::from("test.png") })
            .expect("send should succeed");

        let events = watcher.poll_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AssetEvent::FileChanged { path } => {
                assert_eq!(path, &PathBuf::from("test.png"));
            }
            other => panic!("Expected FileChanged, got {:?}", other),
        }
    }

    #[test]
    fn asset_watcher_watch_nonexistent_directory_returns_error() {
        let mut watcher = AssetWatcher::new();
        let result = watcher.watch_directory(
            Path::new("/tmp/rw_nonexistent_asset_test_dir_xyzzy"),
            |_| true,
        );
        assert!(result.is_err(), "Expected error for nonexistent directory");
    }

    /// Verify that the filter predicate is actually applied.
    #[test]
    fn asset_watcher_filter_predicate() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|e| e.to_string())?;

        let filter_calls = Arc::new(AtomicUsize::new(0));
        let filter_calls_clone = filter_calls.clone();

        let mut watcher = AssetWatcher::new();
        watcher.watch_directory(dir.path(), {
            move |_path| {
                filter_calls_clone.fetch_add(1, Ordering::SeqCst);
                false // filter out everything
            }
        })?;

        // Trigger a file-system notification by creating a file.
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").map_err(|e| e.to_string())?;

        // Give the notify backend a moment to deliver the event.
        std::thread::sleep(Duration::from_millis(200));

        // Because the predicate returns false, the channel should be empty.
        let events = watcher.poll_events();
        assert!(
            events.is_empty(),
            "Expected no events when predicate filters everything, got {:?}",
            events
        );

        // Sanity: the filter should have been called at least once.
        let calls = filter_calls.load(Ordering::SeqCst);
        assert!(calls > 0, "Expected at least one filter call, got {}", calls);

        Ok(())
    }
}
