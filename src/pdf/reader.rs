// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF reader and parsing logic.
//!
//! # Known Limitations
//!
//! This is a minimal PDF reader implementation. The following features are NOT
//! yet supported:
//!
//! - **Tokenization**: The parser operates on a line-level basis rather than
//!   proper PDF tokenization. A proper `Lexer` should be implemented to
//!   produce tokens for names (`/Name`), numbers, strings, hex strings,
//!   dictionaries (`<< ... >>`), arrays (`[ ... ]`), etc.
//! - **Cross-reference stream (XRefStm)**: Only classic cross-reference tables
//!   are parsed; cross-reference streams (introduced in PDF 1.5) are ignored.
//! - **Object streams (ObjStm)**: Compressed objects stored inside object
//!   streams are not supported. All objects must be direct or in classic
//!   indirect-object format (`N 0 obj ... endobj`).
//! - **Compressed object data**: Streams using FlateDecode, LZWDecode, or
//!   other compression filters are not decompressed.
//! - **Encryption**: Security is detected. A document that carries a standard
//!   `/Encrypt` dictionary is reported through [`PdfSecurity`] (permissions and a
//!   non-empty `user_password` marker), and its `/Encrypt` reference is kept in the
//!   security marker text. The reader does **not** decrypt encrypted content streams:
//!   a stream opened as a `/AESV3` hex string is not fed through a PDF string parser,
//!   so encrypted payloads are surfaced as-is rather than silently mis-decoded.
//! - **Font subsetting / CID fonts**: Only core Helvetica is used as a
//!   default font resource. Embedded font programs are not parsed.
//! - **Interactive form filling**: Form field definitions are stored but
//!   not populated from the PDF's AcroForm structure.
//! - **Page tree traversal**: Only flat `/Type /Page` objects are detected;
//!   page-tree nodes with `/Type /Pages` and `/Kids` arrays are not
//!   recursively resolved.
//!
//! These limitations mean the reader works correctly only for simple,
//! uncompressed PDFs with no object streams and flat page structures.
//! For production use, consider integrating a more complete PDF library.

use crate::core::Size;
use crate::pdf::metadata::PdfMetadata;
use crate::pdf::page::PdfPageImpl;
use crate::pdf::security::parse_security_diagnostics;
use crate::pdf::types::*;
use crate::pdf::PdfDocument;
use crate::pdf::PdfDocumentImpl;
use std::collections::HashMap;
use std::fs;

/// Detects whether the document declares a standard security handler.
///
/// Matches the `/Encrypt` trailer entry (or a `/Filter /Standard` dictionary) that
/// the writer emits once document-level encryption is applied. Used to distinguish
/// "a real encryption dictionary is present" from "only an intent marker was
/// written", so the recovered [`PdfSecurity`] does not mis-report an encrypted
/// document as open.
fn declares_encryption(text: &str) -> bool {
    text.contains("/Encrypt ")
        || (text.contains("/Filter /Standard") && text.contains("/CFM /AESV3"))
}

/// Recover as much of the original [`PdfSecurity`] as the file actually proves.
///
/// For a real encryption dictionary the passwords are single-use and cannot be
/// recovered from the file, but a non-empty `user_password` is set as an honest
/// "this document is encrypted and needs a password" marker (it is never the real
/// secret). Documents that carry only the legacy `% RW-NOTE` intent marker parse
/// through unchanged.
fn recover_security(text: &str) -> Option<PdfSecurity> {
    // An applied standard handler is the strongest evidence of the file's state;
    // its `/P` entry is authoritative and must win over the intent marker text.
    if declares_encryption(text) {
        let permissions = parse_permission_value(text);
        return Some(PdfSecurity {
            user_password: Some(String::from("encrypted")),
            owner_password: None,
            print_permission: permissions & (1 << 2) != 0,
            edit_permission: permissions & (1 << 3) != 0,
            copy_permission: permissions & (1 << 4) != 0,
            annotation_permission: permissions & (1 << 5) != 0,
        });
    }
    // Fall back to the legacy `% RW-NOTE` intent marker, which carries permission
    // flags but means the document itself is not encrypted.
    parse_security_diagnostics(text)
}

/// Parse the integer following a `/P ` entry in an encryption dictionary.
///
/// `/P` is a signed 32-bit value, so a negative permission word (almost all of
/// them, since the reserved high bits are set) must be parsed as signed and then
/// reinterpreted as the raw bit pattern. Only candidates whose following token is
/// actually an integer are accepted, otherwise the catalog's `/Pages` would be
/// mistaken for the permission entry.
fn parse_permission_value(text: &str) -> u32 {
    for (index, _) in text.match_indices("/P ") {
        let rest = &text[index + "/P ".len()..];
        let token: String =
            rest.chars().take_while(|ch| ch.is_ascii_digit() || *ch == '-').collect();
        if let Ok(value) = token.parse::<i64>() {
            return value as u32;
        }
    }
    0
}

