// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Window and lifecycle handles: window options/state, layout application and
//! the message-box handle.

use super::*;
use crate::core::{ObjectId, Orientation, Point, Rect};
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

    /// Sets the maximum client size.
    pub fn max_size(mut self, width: u32, height: u32) -> Self {
        self.attributes.max_size = Some((width, height));
        self
    }

    /// Whether the window content is transparent.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.attributes.transparent = Some(transparent);
        self
    }

    /// Whether the window stays above other windows.
    pub fn always_on_top(mut self, on: bool) -> Self {
        self.attributes.always_on_top = Some(on);
        self
    }

    /// Preferred light/dark appearance (`true` = dark).
    pub fn preferred_theme(mut self, dark: bool) -> Self {
        self.attributes.preferred_theme = Some(dark);
        self
    }

    /// Whether the OS may include the window in screenshots and recordings.
    pub fn content_protected(mut self, protected: bool) -> Self {
        self.attributes.content_protected = Some(protected);
        self
    }

    /// Sets the window icon from a file path.
    pub fn icon(mut self, path: &str) -> Self {
        self.attributes.window_icon = Some(path.to_string());
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
        crate::platform::window_control::record_window_position(id, Point::new(x, y));
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
        crate::platform::window_control::record_window_position(self.id, Point::new(x, y));
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

    /// Chooses how this window's finished frame reaches the screen.
    ///
    /// # What the three intents mean
    ///
    /// * [`FramePresentation::Default`](crate::render::present::FramePresentation::Default) — what every window had before this call existed: the
    ///   platform canvas paints the software frame itself. Setting it back is always possible.
    /// * [`FramePresentation::Exact`](crate::render::present::FramePresentation::Exact) — the same frame, delivered through the presentation path and
    ///   byte for byte identical. The delivery method changes; the picture does not.
    /// * [`FramePresentation::Smooth`](crate::render::present::FramePresentation::Smooth) — the frame's command stream is drawn where that is
    ///   faithful, which may smooth the edges of circles, rings, arcs and rounded outlines.
    ///
    /// # Returns
    ///
    /// `true` when this backend can present **this window** and the intent has been recorded.
    /// `false` when it cannot — no native surface target, a window that does not exist, or a
    /// build without the presentation path. A `false` answer leaves the window on
    /// [`FramePresentation::Default`](crate::render::present::FramePresentation::Default); it never
    /// silently switches the picture (BLUE29 B29.16).
    ///
    /// The capability question is asked at runtime, per window (principles #41/#53): the same
    /// binary answers differently for a live window and a stale id, and nothing here branches on
    /// the operating system.
    ///
    /// # What consumes the recorded intent
    ///
    /// The backend's own paint path, when it has one: on macOS both canvases
    /// (`src/platform/macos/canvas.rs` and `macos_objc2/native.rs`) offer each finished frame to
    /// [`crate::render::present::present_window_frame`] before blitting it, and blit only when that
    /// path refuses the frame (occluded, timed out, no target). A backend whose canvas still blits
    /// unconditionally answers `false` from `canvas_presents_frames`, so this method returns
    /// `false` there rather than promising a change that would not happen.
    ///
    /// A host can also drive [`crate::render::present::FramePresenter`] directly when it wants to
    /// control the frame it hands over.
    pub fn set_frame_presentation(
        &self,
        intent: crate::render::present::FramePresentation,
    ) -> bool {
        use crate::render::present::{request, FramePresentation};
        if intent.is_default() {
            request(self.id, FramePresentation::Default);
            crate::widget::runtime::request_repaint(self.id);
            return true;
        }
        let platform = crate::platform::get_platform();
        if !platform.canvas_presents_frames() {
            log::debug!(
                "[window] set_frame_presentation({}): refused for window {} — this backend's \
                 canvas still paints every frame itself, so the window stays on the default \
                 delivery path",
                intent.label(),
                self.id
            );
            return false;
        }
        if !platform.frame_presentation_supported(self.id) {
            log::debug!(
                "[window] set_frame_presentation({}): refused for window {} — this backend cannot \
                 present it, so the window stays on the default delivery path",
                intent.label(),
                self.id
            );
            return false;
        }
        request(self.id, intent);
        // The next paint reads the intent, so ask for one: without this the change would only
        // take effect at the next unrelated invalidation.
        crate::widget::runtime::request_repaint(self.id);
        true
    }

    /// The intent this window will use for its next frame.
    ///
    /// Answers [`FramePresentation::Default`](crate::render::present::FramePresentation::Default)
    /// for a window that never asked, and for one whose
    /// request was refused — the two states are the same to a caller, which is the point: a
    /// refused request must not look like an accepted one. The **consumer** of the value is
    /// [`crate::render::present::FramePresenter`]; see [`Self::set_frame_presentation`].
    pub fn frame_presentation(&self) -> crate::render::present::FramePresentation {
        crate::render::present::requested(self.id)
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
    /// # The widget paints inside `rect`
    ///
    /// A surface is rasterised into a buffer of its own rectangle, so nothing the
    /// widget paints can land outside it — an open popup is cut off at the edge
    /// rather than drawn over a neighbour. A record-backed backend that cannot host
    /// a surface adopts the widget into its window tree, and it keeps the same
    /// bound (`Widget::paint_clip`), so the picture does not depend on which of the
    /// two paths the backend took.
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
    /// The example's `CodeEditor` is behind `code-editor`, so the fence is gated on it.
    ///
    /// ```no_run
    /// # #[cfg(feature = "code-editor")] {
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
    /// # }
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
        // The pass below is guarded by the size it last applied, and a freshly registered layout
        // has never been applied to any size. Dropping the record is what keeps the guard from
        // swallowing a layout set at an unchanged size (LY3-01).
        invalidate_applied_layout(self.id);
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
    let scale = context.layout_scale.max(context.font_scale);
    // LY3-01: a `Moved` trigger re-runs this pass on every tick even though neither the client
    // size nor the device scale changed, and the layout is a pure function of exactly those two
    // inputs (plus the layout object itself, which only changes through `set_layout`/
    // `store_layout` — both of which invalidate the record). The scale is compared against the
    // value the previous pass published, so a DPI or text-scale switch at an unchanged pixel size
    // still re-lays the tree.
    if applied_layout_is_current(window_id, width, height, scale) {
        return;
    }
    // Published before the pass so a diagnostic can read the scale that was actually used
    // even when the layout ignores the context entirely.
    LAYOUT_CONTEXT_SCALES.with(|map| {
        map.borrow_mut().insert(window_id, scale);
    });
    WINDOW_STATES.with(|map| {
        if let Some(state) = map.borrow_mut().get_mut(&window_id) {
            state.applied = Some((width, height));
        }
    });
    // LY-07: the list is taken from a per-thread pool and returned after it has been applied, so a
    // resize reuses one allocation per nesting level instead of making one.
    let mut scratch = crate::layout::scratch::take_geometry_scratch();
    let child_geometries = LAYOUTS.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&window_id) else {
            // A document-loaded (JSON / C ABI) root layout lives in the declarative registry,
            // not in `LAYOUTS`. The window path used to return an empty list here, so a resize
            // never replayed the root layout and the whole tree kept its load-time geometry
            // (LY-01). `declarative::apply_layout` is the one store for that path; it is not
            // mirrored into `LAYOUTS`.
            if crate::layout::declarative::has_layout(window_id) {
                // The pool entry is not used on this path: `apply_layout` builds its own list and
                // returns it to the caller, so handing it back keeps the pool warm. The context
                // built above is handed down instead of a second one being built there (LY3-04).
                crate::layout::scratch::return_geometry_scratch(scratch);
                return crate::layout::declarative::apply_layout_in_context(
                    window_id,
                    Rect::new(0, 0, width, height),
                    &context,
                );
            }
            return scratch;
        };
        layout.update_with_context(
            Rect::new(0, 0, width, height),
            &context,
            &mut |widget_id, geometry| {
                scratch.push((widget_id, geometry));
            },
        );
        scratch
    });

    // Indexed rather than consuming: the allocation has to survive the loop so the pool can take
    // it back. `(ObjectId, Rect)` is `Copy`, so this costs a copy, not a move of the buffer.
    for (widget_id, geometry) in child_geometries.iter().copied() {
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
        //
        // Both registries are applied when both hold an entry: they used to be an
        // `else if`, so a widget with a panel layout and a declarative layout only ever ran
        // the panel side and the document's layout went stale (LY-02). The order is fixed:
        // panel first, declarative second, so a document that wraps a panel wins.
        if widget_hosts_a_layout(widget_id) {
            apply_panel_geometry(widget_id, geometry);
        }
        if crate::layout::declarative::has_layout(widget_id) {
            crate::layout::declarative::apply_layout_in_context(widget_id, geometry, &context);
        }
    }
    crate::layout::scratch::return_geometry_scratch(child_geometries);
}

