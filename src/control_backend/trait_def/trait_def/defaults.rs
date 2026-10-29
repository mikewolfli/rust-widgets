// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Default bodies for the classic widget set, its item accessors, the trigger
//! plumbing and the shared widget-state queries.
//!
//! Each `macro_rules!` below is one contiguous run of the trait body and is
//! invoked from `core` at the position its methods originally occupied. The
//! bodies name `ObjectId`, `String` and the trigger types, which resolve at the
//! invocation site, so those imports live in `core` rather than here.

macro_rules! default_widget_creation {
    () => {
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

        /// Mount an already-built widget box under `parent`.
        ///
        /// # Why this exists alongside [`create_widget`](ControlBackend::create_widget)
        ///
        /// `create_widget` takes a *name* and builds the control inside the backend. A caller that
        /// has already built the widget — the JSON layout loader, which applies a document's whole
        /// property set before the control exists — cannot use it. Without this method that caller
        /// had nowhere to hand the box over, and the JSON loader's answer was to index the widget's
        /// own `id()` and drop the box: a registration the runtime never saw, so every handle the
        /// document resolved addressed nothing.
        ///
        /// `parent` is `None` for a root (a window) and `Some(id)` for a child. `declared_id` is the
        /// id the caller wants the widget *known by* (`0` for "assign one"), which is what keeps a
        /// declarative document's `"id"` addressable rather than translated into a runtime id the
        /// document has never heard of.
        ///
        /// Returns the id the widget is mounted under, or `0` when this backend has no widget
        /// runtime to mount into — never a fabricated id.
        ///
        /// # Why the default reports `0`
        ///
        /// A backend whose controls *are* platform widgets (the native one) does not hold the
        /// widget boxes at all; its `create_*` methods build the platform control directly. It
        /// therefore cannot adopt a box, and saying `0` is the honest answer rather than pretending
        /// to mount it somewhere the accessors will not look.
        fn mount_widget_box(
            &self,
            _widget: crate::compat::Box<dyn crate::widget::Widget>,
            _parent: Option<ObjectId>,
            _declared_id: ObjectId,
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
    };
}
pub(crate) use default_widget_creation;

macro_rules! default_combo_box_items {
    () => {
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
    };
}
pub(crate) use default_combo_box_items;

macro_rules! default_list_box_items {
    () => {
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
    };
}
pub(crate) use default_list_box_items;

macro_rules! default_menu_dialog_view_widgets {
    () => {
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
        /// Reports that the entry at `index` of `menu` was chosen by the user.
        ///
        /// Backends that own a menu-trigger queue implement this so a **self-drawn** menu
        /// selection reaches [`poll_menu_triggered`](Self::poll_menu_triggered), the same way a
        /// native menu's selection does. The default is a no-op: a backend whose menus are host
        /// objects (`NSMenu`, `HMENU`) already has the host feeding that queue.
        ///
        /// `index` is the entry's position in the menu's own entry list, which is what the menu's
        /// `triggered_index` signal reports; the backend maps it back to the id it handed out from
        /// [`Self::menu_add_item`].
        fn note_menu_selection(&self, _menu: ObjectId, _index: usize) {}
        /// Whether the native menu bar behind this backend can present an icon on an item.
        ///
        /// Mirrors [`Platform::supports_menu_item_icons`](crate::platform::Platform::supports_menu_item_icons),
        /// the same way [`ControlBackend::menu_add_item`] mirrors `Platform::menu_add_item`: the control
        /// layer is what callers hold, and the answer is the platform's. `false` by default, because a
        /// backend that has not said otherwise has no icon surface to promise.
        fn supports_menu_item_icons(&self) -> bool {
            false
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
    };
}
pub(crate) use default_menu_dialog_view_widgets;

macro_rules! default_dock_panel {
    () => {
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
    };
}
pub(crate) use default_dock_panel;

macro_rules! default_container_and_chart_widgets {
    () => {
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
        /// Create heat map control.
        fn create_heatmap(
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
    };
}
pub(crate) use default_container_and_chart_widgets;

macro_rules! default_advanced_widgets {
    () => {
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
        ///
        /// # Where the control lives now
        ///
        /// The web controls (`web_engine_view`, `web_find_bar`, `web_history_panel`) ship in the
        /// optional `rust-widgets-webview` crate, which contributes them through
        /// `rust_widgets_webview::install(&mut WidgetFactory)`. This crate no longer carries the
        /// control, so a backend that mounts **by name** from
        /// [`WidgetFactory::shared`](crate::widget::capability::WidgetFactory::shared) answers `0`
        /// here — the same "no web support" answer
        /// [`ControlBackend::create_web_engine_page`] documents, rather than resolving
        /// `WidgetKind::WebEngineView` to the `media_player` that also carries that kind.
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
        ///
        /// `0` when the host has not installed `rust-widgets-webview`; see
        /// [`ControlBackend::create_web_view`].
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
        /// Create chart tooltip control.
        fn create_chart_tooltip(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create compass control.
        fn create_compass(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create dependency editor control.
        fn create_dependency_editor(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create diff file list control.
        fn create_diff_file_list(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create filter row control.
        fn create_filter_row(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create find bar control.
        fn create_find_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create web find bar control.
        fn create_web_find_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a drop-down menu panel.
        fn create_menu_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a speed dial.
        fn create_speed_dial(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a swipe-action background.
        fn create_swipe_actions(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a drawer section heading.
        fn create_drawer_section_header(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a layer panel.
        fn create_layer_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create a playlist panel.
        fn create_playlist_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create grid view header control.
        fn create_grid_view_header(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create kanban card view control.
        fn create_kanban_card_view(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create kanban column header control.
        fn create_kanban_column_header(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create legend control.
        fn create_legend(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create markdown outline panel control.
        fn create_markdown_outline_panel(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create range preset bar control.
        fn create_range_preset_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create scale bar control.
        fn create_scale_bar(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create shape canvas control.
        fn create_shape_canvas(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create swatch control.
        fn create_swatch(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tile loader control.
        fn create_tile_loader(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create timeline axis control.
        fn create_timeline_axis(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create toast host control.
        fn create_toast_host(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create tooltip manager control.
        fn create_tooltip_manager(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create zoom control.
        fn create_zoom_control(
            &self,
            _parent: ObjectId,
            _x: i32,
            _y: i32,
            _width: u32,
            _height: u32,
        ) -> ObjectId {
            0
        }
        /// Create zoom pan controller.
        fn create_zoom_pan_controller(
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
        /// Remove and return the next typed trigger event belonging to `widget_id`.
        ///
        /// The default is `None` -- a backend that keeps no addressable queue has nothing to offer here,
        /// which is truthful rather than a failure, and it is what the profiles with no queue rely on.
        fn pop_widget_trigger_event_for(&self, _widget_id: ObjectId) -> Option<WidgetTriggerEvent> {
            None
        }
        /// Inject a typed widget trigger event.
        fn inject_widget_trigger_event(
            &self,
            _widget_id: ObjectId,
            _kind: WidgetTriggerKind,
        ) -> bool {
            false
        }
    };
}
pub(crate) use default_advanced_widgets;

macro_rules! default_visibility {
    () => {
        /// Show widget.
        fn show_widget(&self, widget_id: ObjectId) {
            self.set_widget_visible(widget_id, true);
        }
        /// Hide widget.
        fn hide_widget(&self, widget_id: ObjectId) {
            self.set_widget_visible(widget_id, false);
        }
    };
}
pub(crate) use default_visibility;

macro_rules! default_geometry_queries {
    () => {
        /// Get widget geometry.
        fn get_widget_geometry(&self, _widget_id: ObjectId) -> Option<(i32, i32, u32, u32)> {
            None
        }

        /// Translates the geometry of every id in `ids` by `(dx, dy)`, returning how many resolved.
        ///
        /// # Why this is one call rather than a loop at the call site
        ///
        /// A scroll step moves every descendant of a viewport, and the default body below is the
        /// same read-then-write pair a caller would write itself. Putting it on the backend is what
        /// lets a host with a native window send one batched geometry message instead of two calls
        /// per descendant, without the scroll container knowing which kind of host it is (CP-001).
        /// A backend that cannot batch keeps the default and remains correct.
        fn translate_widget_geometries(
            &self,
            ids: &[ObjectId],
            dx: i32,
            dy: i32,
            moved_out: &mut usize,
        ) {
            if dx == 0 && dy == 0 {
                return;
            }
            for &id in ids {
                if let Some((x, y, width, height)) = self.get_widget_geometry(id) {
                    self.set_widget_geometry(
                        id,
                        x.saturating_add(dx),
                        y.saturating_add(dy),
                        width,
                        height,
                    );
                    *moved_out += 1;
                }
            }
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
    };
}
pub(crate) use default_geometry_queries;
