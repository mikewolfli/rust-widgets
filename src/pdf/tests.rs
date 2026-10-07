// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

/// N-M-08: a rejected `reorder_pages` leaves the document's pages intact.
///
/// The old implementation drained `self.pages` into a local before validating the requested order, so
/// a duplicate index (`[0, 0]` on a two-page document) returned `false` *after* emptying the document.
/// The page count must be unchanged after a failed reorder.
#[test]
fn reorder_pages_failure_keeps_the_original_pages() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 595, height: 842 });
    doc.add_page(Size { width: 595, height: 842 });
    let before = doc.page_count();
    assert_eq!(before, 2, "the fixture starts with two pages");

    // A duplicate index is not a permutation: the call must fail and change nothing.
    assert!(!doc.reorder_pages(&[0, 0]), "a duplicate index must be rejected");
    assert_eq!(doc.page_count(), before, "a failed reorder must not lose pages");

    // An out-of-range index likewise.
    assert!(!doc.reorder_pages(&[0, 2]), "an out-of-range index must be rejected");
    assert_eq!(doc.page_count(), before, "a failed reorder must not lose pages");

    // A wrong length is rejected before any work.
    assert!(!doc.reorder_pages(&[0]), "a wrong-length order must be rejected");
    assert_eq!(doc.page_count(), before);

    // A valid permutation succeeds and still has both pages.
    assert!(doc.reorder_pages(&[1, 0]), "a valid permutation must succeed");
    assert_eq!(doc.page_count(), before);
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
        page.draw_image(
            &[0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11],
            Rect { x: 5, y: 5, width: 2, height: 2 },
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
    // Honest security marker: records intent, warns the output is NOT encrypted
    // (unless the standard handler actually encrypts it), and never echoes
    // plaintext passwords into the file.
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert!(text.contains("% RW-NOTE: PDF encryption requested"));
        assert!(text.contains("NOT encrypted"));
    }
    #[cfg(feature = "pdf-encryption")]
    {
        assert!(text.contains("% RW-NOTE: PDF encryption applied"));
        assert!(!text.contains("NOT encrypted"));
        assert!(text.contains("/Filter /Standard"));
        assert!(text.contains("/Encrypt "));
        // Permission flags are encoded in the dictionary's `/P` value (signed), so
        // locate it directly rather than scanning for the flag substrings.
        let permissions = parse_applied_permission_value(&text);
        assert_eq!(permissions & (1 << 2), 0, "print denied -> bit 3 clear");
        assert_ne!(permissions & (1 << 3), 0, "edit allowed -> bit 4 set");
        assert_eq!(permissions & (1 << 4), 0, "copy denied -> bit 5 clear");
        assert_eq!(permissions & (1 << 5), 0, "annotate denied -> bit 6 clear");
    }
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert!(text.contains("print=false"));
        assert!(text.contains("edit=true"));
        assert!(text.contains("copy=false"));
        assert!(text.contains("annot=false"));
    }
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
    // Passwords cannot round-trip: the writer never embeds them. Under the
    // encryption feature the reader reports a password *requirement* marker for
    // an encrypted file, so the recovered permissions come from the applied
    // dictionary rather than the intent marker.
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert_eq!(security.user_password, None);
        assert_eq!(security.owner_password, None);
    }
    #[cfg(feature = "pdf-encryption")]
    {
        assert!(security.user_password.is_some());
        assert_eq!(security.owner_password, None);
    }
    // Permissions round-trip from the intent marker / applied dictionary.
    assert!(!security.print_permission);
    assert!(!security.edit_permission);
    assert!(security.copy_permission);
    assert!(!security.annotation_permission);
}
/// The combined pipeline keeps image/route markers in the plaintext content stream
/// only when the document is not encrypted; under encryption they live inside the
/// ciphertext and must not leak into the file.
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
        page.draw_image(&[0x7F; 6], Rect { x: 16, y: 16, width: 2, height: 1 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/AcroForm"));
    assert!(text.contains("/Annots ["));
    assert!(text.contains("/Subtype /Widget"));
    assert!(text.contains("/T (email)"));
    assert!(text.contains("/T (newsletter)"));
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert!(text.contains("% RW-NOTE: PDF encryption requested"));
        assert!(text.contains("NOT encrypted"));
    }
    #[cfg(feature = "pdf-encryption")]
    {
        assert!(text.contains("% RW-NOTE: PDF encryption applied"));
        assert!(!text.contains("NOT encrypted"));
        assert!(text.contains("/Filter /Standard"));
    }
    assert!(!text.contains("combo-user"));
    assert!(!text.contains("combo-owner"));
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert!(text.contains("% rw-image-route:exact-rgb"));
    }
    #[cfg(feature = "pdf-encryption")]
    {
        // The image operators were encrypted along with the rest of the stream.
        assert!(!text.contains("% rw-image-route:exact-rgb"));
    }
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
        page.draw_image(
            &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC],
            Rect { x: 2, y: 2, width: 2, height: 2 },
        );
        page.draw_text("ok", 10.0, 10.0, 10.0, Color { r: 0, g: 0, b: 0, a: 255 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let reader = PdfReader::new();
    let mut loaded = reader.load_from_bytes(&bytes).expect("load bytes");
    let security = loaded.security();
    // Intent marker round-trips permissions; passwords stay out of the file
    // (the encryption feature reports only a requirement marker, never a secret).
    #[cfg(not(feature = "pdf-encryption"))]
    {
        assert_eq!(security.user_password, None);
        assert_eq!(security.owner_password, None);
    }
    #[cfg(feature = "pdf-encryption")]
    {
        assert!(security.user_password.is_some());
        assert_eq!(security.owner_password, None);
    }
    assert!(security.print_permission);
    assert!(!security.edit_permission);
    assert!(!security.copy_permission);
    assert!(!security.annotation_permission);
    let page = loaded.get_page(0).expect("loaded page exists");
    let content_bytes = page.content();
    // The image route markers and text operators live in the plaintext content
    // stream only when the document is not encrypted.
    #[cfg(not(feature = "pdf-encryption"))]
    {
        let content = String::from_utf8_lossy(&content_bytes);
        assert!(content.contains("% rw-image-route:exact-rgb"));
        assert!(content.contains("% rw-image-source-len:12"));
        assert!(content.contains("% rw-image-expected-rgb-len:12"));
        assert!(content.contains("BT /F1"));
    }
    #[cfg(feature = "pdf-encryption")]
    {
        let content_text = String::from_utf8_lossy(&content_bytes);
        assert!(content_text.starts_with('<'), "encrypted content is a hex string");
        assert!(!content_text.contains("BT /F1"));
    }
}
#[test]
fn writer_image_with_mismatched_payload_is_rejected() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 100, height: 100 });
    {
        let page = doc.get_page(0).expect("page exists");
        page.draw_image(&[0x01], Rect { x: 0, y: 0, width: 2, height: 1 });
    }
    let bytes = doc.to_bytes().expect("serialize document");
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("% rw-image-route:"));
    assert!(!text.contains("% rw-image-source-len:1"));
    assert!(!text.contains("% rw-image-expected-rgb-len:6"));
    assert!(!text.contains("BI\n"));
    assert!(!text.contains("010000000000>"));
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

