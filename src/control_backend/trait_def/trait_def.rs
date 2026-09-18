// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unified control backend contract.
//!
//! This trait defines the full widget-creation and lifecycle surface. Core
//! methods (window, button, checkbox, etc.) are required; all non-core methods
//! have safe default implementations returning `0`, `false`, `None`, or `()`
//! so that minimal backends (e.g. embedded-mini) only need to override the
//! ~15 core widget methods they support.
//!
//! Helper traits and pattern implementations are also provided in
//! [`custom`](crate::control_backend::custom) and
//! [`native`](crate::control_backend::native) modules for common use cases.

use crate::compat::String;
use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
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

    /// Create a widget from a standard geometry pattern with default text "".
    ///
    /// This is a convenience helper that calls `create_widget(geom)` with
    /// default text. Override `create_widget` for custom widget creation.
    /// Default returns a no-op (0) handle.
    #[allow(clippy::too_many_arguments)]
    fn create_widget(
        &self,
        _kind: &str,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    // ── Concrete widget creation methods ──

    /// Destroy a previously created widget and release its resources.
    ///
    /// Returns `true` when the widget existed and was torn down. The default
    /// returns `false` so existing backends keep compiling; the native and
    /// custom backends forward this to the active platform backend.
    fn destroy_widget(&self, _widget_id: ObjectId) -> bool {
        false
    }

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
    /// Add item to combo box.
    fn combo_box_add_item(&self, _widget_id: ObjectId, _text: &str) -> bool {
        false
    }
    /// Clear all items from combo box.
    fn combo_box_clear_items(&self, _widget_id: ObjectId) -> bool {
        false
    }
    /// Read a combo box item's text by index.
    ///
    /// Default: the backend keeps no items, so there is nothing to read. A
    /// backend that owns a control answers from it rather than from a copy.
    fn combo_box_item_text(&self, _widget_id: ObjectId, _index: usize) -> Option<String> {
        None
    }
    /// Create list box control.
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Add item to list box.
    fn list_box_add_item(&self, _widget_id: ObjectId, _text: &str) -> bool {
        false
    }
    /// Remove item from list box by index.
    fn list_box_remove_item(&self, _widget_id: ObjectId, _index: usize) -> bool {
        false
    }
    /// Clear all items from list box.
    fn list_box_clear_items(&self, _widget_id: ObjectId) -> bool {
        false
    }
    /// Read a list box item's text by index.
    fn list_box_item_text(&self, _widget_id: ObjectId, _index: usize) -> Option<String> {
        None
    }
    /// Read the accelerator text bound to a menu entry.
    ///
    /// Default: the backend keeps no menu entries, so there is nothing to read.
    fn menu_item_shortcut(&self, _menu_item: ObjectId) -> Option<String> {
        None
    }
    /// Create panel control.
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    /// Create menu bar host control.
    fn create_menu_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create menu host control.
    fn create_menu(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Attach menu bar to top-level window.
    fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
        false
    }
    /// Add menu item to menu host control.
    fn menu_add_item(
        &self,
        _parent_menu: ObjectId,
        _text: &str,
        _shortcut: Option<&str>,
    ) -> ObjectId {
        0
    }
    /// Create tool bar host control.
    fn create_tool_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create status bar host control.
    fn create_status_bar(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create dialog control.
    fn create_dialog(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create message box control.
    #[allow(clippy::too_many_arguments)]
    fn create_message_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create file dialog control.
    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create color dialog control.
    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create font dialog control.
    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create popup window control.
    fn create_popup_window(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create text edit control.
    fn create_text_edit(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create rich edit control.
    fn create_rich_edit(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create spin box control.
    fn create_spin_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create list view control.
    fn create_list_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tree view control.
    fn create_tree_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create scroll bar control.
    fn create_scroll_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create scroll area control.
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId;
    /// Create dock panel control.
    fn create_dock_panel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
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
    /// Create tab widget control.
    fn create_tab_widget(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create splitter control.
    fn create_splitter(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create stack widget control.
    fn create_stack_widget(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create MDI area control.
    fn create_mdi_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create canvas control.
    fn create_canvas(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create table control.
    fn create_table(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create grid control.
    fn create_grid(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create chart control.
    fn create_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create radar (spider) chart control.
    fn create_radar_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create kanban board control.
    fn create_kanban_board(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cascader (multi-level path chooser) control.
    fn create_cascader(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create query builder (condition-row filter editor) control.
    fn create_query_builder(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create emoji picker (caller-supplied glyph shell) control.
    fn create_emoji_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create mention (triggered candidate completion) control.
    fn create_mention(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
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
    /// Create check list box control.
    fn create_check_list_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create double spin box control.
    fn create_double_spin_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create dial control.
    fn create_dial(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create wizard control.
    fn create_wizard(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create date picker control.
    fn create_date_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create time picker control.
    fn create_time_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create date time picker control.
    fn create_date_time_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create directory dialog control.
    fn create_directory_dialog(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create data view control.
    fn create_data_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create property grid control.
    fn create_property_grid(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create toolbox control.
    fn create_toolbox(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create collapsible pane control.
    fn create_collapsible_pane(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create dock widget control.
    fn create_dock_widget(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create web view control.
    fn create_web_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create activity indicator control.
    fn create_activity_indicator(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create calendar control.
    fn create_calendar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create column view control.
    fn create_column_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create undo view control.
    fn create_undo_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create command link control.
    fn create_command_link(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create LCD number control.
    fn create_lcd_number(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create font combo box control.
    fn create_font_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create web engine view control.
    fn create_web_engine_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create a web engine page, which is the *same control* as
    /// [`ControlBackend::create_web_engine_view`].
    ///
    /// The nine `create_web_engine_*` methods below all name one view under the
    /// render pipeline's per-symbol names (`page`, `settings`, `download`, …), so
    /// each forwards to the view rather than building a different control. They
    /// remain on the trait because they are part of the C ABI's method set; a
    /// backend with no web support returns `0` from the view method and therefore
    /// `0` here too, honestly.
    fn create_web_engine_page(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine settings control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_settings(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine download item control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_download_item(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine cookie store control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_cookie_store(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine web channel control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_web_channel(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine find text result control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_find_text_result(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine notification control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_notification(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine script dialog control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_script_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create web engine context menu request control. See
    /// [`ControlBackend::create_web_engine_page`].
    fn create_web_engine_context_menu_request(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        self.create_web_engine_view(parent, x, y, width, height)
    }
    /// Create action control.
    fn create_action(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tool button control.
    fn create_tool_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tool box control.
    fn create_tool_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create context menu control.
    fn create_context_menu(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create freeform shape control.
    fn create_freeform_shape(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tab bar control.
    fn create_tab_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create pie menu control.
    fn create_pie_menu(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create ribbon bar control.
    fn create_ribbon_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Poll next menu trigger id.
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        None
    }
    /// Inject a menu trigger id.
    fn inject_menu_trigger(&self, _menu_item_id: ObjectId) -> bool {
        false
    }
    /// Poll next widget id trigger if available.
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    /// Poll next typed widget trigger event.
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        None
    }
    /// Inject a typed widget trigger event.
    fn inject_widget_trigger_event(&self, _widget_id: ObjectId, _kind: WidgetTriggerKind) -> bool {
        false
    }
    /// Set widget text.
    fn set_widget_text(&self, widget_id: ObjectId, text: &str);
    /// Get widget text.
    fn get_widget_text(&self, widget_id: ObjectId) -> String;
    /// Show widget.
    fn show_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, true);
    }
    /// Hide widget.
    fn hide_widget(&self, widget_id: ObjectId) {
        self.set_widget_visible(widget_id, false);
    }
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
    /// Get widget geometry.
    fn get_widget_geometry(&self, _widget_id: ObjectId) -> Option<(i32, i32, u32, u32)> {
        None
    }

    /// Report that a container's client area became `width` by `height`.
    ///
    /// A host calls this when the *user* resizes a window — the case no
    /// `set_widget_geometry` covers, because the program was not the one changing the
    /// size. Two things happen: the size becomes answerable through
    /// [`Self::window_client_size`], and a `Resized` trigger is queued so the app layer
    /// re-runs the window's layout.
    ///
    /// Returns `false` when the id addresses nothing this backend created, so a stale id
    /// cannot inject a phantom resize.
    fn queue_resize_trigger(&self, _window_id: ObjectId, _width: u32, _height: u32) -> bool {
        false
    }

    /// The client size last reported for `window_id`, or `None` when none was.
    ///
    /// This is the "how big is it now?" half of [`Self::queue_resize_trigger`]: the
    /// resize event carries only an id, and it is consumed by polling, so a host reads
    /// the dimensions from here rather than trying to race the queue.
    fn window_client_size(&self, _window_id: ObjectId) -> Option<(u32, u32)> {
        None
    }
    /// Enable or disable IME input handling for a widget.
    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool;
    /// Query IME enabled state for a widget.
    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool;
    /// Set accessibility name/label for a widget.
    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool;
    /// Read accessibility name/label for a widget.
    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String;
    /// Set clipboard text.
    fn set_clipboard_text(&self, _text: &str) -> bool {
        false
    }
    /// Get clipboard text.
    fn get_clipboard_text(&self) -> String {
        String::new()
    }
    /// Begin drag operation.
    fn begin_drag(&self, _source: ObjectId, _mime_type: &str, _payload: &[u8]) -> bool {
        false
    }
    /// Poll next drop event.
    fn poll_drop_event(&self) -> Option<crate::platform::DropEvent> {
        None
    }
    /// Inject a drop event.
    fn inject_drop_event(&self, _event: crate::platform::DropEvent) -> bool {
        false
    }

    // ── Modern widget set (BLUE13 R2.1–R2.14 + mobile/Cupertino/media families) ──
    //
    // These defaults return `0` as the explicit unsupported marker; backends that
    // support a widget kind override the corresponding method.

    /// Create adaptive scaffold control.
    fn create_adaptive_scaffold(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create animated image control.
    fn create_animated_image(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create app bar control.
    fn create_app_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create arc control.
    fn create_arc(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create audio visualizer control.
    fn create_audio_visualizer(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create auto-complete edit control.
    fn create_auto_complete_edit(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create avatar control.
    fn create_avatar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create badge control.
    fn create_badge(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create bar chart control.
    fn create_bar_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create barcode scanner control.
    fn create_barcode_scanner(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create bezier curve editor control.
    fn create_bezier_curve_editor(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create bottom navigation bar control.
    fn create_bottom_navigation_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create bottom sheet control.
    fn create_bottom_sheet(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create camera preview control.
    fn create_camera_preview(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create carousel control.
    fn create_carousel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create chip control.
    fn create_chip(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create color history control.
    fn create_color_history(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create color well control.
    fn create_color_well(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino alert dialog control.
    fn create_cupertino_alert_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino date picker control.
    fn create_cupertino_date_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino navigation bar control.
    fn create_cupertino_navigation_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino segmented control.
    fn create_cupertino_segmented_control(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino slider control.
    fn create_cupertino_slider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create cupertino switch control.
    fn create_cupertino_switch(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create date range picker control.
    fn create_date_range_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create divider control.
    fn create_divider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create dropdown control.
    fn create_dropdown(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create dropdown menu control.
    fn create_dropdown_menu(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create editable combo box control.
    fn create_editable_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create empty state control.
    fn create_empty_state(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create floating action button (FAB) control.
    fn create_fab(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create find/replace dialog control.
    fn create_find_replace_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create floating label control.
    fn create_floating_label(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create font preview control.
    fn create_font_preview(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create frame control.
    fn create_frame(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create grid table control.
    fn create_grid_table(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create hero animation control.
    fn create_hero_animation(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create icon control.
    fn create_icon(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create image gallery control.
    fn create_image_gallery(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create image view control.
    fn create_image_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create IME preedit control.
    fn create_ime_preedit(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create in-place editor control.
    fn create_inplace_editor(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create keyboard control.
    fn create_keyboard(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create line control.
    fn create_line(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create line chart control.
    fn create_line_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create lottie widget control.
    fn create_lottie_widget(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create masked edit control.
    fn create_masked_edit(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create masonry layout control.
    fn create_masonry_layout(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create material navigation rail control.
    fn create_material_navigation_rail(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create material snackbar control.
    fn create_material_snackbar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create menu button control.
    fn create_menu_button(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create meter control.
    fn create_meter(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create mini canvas control.
    fn create_mini_canvas(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create mini chart control.
    fn create_mini_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create mobile date picker control.
    fn create_mobile_date_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create modal bottom sheet control.
    fn create_modal_bottom_sheet(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create multi-select combo box control.
    fn create_multi_select_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create navigation drawer control.
    fn create_navigation_drawer(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create navigation stack control.
    fn create_navigation_stack(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create pie chart control.
    fn create_pie_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create popover control.
    fn create_popover(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create progress circle control.
    fn create_progress_circle(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create properties panel control.
    fn create_properties_panel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create QR code control.
    fn create_qr_code(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create QRCode control (alias for `create_qr_code` used by the route-matrix
    /// generator, which derives `create_qrcode` from `WidgetKind::QRCode`).
    fn create_qrcode(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create range slider control.
    fn create_range_slider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create rating control.
    fn create_rating(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create refresh control.
    fn create_refresh_control(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create rive widget control.
    fn create_rive_widget(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create roller control.
    fn create_roller(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create safe area control.
    fn create_safe_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create search bar control.
    fn create_search_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create search box control.
    fn create_search_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create segmented button control.
    fn create_segmented_button(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create shortcut editor control.
    fn create_shortcut_editor(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create skeleton loader control.
    fn create_skeleton_loader(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create number picker control.
    fn create_number_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create OTP input control.
    fn create_otp_input(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create pagination control.
    fn create_pagination(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create banner control.
    fn create_banner(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create colour picker control.
    fn create_color_picker(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create toast control.
    fn create_toast(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create splash screen control.
    fn create_splash_screen(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create candlestick (K-line) chart control.
    fn create_candlestick_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    /// Create volume histogram control.
    fn create_volume_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    /// Create market depth chart control.
    fn create_depth_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    /// Create order book ladder control.
    fn create_order_book(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    /// Create quote board control.
    fn create_quote_board(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }

    /// Create technical indicator chart control.
    fn create_indicator_chart(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create sparkline control.
    fn create_sparkline(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create spinner control.
    fn create_spinner(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create stepper control.
    fn create_stepper(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create swipe-to-dismiss control.
    fn create_swipe_to_dismiss(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create switch control.
    fn create_switch(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tab view control.
    fn create_tab_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tag input control.
    fn create_tag_input(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create breadcrumb trail control.
    ///
    /// Promoted from a shared `Panel` kind to its own kind in 2.4.0; the typed
    /// method is what keeps `WidgetKind::Breadcrumb` reachable by name from the
    /// bindings and the route matrix.
    fn create_breadcrumb(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create drag-and-drop upload target control.
    fn create_drop_zone(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create freehand signature capture control.
    fn create_signature_pad(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tree-table control (a tree whose rows carry editable columns).
    fn create_tree_table(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create text area control.
    fn create_text_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create tooltip control.
    fn create_tooltip(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create video player control.
    fn create_video_player(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    /// Create wizard dialog control.
    fn create_wizard_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
}
