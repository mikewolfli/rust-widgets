// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! XDG desktop portal appearance settings for the Wayland backend (BLUE28 B28.04).
//!
//! Wayland's core protocol carries no desktop-wide colour preference. The portal is the
//! cross-desktop answer: `org.freedesktop.portal.Settings.Read` with the namespace
//! `org.freedesktop.appearance` and a key such as `color-scheme` (`0` no preference, `1` dark,
//! `2` light) or `contrast` (`0`/`1`).
//!
//! # Honest absence
//!
//! A headless compositor (the weston CI session) has no portal; the D-Bus name is missing, the
//! call fails, and every accessor answers `None`. That is the documented "cannot tell" answer,
//! not a fabricated light theme.
//!
//! # Why the result is cached
//!
//! These accessors are synchronous and can be called from the frame path. A blocking D-Bus call
//! per frame would be unacceptable, so each key is cached for [`CACHE_TTL`]. The first lookup is
//! bounded by [`METHOD_TIMEOUT`], which also bounds a session whose portal is present but wedged.

#![cfg(all(feature = "wayland-portal", unix_desktop))]

use crate::compat::{lock, Instant, Mutex};
use std::time::Duration;

/// How long a successful or failed lookup is reused.
const CACHE_TTL: Duration = Duration::from_secs(2);
/// How long one portal method may take before it counts as absent.
const METHOD_TIMEOUT: Duration = Duration::from_millis(400);

/// One cached value and when it was read.
struct Cached {
    at: Instant,
    value: Option<u32>,
}

static COLOR_SCHEME: Mutex<Option<Cached>> = Mutex::new(None);
static CONTRAST: Mutex<Option<Cached>> = Mutex::new(None);

/// The desktop's colour scheme, or `None` when no preference was reported.
///
/// `true` is dark and `false` is light. The portal's `0` ("no preference") and a missing portal
/// both answer `None`, because neither names a scheme.
pub(crate) fn color_scheme() -> Option<bool> {
    color_scheme_from(read_u32("color-scheme", &COLOR_SCHEME))
}

/// Whether the desktop asks for increased contrast, or `None` when the portal does not report it.
pub(crate) fn contrast() -> Option<bool> {
    contrast_from(read_u32("contrast", &CONTRAST))
}

/// Maps the portal's `color-scheme` value: `1` dark, `2` light, `0`/absent no preference.
fn color_scheme_from(value: Option<u32>) -> Option<bool> {
    match value? {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// Maps the portal's `contrast` value: `1` increased contrast, `0` normal.
fn contrast_from(value: Option<u32>) -> Option<bool> {
    value.map(|value| value == 1)
}

/// Reads one `org.freedesktop.appearance` key, caching the answer for [`CACHE_TTL`].
fn read_u32(key: &str, cache: &Mutex<Option<Cached>>) -> Option<u32> {
    {
        let guard = lock(cache);
        if let Some(cached) = guard.as_ref() {
            if cached.at.elapsed() < CACHE_TTL {
                return cached.value;
            }
        }
    }

    let value = query(key);
    *lock(cache) = Some(Cached { at: Instant::now(), value });
    value
}

/// One blocking portal lookup. `None` on any failure (no bus, no portal, no key).
fn query(key: &str) -> Option<u32> {
    let connection = zbus::blocking::connection::Builder::session()
        .ok()?
        .method_timeout(METHOD_TIMEOUT)
        .build()
        .ok()?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .ok()?;
    let value: zbus::zvariant::OwnedValue =
        proxy.call("Read", &("org.freedesktop.appearance", key)).ok()?;
    value.downcast_ref::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache must reuse a reading rather than call the bus twice in a row.
    ///
    /// A fake value is installed directly; `query` is not called because a host without a portal
    /// must not make this test depend on one.
    #[test]
    fn a_cached_reading_is_reused_until_it_expires() {
        let cache = Mutex::new(Some(Cached { at: Instant::now(), value: Some(1) }));
        assert_eq!(read_u32("color-scheme", &cache), Some(1));
        // The reading is still fresh, so the same value is answered without a bus round trip.
        assert_eq!(read_u32("color-scheme", &cache), Some(1));
    }

    /// An expired entry is not reused; without a bus the fresh answer is `None`.
    #[test]
    fn an_expired_reading_is_not_answered() {
        let cache = Mutex::new(Some(Cached {
            at: Instant::now() - CACHE_TTL - Duration::from_secs(1),
            value: Some(2),
        }));
        // There is no portal in the test host, so the expired value must not survive.
        assert_eq!(read_u32("color-scheme", &cache), None);
    }

    #[test]
    fn the_scheme_mapping_distinguishes_no_preference_from_light() {
        assert_eq!(color_scheme_from(Some(1)), Some(true));
        assert_eq!(color_scheme_from(Some(2)), Some(false));
        // `0` is "no preference": the accessor must not turn it into `false`.
        assert_eq!(color_scheme_from(Some(0)), None);
        assert_eq!(color_scheme_from(None), None);
        assert_eq!(contrast_from(Some(1)), Some(true));
        assert_eq!(contrast_from(Some(0)), Some(false));
        assert_eq!(contrast_from(None), None);
    }
}