// ── Document-level standard encryption (pdf-encryption feature) ──

/// A non-default security profile under `pdf-encryption` must produce a real
/// `/Encrypt` dictionary object, a `/Crypt` filter, a trailer `/Encrypt N 0 R`
/// reference, and an honest "applied" marker that never says "NOT encrypted".
#[cfg(feature = "pdf-encryption")]
#[test]
fn writer_emits_encrypt_dictionary_and_trailer_reference() {
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
    let bytes = doc.to_bytes().expect("serialize encrypted document");
    let text = String::from_utf8_lossy(&bytes);

    // The standard security handler and its crypt filter are present.
    assert!(text.contains("/Filter /Standard"));
    assert!(text.contains("/V 5"));
    assert!(text.contains("/R 6"));
    assert!(text.contains("/Length 16"));
    assert!(text.contains("/StmF /StdCF"));
    assert!(text.contains("/StrF /StdCF"));
    assert!(text.contains("/CFM /AESV3"));
    assert!(text.contains("/Type /CryptFilter"));

    // The trailer and catalog both reference the encryption dictionary by number.
    let encrypt_id = parse_encrypt_object_id(&text).expect("trailer must reference /Encrypt");
    assert!(text.contains(&format!("{encrypt_id} 0 obj")));
    let trailer_start = text.find("trailer\n").expect("a trailer exists");
    assert!(text[trailer_start..].contains(&format!("/Encrypt {encrypt_id} 0 R")));
    let catalog_start = text.find("1 0 obj").expect("catalog object");
    let catalog_end = text[catalog_start..].find("endobj").expect("catalog end") + catalog_start;
    assert!(text[catalog_start..catalog_end].contains(&format!("/Encrypt {encrypt_id} 0 R")));

    // Honest marker: encryption applied, never the "NOT encrypted" claim.
    assert!(text.contains("% RW-NOTE: PDF encryption applied"));
    assert!(!text.contains("NOT encrypted"));

    // Passwords are never echoed in plain text.
    assert!(!text.contains("user-secret"));
    assert!(!text.contains("owner-secret"));
}

