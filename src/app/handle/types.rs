// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-safe widget handles backed by `ObjectId`.
//!
//! Each handle type wraps a raw `ObjectId` and exposes only the operations
//! that are valid for that widget kind.  Handles also support event callbacks
//! via the `WidgetHandle` extension trait.

use super::*;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};

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
pub type ValueChangedCallback = Rc<RefCell<dyn FnMut(&str)>>;

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
    pub(crate) id: ObjectId,
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

    /// Routes a click into the control's own `clicked` signal.
    ///
    /// # Why the refusal is gone (DEMO-005)
    ///
    /// This used to record an unwired binding on the grounds that a library-painted control
    /// "emits its own signals rather than a platform click callback". The premise was wrong:
    /// every control's [`BaseWidget`](crate::widget::BaseWidget) carries `clicked`, and that is
    /// exactly the signal [`register_click_callback`] connects to for every other handle. A
    /// mounted surface therefore has a real click channel; refusing it made a callback registered
    /// here unreachable while the same callback on a `ButtonHandle` worked — the asymmetry this
    /// closes. There is still one definition of "what a click is" (`base.clicked`), so this is the
    /// same wiring and not a second one.
    fn on_click<F: FnMut() + Send + 'static>(&self, f: F) {
        register_click_callback(self.id, f);
    }

    /// Routes a value change into the control's published `value_changed` event.
    ///
    /// # Why this re-dispatches instead of connecting directly (DEMO-005)
    ///
    /// The signature takes a `FnMut(&str)` that is **not** `Send`, and every slot on a control's
    /// signal must be `Send + Sync` (the signal machinery is shared). Value callbacks therefore
    /// live in the runtime's thread-local table, and the thing that runs them is
    /// [`dispatch_trigger`]. So this subscribes a `Send + Sync` slot that captures only the id —
    /// the same shape [`WidgetHandle::on_click_cross_thread`] uses — and the slot re-dispatches the
    /// trigger, which reads the control's text and runs the stored callback on the thread that
    /// emitted. A control that publishes no `value_changed` event has no channel to hang that on,
    /// so the binding is recorded as unwired rather than left in a table nothing reads.
    fn on_value_changed<F: FnMut(&str) + 'static>(&self, f: F) {
        let widget_id = self.id;
        let subscribed = crate::widget::runtime::with_widget(widget_id, |widget| {
            widget.event_signal_dyn("value_changed")
        })
        .flatten()
        .map(|signal| {
            signal.subscribe(Box::new(
                move |_payload: &crate::widget::capability::CapabilityValue| {
                    dispatch_trigger(widget_id, WidgetTriggerKind::ValueChanged);
                },
            ))
        });
        match subscribed {
            // The connection handle is deliberately dropped: `EventSignalRef::disconnect` is an
            // explicit call, so the subscription lives until the control does. A later
            // `remove_callbacks` clears the thread-local table, and the slot then finds no
            // callback to run — the observable end state the caller asked for.
            Some(_connection) => register_value_callback(widget_id, Rc::new(RefCell::new(f))),
            None => {
                log::warn!(
                    "on_value_changed ignored for surface id={widget_id}: the control publishes \
                     no `value_changed` event, so the callback could never run"
                );
                record_unwired_binding(widget_id);
            }
        }
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
    /// Returns `true` when the control accepted the value; `false` when it did not — an id that
    /// is not a mounted widget, or a control that publishes no `value`. A caller branches on the
    /// result at runtime if it cares; it never branches on the OS. See
    /// `write_control_property` for why this is a property write rather than a call into the
    /// platform's host-native surface.
    fn set_value(&self, value: f64) -> bool {
        write_control_property(
            self.raw_id(),
            "value",
            crate::widget::capability::CapabilityValue::Float(value),
        )
    }

    /// Read this widget's primary numeric value.
    fn value(&self) -> Option<f64> {
        read_control_property(self.raw_id(), "value").as_ref().and_then(capability_as_f64)
    }

    /// Set this widget's `(min, max)` range, when it has one.
    fn set_range(&self, min: f64, max: f64) -> bool {
        {
            let low = write_control_property(
                self.raw_id(),
                "minimum",
                crate::widget::capability::CapabilityValue::Float(min),
            );
            let high = write_control_property(
                self.raw_id(),
                "maximum",
                crate::widget::capability::CapabilityValue::Float(max),
            );
            low && high
        }
    }

    /// Read this widget's `(min, max)` range, when it has one.
    fn range(&self) -> Option<(f64, f64)> {
        {
            let low = read_control_property(self.raw_id(), "minimum")
                .as_ref()
                .and_then(capability_as_f64)?;
            let high = read_control_property(self.raw_id(), "maximum")
                .as_ref()
                .and_then(capability_as_f64)?;
            Some((low, high))
        }
    }

    /// Set this widget's selection index (combo box, list box, tab widget).
    ///
    /// `index == None` clears the selection where the control supports it.
    fn set_selected_index(&self, index: Option<usize>) -> bool {
        {
            let value = match index {
                Some(i) => crate::widget::capability::CapabilityValue::UInt(i as u64),
                None => crate::widget::capability::CapabilityValue::Null,
            };
            write_control_property(self.raw_id(), "selected_index", value)
        }
    }

    /// Read this widget's selection index.
    fn selected_index(&self) -> Option<usize> {
        read_control_property(self.raw_id(), "selected_index")
            .as_ref()
            .and_then(capability_as_usize)
    }

    /// Set a checkable control's checked state (check box, radio button, toggle
    /// button).
    fn set_checked(&self, checked: bool) -> bool {
        write_control_property(
            self.raw_id(),
            "checked",
            crate::widget::capability::CapabilityValue::Bool(checked),
        )
    }

    /// Read this widget's checked state, or `None` when it is not checkable.
    fn checked(&self) -> Option<bool> {
        match read_control_property(self.raw_id(), "checked") {
            Some(crate::widget::capability::CapabilityValue::Bool(checked)) => Some(checked),
            _ => None,
        }
    }

    /// Set this widget's increment step (slider, spin box, scroll bar).
    fn set_step(&self, step: f64) -> bool {
        write_control_property(self.raw_id(), "single_step", step_carrier(step))
    }

    /// Read this widget's increment step.
    fn step(&self) -> Option<f64> {
        read_control_property(self.raw_id(), "single_step").as_ref().and_then(capability_as_f64)
    }

    /// Set a progress-style control's indeterminate (busy) state.
    fn set_indeterminate(&self, indeterminate: bool) -> bool {
        write_control_property(
            self.raw_id(),
            "indeterminate",
            crate::widget::capability::CapabilityValue::Bool(indeterminate),
        )
    }

    /// Read a progress-style control's indeterminate state.
    fn is_indeterminate(&self) -> Option<bool> {
        match read_control_property(self.raw_id(), "indeterminate") {
            Some(crate::widget::capability::CapabilityValue::Bool(v)) => Some(v),
            _ => None,
        }
    }

    /// Set a text-entry control's read-only state.
    fn set_read_only(&self, read_only: bool) -> bool {
        write_control_property(
            self.raw_id(),
            "read_only",
            crate::widget::capability::CapabilityValue::Bool(read_only),
        )
    }

    /// Read a text-entry control's read-only state.
    fn is_read_only(&self) -> Option<bool> {
        match read_control_property(self.raw_id(), "read_only") {
            Some(crate::widget::capability::CapabilityValue::Bool(v)) => Some(v),
            _ => None,
        }
    }

    /// Set a text-entry control's maximum accepted length.
    fn set_max_length(&self, max_length: u32) -> bool {
        write_control_property(
            self.raw_id(),
            "max_length",
            crate::widget::capability::CapabilityValue::UInt(u64::from(max_length)),
        )
    }

    /// Read a text-entry control's maximum accepted length.
    fn max_length(&self) -> Option<u32> {
        match read_control_property(self.raw_id(), "max_length") {
            Some(crate::widget::capability::CapabilityValue::UInt(v)) => u32::try_from(v).ok(),
            _ => None,
        }
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
        write_control_property(
            self.raw_id(),
            "cursor_position",
            crate::widget::capability::CapabilityValue::UInt(u64::from(end.max(start))),
        )
    }

    /// Read a text entry's selection range, or `None` when nothing is selected.
    fn widget_selection(&self) -> Option<(u32, u32)> {
        {
            let caret = match read_control_property(self.raw_id(), "cursor_position") {
                Some(crate::widget::capability::CapabilityValue::UInt(v)) => {
                    u32::try_from(v).ok()?
                }
                _ => return None,
            };
            Some((0, caret))
        }
    }

    /// Set a text entry's placeholder (cue) text.
    fn set_widget_placeholder(&self, text: &str) -> bool {
        write_control_property(
            self.raw_id(),
            "placeholder_text",
            crate::widget::capability::CapabilityValue::String(text.to_string()),
        )
    }

    /// Read a text entry's placeholder text.
    fn widget_placeholder(&self) -> Option<String> {
        match read_control_property(self.raw_id(), "placeholder_text") {
            Some(crate::widget::capability::CapabilityValue::String(v)) => Some(v),
            _ => None,
        }
    }

    /// Set a text entry's echo mode.
    fn set_widget_echo_mode(&self, mode: EchoMode) -> bool {
        write_control_property(
            self.raw_id(),
            "echo_mode",
            crate::widget::capability::CapabilityValue::String(echo_mode_token(mode).to_string()),
        )
    }

    /// Read a text entry's echo mode.
    fn widget_echo_mode(&self) -> Option<EchoMode> {
        match read_control_property(self.raw_id(), "echo_mode") {
            Some(crate::widget::capability::CapabilityValue::String(v)) => echo_mode_from_token(&v),
            _ => None,
        }
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
        write_control_property(
            self.raw_id(),
            "tristate_enabled",
            crate::widget::capability::CapabilityValue::Bool(enabled),
        )
    }

    /// Read a checkable control's tri-state mode.
    fn is_tristate(&self) -> Option<bool> {
        match read_control_property(self.raw_id(), "tristate_enabled") {
            Some(crate::widget::capability::CapabilityValue::Bool(v)) => Some(v),
            _ => None,
        }
    }

    /// Put a radio button into a named mutually-exclusive group.
    fn set_group(&self, group: &str) -> bool {
        write_control_property(
            self.raw_id(),
            "group_id",
            crate::widget::capability::CapabilityValue::String(group.to_string()),
        )
    }

    /// Read a radio button's group name.
    fn group(&self) -> Option<String> {
        match read_control_property(self.raw_id(), "group_id") {
            Some(crate::widget::capability::CapabilityValue::String(v)) => Some(v),
            _ => None,
        }
    }

    /// Set a scrollable container's scroll offset.
    fn set_scroll_position(&self, x: i32, y: i32) -> bool {
        {
            let wrote_x = write_control_property(
                self.raw_id(),
                "scroll_position_x",
                crate::widget::capability::CapabilityValue::Int(i64::from(x)),
            );
            let wrote_y = write_control_property(
                self.raw_id(),
                "scroll_position_y",
                crate::widget::capability::CapabilityValue::Int(i64::from(y)),
            );
            wrote_x && wrote_y
        }
    }

    /// Read a scrollable container's scroll offset.
    fn scroll_position(&self) -> Option<(i32, i32)> {
        {
            let x = match read_control_property(self.raw_id(), "scroll_position_x") {
                Some(crate::widget::capability::CapabilityValue::Int(v)) => {
                    i32::try_from(v).ok()?
                }
                _ => return None,
            };
            let y = match read_control_property(self.raw_id(), "scroll_position_y") {
                Some(crate::widget::capability::CapabilityValue::Int(v)) => {
                    i32::try_from(v).ok()?
                }
                _ => return None,
            };
            Some((x, y))
        }
    }

    /// Register a callback for the "clicked" trigger.
    ///
    /// The closure is invoked whenever the widget receives a
    /// [`WidgetTriggerKind::Clicked`] event.
    fn on_click<F: FnMut() + Send + 'static>(&self, f: F);

    /// Register a click callback that may be invoked from another thread.
    ///
    /// # Why the convenience form is not this
    ///
    /// [`Self::on_click`] stores the closure beside the thread-local widget runtime (W-P08), which
    /// is where a control's own signal is emitted. That is lock-free but the closure is not visible
    /// to another thread. A host that genuinely emits a widget's `clicked` signal off the UI thread
    /// asks for this variant instead; it keeps the `FnMut` behind a `Mutex` and captures it in the
    /// slot, so the lock is paid only by the code that needs it.
    fn on_click_cross_thread<F: FnMut() + Send + 'static>(&self, f: F) {
        register_click_callback_cross_thread(self.raw_id(), f);
    }

    /// Register a callback for the "value changed" trigger.
    ///
    /// The closure receives the widget's current text at the time of
    /// the [`WidgetTriggerKind::ValueChanged`] event.
    fn on_value_changed<F: FnMut(&str) + 'static>(&self, f: F);

    /// Register an observer for the uncommitted IME **pre-edit** text.
    ///
    /// The closure runs on the loop's thread for every composition update the input method
    /// delivers to this widget: the first argument is the uncommitted string and the second its
    /// cursor position. It runs **before** the commit, so a host can log or assert the
    /// composition frame by frame instead of only seeing the final text.
    ///
    /// Returns whether the subscription was installed. `false` means the id is not a mounted
    /// control that accepts IME input (a window, a button, or a destroyed field), and the
    /// callback is dropped rather than left registered where it could never fire. The ignored
    /// binding is also counted by [`unwired_binding_count`], so "how many of my handlers are
    /// unwired?" answers this case too.
    fn on_ime_preedit<F: FnMut(&str, usize) + 'static>(&self, f: F) -> bool {
        let installed = crate::widget::runtime::register_ime_preedit_callback(self.raw_id(), f);
        if !installed {
            log::warn!(
                "on_ime_preedit ignored for id={}: it is not mounted or does not accept IME \
                 input, so no composition will reach it",
                self.raw_id()
            );
            record_unwired_binding(self.raw_id());
        }
        installed
    }

    /// Register a callback for the control being **closed** / dismissed.
    ///
    /// # Why this has a default body
    ///
    /// Every handle reaches the same base signal, so there is nothing per-handle to decide: the
    /// callback connects to [`BaseWidget::closed`](crate::widget::BaseWidget::closed), which a
    /// closeable control emits from its own `close`/`dismiss` path. A default body keeps the trait
    /// from forcing twenty-odd `impl_handle!` expansions to restate it — and, more importantly,
    /// means a control that *gains* a close path gets the wiring for free rather than silently
    /// lacking it.
    ///
    /// # The defect this closes
    ///
    /// `on_close` used to be a `WindowHandle`-only method that stored its callback in a side table
    /// (`WindowState::close_callback`) which **only `WindowHandle::close` read**. The widget's own
    /// `closed` signal — the one a `Dialog`/`PopupWindow`/`NavigationDrawer` emits — never reached
    /// it, so a handler declared through the JSON/designer `closed` route could not be bound at all
    /// and `JsonTriggerMarker::Closed` had to refuse it outright.
    fn on_close<F: FnMut() + Send + 'static>(&self, f: F) {
        register_close_callback(self.raw_id(), f);
    }

    /// Register a close callback that may be invoked from another thread.
    ///
    /// The [`Self::on_close`] counterpart of [`Self::on_click_cross_thread`]: the default form is
    /// thread-local and lock-free, this one keeps the `Mutex` so a cross-thread emission can run it.
    fn on_close_cross_thread<F: FnMut() + Send + 'static>(&self, f: F) {
        register_close_callback_cross_thread(self.raw_id(), f);
    }
}

