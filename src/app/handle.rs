// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-safe widget handles backed by `ObjectId`.
//!
//! Each handle type wraps a raw `ObjectId` and exposes only the operations
//! that are valid for that widget kind.  Handles also support event callbacks
//! via the `WidgetHandle` extension trait.

use alloc::rc::Rc;
use core::cell::RefCell;

use crate::core::{ObjectId, Orientation, Rect};
use crate::platform::{WidgetTriggerKind, WindowStateFlag};

// ═══════════════════════════════════════════════════════════════
// Supporting types used by widget handles
// ═══════════════════════════════════════════════════════════════

/// The visual state of a tri-state check-box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckState {
    /// Box is not checked.
    Unchecked,
    /// Box is checked.
    Checked,
    /// Box is in an indeterminate / partially-checked state.
    PartiallyChecked,
}

/// Controls how text is displayed in a line-edit widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EchoMode {
    /// Display characters as-is.
    Normal,
    /// Mask every character (e.g. for passwords).
    Password,
    /// Do not echo characters at all.
    NoEcho,
}

/// Determines how many rows can be selected in a list / table view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionMode {
    /// At most one row can be selected.
    Single,
    /// Multiple rows can be selected (toggle behaviour).
    Multi,
    /// Multiple rows can be selected with modifier keys (Ctrl/Shift).
    Extended,
    /// No row can be selected.
    None,
}

/// Data model interface for list / table views.
///
/// Widgets that display tabular data (such as [`ListViewHandle`]) use
/// a `Box<dyn ListModel>` to query the number of rows and the text for
/// each cell.
pub trait ListModel {
    /// Return the number of rows in the model.
    fn row_count(&self) -> usize;
    /// Return the text for the cell at `(row, col)`.
    fn text(&self, row: usize, col: usize) -> String;
    /// Update the text for the cell at `(row, col)`.
    fn set_text(&mut self, row: usize, col: usize, text: &str);
}

// ═══════════════════════════════════════════════════════════════
// Shared callback storage
// ═══════════════════════════════════════════════════════════════

/// Boxed callback invoked when a widget is triggered.
pub type ClickCallback = Rc<RefCell<dyn FnMut()>>;

/// Boxed callback invoked when a widget value changes.
pub type ValueChangedCallback = Rc<RefCell<dyn FnMut(String)>>;

// ═══════════════════════════════════════════════════════════════
// Self-drawn widget mounting
// ═══════════════════════════════════════════════════════════════

/// Why a self-drawn widget could not be mounted into a window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfDrawnMountError {
    /// The calling thread has no widget registry — mounting must happen on the
    /// thread that drives the UI.
    NoRegistryOnThread,
    /// This backend has no self-drawn surface (`Platform::supports_self_drawn`
    /// returned `false`). Carries the backend name for the message.
    UnsupportedByBackend(&'static str),
    /// The backend claims support but refused this particular mount (unknown
    /// parent, wrong parent kind, allocation failure). Carries the backend name.
    RejectedByBackend(&'static str),
    /// `mount_widget_by_name` was given a name the widget factory does not know.
    UnknownWidgetName,
}

impl core::fmt::Display for SelfDrawnMountError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoRegistryOnThread => write!(
                f,
                "self-drawn widgets must be mounted on the UI thread (no registry on this thread)"
            ),
            Self::UnsupportedByBackend(backend) => write!(
                f,
                "backend '{backend}' cannot display self-drawn widgets; \
                 it has no native canvas surface"
            ),
            Self::RejectedByBackend(backend) => {
                write!(f, "backend '{backend}' refused the mount (see logs for the reason)")
            }
            Self::UnknownWidgetName => {
                write!(f, "the widget factory has no widget registered under that name")
            }
        }
    }
}

impl std::error::Error for SelfDrawnMountError {}

/// Handle to a self-drawn widget mounted in a window.
///
/// Keeps the widget's registry id so the caller can move or unmount it. Dropping
/// the handle is **not** enough to remove the widget: the window still owns it,
/// because the native surface outlives any single Rust value. Call
/// [`SelfDrawnHandle::unmount`] for that; `Drop` only detaches this handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfDrawnHandle {
    id: ObjectId,
}

impl SelfDrawnHandle {
    /// Wraps a registry id.
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Returns the widget registry id.
    pub fn raw_id(&self) -> ObjectId {
        self.id
    }

    /// Returns the widget's current geometry.
    pub fn geometry(&self) -> Option<Rect> {
        crate::widget::runtime::geometry_of(self.id)
    }

    /// Moves and/or resizes the mounted widget.
    ///
    /// Returns `false` when the widget is no longer mounted.
    pub fn set_geometry(&self, rect: Rect) -> bool {
        crate::resize_self_drawn(self.id, rect)
    }

    /// Removes the widget from its window and drops it.
    ///
    /// Returns `false` when it was already unmounted.
    pub fn unmount(&self) -> bool {
        let removed = crate::unmount_self_drawn(self.id);
        crate::widget::runtime::unregister(self.id);
        removed
    }

    /// Runs `f` against the mounted widget, then repaints it if `f` reports a change.
    ///
    /// # Why this exists
    ///
    /// A self-drawn widget owns its own interaction model, so a native menu item
    /// or tool-bar button cannot drive it through the platform event queue —
    /// there is no OS control to send a command to. This is the generic bridge:
    /// the caller decides what to do with the widget, and this method guarantees
    /// the change becomes visible.
    ///
    /// `f` returns whether it changed anything; returning `false` skips the
    /// repaint. Returns `None` when the widget is no longer mounted.
    ///
    /// ```no_run
    /// use rust_widgets::app::{App, WidgetHandle};
    /// use rust_widgets::core::Rect;
    /// use rust_widgets::widget::special_widgets::code_editor::CodeEditor;
    ///
    /// let mut app = App::new();
    /// app.init();
    /// let win = app.new_window("Editor", 0, 0, 800, 600);
    /// let editor = win
    ///     .mount_widget_by_name("code_editor", Rect::new(0, 0, 800, 600), "")
    ///     .expect("backend supports self-drawn widgets");
    ///
    /// // Downcast to the concrete widget and drive it directly.
    /// editor.update(|widget| {
    ///     let Some(editor) = (widget as &mut dyn std::any::Any).downcast_mut::<CodeEditor>()
    ///     else {
    ///         return false;
    ///     };
    ///     editor.undo()
    /// });
    /// ```
    pub fn update(&self, f: impl FnOnce(&mut dyn crate::widget::Widget) -> bool) -> Option<bool> {
        let changed = crate::widget::runtime::with_widget_mut(self.id, f)?;
        if changed {
            crate::widget::runtime::request_repaint(self.id);
        }
        Some(changed)
    }

    /// Reads state from the mounted widget without repainting.
    ///
    /// Returns `None` when the widget is no longer mounted.
    pub fn read<R>(&self, f: impl FnOnce(&dyn crate::widget::Widget) -> R) -> Option<R> {
        crate::widget::runtime::with_widget(self.id, f)
    }
}