/// The page content stream is encrypted, so its plaintext operators no longer
/// appear and the stream body is a `<...>` hex string of whole AES blocks.
#[cfg(feature = "pdf-encryption")]
#[test]
fn writer_encrypts_page_content_stream() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 300, height: 200 });
    doc.set_security(PdfSecurity {
        user_password: Some("pw".to_string()),
        owner_password: None,
        print_permission: true,
        edit_permission: true,
        copy_permission: true,
        annotation_permission: true,
    });
    {
        let page = doc.get_page(0).expect("page exists");
        page.draw_text("secret text", 10.0, 10.0, 12.0, Color { r: 0, g: 0, b: 0, a: 255 });
    }
    let bytes = doc.to_bytes().expect("serialize encrypted document");
    let text = String::from_utf8_lossy(&bytes);

    // The plaintext operator string must not survive into the ciphertext.
    assert!(!text.contains("secret text"));
    // The content object's stream body is a single AESV3 hex payload.
    let stream_start = text.find("stream\n").expect("a content stream exists") + "stream\n".len();
    let rest = &text[stream_start..];
    let body = &rest[..rest.find("\nendstream").expect("stream terminator")];
    assert!(body.starts_with('<') && body.ends_with('>'), "encrypted stream is a hex string");
    let hex = &body[1..body.len() - 1];
    assert!(hex.chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(hex.len() % 32, 0, "IV + ciphertext must be whole AES blocks");
}

/// The reader must stop reporting an encrypted document as if it were open: it
/// detects `/Encrypt`, keeps the `/Encrypt` reference in its marker, and never
/// claims the file is decrypted.
#[cfg(feature = "pdf-encryption")]
#[test]
fn reader_reports_encrypted_document_without_claiming_decryption() {
    let writer = PdfWriter::new();
    let mut doc = writer.create_document(Size { width: 200, height: 120 });
    doc.set_security(PdfSecurity {
        user_password: Some("reader-secret".to_string()),
        owner_password: Some("owner-secret".to_string()),
        print_permission: false,
        edit_permission: true,
        copy_permission: false,
        annotation_permission: false,
    });
    let bytes = doc.to_bytes().expect("serialize encrypted document");
    let reader = PdfReader::new();
    let loaded = reader.load_from_bytes(&bytes).expect("load encrypted bytes");
    let security = loaded.security();
    // The file is flagged as encrypted, with permissions recovered from the marker.
    assert!(security.user_password.is_some(), "encrypted docs report a password requirement");
    assert!(!security.print_permission);
    assert!(security.edit_permission);
    assert!(!security.copy_permission);
    assert!(!security.annotation_permission);
    // The raw secret never appears anywhere in the file.
    let text = String::from_utf8_lossy(&bytes);
    assert!(!text.contains("reader-secret"));
    assert!(!text.contains("owner-secret"));
}

/// Extract the object number from the trailer's `/Encrypt N 0 R` entry.
#[cfg(feature = "pdf-encryption")]
fn parse_encrypt_object_id(text: &str) -> Option<u32> {
    let start = text.find("/Encrypt ")? + "/Encrypt ".len();
    let digits: String = text[start..].chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u32>().ok()
}

/// Extract the signed `/P` value an applied encryption dictionary encoded as bits.
#[cfg(feature = "pdf-encryption")]
fn parse_applied_permission_value(text: &str) -> u32 {
    let start = text.find("/P ").expect("an applied dictionary has a /P entry") + "/P ".len();
    let rest = &text[start..];
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit() || *ch == '-').collect();
    digits.parse::<i64>().expect("the /P entry is an integer") as u32
}