// ── Global callback registry ──────────────────────────────────

use crate::signal::ConnectionHandle;
use core::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// The per-widget lock-free callback table, keyed by widget id and callback token.
type LocalCallbackMap = RefCell<HashMap<(ObjectId, u64), Box<dyn FnMut()>>>;

/// Which of a widget's own signals a stored [`ConnectionHandle`] belongs to.
///
/// `on_click` connects to `BaseWidget::clicked` and `on_close` to
/// `BaseWidget::closed`; removal must disconnect through the matching signal, so
/// the kind is recorded alongside the handle (D08-A-01).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SignalKind {
    Clicked,
    Closed,
    /// A payload-carrying control event connected by a handle method (`on_triggered`).
    Triggered,
    /// A menu-bar entry event whose payload also carries the entry rectangle.
    TriggeredEntry,
}

/// A recorded subscription made by `on_click` / `on_close`.
struct SignalConnection {
    kind: SignalKind,
    handle: ConnectionHandle,
}

thread_local! {
    // # Why the per-cell `allow(clippy::missing_const_for_thread_local)`
    //
    // OpenHarmony's std resolves `HashMap::new()` as a `const fn` where the host's does not, so
    // the lint fires there and asks for a `const` initializer that would not compile on any other
    // target. Same per-cell form (and same reason) as `widget::runtime`'s registry block — a
    // crate-wide allow would hide a genuine finding in a cell that really can be `const`.
    //
    // # Why a *list* of callbacks per widget
    //
    // Both tables were `HashMap<ObjectId, Callback>` — one slot per widget. A node that declared
    // two handlers (a published `events` binding plus an `on_*` one, or two published names) had
    // the second `insert` **silently replace** the first, so a document could name two handlers and
    // get one, with no warning and nothing to query. A list keeps every binding the document asked
    // for; `remove_callbacks` still clears the whole list on drop, so the leak a rebuild would
    // otherwise produce is unchanged.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static CLICK_CALLBACKS: RefCell<HashMap<ObjectId, Vec<ClickCallback>>> =
        RefCell::new(HashMap::new());
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static VALUE_CALLBACKS: RefCell<HashMap<ObjectId, Vec<ValueChangedCallback>>> =
        RefCell::new(HashMap::new());

    /// The signal connections `on_click` / `on_close` made for each widget.
    ///
    /// `on_click` and `on_close` connect to the widget's **own** `clicked` /
    /// `closed` signals, so their callbacks are held by the signal rather than in
    /// `CLICK_CALLBACKS`. Removing them therefore means disconnecting the handle
    /// the connect returned — the value that used to be discarded. This table
    /// remembers each handle (and which signal it belongs to) so
    /// [`remove_callbacks`] can run the real unsubscribe instead of only
    /// clearing the legacy tables (D08-A-01). Dropping the handle's `Connection`
    /// also releases the captured callback, which plain table-clearing did not.
    #[allow(clippy::missing_const_for_thread_local)]
    static SIGNAL_CONNECTIONS: RefCell<HashMap<ObjectId, Vec<SignalConnection>>> =
        RefCell::new(HashMap::new());

    /// Callbacks registered by the lock-free convenience path (`on_click` / `on_close`).
    ///
    /// # Why a thread-local table instead of `Arc<Mutex<F>>` (W-P08)
    ///
    /// The widget runtime is thread-local (`with_widget`/`with_widget_mut` operate on the calling
    /// thread's registry), so a control's own signal is emitted on the thread that mounted it. The
    /// old convenience path wrapped every user closure in `Arc<Mutex<F>>` and locked it on every
    /// click: a heap allocation per binding and a lock on every dispatch, for a cross-thread
    /// guarantee this path never used. Keeping the closure here — beside the runtime registry that
    /// owns the widget — makes the signal slot capture only the id and its token, both `Copy`, so
    /// the slot stays `Send + Sync` without a lock.
    ///
    /// This is safe rather than an `unsafe impl`: a cross-thread emit looks up *that* thread's
    /// table, finds nothing, warns and skips. It cannot tear the closure. A host that really does
    /// need a callback reachable from another thread asks for it explicitly with
    /// `on_click_cross_thread` / `on_close_cross_thread`, which keep the `Arc<Mutex<F>>` shape.
    #[allow(clippy::missing_const_for_thread_local)]
    static LOCAL_CALLBACKS: LocalCallbackMap =
        RefCell::new(HashMap::new());

    /// A per-widget removal generation, bumped when `remove_callbacks` removes something for
    /// that widget.
    ///
    /// `dispatch_trigger` takes a widget's callbacks out of the table, then a
    /// `ValueCallGuard`/`ClickCallGuard` puts them back on drop. If the callback
    /// itself called `remove_callbacks`, the guard's unconditional restore would
    /// re-register a handler the caller had just removed (D08-A-02). The guard
    /// records the generation it started with and, on drop, restores the callbacks
    /// **only if the generation is unchanged** — so a removal during dispatch wins
    /// over the restore, while an ordinary panic (which does not change the
    /// generation) still re-registers.
    #[allow(clippy::missing_const_for_thread_local)]
    static CALLBACK_GENERATIONS: RefCell<HashMap<ObjectId, u64>> = RefCell::new(HashMap::new());

    /// The ids whose callbacks are currently held by a dispatch guard (ENG-E1).
    ///
    /// `remove_callbacks` needs this because a dispatch that is in flight has already moved the
    /// callbacks out of their tables: `had_entries` is false, yet the guard still holds a snapshot
    /// whose restore must lose to this removal (D08-A-02). Together with the table removals this
    /// is what lets the generation be bumped only for ids that actually had a callback, instead of
    /// for every widget that is ever destroyed.
    #[allow(clippy::missing_const_for_thread_local)]
    static DISPATCH_IN_FLIGHT: RefCell<HashSet<ObjectId>> = RefCell::new(HashSet::new());

    /// How many callbacks have been requested for an id with no mounted widget.
    ///
    /// Thread-local like the two tables it counts for: a widget id is only meaningful on the
    /// thread that mounted it, so the same rule that makes the tables per-thread applies here.
    /// A `Cell` rather than an `AtomicUsize` for that reason — sharing the count across threads
    /// would make it a sum over threads whose ids do not refer to one another.
    ///
    /// The initializer *is* `const` and the lint still fires, because clippy reports
    /// `missing_const_for_thread_local` for the **whole `thread_local!` block** when any cell in it
    /// cannot be `const` — and the three `HashMap` tables above cannot be. The allow is therefore on
    /// every cell, matching `widget::runtime`'s registry block, rather than only on the cells that
    /// need it: a per-cell allow that does not cover the reported span does not silence it, and
    /// moving the allow to the block would hide a genuine finding in a cell that really can be
    /// `const`.
    #[allow(clippy::missing_const_for_thread_local)]
    static UNWIRED_BINDINGS: Cell<usize> = const { Cell::new(0) };
}