impl WidgetHandle for SelfDrawnHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Self-drawn widgets route input into themselves.
    ///
    /// A `CodeEditor` handles its own clicks, keys and IME commits through
    /// `EventHandler`; there is no separate platform control to attach a
    /// click callback to. This deliberately does **not** register a callback
    /// that would never fire — read the widget's own signals instead (for the
    /// editor: `text_changed`, `cursor_moved`, `selection_changed`).
    fn on_click<F: FnMut() + 'static>(&self, _f: F) {
        log::debug!(
            "SelfDrawnHandle::on_click ignored for id={}: self-drawn widgets emit their own \
             signals rather than a platform click callback",
            self.id
        );
    }

    /// See [`SelfDrawnHandle::on_click`]; the same reasoning applies.
    fn on_value_changed<F: FnMut(String) + 'static>(&self, _f: F) {
        log::debug!(
            "SelfDrawnHandle::on_value_changed ignored for id={}: self-drawn widgets emit \
             their own signals",
            self.id
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// WidgetHandle trait – shared behaviour for all handles
// ═══════════════════════════════════════════════════════════════

/// Common operations available on every widget handle.
///
/// Implemented automatically by the `impl_handle!` macro and by
/// [`WindowHandle`].
pub trait WidgetHandle: Sized {
    /// Return the raw [`ObjectId`] backing this handle.
    fn raw_id(&self) -> ObjectId;

    /// Construct a handle from a raw [`ObjectId`].
    fn from_raw(id: ObjectId) -> Self;

    /// Show the widget.
    fn show(&self) {
        crate::show_widget(self.raw_id());
    }

    /// Hide the widget.
    fn hide(&self) {
        crate::hide_widget(self.raw_id());
    }

    /// Set widget geometry (position + size).
    fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
    }

    /// Update the widget's text / label.
    fn set_text(&self, text: &str) {
        crate::set_widget_text(self.raw_id(), text);
    }

    /// Read the widget's current text.
    fn text(&self) -> String {
        crate::get_widget_text(self.raw_id())
    }

    /// Enable the widget (accept user input).
    fn enable(&self) {
        crate::set_widget_enabled(self.raw_id(), true);
    }

    /// Disable the widget (ignore user input).
    fn disable(&self) {
        crate::set_widget_enabled(self.raw_id(), false);
    }

    /// Check whether the widget is currently enabled.
    fn is_enabled(&self) -> bool {
        crate::is_widget_enabled(self.raw_id())
    }

    /// Show or hide the widget.
    fn set_visible(&self, visible: bool) {
        crate::set_widget_visible(self.raw_id(), visible);
    }

    /// Check whether the widget is currently visible.
    fn is_visible(&self) -> bool {
        crate::is_widget_visible(self.raw_id())
    }

    /// Set this widget's primary numeric value (slider, progress bar, spin box,
    /// scroll bar, ...).
    ///
    /// Returns `true` when the backend wrote it to a real native control or an
    /// authoritative state model; `false` when this backend's control has no
    /// numeric value. Callers branch on the result at runtime if they care — they
    /// never branch on the OS. See [`crate::Platform::set_widget_value`].
    fn set_value(&self, value: f64) -> bool {
        crate::platform::get_platform().set_widget_value(self.raw_id(), value)
    }

    /// Read this widget's primary numeric value.
    fn value(&self) -> Option<f64> {
        crate::platform::get_platform().widget_value(self.raw_id())
    }

    /// Set this widget's `(min, max)` range, when it has one.
    fn set_range(&self, min: f64, max: f64) -> bool {
        crate::platform::get_platform().set_widget_range(self.raw_id(), min, max)
    }

    /// Read this widget's `(min, max)` range, when it has one.
    fn range(&self) -> Option<(f64, f64)> {
        crate::platform::get_platform().widget_range(self.raw_id())
    }

    /// Set this widget's selection index (combo box, list box, tab widget).
    ///
    /// `index == None` clears the selection where the control supports it.
    fn set_selected_index(&self, index: Option<usize>) -> bool {
        crate::platform::get_platform().set_widget_selected_index(self.raw_id(), index)
    }

    /// Read this widget's selection index.
    fn selected_index(&self) -> Option<usize> {
        crate::platform::get_platform().widget_selected_index(self.raw_id())
    }

    /// Set a checkable control's checked state (check box, radio button, toggle
    /// button).
    fn set_checked(&self, checked: bool) -> bool {
        crate::platform::get_platform().set_widget_checked(self.raw_id(), checked)
    }

    /// Read this widget's checked state, or `None` when it is not checkable.
    fn checked(&self) -> Option<bool> {
        crate::platform::get_platform().is_widget_checked(self.raw_id())
    }

    /// Set this widget's increment step (slider, spin box, scroll bar).
    fn set_step(&self, step: f64) -> bool {
        crate::platform::get_platform().set_widget_step(self.raw_id(), step)
    }

    /// Read this widget's increment step.
    fn step(&self) -> Option<f64> {
        crate::platform::get_platform().widget_step(self.raw_id())
    }

    /// Set a progress-style control's indeterminate (busy) state.
    fn set_indeterminate(&self, indeterminate: bool) -> bool {
        crate::platform::get_platform().set_widget_indeterminate(self.raw_id(), indeterminate)
    }

    /// Read a progress-style control's indeterminate state.
    fn is_indeterminate(&self) -> Option<bool> {
        crate::platform::get_platform().is_widget_indeterminate(self.raw_id())
    }

    /// Set a text-entry control's read-only state.
    fn set_read_only(&self, read_only: bool) -> bool {
        crate::platform::get_platform().set_widget_read_only(self.raw_id(), read_only)
    }

    /// Read a text-entry control's read-only state.
    fn is_read_only(&self) -> Option<bool> {
        crate::platform::get_platform().is_widget_read_only(self.raw_id())
    }

    /// Set a text-entry control's maximum accepted length.
    fn set_max_length(&self, max_length: u32) -> bool {
        crate::platform::get_platform().set_widget_max_length(self.raw_id(), max_length)
    }

    /// Read a text-entry control's maximum accepted length.
    fn max_length(&self) -> Option<u32> {
        crate::platform::get_platform().widget_max_length(self.raw_id())
    }

    /// Apply or clear a window state (maximised, minimised, full-screen, ...).
    ///
    /// Returns `false` when this handle is not a window (or the backend cannot
    /// honour the state), so a control can share the call shape without
    /// pretending the write landed.
    fn set_window_state(&self, flag: WindowStateFlag, on: bool) -> bool {
        crate::platform::get_platform().set_window_state(self.raw_id(), flag, on)
    }

    /// Read a window state, or `None` when this handle is not a window.
    fn is_window_in_state(&self, flag: WindowStateFlag) -> Option<bool> {
        crate::platform::get_platform().is_window_in_state(self.raw_id(), flag)
    }

    /// Set a window's minimum content size.
    fn set_window_min_size(&self, width: u32, height: u32) -> bool {
        crate::platform::get_platform().set_window_min_size(self.raw_id(), width, height)
    }

    /// Read a window's minimum content size.
    fn window_min_size(&self) -> Option<(u32, u32)> {
        crate::platform::get_platform().window_min_size(self.raw_id())
    }

    /// Set a window's icon from a file path.
    fn set_window_icon(&self, path: &str) -> bool {
        crate::platform::get_platform().set_window_icon(self.raw_id(), path)
    }

    /// Read a window's icon path, if one was set.
    fn window_icon(&self) -> Option<String> {
        crate::platform::get_platform().window_icon(self.raw_id())
    }

    /// Register a callback for the "clicked" trigger.
    ///
    /// The closure is invoked whenever the widget receives a
    /// [`WidgetTriggerKind::Clicked`] event.
    fn on_click<F: FnMut() + 'static>(&self, f: F);

    /// Register a callback for the "value changed" trigger.
    ///
    /// The closure receives the widget's current text at the time of
    /// the [`WidgetTriggerKind::ValueChanged`] event.
    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F);
}

