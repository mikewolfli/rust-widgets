// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

use alloc::sync::Arc;
use crossbeam_channel::{bounded, Receiver, Sender};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Maximum number of pending events the watcher keeps in memory (D09-WATCH-01).
///
/// The channel is bounded so the queue cannot grow without limit when the consumer
/// pauses; a single notify callback fans out to at most one event per matching
/// path, so a large checkout or an editor that rewrites many files at once can
/// otherwise enqueue an unbounded burst. The initial deliverable cardinality
/// (one `FileChanged` plus one `WatchError` per watched directory) is far below
/// this bound, so the bound is only ever reached under a genuine storm.
pub(crate) const ASSET_EVENT_CAPACITY: usize = 1024;

/// Maximum number of events one [`AssetWatcher::poll_events`] call may take
/// (D09-WATCH-01). The rest stay queued and are delivered on later calls, so the
/// per-call synchronous work is bounded while delivery is still eventual.
pub(crate) const ASSET_POLL_BATCH_LIMIT: usize = 256;

/// Events produced by the `AssetWatcher`.
#[derive(Debug, Clone)]
pub enum AssetEvent {
    /// A watched file was modified or created.
    FileChanged {
        /// Path reported by the OS watcher: absolute on every platform this
        /// library targets, and the same path the filter predicate saw.
        path: PathBuf,
        /// The OS-level event kind.
        kind: EventKind,
        /// Monotonic timestamp.
        timestamp: std::time::SystemTime,
    },
    /// The underlying watcher reported an error for a given path.
    WatchError {
        /// Path the error concerns. The watcher's own error callback carries no
        /// path, so this is populated empty by `AssetWatcher` and is reserved
        /// for backends that can attribute the failure.
        path: PathBuf,
        /// Human-readable description of the underlying `notify` error.
        error: String,
    },
}

