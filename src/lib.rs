// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
/// C ABI and language bindings (C / Java-JNI / etc.).
///
/// Available on the `desktop` profile and on any profile that exposes an
/// FFI consumer (`jni` for Java, `mobile-api` for the mobile runtime). The
/// `mini` profile excludes it because it is `alloc`-free / no-std oriented.
#[cfg(all(any(feature = "desktop", feature = "jni", feature = "mobile-api"), not(alloc_frugal)))]
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
#[cfg(embedded_surface)]
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
///
/// **Decoding** is real for: PNG (all bit depths, scanline filters, palette), JPEG,
/// BMP, QOI, Farbfeld, PNM (P5/P6). GIF/WebP/TIFF/AVIF/ICO/SVG decode returns an
/// explicit `Err` rather than fabricated pixels until a codec lands.
#[cfg(feature = "image")]
pub mod image;
/// Declarative JSON window engine (QML-like).
#[cfg(all(any(feature = "desktop", feature = "tablet", feature = "mobile"), widgets_unstripped))]
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
#[cfg(widgets_unstripped)]
pub mod web;
/// Optional WGPU GPU acceleration backend (gated behind `gpu-wgpu` feature).
#[cfg(feature = "gpu-wgpu")]
pub mod wgpu_backend;
/// Widget definitions and widget helpers.
pub mod widget;
// Re-export all widget types for convenience
pub use widget::*;
// NOTE: there is no top-level `chart` module. The chart *engine* (layout, axes,
// ticks, SVG context, adapter) and the chart *widgets* live together under
// `crate::widget::chart_widgets`, because they are two layers of one feature.
// The engine is reachable as `rust_widgets::widget::chart_widgets::charts`
// (and `::types`/`::layout`/`::svg`/`::adapter`).
#[cfg(not(feature = "i18n"))]
#[macro_export]
macro_rules! tr {
    ($key:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
    ($key:expr, $context:expr, $count:expr) => {{
        log::warn!("i18n tr! called but the i18n feature is disabled, key={}", $key);
        $key.to_string()
    }};
}
/// Application lifecycle wrapper and type-safe widget handles (not available in mini mode).
#[cfg(all(any(feature = "desktop", feature = "tablet", feature = "mobile"), widgets_unstripped))]
pub mod app;
/// Index-based widget registry for runtime lookup.
pub mod index;
#[cfg(feature = "pdf")]
/// PDF rendering/export support.
pub mod pdf;
#[cfg(feature = "print")]
/// Print and preview support.
pub mod print;
/// Initialize global platform and i18n subsystems.
///
/// One function for every profile (BLUE15 rule #58): the branch that used to be
/// a `cfg`-gated pair now asks [`platform::profile`], so adding a profile does
/// not mean adding a copy of this function.
pub fn init() {
    trace_runtime_route("init");
    platform::profile::runtime_init();
    platform::profile::init_optional_subsystems();
}
/// Run platform main event loop.
pub fn run() {
    trace_runtime_route("run");
    platform::profile::runtime_run();
}
/// Request platform event loop shutdown.
pub fn quit() {
    trace_runtime_route("quit");
    platform::profile::runtime_quit();
}
/// Logs the resolved profile/backend/route when `RUST_WIDGETS_TRACE_RUNTIME=1`.
fn trace_runtime_route(stage: &str) {
    if std::env::var("RUST_WIDGETS_TRACE_RUNTIME").ok().as_deref() == Some("1") {
        log::info!(
            "[rust_widgets.runtime] stage={} profile={} backend={} route={}",
            stage,
            platform::profile::profile_name(),
            platform::platform_facts().backend_name(),
            platform::profile::route_name()
        );
    }
}
// Convenient wrapper functions for platform operations
// Users can call these directly without manually getting a platform instance

/// Resolve the backend that should create a widget of this kind.
///
/// # Why this exists
///
/// A widget kind may map onto a real platform primitive (a `Button` on Win32/
/// AppKit/GTK) or have no primitive at all (`Chart`, `CodeEditor`, …), in which
/// case the platform layer supplies the surface. **Which of the two happens is a
/// backend decision and must not leak to callers**: this function is the single
/// place the creation path asks for it, so the choice cannot drift between call
/// sites (see `control_backend::dispatcher`).
///
/// On a profile without an OS runtime (`mini`, `embedded`) the custom state
/// backend answers instead, so the same call works everywhere.
#[cfg(not(alloc_frugal))]
fn backend_for_kind(kind: widget::WidgetKind) -> &'static dyn control_backend::ControlBackend {
    control_backend::get_control_backend_for_widget(kind)
}

