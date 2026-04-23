//! PDF document implementation.

use crate::core::{Color, Rect};
use crate::pdf::types::*;
use crate::pdf::metadata::PdfMetadata;
use crate::pdf::page::PdfPageImpl;
use crate::pdf::writer::{build_minimal_pdf_bytes, sanitize_pdf_font_name};
use crate::pdf::PdfDocument;
use crate::pdf::PdfPage;
use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind};
use crate::core::Size;

pub(crate) struct PdfDocumentImpl {
    /// Ordered page list.
    pub(crate) pages: Vec<Box<dyn PdfPage>>,
    /// Document metadata block.
    pub(crate) metadata: PdfMetadata,
    /// Document security policy.
    pub(crate) security: PdfSecurity,
    /// Declared font resources for page content streams.
    pub(crate) fonts: Vec<PdfFontResource>,
    /// Optional page-number footer stamping options.
    pub(crate) pagination: PdfPagination,
}
impl PdfDocumentImpl {
    pub(crate) fn new(page_size: Size) -> Self {
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
    pub(crate) fn new_with_embedded_font(
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
    pub(crate) fn default_font_resource(&self) -> &str {
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