/// Records that `id`'s callbacks are on a dispatch guard's stack, returning whether the entry was
/// added by this call.
fn note_dispatch_in_flight(id: ObjectId) -> bool {
    DISPATCH_IN_FLIGHT
        .with(|set| set.try_borrow_mut().map(|mut set| set.insert(id)).unwrap_or(false))
}

/// Removes the in-flight note a guard added when it drops.
fn clear_dispatch_in_flight(id: ObjectId) {
    let _ = DISPATCH_IN_FLIGHT.try_with(|set| {
        if let Ok(mut set) = set.try_borrow_mut() {
            set.remove(&id);
        }
    });
}

/// Whether `id` currently has callbacks on a dispatch guard's stack.
fn dispatch_in_flight(id: ObjectId) -> bool {
    DISPATCH_IN_FLIGHT
        .try_with(|set| set.try_borrow().map(|set| set.contains(&id)).unwrap_or(false))
        .unwrap_or(false)
}

/// The current removal generation for `id`, defaulting to `0`.
fn callback_generation(id: ObjectId) -> u64 {
    CALLBACK_GENERATIONS
        .with(|map| map.try_borrow().ok().and_then(|map| map.get(&id).copied()))
        .unwrap_or(0)
}

/// Bump `id`'s removal generation, returning the new value.
///
/// Uses `try_borrow_mut` for the same re-entrancy reason as [`remove_callbacks`]: a
/// refused borrow cannot be helped here, so the previous value is returned and the
/// bump is skipped (the caller is already removing callbacks from the tables, which
/// is the operation that matters).
fn bump_callback_generation(id: ObjectId) -> u64 {
    CALLBACK_GENERATIONS.with(|map| match map.try_borrow_mut() {
        Ok(mut map) => {
            let next = map.get(&id).copied().unwrap_or(0).wrapping_add(1);
            map.insert(id, next);
            next
        }
        Err(_) => callback_generation(id),
    })
}

