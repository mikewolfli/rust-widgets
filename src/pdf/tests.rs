//! PDF tests.

use crate::core::{Color, Rect, Size};
use crate::pdf::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
fn temp_path_with_suffix(suffix: &str) -> String {
    let ts =
        SystemTime::now().duration_since(UNIX_EPOCH).expect("clock moved backwards").as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("rw_pdf_test_{}_{}", ts, suffix));
    path.to_string_lossy().to_string()
}
#[test]
fn writer_embeds_font_stream_when_font_path_is_provided() {
    let font_path = temp_path_with_suffix("font.ttf");
    fs::write(&font_path, b"RW_TEST_FONT_BYTES").expect("write test font file");
    let writer = PdfWriter::new();
    let mut doc = writer
        .create_document_with_font_path(Size { width: 595, height: 842 }, "Test Font", &font_path)
        .expect("create document with font path");
    let page = doc.get_page(0).expect("page 0 must exist");
    page.draw_text("hello", 20.0, 20.0, 12.0, Color { r: 0, g: 0, b: 0, a: 255 });
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
        Size { width: 595, height: 842 },
        "EmptyFont",
        &font_path,
    );
    assert!(result.is_err());
    let _ = fs::remove_file(font_path);
}
#[test]
fn writer_stamps_page_number_footer_when_enabled() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
    doc.add_page(Size { width: 595, height: 842 });
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
    let mut doc = writer.create_document(Size { width: 600, height: 840 });
    doc.set_page_numbering_enabled(true);
    doc.set_page_numbering_layout(100.0, 36.0, 12.0);
    let bytes = doc.to_bytes().expect("serialize document");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("BT /F1 12.00 Tf 500.00 36.00 Td (Page 1/1)"));
}
#[test]
fn reader_roundtrip_preserves_page_stream_and_media_box() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 612, height: 792 });
    {
        let page = doc.get_page(0).expect("page exists");
        page.draw_text("hello", 20.0, 24.0, 12.0, Color { r: 0, g: 0, b: 0, a: 255 });
        page.draw_line(10.0, 10.0, 60.0, 10.0, 1.5, Color { r: 10, g: 20, b: 30, a: 255 });
        page.draw_rect(
            Rect { x: 12, y: 14, width: 20, height: 10 },
            1.0,
            Color { r: 40, g: 50, b: 60, a: 255 },
        );
        page.fill_rect(
            Rect { x: 40, y: 20, width: 15, height: 8 },
            Color { r: 70, g: 80, b: 90, a: 255 },
        );
        page.draw_image(&[0xAB, 0xCD, 0xEF], Rect { x: 5, y: 5, width: 2, height: 2 });
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
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
    {
        let page = doc.get_page(0).expect("page exists");
        page.add_text_field("full_name", Rect { x: 40, y: 700, width: 200, height: 24 }, "Alice");
        page.add_checkbox("agree", Rect { x: 40, y: 660, width: 16, height: 16 }, true);
        page.add_button("submit", Rect { x: 40, y: 620, width: 80, height: 24 }, "Submit");
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
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
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
    // Honest security marker: records intent, warns the output is NOT
    // encrypted, and never echoes plaintext passwords into the file.
    assert!(text.contains("% RW-NOTE: PDF encryption requested"));
    assert!(text.contains("NOT encrypted"));
    assert!(text.contains("print=false"));
    assert!(text.contains("edit=true"));
    assert!(text.contains("copy=false"));
    assert!(text.contains("annot=false"));
    assert!(!text.contains("user-secret"));
    assert!(!text.contains("owner-secret"));
    assert!(!text.contains("/RWUserPassword"));
}
#[test]
fn reader_roundtrip_restores_security_diagnostics() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
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
    // Passwords cannot round-trip: the writer never embeds them.
    assert_eq!(security.user_password, None);
    assert_eq!(security.owner_password, None);
    // Permissions round-trip from the intent marker.
    assert!(!security.print_permission);
    assert!(!security.edit_permission);
    assert!(security.copy_permission);
    assert!(!security.annotation_permission);
}
#[test]
fn writer_combined_pipeline_emits_form_security_and_image_markers() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
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
            Rect { x: 32, y: 720, width: 240, height: 22 },
            "alice@example.com",
        );
        page.add_checkbox("newsletter", Rect { x: 32, y: 688, width: 14, height: 14 }, false);
        page.draw_image(&[0x7F], Rect { x: 16, y: 16, width: 2, height: 1 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/AcroForm"));
    assert!(text.contains("/Annots ["));
    assert!(text.contains("/Subtype /Widget"));
    assert!(text.contains("/T (email)"));
    assert!(text.contains("/T (newsletter)"));
    assert!(text.contains("% RW-NOTE: PDF encryption requested"));
    assert!(text.contains("NOT encrypted"));
    assert!(!text.contains("combo-user"));
    assert!(!text.contains("combo-owner"));
    assert!(text.contains("% rw-image-route:raw-truncate-pad"));
}
#[test]
fn reader_roundtrip_preserves_security_and_image_route_markers() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 300, height: 200 });
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
        page.draw_image(&[0x11, 0x22, 0x33], Rect { x: 2, y: 2, width: 2, height: 2 });
        page.draw_text("ok", 10.0, 10.0, 10.0, Color { r: 0, g: 0, b: 0, a: 255 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let reader = PdfReader::new();
    let mut loaded = reader.load_from_bytes(&bytes).expect("load bytes");
    let security = loaded.security();
    // Intent marker round-trips permissions; passwords stay out of the file.
    assert_eq!(security.user_password, None);
    assert_eq!(security.owner_password, None);
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
    let mut doc = writer.create_document(Size { width: 100, height: 100 });
    {
        let page = doc.get_page(0).expect("page exists");
        page.draw_image(&[0x01, 0x02, 0x03], Rect { x: 0, y: 0, width: 2, height: 2 });
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
    let mut doc = writer.create_document(Size { width: 100, height: 100 });
    {
        let page = doc.get_page(0).expect("page exists");
        page.draw_image(&[0x0A, 0x14, 0x1E, 0xFF], Rect { x: 0, y: 0, width: 1, height: 1 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("% rw-image-route:exact-rgba-drop-alpha"));
    assert!(text.contains("0A141E>"));
}
