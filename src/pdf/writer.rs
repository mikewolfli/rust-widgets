//! PDF writer for document generation.

use crate::core::{Color, Rect, Size};
use crate::pdf::types::*;
use crate::pdf::document::PdfDocumentImpl;
use crate::pdf::PdfDocument;
use crate::pdf::security::serialize_security_diagnostics_entries;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::path::Path;

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

pub(crate) fn build_minimal_pdf_bytes(doc: &PdfDocumentImpl) -> Result<Vec<u8>, std::io::Error> {
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
        let mut stream: Vec<u8> = page.content();
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
pub(crate) fn append_page_number_footer(
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

pub(crate) fn serialize_pdf_form_field_widget(field: &PdfFormField) -> String {
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
pub(crate) fn pdf_rect(rect: &Rect) -> String {
    let x1 = rect.x;
    let y1 = rect.y;
    let x2 = rect.x + rect.width as f32 as i32;
    let y2 = rect.y + rect.height as f32 as i32;
    format!("{} {} {} {}", x1, y1, x2, y2)
}
pub(crate) fn pdf_form_field_name(field: &PdfFormField) -> &str {
    match field {
        PdfFormField::TextField { name, .. }
        | PdfFormField::CheckBox { name, .. }
        | PdfFormField::Button { name, .. }
        | PdfFormField::ComboBox { name, .. }
        | PdfFormField::ListBox { name, .. } => name,
    }
}

pub(crate) fn pdf_escape_literal(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

pub(crate) fn sanitize_pdf_font_name(name: &str) -> String {
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


