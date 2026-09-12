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

/// A `Primary` shortcut on a menu item must be registered as a real accelerator,
/// and must use the macOS spelling (`⌘S`), not the Windows one.
///
/// This is the runtime-facing half of the accelerator work: the parsing tests in
/// `types.rs` prove the chord reduces to the right key/mask, and this proves the
/// backend records it against the item so `menu_item_shortcut` can report it.
#[test]
fn macos_menu_item_registers_primary_accelerator() {
    use crate::shortcut::{Key, Shortcut};

    let platform = MacOSPlatform::new();
    // Menu items require a menu parent; a menu requires a menu bar or menu.
    let window = platform.create_window("Window", 0, 0, 320, 240);
    let bar = platform.create_menu_bar(window, 0, 0, 320, 24);
    let menu = platform.create_menu(bar, "File", 0, 0, 80, 24);

    let chord = crate::format_shortcut(&Shortcut::primary(Key::S));
    assert_eq!(chord, "⌘S", "macOS must render PRIMARY as the Command glyph");

    let item = platform.menu_add_item(menu, "Save", Some(&chord));
    assert_ne!(item, 0, "menu item must be created");
    assert_eq!(
        platform.menu_item_shortcut(item).as_deref(),
        Some("⌘S"),
        "the registered accelerator must be reported back"
    );
}

/// An item created without a shortcut must not invent one.
#[test]
fn macos_menu_item_without_shortcut_reports_none() {
    let platform = MacOSPlatform::new();
    let window = platform.create_window("Window", 0, 0, 320, 240);
    let bar = platform.create_menu_bar(window, 0, 0, 320, 24);
    let menu = platform.create_menu(bar, "File", 0, 0, 80, 24);

    let item = platform.menu_add_item(menu, "Plain", None);
    assert_ne!(item, 0);
    assert_eq!(platform.menu_item_shortcut(item), None);
}

/// The spelled-out desktop form must bind on macOS too, so text produced by a
/// cross-platform caller is not silently unusable here.
#[test]
fn macos_menu_item_accepts_desktop_spelling() {
    let platform = MacOSPlatform::new();
    let window = platform.create_window("Window", 0, 0, 320, 240);
    let bar = platform.create_menu_bar(window, 0, 0, 320, 24);
    let menu = platform.create_menu(bar, "Edit", 0, 0, 80, 24);

    let item = platform.menu_add_item(menu, "Undo", Some("Ctrl+Z"));
    assert_ne!(item, 0);
    assert_eq!(
        platform.menu_item_shortcut(item).as_deref(),
        Some("Ctrl+Z"),
        "the desktop spelling must be accepted and reported verbatim"
    );
}

/// Runtime proof that a typed `Primary` shortcut reaches a live NSMenuItem and
/// that activating the item delivers its id to the widget layer.
///
/// This is the strongest available check short of a human key press: it builds a
/// real window and menu bar, reads the `keyEquivalent` AppKit actually stored, then
/// invokes the item's action/target pair the way AppKit does when the chord fires.
///
/// Requires an AppKit main-thread session. When the backend cannot create native
/// views (headless test runner) the test still exercises the state-only path and
/// asserts the accelerator bookkeeping, rather than passing vacuously.
#[test]
fn macos_menu_item_shortcut_is_registered_and_triggers() {
    use crate::shortcut::{Key, Shortcut};

    let platform = MacOSPlatform::new();
    Platform::init(&platform);

    let window = Platform::create_window(&platform, "Probe", 0, 0, 900, 600);
    assert_ne!(window, 0, "window must be created");
    let bar = Platform::create_menu_bar(&platform, window, 0, 0, 0, 0);
    let menu = Platform::create_menu(&platform, bar, "Edit", 0, 0, 0, 0);

    let undo_chord = crate::format_shortcut(&Shortcut::primary(Key::Z));
    let undo = Platform::menu_add_item(&platform, menu, "Undo", Some(&undo_chord));
    assert_ne!(undo, 0, "Undo item must be created");

    // The accelerator must be reported whatever path created the item.
    assert_eq!(
        Platform::menu_item_shortcut(&platform, undo).as_deref(),
        Some("⌘Z"),
        "Undo must report the Cmd+Z accelerator"
    );

    // A native handle exists only on the main thread with a real AppKit session.
    let Some(handle) = platform.get_handle(undo) else {
        println!("note: no native handle; AppKit session unavailable, state checks only");
        return;
    };
    if handle.ptr == 0 {
        println!("note: state-only handle; AppKit session unavailable, state checks only");
        return;
    }

    // Read back what AppKit actually stored on the NSMenuItem.
    let (key, mask) = read_ns_menu_item_equivalent(handle.ptr)
        .expect("a native NSMenuItem must expose its key equivalent");
    println!("NSMenuItem keyEquivalent = {key:?}, mask = {mask:#x}");
    assert_eq!(key, "z", "the key equivalent must be the letter z");
    // NSEventModifierFlagCommand == 1 << 20.
    assert_eq!(mask, 1 << 20, "the modifier mask must be Command only");

    // Invoke the action the way AppKit does on a key press.
    invoke_ns_menu_item_action(handle.ptr);
    let mut delivered = None;
    for _ in 0..200 {
        if let Some(id) = Platform::poll_menu_triggered(&platform) {
            delivered = Some(id);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(
        delivered,
        Some(undo),
        "activating the menu item must deliver exactly that item's id"
    );
}

/// Reads `keyEquivalent` / `keyEquivalentModifierMask` from a native NSMenuItem.
#[cfg(target_os = "macos")]
fn read_ns_menu_item_equivalent(ptr: usize) -> Option<(String, u64)> {
    use cocoa::base::{id, nil};
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;

    // SAFETY: `ptr` came from the platform's handle table for a widget that was
    // created in this process, and the selectors below are declared by NSMenuItem.
    unsafe {
        let item = ptr as *const Object as id;
        if item == nil {
            return None;
        }
        let key: id = msg_send![item, keyEquivalent];
        if key == nil {
            return None;
        }
        let utf8: *const std::os::raw::c_char = msg_send![key, UTF8String];
        if utf8.is_null() {
            return None;
        }
        let text = CStr::from_ptr(utf8).to_string_lossy().into_owned();
        let mask: u64 = msg_send![item, keyEquivalentModifierMask];
        Some((text, mask))
    }
}

/// Sends the NSMenuItem's action to its target, as AppKit does on a key press.
#[cfg(target_os = "macos")]
fn invoke_ns_menu_item_action(ptr: usize) {
    use cocoa::base::{id, nil};
    use objc::runtime::{Object, Sel};
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: `ptr` is a live NSMenuItem owned by this process; `performSelector:
    // withObject:` is the standard AppKit delivery path for a menu action.
    unsafe {
        let item = ptr as *const Object as id;
        let action: Sel = msg_send![item, action];
        let target: id = msg_send![item, target];
        if target == nil {
            return;
        }
        let _: () = msg_send![target, performSelector: action withObject: item];
    }
}
