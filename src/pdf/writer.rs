//! PDF writer for document generation.

use crate::core::{Rect, Size};
use crate::pdf::annotation::{Annotation, AnnotationFlags, AnnotationType};
use crate::pdf::hyperlink::Hyperlink as PdfHyperlink;
use crate::pdf::hyperlink::LinkAction as HyperlinkLinkAction;
use crate::pdf::security::serialize_security_diagnostics_entries;
use crate::pdf::types::*;
use crate::pdf::PdfDocument;
use crate::pdf::PdfDocumentImpl;
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
    // Track all annotation/hyperlink/form-field objects across all pages so we can
    // build a combined /AcroForm later if needed.  We store (object_id, page_index).
    let mut all_widget_object_ids: Vec<(u32, usize)> = Vec::new();

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
            all_widget_object_ids.push((field_obj_id, index));
        }
        // --- Annotation objects for this page ---
        let page_annotations = doc
            .annotation_manager
            .get_page_annotations((index + 1) as u32);
        let mut page_annotation_ids = Vec::new();
        for annotation in &page_annotations {
            let annot_obj_id = (objects.len() + 1) as u32;
            objects.push(serialize_pdf_annotation(annotation));
            page_annotation_ids.push(annot_obj_id);
        }
        // --- Hyperlink objects for this page ---
        let page_links = doc.hyperlink_manager.get_page_links((index + 1) as u32);
        for link in &page_links {
            let link_obj_id = (objects.len() + 1) as u32;
            objects.push(serialize_pdf_hyperlink(link));
            page_annotation_ids.push(link_obj_id);
        }
        // Collect all /Annots entries: form fields + annotations + hyperlinks
        let mut all_annot_ids = Vec::new();
        all_annot_ids.extend(page_form_field_ids);
        all_annot_ids.extend(page_annotation_ids);
        let annots_entry = if all_annot_ids.is_empty() {
            String::new()
        } else {
            let refs = all_annot_ids
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

// ---------------------------------------------------------------------------
// Annotation serialization helpers
// ---------------------------------------------------------------------------

/// Serialise a single [`Annotation`] as a PDF annotation dictionary object.
pub(crate) fn serialize_pdf_annotation(annotation: &Annotation) -> String {
    let subtype = annotation_type_name(annotation.annotation_type);
    let rect = pdf_rect(&annotation.rect);
    let mut dict = format!("<< /Type /Annot /Subtype /{} /Rect [{}]", subtype, rect);
    // Contents (optional)
    if !annotation.contents.is_empty() {
        dict.push_str(&format!(
            " /Contents ({})",
            pdf_escape_literal(&annotation.contents)
        ));
    }
    // Author (optional)
    if !annotation.author.is_empty() {
        dict.push_str(&format!(" /T ({})", pdf_escape_literal(&annotation.author)));
    }
    // Creation date (optional)
    if !annotation.creation_date.is_empty() {
        dict.push_str(&format!(
            " /CreationDate ({})",
            pdf_escape_literal(&annotation.creation_date)
        ));
    }
    // Modification date (optional)
    if !annotation.modification_date.is_empty() {
        dict.push_str(&format!(
            " /M ({})",
            pdf_escape_literal(&annotation.modification_date)
        ));
    }
    // Flags (optional, default 0)
    let flags = encode_annotation_flags(annotation.flags);
    if flags != 0 {
        dict.push_str(&format!(" /F {}", flags));
    }
    // Color /C array (optional)
    if let Some(color) = &annotation.color {
        dict.push_str(&format!(
            " /C [{:.3} {:.3} {:.3}]",
            color.r as f32 / 255.0,
            color.g as f32 / 255.0,
            color.b as f32 / 255.0,
        ));
    }
    // Opacity as /CA (optional, default 1.0)
    if (annotation.opacity - 1.0).abs() > f32::EPSILON {
        dict.push_str(&format!(" /CA {:.3}", annotation.opacity));
    }
    // Page reference (required for some viewers but can be inferred)
    dict.push_str(&format!(" /P {} 0 R", annotation.page));
    dict.push_str(" >>");
    dict
}

/// Map our [`AnnotationType`] to the PDF `/Subtype` name.
fn annotation_type_name(at: AnnotationType) -> &'static str {
    match at {
        AnnotationType::Text => "Text",
        AnnotationType::Highlight => "Highlight",
        AnnotationType::Underline => "Underline",
        AnnotationType::StrikeOut => "StrikeOut",
        AnnotationType::Squiggly => "Squiggly",
        AnnotationType::Link => "Link",
        AnnotationType::Popup => "Popup",
        AnnotationType::Line => "Line",
        AnnotationType::Square => "Square",
        AnnotationType::Circle => "Circle",
        AnnotationType::Polygon => "Polygon",
        AnnotationType::PolyLine => "PolyLine",
        AnnotationType::Ink => "Ink",
        AnnotationType::Stamp => "Stamp",
        AnnotationType::Caret => "Caret",
        AnnotationType::FileAttachment => "FileAttachment",
        AnnotationType::Sound => "Sound",
        AnnotationType::Movie => "Movie",
        AnnotationType::Widget => "Widget",
        AnnotationType::Screen => "Screen",
        AnnotationType::PrinterMark => "PrinterMark",
        AnnotationType::TrapNet => "TrapNet",
        AnnotationType::Watermark => "Watermark",
        AnnotationType::ThreeD => "3D",
    }
}

