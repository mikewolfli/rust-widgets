// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `ControlBackend` declaration and the methods every backend must provide.
//!
//! The defaulted half of the body lives in `defaults` and `defaults_catalog` as
//! `macro_rules!` fragments, each invoked here where its methods sat in the
//! original single-trait file, so the expansion keeps that order. The fragment
//! bodies resolve their names in this module, which is why the crate imports
//! below stay here.

use crate::compat::String;
use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};

use super::defaults::{
    default_advanced_widgets, default_combo_box_items, default_container_and_chart_widgets,
    default_dock_panel, default_geometry_queries, default_list_box_items,
    default_menu_dialog_view_widgets, default_visibility, default_widget_creation,
};
use super::defaults_catalog::default_modern_widgets;

/// The contract every control backend satisfies.
///
/// # Required vs. inherited
///
/// Only a small core is *required*. A backend must provide:
///
/// * [`backend_name`](ControlBackend::backend_name) and
///   [`kind`](ControlBackend::kind) — identity, used by diagnostics and by the
///   dispatcher's cache key;
/// * the widget **creation** methods for the kinds it actually supports
///   ([`create_window`](ControlBackend::create_window),
///   [`create_button`](ControlBackend::create_button), …);
/// * the **write** side of the shared widget-state accessors
///   ([`set_widget_text`](ControlBackend::set_widget_text),
///   [`set_widget_enabled`](ControlBackend::set_widget_enabled),
///   [`set_widget_visible`](ControlBackend::set_widget_visible),
///   [`set_widget_geometry`](ControlBackend::set_widget_geometry)), which every
///   backend can answer because it owns the state it just allocated;
/// * the IME and accessibility-name accessors, whose defaults likewise cannot
///   be invented (a wrong label is worse than a missing one).
///
/// Everything else has a default body and **may** be inherited. That is the
/// whole point of the split: the crate declares well over a hundred
/// `create_*` methods, so a minimal backend (say `embedded-mini`) overrides
/// only the handful of kinds it can actually host and lets the rest fall
/// through, rather than being forced to write a hundred stubs.
///
/// # The honest-failure convention
///
/// The inherited defaults do not pretend to work. They report absence:
///
/// * `create_*` returns `ObjectId` `0` — an id that no allocation can produce,
///   so a caller that failed to override one gets an id it cannot store or
///   query rather than a plausible-looking handle;
/// * boolean queries return `false`, accessor results return `None`,
///   mutating operations return `false` for "not performed", and `()`-returning
///   operations do nothing;
/// * read accessors return the empty string (see
///   [`get_widget_text`](ControlBackend::get_widget_text)).
///
/// A backend must therefore **never** return a defaulted success to mean
/// "done", and a caller must not read `0` / `false` / `None` as a silent
/// success. This is deliberate (principle #37): a stub that fakes a control
/// would put a second, divergent copy of widget semantics behind a code path
/// that tests cannot distinguish from a working backend.
///
/// # Threading and lifetimes
///
/// Implementations must be `Send + Sync`: the process-wide backend from
/// [`get_control_backend`](crate::control_backend::get_control_backend) is
/// shared across threads. Returned `ObjectId`s stay valid until
/// [`destroy_widget`](ControlBackend::destroy_widget) is called; recycling an id
/// for a different widget is not permitted while the old one is alive.
pub trait ControlBackend: Send + Sync {
    /// Backend display name.
    fn backend_name(&self) -> &'static str;
    /// Backend family kind.
    fn kind(&self) -> ControlBackendKind;

    // ── Widget creation helpers (default implementations) ──

    default_widget_creation!();

    /// Create top-level window.
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create button control.
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create checkbox control.
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create line edit control.
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create label control.
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create radio button control.
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create slider control.
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create progress bar control.
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create combo box control.
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    default_combo_box_items!();
    /// Create list box control.
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    default_list_box_items!();
    /// Create panel control.
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    default_menu_dialog_view_widgets!();
    /// Create scroll area control.
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    default_dock_panel!();
    /// Create group box control.
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    default_container_and_chart_widgets!();
    /// Create toggle button control.
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    default_advanced_widgets!();
    /// Set widget text.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str);
    /// Get widget text.
    fn get_widget_text(&self, widget_id: ObjectId) -> String;
    default_visibility!();
    /// Set widget enabled state.
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool);
    /// Read widget enabled state.
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set widget visibility.
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool);
    /// Read widget visibility state.
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool;
    /// Set widget geometry.
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32);
    default_geometry_queries!();
    /// Enable or disable IME input handling for a widget.
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool;
    /// Query IME enabled state for a widget.
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set accessibility name/label for a widget.
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool;
    /// Read accessibility name/label for a widget.
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String;
    default_modern_widgets!();
}
