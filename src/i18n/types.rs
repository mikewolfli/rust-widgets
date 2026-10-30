// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! i18n types - data structures for internationalization
use crate::compat::{HashMap, String, Vec};
use serde::{Deserialize, Serialize};

/// The two directions of a key-set comparison between a catalogue and a required set.
///
/// # Why both directions, and why they are different defects
///
/// A **missing** key is a label the program will paint as a raw identifier (or in the fallback
/// language): the catalogue is incomplete, and the file should be refused at load time rather
/// than shipping. An **unexpected** key is a label nothing asks for: usually a stale entry left
/// behind by a rename, which silently accumulates and makes a translator's work look complete when
/// part of it is dead. Reporting them separately is what lets a tool say which of the two it found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyAudit {
    /// Keys the caller requires that the catalogue does not provide, sorted.
    pub missing: Vec<String>,
    /// Keys the catalogue provides that the caller does not require, sorted.
    pub unexpected: Vec<String>,
}

impl KeyAudit {
    /// Whether both directions are empty — the catalogue is exactly the required set.
    pub fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.unexpected.is_empty()
    }
}
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
