//! PDF document abstraction and in-memory implementation.
//!
//! # Coordinate System
//!
//! PDF uses a **bottom-left origin** coordinate system, while the framework uses **top-left origin**.
//! All drawing operations in this module automatically convert between coordinate systems.
//!
//! - **Input coordinates**: Screen coordinates (top-left origin, Y increases downward)
//! - **Internal PDF coordinates**: PDF coordinates (bottom-left origin, Y increases upward)
//!
//! The conversion is handled automatically by the implementation, so you can use screen
//! coordinates when calling drawing methods.

pub mod annotation;
pub mod form;
pub mod hyperlink;
pub mod security;

pub use annotation::*;
pub use form::*;
pub use hyperlink::*;
pub use security::*;

use crate::core::coords::to_pdf_y;
use crate::core::{Color, Rect, Size};
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// PDF page
pub trait PdfPage {
    /// Get page size
    fn size(&self) -> Size;

    /// Set page size
    fn set_size(&mut self, size: Size);

    /// Draw text
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);

    /// Draw line
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);

    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color);

    /// Draw filled rectangle
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Draw image
    fn draw_image(&mut self, image: &[u8], rect: Rect);

    /// Add text field
    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str);

    /// Add checkbox
    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool);

    /// Add button
    fn add_button(&mut self, name: &str, rect: Rect, text: &str);

    /// Get page content as bytes
    fn content(&self) -> Vec<u8>;

    /// Get a snapshot of all form fields attached to the page.
    fn form_fields(&self) -> Vec<PdfFormField>;
}

/// PDF document
pub trait PdfDocument {
    /// Get number of pages
    fn page_count(&self) -> u32;

    /// Get page by index
    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage>;

    /// Add a new page
    fn add_page(&mut self, size: Size) -> u32;

    /// Insert a page at specified position
    fn insert_page(&mut self, index: u32, size: Size) -> u32;

    /// Remove a page
    fn remove_page(&mut self, index: u32) -> bool;

    /// Reorder pages
    fn reorder_pages(&mut self, new_order: &[u32]) -> bool;

    /// Get document metadata
    fn metadata(&self) -> &PdfMetadata;

    /// Set document metadata
    fn set_metadata(&mut self, metadata: PdfMetadata);

    /// Get document security settings
    fn security(&self) -> &PdfSecurity;

    /// Set document security settings
    fn set_security(&mut self, security: PdfSecurity);

    /// Enable or disable automatic page-number footer stamping.
    fn set_page_numbering_enabled(&mut self, enabled: bool);

    /// Configure page-number footer format.
    ///
    /// Example output with default prefix: `Page 1/3`.
    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32);

    /// Configure page-number footer layout.
    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32);

    /// Save to file
    fn save(&self, path: &str) -> Result<(), std::io::Error>;

    /// Save to bytes
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}

/// PDF metadata
#[derive(Debug, Clone)]
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

/// PDF security settings
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfSecurity {
    /// Optional password required to open the document.
    pub user_password: Option<String>,
    /// Optional owner password for privilege changes.
    pub owner_password: Option<String>,
    /// Whether printing is allowed.
    pub print_permission: bool,
    /// Whether content editing is allowed.
    pub edit_permission: bool,
    /// Whether content copy is allowed.
    pub copy_permission: bool,
    /// Whether annotations are allowed.
    pub annotation_permission: bool,
}

impl Default for PdfSecurity {
    fn default() -> Self {
        Self {
            user_password: None,
            owner_password: None,
            print_permission: true,
            edit_permission: true,
            copy_permission: true,
            annotation_permission: true,
        }
    }
}

/// PDF writer
pub struct PdfWriter {
    /// Backend profile name used by writer diagnostics.
    backend_name: &'static str,
}

impl PdfWriter {
    /// Create a new PDF writer
    pub fn new() -> Self {
        Self {
            backend_name: "pdf-minimal-v1",
        }
    }

    /// Create a new document
    pub fn create_document(&self, page_size: Size) -> Box<dyn PdfDocument> {
        Box::new(PdfDocumentImpl::new(page_size))
    }

    /// Create a new document with an embedded TrueType font loaded from path.
    pub fn create_document_with_font_path(
        &self,
        page_size: Size,
        base_font: &str,
        font_path: &str,
    ) -> Result<Box<dyn PdfDocument>, std::io::Error> {
        let doc = PdfDocumentImpl::new_with_embedded_font(page_size, base_font, font_path)?;
        Ok(Box::new(doc))
    }

    /// Return active writer backend name.
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }
}

impl Default for PdfWriter {
    fn default() -> Self {
        Self::new()
    }
}

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
                doc.pages.push(Box::new(PdfPageImpl {
                    size: page.size,
                    content: page.content,
                    font_resource: doc.default_font_resource().to_string(),
                    form_fields: HashMap::new(),
                }));
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