/// Whether the last layout pass already ran for `(width, height, scale)` (LY3-01).
///
/// The scale is read from [`LAYOUT_CONTEXT_SCALES`] rather than stored beside the size, because
/// that table already records exactly "the scale the previous pass was asked with" — a second copy
/// would be a mirror that could disagree with it (principle #99).
fn applied_layout_is_current(window_id: ObjectId, width: u32, height: u32, scale: f32) -> bool {
    let applied = WINDOW_STATES
        .try_with(|map| map.borrow().get(&window_id).and_then(|state| state.applied))
        .ok()
        .flatten();
    if applied != Some((width, height)) {
        return false;
    }
    LAYOUT_CONTEXT_SCALES.try_with(|map| map.borrow().get(&window_id).copied()).ok().flatten()
        == Some(scale)
}

/// Forgets the size the last layout pass applied, so the next pass runs even at the same size.
///
/// Called whenever a **new** layout is registered for `window_id`: the guard compares the client
/// size and the published scale, and a layout that has just been stored has been applied to
/// neither (LY3-01).
pub(crate) fn invalidate_applied_layout(window_id: ObjectId) {
    let _ = WINDOW_STATES.try_with(|map| {
        if let Some(state) = map.borrow_mut().get_mut(&window_id) {
            state.applied = None;
        }
    });
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

    /// Replaces the text drawn for one standard button (RW-123).
    ///
    /// Returns `false` when the id is not a mounted message box, so a host with a stale id gets
    /// an honest negative rather than a silent no-op.
    pub fn set_button_text(
        &self,
        button: crate::widget::dialog::message_box::StandardButton,
        text: &str,
    ) -> bool {
        crate::widget::runtime::with_widget_mut(self.id, |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::MessageBox>(
                widget,
            )
            .map(|dialog| dialog.set_button_text(button, text))
        })
        .flatten()
        .is_some()
    }

    /// Removes a host-supplied label, returning the button to its translated standard text.
    pub fn clear_button_text(
        &self,
        button: crate::widget::dialog::message_box::StandardButton,
    ) -> bool {
        crate::widget::runtime::with_widget_mut(self.id, |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::MessageBox>(
                widget,
            )
            .map(|dialog| dialog.clear_button_text(button))
        })
        .flatten()
        .is_some()
    }

    /// The text the dialog draws for `button`, or `None` when the id is not a mounted message box
    /// or the button is not in its current set.
    pub fn button_text(
        &self,
        button: crate::widget::dialog::message_box::StandardButton,
    ) -> Option<String> {
        crate::widget::runtime::read_widget(self.id, |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::dialog::MessageBox>(
                widget,
            )
            .and_then(|dialog| dialog.button_text(button))
        })
        .flatten()
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
    /// Largest client size the platform accepted, mirrored for the backends whose
    /// `Platform` has no `window_max_size` read (HB-01).
    pub(crate) max_w: u32,
    pub(crate) max_h: u32,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) fullscreen: bool,
    pub(crate) resizable: bool,
    pub(crate) decorated: bool,
    /// The client size the last [`apply_window_layout`] pass ran for (LY3-01).
    ///
    /// Not a platform mirror: it records what this module last did, so a `Moved` trigger — which
    /// changes neither the size nor the device scale — no longer re-solves the whole tree. `None`
    /// means "no pass applied", which also covers a layout registered after the last pass; see
    /// [`invalidate_applied_layout`].
    pub(crate) applied: Option<(u32, u32)>,
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
            .field("max_w", &self.max_w)
            .field("max_h", &self.max_h)
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
            max_w: 0,
            max_h: 0,
            maximized: false,
            minimized: false,
            fullscreen: false,
            resizable: true,
            decorated: true,
            applied: None,
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

    /// Set the maximum window size.
    ///
    /// Pushed through [`crate::platform::Platform::set_window_max_size`]. The `Platform` has no
    /// read for it, so the mirror this records is the only readback `max_size` has — and it is
    /// written only when the platform accepted the request (HB-01).
    pub fn set_max_size(&self, w: u32, h: u32) -> bool {
        let pushed = crate::platform::get_platform().set_window_max_size(self.raw_id(), w, h);
        if pushed {
            WINDOW_STATES.with(|map| {
                let mut map = map.borrow_mut();
                let state = map.entry(self.raw_id()).or_default();
                state.max_w = w;
                state.max_h = h;
            });
        }
        pushed
    }

    /// Read the maximum window size, if one was accepted.
    pub fn max_size(&self) -> Option<(u32, u32)> {
        WINDOW_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).and_then(|state| {
                if state.max_w == 0 && state.max_h == 0 {
                    None
                } else {
                    Some((state.max_w, state.max_h))
                }
            })
        })
    }

    /// Whether this window is the one that most recently gained keyboard focus.
    ///
    /// The platform trait has no focus query, so this reads the active-window record maintained by
    /// [`crate::platform::window_control::notify_window_focus`] — the same notification the
    /// `focus_gained`/`focus_lost` signals fire from (HB-12).
    pub fn is_active(&self) -> bool {
        crate::platform::window_control::active_window_id() == Some(self.raw_id())
    }

    /// Set whether the window content is transparent.
    pub fn set_transparent(&self, transparent: bool) -> bool {
        crate::platform::get_platform().set_window_transparent(self.raw_id(), transparent)
    }

    /// Set whether the window stays above other windows.
    pub fn set_always_on_top(&self, on: bool) -> bool {
        crate::platform::get_platform().set_window_always_on_top(self.raw_id(), on)
    }

    /// Request a per-window light/dark appearance (`true` = dark).
    pub fn set_preferred_theme(&self, dark: bool) -> bool {
        crate::platform::get_platform().set_window_preferred_theme(self.raw_id(), dark)
    }

    /// Set whether the OS may include the window in screenshots and recordings.
    pub fn set_content_protected(&self, protected: bool) -> bool {
        crate::platform::get_platform().set_window_content_protected(self.raw_id(), protected)
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
        let _ = self.set_maximized_applied(maximized);
    }

    /// [`Self::set_maximized`] with the platform's answer, for the generic handle path.
    ///
    /// The generic `WidgetHandle::set_window_state` used to call the platform directly and write
    /// no mirror, so a backend that answers `None` to the read made `is_maximized` fall back to the
    /// mirror's default and report the opposite of what was set (HB-04). Both paths now share this
    /// one writer.
    pub(crate) fn set_maximized_applied(&self, maximized: bool) -> bool {
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
        applied
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
        crate::platform::window_control::record_window_position(
            self.raw_id(),
            Point::new(centred_x, centred_y),
        );
    }

    /// Raise this window and give it the keyboard focus.
    ///
    /// This is the semantic counterpart of the user clicking the window: the host asks for it
    /// when a command or a `window.activate` action should bring an existing window forward.
    /// The platform decides how; a backend with no windowing system answers `false` rather
    /// than pretending the window became active.
    pub fn activate(&self) -> bool {
        crate::platform::get_platform().activate_window(self.raw_id())
    }

    /// The window's top-left position in screen coordinates, when it can be known.
    ///
    /// The backend is asked first because it reports what the window manager actually did. The
    /// in-process mirror is the fallback — it is written at creation, by [`Self::set_position`]
    /// when the platform accepted the move, and by the platform's own move notification — so a
    /// backend that has no position query (a state-only window) still answers with the last
    /// position the library knows rather than inventing one.
    ///
    /// On a backend whose window system cannot report a move to the client at all (a Wayland
    /// toplevel, for one), the answer is that fallback and **not** a live reading. A host that
    /// needs to tell the two apart asks
    /// [`Platform::reports_window_moves`](crate::platform::Platform::reports_window_moves)
    /// instead of inferring it from a stale coordinate.
    pub fn position(&self) -> Option<Point> {
        if let Some((x, y)) = crate::platform::get_platform().window_position(self.raw_id()) {
            return Some(Point::new(x, y));
        }
        crate::platform::window_control::window_position_mirror(self.raw_id()).or_else(|| {
            WINDOW_STATES.with(|map| {
                map.borrow().get(&self.raw_id()).map(|state| Point::new(state.x, state.y))
            })
        })
    }

    /// Move the window to `position` in screen coordinates.
    ///
    /// The mirror is updated only when the platform accepted the request, matching the
    /// `set_min_size`/`set_icon` rule: a rejected move must not be reported by [`Self::position`]
    /// as if it happened. Returns whether the backend accepted it.
    pub fn set_position(&self, position: Point) -> bool {
        let applied = crate::platform::get_platform().set_window_position(
            self.raw_id(),
            position.x,
            position.y,
        );
        if applied {
            // A host-driven move is a real move notification on every backend, including the
            // ones whose protocol cannot observe OS-initiated moves (wayland/redox). Routing it
            // through the same notification the backends use updates the mirror and emits the
            // window's `moved` signal, so `position()` never disagrees with the request (PL2-01).
            crate::platform::window_control::notify_window_moved(self.raw_id(), position);
        }
        applied
    }

    /// Reports that the window's top-left corner is now `position`.
    ///
    /// # Why a host needs this
    ///
    /// A backend whose window protocol cannot report OS-initiated moves (a Wayland toplevel, a
    /// Redox orbclient window) has no event to translate, so its `moved` signal and the position
    /// mirror would otherwise never change when the user drags the window. A host that *does* learn
    /// the new position — from its own compositor integration, a platform channel, or a toolkit
    /// callback — reports it here. The call routes through the same
    /// [`notify_window_moved`](crate::platform::window_control) every backend uses, so the mirror
    /// and the window's `moved` signal agree regardless of how the fact arrived.
    ///
    /// This is the counterpart of [`Self::set_position`] for a move the library did not request:
    /// `set_position` asks the platform and notifies on success, this one takes the fact as given.
    pub fn report_moved(&self, position: Point) {
        crate::platform::window_control::notify_window_moved(self.raw_id(), position);
    }

    /// The control inside this window that currently owns keyboard focus.
    ///
    /// `None` when nothing is focused, or when the focused control belongs to another window —
    /// which is exactly the distinction a window-level key handler needs before deciding whether
    /// a chord is its own.
    pub fn focused_control(&self) -> Option<ObjectId> {
        let focused = crate::widget::runtime::focused_widget()?;
        (crate::widget::runtime::top_level_window_of(focused) == Some(self.raw_id()))
            .then_some(focused)
    }

    /// Installs the window-level key handler, ahead of the focused control.
    ///
    /// # Contract
    ///
    /// The handler is called for a [`crate::event::Event::KeyPress`] routed through this
    /// window, with the chord normalised to the platform's accelerator convention
    /// (`Shortcut::primary` answers to `Ctrl` on Windows/Linux and `Command` on macOS).
    /// Returning `true` claims the key; returning `false` lets it continue to the host's
    /// accelerator hook and then to the focused control.
    ///
    /// # What is never offered
    ///
    /// * committed text (`TextInput`, `ImePreedit`, `ImeCommit`);
    /// * any key while an input method is composing;
    /// * a text-producing key with no command modifier, so a typed `s` (and `Shift+S`) is
    ///   text rather than a command.
    ///
    /// This happens before the global shortcut hook, so a window can own a chord that a global
    /// binding also registered. Replacing the handler for the same window replaces the previous
    /// one; call [`Self::clear_key_handler`] to remove it.
    pub fn set_key_handler<F>(&self, handler: F)
    where
        F: Fn(&crate::shortcut::Shortcut, &crate::event::FocusInfo) -> bool + 'static,
    {
        crate::shortcut::route::set_key_handler(self.raw_id(), alloc::boxed::Box::new(handler));
    }

    /// Removes this window's key handler, returning whether one was installed.
    pub fn clear_key_handler(&self) -> bool {
        crate::shortcut::route::clear_key_handler(self.raw_id())
    }

    /// Installs a close veto handler for this window.
    ///
    /// The handler returns `true` when the window may close and `false` to refuse it — the
    /// "save before closing" path. It is consulted by every OS close notification (the macOS
    /// `windowShouldClose:`, Win32 `WM_CLOSE`, and GTK's `delete-event`) **before** the window
    /// is destroyed. In addition, a control that consumes
    /// [`crate::event::Event::WindowCloseRequest`] also vetoes the close, so a widget-level
    /// handler has the same power without reaching into the window API.
    ///
    /// This is a veto, not the notification callback: [`Self::on_close`] runs after the close
    /// has been accepted and reports the lifecycle fact.
    pub fn set_close_handler<F>(&self, handler: F)
    where
        F: Fn() -> bool + 'static,
    {
        crate::platform::window_control::set_close_handler(
            self.raw_id(),
            alloc::boxed::Box::new(handler),
        );
    }

    /// Removes the close veto handler, returning whether one was installed.
    pub fn clear_close_handler(&self) -> bool {
        crate::platform::window_control::clear_close_handler(self.raw_id())
    }

    /// Registers a callback for the window gaining keyboard focus.
    ///
    /// Returns `false` when the id does not name a mounted window, so a caller can tell a
    /// registration that took from one that had nothing to attach to.
    pub fn on_focus_gained<F>(&self, callback: F) -> bool
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_window_signal(callback, WindowSignal::FocusGained)
    }

    /// Registers a callback for the window losing keyboard focus.
    pub fn on_focus_lost<F>(&self, callback: F) -> bool
    where
        F: FnMut() + Send + Sync + 'static,
    {
        self.connect_window_signal(callback, WindowSignal::FocusLost)
    }

    /// Registers a callback for the window moving, receiving the new top-left position.
    ///
    /// The callback is invoked by the platform notification path, not only by
    /// [`Self::set_position`], so a user drag and a programmatic move are one stream.
    pub fn on_moved<F>(&self, callback: F) -> bool
    where
        F: FnMut(Point) + Send + Sync + 'static,
    {
        let id = self.raw_id();
        let mut callback = Some(callback);
        let mut connected = false;
        let _ = crate::widget::runtime::with_widget_mut(id, |widget| {
            if let Some(window) =
                crate::widget::capability::widget_as::<crate::widget::Window>(widget)
            {
                if let Some(mut callback) = callback.take() {
                    let handle = window.moved.connect(move |position| callback(*position));
                    // Recorded so `remove_callbacks` can disconnect it; these three registrars
                    // used to discard the handle (HB-03).
                    super::record_signal_connection(id, super::SignalKind::Moved, handle);
                    connected = true;
                }
            }
        });
        connected
    }

    /// Shared connection helper for the two unit signals.
    fn connect_window_signal<F>(&self, callback: F, signal: WindowSignal) -> bool
    where
        F: FnMut() + Send + Sync + 'static,
    {
        let id = self.raw_id();
        let mut callback = Some(callback);
        let mut connected = false;
        let _ = crate::widget::runtime::with_widget_mut(id, |widget| {
            if let Some(window) =
                crate::widget::capability::widget_as::<crate::widget::Window>(widget)
            {
                match signal {
                    WindowSignal::FocusGained => {
                        if let Some(callback) = callback.take() {
                            let handle = window.focus_gained.connect(callback);
                            super::record_signal_connection(
                                id,
                                super::SignalKind::FocusGained,
                                handle,
                            );
                            connected = true;
                        }
                    }
                    WindowSignal::FocusLost => {
                        if let Some(callback) = callback.take() {
                            let handle = window.focus_lost.connect(callback);
                            super::record_signal_connection(
                                id,
                                super::SignalKind::FocusLost,
                                handle,
                            );
                            connected = true;
                        }
                    }
                }
            }
        });
        connected
    }
}

/// Which unit signal [`WindowHandle::connect_window_signal`] connects.
enum WindowSignal {
    FocusGained,
    FocusLost,
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