/// Registers `f` as the click callback for `widget_id`.
///
/// Shared by every handle that offers `on_click`, so the rule cannot differ between
/// them.
///
/// # The defect this fixes
///
/// The callback used to go into a table of its own (`CLICK_CALLBACKS`) that only
/// `dispatch_trigger(Clicked)` reads. **Nothing calls that on a real click**: a button
/// emits its own `base.clicked` from `handle_event` when the pointer is released over
/// it, and the platform never manufactures a `Clicked` trigger. The two halves never met,
/// so every `on_click` callback in a native window was dead — the control was visible,
/// its geometry was right, the click was delivered, and the callback simply never ran.
///
/// # Why this connects rather than bridging
///
/// Connecting to the widget's own signal removes the second table instead of keeping the
/// two in step. There is then one place a click is announced (`base.clicked`), whichever
/// emitter produced it — mouse release, touch end, or a tap — and the callback runs from
/// there. The alternative, having the widget's emission call into `CLICK_CALLBACKS`,
/// would leave two sources of truth for "what runs when this is clicked".
///
/// # Why the closure no longer lives behind a lock (W-P08)
///
/// See [`LOCAL_CALLBACKS`]: the slot captures only `(widget_id, token)` and the closure is stored
/// in the thread-local table the widget runtime already lives in. The old `Arc<Mutex<F>>` wrapper
/// allocated once per binding and locked on every click.
pub(crate) fn register_click_callback<F: FnMut() + Send + 'static>(widget_id: ObjectId, f: F) {
    let token = store_local_callback(widget_id, f);
    let binding = LocalCallbackBinding { widget_id, token };
    let connected = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().clicked.connect(move || {
            // Keeps the binding alive inside the slot, so the closure is removed from the table
            // when the widget (and therefore this slot) is dropped even if the host never calls
            // `remove_callbacks`. `let _ = &binding` is what makes the capture explicit: an
            // unused `move` capture would otherwise not be captured at all.
            let _ = &binding;
            run_local_callback(widget_id, token, "click")
        })
    });
    if let Some(handle) = connected {
        // Record the handle so `remove_callbacks` can really unsubscribe. The
        // returned `ConnectionHandle` used to be discarded, which left the widget
        // signal holding the callback after an explicit removal (D08-A-01).
        record_signal_connection(widget_id, SignalKind::Clicked, handle);
    } else {
        // No widget, so the stored closure can never run: drop it rather than leaking it.
        remove_local_callback(widget_id, token);
        // `warn!`, not `debug!`. The condition is not routine: it means the caller asked for a
        // callback that will never run, because there is no widget at this id. It happens for
        // real — a handle built from an id the runtime never mounted (a declarative document
        // loaded outside a window thread, a stale id after `unregister`) — and at `debug!` a
        // program that wired its whole UI to nothing said so only to whoever had turned logging
        // up. A handler that never fires is exactly the defect a subscriber cannot see from the
        // outside, so it is reported at the level the default log filter shows.
        log::warn!(
            "on_click ignored for id={widget_id}: it is not a live widget, so there is no \
             signal for a click to arrive on. The callback will never run."
        );
        record_unwired_binding(widget_id);
    }
}

/// The cross-thread form of [`register_click_callback`], keeping the `Arc<Mutex<F>>` shape.
fn register_click_callback_cross_thread<F: FnMut() + Send + 'static>(widget_id: ObjectId, f: F) {
    let held: Arc<Mutex<F>> = Arc::new(Mutex::new(f));
    let connected = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().clicked.connect(move || match held.lock() {
            Ok(mut callback) => callback(),
            Err(_) => log::warn!(
                "the click callback for widget {widget_id} could not be locked; \
                 skipping this click rather than panicking"
            ),
        })
    });
    if let Some(handle) = connected {
        record_signal_connection(widget_id, SignalKind::Clicked, handle);
    } else {
        log::warn!(
            "on_click ignored for id={widget_id}: it is not a live widget, so there is no \
             signal for a click to arrive on. The callback will never run."
        );
        record_unwired_binding(widget_id);
    }
}

/// Stores `f` in the local callback table and returns its token.
fn store_local_callback<F: FnMut() + Send + 'static>(widget_id: ObjectId, f: F) -> u64 {
    static NEXT_CALLBACK_TOKEN: AtomicU64 = AtomicU64::new(1);
    let token = NEXT_CALLBACK_TOKEN.fetch_add(1, AtomicOrdering::Relaxed);
    LOCAL_CALLBACKS.with(|map| {
        map.borrow_mut().insert((widget_id, token), Box::new(f));
    });
    token
}

/// Drops a stored local callback, used when the connection could not be made.
fn remove_local_callback(widget_id: ObjectId, token: u64) {
    // `try_with`, not `with`: the binding lives inside the widget's signal slot, which can be
    // dropped by a thread-local destructor at thread teardown. `with()` panics once the table's
    // own destruction has begun, and a panic inside a TLS destructor aborts the whole process
    // (the `control_click_reaches_widget_test` SIGABRT). By then there is nothing left to
    // remove, so the error is the correct answer.
    let _ = LOCAL_CALLBACKS.try_with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&(widget_id, token));
        }
    });
}

/// Releases the table entry when the signal slot that owns it is dropped.
///
/// The slot is owned by the widget's signal, so dropping the widget, disconnecting the slot, or
/// `remove_callbacks` all end here. That keeps the local table's lifetime tied to the connection
/// rather than to the thread: the old `Arc<Mutex<F>>` was freed by the same slot drop, and this
/// guard restores that property without putting a lock back in the dispatch path.
struct LocalCallbackBinding {
    widget_id: ObjectId,
    token: u64,
}

impl Drop for LocalCallbackBinding {
    fn drop(&mut self) {
        remove_local_callback(self.widget_id, self.token);
    }
}