/// PDF document implementation
struct PdfDocumentImpl {
    /// Ordered page list.
    pages: Vec<Box<dyn PdfPage>>,
    /// Document metadata block.
    metadata: PdfMetadata,
    /// Document security policy.
    security: PdfSecurity,
    /// Declared font resources for page content streams.
    fonts: Vec<PdfFontResource>,
    /// Optional page-number footer stamping options.
    pagination: PdfPagination,
}

impl PdfDocumentImpl {
    fn new(page_size: Size) -> Self {
        let mut document = Self {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            security: PdfSecurity::default(),
            fonts: vec![PdfFontResource::core_helvetica("F1")],
            pagination: PdfPagination::default(),
        };
        document.add_page(page_size);
        document
    }

    fn new_with_embedded_font(
        page_size: Size,
        base_font: &str,
        font_path: &str,
    ) -> Result<Self, std::io::Error> {
        let font_data = fs::read(font_path)?;
        if font_data.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "font file is empty"));
        }

        let mut document = Self {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            security: PdfSecurity::default(),
            fonts: vec![PdfFontResource {
                resource_name: "F1".to_string(),
                base_font: sanitize_pdf_font_name(base_font),
                source_path: Some(font_path.to_string()),
                embedded_data: font_data,
            }],
            pagination: PdfPagination::default(),
        };
        document.add_page(page_size);
        Ok(document)
    }

    fn default_font_resource(&self) -> &str {
        self.fonts
            .first()
            .map(|font| font.resource_name.as_str())
            .unwrap_or("F1")
    }
}

impl PdfDocument for PdfDocumentImpl {
    fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    fn get_page(&mut self, index: u32) -> Option<&mut dyn PdfPage> {
        if index < self.pages.len() as u32 {
            Some(&mut *self.pages[index as usize])
        } else {
            None
        }
    }

    fn add_page(&mut self, size: Size) -> u32 {
        let page = Box::new(PdfPageImpl::new(size, self.default_font_resource()));
        self.pages.push(page);
        (self.pages.len() - 1) as u32
    }

    fn insert_page(&mut self, index: u32, size: Size) -> u32 {
        if index <= self.pages.len() as u32 {
            let page = Box::new(PdfPageImpl::new(size, self.default_font_resource()));
            self.pages.insert(index as usize, page);
            index
        } else {
            self.add_page(size)
        }
    }

    fn remove_page(&mut self, index: u32) -> bool {
        if index < self.pages.len() as u32 && self.pages.len() > 1 {
            self.pages.remove(index as usize);
            true
        } else {
            false
        }
    }

    fn reorder_pages(&mut self, new_order: &[u32]) -> bool {
        if new_order.len() != self.pages.len() {
            return false;
        }
        let mut reordered: Vec<Box<dyn PdfPage>> = Vec::with_capacity(self.pages.len());
        let mut slots: Vec<Option<Box<dyn PdfPage>>> = self.pages.drain(..).map(Some).collect();
        for index in new_order {
            let Some(slot) = slots.get_mut(*index as usize) else {
                return false;
            };
            let Some(page) = slot.take() else {
                return false;
            };
            reordered.push(page);
        }
        self.pages = reordered;
        true
    }

    fn metadata(&self) -> &PdfMetadata {
        &self.metadata
    }

    fn set_metadata(&mut self, metadata: PdfMetadata) {
        self.metadata = metadata;
    }

    fn security(&self) -> &PdfSecurity {
        &self.security
    }

    fn set_security(&mut self, security: PdfSecurity) {
        self.security = security;
    }

    fn set_page_numbering_enabled(&mut self, enabled: bool) {
        self.pagination.enabled = enabled;
    }

    fn set_page_numbering_format(&mut self, prefix: &str, start_at: u32) {
        self.pagination.prefix = if prefix.trim().is_empty() {
            "Page".to_string()
        } else {
            prefix.to_string()
        };
        self.pagination.start_at = start_at.max(1);
    }

    fn set_page_numbering_layout(&mut self, right_margin: f32, bottom_margin: f32, font_size: f32) {
        self.pagination.right_margin = right_margin.max(0.0);
        self.pagination.bottom_margin = bottom_margin.max(0.0);
        self.pagination.font_size = font_size.max(6.0);
    }

    fn save(&self, path: &str) -> Result<(), std::io::Error> {
        fs::write(path, self.to_bytes()?)?;
        Ok(())
    }

    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        build_minimal_pdf_bytes(self)
    }
}

/// PDF page implementation
struct PdfPageImpl {
    /// Page size in points.
    size: Size,
    /// Encoded draw command payload (placeholder implementation).
    content: Vec<u8>,
    /// Font resource key used for text operators.
    font_resource: String,
    /// Form field definitions keyed by field name.
    form_fields: HashMap<String, PdfFormField>,
}

impl PdfPageImpl {
    fn new(size: Size, font_resource: &str) -> Self {
        Self {
            size,
            content: Vec::new(),
            font_resource: font_resource.to_string(),
            form_fields: HashMap::new(),
        }
    }
}

