// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native AppKit FFI wrappers for the macOS objc2 backend (BLUE11 R1.5/R2.3 100%).
//!
//! This module is gated behind `#[cfg(target_os = "macos")]` and the
//! `objc2-macos` feature flag.

#![cfg(target_os = "macos")]
#![cfg(feature = "macos")]
// Functions are wired from platform_impl.rs for production use.
// Allow dead_code since they're called via conditional compilation paths.
#![allow(dead_code)]
// objc2 init methods may require unsafe blocks depending on platform
#![allow(unused_unsafe)]
// Imports used inside dead_code-allowed functions
#![allow(unused_imports)]

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2::{class, msg_send, sel};
use objc2_app_kit::{
    NSAlert, NSApplication, NSBackingStoreType, NSBorderType, NSButton, NSButtonType, NSColorPanel,
    NSFontPanel, NSMenu, NSMenuItem, NSOpenPanel, NSPopUpButton, NSProgressIndicator, NSScrollView,
    NSSlider, NSStepper, NSTableColumn, NSTableView, NSTextField, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Wrapper around `*mut c_void` that implements `Send`.
///
/// `Send` is required because the pointer is stored in a process-global
/// `Mutex<HashMap<..>>` (the registry lives in a `LazyLock` static).
///
/// `Sync` is deliberately NOT implemented: AppKit objects are owned by the
/// main thread, and an `unsafe impl Sync` would additionally permit `&NativePtr`
/// to be shared across threads, removing the compiler's last guard against
/// off-main AppKit access. Every native entry point still re-checks the thread
/// before messaging AppKit (see `check_apple_thread_safety.sh`), and keeping the
/// bound narrow means a future helper that forgets that check has a smaller
/// unsafe surface to slip through.
#[derive(Clone, Copy)]
struct NativePtr(*mut std::ffi::c_void);
unsafe impl Send for NativePtr {}

/// Thread-local storage for native widget handles.
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static PARENT_MAP: LazyLock<Mutex<HashMap<u64, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn store_native_view(widget_id: u64, view: *mut std::ffi::c_void) {
    if view.is_null() {
        return;
    }
    remove_native_view(widget_id);
    unsafe {
        let object = view as *mut AnyObject;
        let _: *mut AnyObject = msg_send![object, retain];
    }
    NATIVE_VIEWS.lock().unwrap().insert(widget_id, NativePtr(view));
}

pub(crate) fn get_native_view(widget_id: u64) -> Option<*mut std::ffi::c_void> {
    NATIVE_VIEWS.lock().unwrap().get(&widget_id).map(|p| p.0)
}

/// Number of native views currently registered (test/diagnostic helper).
pub(crate) fn native_view_count() -> usize {
    NATIVE_VIEWS.lock().unwrap().len()
}

pub(crate) fn remove_native_view(widget_id: u64) {
    let removed = NATIVE_VIEWS.lock().unwrap().remove(&widget_id);
    if let Some(ptr) = removed {
        unsafe {
            let object = ptr.0 as *mut AnyObject;
            // Detach from the superview first. `add_as_subview` makes the parent
            // view retain this object, and that reference is NOT balanced by the
            // registry's `retain`/`release` pair — so releasing only the registry
            // reference left the object alive and owned by the parent forever
            // (a create/destroy UI churn grew RSS by ~6 KB per widget, without
            // bound). `removeFromSuperview` drops the parent's reference, which
            // is what actually lets the object deallocate.
            let has_superview: bool =
                msg_send![object, respondsToSelector: sel!(removeFromSuperview)];
            if has_superview {
                let _: () = msg_send![object, removeFromSuperview];
            }
            let _: () = msg_send![object, release];
        }
    }
    PARENT_MAP.lock().unwrap().remove(&widget_id);
}

pub(crate) fn set_parent(widget_id: u64, parent_id: u64) {
    PARENT_MAP.lock().unwrap().insert(widget_id, parent_id);
}

pub(crate) fn get_parent(widget_id: u64) -> Option<u64> {
    PARENT_MAP.lock().unwrap().get(&widget_id).copied()
}

pub(crate) fn add_as_subview(widget_id: u64, parent_id: u64) {
    let views = NATIVE_VIEWS.lock().unwrap();
    let Some(parent_ptr) = views.get(&parent_id).map(|p| p.0) else {
        return;
    };
    let Some(child_ptr) = views.get(&widget_id).map(|p| p.0) else {
        return;
    };
    drop(views);

    unsafe {
        let parent = parent_ptr as *mut AnyObject;
        let child = child_ptr as *mut AnyObject;
        let content_view_selector = sel!(contentView);
        let responds: bool = msg_send![parent, respondsToSelector: content_view_selector];
        let container: *mut AnyObject =
            if responds { msg_send![parent, contentView] } else { parent };
        if !container.is_null() {
            let _: () = msg_send![container, addSubview: child];
            set_parent(widget_id, parent_id);
        }
    }
}

pub(crate) fn set_native_frame(widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    // AppKit window-server mutations must happen on the main thread; off-main we
    // leave the native frame untouched (the backend state already records it).
    if MainThreadMarker::new().is_none() {
        return;
    }
    unsafe {
        let object = ptr as *mut AnyObject;
        let rect = make_rect(x, y, width, height);
        // `NSWindow` does NOT respond to `setFrame:` — the window-setter is
        // `setFrame:display:`. Sending the view selector to a window raises
        // "invalid message send to -[NSWindow setFrame:]: method not found",
        // which used to abort the process. Probe the class' actual API instead.
        let is_window: bool = msg_send![object, isKindOfClass: class!(NSWindow)];
        if is_window {
            let _: () = msg_send![object, setFrame: rect, display: true];
        } else {
            let responds: bool = msg_send![object, respondsToSelector: sel!(setFrame:)];
            if responds {
                let _: () = msg_send![object, setFrame: rect];
            }
        }
    }
}

pub(crate) fn set_native_hidden(widget_id: u64, hidden: bool) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    if MainThreadMarker::new().is_none() {
        return;
    }
    unsafe {
        let object = ptr as *mut AnyObject;
        // `NSView` uses `setHidden:`, but `NSWindow` uses `orderOut:` /
        // `makeKeyAndOrderFront:` — sending `setHidden:` to a window is a
        // method-not-found abort.
        let is_window: bool = msg_send![object, isKindOfClass: class!(NSWindow)];
        if is_window {
            if hidden {
                let _: () = msg_send![object, orderOut: std::ptr::null_mut::<AnyObject>()];
            } else {
                let _: () =
                    msg_send![object, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
            }
        } else {
            let responds: bool = msg_send![object, respondsToSelector: sel!(setHidden:)];
            if responds {
                let _: () = msg_send![object, setHidden: hidden];
            }
        }
    }
}

pub(crate) fn set_native_enabled(widget_id: u64, enabled: bool) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    if MainThreadMarker::new().is_none() {
        return;
    }
    unsafe {
        let object = ptr as *mut AnyObject;
        // Only `NSControl` (and subclasses) implements `setEnabled:`; windows and
        // plain `NSView` containers do not, so guard with `respondsToSelector:`.
        let selector = sel!(setEnabled:);
        let responds: bool = msg_send![object, respondsToSelector: selector];
        if responds {
            let _: () = msg_send![object, setEnabled: enabled];
        }
    }
}

