// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Spin box widget for numeric input.
//!
//! # The value box yields to the step column, and the column is assembled
//!
//! BLUE22 §F.2.2 puts this control first in the §B.8 migration queue, and the reason is the one
//! insight the whole composite section rests on: the sibling-column relation reads
//! `leftPadding: padding + (mirrored ? up.width : down.width)`, i.e. **the text side's inset is
//! the sibling column's width**. The value's box is therefore not "the field minus a constant"
//! — it is whatever the column leaves.
//!
//! That relation is now expressed by assembling the row: a value column that declares
//! [`LayoutParams::filled`] and a step column of two buttons at their own width, handed to a
//! [`FlexLayout`]. The value column takes the remainder because it *asked to fill*, and the
//! column pushes it narrower when the column grows. Nothing here computes an `x`.
//!
//! The public accessors ([`SpinBox::editable_rect`], [`SpinBox::up_button`],
//! [`SpinBox::down_button`]) keep their names and their meaning: they report the layout's answer
//! rather than a second derivation of it, so the paint path, the hit test and the tests all read
//! one geometry.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
#[cfg(full_widgets)]
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
#[cfg(full_widgets)]
use crate::widget::composite::CompositeBuilder;
use crate::widget::decorations::{
    DecorationLayout, DecorationMetrics, DecorationSlots, DECORATION_GAP,
};
use crate::widget::metrics::{dimensions, ControlMetrics};

