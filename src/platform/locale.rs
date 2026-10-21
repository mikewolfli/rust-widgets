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
#[cfg(not(alloc_frugal))]
#[cfg_attr(not(any(unix_desktop, target_vendor = "apple")), allow(dead_code))]
pub(crate) fn from_environment() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find_map(|value| normalize(&value))
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
#[cfg(alloc_frugal)]
#[cfg_attr(not(any(unix_desktop, target_vendor = "apple")), allow(dead_code))]
pub(crate) fn from_environment() -> Option<String> {
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

/// A revision of the environment facts, for [`Platform::environment_generation`].
///
/// Returns `0` when the backend read nothing (`None` for all four facts): zero means "this backend
/// does not report environment changes", which is the same contract `display_generation` uses.
/// Otherwise the value is an FNV-1a hash of the facts, so any change produces a different number.
/// The optional facts are distinguished from each other and from their `false` value by hashing a
/// presence byte before the value.
///
/// [`Platform::environment_generation`]: crate::platform::Platform::environment_generation
pub(crate) fn generation_hash(
    color_scheme: Option<bool>,
    high_contrast: Option<bool>,
    reduce_motion: Option<bool>,
    locale: Option<&str>,
) -> u64 {
    if color_scheme.is_none()
        && high_contrast.is_none()
        && reduce_motion.is_none()
        && locale.is_none()
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

    #[test]
    fn generation_is_zero_only_when_nothing_was_read() {
        assert_eq!(generation_hash(None, None, None, None), 0);
        let baseline = generation_hash(Some(false), Some(false), Some(false), Some("en-US"));
        assert_ne!(baseline, 0);
        assert_ne!(baseline, generation_hash(Some(true), Some(false), Some(false), Some("en-US")));
        assert_ne!(baseline, generation_hash(Some(false), Some(true), Some(false), Some("en-US")));
        assert_ne!(baseline, generation_hash(Some(false), Some(false), Some(true), Some("en-US")));
        assert_ne!(baseline, generation_hash(Some(false), Some(false), Some(false), Some("de-DE")));
        assert_eq!(baseline, generation_hash(Some(false), Some(false), Some(false), Some("en-US")));
    }
}
