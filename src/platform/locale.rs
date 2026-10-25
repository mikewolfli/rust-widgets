// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Locale interning and the environment-based fallback shared by the desktop backends.
//!
//! # Why an interner
//!
//! [`Platform::system_locale`](crate::platform::Platform::system_locale) returns
//! `Option<&'static str>` because the style layer's environment snapshot stores the tag as a
//! borrowed string and is read once per frame. A backend that leaked a fresh `String` on every
//! call would grow the process for as long as it runs. Interning keeps one leaked value per
//! distinct tag alive for the process, so a repeated read is a lookup rather than an allocation.
//!
//! # Why the environment fallback lives here
//!
//! Wayland, GTK and Redox have no standard synchronous locale query that does not pull in another
//! dependency, but every Unix process already carries the answer in `LC_ALL` / `LC_MESSAGES` /
//! `LANG`. Parsing it is platform knowledge, so it stays in this layer (principle #36) and the
//! backends call one function instead of copying the parsing rules.

use crate::compat::{Mutex, OnceLock, String};
use crate::core::Color;
use crate::platform::Platform;

/// The interned locale tags, keyed by their own leaked text.
///
/// # Why a map and not one "current" slot
///
/// The original interner held a single `Option<&'static str>` and replaced it whenever the host
/// reported a different tag. Two threads (or a test suite running platform-locale tests in
/// parallel) could then observe the same tag through two different pointers: one caller read the
/// slot, another replaced it, and the first caller's next read returned a fresh leak. The
/// contract — repeated reads of the **same tag** return the same pointer — is a property of the
/// tag, not of the most recent call, so the map stores it per tag. The number of distinct locales
/// a process sees is tiny, so the memory this keeps is bounded by real host behaviour rather than
/// by call count.
#[cfg_attr(alloc_frugal, allow(dead_code))]
fn cache() -> &'static Mutex<crate::compat::HashMap<&'static str, &'static str>> {
    static CACHE: OnceLock<Mutex<crate::compat::HashMap<&'static str, &'static str>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(crate::compat::HashMap::new()))
}

/// Interns `value`, returning the process-lifetime tag.
///
/// The returned slice is valid for the rest of the process. Each distinct tag is leaked once;
/// repeated reads of the same tag return the same pointer, in any order and from any thread.
#[cfg_attr(alloc_frugal, allow(dead_code))]
pub(crate) fn intern(value: Option<String>) -> Option<&'static str> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    let mut guard = crate::compat::lock(cache());
    if let Some(existing) = guard.get(value.as_str()) {
        return Some(*existing);
    }
    let leaked: &'static str = crate::compat::Box::leak(value.into_boxed_str());
    guard.insert(leaked, leaked);
    Some(leaked)
}

/// The locale named by the process environment, normalized to a BCP-47-ish tag.
///
/// The precedence is the POSIX one: `LC_ALL` overrides `LC_MESSAGES`, which overrides `LANG`.
/// `LANGUAGE` is a GNU gettext extension and is consulted last. `C`, `POSIX` and an unset variable
/// answer `None` — "the locale is not specified", which is a different fact from "English".
///
/// This is the allocating spelling of [`interned_from_environment`], kept for the callers that
/// hand the tag straight to [`intern`]: it is one read away from the cached, interned slice, so the
/// four `std::env::var` lookups and the normalization happen once per
/// [`ENVIRONMENT_TTL`](crate::platform::environment_cache::ENVIRONMENT_TTL) rather than once per
/// poll (P-04).
///
/// # Why only the Apple backends still call this
///
/// The Unix desktop backends moved to [`interned_from_environment`], which is the same reading
/// without the second allocation. The Apple backends answer from their own native locale API first
/// and use this only as the fallback when that answers nothing, which is why the allocating
/// spelling is still reachable — and why the dead-code allowance is narrowed to non-Apple builds
/// rather than the old "not unix and not Apple" one.
#[cfg(not(alloc_frugal))]
#[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
pub(crate) fn from_environment() -> Option<String> {
    interned_from_environment().map(String::from)
}

