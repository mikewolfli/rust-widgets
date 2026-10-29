// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pointer-event handles: the controls driven by clicks and drags, and the
//! `impl_handle!` macro that generates the standard handle boilerplate.

use super::*;
use crate::core::{ObjectId, Orientation, Rect};
use crate::widget::numeric::{ordered_clamp_i32, ordered_clamp_u32};
use alloc::rc::Rc;
use core::cell::RefCell;
use std::collections::HashMap;

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

            /// Removes every callback registered against this handle's id.
            ///
            /// # Why this is explicit rather than automatic
            ///
            /// [`Drop`] used to call `remove_callbacks` unconditionally, on the reasoning that a
            /// handle owns the callbacks it installed. It does not: a handle is a *view* of an
            /// `ObjectId` that any number of callers can build from the same id, and the
            /// callbacks are keyed by that id rather than by the handle. Three things followed,
            /// all of them defects:
            ///
            /// 1. **A temporary handle wiped its own wiring.** `ButtonHandle::from_raw(id)
            ///    .on_click(f)` — the idiomatic one-liner, and what the JSON loader's event
            ///    binder does — installed `f` and then dropped the handle, which removed it.
            ///    Every declarative `events:`/`on_*` binding was installed and uninstalled in
            ///    the same statement, so no document's handler ever ran.
            /// 2. **One handle's drop removed another's callbacks.** Two handles for the same
            ///    widget (the loader's binding and the caller's `widget_by_name`) shared nothing
            ///    but the id, so the first to drop unregistered both.
            /// 3. **Dropping a handle for a live widget is ordinary.** A helper that resolves a
            ///    handle, reads a property and returns is not a teardown, and removing
            ///    subscriptions made it one.
            ///
            /// So a drop is now inert and unsubscribing is this call. It mirrors the widget
            /// runtime itself, where `register` mounts a control that outlives whatever box
            /// produced it — the id is the identity, and the handles are views of it.
            ///
            /// A caller that *is* tearing the widget down calls this; `destroy_widget` and the
            /// window-close path already do, so no teardown path lost its cleanup.
            pub fn remove_callbacks(&self) {
                remove_callbacks(self.id);
            }
        }

        impl WidgetHandle for $name {
            fn raw_id(&self) -> ObjectId {
                self.id
            }

            fn from_raw(id: ObjectId) -> Self {
                Self { id }
            }

            fn on_click<F: FnMut() + Send + 'static>(&self, f: F) {
                // Shared with `WindowHandle::on_click`: a click callback must be connected
                // to the widget's own `clicked` signal, or nothing runs it. See
                // `register_click_callback` for the defect this address.
                register_click_callback(self.id, f);
            }

            fn on_value_changed<F: FnMut(&str) + 'static>(&self, f: F) {
                register_value_callback(self.id, Rc::new(RefCell::new(f)));
            }
        }

        /// Dropping a handle deliberately does nothing; use the `remove_callbacks` method.
        ///
        /// Written out rather than omitted so the *absence* is a documented decision: an empty
        /// `impl Drop` is a statement, where no `impl` at all reads as "nobody thought about it".
        impl Drop for $name {
            fn drop(&mut self) {}
        }
    };
}
pub(crate) use impl_handle;
impl_handle!(ButtonHandle, "Type-safe handle for a Button widget.");
impl_handle!(LabelHandle, "Type-safe handle for a Label widget.");
impl_handle!(CheckBoxHandle, "Type-safe handle for a CheckBox widget.");
impl_handle!(RadioButtonHandle, "Type-safe handle for a RadioButton widget.");
impl_handle!(ComboBoxHandle, "Type-safe handle for a ComboBox widget.");
impl_handle!(ListBoxHandle, "Type-safe handle for a ListBox widget.");
impl_handle!(SliderHandle, "Type-safe handle for a Slider widget.");
impl_handle!(ProgressBarHandle, "Type-safe handle for a ProgressBar widget.");
impl_handle!(SplitterHandle, "Type-safe handle for a Splitter (pane container) widget.");
impl_handle!(PanelHandle, "Type-safe handle for a Panel widget.");
impl_handle!(ListViewHandle, "Type-safe handle for a ListView widget.");
impl_handle!(TabWidgetHandle, "Type-safe handle for a TabWidget (tab container) widget.");
impl_handle!(GridWidgetHandle, "Type-safe handle for a GridWidget (grid layout) widget.");
impl_handle!(FrameHandle, "Type-safe handle for a Frame widget.");
impl_handle!(DialogHandle, "Type-safe handle for a generic Dialog widget.");
impl_handle!(WebViewHandle, "Type-safe handle for a WebView (web content) widget.");

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

    /// Whether the drop-down list is showing.
    ///
    /// Reads the control's published `open` property, so it describes what was painted rather
    /// than a host-side copy. `false` for an id that is not a combo box.
    pub fn is_open(&self) -> bool {
        crate::widget::capability::widget_access::read_flag(self.raw_id(), &["open"])
            .unwrap_or(false)
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
pub(crate) struct SliderState {
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    pub(crate) orientation: Orientation,
}

impl Default for SliderState {
    /// `value: 0`, matching [`Slider::new`](crate::widget::display_widgets::slider::Slider::new)
    /// and the capability default (`Int(0)`).
    ///
    /// It used to be `50`, which made a freshly created slider disagree three ways: the control's
    /// own property route answered `0`, the schema default was `0`, and the handle reported and
    /// **wrote back** `50` (see `set_value`, which re-reads this mirror to push the clamped value to
    /// the backend). A caller that created a slider, called `set_range(0, 100)`, and never touched
    /// `set_value` therefore shipped `50` to the platform while every declarative route said `0`.
    fn default() -> Self {
        Self { value: 0, min: 0, max: 100, step: 1, orientation: Orientation::Horizontal }
    }
}

thread_local! {
    pub(crate) static SLIDER_STATES: RefCell<HashMap<ObjectId, SliderState>> = RefCell::new(HashMap::new());
}

/// # Slider-specific operations
impl SliderHandle {
    /// Set the current slider value (clamped to min/max range).
    ///
    /// The value is written both to the in-process mirror and to the control
    /// through its `value` property (via `write_control_property`), so the change is
    /// visible on screen and not just to `value()`. On a backend without a
    /// numeric value for sliders the mirror is still updated, because it is the
    /// authoritative copy for the self-drawn path.
    pub fn set_value(&self, value: i32) {
        // Clamped once and written from the same guarded borrow: the value used to be re-read
        // through `self.value()`, which took the mirror lock a second time.
        let clamped = SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = ordered_clamp_i32(value, state.min, state.max);
            state.value
        });
        write_control_property(
            self.raw_id(),
            "value",
            crate::widget::capability::CapabilityValue::Int(i64::from(clamped)),
        );
    }

    /// Return the current slider value.
    ///
    /// The in-process mirror is authoritative: it is what the self-drawn path
    /// renders and what a native write was clamped to, so both agree.
    ///
    /// An id with no mirror entry falls back to `SliderState::default().value` rather than to a
    /// literal, so the "not yet written" answer is the same one `Default` gives and cannot drift
    /// from the control's own starting value.
    pub fn value(&self) -> i32 {
        SLIDER_STATES.with(|map| {
            map.borrow()
                .get(&self.raw_id())
                .map(|s| s.value)
                .unwrap_or(SliderState::default().value)
        })
    }

    /// Set the slider range (min/max). The current value is clamped into it.
    ///
    /// # Crossed bounds
    ///
    /// The two bounds are ordered rather than trusted: `set_range(100, 0)` is a
    /// descending slider, and the current value is clamped into the span it
    /// defines. This used to write the pair through and then clamp against it,
    /// and `i32::clamp` **panics** on `min > max` — so a perfectly ordinary
    /// descending range took the process down at the first call. The range is
    /// still forwarded to the backend exactly as given, because which end is
    /// the minimum is the caller's choice, not something to silently rewrite.
    pub fn set_range(&self, min: i32, max: i32) {
        SLIDER_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.max = max;
            state.value = ordered_clamp_i32(state.value, state.min, state.max);
        });
        {
            let low = write_control_property(
                self.raw_id(),
                "minimum",
                crate::widget::capability::CapabilityValue::Int(i64::from(min)),
            );
            let high = write_control_property(
                self.raw_id(),
                "maximum",
                crate::widget::capability::CapabilityValue::Int(i64::from(max)),
            );
            // Both halves are written; a refusal has already been logged by the funnel, and this
            // signature has no channel to report one on.
            note_property_pair("slider.set_range", low, high);
        };
    }

    /// Set the slider step increment.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well (the write goes through the control's own `single_step`).
    pub fn set_step(&self, step: i32) {
        SLIDER_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
        write_control_property(self.raw_id(), "single_step", step_carrier(f64::from(step)));
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
pub(crate) struct ProgressBarState {
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
    pub(crate) static PROGRESS_BAR_STATES: RefCell<HashMap<ObjectId, ProgressBarState>> = RefCell::new(HashMap::new());
}

/// # Progress-bar specific operations
impl ProgressBarHandle {
    /// Set the current progress value (clamped to min/max).
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through the control's own `value` property, so the bar actually moves. (It used to call
    /// `Platform::set_widget_value`, a host-native surface this library never populates: the call
    /// answered `false` and was dropped, so the bar did not move at all.)
    pub fn set_value(&self, value: u32) {
        // Clamped once and written from the same guarded borrow (see `SliderHandle::set_value`).
        let clamped = PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.value = ordered_clamp_u32(value, state.min, state.max);
            state.value
        });
        write_control_property(
            self.raw_id(),
            "value",
            crate::widget::capability::CapabilityValue::Int(i64::from(clamped)),
        );
    }

    /// Return the current progress value.
    pub fn value(&self) -> u32 {
        PROGRESS_BAR_STATES
            .with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the minimum value.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well (the write goes through the control's own bounds).
    ///
    /// # Crossed bounds
    ///
    /// `set_min(100)` on a bar still at its `0..100` default crosses the bounds.
    /// `u32::clamp` panics on that, and although the panic is not reachable here
    /// (the call sites clamp with the ordering already applied) the pair was
    /// forwarded to the control as `(100, 0)` — an inverted range stored on the
    /// control, read back verbatim by `range()`. Both are fixed by
    /// the same change: order the bounds once, use that ordering for the clamp
    /// and for the write.
    pub fn set_min(&self, min: u32) {
        let range = PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.min = min;
            state.value = ordered_clamp_u32(state.value, state.min, state.max);
            (state.min.min(state.max), state.min.max(state.max))
        });
        {
            let low = write_control_property(
                self.raw_id(),
                "minimum",
                crate::widget::capability::CapabilityValue::Int(i64::from(range.0)),
            );
            let high = write_control_property(
                self.raw_id(),
                "maximum",
                crate::widget::capability::CapabilityValue::Int(i64::from(range.1)),
            );
            note_property_pair("progress_bar.set_min", low, high);
        };
    }

    /// Set the maximum value.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well (the write goes through the control's own bounds).
    ///
    /// # Crossed bounds
    ///
    /// Mirror image of [`ProgressBarHandle::set_min`]: `set_max(0)` on a `0..100`
    /// bar crosses the bounds, and the inverted pair used to be written through.
    /// The ordering is applied once and used for both the clamp and the write.
    pub fn set_max(&self, max: u32) {
        let range = PROGRESS_BAR_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.max = max;
            state.value = ordered_clamp_u32(state.value, state.min, state.max);
            (state.min.min(state.max), state.min.max(state.max))
        });
        {
            let low = write_control_property(
                self.raw_id(),
                "minimum",
                crate::widget::capability::CapabilityValue::Int(i64::from(range.0)),
            );
            let high = write_control_property(
                self.raw_id(),
                "maximum",
                crate::widget::capability::CapabilityValue::Int(i64::from(range.1)),
            );
            note_property_pair("progress_bar.set_max", low, high);
        };
    }

    /// Set whether the progress bar is in indeterminate mode.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_indeterminate`], so a native bar
    /// animates instead of showing a fixed fraction.
    /// # Limitation
    ///
    /// `ProgressBar` publishes no `indeterminate` property (the concept lives on `Arc` and
    /// `ProgressCircle`), so this write is **refused** and the state is not recorded: the mirror
    /// is only updated once the control accepted the value, which keeps `is_indeterminate` from
    /// reporting a state nothing is in. The refusal is logged.
    pub fn set_indeterminate(&self, indeterminate: bool) {
        let applied = write_control_property(
            self.raw_id(),
            "indeterminate",
            crate::widget::capability::CapabilityValue::Bool(indeterminate),
        );
        if applied {
            PROGRESS_BAR_STATES.with(|map| {
                map.borrow_mut()
                    .entry(self.raw_id())
                    .or_insert_with(Default::default)
                    .indeterminate = indeterminate;
            });
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// CheckBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub(crate) struct CheckBoxState {
    tristate: bool,
    check_state: CheckState,
}

impl Default for CheckBoxState {
    fn default() -> Self {
        Self { tristate: false, check_state: CheckState::Unchecked }
    }
}

thread_local! {
    pub(crate) static CHECKBOX_STATES: RefCell<HashMap<ObjectId, CheckBoxState>> = RefCell::new(HashMap::new());
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
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through the control's own `checked` property, so the box really moves. (See
    /// `write_control_property` for the host-native surface this replaced.)
    ///
    /// This always lands on a definite state, tri-state mode or not, so
    /// [`CheckBoxHandle::check_state`] and the control cannot disagree.
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
    /// The control's two-position `checked` flag cannot carry the mixed state, so
    /// `PartiallyChecked` is pushed as "on", while this handle keeps the precise three-way state
    /// in its mirror — which is what [`CheckBoxHandle::check_state`] reports.
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
            // Only the *control's* acceptance can change the answer: the mirror already holds the
            // precise state, and the control's two-position `checked` flag is what the box paints.
            write_control_property(
                self.raw_id(),
                "checked",
                crate::widget::capability::CapabilityValue::Bool(state != CheckState::Unchecked),
            );
        }
        applied
    }

    /// Enable/disable tri-state mode.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
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
            write_control_property(
                self.raw_id(),
                "checked",
                crate::widget::capability::CapabilityValue::Bool(false),
            );
        }
        write_control_property(
            self.raw_id(),
            "tristate_enabled",
            crate::widget::capability::CapabilityValue::Bool(tristate),
        );
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
pub(crate) struct RadioButtonState {
    selected: bool,
    group: String,
}

