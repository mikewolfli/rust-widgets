//! macOS platform tests.

#![allow(deprecated)] // Cocoa 0.24 fallback; remove when objc2 backend fully replaces cocoa

use crate::core::ObjectId;
use crate::platform::macos::{HandleKind, MacOSPlatform};
use crate::platform::{DropEvent, Platform};
fn insert_dummy_widget(platform: &MacOSPlatform) -> ObjectId {
    platform.state.create_widget(HandleKind::Button, "dummy", 0, 0, 10, 10)
}
#[test]
fn macos_backend_ime_and_accessibility_state_roundtrip() {
    let platform = MacOSPlatform::new();
    let widget_id = insert_dummy_widget(&platform);
    assert!(Platform::set_widget_ime_enabled(&platform, widget_id, true));
    assert!(Platform::is_widget_ime_enabled(&platform, widget_id));
    assert!(Platform::set_widget_accessibility_name(&platform, widget_id, "Accessible"));
    assert_eq!(
        Platform::get_widget_accessibility_name(&platform, widget_id),
        "Accessible".to_string()
    );
}
#[test]
fn macos_backend_clipboard_and_drag_drop_roundtrip() {
    let platform = MacOSPlatform::new();
    let widget_id = insert_dummy_widget(&platform);
    assert!(Platform::set_clipboard_text(&platform, "hello"));
    assert_eq!(Platform::get_clipboard_text(&platform), "hello".to_string());
    assert!(Platform::begin_drag(&platform, widget_id, "text/plain", b"abc"));
    let event = Platform::poll_drop_event(&platform).expect("drop event should exist");
    assert_eq!(event.source_widget_id, widget_id);
    assert_eq!(event.mime, "text/plain");
    assert_eq!(event.payload, b"abc".to_vec());
    let injected = DropEvent {
        source_widget_id: widget_id,
        target_widget_id: widget_id,
        mime: "application/octet-stream".to_string(),
        payload: vec![1, 2, 3],
    };
    assert!(Platform::inject_drop_event(&platform, injected.clone()));
    assert_eq!(Platform::poll_drop_event(&platform), Some(injected));
}

// ---- Cocoa-legacy dialog routing ----
//
// Dialog creation routes to a real AppKit object (`NSAlert` / `NSOpenPanel` /
// `NSColorPanel` / `NSFontPanel`) only on the AppKit main thread. Rust unit
// tests run on background threads, so these tests verify the deterministic
// state-backed fallback still registers the correct semantic handle kind and
// never touches the window server off-main.

#[test]
fn cocoa_legacy_dialog_routing_message_box_kind_preserved() {
    let platform = MacOSPlatform::new();
    let msg_box =
        Platform::create_message_box(&platform, 0, "Confirm", "Save changes?", 0, 0, 300, 120);
    assert!(msg_box > 0, "MessageBox should be created");
    let handle = platform.get_handle(msg_box).expect("MessageBox handle should be registered");
    assert_eq!(handle.kind, HandleKind::MessageBox);
    // Off-main (unit-test) path: state-backed fallback, no native object.
    assert_eq!(handle.ptr, 0, "no native NSAlert may be created off the main thread");
    assert_eq!(Platform::get_widget_text(&platform, msg_box), "Save changes?");
}

#[test]
fn cocoa_legacy_dialog_routing_kinds_preserved() {
    let platform = MacOSPlatform::new();
    let file_dlg = Platform::create_file_dialog(&platform, 0, 0, 0, 400, 300);
    let color_dlg = Platform::create_color_dialog(&platform, 0, 0, 0, 300, 200);
    let font_dlg = Platform::create_font_dialog(&platform, 0, 0, 0, 300, 200);
    assert!(file_dlg > 0 && color_dlg > 0 && font_dlg > 0);
    let file_handle = platform.get_handle(file_dlg).expect("FileDialog handle");
    let color_handle = platform.get_handle(color_dlg).expect("ColorDialog handle");
    let font_handle = platform.get_handle(font_dlg).expect("FontDialog handle");
    assert_eq!(file_handle.kind, HandleKind::FileDialog);
    assert_eq!(color_handle.kind, HandleKind::ColorDialog);
    assert_eq!(font_handle.kind, HandleKind::FontDialog);
    assert_eq!(file_handle.ptr, 0);
    assert_eq!(color_handle.ptr, 0);
    assert_eq!(font_handle.ptr, 0);
}