impl PdfPage for PdfPageImpl {
    fn size(&self) -> Size {
        self.size
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
        let escaped = pdf_escape_literal(text);
        let pdf_y = to_pdf_y(y, self.size.height as f32);
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} rg\nBT /{} {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                self.font_resource,
                font_size,
                x,
                pdf_y,
                escaped
            )
            .as_bytes(),
        );
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
        let pdf_y1 = to_pdf_y(y1, self.size.height as f32);
        let pdf_y2 = to_pdf_y(y2, self.size.height as f32);
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} RG\n{:.2} w {:.2} {:.2} m {:.2} {:.2} l S\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                width,
                x1,
                pdf_y1,
                x2,
                pdf_y2
            )
            .as_bytes(),
        );
    }

    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color) {
        let pdf_y = to_pdf_y(rect.y as f32 + rect.height as f32, self.size.height as f32);
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} RG\n{:.2} w {} {} {} {} re S\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                width,
                rect.x,
                pdf_y,
                rect.width,
                rect.height
            )
            .as_bytes(),
        );
    }

    fn fill_rect(&mut self, rect: Rect, color: Color) {
        let pdf_y = to_pdf_y(rect.y as f32 + rect.height as f32, self.size.height as f32);
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} rg\n{} {} {} {} re f\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                rect.x,
                pdf_y,
                rect.width,
                rect.height
            )
            .as_bytes(),
        );
    }

    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        if image.is_empty() || rect.width == 0 || rect.height == 0 {
            return;
        }

        let width = rect.width.max(1) as usize;
        let height = rect.height.max(1) as usize;
        let (rgb, route) = normalize_image_payload_to_rgb(image, width, height);
        let hex = hex_encode(&rgb);
        let expected_rgb_len = width.saturating_mul(height).saturating_mul(3);

        let pdf_y = to_pdf_y(rect.y as f32 + rect.height as f32, self.size.height as f32);
        self.content.extend_from_slice(
            format!(
                "q\n{} 0 0 {} {} {} cm\n% rw-image-route:{}\n% rw-image-source-len:{}\n% rw-image-expected-rgb-len:{}\nBI\n/W {}\n/H {}\n/CS /RGB\n/BPC 8\n/F [/ASCIIHexDecode]\nID\n{}>\nEI\nQ\n",
                rect.width,
                rect.height,
                rect.x,
                pdf_y,
                route.as_str(),
                image.len(),
                expected_rgb_len,
                rect.width,
                rect.height,
                hex
            )
            .as_bytes(),
        );
    }

    fn add_text_field(&mut self, name: &str, rect: Rect, default_text: &str) {
        let field = PdfFormField::TextField {
            name: name.to_string(),
            rect,
            value: default_text.to_string(),
        };
        self.form_fields.insert(name.to_string(), field);
    }

    fn add_checkbox(&mut self, name: &str, rect: Rect, checked: bool) {
        let field = PdfFormField::CheckBox {
            name: name.to_string(),
            rect,
            checked,
        };
        self.form_fields.insert(name.to_string(), field);
    }

    fn add_button(&mut self, name: &str, rect: Rect, text: &str) {
        let field = PdfFormField::Button {
            name: name.to_string(),
            rect,
            text: text.to_string(),
        };
        self.form_fields.insert(name.to_string(), field);
    }

    fn content(&self) -> Vec<u8> {
        self.content.clone()
    }

    fn form_fields(&self) -> Vec<PdfFormField> {
        let mut fields = self.form_fields.values().cloned().collect::<Vec<_>>();
        fields.sort_by(|left, right| pdf_form_field_name(left).cmp(pdf_form_field_name(right)));
        fields
    }
}