use crate::widget::capability::coercion::{expect_bool, expect_f64, expect_i64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_f64;
#[cfg(full_widgets)]
use crate::widget::WidgetFactory;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Upper bound on [`SpinBox::decimals`], matching the widest precision the default
/// display grammar can print. A larger precision would render digits that are pure
/// binary noise, which is worse than refusing the setting.
pub const SPIN_BOX_MAX_DECIMALS: u32 = 9;

/// Width of one step button in the spin box's trailing button column: 20.
///
/// See [`dimensions::SPIN_BOX_STEP_BUTTON_WIDTH`] for why the number lives in the shared table:
/// the assembled row's step column, the hint's floor and the arrow's box are three readings of it.
const SPIN_BOX_BUTTON_WIDTH: u32 = dimensions::SPIN_BOX_STEP_BUTTON_WIDTH;

/// The buttons stacked in the trailing column, counted for the width derivation.
///
/// Reads the shared table so the assembly, the hint's floor and the column's own width are one
/// fact; see [`dimensions::SPIN_BOX_STEP_BUTTONS`].
const SPIN_BOX_BUTTONS: u32 = dimensions::SPIN_BOX_STEP_BUTTONS;

/// Shape of the step arrows drawn inside the button column: 4 px of half-span, 5 px of drop.
///
/// One fact for both arrows: they carried five literals each (`4`, `4.0`, and three
/// halved `2.0`s) whose relations were only correct because they happened to be written
/// together, so changing the arrow's size meant finding all ten.
const SPIN_BOX_ARROW_HALF_SPAN: f32 = 4.0;
const SPIN_BOX_ARROW_DROP: f32 = 5.0;

/// Spin box widget for numeric input.
///
/// # Integer or decimal (BLUE20 layer 4c)
///
/// The value is stored as an `f64` and the displayed precision is a setting
/// ([`SpinBox::decimals`]). `decimals == 0` is the integer case and is the default, so
/// the widget's integer behaviour is unchanged: every integer accessor round-trips
/// exactly, and `"5"` displays as `"5"`. Raising `decimals` does not add a *second*
/// control — `double_spin_box` is already an alias of this one — it adds the decimal
/// mode the alias promises. Two near-identical controls would have to be maintained in
/// lockstep and would drift; one control with a precision setting cannot.
pub struct SpinBox {
    base: BaseWidget,
    /// Shown value. Held as `f64` so a decimal spin box and an integer spin box are the
    /// same widget; `decimals == 0` restricts it to whole numbers.
    value: f64,
    /// Lower bound of the value range; also the value that triggers
    /// `special_value_text`.
    minimum: f64,
    /// Upper bound of the value range; the value a wrapping `step_up` from
    /// `maximum` lands on.
    maximum: f64,
    /// Amount added or subtracted per step. Applied to the current value, so a
    /// step may be clamped (or wrapped) rather than landing on a multiple.
    single_step: f64,
    /// Digits shown after the decimal separator. `0` means an integer spin box, which
    /// is the default and the previous behaviour.
    ///
    /// Not a display-only filter: the value itself is rounded to this precision on every
    /// write, so `get("value")` cannot report a number the user never could have entered.
    decimals: u32,
    /// Text placed before the number in the displayed value; display only — it
    /// is not part of the parsed numeric value.
    prefix: String,
    /// Text placed after the number in the displayed value; display only.
    suffix: String,
    /// Replaces the formatted number in the display whenever the value equals
    /// `minimum`, e.g. `"Auto"` for a value of 0. `None` always shows the
    /// number.
    special_value_text: Option<String>,
    /// When true, stepping past `minimum` jumps to `maximum` (and vice versa)
    /// instead of clamping. Affects only the step buttons / keyboard stepping,
    /// not `set_value`.
    wrapping: bool,
    /// Emitted with the new value after any change, including clamping by
    /// `minimum` / `maximum`. Not emitted when the value is set to the value it
    /// already had.
    pub value_changed: Signal1<i32>,
    /// Emitted without a payload when an in-progress edit is committed or
    /// cancelled (Enter, focus loss, or step).
    pub editing_finished: GenericSignal,
}
impl SpinBox {
    /// The band the whole control is painted in: full width, one text field tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// A spin box is a **field plus a step column**, and both are chrome of a fixed height:
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. This
    /// control used to be the one exception — the 240x120 census cell drew a 240x120 slab
    /// whose buttons were **120 px tall** (`spin_box.svg` carried `rect x=200 width=20
    /// height=120` twice) — so a spin box and the text field it sits beside in the same form
    /// were different objects. The band is the single derivation the surface, the button
    /// column, the value's line box and the hit test all read.
    fn row_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The trailing button column: the **remainder** of the band, not a fixed rectangle.
    ///
    /// # Why the column is derived from its width rather than from the band's edge
    ///
    /// This is BLUE22 §B.9's rule: a sub-part's box is derived from its siblings, so a
    /// sub-part that grows *pushes* its neighbours rather than overlapping them. The buttons
    /// were placed at `band.right() - button_width * 2` and the value's text at the field's
    /// leading padding — two derivations from two different edges with no relation between
    /// them, which is why a wider button or a larger font ran the value underneath the
    /// buttons (`spin_box`'s value was drawn at x = 4 with the column starting at x = 200).
    ///
    /// The column's width is now what the layout hands back for the step column, so it is one
    /// reading of the same assembly the value's box comes from — not a second piece of arithmetic
    /// that happens to agree.
    fn button_column(&self) -> Rect {
        self.assemble_row().1
    }

    /// The box the user may type in: whatever the button column leaves.
    ///
    /// The sibling-column relation derives exactly this —
    /// the text area *yields* to the button column, so the two cannot overlap at any font or
    /// button size.
    fn editable_rect(&self) -> Rect {
        self.assemble_row().0
    }

    /// The upper/lower half of the button column that the *up* step owns.
    fn up_button(&self) -> Rect {
        let column = self.button_column();
        let half = column.height / 2;
        Rect::new(column.x, column.y, column.width, half)
    }

    /// The upper/lower half of the button column that the *down* step owns.
    ///
    /// Derived from the same column as [`Self::up_button`], so the two tile it: they were
    /// two independent `+ height`/`+ height / 2` expressions in `draw` and a third spelling
    /// in the hit test.
    fn down_button(&self) -> Rect {
        let column = self.button_column();
        let half = column.height / 2;
        Rect::new(
            column.x,
            column.y + column.height.saturating_sub(half) as i32,
            column.width,
            half,
        )
    }

    /// The value box and the step column, placed by the layout that owns the tiling.
    ///
    /// # Why the row is assembled rather than computed
    ///
    /// This is the sibling-column relation — `leftPadding: padding + down.width` — with the
    /// arithmetic done by the thing that owns it. The value column *asks to fill* and the step
    /// column declares its own width, so the value's box is the remainder **by construction**: a
    /// wider column, or a larger font on the buttons, narrows the value rather than running it
    /// underneath, and no code here can express the overlap the comment above records.
    ///
    /// # Why the column is two children and not one
    ///
    /// The column's two halves are `up` and `down`, tiled vertically — the same `VBox` relation
    /// the hand-written `+ height / 2` used to express, now spelled as a second layout. The two
    /// buttons are what the column's *width* is derived from (the same relation again: the width
    /// is one button's), so they are the children that decide it.
    ///
    /// # What the assembly is measured from
    ///
    /// Both numbers come from the shared table ([`dimensions::SPIN_BOX_STEP_BUTTON_WIDTH`],
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`]), so the hint `size_hint` reports and the boxes this
    /// method hands the paint path are two readings of one derivation.
    fn assemble_row(&self) -> (Rect, Rect) {
        let band = self.row_band();
        let column_width = self.step_column_width();
        // # Why the stripped profiles take the direct route
        //
        // `mini`/`embedded` have neither `WidgetFactory` nor `Box` under `alloc_frugal`
        // (principle #47: `full_widgets` is "a device profile *and* an unstripped widget set"), so
        // the assembly cannot exist there. The two-column split is one subtraction, and both arms
        // of this function are readings of the *same* two numbers (`column_width` and the band), so
        // the fallback is not a second derivation — it is the same relation written the only way
        // that profile can express it.
        #[cfg(not(full_widgets))]
        {
            (
                Rect::new(band.x, band.y, band.width.saturating_sub(column_width), band.height),
                Rect::new(
                    band.x + band.width.saturating_sub(column_width) as i32,
                    band.y,
                    column_width,
                    band.height,
                ),
            )
        }
        #[cfg(full_widgets)]
        {
            let factory = WidgetFactory::new_with_defaults();
            let mut row = CompositeBuilder::new(
                Box::new(FlexLayout::with_params(
                    FlexDirection::Row,
                    FlexWrap::NoWrap,
                    JustifyContent::FlexStart,
                    AlignItems::Stretch,
                    0,
                    0,
                )),
                EdgeOffsets::all(0),
                Size::new(0, 0),
            );
            // The value column: it fills whatever the step column leaves. Its own floor is the
            // field's padding, so a band narrower than the step column collapses it to nothing rather
            // than giving it a negative width.
            let value = row.add_sized(
                &factory,
                "label",
                "",
                Size::new(dimensions::TEXT_FIELD_PADDING_H, band.height),
                LayoutParams::filled(),
            );
            debug_assert!(value.is_some(), "the value column is a core control");
            // One column of two stacked buttons. Its height is the band's, so the layout has nothing
            // to resolve on the cross axis: the column is as tall as the field it belongs to,
            // whatever the field's own height turns out to be.
            let column = row.add_sized(
                &factory,
                "label",
                "",
                Size::new(column_width, band.height),
                LayoutParams::new(),
            );
            debug_assert!(column.is_some(), "the step column is a core control");

            let mut placed: Vec<Rect> = Vec::with_capacity(2);
            row.arrange(band, &mut |_, rect| placed.push(rect));
            match (placed.first(), placed.get(1)) {
                (Some(value), Some(column)) => (*value, *column),
                // `debug_assert!` above makes this unreachable in a debug build; the fallback keeps
                // the two boxes tiling the band instead of leaving a sub-part at a stale rectangle.
                _ => {
                    let column_width = column_width.min(band.width);
                    (
                        Rect::new(
                            band.x,
                            band.y,
                            band.width.saturating_sub(column_width),
                            band.height,
                        ),
                        Rect::new(
                            band.x + band.width.saturating_sub(column_width) as i32,
                            band.y,
                            column_width,
                            band.height,
                        ),
                    )
                }
            }
        }
    }

    /// The width the step column occupies: two buttons, clamped to the band.
    ///
    /// One derivation for both arms of [`Self::assemble_row`] *and* for the `size_hint` floor, so
    /// the hint a layout is told and the column the control draws cannot disagree.
    fn step_column_width(&self) -> u32 {
        dimensions::SPIN_BOX_STEP_BUTTON_WIDTH
            .saturating_mul(SPIN_BOX_BUTTONS)
            .min(self.row_band().width)
    }

    /// Creates a spin box with default range 0-99 and integer precision.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SpinBox, geometry, "SpinBox"),
            value: 0.0,
            minimum: 0.0,
            maximum: 99.0,
            single_step: 1.0,
            decimals: 0,
            prefix: String::new(),
            suffix: String::new(),
            special_value_text: None,
            wrapping: false,
            value_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
        }
    }
    /// Returns the current value, as an integer.
    ///
    /// Rounds rather than truncates: with `decimals == 3` and a value of `2.7`, "the
    /// value as an integer" is `3`, and truncation would also turn `-2.7` into `-2`, an
    /// error that grows with magnitude. The exact value is [`Self::value_f64`].
    pub fn value(&self) -> i32 {
        round_to_i32(self.value)
    }
    /// Returns the current value at full precision.
    pub fn value_f64(&self) -> f64 {
        self.value
    }
    /// Sets the value, clamped to the range and rounded to [`Self::decimals`].
    pub fn set_value(&mut self, value: i32) {
        self.set_value_f64(value as f64);
    }
    /// Sets the value as a decimal, clamped and rounded to [`Self::decimals`].
    ///
    /// A non-finite input is ignored rather than stored: `NaN` has no position in a
    /// range, and storing it would make every later comparison (`==`, `<`) false and the
    /// control unreadable. `±inf` is a legitimate open bound, so it is clamped normally.
    pub fn set_value_f64(&mut self, value: f64) {
        if value.is_nan() {
            return;
        }
        let clamped = ordered_clamp_f64(value, self.minimum, self.maximum);
        let rounded = round_to_decimals(clamped, self.decimals);
        // `==` on two f64s that both passed through the same rounding is exact: this is
        // not approximate comparison, it is "did the write change anything".
        if self.value == rounded {
            return;
        }
        self.value = rounded;
        self.value_changed.emit(self.value());
        self.base.request_redraw();
    }
    /// Returns minimum value, as an integer.
    pub fn minimum(&self) -> i32 {
        round_to_i32(self.minimum)
    }
    /// Returns the minimum at full precision.
    pub fn minimum_f64(&self) -> f64 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        self.set_minimum_f64(minimum as f64);
    }
    /// Sets the minimum as a decimal, re-clamping the range and the value.
    pub fn set_minimum_f64(&mut self, minimum: f64) {
        if minimum.is_nan() {
            return;
        }
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value_f64(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Returns maximum value, as an integer.
    pub fn maximum(&self) -> i32 {
        round_to_i32(self.maximum)
    }
    /// Returns the maximum at full precision.
    pub fn maximum_f64(&self) -> f64 {
        self.maximum
    }
    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        self.set_maximum_f64(maximum as f64);
    }
    /// Sets the maximum as a decimal, re-clamping the range and the value.
    pub fn set_maximum_f64(&mut self, maximum: f64) {
        if maximum.is_nan() {
            return;
        }
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value_f64(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        self.set_range_f64(minimum as f64, maximum as f64);
    }
    /// Sets both bounds as decimals in one call.
    ///
    /// The bounds are ordered before being stored, so `set_range_f64(10.0, 0.0)` names
    /// the range `[0.0, 10.0]` instead of leaving an inverted pair that `set_value_f64`
    /// would then clamp against (principle #17, and the panic `ordered_clamp_f64`
    /// exists to prevent is only half the story — an inverted *stored* pair silently
    /// collapses the range to a point).
    pub fn set_range_f64(&mut self, minimum: f64, maximum: f64) {
        if minimum.is_nan() || maximum.is_nan() {
            return;
        }
        self.minimum = minimum.min(maximum);
        self.maximum = minimum.max(maximum);
        self.set_value_f64(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Returns the number of digits shown after the decimal separator.
    pub fn decimals(&self) -> u32 {
        self.decimals
    }
    /// Sets the display precision, `0` for an integer spin box.
    ///
    /// Values above [`SPIN_BOX_MAX_DECIMALS`] are clamped rather than accepted: the
    /// digits past that point are binary noise, and printing them would show the user a
    /// number the widget does not actually hold. Changing the precision re-rounds the
    /// current value immediately, so what is displayed and what is stored agree. It does
    /// **not** emit `value_changed`: no step was taken, and a caller counting changes
    /// would otherwise see a phantom one.
    pub fn set_decimals(&mut self, decimals: u32) {
        let decimals = decimals.min(SPIN_BOX_MAX_DECIMALS);
        if self.decimals == decimals {
            return;
        }
        self.decimals = decimals;
        self.value = round_to_decimals(self.value, decimals);
        self.base.request_redraw();
    }
    /// Returns single step value, as an integer.
    pub fn single_step(&self) -> i32 {
        round_to_i32(self.single_step)
    }
    /// Returns the step at full precision.
    pub fn single_step_f64(&self) -> f64 {
        self.single_step
    }
    /// Sets single step value.
    ///
    /// A step of zero would make `step_up`/`step_down` do nothing at all, which looks
    /// like a broken button rather than a configured one, so the magnitude is floored at
    /// `1` in integer mode. In decimal mode the floor is one unit of the last displayed
    /// place (`0.01` at two decimals), because `0.5` is a meaningful step for a decimal
    /// spin box and a meaningful step must never round to "no step".
    pub fn set_single_step(&mut self, step: i32) {
        self.set_single_step_f64(step as f64);
    }
    /// Sets the step as a decimal, floored at the smallest representable increment.
    pub fn set_single_step_f64(&mut self, step: f64) {
        let floor = self.smallest_step();
        self.single_step = if step.is_nan() { floor } else { step.abs().max(floor) };
        self.base.request_redraw();
    }
    /// The smallest step that still changes the displayed value.
    fn smallest_step(&self) -> f64 {
        if self.decimals == 0 {
            1.0
        } else {
            10f64.powi(-(self.decimals as i32))
        }
    }
    /// Returns prefix text.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    /// Sets prefix text.
    pub fn set_prefix(&mut self, prefix: String) {
        self.prefix = prefix;
        self.base.request_redraw();
    }
    /// Returns suffix text.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }
    /// Sets suffix text.
    pub fn set_suffix(&mut self, suffix: String) {
        self.suffix = suffix;
        self.base.request_redraw();
    }
    /// Returns special value text.
    pub fn special_value_text(&self) -> Option<&str> {
        self.special_value_text.as_deref()
    }
    /// Sets special value text.
    pub fn set_special_value_text(&mut self, text: Option<String>) {
        self.special_value_text = text;
        self.base.request_redraw();
    }
    /// Returns whether wrapping is enabled.
    pub fn wrapping(&self) -> bool {
        self.wrapping
    }
    /// Sets wrapping state.
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
        self.base.request_redraw();
    }
    /// Increments value by single step.
    pub fn step_up(&mut self) {
        let mut new_value = self.value + self.single_step;
        if new_value > self.maximum {
            if self.wrapping {
                new_value = self.minimum;
            } else {
                new_value = self.maximum;
            }
        }
        self.set_value_f64(new_value);
    }
    /// Decrements value by single step.
    pub fn step_down(&mut self) {
        let mut new_value = self.value - self.single_step;
        if new_value < self.minimum {
            if self.wrapping {
                new_value = self.maximum;
            } else {
                new_value = self.minimum;
            }
        }
        self.set_value_f64(new_value);
    }
    /// Returns the formatted number, without prefix or suffix.
    ///
    /// The precision comes from [`Self::decimals`], so an integer spin box prints `5`
    /// and a two-decimal one prints `1.50` — the same digits the user could have typed.
    /// Trailing zeros are kept, because dropping them would make a two-decimal spin box
    /// look like it had accepted `1.5` when it holds `1.50`.
    pub fn formatted_value(&self) -> String {
        match self.decimals {
            0 => format!("{}", round_to_i32(self.value)),
            n => format!("{:.*}", n as usize, self.value),
        }
    }
    /// Returns display text: the special value text, or prefix + number + suffix.
    ///
    /// # Why this is the *announcement* string and not what is drawn
    ///
    /// Reading out `$12` is correct — a screen reader should hear the unit. But **drawing** it as one
    /// string puts the prefix inside the value's glyph run, which makes the value's own origin include
    /// it: the caret lands after the `$` at position 0, and a select-all would take the `$` with it. The
    /// painter therefore draws [`Self::value_text`] in the value's box and the two slots in their own,
    /// while this stays the form a caller announces or logs — which is why it is `pub`.
    ///
    /// See [`crate::widget::decorations`] for the full argument.
    pub fn display_text(&self) -> String {
        if let Some(special) = &self.special_value_text {
            if self.value == self.minimum {
                return special.clone();
            }
        }
        format!("{}{}{}", self.prefix, self.formatted_value(), self.suffix)
    }

    /// The text drawn inside the value's own box: the special value text, or just the number.
    ///
    /// The slots are **excluded**, which is what makes the value's origin independent of them.
    fn value_text(&self) -> String {
        if let Some(special) = &self.special_value_text {
            if self.value == self.minimum {
                return special.clone();
            }
        }
        self.formatted_value()
    }
}

/// Rounds to the nearest integer, saturating instead of wrapping out of `i32`.
///
/// `as i32` truncates *and* saturates silently since Rust 1.45, which is two surprises
/// at once; the round-then-saturate here is explicit about both halves. The saturation
/// matters because `f64::INFINITY` is a legal bound, and an unbounded spin box reading
/// its `i32` value must not report `i32::MAX` for `0.0`.
fn round_to_i32(value: f64) -> i32 {
    if value.is_nan() {
        return 0;
    }
    let rounded = value.round();
    if rounded >= i32::MAX as f64 {
        i32::MAX
    } else if rounded <= i32::MIN as f64 {
        i32::MIN
    } else {
        rounded as i32
    }
}

/// Rounds `value` to `decimals` places after the decimal separator.
///
/// # Why this goes through the decimal text
///
/// The obvious form is `(value * 10^n).round() / 10^n`, and it can disagree with what the
/// widget prints. `format!("{:.n}", x)` is correctly rounded *once*, from `x`; the scale
/// form rounds an intermediate product that carries its own representation error, and a
/// product that lands on a tie rounds the wrong way. Measured on this build,
/// `2.0965 x 1000.0` is exactly `2096.5000000000005`, so the scale form gives `2.097` while
/// the text says `2.096`. Those are the same number printed two ways, and a spin box whose
/// stored value disagrees with its own display is showing the user a figure it does not
/// hold. Taking the text as the authority is what makes the two agree to the digit.
///
/// # What this does *not* do
///
/// It does not "round the way a human would read the literal". `1.005` and `2.675` as
/// `f64` **are** `1.00499999999999989...` and `2.67499999999999982...`, so rounding them to
/// two places gives `1.00` and `2.67` — correctly, and the same answer a decimal parser
/// would produce from the identical text. No rounding scheme can recover a midpoint the
/// input never stored; that would require carrying the value as decimal text instead of
/// `f64`, which is a much larger change than this setting. The invariant guaranteed here is
/// the one that matters: **the stored value equals the number the widget prints.**
fn round_to_decimals(value: f64, decimals: u32) -> f64 {
    if decimals == 0 {
        let rounded = value.round();
        // `-0.0` and `0.0` compare equal but format differently (`-0`), and a value of
        // `-0.4` rounded to integers is zero, not "negative zero".
        return if rounded == 0.0 { 0.0 } else { rounded };
    }
    if !value.is_finite() {
        return value;
    }
    let text = format!("{:.*}", decimals as usize, value);
    text.parse::<f64>().unwrap_or(value)
}
// Implement Widget trait
impl Widget for SpinBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Width follows the *formatted* text, not the integer value: a spin box showing
        // `1234.50` is wider than one showing `1234`, and sizing off the integer form would
        // clip the decimals it was explicitly asked to display.
        //
        // The text is a **content** width and is handed to `ControlMetrics::implicit_size`
        // as such, with the step column expressed as the floor rather than added on top. That
        // is the whole reason the metric exists: the number a layout is told and the box the
        // control draws are then two readings of one formula. The two are deliberately not
        // bit-identical here — a hint is measured with a nominal advance per character
        // because `size_hint` has no `RenderContext` to measure with, while the paint path
        // *fits* the value into the box this hint produced. A hint that is a few pixels
        // generous is the safe direction; one that is short would elide the very digits the
        // caller asked for.
        let value = self.formatted_value();
        let text_width = value.len() as u32 * 8;
        let field_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let padding = EdgeOffsets {
            top: field_air,
            right: dimensions::TEXT_FIELD_PADDING_H,
            bottom: field_air,
            left: dimensions::TEXT_FIELD_PADDING_H,
        };
        // The floor is the field's own height plus the step column the band tiles; every term
        // is a value from the shared table, so the hint and the paint cannot disagree about
        // how many pixels the buttons take.
        let floor = Size::new(
            dimensions::TEXT_FIELD_PADDING_H * 2 + SPIN_BOX_BUTTON_WIDTH * SPIN_BOX_BUTTONS,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        ControlMetrics::implicit_size(Size::new(text_width, 0), padding, floor)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SpinBox`'s property contract.
///
/// # Integers stay integers (principle #21)
///
/// The three numeric properties report `Int` whenever `decimals == 0`, which is the
/// default, so existing callers that write `{"value": 3}` and read back `Int(3)` are
/// unaffected. Only a caller that has opted into decimals sees a `Float`, and a caller
/// that sends an `Int` to a decimal spin box is accepted (it is an exact number) rather
/// than rejected as a type error.
impl WidgetProperties for SpinBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        // Integer mode reports integers; the caller that set `decimals(0)` asked for an
        // integer spin box and should not have to unwrap a float to read it.
        let as_number = |value: f64| {
            if self.decimals == 0 {
                CapabilityValue::Int(round_to_i32(value) as i64)
            } else {
                CapabilityValue::Float(value)
            }
        };
        match name {
            "minimum" => Ok(as_number(self.minimum_f64())),
            "maximum" => Ok(as_number(self.maximum_f64())),
            "value" => Ok(as_number(self.value_f64())),
            "single_step" => Ok(as_number(self.single_step_f64())),
            "decimals" => Ok(CapabilityValue::Int(self.decimals() as i64)),
            "prefix" => Ok(CapabilityValue::String(self.prefix().to_string())),
            "suffix" => Ok(CapabilityValue::String(self.suffix().to_string())),
            // The announcement form: `$12`, or the special text at the minimum. Published because it is
            // what a caller shows in a log or reads aloud, and re-deriving it from `value` + the two
            // slots in the consumer is the second derivation this contract exists to remove.
            "display_text" => Ok(CapabilityValue::String(self.display_text())),
            // Just the number, without the unit marks — the string the painter puts in the value's box.
            "value_text" => Ok(CapabilityValue::String(self.value_text())),
            "special_value_text" => match self.special_value_text() {
                Some(text) => Ok(CapabilityValue::String(text.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "wrapping" => Ok(CapabilityValue::Bool(self.wrapping())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        // A number arrives as `Int` or `Float` depending on how the caller spelled it
        // (`3` vs `3.0`), and both mean the same value. Accepting both is what keeps
        // `{"value": 3}` valid after a spin box has been given decimals.
        fn number(value: CapabilityValue) -> Result<f64, CapabilityAccessError> {
            match value {
                CapabilityValue::Int(int) => Ok(int as f64),
                other => expect_f64(other),
            }
        }
        match name {
            "minimum" => {
                self.set_minimum_f64(number(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum_f64(number(value)?);
                Ok(())
            }
            "value" => {
                self.set_value_f64(number(value)?);
                Ok(())
            }
            "single_step" => {
                self.set_single_step_f64(number(value)?);
                Ok(())
            }
            "decimals" => {
                let decimals = expect_i64(value)?;
                // `decimals` is declared `UInt`, so a negative is not a type error but a
                // value that addresses nothing. Answering `OutOfRange` says exactly that
                // and keeps the caller's mistake (their argument) distinct from a
                // capability gap (`UnsupportedOnWidget`) — the distinction
                // `CapabilityAccessError::OutOfRange` exists to preserve.
                let decimals =
                    u32::try_from(decimals).map_err(|_| CapabilityAccessError::OutOfRange)?;
                self.set_decimals(decimals);
                Ok(())
            }
            "prefix" => {
                self.set_prefix(expect_string(value)?);
                Ok(())
            }
            "suffix" => {
                self.set_suffix(expect_string(value)?);
                Ok(())
            }
            "special_value_text" => {
                match value {
                    CapabilityValue::Null => self.set_special_value_text(None),
                    other => self.set_special_value_text(Some(expect_string(other)?)),
                }
                Ok(())
            }
            "wrapping" => {
                self.set_wrapping(expect_bool(value)?);
                Ok(())
            }
            // Both are derived from the value and the two slots, so a writer would be a second way to
            // say something the control already answers — and one of the two could disagree.
            "display_text" | "value_text" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SPIN_BOX_PROPERTIES`.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "decimals",
            "prefix",
            "suffix",
            "special_value_text",
            "wrapping",
            "display_text",
            "value_text",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `spin_box` publishes.
    ///
    /// `step_up` / `step_down` are the zero-argument actions: they move the value by
    /// `single_step` and wrap when wrapping is enabled, which is exactly the effect a
    /// caller asking for "one step" means. `set_range` and `set_value` assign state and
    /// therefore need a payload, so they are answered through the property route rather
    /// than guessed at; `OutOfRange` says the name is right and the invocation is not.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "step_up" => {
                self.step_up();
                Ok(())
            }
            "step_down" => {
                self.step_down();
                Ok(())
            }
            "set_range" | "set_value" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl SpinBox {
    /// Handle click/tap on the increment/decrement buttons.
    ///
    /// # Why this takes no rectangle or width
    ///
    /// It used to take both and re-derive the buttons as `rect.right() - button_width * 2`,
    /// which was a **third** copy of the column's arithmetic (one in `draw`, one in the
    /// event arm, one here) — three spellings that described the same two buttons and
    /// agreed only while nobody edited one of them. The boxes now come from the same
    /// accessors the paint path uses, and the caller passes only what it alone knows: where
    /// the pointer landed.
    fn handle_button_click(&mut self, pos: Point) {
        if self.down_button().contains_point(pos) {
            self.step_down();
            self.base.clicked.emit();
        } else if self.up_button().contains_point(pos) {
            self.step_up();
            self.base.clicked.emit();
        }
    }
}
impl EventHandler for SpinBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    self.handle_button_click(*pos);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                self.handle_button_click(*pos);
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    38 => {
                        // Up arrow
                        self.step_up();
                    }
                    40 => {
                        // Down arrow
                        self.step_down();
                    }
                    13 => {
                        // Enter
                        self.editing_finished.emit();
                    }
                    27 => {
                        // Escape
                        self.editing_finished.emit();
                    }
                    // Unknown key; ignore
                    _ => {}
                }
            }
            Event::FocusLost => {
                self.editing_finished.emit();
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for SpinBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // ── The band actually painted ──
        //
        // Every measurement below is taken from one of the four derived boxes (`row_band`,
        // `editable_rect`, `up_button`, `down_button`), so they cannot disagree about where
        // the field is, how tall the buttons are or where the value may be written. The
        // control's own rectangle decides *where* the band sits and nothing else.
        let band = self.row_band();
        // A band with no room left is not a spin box: the guard keeps the two buttons and
        // the value from being emitted as zero-extent elements, which is the shape the SVG
        // census reads as an invisible control.
        if band.width == 0 || band.height == 0 {
            return;
        }
        let editable = self.editable_rect();
        let down_button = self.down_button();
        let up_button = self.up_button();
        // The value's own text origin. The slots are laid out *below* from the same `editable` box, so
        // the value's start is derived once rather than the two being separate sums of the same padding.
        let style = self.style();
        let bg = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        // The stepper buttons are part of this control's own chrome, so they follow
        // the widget's style instead of a fixed grey: the theme resolved a colour for
        // the spin box, and a literal made the two button wells ignore it (and stay
        // light in a dark theme). The arrows are ink drawn on those wells, so they
        // read `text_color` — the same colour the value's text uses.
        let button_bg = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        let button_border = style.border_color.unwrap_or(Color::rgb(200, 200, 200));
        let arrow_color = style.text_color.unwrap_or(Color::rgb(100, 100, 100));
        let default_font = Font::default();
        let font = style.font.as_ref().unwrap_or(&default_font);
        // Draw background
        context.fill_rect(band, bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(band, border_color);
        }
        // Draw up/down buttons. Each is the half of the trailing column it owns, so the two
        // are the same height whatever the band is, and neither can overlap the value.
        context.fill_rect(down_button, button_bg);
        context.draw_rect(down_button, button_border);
        draw_step_arrow(context, down_button, false, arrow_color);
        context.fill_rect(up_button, button_bg);
        context.draw_rect(up_button, button_border);
        draw_step_arrow(context, up_button, true, arrow_color);
        // Draw text. The value is bounded to the **editable** box, so it is fitted into the
        // space the button column left instead of being written from the field's leading
        // padding and allowed to run under the buttons.
        //
        // # Why the slots are drawn separately rather than concatenated
        //
        // `display_text()` still produces the announcement form (`$12`), because that is what a screen
        // reader should hear. What is *painted* is `value_text()` inside the value's own box, with the
        // prefix in its box on the leading side and the suffix anchored to the trailing one. Drawing the
        // concatenated string instead put the `$` inside the value's glyph run, so the value's origin
        // included it — the caret would sit after the `$` at the start of the field, and a select-all
        // would copy the unit along with the number.
        let slots = DecorationSlots {
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            ..Default::default()
        };
        let metrics =
            DecorationMetrics::measure(&slots, |text| context.measure_text(text, font).width);
        let line_height = font.effective_line_height().max(1.0) as u32;
        let layout = DecorationLayout::compute(
            editable,
            dimensions::TEXT_FIELD_PADDING_H,
            line_height,
            DECORATION_GAP,
            metrics,
            0,
            &slots,
        );
        let value_text = self.value_text();
        if !value_text.is_empty() {
            let line = context.text_line(editable, font);
            context.draw_text_fitted(
                Rect::new(layout.value.x, line.y, layout.value.width, line.height),
                &value_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
        // The slots are chrome, not content, so they are drawn a step toward the field's own fill —
        // the same "less prominent than the value" reading `lineedit` uses for the same two slots.
        let slot_color = text_color.blend(&bg, 0.35);
        let line = context.text_line(editable, font);
        if let Some(prefix_box) = layout.prefix {
            context.draw_text(
                Point::new(prefix_box.x, line.y),
                &self.prefix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(suffix_box) = layout.suffix {
            context.draw_text(
                Point::new(suffix_box.x, line.y),
                &self.suffix,
                font,
                slot_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

/// Draws one step arrow inside `button`, pointing up when `up` is set.
///
/// # Why the origin is the box's centre line, not its edge
///
/// The chevron is a **shape**, not text, so it is centred on the box rather than placed by
/// a text origin — the five literals per arrow (`4`, and the three `4.0 / 2.0` halves) were
/// each an independent offset from the button's midpoint, and none of them was named. The
/// two shapes here are the same three-point chevron reflected about the midpoint, so the up
/// and down arrows cannot disagree about the arrow's size or its drop.
fn draw_step_arrow(context: &mut RenderContext, button: Rect, up: bool, color: Color) {
    let cx = button.x as f32 + button.width as f32 / 2.0;
    let cy = button.y as f32 + button.height as f32 / 2.0;
    let span = SPIN_BOX_ARROW_HALF_SPAN;
    // A downward chevron opens downward; an upward one is the same shape with the drop
    // negated, so the two are one derivation read with a sign.
    let drop = if up { -SPIN_BOX_ARROW_DROP } else { SPIN_BOX_ARROW_DROP };
    let left = Point::from_f32(cx - span, cy - drop / 2.0);
    let tip = Point::from_f32(cx, cy + drop / 2.0);
    let right = Point::from_f32(cx + span, cy - drop / 2.0);
    context.draw_line(left, tip, color);
    context.draw_line(tip, right, color);
    context.draw_line(right, left, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn spinbox_creation_defaults() {
        let sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_eq!(sb.value(), 0);
        assert_eq!(sb.minimum(), 0);
        assert_eq!(sb.maximum(), 99);
        assert_eq!(sb.single_step(), 1);
        assert!(sb.prefix().is_empty());
        assert!(sb.suffix().is_empty());
        assert!(!sb.wrapping());
        assert!(sb.special_value_text().is_none());
    }

    #[test]
    fn spinbox_set_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        assert_eq!(sb.value(), 50);
        sb.set_value(200); // clamp to max
        assert_eq!(sb.value(), 99);
        sb.set_value(-10); // clamp to min
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn spinbox_set_range() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_minimum(-50);
        sb.set_maximum(200);
        assert_eq!(sb.minimum(), -50);
        assert_eq!(sb.maximum(), 200);
    }

    #[test]
    fn spinbox_set_range_reclamps_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.set_range(60, 100);
        assert_eq!(sb.value(), 60);
    }

    #[test]
    fn spinbox_prefix_suffix() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_prefix("$".to_string());
        assert_eq!(sb.prefix(), "$");
        sb.set_suffix(" USD".to_string());
        assert_eq!(sb.suffix(), " USD");
        sb.set_prefix(String::new());
        assert!(sb.prefix().is_empty());
    }

    /// The two forms differ: the announcement string carries the unit marks, and the drawn value does
    /// not.
    ///
    /// # The defect this pins
    ///
    /// The painter used the concatenated string, so the value's own glyph run contained the `$` — which
    /// made the value's origin include it. That is the same string a caret would be measured against, so
    /// on a spin box it was invisible (there is no caret), but it made `value_text` and `display_text` the
    /// same thing, and the two are genuinely different statements: one is read out, one is drawn.
    #[test]
    fn the_announcement_string_carries_the_units_and_the_drawn_value_does_not() {
        use crate::widget::capability::WidgetProperties;

        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(12);
        assert_eq!(sb.value_text(), "12", "the drawn value is the number alone");
        assert_eq!(sb.display_text(), "12", "and with no units the two agree");

        sb.set_prefix("$".to_string());
        sb.set_suffix(" USD".to_string());
        assert_eq!(sb.value_text(), "12", "the units are not part of the value");
        assert_eq!(sb.display_text(), "$12 USD", "but they are part of the announcement");

        // Both are reachable through the contract, and neither is writable.
        assert_eq!(sb.get("display_text").unwrap().as_str(), Some("$12 USD"));
        assert_eq!(sb.get("value_text").unwrap().as_str(), Some("12"));
        assert!(sb.set("display_text", CapabilityValue::String("x".into())).is_err());
        assert!(sb.set("value_text", CapabilityValue::String("x".into())).is_err());
    }

    /// The prefix is drawn to the **left** of the value and the suffix to its right, as two runs.
    ///
    /// This is what makes the slots chrome rather than content, and it is readable from the SVG: three
    /// separate `draw_text` calls produce three paths, and their order on the x axis is the layout.
    #[test]
    fn the_slots_are_drawn_on_either_side_of_the_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 200, 24));
        sb.set_value(7);
        sb.set_prefix("$".to_string());
        sb.set_suffix("%".to_string());
        let svg = crate::widget::svg::render_to_svg(&mut sb);

        let mut runs = crate::widget::svg::text_ink_boxes(&svg);
        runs.sort_by_key(|b| b.0);
        assert_eq!(runs.len(), 3, "a prefix, a value and a suffix are three runs: {runs:?}");
        // Read left to right the order is prefix, value, suffix.
        assert!(
            runs[0].2 <= runs[1].0,
            "the prefix ({:?}) must end before the value ({:?}) begins",
            runs[0],
            runs[1]
        );
        assert!(
            runs[1].2 <= runs[2].0,
            "the value ({:?}) must end before the suffix ({:?}) begins",
            runs[1],
            runs[2]
        );
        // And with no slots at all there is exactly one run, so the slots are what added the others.
        let mut bare = SpinBox::new(Rect::new(0, 0, 200, 24));
        bare.set_value(7);
        let bare_svg = crate::widget::svg::render_to_svg(&mut bare);
        assert_eq!(crate::widget::svg::text_ink_boxes(&bare_svg).len(), 1);
    }

    /// A prefix wide enough to consume the field's inner box leaves the value **no room**, and the clamp
    /// is what stops the two from overlapping.
    ///
    /// # Why the assertion is "one run", not "two runs that do not overlap"
    ///
    /// The prefix is 17 characters in a field whose editable box is roughly 170 px wide at this font, so
    /// the prefix legitimately takes all of it. The value's box then clamps to zero width and nothing is
    /// painted for it — which *is* the no-overlap guarantee, expressed as "the second run was not drawn"
    /// rather than as "the second run was drawn somewhere harmless". Asserting two runs would be asserting
    /// that the value was painted on top of the prefix.
    #[test]
    fn a_prefix_wide_enough_to_fill_the_field_leaves_the_value_no_room() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 200, 24));
        sb.set_value(7);
        assert_eq!(sb.value_text(), "7");

        let mut bare = SpinBox::new(Rect::new(0, 0, 200, 24));
        bare.set_value(7);
        let bare_runs =
            crate::widget::svg::text_ink_boxes(&crate::widget::svg::render_to_svg(&mut bare));
        assert_eq!(bare_runs.len(), 1, "the value is drawn when nothing squeezes it");

        sb.set_prefix("a-very-long-prefix".to_string());
        let runs = crate::widget::svg::text_ink_boxes(&crate::widget::svg::render_to_svg(&mut sb));
        assert_eq!(
            runs.len(),
            1,
            "the value's box clamped to zero width, so only the prefix has ink: {runs:?}"
        );
        // And what was drawn starts at or after the field's padding — nothing was pushed off the leading
        // edge.
        assert!(
            runs[0].0 >= sb.editable_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32,
            "the prefix must not escape the field: {runs:?}"
        );
    }

    #[test]
    fn spinbox_single_step() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_single_step(5);
        assert_eq!(sb.single_step(), 5);
        sb.set_single_step(0); // floors at 1
        assert_eq!(sb.single_step(), 1);
    }

    #[test]
    fn spinbox_wrapping() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(!sb.wrapping());
        sb.set_wrapping(true);
        assert!(sb.wrapping());
        sb.set_wrapping(false);
        assert!(!sb.wrapping());
    }

    #[test]
    fn spinbox_step_up() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.step_up();
        assert_eq!(sb.value(), 51);
    }

    #[test]
    fn spinbox_step_up_clamps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(99);
        sb.step_up();
        assert_eq!(sb.value(), 99); // clamped to max
    }

    #[test]
    fn spinbox_step_up_wraps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_wrapping(true);
        sb.set_value(99);
        sb.step_up();
        assert_eq!(sb.value(), 0); // wraps to min
    }

    #[test]
    fn spinbox_step_down() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(50);
        sb.step_down();
        assert_eq!(sb.value(), 49);
    }

    #[test]
    fn spinbox_step_down_clamps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(0);
        sb.step_down();
        assert_eq!(sb.value(), 0); // clamped to min
    }

    #[test]
    fn spinbox_step_down_wraps() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_wrapping(true);
        sb.set_value(0);
        sb.step_down();
        assert_eq!(sb.value(), 99); // wraps to max
    }

    #[test]
    fn spinbox_special_value_text() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.special_value_text().is_none());
        sb.set_special_value_text(Some("Zero".to_string()));
        assert_eq!(sb.special_value_text(), Some("Zero"));
        sb.set_special_value_text(None);
        assert!(sb.special_value_text().is_none());
    }

    #[test]
    fn spinbox_geometry_delegation() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_geometry(Rect::new(10, 10, 200, 30));
        assert_eq!(sb.geometry(), Rect::new(10, 10, 200, 30));
    }

    #[test]
    fn spinbox_visibility() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn spinbox_enabled() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert!(sb.is_enabled());
        sb.set_enabled(false);
        assert!(!sb.is_enabled());
        sb.set_enabled(true);
        assert!(sb.is_enabled());
    }

    #[test]
    fn spinbox_id_kind() {
        let sb_a = SpinBox::new(Rect::new(0, 0, 100, 24));
        let sb_b = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::SpinBox);
        assert_eq!(sb_b.kind(), WidgetKind::SpinBox);
    }

    #[test]
    fn spinbox_signal_accessors() {
        let sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        let _value_changed = &sb.value_changed;
        let _editing_finished = &sb.editing_finished;
    }

    // ── Decimal mode (BLUE20 layer 4c) ─────────────────────────────────────────

    /// The default must be an integer spin box, or every existing caller changes
    /// behaviour. This is the forward-compatibility half of the feature (principle #21).
    #[test]
    fn spinbox_defaults_to_integer_precision() {
        let sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_eq!(sb.decimals(), 0);
        assert_eq!(sb.formatted_value(), "0", "integer mode must print no decimals");
    }

    /// The round trip the task names: `set_decimals(2)` then `set_value(1.5)`.
    #[test]
    fn spinbox_decimal_round_trip() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(2);
        sb.set_value_f64(1.5);
        assert_eq!(sb.value_f64(), 1.5);
        assert_eq!(sb.value(), 2, "the integer reading rounds, it does not truncate");
        assert_eq!(sb.formatted_value(), "1.50", "a two-decimal box shows two decimals");
    }

    /// Steps in decimal mode move by the decimal step, not by one.
    #[test]
    fn spinbox_decimal_step() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(2);
        sb.set_single_step_f64(0.25);
        sb.set_value_f64(1.0);
        sb.step_up();
        assert_eq!(sb.value_f64(), 1.25);
        sb.step_down();
        sb.step_down();
        assert_eq!(sb.value_f64(), 0.75);
    }

    /// A step smaller than one displayed unit would make the buttons look broken, so
    /// the floor is the unit of the last displayed place — not a hardcoded `1`.
    #[test]
    fn spinbox_decimal_step_floor_is_the_last_place() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(2);
        sb.set_single_step_f64(0.0);
        assert_eq!(sb.single_step_f64(), 0.01);
        sb.set_single_step_f64(-0.5);
        assert_eq!(sb.single_step_f64(), 0.5, "a negative step is a magnitude, not a direction");
    }

    /// Changing the precision must re-round the stored value, or the display and the
    /// value disagree and the control reports digits the user could not have entered.
    #[test]
    fn spinbox_set_decimals_rerounds_the_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(4);
        sb.set_value_f64(1.23456);
        assert_eq!(sb.value_f64(), 1.2346);
        sb.set_decimals(2);
        assert_eq!(sb.value_f64(), 1.23);
        sb.set_decimals(0);
        assert_eq!(sb.value_f64(), 1.0);
    }

    /// Storing and displaying must agree to the digit. `2.0965` at three places is the
    /// case that separates the two implementations on this build: the scale form computes
    /// `2096.5000000000005` and rounds it to `2.097`, while a correctly-rounded decimal
    /// formatter prints `2.096`. Using the text as the authority is what keeps the stored
    /// value equal to the digits on screen.
    #[test]
    fn spinbox_stored_value_equals_the_printed_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(3);
        sb.set_value_f64(2.0965);
        assert_eq!(
            sb.formatted_value(),
            "2.096",
            "the printed digits must be the correctly-rounded ones"
        );
        assert_eq!(
            sb.value_f64(),
            2.096,
            "and the stored value must be exactly the number that was printed — the scale \
             form would store 2.097 here and disagree with its own display"
        );
    }

    /// A literal that is already below the midpoint must round **down**. This is the
    /// honest half of the rounding story: no scheme can recover digits the input never
    /// held, and `1.005` as an `f64` is `1.004999...`, so `1.00` is the correct answer —
    /// and the same one a decimal parser produces from the identical text.
    #[test]
    fn spinbox_rounds_a_below_midpoint_literal_down() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(2);
        sb.set_value_f64(1.005);
        assert_eq!(
            sb.formatted_value(),
            "1.00",
            "1.005 as an f64 is 1.004999...; 1.00 is the correct rounding of the value that was \
             actually stored, not a rounding failure"
        );
        assert_eq!(sb.value_f64(), 1.0);
        // The same text through a decimal parser lands on the same `f64`, which is the
        // proof that this is the input's limit and not the widget's rounding bug.
        let parsed: f64 = "1.005".parse().unwrap();
        assert_eq!(parsed, 1.005, "the literal and the parsed text must be the same f64");
    }

    /// `NaN` has no position in a range. Storing it would make every later comparison
    /// false and the control unreadable, so the write must be refused rather than kept.
    #[test]
    fn spinbox_rejects_nan_and_keeps_the_previous_value() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value_f64(3.0);
        sb.set_value_f64(f64::NAN);
        assert_eq!(sb.value_f64(), 3.0);
        sb.set_minimum_f64(f64::NAN);
        assert_eq!(sb.minimum_f64(), 0.0);
        sb.set_maximum_f64(f64::NAN);
        assert_eq!(sb.maximum_f64(), 99.0);
    }

    /// Infinity is a legitimate open bound, so it is clamped, not rejected.
    #[test]
    fn spinbox_infinite_bounds_are_a_valid_range() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_maximum_f64(f64::INFINITY);
        sb.set_range_f64(0.0, f64::INFINITY);
        sb.set_value_f64(1.0e9);
        assert_eq!(sb.value_f64(), 1.0e9);
    }

    /// Crossed bounds name the same range in either order. Storing an inverted pair
    /// would silently collapse the range to a single point.
    #[test]
    fn spinbox_crossed_decimal_bounds_are_ordered() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_range_f64(10.0, 0.0);
        assert_eq!(sb.minimum_f64(), 0.0);
        assert_eq!(sb.maximum_f64(), 10.0);
        sb.set_value_f64(20.0);
        assert_eq!(sb.value_f64(), 10.0);
    }

    /// `-0.0` formats as `-0`, which reads as a defect. Rounding to integers must
    /// normalise it — and `decimals == 0` is exactly where the sign can be lost.
    #[test]
    fn spinbox_negative_zero_rounds_to_zero() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_minimum(-5);
        sb.set_value_f64(-0.4);
        assert_eq!(sb.value_f64(), 0.0);
        assert_eq!(sb.formatted_value(), "0", "a zero must not be printed as `-0`");
    }

    /// The property route must stay integer-typed until decimals are enabled, and
    /// report `Float` afterwards — the declaration says `Number`, and this is what
    /// that word means at runtime.
    #[test]
    fn spinbox_value_property_switches_carrier_with_decimals() {
        use crate::widget::capability::WidgetProperties;
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        assert_eq!(sb.get("value").unwrap(), CapabilityValue::Int(0));
        assert_eq!(sb.set("value", CapabilityValue::Int(7)), Ok(()));
        assert_eq!(sb.get("value").unwrap(), CapabilityValue::Int(7));

        sb.set("decimals", CapabilityValue::Int(2)).unwrap();
        assert_eq!(sb.get("decimals").unwrap(), CapabilityValue::Int(2));
        // A decimal write is accepted and read back as a Float.
        assert_eq!(sb.set("value", CapabilityValue::Float(2.5)), Ok(()));
        assert_eq!(sb.get("value").unwrap(), CapabilityValue::Float(2.5));
        // An integer write still works, which is what keeps `{"value": 3}` valid.
        assert_eq!(sb.set("value", CapabilityValue::Int(3)), Ok(()));
        assert_eq!(sb.get("value").unwrap(), CapabilityValue::Float(3.0));
    }

    /// A precision beyond what the display grammar can print is clamped, not accepted:
    /// the digits past that point are binary noise the widget does not hold.
    #[test]
    fn spinbox_decimals_are_bounded_and_negative_is_refused() {
        use crate::widget::capability::WidgetProperties;
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(99);
        assert_eq!(sb.decimals(), SPIN_BOX_MAX_DECIMALS);
        assert_eq!(
            sb.set("decimals", CapabilityValue::Int(-1)),
            Err(CapabilityAccessError::OutOfRange),
            "a negative precision is a value error, not a silently accepted magnitude"
        );
        assert_eq!(sb.decimals(), SPIN_BOX_MAX_DECIMALS, "the refused write must not take effect");
    }

    /// The width must follow the formatted text; sizing off the integer form clips
    /// exactly the decimals the caller asked to display.
    ///
    /// # What this pins, and why the range is widened first
    ///
    /// The box starts with the default `0..=99`, and `set_decimals` re-clamps the value to
    /// that range — so the old form of this test (`set_value(1234)` then `set_decimals(2)`)
    /// never measured `1234.00` at all: it measured `99` against `99.00`, whose two
    /// characters are both under the hint's floor and therefore both produce the same width.
    /// The comparison passed only because the pre-migration formula had no floor to reach.
    /// The range is widened so the value survives the precision change and the assertion
    /// measures the thing its message claims: the formatted text driving the width.
    #[test]
    fn spinbox_size_hint_accounts_for_decimals() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_range(0, 1_000_000);
        sb.set_value(1234);
        assert_eq!(sb.formatted_value(), "1234", "the value must survive the setup");
        let integer_width = sb.size_hint().width;
        sb.set_decimals(2);
        assert_eq!(sb.formatted_value(), "1234.00", "two more digits than the integer form");
        assert!(
            sb.size_hint().width > integer_width,
            "a two-decimal box showing 1234.00 must be wider than one showing 1234"
        );
    }

    /// The reported size and the painted band are two readings of one formula.
    ///
    /// # Why this exists
    ///
    /// This is BLUE22 §B.6 rule 3: a composite's own `size_hint` must be derived through
    /// `ControlMetrics::implicit_size` over a floor from the `dimensions` table, so the drawn
    /// box and the reported size agree. Before the migration the hint was
    /// `Size::new(val_w.max(60), 24)` — a free-standing literal — while `draw` painted
    /// `geometry()`: the reported 24 px height and the 120 px slab on screen were two
    /// different descriptions of the same control.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let sb = SpinBox::new(Rect::new(0, 0, 240, 120));
        let band = sb.row_band();
        assert_eq!(
            band.height,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
            "the painted band is the field's own height, whatever rectangle the control was given"
        );
        assert_eq!(
            sb.size_hint().height,
            band.height,
            "a layout must be told the height the control actually draws"
        );
        // Centred in the area it was given, not stretched across it.
        assert_eq!(band.width, 240);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    /// The value can never be laid over the step column: the two tile the band.
    ///
    /// # Why this exists
    ///
    /// This is BLUE22 §B.9 — a sub-part's box is derived from its siblings. The buttons used
    /// to sit at `geometry().right() - 20 * 2` while the text was written from the field's
    /// leading edge with no upper bound, so a wider button or a larger font put the value
    /// *underneath* the buttons. Deriving the editable box as "whatever the column leaves"
    /// makes that overlap unrepresentable.
    #[test]
    fn the_value_box_ends_where_the_button_column_begins() {
        // Widths at which both boxes are representable: the column is
        // `SPIN_BOX_BUTTON_WIDTH * SPIN_BOX_BUTTONS` (40) **plus the value column's own floor**, and
        // a band narrower than that cannot hold both — see
        // `a_band_narrower_than_the_step_column_overhangs`.
        let narrowest =
            (SPIN_BOX_BUTTON_WIDTH * SPIN_BOX_BUTTONS) + dimensions::TEXT_FIELD_PADDING_H;
        for width in [narrowest, 64, 240, 400] {
            let sb = SpinBox::new(Rect::new(0, 0, width, 120));
            let editable = sb.editable_rect();
            let column = sb.button_column();
            assert_eq!(
                editable.x + editable.width as i32,
                column.x,
                "the two boxes must share an edge at control width {width}"
            );
            assert_eq!(
                editable.width + column.width,
                sb.row_band().width,
                "the two boxes must tile the band at control width {width}"
            );
            assert!(
                column.x + column.width as i32 <= sb.row_band().x + sb.row_band().width as i32,
                "the button column must stay inside the band at control width {width}"
            );
        }
    }

    /// A band narrower than the step column keeps both columns inside it.
    ///
    /// # What this pins (and what G-1 changed)
    ///
    /// The step column is 40 px (two 20 px buttons) and the value column's floor is the field's own
    /// padding, so a 30 px band cannot hold both at those sizes — arithmetic, not a bug.
    ///
    /// Before G-1 was resolved the value column kept its floor and the step column was pushed past
    /// the band's trailing edge, which on the SVG backend means **no step buttons in the picture at
    /// all** rather than step buttons that overflow. The teardown of a numeric field that silently
    /// loses its steppers is exactly the kind of loss that is worse than a compressed control, so
    /// the assertion is containment.
    ///
    /// The case is not reachable from a real form: a spin box's `size_hint` floor is the whole
    /// column plus twice the field padding, which is wider than this band.
    #[test]
    fn a_band_narrower_than_the_step_column_keeps_both_inside_it() {
        let width = 30u32;
        let sb = SpinBox::new(Rect::new(0, 0, width, 120));
        let band = sb.row_band();
        let value = sb.editable_rect();
        let column = sb.button_column();
        for (label, rect) in [("value", value), ("step column", column)] {
            assert!(
                rect.x >= band.x && rect.x + rect.width as i32 <= band.x + band.width as i32,
                "the {label} must stay inside the band: {rect:?} in {band:?}"
            );
            assert!(rect.width > 0, "and neither is dropped: the {label} is {rect:?}");
        }
        // They still tile the band: the value yields to the column, which is the control's relation.
        assert_eq!(value.x + value.width as i32, column.x, "the two boxes share an edge");
        assert_eq!(value.width + column.width, width, "and they account for the band");
    }

    /// The two step buttons tile the column and never overlap each other.
    #[test]
    fn the_step_buttons_tile_their_column() {
        for height in [0u32, 10, 48, 120] {
            let sb = SpinBox::new(Rect::new(0, 0, 200, height));
            let up = sb.up_button();
            let down = sb.down_button();
            assert_eq!(up.x, down.x, "both buttons occupy the one column at height {height}");
            assert_eq!(up.width, down.width);
            assert_eq!(
                up.y + up.height as i32,
                down.y,
                "the up button must end where the down button begins at height {height}"
            );
            assert_eq!(
                up.height + down.height,
                sb.button_column().height,
                "the two buttons must tile the column at height {height}"
            );
        }
    }

    /// A press lands on the step the *pointed-at* button performs.
    ///
    /// The hit test used to re-derive the buttons from a `button_width` argument of its own,
    /// so the clickable column and the painted one were two independent derivations of the
    /// same edge. This drives the press through the same accessors the paint path uses.
    #[test]
    fn a_press_on_each_half_steps_in_the_direction_that_is_painted() {
        let rect = Rect::new(0, 0, 200, 120);
        let top = SpinBox::new(rect).up_button();
        let bottom = SpinBox::new(rect).down_button();

        let mut sb = SpinBox::new(rect);
        sb.set_value(50);
        sb.handle_event(&Event::MousePress { pos: Point::new(top.x + 1, top.y + 1), button: 1 });
        assert_eq!(sb.value(), 51, "the upper half of the column must step up");

        sb.handle_event(&Event::MousePress {
            pos: Point::new(bottom.x + 1, bottom.y + 1),
            button: 1,
        });
        assert_eq!(sb.value(), 50, "the lower half of the column must step down");
    }

    /// A wider step column pushes the value box narrower rather than overlapping it.
    ///
    /// # What this pins
    ///
    /// This is the sibling-column relation — `leftPadding: padding + down.width` — stated as a
    /// *test*: the value's box is derived from the column rather than from the band's far edge, so a
    /// wider column must move the value's trailing edge. The relation was previously satisfied by
    /// two independent subtractions from two different edges, which is why a wider button or a
    /// larger font ran the value underneath the buttons instead of narrowing it.
    #[test]
    fn a_wider_step_column_pushes_the_value_box() {
        let width = 240u32;
        let band_width = ControlMetrics::full_width_band(
            Rect::new(0, 0, width, 120),
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        )
        .width;
        // Two step buttons of the shared width: the column the assembly derives.
        let column = SPIN_BOX_BUTTON_WIDTH * SPIN_BOX_BUTTONS;
        let sb = SpinBox::new(Rect::new(0, 0, width, 120));
        let editable = sb.editable_rect();
        assert_eq!(
            editable.width,
            band_width - column,
            "the value box is the band minus the column, not minus a constant from its own edge"
        );
        assert_eq!(
            editable.width + sb.button_column().width,
            band_width,
            "the two boxes consume the band between them"
        );
    }

    /// A press on the field itself is not a step.
    #[test]
    fn a_press_inside_the_value_box_does_not_step() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 200, 120));
        sb.set_value(50);
        let editable = sb.editable_rect();
        sb.handle_event(&Event::MousePress {
            pos: Point::new(editable.x + 1, editable.y + editable.height as i32 / 2),
            button: 1,
        });
        assert_eq!(sb.value(), 50, "the text area is where the user types, not where they step");
    }

    /// Setting the precision re-rounds the value but must not claim the value changed:
    /// no step was taken, so a caller counting `value_changed` would see a phantom one.
    #[test]
    fn spinbox_set_decimals_does_not_emit_value_changed() {
        use crate::compat::Arc;
        use core::sync::atomic::{AtomicU32, Ordering};
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_decimals(3);
        sb.set_value_f64(1.2345);
        // `Signal1::connect` requires `Send + Sync + 'static`, so the counter is an
        // atomic behind an `Arc` rather than an `Rc<Cell<..>>`.
        let seen = Arc::new(AtomicU32::new(0));
        let counter = seen.clone();
        sb.value_changed.connect(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        sb.set_decimals(1);
        assert_eq!(
            seen.load(Ordering::SeqCst),
            0,
            "re-rounding on a precision change is not a value change"
        );
        assert_eq!(sb.value_f64(), 1.2);
        sb.step_up();
        assert_eq!(seen.load(Ordering::SeqCst), 1, "a real step still emits");
    }
}