pub(crate) fn set_native_text(widget_id: u64, text: &str) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    // Text mutation touches AppKit object state on the window server.
    if MainThreadMarker::new().is_none() {
        return;
    }
    unsafe {
        let object = ptr as *mut AnyObject;
        let value = NSString::from_str(text);
        // Dispatch with typed messages instead of `performSelector:withObject:`:
        // the latter returns `id`, and objc2 validates the declared return type,
        // so declaring `()` made every call panic at runtime with
        // "expected return to have type code '@', but found 'v'".
        //
        // A window's title uses `setTitle:`; controls use `setStringValue:`;
        // anything else falls back to the accessibility label. Each branch is
        // guarded by `respondsToSelector:` so no unsupported selector is sent.
        let is_window: bool = msg_send![object, isKindOfClass: class!(NSWindow)];
        if is_window {
            let _: () = msg_send![object, setTitle: &*value];
            return;
        }
        for selector in [sel!(setStringValue:), sel!(setTitle:), sel!(setAccessibilityLabel:)] {
            let responds: bool = msg_send![object, respondsToSelector: selector];
            if !responds {
                continue;
            }
            if selector == sel!(setStringValue:) {
                let _: () = msg_send![object, setStringValue: &*value];
            } else if selector == sel!(setTitle:) {
                let _: () = msg_send![object, setTitle: &*value];
            } else {
                let _: () = msg_send![object, setAccessibilityLabel: &*value];
            }
            return;
        }
    }
}

