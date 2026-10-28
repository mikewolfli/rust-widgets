// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Progress dialog widget.
use crate::compat::{Rc, RefCell};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_i64, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Progress dialog widget.
/// Progress dialog widget.
///
/// A modal-style dialog that reports the progress of a long operation on an
/// integer range and offers a single cancel affordance. It owns no timer: the
/// caller drives it by calling [`ProgressDialog::set_value`], which is the only
/// way the displayed progress changes.
///
pub struct ProgressDialog {
    base: BaseWidget,
    title: String,
    label_text: String,
    value: i32,
    minimum: i32,
    maximum: i32,
    cancel_button_text: String,
    was_canceled: bool,
    auto_close: bool,
    auto_reset: bool,
    modal: bool,
    /// Whether the Cancel button is painted (and hit-tested). Defaults to `true`.
    show_cancel_button: bool,
    /// Whether the progress bar is painted. Defaults to `true`.
    show_bar: bool,
    /// The delay below which the dialog should not be shown, in milliseconds.
    ///
    /// This control owns no timer: the host decides when to show it and can consult
    /// [`ProgressDialog::should_show_after`] with the operation's own elapsed time. That is
    /// the decision the delay exists for, kept as a pure function rather than as a clock
    /// this widget would have to guess at.
    minimum_duration_ms: u64,
    /// Whether the painted fill eases toward a new value instead of jumping.
    animated: bool,
    /// The fraction the bar currently paints, driven by [`animated`](Self::animated).
    displayed: PropertyDriver,
    /// Whether the fill reads the theme's action colour instead of the success token.
    accent_fill: bool,
    /// Whether the bar is in indeterminate (busy) mode; `set_value` is ignored while set.
    busy: bool,
    /// The busy slider's phase, advanced by [`Widget::tick`].
    busy_phase: f32,
    /// Cached action labels, keyed by the button caption (G-P09).
    label_cache: RefCell<Option<(String, Rc<Vec<String>>)>>,
    /// The Cancel button's rect from the last paint, with the frame it was derived from.
    ///
    /// The press path reads this instead of building a paint backend to re-measure the
    /// caption, which is what `cancel_button_rect` used to do on every event (G-P16).
    cancel_rect_cache: core::cell::Cell<Option<(Rect, Rect)>>,
    /// Signal emitted when the user cancels, either by pressing Escape while
    /// the dialog is enabled or by calling [`ProgressDialog::cancel`] directly.
    ///
    /// Carries no payload; the reason is not distinguishible from the signal
    /// alone. Connect through [`Widget::connection_scope`] so the slot is
    /// disconnected when the dialog is dropped.
    pub canceled: GenericSignal,
    /// Emitted with the new progress value whenever [`ProgressDialog::set_value`]
    /// actually moves it.
    pub value_changed: Signal1<i32>,
    /// Emitted with the new label whenever [`ProgressDialog::set_label_text`] changes it.
    pub label_changed: Signal1<String>,
    /// Emitted once when the value first reaches the maximum.
    ///
    /// Whether the dialog then hides is [`ProgressDialog::auto_close`]'s decision; this
    /// signal reports completion itself, which is what lets a host chain the next step or
    /// record the result without polling [`ProgressDialog::value`].
    pub finished: GenericSignal,
}
impl ProgressDialog {
    /// Creates a dialog with an empty title and label, a range of `0..=100`, a
    /// value of `0`, the translated "Cancel" button text, and `auto_close`,
    /// `auto_reset`, and `modal` all `true`.
    ///
    /// `geometry` is in parent-relative logical pixels; the default size hint
    /// is 350x120, which the caller is free to override.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressDialog, geometry, "ProgressDialog"),
            title: String::new(),
            label_text: String::new(),
            value: 0,
            minimum: 0,
            maximum: 100,
            cancel_button_text: super::localized_label("common.button.cancel", "Cancel"),
            was_canceled: false,
            auto_close: true,
            auto_reset: true,
            modal: true,
            show_cancel_button: true,
            show_bar: true,
            minimum_duration_ms: 0,
            animated: false,
            displayed: PropertyDriver::at(0.0, MotionSlot::Normal),
            accent_fill: false,
            busy: false,
            busy_phase: 0.0,
            label_cache: RefCell::new(None),
            cancel_rect_cache: core::cell::Cell::new(None),
            canceled: GenericSignal::new(),
            value_changed: Signal1::new(),
            label_changed: Signal1::new(),
            finished: GenericSignal::new(),
        }
    }
    /// Returns the dialog title, drawn in the title bar. Empty by default.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Returns the label drawn above the progress bar. Empty by default.
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    /// Returns the current value, always within `minimum() ..= maximum()`.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Returns the lower bound of the progress range. Defaults to `0`.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Returns the upper bound of the progress range. Defaults to `100`; this
    /// is also the value at which `auto_close` hides the dialog.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Returns `true` once the user (or [`ProgressDialog::cancel`]) has
    /// cancelled. Cleared again by [`ProgressDialog::reset`].
    pub fn was_canceled(&self) -> bool {
        self.was_canceled
    }
    /// Returns whether reaching the maximum hides the dialog automatically.
    /// Defaults to `true`.
    pub fn auto_close(&self) -> bool {
        self.auto_close
    }
    /// Returns whether cancellation resets the value back to the minimum.
    /// Defaults to `true`.
    pub fn auto_reset(&self) -> bool {
        self.auto_reset
    }
    /// Returns the cancel button caption. Defaults to the translated
    /// `common.button.cancel` string.
    pub fn cancel_button_text(&self) -> &str {
        &self.cancel_button_text
    }
    /// Sets the title bar text and requests a redraw.
    pub fn set_title(&mut self, t: impl Into<String>) {
        let t = t.into();
        if self.title == t {
            return;
        }
        self.title = t;
        self.base.request_redraw();
    }
    /// Sets the label shown above the progress bar and requests a redraw.
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        let t = t.into();
        if self.label_text == t {
            return;
        }
        self.label_text = t;
        self.label_changed.emit(self.label_text.clone());
        self.base.request_redraw();
    }
    /// Sets the lower bound of the progress range.
    ///
    /// The new bound is **not** applied to the current value, so the value can
    /// temporarily sit outside the range until the next
    /// [`ProgressDialog::set_value`] call clamps it. A `min` above `max` makes
    /// [`ProgressDialog::progress_fraction`] report full progress.
    pub fn set_minimum(&mut self, min: i32) {
        if self.minimum == min {
            return;
        }
        self.minimum = min;
        self.sync_displayed();
        self.base.request_redraw();
    }
    /// Sets the upper bound of the progress range. As with
    /// [`ProgressDialog::set_minimum`], the current value is not re-clamped
    /// until the next `set_value`.
    pub fn set_maximum(&mut self, max: i32) {
        if self.maximum == max {
            return;
        }
        self.maximum = max;
        self.sync_displayed();
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        if self.minimum == min && self.maximum == max {
            return;
        }
        self.minimum = min;
        self.maximum = max;
        self.sync_displayed();
        self.base.request_redraw();
    }
    /// Toggles automatic hiding at completion; requests a redraw. See
    /// [`ProgressDialog::auto_close`].
    pub fn set_auto_close(&mut self, v: bool) {
        if self.auto_close == v {
            return;
        }
        self.auto_close = v;
        self.base.request_redraw();
    }
    /// Toggles automatic reset on restart; requests a redraw. See
    /// [`ProgressDialog::auto_reset`].
    ///
    /// The flag is consulted by [`ProgressDialog::cancel`]: with it on (the default) a
    /// cancellation returns the value to the range's minimum, so a dialog that is shown
    /// again starts from the beginning. With it off, the value the user cancelled at is
    /// preserved. Completion keeps the value at the maximum in either mode, which is the
    /// contract [`ProgressDialog::set_value`] documents.
    pub fn set_auto_reset(&mut self, v: bool) {
        if self.auto_reset == v {
            return;
        }
        self.auto_reset = v;
        self.base.request_redraw();
    }
    /// Returns whether the Cancel button is painted. Defaults to `true`.
    pub fn is_cancel_button_visible(&self) -> bool {
        self.show_cancel_button
    }
    /// Shows or hides the Cancel button.
    ///
    /// Hiding it also removes its hit target, so a click in that band cannot cancel an
    /// operation the caller chose to make non-cancellable.
    pub fn set_cancel_button_visible(&mut self, visible: bool) {
        if self.show_cancel_button == visible {
            return;
        }
        self.show_cancel_button = visible;
        self.base.request_redraw();
    }
    /// Returns whether the progress bar is painted. Defaults to `true`.
    pub fn is_bar_visible(&self) -> bool {
        self.show_bar
    }
    /// Shows or hides the progress bar.
    ///
    /// A dialog without a bar is the "busy" form: the label and the cancel affordance
    /// remain, and the value still advances so `progress_fraction` keeps reporting.
    pub fn set_bar_visible(&mut self, visible: bool) {
        if self.show_bar == visible {
            return;
        }
        self.show_bar = visible;
        self.base.request_redraw();
    }
    /// Returns the minimum duration below which the dialog should stay hidden, in ms.
    pub fn minimum_duration(&self) -> u64 {
        self.minimum_duration_ms
    }
    /// Sets the minimum duration below which the dialog should stay hidden. See
    /// [`ProgressDialog::should_show_after`].
    pub fn set_minimum_duration(&mut self, ms: u64) {
        if self.minimum_duration_ms == ms {
            return;
        }
        self.minimum_duration_ms = ms;
        self.base.request_redraw();
    }
    /// Whether an operation that has run for `elapsed_ms` merits showing the dialog.
    ///
    /// The answer is `elapsed_ms >= minimum_duration()`, so a zero duration (the default)
    /// always shows. A host that shows the dialog after this returns `true` avoids a flash
    /// for an operation that completed almost immediately.
    pub fn should_show_after(&self, elapsed_ms: u64) -> bool {
        elapsed_ms >= self.minimum_duration_ms
    }
    /// Shows the dialog immediately, bypassing [`minimum_duration`](Self::minimum_duration).
    ///
    /// This is the explicit override for the case the delay exists to suppress: the host
    /// already knows the operation is long-running.
    pub fn force_show(&mut self) {
        self.show();
        self.base.request_redraw();
    }
    /// Returns whether the bar's fill eases toward a new value.
    pub fn is_animated(&self) -> bool {
        self.animated
    }
    /// Enables or disables the bar's easing and requests a redraw.
    ///
    /// Off by default, so `set_value` paints the exact fraction it always did. Turning it
    /// on snaps the driver to the current fraction, so enabling it mid-operation cannot
    /// replay the bar from zero.
    pub fn set_animated(&mut self, animated: bool) {
        if self.animated == animated {
            return;
        }
        self.animated = animated;
        let fraction = self.progress_fraction();
        self.displayed.jump_to(fraction);
        self.base.request_redraw();
    }
    /// Returns whether the fill uses the theme's action colour rather than the success token.
    pub fn uses_accent_fill(&self) -> bool {
        self.accent_fill
    }
    /// Selects the action colour for the bar's fill instead of the success token.
    ///
    /// Off by default: the success colour is the progress dialog's documented semantic
    /// ("the operation completed"). An accent fill is the alternative convention where the
    /// bar is an affordance rather than a state, and this is the switch for it.
    pub fn set_accent_fill(&mut self, accent: bool) {
        if self.accent_fill == accent {
            return;
        }
        self.accent_fill = accent;
        self.base.request_redraw();
    }
    /// Sets the caption of the cancel button and requests a redraw.
    pub fn set_cancel_button_text(&mut self, t: impl Into<String>) {
        let t = t.into();
        if self.cancel_button_text == t {
            return;
        }
        self.cancel_button_text = t;
        // A wider caption changes the button's measured width, so the cached press box must
        // not survive the caption change.
        self.cancel_rect_cache.set(None);
        self.base.request_redraw();
    }
    /// Returns whether the dialog is modal (blocks interaction with the widgets
    /// behind it). Defaults to `true`.
    ///
    /// Records the intent; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`).
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality intent. See [`ProgressDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        self.base.request_redraw();
    }
    /// Whether the bar is in indeterminate (busy) mode.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Switches the bar between determinate and indeterminate mode (DLG-15).
    ///
    /// While busy, the bar paints a back-and-forth slider instead of a fraction, and
    /// [`Self::set_value`] is ignored; the stored value is kept so leaving busy mode shows
    /// the previous determinate state rather than a reset.
    pub fn set_busy(&mut self, busy: bool) {
        if self.busy == busy {
            return;
        }
        self.busy = busy;
        self.busy_phase = 0.0;
        self.base.request_redraw();
    }

    /// Moves the progress value, clamped into `minimum() ..= maximum()`.
    ///
    /// Side effects: when `auto_close` is set and the clamped value reaches the
    /// maximum, the dialog hides itself. Hiding does not emit `canceled`. The
    /// value is set before hiding, so [`ProgressDialog::value`] still reports
    /// the maximum afterwards. A redraw is requested in all cases.
    pub fn set_value(&mut self, value: i32) {
        // A busy bar states that no value is known; accepting one would make the label
        // and the slider disagree about which story the dialog is telling (DLG-15).
        if self.busy {
            return;
        }
        let value = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.value == value {
            return;
        }
        let previous = self.value;
        self.value = value;
        self.sync_displayed();
        self.value_changed.emit(self.value);
        // Completion is a transition, not a state: reporting it on every write at the
        // maximum would make a host's "advance to the next step" handler fire repeatedly.
        if self.value >= self.maximum && previous < self.maximum {
            self.finished.emit();
        }
        if self.auto_close && self.value >= self.maximum {
            self.hide();
        }
        self.base.request_redraw();
    }

    /// Aims the displayed fraction at the model's current value.
    ///
    /// Easing is the only thing that makes this different from a jump: with animation off
    /// the driver is placed exactly on the fraction the draw would have computed anyway.
    fn sync_displayed(&mut self) {
        let fraction = self.progress_fraction();
        if self.animated {
            self.displayed.set_target(fraction);
        } else {
            self.displayed.jump_to(fraction);
        }
    }
    /// Resets the value to the minimum and clears the cancelled flag.
    ///
    /// Does not show the dialog: combine with [`Widget::show`] when reusing a
    /// hidden dialog. The `auto_reset` flag is not consulted here; it is consulted by
    /// [`ProgressDialog::cancel`] and this method is the explicit unconditional reset.
    pub fn reset(&mut self) {
        self.value = self.minimum;
        self.was_canceled = false;
        let fraction = self.progress_fraction();
        self.displayed.jump_to(fraction);
        self.base.request_redraw();
    }
    /// Marks the dialog as cancelled, emits `canceled`, and hides the dialog.
    ///
    /// The signal is emitted **before** the dialog is hidden, so a slot that
    /// reads geometry still sees the visible state. When [`auto_reset`](Self::auto_reset)
    /// is on (the default) the value returns to the minimum, so a dialog that is shown
    /// again for a different operation does not start at the cancelled position. Calling
    /// `cancel` twice emits the signal twice.
    pub fn cancel(&mut self) {
        self.was_canceled = true;
        self.canceled.emit();
        if self.auto_reset {
            self.value = self.minimum;
            let fraction = self.progress_fraction();
            self.displayed.jump_to(fraction);
            self.base.request_redraw();
        }
        self.hide();
    }

    /// Compatibility spelling of [`Self::canceled`] (W-19): the same emitted signal.
    pub fn cancelled(&self) -> &GenericSignal {
        &self.canceled
    }

    /// Returns progress as a fraction in `0.0..=1.0`, where `0.0` is the
    /// minimum and `1.0` is the maximum.
    ///
    /// An empty or inverted range (`maximum <= minimum`) reports `1.0`
    /// unconditionally, i.e. a single-valued range is treated as complete
    /// rather than as division by zero.
    pub fn progress_fraction(&self) -> f32 {
        let range = self.maximum - self.minimum;
        if range <= 0 {
            return 1.0;
        }
        (self.value - self.minimum) as f32 / range as f32
    }
}
impl Widget for ProgressDialog {
    /// Access to the shared base-widget state; all default trait behaviour
    /// delegates through this.
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    /// Mutable access to the shared base-widget state.
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 120)
    }

    /// Advances the bar's easing, returning whether another frame is owed.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if self.busy {
            // A busy slider moves on its own clock; it is the only thing that owes frames.
            self.busy_phase = (self.busy_phase + delta_ms as f32 * 0.0012) % 1.0;
            self.base.request_redraw();
        }
        self.displayed.tick(delta_ms) || self.busy
    }

    /// Whether the bar's fill is mid-transition.
    fn is_animating(&self) -> bool {
        self.displayed.is_moving() || self.busy
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `ProgressDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "canceled" => Some(EventSignalRef::unit("canceled", &self.canceled)),
            // W-19 compatibility spelling of the same event.
            "finished" => Some(EventSignalRef::unit("finished", &self.finished)),
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::Int(*value as i64)
                }))
            }
            "label_changed" => {
                Some(EventSignalRef::mapped("label_changed", &self.label_changed, |value| {
                    CapabilityValue::String(value.clone())
                }))
            }
            _ => None,
        }
    }
}

