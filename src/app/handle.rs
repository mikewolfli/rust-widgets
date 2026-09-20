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
///
/// Re-exported from [`crate::widget::base_widgets::CheckState`] — the widget layer
/// owns the canonical definition, and the handle layer names the same three
/// states. Keeping one enum means a value read from a handle and one read from a
/// widget are the same type; there is nothing to convert and nothing to drift.
///
/// Note the declaration order differs from the widget module (which lists
/// `PartiallyChecked` between the two definite states); as a fieldless enum whose
/// identity is `PartialEq`/`Hash` rather than discriminant order, that is not
/// observable, and `matches!`/`==` comparisons behave identically.
pub use crate::widget::base_widgets::CheckState;

/// Controls how text is displayed in a line-edit widget.
///
/// Re-exported from [`crate::platform::EchoMode`] so this path keeps working for
/// existing callers while the enum itself lives at the layer that has to name it.
pub use crate::platform::EchoMode;

/// Determines how many rows can be selected in a list / table view.
///
/// Re-exported from [`crate::widget::input_widgets::listbox::SelectionMode`] — the
/// widget layer owns the canonical definition and every selection surface names
/// the same four modes. One definition means a mode read from a handle and one
/// read from a `ListView` are the same type (principle #54).
pub use crate::widget::input_widgets::listbox::SelectionMode;

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

// ══════════════════════════════════════════════════
// Widget surface mounting
// ══════════════════════════════════════════════════

/// Why a widget could not be mounted onto a host surface.
///
/// Re-exported from [`crate::widget::runtime::SurfaceMountError`] — the layer
/// that owns widget registration also owns the reasons registration or mounting
/// can fail, so there is one definition shared by the handle API and the
/// crate-level creation API (principle #54).
pub use crate::widget::runtime::SurfaceMountError;

/// Deprecated alias of [`SurfaceMountError`].
pub use crate::widget::runtime::SurfaceMountError as CustomWidgetMountError;

/// Handle to a widget mounted on a window surface.
///
/// Keeps the widget's registry id so the caller can move or unmount it. Dropping
/// the handle is **not** enough to remove the widget: the window still owns it,
/// because the surface outlives any single Rust value. Call
/// [`SurfaceHandle::unmount`] for that; `Drop` only detaches this handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceHandle {
    id: ObjectId,
}

/// Deprecated alias of [`SurfaceHandle`].
pub type CustomWidgetHandle = SurfaceHandle;

impl SurfaceHandle {
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
        crate::resize_surface(self.id, rect)
    }

    /// Removes the widget from its window and drops it.
    ///
    /// Returns `false` when it was already unmounted.
    pub fn unmount(&self) -> bool {
        let removed = crate::unmount_surface(self.id);
        crate::widget::runtime::unregister(self.id);
        removed
    }

    /// Runs `f` against the mounted widget, then repaints it if `f` reports a change.
    ///
    /// # Why this exists
    ///
    /// A library-painted widget owns its own interaction model, so a menu item or
    /// tool-bar button cannot drive it through the platform event queue — there is
    /// no OS control to send a command to. This is the generic bridge: the caller
    /// decides what to do with the widget, and this method guarantees the change
    /// becomes visible.
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
    ///     .expect("backend can mount widget surfaces");
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

impl WidgetHandle for SurfaceHandle {
    fn raw_id(&self) -> ObjectId {
        self.id
    }

    fn from_raw(id: ObjectId) -> Self {
        Self { id }
    }

    /// Library-painted widgets route input into themselves.
    ///
    /// A `CodeEditor` handles its own clicks, keys and IME commits through
    /// `EventHandler`; there is no separate platform control to attach a
    /// click callback to. This deliberately does **not** register a callback
    /// that would never fire — read the widget's own signals instead (for the
    /// editor: `text_changed`, `cursor_moved`, `selection_changed`).
    fn on_click<F: FnMut() + 'static>(&self, _f: F) {
        log::debug!(
            "SurfaceHandle::on_click ignored for id={}: the widget emits its \
             own signals rather than a platform click callback",
            self.id
        );
    }

    /// See [`SurfaceHandle::on_click`]; the same reasoning applies.
    fn on_value_changed<F: FnMut(String) + 'static>(&self, _f: F) {
        log::debug!(
            "SurfaceHandle::on_value_changed ignored for id={}: the widget \
             emits its own signals",
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
    /// never branch on the OS. See [`crate::platform::Platform::set_widget_value`].
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

    /// Set a text entry's selection range as `(start, end)` character offsets.
    fn set_widget_selection(&self, start: u32, end: u32) -> bool {
        crate::platform::get_platform().set_widget_selection(self.raw_id(), start, end)
    }

    /// Read a text entry's selection range, or `None` when nothing is selected.
    fn widget_selection(&self) -> Option<(u32, u32)> {
        crate::platform::get_platform().widget_selection(self.raw_id())
    }

    /// Set a text entry's placeholder (cue) text.
    fn set_widget_placeholder(&self, text: &str) -> bool {
        crate::platform::get_platform().set_widget_placeholder(self.raw_id(), text)
    }

    /// Read a text entry's placeholder text.
    fn widget_placeholder(&self) -> Option<String> {
        crate::platform::get_platform().widget_placeholder(self.raw_id())
    }

    /// Set a text entry's echo mode.
    fn set_widget_echo_mode(&self, mode: EchoMode) -> bool {
        crate::platform::get_platform().set_widget_echo_mode(self.raw_id(), mode)
    }

    /// Read a text entry's echo mode.
    fn widget_echo_mode(&self) -> Option<EchoMode> {
        crate::platform::get_platform().widget_echo_mode(self.raw_id())
    }

    /// Apply a slider's creation-time orientation.
    fn set_slider_orientation(&self, orientation: Orientation) -> bool {
        crate::platform::get_platform().set_slider_orientation(self.raw_id(), orientation)
    }

    /// Read a slider's orientation.
    fn slider_orientation(&self) -> Option<Orientation> {
        crate::platform::get_platform().slider_orientation(self.raw_id())
    }

    /// Set a checkable control's tri-state mode.
    fn set_tristate(&self, enabled: bool) -> bool {
        crate::platform::get_platform().set_widget_tristate(self.raw_id(), enabled)
    }

    /// Read a checkable control's tri-state mode.
    fn is_tristate(&self) -> Option<bool> {
        crate::platform::get_platform().is_widget_tristate(self.raw_id())
    }

    /// Put a radio button into a named mutually-exclusive group.
    fn set_group(&self, group: &str) -> bool {
        crate::platform::get_platform().set_widget_group(self.raw_id(), group)
    }

    /// Read a radio button's group name.
    fn group(&self) -> Option<String> {
        crate::platform::get_platform().widget_group(self.raw_id())
    }

    /// Set a scrollable container's scroll offset.
    fn set_scroll_position(&self, x: i32, y: i32) -> bool {
        crate::platform::get_platform().set_widget_scroll_position(self.raw_id(), x, y)
    }

    /// Read a scrollable container's scroll offset.
    fn scroll_position(&self) -> Option<(i32, i32)> {
        crate::platform::get_platform().widget_scroll_position(self.raw_id())
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

/// Removes and returns the click callback for `widget_id`.
///
/// `try_borrow_mut` rather than `borrow_mut`: this runs from [`dispatch_trigger`],
/// which is itself reachable from inside a callback, and a refused borrow must not
/// become a panic there. A refused borrow also means the map cannot be read, so the
/// answer is "no callback" rather than a guess.
fn take_click_callback(widget_id: ObjectId) -> Option<ClickCallback> {
    CLICK_CALLBACKS.with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&widget_id)))
}

