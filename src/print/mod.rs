//! Printing and print preview support.

use crate::core::{Rect, Size};

/// Print document
pub trait PrintDocument {
    /// Get number of pages
    fn page_count(&self) -> u32;
    
    /// Draw page
    fn draw_page(&self, page_num: u32, context: &mut dyn PrintContext);
}

/// Print context
pub trait PrintContext {
    /// Draw text
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32);
    
    /// Draw line
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32);
    
    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, width: f32);
    
    /// Draw filled rectangle
    fn fill_rect(&mut self, rect: Rect, color: u32);
    
    /// Draw image
    fn draw_image(&mut self, image: &[u8], rect: Rect);
    
    /// Get page size
    fn page_size(&self) -> Size;
}

/// Print dialog
pub struct PrintDialog {
    copies: u32,
}

impl PrintDialog {
    pub fn new() -> Self {
        Self { copies: 1 }
    }

    pub fn set_copies(&mut self, copies: u32) {
        self.copies = copies.max(1);
    }

    pub fn show(&self) -> bool {
        self.copies >= 1
    }
}

impl Default for PrintDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Print preview dialog
pub struct PrintPreviewDialog {
    page_count: u32,
    current_page: u32,
}

impl PrintPreviewDialog {
    pub fn new(document: Box<dyn PrintDocument>) -> Self {
        Self {
            page_count: document.page_count(),
            current_page: 0,
        }
    }

    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    pub fn current_page(&self) -> u32 {
        self.current_page
    }

    pub fn next_page(&mut self) {
        if self.current_page + 1 < self.page_count {
            self.current_page += 1;
        }
    }

    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }

    pub fn show(&self) -> bool {
        self.page_count > 0
    }
}

/// Printer
pub struct Printer {
    page_size: Size,
}

impl Printer {
    pub fn new() -> Self {
        Self {
            page_size: Size {
                width: 595,
                height: 842,
            },
        }
    }

    pub fn print(&self, document: &dyn PrintDocument) {
        let mut context = MemoryPrintContext::new(self.page_size);
        for page in 0..document.page_count() {
            document.draw_page(page, &mut context);
            context.end_page();
        }
    }
}

impl Default for Printer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MemoryPrintContext {
    page_size: Size,
    pub commands: Vec<String>,
}

impl MemoryPrintContext {
    pub fn new(page_size: Size) -> Self {
        Self {
            page_size,
            commands: Vec::new(),
        }
    }

    pub fn end_page(&mut self) {
        self.commands.push("page-break".to_string());
    }
}

impl PrintContext for MemoryPrintContext {
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32) {
        self.commands.push(format!("text:{text}@{x},{y}:{font_size}"));
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32) {
        self.commands.push(format!("line:{x1},{y1}->{x2},{y2}:{width}"));
    }

    fn draw_rect(&mut self, rect: Rect, width: f32) {
        self.commands.push(format!("rect:{},{},{},{}:{}", rect.x, rect.y, rect.width, rect.height, width));
    }

    fn fill_rect(&mut self, rect: Rect, color: u32) {
        self.commands.push(format!("fill:{},{},{},{}:{color}", rect.x, rect.y, rect.width, rect.height));
    }

    fn draw_image(&mut self, image: &[u8], rect: Rect) {
        self.commands.push(format!("img:{}bytes:{},{},{},{}", image.len(), rect.x, rect.y, rect.width, rect.height));
    }

    fn page_size(&self) -> Size {
        self.page_size
    }
}
