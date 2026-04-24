//! PDF page implementation.

use crate::core::{Color, Rect, Size};
use crate::pdf::types::*;
use crate::pdf::PdfPage;
use crate::pdf::writer::{pdf_escape_literal, pdf_form_field_name};
use crate::core::coords::to_pdf_y;
use crate::pdf::reader::hex_encode;
use std::collections::HashMap;

pub(crate) struct PdfPageImpl {
    /// Page size in points.
    pub(crate) size: Size,
    /// Encoded draw command payload (placeholder implementation).
    pub(crate) content: Vec<u8>,
    /// Font resource key used for text operators.
    pub(crate) font_resource: String,
    /// Form field definitions keyed by field name.
    pub(crate) form_fields: HashMap<String, PdfFormField>,
}
impl PdfPageImpl {
    pub(crate) fn new(size: Size, font_resource: &str) -> Self {
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