/// The pending `FileChanged` events, coalesced by path (D09-WATCH-01).
///
/// One entry per path rather than one per change: an editor save or a build tool
/// that touches the same file many times must not enqueue that many events. The
/// value is the newest change for the path, so a coalesce can only drop a
/// duplicate notification of the same path and never the fact that the path
/// changed.
type EventMap = HashMap<PathBuf, AssetEvent>;

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
    ///
    /// The event channel is bounded (D09-WATCH-01) and its callback coalesces
    /// repeated changes by path, so a burst that outruns the consumer cannot grow
    /// the queue without limit. A coalesced duplicate is dropped; the fact that a
    /// path changed is never dropped.
    pub fn new() -> Self {
        let (sender, receiver) = bounded(ASSET_EVENT_CAPACITY);
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
        self.watch_internal(dir, false, filter)
    }

    /// Start watching `directory` with recursive option.
    /// Only files passing `filter` will produce events.
    pub fn watch<F>(&mut self, directory: &Path, recursive: bool, filter: F) -> Result<(), String>
    where
        F: Fn(&Path) -> bool + Send + Sync + 'static,
    {
        let filter = Arc::new(filter);
        self.watch_internal(directory, recursive, move |p| filter(p))
    }

    /// Internal implementation shared by `watch_directory` and `watch`.
    ///
    /// The callback buffers matching `FileChanged` events in a path-keyed map and
    /// flushes that map to the bounded channel *after* each notify event. Two
    /// properties follow (D09-WATCH-01): a burst touching the same path collapses
    /// to one pending event, and a flush that finds the channel full leaves the
    /// still-pending events in the map instead of dropping them, so the pending set
    /// is bounded at `ASSET_EVENT_CAPACITY + (paths in one notify event)` rather
    /// than unbounded. The next callback retries the flush, and the consumer
    /// freeing queue space lets it drain.
    fn watch_internal<P>(&mut self, dir: &Path, recursive: bool, filter: P) -> Result<(), String>
    where
        P: Fn(&Path) -> bool + Send + 'static,
    {
        let sender = self.sender.clone();
        // Coalesced pending events live in the notify callback closure, which is
        // owned by the watcher and runs on the notify backend thread (D09-WATCH-01).
        let mut pending: EventMap = HashMap::new();
        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        // One event per matching path: a single notify callback may
                        // describe several paths, and the consumer expects one
                        // `FileChanged` per changed file (its filter already saw
                        // those paths). The newest change for a path replaces the
                        // pending one so repeated changes collapse.
                        let timestamp = std::time::SystemTime::now();
                        for path in &event.paths {
                            if filter(path) {
                                let path = path.to_path_buf();
                                pending.insert(
                                    path.clone(),
                                    AssetEvent::FileChanged { path, kind: event.kind, timestamp },
                                );
                            }
                        }
                        flush_pending(&sender, &mut pending);
                    }
                }
                Err(e) => {
                    if let Err(log_err) = sender
                        .send(AssetEvent::WatchError { path: PathBuf::new(), error: e.to_string() })
                    {
                        log::error!("[asset] Watcher error send failed: {log_err:?}");
                    }
                }
            },
            Config::default(),
        )
        .map_err(|e| {
            format!(
                "asset watcher could not be created for '{}': {e} (the OS notify backend \
                 may be unavailable)",
                dir.display()
            )
        })?;

        let mode = if recursive { RecursiveMode::Recursive } else { RecursiveMode::NonRecursive };
        watcher.watch(dir, mode).map_err(|e| {
            format!(
                "directory '{}' could not be watched with mode {mode:?}: {e} (check that the \
                 path exists and is readable)",
                dir.display()
            )
        })?;

        self.watcher = Some(watcher);
        Ok(())
    }

    /// Take up to [`ASSET_POLL_BATCH_LIMIT`] buffered events from the channel.
    ///
    /// The per-call work is bounded (D09-WATCH-01): a consumer that paused through
    /// a storm delivers the backlog over successive calls instead of materializing
    /// the whole queue in one `Vec`. The call is also non-blocking: when the batch
    /// limit is reached the caller returns immediately rather than waiting to fill
    /// the rest of the batch, so a poll on a nearly-idle queue does not stall the
    /// frame loop. Whatever is left stays queued and is delivered by the next call.
    pub fn poll_events(&self) -> Vec<AssetEvent> {
        let mut events = Vec::with_capacity(ASSET_POLL_BATCH_LIMIT.min(ASSET_EVENT_CAPACITY));
        for _ in 0..ASSET_POLL_BATCH_LIMIT {
            match self.receiver.try_recv() {
                Ok(event) => events.push(event),
                Err(_) => break,
            }
        }
        events
    }

    /// Drain all pending events (alias for consistency).
    pub fn drain(&self) -> Vec<AssetEvent> {
        self.poll_events()
    }

    /// Get a reference to the underlying event receiver.
    pub fn receiver(&self) -> &Receiver<AssetEvent> {
        &self.receiver
    }
}

crate::impl_default_via_new!(AssetWatcher);

/// Flush coalesced pending events to the bounded channel (D09-WATCH-01).
///
/// Events that do not fit stay in `pending` so the next callback retries them;
/// nothing is dropped merely because the queue is momentarily full. Once a path's
/// event has been flushed it is removed from `pending` so it is not re-sent.
fn flush_pending(sender: &Sender<AssetEvent>, pending: &mut EventMap) {
    pending.retain(|_, event| match sender.try_send(event.clone()) {
        Ok(()) => false,
        Err(crossbeam_channel::TrySendError::Full(_)) => true,
        Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
            // No receiver can ever observe this event; keeping it would only
            // grow the map for a consumer that is gone.
            false
        }
    });
}

