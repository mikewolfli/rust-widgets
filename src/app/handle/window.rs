// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window and lifecycle handles: window options/state, layout application and
//! the message-box handle.

use super::*;
use crate::core::{ObjectId, Orientation, Rect};
use crate::platform::WindowStateFlag;
use alloc::rc::Rc;
use core::cell::RefCell;
use std::collections::HashMap;

/// Updates the window-size mirror used by `apply_window_layout` and the geometry-aware
/// window helpers, without routing back into the platform.
pub(crate) fn set_window_size(window_id: ObjectId, width: u32, height: u32) {
    WINDOW_STATES.with(|map| {
        let mut map = map.borrow_mut();
        let state = map.entry(window_id).or_insert_with(Default::default);
        state.w = width;
        state.h = height;
    });
}

// ═══════════════════════════════════════════════════════════════
// WindowHandle
// ═══════════════════════════════════════════════════════════════

/// Type-safe handle for a top-level window.
///
/// In addition to the common widget operations, `WindowHandle` provides
/// factory methods for creating child widgets inside the window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowHandle {
    id: ObjectId,
}

/// Builder for a window created with attributes.
///
/// BLUE28 B28.41: `create_window` stays the five-argument historical entry point; a host that
/// needs a borderless/transparent/non-resizable window builds one of these and reads the
/// returned [`crate::platform::WindowCreation::unsupported`] list instead of guessing whether a
/// request was honoured.
#[derive(Debug, Clone, Default)]
pub struct WindowOptions {
    attributes: crate::platform::WindowAttributes,
}

impl WindowOptions {
    /// A window with `title` and the historical defaults (hidden until shown, 800×600 at 0,0).
    pub fn new(title: &str) -> Self {
        Self {
            attributes: crate::platform::WindowAttributes {
                title: title.to_string(),
                ..Default::default()
            },
        }
    }

    /// Sets the client size.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.attributes.size = Some((width, height));
        self
    }

    /// Sets the top-left position.
    pub fn position(mut self, x: i32, y: i32) -> Self {
        self.attributes.position = Some((x, y));
        self
    }

    /// Sets the minimum client size.
    pub fn min_size(mut self, width: u32, height: u32) -> Self {
        self.attributes.min_size = Some((width, height));
        self
    }

    /// Whether the user may resize the window.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.attributes.resizable = Some(resizable);
        self
    }

    /// Whether the OS draws a title bar and border.
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.attributes.decorations = Some(decorations);
        self
    }

    /// Whether the window starts visible.
    pub fn visible(mut self, visible: bool) -> Self {
        self.attributes.visible = Some(visible);
        self
    }

    /// Whether the window starts maximised.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.attributes.maximized = Some(maximized);
        self
    }

    /// Whether the window starts fullscreen.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.attributes.fullscreen = Some(fullscreen);
        self
    }

    /// The attributes built so far, for a caller that wants them directly.
    pub fn attributes(&self) -> &crate::platform::WindowAttributes {
        &self.attributes
    }

    /// Creates the window through the library's widget path and applies the attributes.
    ///
    /// The returned `WindowCreation::window` is the widget id (what every other API uses); the
    /// attributes are applied to the **host** window that widget is attached to. Attributes a
    /// backend cannot honour are returned in `unsupported`, never silently ignored.
    pub fn create(self) -> crate::platform::WindowCreation {
        let (x, y) = self.attributes.position.unwrap_or((0, 0));
        let (width, height) = self.attributes.size.unwrap_or((800, 600));
        let window = crate::create_window(&self.attributes.title, x, y, width, height);
        if window == 0 {
            return crate::platform::WindowCreation { window, unsupported: Vec::new() };
        }
        let host = crate::widget::runtime::host_window_for(window).unwrap_or(window);
        let unsupported =
            crate::platform::get_platform().apply_window_attributes(host, &self.attributes);
        crate::platform::WindowCreation { window, unsupported }
    }
}

impl WindowHandle {
    /// Rebuilds a handle from a raw object id.
    ///
    /// The id is **not** verified: nothing is looked up and no error is possible,
    /// so this can be given an id that addresses no window — or a widget of
    /// another kind. Every later operation on such a handle either fails
    /// quietly or returns `None`/`false`; nothing panics.
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Records a freshly created window's geometry in the handle-side mirror.
    ///
    /// # Why this is needed and why it lives here
    ///
    /// [`WindowHandle::set_layout`] needs the window's size to lay its children out, and
    /// it reads that size from the mirror this writes. Without the call, a window's layout
    /// could not be applied until something happened to call `set_geometry` — so
    /// `new_window(..)` followed by `set_layout(..)` silently did nothing, and the controls
    /// stayed at their creation coordinates. The mirror is private to this module, so the
    /// constructor path cannot write it directly.
    pub(crate) fn record_created_geometry(id: ObjectId, x: i32, y: i32, w: u32, h: u32) {
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(id).or_default();
            state.x = x;
            state.y = y;
            state.w = w;
            state.h = h;
        });
    }

    /// The underlying object id, for the raw `set_widget_*` functions and for
    /// `Platform` calls that have no handle wrapper.
    pub fn raw_id(&self) -> ObjectId {
        self.id
    }
}

impl WidgetHandle for WindowHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Override `set_geometry` to store window size in `WindowState` for
    /// later use by `center_on_screen()` and other geometry-aware methods.
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.id).or_insert_with(Default::default);
            state.x = x;
            state.y = y;
            state.w = w;
            state.h = h;
        });
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
        apply_window_layout(self.id);
    }

    fn on_click<F: FnMut() + Send + 'static>(&self, f: F) {
        register_click_callback(self.id, f);
    }

    fn on_value_changed<F: FnMut(&str) + 'static>(&self, f: F) {
        register_value_callback(self.id, Rc::new(RefCell::new(f)));
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {}
}

