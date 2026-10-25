// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The process-wide environment revision bumped by an OS change notification.
//!
//! # Why a counter and not a callback
//!
//! The library does not own the host's event loop, so it cannot be *pushed* a callback on a
//! thread it is not running. What it can be given is a value it compares against the one it saw
//! last, which is the same shape [`crate::platform::Platform::display_generation`] and the asset
//! watcher already use. A backend that receives a native notification (macOS
//! `NSDistributedNotificationCenter`, Windows `WM_SETTINGCHANGE`, GTK's `notify::` signals)
//! calls [`bump`]; the style layer's bounded poll then sees a different revision and re-reads the
//! facts immediately instead of waiting for its interval.
//!
//! # Why this is separate from a backend's own revision
//!
//! A backend's [`Platform::environment_generation`] is derived from values it can read — which is
//! the honest fallback when there is no notification source, but it cannot distinguish "the user
//! changed the setting and changed it back between two reads" from "nothing happened". A pushed
//! notification can. Keeping the pushed revision here means every backend, including the ones
//! that derive instead of listen, gains the same hook without a field on each implementation.
//!
//! [`Platform::environment_generation`]: crate::platform::Platform::environment_generation

use core::sync::atomic::{AtomicU64, Ordering};

/// Notifications observed since process start. Zero means none, which is also the value that says
/// "this backend cannot report changes" on [`Platform::environment_generation`], so the two
/// meanings are compatible: before a notification there is nothing to report.
static NOTIFIED: AtomicU64 = AtomicU64::new(0);

/// Records one host notification and returns the new revision.
pub fn bump() -> u64 {
    NOTIFIED.fetch_add(1, Ordering::SeqCst).wrapping_add(1)
}

/// The number of notifications observed so far.
pub fn generation() -> u64 {
    NOTIFIED.load(Ordering::SeqCst)
}

/// Serialises tests that advance the process-wide counter.
///
/// # Why the tests need this
///
/// The counter is one value for the whole process, and `cargo test` runs the test threads in
/// parallel. A test that reads `generation()`, acts, and asserts a delta is only meaningful while
/// no other test bumps in between — and the environment-fact cache tests deliberately bump to
/// prove a notification invalidates a reading. One lock owned by this module (the owner of the
/// counter) is the smallest thing that makes both assertions deterministic.
#[cfg(test)]
pub(crate) fn test_lock() -> crate::compat::MutexGuard<'static, ()> {
    static LOCK: crate::compat::OnceLock<crate::compat::Mutex<()>> = crate::compat::OnceLock::new();
    crate::compat::lock(LOCK.get_or_init(|| crate::compat::Mutex::new(())))
}

/// Folds the notification revision into a backend's own revision.
///
/// A backend revision of `0` means "not derived here"; the notification count is then the whole
/// answer, and stays `0` until the host actually notifies. A non-zero backend revision is mixed
/// with the notification count so a push changes the returned value even when the derived values
/// have not changed yet (AppKit updates its cached appearance a moment after posting, for
/// example).
pub fn combine(base: u64) -> u64 {
    let notified = generation();
    if base == 0 {
        return notified;
    }
    let mut hash = base;
    for byte in notified.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bump is observable even when the backend derives `0`.
    #[test]
    fn a_bump_is_a_new_revision_and_zero_is_none() {
        let _guard = test_lock();
        let before = generation();
        let first = bump();
        let second = bump();
        assert_eq!(first, before + 1);
        assert_eq!(second, first + 1);
        assert_ne!(combine(0), 0);
        // A derived base stays non-zero and a bump still changes it.
        assert_ne!(combine(7), 0);
        assert_ne!(combine(7), 7);
    }
}
