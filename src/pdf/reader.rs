//! PDF reader and parsing logic.

use crate::core::{Rect, Size};
use crate::pdf::types::*;
use crate::pdf::document::PdfDocumentImpl;
use crate::pdf::PdfDocument;
use crate::pdf::metadata::PdfMetadata;
use crate::pdf::page::PdfPageImpl;
use crate::pdf::security::parse_security_diagnostics;
use std::collections::HashMap;
use std::fs;

/// PDF reader
pub struct PdfReader {
    /// Backend profile name used by reader diagnostics.
    backend_name: &'static str,
}
impl PdfReader {
    /// Create a new PDF reader
    pub fn new() -> Self {
        Self {
            backend_name: "pdf-minimal-v1",
        }
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
        };
        if let Some(security) = parse_security_diagnostics(&text) {
            doc.security = security;
        }
        if parsed_pages.is_empty() {
            for _ in 0..page_count {
                doc.add_page(Size {
                    width: 595,
                    height: 842,
                });
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
impl Default for PdfReader {
    fn default() -> Self {
        Self::new()
    }
}

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
        let size = parse_page_media_box(body).unwrap_or(Size {
            width: 595,
            height: 842,
        });
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
        out.push_str(&format!("{:02X}", byte));
    }
    out
}