// ── Kinds that reduced profiles compile out ──
//
// `embedded` drops these `WidgetKind` variants, but the `create_*` functions that
// route on them must stay callable in **every** profile (the public API surface
// must not vary — principle #53). These aliases name the variant when it exists
// and substitute the closest always-present kind when it does not, so the call
// site stays a single unconditional expression.
//
// `Panel` is the stand-in: it exists in every profile, it is a container surface,
// and the backends that run reduced profiles implement it.

/// `WidgetKind::MenuBar` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(all(not(embedded_surface), feature = "desktop"))]
const KIND_MENU_BAR: widget::WidgetKind = widget::WidgetKind::MenuBar;
#[cfg(not(alloc_frugal))]
#[cfg(not(all(not(embedded_surface), feature = "desktop")))]
const KIND_MENU_BAR: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::Menu` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(all(not(embedded_surface), feature = "desktop"))]
const KIND_MENU: widget::WidgetKind = widget::WidgetKind::Menu;
#[cfg(not(alloc_frugal))]
#[cfg(not(all(not(embedded_surface), feature = "desktop")))]
const KIND_MENU: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::ToolBar` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(all(not(embedded_surface), feature = "desktop"))]
const KIND_TOOL_BAR: widget::WidgetKind = widget::WidgetKind::ToolBar;
#[cfg(not(alloc_frugal))]
#[cfg(not(all(not(embedded_surface), feature = "desktop")))]
const KIND_TOOL_BAR: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::StatusBar` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(all(not(embedded_surface), feature = "desktop"))]
const KIND_STATUS_BAR: widget::WidgetKind = widget::WidgetKind::StatusBar;
#[cfg(not(alloc_frugal))]
#[cfg(not(all(not(embedded_surface), feature = "desktop")))]
const KIND_STATUS_BAR: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::ListView` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(all(not(embedded_surface), feature = "desktop"))]
const KIND_LIST_VIEW: widget::WidgetKind = widget::WidgetKind::ListView;
#[cfg(not(alloc_frugal))]
#[cfg(not(all(not(embedded_surface), feature = "desktop")))]
const KIND_LIST_VIEW: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::MessageBox` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(full_widgets)]
const KIND_MESSAGE_BOX: widget::WidgetKind = widget::WidgetKind::MessageBox;
#[cfg(not(alloc_frugal))]
#[cfg(not(full_widgets))]
const KIND_MESSAGE_BOX: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::FileDialog` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(full_widgets)]
const KIND_FILE_DIALOG: widget::WidgetKind = widget::WidgetKind::FileDialog;
#[cfg(not(alloc_frugal))]
#[cfg(not(full_widgets))]
const KIND_FILE_DIALOG: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::ColorDialog` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(full_widgets)]
const KIND_COLOR_DIALOG: widget::WidgetKind = widget::WidgetKind::ColorDialog;
#[cfg(not(alloc_frugal))]
#[cfg(not(full_widgets))]
const KIND_COLOR_DIALOG: widget::WidgetKind = widget::WidgetKind::Panel;

/// `WidgetKind::FontDialog` where available, else the always-present fallback.
#[cfg(not(alloc_frugal))]
#[cfg(full_widgets)]
const KIND_FONT_DIALOG: widget::WidgetKind = widget::WidgetKind::FontDialog;
#[cfg(not(alloc_frugal))]
#[cfg(not(full_widgets))]
const KIND_FONT_DIALOG: widget::WidgetKind = widget::WidgetKind::Panel;

/// Create a top-level window with specified title and geometry.
///
/// # Example
/// ```
/// let window_id = rust_widgets::create_window("My App", 100, 100, 800, 600);
/// ```
#[cfg(not(alloc_frugal))]
pub fn create_window(
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Window).create_window(title, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a button control as a child of specified parent.
///
/// The backend decides whether this becomes a platform button or is painted by
/// the platform's custom surface; callers get a handle either way.
pub fn create_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Button).create_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a check-box. The backend decides whether it is a platform control
/// or is painted by the platform's custom surface.
pub fn create_checkbox(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::CheckBox)
        .create_checkbox(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a single-line text editor. The backend chooses how it is hosted.
pub fn create_line_edit(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::LineEdit)
        .create_line_edit(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a read-only text label. The backend chooses how it is hosted.
pub fn create_label(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Label).create_label(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a radio button. The backend chooses how it is hosted.
pub fn create_radio_button(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::RadioButton)
        .create_radio_button(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a slider. The backend chooses how it is hosted.
pub fn create_slider(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::Slider).create_slider(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a progress bar. The backend chooses how it is hosted.
pub fn create_progress_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ProgressBar)
        .create_progress_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a combo box. The backend chooses how it is hosted.
pub fn create_combo_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ComboBox).create_combo_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list box. The backend chooses how it is hosted.
pub fn create_list_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ListBox).create_list_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a panel (a container surface). The backend chooses how it is hosted.
pub fn create_panel(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    // `WidgetKind::Panel` is the closest always-available kind; on profiles where
    // a dedicated panel kind exists the routing table answers for it.
    backend_for_kind(widget::WidgetKind::Panel).create_panel(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a message box dialog as a child of specified parent.
///
/// Creates a message box. The backend chooses how it is hosted.
pub fn create_message_box(
    parent: crate::core::ObjectId,
    title: &str,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_MESSAGE_BOX).create_message_box(parent, title, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a file dialog as a child of specified parent.
///
/// Creates a file dialog. The backend chooses how it is hosted.
pub fn create_file_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_FILE_DIALOG).create_file_dialog(parent, "", x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a color dialog as a child of specified parent.
pub fn create_color_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_COLOR_DIALOG).create_color_dialog(parent, "", x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Create a font dialog as a child of specified parent.
pub fn create_font_dialog(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_FONT_DIALOG).create_font_dialog(parent, "", x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a spin box. The backend chooses how it is hosted.
pub fn create_spin_box(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::SpinBox).create_spin_box(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
/// Creates a list view. The backend chooses how it is hosted.
pub fn create_list_view(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_LIST_VIEW).create_list_view(parent, x, y, width, height)
}
/// Creates a scroll area. The backend chooses how it is hosted.
#[cfg(not(alloc_frugal))]
pub fn create_scroll_area(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(widget::WidgetKind::ScrollArea).create_scroll_area(parent, x, y, width, height)
}
/// Mounts a custom-painted widget into a window.
///
/// "Custom-painted" means the widget renders itself through
/// [`widget::Draw`] rather than mapping onto an existing OS control, so no
/// `create_*` function applies. Which surface hosts it — a child window, a
/// drawing area, a view — is decided inside `src/platform/` and never named
/// here; callers only need to know whether the backend can display it, which
/// [`supports_custom_widgets`] answers.
///
/// `id` must already be registered in [`widget::runtime`]. Prefer the
/// higher-level [`app::WindowHandle::mount_custom_widget`], which performs the
/// registration for you and reports failures as a `Result`.
///
/// Returns `false` when the backend cannot host custom-painted widgets, or when
/// it refuses this particular mount. Backends that cannot display them log why.
#[cfg(not(alloc_frugal))]
pub fn mount_custom_widget(
    parent: crate::core::ObjectId,
    id: crate::core::ObjectId,
    rect: crate::core::Rect,
) -> bool {
    platform::get_platform().mount_custom_widget(parent, id, rect)
}

/// Moves and resizes a mounted custom-painted widget.
#[cfg(not(alloc_frugal))]
pub fn resize_custom_widget(id: crate::core::ObjectId, rect: crate::core::Rect) -> bool {
    platform::get_platform().resize_custom_widget(id, rect)
}

/// Unmounts a custom-painted widget from its window.
#[cfg(not(alloc_frugal))]
pub fn unmount_custom_widget(id: crate::core::ObjectId) -> bool {
    platform::get_platform().unmount_custom_widget(id)
}

/// Marks a mounted custom-painted widget as needing a repaint.
///
/// Returns `false` when the id is not a custom-painted widget mounted on the
/// active backend.
#[cfg(not(alloc_frugal))]
pub fn request_custom_repaint(id: crate::core::ObjectId) -> bool {
    platform::get_platform().repaint_custom_widget(id)
}

/// Returns `true` when the active backend can display custom-painted widgets.
#[cfg(not(alloc_frugal))]
pub fn supports_custom_widgets() -> bool {
    platform::get_platform().supports_custom_widgets()
}

/// Mounts a widget object on the platform's surface, or reports why it cannot.
///
/// This is the **single implementation** of the register → mount → roll back on
/// failure sequence, used by both [`create_widget_of_kind`] (for kinds with no
/// platform control) and [`app::WindowHandle::mount_custom_widget`]. Keeping one
/// copy is what stops the two paths from disagreeing about ownership: on any
/// failure the widget is unregistered, so the registry never holds a widget the
/// backend is not showing.
///
/// Returns `Ok(id)` with the widget live in the registry, or `Err(reason)`.
#[cfg(not(alloc_frugal))]
fn mount_widget_object(
    parent: crate::core::ObjectId,
    widget: Box<dyn widget::Widget>,
    rect: crate::core::Rect,
) -> Result<crate::core::ObjectId, widget::runtime::CustomWidgetMountError> {
    use widget::runtime::CustomWidgetMountError;

    // Register first: the backend looks the widget up by id on every repaint.
    let id = widget::runtime::register(widget).ok_or(CustomWidgetMountError::NoRegistryOnThread)?;
    widget::runtime::set_geometry(id, rect);

    if !platform::get_platform().mount_custom_widget(parent, id, rect) {
        // Do not leave a widget stranded when the backend refused to show it.
        widget::runtime::unregister(id);
        if !supports_custom_widgets() {
            return Err(CustomWidgetMountError::UnsupportedByBackend(backend_name()));
        }
        return Err(CustomWidgetMountError::RejectedByBackend(backend_name()));
    }
    Ok(id)
}

/// Creates a widget of any kind — **the single, mechanism-free creation entry**.
///
/// # How this differs from the per-kind `create_*` functions
///
/// The `create_*` functions are the typo-safe spelling for the common widgets
/// (`create_button`, `create_slider`, …). This function is the general form: hand
/// Creates a widget of `kind`, painted by the library.
///
/// # What this does now
///
/// Every kind is painted by the library (BLUE15 rule #55), so there is a single
/// path: build (or accept) the `Box<dyn Widget>` and hand it to the host surface
/// through `widget::runtime`. The `Platform` layer supplies a surface to paint on,
/// not a control to map onto, so no branch here asks which mechanism to use.
///
/// # The `widget` argument
///
/// A caller may supply a pre-built object so that a widget with constructor
/// arguments the factory cannot express (`CodeEditor` with initial text, a chart
/// with series) reaches this path without a bespoke branch. When it is `None` the
/// widget factory is asked; that factory is the single place that knows every
/// kind's constructor (rule #65).
///
/// Returns `0` — a truthful "not created" — when the kind has no constructor and
/// the caller supplied no object. Callers already handle `0` for the per-kind
/// functions, so this needs no separate error channel.
#[cfg(not(alloc_frugal))]
pub fn create_widget_of_kind(
    kind: widget::WidgetKind,
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    widget: Option<Box<dyn widget::Widget>>,
) -> crate::core::ObjectId {
    let rect = crate::core::Rect::new(x, y, width, height);

    // Prefer the caller's object; otherwise build from the factory, which is the
    // only component that knows every kind's constructor.
    let widget = widget.or_else(|| {
        #[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
        {
            widget::WidgetFactory::new_with_defaults().create(&kind_name(kind), rect, text)
        }
        #[cfg(not(any(feature = "desktop", feature = "tablet", feature = "mobile")))]
        {
            let _ = (text, rect);
            None
        }
    });

    let Some(widget) = widget else {
        log::warn!(
            "create_widget_of_kind: no constructor for {kind:?} and no widget object was \
             supplied; returning 0"
        );
        return 0;
    };

    match mount_widget_object(parent, widget, rect) {
        Ok(id) => id,
        Err(error) => {
            log::warn!("create_widget_of_kind: cannot host {kind:?}: {error}");
            0
        }
    }
}

/// The canonical factory name for a [`WidgetKind`].
///
/// Derived from the widget capability registry (the same table the factory
/// dispatches on) rather than from `Debug` output, so the two can never disagree.
/// Falls back to the debug name for kinds with no registered capability or when
/// the capability module is compiled out (embedded/mini).
#[cfg(not(alloc_frugal))]
fn kind_name(kind: widget::WidgetKind) -> alloc::string::String {
    #[cfg(any(feature = "desktop", feature = "tablet", feature = "mobile"))]
    {
        use alloc::string::ToString;
        if let Some(capability) =
            widget::WidgetFactory::new_with_defaults().capability_by_kind(kind)
        {
            return capability.canonical_name.to_string();
        }
    }
    alloc::format!("{kind:?}").to_lowercase()
}

/// Stub for mini mode (no platform runtime, no windows).
#[cfg(alloc_frugal)]
pub fn mount_custom_widget(
    _parent: crate::core::ObjectId,
    _id: crate::core::ObjectId,
    _rect: crate::core::Rect,
) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn resize_custom_widget(_id: crate::core::ObjectId, _rect: crate::core::Rect) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn unmount_custom_widget(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn request_custom_repaint(_id: crate::core::ObjectId) -> bool {
    false
}

/// Stub for mini mode.
#[cfg(alloc_frugal)]
pub fn supports_custom_widgets() -> bool {
    false
}

/// Show a widget by its object id.
///
/// This is a convenience wrapper around `platform::get_platform().show_widget()`.
#[cfg(not(alloc_frugal))]
pub fn show_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().show_widget(widget_id);
}
/// Hide a widget by its object id.
///
/// This is a convenience wrapper around `platform::get_platform().hide_widget()`.
#[cfg(not(alloc_frugal))]
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    platform::get_platform().hide_widget(widget_id);
}
/// Set geometry of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_geometry()`.
#[cfg(not(alloc_frugal))]
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
#[cfg(not(alloc_frugal))]
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    platform::get_platform().set_widget_text(widget_id, text);
}
/// Get text of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().get_widget_text()`.
#[cfg(not(alloc_frugal))]
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_enabled()`.
#[cfg(not(alloc_frugal))]
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    platform::get_platform().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_enabled()`.
#[cfg(not(alloc_frugal))]
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
///
/// This is a convenience wrapper around `platform::get_platform().set_widget_visible()`.
#[cfg(not(alloc_frugal))]
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    platform::get_platform().set_widget_visible(widget_id, visible);
}
/// Check if a widget is visible.
///
/// This is a convenience wrapper around `platform::get_platform().is_widget_visible()`.
#[cfg(not(alloc_frugal))]
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_visible(widget_id)
}

/// Set a widget's primary numeric value (slider, progress bar, spin box, ...).
///
/// Returns `false` when this backend's control has no numeric value, so callers
/// never mistake "unsupported" for "set to 0".
#[cfg(not(alloc_frugal))]
pub fn set_widget_value(widget_id: crate::core::ObjectId, value: f64) -> bool {
    platform::get_platform().set_widget_value(widget_id, value)
}

/// Read a widget's primary numeric value.
#[cfg(not(alloc_frugal))]
pub fn widget_value(widget_id: crate::core::ObjectId) -> Option<f64> {
    platform::get_platform().widget_value(widget_id)
}

/// Set a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn set_widget_range(widget_id: crate::core::ObjectId, min: f64, max: f64) -> bool {
    platform::get_platform().set_widget_range(widget_id, min, max)
}

/// Read a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn widget_range(widget_id: crate::core::ObjectId) -> Option<(f64, f64)> {
    platform::get_platform().widget_range(widget_id)
}