// ── Global callback registry ──────────────────────────────────

use std::collections::HashMap;

thread_local! {
    static CLICK_CALLBACKS: RefCell<HashMap<ObjectId, ClickCallback>> = RefCell::new(HashMap::new());
    static VALUE_CALLBACKS: RefCell<HashMap<ObjectId, ValueChangedCallback>> = RefCell::new(HashMap::new());
}

/// Remove all registered callbacks for the given widget id.
///
/// Call this when a widget is destroyed to prevent callback leaks
/// from thread-local storage.
///
/// Uses `try_borrow_mut` to avoid panicking when called re-entrantly
/// (e.g. during callback dispatch when a handle is dropped).
pub fn remove_callbacks(id: ObjectId) {
    CLICK_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    VALUE_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
}

/// Dispatch a trigger event to the registered callback for `widget_id`.
/// Returns `true` if a callback was found and invoked.
///
/// The callback is **removed** from the map before invocation and then
/// **re-inserted** afterwards, so that re-entrant calls to
/// `remove_callbacks` (from a handle Drop inside the callback) do not
/// panic on a double borrow.
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
    match kind {
        WidgetTriggerKind::Clicked | WidgetTriggerKind::Unknown => {
            let cb = CLICK_CALLBACKS.with(|map| map.borrow_mut().remove(&widget_id));
            if let Some(cb) = cb {
                (cb.borrow_mut())();
                CLICK_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(widget_id, cb);
                });
                true
            } else {
                false
            }
        }
        WidgetTriggerKind::ValueChanged | WidgetTriggerKind::SelectionChanged => {
            let text = crate::get_widget_text(widget_id);
            let cb = VALUE_CALLBACKS.with(|map| map.borrow_mut().remove(&widget_id));
            if let Some(cb) = cb {
                (cb.borrow_mut())(text);
                VALUE_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(widget_id, cb);
                });
                true
            } else {
                false
            }
        }
        WidgetTriggerKind::Closed => {
            // Clean up callbacks when a widget is closed/destroyed.
            remove_callbacks(widget_id);
            false
        }
    }
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

impl WindowHandle {
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

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

    fn on_click<F: FnMut() + 'static>(&self, f: F) {
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }

    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
        VALUE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        remove_callbacks(self.id);
    }
}

impl WindowHandle {
    pub fn set_title(&self, title: &str) {
        crate::set_widget_text(self.id, title);
    }

    // ── Child-widget factory methods ──────────────────────

    pub fn new_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> ButtonHandle {
        ButtonHandle::from_raw(crate::create_button(self.id, text, x, y, w, h))
    }

    pub fn new_label(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LabelHandle {
        LabelHandle::from_raw(crate::create_label(self.id, text, x, y, w, h))
    }

    pub fn new_checkbox(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> CheckBoxHandle {
        CheckBoxHandle::from_raw(crate::create_checkbox(self.id, text, x, y, w, h))
    }

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

    pub fn new_line_edit(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LineEditHandle {
        LineEditHandle::from_raw(crate::create_line_edit(self.id, text, x, y, w, h))
    }

    pub fn new_combo_box(&self, x: i32, y: i32, w: u32, h: u32) -> ComboBoxHandle {
        ComboBoxHandle::from_raw(crate::create_combo_box(self.id, x, y, w, h))
    }

    pub fn new_list_box(&self, x: i32, y: i32, w: u32, h: u32) -> ListBoxHandle {
        ListBoxHandle::from_raw(crate::create_list_box(self.id, x, y, w, h))
    }

    pub fn new_slider(&self, x: i32, y: i32, w: u32, h: u32) -> SliderHandle {
        SliderHandle::from_raw(crate::create_slider(self.id, x, y, w, h))
    }

    pub fn new_progress_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ProgressBarHandle {
        ProgressBarHandle::from_raw(crate::create_progress_bar(self.id, x, y, w, h))
    }

    /// Mount a **self-drawn** widget into this window.
    ///
    /// # What this is for
    ///
    /// Widgets that paint themselves through `Draw` (`CodeEditor`, `ColorPicker`,
    /// `GanttWidget`, `TerminalView`, …) have no OS control to map onto, so the
    /// `new_*` factory methods above cannot host them. This method hands the
    /// widget to a native canvas surface that repaints it whenever the window
    /// is invalidated, and forwards pointer/keyboard input back into the widget.
    ///
    /// # Ownership
    ///
    /// The window takes ownership through the process-wide widget registry; the
    /// returned handle can move, resize and unmount it.
    ///
    /// # Returns
    ///
    /// `Ok(handle)` when the backend mounted the widget, `Err(reason)` when it
    /// could not — a backend without self-drawn support (see
    /// `Platform::supports_self_drawn`), an off-UI-thread call, or an unknown
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
    /// win.mount_self_drawn(Box::new(editor), Rect::new(0, 0, 900, 600))
    ///     .expect("backend must support self-drawn widgets");
    /// win.show();
    /// app.run();
    /// ```
    pub fn mount_self_drawn(
        &self,
        widget: Box<dyn crate::widget::Widget>,
        rect: Rect,
    ) -> Result<SelfDrawnHandle, SelfDrawnMountError> {
        // The widget must be registered before the backend can be asked to show
        // it, because the backend looks it up by id on every repaint.
        let id = crate::widget::runtime::register(widget)
            .ok_or(SelfDrawnMountError::NoRegistryOnThread)?;
        crate::widget::runtime::set_geometry(id, rect);

        let mounted = crate::mount_self_drawn(self.id, id, rect);
        if !mounted {
            // Do not leave a widget stranded in the registry when the backend
            // refused to show it. Dropping it here keeps the two in step.
            crate::widget::runtime::unregister(id);
            if !crate::supports_self_drawn() {
                return Err(SelfDrawnMountError::UnsupportedByBackend(crate::backend_name()));
            }
            return Err(SelfDrawnMountError::RejectedByBackend(crate::backend_name()));
        }
        Ok(SelfDrawnHandle { id })
    }

    /// Mount a self-drawn widget, creating it from the widget factory by name.
    ///
    /// Convenience wrapper over [`WindowHandle::mount_self_drawn`] for callers
    /// that already address widgets by their capability name (`"code_editor"`,
    /// `"color_picker"`, …).
    pub fn mount_widget_by_name(
        &self,
        name: &str,
        rect: Rect,
        text: &str,
    ) -> Result<SelfDrawnHandle, SelfDrawnMountError> {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let widget =
            factory.create(name, rect, text).ok_or(SelfDrawnMountError::UnknownWidgetName)?;
        self.mount_self_drawn(widget, rect)
    }

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
        FrameHandle::from_raw(crate::create_panel(self.id, x, y, w, h))
    }

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

    /// Create a tool bar strip for this window.
    pub fn new_tool_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ToolBarHandle {
        ToolBarHandle::from_raw(crate::create_tool_bar(self.id, x, y, w, h))
    }

    /// Create a status bar for this window.
    pub fn new_status_bar(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> StatusBarHandle {
        StatusBarHandle::from_raw(crate::create_status_bar(self.id, text, x, y, w, h))
    }

    pub fn new_list_view(&self, x: i32, y: i32, w: u32, h: u32) -> ListViewHandle {
        ListViewHandle::from_raw(crate::create_list_view(self.id, x, y, w, h))
    }

    pub fn new_scroll_area(&self, x: i32, y: i32, w: u32, h: u32) -> ScrollAreaHandle {
        ScrollAreaHandle::from_raw(crate::create_scroll_area(self.id, x, y, w, h))
    }

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
}

thread_local! {
    static LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
}

/// Apply a window's current layout to all child widget geometries.
///
/// Layouts use the window client area as their coordinate space. Geometry
/// updates happen after the layout borrow is released so a backend callback
/// cannot re-enter the layout map while it is borrowed.
fn apply_window_layout(window_id: ObjectId) {
    let Some((width, height)) =
        WINDOW_STATES.with(|map| map.borrow().get(&window_id).map(|state| (state.w, state.h)))
    else {
        return;
    };

    let child_geometries = LAYOUTS.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&window_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update(Rect::new(0, 0, width, height), &mut |widget_id, geometry| {
            geometries.push((widget_id, geometry));
        });
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
    }
}

// ═══════════════════════════════════════════════════════════════
// Macro for standard widget handles
// ═══════════════════════════════════════════════════════════════

macro_rules! impl_handle {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name {
            id: ObjectId,
        }

        impl $name {
            pub fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }
        }

        impl WidgetHandle for $name {
            fn raw_id(&self) -> ObjectId {
                self.id
            }

            fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }

            fn on_click<F: FnMut() + 'static>(&self, f: F) {
                CLICK_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
                });
            }

            fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
                VALUE_CALLBACKS.with(|map| {
                    map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
                });
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                remove_callbacks(self.id);
            }
        }
    };
}