/// PDF form field types
#[derive(Debug, Clone)]
pub enum PdfFormField {
    /// Text field with default value.
    TextField {
        name: String,
        rect: Rect,
        value: String,
    },
    /// Checkbox field.
    CheckBox {
        name: String,
        rect: Rect,
        checked: bool,
    },
    /// Button field.
    Button {
        name: String,
        rect: Rect,
        text: String,
    },
    ComboBox {
        name: String,
        rect: Rect,
        value: String,
        options: Vec<String>,
    },
    ListBox {
        name: String,
        rect: Rect,
        selected: Vec<usize>,
        options: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct PdfPagination {
    /// Enable page-number footer output.
    enabled: bool,
    /// Prefix text before current/total numbers.
    prefix: String,
    /// One-based starting page number.
    start_at: u32,
    /// Distance from page right edge in points.
    right_margin: f32,
    /// Distance from page bottom edge in points.
    bottom_margin: f32,
    /// Font size for footer page-number text.
    font_size: f32,
}

impl Default for PdfPagination {
    fn default() -> Self {
        Self {
            enabled: false,
            prefix: "Page".to_string(),
            start_at: 1,
            right_margin: 140.0,
            bottom_margin: 20.0,
            font_size: 10.0,
        }
    }
}

fn pdf_escape_literal(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[derive(Debug, Clone)]
struct PdfFontResource {
    /// Resource key referenced by page content streams (e.g. F1).
    resource_name: String,
    /// PostScript-like base font name.
    base_font: String,
    /// Optional source path for embedded font diagnostics.
    source_path: Option<String>,
    /// Optional embedded font bytes.
    embedded_data: Vec<u8>,
}

impl PdfFontResource {
    fn core_helvetica(resource_name: &str) -> Self {
        Self {
            resource_name: resource_name.to_string(),
            base_font: "Helvetica".to_string(),
            source_path: None,
            embedded_data: Vec::new(),
        }
    }
}

fn sanitize_pdf_font_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "EmbeddedFont".to_string();
    }
    trimmed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn build_minimal_pdf_bytes(doc: &PdfDocumentImpl) -> Result<Vec<u8>, std::io::Error> {
    if doc.pages.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "document must contain at least one page",
        ));
    }

    let mut objects: Vec<String> = Vec::new();

    // 1: Catalog (filled after objects known)
    objects.push(String::new());
    // 2: Pages tree (filled after page objects known)
    objects.push(String::new());
    let mut font_object_ids: HashMap<String, u32> = HashMap::new();

    for font in &doc.fonts {
        let font_obj_id = if font.embedded_data.is_empty() {
            let id = (objects.len() + 1) as u32;
            objects.push(format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{} >>",
                font.base_font
            ));
            id
        } else {
            let file_obj_id = (objects.len() + 1) as u32;
            let mut stream_prefix = String::new();
            if let Some(path) = &font.source_path {
                let normalized = Path::new(path).to_string_lossy();
                stream_prefix.push_str(&format!("% font-source:{}\\n", normalized));
            }
            let mut stream_bytes = stream_prefix.into_bytes();
            stream_bytes.extend_from_slice(&font.embedded_data);
            let stream_text = String::from_utf8_lossy(&stream_bytes);
            objects.push(format!(
                "<< /Length {} >>\\nstream\\n{}\\nendstream",
                stream_bytes.len(),
                stream_text
            ));

            let descriptor_obj_id = (objects.len() + 1) as u32;
            let mut descriptor = format!(
                "<< /Type /FontDescriptor /FontName /{} /Flags 32 /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontBBox [0 -200 1000 900] /FontFile2 {} 0 R",
                font.base_font,
                file_obj_id
            );
            if let Some(path) = &font.source_path {
                descriptor.push_str(&format!(" /RWFontPath ({})", pdf_escape_literal(path)));
            }
            descriptor.push_str(" >>");
            objects.push(descriptor);

            let id = (objects.len() + 1) as u32;
            objects.push(format!(
                "<< /Type /Font /Subtype /TrueType /BaseFont /{} /Encoding /WinAnsiEncoding /FontDescriptor {} 0 R >>",
                font.base_font,
                descriptor_obj_id
            ));
            id
        };

        font_object_ids.insert(font.resource_name.clone(), font_obj_id);
    }

    let mut page_object_ids = Vec::new();
    let mut all_form_field_object_ids = Vec::new();

    let font_resources = doc
        .fonts
        .iter()
        .filter_map(|font| {
            font_object_ids
                .get(&font.resource_name)
                .map(|id| format!("/{} {} 0 R", font.resource_name, id))
        })
        .collect::<Vec<_>>()
        .join(" ");

    for (index, page) in doc.pages.iter().enumerate() {
        let content_obj_id = (objects.len() + 1) as u32;
        let mut stream = page.content();
        if doc.pagination.enabled {
            append_page_number_footer(
                &mut stream,
                page.size(),
                index,
                doc.pages.len(),
                &doc.pagination,
            );
        }
        let stream_text = String::from_utf8_lossy(&stream);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream_text
        ));

        let page_form_fields = page.form_fields();
        let mut page_form_field_ids = Vec::new();
        for field in page_form_fields {
            let field_obj_id = (objects.len() + 1) as u32;
            objects.push(serialize_pdf_form_field_widget(&field));
            page_form_field_ids.push(field_obj_id);
            all_form_field_object_ids.push(field_obj_id);
        }

        let annots_entry = if page_form_field_ids.is_empty() {
            String::new()
        } else {
            let refs = page_form_field_ids
                .iter()
                .map(|id| format!("{id} 0 R"))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" /Annots [{}]", refs)
        };

        let page_obj_id = (objects.len() + 1) as u32;
        let size = page.size();
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {} >> >> /Contents {} 0 R{} >>",
            size.width,
            size.height,
            font_resources,
            content_obj_id,
            annots_entry
        ));
        page_object_ids.push(page_obj_id);
    }

    let info_obj_id = (objects.len() + 1) as u32;
    let security_entries = serialize_security_diagnostics_entries(&doc.security);
    objects.push(format!(
        "<< /Title ({}) /Author ({}) /Subject ({}) /Creator ({}) /Producer ({}){} >>",
        pdf_escape_literal(&doc.metadata.title),
        pdf_escape_literal(&doc.metadata.author),
        pdf_escape_literal(&doc.metadata.subject),
        pdf_escape_literal(&doc.metadata.creator),
        pdf_escape_literal(&doc.metadata.producer),
        security_entries,
    ));

    let kids = page_object_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");

    let acroform_obj_id = if all_form_field_object_ids.is_empty() {
        None
    } else {
        let id = (objects.len() + 1) as u32;
        let refs = all_form_field_object_ids
            .iter()
            .map(|field_id| format!("{field_id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        objects.push(format!("<< /Fields [{}] /NeedAppearances true >>", refs));
        Some(id)
    };

    objects[0] = if let Some(acroform_id) = acroform_obj_id {
        format!(
            "<< /Type /Catalog /Pages 2 0 R /AcroForm {} 0 R >>",
            acroform_id
        )
    } else {
        "<< /Type /Catalog /Pages 2 0 R >>".to_string()
    };
    objects[1] = format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        page_object_ids.len(),
        kids
    );

    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets: Vec<usize> = Vec::new();
    for (idx, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let obj_id = idx + 1;
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", obj_id, body).as_bytes());
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }

    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            info_obj_id,
            xref_offset
        )
        .as_bytes(),
    );

    Ok(out)
}

