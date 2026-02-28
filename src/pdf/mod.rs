//! PDF document abstraction and in-memory implementation.

use crate::core::{Rect, Size, Color};
use std::collections::HashMap;
use std::fs;

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
    // PDF writer properties
}

impl PdfWriter {
    /// Create a new PDF writer
    pub fn new() -> Self {
        Self {}
    }
    
    /// Create a new document
    pub fn create_document(&self, page_size: Size) -> Box<dyn PdfDocument> {
        Box::new(PdfDocumentImpl::new(page_size))
    }
}

/// PDF reader
pub struct PdfReader {
    // PDF reader properties
}

impl PdfReader {
    /// Create a new PDF reader
    pub fn new() -> Self {
        Self {}
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
        for line in text.lines() {
            if let Some(num) = line.strip_prefix("pages:") {
                if let Ok(parsed) = num.parse::<usize>() {
                    page_count = parsed.max(1);
                }
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
}

/// PDF document implementation
struct PdfDocumentImpl {
    pages: Vec<Box<dyn PdfPage>>,
    metadata: PdfMetadata,
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
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-FAKE\n");
        out.extend_from_slice(format!("pages:{}\n", self.pages.len()).as_bytes());
        for (i, page) in self.pages.iter().enumerate() {
            out.extend_from_slice(format!("page:{i}:{}bytes\n", page.content().len()).as_bytes());
        }
        Ok(out)
    }
}

/// PDF page implementation
struct PdfPageImpl {
    size: Size,
    content: Vec<u8>,
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
    
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, _color: Color) {
        self.content.extend_from_slice(format!("text:{text}@{x},{y}:{font_size}\n").as_bytes());
    }
    
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, _color: Color) {
        self.content.extend_from_slice(format!("line:{x1},{y1}->{x2},{y2}:{width}\n").as_bytes());
    }
    
    fn draw_rect(&mut self, rect: Rect, width: f32, _color: Color) {
        self.content.extend_from_slice(format!("rect:{},{},{},{}:{width}\n", rect.x, rect.y, rect.width, rect.height).as_bytes());
    }
    
    fn fill_rect(&mut self, rect: Rect, _color: Color) {
        self.content.extend_from_slice(format!("fill:{},{},{},{}\n", rect.x, rect.y, rect.width, rect.height).as_bytes());
    }
    
    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        self.content.extend_from_slice(format!("img:{}:{},{},{},{}\n", image.len(), rect.x, rect.y, rect.width, rect.height).as_bytes());
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
    TextField {
        name: String,
        rect: Rect,
        value: String,
    },
    CheckBox {
        name: String,
        rect: Rect,
        checked: bool,
    },
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