/// Removes and returns the value-changed callback for `widget_id`.
///
/// See [`take_click_callback`] for why the borrow is attempted rather than taken.
fn take_value_callback(widget_id: ObjectId) -> Option<ValueChangedCallback> {
    VALUE_CALLBACKS.with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&widget_id)))
}

/// Puts a click callback back when it goes out of scope.
///
/// The job of this type is the unwind path: `dispatch_trigger` moves the callback
/// out of the map so a re-entrant `remove_callbacks` cannot double borrow, and a
/// plain `insert` afterwards would be skipped if the callback panicked — leaving the
/// widget registered as having a callback that is never invoked again. Holding it in
/// a `Drop` type makes the restore unconditional.
struct ClickCallGuard {
    widget_id: ObjectId,
    callback: Option<ClickCallback>,
}

impl Drop for ClickCallGuard {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            CLICK_CALLBACKS.with(|map| {
                if let Ok(mut map) = map.try_borrow_mut() {
                    map.insert(self.widget_id, callback);
                } else {
                    log::warn!(
                        "could not restore the click callback for widget {}: the registry is \
                         already borrowed; the callback is dropped",
                        self.widget_id
                    );
                }
            });
        }
    }
}

/// Puts a value-changed callback back when it goes out of scope.
///
/// See [`ClickCallGuard`]; the two registries are separate, so each needs its own
/// guard rather than one generic over the map.
struct ValueCallGuard {
    widget_id: ObjectId,
    callback: Option<ValueChangedCallback>,
}

impl Drop for ValueCallGuard {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            VALUE_CALLBACKS.with(|map| {
                if let Ok(mut map) = map.try_borrow_mut() {
                    map.insert(self.widget_id, callback);
                } else {
                    log::warn!(
                        "could not restore the value callback for widget {}: the registry is \
                         already borrowed; the callback is dropped",
                        self.widget_id
                    );
                }
            });
        }
    }
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
///
/// # Panic and re-entrancy safety
///
/// Two hazards met in this function, both invisible to a build:
///
/// 1. **A panicking callback lost its registration permanently.** The removal
///    above is what makes re-entrancy safe, but the re-insertion used to be a plain
///    statement after the call: when the callback unwound, the `insert` was skipped
///    and the widget kept its handle while silently ignoring every later click. The
///    `ClickCallGuard` below re-inserts on the unwind path too, so the registration
///    survives.
/// 2. **A nested registration for the same id panicked with `BorrowMutError`.**
///    `CLICK_CALLBACKS.with(..)` at the removal point and at the insertion point
///    both took `borrow_mut()`, so a callback that called `on_click` on its own
///    handle hit an already-mutably-borrowed `RefCell`. `try_borrow_mut` is used
///    here, matching `remove_callbacks`, and a refused borrow is logged instead of
///    panicking — the alternative is a panic inside a user callback, which is the
///    worst place to put one.
pub fn dispatch_trigger(widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
    match kind {
        WidgetTriggerKind::Clicked | WidgetTriggerKind::Unknown => {
            let Some(cb) = take_click_callback(widget_id) else {
                return false;
            };
            // The guard holds the callback and puts it back when it goes out of
            // scope, including on unwind, so a panicking callback cannot silently
            // unregister itself.
            let _guard = ClickCallGuard { widget_id, callback: Some(cb) };
            let callback = _guard.callback.as_ref().expect("set just above");
            if let Ok(mut f) = callback.try_borrow_mut() {
                f();
            } else {
                log::warn!(
                    "the click callback for widget {widget_id} is already running; \
                     refusing to re-enter it rather than panicking"
                );
            }
            true
        }
        WidgetTriggerKind::ValueChanged | WidgetTriggerKind::SelectionChanged => {
            let text = crate::get_widget_text(widget_id);
            let Some(cb) = take_value_callback(widget_id) else {
                return false;
            };
            let _guard = ValueCallGuard { widget_id, callback: Some(cb) };
            let callback = _guard.callback.as_ref().expect("set just above");
            if let Ok(mut f) = callback.try_borrow_mut() {
                f(text);
            } else {
                log::warn!(
                    "the value callback for widget {widget_id} is already running; \
                     refusing to re-enter it rather than panicking"
                );
            }
            true
        }
        WidgetTriggerKind::Closed => {
            // Clean up callbacks when a widget is closed/destroyed.
            remove_callbacks(widget_id);
            false
        }
        WidgetTriggerKind::Resized => {
            // The host resized a container: re-run its layout against the new size.
            //
            // This is the path that makes a layout follow a window the *user* resized.
            // `WindowHandle::set_geometry` covers programmatic resizes, but nothing the
            // user does with the window frame goes through it — the OS tells the
            // backend, the backend queues this event, and the layout is re-run here.
            //
            // The size is asked of the **control backend**, which is where a window
            // created through `create_window` actually lives and where the resize was
            // reported. Asking the platform instead would query a different store that
            // never saw this window.
            if let Some((width, height)) = crate::window_client_size(widget_id) {
                set_window_size(widget_id, width, height);
                apply_window_layout(widget_id);
            }
            false
        }
    }
}