/// Set a widget's selection index (combo box, list box, tab widget).
#[cfg(not(alloc_frugal))]
pub fn set_widget_selected_index(widget_id: crate::core::ObjectId, index: Option<usize>) -> bool {
    platform::get_platform().set_widget_selected_index(widget_id, index)
}

/// Read a widget's selection index.
#[cfg(not(alloc_frugal))]
pub fn widget_selected_index(widget_id: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().widget_selected_index(widget_id)
}

/// Set a widget's checked state (check box, radio button, toggle button).
#[cfg(not(alloc_frugal))]
pub fn set_widget_checked(widget_id: crate::core::ObjectId, checked: bool) -> bool {
    platform::get_platform().set_widget_checked(widget_id, checked)
}

/// Read a widget's checked state, or `None` when it is not checkable.
#[cfg(not(alloc_frugal))]
pub fn is_widget_checked(widget_id: crate::core::ObjectId) -> Option<bool> {
    platform::get_platform().is_widget_checked(widget_id)
}

/// Set a widget's increment step (slider, spin box, scroll bar).
#[cfg(not(alloc_frugal))]
pub fn set_widget_step(widget_id: crate::core::ObjectId, step: f64) -> bool {
    platform::get_platform().set_widget_step(widget_id, step)
}

/// Read a widget's increment step.
#[cfg(not(alloc_frugal))]
pub fn widget_step(widget_id: crate::core::ObjectId) -> Option<f64> {
    platform::get_platform().widget_step(widget_id)
}