thread_local! {
    pub(crate) static RADIO_BUTTON_STATES: RefCell<HashMap<ObjectId, RadioButtonState>> = RefCell::new(HashMap::new());
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
    /// Both the selection *and* the de-selection of siblings are written through each control's
    /// own `checked` property, so the group is actually mutually exclusive on screen rather than
    /// only in the in-process mirror.
    pub fn select(&self) {
        let group = RADIO_BUTTON_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.selected = true;
            state.group.clone()
        });
        write_control_property(
            self.raw_id(),
            "checked",
            crate::widget::capability::CapabilityValue::Bool(true),
        );

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
            // Write each sibling's `checked` through its own property contract, exactly as this
            // button's own selection was written above. `Platform::set_widget_checked` only
            // touched the in-process mirror, so the crate-root reader (`is_widget_checked`,
            // property-backed) and this handle's mirror reader disagreed about the same control
            // (API-01). The mirror update above is kept as the handle's own read cache.
            for id in siblings {
                write_control_property(
                    id,
                    "checked",
                    crate::widget::capability::CapabilityValue::Bool(false),
                );
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
        write_control_property(
            self.raw_id(),
            "group_id",
            crate::widget::capability::CapabilityValue::String(group.to_string()),
        );
    }
}

/// # Tab-widget specific operations
///
/// A tab widget's pages are child ids, not layout tracks: one page is selected and drawn in
/// the content rect, so the operations here are content/selection operations rather than
/// `Layout` registrations. Every method answers honestly when the id is not a mounted
/// `TabWidget`.
impl TabWidgetHandle {
    /// Appends a tab titled `title`, hosting `page` (or no page).
    ///
    /// Returns the new index, or `None` when the id is not a mounted tab widget.
    pub fn add_tab(&self, title: &str, page: Option<ObjectId>) -> Option<usize> {
        let added = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.add_tab(title.to_owned(), page))
        })
        .flatten();
        if added.is_some() {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        added
    }

    /// Removes the tab at `index`.
    ///
    /// Returns `false` when the id is not a tab widget or the index is out of range.
    pub fn remove_tab(&self, index: usize) -> bool {
        let removed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.remove_tab(index);
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if removed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        removed
    }

    /// Returns the number of tabs, or `0` when the id is not a tab widget.
    pub fn tab_count(&self) -> usize {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.count())
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    /// Returns the selected tab index, or `None` when the id is not a tab widget.
    pub fn current_index(&self) -> Option<usize> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::TabWidget>(widget)
                .and_then(|tabs| tabs.selected_index())
        })
        .flatten()
    }

    /// Selects the tab at `index`.
    ///
    /// Returns `false` when the id is not a tab widget or the index is out of range; an
    /// out-of-range index is not silently clamped to the last tab.
    pub fn set_current_index(&self, index: usize) -> bool {
        let selected = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.set_current_index(index);
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if selected {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        selected
    }

    /// Replaces the title of the tab at `index`.
    pub fn set_tab_text(&self, index: usize, text: &str) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| {
                    let valid = index < tabs.count();
                    if valid {
                        tabs.set_tab_text(index, text.to_owned());
                    }
                    valid
                })
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Shows or hides close buttons on the tabs.
    pub fn set_closable(&self, closable: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_closable(closable))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Enables or disables drag-to-reorder on the tab strip.
    pub fn set_movable(&self, movable: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_movable(movable))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Moves the tab strip to one of the four sides.
    pub fn set_tab_position(&self, position: crate::app::TabPosition) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_position(position))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Selects the tab corner shape.
    pub fn set_tab_shape(&self, shape: crate::app::TabShape) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::TabWidget>(widget)
                .map(|tabs| tabs.set_tab_shape(shape))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }
}