impl WindowHandle {
    /// Sets the window's title bar text and repaints.
    ///
    /// Routed through `crate::set_widget_text`, so on a non-window id it has no
    /// effect. On macOS the title is also what the Dock and the application menu
    /// show.
    pub fn set_title(&self, title: &str) {
        crate::set_widget_text(self.id, title);
    }

    // ── Child-widget factory methods ──────────────────────

    /// Creates a push button as a child of this window.
    ///
    /// `x`/`y` are in the window's client-area coordinates and `w`/`h` in pixels.
    /// The returned handle is the caller's way to reach the button from then on.
    pub fn new_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> ButtonHandle {
        ButtonHandle::from_raw(crate::create_button(self.id, text, x, y, w, h))
    }

    /// Creates a static text label as a child of this window.
    pub fn new_label(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LabelHandle {
        LabelHandle::from_raw(crate::create_label(self.id, text, x, y, w, h))
    }

    /// Creates a check box as a child of this window.
    ///
    /// Starts unchecked and single-state; use [`CheckBoxHandle::set_tristate`]
    /// for a three-state box.
    pub fn new_checkbox(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> CheckBoxHandle {
        CheckBoxHandle::from_raw(crate::create_checkbox(self.id, text, x, y, w, h))
    }

    /// Creates a radio button as a child of this window.
    ///
    /// Radio buttons are mutually exclusive within a group and independent
    /// across groups; see [`RadioButtonHandle::set_group`]. A new button starts
    /// with no group and unselected.
    pub fn new_radio_button(
        &self,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> RadioButtonHandle {
        RadioButtonHandle::from_raw(crate::create_radio_button(self.id, text, x, y, w, h))
    }

    /// Creates a single-line text entry as a child of this window, with `text` as
    /// its initial contents.
    pub fn new_line_edit(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LineEditHandle {
        LineEditHandle::from_raw(crate::create_line_edit(self.id, text, x, y, w, h))
    }

    /// Creates an empty drop-down list as a child of this window.
    ///
    /// It has no items, so nothing is selected; add them with
    /// [`ComboBoxHandle::add_item`].
    pub fn new_combo_box(&self, x: i32, y: i32, w: u32, h: u32) -> ComboBoxHandle {
        ComboBoxHandle::from_raw(crate::create_combo_box(self.id, x, y, w, h))
    }

    /// Creates an empty single-selection list box as a child of this window.
    pub fn new_list_box(&self, x: i32, y: i32, w: u32, h: u32) -> ListBoxHandle {
        ListBoxHandle::from_raw(crate::create_list_box(self.id, x, y, w, h))
    }

    /// Creates a horizontal slider as a child of this window.
    ///
    /// Orientation cannot be changed afterwards on most backends; use
    /// [`WindowHandle::new_slider_with_orientation`] when you need a vertical
    /// one.
    pub fn new_slider(&self, x: i32, y: i32, w: u32, h: u32) -> SliderHandle {
        SliderHandle::from_raw(crate::create_slider(self.id, x, y, w, h))
    }

    /// Create a slider whose orientation is chosen at creation time.
    ///
    /// Orientation is a **creation-time** property on every desktop toolkit, not a
    /// settable attribute: Win32 fixes it with the `TBS_VERT` window style (there
    /// is no `TBM_*` message to change it afterwards), AppKit picks the track
    /// direction when the `NSSlider` is configured, and only GTK exposes a live
    /// `set_orientation`. Routing it through creation is therefore the only shape
    /// that can be honoured consistently on all three, instead of a `set_` method
    /// that silently does nothing on two of them.
    pub fn new_slider_with_orientation(
        &self,
        orientation: Orientation,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> SliderHandle {
        let handle = SliderHandle::from_raw(crate::create_slider(self.id, x, y, w, h));
        crate::platform::get_platform().set_slider_orientation(handle.raw_id(), orientation);
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(handle.raw_id()).or_default().orientation = orientation;
        });
        handle
    }

    /// Creates a progress bar as a child of this window.
    ///
    /// Starts in the determinate state; [`ProgressBarHandle::set_indeterminate`]
    /// switches it to a busy animation for work of unknown duration.
    pub fn new_progress_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ProgressBarHandle {
        ProgressBarHandle::from_raw(crate::create_progress_bar(self.id, x, y, w, h))
    }

    /// Creates a tab widget as a child of this window.
    ///
    /// Pages are child ids added with [`TabWidgetHandle::add_tab`]; the selected page is
    /// laid out to the content rect by the tab widget itself.
    pub fn new_tab_widget(&self, x: i32, y: i32, w: u32, h: u32) -> TabWidgetHandle {
        TabWidgetHandle::from_raw(crate::create_tab_widget(self.id, x, y, w, h))
    }

    /// Creates a grid widget as a child of this window.
    ///
    /// This is the **visual** row/column matrix, distinct from `GridLayout` placement.
    pub fn new_grid_widget(&self, x: i32, y: i32, w: u32, h: u32) -> GridWidgetHandle {
        GridWidgetHandle::from_raw(crate::create_grid_widget(self.id, x, y, w, h))
    }

    /// Creates a splitter as a child of this window.
    pub fn new_splitter(&self, x: i32, y: i32, w: u32, h: u32) -> SplitterHandle {
        SplitterHandle::from_raw(crate::create_splitter(self.id, x, y, w, h))
    }

    /// Creates a generic dialog as a child of this window.
    pub fn new_dialog(&self, title: &str, x: i32, y: i32, w: u32, h: u32) -> DialogHandle {
        DialogHandle::from_raw(crate::create_dialog(self.id, title, x, y, w, h))
    }

    /// Mount a widget onto a surface in this window.
    ///
    /// # What this is for
    ///
    /// Widgets paint themselves through `Draw` (`CodeEditor`, `ColorPicker`,
    /// `GanttWidget`, `TerminalView`, …). This method hands the
    /// widget to a surface the backend provides that repaints it whenever the
    /// window is invalidated, and forwards pointer/keyboard input back into the
    /// widget.
    ///
    /// # Cross-platform by construction
    ///
    /// Which surface that is (a child window, a drawing area, a view) is decided
    /// inside `src/platform/` and is deliberately **not** part of this contract.
    /// The same call works on every backend; when one cannot host a surface it
    /// says so through `Err`, rather than the caller pre-checking an OS.
    ///
    /// # Ownership
    ///
    /// The window takes ownership through the process-wide widget registry; the
    /// returned handle can move, resize and unmount it.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` when the backend mounted the widget, `Err(reason)` when it
    /// could not — a backend with no surface to offer (see
    /// `Platform::supports_surfaces`), an off-UI-thread call, or an unknown
    /// parent. Callers must surface the error rather than showing a blank window.
    ///
    /// ```no_run
    /// use rust_widgets::app::{App, WidgetHandle};
    /// use rust_widgets::core::Rect;
    /// use rust_widgets::widget::special_widgets::code_editor::{CodeEditor, CodeEditorConfig};
    ///
    /// let mut app = App::new();
    /// app.init();
    /// let win = app.new_window("Editor", 0, 0, 900, 600);
    /// let editor = CodeEditor::with_config(Rect::new(0, 0, 900, 600), CodeEditorConfig::new())
    ///     .expect("valid config");
    /// win.mount_surface(Box::new(editor), Rect::new(0, 0, 900, 600))
    ///     .expect("backend must be able to mount widget surfaces");
    /// win.show();
    /// app.run();
    /// ```
    pub fn mount_surface(
        &self,
        widget: Box<dyn crate::widget::Widget>,
        rect: Rect,
    ) -> Result<SurfaceHandle, SurfaceMountError> {
        // One implementation of register → mount → roll back, shared with
        // `create_widget_of_kind`, so the two creation paths cannot disagree about
        // ownership or error reporting.
        let id = crate::mount_widget_object(self.id, widget, rect)?;
        Ok(SurfaceHandle { id })
    }

    /// Mount a widget, creating it from the widget factory by name.
    ///
    /// Convenience wrapper over [`WindowHandle::mount_surface`] for callers
    /// that already address widgets by their capability name (`"code_editor"`,
    /// `"color_picker"`, …).
    pub fn mount_widget_by_name(
        &self,
        name: &str,
        rect: Rect,
        text: &str,
    ) -> Result<SurfaceHandle, SurfaceMountError> {
        let factory = crate::widget::WidgetFactory::shared();
        let widget =
            factory.create(name, rect, text).ok_or(SurfaceMountError::UnknownWidgetName)?;
        self.mount_surface(widget, rect)
    }

    /// Deprecated alias of [`WindowHandle::mount_surface`].
    #[deprecated(
        note = "renamed to `mount_surface`; 'custom' named a mechanism that no longer exists"
    )]
    pub fn mount_custom_widget(
        &self,
        widget: Box<dyn crate::widget::Widget>,
        rect: Rect,
    ) -> Result<SurfaceHandle, SurfaceMountError> {
        self.mount_surface(widget, rect)
    }

    /// Creates a container panel as a child of this window.
    ///
    /// A panel is a layout surface rather than a control: it groups children and
    /// can carry a title. The handle's geometry is recorded so
    /// [`PanelHandle::set_geometry`] and [`PanelHandle::set_layout`] can
    /// re-lay out its contents.
    pub fn new_panel(&self, x: i32, y: i32, w: u32, h: u32) -> PanelHandle {
        let panel = PanelHandle::from_raw(crate::create_panel(self.id, x, y, w, h));
        PANEL_STATES.with(|map| {
            map.borrow_mut().insert(panel.raw_id(), PanelState::with_geometry(x, y, w, h));
        });
        panel
    }

    /// Create a new frame (group box).
    ///
    /// Internally, a Frame is backed by a Panel widget (`create_panel`).
    /// The terminology difference is cosmetic — `new_frame` exists for API
    /// clarity when the widget is used as a visual frame/group box.
    pub fn new_frame(&self, x: i32, y: i32, w: u32, h: u32) -> FrameHandle {
        let frame = FrameHandle::from_raw(crate::create_panel(self.id, x, y, w, h));
        PANEL_STATES.with(|map| {
            map.borrow_mut().insert(frame.raw_id(), PanelState::with_geometry(x, y, w, h));
        });
        frame
    }

    /// Creates a group box as a child of this window.
    ///
    /// A group box is a panel with a title, so it is the same widget as
    /// [`WindowHandle::new_frame`] with the title already set. Use this spelling when the
    /// control's grouping role is what matters; use `new_frame` when its border is.
    pub fn new_group_box(&self, title: &str, x: i32, y: i32, w: u32, h: u32) -> FrameHandle {
        let frame = self.new_frame(x, y, w, h);
        frame.set_title(title);
        frame
    }

    /// Creates a spin box as a child of this window: a numeric field with an
    /// increment/decrement pair.
    ///
    /// Starts at 0 with the full numeric range; constrain it with
    /// [`SpinBoxHandle::set_range`] and label it with
    /// [`SpinBoxHandle::set_prefix`]/[`SpinBoxHandle::set_suffix`].
    pub fn new_spin_box(&self, x: i32, y: i32, w: u32, h: u32) -> SpinBoxHandle {
        SpinBoxHandle::from_raw(crate::create_spin_box(self.id, x, y, w, h))
    }

    /// Create a native menu bar for this window.
    ///
    /// On macOS the bar also becomes the application's main menu when attached
    /// with [`WindowHandle::attach_menu_bar`]. Menus are added with
    /// [`WindowHandle::new_menu`] and items with
    /// [`WindowHandle::new_menu_item`].
    pub fn new_menu_bar(&self, x: i32, y: i32, w: u32, h: u32) -> MenuBarHandle {
        MenuBarHandle::from_raw(crate::create_menu_bar(self.id, x, y, w, h))
    }

    /// Attach a menu bar to this window.
    ///
    /// Returns `false` when the backend cannot attach it.
    pub fn attach_menu_bar(&self, menu_bar: &MenuBarHandle) -> bool {
        crate::attach_menu_bar_to_window(self.id, menu_bar.raw_id())
    }

    /// Add a top-level menu to a menu bar.
    ///
    /// # Parent
    ///
    /// `menu_bar` must be the bar returned by [`WindowHandle::new_menu_bar`] —
    /// **not** this window. A menu's parent is structurally its bar, and the
    /// macOS backend only attaches the submenu when the parent is a
    /// `HandleKind::MenuBar` (or a nested `Menu`); passing a window does nothing
    /// and the menu silently never appears.
    pub fn new_menu(
        &self,
        menu_bar: &MenuBarHandle,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> MenuHandle {
        MenuHandle::from_raw(crate::create_menu(menu_bar.raw_id(), text, x, y, w, h))
    }

    /// Add an item to a menu, with a shortcut typed rather than spelled.
    ///
    /// This is the portable form: pass [`crate::shortcut::Shortcut::primary`] and
    /// the item shows `⌘S` on macOS and `Ctrl+S` on Windows/Linux. Prefer it over
    /// [`WindowHandle::new_menu_item`], which takes an already-formatted display
    /// string and therefore cannot adapt to the host's notation.
    ///
    /// ```rust,no_run
    /// use rust_widgets::app::{App, WidgetHandle};
    /// use rust_widgets::shortcut::{Key, Shortcut};
    ///
    /// let mut app = App::new();
    /// app.init();
    /// let win = app.new_window("Editor", 0, 0, 800, 600);
    /// let bar = win.new_menu_bar(0, 0, 0, 0);
    /// let file = win.new_menu(&bar, "File", 0, 0, 0, 0);
    /// // One declaration, native spelling on every desktop OS.
    /// let save = win.new_menu_item_with_shortcut(&file, "Save", Some(Shortcut::primary(Key::S)));
    /// assert_ne!(save.raw_id(), 0);
    /// ```
    pub fn new_menu_item_with_shortcut(
        &self,
        menu: &MenuHandle,
        text: &str,
        shortcut: Option<crate::shortcut::Shortcut>,
    ) -> MenuItemHandle {
        let rendered = shortcut.map(|shortcut| crate::format_shortcut(&shortcut));
        self.new_menu_item(menu, text, rendered.as_deref())
    }

    /// Add an item to a menu, optionally with a keyboard shortcut such as
    /// `"Cmd+Z"`. Returns the item handle, whose id can be fed to
    /// `Platform::poll_menu_triggered` to detect activation.
    ///
    /// The shortcut is the **display text**: what the user sees is what you pass.
    /// Use [`WindowHandle::new_menu_item_with_shortcut`] when you want the host
    /// OS to choose the notation from a typed
    /// [`crate::shortcut::Shortcut`].
    pub fn new_menu_item(
        &self,
        menu: &MenuHandle,
        text: &str,
        shortcut: Option<&str>,
    ) -> MenuItemHandle {
        MenuItemHandle::from_raw(crate::menu_add_item(menu.raw_id(), text, shortcut))
    }

    /// Add a menu item that would carry an icon, keeping the text when the platform cannot show one.
    ///
    /// # Why this exists instead of an `icon` parameter on `new_menu_item`
    ///
    /// [`Platform::menu_add_item`](crate::platform::Platform::menu_add_item) has no icon parameter, so
    /// an icon handed to a native menu bar can only be (a) presented, (b) reported unsupported, or
    /// (c) silently dropped. (c) is the one blue27 §19.11 forbids, because the item then looks exactly
    /// like one whose icon failed to resolve — §19.13 case I15 requires those two to be
    /// distinguishable. So this method asks
    /// [`menu_item_icons_supported`](crate::menu_item_icons_supported) and, when the
    /// answer is `false`, creates the item with its **text** and names the unsupported presentation in
    /// the log. The item is always created: losing a menu entry because of a decoration would be far
    /// worse than losing the decoration.
    ///
    /// A caller that wants to avoid the whole path asks the query itself and never builds the icon.
    pub fn new_menu_item_with_icon(
        &self,
        menu: &MenuHandle,
        text: &str,
        shortcut: Option<&str>,
        icon: Option<crate::widget::IconSource>,
    ) -> MenuItemHandle {
        if icon.is_some() && !crate::menu_item_icons_supported() {
            log::warn!(
                "native menu item {text:?} was given an icon, but this platform's menu bar has no \
                 icon surface; the item was created with its text only (this is an unsupported \
                 presentation, not a load failure)"
            );
        }
        self.new_menu_item(menu, text, shortcut)
    }

    /// Create a tool bar strip for this window.
    pub fn new_tool_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ToolBarHandle {
        ToolBarHandle::from_raw(crate::create_tool_bar(self.id, x, y, w, h))
    }

    /// Create a status bar for this window.
    pub fn new_status_bar(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> StatusBarHandle {
        StatusBarHandle::from_raw(crate::create_status_bar(self.id, text, x, y, w, h))
    }

    /// Creates a single-selection table of rows and columns as a child of this
    /// window.
    ///
    /// The view starts with no columns and no model, so it has nothing to show;
    /// build it with [`ListViewHandle::add_column`] and
    /// [`ListViewHandle::set_model`].
    pub fn new_list_view(&self, x: i32, y: i32, w: u32, h: u32) -> ListViewHandle {
        ListViewHandle::from_raw(crate::create_list_view(self.id, x, y, w, h))
    }

    /// Creates a scrollable viewport as a child of this window.
    ///
    /// Its content is larger than the viewport and is reached by scrolling; the
    /// extent of that content has to be declared with
    /// [`ScrollAreaHandle::set_content_size`] or no scrollbar can appear.
    pub fn new_scroll_area(&self, x: i32, y: i32, w: u32, h: u32) -> ScrollAreaHandle {
        ScrollAreaHandle::from_raw(crate::create_scroll_area(self.id, x, y, w, h))
    }

    /// Creates a modal message box as a child of this window.
    ///
    /// `title` is the box's own caption; `text` is the body. It is created
    /// hidden — call [`MessageBoxHandle::show_modal`] to display it. Note that
    /// `MessageBoxHandle` deliberately does not implement the geometry and
    /// enable/disable operations the other handles have, so the position and size
    /// passed here are the ones the backend chooses to honour.
    pub fn new_message_box(
        &self,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> MessageBoxHandle {
        MessageBoxHandle::from_raw(crate::create_message_box(self.id, title, text, x, y, w, h))
    }

    /// Apply a layout manager to this window.
    ///
    /// The layout is stored internally and used to reposition children.
    /// Only one layout can be active at a time; calling this again replaces it.
    ///
    /// ```rust,no_run
    /// use rust_widgets::app::{App, WidgetHandle};
    /// use rust_widgets::layout::{BoxLayout, Layout, Orientation};
    ///
    /// let mut app = App::new();
    /// app.init();
    /// let win = app.new_window("Layout Demo", 0, 0, 400, 300);
    /// let btn1 = win.new_button("Left", 0, 0, 0, 0);
    /// let btn2 = win.new_button("Right", 0, 0, 0, 0);
    ///
    /// let mut layout = BoxLayout::new(Orientation::Horizontal, 8, 4);
    /// layout.add_widget(btn1.raw_id(), 1);
    /// layout.add_widget(btn2.raw_id(), 1);
    /// win.set_layout(layout);
    /// ```
    pub fn set_layout(&self, layout: impl crate::layout::Layout + 'static) {
        LAYOUTS.with(|map| {
            map.borrow_mut().insert(self.id, Box::new(layout));
        });
        apply_window_layout(self.id);
    }

    /// Reports the device scale the window's last layout pass was **asked with**.
    ///
    /// # Why this exists
    ///
    /// A `BoxLayout`, `FlexLayout` or `WrapLayout` at a window's top level scales its
    /// gaps and margins by the device's text/DPI scale, and applies a minimum touch
    /// target. Whether that happened is therefore a property of *which entry point the
    /// window path called*, and on a host with no text preference and no DPI scaling the
    /// resulting rectangles are byte-identical either way — so geometry alone cannot
    /// distinguish "the context was delivered" from "the context was dropped".
    ///
    /// This accessor answers that question directly instead of by inference, and it is
    /// the whole reason it is public rather than test-only: a host diagnosing "my layout
    /// does not follow the user's text-size setting" needs exactly this fact, and
    /// `platform::profile::text_scale()` alone cannot tell it whether the layout was
    /// told.
    ///
    /// Returns `None` when no layout is registered for this window, or when the last pass
    /// ran without a context (`LayoutContext` is not `Copy`-reportable through a trait
    /// object, so the scale is published rather than the struct).
    pub fn layout_context_scale(&self) -> Option<f32> {
        LAYOUT_CONTEXT_SCALES.with(|map| map.borrow().get(&self.id).copied())
    }
}

thread_local! {
    pub(crate) static LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
}

// The device scale each window's **last** layout pass was asked with.
//
// Separate from `LAYOUTS` so the layout itself stays a pure function of its input: a
// layout has no business remembering the device it was asked for, and
// `WindowHandle::layout_context_scale` needs the fact even when the caller's layout is an
// opaque `Box<dyn Layout>` that never implemented `update_with_context`.
thread_local! {
    pub(crate) static LAYOUT_CONTEXT_SCALES: RefCell<HashMap<ObjectId, f32>> = RefCell::new(HashMap::new());
}

/// Apply a window's current layout to all child widget geometries.
///
/// Layouts use the window client area as their coordinate space. Geometry
/// updates happen after the layout borrow is released so a backend callback
/// cannot re-enter the layout map while it is borrowed.
///
/// # Why the layout is asked with a device context
///
/// The window layout is the one place every control's placement is decided, so it is the
/// place a device fact like text scale and the minimum touch target has to reach. This
/// used to call [`Layout::update`](crate::layout::Layout::update), which carries no
/// context, so the three layouts that *do* honour a context — `BoxLayout`, `FlexLayout`,
/// `WrapLayout` — laid the window's own children out with unscaled gaps and no touch
/// floor. The same layouts were already being asked correctly one level down, because
/// [`CompositeBuilder::arrange`](crate::widget::composite::CompositeBuilder::arrange)
/// routes through `arrange_with_context`; a window whose top level is a `BoxLayout`
/// therefore disagreed with the composites inside it about the same device.
///
/// [`LayoutContext::default`](crate::layout::types::LayoutContext::default) is not a
/// literal: it reads this device's text scale and recommended touch target, which is what
/// makes the fix a device fact rather than a constant. A layout that does not override
/// `update_with_context` inherits the trait default, which forwards to `update`, so every
/// layout that has not opted in lays out exactly as it did before.
pub(crate) fn apply_window_layout(window_id: ObjectId) {
    let Some((width, height)) =
        WINDOW_STATES.with(|map| map.borrow().get(&window_id).map(|state| (state.w, state.h)))
    else {
        return;
    };

    let context = crate::layout::types::LayoutContext::default();
    // Published before the pass so a diagnostic can read the scale that was actually used
    // even when the layout ignores the context entirely.
    LAYOUT_CONTEXT_SCALES.with(|map| {
        map.borrow_mut().insert(window_id, context.layout_scale.max(context.font_scale));
    });
    let child_geometries = LAYOUTS.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&window_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update_with_context(
            Rect::new(0, 0, width, height),
            &context,
            &mut |widget_id, geometry| {
                geometries.push((widget_id, geometry));
            },
        );
        geometries
    });

    for (widget_id, geometry) in child_geometries {
        crate::set_widget_geometry(
            widget_id,
            geometry.x,
            geometry.y,
            geometry.width,
            geometry.height,
        );
        // A child that hosts its own layout has just been given a new box, so its
        // children have to be laid out again. Without this a panel positioned by the
        // window layout keeps whatever child geometry it computed against its original
        // (usually zero-area) rect — the children exist but are never placed.
        if widget_hosts_a_layout(widget_id) {
            apply_panel_geometry(widget_id, geometry);
        } else if crate::layout::declarative::has_layout(widget_id) {
            // The declarative (JSON / C ABI) registry is a second layout store. A
            // container loaded from a document is placed by the window just like a
            // panel, so its own layout must be asked against the new rectangle too.
            crate::layout::declarative::apply_layout(widget_id, geometry);
        }
    }
}

/// Whether `id` has a child layout registered on it.
pub(crate) fn widget_hosts_a_layout(id: ObjectId) -> bool {
    PANEL_LAYOUTS.try_with(|map| map.borrow().contains_key(&id)).unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════
// MessageBoxHandle – custom, NOT from macro
// ═══════════════════════════════════════════════════════════════

/// Type-safe handle for a modal message-box dialog.
///
/// Unlike normal widgets, a message-box exposes only dialog-oriented
/// operations — it does **not** support `set_text`, `enable`, or
/// `set_geometry` because those semantics do not apply.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageBoxHandle {
    id: ObjectId,
}

impl MessageBoxHandle {
    /// Rebuilds a handle from a raw object id.
    ///
    /// The id is not verified, so this can name a widget of another kind or
    /// nothing at all; the methods below then do nothing.
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// The underlying object id.
    pub fn raw_id(&self) -> ObjectId {
        self.id
    }

    /// Show the message-box modally.
    ///
    /// Shows the box and pushes it onto the modal stack, so input outside the box's
    /// subtree is blocked until [`MessageBoxHandle::close`] (or a dismissal) pops it.
    pub fn show_modal(&self) {
        crate::show_widget(self.id);
        crate::widget::runtime::enter_modal(self.id);
    }

    /// Dismiss the message-box.
    ///
    /// Hides the box and removes it from the modal stack, restoring input to
    /// whatever it was blocking.
    pub fn close(&self) {
        crate::widget::runtime::exit_modal(self.id);
        crate::hide_widget(self.id);
    }

    /// Update the dialog title.
    pub fn set_title(&self, title: &str) {
        crate::set_widget_text(self.id, title);
    }
}

impl WidgetHandle for MessageBoxHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    fn on_click<F: FnMut() + Send + 'static>(&self, f: F) {
        register_click_callback(self.id, f);
    }

