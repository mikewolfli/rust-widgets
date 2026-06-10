//! rust_widgets - cross-platform native GUI architecture in pure Rust.

// BLUE11 R9.6: Unsafe code audit — unsafe is required for platform FFI
// Note: Removed `#![allow(unsafe_code)]` — default is allow, no-op.
// BLUE11 R4.7: Documentation completeness
// Note: Missing docs warnings silenced to reduce noise. Docs added for public API items.
#![warn(missing_docs)]
// BLUE11: Clippy lints enabled for quality enforcement.
// Individual allows are placed next to their specific violations.
#![cfg_attr(test, allow(clippy::all))]
// Conditional no_std for mini builds (BLUE13 Phase 3)
#![cfg_attr(feature = "mini", no_std)]
#[cfg(feature = "mini")]
extern crate alloc;

// ── BLUE13 Phase 3: Conditional no_std bridge for mini builds ──
// Vec/String/Box/BTreeMap are available via alloc under no_std.
// HashMap and Mutex are aliased for mini to avoid cfg noise in 200+ source files.
#[cfg(feature = "mini")]
pub(crate) use alloc::boxed::Box;
#[cfg(feature = "mini")]
pub(crate) use alloc::collections::BTreeMap;
#[cfg(feature = "mini")]
pub(crate) use alloc::format;
#[cfg(feature = "mini")]
pub(crate) use alloc::string::String;
#[cfg(feature = "mini")]
pub(crate) use alloc::string::ToString;
#[cfg(feature = "mini")]
pub(crate) use alloc::vec;
#[cfg(feature = "mini")]
pub(crate) use alloc::vec::Vec;
#[cfg(feature = "mini")]
pub(crate) use core::cell::Cell;
#[cfg(feature = "mini")]
pub(crate) use core::cell::RefCell;

// ── Type alias bridge: Mini uses BTreeMap (sorted, alloc-compatible)
//    instead of HashMap; RefCell instead of Mutex (single-threaded).
#[cfg(feature = "mini")]
pub(crate) type HashMap<K, V> = BTreeMap<K, V>;
#[cfg(feature = "mini")]
pub(crate) type Mutex<T> = RefCell<T>;

/// Action/command system.
pub mod action;
/// Desktop-only: Generic asset file watcher.
#[cfg(feature = "desktop")]
pub mod asset;
/// Desktop-only: C ABI bindings for desktop runtime.
#[cfg(feature = "desktop")]
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Reactive data binding system — Model → View automatic synchronization.
pub mod data_binding;
/// Embedded system optimizations and support.
#[cfg(feature = "embedded")]
pub mod embedded;
/// Unified error system (ErrorId, RwError, c_try!).
pub mod error;
/// Event types and dispatch helpers.
pub mod event;
/// Gesture recognizer system (gated behind `touch` feature).
#[cfg(feature = "touch")]
pub mod gesture;
/// Hardware-adaptive GPU management.
pub mod gpu;
/// Desktop-only: Internationalization module.
#[cfg(feature = "desktop")]
pub mod i18n;
/// Declarative JSON window engine (QML-like).
#[cfg(not(feature = "mini"))]
pub mod json;
/// Layout managers.
pub mod layout;
/// Memory management utilities.
pub mod memory;
/// Advanced widgets (gated behind `advanced-widgets` feature).
#[cfg(feature = "advanced-widgets")]
pub mod menu_config;
/// Object tree and object utilities.
pub mod object;
/// Performance monitoring and optimization.
pub mod performance;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Quality management for adaptive rendering.
pub mod quality;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Global shortcut system for keyboard shortcuts.
pub mod shortcut;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
/// Test infrastructure and utilities.
pub mod test;
/// Desktop-only: Theme management.
#[cfg(feature = "desktop")]
pub mod theme;
/// Undo/Redo framework for undoable commands and cross-widget undo/redo.
pub mod undo;
/// Generic utility modules (asset watcher, helpers, etc.).
pub mod util;
/// Web view and engine components.
pub mod web;
/// Optional WGPU GPU acceleration backend (gated behind `gpu-wgpu` feature).
#[cfg(feature = "gpu-wgpu")]
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
// Re-export all widget types for convenience
pub use widget::*;
#[cfg(not(feature = "desktop"))]
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $key.to_string()
    };
    ($key:expr, $count:expr) => {
        $key.to_string()
    };
    ($key:expr, $context:expr, $count:expr) => {
        $key.to_string()
    };
}
/// Application lifecycle wrapper and type-safe widget handles.
pub mod app;
#[cfg(feature = "chart")]
/// Charting primitives.
pub mod chart;
/// Index-based widget registry for runtime lookup.
pub mod index;
#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;
#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;
/// Initialize global platform and i18n subsystems.
pub fn init() {
    trace_runtime_route("init");
    init_runtime_backend();
    init_i18n_runtime();
}
/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    run_runtime_backend();
}
/// Request platform event loop shutdown.
pub fn quit() {
    trace_runtime_route("quit");
    quit_runtime_backend();
}
fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() == Some("1") {
        log::info!(
            "[rust_widgets.runtime] stage={} profile={} backend={} route={}",
            stage,
            runtime_profile_name(),
            platform::get_platform().backend_name(),
            runtime_route_name()
        );
    }
}
// ── Runtime profile names ──