/// Set a progress-style widget's indeterminate (busy) state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_indeterminate(widget_id: crate::core::ObjectId, indeterminate: bool) -> bool {
    platform::get_platform().set_widget_indeterminate(widget_id, indeterminate)
}

/// Read a progress-style widget's indeterminate state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_indeterminate(widget_id: crate::core::ObjectId) -> Option<bool> {
    platform::get_platform().is_widget_indeterminate(widget_id)
}

/// Set a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_read_only(widget_id: crate::core::ObjectId, read_only: bool) -> bool {
    platform::get_platform().set_widget_read_only(widget_id, read_only)
}

/// Read a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_read_only(widget_id: crate::core::ObjectId) -> Option<bool> {
    platform::get_platform().is_widget_read_only(widget_id)
}

/// Set a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn set_widget_max_length(widget_id: crate::core::ObjectId, max_length: u32) -> bool {
    platform::get_platform().set_widget_max_length(widget_id, max_length)
}

/// Read a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn widget_max_length(widget_id: crate::core::ObjectId) -> Option<u32> {
    platform::get_platform().widget_max_length(widget_id)
}

/// Apply or clear a window state (maximised, minimised, full-screen, ...).
///
/// Returns `false` when the id is not a window or the backend cannot honour the
/// state on its toolkit.
#[cfg(not(alloc_frugal))]
pub fn set_window_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
    on: bool,
) -> bool {
    platform::get_platform().set_window_state(widget_id, flag, on)
}