/// Encode [`AnnotationFlags`] as a PDF bitmask.
fn encode_annotation_flags(flags: AnnotationFlags) -> u32 {
    let mut value: u32 = 0;
    if flags.invisible {
        value |= 1 << 0;
    }
    if flags.hidden {
        value |= 1 << 1;
    }
    if flags.print {
        value |= 1 << 2;
    }
    if flags.no_zoom {
        value |= 1 << 3;
    }
    if flags.no_rotate {
        value |= 1 << 4;
    }
    if flags.no_view {
        value |= 1 << 5;
    }
    if flags.read_only {
        value |= 1 << 6;
    }
    if flags.locked {
        value |= 1 << 7;
    }
    if flags.toggle_no_view {
        value |= 1 << 8;
    }
    if flags.locked_contents {
        value |= 1 << 9;
    }
    value
}

// ---------------------------------------------------------------------------
// Hyperlink serialization helpers
// ---------------------------------------------------------------------------

/// Serialise a single [`PdfHyperlink`] (from crate::pdf::hyperlink) as a PDF
/// Link annotation dictionary object.
pub(crate) fn serialize_pdf_hyperlink(link: &PdfHyperlink) -> String {
    let rect = pdf_rect(&link.rect);
    let mut dict = format!("<< /Type /Annot /Subtype /Link /Rect [{}]", rect);
    // Border (optional)
    dict.push_str(&format!(
        " /Border [{} {} {}",
        link.border.horizontal_corner_radius,
        link.border.vertical_corner_radius,
        link.border.border_width,
    ));
    if let Some(ref dash) = link.border.dash_pattern {
        let dash_str = dash
            .iter()
            .map(|v| format!("{:.1}", v))
            .collect::<Vec<_>>()
            .join(" ");
        dict.push_str(&format!(" /D [{}]", dash_str));
    }
    dict.push(']');
    // Highlight mode
    dict.push_str(&format!(
        " /H /{}",
        highlight_mode_name(link.highlight_mode)
    ));
    // Action
    dict.push_str(&format!(" /A {}", serialize_link_action(&link.action)));
    // Tooltip (optional)
    if !link.tooltip.is_empty() {
        dict.push_str(&format!(
            " /Contents ({})",
            pdf_escape_literal(&link.tooltip)
        ));
    }
    dict.push_str(" >>");
    dict
}

fn highlight_mode_name(mode: crate::pdf::hyperlink::HighlightMode) -> &'static str {
    use crate::pdf::hyperlink::HighlightMode;
    match mode {
        HighlightMode::None => "N",
        HighlightMode::Invert => "I",
        HighlightMode::Outline => "O",
        HighlightMode::Push => "P",
    }
}

/// Serialize a [`HyperlinkLinkAction`] as a PDF action dictionary.
fn serialize_link_action(action: &HyperlinkLinkAction) -> String {
    match action {
        HyperlinkLinkAction::GoToPage { page, x, y } => {
            format!(
                "<< /Type /Action /S /GoTo /D [{} 0 R /XYZ {} {} null] >>",
                page, x, y
            )
        }
        HyperlinkLinkAction::GoToNamedDestination(name) => {
            format!(
                "<< /Type /Action /S /GoTo /D ({}) >>",
                pdf_escape_literal(name)
            )
        }
        HyperlinkLinkAction::Uri(uri) => {
            format!(
                "<< /Type /Action /S /URI /URI ({}) >>",
                pdf_escape_literal(uri)
            )
        }
        HyperlinkLinkAction::LaunchFile(path) => {
            format!(
                "<< /Type /Action /S /Launch /F ({}) >>",
                pdf_escape_literal(path)
            )
        }
        HyperlinkLinkAction::JavaScript(script) => {
            format!(
                "<< /Type /Action /S /JavaScript /JS ({}) >>",
                pdf_escape_literal(script)
            )
        }
        HyperlinkLinkAction::NamedAction(named) => {
            let name_str = match named {
                crate::pdf::hyperlink::NamedAction::NextPage => "NextPage",
                crate::pdf::hyperlink::NamedAction::PrevPage => "PrevPage",
                crate::pdf::hyperlink::NamedAction::FirstPage => "FirstPage",
                crate::pdf::hyperlink::NamedAction::LastPage => "LastPage",
                crate::pdf::hyperlink::NamedAction::Print => "Print",
                crate::pdf::hyperlink::NamedAction::SaveAs => "SaveAs",
            };
            format!("<< /Type /Action /S /Named /N /{} >>", name_str)
        }
    }
}