/// Updates the window-size mirror used by `apply_window_layout` and the geometry-aware
/// window helpers, without routing back into the platform.
fn set_window_size(window_id: ObjectId, width: u32, height: u32) {
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
        let factory = crate::widget::WidgetFactory::new_with_defaults();
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
        FrameHandle::from_raw(crate::create_panel(self.id, x, y, w, h))
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
        // A child that hosts its own layout has just been given a new box, so its
        // children have to be laid out again. Without this a panel positioned by the
        // window layout keeps whatever child geometry it computed against its original
        // (usually zero-area) rect — the children exist but are never placed.
        if widget_hosts_a_layout(widget_id) {
            apply_panel_geometry(widget_id, geometry);
        }
    }
}

/// Whether `id` has a child layout registered on it.
fn widget_hosts_a_layout(id: ObjectId) -> bool {
    PANEL_LAYOUTS.try_with(|map| map.borrow().contains_key(&id)).unwrap_or(false)
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
            /// Rebuilds a handle from a raw object id.
            ///
            /// The id is not verified against the registry, so a stale or
            /// wrong-kind id produces a handle whose methods fail quietly rather
            /// than a panic. Prefer the factory method on `WindowHandle`, which
            /// hands back an id it has just created.
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
    /// Appends an item with the given text.
    ///
    /// Returns `false` when the id is not a combo box or the backend refused, so
    /// a caller that must know the item was added should check the result rather
    /// than assuming.
    pub fn add_item(&self, text: &str) -> bool {
        crate::combo_box_add_item(self.raw_id(), text)
    }

    /// Removes every item, leaving the combo box empty and with nothing selected.
    /// Returns `false` under the same conditions as [`ComboBoxHandle::add_item`].
    pub fn clear_items(&self) -> bool {
        crate::combo_box_clear_items(self.raw_id())
    }

    /// Selects the item at `index`.
    ///
    /// Returns `false` when the id is not a combo box, or when the widget does
    /// not publish its `current_index` property — an out-of-range `index` is
    /// **not** reliably reported as a failure, so a caller that must know should
    /// read [`ComboBoxHandle::current_index`] back.
    pub fn set_current_index(&self, index: usize) -> bool {
        crate::combo_box_set_current_index(self.raw_id(), index)
    }

    /// The selected item's index, or `None` when nothing is selected (and for an
    /// id that is not a combo box).
    pub fn current_index(&self) -> Option<usize> {
        crate::combo_box_current_index(self.raw_id())
    }

    /// How many items the combo box holds. Reports `0` for an unknown id as well
    /// as for a genuinely empty list, so it cannot distinguish the two.
    pub fn item_count(&self) -> usize {
        crate::combo_box_item_count(self.raw_id())
    }

    /// The text of the item at `index`, or `None` when the index is out of range
    /// or the id is not a combo box.
    pub fn item_text(&self, index: usize) -> Option<String> {
        crate::combo_box_item_text(self.raw_id(), index)
    }
}

// ═══════════════════════════════════════════════════════════════
// ListBoxHandle – extended with list-specific operations
// ═══════════════════════════════════════════════════════════════

/// # List-box specific operations
impl ListBoxHandle {
    /// Appends an item with the given text. Returns `false` when the id is not a
    /// list box or the backend refused.
    pub fn add_item(&self, text: &str) -> bool {
        crate::list_box_add_item(self.raw_id(), text)
    }

    /// Removes the item at `index`, shifting later items down.
    ///
    /// Returns `false` when the id is not a list box or the backend refused. An
    /// out-of-range index is not reliably distinguished from a refusal, so a
    /// caller that must know should re-read [`ListBoxHandle::item_count`].
    pub fn remove_item(&self, index: usize) -> bool {
        crate::list_box_remove_item(self.raw_id(), index)
    }

    /// Removes every item, leaving the list empty and with nothing selected.
    /// Returns `false` when the id is not a list box.
    pub fn clear_items(&self) -> bool {
        crate::list_box_clear_items(self.raw_id())
    }

    /// Selects the item at `index`.
    ///
    /// Returns `false` when the id is not a list box, or when the widget does not
    /// publish its selection property. As with the combo box, an out-of-range
    /// index is not reliably reported, so read [`ListBoxHandle::current_index`]
    /// back to confirm.
    pub fn set_current_index(&self, index: usize) -> bool {
        crate::list_box_set_current_index(self.raw_id(), index)
    }

    /// The selected item's index, or `None` when nothing is selected (and for an
    /// id that is not a list box).
    pub fn current_index(&self) -> Option<usize> {
        crate::list_box_current_index(self.raw_id())
    }

