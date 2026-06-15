//! Native AppKit FFI wrappers for the macOS objc2 backend (BLUE11 R1.5/R2.3 100%).
//!
//! This module is gated behind `#[cfg(target_os = "macos")]` and the
//! `objc2-macos` feature flag.

#![cfg(target_os = "macos")]
#![cfg(feature = "objc2-macos")]
// Functions are wired from platform_impl.rs for production use.
// Allow dead_code since they're called via conditional compilation paths.
#![allow(dead_code)]
// objc2 init methods may require unsafe blocks depending on platform
#![allow(unused_unsafe)]
// Imports used inside dead_code-allowed functions
#![allow(unused_imports)]

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSBackingStoreType, NSBorderType, NSButton, NSButtonType, NSMenu, NSMenuItem, NSPopUpButton,
    NSProgressIndicator, NSScrollView, NSSlider, NSStepper, NSTableColumn, NSTableView,
    NSTextField, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Wrapper around `*mut c_void` that implements Send and Sync.
#[derive(Clone, Copy)]
struct NativePtr(*mut std::ffi::c_void);
unsafe impl Send for NativePtr {}
unsafe impl Sync for NativePtr {}

/// Thread-local storage for native widget handles.
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn store_native_view(widget_id: u64, view: *mut std::ffi::c_void) {
    NATIVE_VIEWS.lock().unwrap().insert(widget_id, NativePtr(view));
}

pub(crate) fn get_native_view(widget_id: u64) -> Option<*mut std::ffi::c_void> {
    NATIVE_VIEWS.lock().unwrap().get(&widget_id).map(|p| p.0)
}

pub(crate) fn remove_native_view(widget_id: u64) {
    NATIVE_VIEWS.lock().unwrap().remove(&widget_id);
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
    let slider = unsafe { NSSlider::initWithFrame(mtm.alloc(), rect) };
    slider
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
    let view = unsafe { NSView::initWithFrame(mtm.alloc(), rect) };
    view
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
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            None,
            &NSString::from_str(key_equivalent),
        )
    };
    item
}
