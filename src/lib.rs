//! rust_widgets - cross-platform native GUI architecture in pure Rust.

// BLUE11 R9.6: Unsafe code audit — unsafe is required for platform FFI
// Note: Removed `#![allow(unsafe_code)]` — default is allow, no-op.
// BLUE11 R4.7: Documentation completeness
// Note: Missing docs warnings silenced to reduce noise. Docs added for public API items.
#![allow(missing_docs)]
// BLUE11: Clippy lints enabled for quality enforcement.
// Individual allows are placed next to their specific violations.
#![cfg_attr(test, allow(clippy::needless_pass_by_value, clippy::unwrap_used))]
// Required unconditionally — `alloc` is available in both std (re-exported)
// and no_std contexts. `core` is always available via `extern crate std` under
// std, but we need direct `alloc::` paths in `compat` for no_std builds.
extern crate alloc;

// ── BLUE13 Phase 3: Alloc bridge — unified imports for std and no_std ──
// All crate files import from `compat` instead of directly from std.
pub mod compat;

/// Action/command system.
pub mod action;
/// Desktop-only: Generic asset file watcher.
#[cfg(feature = "desktop")]
pub mod asset;
/// Audio module — format detection, decoding, encoding, sample processing, and normalization.
#[cfg(feature = "audio")]
pub mod audio;
/// Desktop-only: C ABI bindings for desktop runtime.
#[cfg(all(feature = "desktop", not(feature = "mini")))]
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
/// Internationalization module.
#[cfg(feature = "i18n")]
pub mod i18n;
/// Image module — format detection, decoding, encoding, transform, and color conversion.
/// Supports all mainstream formats: PNG, JPEG, BMP, GIF, WebP, TIFF, AVIF, ICO, PNM, QOI, Farbfeld, SVG, SVGZ.
#[cfg(feature = "image")]
pub mod image;
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
/// Video module — container format detection, frame extraction, metadata, and playback.
#[cfg(feature = "video")]
pub mod video;
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
    ($key:expr) => {{
        log::warn!("i18n tr! called but i18n not loaded (non-desktop build), key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $count:expr) => {{
        log::warn!("i18n tr! called but i18n not loaded (non-desktop build), key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $context:expr, $count:expr) => {{
        log::warn!("i18n tr! called but i18n not loaded (non-desktop build), key={}", $key);
        $key.to_string()
    }};
}
/// Application lifecycle wrapper and type-safe widget handles (not available in mini mode).
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn init() {
    trace_runtime_route("init");
    init_runtime_backend();
    init_i18n_runtime();
}
/// Stub init for mini mode (no platform runtime).
#[cfg(feature = "mini")]
pub fn init() {
    log::info!("rust_widgets: mini mode init (no platform runtime)");
}
/// Run platform main event loop.
#[cfg(not(feature = "mini"))]
pub fn run() {
    trace_runtime_route("run");
    run_runtime_backend();
}
/// Stub run for mini mode (no platform runtime).
#[cfg(feature = "mini")]
pub fn run() {
    log::info!("rust_widgets: mini mode run (no platform event loop)");
}
/// Request platform event loop shutdown.
#[cfg(not(feature = "mini"))]
pub fn quit() {
    trace_runtime_route("quit");
    quit_runtime_backend();
}
/// Stub quit for mini mode (no platform runtime).
#[cfg(feature = "mini")]
pub fn quit() {
    log::info!("rust_widgets: mini mode quit (no platform to shut down)");
}
#[cfg(not(feature = "mini"))]
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
#[cfg(all(
    not(any(feature = "embedded", feature = "profile-embedded-mini")),
    not(feature = "mini")
))]
fn init_runtime_backend() {
    platform::init();
}
#[cfg(any(feature = "embedded", feature = "profile-embedded-mini"))]
fn init_runtime_backend() {
    render_engine::default_render_engine().init();
}
#[cfg(all(not(feature = "embedded"), not(feature = "mini")))]
fn run_runtime_backend() {
    platform::run();
}
#[cfg(feature = "embedded")]
fn run_runtime_backend() {
    render_engine::default_render_engine().run();
}
#[cfg(all(not(feature = "embedded"), not(feature = "mini")))]
fn quit_runtime_backend() {
    platform::quit();
}
#[cfg(feature = "embedded")]
fn quit_runtime_backend() {
    render_engine::default_render_engine().quit();
}
/// Initialize i18n system when i18n feature is enabled.
#[cfg(feature = "i18n")]
fn init_i18n_runtime() {
    i18n::init();
}

/// Tablet/mobile without i18n: log debug message.
#[cfg(all(not(feature = "i18n"), any(feature = "tablet", feature = "mobile")))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped — i18n module not loaded on this device profile");
}