/// Desktop: full native platform runtime.
#[cfg(feature = "desktop")]
fn runtime_profile_name() -> &'static str {
    "desktop"
}

/// Tablet: touch-first, native platform.
#[cfg(all(
    feature = "tablet",
    not(any(feature = "desktop", feature = "mobile", feature = "embedded"))
))]
fn runtime_profile_name() -> &'static str {
    "tablet"
}

/// Mobile: touch-first, mobile API.
#[cfg(all(
    feature = "mobile",
    not(any(feature = "desktop", feature = "tablet", feature = "embedded"))
))]
fn runtime_profile_name() -> &'static str {
    "mobile"
}

/// Embedded: stripped-down render-engine-only runtime.
#[cfg(all(
    feature = "embedded",
    not(any(feature = "desktop", feature = "tablet", feature = "mobile"))
))]
fn runtime_profile_name() -> &'static str {
    "embedded"
}

/// Embedded-mini: LVGL-style ultra-lightweight bare-metal runtime.
#[cfg(all(
    feature = "profile-embedded-mini",
    not(any(feature = "desktop", feature = "tablet", feature = "mobile"))
))]
fn runtime_profile_name() -> &'static str {
    "embedded-mini"
}

/// Embedded: stripped-down render-engine-only runtime.
#[cfg(all(
    feature = "embedded",
    not(any(
        feature = "desktop",
        feature = "tablet",
        feature = "mobile",
        feature = "profile-embedded-mini"
    ))
))]
fn runtime_profile_name() -> &'static str {
    "embedded"
}

/// Fallback (no device feature selected).
#[cfg(not(any(
    feature = "desktop",
    feature = "tablet",
    feature = "mobile",
    feature = "embedded",
    feature = "profile-embedded-mini"
)))]
fn runtime_profile_name() -> &'static str {
    "unknown"
}
#[cfg(not(any(feature = "embedded", feature = "profile-embedded-mini")))]
fn runtime_route_name() -> &'static str {
    "native-platform"
}
#[cfg(any(feature = "embedded", feature = "profile-embedded-mini"))]
fn runtime_route_name() -> &'static str {
    "embedded-render-engine"
}
#[cfg(not(any(feature = "embedded", feature = "profile-embedded-mini")))]
fn init_runtime_backend() {
    platform::init();
}
#[cfg(any(feature = "embedded", feature = "profile-embedded-mini"))]
fn init_runtime_backend() {
    render_engine::default_render_engine().init();
}
#[cfg(not(feature = "embedded"))]
fn run_runtime_backend() {
    platform::run();
}
#[cfg(feature = "embedded")]
fn run_runtime_backend() {
    render_engine::default_render_engine().run();
}
#[cfg(not(feature = "embedded"))]
fn quit_runtime_backend() {
    platform::quit();
}
#[cfg(feature = "embedded")]
fn quit_runtime_backend() {
    render_engine::default_render_engine().quit();
}
/// Desktop: initialize i18n system.
#[cfg(feature = "desktop")]
fn init_i18n_runtime() {
    i18n::init();
}

/// Tablet/mobile: i18n not loaded; log debug message.
#[cfg(all(not(feature = "desktop"), any(feature = "tablet", feature = "mobile")))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped — i18n module not loaded on this device profile");
}

/// Embedded: stripped-down, no i18n.
#[cfg(all(
    feature = "embedded",
    not(any(feature = "desktop", feature = "tablet", feature = "mobile"))
))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped in embedded mode — no i18n module loaded");
}