impl_handle!(ButtonHandle, "Type-safe handle for a Button widget.");
impl_handle!(LabelHandle, "Type-safe handle for a Label widget.");
impl_handle!(CheckBoxHandle, "Type-safe handle for a CheckBox widget.");
impl_handle!(RadioButtonHandle, "Type-safe handle for a RadioButton widget.");
impl_handle!(LineEditHandle, "Type-safe handle for a LineEdit widget.");
impl_handle!(ComboBoxHandle, "Type-safe handle for a ComboBox widget.");
impl_handle!(ListBoxHandle, "Type-safe handle for a ListBox widget.");
impl_handle!(SliderHandle, "Type-safe handle for a Slider widget.");
impl_handle!(ProgressBarHandle, "Type-safe handle for a ProgressBar widget.");
impl_handle!(PanelHandle, "Type-safe handle for a Panel widget.");
impl_handle!(SpinBoxHandle, "Type-safe handle for a SpinBox widget.");
impl_handle!(ListViewHandle, "Type-safe handle for a ListView widget.");
impl_handle!(ScrollAreaHandle, "Type-safe handle for a ScrollArea widget.");
impl_handle!(TextEditHandle, "Type-safe handle for a TextEdit (multi-line text) widget.");
impl_handle!(ScrollBarHandle, "Type-safe handle for a ScrollBar widget.");
impl_handle!(TabWidgetHandle, "Type-safe handle for a TabWidget (tab container) widget.");
impl_handle!(GridWidgetHandle, "Type-safe handle for a GridWidget (grid layout) widget.");
impl_handle!(FrameHandle, "Type-safe handle for a Frame widget.");
impl_handle!(DialogHandle, "Type-safe handle for a generic Dialog widget.");
impl_handle!(WebViewHandle, "Type-safe handle for a WebView (web content) widget.");
impl_handle!(MenuBarHandle, "Type-safe handle for a native menu bar.");
impl_handle!(MenuHandle, "Type-safe handle for a native menu (a menu-bar entry).");
impl_handle!(MenuItemHandle, "Type-safe handle for a native menu item.");
impl_handle!(ToolBarHandle, "Type-safe handle for a native tool bar.");
impl_handle!(StatusBarHandle, "Type-safe handle for a native status bar.");

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
    pub fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    pub fn raw_id(&self) -> ObjectId {
        self.id
    }

    /// Show the message-box modally.
    pub fn show_modal(&self) {
        crate::show_widget(self.id);
    }

    /// Dismiss the message-box.
    pub fn close(&self) {
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

    fn on_click<F: FnMut() + 'static>(&self, f: F) {
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }

    fn on_value_changed<F: FnMut(String) + 'static>(&self, f: F) {
        VALUE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(self.id, Rc::new(RefCell::new(f)));
        });
    }
}

impl Drop for MessageBoxHandle {
    fn drop(&mut self) {
        remove_callbacks(self.id);
    }
}

// ═══════════════════════════════════════════════════════════════
// ComboBoxHandle – extended with combo-specific operations
// ═══════════════════════════════════════════════════════════════

/// # Combo-box specific operations
impl ComboBoxHandle {
    pub fn add_item(&self, text: &str) -> bool {
        crate::combo_box_add_item(self.raw_id(), text)
    }

    pub fn clear_items(&self) -> bool {
        crate::combo_box_clear_items(self.raw_id())
    }

    pub fn set_current_index(&self, index: usize) -> bool {
        crate::combo_box_set_current_index(self.raw_id(), index)
    }

    pub fn current_index(&self) -> Option<usize> {
        crate::combo_box_current_index(self.raw_id())
    }

    pub fn item_count(&self) -> usize {
        crate::combo_box_item_count(self.raw_id())
    }

    pub fn item_text(&self, index: usize) -> Option<String> {
        crate::combo_box_item_text(self.raw_id(), index)
    }
}

// ═══════════════════════════════════════════════════════════════
// ListBoxHandle – extended with list-specific operations
// ═══════════════════════════════════════════════════════════════

/// # List-box specific operations
impl ListBoxHandle {
    pub fn add_item(&self, text: &str) -> bool {
        crate::list_box_add_item(self.raw_id(), text)
    }

    pub fn remove_item(&self, index: usize) -> bool {
        crate::list_box_remove_item(self.raw_id(), index)
    }

    pub fn clear_items(&self) -> bool {
        crate::list_box_clear_items(self.raw_id())
    }