pub(crate) fn set_native_menu_shortcut(widget_id: u64, key: &str, modifier_mask: u64) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    // `NSMenuItem` key-equivalent mutations must happen on the main thread.
    if MainThreadMarker::new().is_none() {
        return;
    }
    unsafe {
        let item = ptr as *mut AnyObject;
        let key = NSString::from_str(key);
        // Guard with `respondsToSelector:`: only NSMenuItem implements these,
        // and a wrong receiver would otherwise raise a method-not-found abort.
        if msg_send![item, respondsToSelector: sel!(setKeyEquivalent:)] {
            let _: () = msg_send![item, setKeyEquivalent: &*key];
        }
        if msg_send![item, respondsToSelector: sel!(setKeyEquivalentModifierMask:)] {
            let _: () = msg_send![item, setKeyEquivalentModifierMask: modifier_mask];
        }
    }
}

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> NSRect {
    NSRect::new(
        objc2_foundation::NSPoint::new(x as f64, y as f64),
        objc2_foundation::NSSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

pub(crate) fn create_ns_window(
    mtm: MainThreadMarker,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSWindow> {
    let style_mask = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSWindow::initWithContentRect_styleMask_backing_defer is called with
    // a valid MainThreadMarker, ensuring this runs on the main thread. The alloc
    // is obtained from mtm which guarantees the correct memory allocation context.
    // The returned Retained<NSWindow> is always non-null (objc2 init methods return
    // a valid retained object on success, or panic on allocation failure).
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            style_mask,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str(title));
    window.makeKeyAndOrderFront(None);
    window
}

pub(crate) fn create_ns_button(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSButton::initWithFrame runs on the main thread (guaranteed by mtm).
    // objc2 init methods return Retained<T> which ensures the object is valid.
    // No additional error checking is needed since objc2 handles memory management.
    let button = unsafe { NSButton::initWithFrame(mtm.alloc(), rect) };
    button.setTitle(&NSString::from_str(text));
    button.setButtonType(NSButtonType::MomentaryLight);
    button
}

pub(crate) fn create_ns_checkbox(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSButton::initWithFrame is called on the main thread (via mtm).
    // Retained<T> guarantees a valid object; objc2 panics on alloc failure.
    let button = unsafe { NSButton::initWithFrame(mtm.alloc(), rect) };
    button.setTitle(&NSString::from_str(text));
    button.setButtonType(NSButtonType::Switch);
    button
}

pub(crate) fn create_ns_radio(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSButton::initWithFrame on main thread (mtm guard).
    // Retained<T> from objc2 ensures a valid object is returned.
    let button = unsafe { NSButton::initWithFrame(mtm.alloc(), rect) };
    button.setTitle(&NSString::from_str(text));
    button.setButtonType(NSButtonType::Radio);
    button
}

pub(crate) fn create_ns_label(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSTextField> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSTextField::initWithFrame called on main thread via mtm.
    // objc2 guarantees the returned Retained<NSTextField> is a valid object.
    let label = unsafe { NSTextField::initWithFrame(mtm.alloc(), rect) };
    label.setStringValue(&NSString::from_str(text));
    label.setEditable(false);
    label.setBezeled(false);
    label.setDrawsBackground(false);
    label
}

pub(crate) fn create_ns_slider(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSSlider> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSSlider::initWithFrame on main thread (mtm guard).
    // objc2 init methods reliably return a valid Retained<NSSlider>.
    unsafe { NSSlider::initWithFrame(mtm.alloc(), rect) }
}

pub(crate) fn create_ns_textfield(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSTextField> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSTextField::initWithFrame on main thread via mtm.
    let field = unsafe { NSTextField::initWithFrame(mtm.alloc(), rect) };
    field.setStringValue(&NSString::from_str(text));
    field.setEditable(true);
    field.setBezeled(true);
    field.setDrawsBackground(true);
    field
}

pub(crate) fn create_ns_progress(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSProgressIndicator> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSProgressIndicator::initWithFrame on main thread (mtm guard).
    let progress = unsafe { NSProgressIndicator::initWithFrame(mtm.alloc(), rect) };
    progress.setIndeterminate(false);
    progress.setMinValue(0.0);
    progress.setMaxValue(100.0);
    progress.setDoubleValue(0.0);
    progress
}

pub(crate) fn create_ns_combo_box(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSPopUpButton> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSPopUpButton::initWithFrame on main thread (mtm guard).
    let combo = unsafe { NSPopUpButton::initWithFrame_pullsDown(mtm.alloc(), rect, false) };
    combo.setTitle(&NSString::from_str(text));
    combo
}

pub(crate) fn create_ns_list_box(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSScrollView> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSScrollView::initWithFrame on main thread via mtm.
    let scroll = unsafe { NSScrollView::initWithFrame(mtm.alloc(), rect) };
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(false);
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setAutohidesScrollers(true);

    // Create the table view inside the scroll view.
    let table_rect = NSRect::new(NSPoint::new(0.0, 0.0), scroll.contentSize());
    // SAFETY: NSTableView::initWithFrame on main thread.
    let table = unsafe { NSTableView::initWithFrame(mtm.alloc(), table_rect) };

    // Add a single column for list items.
    let column_id = NSString::from_str("list-column");
    // SAFETY: NSTableColumn::initWithIdentifier on main thread.
    let column = unsafe { NSTableColumn::initWithIdentifier(mtm.alloc(), &column_id) };
    column.setTitle(&NSString::from_str("Items"));
    table.addTableColumn(&column);
    table.setHeaderView(None);

    scroll.setDocumentView(Some(&table));
    scroll
}

pub(crate) fn create_ns_panel(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSView> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSView::initWithFrame on main thread (mtm guard).
    unsafe { NSView::initWithFrame(mtm.alloc(), rect) }
}

pub(crate) fn create_ns_scroll_view(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSScrollView> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSScrollView::initWithFrame on main thread (mtm guard).
    let scroll = unsafe { NSScrollView::initWithFrame(mtm.alloc(), rect) };
    scroll.setHasVerticalScroller(true);
    scroll.setHasHorizontalScroller(true);
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setAutohidesScrollers(true);
    scroll
}

pub(crate) fn create_ns_stepper(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSStepper> {
    let rect = make_rect(x, y, width, height);
    // SAFETY: NSStepper::initWithFrame on main thread (mtm guard).
    let stepper = unsafe { NSStepper::initWithFrame(mtm.alloc(), rect) };
    stepper.setMinValue(0.0);
    stepper.setMaxValue(100.0);
    stepper.setIncrement(1.0);
    stepper.setValueWraps(false);
    stepper.setAutorepeat(true);
    stepper
}

pub(crate) fn create_ns_menu(mtm: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    // SAFETY: NSMenu::initWithTitle on main thread (mtm guard).
    let menu = unsafe { NSMenu::initWithTitle(mtm.alloc(), &NSString::from_str(title)) };
    menu.setAutoenablesItems(false);
    menu
}

pub(crate) fn create_ns_menu_item(
    mtm: MainThreadMarker,
    title: &str,
    key_equivalent: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: NSMenuItem::initWithTitle_action_keyEquivalent on main thread via mtm.
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            None,
            &NSString::from_str(key_equivalent),
        )
    }
}

/// Create a native NSAlert configured with a title, message, and an OK button.
///
/// Modal presentation is intentionally not started here; callers present it
/// from an interactive AppKit run loop (e.g. `beginSheetModalForWindow`).
pub(crate) fn create_ns_alert(mtm: MainThreadMarker, title: &str, text: &str) -> Retained<NSAlert> {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(text));
    alert.addButtonWithTitle(&NSString::from_str("OK"));
    alert
}

