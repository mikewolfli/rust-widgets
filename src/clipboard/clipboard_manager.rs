// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::String;
#[cfg(not(alloc_frugal))]
use crate::platform::get_platform;
use crate::platform::Platform;
/// High-level clipboard access facade.
///
/// This manager forwards clipboard operations to the active platform backend.
pub struct ClipboardManager;
impl ClipboardManager {
    /// Set plain text into the system/process clipboard.
    ///
    /// Returns `true` when the backend accepts the update.
    #[cfg(not(alloc_frugal))]
    pub fn set_text(text: impl AsRef<str>) -> bool {
        // Through the crate-root entry so the APP-012 publish (which lives in the control backend)
        // runs for a copy initiated here too (XC-02).
        crate::set_clipboard_text(text.as_ref())
    }
    /// Clipboard not available in mini mode.
    #[cfg(alloc_frugal)]
    pub fn set_text(_text: impl AsRef<str>) -> bool {
        false
    }
    /// Read plain text from the clipboard.
    ///
    /// Returns an empty string when no text is available.
    #[cfg(not(alloc_frugal))]
    pub fn text() -> String {
        Self::text_with(get_platform())
    }
    /// Clipboard not available in mini mode.
    #[cfg(alloc_frugal)]
    pub fn text() -> String {
        String::new()
    }
    /// Set plain text into the system/process clipboard via an explicit platform.
    /// Kept for tests and any `&dyn Platform` caller; the manager's own `set_text` now routes
    /// through the crate-root entry so the APP-012 publish runs (XC-02).
    #[cfg_attr(not(alloc_frugal), allow(dead_code))]
    pub(crate) fn set_text_with(platform: &dyn Platform, text: &str) -> bool {
        platform.set_clipboard_text(text)
    }
    /// Read plain text from the clipboard via an explicit platform.
    /// Kept under mini so tests (and any `&dyn Platform` caller) can use it.
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    pub(crate) fn text_with(platform: &dyn Platform) -> String {
        platform.get_clipboard_text()
    }
}