fn append_page_number_footer(
    stream: &mut Vec<u8>,
    page_size: Size,
    page_index: usize,
    page_count: usize,
    pagination: &PdfPagination,
) {
    let current = pagination.start_at.saturating_add(page_index as u32);
    let total = pagination
        .start_at
        .saturating_add(page_count.saturating_sub(1) as u32);
    let label = format!("{} {}/{}", pagination.prefix, current, total);
    let escaped = pdf_escape_literal(&label);
    let x = (page_size.width as f32 - pagination.right_margin).max(12.0);
    let y = pagination.bottom_margin.max(8.0);
    let footer = format!(
        "0 0 0 rg\nBT /F1 {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n",
        pagination.font_size, x, y, escaped
    );
    stream.extend_from_slice(footer.as_bytes());
}

fn serialize_pdf_form_field_widget(field: &PdfFormField) -> String {
    match field {
        PdfFormField::TextField { name, rect, value } => format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T ({}) /Rect [{}] /V ({}) >>",
            pdf_escape_literal(name),
            pdf_rect(rect),
            pdf_escape_literal(value),
        ),
        PdfFormField::CheckBox {
            name,
            rect,
            checked,
        } => {
            let state = if *checked { "Yes" } else { "Off" };
            format!(
                "<< /Type /Annot /Subtype /Widget /FT /Btn /T ({}) /Rect [{}] /V /{} /AS /{} >>",
                pdf_escape_literal(name),
                pdf_rect(rect),
                state,
                state,
            )
        }
        PdfFormField::Button { name, rect, text } => format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T ({}) /Rect [{}] /MK << /CA ({}) >> >>",
            pdf_escape_literal(name),
            pdf_rect(rect),
            pdf_escape_literal(text),
        ),
        PdfFormField::ComboBox {
            name,
            rect,
            value,
            options,
        } => {
            let options_text = options
                .iter()
                .map(|option| format!("({})", pdf_escape_literal(option)))
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T ({}) /Rect [{}] /V ({}) /Opt [{}] >>",
                pdf_escape_literal(name),
                pdf_rect(rect),
                pdf_escape_literal(value),
                options_text,
            )
        }
        PdfFormField::ListBox {
            name,
            rect,
            selected,
            options,
        } => {
            let options_text = options
                .iter()
                .map(|option| format!("({})", pdf_escape_literal(option)))
                .collect::<Vec<_>>()
                .join(" ");
            let selected_text = selected
                .iter()
                .map(|index| index.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "<< /Type /Annot /Subtype /Widget /FT /Ch /T ({}) /Rect [{}] /Opt [{}] /I [{}] >>",
                pdf_escape_literal(name),
                pdf_rect(rect),
                options_text,
                selected_text,
            )
        }
    }
}

fn pdf_rect(rect: &Rect) -> String {
    let x1 = rect.x;
    let y1 = rect.y;
    let x2 = rect.x + rect.width as i32;
    let y2 = rect.y + rect.height as i32;
    format!("{} {} {} {}", x1, y1, x2, y2)
}

fn pdf_form_field_name(field: &PdfFormField) -> &str {
    match field {
        PdfFormField::TextField { name, .. }
        | PdfFormField::CheckBox { name, .. }
        | PdfFormField::Button { name, .. }
        | PdfFormField::ComboBox { name, .. }
        | PdfFormField::ListBox { name, .. } => name,
    }
}

fn serialize_security_diagnostics_entries(security: &PdfSecurity) -> String {
    if *security == PdfSecurity::default() {
        return String::new();
    }

    let user_password = security.user_password.as_deref().unwrap_or("");
    let owner_password = security.owner_password.as_deref().unwrap_or("");
    format!(
        " /RWSecurityUnsupported true /RWUserPassword ({}) /RWOwnerPassword ({}) /RWPermPrint {} /RWPermEdit {} /RWPermCopy {} /RWPermAnnot {}",
        pdf_escape_literal(user_password),
        pdf_escape_literal(owner_password),
        security.print_permission,
        security.edit_permission,
        security.copy_permission,
        security.annotation_permission,
    )
}

