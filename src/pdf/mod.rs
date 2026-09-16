// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF generation, parsing, and document manipulation.

/// Annotation dictionaries attached to pages (links, notes, markup).
pub mod annotation;
/// In-memory document and page tree model built by the writer and consumed by
/// the reader.
pub mod document;
/// Renders widget trees into PDF files via the SVG pipeline.
pub mod export;
/// AcroForm field model (text fields, checkboxes, choices).
pub mod form;
/// Hyperlink targets and internal/external link actions.
pub mod hyperlink;
/// Document-information (Info dictionary) metadata.
pub mod metadata;
/// Page geometry, rotation, and content-stream access.
pub mod page;
/// Parses an existing PDF back into the document model.
pub mod reader;
/// Encryption and permission flags.
pub mod security;
#[cfg(test)]
mod tests;
/// The `PdfDocument` / writer trait surface shared by the backends.
pub mod traits;
/// Shared PDF value types (page size, colour space, version).
pub mod types;
pub mod writer;

pub(crate) use crate::pdf::document::*;
pub use crate::pdf::export::*;
pub use crate::pdf::reader::*;
pub use crate::pdf::traits::*;
pub use crate::pdf::types::*;
pub use crate::pdf::writer::*;