/// PDF reader
pub struct PdfReader {
    /// Backend profile name used by reader diagnostics.
    backend_name: &'static str,
}
impl PdfReader {
    /// Create a new PDF reader
    pub fn new() -> Self {
        Self { backend_name: "pdf-minimal-v1" }
    }
    /// Load PDF from file
    pub fn load(&self, path: &str) -> Result<Box<dyn PdfDocument>, std::io::Error> {
        let bytes = fs::read(path)?;
        self.load_from_bytes(&bytes)
    }
    /// Load PDF from bytes
    pub fn load_from_bytes(&self, data: &[u8]) -> Result<Box<dyn PdfDocument>, std::io::Error> {
        let text = String::from_utf8_lossy(data);
        let mut page_count = 1;
        // Legacy fallback parser for older fake format.
        for line in text.lines() {
            if let Some(num) = line.strip_prefix("pages:") {
                if let Ok(parsed) = num.parse::<usize>() {
                    page_count = parsed.max(1);
                }
            }
        }
        // Parse real PDF `/Count N` tokens and use the largest value found.
        for token in text.split("/Count ").skip(1) {
            let digits: String = token.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            if let Ok(parsed) = digits.parse::<usize>() {
                page_count = page_count.max(parsed.max(1));
            }
        }
        let parsed_pages = parse_pdf_pages(&text);
        let mut doc = PdfDocumentImpl {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            security: PdfSecurity::default(),
            fonts: vec![PdfFontResource::core_helvetica("F1")],
            pagination: PdfPagination::default(),
            annotation_manager: crate::pdf::annotation::AnnotationManager::new(),
            hyperlink_manager: crate::pdf::hyperlink::HyperlinkManager::new(),
        };
        if let Some(security) = recover_security(&text) {
            doc.security = security;
        }
        if parsed_pages.is_empty() {
            for _ in 0..page_count {
                doc.add_page(Size { width: 595, height: 842 });
            }
        } else {
            for page in parsed_pages {
                let page_impl = PdfPageImpl {
                    size: page.size,
                    content: page.content,
                    font_resource: doc.default_font_resource().to_string(),
                    form_fields: HashMap::new(),
                };
                doc.pages.push(Box::new(page_impl));
            }
        }
        Ok(Box::new(doc))
    }
    /// Return active reader backend name.
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}
crate::impl_default_via_new!(PdfReader);

pub(crate) fn parse_pdf_pages(text: &str) -> Vec<ParsedPdfPage> {
    let objects = parse_pdf_objects(text);
    if objects.is_empty() {
        return Vec::new();
    }
    let mut pages = Vec::new();
    for body in objects.values() {
        let is_page_object = body.contains("/Type /Page ")
            || body.contains("/Type /Page\n")
            || body.contains("/Type /Page\r");
        if !is_page_object {
            continue;
        }
        let size = parse_page_media_box(body).unwrap_or(Size { width: 595, height: 842 });
        let content_obj_id = parse_contents_object_id(body);
        let content = content_obj_id
            .and_then(|id| objects.get(&id))
            .and_then(|content_body| extract_stream(content_body))
            .map(|stream| stream.as_bytes().to_vec())
            .unwrap_or_default();
        pages.push(ParsedPdfPage { size, content });
    }
    pages
}
pub(crate) fn parse_pdf_objects(text: &str) -> HashMap<u32, String> {
    let mut objects = HashMap::new();
    let mut current_id: Option<u32> = None;
    let mut body = String::new();
    for line in text.lines() {
        if current_id.is_none() {
            let mut parts = line.split_whitespace();
            if let (Some(id), Some(generation), Some(obj_kw)) =
                (parts.next(), parts.next(), parts.next())
            {
                if generation == "0" && obj_kw == "obj" {
                    if let Ok(parsed_id) = id.parse::<u32>() {
                        current_id = Some(parsed_id);
                        body.clear();
                    }
                }
            }
            continue;
        }
        if line.trim() == "endobj" {
            if let Some(id) = current_id.take() {
                objects.insert(id, body.clone());
            }
            body.clear();
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    objects
}
pub(crate) fn parse_page_media_box(page_obj: &str) -> Option<Size> {
    let marker = "/MediaBox [0 0 ";
    let start = page_obj.find(marker)? + marker.len();
    let rest = &page_obj[start..];
    let mut parts = rest.split_whitespace();
    let width = parts.next()?.trim().parse::<u32>().ok()?;
    let height_raw = parts.next()?;
    let height = height_raw.trim_end_matches(']').parse::<u32>().ok()?;
    Some(Size { width, height })
}
pub(crate) fn parse_contents_object_id(page_obj: &str) -> Option<u32> {
    let marker = "/Contents ";
    let start = page_obj.find(marker)? + marker.len();
    let rest = &page_obj[start..];
    let id = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u32>()
        .ok()?;
    Some(id)
}
pub(crate) fn extract_stream(content_obj: &str) -> Option<&str> {
    let stream_start = content_obj.find("stream\n")? + "stream\n".len();
    let rest = &content_obj[stream_start..];
    let stream_end = rest.find("\nendstream")?;
    Some(&rest[..stream_end])
}
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02X}"));
    }
    out
}