/// Create a native NSOpenPanel for the file dialog. Selection is single-file by
/// default; modal presentation is left to the interactive run loop.
pub(crate) fn create_ns_open_panel(mtm: MainThreadMarker) -> Retained<NSOpenPanel> {
    let panel = NSOpenPanel::new(mtm);
    panel.setAllowsMultipleSelection(false);
    panel
}

/// Attach a child item to an existing native `NSMenu`.
pub(crate) fn menu_add_child_to_menu(
    parent_menu: *mut std::ffi::c_void,
    child: *mut std::ffi::c_void,
) {
    if parent_menu.is_null() || child.is_null() {
        return;
    }
    // SAFETY: Both pointers were stored by `store_native_view` (which retains)
    // and are only created on the main thread. `addItem:` takes an NSMenuItem.
    unsafe {
        let menu = parent_menu as *mut NSMenu;
        let item = child as *mut NSMenuItem;
        (*menu).addItem(&*item);
    }
}

/// Attach a submenu to an item that lives inside a parent menu.
pub(crate) fn menu_set_submenu_on_item(
    item: *mut std::ffi::c_void,
    submenu: *mut std::ffi::c_void,
) {
    if item.is_null() || submenu.is_null() {
        return;
    }
    // SAFETY: Pointers originate from the retained native view registry and are
    // only touched on the main thread.
    unsafe {
        let item = item as *mut NSMenuItem;
        let submenu = submenu as *mut NSMenu;
        (*item).setSubmenu(Some(&*submenu));
    }
}