// ---------------------------------------------------------------------------
// Form field serialization
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::pdf::annotation::Annotation;
    use crate::pdf::hyperlink::Hyperlink as PdfHyperlink2;
    use crate::pdf::hyperlink::LinkAction as HyperlinkAction;

    #[test]
    fn test_annotation_serialization_basic() {
        let annot = Annotation::new(
            "a1".to_string(),
            1,
            crate::pdf::annotation::AnnotationType::Highlight,
            Rect::new(50, 100, 200, 30),
        )
        .with_contents("highlighted text".to_string())
        .with_author("tester".to_string());
        let s = serialize_pdf_annotation(&annot);
        assert!(s.starts_with("<< /Type /Annot /Subtype /Highlight /Rect ["));
        assert!(s.contains("/Contents (highlighted text)"));
        assert!(s.contains("/T (tester)"));
        assert!(s.contains(">>"));
    }

    #[test]
    fn test_hyperlink_serialization_uri() {
        let link = PdfHyperlink2::new(
            "l1".to_string(),
            1,
            Rect::new(10, 10, 100, 20),
            HyperlinkAction::Uri("https://example.com".to_string()),
        )
        .with_tooltip("Example".to_string());
        let s = serialize_pdf_hyperlink(&link);
        assert!(s.starts_with("<< /Type /Annot /Subtype /Link /Rect ["));
        assert!(s.contains("/A << /Type /Action /S /URI /URI (https://example.com) >>"));
        assert!(s.contains("/Contents (Example)"));
        assert!(s.contains(">>"));
    }

    #[test]
    fn test_hyperlink_serialization_goto_page() {
        let link = PdfHyperlink2::new(
            "l2".to_string(),
            1,
            Rect::new(10, 10, 100, 20),
            HyperlinkAction::GoToPage {
                page: 3,
                x: 0.0,
                y: 0.0,
            },
        );
        let s = serialize_pdf_hyperlink(&link);
        assert!(s.contains("/S /GoTo /D [3 0 R /XYZ 0 0 null]"));
    }

    #[test]
    fn test_annotation_flags_encoding() {
        let flags = AnnotationFlags {
            hidden: true,
            print: true,
            locked: true,
            ..AnnotationFlags::default()
        };
        let encoded = encode_annotation_flags(flags);
        assert!(encoded & (1 << 1) != 0); // hidden
        assert!(encoded & (1 << 2) != 0); // print
        assert!(encoded & (1 << 7) != 0); // locked
        assert_eq!(encoded & (1 << 0), 0); // invisible not set
    }

    #[test]
    fn test_annotation_serialization_with_color_and_opacity() {
        let annot = Annotation::new(
            "a2".to_string(),
            2,
            crate::pdf::annotation::AnnotationType::Text,
            Rect::new(0, 0, 50, 50),
        )
        .with_color(Color::RED)
        .with_opacity(0.5);
        let s = serialize_pdf_annotation(&annot);
        assert!(s.contains("/Subtype /Text"));
        assert!(s.contains("/C ["));
        assert!(s.contains("/CA 0.500"));
    }

    #[test]
    fn test_form_field_widget_text() {
        let field = PdfFormField::TextField {
            name: "name".to_string(),
            rect: Rect::new(10, 20, 150, 30),
            value: "default".to_string(),
        };
        let s = serialize_pdf_form_field_widget(&field);
        assert!(s.contains("/Subtype /Widget"));
        assert!(s.contains("/FT /Tx"));
        assert!(s.contains("/T (name)"));
        assert!(s.contains("/V (default)"));
    }

    #[test]
    fn test_form_field_widget_checkbox() {
        let field = PdfFormField::CheckBox {
            name: "agree".to_string(),
            rect: Rect::new(10, 20, 15, 15),
            checked: true,
        };
        let s = serialize_pdf_form_field_widget(&field);
        assert!(s.contains("/FT /Btn"));
        assert!(s.contains("/V /Yes"));
        assert!(s.contains("/AS /Yes"));
    }
}