    pub fn set_current_index(&self, index: usize) -> bool {
        crate::list_box_set_current_index(self.raw_id(), index)
    }

    pub fn current_index(&self) -> Option<usize> {
        crate::list_box_current_index(self.raw_id())
    }

    pub fn item_count(&self) -> usize {
        crate::list_box_item_count(self.raw_id())
    }

    pub fn item_text(&self, index: usize) -> Option<String> {
        crate::list_box_item_text(self.raw_id(), index)
    }
}

// ═══════════════════════════════════════════════════════════════
// SliderHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct SliderState {
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    orientation: Orientation,
}

impl Default for SliderState {
    fn default() -> Self {
        Self { value: 50, min: 0, max: 100, step: 1, orientation: Orientation::Horizontal }
    }
}

thread_local! {
    static SLIDER_STATES: RefCell<HashMap<ObjectId, SliderState>> = RefCell::new(HashMap::new());
}

/// # Slider-specific operations
impl SliderHandle {
    /// Set the current slider value (clamped to min/max range).
    ///
    /// The value is written both to the in-process mirror and to the native
    /// control through [`crate::Platform::set_widget_value`], so the change is
    /// visible on screen and not just to `value()`. On a backend without a
    /// numeric value for sliders the mirror is still updated, because it is the
    /// authoritative copy for the self-drawn path.
    pub fn set_value(&self, value: i32) {
        SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
        crate::platform::get_platform().set_widget_value(self.raw_id(), f64::from(self.value()));
    }

    /// Return the current slider value.
    ///
    /// The in-process mirror is authoritative: it is what the self-drawn path
    /// renders and what a native write was clamped to, so both agree.
    pub fn value(&self) -> i32 {
        SLIDER_STATES.with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(50))
    }

    /// Set the slider range (min/max). The current value is clamped.
    pub fn set_range(&self, min: i32, max: i32) {
        SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
        });
        crate::platform::get_platform().set_widget_range(
            self.raw_id(),
            f64::from(min),
            f64::from(max),
        );
    }

    /// Set the slider step increment.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_step`].
    pub fn set_step(&self, step: i32) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
        crate::platform::get_platform().set_widget_step(self.raw_id(), f64::from(step));
    }

    /// Set the slider orientation.
    ///
    /// This affects the **self-drawn** rendering only. The native backends do not
    /// expose a uniform post-creation orientation change — GTK has
    /// `set_orientation`, but AppKit encodes it in the class (`NSSlider` vs a
    /// vertical variant) and Win32 in the creation style (`TBS_VERT`) — so there
    /// is no cross-OS setter to call. Changing orientation on a native slider is
    /// therefore not reflected on screen; recreate the control instead. This is a
    /// deliberate per-OS limitation, not a silently dropped write: the mirror is
    /// authoritative for the self-drawn path, and native callers are told here.
    pub fn set_orientation(&self, orientation: Orientation) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).orientation =
                orientation;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ProgressBarHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct ProgressBarState {
    value: u32,
    min: u32,
    max: u32,
    indeterminate: bool,
}

impl Default for ProgressBarState {
    fn default() -> Self {
        Self { value: 0, min: 0, max: 100, indeterminate: false }
    }
}

thread_local! {
    static PROGRESS_BAR_STATES: RefCell<HashMap<ObjectId, ProgressBarState>> = RefCell::new(HashMap::new());
}

/// # Progress-bar specific operations
impl ProgressBarHandle {
    /// Set the current progress value (clamped to min/max).
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_value`], so the bar actually moves.
    pub fn set_value(&self, value: u32) {
        PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
        crate::platform::get_platform().set_widget_value(self.raw_id(), f64::from(self.value()));
    }

    /// Return the current progress value.
    pub fn value(&self) -> u32 {
        PROGRESS_BAR_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the minimum value.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_range`].
    pub fn set_min(&self, min: u32) {
        let range = PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.value = state.value.clamp(state.min, state.max);
            (state.min, state.max)
        });
        crate::platform::get_platform().set_widget_range(
            self.raw_id(),
            f64::from(range.0),
            f64::from(range.1),
        );
    }

    /// Set the maximum value.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_range`].
    pub fn set_max(&self, max: u32) {
        let range = PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
            (state.min, state.max)
        });
        crate::platform::get_platform().set_widget_range(
            self.raw_id(),
            f64::from(range.0),
            f64::from(range.1),
        );
    }

    /// Set whether the progress bar is in indeterminate mode.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_indeterminate`], so a native bar
    /// animates instead of showing a fixed fraction.
    pub fn set_indeterminate(&self, indeterminate: bool) {
        PROGRESS_BAR_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).indeterminate =
                indeterminate;
        });
        crate::platform::get_platform().set_widget_indeterminate(self.raw_id(), indeterminate);
    }
}

// ═══════════════════════════════════════════════════════════════
// CheckBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct CheckBoxState {
    checked: bool,
    tristate: bool,
    check_state: CheckState,
}

impl Default for CheckBoxState {
    fn default() -> Self {
        Self { checked: false, tristate: false, check_state: CheckState::Unchecked }
    }
}

thread_local! {
    static CHECKBOX_STATES: RefCell<HashMap<ObjectId, CheckBoxState>> = RefCell::new(HashMap::new());
}

/// # Check-box specific operations
impl CheckBoxHandle {
    /// Return whether the check-box is checked (non-tristate mode).
    pub fn is_checked(&self) -> bool {
        CHECKBOX_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.checked).unwrap_or(false))
    }

    /// Set the check-box to checked or unchecked.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_checked`], so the box really moves.
    pub fn set_checked(&self, checked: bool) {
        CHECKBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.checked = checked;
            if !state.tristate {
                state.check_state =
                    if checked { CheckState::Checked } else { CheckState::Unchecked };
            }
        });
        crate::platform::get_platform().set_widget_checked(self.raw_id(), checked);
    }

    /// Enable/disable tri-state mode.
    ///
    /// **Not uniformly native.** Win32 has `BS_3STATE`/`BS_AUTO3STATE` and GTK's
    /// `ToggleButton` exposes `set_inconsistent`, but AppKit's `NSButton` has no
    /// third state for a check box. The crate has no `set_widget_tristate`
    /// capability for that reason, so this setter drives the self-drawn path only.
    /// Enabling it does **not** turn a native check box into a tri-state control.
    pub fn set_tristate(&self, tristate: bool) {
        CHECKBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).tristate =
                tristate;
        });
    }

    /// Return the current check state of a tri-state check-box.
    pub fn check_state(&self) -> CheckState {
        CHECKBOX_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).map(|s| s.check_state).unwrap_or(CheckState::Unchecked)
        })
    }
}

// ═══════════════════════════════════════════════════════════════
// RadioButtonHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct RadioButtonState {
    selected: bool,
    group: String,
}

thread_local! {
    static RADIO_BUTTON_STATES: RefCell<HashMap<ObjectId, RadioButtonState>> = RefCell::new(HashMap::new());
}

