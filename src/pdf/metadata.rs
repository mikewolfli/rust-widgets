// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF metadata, security, and pagination support.
/// Standard document-information (Info dictionary) fields for a generated PDF.
///
/// Every field is free-form; the defaults below are the values written when the
/// caller does not override them.
pub struct PdfMetadata {
    /// Document title.
    pub title: String,
    /// Document author.
    pub author: String,
    /// Document subject.
    pub subject: String,
    /// Document keywords.
    pub keywords: Vec<String>,
    /// Application creating the source document.
    pub creator: String,
    /// PDF producer implementation name.
    pub producer: String,
    /// Creation timestamp string. Carried through verbatim; this library does
    /// not parse or re-format it, so the caller owns the convention (PDF's own
    /// is `D:YYYYMMDDHHmmSS`).
    pub creation_date: Option<String>,
    /// Last modification timestamp string. Verbatim, as for `creation_date`.
    pub modification_date: Option<String>,
}
impl Default for PdfMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            subject: String::new(),
            keywords: Vec::new(),
            creator: "Rust Widgets PDF".to_string(),
            producer: "Rust Widgets PDF Library".to_string(),
            creation_date: None,
            modification_date: None,
        }
    }
}