    fn on_value_changed<F: FnMut(&str) + 'static>(&self, f: F) {
        register_value_callback(self.id, Rc::new(RefCell::new(f)));
    }
}

impl Drop for MessageBoxHandle {
    fn drop(&mut self) {}
}

// ═══════════════════════════════════════════════════════════════
// ComboBoxHandle – extended with combo-specific operations
// ═══════════════════════════════════════════════════════════════
// ═══════════════════════════════════════════════════════════════
// WindowHandle – extended state (new methods beyond factories)
// ═══════════════════════════════════════════════════════════════

#[derive(Clone)]
pub(crate) struct WindowState {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) icon: String,
    pub(crate) min_w: u32,
    pub(crate) min_h: u32,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) fullscreen: bool,
    pub(crate) resizable: bool,
    pub(crate) decorated: bool,
}

impl std::fmt::Debug for WindowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowState")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("w", &self.w)
            .field("h", &self.h)
            .field("icon", &self.icon)
            .field("min_w", &self.min_w)
            .field("min_h", &self.min_h)
            .field("maximized", &self.maximized)
            .field("minimized", &self.minimized)
            .field("fullscreen", &self.fullscreen)
            .field("resizable", &self.resizable)
            .field("decorated", &self.decorated)
            .finish()
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 800,
            h: 600,
            icon: String::new(),
            min_w: 0,
            min_h: 0,
            maximized: false,
            minimized: false,
            fullscreen: false,
            resizable: true,
            decorated: true,
        }
    }
}

