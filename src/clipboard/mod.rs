// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Clipboard and drag-drop managers.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/bindings/binding_impl.rs:1` (`rw_set_clipboard_text` / `rw_get_clipboard_text` route here).
mod clipboard_manager;
mod drag_drop_manager;
pub use clipboard_manager::ClipboardManager;
pub use drag_drop_manager::DragDropManager;

/// Serialises tests that read or write the process-wide clipboard.
///
/// # Why this exists
///
/// The clipboard is **one system-wide resource**, not per-process state, so two tests that
/// copy and paste race on it exactly as two tests switching the theme race on the theme
/// registry. The loser's assertion then sees the winner's text. That made
/// `widget::input_widgets::lineedit::tests::lineedit_clipboard_copy_paste_cut` fail
/// intermittently under the default multi-threaded harness while passing in isolation — a
/// flake, which is worse than a failure because it trains a reader to re-run until it is
/// green rather than to look at it.
///
/// Public rather than `#[cfg(test)]`, matching `theme::theme_test_guard`: an integration
/// test is a separate crate and cannot see crate-test-only items, yet touches the same
/// clipboard. Not for production use — an application has no other tests to race against.
pub fn clipboard_test_guard() -> crate::compat::MutexGuard<'static, ()> {
    use crate::compat::{lock, Mutex, OnceLock};
    // `compat::lock` rather than `Mutex::lock` directly: the two profile arms of
    // `compat::Mutex` are `spin::Mutex` (which returns the guard) and `std::sync::Mutex`
    // (which returns a `Result`), so calling `lock()` here would compile on one and fail on
    // the other. The helper is the crate's single answer to that difference, which is
    // exactly the kind of platform detail that must not leak into a call site
    // (principle #36) — and it is what `theme_test_guard` uses.
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    lock(GUARD.get_or_init(|| Mutex::new(())))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;
    use crate::core::PlatformFamily;
    use crate::platform::{DropEvent, Platform, StubPlatform};
    #[test]
    fn clipboard_roundtrip() {
        let stub = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
        assert!(ClipboardManager::set_text_with(&stub, "hello"));
        assert_eq!(ClipboardManager::text_with(&stub), "hello".to_string());
    }
    #[test]
    fn drop_event_queue_roundtrip() {
        let stub = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
        let source = stub.create_window("source", 0, 0, 10, 10);
        let target = stub.create_window("target", 10, 10, 10, 10);
        let event = DropEvent {
            source_widget_id: source,
            target_widget_id: target,
            mime: "text/plain".to_string(),
            payload: b"payload".to_vec(),
        };
        assert!(DragDropManager::inject_drop_event_with(&stub, event.clone()));
        let queued = DragDropManager::poll_drop_event_with(&stub);
        assert!(queued.is_some());
        assert_eq!(queued.unwrap(), event);
        assert!(DragDropManager::poll_drop_event_with(&stub).is_none());
    }
}