/// The locale named by the process environment, interned and cached.
///
/// # Why the cache is here
///
/// A backend that answers `system_locale` from the POSIX variables used to read four environment
/// variables and normalize/replace the result on **every** call, and the style layer calls it on
/// every 250 ms environment poll. The answer only changes when the process environment changes,
/// which no running process does often, so the reading is reused for
/// [`ENVIRONMENT_TTL`](crate::platform::environment_cache::ENVIRONMENT_TTL) and dropped early when
/// a host notification says the facts may have changed (P-02/P-04).
///
/// The value is interned once, so the returned slice is the same pointer every time — the property
/// the style layer's snapshot depends on.
#[cfg(not(alloc_frugal))]
#[cfg_attr(not(any(unix_desktop, target_vendor = "apple")), allow(dead_code))]
pub(crate) fn interned_from_environment() -> Option<&'static str> {
    static CACHE: crate::platform::environment_cache::EnvironmentFactCache<Option<&'static str>> =
        OnceLock::new();
    crate::platform::environment_cache::cached_environment_fact(&CACHE, || {
        let tag = ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find_map(|value| normalize(&value));
        intern(tag)
    })
}

/// The locale named by the process environment, for the allocation-frugal profile.
///
/// `mini` is `no_std`, so there is no `std::env` to read and this backend cannot observe the
/// POSIX locale variables at all. The honest answer is `None` ("this backend does not report a
/// locale"), not a hardcoded `en-US`: the style layer treats a missing tag as "unknown" and
/// leaves its own default in place, while a fabricated tag would silently pin every mini host to
/// one language.
///
/// This arm exists so a backend can call [`from_environment`] unconditionally. Without it the
/// call site had to be `cfg`-split per profile, and `linux`/`macos` were missed — the Linux
/// `mini` build failed with `cannot find function from_environment` (feedback-20261019 §4).
///
/// Only the Apple backends still call it: the Unix desktop backends answer from
/// [`interned_from_environment`] in both profiles, so the dead-code allowance matches the
/// non-frugal arm's.
#[cfg(alloc_frugal)]
#[cfg_attr(not(target_vendor = "apple"), allow(dead_code))]
pub(crate) fn from_environment() -> Option<String> {
    None
}

/// See the `not(alloc_frugal)` arm: a profile with no `std::env` reports no locale.
#[cfg(alloc_frugal)]
#[cfg_attr(not(any(unix_desktop, target_vendor = "apple")), allow(dead_code))]
pub(crate) fn interned_from_environment() -> Option<&'static str> {
    None
}

/// Normalizes a POSIX locale value such as `de_DE.UTF-8@euro` to a BCP-47 tag (`de-DE`).
///
/// The encoding and modifier are removed because they describe the C library's `LC_CTYPE`, not
/// the user's language. A tag with no territory (`de`) is kept as-is. `C`/`POSIX` answer `None`.
#[cfg(not(alloc_frugal))]
#[cfg_attr(not(any(unix_desktop, target_vendor = "apple")), allow(dead_code))]
pub(crate) fn normalize(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "C" || value == "POSIX" {
        return None;
    }
    let without_modifier = value.split('@').next().unwrap_or(value);
    let without_encoding = without_modifier.split('.').next().unwrap_or(without_modifier);
    let tag = without_encoding.replace('_', "-");
    if tag.is_empty() {
        None
    } else {
        Some(tag)
    }
}

/// The environment revision of `platform`, over the **complete** fact set the style snapshot
/// consumes.
///
/// # Why this is the shared accessor (P-06)
///
/// The eight backends that derive their revision used to spell out a four-fact hash of their own
/// reads (`system_theme`, `high_contrast`, `reduce_motion`, `system_locale`) verbatim. Adding a
/// fact then meant editing eight files, which is exactly why P-03's accent and reduced-transparency
/// facts were missing from all of them. Every backend whose revision is derived answers
/// `environment_revision(self)` and nothing else, so a new fact is added here once.
///
/// The hash covers, in order: appearance, high contrast, reduced motion, locale, accent colour and
/// reduced transparency — the six facts [`crate::style::environment::EnvironmentSnapshot`] reads.
/// A backend that cannot observe one of them answers `None` through the trait default, which the
/// hash distinguishes from `Some(false)`; the previous four-fact spelling could not see an accent
/// or transparency change at all, so the snapshot kept its stale value indefinitely (P-03).
///
/// Returns `0` when the backend read nothing: zero means "this backend does not report environment
/// changes", which is the same contract `display_generation` uses.
///
/// # Why the parameter is `&dyn Platform` rather than the six values
///
/// So the fact set is decided here, once. A caller passing the values could omit one, which is the
/// drift this function exists to remove; the type system cannot check a list of six positional
/// arguments for completeness, but it can check that this function is spelled the same way in
/// every backend.
///
/// [`Platform::environment_generation`]: crate::platform::Platform::environment_generation
pub(crate) fn environment_revision(platform: &dyn Platform) -> u64 {
    hash_facts(
        platform.system_theme(),
        platform.high_contrast(),
        platform.reduce_motion(),
        platform.system_locale(),
        platform.accent_color(),
        platform.reduce_transparency(),
    )
}

