// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Key-event handles: text-entry controls whose state is driven by key input.

use super::*;
use crate::core::ObjectId;
use crate::widget::numeric::ordered_clamp_i32;
use alloc::rc::Rc;
use core::cell::RefCell;
use std::collections::HashMap;

impl_handle!(LineEditHandle, "Type-safe handle for a LineEdit widget.");
impl_handle!(SpinBoxHandle, "Type-safe handle for a SpinBox widget.");
impl_handle!(TextEditHandle, "Type-safe handle for a TextEdit (multi-line text) widget.");
// ═══════════════════════════════════════════════════════════════
// LineEditHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub(crate) struct LineEditState {
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
    pub(crate) static LINE_EDIT_STATES: RefCell<HashMap<ObjectId, LineEditState>> = RefCell::new(HashMap::new());
}

/// # Line-edit specific operations
impl LineEditHandle {
    /// Set the placeholder text shown when the field is empty.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_placeholder`] (`EM_SETCUEBANNER` on
    /// Windows, `set_placeholder_text` on GTK). AppKit's `NSTextView` has no
    /// placeholder concept, so on macOS the platform call reports `false` and only
    /// the mirror changes — a genuine per-OS difference, not a dropped write.
    pub fn set_placeholder(&self, text: &str) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).placeholder =
                text.to_owned();
        });
        write_control_property(
            self.raw_id(),
            "placeholder_text",
            crate::widget::capability::CapabilityValue::String(text.to_string()),
        );
    }

    /// Set whether the line-edit is read-only.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_read_only`], so the native field
    /// actually stops accepting input. On a backend whose text control is not an
    /// entry, the platform call reports `false` and only the mirror changes —
    /// that is a legitimate per-OS difference, not a dropped write.
    pub fn set_read_only(&self, read_only: bool) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).read_only =
                read_only;
        });
        write_control_property(
            self.raw_id(),
            "read_only",
            crate::widget::capability::CapabilityValue::Bool(read_only),
        );
    }

    /// Set the maximum number of characters allowed.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_max_length`]. Not every OS text
    /// control has a settable limit (AppKit's `NSTextField` does not), so the
    /// platform call may report `false`; the mirror still updates for the
    /// self-drawn path and for callers that enforce the limit themselves.
    pub fn set_max_length(&self, len: u32) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).max_length = len;
        });
        write_control_property(
            self.raw_id(),
            "max_length",
            crate::widget::capability::CapabilityValue::UInt(u64::from(len)),
        );
    }

    /// Clear the line-edit text.
    pub fn clear(&self) {
        crate::set_widget_text(self.raw_id(), "");
    }

    /// Set the echo mode (Normal / Password / NoEcho).
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_echo_mode`] (`EM_SETPASSWORDCHAR` on
    /// Windows, `set_visibility` on GTK). AppKit picks the text class instead, so
    /// macOS reports `false` here; and `NoEcho` has no equivalent on any toolkit,
    /// so it is refused rather than silently treated as `Password`.
    pub fn set_echo_mode(&self, mode: EchoMode) {
        LINE_EDIT_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).echo_mode = mode;
        });
        write_control_property(
            self.raw_id(),
            "echo_mode",
            crate::widget::capability::CapabilityValue::String(echo_mode_token(mode).to_string()),
        );
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
    /// Written through the control's `cursor_position` property as the end of the value, so the
    /// control's caret matches what the mirror reports. The contract names the caret and not an
    /// anchor pair, so there is no property that selects *and* keeps an anchor: the caret lands at
    /// the end rather than a range being highlighted.
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
        write_control_property(
            self.raw_id(),
            "cursor_position",
            crate::widget::capability::CapabilityValue::UInt(u64::from(u32::MAX)),
        );
    }

    /// Set the selection range (start..end).
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well
    /// through [`crate::platform::Platform::set_widget_selection`].
    pub fn set_selection(&self, start: u32, end: u32) {
        LINE_EDIT_STATES.with(|map| {
            let mut map = map.borrow_mut();
            let state = map.entry(self.raw_id()).or_default();
            state.select_all = false;
            state.selection_start = start;
            state.selection_end = end;
        });
        write_control_property(
            self.raw_id(),
            "cursor_position",
            crate::widget::capability::CapabilityValue::UInt(u64::from(end.max(start))),
        );
    }
}

// ═══════════════════════════════════════════════════════════════
// SpinBoxHandle – extended state
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub(crate) struct SpinBoxState {
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
    pub(crate) static SPINBOX_STATES: RefCell<HashMap<ObjectId, SpinBoxState>> = RefCell::new(HashMap::new());
}

/// Writes one of a spin box's affix properties through its published contract.
///
/// # Why the write goes through the capability rather than a cast to `SpinBox`
///
/// The handle holds an `ObjectId`, not a `SpinBox`, so reaching the control's own
/// `set_prefix` would need a downcast — and the capability layer is the one place that
/// already knows which controls publish `prefix`/`suffix` and what kind they are. Going
/// through it also means a control that does **not** publish the name refuses the write
/// instead of silently accepting it into a mirror nothing reads.
///
/// The result is logged rather than propagated: `SpinBoxHandle::set_prefix` returns `()`,
/// so there is no channel to report a refusal on, and dropping it silently is the defect
/// this helper exists to remove. A control that refuses (an unmounted id, or a backend
/// build where `SpinBox` is absent) says so at `warn`.
fn write_spinbox_affix(id: ObjectId, property: &str, value: &str) {
    let _ = write_control_property(
        id,
        property,
        crate::widget::capability::CapabilityValue::String(value.to_string()),
    );
}

/// # Spin-box specific operations
impl SpinBoxHandle {
    /// Set the spin-box value (clamped to range).
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well (the write goes through the control's own `value`).
    /// matches `value()`.
    pub fn set_value(&self, value: i32) {
        // See `SliderHandle::set_value`: one mirror access, not two.
        let clamped = SPINBOX_STATES.with(|map| {
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

    /// Return the current spin-box value.
    pub fn value(&self) -> i32 {
        SPINBOX_STATES.with(|map| map.borrow().get(&self.raw_id()).map(|s| s.value).unwrap_or(0))
    }

    /// Set the spin-box range (min/max). The current value is clamped into it.
    ///
    /// # Crossed bounds
    ///
    /// Same defect and same repair as [`SliderHandle::set_range`]: the pair is
    /// ordered before clamping, so a descending range (`set_range(100, 0)`) is a
    /// valid call instead of an `i32::clamp` panic.
    pub fn set_range(&self, min: i32, max: i32) {
        SPINBOX_STATES.with(|map| {
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
            note_property_pair("spin_box.set_range", low, high);
        };
    }

    /// Set the prefix text displayed before the value.
    ///
    /// # Why this now reaches the control
    ///
    /// It used to write only a `SpinBoxState` mirror and say the value "has **no effect on
    /// rendering** by the library (… the self-drawn path does not read this mirror)". That was
    /// false for this library's own `SpinBox`, which stores `prefix`/`suffix` fields, paints them
    /// either side of the value (`SpinBox::display_text`) and answers them through its property
    /// contract. So the handle held a value nothing read while the control kept its own, and the
    /// two entry points for one control disagreed silently — the *other* one
    /// (`write_property(id, "prefix", ..)`) did render.
    ///
    /// The mirror is kept (it is the value a caller can still read back and the one a host that
    /// formats the value itself consults) and the write is **also** routed through the widget's
    /// published property contract, so both spellings now agree.
    ///
    /// A backend with a native affix is still not involved: the property route reaches the
    /// library's control, not the OS control, which is the same division `set_title` uses.
    pub fn set_prefix(&self, prefix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).prefix =
                prefix.to_owned();
        });
        write_spinbox_affix(self.raw_id(), "prefix", prefix);
    }

    /// Set the suffix text displayed after the value.
    ///
    /// See [`SpinBoxHandle::set_prefix`]: the value reaches the control's own `suffix`
    /// property as well as the in-process mirror.
    pub fn set_suffix(&self, suffix: &str) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).suffix =
                suffix.to_owned();
        });
        write_spinbox_affix(self.raw_id(), "suffix", suffix);
    }

    /// Set the spin-box step increment.
    ///
    /// Written to the control through its property contract, and recorded in the
    /// in-process mirror as well (the write goes through the control's own `single_step`).
    pub fn set_step(&self, step: i32) {
        SPINBOX_STATES.with(|map| {
            map.borrow_mut().entry(self.raw_id()).or_insert_with(Default::default).step = step;
        });
        write_control_property(self.raw_id(), "single_step", step_carrier(f64::from(step)));
    }
}