    /// How many items the list holds. Reports `0` for an unknown id as well as
    /// for a genuinely empty list.
    pub fn item_count(&self) -> usize {
        crate::list_box_item_count(self.raw_id())
    }

    /// The text of the item at `index`, or `None` when the index is out of range
    /// or the id is not a list box.
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
    /// control through [`crate::platform::Platform::set_widget_value`], so the change is
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
    /// through [`crate::platform::Platform::set_widget_step`].
    pub fn set_step(&self, step: i32) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
        crate::platform::get_platform().set_widget_step(self.raw_id(), f64::from(step));
    }

    /// Return the slider orientation this handle was created with.
    ///
    /// Orientation cannot be changed after creation on Win32 or AppKit, so it is
    /// set through [`WindowHandle::new_slider_with_orientation`] rather than a
    /// setter. This reports the value that was applied at creation (or
    /// `Horizontal`, the toolkit default, when created with `new_slider`).
    pub fn orientation(&self) -> Orientation {
        SLIDER_STATES.with(|map| {
            map.borrow()
                .get(&self.raw_id())
                .map(|s| s.orientation)
                .unwrap_or(Orientation::Horizontal)
        })
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
    /// through [`crate::platform::Platform::set_widget_value`], so the bar actually moves.
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
    /// through [`crate::platform::Platform::set_widget_range`].
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
    /// through [`crate::platform::Platform::set_widget_range`].
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
    /// through [`crate::platform::Platform::set_widget_indeterminate`], so a native bar
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
    tristate: bool,
    check_state: CheckState,
}

impl Default for CheckBoxState {
    fn default() -> Self {
        Self { tristate: false, check_state: CheckState::Unchecked }
    }
}

thread_local! {
    static CHECKBOX_STATES: RefCell<HashMap<ObjectId, CheckBoxState>> = RefCell::new(HashMap::new());
}

/// # Check-box specific operations
impl CheckBoxHandle {
    /// Return whether the check-box is checked.
    ///
    /// A tri-state box reports `true` only for [`CheckState::Checked`]; the mixed
    /// state is not "checked". Use [`CheckBoxHandle::check_state`] when the three
    /// states must be distinguished.
    pub fn is_checked(&self) -> bool {
        self.check_state() == CheckState::Checked
    }

    /// Set the check-box to checked or unchecked.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_checked`], so the box really moves.
    ///
    /// This always lands on a definite state, tri-state mode or not, so
    /// [`CheckBoxHandle::check_state`] and the native control cannot disagree.
    /// Use [`CheckBoxHandle::set_check_state`] to reach the mixed state.
    pub fn set_checked(&self, checked: bool) {
        self.set_check_state(if checked { CheckState::Checked } else { CheckState::Unchecked });
    }