/// Runs the local callback `(widget_id, token)`.
///
/// # Why it is taken out and restored
///
/// A user callback may register another handler on the same widget; holding the `RefCell` borrow
/// across the call would make that a borrow panic. The entry is therefore removed, run, and
/// restored by a `Drop` guard — which also restores it when the callback panics. The guard checks
/// the widget's removal generation first, so a `remove_callbacks` performed *inside* the callback
/// wins over the restore (the same rule D08-A-02 applies to the click/value guard).
fn run_local_callback(widget_id: ObjectId, token: u64, signal: &'static str) {
    let key = (widget_id, token);
    let generation = callback_generation(widget_id);
    let taken =
        LOCAL_CALLBACKS.with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&key)));
    let Some(callback) = taken else {
        // Most often: the signal was emitted from another thread. The table is per-thread, so the
        // closure registered beside the widget is not visible there. The thread-safe variant
        // (`on_click_cross_thread`) is the supported way to wire that case.
        log::warn!(
            "the {signal} callback for widget {widget_id} is not registered on this thread; \
             skipping it. Cross-thread wiring must use the explicit cross-thread variant."
        );
        return;
    };

    struct Restore {
        key: (ObjectId, u64),
        generation: u64,
        callback: Option<Box<dyn FnMut()>>,
    }

    impl Drop for Restore {
        fn drop(&mut self) {
            let Some(callback) = self.callback.take() else {
                return;
            };
            // A removal during dispatch wins over this restore.
            if callback_generation(self.key.0) != self.generation {
                return;
            }
            LOCAL_CALLBACKS.with(|map| {
                if let Ok(mut map) = map.try_borrow_mut() {
                    map.insert(self.key, callback);
                }
            });
        }
    }

    let mut restore = Restore { key, generation, callback: Some(callback) };
    if let Some(callback) = restore.callback.as_mut() {
        callback();
    }
}

/// Registers `f` as the close callback for `widget_id`.
///
/// The close counterpart of [`register_click_callback`], and for the same reason: a callback must
/// be connected to the widget's **own** signal, because nothing manufactures a
/// `WidgetTriggerKind::Closed` for it. Without this, `WindowHandle::on_close` (and the JSON/designer
/// `closed` route) registered a callback that nothing ever ran.
///
/// Connects to [`BaseWidget::closed`], which every closeable control emits from its own `close` /
/// `dismiss` path — so the callback fires whenever the control announces that lifecycle fact,
/// whichever code path produced it.
pub(crate) fn register_close_callback<F: FnMut() + Send + 'static>(widget_id: ObjectId, f: F) {
    let token = store_local_callback(widget_id, f);
    let binding = LocalCallbackBinding { widget_id, token };
    let connected = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().closed.connect(move || {
            let _ = &binding;
            run_local_callback(widget_id, token, "close")
        })
    });
    if let Some(handle) = connected {
        // Same recording as `register_click_callback`: the close subscription must
        // be removable, not just dropped on the floor (D08-A-01).
        record_signal_connection(widget_id, SignalKind::Closed, handle);
    } else {
        remove_local_callback(widget_id, token);
        // Same reasoning as `register_click_callback`: a close handler that can never run is the
        // failure a caller cannot see from the outside.
        log::warn!(
            "on_close ignored for id={widget_id}: it is not a live widget, so there is no signal \
             for a close to arrive on. The callback will never run."
        );
        record_unwired_binding(widget_id);
    }
}

/// The cross-thread form of [`register_close_callback`], keeping the `Arc<Mutex<F>>` shape.
fn register_close_callback_cross_thread<F: FnMut() + Send + 'static>(widget_id: ObjectId, f: F) {
    let held: Arc<Mutex<F>> = Arc::new(Mutex::new(f));
    let connected = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().closed.connect(move || match held.lock() {
            Ok(mut callback) => callback(),
            Err(_) => log::warn!(
                "the close callback for widget {widget_id} could not be locked; skipping this \
                 close rather than panicking"
            ),
        })
    });
    if let Some(handle) = connected {
        record_signal_connection(widget_id, SignalKind::Closed, handle);
    } else {
        log::warn!(
            "on_close ignored for id={widget_id}: it is not a live widget, so there is no signal \
             for a close to arrive on. The callback will never run."
        );
        record_unwired_binding(widget_id);
    }
}

/// Records that a callback was requested for an id with no live widget.
///
/// # Why a count and not just a log line
///
/// A log line answers "did this happen?" only for someone who was watching at the time. A program
/// that loads a document off the UI thread wires every handler it declares to nothing, and the
/// question "how many of my handlers are unwired?" has to be answerable *afterwards*, from the
/// program. This is that answer, and it is the same shape for both callback tables because a
/// caller asking the question is asking about the wiring as a whole.
pub fn unwired_binding_count() -> usize {
    UNWIRED_BINDINGS.with(|count| count.get())
}

/// A per-widget account of the bindings the handle layer currently holds (W-300).
///
/// # Why one report rather than one number
///
/// `unwired_binding_count` answers "how many bindings were refused?", but a host debugging a wire
/// needs the other half: "how many bindings does this control actually have, and of which kind?".
/// Counting a control's click callbacks without its close connections makes "the click works but
/// close does not" impossible to see from the API, so every table the layer owns is reported here.
/// The counts are of **subscriptions**, not handler names: two declarations that share a handler
/// name are two bindings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BindingReport {
    /// Callbacks held by the legacy per-widget click table.
    pub click_callbacks: usize,
    /// Callbacks held by the legacy per-widget value-changed table.
    pub value_callbacks: usize,
    /// Lock-free closures registered by `on_click`.
    pub local_callbacks: usize,
    /// Connections recorded for the widget's own `clicked` signal.
    pub click_connections: usize,
    /// Connections recorded for the widget's own `closed` signal.
    pub close_connections: usize,
    /// Connections recorded for payload-carrying `on_triggered` events.
    pub triggered_connections: usize,
}

impl BindingReport {
    /// The total number of subscriptions across every table.
    pub fn total(&self) -> usize {
        self.click_callbacks
            + self.value_callbacks
            + self.local_callbacks
            + self.click_connections
            + self.close_connections
            + self.triggered_connections
    }
}

/// The bindings currently held for `widget_id`, per table (W-300).
///
/// A table that is borrowed at the moment of the query contributes `0` rather than panicking: this
/// runs from callback-adjacent code, and a refused borrow means the answer cannot be read, not that
/// the widget has no bindings. The own-signal connections are counted by kind, so a caller can tell
/// a click wire from a close wire on the same control.
pub fn binding_report(widget_id: ObjectId) -> BindingReport {
    let click_callbacks = CLICK_CALLBACKS
        .with(|map| map.try_borrow().ok().and_then(|map| map.get(&widget_id).map(Vec::len)))
        .unwrap_or(0);
    let value_callbacks = VALUE_CALLBACKS
        .with(|map| map.try_borrow().ok().and_then(|map| map.get(&widget_id).map(Vec::len)))
        .unwrap_or(0);
    let local_callbacks = LOCAL_CALLBACKS
        .with(|map| {
            map.try_borrow().ok().map(|map| map.keys().filter(|(id, _)| *id == widget_id).count())
        })
        .unwrap_or(0);
    let (click_connections, close_connections, triggered_connections) = SIGNAL_CONNECTIONS
        .with(|map| {
            map.try_borrow().ok().map(|map| {
                let mut clicked = 0;
                let mut closed = 0;
                let mut triggered = 0;
                if let Some(connections) = map.get(&widget_id) {
                    for connection in connections {
                        match connection.kind {
                            SignalKind::Clicked => clicked += 1,
                            SignalKind::Closed => closed += 1,
                            SignalKind::Triggered | SignalKind::TriggeredEntry => triggered += 1,
                        }
                    }
                }
                (clicked, closed, triggered)
            })
        })
        .unwrap_or((0, 0, 0));
    BindingReport {
        click_callbacks,
        value_callbacks,
        local_callbacks,
        click_connections,
        close_connections,
        triggered_connections,
    }
}