/// # Radio-button specific operations
impl RadioButtonHandle {
    /// Return whether this radio button is currently selected.
    pub fn is_selected(&self) -> bool {
        RADIO_BUTTON_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.selected).unwrap_or(false))
    }

    /// Select this radio button and deselect all others in the same group.
    ///
    /// Both the selection *and* the de-selection of siblings are pushed to the
    /// native controls through [`crate::Platform::set_widget_checked`], so the
    /// group is actually mutually exclusive on screen rather than only in the
    /// in-process mirror.
    pub fn select(&self) {
        let group = RADIO_BUTTON_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.selected = true;
            state.group.clone()
        });
        crate::platform::get_platform().set_widget_checked(self.raw_id(), true);

        // Deselect all other radio buttons in the same group.
        if !group.is_empty() {
            let siblings: Vec<ObjectId> = RADIO_BUTTON_STATES.with(|map| {
                let mut map = map.borrow_mut();
                let mut siblings = Vec::new();
                for (id, state) in map.iter_mut() {
                    if *id != self.raw_id() && state.group == group {
                        state.selected = false;
                        siblings.push(*id);
                    }
                }
                siblings
            });
            // Clear the native state outside the borrow above so a backend
            // callback cannot re-enter the map while it is borrowed.
            let platform = crate::platform::get_platform();
            for id in siblings {
                platform.set_widget_checked(id, false);
            }
        }
    }

    /// Set the group name for this radio button.
    /// Radio buttons in the same group are mutually exclusive.
    pub fn set_group(&self, group: &str) {
        RADIO_BUTTON_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).group =
                group.to_owned();
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// LineEditHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct LineEditState {
    placeholder: String,
    read_only: bool,
    max_length: u32,
    echo_mode: EchoMode,
    selection_start: u32,
    selection_end: u32,
    select_all: bool,
}

impl Default for LineEditState {
    fn default() -> Self {
        Self {
            placeholder: String::new(),
            read_only: false,
            max_length: 32767,
            echo_mode: EchoMode::Normal,
            selection_start: 0,
            selection_end: 0,
            select_all: false,
        }
    }
}

thread_local! {
    static LINE_EDIT_STATES: RefCell<HashMap<ObjectId, LineEditState>> = RefCell::new(HashMap::new());
}

/// # Line-edit specific operations
impl LineEditHandle {
    /// Set the placeholder text shown when the field is empty.
    pub fn set_placeholder(&self, text: &str) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).placeholder =
                text.to_owned();
        });
    }

    /// Set whether the line-edit is read-only.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_read_only`], so the native field
    /// actually stops accepting input. On a backend whose text control is not an
    /// entry, the platform call reports `false` and only the mirror changes —
    /// that is a legitimate per-OS difference, not a dropped write.
    pub fn set_read_only(&self, read_only: bool) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).read_only =
                read_only;
        });
        crate::platform::get_platform().set_widget_read_only(self.raw_id(), read_only);
    }

    /// Set the maximum number of characters allowed.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_max_length`]. Not every OS text
    /// control has a settable limit (AppKit's `NSTextField` does not), so the
    /// platform call may report `false`; the mirror still updates for the
    /// self-drawn path and for callers that enforce the limit themselves.
    pub fn set_max_length(&self, len: u32) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).max_length = len;
        });
        crate::platform::get_platform().set_widget_max_length(self.raw_id(), len);
    }

    /// Clear the line-edit text.
    pub fn clear(&self) {
        crate::set_widget_text(self.raw_id(), "");
    }

    /// Set the echo mode (Normal / Password / NoEcho).
    pub fn set_echo_mode(&self, mode: EchoMode) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).echo_mode = mode;
        });
    }

    /// Return the placeholder text set via [`Self::set_placeholder`].
    pub fn placeholder(&self) -> String {
        LINE_EDIT_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).map(|s| s.placeholder.clone()).unwrap_or_default()
        })
    }

    /// Return whether the line-edit is read-only.
    pub fn is_read_only(&self) -> bool {
        LINE_EDIT_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.read_only).unwrap_or(false))
    }

    /// Return the maximum number of characters allowed (default 32767).
    pub fn max_length(&self) -> u32 {
        LINE_EDIT_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.max_length).unwrap_or(32767))
    }

    /// Return the current echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        LINE_EDIT_STATES.with(|map| {
            map.borrow().get(&self.raw_id()).map(|s| s.echo_mode).unwrap_or(EchoMode::Normal)
        })
    }

    /// Return the current selection range `(start, end)`.
    ///
    /// When [`Self::select_all`] was used, the range is `(0, u32::MAX)` and
    /// [`Self::is_select_all`] reports `true`.
    pub fn selection(&self) -> (u32, u32) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow()
                .get(&self.raw_id())
                .map(|s| (s.selection_start, s.selection_end))
                .unwrap_or((0, 0))
        })
    }

    /// Return whether the whole text range is selected via [`Self::select_all`].
    pub fn is_select_all(&self) -> bool {
        LINE_EDIT_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.select_all).unwrap_or(false))
    }

    /// Select all text in the line-edit.
    pub fn select_all(&self) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = true;
            state.selection_start = 0;
            state.selection_end = u32::MAX;
        });
    }

    /// Set the selection range (start..end).
    pub fn set_selection(&self, start: u32, end: u32) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = false;
            state.selection_start = start;
            state.selection_end = end;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ScrollAreaHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct ScrollAreaState {
    scroll_x: i32,
    scroll_y: i32,
    content_w: u32,
    content_h: u32,
}

thread_local! {
    static SCROLL_AREA_STATES: RefCell<HashMap<ObjectId, ScrollAreaState>> = RefCell::new(HashMap::new());
}

/// # Scroll-area specific operations
impl ScrollAreaHandle {
    /// Set the scroll offset.
    pub fn set_scroll_position(&self, x: i32, y: i32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.scroll_x = x;
            state.scroll_y = y;
        });
    }

    /// Return the current scroll offset.
    pub fn scroll_position(&self) -> (i32, i32) {
        SCROLL_AREA_STATES.with(|map| {
            let map = map.borrow();
            map.get(&self.raw_id()).map(|s| (s.scroll_x, s.scroll_y)).unwrap_or((0, 0))
        })
    }

    /// Set the content size (in virtual pixels).
    pub fn set_content_size(&self, w: u32, h: u32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.content_w = w;
            state.content_h = h;
        });
    }

    /// Scroll to the bottom of the content.
    pub fn scroll_to_bottom(&self) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.get(&self.raw_id()).cloned().unwrap_or_default();
            let state_mut = map.entry(self.raw_id()).or_default();
            state_mut.scroll_y = state.content_h as i32;
        });
    }

    /// Scroll to the top of the content.
    pub fn scroll_to_top(&self) {
        SCROLL_AREA_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).scroll_y = 0;
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// ListViewHandle – extended state
// ═══════════════════════════════════════════════════════════════

struct ListViewState {
    columns: Vec<(String, u32)>,
    model: Option<Rc<dyn ListModel>>,
    selected_row: Option<usize>,
    selection_mode: SelectionMode,
}