/// The one FNV-1a mixing implementation behind [`environment_revision`].
///
/// Ordered so the four facts that were always hashed keep their positions; the accent and the
/// transparency preference were appended, which is invisible to callers because the value is an
/// opaque revision rather than a persisted one.
///
/// Private and six-parameter by design: the only way to produce a revision is
/// [`environment_revision`] (from a platform) or the tests below (with explicit facts), so no
/// caller can hash a subset of the facts and reintroduce P-03.
fn hash_facts(
    color_scheme: Option<bool>,
    high_contrast: Option<bool>,
    reduce_motion: Option<bool>,
    locale: Option<&str>,
    accent: Option<Color>,
    reduce_transparency: Option<bool>,
) -> u64 {
    if color_scheme.is_none()
        && high_contrast.is_none()
        && reduce_motion.is_none()
        && locale.is_none()
        && accent.is_none()
        && reduce_transparency.is_none()
    {
        return 0;
    }
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    let mut mix = |part: u64| {
        hash ^= part;
        hash = hash.wrapping_mul(FNV_PRIME);
    };
    for fact in [color_scheme, high_contrast, reduce_motion] {
        match fact {
            None => mix(0),
            Some(false) => mix(1),
            Some(true) => mix(2),
        }
    }
    match locale {
        None => mix(0),
        Some(tag) => {
            mix(1);
            for byte in tag.bytes() {
                mix(u64::from(byte));
            }
        }
    }
    match accent {
        None => mix(0),
        Some(color) => {
            // Presence byte, then the four channels: two colours must not collide, and a colour
            // must not collide with the "not observed" value.
            mix(1);
            for channel in [color.r, color.g, color.b, color.a] {
                mix(u64::from(channel));
            }
        }
    }
    match reduce_transparency {
        None => mix(0),
        Some(false) => mix(1),
        Some(true) => mix(2),
    }
    hash
}

#[cfg(all(test, not(alloc_frugal)))]
mod tests {
    use super::*;

    #[test]
    fn normalization_strips_encoding_and_modifier() {
        assert_eq!(normalize("de_DE.UTF-8@euro").as_deref(), Some("de-DE"));
        assert_eq!(normalize("pt_BR.utf8").as_deref(), Some("pt-BR"));
        assert_eq!(normalize("ja").as_deref(), Some("ja"));
        assert_eq!(normalize("C"), None);
        assert_eq!(normalize("POSIX"), None);
        assert_eq!(normalize(""), None);
    }

    #[test]
    fn interning_is_stable_and_replaces_on_change() {
        let first = intern(Some(String::from("en-US"))).expect("a tag");
        let again = intern(Some(String::from("en-US"))).expect("a tag");
        assert_eq!(first.as_ptr(), again.as_ptr(), "the same tag must be interned once");
        let second = intern(Some(String::from("fr-FR"))).expect("a tag");
        assert_eq!(second, "fr-FR");
        let third = intern(Some(String::from("fr-FR"))).expect("a tag");
        assert_eq!(second.as_ptr(), third.as_ptr());
        // A tag interned **before** another one intervened must still be the same pointer. The
        // single-slot interner failed this: reading `en-US` again after `fr-FR` leaked a second
        // copy, so two callers of the same tag compared unequal.
        let first_again = intern(Some(String::from("en-US"))).expect("a tag");
        assert_eq!(
            first.as_ptr(),
            first_again.as_ptr(),
            "a tag's pointer must not depend on order"
        );
        // Return the interner to the first value so later tests see a deterministic state.
        let _ = intern(Some(String::from("en-US")));
    }

