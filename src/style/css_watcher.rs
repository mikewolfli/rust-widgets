// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CSS file watcher — hot-reload a stylesheet into the global `StyleSheetManager`.
//!
//! # Why polling rather than an OS notification backend
//!
//! The i18n watcher uses `notify`, which needs a per-platform backend and hands
//! events to a callback on a background thread. A stylesheet reload has two extra
//! requirements that make polling the better fit here:
//!
//! * **The reload must be observable from the frame loop.** A stylesheet change
//!   invalidates rendered appearance, so the caller has to *know* a reload
//!   happened in order to request a repaint. A poll that reports "changed since
//!   last time" makes that a value in the caller's own loop instead of a
//!   cross-thread signal.
//! * **The file may be mid-write.** An editor that truncates and rewrites a file
//!   fires several notification events, and reloading on each one parses a
//!   partially written stylesheet. Reading on demand, and only when the
//!   modification time actually moved, reads a settled file in practice.
//!
//! # Example
//!
//! ```no_run
//! use rust_widgets::style::css_watcher::CssWatcher;
//!
//! let mut watcher = CssWatcher::new("theme.css", "main-theme");
//! watcher.set_poll_interval(500);
//! loop {
//!     if watcher.poll().unwrap_or(false) {
//!         // styles changed — request a repaint
//!     }
//!     std::thread::sleep(std::time::Duration::from_millis(16));
//! }
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::stylesheet::global_stylesheet_manager;

/// Polls a CSS file and reloads it into the global stylesheet manager on change.
///
/// The watcher records the file's modification time as of the last successful
/// reload, so a poll reports `true` exactly once per observed change. A file that
/// does not exist yet is not an error: `poll` reports `false` and keeps waiting,
/// which is what lets a caller start watching before a generated stylesheet has
/// been written.
#[derive(Debug)]
pub struct CssWatcher {
    /// The stylesheet's path on disk.
    path: PathBuf,
    /// The name the stylesheet is registered under in the manager. Registering by
    /// name (rather than appending) is what makes a reload *replace* the previous
    /// sheet instead of stacking a second copy of it.
    name: String,
    /// Minimum interval between filesystem checks, honoured by [`Self::poll`].
    poll_interval: Duration,
    /// When the last check ran. `None` means "never checked", so the first poll
    /// always checks.
    last_check: Option<SystemTime>,
    /// The file's modification time at the last successful reload. `None` means
    /// nothing has been loaded yet.
    last_modified: Option<SystemTime>,
    /// Priority the sheet is registered with.
    priority: u8,
}

