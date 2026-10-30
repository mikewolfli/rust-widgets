// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n options - configuration and initialization types

// `String`/`Vec` resolve through the `compat` bridge rather than the prelude,
// which `#![no_std]` removes on the `mini` profile. `MiniToString` supplies the
// `ToString` trait, which the no-std prelude does not carry either.
use crate::compat::{MiniToString, String, Vec};

/// The language the host's environment asks for, as an i18n language code.
///
/// # Why this lives here rather than in the manager
///
/// The OS query is platform knowledge and stays in the platform layer
/// ([`Platform::system_locale`](crate::platform::Platform::system_locale)); this function only
/// reduces the tag that layer returns to the code a translation file is keyed by. A POSIX locale
/// such as `zh_CN.UTF-8` is normalised by the platform to `zh-CN`, and the **primary** subtag
/// (`zh`) is what a catalogue is named after — `language/zh.json`, not `language/zh-CN.json`.
///
/// Returns `None` when the backend reports no locale (`C`/`POSIX`/unset, or a target with no
/// locale source). `None` is deliberately distinct from `Some("en")`: "the environment did not
/// say" must not be reported as "the environment asked for English", or a caller could not tell
/// whether to fall back.
pub fn system_language() -> Option<String> {
    #[cfg(not(alloc_frugal))]
    {
        crate::platform::get_platform().system_locale().map(primary_language)
    }
    #[cfg(alloc_frugal)]
    {
        // The frugal profile links no environment source, so the platform layer answers `None`
        // there too. Spelled as its own arm rather than a call so the profile keeps compiling
        // without the runtime module.
        None
    }
}

/// The primary subtag of a BCP-47-ish tag, lower-cased.
///
/// `"zh-CN"` and `"zh_CN"` both answer `"zh"`; `"en"` answers `"en"`; an empty tag answers the
/// empty string, which the caller treats as "no answer". Private because it is only meaningful for
/// the value this module's own function accepts.
///
/// Gated with the arm that calls it: the frugal profile has no locale source, so the reduction
/// would be dead code there rather than a reachable fallback.
#[cfg(not(alloc_frugal))]
fn primary_language(tag: &str) -> String {
    let primary = tag.split(['-', '_']).next().unwrap_or(tag);
    primary.to_ascii_lowercase()
}

/// Initialization options for i18n system
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Language code (e.g., "en", "zh", "ja")
    pub language: String,
    /// Directory containing translation files
    pub preload_dir: Option<String>,
    /// Enable diagnostics output
    pub diagnostics: bool,
}
impl Default for InitOptions {
    fn default() -> Self {
        Self { language: "en".to_string(), preload_dir: None, diagnostics: false }
    }
}

impl InitOptions {
    /// Options that start from the system language, falling back to `en`.
    ///
    /// # What "follow the system" means at this level
    ///
    /// [`system_language`] answers the primary subtag of the platform locale (`zh` for
    /// `zh-CN`). This constructor uses it when it is available and keeps `en` when it is not,
    /// which is the same default a caller who wrote `InitOptions::default()` would get — so an
    /// application that never configures a language behaves identically on every host.
    ///
    /// A language the host asks for but has not shipped a catalogue for is **not** corrected
    /// here: the manager already falls back to the `en` catalogue at lookup time, and quietly
    /// rewriting the requested language would hide a packaging gap.
    pub fn from_system() -> Self {
        Self { language: system_language().unwrap_or_else(|| "en".to_string()), ..Self::default() }
    }
}
/// Initialization report
#[derive(Debug, Clone)]
pub struct InitReport {
    /// Number of translation files loaded
    pub files_loaded: usize,
    /// Total number of translations
    pub translations_count: usize,
    /// Any errors that occurred during initialization
    pub errors: Vec<String>,
}
impl InitReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self { files_loaded: 0, translations_count: 0, errors: Vec::new() }
    }
}
crate::impl_default_via_new!(InitReport);