/// Clears the unwired-binding count. Exists so a test can assert about one scenario rather than
/// about every scenario that ran before it on the same thread.
pub fn reset_unwired_binding_count() {
    UNWIRED_BINDINGS.with(|count| count.set(0));
}

/// Bumps the unwired-binding counter. See [`unwired_binding_count`].
///
/// `pub(crate)` because a binding can be refused outside this module: the JSON loader
/// declines a marker whose trigger has no signal (see `JsonTriggerMarker::unroutable_reason`),
/// and that refusal must reach the same observable count as a handle wired to a dead id, so a
/// caller asking "was anything left unwired?" gets one answer rather than two.
pub(crate) fn record_unwired_binding(_widget_id: ObjectId) {
    UNWIRED_BINDINGS.with(|count| count.set(count.get().saturating_add(1)));
}

/// Removes and returns every click callback for `widget_id`.
///
/// `try_borrow_mut` rather than `borrow_mut`: this runs from [`dispatch_trigger`],
/// which is itself reachable from inside a callback, and a refused borrow must not
/// become a panic there. A refused borrow also means the map cannot be read, so the
/// answer is "no callbacks" rather than a guess.
fn take_click_callbacks(widget_id: ObjectId) -> Vec<ClickCallback> {
    CLICK_CALLBACKS
        .with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&widget_id)))
        .unwrap_or_default()
}

/// Removes and returns every value-changed callback for `widget_id`.
///
/// See [`take_click_callbacks`] for why the borrow is attempted rather than taken.
fn take_value_callbacks(widget_id: ObjectId) -> Vec<ValueChangedCallback> {
    VALUE_CALLBACKS
        .with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&widget_id)))
        .unwrap_or_default()
}

/// Puts click callbacks back when they go out of scope.
///
/// The job of this type is the unwind path: `dispatch_trigger` moves the callbacks
/// out of the map so a re-entrant `remove_callbacks` cannot double borrow, and a
/// plain `insert` afterwards would be skipped if a callback panicked — leaving the
/// widget registered as having callbacks that are never invoked again. Holding them in
/// a `Drop` type makes the restore unconditional.
struct ClickCallGuard {
    widget_id: ObjectId,
    callbacks: Option<Vec<ClickCallback>>,
    /// The removal generation captured when the guard was built. On drop the
    /// callbacks are restored only if it still matches, so a `remove_callbacks`
    /// that ran *inside* the callback is not undone by this restore (D08-A-02).
    generation: u64,
    /// Whether this guard registered itself in [`DISPATCH_IN_FLIGHT`] (ENG-E1).
    in_flight: bool,
}

impl Drop for ClickCallGuard {
    fn drop(&mut self) {
        // Clear the in-flight note first: the restore path below may run `remove_callbacks`'
        // readers, and the note describes the dispatch, which is over either way.
        if self.in_flight {
            clear_dispatch_in_flight(self.widget_id);
        }
        if let Some(callbacks) = self.callbacks.take() {
            if callback_generation(self.widget_id) != self.generation {
                // The callbacks were removed while they were dispatched; honour the
                // removal instead of resurrecting them.
                return;
            }
            CLICK_CALLBACKS.with(|map| {
                if let Ok(mut map) = map.try_borrow_mut() {
                    // **Prepend**, don't replace: a callback registered while the dispatch was in
                    // flight (a handler that binds another handler) must survive. `insert` would
                    // have dropped it, which is the same "second binding disappears" defect in a
                    // different place.
                    map.entry(self.widget_id).or_default().splice(0..0, callbacks);
                } else {
                    log::warn!(
                        "could not restore the click callbacks for widget {}: the registry is \
                         already borrowed; the callbacks are dropped",
                        self.widget_id
                    );
                }
            });
        }
    }
}

/// Puts value-changed callbacks back when they go out of scope.
///
/// See [`ClickCallGuard`]; the two registries are separate, so each needs its own
/// guard rather than one generic over the map.
struct ValueCallGuard {
    widget_id: ObjectId,
    callbacks: Option<Vec<ValueChangedCallback>>,
    /// See [`ClickCallGuard::generation`].
    generation: u64,
    /// See [`ClickCallGuard::in_flight`].
    in_flight: bool,
}

impl Drop for ValueCallGuard {
    fn drop(&mut self) {
        if self.in_flight {
            clear_dispatch_in_flight(self.widget_id);
        }
        if let Some(callbacks) = self.callbacks.take() {
            if callback_generation(self.widget_id) != self.generation {
                return;
            }
            VALUE_CALLBACKS.with(|map| {
                if let Ok(mut map) = map.try_borrow_mut() {
                    map.entry(self.widget_id).or_default().splice(0..0, callbacks);
                } else {
                    log::warn!(
                        "could not restore the value callbacks for widget {}: the registry is \
                         already borrowed; the callbacks are dropped",
                        self.widget_id
                    );
                }
            });
        }
    }
}

/// Record a subscription made to a widget's own signal, for later removal.
///
/// `kind` is ignored when the widget is not live at removal time (the runtime
/// entry will be gone), so the entry is still useful as bookkeeping; it is
/// cleared by [`remove_callbacks`] either way.
pub(crate) fn record_signal_connection(
    widget_id: ObjectId,
    kind: SignalKind,
    handle: ConnectionHandle,
) {
    SIGNAL_CONNECTIONS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.entry(widget_id).or_default().push(SignalConnection { kind, handle });
        }
    });
}