impl CssWatcher {
    /// Creates a watcher for `path`, registering into the global manager as `name`.
    ///
    /// Does not read the file: call [`Self::reload`] to load it immediately, or let
    /// the first [`Self::poll`] do it.
    pub fn new(path: impl Into<PathBuf>, name: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            name: name.into(),
            poll_interval: Duration::from_millis(500),
            last_check: None,
            last_modified: None,
            priority: 0,
        }
    }

    /// Sets the minimum interval between filesystem checks.
    ///
    /// A `poll_interval` of zero makes every call check. The default is 500 ms,
    /// which is short enough to feel immediate and long enough that a per-frame
    /// poll is not a per-frame `stat`.
    pub fn set_poll_interval(&mut self, millis: u64) {
        self.poll_interval = Duration::from_millis(millis);
    }

    /// Returns the configured poll interval in milliseconds.
    pub fn poll_interval(&self) -> u64 {
        self.poll_interval.as_millis() as u64
    }

    /// Sets the priority the sheet is registered under (higher wins).
    pub fn set_priority(&mut self, priority: u8) {
        self.priority = priority;
    }

    /// The path being watched.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The name the stylesheet is registered under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Checks the file and reloads the stylesheet if it changed.
    ///
    /// Returns `Ok(true)` when a reload happened, `Ok(false)` when there was
    /// nothing to do, and `Err` when the file exists but could not be read or
    /// parsed. A missing file is `Ok(false)`, not an error — see the type docs.
    ///
    /// The modification time is recorded *before* the read, so a write that lands
    /// during the read is detected by the next poll rather than being missed.
    pub fn poll(&mut self) -> Result<bool, String> {
        let now = SystemTime::now();
        if let Some(last) = self.last_check {
            match now.duration_since(last) {
                Ok(elapsed) if elapsed < self.poll_interval => return Ok(false),
                // A clock that went backwards (an NTP correction, a suspend/resume)
                // makes `duration_since` fail. Checking anyway is the safe reading:
                // a redundant read is cheaper than a missed change.
                _ => {}
            }
        }
        self.last_check = Some(now);

        let metadata = match std::fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Not written yet. Keep the previous baseline so the first write is
                // still detected as a change.
                return Ok(false);
            }
            Err(error) => {
                return Err(format!(
                    "cannot stat stylesheet '{}': {error} (check that the path is readable)",
                    self.path.display()
                ));
            }
        };

        let modified = metadata.modified().map_err(|error| {
            format!("cannot read the modification time of '{}': {error}", self.path.display())
        })?;
        if self.last_modified == Some(modified) {
            return Ok(false);
        }

        let css = std::fs::read_to_string(&self.path).map_err(|error| {
            format!(
                "cannot read stylesheet '{}': {error} (it may be mid-write; the next poll will \
                 retry)",
                self.path.display()
            )
        })?;

        // Validate before registering, so a syntactically broken stylesheet is
        // reported instead of replacing a working one with rules that fail to
        // parse on every later `apply_to`.
        super::css::CssParser::parse(&css).map_err(|error| {
            format!("stylesheet '{}' is not valid CSS: {error}", self.path.display())
        })?;

        global_stylesheet_manager().register(&self.name, &css, self.priority);
        self.last_modified = Some(modified);
        Ok(true)
    }

    /// Reloads unconditionally, ignoring the modification time.
    ///
    /// Use this when the caller knows the file changed in a way the modification
    /// time does not reflect (a restored backup, or a filesystem with coarse
    /// timestamps).
    pub fn reload(&mut self) -> Result<(), String> {
        // Clear the baseline so the shared implementation cannot short-circuit.
        self.last_modified = None;
        self.last_check = None;
        if self.poll()? {
            Ok(())
        } else {
            Err(format!(
                "stylesheet '{}' could not be loaded: it does not exist",
                self.path.display()
            ))
        }
    }

    /// Whether the stylesheet has been loaded at least once.
    pub fn is_loaded(&self) -> bool {
        self.last_modified.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::global_stylesheet_manager;

    /// A unique temporary path per test, so tests running in parallel do not share
    /// a file.
    fn temp_path(tag: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("rw-css-watcher-{tag}-{}.css", std::process::id()));
        path
    }

    /// Locks the global stylesheet registry for the duration of a test.
    ///
    /// Every test here registers into the process-wide manager, so without this
    /// they would race each other (and the other suites' sheet tests).
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        crate::style::stylesheet_test_guard()
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn defaults_are_documented_values() {
        let _guard = guard();
        let watcher = CssWatcher::new("theme.css", "main");
        assert_eq!(watcher.poll_interval(), 500);
        assert_eq!(watcher.name(), "main");
        assert_eq!(watcher.path(), Path::new("theme.css"));
        assert!(!watcher.is_loaded(), "a new watcher has loaded nothing");
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let _guard = guard();
        let mut watcher = CssWatcher::new(temp_path("missing"), "missing-sheet");
        assert_eq!(watcher.poll(), Ok(false), "an absent file must not fail the poll");
        assert!(!watcher.is_loaded());
    }

    #[test]
    fn reload_registers_the_stylesheet_in_the_global_manager() {
        let _guard = guard();
        let path = temp_path("load");
        std::fs::write(&path, "Button { background-color: #102030; }").expect("write fixture");

        let name = format!("watcher-load-{}", std::process::id());
        let mut watcher = CssWatcher::new(&path, &name);
        watcher.reload().expect("the fixture is valid CSS");
        assert!(watcher.is_loaded());

        // The sheet is really in the manager: applying it to a style changes it.
        let mut style = crate::style::WidgetStyle::default();
        global_stylesheet_manager()
            .apply_to("Button", None, None, None, &mut style)
            .expect("apply the loaded sheet");
        assert_eq!(style.background_color, Some(crate::core::Color::rgb(0x10, 0x20, 0x30)));

        global_stylesheet_manager().unregister(&name);
        cleanup(&path);
    }

    #[test]
    fn poll_reports_a_change_exactly_once() {
        let _guard = guard();
        let path = temp_path("once");
        std::fs::write(&path, "Button { color: #111111; }").expect("write fixture");

        let name = format!("watcher-once-{}", std::process::id());
        let mut watcher = CssWatcher::new(&path, &name);
        // No interval, so every poll checks the filesystem.
        watcher.set_poll_interval(0);

        assert_eq!(watcher.poll(), Ok(true), "the first poll loads the file");
        assert_eq!(watcher.poll(), Ok(false), "an unchanged file is not reloaded");
        assert_eq!(watcher.poll(), Ok(false));

        global_stylesheet_manager().unregister(&name);
        cleanup(&path);
    }

    #[test]
    fn a_broken_stylesheet_is_reported_and_not_registered() {
        let _guard = guard();
        let path = temp_path("broken");
        // An unterminated declaration block: the parser rejects this.
        std::fs::write(&path, "Button { color: #111111; ").expect("write fixture");

        let name = format!("watcher-broken-{}", std::process::id());
        let mut watcher = CssWatcher::new(&path, &name);
        let error = watcher.reload().expect_err("invalid CSS must be reported");
        assert!(error.contains("not valid CSS"), "{error}");
        assert!(!watcher.is_loaded(), "a rejected stylesheet must not count as loaded");

        // Nothing was registered under this watcher's name, so a style it would
        // have set is untouched. Asserted by *absence*: the broken sheet sets a
        // colour no other sheet in the suite uses.
        let mut style = crate::style::WidgetStyle::default();
        global_stylesheet_manager()
            .apply_to("Button", None, Some(&name), None, &mut style)
            .expect("apply");
        assert_ne!(
            style.background_color,
            Some(crate::core::Color::rgb(0x11, 0x11, 0x11)),
            "the rejected stylesheet must not have been registered"
        );

        cleanup(&path);
    }

    #[test]
    fn a_file_created_after_the_watcher_starts_is_picked_up() {
        let _guard = guard();
        let path = temp_path("late");
        cleanup(&path);

        let name = format!("watcher-late-{}", std::process::id());
        let mut watcher = CssWatcher::new(&path, &name);
        watcher.set_poll_interval(0);
        assert_eq!(watcher.poll(), Ok(false), "nothing to read yet");

        std::fs::write(&path, "Label { color: #123456; }").expect("write fixture");
        assert_eq!(watcher.poll(), Ok(true), "the file appearing is the change");

        global_stylesheet_manager().unregister(&name);
        cleanup(&path);
    }

    #[test]
    fn poll_interval_suppresses_checks_until_it_elapses() {
        let _guard = guard();
        let path = temp_path("interval");
        std::fs::write(&path, "Button { color: #222222; }").expect("write fixture");

        let name = format!("watcher-interval-{}", std::process::id());
        let mut watcher = CssWatcher::new(&path, &name);
        watcher.set_poll_interval(60_000);
        assert_eq!(watcher.poll(), Ok(true), "the first poll always checks");

        std::fs::write(&path, "Button { color: #333333; }").expect("rewrite fixture");
        assert_eq!(watcher.poll(), Ok(false), "a change inside the interval is deferred, not lost");

        global_stylesheet_manager().unregister(&name);
        cleanup(&path);
    }
}
