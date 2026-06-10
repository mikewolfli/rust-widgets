//! CSS file watcher for hot-reloading stylesheets (BLUE13 R1.7).
//!
//! A simple polling-based watcher that monitors a CSS file for changes
//! and reloads it into the global [`StyleSheetManager`] when modified.

use crate::style::stylesheet::global_stylesheet_manager;
use std::fs;
use std::time::{Duration, SystemTime};

/// Polling-based CSS file watcher for hot-reloading stylesheets.
///
/// Watches a single CSS file on disk and, when it changes, reloads the
/// content into the global [`StyleSheetManager`] under the configured name.
/// Uses a configurable polling interval to avoid excessive I/O.
pub struct CssWatcher {
    /// Path to the CSS file being watched.
    path: String,
    /// Last known modification time of the file.
    last_modified: Option<SystemTime>,
    /// Name to register/replace in the global StyleSheetManager.
    sheet_name: String,
    /// Minimum time between polls.
    poll_interval: Duration,
    /// Timestamp of the last poll.
    last_poll: SystemTime,
}

impl CssWatcher {
    /// Create a new CSS watcher for the given file path.
    ///
    /// The `sheet_name` is used when registering/replacing the stylesheet
    /// in the global manager. The file is not read until the first call to
    /// [`poll()`](CssWatcher::poll) or [`reload()`](CssWatcher::reload).
    pub fn new(path: &str, sheet_name: &str) -> Self {
        Self {
            path: path.to_string(),
            last_modified: None,
            sheet_name: sheet_name.to_string(),
            poll_interval: Duration::from_millis(500),
            last_poll: SystemTime::now(),
        }
    }

    /// Set the polling interval in milliseconds.
    ///
    /// The default interval is 500 ms. This controls how often
    /// [`poll()`](CssWatcher::poll) actually checks the filesystem.
    pub fn set_poll_interval(&mut self, ms: u64) {
        self.poll_interval = Duration::from_millis(ms);
    }

    /// Poll the file for changes.
    ///
    /// Reloads CSS into the global [`StyleSheetManager`] if the file has been
    /// modified since the last poll. Respects the configured polling interval:
    /// if not enough time has passed, returns `Ok(false)` without checking.
    ///
    /// Returns:
    /// - `Ok(true)` if the file was modified and reloaded.
    /// - `Ok(false)` if no change was detected or the poll interval hasn't elapsed.
    /// - `Err(String)` if an I/O error occurred.
    pub fn poll(&mut self) -> Result<bool, String> {
        // Check if enough time has passed since the last poll.
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_poll).unwrap_or_default();
        if elapsed < self.poll_interval {
            return Ok(false);
        }
        self.last_poll = now;

        // Read file metadata to get the modification time.
        let metadata = fs::metadata(&self.path).map_err(|e| {
            format!("CssWatcher: failed to read metadata for '{}': {}", self.path, e)
        })?;
        let modified = metadata.modified().map_err(|e| {
            format!("CssWatcher: failed to get modified time for '{}': {}", self.path, e)
        })?;

        // Compare with the last known modification time.
        if self.last_modified.is_some_and(|last| modified <= last) {
            return Ok(false);
        }

        // File has changed — read and reload.
        let css = fs::read_to_string(&self.path)
            .map_err(|e| format!("CssWatcher: failed to read '{}': {}", self.path, e))?;

        // Register/replace the stylesheet in the global manager.
        let mut mgr = global_stylesheet_manager();
        mgr.register(&self.sheet_name, &css, 0);

        self.last_modified = Some(modified);
        Ok(true)
    }

    /// Force-reload the CSS file regardless of modification time.
    ///
    /// This always reads the file and registers it in the global
    /// [`StyleSheetManager`], updating the internal modification timestamp.
    pub fn reload(&mut self) -> Result<(), String> {
        let metadata = fs::metadata(&self.path).map_err(|e| {
            format!("CssWatcher: failed to read metadata for '{}': {}", self.path, e)
        })?;
        let modified = metadata.modified().map_err(|e| {
            format!("CssWatcher: failed to get modified time for '{}': {}", self.path, e)
        })?;

        let css = fs::read_to_string(&self.path)
            .map_err(|e| format!("CssWatcher: failed to read '{}': {}", self.path, e))?;

        let mut mgr = global_stylesheet_manager();
        mgr.register(&self.sheet_name, &css, 0);

        self.last_modified = Some(modified);
        Ok(())
    }

    /// Return the path being watched.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl Default for CssWatcher {
    /// Creates a default watcher watching `"style.css"` with sheet name `"main"`.
    fn default() -> Self {
        Self::new("style.css", "main")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn css_watcher_poll_not_yet_ready() {
        let mut watcher = CssWatcher::new("test.css", "test");
        // Set a long poll interval and call poll immediately.
        watcher.set_poll_interval(10_000);
        // Should return Ok(false) because the interval hasn't elapsed.
        let result = watcher.poll();
        assert_eq!(result, Ok(false), "should skip poll when interval not elapsed");
    }

    #[test]
    fn css_watcher_poll_nonexistent_file_returns_error() {
        let mut watcher = CssWatcher::new("/tmp/nonexistent-file-839201.css", "test");
        // Set interval to 0 so the interval guard doesn't short-circuit the metadata check.
        watcher.set_poll_interval(0);
        let result = watcher.poll();
        assert!(result.is_err(), "polling a non-existent file should return Err: got {:?}", result);
    }

    #[test]
    fn css_watcher_reload_nonexistent_file_returns_error() {
        let mut watcher = CssWatcher::new("/tmp/nonexistent-file-839202.css", "test");
        let result = watcher.reload();
        assert!(
            result.is_err(),
            "force-reloading a non-existent file should return Err: got {:?}",
            result
        );
    }

    #[test]
    fn css_watcher_detects_file_change() {
        // Create a temp CSS file with initial content.
        let mut tmp = NamedTempFile::new().expect("failed to create temp file");
        let path = tmp.path().to_string_lossy().to_string();

        write!(tmp, "/* initial */").expect("write initial content");

        let mut watcher = CssWatcher::new(&path, "test-watcher");
        watcher.set_poll_interval(0);

        // First poll: last_modified is None, so it should load the initial content.
        let first = watcher.poll().expect("first poll should succeed");
        assert!(first, "first poll should load initial content");

        // Verify the sheet was registered.
        {
            let mgr = global_stylesheet_manager();
            assert!(mgr.len() >= 1, "manager should have at least 1 sheet after first poll");
        }

        // Modify the file.
        write!(tmp, "/* modified */").expect("write modified content");
        tmp.flush().expect("flush temp file");

        // Poll again — should detect the change.
        let changed = watcher.poll().expect("poll should succeed");
        assert!(changed, "poll should detect file modification");

        // Clean up the registered sheet.
        {
            let mut mgr = global_stylesheet_manager();
            mgr.unregister("test-watcher");
        }
    }
}