/// Embedded: stripped-down, no i18n.
#[cfg(all(feature = "embedded", not(feature = "i18n")))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped in embedded mode — no i18n module loaded");
}

/// Fallback: no i18n feature selected.
#[cfg(not(any(feature = "i18n", feature = "embedded")))]
fn init_i18n_runtime() {
    log::debug!("i18n init skipped — unknown device profile, no i18n module loaded");
}
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_window(title, x, y, width, height)
}
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn show_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().show_widget(widget_id);
}
/// Hide a widget by its object id.
///
/// This is a convenience wrapper around `platform::get_platform().hide_widget()`.
#[cfg(not(feature = "mini"))]
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().hide_widget(widget_id);
}
/// Set geometry of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_geometry()`.
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    platform::get_platform().set_widget_text(widget_id, text);
}
/// Get text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().get_widget_text()`.
#[cfg(not(feature = "mini"))]
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_enabled()`.
#[cfg(not(feature = "mini"))]
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    platform::get_platform().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_enabled()`.
#[cfg(not(feature = "mini"))]
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_visible()`.
#[cfg(not(feature = "mini"))]
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    platform::get_platform().set_widget_visible(widget_id, visible);
}
/// Check if a widget is visible.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_visible()`.
#[cfg(not(feature = "mini"))]
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_visible(widget_id)
}
// ComboBox operations
#[cfg(not(feature = "mini"))]
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().combo_box_add_item(combo_box, text)
}
#[cfg(not(feature = "mini"))]
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    platform::get_platform().combo_box_clear_items(combo_box)
}
#[cfg(not(feature = "mini"))]
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().combo_box_set_current_index(combo_box, index)
}
#[cfg(not(feature = "mini"))]
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().combo_box_current_index(combo_box)
}
#[cfg(not(feature = "mini"))]
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    platform::get_platform().combo_box_item_count(combo_box)
}
#[cfg(not(feature = "mini"))]
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().combo_box_item_text(combo_box, index)
}
// ListBox operations
#[cfg(not(feature = "mini"))]
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().list_box_add_item(list_box, text)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_remove_item(list_box, index)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    platform::get_platform().list_box_clear_items(list_box)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_set_current_index(list_box, index)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().list_box_current_index(list_box)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    platform::get_platform().list_box_item_count(list_box)
}
#[cfg(not(feature = "mini"))]
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().list_box_item_text(list_box, index)
}
// Event polling
#[cfg(not(feature = "mini"))]
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_widget_triggered()
}
#[cfg(not(feature = "mini"))]
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    platform::get_platform().poll_widget_trigger_event()
}
#[cfg(not(feature = "mini"))]
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    platform::get_platform().inject_widget_trigger_event(widget_id, kind)
}
// Clipboard
#[cfg(not(feature = "mini"))]
pub fn set_clipboard_text(text: &str) -> bool {
    platform::get_platform().set_clipboard_text(text)
}
#[cfg(not(feature = "mini"))]
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
#[cfg(not(feature = "mini"))]
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
// Menu operations
#[cfg(not(feature = "mini"))]
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_menu_bar(parent, x, y, width, height)
}
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    platform::get_platform().attach_menu_bar_to_window(window, menu_bar)
}
#[cfg(not(feature = "mini"))]
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    platform::get_platform().menu_add_item(parent_menu, text, shortcut)
}
#[cfg(not(feature = "mini"))]
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_menu_triggered()
}
#[cfg(not(feature = "mini"))]
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    platform::get_platform().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
#[cfg(not(feature = "mini"))]
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    platform::get_platform().create_tool_bar(parent, x, y, width, height)
}
#[cfg(not(feature = "mini"))]
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
#[cfg(not(feature = "mini"))]
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
#[cfg(not(feature = "mini"))]
pub fn poll_drop_event() -> Option<DropEvent> {
    platform::get_platform().poll_drop_event()
}
#[cfg(not(feature = "mini"))]
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
#[cfg(not(feature = "mini"))]
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    platform::get_platform().set_widget_ime_enabled(widget_id, enabled)
}
#[cfg(not(feature = "mini"))]
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
#[cfg(not(feature = "mini"))]
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
#[cfg(not(feature = "mini"))]
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    platform::get_platform().set_widget_accessibility_name(widget_id, name)
}
#[cfg(not(feature = "mini"))]
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_accessibility_name(widget_id)
}
// Re-exports from platform module for convenience
#[cfg(not(feature = "mini"))]
pub use platform::{
    capabilities, dpi_scale_factor, get_platform, init as platform_init, quit as platform_quit,
    run as platform_run, runtime_gui_mode, runtime_gui_mode_for,
};
pub use platform::{
    CapabilityContract, DesktopBackend, DropEvent, EmbeddedCapabilityContract, MobileBackend,
    NativeCapabilityContract, PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent,
    WidgetTriggerKind,
};
