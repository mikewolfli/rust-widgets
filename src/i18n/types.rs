// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n types - data structures for internationalization
use crate::compat::{HashMap, String};
use serde::{Deserialize, Serialize};
/// Translation entry
///
/// One message in one language, as loaded from a translation file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Translation {
    /// Optional disambiguation tag for lookups. When several entries share a
    /// `message`, the context distinguishes them; `None` means the message is
    /// unambiguous on its own.
    pub context: Option<String>,
    /// The translated text for the singular form. Also the key's human-readable
    /// identity in the source language.
    pub message: String,
    /// Plural forms keyed by *category number*, not by count: the loader or
    /// lookup code is responsible for mapping a quantity to the right key, so
    /// the meaning of each number is defined by the translation file rather than
    /// by this type. `None` means the message has no plural forms.
    pub plural: Option<HashMap<u32, String>>,
}
/// Translation file
///
/// The on-disk (or embedded) unit for one language: a flat map from lookup key
/// to [`Translation`] entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct TranslationFile {
    /// BCP 47-ish language code this file provides, for example `"en"` or
    /// `"en-US"`. Stored as given; no normalisation is performed, so `"en"` and
    /// `"en-US"` are different languages to a consumer.
    pub language: String,
    /// The messages themselves, keyed by the lookup string callers pass to the
    /// translation API.
    pub translations: HashMap<String, Translation>,
}
/// Reload event notification
///
/// Reports the outcome of an attempt to reload translation data at runtime, so
/// a listener can react to both success and failure without polling.
#[derive(Debug, Clone)]
pub enum ReloadEvent {
    /// Translation file was reloaded
    TranslationReloaded {
        /// Language code of the file that was loaded.
        language: String,
        /// When the reload completed, taken from the system clock; used for
        /// ordering events, so it can move backwards if the clock is adjusted.
        timestamp: std::time::SystemTime,
    },
    /// Error during reload
    ReloadError {
        /// Language code of the file that failed to load.
        language: String,
        /// Human-readable failure description. Not structured, so a consumer
        /// wanting to distinguish causes must match on the text.
        error: String,
    },
}