/// Remove all registered callbacks for the given widget id.
///
/// Call this when a widget is destroyed to prevent callback leaks
/// from thread-local storage.
///
/// Besides clearing the legacy `CLICK_CALLBACKS` / `VALUE_CALLBACKS` tables,
/// this disconnects the widget's own `clicked` / `closed` signals at the handles
/// `on_click` / `on_close` recorded, so an explicitly removed handler really
/// stops running and its captured resources are released (D08-A-01). Without
/// that step the connect-time handle was discarded and the signal kept invoking
/// a callback the caller had unmounted.
///
/// Uses `try_borrow_mut` to avoid panicking when called re-entrantly
/// (e.g. during callback dispatch when a handle is dropped).
pub fn remove_callbacks(id: ObjectId) {
    // The generation is bumped only for an id that actually held a callback (or has one on a
    // dispatch guard's stack). The old code bumped unconditionally on every destroy, and ids are
    // never reused, so `CALLBACK_GENERATIONS` grew by one entry per widget ever destroyed — the
    // one per-id table the LYT-X06 cleanup chain did not shrink (ENG-E1). A bump is still
    // required while a dispatch is in flight even with empty tables: the guard's restore must
    // lose to this removal (D08-A-02).
    let mut had_entries = false;
    CLICK_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            had_entries |= map.remove(&id).is_some();
        }
    });
    // The lock-free convenience path stores the closure itself beside the runtime registry, so an
    // explicit removal must drop it too; otherwise every removed handler stayed alive until the
    // thread exited (W-P08).
    let mut removed_local = 0usize;
    LOCAL_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.retain(|(widget_id, _), _| {
                if *widget_id == id {
                    removed_local += 1;
                    false
                } else {
                    true
                }
            });
        }
    });
    had_entries |= removed_local > 0;
    VALUE_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            had_entries |= map.remove(&id).is_some();
        }
    });
    // Bump after the tables are read, and before anything below can re-enter a guard's drop: a
    // guard that is mid-dispatch on this thread must see the change and decline to restore the
    // callbacks it holds (D08-A-02).
    if had_entries || dispatch_in_flight(id) {
        bump_callback_generation(id);
    }
    // The IME pre-edit observers are stored by the widget runtime (they are per-widget state like
    // focus and hover), so an explicit "remove every callback for this id" must clear them too.
    crate::widget::runtime::clear_ime_preedit_callbacks(id);
    // Mirror state tables are caches of property values, not authoritative data; a closed
    // widget's entry can never be read again, so keeping it was an unbounded leak in a
    // create/destroy loop. The callbacks above had this cleanup already; the mirrors did not.
    SLIDER_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    PROGRESS_BAR_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    CHECKBOX_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    RADIO_BUTTON_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    LINE_EDIT_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    SCROLL_AREA_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    LIST_VIEW_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    SPINBOX_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    PANEL_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    // The layout registries are keyed by the same id and hold a `Box<dyn Layout>` each, so a
    // create/destroy loop leaked a whole layout per window, panel and declarative container.
    // They are registration caches, not authoritative state: once the widget is gone nothing
    // can lay it out again.
    LAYOUTS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    PANEL_LAYOUTS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    LAYOUT_CONTEXT_SCALES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    WINDOW_STATES.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    WINDOW_CLOSE_CALLBACKS.with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            map.remove(&id);
        }
    });
    // Disconnect the signal subscriptions this widget's handles made. The
    // handles are taken out of the table first so a re-entrant removal cannot see
    // the same handles twice, then applied against the live widget.
    let connections: Vec<SignalConnection> = SIGNAL_CONNECTIONS
        .with(|map| map.try_borrow_mut().ok().and_then(|mut map| map.remove(&id)))
        .unwrap_or_default();
    if !connections.is_empty() {
        crate::widget::runtime::with_widget_mut(id, |widget| {
            for connection in &connections {
                match connection.kind {
                    SignalKind::Clicked => {
                        widget.base().clicked.disconnect(connection.handle);
                    }
                    SignalKind::Closed => {
                        widget.base().closed.disconnect(connection.handle);
                    }
                    SignalKind::Triggered => {
                        if let Some(bar) = (widget as &mut dyn core::any::Any)
                            .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>(
                        ) {
                            bar.triggered.disconnect(connection.handle);
                        } else if let Some(menu) = (widget as &mut dyn core::any::Any)
                            .downcast_mut::<crate::widget::menu_toolbar::menu::Menu>(
                        ) {
                            menu.triggered.disconnect(connection.handle);
                        }
                    }
                    SignalKind::TriggeredEntry => {
                        if let Some(bar) = (widget as &mut dyn core::any::Any)
                            .downcast_mut::<crate::widget::menu_toolbar::menu_bar::MenuBar>(
                        ) {
                            bar.triggered_entry.disconnect(connection.handle);
                        }
                    }
                }
            }
        });
    }
}

/// Registers `f` as a value-changed callback for `widget_id`.
///
/// # The check this performs that `register_click_callback` does not need
///
/// A value callback is *pushed into a table* rather than connected to a signal, because a
/// control's value travels through the router rather than through one signal of its own. The
/// table therefore has no widget to refuse the write — an id that addresses no widget still gets
/// an entry, and nothing ever removes it, because the only remover is a handle's `Drop` and the
/// handle may outlive the widget. The result is a callback that is registered, never fires, and
/// keeps a closure alive.
///
/// Asking the runtime whether the id is mounted is what closes that window, and it is the same
/// question `register_click_callback` gets answered for free by the connect. It costs one probe
/// and is asked only while wiring, not per event.
pub(crate) fn register_value_callback(widget_id: ObjectId, f: ValueChangedCallback) {
    if !crate::widget::runtime::is_mounted(widget_id) {
        // `warn!`, for the reason spelled out on `register_click_callback`: a handler that can
        // never run is the failure a caller cannot see from the outside.
        log::warn!(
            "on_value_changed ignored for id={widget_id}: it is not a live widget, so no trigger \
             will reach it. The callback will never run."
        );
        record_unwired_binding(widget_id);
        return;
    }
    VALUE_CALLBACKS.with(|map| {
        map.borrow_mut().entry(widget_id).or_default().push(f);
    });
}