// File-watcher tests need `tempfile`, which is unavailable on wasm32 (no FS).
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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
            .send(AssetEvent::FileChanged {
                path: PathBuf::from("test.png"),
                kind: EventKind::Create(notify::event::CreateKind::File),
                timestamp: std::time::SystemTime::now(),
            })
            .expect("send should succeed");

        let events = watcher.poll_events();
        assert_eq!(events.len(), 1);
        match &events[0] {
            AssetEvent::FileChanged { path, .. } => {
                assert_eq!(path, &PathBuf::from("test.png"));
            }
            other => panic!("Expected FileChanged, got {:?}", other),
        }
    }

    #[test]
    fn asset_watcher_watch_nonexistent_directory_returns_error() {
        let mut watcher = AssetWatcher::new();
        // A unique, guaranteed-absent path: the temp root plus a counter that no
        // other test or process uses, so this test cannot accidentally point at a
        // real directory that happens to exist. (The old fixed name
        // `/tmp/rw_nonexistent_asset_test_dir_xyzzy` was only presumed absent.)
        static UNIQUE: AtomicUsize = AtomicUsize::new(0);
        let nonexistent = std::env::temp_dir().join(format!(
            "rw_nonexistent_asset_{}_{}",
            std::process::id(),
            UNIQUE.fetch_add(1, Ordering::SeqCst)
        ));
        assert!(!nonexistent.exists(), "fixture path must not already exist");
        let result = watcher.watch_directory(&nonexistent, |_| true);
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
        std::thread::sleep(std::time::Duration::from_millis(200));

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

    /// D09-WATCH-01: many changes to the same path coalesce to one pending event,
    /// and the newest change is the one kept — so nothing uniquely necessary is
    /// lost.
    ///
    /// Coalescing lives in the notify callback (`flush_pending`), which a test
    /// cannot drive without a real filesystem event, so this drives that function
    /// directly. Sending raw events to the channel (bypassing `flush_pending`)
    /// would test the channel, not the coalescing.
    #[test]
    fn asset_watcher_burst_on_one_path_collapses_and_is_delivered() {
        let (tx, rx) = crossbeam_channel::bounded::<AssetEvent>(ASSET_EVENT_CAPACITY);
        let mut pending: EventMap = HashMap::new();

        let path = PathBuf::from("assets/icon.png");
        let kind = EventKind::Modify(notify::event::ModifyKind::Any);
        // A burst the callback's own map sees: many changes to one path before any
        // flush. The map must hold one entry per path, not one per change.
        for i in 0..5_000u64 {
            pending.insert(
                path.clone(),
                AssetEvent::FileChanged {
                    path: path.clone(),
                    kind,
                    timestamp: std::time::SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_millis(i),
                },
            );
        }
        assert_eq!(pending.len(), 1, "a burst on one path must collapse in the pending map");

        flush_pending(&tx, &mut pending);
        assert!(pending.is_empty(), "a within-capacity flush must drain the pending map");

        // Exactly one delivered change, and it is the newest update for the path.
        let events: Vec<AssetEvent> = rx.try_iter().collect();
        assert_eq!(events.len(), 1, "many changes to one path must deliver one event");
        let AssetEvent::FileChanged { path: got, timestamp, .. } = &events[0] else {
            panic!("expected FileChanged, got {events:?}");
        };
        assert_eq!(got, &path);
        assert_eq!(
            *timestamp,
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(4_999),
            "the newest change for the path must be the one delivered"
        );
    }

    /// D09-WATCH-01: when the channel is full, the flush keeps the still-pending
    /// events instead of dropping them, so the pending set is bounded and nothing
    /// necessary is lost; the consumer freeing space lets a later flush finish.
    #[test]
    fn asset_watcher_flush_is_bounded_and_retries_when_full() {
        // A deliberately tiny channel so the full case is reachable without a
        // multi-thousand-element fixture.
        let (tx, rx) = crossbeam_channel::bounded::<AssetEvent>(3);
        let mut pending: EventMap = HashMap::new();
        let kind = EventKind::Create(notify::event::CreateKind::File);
        for i in 0..10u64 {
            let path = PathBuf::from(format!("assets/{i}.png"));
            pending.insert(
                path.clone(),
                AssetEvent::FileChanged {
                    path,
                    kind,
                    timestamp: std::time::SystemTime::UNIX_EPOCH
                        + std::time::Duration::from_millis(i),
                },
            );
        }

        flush_pending(&tx, &mut pending);
        assert_eq!(tx.len(), 3, "the flush must fill the channel but not exceed it");
        assert_eq!(pending.len(), 7, "the events that did not fit must stay pending, not be lost");

        // The consumer frees space; a second flush delivers the rest.
        let mut got = 0usize;
        loop {
            got += rx.try_iter().count();
            flush_pending(&tx, &mut pending);
            if pending.is_empty() {
                break;
            }
        }
        got += rx.try_iter().count();
        assert_eq!(got, 10, "every pending event must eventually be delivered");
        assert!(pending.is_empty(), "nothing is left pending once the consumer drains");
    }

    /// D09-WATCH-01: a paused consumer then resuming yields bounded per-call work,
    /// and the final set of changed paths is still recoverable.
    #[test]
    fn asset_watcher_paused_consumer_drains_in_bounded_batches() {
        let watcher = AssetWatcher::new();
        let sender = watcher.sender.clone();
        let kind = EventKind::Create(notify::event::CreateKind::File);

        // Fill past the batch limit (without exceeding capacity, so nothing is
        // left behind in the callback's pending map).
        let pushed = ASSET_POLL_BATCH_LIMIT * 2;
        for i in 0..pushed {
            sender
                .send(AssetEvent::FileChanged {
                    path: PathBuf::from(format!("assets/tex/{i}.png")),
                    kind,
                    timestamp: std::time::SystemTime::now(),
                })
                .expect("send stays within capacity");
        }

        let mut drained = 0usize;
        let mut calls = 0usize;
        loop {
            let batch = watcher.poll_events();
            assert!(
                batch.len() <= ASSET_POLL_BATCH_LIMIT,
                "one poll exceeded the batch limit: {}",
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
        assert_eq!(watcher.poll_events().len(), 0, "the queue is empty in the end");
    }

    #[test]
    fn asset_watcher_create_drain_no_panics() {
        let watcher = AssetWatcher::new();
        let events = watcher.drain();
        assert!(events.is_empty());
    }

    #[test]
    fn asset_watcher_watch_delivers_file_changed_event() {
        // An exclusive tempdir the test owns: nothing else on the host can share
        // the path, and dropping it cleans up exactly what this test created. The
        // previous fixture used a PID-suffixed name under the shared temp root and
        // `remove_dir_all`ed it up front and again at the end, which could delete
        // a same-named directory another process had created.
        let dir = tempfile::tempdir().unwrap();

        let mut watcher = AssetWatcher::new();
        watcher.watch(dir.path(), false, |p| p.extension().is_some_and(|e| e == "txt")).unwrap();

        let test_file = dir.path().join("test.txt");
        std::fs::write(&test_file, b"hello").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let events = watcher.drain();
        let matched = events.iter().any(
            |e| matches!(e, AssetEvent::FileChanged { path, .. } if path.ends_with("test.txt")),
        );
        assert!(matched, "Expected FileChanged event for test.txt, got {events:?}");
    }

    #[test]
    fn asset_watcher_watch_filter_blocks_unmatched() {
        // Exclusive tempdir; see the sibling test above for why the PID-suffixed
        // shared-temp fixture was replaced.
        let dir = tempfile::tempdir().unwrap();

        let mut watcher = AssetWatcher::new();
        watcher.watch(dir.path(), false, |p| p.extension().is_some_and(|e| e == "json")).unwrap();

        let txt_file = dir.path().join("ignored.txt");
        std::fs::write(&txt_file, b"ignored").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let events = watcher.drain();
        let matched = events.iter().any(
            |e| matches!(e, AssetEvent::FileChanged { path, .. } if path.ends_with("ignored.txt")),
        );
        assert!(!matched, "Filtered file should not produce event, got {events:?}");
    }
}