// ═══════════════════════════════════════════════════════════════
// GridWidgetHandle – cell matrix properties
// ═══════════════════════════════════════════════════════════════

/// # Grid-widget specific operations
///
/// This is the **visual** row/column matrix. A child is placed into a cell by a
/// [`crate::layout::GridLayout`] on the grid's parent; the methods here shape the matrix
/// (rows, columns, spacing), not the child placement.
impl GridWidgetHandle {
    /// Returns the row count, or `None` when the id is not a grid widget.
    pub fn rows(&self) -> Option<u32> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::GridWidget>(widget)
                .map(|grid| grid.rows())
        })
        .flatten()
    }

    /// Returns the column count, or `None` when the id is not a grid widget.
    pub fn columns(&self) -> Option<u32> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::GridWidget>(widget)
                .map(|grid| grid.columns())
        })
        .flatten()
    }

    /// Returns the inter-cell spacing, or `None` when the id is not a grid widget.
    pub fn spacing(&self) -> Option<u32> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::GridWidget>(widget)
                .map(|grid| grid.spacing())
        })
        .flatten()
    }

    /// Sets the row count (clamped to at least 1).
    pub fn set_rows(&self, rows: u32) -> bool {
        self.mutate(|grid| grid.set_rows(rows))
    }

    /// Sets the column count (clamped to at least 1).
    pub fn set_columns(&self, columns: u32) -> bool {
        self.mutate(|grid| grid.set_columns(columns))
    }

    /// Sets the inter-cell spacing.
    pub fn set_spacing(&self, spacing: u32) -> bool {
        self.mutate(|grid| grid.set_spacing(spacing))
    }

    fn mutate(&self, f: impl FnOnce(&mut crate::widget::GridWidget)) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::GridWidget>(widget)
                .map(f)
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }
}