fn parse_security_diagnostics(text: &str) -> Option<PdfSecurity> {
    if !text.contains("/RWSecurityUnsupported true") {
        return None;
    }

    let user_password = parse_pdf_literal_by_key(text, "/RWUserPassword").filter(|v| !v.is_empty());
    let owner_password =
        parse_pdf_literal_by_key(text, "/RWOwnerPassword").filter(|v| !v.is_empty());

    Some(PdfSecurity {
        user_password,
        owner_password,
        print_permission: parse_pdf_bool_by_key(text, "/RWPermPrint").unwrap_or(true),
        edit_permission: parse_pdf_bool_by_key(text, "/RWPermEdit").unwrap_or(true),
        copy_permission: parse_pdf_bool_by_key(text, "/RWPermCopy").unwrap_or(true),
        annotation_permission: parse_pdf_bool_by_key(text, "/RWPermAnnot").unwrap_or(true),
    })
}

fn parse_pdf_bool_by_key(text: &str, key: &str) -> Option<bool> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?.trim_start();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_pdf_literal_by_key(text: &str, key: &str) -> Option<String> {
    let start = text.find(key)? + key.len();
    let rest = text.get(start..)?.trim_start();
    let literal_start = rest.find('(')? + 1;
    let literal_tail = rest.get(literal_start..)?;
    let literal_end = literal_tail.find(')')?;
    Some(literal_tail[..literal_end].to_string())
}

struct ParsedPdfPage {
    size: Size,
    content: Vec<u8>,
}

