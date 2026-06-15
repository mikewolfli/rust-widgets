//! Widget creation helpers for the macOS objc2 backend.
//!
//! Provides high-level orchestration functions that coordinate between
//! `BackendState` (logical state) and `native.rs` (AppKit native views).
//! These are called from `platform_impl.rs` methods.

#![cfg(target_os = "macos")]
#![cfg(feature = "objc2-macos")]

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSButton, NSMenu, NSMenuItem, NSPopUpButton, NSProgressIndicator, NSScrollView,
};
use objc2_app_kit::{NSSlider, NSTextField, NSView, NSWindow};

use super::native;

/// Orchestrate full window creation: native NSWindow + state tracking.
pub(crate) fn create_window(
    mtm: MainThreadMarker,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSWindow> {
    native::create_ns_window(mtm, title, x, y, width, height)
}

/// Orchestrate button creation.
pub(crate) fn create_button(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    native::create_ns_button(mtm, text, x, y, width, height)
}

/// Orchestrate checkbox creation.
pub(crate) fn create_checkbox(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    native::create_ns_checkbox(mtm, text, x, y, width, height)
}

/// Orchestrate radio button creation.
pub(crate) fn create_radio(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSButton> {
    native::create_ns_radio(mtm, text, x, y, width, height)
}

/// Orchestrate label creation.
pub(crate) fn create_label(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSTextField> {
    native::create_ns_label(mtm, text, x, y, width, height)
}

/// Orchestrate editable text field creation.
pub(crate) fn create_textfield(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSTextField> {
    native::create_ns_textfield(mtm, text, x, y, width, height)
}

/// Orchestrate slider creation.
pub(crate) fn create_slider(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSSlider> {
    native::create_ns_slider(mtm, x, y, width, height)
}

/// Orchestrate progress bar creation.
pub(crate) fn create_progress(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSProgressIndicator> {
    native::create_ns_progress(mtm, x, y, width, height)
}

/// Orchestrate combo box creation (backed by NSPopUpButton).
pub(crate) fn create_combo_box(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSPopUpButton> {
    native::create_ns_combo_box(mtm, "", x, y, width, height)
}

/// Orchestrate list box creation (NSTableView inside NSScrollView).
pub(crate) fn create_list_box(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSScrollView> {
    native::create_ns_list_box(mtm, x, y, width, height)
}

/// Orchestrate panel creation.
pub(crate) fn create_panel(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<NSView> {
    native::create_ns_panel(mtm, x, y, width, height)
}

/// Orchestrate menu creation.
pub(crate) fn create_menu(mtm: MainThreadMarker, title: &str) -> Retained<NSMenu> {
    native::create_ns_menu(mtm, title)
}

/// Orchestrate menu item creation.
pub(crate) fn create_menu_item(
    mtm: MainThreadMarker,
    title: &str,
    key_equivalent: &str,
) -> Retained<NSMenuItem> {
    native::create_ns_menu_item(mtm, title, key_equivalent)
}