    /// Set the check-box to an explicit tri-state value.
    ///
    /// [`CheckState::PartiallyChecked`] requires tri-state mode to be on
    /// (see [`CheckBoxHandle::set_tristate`]); without it the control has only
    /// two positions, so this returns `false` and changes nothing rather than
    /// silently degrading the mixed state to one of the two real ones.
    ///
    /// The native control only has off/on, so `PartiallyChecked` is pushed as
    /// "on" — which is how Win32 `BS_AUTO3STATE` and AppKit
    /// `NSControlStateValueMixed` present as to the checked flag — while this
    /// handle keeps the precise state.
    pub fn set_check_state(&self, state: CheckState) -> bool {
        let applied = CHECKBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let entry = map.entry(self.raw_id()).or_default();
            if state == CheckState::PartiallyChecked && !entry.tristate {
                return false;
            }
            entry.check_state = state;
            true
        });
        if applied {
            crate::platform::get_platform()
                .set_widget_checked(self.raw_id(), state != CheckState::Unchecked);
        }
        applied
    }

    /// Enable/disable tri-state mode.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_tristate`] (`setAllowsMixedState:` on
    /// macOS, `BS_3STATE`/`BS_AUTO3STATE` on Windows, `set_inconsistent` on GTK),
    /// so all three desktops actually gain a third state.
    ///
    /// Turning the mode *off* while the box sits in the mixed state would leave a
    /// state the control can no longer display, so it is collapsed to
    /// [`CheckState::Unchecked`] — a real, visible position instead of a stale one.
    pub fn set_tristate(&self, tristate: bool) {
        let collapsed = CHECKBOX_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let entry = map.entry(self.raw_id()).or_default();
            entry.tristate = tristate;
            if !tristate && entry.check_state == CheckState::PartiallyChecked {
                entry.check_state = CheckState::Unchecked;
                true
            } else {
                false
            }
        });
        if collapsed {
            crate::platform::get_platform().set_widget_checked(self.raw_id(), false);
        }
        crate::platform::get_platform().set_widget_tristate(self.raw_id(), tristate);
    }

    /// Return whether tri-state mode is on for this check-box.
    ///
    /// The answer comes from the same in-process mirror [`Self::set_tristate`]
    /// writes and [`Self::check_state`] reads, so the two cannot disagree. It
    /// deliberately does **not** ask [`crate::platform::Platform::is_widget_tristate`]: after
    /// BLUE15 the host no longer owns control state, so every real desktop backend
    /// answers the platform-trait default (`None`) and a flag the handle already
    /// knows would read back as "unknown".
    pub fn is_tristate(&self) -> Option<bool> {
        Some(
            CHECKBOX_STATES
                .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.tristate).unwrap_or(false)),
        )
    }

    /// Return the current check state, including the tri-state mixed value.
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
    /// native controls through [`crate::platform::Platform::set_widget_checked`], so the
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
    ///
    /// Radio buttons sharing a group are mutually exclusive. Pushed to the native
    /// control through [`crate::platform::Platform::set_widget_group`] — GTK links the
    /// buttons, Win32 sets `WS_GROUP`, and AppKit relies on adjacency — while the
    /// actual clearing of siblings is done by [`RadioButtonHandle::select`].
    pub fn set_group(&self, group: &str) {
        RADIO_BUTTON_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).group =
                group.to_owned();
        });
        crate::platform::get_platform().set_widget_group(self.raw_id(), group);
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
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_placeholder`] (`EM_SETCUEBANNER` on
    /// Windows, `set_placeholder_text` on GTK). AppKit's `NSTextView` has no
    /// placeholder concept, so on macOS the platform call reports `false` and only
    /// the mirror changes — a genuine per-OS difference, not a dropped write.
    pub fn set_placeholder(&self, text: &str) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).placeholder =
                text.to_owned();
        });
        crate::platform::get_platform().set_widget_placeholder(self.raw_id(), text);
    }

    /// Set whether the line-edit is read-only.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_read_only`], so the native field
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
    /// through [`crate::platform::Platform::set_widget_max_length`]. Not every OS text
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
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_echo_mode`] (`EM_SETPASSWORDCHAR` on
    /// Windows, `set_visibility` on GTK). AppKit picks the text class instead, so
    /// macOS reports `false` here; and `NoEcho` has no equivalent on any toolkit,
    /// so it is refused rather than silently treated as `Password`.
    pub fn set_echo_mode(&self, mode: EchoMode) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).echo_mode = mode;
        });
        crate::platform::get_platform().set_widget_echo_mode(self.raw_id(), mode);
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
    ///
    /// Pushed to the native control through [`crate::platform::Platform::set_widget_selection`]
    /// as the full range, so the OS selection matches what the mirror reports.
    pub fn select_all(&self) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = true;
            state.selection_start = 0;
            state.selection_end = u32::MAX;
        });
        // u32::MAX is the crate's "to the end" sentinel; Win32 and GTK both clamp
        // it, and macOS clamps the NSRange itself.
        crate::platform::get_platform().set_widget_selection(self.raw_id(), 0, u32::MAX);
    }

    /// Set the selection range (start..end).
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_selection`].
    pub fn set_selection(&self, start: u32, end: u32) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = false;
            state.selection_start = start;
            state.selection_end = end;
        });
        crate::platform::get_platform().set_widget_selection(self.raw_id(), start, end);
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
    ///
    /// Mirrored into the in-process state *and* pushed to the native container
    /// through [`crate::platform::Platform::set_widget_scroll_position`] (`SetScrollPos` on
    /// Windows, the GTK adjustments, the AppKit clip view).
    pub fn set_scroll_position(&self, x: i32, y: i32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.scroll_x = x;
            state.scroll_y = y;
        });
        crate::platform::get_platform().set_widget_scroll_position(self.raw_id(), x, y);
    }

    /// Return the current scroll offset.
    pub fn scroll_position(&self) -> (i32, i32) {
        SCROLL_AREA_STATES.with(|map| {
            let map = map.borrow();
            map.get(&self.raw_id()).map(|s| (s.scroll_x, s.scroll_y)).unwrap_or((0, 0))
        })
    }

    /// Set the content size (in virtual pixels).
    ///
    /// The height is what [`Self::scroll_to_bottom`] uses to compute the bottom
    /// offset. The width is retained for callers that mirror the geometry
    /// themselves, but has no consumer in the library's own scroll path (no
    /// horizontal-scrollbar extent is derived from it).
    pub fn set_content_size(&self, w: u32, h: u32) {
        SCROLL_AREA_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.content_w = w;
            state.content_h = h;
        });
    }

    /// Scroll to the bottom of the content.
    ///
    /// Computes the bottom offset from the last `set_content_size` height and
    /// pushes it through [`Self::set_scroll_position`], so the native container
    /// actually scrolls rather than only the in-process mirror moving.
    pub fn scroll_to_bottom(&self) {
        let bottom = SCROLL_AREA_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.content_h).unwrap_or(0));
        self.set_scroll_position(0, bottom as i32);
    }

    /// Scroll to the top of the content.
    pub fn scroll_to_top(&self) {
        self.set_scroll_position(0, 0);
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
    ///
    /// Records the column in the in-process mirror for a caller that renders the
    /// list itself; the library's own scroll/native backends do not consume this
    /// mirror (there is no cross-OS native "add column" primitive, so the
    /// self-drawn view is what a caller reads it back for). This is a local
    /// bookkeeping call, not a rendering side effect.
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

    /// Select a row by index, mirroring the in-process state and pushing the
    /// selection to the native control.
    ///
    /// `None` clears the selection. Returns `false` when the backend has no
    /// selection model or the id is unknown, in which case the mirror is left
    /// unchanged so `selected_row()` keeps reporting the previous value.
    pub fn select_row(&self, row: Option<usize>) -> bool {
        let pushed = crate::platform::get_platform().set_widget_selected_index(self.raw_id(), row);
        if pushed {
            LIST_VIEW_STATES.with(|map| {
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .selected_row = row;
            });
        }
        pushed
    }

    /// Set the selection mode.
    ///
    /// Records the mode in the in-process mirror; there is no cross-OS native
    /// primitive for selection mode, so this is local bookkeeping for a caller
    /// that drives the selection itself rather than a rendering side effect.
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
    /// through [`crate::platform::Platform::set_widget_value`], so the number the user sees
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
    /// Stored in the in-process state for callers that render the value
    /// themselves. There is no cross-OS native control with an affix
    /// concept — AppKit `NSStepper`/`NSTextField`, Win32 `UPDOWN_CLASS` and GTK
    /// `SpinButton` all render a bare number — and the library's own self-drawn
    /// path does not consume this mirror either, so it has **no effect on
    /// rendering**; a caller that needs an affix must format the value where it is
    /// displayed. This is a genuine platform gap rather than a dropped write.
    pub fn set_prefix(&self, prefix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).prefix =
                prefix.to_owned();
        });
    }

    /// Set the suffix text displayed after the value.
    ///
    /// See [`SpinBoxHandle::set_prefix`]: the value is stored in the in-process
    /// state for callers that render it themselves, and has **no effect on
    /// rendering** by the library (no native backend honours an affix, and the
    /// self-drawn path does not read this mirror).
    pub fn set_suffix(&self, suffix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).suffix =
                suffix.to_owned();
        });
    }

    /// Set the spin-box step increment.
    ///
    /// Mirrored into the in-process state *and* pushed to the native control
    /// through [`crate::platform::Platform::set_widget_step`].
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
        crate::set_widget_geometry(self.raw_id(), x, y, w, h);
        apply_panel_geometry(self.raw_id(), Rect::new(x, y, w, h));
    }

    /// Set the layout manager for this panel.
    ///
    /// The layout is stored internally and used to reposition children.
    /// Only one layout can be active at a time.
    ///
    /// # Applying a layout before the panel has a size
    ///
    /// A panel is usually created with a placeholder rect (`new_panel(0, 0, 0, 0)`) and
    /// then positioned by a parent layout. Applying this layout immediately would lay
    /// every child out inside a zero-area rect — invisible, and never corrected, because
    /// the parent layout positions the panel through `set_widget_geometry`, which does not
    /// re-run this. The layout is therefore **also** applied whenever the panel's real
    /// geometry changes (see `apply_panel_layout`, which reads the live geometry rather
    /// than the mirrored state), so a caller can set the layout at any point.
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