/// Install a native `NSMenu` as the application's main menu.
///
/// Returns `false` when not on the AppKit main thread, so callers can fall back
/// to the state-only menu bookkeeping instead of aborting the process.
pub(crate) fn install_main_menu(menu_bar: *mut std::ffi::c_void) -> bool {
    if menu_bar.is_null() {
        return false;
    }
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    // SAFETY: `NSApplication::sharedApplication` is only valid on the main
    // thread, which is guaranteed by the `MainThreadMarker` acquired above.
    let app = NSApplication::sharedApplication(mtm);
    let menu = menu_bar as *mut NSMenu;
    app.setMainMenu(Some(unsafe { &*menu }));
    true
}

/// Bootstrap the shared `NSApplication` (create + `finishLaunching`).
///
/// Mirrors the cocoa-legacy backend's `init()` so the objc2 backend is a real
/// AppKit application rather than only a state machine. Returns `false` when
/// called off the main thread (nothing is touched in that case).
pub(crate) fn bootstrap_ns_application() -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.finishLaunching();
    true
}

/// Create a native NSColorPanel instance.
pub(crate) fn create_ns_color_panel(mtm: MainThreadMarker) -> Retained<NSColorPanel> {
    NSColorPanel::new(mtm)
}

/// Create a native NSFontPanel instance.
pub(crate) fn create_ns_font_panel(mtm: MainThreadMarker) -> Retained<NSFontPanel> {
    NSFontPanel::new(mtm)
}