/// Registers a value-changed callback directly, without going through a handle.
///
/// # Why this exists rather than a test reaching into the registry
///
/// [`dispatch_trigger`] resolves a `ValueChanged` trigger through `VALUE_CALLBACKS`, and
/// the only production writer is `WindowHandle::on_value_changed` -- which needs a
/// *handle*, and therefore a window, to install one. A test that observes the dispatch
/// path (BLUE24 §1 criterion 3 drives a frame that must drain before it advances) has
/// neither, and reaching into the private registry from another module would make the
/// test an unofficial second writer of state the router reads.
///
/// Writing through the same table the handle writes keeps one writer, so what a test
/// observes through this seam is the production path rather than a parallel one.
#[cfg(test)]
pub(crate) fn set_widget_value_callback(id: ObjectId, callback: ValueChangedCallback) {
    VALUE_CALLBACKS.with(|map| {
        map.borrow_mut().entry(id).or_default().push(callback);
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
            let callbacks = take_click_callbacks(widget_id);
            if callbacks.is_empty() {
                return false;
            }
            // The guard holds the callbacks and puts them back when it goes out of
            // scope, including on unwind, so a panicking callback cannot silently
            // unregister itself. **Every** callback runs: a widget with two handlers
            // must fire both, which is what `Vec` here is for.
            let guard = ClickCallGuard {
                widget_id,
                callbacks: Some(callbacks),
                generation: callback_generation(widget_id),
                in_flight: note_dispatch_in_flight(widget_id),
            };
            for callback in guard.callbacks.as_ref().expect("set just above") {
                if let Ok(mut f) = callback.try_borrow_mut() {
                    f();
                } else {
                    log::warn!(
                        "the click callback for widget {widget_id} is already running; \
                         refusing to re-enter it rather than panicking"
                    );
                }
            }
            true
        }
        WidgetTriggerKind::ValueChanged | WidgetTriggerKind::SelectionChanged => {
            let callbacks = take_value_callbacks(widget_id);
            if callbacks.is_empty() {
                return false;
            }
            let text = crate::get_widget_text(widget_id);
            let guard = ValueCallGuard {
                widget_id,
                callbacks: Some(callbacks),
                generation: callback_generation(widget_id),
                in_flight: note_dispatch_in_flight(widget_id),
            };
            for callback in guard.callbacks.as_ref().expect("set just above") {
                if let Ok(mut f) = callback.try_borrow_mut() {
                    f(text.as_str());
                } else {
                    log::warn!(
                        "the value callback for widget {widget_id} is already running; \
                         refusing to re-enter it rather than panicking"
                    );
                }
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
            //
            // # Why the no-size case is reported rather than dropped
            //
            // This branch used to have no `else`. A `Resized` event whose size could not
            // be resolved was consumed and discarded in silence — and because the size is
            // the *only* thing that decides whether the branch does anything, a backend
            // that queued the event against an id the control backend did not know looked
            // identical to a window that had simply not been resized. That is how the
            // Windows `WM_SIZE` arm reported a platform id for years without anyone
            // noticing: the event was queued, refused on the way in, or resolved to
            // nothing on the way out, and no log line existed at either point. Warning
            // here makes the failure diagnosable from the host's own output.
            if let Some((width, height)) = crate::window_client_size(widget_id) {
                set_window_size(widget_id, width, height);
                apply_window_layout(widget_id);
                // The layout pass moves children, but it cannot *guarantee* a paint:
                // a layout whose children keep their geometry (a fixed-size control
                // centered in a larger window) changes only the background around
                // them. Asking for the window explicitly is what makes a backend
                // that paints the whole tree (GTK, Win32) redraw after the OS
                // resized it, instead of keeping the old frame until an unrelated
                // expose happens to arrive.
                crate::widget::runtime::request_repaint(widget_id);
                // Re-running the layout is only half the contract the event type
                // documents. `Event::Resize` existed, was published, and had a
                // constructor — but nothing ever produced it, so a control that handled
                // it (the code editor refreshes its visible rows and re-measures) never
                // heard about a resize at all. Delivering it **after** the layout pass
                // means a handler that reads its own geometry sees the new box, not the
                // stale one.
                crate::widget::runtime::dispatch_resize(widget_id, width, height);
            } else {
                log::warn!(
                    "a Resized trigger arrived for widget {widget_id}, but no client size is \
                     recorded for it; the layout was not re-run"
                );
            }
            false
        }
        WidgetTriggerKind::Moved => {
            // The window moved without resizing. A layout anchored to the screen (a panel that
            // tracks a display edge, a popup placed relative to the window) must re-run, but the
            // window's own size is unchanged, so this does not touch the size record.
            apply_window_layout(widget_id);
            false
        }
    }
}

/// Delivers every queued trigger, dispatching each one.
///
/// Returns the number of events consumed.
///
/// # Where the implementation lives
///
/// At the **crate root** ([`crate::drain_triggers`]). This profile's `crate::app` is gated
/// `full_widgets`, but the trigger queue is available to every profile except
/// `alloc_frugal`, and the platform backends — which are not gated — are the actual
/// callers. While the only definition lived here, each backend's call compiled on
/// `desktop` and failed on `mini` with `cannot find 'app' in 'crate'`.
///
/// This re-export is kept because it is part of the published surface (`crate::app` is
/// the documented entry point for applications) and existing callers name it.
pub use crate::drain_triggers;

// ─────────────────────────────────────────────────────────────────────────────
// The one route to a mounted control's properties
// ─────────────────────────────────────────────────────────────────────────────

/// Writes one property on the control the id addresses, and reports whether it took.
///
/// # Why every handle setter must go through here
///
/// The handles used to write an in-process mirror and then call
/// [`Platform::set_widget_value`](crate::platform::Platform::set_widget_value) and its
/// siblings — a surface that exists for **host-native** controls. This library creates no
/// native controls: `App`-created widgets are mounted into `widget::runtime` by
/// `control_backend::custom`, so the platform holds no record for their ids and every one of
/// those calls answered `false`. The result was discarded, the mirror was written anyway, and
/// the getters read only the mirror — measured, with a line edit created through a
/// `WindowHandle`:
///
/// ```text
/// platform.set_widget_placeholder(id, "hint") -> false
/// handle.placeholder()                        -> "hint"      <- what the getter reported
/// widget read_property(placeholder_text)      -> String("")  <- what the control held
/// ```
///
/// So a host could set a slider's value, a field's placeholder or a check box's state and the
/// control never received it, while the read-back said it had. Two mechanisms for one fact
/// (only one of which reaches the control), and a mirror that drifted into a wrong answer.
///
/// This helper is the single mechanism. It is the same one `write_spinbox_affix` already used
/// for the spin box's affixes, generalised: `WidgetFactory::write_property` dispatches on the
/// control's own [`WidgetProperties`](crate::widget::capability::WidgetProperties) contract, so
/// a control that does not publish `name` **refuses** the write rather than accepting it into
/// a store nothing reads.
///
/// Returns `false` when the id is not a mounted widget, or when the control refused the write
/// (an unpublished name, or a value of the wrong kind). The refusal is also logged, because a
/// silent `false` is the defect this replaced.
pub(crate) fn write_control_property(
    id: ObjectId,
    name: &str,
    value: crate::widget::capability::CapabilityValue,
) -> bool {
    let written = crate::widget::runtime::with_widget_mut(id, |widget| {
        crate::widget::capability::WidgetFactory::shared().write_property(widget, name, value)
    });
    match written {
        Some(Ok(())) => true,
        Some(Err(error)) => {
            log::warn!("[handle] `{name}` was refused for id={id}: {error:?}");
            false
        }
        None => {
            log::warn!("[handle] `{name}` ignored for id={id}: the id is not a mounted widget");
            false
        }
    }
}

/// Records the outcome of a two-property write whose caller has no error channel.
///
/// Several setters (`set_range`, `set_content_size`, `set_scroll_position`) write a pair and
/// return `()`. They used to compose the two results as `let _ = a && b;`, which both discarded
/// the fact and read as a placeholder expression (project rule: no pseudo-expressions). This
/// keeps the call site meaningful: the individual refusal is already logged at `warn` by
/// [`write_control_property`], and this adds the operation that issued it.
pub(crate) fn note_property_pair(operation: &str, first: bool, second: bool) {
    if !(first && second) {
        log::debug!(
            "[handle] {operation}: one of the two property writes was refused \
             (first={first}, second={second})"
        );
    }
}

/// Reads one property off the control the id addresses.
///
/// `None` means "this control does not publish that name, or the id is not mounted" — the
/// same two cases, which a caller distinguishes by whether it expects the control to have the
/// property at all. See [`write_control_property`] for why the platform is not asked.
pub(crate) fn read_control_property(
    id: ObjectId,
    name: &str,
) -> Option<crate::widget::capability::CapabilityValue> {
    crate::widget::runtime::with_widget_mut(id, |widget| {
        crate::widget::capability::WidgetFactory::shared().read_property(widget, name).ok()
    })
    .flatten()
}

/// Extracts a `f64` from a numeric capability value, whichever carrier it arrived in.
fn capability_as_f64(value: &crate::widget::capability::CapabilityValue) -> Option<f64> {
    use crate::widget::capability::CapabilityValue as V;
    match value {
        V::Int(v) => Some(*v as f64),
        V::UInt(v) => Some(*v as f64),
        V::Float(v) => Some(*v),
        _ => None,
    }
}

/// The carrier `single_step` expects, for the value given.
///
/// The property is declared `Int` on the slider, dial and scroll bar and `Number` on the spin
/// box, so an integral step is written as the integer carrier the declaration asks for. A
/// fractional step keeps the float carrier; a control that cannot represent it **refuses**
/// (the funnel logs it) rather than silently rounding the step the caller asked for.
pub(crate) fn step_carrier(step: f64) -> crate::widget::capability::CapabilityValue {
    if step.fract() == 0.0 && step >= i64::MIN as f64 && step <= i64::MAX as f64 {
        crate::widget::capability::CapabilityValue::Int(step as i64)
    } else {
        crate::widget::capability::CapabilityValue::Float(step)
    }
}

/// The published spelling of an [`EchoMode`].
///
/// The property contract carries tokens, not Rust variants, so the handle and the control name
/// the three modes the same way. Kept beside the reverse mapping so a mode added to the enum
/// fails to compile here rather than silently losing its token.
pub(crate) fn echo_mode_token(mode: EchoMode) -> &'static str {
    match mode {
        EchoMode::Normal => "normal",
        EchoMode::Password => "password",
        EchoMode::NoEcho => "no_echo",
    }
}

/// The [`EchoMode`] a published token names, or `None` for an unknown token.
fn echo_mode_from_token(token: &str) -> Option<EchoMode> {
    match token {
        "normal" => Some(EchoMode::Normal),
        "password" => Some(EchoMode::Password),
        "no_echo" => Some(EchoMode::NoEcho),
        _ => None,
    }
}

/// Extracts a `usize` from an unsigned capability value.
fn capability_as_usize(value: &crate::widget::capability::CapabilityValue) -> Option<usize> {
    use crate::widget::capability::CapabilityValue as V;
    match value {
        V::UInt(v) => usize::try_from(*v).ok(),
        V::Int(v) => usize::try_from(*v).ok(),
        _ => None,
    }
}