impl Default for ListViewState {
    fn default() -> Self {
        Self {
            columns: Vec::new(),
            model: None,
            selected_row: None,
            selection_mode: SelectionMode::Single,
        }
    }
}

thread_local! {
    static LIST_VIEW_STATES: RefCell<HashMap<ObjectId, ListViewState>> = RefCell::new(HashMap::new());
}

/// # List-view specific operations
impl ListViewHandle {
    /// Add a column with the given title and width.
    pub fn add_column(&self, title: &str, width: u32) {
        LIST_VIEW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.columns.push((title.to_owned(), width));
        });
    }

    /// Set the data model for this list view.
    pub fn set_model(&self, model: Box<dyn ListModel>) {
        let model: Rc<dyn ListModel> = Rc::from(model);
        LIST_VIEW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).model =
                Some(model);
        });
    }

    /// Return the currently selected row, if any.
    pub fn selected_row(&self) -> Option<usize> {
        LIST_VIEW_STATES.with(|map| map.borrow().get(&self.raw_id()).and_then(|s| s.selected_row))
    }

    /// Set the selection mode.
    pub fn set_selection_mode(&self, mode: SelectionMode) {
        LIST_VIEW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).selection_mode =
                mode;
        });
    }

    /// Return the current model, if one is set.
    /// Returns `None` if no model has been assigned.
    pub fn model(&self) -> Option<Rc<dyn ListModel>> {
        LIST_VIEW_STATES.with(|map| map.borrow().get(&self.raw_id()).and_then(|s| s.model.clone()))
    }
}

// ═══════════════════════════════════════════════════════════════
// SpinBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct SpinBoxState {
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    prefix: String,
    suffix: String,
}

impl Default for SpinBoxState {
    fn default() -> Self {
        Self { value: 0, min: 0, max: 100, step: 1, prefix: String::new(), suffix: String::new() }
    }
}

thread_local! {
    static SPINBOX_STATES: RefCell<HashMap<ObjectId, SpinBoxState>> = RefCell::new(HashMap::new());
}

/// # Spin-box specific operations
impl SpinBoxHandle {
    /// Set the spin-box value (clamped to range).
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_value`], so the number the user sees
    /// matches `value()`.
    pub fn set_value(&self, value: i32) {
        SPINBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = value.clamp(state.min, state.max);
        });
        crate::platform::get_platform().set_widget_value(self.raw_id(), f64::from(self.value()));
    }

    /// Return the current spin-box value.
    pub fn value(&self) -> i32 {
        SPINBOX_STATES.with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the spin-box range. The current value is clamped.
    pub fn set_range(&self, min: i32, max: i32) {
        SPINBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.max = max;
            state.value = state.value.clamp(state.min, state.max);
        });
        crate::platform::get_platform().set_widget_range(
            self.raw_id(),
            f64::from(min),
            f64::from(max),
        );
    }

    /// Set the prefix text displayed before the value.
    ///
    /// **Self-drawn only.** There is no cross-OS native control with an affix
    /// concept — AppKit `NSStepper`/`NSTextField`, Win32 `UPDOWN_CLASS` and GTK
    /// `SpinButton` all render a bare number, and formatting is left to the
    /// application. This setter therefore changes what the self-drawn path
    /// renders, and has no effect on a native control. Callers that need an
    /// affix on a native control must format the value themselves where the value
    /// is consumed; this is a genuine platform gap rather than a dropped write.
    pub fn set_prefix(&self, prefix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).prefix =
                prefix.to_owned();
        });
    }

    /// Set the suffix text displayed after the value.
    ///
    /// **Self-drawn only** — see [`SpinBoxHandle::set_prefix`] for why no native
    /// backend can honour this.
    pub fn set_suffix(&self, suffix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).suffix =
                suffix.to_owned();
        });
    }

    /// Set the spin-box step increment.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::Platform::set_widget_step`].
    pub fn set_step(&self, step: i32) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
        crate::platform::get_platform().set_widget_step(self.raw_id(), f64::from(step));
    }
}

// ═══════════════════════════════════════════════════════════════
// PanelHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
struct PanelState {
    title: String,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl PanelState {
    fn with_geometry(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { title: String::new(), x, y, w, h }
    }
}

thread_local! {
    static PANEL_LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
    static PANEL_STATES: RefCell<HashMap<ObjectId, PanelState>> = RefCell::new(HashMap::new());
}

/// # Panel-specific operations
impl PanelHandle {
    /// Set panel geometry and reapply the active child layout.
    pub fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        PANEL_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.x = x;
            state.y = y;
            state.w = w;
            state.h = h;
        });
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
        apply_panel_layout(self.raw_id());
    }

    /// Set the layout manager for this panel.
    ///
    /// The layout is stored internally and used to reposition children.
    /// Only one layout can be active at a time.
    pub fn set_layout(&self, layout: Box<dyn crate::layout::Layout>) {
        PANEL_LAYOUTS.with(|map| {
            map.borrow_mut().insert(self.raw_id(), layout);
        });
        apply_panel_layout(self.raw_id());
    }

    /// Set the panel title text.
    pub fn set_title(&self, title: &str) {
        PANEL_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).title =
                title.to_owned();
        });
        crate::set_widget_text(self.raw_id(), title);
    }
}

fn apply_panel_layout(panel_id: ObjectId) {
    let Some(rect) = PANEL_STATES.with(|map| {
        map.borrow().get(&panel_id).map(|state| Rect::new(state.x, state.y, state.w, state.h))
    }) else {
        return;
    };

    let child_geometries = PANEL_LAYOUTS.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&panel_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update(rect, &mut |widget_id, geometry| {
            geometries.push((widget_id, geometry));
        });
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
    }
}

// ═══════════════════════════════════════════════════════════════
// WindowHandle – extended state (new methods beyond factories)
// ═══════════════════════════════════════════════════════════════

#[derive(Clone)]
struct WindowState {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    icon: String,
    min_w: u32,
    min_h: u32,
    maximized: bool,
    minimized: bool,
    fullscreen: bool,
    resizable: bool,
    decorated: bool,
    close_callback: Option<ClickCallback>,
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
            .field("close_callback", &self.close_callback.as_ref().map(|_| "<fn>"))
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
            close_callback: None,
        }
    }
}

thread_local! {
    static WINDOW_STATES: RefCell<HashMap<ObjectId, WindowState>> = RefCell::new(HashMap::new());
}

/// # Window-specific state operations
impl WindowHandle {
    /// Return the window title.
    pub fn title(&self) -> String {
        crate::get_widget_text(self.raw_id())
    }

