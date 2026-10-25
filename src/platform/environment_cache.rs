// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The shared TTL wrapper for environment facts (P-02 / P-201).
//!
//! # The defect this removes
//!
//! The style layer re-reads the platform's environment revision on a bounded 250 ms poll
//! ([`crate::style::environment::ENVIRONMENT_POLL_INTERVAL_MS`]), and every backend that derives
//! its revision asks the operating system for the facts behind it. Windows answered that with a
//! 1 s cache per registry-backed fact, while `high_contrast`, `reduce_motion` and `system_locale`
//! were re-read on every poll; the macOS and JNI-backed Android backends had no cache at all. The
//! same mechanism therefore cost an order of magnitude more on some backends than on others.
//!
//! # Why the wrapper is shared rather than copied
//!
//! A per-backend cache drifts in two ways that matter here: the TTL values diverge, and a
//! notification that is supposed to drop the cache reaches only the backend that remembered to
//! write the invalidation. Both are properties of the *mechanism*, so the mechanism lives once,
//! here, and the backends only declare their slot.
//!
//! # Invalidation
//!
//! Two things make a cached reading stale:
//!
//! * **Time.** [`ENVIRONMENT_TTL`] bounds how long a reading is reused. It is the same one second
//!   Windows already used, far below the rate at which anyone changes a system preference.
//! * **A host notification.** [`crate::platform::environment_notify::bump`] advances a
//!   process-wide revision that
//!   [`Platform::notify_environment_changed`](crate::platform::Platform::notify_environment_changed)
//!   calls by default. Every reading records the revision it was taken at, so a notification
//!   invalidates **every** slot at once — which is exactly the trait's documented promise that
//!   "a backend that caches the facts drops that cache so the next read goes to the host". Without
//!   this, a notified change would still be answered from the old value for up to a second, which
//!   is the mismatch P-201 measured.
//!
//! The cache is not a value source: it never invents a fact. A miss runs the caller's reader, and
//! whatever that reader answers — including `None` for "this backend cannot observe it" — is what
//! is cached and returned.

use crate::compat::{Duration, Instant, Mutex, OnceLock};

/// How long one environment reading is reused.
///
/// One second, the value the Windows backend already used before this module existed: long enough
/// that a 250 ms poll does not repeat a registry or JNI transaction four times, short enough that a
/// change reaches the screen without a notification source.
pub(crate) const ENVIRONMENT_TTL: Duration = Duration::from_secs(1);

/// One fact's slot: when it was read, the notification revision it was read at, and the value.
///
/// The revision is stored **beside** the time rather than folded into it because the two
/// invalidation triggers are independent: a notification at 900 ms must not reuse a reading taken
/// at 100 ms even though the TTL has not expired.
pub(crate) type EnvironmentFactCache<T> = OnceLock<Mutex<Option<(Instant, u64, T)>>>;

/// Returns a cached copy of an environment fact, re-reading it when the TTL or a host notification
/// says the stored one is stale.
///
/// `read` runs only on a miss (a first call, an expired reading, or a notification since the last
/// one), so the OS-facing query is what stays inside the closure and the common path is a
/// mutex-guarded copy. The lock is never held across `read`: a reader that touches a toolkit object
/// must not block every other reader while it waits for that object.
pub(crate) fn cached_environment_fact<T: Copy>(
    cache: &'static EnvironmentFactCache<T>,
    read: impl FnOnce() -> T,
) -> T {
    let revision = crate::platform::environment_notify::generation();
    let slot = cache.get_or_init(|| Mutex::new(None));
    {
        let guard = crate::compat::lock(slot);
        if let Some((sampled_at, sampled_revision, value)) = *guard {
            if sampled_revision == revision && sampled_at.elapsed() < ENVIRONMENT_TTL {
                return value;
            }
        }
    }
    let value = read();
    *crate::compat::lock(slot) = Some((Instant::now(), revision, value));
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cache answers from its slot inside the TTL and re-reads once the reading is old.
    #[test]
    fn a_reading_is_reused_inside_the_ttl_only() {
        // The notification revision is process-wide, so a concurrent bump would look like a change
        // to this cache. See `environment_notify::test_lock`.
        let _guard = crate::platform::environment_notify::test_lock();
        static CACHE: EnvironmentFactCache<u32> = OnceLock::new();
        let mut reads = 0;
        let first = cached_environment_fact(&CACHE, || {
            reads += 1;
            7
        });
        let second = cached_environment_fact(&CACHE, || {
            reads += 1;
            8
        });
        assert_eq!((first, second), (7, 7), "the second call must reuse the reading");
        assert_eq!(reads, 1);

        // Plant a reading older than the TTL: the next call must go back to the reader.
        *crate::compat::lock(CACHE.get_or_init(|| Mutex::new(None))) =
            Some((Instant::now() - ENVIRONMENT_TTL - Duration::from_millis(1), 0, 7));
        let third = cached_environment_fact(&CACHE, || {
            reads += 1;
            9
        });
        assert_eq!(third, 9);
        assert_eq!(reads, 2);
    }

    /// A notification invalidates a reading that is still inside its TTL.
    #[test]
    fn a_notification_invalidates_a_fresh_reading() {
        let _guard = crate::platform::environment_notify::test_lock();
        static CACHE: EnvironmentFactCache<u32> = OnceLock::new();
        let first = cached_environment_fact(&CACHE, || 1);
        assert_eq!(first, 1);
        crate::platform::environment_notify::bump();
        let second = cached_environment_fact(&CACHE, || 2);
        assert_eq!(second, 2, "a bump must make the next read go to the host");
    }
}