/// `ProgressDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind.
impl WidgetProperties for ProgressDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "label_text" => Ok(CapabilityValue::String(self.label_text().to_string())),
            "value" => Ok(CapabilityValue::Int(i64::from(self.value()))),
            "minimum" => Ok(CapabilityValue::Int(i64::from(self.minimum()))),
            "maximum" => Ok(CapabilityValue::Int(i64::from(self.maximum()))),
            "auto_reset" => Ok(CapabilityValue::Bool(self.auto_reset())),
            "show_cancel_button" => Ok(CapabilityValue::Bool(self.is_cancel_button_visible())),
            "show_bar" => Ok(CapabilityValue::Bool(self.is_bar_visible())),
            "minimum_duration" => Ok(CapabilityValue::UInt(self.minimum_duration())),
            "animated" => Ok(CapabilityValue::Bool(self.is_animated())),
            "accent_fill" => Ok(CapabilityValue::Bool(self.uses_accent_fill())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "label_text" => {
                self.set_label_text(expect_string(value)?);
                Ok(())
            }
            // `value` is the payload `set_value` carries, so it must be writable through the
            // property route the command's own documentation points at — publishing `set_value`
            // while leaving `value` unwritable is the schema/contract disagreement
            // `capability::properties_tests` catches.
            "value" => {
                self.set_value(
                    i32::try_from(expect_i64(value)?)
                        .map_err(|_| CapabilityAccessError::OutOfRange)?,
                );
                Ok(())
            }
            // `minimum`/`maximum` are the pair `set_range` carries; each is also writable on its
            // own, which is what makes `set_minimum`/`set_maximum` reachable.
            "minimum" => {
                self.set_minimum(
                    i32::try_from(expect_i64(value)?)
                        .map_err(|_| CapabilityAccessError::OutOfRange)?,
                );
                Ok(())
            }
            "maximum" => {
                self.set_maximum(
                    i32::try_from(expect_i64(value)?)
                        .map_err(|_| CapabilityAccessError::OutOfRange)?,
                );
                Ok(())
            }
            "auto_reset" => {
                self.set_auto_reset(expect_bool(value)?);
                Ok(())
            }
            "show_cancel_button" => {
                self.set_cancel_button_visible(expect_bool(value)?);
                Ok(())
            }
            "show_bar" => {
                self.set_bar_visible(expect_bool(value)?);
                Ok(())
            }
            "minimum_duration" => {
                self.set_minimum_duration(u64::from(expect_u32(value)?));
                Ok(())
            }
            "animated" => {
                self.set_animated(expect_bool(value)?);
                Ok(())
            }
            "accent_fill" => {
                self.set_accent_fill(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "label_text",
            "value",
            "minimum",
            "maximum",
            "auto_reset",
            "show_cancel_button",
            "show_bar",
            "minimum_duration",
            "animated",
            "accent_fill",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `progress_dialog` publishes.
    ///
    /// All four names carry a value — `set_value` and `set_range` the progress,
    /// `set_title` and `set_label_text` the text — so a payload-less invocation
    /// is reported as needing one rather than being called unknown. Cancelling
    /// is not published as a command, so it has no entry here.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_range" | "set_title" | "set_label_text" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for ProgressDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.button_at(*pos).is_some() {
                    self.cancel();
                }
            }
            Event::KeyPress { key, .. } if *key == key_codes::ENTER => self.cancel(),
            Event::KeyPress { key, .. } if *key == key_codes::ESCAPE => self.cancel(),
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl ProgressDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 panel whose label, bar and button were then pinned to
    /// literals (`48`, `62`, `rect.height - 40`) written for a taller default size — so the
    /// bar sat in the top third and the cancel button floated well below it.
    /// [`ControlMetrics::painted_box`] caps each axis at the dialog's own intrinsic size and
    /// centres what is left; every band below is derived from this one rect.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }

    /// The action labels this dialog paints, in row order.
    ///
    /// A progress dialog has one action, so the row is the caller's Cancel caption alone.
    /// `draw` and [`ProgressDialog::button_at`] both call this.
    fn action_labels(&self) -> Rc<Vec<String>> {
        if let Some((text, labels)) = self.label_cache.borrow().as_ref() {
            if *text == self.cancel_button_text {
                return Rc::clone(labels);
            }
        }
        let labels = Rc::new(vec![self.cancel_button_text.clone()]);
        *self.label_cache.borrow_mut() =
            Some((self.cancel_button_text.clone(), Rc::clone(&labels)));
        labels
    }

    /// The rect the single Cancel button occupies.
    ///
    /// # Why this is a function of `rect`, `context` and `labels`
    ///
    /// A lone action is centred in its band rather than trailing-anchored, so its x is *not*
    /// whatever `action_row_geometry` returns — that helper owns the size and the row, and the
    /// centring is added here. Both the paint path and the hit test call this one function, so
    /// the extra step cannot exist in one and be forgotten in the other.
    fn cancel_button_rect_for(
        &self,
        rect: Rect,
        context: &RenderContext,
        labels: &[String],
    ) -> Rect {
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT),
            body.y,
        );
        let row = super::message_box::action_row_geometry(
            context,
            effective_font(self.base.style()),
            labels,
            button_band,
            false,
        );
        let btn_w = row.buttons[0].width;
        let btn_x = (rect.x + rect.width as i32 / 2 - btn_w as i32 / 2).max(rect.x);
        Rect::new(btn_x, button_band.y, btn_w, button_band.height.max(1))
    }

    /// The button rect from the last paint, or a one-time measurement before the first frame.
    fn cancel_button_rect(&self) -> Rect {
        let frame = self.frame_rect();
        if let Some((cached_frame, cached)) = self.cancel_rect_cache.get() {
            if cached_frame == frame {
                return cached;
            }
        }
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let labels = self.action_labels();
        let rect = self.cancel_button_rect_for(frame, &context, &labels);
        self.cancel_rect_cache.set(Some((frame, rect)));
        rect
    }

    /// The index of the action button under `pos`, or `None`.
    fn button_at(&self, pos: Point) -> Option<usize> {
        if !self.show_cancel_button {
            return None;
        }
        if self.cancel_button_rect().contains_point(pos) {
            Some(0)
        } else {
            None
        }
    }
}