// ═══════════════════════════════════════════════════════════════
// SplitterHandle – pane operations
// ═══════════════════════════════════════════════════════════════

/// # Splitter-specific operations
///
/// A splitter's panes are child ids with relative weights. The panes are its content model;
/// the splitter itself computes the rectangles from the ratios.
impl SplitterHandle {
    /// Adds a pane with `stretch` relative weight and returns its index.
    pub fn add_pane(&self, pane: ObjectId, stretch: u32) -> Option<usize> {
        let added = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.add_pane(pane, stretch))
        })
        .flatten();
        if added.is_some() {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        added
    }

    /// Removes a pane by id.
    pub fn remove_pane(&self, pane: ObjectId) -> bool {
        let removed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.remove_pane(pane))
        })
        .flatten()
        .unwrap_or(false);
        if removed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        removed
    }

    /// Returns the pane count, or `0` when the id is not a splitter.
    pub fn pane_count(&self) -> usize {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.pane_count())
                .unwrap_or(0)
        })
        .unwrap_or(0)
    }

    /// Returns the splitter orientation, or `None` when the id is not a splitter.
    pub fn orientation(&self) -> Option<Orientation> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.orientation())
        })
        .flatten()
    }

    /// Sets the splitter orientation.
    pub fn set_orientation(&self, orientation: Orientation) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.set_orientation(orientation))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Sets one pane's relative ratio.
    pub fn set_ratio(&self, index: usize, ratio: f32) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::Splitter>(widget)
                .map(|splitter| splitter.set_ratio(index, ratio))
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Returns one pane's relative ratio, or `None` when the index is out of range or the id
    /// is not a splitter.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::Splitter>(widget)
                .and_then(|splitter| splitter.ratio(index))
        })
        .flatten()
    }
}

