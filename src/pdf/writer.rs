// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PDF writer for document generation.

use crate::core::{Rect, Size};
use crate::pdf::annotation::{Annotation, AnnotationFlags, AnnotationType};
use crate::pdf::hyperlink::Hyperlink as PdfHyperlink;
use crate::pdf::hyperlink::LinkAction as HyperlinkLinkAction;
use crate::pdf::security::serialize_security_diagnostics_entries;
#[cfg(feature = "pdf-encryption")]
use crate::pdf::security::{security_profile_encryption, PdfEncryption};
use crate::pdf::types::*;
use crate::pdf::PdfDocument;
use crate::pdf::PdfDocumentImpl;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

/// PDF writer
pub struct PdfWriter {
    /// Backend profile name used by writer diagnostics.
    backend_name: &'static str,
}
impl PdfWriter {
    /// Create a new PDF writer
    pub fn new() -> Self {
        Self { backend_name: "pdf-minimal-v1" }
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
crate::impl_default_via_new!(PdfWriter);

/// Document-level encryption state for the bytes currently being written.
///
/// A single concrete type exists in every build so `build_minimal_pdf_bytes` does
/// not need a feature-gated type annotation; without the `pdf-encryption` feature
/// the only inhabitant is [`WriterEncryption::Open`].
#[derive(Debug, Clone)]
pub(crate) enum WriterEncryption {
    /// No document-level encryption is applied.
    Open,
    /// The standard handler is applied with this key material.
    #[cfg(feature = "pdf-encryption")]
    Applied(Box<PdfEncryption>),
}

impl WriterEncryption {
    /// Encrypt one stream's bytes, or return them unchanged when open.
    ///
    /// A stream body is binary, so the encrypted form is the raw IV followed by the ciphertext —
    /// not the hex `<...>` string syntax that string objects use.
    fn encrypt_stream(&self, stream: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            WriterEncryption::Open => Ok(stream.to_vec()),
            #[cfg(feature = "pdf-encryption")]
            WriterEncryption::Applied(encryption) => encryption.encrypt_stream(stream),
        }
    }

    /// Renders one PDF string value.
    ///
    /// An open build keeps the literal `(...)` form. An encrypted build replaces it with the
    /// AES-256 hex-string syntax (`/CFM /AESV3`): ISO 32000-2 encrypts **every** string in the
    /// document, so a plaintext literal is mangled by a conforming reader.
    fn text(&self, value: &str) -> Result<String, Error> {
        match self {
            WriterEncryption::Open => Ok(format!("({})", pdf_escape_literal(value))),
            #[cfg(feature = "pdf-encryption")]
            WriterEncryption::Applied(encryption) => encryption
                .encrypt_content(value.as_bytes())
                .map_err(|error| Error::other(format!("PDF string encryption failed: {error}"))),
        }
    }

    /// Renders an array's worth of string values separated by spaces (no brackets).
    fn text_array(&self, values: &[String]) -> Result<String, Error> {
        let mut rendered = Vec::with_capacity(values.len());
        for value in values {
            rendered.push(self.text(value)?);
        }
        Ok(rendered.join(" "))
    }

    /// The document identifier to write into the trailer's `/ID`, when encryption is applied.
    ///
    /// ISO 32000-1 Algorithm 2 hashes the first `/ID` element into the file key, so a file
    /// without it cannot be decrypted by a conforming reader even when `/O`/`/U` are correct.
    #[cfg(feature = "pdf-encryption")]
    fn file_id_hex(&self) -> Option<String> {
        match self {
            WriterEncryption::Open => None,
            WriterEncryption::Applied(encryption) => Some(encryption.file_id_hex()),
        }
    }

    #[cfg(not(feature = "pdf-encryption"))]
    fn file_id_hex(&self) -> Option<String> {
        None
    }

    /// Build the standard encryption dictionary when encryption is applied.
    ///
    /// Only meaningful with the `pdf-encryption` feature: without it no ciphertext
    /// exists, so the sole inhabitant is [`WriterEncryption::Open`] and there is no
    /// dictionary to build.
    #[cfg(feature = "pdf-encryption")]
    fn encryption_dictionary(&self) -> Option<String> {
        match self {
            WriterEncryption::Open => None,
            WriterEncryption::Applied(encryption) => Some(encryption.build_encryption_dictionary()),
        }
    }
}

/// One entry in the object table built by [`build_minimal_pdf_bytes`].
///
/// A text object is a dictionary or array. A stream object carries its **raw** bytes: the
/// encrypted body is binary, so wrapping it in the hex `<...>` string syntax (which is what the
/// first interop attempt did) made readers decrypt the ASCII text instead of the ciphertext.
enum PdfObjectBody {
    Text(String),
    Stream { dict: String, data: Vec<u8> },
}

pub(crate) fn build_minimal_pdf_bytes(doc: &PdfDocumentImpl) -> Result<Vec<u8>, std::io::Error> {
    if doc.pages.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "document must contain at least one page"));
    }
    // Document-level standard security: a non-default `PdfSecurity` profile under the
    // `pdf-encryption` feature turns into a real `/Encrypt` dictionary and AES-256-CBC
    // encryption (PDF 2.0, V5/R6, `/CFM /AESV3`) of every stream and string. Without the
    // feature the profile stays intent only (see the `% RW-NOTE` marker below), so the file
    // is honest either way.
    #[cfg(feature = "pdf-encryption")]
    let writer_encryption = match security_profile_encryption(&doc.security) {
        Some(Ok(encryption)) => WriterEncryption::Applied(Box::new(encryption)),
        None => WriterEncryption::Open,
        Some(Err(error)) => {
            return Err(Error::other(format!("PDF encryption setup failed: {error}")))
        }
    };
    #[cfg(not(feature = "pdf-encryption"))]
    let writer_encryption = WriterEncryption::Open;
    let mut objects: Vec<PdfObjectBody> = Vec::new();

    // 1: Catalog (filled after objects known)
    objects.push(PdfObjectBody::Text(String::new()));
    // 2: Pages tree (filled after page objects known)
    objects.push(PdfObjectBody::Text(String::new()));
    let mut font_object_ids: HashMap<String, u32> = HashMap::new();
    for font in &doc.fonts {
        let font_obj_id = if font.embedded_data.is_empty() {
            let id = (objects.len() + 1) as u32;
            objects.push(PdfObjectBody::Text(format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /{} >>",
                font.base_font
            )));
            id
        } else {
            let file_obj_id = (objects.len() + 1) as u32;
            // The font program is binary, so it travels as a raw stream body rather than a
            // `String`. ASCII85 keeps the unencrypted body ASCII; with encryption the bytes are
            // encrypted **after** the filter, exactly as a reader reverses them (decrypt, then
            // decode), and the `/Length` is the emitted body's byte count either way.
            let encoded = ascii85_encode(&font.embedded_data);
            let data = writer_encryption.encrypt_stream(encoded.as_bytes()).map_err(|error| {
                Error::other(format!("PDF font-stream encryption failed: {error}"))
            })?;
            objects.push(PdfObjectBody::Stream {
                dict: format!("<< /Length {} /Filter /ASCII85Decode >>", data.len()),
                data,
            });
            let descriptor_obj_id = (objects.len() + 1) as u32;
            let mut descriptor = format!(
                "<< /Type /FontDescriptor /FontName /{} /Flags 32 /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontBBox [0 -200 1000 900] /FontFile2 {} 0 R",
                font.base_font,
                file_obj_id
            );
            if let Some(path) = &font.source_path {
                let literal = writer_encryption.text(path)?;
                descriptor.push_str(&format!(" /RWFontPath {literal}"));
            }
            descriptor.push_str(" >>");
            objects.push(PdfObjectBody::Text(descriptor));
            let id = (objects.len() + 1) as u32;
            objects.push(PdfObjectBody::Text(format!(
                "<< /Type /Font /Subtype /TrueType /BaseFont /{} /Encoding /WinAnsiEncoding /FontDescriptor {} 0 R >>",
                font.base_font,
                descriptor_obj_id
            )));
            id
        };
        font_object_ids.insert(font.resource_name.clone(), font_obj_id);
    }
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
    // Two-phase emission. Phase one writes each page's content stream and its page
    // object with an **empty** annotation list, so every page object number is known
    // before any annotation dictionary is serialised. Phase two emits the form-field
    // widgets, annotations and hyperlinks — a `/P` entry or a GoTo `/D` must name the
    // page object, not the 1-based page number the managers index by — and back-patches
    // each page's `/Annots` array. Predicting the object count instead (the previous
    // approach) silently drifted the moment an annotation contributed more than one
    // object; PDF object order carries no meaning, so deferring is both simpler and exact.
    let mut page_object_ids: Vec<u32> = Vec::with_capacity(doc.pages.len());
    // Index into `objects` of each page object, for the `/Annots` back-patch in phase two.
    let mut page_object_slots: Vec<usize> = Vec::with_capacity(doc.pages.len());
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
        let stream_bytes = writer_encryption.encrypt_stream(&stream).map_err(|error| {
            Error::other(format!("PDF content-stream encryption failed: {error}"))
        })?;
        objects.push(PdfObjectBody::Stream {
            dict: format!("<< /Length {} >>", stream_bytes.len()),
            data: stream_bytes,
        });
        let page_slot = objects.len();
        page_object_ids.push((page_slot + 1) as u32);
        page_object_slots.push(page_slot);
        let size = page.size();
        objects.push(PdfObjectBody::Text(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << {} >> >> /Contents {} 0 R >>",
            size.width,
            size.height,
            font_resources,
            content_obj_id
        )));
    }
    // Phase two: emit widgets, annotations and links, then connect each page's `/Annots`.
    for (index, page) in doc.pages.iter().enumerate() {
        let page_object_id = page_object_ids[index];
        let mut page_form_field_ids = Vec::new();
        for field in page.form_fields() {
            let field_obj_id = (objects.len() + 1) as u32;
            objects.push(PdfObjectBody::Text(serialize_pdf_form_field_widget(
                &field,
                &writer_encryption,
            )?));
            page_form_field_ids.push(field_obj_id);
            all_form_field_object_ids.push(field_obj_id);
        }
        // --- Annotation objects for this page ---
        let page_annotations = doc.annotation_manager.get_page_annotations((index + 1) as u32);
        let mut page_annotation_ids = Vec::new();
        for annotation in &page_annotations {
            let annot_obj_id = (objects.len() + 1) as u32;
            objects.push(PdfObjectBody::Text(serialize_pdf_annotation(
                annotation,
                page_object_id,
                &writer_encryption,
            )?));
            page_annotation_ids.push(annot_obj_id);
        }
        // --- Hyperlink objects for this page ---
        let page_links = doc.hyperlink_manager.get_page_links((index + 1) as u32);
        for link in &page_links {
            let link_obj_id = (objects.len() + 1) as u32;
            objects.push(PdfObjectBody::Text(serialize_pdf_hyperlink(
                link,
                &page_object_ids,
                &writer_encryption,
            )?));
            page_annotation_ids.push(link_obj_id);
        }
        // Collect all /Annots entries: form fields + annotations + hyperlinks
        let mut all_annot_ids = Vec::new();
        all_annot_ids.extend(page_form_field_ids);
        all_annot_ids.extend(page_annotation_ids);
        if !all_annot_ids.is_empty() {
            let refs =
                all_annot_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" ");
            let slot = page_object_slots[index];
            let page_body = match &objects[slot] {
                PdfObjectBody::Text(body) => body.clone(),
                PdfObjectBody::Stream { .. } => {
                    return Err(Error::other("internal: page object slot holds a stream"));
                }
            };
            let body = page_body
                .strip_suffix(" >>")
                .expect("page object template always ends with a dictionary close");
            objects[slot] = PdfObjectBody::Text(format!("{body} /Annots [{refs}] >>"));
        }
    }
    let info_obj_id = (objects.len() + 1) as u32;
    let title = writer_encryption.text(&doc.metadata.title)?;
    let author = writer_encryption.text(&doc.metadata.author)?;
    let subject = writer_encryption.text(&doc.metadata.subject)?;
    let creator = writer_encryption.text(&doc.metadata.creator)?;
    let producer = writer_encryption.text(&doc.metadata.producer)?;
    objects.push(PdfObjectBody::Text(format!(
        "<< /Title {title} /Author {author} /Subject {subject} /Creator {creator} /Producer {producer} >>"
    )));
    let kids = page_object_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" ");
    // Emit the standard security handler as its own dictionary object, then extend
    // the catalog with a `/Crypt` cross-reference filter object referenced by the
    // encryption dictionary's `/StmF` / `/StrF` `/StdCF` name. Without this indirection
    // `/StdCF` would resolve to nothing and readers (and honest docs) would reject it.
    #[cfg(feature = "pdf-encryption")]
    let encrypt_object_id = writer_encryption.encryption_dictionary().map(|dictionary| {
        let encryption_id = (objects.len() + 1) as u32;
        objects.push(PdfObjectBody::Text(dictionary));
        encryption_id
    });
    #[cfg(not(feature = "pdf-encryption"))]
    let encrypt_object_id: Option<u32> = None;
    let acroform_obj_id = if all_form_field_object_ids.is_empty() {
        None
    } else {
        let id = (objects.len() + 1) as u32;
        let refs = all_form_field_object_ids
            .iter()
            .map(|field_id| format!("{field_id} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        objects.push(PdfObjectBody::Text(format!("<< /Fields [{refs}] /NeedAppearances true >>")));
        Some(id)
    };
    // The `/Encrypt` dictionary belongs in the **trailer** (ISO 32000-1 §7.5.5); the catalog
    // carries only the document structure. Keeping a second copy in the catalog is non-standard
    // and was the shape qpdf used to report encryption while failing to decrypt the streams.
    objects[0] = PdfObjectBody::Text(match acroform_obj_id {
        Some(acroform_id) => {
            format!("<< /Type /Catalog /Pages 2 0 R /AcroForm {acroform_id} 0 R >>")
        }
        None => "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
    });
    objects[1] = PdfObjectBody::Text(format!(
        "<< /Type /Pages /Count {} /Kids [{}] >>",
        page_object_ids.len(),
        kids
    ));
    let mut out = Vec::new();
    // R=6 is the PDF 2.0 standard security handler; declare that version for an encrypted file
    // so the header does not promise less than the encryption dictionary requires.
    let pdf_version = if encrypt_object_id.is_some() { "2.0" } else { "1.4" };
    out.extend_from_slice(format!("%PDF-{pdf_version}\n").as_bytes());
    out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");
    let mut offsets: Vec<usize> = Vec::new();
    for (idx, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let obj_id = idx + 1;
        out.extend_from_slice(format!("{obj_id} 0 obj\n").as_bytes());
        match body {
            PdfObjectBody::Text(text) => out.extend_from_slice(text.as_bytes()),
            PdfObjectBody::Stream { dict, data } => {
                out.extend_from_slice(dict.as_bytes());
                out.extend_from_slice(b"\nstream\n");
                out.extend_from_slice(data);
                out.extend_from_slice(b"\nendstream");
            }
        }
        out.extend_from_slice(b"\nendobj\n");
    }
    // Security marker as a standalone comment line *outside* any dictionary so
    // it can never corrupt object/trailer structure or leak plaintext secrets.
    let security_marker = match encrypt_object_id {
        Some(encryption_id) => format!(
            "% RW-NOTE: PDF encryption applied (standard /Filter /Standard handler, AES-256-CBC, R=6); see /Encrypt {encryption_id} 0 R"
        ),
        None => serialize_security_diagnostics_entries(&doc.security),
    };
    if !security_marker.is_empty() {
        out.extend_from_slice(format!("\n{security_marker}\n").as_bytes());
    }
    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    let id_entry = writer_encryption
        .file_id_hex()
        .map(|id| format!(" /ID [<{id}> <{id}>]"))
        .unwrap_or_default();
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R{}{} >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            info_obj_id,
            encrypt_object_id.map(|id| format!(" /Encrypt {id} 0 R")).unwrap_or_default(),
            id_entry,
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
pub(crate) fn serialize_pdf_annotation(
    annotation: &Annotation,
    page_object_id: u32,
    encryption: &WriterEncryption,
) -> Result<String, Error> {
    let subtype = annotation_type_name(annotation.annotation_type);
    let rect = pdf_rect(&annotation.rect);
    let mut dict = format!("<< /Type /Annot /Subtype /{subtype} /Rect [{rect}]");
    // Contents (optional)
    if !annotation.contents.is_empty() {
        dict.push_str(&format!(" /Contents {}", encryption.text(&annotation.contents)?));
    }
    // Author (optional)
    if !annotation.author.is_empty() {
        dict.push_str(&format!(" /T {}", encryption.text(&annotation.author)?));
    }
    // Creation date (optional)
    if !annotation.creation_date.is_empty() {
        dict.push_str(&format!(" /CreationDate {}", encryption.text(&annotation.creation_date)?));
    }
    // Modification date (optional)
    if !annotation.modification_date.is_empty() {
        dict.push_str(&format!(" /M {}", encryption.text(&annotation.modification_date)?));
    }
    // Flags (optional, default 0)
    let flags = encode_annotation_flags(annotation.flags);
    if flags != 0 {
        dict.push_str(&format!(" /F {flags}"));
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
    // Opacity as /CA (optional, default 1.0).
    //
    // `opacity` is a `pub` field, so a caller can bypass `with_opacity`'s normalisation and store a
    // non-finite value directly. Writing `NaN`/`inf` into `/CA` would produce a PDF no viewer can
    // parse, so a non-finite value is treated as the documented default of full opacity here, which
    // is the same normalisation the setter applies.
    let opacity =
        if annotation.opacity.is_finite() { annotation.opacity.clamp(0.0, 1.0) } else { 1.0 };
    if (opacity - 1.0).abs() > f32::EPSILON {
        dict.push_str(&format!(" /CA {opacity:.3}"));
    }
    // Page reference: the object number of the page whose `/Annots` lists this entry.
    // `annotation.page` is a 1-based page number, not an object number, so writing it
    // here made every annotation point at an unrelated object.
    dict.push_str(&format!(" /P {page_object_id} 0 R"));
    dict.push_str(" >>");
    Ok(dict)
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
pub(crate) fn serialize_pdf_hyperlink(
    link: &PdfHyperlink,
    page_object_ids: &[u32],
    encryption: &WriterEncryption,
) -> Result<String, Error> {
    let rect = pdf_rect(&link.rect);
    let mut dict = format!("<< /Type /Annot /Subtype /Link /Rect [{rect}]");
    // Border (optional)
    dict.push_str(&format!(
        " /Border [{} {} {}",
        link.border.horizontal_corner_radius,
        link.border.vertical_corner_radius,
        link.border.border_width,
    ));
    if let Some(ref dash) = link.border.dash_pattern {
        let dash_str = dash.iter().map(|v| format!("{v:.1}")).collect::<Vec<_>>().join(" ");
        dict.push_str(&format!(" /D [{dash_str}]"));
    }
    dict.push(']');
    // Highlight mode
    dict.push_str(&format!(" /H /{}", highlight_mode_name(link.highlight_mode)));
    // Action. A `GoToPage` whose page number names nothing in this document has no
    // honest target, so the annotation is emitted without an action rather than with a
    // reference to an unrelated object.
    if let Some(action) = serialize_link_action(&link.action, page_object_ids, encryption)? {
        dict.push_str(&format!(" /A {action}"));
    }
    // Tooltip (optional)
    if !link.tooltip.is_empty() {
        dict.push_str(&format!(" /Contents {}", encryption.text(&link.tooltip)?));
    }
    dict.push_str(" >>");
    Ok(dict)
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

/// Resolves a **1-based page number** to the page object's object number.
///
/// `page_object_ids` is indexed by page position (page 1 is index 0). A number outside
/// the document is clamped to the nearest existing page, so a caller that stores a stale
/// page number still produces a valid reference.
fn resolve_page_object(page: u32, page_object_ids: &[u32]) -> Option<u32> {
    if page_object_ids.is_empty() {
        return None;
    }
    let index = (page.saturating_sub(1) as usize).min(page_object_ids.len() - 1);
    page_object_ids.get(index).copied()
}

/// Serialize a [`HyperlinkLinkAction`] as a PDF action dictionary.
///
/// Returns `None` for a `GoToPage` that cannot be resolved to a page object.
fn serialize_link_action(
    action: &HyperlinkLinkAction,
    page_object_ids: &[u32],
    encryption: &WriterEncryption,
) -> Result<Option<String>, Error> {
    match action {
        HyperlinkLinkAction::GoToPage { page, x, y } => {
            let Some(page_object_id) = resolve_page_object(*page, page_object_ids) else {
                return Ok(None);
            };
            Ok(Some(format!(
                "<< /Type /Action /S /GoTo /D [{page_object_id} 0 R /XYZ {x} {y} null] >>"
            )))
        }
        HyperlinkLinkAction::GoToNamedDestination(name) => {
            Ok(Some(format!("<< /Type /Action /S /GoTo /D {} >>", encryption.text(name)?)))
        }
        HyperlinkLinkAction::Uri(uri) => {
            Ok(Some(format!("<< /Type /Action /S /URI /URI {} >>", encryption.text(uri)?)))
        }
        HyperlinkLinkAction::LaunchFile(path) => {
            Ok(Some(format!("<< /Type /Action /S /Launch /F {} >>", encryption.text(path)?)))
        }
        HyperlinkLinkAction::JavaScript(script) => {
            Ok(Some(format!("<< /Type /Action /S /JavaScript /JS {} >>", encryption.text(script)?)))
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
            Ok(Some(format!("<< /Type /Action /S /Named /N /{name_str} >>")))
        }
    }
}

// ---------------------------------------------------------------------------
// Form field serialization
// ---------------------------------------------------------------------------

pub(crate) fn serialize_pdf_form_field_widget(
    field: &PdfFormField,
    encryption: &WriterEncryption,
) -> Result<String, Error> {
    match field {
        PdfFormField::TextField { name, rect, value } => Ok(format!(
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T {} /Rect [{}] /V {} >>",
            encryption.text(name)?,
            pdf_rect(rect),
            encryption.text(value)?,
        )),
        PdfFormField::CheckBox {
            name,
            rect,
            checked,
        } => {
            let state = if *checked { "Yes" } else { "Off" };
            Ok(format!(
                "<< /Type /Annot /Subtype /Widget /FT /Btn /T {} /Rect [{}] /V /{} /AS /{} >>",
                encryption.text(name)?,
                pdf_rect(rect),
                state,
                state,
            ))
        }
        PdfFormField::Button { name, rect, text } => Ok(format!(
            "<< /Type /Annot /Subtype /Widget /FT /Btn /Ff 65536 /T {} /Rect [{}] /MK << /CA {} >> >>",
            encryption.text(name)?,
            pdf_rect(rect),
            encryption.text(text)?,
        )),
        PdfFormField::ComboBox {
            name,
            rect,
            value,
            options,
        } => {
            let options_text = encryption.text_array(options.as_slice())?;
            Ok(format!(
                "<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T {} /Rect [{}] /V {} /Opt [{}] >>",
                encryption.text(name)?,
                pdf_rect(rect),
                encryption.text(value)?,
                options_text,
            ))
        }
        PdfFormField::ListBox {
            name,
            rect,
            selected,
            options,
        } => {
            let options_text = encryption.text_array(options.as_slice())?;
            let selected_text = selected
                .iter()
                .map(|index| index.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            Ok(format!(
                "<< /Type /Annot /Subtype /Widget /FT /Ch /T {} /Rect [{}] /Opt [{}] /I [{}] >>",
                encryption.text(name)?,
                pdf_rect(rect),
                options_text,
                selected_text,
            ))
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
    let total = pagination.start_at.saturating_add(page_count.saturating_sub(1) as u32);
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
    // `rect.width as f32 as i32` was a double narrowing that both lost precision (a `u32` above
    // `2^24` is not exactly representable in `f32`) and could overflow on the addition. The width
    // is already an integer count of pixels, so it converts straight to `i32` — saturating at the
    // bound — and the sum saturates rather than wrapping into a negative coordinate.
    let x1 = rect.x;
    let y1 = rect.y;
    let w = i32::try_from(rect.width).unwrap_or(i32::MAX);
    let h = i32::try_from(rect.height).unwrap_or(i32::MAX);
    let x2 = rect.x.saturating_add(w);
    let y2 = rect.y.saturating_add(h);
    format!("{x1} {y1} {x2} {y2}")
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
    text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

pub(crate) fn sanitize_pdf_font_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "EmbeddedFont".to_string();
    }
    trimmed
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' })
        .collect()
}

/// Encodes `bytes` as PDF ASCII85 (`/Filter /ASCII85Decode`).
///
/// Binary font programs cannot live in a Rust `String`, and `from_utf8_lossy` corrupts them.
/// ASCII85 keeps the stream body within printable ASCII while preserving every input byte, and
/// the reader's `ASCII85Decode` filter restores it. The terminator is `~>`, as the PDF spec
/// requires.
fn ascii85_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 5 / 4 + 4);
    for chunk in bytes.chunks(4) {
        let mut block = [0u8; 4];
        let count = chunk.len();
        block[..count].copy_from_slice(chunk);
        let value = u32::from_be_bytes(block);
        if count == 4 && value == 0 {
            out.push('z');
            continue;
        }
        let mut digits = [0u8; 5];
        let mut v = value;
        for slot in digits.iter_mut().rev() {
            *slot = (v % 85) as u8;
            v /= 85;
        }
        // A partial final group emits only the characters that carry its bytes.
        let emit = count + 1;
        for &digit in digits.iter().take(emit) {
            out.push((b'!' + digit) as char);
        }
    }
    out.push_str("~>");
    out
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
        let s = serialize_pdf_annotation(&annot, 1, &WriterEncryption::Open).expect("open");
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
        let s = serialize_pdf_hyperlink(&link, &[], &WriterEncryption::Open).expect("open");
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
            HyperlinkAction::GoToPage { page: 3, x: 0.0, y: 0.0 },
        );
        let s = serialize_pdf_hyperlink(&link, &[3], &WriterEncryption::Open).expect("open");
        assert!(s.contains("/S /GoTo /D [3 0 R /XYZ 0 0 null]"));
    }

    /// An annotation's `/P` and a GoTo link's `/D` must name the **page object** the
    /// document actually emits, not the 1-based page number the managers index by.
    #[test]
    fn annotations_and_goto_links_name_page_objects_not_page_numbers() {
        use crate::pdf::annotation::{Annotation, AnnotationType};
        use crate::pdf::hyperlink::{Hyperlink, LinkAction};

        let mut doc = PdfDocumentImpl::new(Size { width: 100, height: 100 });
        doc.add_page(Size { width: 200, height: 100 });
        doc.annotation_manager.add_annotation(Annotation::new(
            "a0".to_string(),
            1,
            AnnotationType::Text,
            Rect::new(1, 2, 3, 4),
        ));
        doc.hyperlink_manager.add_link(Hyperlink::new(
            "l0".to_string(),
            1,
            Rect::new(5, 6, 7, 8),
            LinkAction::GoToPage { page: 2, x: 0.0, y: 0.0 },
        ));

        let bytes = build_minimal_pdf_bytes(&doc).expect("serialize");
        let text = String::from_utf8_lossy(&bytes);
        let objects = crate::pdf::reader::parse_pdf_objects(&text);
        let mut page_ids: Vec<u32> = objects
            .iter()
            .filter(|(_, body)| body.contains("/Type /Page "))
            .map(|(id, _)| *id)
            .collect();
        page_ids.sort_unstable();
        assert_eq!(page_ids.len(), 2);

        let annotation = objects
            .values()
            .find(|body| body.contains("/Subtype /Text"))
            .expect("annotation object");
        assert!(
            annotation.contains(&format!("/P {} 0 R", page_ids[0])),
            "annotation must point at its page object: {annotation}"
        );

        let link =
            objects.values().find(|body| body.contains("/Subtype /Link")).expect("link object");
        assert!(
            link.contains(&format!("/S /GoTo /D [{} 0 R", page_ids[1])),
            "GoTo must point at the target page object: {link}"
        );
    }

    /// The `/Annots` array is back-patched from the emitted objects, so every annotation, link and
    /// widget is listed by its page even though page objects are written first. The previous
    /// object-count projection silently lost or mis-numbered entries once a page contributed a
    /// different number of objects than predicted.
    #[test]
    fn page_annots_list_every_annotation_object_after_back_patch() {
        use crate::pdf::annotation::{Annotation, AnnotationType};
        use crate::pdf::hyperlink::{Hyperlink, LinkAction};

        let mut doc = PdfDocumentImpl::new(Size { width: 100, height: 100 });
        doc.add_page(Size { width: 200, height: 100 });
        for (page, id, rect) in [
            (1u32, "a0", Rect::new(1, 2, 3, 4)),
            (1u32, "a1", Rect::new(5, 6, 7, 8)),
            (2u32, "a2", Rect::new(9, 10, 11, 12)),
        ] {
            doc.annotation_manager.add_annotation(Annotation::new(
                id.to_string(),
                page,
                AnnotationType::Text,
                rect,
            ));
        }
        doc.hyperlink_manager.add_link(Hyperlink::new(
            "l0".to_string(),
            1,
            Rect::new(0, 0, 10, 10),
            LinkAction::Uri("https://example.com".to_string()),
        ));

        let bytes = build_minimal_pdf_bytes(&doc).expect("serialize");
        let text = String::from_utf8_lossy(&bytes);
        let objects = crate::pdf::reader::parse_pdf_objects(&text);

        let mut page_ids: Vec<u32> = objects
            .iter()
            .filter(|(_, body)| body.contains("/Type /Page "))
            .map(|(id, _)| *id)
            .collect();
        page_ids.sort_unstable();
        assert_eq!(page_ids.len(), 2);

        let mut annotation_ids: Vec<u32> = objects
            .iter()
            .filter(|(_, body)| body.contains("/Type /Annot"))
            .map(|(id, _)| *id)
            .collect();
        annotation_ids.sort_unstable();
        assert_eq!(annotation_ids.len(), 4, "three annotations and one link");

        let listed = |page_id: u32| -> Vec<u32> {
            let body = objects.get(&page_id).expect("page object");
            let start = body
                .find("/Annots [")
                .unwrap_or_else(|| panic!("page {page_id} has no /Annots array: {body}"))
                + "/Annots [".len();
            let end = start + body[start..].find(']').expect("closed /Annots array");
            body[start..end]
                .split_whitespace()
                .step_by(3) // each reference is `<object> 0 R`
                .filter_map(|token| token.parse::<u32>().ok())
                .collect()
        };
        let page_one = listed(page_ids[0]);
        let page_two = listed(page_ids[1]);
        assert_eq!(page_one.len(), 3, "page 1 carries two annotations and one link");
        assert_eq!(page_two.len(), 1, "page 2 carries one annotation");
        for id in &annotation_ids {
            assert!(
                page_one.contains(id) || page_two.contains(id),
                "annotation object {id} is listed by no page"
            );
        }
        for id in page_one.iter().chain(page_two.iter()) {
            assert!(annotation_ids.contains(id), "{id} is not an annotation object");
        }
    }

    /// In an encrypted document a reader decrypts **every** string, so a plaintext literal is
    /// mangled (qpdf showed `/Producer` as a 16-byte hex blob). The writer must therefore emit the
    /// info values as per-object AESV3 hex strings.
    #[cfg(feature = "pdf-encryption")]
    #[test]
    fn encrypted_documents_encrypt_string_values_too() {
        let mut doc = PdfDocumentImpl::new(Size { width: 100, height: 100 });
        doc.metadata.title = "Secret Title".to_string();
        doc.set_security(PdfSecurity {
            user_password: Some("user".to_string()),
            owner_password: Some("owner".to_string()),
            print_permission: true,
            edit_permission: false,
            copy_permission: false,
            annotation_permission: true,
        });
        let bytes = build_minimal_pdf_bytes(&doc).expect("serialize");
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("Secret Title"), "the title must not be echoed in plaintext");
        let objects = crate::pdf::reader::parse_pdf_objects(&text);
        let info = objects.values().find(|body| body.contains("/Producer")).expect("info object");
        assert!(info.contains("/Title <"), "the title must be an encrypted hex string: {info}");
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
        let s = serialize_pdf_annotation(&annot, 2, &WriterEncryption::Open).expect("open");
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
        let s = serialize_pdf_form_field_widget(&field, &WriterEncryption::Open).expect("open");
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
        let s = serialize_pdf_form_field_widget(&field, &WriterEncryption::Open).expect("open");
        assert!(s.contains("/FT /Btn"));
        assert!(s.contains("/V /Yes"));
        assert!(s.contains("/AS /Yes"));
    }
}
