// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The process-wide clipboard test guard, kept out of `mod.rs` (principle #8).

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