/// Read a window state, or `None` when the id is not a window.
#[cfg(not(alloc_frugal))]
pub fn is_window_in_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
) -> Option<bool> {
    platform::get_platform().is_window_in_state(widget_id, flag)
}

/// Set a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn set_window_min_size(widget_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    platform::get_platform().set_window_min_size(widget_id, width, height)
}

/// Read a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn window_min_size(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    platform::get_platform().window_min_size(widget_id)
}

/// Set a window's icon from a file path.
#[cfg(not(alloc_frugal))]
pub fn set_window_icon(widget_id: crate::core::ObjectId, path: &str) -> bool {
    platform::get_platform().set_window_icon(widget_id, path)
}

/// Read a window's icon path, if one was set.
#[cfg(not(alloc_frugal))]
pub fn window_icon(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().window_icon(widget_id)
}

/// Set a text entry's selection range as `(start, end)` character offsets.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selection(widget_id: crate::core::ObjectId, start: u32, end: u32) -> bool {
    platform::get_platform().set_widget_selection(widget_id, start, end)
}

/// Read a text entry's selection range, or `None` when nothing is selected.
#[cfg(not(alloc_frugal))]
pub fn widget_selection(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    platform::get_platform().widget_selection(widget_id)
}

/// Set a text entry's placeholder (cue) text.
#[cfg(not(alloc_frugal))]
pub fn set_widget_placeholder(widget_id: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().set_widget_placeholder(widget_id, text)
}

