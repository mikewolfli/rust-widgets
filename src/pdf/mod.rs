//! PDF document abstraction and in-memory implementation.

use crate::core::{Rect, Size, Color};
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};

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
    
    /// Save to file
    fn save(&self, path: &str) -> Result<(), std::io::Error>;
    
    /// Save to bytes
    fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error>;
}

/// PDF metadata
#[derive(Debug, Clone)]
pub struct PdfMetadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub keywords: Vec<String>,
    pub creator: String,
    pub producer: String,
    pub creation_date: Option<String>,
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
#[derive(Debug, Clone)]
pub struct PdfSecurity {
    pub user_password: Option<String>,
    pub owner_password: Option<String>,
    pub print_permission: bool,
    pub edit_permission: bool,
    pub copy_permission: bool,
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

    /// Return active writer backend name.
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
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
            let digits: String = token
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            if let Ok(parsed) = digits.parse::<usize>() {
                page_count = page_count.max(parsed.max(1));
            }
        }

        let mut doc = PdfDocumentImpl {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            security: PdfSecurity::default(),
        };
        for _ in 0..page_count {
            doc.add_page(Size { width: 595, height: 842 });
        }

        Ok(Box::new(doc))
    }

    /// Return active reader backend name.
    pub fn backend_name(&self) -> &'static str {
        self.backend_name
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
}

impl PdfDocumentImpl {
    fn new(page_size: Size) -> Self {
        let mut document = Self {
            pages: Vec::new(),
            metadata: PdfMetadata::default(),
            security: PdfSecurity::default(),
        };
        document.add_page(page_size);
        document
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
        let page = Box::new(PdfPageImpl::new(size));
        self.pages.push(page);
        (self.pages.len() - 1) as u32
    }
    
    fn insert_page(&mut self, index: u32, size: Size) -> u32 {
        if index <= self.pages.len() as u32 {
            let page = Box::new(PdfPageImpl::new(size));
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
    /// Form field definitions keyed by field name.
    form_fields: HashMap<String, PdfFormField>,
}

impl PdfPageImpl {
    fn new(size: Size) -> Self {
        Self {
            size,
            content: Vec::new(),
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
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} rg\nBT /F1 {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                font_size,
                x,
                y,
                escaped
            )
            .as_bytes(),
        );
    }
    
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color) {
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} RG\n{:.2} w {:.2} {:.2} m {:.2} {:.2} l S\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                width,
                x1,
                y1,
                x2,
                y2
            )
            .as_bytes(),
        );
    }
    
    fn draw_rect(&mut self, rect: Rect, width: f32, color: Color) {
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} RG\n{:.2} w {} {} {} {} re S\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                width,
                rect.x,
                rect.y,
                rect.width,
                rect.height
            )
            .as_bytes(),
        );
    }
    
    fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.content.extend_from_slice(
            format!(
                "{:.3} {:.3} {:.3} rg\n{} {} {} {} re f\n",
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
                rect.x,
                rect.y,
                rect.width,
                rect.height
            )
            .as_bytes(),
        );
    }
    
    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        self.content.extend_from_slice(
            format!(
                "% image {} bytes at {},{},{},{}\n",
                image.len(),
                rect.x,
                rect.y,
                rect.width,
                rect.height
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
}

/// PDF form field types
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

fn pdf_escape_literal(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
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
    // 3: Font object
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    let mut page_object_ids = Vec::new();

    for page in &doc.pages {
        let content_obj_id = (objects.len() + 1) as u32;
        let stream = page.content();
        let stream_text = String::from_utf8_lossy(&stream);
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream_text
        ));

        let page_obj_id = (objects.len() + 1) as u32;
        let size = page.size();
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            size.width,
            size.height,
            content_obj_id
        ));
        page_object_ids.push(page_obj_id);
    }

    let info_obj_id = (objects.len() + 1) as u32;
    objects.push(format!(
        "<< /Title ({}) /Author ({}) /Subject ({}) /Creator ({}) /Producer ({}) >>",
        pdf_escape_literal(&doc.metadata.title),
        pdf_escape_literal(&doc.metadata.author),
        pdf_escape_literal(&doc.metadata.subject),
        pdf_escape_literal(&doc.metadata.creator),
        pdf_escape_literal(&doc.metadata.producer),
    ));

    let kids = page_object_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");

    objects[0] = "<< /Type /Catalog /Pages 2 0 R >>".to_string();
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