// ═══════════════════════════════════════════════════════════════
// DialogHandle – content and lifecycle operations
// ═══════════════════════════════════════════════════════════════

/// # Dialog-specific operations
///
/// A dialog owns its chrome and exactly one content child. Content placement is the content
/// widget's own layout; the dialog only gives it [`Dialog::content_rect`](crate::widget::dialog::dialog_widget::Dialog::content_rect).
impl DialogHandle {
    /// Sets the dialog title.
    pub fn set_title(&self, title: &str) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::Dialog>(
                widget,
            )
            .map(|dialog| dialog.set_title(title))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Sets or clears the dialog's content child.
    ///
    /// Returns `false` when the id is not a dialog, or when a proposed child is not mounted.
    /// Clearing (`None`) is always accepted for a mounted dialog.
    pub fn set_content_widget(&self, content: Option<ObjectId>) -> bool {
        if let Some(id) = content {
            if !crate::widget::runtime::is_mounted(id) {
                return false;
            }
        }
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::Dialog>(
                widget,
            )
            .map(|dialog| dialog.set_content_widget(content))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Returns the dialog's content child, or `None` when it has none / is not a dialog.
    pub fn content_widget(&self) -> Option<ObjectId> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::dialog::Dialog>(widget)
                .and_then(|dialog| dialog.content_widget())
        })
        .flatten()
    }

    /// Sets the modality intent.
    pub fn set_modal(&self, modal: bool) -> bool {
        let changed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::Dialog>(
                widget,
            )
            .map(|dialog| dialog.set_modal(modal))
        })
        .flatten()
        .is_some();
        if changed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        changed
    }

    /// Opens the dialog (shows it and enters the modal stack when modal).
    pub fn open(&self) -> bool {
        let opened = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::Dialog>(
                widget,
            )
            .map(|dialog| dialog.open())
        })
        .flatten()
        .is_some();
        if opened {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        opened
    }

    /// Closes the dialog (hides it and leaves the modal stack).
    pub fn close(&self) -> bool {
        let closed = crate::widget::runtime::with_widget_mut(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as_mut::<crate::widget::dialog::Dialog>(
                widget,
            )
            .map(|dialog| dialog.close())
        })
        .flatten()
        .is_some();
        if closed {
            crate::widget::runtime::request_repaint_subtree(self.raw_id());
        }
        closed
    }

    /// Returns the rectangle the content child is placed in.
    pub fn content_rect(&self) -> Option<Rect> {
        crate::widget::runtime::with_widget(self.raw_id(), |widget| {
            crate::widget::capability::coercion::widget_as::<crate::widget::dialog::Dialog>(widget)
                .map(|dialog| dialog.content_rect())
        })
        .flatten()
    }
}

// ═══════════════════════════════════════════════════════════════
// ListViewHandle – extended state
// ═══════════════════════════════════════════════════════════════

pub(crate) struct ListViewState {
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
    pub(crate) static LIST_VIEW_STATES: RefCell<HashMap<ObjectId, ListViewState>> = RefCell::new(HashMap::new());
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