    /// Set the window icon from a file path.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_icon`] (AppKit
    /// `setRepresentation:`, Win32 `WM_SETICON`, GTK `set_icon_from_file`) as well
    /// as into the in-process mirror. Returns `false` when the backend could not
    /// load the file, so a bad path is visible instead of silently ignored.
    pub fn set_icon(&self, path: &str) -> bool {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).icon =
                path.to_owned();
        });
        crate::platform::get_platform().set_window_icon(self.raw_id(), path)
    }

    /// Set the minimum window size.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_min_size`]
    /// (`setContentMinSize:` on macOS, the `WM_GETMINMAXINFO` handler on Windows,
    /// `set_geometry_hints` on GTK) as well as into the in-process mirror.
    pub fn set_min_size(&self, w: u32, h: u32) -> bool {
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min_w = w;
            state.min_h = h;
        });
        crate::platform::get_platform().set_window_min_size(self.raw_id(), w, h)
    }

    /// Read the minimum window size the backend has recorded, if any.
    pub fn min_size(&self) -> Option<(u32, u32)> {
        crate::platform::get_platform().window_min_size(self.raw_id())
    }

    /// Read the icon path this window was given, if any.
    pub fn icon(&self) -> Option<String> {
        crate::platform::get_platform().window_icon(self.raw_id())
    }

    /// Maximize or restore the window.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_state`]
    /// (`zoom:` on macOS, `ShowWindow(SW_MAXIMIZE)` on Windows, `maximize()` on
    /// GTK) as well as into the in-process mirror. `is_maximized` reports what
    /// the OS window actually is when a native window exists.
    pub fn set_maximized(&self, maximized: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).maximized =
                maximized;
        });
        crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Maximized,
            maximized,
        );
    }

    /// Return whether the window is maximized.
    pub fn is_maximized(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Maximized)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.maximized, false))
    }

    /// Minimize or restore the window.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_state`]
    /// (`miniaturize:`/`deminiaturize:` on macOS, `SW_MINIMIZE`/`SW_RESTORE` on
    /// Windows, `iconify()`/`deiconify()` on GTK).
    pub fn set_minimized(&self, minimized: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).minimized =
                minimized;
        });
        crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Minimized,
            minimized,
        );
    }

    /// Return whether the window is minimized.
    pub fn is_minimized(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Minimized)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.minimized, false))
    }

    /// Set fullscreen mode.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_state`]
    /// (`toggleFullScreen:` on macOS, frame-style manipulation on Windows,
    /// `fullscreen()`/`unfullscreen()` on GTK).
    pub fn set_fullscreen(&self, fullscreen: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).fullscreen =
                fullscreen;
        });
        crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Fullscreen,
            fullscreen,
        );
    }

    /// Return whether the window is fullscreen.
    pub fn is_fullscreen(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Fullscreen)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.fullscreen, false))
    }

    /// Set whether the window is resizable.
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_state`], which
    /// toggles `NSWindowStyleMaskResizable` on macOS, `WS_THICKFRAME` on
    /// Windows, and `set_resizable` on GTK.
    pub fn set_resizable(&self, resizable: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).resizable =
                resizable;
        });
        crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Resizable,
            resizable,
        );
    }

    /// Return whether the window is resizable.
    pub fn is_resizable(&self) -> bool {
        crate::platform::get_platform()
            .is_window_in_state(self.raw_id(), WindowStateFlag::Resizable)
            .unwrap_or_else(|| self.mirrored_flag(|s| s.resizable, true))
    }

    /// Set whether the window has window decorations (title bar, borders).
    ///
    /// Pushed to the OS through [`crate::Platform::set_window_state`], which
    /// toggles `NSWindowStyleMaskTitled` on macOS, `WS_CAPTION` on Windows, and
    /// `set_decorated` on GTK.
    pub fn set_decorated(&self, decorated: bool) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).decorated =
                decorated;
        });
        crate::platform::get_platform().set_window_state(
            self.raw_id(),
            WindowStateFlag::Decorated,
            decorated,
        );
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
    pub fn on_close(&self, callback: ClickCallback) {
        WINDOW_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).close_callback =
                Some(callback);
        });
    }

    /// Programmatically close the window.
    pub fn close(&self) {
        // Invoke the close callback if one is registered.
        WINDOW_STATES.with(|map| {
            let mut map = map.borrow_mut();
            if let Some(state) = map.get_mut(&self.raw_id()) {
                if let Some(cb) = &state.close_callback {
                    (cb.borrow_mut())();
                }
            }
        });
        crate::hide_widget(self.raw_id());
    }

    /// Center the window on the screen.
    ///
    /// Uses a default virtual screen size of 1920×1080 as a fallback.
    /// Real platforms should query the actual screen geometry.
    pub fn center_on_screen(&self) {
        // Query the stored window geometry to preserve current size.
        let (win_w, win_h) = WINDOW_STATES.with(|map| {
            let state = map.borrow().get(&self.raw_id()).cloned().unwrap_or_default();
            (state.w, state.h)
        });
        let screen_w = 1920i32;
        let screen_h = 1080i32;
        crate::set_widget_geometry(
            self.raw_id(),
            (screen_w - win_w as i32) / 2,
            (screen_h - win_h as i32) / 2,
            win_w,
            win_h,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ObjectId;
    use alloc::rc::Rc;
    use core::cell::RefCell;

    #[test]
    fn remove_callbacks_cleans_up() {
        let id: ObjectId = 42;
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(id, Rc::new(RefCell::new(|| {})));
            assert!(map.borrow().contains_key(&id));
        });
        remove_callbacks(id);
        CLICK_CALLBACKS.with(|map| {
            assert!(!map.borrow().contains_key(&id));
        });
    }

    #[test]
    fn line_edit_handle_state_defaults() {
        let id: ObjectId = 5001;
        let handle = LineEditHandle::from_raw(id);
        assert_eq!(handle.placeholder(), "");
        assert!(!handle.is_read_only());
        assert_eq!(handle.max_length(), 32767);
        assert_eq!(handle.echo_mode(), EchoMode::Normal);
        assert_eq!(handle.selection(), (0, 0));
        assert!(!handle.is_select_all());
    }

    #[test]
    fn line_edit_handle_state_roundtrip() {
        let id: ObjectId = 5002;
        let handle = LineEditHandle::from_raw(id);
        handle.set_placeholder("Type here...");
        handle.set_read_only(true);
        handle.set_max_length(64);
        handle.set_echo_mode(EchoMode::Password);

        assert_eq!(handle.placeholder(), "Type here...");
        assert!(handle.is_read_only());
        assert_eq!(handle.max_length(), 64);
        assert_eq!(handle.echo_mode(), EchoMode::Password);
    }

    #[test]
    fn line_edit_handle_selection_semantics() {
        let id: ObjectId = 5003;
        let handle = LineEditHandle::from_raw(id);

        // select_all sets an unbounded range and the select-all marker.
        handle.select_all();
        assert!(handle.is_select_all());
        assert_eq!(handle.selection(), (0, u32::MAX));

        // A precise selection clears the select-all marker.
        handle.set_selection(2, 5);
        assert!(!handle.is_select_all());
        assert_eq!(handle.selection(), (2, 5));
    }

    #[test]
    fn line_edit_handle_states_are_scoped_per_widget() {
        let a = LineEditHandle::from_raw(5004);
        let b = LineEditHandle::from_raw(5005);
        a.set_max_length(8);
        b.set_read_only(true);
        assert_eq!(a.max_length(), 8);
        assert!(!a.is_read_only());
        assert_eq!(b.max_length(), 32767);
        assert!(b.is_read_only());
    }
}