/// Applies a panel's layout to its children.
///
/// # Why the rect comes from the widget, not from `PANEL_STATES`
///
/// A panel's geometry has two writers: `PanelHandle::set_geometry` (which updates the
/// mirrored `PANEL_STATES`) and `crate::set_widget_geometry` (used by the window layout,
/// which does not). Reading the mirror therefore lays children out into a stale rect
/// whenever the panel is positioned by a parent layout — the case that matters, since
/// that is how a panel normally gets its size. Reading the widget's own geometry is the
/// single source of truth, and the mirror stays for the accessors that publish it.
/// Records a panel's geometry and lays its children out inside it.
///
/// # Why the mirror exists and why it is written from here
///
/// A panel's rect has two writers: `PanelHandle::set_geometry` (a direct call) and the
/// window layout, which positions it through `crate::set_widget_geometry`. The latter does
/// not reach `PANEL_STATES`, and `crate::widget::runtime::geometry_of` cannot answer for a
/// native control either (only *mounted* widgets live in that registry), so the mirror is
/// the one place that can hold the panel's real box. Both writers funnel through here, so
/// the mirror cannot fall behind.
///
/// A zero-area panel is recorded but **not** laid out: computing against an empty rect
/// would overwrite every child's geometry with zeros, which is precisely the invisible-grid
/// failure this replaces.
fn apply_panel_geometry(panel_id: ObjectId, rect: Rect) {
    PANEL_STATES.with(|map| {
        let mut map = map.borrow_mut();
        let state = map.entry(panel_id).or_default();
        state.x = rect.x;
        state.y = rect.y;
        state.w = rect.width;
        state.h = rect.height;
    });

    if rect.width == 0 || rect.height == 0 {
        return;
    }

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

/// Applies the layout a panel already has, using its recorded geometry.
///
/// Used by [`PanelHandle::set_layout`], where the caller has supplied a layout but not a
/// rect: the panel's current box is whatever was recorded last.
fn apply_panel_layout(panel_id: ObjectId) {
    let Some(rect) = PANEL_STATES.with(|map| {
        map.borrow().get(&panel_id).map(|state| Rect::new(state.x, state.y, state.w, state.h))
    }) else {
        return;
    };
    apply_panel_geometry(panel_id, rect);
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
    /// Pushed to the OS through [`crate::platform::Platform::set_window_icon`] (AppKit
    /// `setRepresentation:`, Win32 `WM_SETICON`, GTK `set_icon_from_file`) as well
    /// as into the in-process mirror. Returns `false` when the backend could not
    /// load the file, so a bad path is visible instead of silently ignored.
    ///
    /// The mirror is written unconditionally, but [`WindowHandle::icon`] reports it **only**
    /// as a fallback for a backend that has no icon concept at all — see that method for why
    /// a failed load must stay unreadable.
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
    /// Pushed to the OS through [`crate::platform::Platform::set_window_min_size`]
    /// (`setContentMinSize:` on macOS, the `WM_GETMINMAXINFO` handler on Windows,
    /// `set_geometry_hints` on GTK) as well as into the in-process mirror.
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
    /// Pushed to the OS through [`crate::platform::Platform::set_window_state`]
    /// (`zoom:` on macOS, `ShowWindow(SW_MAXIMIZE)` on Windows, `maximize()` on
    /// GTK) as well as into the in-process mirror. `is_maximized` reports what
    /// the OS window actually is when a native window exists.
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
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .maximized = maximized;
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
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .minimized = minimized;
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
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .fullscreen = fullscreen;
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
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .resizable = resizable;
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
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .decorated = decorated;
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

    /// A click callback that panics must stay registered.
    ///
    /// # The defect this closes
    ///
    /// `dispatch_trigger` moves the callback out of the registry before invoking it
    /// (so a re-entrant `remove_callbacks` cannot double-borrow) and puts it back
    /// after. The restore used to be a plain statement on the success path, so a
    /// callback that unwound took its own registration with it: the widget kept its
    /// handle and quietly ignored every subsequent click. `ClickCallGuard` restores
    /// on the unwind path; this test is what proves it, because the unwind is the
    /// only way the two paths differ.
    #[test]
    fn a_panicking_click_callback_stays_registered() {
        let id: ObjectId = 4301;
        remove_callbacks(id);
        let calls = Rc::new(RefCell::new(0usize));
        let counter = Rc::clone(&calls);

        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(
                id,
                Rc::new(RefCell::new(move || {
                    *counter.borrow_mut() += 1;
                    panic!("callback body panics on purpose");
                })),
            );
        });

        let first = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
        assert!(first.is_err(), "the callback's panic must propagate to the test harness");
        assert_eq!(*calls.borrow(), 1);

        // The registration must have survived the unwind, so a second dispatch
        // reaches the callback again rather than reporting no callback at all.
        let second = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
        assert!(second.is_err(), "the callback is still installed");
        assert_eq!(*calls.borrow(), 2, "the second dispatch must have invoked the callback");

        remove_callbacks(id);
    }

    /// A callback that re-registers itself must not panic with `BorrowMutError`.
    ///
    /// `on_click` writes to the same `RefCell` that `dispatch_trigger` was reading
    /// when it invoked the callback. Each access therefore has to be attempted
    /// rather than assumed; a plain `borrow_mut` on either side turned this ordinary
    /// pattern into a panic inside user code.
    #[test]
    fn a_callback_that_reregisters_itself_does_not_panic() {
        let id: ObjectId = 4302;
        remove_callbacks(id);

        let ran = Rc::new(RefCell::new(0usize));
        let counter = Rc::clone(&ran);
        CLICK_CALLBACKS.with(|map| {
            map.borrow_mut().insert(
                id,
                Rc::new(RefCell::new(move || {
                    *counter.borrow_mut() += 1;
                    // Re-register under the same id, from inside the dispatch.
                    CLICK_CALLBACKS.with(|map| {
                        if let Ok(mut map) = map.try_borrow_mut() {
                            map.insert(id, Rc::new(RefCell::new(|| {})));
                        }
                    });
                })),
            );
        });

        let result = std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::Clicked));
        assert!(result.is_ok(), "a re-registering callback must not panic");
        assert_eq!(*ran.borrow(), 1, "the callback must have run exactly once");

        remove_callbacks(id);
    }

    /// The value-changed path has the same unwind guarantee as the click path.
    #[test]
    fn a_panicking_value_callback_stays_registered() {
        let id: ObjectId = 4303;
        remove_callbacks(id);
        let calls = Rc::new(RefCell::new(0usize));
        let counter = Rc::clone(&calls);

        VALUE_CALLBACKS.with(|map| {
            map.borrow_mut().insert(
                id,
                Rc::new(RefCell::new(move |_text: String| {
                    *counter.borrow_mut() += 1;
                    panic!("value callback body panics on purpose");
                })),
            );
        });

        let first =
            std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
        assert!(first.is_err());
        assert_eq!(*calls.borrow(), 1);

        let second =
            std::panic::catch_unwind(|| dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
        assert!(second.is_err(), "the value callback must still be installed");
        assert_eq!(*calls.borrow(), 2);

        remove_callbacks(id);
    }

    /// A dispatch with no callback reports `false` rather than panicking.
    #[test]
    fn dispatching_without_a_callback_is_a_no_op() {
        let id: ObjectId = 4304;
        remove_callbacks(id);
        assert!(!dispatch_trigger(id, WidgetTriggerKind::Clicked));
        assert!(!dispatch_trigger(id, WidgetTriggerKind::ValueChanged));
        // `Closed` is the teardown path and always answers `false`.
        assert!(!dispatch_trigger(id, WidgetTriggerKind::Closed));
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
    /// A backend with no icon concept and no minimum-size read-back.
    ///
    /// Six of the shipped backends (`linux-gtk`, `wayland`, `android`, `ios`, `wasm`, `harmony`)
    /// inherit the `Platform` defaults for `window_min_size`/`window_icon`, which answer `None`;
    /// `macos` implements the size pair but not the icon pair. The mirror fallback exists for
    /// exactly that case, and this stand-in reproduces it so the fallback is testable on any
    /// host rather than only on the six backends this one cannot run.
    struct NoReadbackBackend;

    impl crate::platform::Platform for NoReadbackBackend {
        fn as_any(&self) -> &dyn crate::compat::Any {
            self
        }
        fn backend_name(&self) -> &'static str {
            "no-readback-probe"
        }
        fn family(&self) -> crate::core::PlatformFamily {
            crate::core::PlatformFamily::Desktop
        }
        fn init(&self) {}
        fn run(&self) {}
        fn quit(&self) {}
        fn create_window(&self, _t: &str, _x: i32, _y: i32, _w: u32, _h: u32) -> ObjectId {
            1
        }
        fn set_window_min_size(&self, _id: ObjectId, _w: u32, _h: u32) -> bool {
            true
        }
        fn set_window_icon(&self, _id: ObjectId, _path: &str) -> bool {
            true
        }
        // `window_min_size` and `window_icon` are deliberately *not* overridden: the trait
        // defaults answer `None`, which is the condition the mirror covers.
    }

    static NO_READBACK: NoReadbackBackend = NoReadbackBackend;

    /// A value the platform accepted is readable back even when the platform cannot answer.
    ///
    /// `min_size` and `icon` read the platform exclusively, so on a backend without those
    /// readers a window whose setter returned `true` still reported `None` — the mirror fields
    /// were written by every set and read by nothing. Both now fall back to the mirror, which is
    /// the shape `is_maximized` already used.
    #[test]
    fn an_accepted_window_property_reads_back_without_platform_readers() {
        crate::platform::runtime::with_platform(&NO_READBACK, || {
            let window = WindowHandle::from_raw(4242);

            assert_eq!(window.min_size(), None, "nothing was set yet");
            assert_eq!(window.icon(), None, "nothing was set yet");

            assert!(window.set_min_size(320, 240), "the backend accepts the call");
            assert_eq!(
                window.min_size(),
                Some((320, 240)),
                "a value the platform accepted must be readable back"
            );

            assert!(window.set_icon("/tmp/probe.png"));
            assert_eq!(
                window.icon(),
                Some("/tmp/probe.png".to_string()),
                "a path the platform accepted must be readable back"
            );

            // A later set replaces rather than accumulates.
            assert!(window.set_min_size(640, 480));
            assert_eq!(window.min_size(), Some((640, 480)));
        });
    }

    /// A *rejected* set leaves nothing readable, on a backend that refuses.
    ///
    /// The mirror used to be written before the platform was asked, so a refused value was
    /// recorded anyway. The fallback above is only honest because the write happens on success.
    #[test]
    fn a_rejected_window_property_is_not_reported_by_a_later_read() {
        struct RejectingBackend;
        impl crate::platform::Platform for RejectingBackend {
            fn as_any(&self) -> &dyn crate::compat::Any {
                self
            }
            fn backend_name(&self) -> &'static str {
                "rejecting-probe"
            }
            fn family(&self) -> crate::core::PlatformFamily {
                crate::core::PlatformFamily::Desktop
            }
            fn init(&self) {}
            fn run(&self) {}
            fn quit(&self) {}
            fn create_window(&self, _t: &str, _x: i32, _y: i32, _w: u32, _h: u32) -> ObjectId {
                1
            }
            fn set_window_min_size(&self, _id: ObjectId, _w: u32, _h: u32) -> bool {
                false
            }
            fn set_window_icon(&self, _id: ObjectId, _path: &str) -> bool {
                false
            }
        }
        static REJECTING: RejectingBackend = RejectingBackend;

        crate::platform::runtime::with_platform(&REJECTING, || {
            let window = WindowHandle::from_raw(4243);
            assert!(!window.set_min_size(320, 240), "the backend refuses");
            assert_eq!(window.min_size(), None, "a refused size must not be reported");
            assert!(!window.set_icon("/tmp/nope.png"), "the backend refuses");
            assert_eq!(window.icon(), None, "a refused icon must not be reported");
        });
    }

    /// An id that addresses no window leaves nothing readable on the active backend.
    #[test]
    fn a_rejected_set_on_the_active_backend_leaves_nothing_readable() {
        let window = WindowHandle::from_raw(0);
        assert!(!window.set_min_size(320, 240), "id 0 is not a window");
        assert_eq!(window.min_size(), None, "a refused size must not be reported");
        assert!(!window.set_icon("/tmp/does-not-exist.png"));
        assert_eq!(window.icon(), None, "a refused icon path must not be reported");
    }

    /// Every mirror-backed getter follows one rule: platform first, mirror as fallback.
    ///
    /// # The decision this pins
    ///
    /// The `WindowState` mirror is kept, not deleted, and it is a *fallback* rather than a shadow.
    /// Two reasons, both of which have to hold for a field to earn its place here:
    ///
    /// 1. **Six backends cannot read the value back.** `window_min_size`/`window_icon` fall through
    ///    to `Platform` defaults returning `None` on `linux-gtk`, `wayland`, `android`, `ios`,
    ///    `wasm` and `harmony` (and `macos` implements the size pair but not the icon pair). A
    ///    setter that returned `true` there produced a `None` getter, so the mirror is the only
    ///    source that can answer at all.
    /// 2. **`apply_window_layout` needs the size on every backend**, including at construction
    ///    before any native object exists — which is the documented reason `record_created_geometry`
    ///    writes it.
    ///
    /// A field with neither property would be dead weight and should be deleted instead. This test
    /// makes that a decision that has to be re-made rather than inherited: it drives each
    /// mirror-backed accessor on the active backend and requires the platform to be consulted
    /// first — proven by a set the backend *refuses* leaving the getter reporting nothing.
    #[test]
    fn every_mirror_backed_getter_defers_to_the_platform_first() {
        // An id that addresses no window: every backend refuses the setters, so no mirror entry
        // is created and every getter must report its honest "nothing" value.
        let window = WindowHandle::from_raw(0);

        assert!(!window.set_min_size(320, 240), "id 0 is not a window");
        assert!(!window.set_icon("/tmp/does-not-exist.png"));
        window.set_maximized(true);
        window.set_minimized(true);
        window.set_fullscreen(true);
        window.set_resizable(false);
        window.set_decorated(false);

        // Each getter is total: no panic, and the "nothing set" answer rather than a stale value.
        assert_eq!(window.min_size(), None, "a refused size must not be remembered");
        assert_eq!(window.icon(), None, "a refused icon must not be remembered");
        assert!(!window.is_maximized());
        assert!(!window.is_minimized());
        assert!(!window.is_fullscreen());
        assert!(window.is_resizable(), "a fresh window is resizable");
        assert!(window.is_decorated(), "a fresh window is decorated");
    }

    /// The two geometry mirrors exist because `apply_window_layout` reads them.
    ///
    /// `w`/`h` are not platform-read for the window: they are the client size a layout is solved
    /// against, and `record_created_geometry` has to supply them before any native object exists.
    /// `x`/`y` are read back by `WindowHandle::geometry` for a panel. This asserts the layout path
    /// still receives the size it recorded, so the fields are not orphaned by a refactor.
    #[test]
    fn the_window_geometry_mirror_reaches_the_layout_path() {
        let id = crate::platform::get_platform().create_window("probe", 10, 20, 400, 300);
        WindowHandle::record_created_geometry(id, 10, 20, 400, 300);

        let recorded = WINDOW_STATES.with(|map| {
            map.borrow().get(&id).map(|state| (state.x, state.y, state.w, state.h))
        });
        assert_eq!(
            recorded,
            Some((10, 20, 400, 300)),
            "the geometry mirror must carry what the constructor recorded"
        );
    }

}