fn parse_pdf_pages(text: &str) -> Vec<ParsedPdfPage> {
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

fn parse_pdf_objects(text: &str) -> HashMap<u32, String> {
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

fn parse_page_media_box(page_obj: &str) -> Option<Size> {
    let marker = "/MediaBox [0 0 ";
    let start = page_obj.find(marker)? + marker.len();
    let rest = &page_obj[start..];
    let mut parts = rest.split_whitespace();
    let width = parts.next()?.trim().parse::<u32>().ok()?;
    let height_raw = parts.next()?;
    let height = height_raw.trim_end_matches(']').parse::<u32>().ok()?;
    Some(Size { width, height })
}

fn parse_contents_object_id(page_obj: &str) -> Option<u32> {
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

fn extract_stream(content_obj: &str) -> Option<&str> {
    let stream_start = content_obj.find("stream\n")? + "stream\n".len();
    let rest = &content_obj[stream_start..];
    let stream_end = rest.find("\nendstream")?;
    Some(&rest[..stream_end])
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02X}", byte));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageEncodingRoute {
    ExactRgb,
    ExactRgbaDropAlpha,
    ExactGrayExpand,
    TruncatedOrPadded,
}

impl ImageEncodingRoute {
    fn as_str(self) -> &'static str {
        match self {
            ImageEncodingRoute::ExactRgb => "exact-rgb",
            ImageEncodingRoute::ExactRgbaDropAlpha => "exact-rgba-drop-alpha",
            ImageEncodingRoute::ExactGrayExpand => "exact-gray-expand",
            ImageEncodingRoute::TruncatedOrPadded => "raw-truncate-pad",
        }
    }
}

fn normalize_image_payload_to_rgb(
    image: &[u8],
    width: usize,
    height: usize,
) -> (Vec<u8>, ImageEncodingRoute) {
    let pixel_count = width.saturating_mul(height);
    let expected_rgb_len = pixel_count.saturating_mul(3);
    let expected_rgba_len = pixel_count.saturating_mul(4);
    let expected_gray_len = pixel_count;

    if image.len() == expected_rgb_len {
        return (image.to_vec(), ImageEncodingRoute::ExactRgb);
    }

    if image.len() == expected_rgba_len {
        let mut rgb = Vec::with_capacity(expected_rgb_len);
        for chunk in image.chunks_exact(4) {
            rgb.extend_from_slice(&chunk[..3]);
        }
        return (rgb, ImageEncodingRoute::ExactRgbaDropAlpha);
    }

    if image.len() == expected_gray_len {
        let mut rgb = Vec::with_capacity(expected_rgb_len);
        for gray in image {
            rgb.push(*gray);
            rgb.push(*gray);
            rgb.push(*gray);
        }
        return (rgb, ImageEncodingRoute::ExactGrayExpand);
    }

    let mut rgb = vec![0u8; expected_rgb_len];
    let copy_len = expected_rgb_len.min(image.len());
    rgb[..copy_len].copy_from_slice(&image[..copy_len]);
    (rgb, ImageEncodingRoute::TruncatedOrPadded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path_with_suffix(suffix: &str) -> String {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("rust_widgets_pdf_test_{}_{}", ts, suffix));
        path.to_string_lossy().to_string()
    }

    #[test]
    fn writer_embeds_font_stream_when_font_path_is_provided() {
        let font_path = temp_path_with_suffix("font.ttf");
        fs::write(&font_path, b"RW_TEST_FONT_BYTES").expect("write test font file");

        let writer = PdfWriter::new();
        let mut doc = writer
            .create_document_with_font_path(
                Size {
                    width: 595,
                    height: 842,
                },
                "Test Font",
                &font_path,
            )
            .expect("create document with font path");

        let page = doc.get_page(0).expect("page 0 must exist");
        page.draw_text(
            "hello",
            20.0,
            20.0,
            12.0,
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        );

        let pdf = doc.to_bytes().expect("serialize pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/FontFile2"));
        assert!(text.contains("/RWFontPath"));
        assert!(text.contains("/BaseFont /Test-Font"));
        assert!(text.contains("BT /F1"));

        let _ = fs::remove_file(font_path);
    }

    #[test]
    fn writer_fails_for_empty_font_file() {
        let font_path = temp_path_with_suffix("empty.ttf");
        fs::write(&font_path, []).expect("write empty test font file");

        let writer = PdfWriter::new();
        let result = writer.create_document_with_font_path(
            Size {
                width: 595,
                height: 842,
            },
            "EmptyFont",
            &font_path,
        );
        assert!(result.is_err());

        let _ = fs::remove_file(font_path);
    }

    #[test]
    fn writer_stamps_page_number_footer_when_enabled() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 595,
            height: 842,
        });
        doc.add_page(Size {
            width: 595,
            height: 842,
        });
        doc.set_page_numbering_enabled(true);
        doc.set_page_numbering_format("Page", 1);

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("(Page 1/2)"));
        assert!(text.contains("(Page 2/2)"));
    }

    #[test]
    fn writer_applies_custom_page_number_layout() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 600,
            height: 840,
        });
        doc.set_page_numbering_enabled(true);
        doc.set_page_numbering_layout(100.0, 36.0, 12.0);

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("BT /F1 12.00 Tf 500.00 36.00 Td (Page 1/1)"));
    }

    #[test]
    fn reader_roundtrip_preserves_page_stream_and_media_box() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 612,
            height: 792,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.draw_text(
                "hello",
                20.0,
                24.0,
                12.0,
                Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            );
            page.draw_line(
                10.0,
                10.0,
                60.0,
                10.0,
                1.5,
                Color {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 255,
                },
            );
            page.draw_rect(
                Rect {
                    x: 12,
                    y: 14,
                    width: 20,
                    height: 10,
                },
                1.0,
                Color {
                    r: 40,
                    g: 50,
                    b: 60,
                    a: 255,
                },
            );
            page.fill_rect(
                Rect {
                    x: 40,
                    y: 20,
                    width: 15,
                    height: 8,
                },
                Color {
                    r: 70,
                    g: 80,
                    b: 90,
                    a: 255,
                },
            );
            page.draw_image(
                &[0xAB, 0xCD, 0xEF],
                Rect {
                    x: 5,
                    y: 5,
                    width: 2,
                    height: 2,
                },
            );
        }

        let bytes = doc.to_bytes().expect("serialize");
        let reader = PdfReader::new();
        let mut loaded = reader.load_from_bytes(&bytes).expect("load bytes");

        let page = loaded.get_page(0).expect("loaded page exists");
        assert_eq!(page.size().width, 612);
        assert_eq!(page.size().height, 792);

        let content_bytes = page.content();
        let content = String::from_utf8_lossy(&content_bytes);
        assert!(content.contains("BT /F1"));
        assert!(content.contains(" m "));
        assert!(content.contains(" re S"));
        assert!(content.contains(" re f"));
        assert!(content.contains("BI"));
        assert!(content.contains("EI"));
    }

    #[test]
    fn writer_serializes_acroform_and_widget_annotations() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 595,
            height: 842,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.add_text_field(
                "full_name",
                Rect {
                    x: 40,
                    y: 700,
                    width: 200,
                    height: 24,
                },
                "Alice",
            );
            page.add_checkbox(
                "agree",
                Rect {
                    x: 40,
                    y: 660,
                    width: 16,
                    height: 16,
                },
                true,
            );
            page.add_button(
                "submit",
                Rect {
                    x: 40,
                    y: 620,
                    width: 80,
                    height: 24,
                },
                "Submit",
            );
        }

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("/AcroForm"));
        assert!(text.contains("/Annots ["));
        assert!(text.contains("/Subtype /Widget"));
        assert!(text.contains("/FT /Tx"));
        assert!(text.contains("/FT /Btn"));
        assert!(text.contains("/NeedAppearances true"));
        assert!(text.contains("/T (full_name)"));
        assert!(text.contains("/T (agree)"));
        assert!(text.contains("/T (submit)"));
    }

    #[test]
    fn writer_serializes_security_diagnostics_when_security_is_set() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 595,
            height: 842,
        });
        doc.set_security(PdfSecurity {
            user_password: Some("user-secret".to_string()),
            owner_password: Some("owner-secret".to_string()),
            print_permission: false,
            edit_permission: true,
            copy_permission: false,
            annotation_permission: false,
        });

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/RWSecurityUnsupported true"));
        assert!(text.contains("/RWUserPassword (user-secret)"));
        assert!(text.contains("/RWOwnerPassword (owner-secret)"));
        assert!(text.contains("/RWPermPrint false"));
        assert!(text.contains("/RWPermEdit true"));
        assert!(text.contains("/RWPermCopy false"));
        assert!(text.contains("/RWPermAnnot false"));
    }

    #[test]
    fn reader_roundtrip_restores_security_diagnostics() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 595,
            height: 842,
        });
        doc.set_security(PdfSecurity {
            user_password: Some("u".to_string()),
            owner_password: Some("o".to_string()),
            print_permission: false,
            edit_permission: false,
            copy_permission: true,
            annotation_permission: false,
        });

        let bytes = doc.to_bytes().expect("serialize document");
        let reader = PdfReader::new();
        let loaded = reader.load_from_bytes(&bytes).expect("load bytes");
        let security = loaded.security();

        assert_eq!(security.user_password.as_deref(), Some("u"));
        assert_eq!(security.owner_password.as_deref(), Some("o"));
        assert!(!security.print_permission);
        assert!(!security.edit_permission);
        assert!(security.copy_permission);
        assert!(!security.annotation_permission);
    }

    #[test]
    fn writer_combined_pipeline_emits_form_security_and_image_markers() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 595,
            height: 842,
        });
        doc.set_security(PdfSecurity {
            user_password: Some("combo-user".to_string()),
            owner_password: Some("combo-owner".to_string()),
            print_permission: false,
            edit_permission: true,
            copy_permission: false,
            annotation_permission: true,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.add_text_field(
                "email",
                Rect {
                    x: 32,
                    y: 720,
                    width: 240,
                    height: 22,
                },
                "alice@example.com",
            );
            page.add_checkbox(
                "newsletter",
                Rect {
                    x: 32,
                    y: 688,
                    width: 14,
                    height: 14,
                },
                false,
            );
            page.draw_image(
                &[0x7F],
                Rect {
                    x: 16,
                    y: 16,
                    width: 2,
                    height: 1,
                },
            );
        }

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("/AcroForm"));
        assert!(text.contains("/Annots ["));
        assert!(text.contains("/Subtype /Widget"));
        assert!(text.contains("/T (email)"));
        assert!(text.contains("/T (newsletter)"));
        assert!(text.contains("/RWSecurityUnsupported true"));
        assert!(text.contains("/RWUserPassword (combo-user)"));
        assert!(text.contains("/RWOwnerPassword (combo-owner)"));
        assert!(text.contains("% rw-image-route:raw-truncate-pad"));
    }

    #[test]
    fn reader_roundtrip_preserves_security_and_image_route_markers() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 300,
            height: 200,
        });
        doc.set_security(PdfSecurity {
            user_password: Some("round-u".to_string()),
            owner_password: Some("round-o".to_string()),
            print_permission: true,
            edit_permission: false,
            copy_permission: false,
            annotation_permission: false,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.draw_image(
                &[0x11, 0x22, 0x33],
                Rect {
                    x: 2,
                    y: 2,
                    width: 2,
                    height: 2,
                },
            );
            page.draw_text(
                "ok",
                10.0,
                10.0,
                10.0,
                Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            );
        }

        let bytes = doc.to_bytes().expect("serialize document");
        let reader = PdfReader::new();
        let mut loaded = reader.load_from_bytes(&bytes).expect("load bytes");

        let security = loaded.security();
        assert_eq!(security.user_password.as_deref(), Some("round-u"));
        assert_eq!(security.owner_password.as_deref(), Some("round-o"));
        assert!(security.print_permission);
        assert!(!security.edit_permission);
        assert!(!security.copy_permission);
        assert!(!security.annotation_permission);

        let page = loaded.get_page(0).expect("loaded page exists");
        let content_bytes = page.content();
        let content = String::from_utf8_lossy(&content_bytes);
        assert!(content.contains("% rw-image-route:raw-truncate-pad"));
        assert!(content.contains("% rw-image-source-len:3"));
        assert!(content.contains("% rw-image-expected-rgb-len:12"));
        assert!(content.contains("BT /F1"));
    }

    #[test]
    fn writer_image_with_short_payload_uses_truncate_pad_not_tiling() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 100,
            height: 100,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.draw_image(
                &[0x01, 0x02, 0x03],
                Rect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                },
            );
        }

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("% rw-image-route:raw-truncate-pad"));
        assert!(text.contains("% rw-image-source-len:3"));
        assert!(text.contains("% rw-image-expected-rgb-len:12"));
        assert!(text.contains("010203000000000000000000>"));
        assert!(!text.contains("010203010203010203010203>"));
    }

    #[test]
    fn writer_image_with_rgba_payload_drops_alpha_deterministically() {
        let writer = PdfWriter::new();
        let mut doc = writer.create_document(Size {
            width: 100,
            height: 100,
        });

        {
            let page = doc.get_page(0).expect("page exists");
            page.draw_image(
                &[0x0A, 0x14, 0x1E, 0xFF],
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            );
        }

        let bytes = doc.to_bytes().expect("serialize document");
        let text = String::from_utf8_lossy(&bytes);

        assert!(text.contains("% rw-image-route:exact-rgba-drop-alpha"));
        assert!(text.contains("0A141E>"));
    }
}