/// Read a text entry's placeholder text.
#[cfg(not(alloc_frugal))]
pub fn widget_placeholder(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().widget_placeholder(widget_id)
}

/// Set a text entry's echo mode.
#[cfg(not(alloc_frugal))]
pub fn set_widget_echo_mode(widget_id: crate::core::ObjectId, mode: platform::EchoMode) -> bool {
    platform::get_platform().set_widget_echo_mode(widget_id, mode)
}

/// Read a text entry's echo mode.
#[cfg(not(alloc_frugal))]
pub fn widget_echo_mode(widget_id: crate::core::ObjectId) -> Option<platform::EchoMode> {
    platform::get_platform().widget_echo_mode(widget_id)
}

/// Apply a slider's creation-time orientation.
#[cfg(not(alloc_frugal))]
pub fn set_slider_orientation(
    widget_id: crate::core::ObjectId,
    orientation: crate::core::Orientation,
) -> bool {
    platform::get_platform().set_slider_orientation(widget_id, orientation)
}

/// Read a slider's orientation.
#[cfg(not(alloc_frugal))]
pub fn slider_orientation(widget_id: crate::core::ObjectId) -> Option<crate::core::Orientation> {
    platform::get_platform().slider_orientation(widget_id)
}

/// Set a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn set_widget_tristate(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    platform::get_platform().set_widget_tristate(widget_id, enabled)
}