thread_local! {
    pub(crate) static WINDOW_STATES: RefCell<HashMap<ObjectId, WindowState>> = RefCell::new(HashMap::new());

    /// Legacy `Rc<RefCell<dyn FnMut()>>` close callbacks, keyed by window id.
    ///
    /// A `ClickCallback` is not `Send`, so it cannot live in the `FnMut() + Send` slot the widget's
    /// `closed` signal needs. It is parked here and looked up from the (sendable) slot, on the
    /// thread the window is used from. See [`WindowHandle::on_close`].
    pub(crate) static WINDOW_CLOSE_CALLBACKS: RefCell<HashMap<ObjectId, ClickCallback>> =
        RefCell::new(HashMap::new());
}

/// # Window-specific state operations
impl WindowHandle {
    /// Return the window title.
    pub fn title(&self) -> String {
        crate::get_widget_text(self.raw_id())
    }

    /// Set the window icon from a file path.
    ///
    /// Pushed to the OS where the backend has a per-window icon concept: Win32 `WM_SETICON`
    /// (`LoadImageW`) and GTK `set_icon_from_file`. The macOS backends answer `false` — an
    /// `NSWindow` has no icon; the Dock/window-switcher icon is the application bundle's
    /// `CFBundleIconFile`. The value is mirrored only when the platform accepted it, so
    /// [`WindowHandle::icon`] cannot report a path the OS rejected.
    pub fn set_icon(&self, path: &str) -> bool {
        let pushed = crate::platform::get_platform().set_window_icon(self.raw_id(), path);
        if pushed {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).icon =
                    path.to_owned();
            });
        }
        pushed
    }

    /// Set the minimum window size.
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_min_size`]:
    /// `setContentMinSize:` on both macOS backends, the `WM_GETMINMAXINFO` handler on Windows,
    /// and `set_geometry_hints(GDK_HINT_MIN_SIZE)` on GTK. Wayland, Android, iOS, HarmonyOS,
    /// Redox and Web record the value but return `false` from the platform setter (they have no
    /// client-side min-size protocol in this backend), so the mirror is populated only when the
    /// OS accepted the request.
    pub fn set_min_size(&self, w: u32, h: u32) -> bool {
        let pushed = crate::platform::get_platform().set_window_min_size(self.raw_id(), w, h);
        if pushed {
            WINDOW_STATES.with(|map| {
                let mut map = map.borrow_mut();
                let state = map.entry(self.raw_id()).or_default();
                state.min_w = w;
                state.min_h = h;
            });
        }
        pushed
    }

    /// Read the minimum window size, if one was set.
    ///
    /// # The mirror is a fallback, not a shadow
    ///
    /// The backend is authoritative. The in-process mirror is consulted only when the backend
    /// reports `None` *and* `set_min_size` succeeded earlier in this process — the same shape
    /// as [`WindowHandle::is_maximized`]. This method previously read the platform exclusively,
    /// so on a backend that does not implement `window_min_size` (`linux-gtk`, `wayland`,
    /// `android`, `ios`, `wasm`, `harmony`) a window whose `set_min_size` had returned `true`
    /// still reported `None` here — the mirror field was written and never read by anything.
    ///
    /// A *failed* `set_min_size` does not populate the mirror, so this cannot report a value
    /// the platform refused. `(0, 0)` means "no minimum was ever accepted" and is what a fresh
    /// window reports.
    pub fn min_size(&self) -> Option<(u32, u32)> {
        if let Some(size) = crate::platform::get_platform().window_min_size(self.raw_id()) {
            return Some(size);
        }
        self.mirrored_min_size()
    }

    /// Read the icon path from the in-process mirror, if one was accepted.
    ///
    /// # Why a rejected icon stays unreadable
    ///
    /// `set_icon` records the mirror only after the platform accepted the path. An earlier
    /// version wrote the mirror first and unconditionally, so a window whose icon failed to
    /// load still reported the rejected path from a getter that never consulted it — the field
    /// was dead, and had it been read it would have been wrong. Recording on success is what
    /// makes the fallback honest.
    pub fn icon(&self) -> Option<String> {
        if let Some(path) = crate::platform::get_platform().window_icon(self.raw_id()) {
            return Some(path);
        }
        WINDOW_STATES.with(|map| {
            map.borrow()
                .get(&self.raw_id())
                .map(|state| state.icon.clone())
                .filter(|path| !path.is_empty())
        })
    }

    /// Read the mirrored minimum size from the in-process state.
    ///
    /// `None` when no successful `set_min_size` was recorded, so a caller can distinguish
    /// "the platform has no answer and nothing was ever set" from a real `(w, h)`.
    fn mirrored_min_size(&self) -> Option<(u32, u32)> {
        WINDOW_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).and_then(|state| {
                if state.min_w == 0 && state.min_h == 0 {
                    None
                } else {
                    Some((state.min_w, state.min_h))
                }
            })
        })
    }

    /// Maximize or restore the window.
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`]: `zoom:` and
    /// `isZoomed` on macOS, `ShowWindow(SW_MAXIMIZE/SW_RESTORE)` on Windows, and
    /// `maximize()`/`unmaximize()` on GTK. Wayland and the state-only mobile backends record the
    /// value but cannot change a native window here, and `is_maximized` falls back to that record.
    pub fn set_maximized(&self, maximized: bool) {
        // The mirror records only what the platform accepted, exactly as `set_min_size` and
        // `set_icon` do — otherwise a flag set on a window that does not exist is reported as
        // applied by the `is_*` getter, which falls back to the mirror when the platform answers
        // `None`. The platform's `bool` result was previously discarded.
        let applied = crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Maximized,
            maximized,
        );
        if applied {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).maximized =
                    maximized;
            });
        }
    }

    /// Return whether the window is maximized.
    pub fn is_maximized(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Maximized)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.maximized, false))
    }

    /// Minimize or restore the window.
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`]
    /// (`miniaturize:`/`deminiaturize:` on macOS, `SW_MINIMIZE`/`SW_RESTORE` on
    /// Windows, `iconify()`/`deiconify()` on GTK).
    pub fn set_minimized(&self, minimized: bool) {
        // The mirror records only what the platform accepted, exactly as `set_min_size` and
        // `set_icon` do — otherwise a flag set on a window that does not exist is reported as
        // applied by the `is_*` getter, which falls back to the mirror when the platform answers
        // `None`. The platform's `bool` result was previously discarded.
        let applied = crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Minimized,
            minimized,
        );
        if applied {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).minimized =
                    minimized;
            });
        }
    }

    /// Return whether the window is minimized.
    pub fn is_minimized(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Minimized)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.minimized, false))
    }

    /// Set fullscreen mode.
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`]
    /// (`toggleFullScreen:` on macOS, frame-style manipulation on Windows,
    /// `fullscreen()`/`unfullscreen()` on GTK).
    pub fn set_fullscreen(&self, fullscreen: bool) {
        // The mirror records only what the platform accepted, exactly as `set_min_size` and
        // `set_icon` do — otherwise a flag set on a window that does not exist is reported as
        // applied by the `is_*` getter, which falls back to the mirror when the platform answers
        // `None`. The platform's `bool` result was previously discarded.
        let applied = crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Fullscreen,
            fullscreen,
        );
        if applied {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).fullscreen =
                    fullscreen;
            });
        }
    }

    /// Return whether the window is fullscreen.
    pub fn is_fullscreen(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Fullscreen)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.fullscreen, false))
    }

    /// Set whether the window is resizable.
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`], which
    /// toggles `NSWindowStyleMaskResizable` on macOS, `WS_THICKFRAME` on
    /// Windows, and `set_resizable` on GTK.
    pub fn set_resizable(&self, resizable: bool) {
        // The mirror records only what the platform accepted, exactly as `set_min_size` and
        // `set_icon` do — otherwise a flag set on a window that does not exist is reported as
        // applied by the `is_*` getter, which falls back to the mirror when the platform answers
        // `None`. The platform's `bool` result was previously discarded.
        let applied = crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Resizable,
            resizable,
        );
        if applied {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).resizable =
                    resizable;
            });
        }
    }

    /// Return whether the window is resizable.
    pub fn is_resizable(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Resizable)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.resizable, true))
    }

    /// Set whether the window has window decorations (title bar, borders).
    ///
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`], which
    /// toggles `NSWindowStyleMaskTitled` on macOS, `WS_CAPTION` on Windows, and
    /// `set_decorated` on GTK.
    pub fn set_decorated(&self, decorated: bool) {
        // The mirror records only what the platform accepted, exactly as `set_min_size` and
        // `set_icon` do — otherwise a flag set on a window that does not exist is reported as
        // applied by the `is_*` getter, which falls back to the mirror when the platform answers
        // `None`. The platform's `bool` result was previously discarded.
        let applied = crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Decorated,
            decorated,
        );
        if applied {
            WINDOW_STATES.with(|map| {
                map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).decorated =
                    decorated;
            });
        }
    }

    /// Is the window decorated?
    pub fn is_decorated(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Decorated)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.decorated, true))
    }

    /// Read one window flag from the in-process mirror.
    ///
    /// Used as the fallback when the backend reports `None` — which happens when
    /// the window was created off the UI thread and has no native object to
    /// query. `default` is the value a fresh OS window would have.
    fn mirrored_flag(&self, pick: fn(&WindowState) -> bool, default: bool) -> bool {
        WINDOW_STATES.with(|map| map.borrow().get(&self.raw_id()).map(pick).unwrap_or(default))
    }

    /// Register a callback invoked when the window is about to close.
    ///
    /// Connects to the widget's own `closed` signal through `register_close_callback`, so the
    /// callback fires when the **widget** closes (a title-bar close, a `Window::close`, or the
    /// published `close` command), not only when [`WindowHandle::close`] is called.
    ///
    /// # Why the `Rc` is parked in a thread-local
    ///
    /// `ClickCallback` is `Rc<RefCell<dyn FnMut()>>` — not `Send` — while the signal slot must be
    /// `FnMut() + Send`. Rather than change this legacy public signature, the `Rc` is stored in a
    /// thread-local keyed by window id and the (sendable) slot looks it up on the same thread the
    /// window lives on, which is where a close is emitted. A missing entry is a no-op, not a panic.
    pub fn on_close(&self, callback: ClickCallback) {
        WINDOW_CLOSE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, callback);
        });
        let id = self.id;
        register_close_callback(id, move || {
            WINDOW_CLOSE_CALLBACKS.with(|map| {
                if let Some(cb) = map.borrow().get(&id) {
                    if let Ok(mut cb) = cb.try_borrow_mut() {
                        cb();
                    }
                }
            });
        });
    }

    /// Programmatically close the window.
    ///
    /// Emits the widget's `closed` signal (which is what a registered [`Self::on_close`] — and the
    /// JSON/designer `closed` route — is connected to) and then hides the window. The side-table
    /// callback this used to read directly is gone: the signal is the one path a close is announced
    /// on, so there is no second table to keep in step.
    pub fn close(&self) {
        crate::close_widget(self.raw_id());
        crate::hide_widget(self.raw_id());
    }

    /// Center the window on the screen.
    ///
    /// The target is the primary display's **work area**, read through
    /// [`Platform::displays`](crate::platform::Platform::displays), so a window centres inside the
    /// space the OS actually lets it occupy (a dock or taskbar is excluded) and on a secondary
    /// monitor's coordinates rather than at the virtual origin. This used to hard-code a
    /// 1920×1080 screen at `(0, 0)`, which placed the window outside the visible area on a
    /// HiDPI or multi-monitor desktop.
    ///
    /// A backend that reports no displays has no geometry to centre on; the historical
    /// 1920×1080 fallback is kept for it and is the only remaining use of that number.
    pub fn center_on_screen(&self) {
        // Query the stored window geometry to preserve current size.
        let (win_w, win_h) = WINDOW_STATES.with(|map| {
            let state = map.borrow().get(&self.raw_id()).cloned().unwrap_or_default();
            (state.w, state.h)
        });
        let (centred_x, centred_y) = centred_position(win_w, win_h);
        crate::set_widget_geometry(self.raw_id(), centred_x, centred_y, win_w, win_h);
        // Re-centring *is* a position change, so the mirror has to follow: leaving `x`/`y`
        // stale made a later `center_on_screen` (or any reader of the mirror) compute from a
        // position the window no longer had. The size fields already round-tripped through
        // `set_widget_geometry`'s mirror update; the position did not.
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_insert_with(Default::default);
            state.x = centred_x;
            state.y = centred_y;
            state.w = win_w;
            state.h = win_h;
        });
    }
}

/// The top-left position that centres a `window_w` × `window_h` window.
///
/// Prefers the primary display's work area and falls back to the historical virtual 1920×1080
/// screen only when the backend enumerates no displays. Keeping the fallback here means the
/// method has one coordinate rule instead of two, and the fallback is visible as a named fact
/// rather than a literal in the middle of the arithmetic.
fn centred_position(window_w: u32, window_h: u32) -> (i32, i32) {
    let platform = crate::platform::platform_facts();
    let displays = platform.displays();
    let display = displays.iter().find(|display| display.is_primary).or_else(|| displays.first());
    let area =
        display.map(|display| display.work_area).filter(|area| area.width > 0 && area.height > 0);
    match area {
        Some(area) => (
            area.x + (area.width as i32 - window_w as i32) / 2,
            area.y + (area.height as i32 - window_h as i32) / 2,
        ),
        None => {
            const FALLBACK_SCREEN: (i32, i32) = (1920, 1080);
            ((FALLBACK_SCREEN.0 - window_w as i32) / 2, (FALLBACK_SCREEN.1 - window_h as i32) / 2)
        }
    }
}