    #[test]
    fn empty_and_absent_values_answer_none() {
        assert_eq!(intern(None), None);
        assert_eq!(intern(Some(String::new())), None);
    }

    /// The four original facts, hashed with the two newer ones absent.
    fn core(
        theme: Option<bool>,
        contrast: Option<bool>,
        motion: Option<bool>,
        locale: Option<&str>,
    ) -> u64 {
        hash_facts(theme, contrast, motion, locale, None, None)
    }

    #[test]
    fn generation_is_zero_only_when_nothing_was_read() {
        assert_eq!(core(None, None, None, None), 0);
        let baseline = core(Some(false), Some(false), Some(false), Some("en-US"));
        assert_ne!(baseline, 0);
        assert_ne!(baseline, core(Some(true), Some(false), Some(false), Some("en-US")));
        assert_ne!(baseline, core(Some(false), Some(true), Some(false), Some("en-US")));
        assert_ne!(baseline, core(Some(false), Some(false), Some(true), Some("en-US")));
        assert_ne!(baseline, core(Some(false), Some(false), Some(false), Some("de-DE")));
        assert_eq!(baseline, core(Some(false), Some(false), Some(false), Some("en-US")));
    }

    /// The complete revision reacts to the two facts the four-fact core could not see.
    ///
    /// # The defect this pins
    ///
    /// `Platform::environment_generation` hashed appearance, contrast, motion and locale only,
    /// while the style snapshot also reads `accent_color()` and `reduce_transparency()`. A user
    /// who changed either kept the old snapshot indefinitely, because the style layer reuses its
    /// cached snapshot whenever the revision is unchanged (P-03).
    #[test]
    fn accent_and_transparency_are_part_of_the_revision() {
        let baseline = hash_facts(
            Some(false),
            Some(false),
            Some(false),
            Some("en-US"),
            Some(Color { r: 0, g: 122, b: 255, a: 255 }),
            Some(false),
        );
        assert_ne!(baseline, 0, "a fully observed environment is a real revision");
        // A different accent is a different revision even though nothing else moved.
        let other_accent = hash_facts(
            Some(false),
            Some(false),
            Some(false),
            Some("en-US"),
            Some(Color { r: 255, g: 59, b: 48, a: 255 }),
            Some(false),
        );
        assert_ne!(baseline, other_accent);
        // Turning the transparency preference on is too.
        let other_transparency = hash_facts(
            Some(false),
            Some(false),
            Some(false),
            Some("en-US"),
            Some(Color { r: 0, g: 122, b: 255, a: 255 }),
            Some(true),
        );
        assert_ne!(baseline, other_transparency);
        // "not observed" is not the same fact as "observed and off".
        let unobserved =
            hash_facts(Some(false), Some(false), Some(false), Some("en-US"), None, None);
        assert_ne!(baseline, unobserved);
    }

    /// A platform that reports only an accent still gets a non-zero revision.
    ///
    /// This is the case the old "zero when the four facts are all `None`" rule answered `0` to,
    /// which the style layer reads as "this backend cannot report changes" — so an accent-only
    /// backend would never have been re-read.
    #[test]
    fn an_accent_only_backend_reports_a_revision() {
        struct AccentOnly(Option<Color>);
        impl Platform for AccentOnly {
            fn as_any(&self) -> &dyn core::any::Any {
                self
            }
            fn backend_name(&self) -> &'static str {
                "accent-only-test"
            }
            fn family(&self) -> crate::core::PlatformFamily {
                crate::core::PlatformFamily::Desktop
            }
            fn accent_color(&self) -> Option<Color> {
                self.0
            }
            fn init(&self) {}
            fn run(&self) {}
            fn quit(&self) -> bool {
                true
            }
            fn create_window(
                &self,
                _title: &str,
                _x: i32,
                _y: i32,
                _w: u32,
                _h: u32,
            ) -> crate::core::ObjectId {
                0
            }
        }

        assert_eq!(environment_revision(&AccentOnly(None)), 0);
        assert_ne!(environment_revision(&AccentOnly(Some(Color { r: 1, g: 2, b: 3, a: 255 }))), 0);
    }
}