/// Read a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn is_widget_tristate(widget_id: crate::core::ObjectId) -> Option<bool> {
    platform::get_platform().is_widget_tristate(widget_id)
}

/// Put a radio button into a named mutually-exclusive group.
#[cfg(not(alloc_frugal))]
pub fn set_widget_group(widget_id: crate::core::ObjectId, group: &str) -> bool {
    platform::get_platform().set_widget_group(widget_id, group)
}

/// Read a radio button's group name.
#[cfg(not(alloc_frugal))]
pub fn widget_group(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().widget_group(widget_id)
}

/// Set a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn set_widget_scroll_position(widget_id: crate::core::ObjectId, x: i32, y: i32) -> bool {
    platform::get_platform().set_widget_scroll_position(widget_id, x, y)
}

/// Read a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn widget_scroll_position(widget_id: crate::core::ObjectId) -> Option<(i32, i32)> {
    platform::get_platform().widget_scroll_position(widget_id)
}
// ComboBox operations
#[cfg(not(alloc_frugal))]
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().combo_box_add_item(combo_box, text)
}
#[cfg(not(alloc_frugal))]
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    platform::get_platform().combo_box_clear_items(combo_box)
}
#[cfg(not(alloc_frugal))]
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().combo_box_set_current_index(combo_box, index)
}
#[cfg(not(alloc_frugal))]
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().combo_box_current_index(combo_box)
}
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    platform::get_platform().combo_box_item_count(combo_box)
}
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().combo_box_item_text(combo_box, index)
}
// ListBox operations
#[cfg(not(alloc_frugal))]
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    platform::get_platform().list_box_add_item(list_box, text)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_remove_item(list_box, index)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    platform::get_platform().list_box_clear_items(list_box)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    platform::get_platform().list_box_set_current_index(list_box, index)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().list_box_current_index(list_box)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    platform::get_platform().list_box_item_count(list_box)
}
#[cfg(not(alloc_frugal))]
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    platform::get_platform().list_box_item_text(list_box, index)
}
// Event polling
#[cfg(not(alloc_frugal))]
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_widget_triggered()
}
#[cfg(not(alloc_frugal))]
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    platform::get_platform().poll_widget_trigger_event()
}
#[cfg(not(alloc_frugal))]
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    platform::get_platform().inject_widget_trigger_event(widget_id, kind)
}
// Clipboard
#[cfg(not(alloc_frugal))]
pub fn set_clipboard_text(text: &str) -> bool {
    platform::get_platform().set_clipboard_text(text)
}
#[cfg(not(alloc_frugal))]
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
// Menu operations
#[cfg(not(alloc_frugal))]
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_MENU_BAR).create_menu_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
pub fn create_menu(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_MENU).create_menu(parent, text, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    platform::get_platform().attach_menu_bar_to_window(window, menu_bar)
}
#[cfg(not(alloc_frugal))]
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    platform::get_platform().menu_add_item(parent_menu, text, shortcut)
}
/// Renders a shortcut the way the current operating system writes it.
///
/// macOS returns `⌘⇧Z`-style glyphs; Windows and Linux return
/// `Ctrl+Shift+Z`. Use this for any label the user reads (menu text, tooltips,
/// the shortcut column of a command palette) so one shortcut declaration reads
/// natively on every platform.
///
/// For the host-independent notation (useful in serialized keymaps and tests),
/// use [`crate::shortcut::Shortcut::format_shortcut`] together with
/// [`crate::shortcut::format_shortcut_for_platform`] instead.
///
/// ```
/// use rust_widgets::shortcut::{Key, Shortcut};
///
/// let shown = rust_widgets::format_shortcut(&Shortcut::primary(Key::Z));
/// #[cfg(target_os = "macos")]
/// assert_eq!(shown, "⌘Z");
/// #[cfg(not(target_os = "macos"))]
/// assert_eq!(shown, "Ctrl+Z");
/// ```
#[cfg(not(alloc_frugal))]
pub fn format_shortcut(shortcut: &crate::shortcut::Shortcut) -> String {
    platform::get_platform().format_shortcut(shortcut)
}
#[cfg(not(alloc_frugal))]
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    platform::get_platform().poll_menu_triggered()
}
/// Returns the accelerator text bound to a menu item, if any.
///
/// Lets a host verify that a shortcut was genuinely registered with the platform
/// (and not merely drawn into a label). Returns `None` for a non-menu-item id or
/// an item created without a shortcut.
#[cfg(not(alloc_frugal))]
pub fn menu_item_shortcut(menu_item: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().menu_item_shortcut(menu_item)
}
/// Returns the backend's native handle for a widget, when it has one.
///
/// The value is opaque: it is the platform's own object pointer or handle (an
/// `NSView*` on macOS, an `HWND` on Windows, ...), and its meaning is entirely
/// backend-specific. It exists so host code and integration tests can reach the
/// underlying control for things the cross-platform API does not model.
///
/// Returns `None` when the widget is unknown, or when the backend created it in
/// state-only mode (for example off the UI thread) and therefore has no native
/// object to return.
#[cfg(not(alloc_frugal))]
pub fn native_handle(widget: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().get_native_handle(widget)
}
#[cfg(not(alloc_frugal))]
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    platform::get_platform().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
#[cfg(not(alloc_frugal))]
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_TOOL_BAR).create_tool_bar(parent, x, y, width, height)
}
#[cfg(not(alloc_frugal))]
pub fn create_status_bar(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    backend_for_kind(KIND_STATUS_BAR).create_status_bar(parent, text, x, y, width, height)
}
// Drag and Drop
#[cfg(not(alloc_frugal))]
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
#[cfg(not(alloc_frugal))]
pub fn poll_drop_event() -> Option<DropEvent> {
    platform::get_platform().poll_drop_event()
}
#[cfg(not(alloc_frugal))]
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
#[cfg(not(alloc_frugal))]
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    platform::get_platform().set_widget_ime_enabled(widget_id, enabled)
}
#[cfg(not(alloc_frugal))]
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    platform::get_platform().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
#[cfg(not(alloc_frugal))]
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    platform::get_platform().set_widget_accessibility_name(widget_id, name)
}
#[cfg(not(alloc_frugal))]
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    platform::get_platform().get_widget_accessibility_name(widget_id)
}
// Re-exports from platform module for convenience
#[cfg(not(alloc_frugal))]
pub use platform::{
    backend_name, capabilities, dpi_scale_factor, get_platform, init as platform_init,
    quit as platform_quit, run as platform_run, runtime_gui_mode, runtime_gui_mode_for,
};
pub use platform::{
    CapabilityContract, DesktopBackend, DropEvent, EmbeddedCapabilityContract, MobileBackend,
    NativeCapabilityContract, PlatformCapabilities, RuntimeGuiMode, WidgetTriggerEvent,
    WidgetTriggerKind, WindowStateFlag,
};