/// Fallback: no device feature selected.
#[cfg(not(any(
    feature = "desktop",
    feature = "tablet",
    feature = "mobile",
    feature = "embedded"
)))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped — unknown device profile, no i18n module loaded");
}
// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting a platform instance
/// Create a top-level window with specified title and geometry.
///
/// This is a convenience wrapper around `platform::get_platform().create_window()`.
///
/// # Example
/// ```
/// let window_id = rust_widgets::create_window("My App", 100, 100, 800, 600);
/// ```
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_window(title, x, y, width, height)
}
/// Create a button control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_button()`.
pub fn create_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_button(parent, text, x, y, width, height)
}
/// Create a checkbox control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_checkbox()`.
pub fn create_checkbox(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_checkbox(parent, text, x, y, width, height)
}
/// Create a line edit control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_line_edit()`.
pub fn create_line_edit(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_line_edit(parent, text, x, y, width, height)
}
/// Create a label control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_label()`.
pub fn create_label(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_label(parent, text, x, y, width, height)
}
/// Create a radio button control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_radio_button()`.
pub fn create_radio_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_radio_button(parent, text, x, y, width, height)
}
/// Create a slider control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_slider()`.
pub fn create_slider(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_slider(parent, x, y, width, height)
}
/// Create a progress bar control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_progress_bar()`.
pub fn create_progress_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_progress_bar(parent, x, y, width, height)
}
/// Create a combo box control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_combo_box()`.
pub fn create_combo_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_combo_box(parent, x, y, width, height)
}
/// Create a list box control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_list_box()`.
pub fn create_list_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_list_box(parent, x, y, width, height)
}
/// Create a panel control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_panel()`.
pub fn create_panel(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_panel(parent, x, y, width, height)
}
/// Create a message box dialog as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_message_box()`.
pub fn create_message_box(
    parent: crate::core::ObjectId,
    title: &str,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_message_box(parent, title, text, x, y, width, height)
}
/// Create a file dialog as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_file_dialog()`.
pub fn create_file_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_file_dialog(parent, x, y, width, height)
}
/// Create a color dialog as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_color_dialog()`.
pub fn create_color_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_color_dialog(parent, x, y, width, height)
}
/// Create a font dialog as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_font_dialog()`.
pub fn create_font_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_font_dialog(parent, x, y, width, height)
}
/// Create a spin box control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_spin_box()`.
pub fn create_spin_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_spin_box(parent, x, y, width, height)
}
/// Create a list view control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_list_view()`.
pub fn create_list_view(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_list_view(parent, x, y, width, height)
}
/// Create a scroll area control as a child of specified parent.
///
/// This is a convenience wrapper around `platform::get_platform().create_scroll_area()`.
pub fn create_scroll_area(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_scroll_area(parent, x, y, width, height)
}
/// Show a widget by its object id.
///
/// This is a convenience wrapper around `platform::get_platform().show_widget()`.
pub fn show_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().show_widget(widget_id);
}
/// Hide a widget by its object id.
///
/// This is a convenience wrapper around `platform::get_platform().hide_widget()`.
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().hide_widget(widget_id);
}
/// Set geometry of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_geometry()`.
pub fn set_widget_geometry(
    widget_id: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    platform::get_platform().set_widget_geometry(widget_id, x, y, width, height);
}
/// Set text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_text()`.
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    platform::get_platform().set_widget_text(widget_id, text);
}
/// Get text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().get_widget_text()`.
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_enabled()`.
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    platform::get_platform().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_enabled()`.
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_visible()`.
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    platform::get_platform().set_widget_visible(widget_id, visible);
}
/// Check if a widget is visible.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_visible()`.
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_visible(widget_id)
}
// ComboBox operations
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().combo_box_add_item(combo_box, text)
}
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    platform::get_platform().combo_box_clear_items(combo_box)
}
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().combo_box_set_current_index(combo_box, index)
}
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().combo_box_current_index(combo_box)
}
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    platform::get_platform().combo_box_item_count(combo_box)
}
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().combo_box_item_text(combo_box, index)
}
// ListBox operations
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().list_box_add_item(list_box, text)
}
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_remove_item(list_box, index)
}
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    platform::get_platform().list_box_clear_items(list_box)
}
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_set_current_index(list_box, index)
}
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().list_box_current_index(list_box)
}
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    platform::get_platform().list_box_item_count(list_box)
}
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().list_box_item_text(list_box, index)
}
// Event polling
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_widget_triggered()
}
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    platform::get_platform().poll_widget_trigger_event()
}
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    platform::get_platform().inject_widget_trigger_event(widget_id, kind)
}
// Clipboard
pub fn set_clipboard_text(text: &str) -> bool {
    platform::get_platform().set_clipboard_text(text)
}
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
// Menu operations
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_menu_bar(parent, x, y, width, height)
}
pub fn create_menu(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_menu(parent, text, x, y, width, height)
}
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    platform::get_platform().attach_menu_bar_to_window(window, menu_bar)
}
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    platform::get_platform().menu_add_item(parent_menu, text, shortcut)
}
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_menu_triggered()
}
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    platform::get_platform().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_tool_bar(parent, x, y, width, height)
}
pub fn create_status_bar(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_status_bar(parent, text, x, y, width, height)
}
// Drag and Drop
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
pub fn poll_drop_event() -> Option<DropEvent> {
    platform::get_platform().poll_drop_event()
}
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    platform::get_platform().set_widget_ime_enabled(widget_id, enabled)
}
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    platform::get_platform().set_widget_accessibility_name(widget_id, name)
}
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_accessibility_name(widget_id)
}
// Re-exports from platform module for convenience
pub use platform::{
    capabilities, dpi_scale_factor, get_platform, init as platform_init, quit as platform_quit,
    run as platform_run, runtime_gui_mode, runtime_gui_mode_for, CapabilityContract,
    DesktopBackend, DropEvent, EmbeddedCapabilityContract, MobileBackend, NativeCapabilityContract,
    PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent, WidgetTriggerKind,
};