impl Draw for ProgressDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A modal dialog dims the page behind it: that dimming is the whole visual difference
        // between "this dialog is up and the page is inert" and "a panel was drawn on the page".
        // The `modal` flag was recorded and enforced by the modal stack, and nothing painted it.
        if self.modal {
            super::draw_modal_scrim(context, self.id(), self.geometry(), self.base.parent());
        }
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The dialog surface already read the style,
        // but the title bar, the progress track and the button were literals, so a light/dark
        // switch left them unchanged and the census reported the control as theme-blind.
        //
        // The theme reads are the cached shared snapshots, so no manager lock is held across
        // the draw (the mutex is not re-entrant).
        let style = self.base.style();
        // The **effective font** — the resolved theme/caller font — for the title, label, the
        // percentage readout and the buttons (D09-STYLE-01).
        let font = effective_font(style);
        // One palette-snapshot read supplies the window fill, the fallback ink/border and the
        // accent fill; the previous path resolved a widget style and then took the manager
        // lock again (G-P13).
        let palette = crate::style::palette_shared();
        let (window_fill, theme_ink, theme_border) = {
            match palette.as_ref() {
                Some(colors) => (colors.background, Some(colors.foreground), Some(colors.outline)),
                None => (Color::WHITE, None, None),
            }
        };
        let ink = style.text_color.or(theme_ink).unwrap_or(Color::rgb(40, 40, 40));
        let surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border =
            crate::widget::draw::border_ink(style.border_color, theme_border, surface, ink);
        // The title bar, the progress track and the button are distinct bands on the panel,
        // derived from it so they stay one visible step apart in either appearance.
        let title_bar = surface.blend(&ink, 0.08);
        let track = surface.blend(&ink, 0.12);
        let button_fill = surface.blend(&ink, 0.12);

        // The filled portion of the bar encodes "complete" rather than a surface
        // appearance, so it reads the theme's semantic `success` token instead of the
        // literal green it used to carry. `semantic_color` takes its own lock and returns an
        // owned `Color`, so no guard outlives the call. A caller can opt into the action
        // colour instead, for the convention where the bar is an affordance.
        let progress_fill = if self.accent_fill {
            palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::rgb(6, 176, 37))
        } else {
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .unwrap_or(Color::rgb(6, 176, 37))
        };

        // Rounded by [`dimensions::DIALOG_RADIUS`]; the radius is clamped to the frame so a
        // box smaller than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.face_with_gradient_corners(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().resolved_corner_radii().to_array(),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }
        // Every label is fitted to the band it belongs to and centred on that band through
        // the shared primitive: the title to the title bar, the label to its own row, and
        // the button caption to the button. None of them was bounded before, so a long
        // caption ran past the frame — the raster backends clip that away and the SVG
        // snapshot showed it as drawing outside the picture.
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        // Guarded on the text being non-empty so a titleless dialog emits no `<text …></text>`.
        if !self.title.is_empty() {
            let title_font = font;
            let title_line = context.text_line(title_bar_band, title_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // The body is everything the title bar leaves, and the button is the bottom band of
        // it. The label and the bar then take the space above the button, so the three rows
        // stack inside the frame instead of each being placed from the frame's top edge plus
        // a literal.
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let content =
            ControlMetrics::content_above_bottom_band(body, dimensions::DIALOG_BUTTON_HEIGHT);
        let label_font = font;
        let label_line_h = context.measure_text("M", label_font).height.max(1);
        // Label: the top row of the content area, bounded by it rather than by `rect.y + 48`.
        // Guarded on the text being non-empty so a labelless dialog emits no empty `<text>`.
        if !self.label_text.is_empty() {
            let label_band = ControlMetrics::top_band(content, label_line_h);
            let label_line = context.text_line(label_band, label_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    content.x + 10,
                    label_line.y,
                    content.width.saturating_sub(20),
                    label_line.height.max(1),
                ),
                &self.label_text,
                label_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // Progress bar: the row below the label, centred in whatever the label row left.
        // The track is chrome and follows the theme; the filled portion is the semantic
        // `success` colour resolved above. The bar is drawn only when that row is non-empty:
        // a zero-height band is an element the SVG backend emits and the rasteriser skips.
        let bar_row = ControlMetrics::content_below_top_band(content, label_line_h);
        let bar_h = 20u32.min(bar_row.height);
        if self.show_bar && bar_row.height > 0 && bar_row.width > 20 {
            let bar_band = ControlMetrics::centered_band(
                Rect::new(
                    bar_row.x + 10,
                    bar_row.y,
                    bar_row.width.saturating_sub(20),
                    bar_row.height,
                ),
                bar_h,
            );
            context.fill_rect(bar_band, track);
            context.draw_rect(bar_band, border);
            // The painted fraction is the driver's when easing is on, so a step in `value`
            // is a smooth move rather than a jump. At rest the driver holds the fraction
            // itself, so the static frame is unchanged.
            let fraction =
                if self.animated { self.displayed.value() } else { self.progress_fraction() };
            if self.busy {
                // Indeterminate mode: a short slider travels back and forth across the track.
                // Its position is a triangle wave of the phase the tick advances, so the
                // motion is continuous and bounded by the bar itself (DLG-15).
                let slider_w = (bar_band.width / 3).max(8);
                let travel = bar_band.width.saturating_sub(slider_w) as f32;
                let phase = self.busy_phase % 1.0;
                let eased = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
                let slider_x = bar_band.x + (travel * eased) as i32;
                context.fill_rect(
                    Rect::new(slider_x, bar_band.y, slider_w, bar_band.height),
                    progress_fill,
                );
            } else {
                let fill_w = (bar_band.width as f32 * fraction) as i32;
                if fill_w > 0 {
                    context.fill_rect(
                        Rect::new(bar_band.x, bar_band.y, fill_w as u32, bar_band.height),
                        progress_fill,
                    );
                }
                // Percentage text. Centred on the bar itself through `draw_text_line`, which
                // re-derives the line box from the band and bounds the fit to it — the old
                // `bar_x + bar_w/2` origin made the caption occupy only its right half and, at
                // 100%, run up to `bar_w/2 + 42` past the bar. The text reads the same fraction
                // the fill does, so an eased bar and its number cannot disagree (DLG-16).
                let pct = (fraction * 100.0) as i32;
                context.draw_text_line_cow(
                    bar_band,
                    &format!("{pct}%"),
                    font,
                    if track.is_dark() { Color::WHITE } else { ink },
                    HorizontalAlignment::Center,
                );
            }
        }
        // Cancel button: the frame's bottom band, and its width from the shared
        // [`action_row_geometry`] derivation — the same 80 px an OK button gets, so a progress
        // dialog's single button is the same object as the action buttons in the five dialogs
        // that draw a pair. It is centred rather than right-anchored because a lone dismiss
        // action is the panel's own call to action, which is the one place the row is not
        // trailing-anchored; the helper still owns the *size*, which is the part that was
        // duplicated.
        let labels = self.action_labels();
        if self.show_cancel_button {
            let btn_rect = self.cancel_button_rect_for(rect, context, &labels);
            // Cache the painted box so the press path reads it rather than re-measuring (G-P16).
            self.cancel_rect_cache.set(Some((rect, btn_rect)));
            context.fill_rect(btn_rect, button_fill);
            context.draw_rect(btn_rect, border);
            context.draw_text_line_cow(
                btn_rect,
                &labels[0],
                font,
                ink,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn set_value_clamps_and_auto_closes_on_max() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.set_range(10, 20);

        dialog.set_value(5);
        assert_eq!(dialog.value(), 10);

        dialog.show();
        dialog.set_value(99);
        assert_eq!(dialog.value(), 20);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn clicking_cancel_emits_canceled_and_hides() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        let canceled = Arc::new(Mutex::new(0usize));
        let canceled_clone = Arc::clone(&canceled);

        dialog.canceled.connect(move || {
            if let Ok(mut n) = canceled_clone.lock() {
                *n += 1;
            }
        });

        dialog.show();
        let cancel = dialog.cancel_button_rect().center();
        dialog.handle_event(&Event::mouse_press(cancel.x, cancel.y, 1));
        assert!(dialog.was_canceled());
        assert_eq!(*canceled.lock().expect("canceled lock"), 1);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn enter_cancels_because_cancel_is_the_only_action() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.show();
        dialog.handle_event(&Event::key_press(13, 0));
        assert!(dialog.was_canceled());
        assert!(!dialog.is_visible());
    }

    #[test]
    fn escape_key_cancels_and_emits_signal() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        let canceled = Arc::new(Mutex::new(0usize));
        let canceled_clone = Arc::clone(&canceled);

        dialog.canceled.connect(move || {
            if let Ok(mut n) = canceled_clone.lock() {
                *n += 1;
            }
        });

        dialog.show();
        dialog.handle_event(&Event::key_press(27, 0));
        assert!(dialog.was_canceled());
        assert_eq!(*canceled.lock().expect("canceled lock"), 1);
        assert!(!dialog.is_visible());
    }

    /// `auto_reset` has a consumer: cancellation returns the value to the minimum.
    #[test]
    fn auto_reset_clears_the_value_on_cancel() {
        let mut resetting = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        resetting.set_range(10, 90);
        resetting.set_value(50);
        resetting.cancel();
        assert_eq!(resetting.value(), 10, "auto_reset returns to the minimum");

        let mut keeping = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        keeping.set_auto_reset(false);
        keeping.set_range(10, 90);
        keeping.set_value(50);
        keeping.cancel();
        assert_eq!(keeping.value(), 50, "with auto_reset off the position is kept");
    }

    /// Hiding the Cancel button removes its hit target, not just its paint.
    #[test]
    fn the_cancel_button_can_be_hidden() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.show();
        dialog.set_cancel_button_visible(false);
        assert!(!dialog.is_cancel_button_visible());
        let cancel = dialog.cancel_button_rect().center();
        dialog.handle_event(&Event::mouse_press(cancel.x, cancel.y, 1));
        assert!(!dialog.was_canceled(), "a hidden button must not be clickable");
        assert!(dialog.is_visible());
    }

    /// The bar can be hidden without losing the value bookkeeping.
    #[test]
    fn the_bar_can_be_hidden() {
        let _guard = crate::style::theme_test_guard();
        let mut with_bar = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        with_bar.set_value(40);
        let painted = crate::widget::svg::render_to_svg(&mut with_bar);

        with_bar.set_bar_visible(false);
        assert!(!with_bar.is_bar_visible());
        let busy = crate::widget::svg::render_to_svg(&mut with_bar);
        assert_ne!(painted, busy, "hiding the bar must change the picture");
        assert_eq!(with_bar.value(), 40, "the value is still tracked");
    }

    /// The minimum duration is a decision function; `force_show` is the explicit override.
    #[test]
    fn the_minimum_duration_gates_showing() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        assert!(dialog.should_show_after(0), "a zero delay always shows");

        dialog.set_minimum_duration(500);
        assert!(!dialog.should_show_after(499));
        assert!(dialog.should_show_after(500));

        dialog.hide();
        assert!(!dialog.is_visible());
        dialog.force_show();
        assert!(dialog.is_visible(), "force_show bypasses the delay");
    }

    /// The bar's easing is opt-in and passes through interior fractions.
    #[test]
    fn the_bar_easing_is_opt_in() {
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.set_value(50);
        assert_eq!(dialog.displayed.value(), 0.5, "a static bar holds the exact fraction");
        assert!(!dialog.is_animating());

        dialog.set_animated(true);
        assert_eq!(dialog.displayed.value(), 0.5, "enabling snaps to the current fraction");
        dialog.set_value(90);
        assert!(dialog.is_animating());
        assert!(dialog.tick(16));
        let mid = dialog.displayed.value();
        assert!(mid > 0.5 && mid < 0.9, "the fill must pass through an interior value, got {mid}");
        while dialog.tick(1000) {}
        assert!((dialog.displayed.value() - 0.9).abs() < 0.001);
        assert!(!dialog.is_animating());
    }

    /// The accent fill is opt-in and paints the theme's action colour instead of the
    /// success token.
    #[test]
    fn the_accent_fill_is_opt_in() {
        let _guard = crate::style::theme_test_guard();
        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        dialog.set_value(50);
        let semantic = crate::widget::svg::render_to_svg(&mut dialog);

        let primary = crate::style::palette_shared()
            .map(|colors| colors.primary)
            .expect("a preset is active");
        let primary_fill = format!("rgba({},{},{},", primary.r, primary.g, primary.b);
        let success = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .expect("the theme declares the success token");
        let success_fill = format!("rgba({},{},{},", success.r, success.g, success.b);

        assert!(semantic.contains(&success_fill), "the default fill is the success token");
        assert!(!semantic.contains(&primary_fill));

        dialog.set_accent_fill(true);
        assert!(dialog.uses_accent_fill());
        let accent = crate::widget::svg::render_to_svg(&mut dialog);
        assert!(accent.contains(&primary_fill), "the accent fill is the theme's action colour");
        assert_ne!(semantic, accent);
    }

    /// Progress and label changes are observable without polling the widget.
    #[test]
    fn value_and_label_changes_emit_their_signals() {
        use std::sync::{Arc, Mutex};

        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        let values = Arc::new(Mutex::new(Vec::new()));
        let labels = Arc::new(Mutex::new(Vec::new()));
        {
            let sink = Arc::clone(&values);
            dialog.value_changed.connect(move |value| sink.lock().unwrap().push(*value));
        }
        {
            let sink = Arc::clone(&labels);
            dialog
                .label_changed
                .connect(move |label| sink.lock().unwrap().push(label.as_ref().clone()));
        }

        dialog.set_value(20);
        dialog.set_value(20);
        dialog.set_value(500);
        dialog.set_label_text("Copying");
        dialog.set_label_text("Copying");

        assert_eq!(*values.lock().unwrap(), vec![20, 100], "only real moves are reported");
        assert_eq!(*labels.lock().unwrap(), vec!["Copying".to_string()]);
    }

    /// Completion is reported once per arrival at the maximum, before auto-close hides.
    #[test]
    fn finishing_emits_finished_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut dialog = ProgressDialog::new(Rect::new(0, 0, 360, 160));
        let finished = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&finished);
        dialog.finished.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        dialog.set_value(99);
        assert_eq!(finished.load(Ordering::SeqCst), 0);
        dialog.set_value(100);
        assert_eq!(finished.load(Ordering::SeqCst), 1);
        assert!(!dialog.is_visible(), "auto-close still hides at the maximum");

        // Already at the maximum: a write that changes nothing is not a second completion.
        dialog.show();
        dialog.set_value(100);
        assert_eq!(finished.load(Ordering::SeqCst), 1);
    }
}