    /// Select a row by index, writing it through the control's selection property and recording
    /// it in the in-process mirror.
    ///
    /// `None` clears the selection. Returns `false` when the control publishes no selection
    /// property (it neither does today — see the limitation in
    /// `handle_setters_reach_their_control_test`) or the id is not mounted, in which case the
    /// mirror is left unchanged so `selected_row()` keeps reporting the previous value rather than
    /// claiming a selection the control does not have.
    pub fn select_row(&self, row: Option<usize>) -> bool {
        let value = match row {
            Some(i) => crate::widget::capability::CapabilityValue::UInt(i as u64),
            None => crate::widget::capability::CapabilityValue::Null,
        };
        let pushed = write_control_property(self.raw_id(), "selected_index", value);
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
// PanelHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Default)]
pub(crate) struct PanelState {
    title: String,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

impl PanelState {
    pub(crate) fn with_geometry(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { title: String::new(), x, y, w, h }
    }
}

thread_local! {
    pub(crate) static PANEL_LAYOUTS: RefCell<HashMap<ObjectId, Box<dyn crate::layout::Layout>>> = RefCell::new(HashMap::new());
    pub(crate) static PANEL_STATES: RefCell<HashMap<ObjectId, PanelState>> = RefCell::new(HashMap::new());
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
    /// # Which children it positions
    ///
    /// Children created with this panel's own factories ([`PanelHandle::new_label`],
    /// [`PanelHandle::new_list_box`], …) are parented to the panel and are the normal
    /// input. A control created by the enclosing window is **also** positioned, for
    /// backwards compatibility: a panel layout can place any child whose id it is
    /// given. That is an implicit contract (the widget's parent is then the window,
    /// not the panel), so new code should prefer the panel factories, which keep the
    /// parent, clipping and layout in one container.
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

    /// Set the title alignment.
    ///
    /// The panel is the `GroupBox` control under another name, so it publishes the same
    /// `alignment` property; writing through that contract keeps a panel and a group box
    /// behaving the same instead of giving the panel a second, private field.
    pub fn set_alignment(&self, alignment: crate::core::Alignment) -> bool {
        write_control_property(
            self.raw_id(),
            "alignment",
            crate::widget::capability::CapabilityValue::String(
                crate::widget::capability::coercion::alignment_to_str(alignment).to_string(),
            ),
        )
    }

    /// Read the title alignment, or `None` when the control does not publish it.
    pub fn alignment(&self) -> Option<crate::core::Alignment> {
        read_control_property(self.raw_id(), "alignment")
            .and_then(|value| crate::widget::capability::coercion::expect_alignment(value).ok())
    }

    /// Make the panel's title checkable (a checkbox on the title row).
    pub fn set_checkable(&self, checkable: bool) -> bool {
        write_control_property(
            self.raw_id(),
            "checkable",
            crate::widget::capability::CapabilityValue::Bool(checkable),
        )
    }

    /// Read whether the panel title is checkable.
    pub fn is_checkable(&self) -> Option<bool> {
        match read_control_property(self.raw_id(), "checkable") {
            Some(crate::widget::capability::CapabilityValue::Bool(checkable)) => Some(checkable),
            _ => None,
        }
    }

    /// Toggle the panel's checked state.
    ///
    /// Returns `false` when the control publishes no `checked` property. The write goes
    /// through the same property the generic [`WidgetHandle::set_checked`] uses, so the
    /// emitted `toggled` signal and a programmatic write cannot disagree.
    pub fn toggle(&self) -> bool {
        let checked = match read_control_property(self.raw_id(), "checked") {
            Some(crate::widget::capability::CapabilityValue::Bool(checked)) => checked,
            _ => return false,
        };
        self.set_checked(!checked)
    }

    // ── Child-widget factory methods ──────────────────────
    //
    // A panel is a container, so a control created for it must be parented to the
    // **panel**, not to the window that happens to contain the panel. Once the parent
    // is the panel, the panel's own layout owns the child, clipping and layout
    // semantics stay inside one container, and the caller no longer has to know that
    // `set_layout` can position a widget created by the parent window.
    //
    // The window factories remain for callers that want window-level placement;
    // these are the panel-level spellings of the same constructors.

    /// Creates a push button as a child of this panel.
    pub fn new_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> ButtonHandle {
        ButtonHandle::from_raw(crate::create_button(self.raw_id(), text, x, y, w, h))
    }

    /// Creates a static text label as a child of this panel.
    pub fn new_label(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LabelHandle {
        LabelHandle::from_raw(crate::create_label(self.raw_id(), text, x, y, w, h))
    }

    /// Creates a check box as a child of this panel.
    pub fn new_checkbox(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> CheckBoxHandle {
        CheckBoxHandle::from_raw(crate::create_checkbox(self.raw_id(), text, x, y, w, h))
    }

    /// Creates a radio button as a child of this panel.
    pub fn new_radio_button(
        &self,
        text: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
    ) -> RadioButtonHandle {
        RadioButtonHandle::from_raw(crate::create_radio_button(self.raw_id(), text, x, y, w, h))
    }

    /// Creates a single-line text entry as a child of this panel.
    pub fn new_line_edit(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LineEditHandle {
        LineEditHandle::from_raw(crate::create_line_edit(self.raw_id(), text, x, y, w, h))
    }

    /// Creates an empty drop-down list as a child of this panel.
    pub fn new_combo_box(&self, x: i32, y: i32, w: u32, h: u32) -> ComboBoxHandle {
        ComboBoxHandle::from_raw(crate::create_combo_box(self.raw_id(), x, y, w, h))
    }

    /// Creates an empty single-selection list box as a child of this panel.
    pub fn new_list_box(&self, x: i32, y: i32, w: u32, h: u32) -> ListBoxHandle {
        ListBoxHandle::from_raw(crate::create_list_box(self.raw_id(), x, y, w, h))
    }

    /// Creates a horizontal slider as a child of this panel.
    pub fn new_slider(&self, x: i32, y: i32, w: u32, h: u32) -> SliderHandle {
        SliderHandle::from_raw(crate::create_slider(self.raw_id(), x, y, w, h))
    }

    /// Creates a progress bar as a child of this panel.
    pub fn new_progress_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ProgressBarHandle {
        ProgressBarHandle::from_raw(crate::create_progress_bar(self.raw_id(), x, y, w, h))
    }

    /// Creates a spin box as a child of this panel.
    pub fn new_spin_box(&self, x: i32, y: i32, w: u32, h: u32) -> SpinBoxHandle {
        SpinBoxHandle::from_raw(crate::create_spin_box(self.raw_id(), x, y, w, h))
    }

    /// Creates a list view as a child of this panel.
    pub fn new_list_view(&self, x: i32, y: i32, w: u32, h: u32) -> ListViewHandle {
        ListViewHandle::from_raw(crate::create_list_view(self.raw_id(), x, y, w, h))
    }

    /// Creates a scrollable viewport as a child of this panel.
    pub fn new_scroll_area(&self, x: i32, y: i32, w: u32, h: u32) -> ScrollAreaHandle {
        ScrollAreaHandle::from_raw(crate::create_scroll_area(self.raw_id(), x, y, w, h))
    }

    /// Creates a tool bar strip as a child of this panel.
    pub fn new_tool_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ToolBarHandle {
        ToolBarHandle::from_raw(crate::create_tool_bar(self.raw_id(), x, y, w, h))
    }

    /// Creates a status bar as a child of this panel.
    pub fn new_status_bar(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> StatusBarHandle {
        StatusBarHandle::from_raw(crate::create_status_bar(self.raw_id(), text, x, y, w, h))
    }

    /// Creates a nested panel as a child of this panel.
    ///
    /// The nested panel's geometry is recorded here as well, so its own
    /// [`PanelHandle::set_layout`] follows the box this panel gives it. See
    /// `apply_panel_geometry`, which re-runs a child panel's layout after the
    /// parent places it.
    pub fn new_panel(&self, x: i32, y: i32, w: u32, h: u32) -> PanelHandle {
        let panel = PanelHandle::from_raw(crate::create_panel(self.raw_id(), x, y, w, h));
        PANEL_STATES.with(|map| {
            map.borrow_mut().insert(panel.raw_id(), PanelState::with_geometry(x, y, w, h));
        });
        panel
    }

    /// Creates a frame (group box) as a child of this panel.
    ///
    /// A frame is the same widget as this panel with a different spelling; the
    /// returned handle's [`FrameHandle::as_panel`] reaches the full panel API.
    pub fn new_frame(&self, x: i32, y: i32, w: u32, h: u32) -> FrameHandle {
        let frame = FrameHandle::from_raw(crate::create_panel(self.raw_id(), x, y, w, h));
        PANEL_STATES.with(|map| {
            map.borrow_mut().insert(frame.raw_id(), PanelState::with_geometry(x, y, w, h));
        });
        frame
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
pub(crate) fn apply_panel_geometry(panel_id: ObjectId, rect: Rect) {
    // A panel layout is recursive: a nested panel is a child like any other, and the
    // box this pass gives it has to become the rect its own layout is asked about.
    // The depth bound is what keeps a corrupted parent/child chain (a panel that
    // ended up as its own descendant) from turning a layout pass into an unbounded
    // recursion; the same bound the widget runtime uses for its parent walks.
    const MAX_PANEL_DEPTH: u32 = 64;
    apply_panel_geometry_bounded(panel_id, rect, 0, MAX_PANEL_DEPTH);
}

fn apply_panel_geometry_bounded(panel_id: ObjectId, rect: Rect, depth: u32, max_depth: u32) {
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
    if depth >= max_depth {
        log::warn!(
            "panel {panel_id} exceeded the {max_depth}-level nesting bound; its layout was not \
             applied"
        );
        return;
    }

    let context = crate::layout::types::LayoutContext::default();
    let child_geometries = PANEL_LAYOUTS.with(|map| {
        let map = map.borrow();
        let Some(layout) = map.get(&panel_id) else {
            return Vec::new();
        };
        let mut geometries = Vec::new();
        layout.update_with_context(rect, &context, &mut |widget_id, geometry| {
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
        if widget_hosts_a_layout(widget_id) {
            apply_panel_geometry_bounded(widget_id, geometry, depth + 1, max_depth);
        } else if crate::layout::declarative::has_layout(widget_id) {
            // See `apply_window_layout`: a container loaded from a document can sit
            // inside a panel, and moving the panel must re-run the document's layout
            // for that container's children.
            crate::layout::declarative::apply_layout(widget_id, geometry);
        }
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
// FrameHandle – the panel-backed group box
// ═══════════════════════════════════════════════════════════════

/// # Frame-specific operations
///
/// `FrameHandle` and `PanelHandle` name the **same** underlying widget: both
/// `WindowHandle::new_frame` and `WindowHandle::new_panel` call `create_panel`, and a
/// frame's only difference is the spelling a caller reads. The layout machinery is
/// therefore shared rather than reimplemented, and [`FrameHandle::as_panel`] exposes
/// the complete panel API to a frame owner.
impl FrameHandle {
    /// Returns the panel view of this frame.
    ///
    /// Use this for child factories and any panel operation not repeated below. One
    /// implementation means a future panel feature cannot exist for panels and silently
    /// be missing for frames.
    pub fn as_panel(&self) -> PanelHandle {
        PanelHandle::from_raw(self.raw_id())
    }

    /// Set the frame's geometry and re-run its child layout.
    ///
    /// # Why this overrides the handle default
    ///
    /// [`WidgetHandle::set_geometry`] only moves the widget. A frame that hosts a layout
    /// has children, and moving the frame without re-running the layout would leave them
    /// at the old box — the same defect the panel's recursive layout exists to remove.
    pub fn set_geometry(&self, x: i32, y: i32, w: u32, h: u32) {
        self.as_panel().set_geometry(x, y, w, h);
    }

    /// Set the frame's layout manager; see [`PanelHandle::set_layout`].
    pub fn set_layout(&self, layout: Box<dyn crate::layout::Layout>) {
        self.as_panel().set_layout(layout);
    }

    /// Set the frame's title text.
    pub fn set_title(&self, title: &str) {
        self.as_panel().set_title(title);
    }

    /// Set the title alignment; see [`PanelHandle::set_alignment`].
    pub fn set_alignment(&self, alignment: crate::core::Alignment) -> bool {
        self.as_panel().set_alignment(alignment)
    }

    /// Read the title alignment; see [`PanelHandle::alignment`].
    pub fn alignment(&self) -> Option<crate::core::Alignment> {
        self.as_panel().alignment()
    }

    /// Make the frame's title checkable; see [`PanelHandle::set_checkable`].
    pub fn set_checkable(&self, checkable: bool) -> bool {
        self.as_panel().set_checkable(checkable)
    }

    /// Read whether the frame title is checkable; see [`PanelHandle::is_checkable`].
    pub fn is_checkable(&self) -> Option<bool> {
        self.as_panel().is_checkable()
    }

    /// Toggle the frame's checked state; see [`PanelHandle::toggle`].
    pub fn toggle(&self) -> bool {
        self.as_panel().toggle()
    }

    /// Creates a push button as a child of this frame.
    pub fn new_button(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> ButtonHandle {
        self.as_panel().new_button(text, x, y, w, h)
    }

    /// Creates a static text label as a child of this frame.
    pub fn new_label(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LabelHandle {
        self.as_panel().new_label(text, x, y, w, h)
    }

    /// Creates a check box as a child of this frame.
    pub fn new_checkbox(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> CheckBoxHandle {
        self.as_panel().new_checkbox(text, x, y, w, h)
    }

    /// Creates a single-line text entry as a child of this frame.
    pub fn new_line_edit(&self, text: &str, x: i32, y: i32, w: u32, h: u32) -> LineEditHandle {
        self.as_panel().new_line_edit(text, x, y, w, h)
    }

    /// Creates an empty drop-down list as a child of this frame.
    pub fn new_combo_box(&self, x: i32, y: i32, w: u32, h: u32) -> ComboBoxHandle {
        self.as_panel().new_combo_box(x, y, w, h)
    }

    /// Creates an empty single-selection list box as a child of this frame.
    pub fn new_list_box(&self, x: i32, y: i32, w: u32, h: u32) -> ListBoxHandle {
        self.as_panel().new_list_box(x, y, w, h)
    }

    /// Creates a horizontal slider as a child of this frame.
    pub fn new_slider(&self, x: i32, y: i32, w: u32, h: u32) -> SliderHandle {
        self.as_panel().new_slider(x, y, w, h)
    }

    /// Creates a progress bar as a child of this frame.
    pub fn new_progress_bar(&self, x: i32, y: i32, w: u32, h: u32) -> ProgressBarHandle {
        self.as_panel().new_progress_bar(x, y, w, h)
    }

    /// Creates a spin box as a child of this frame.
    pub fn new_spin_box(&self, x: i32, y: i32, w: u32, h: u32) -> SpinBoxHandle {
        self.as_panel().new_spin_box(x, y, w, h)
    }

    /// Creates a nested panel as a child of this frame.
    pub fn new_panel(&self, x: i32, y: i32, w: u32, h: u32) -> PanelHandle {
        self.as_panel().new_panel(x, y, w, h)
    }

    /// Creates a nested frame as a child of this frame.
    pub fn new_frame(&self, x: i32, y: i32, w: u32, h: u32) -> FrameHandle {
        self.as_panel().new_frame(x, y, w, h)
    }
}
