//! PDF metadata, security, and pagination support.
use crate::core::Size;
use std::collections::HashMap;
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
    /// Creation timestamp string.
    pub creation_date: Option<String>,
    /// Last modification timestamp string.
    pub modification_date: Option<String>,
}
