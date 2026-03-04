//! rust_widgets - cross-platform native GUI architecture in pure Rust.

/// Action/command system.
pub mod action;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// C ABI bindings for desktop runtime.
pub mod bindings;
/// Clipboard helpers.
pub mod clipboard;
/// Control backend abstraction for native/custom control implementations.
pub mod control_backend;
/// Core types and shared contracts.
pub mod core;
/// Event types and dispatch helpers.
pub mod event;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// Internationalization module for desktop runtime.
pub mod i18n;
/// Layout managers.
pub mod layout;
/// Object tree and object utilities.
pub mod object;
/// Platform abstraction and backend adapters.
pub mod platform;
/// Rendering traits and primitives.
pub mod render;
/// Runtime render-engine abstraction.
pub mod render_engine;
/// Signal-slot utilities.
pub mod signal;
/// Style system primitives.
pub mod style;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// Theme management for desktop runtime.
pub mod theme;
#[cfg(feature = "gpu-wgpu")]
/// Optional WGPU GPU acceleration backend.
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
/// XML utilities for desktop runtime.
pub mod xml;

#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;

#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;

#[cfg(feature = "chart")]
/// Charting primitives.
pub mod chart;

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
        eprintln!(
            "[rust_widgets.runtime] stage={stage} profile={} backend={} route={}",
            runtime_profile_name(),
            platform::get_platform().backend_name(),
            runtime_route_name()
        );
    }
}

#[cfg(not(feature = "embedded"))]
fn runtime_profile_name() -> &'static str {
    "full"
}

#[cfg(feature = "embedded")]
fn runtime_profile_name() -> &'static str {
    "embedded"
}

#[cfg(not(feature = "embedded"))]
fn runtime_route_name() -> &'static str {
    "native-platform"
}

#[cfg(feature = "embedded")]
fn runtime_route_name() -> &'static str {
    "embedded-render-engine"
}

#[cfg(not(feature = "embedded"))]
fn init_runtime_backend() {
    platform::init();
}

#[cfg(feature = "embedded")]
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

#[cfg(all(not(feature = "embedded"), feature = "desktop-runtime"))]
fn init_i18n_runtime() {
    i18n::init();
}

#[cfg(any(feature = "embedded", not(feature = "desktop-runtime")))]
fn init_i18n_runtime() {}

// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting the platform instance

/// Create a top-level window with the specified title and geometry.
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

/// Create a button control as a child of the specified parent.
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

/// Create a checkbox control as a child of the specified parent.
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

/// Create a line edit control as a child of the specified parent.
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

/// Create a label control as a child of the specified parent.
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

/// Create a radio button control as a child of the specified parent.
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

/// Create a slider control as a child of the specified parent.
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

/// Create a progress bar control as a child of the specified parent.
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

/// Create a combo box control as a child of the specified parent.
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

/// Create a list box control as a child of the specified parent.
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

/// Create a panel control as a child of the specified parent.
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

/// Create a message box dialog as a child of the specified parent.
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

/// Create a file dialog as a child of the specified parent.
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

/// Create a color dialog as a child of the specified parent.
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

/// Create a font dialog as a child of the specified parent.
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

/// Create a spin box control as a child of the specified parent.
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

/// Create a list view control as a child of the specified parent.
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

/// Create a scroll area control as a child of the specified parent.
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

/// Set the geometry of a widget.
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

/// Set the text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_text()`.
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    platform::get_platform().set_widget_text(widget_id, text);
}

/// Get the text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().get_widget_text()`.
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_text(widget_id)
}

/// Set the enabled state of a widget.
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

/// Set the visibility of a widget.
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
