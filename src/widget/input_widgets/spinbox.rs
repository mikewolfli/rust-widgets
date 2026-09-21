// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Spin box widget for numeric input.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{expect_bool, expect_f64, expect_i64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_f64;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Upper bound on [`SpinBox::decimals`], matching the widest precision the default
/// display grammar can print. A larger precision would render digits that are pure
/// binary noise, which is worse than refusing the setting.
pub const SPIN_BOX_MAX_DECIMALS: u32 = 9;

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
    fn display_text(&self) -> String {
        if let Some(special) = &self.special_value_text {
            if self.value == self.minimum {
                return special.clone();
            }
        }
        format!("{}{}{}", self.prefix, self.formatted_value(), self.suffix)
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
        // `1234.50` is six characters wider than one showing `1234`, and sizing off the
        // integer form would clip the decimals it was explicitly asked to display.
        let val_w = self.formatted_value().len() as u32 * 10 + 25;
        Size::new(val_w.max(60), 24)
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
    /// Handle click/tap on increment/decrement buttons.
    fn handle_button_click(&mut self, pos: &Point, rect: Rect, button_width: u32) {
        if pos.x as f32 >= rect.x as f32 + rect.width as f32 - button_width as f32 * 2.0 {
            if (pos.x as f32) < rect.x as f32 + rect.width as f32 - button_width as f32 {
                self.step_down();
            } else {
                self.step_up();
            }
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
                let rect = self.geometry();
                let button_width = 20;
                if *button == 1 {
                    self.handle_button_click(pos, rect, button_width);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                let rect = self.geometry();
                let button_width = 20;
                self.handle_button_click(pos, rect, button_width);
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
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let button_width = 20;
        let text_x = rect.x + padding;
        let text_y = rect.y as f32 + rect.height as f32 / 2.0;
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
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
        }
        // Draw up/down buttons
        let down_button_x_f = rect.x as f32 + rect.width as f32 - button_width as f32 * 2.0;
        let up_button_x_f = rect.x as f32 + rect.width as f32 - button_width as f32;
        let button_width_f = button_width as f32;
        let rect_y_f = rect.y as f32;
        let rect_height_f = rect.height as f32;
        // Down button
        context.fill_rect(
            Rect::from_f32(down_button_x_f, rect_y_f, button_width_f, rect_height_f),
            button_bg,
        );
        context.draw_rect(
            Rect::from_f32(down_button_x_f, rect_y_f, button_width_f, rect_height_f),
            button_border,
        );
        // Down arrow
        let down_arrow_x_f = down_button_x_f + button_width_f / 2.0;
        let down_arrow_y_f = rect_y_f + rect_height_f / 2.0;
        let arrow_size = 4;
        let arrow_size_f = arrow_size as f32;
        context.draw_line(
            Point::from_f32(down_arrow_x_f - arrow_size_f, down_arrow_y_f - arrow_size_f / 2.0),
            Point::from_f32(down_arrow_x_f + arrow_size_f, down_arrow_y_f - arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(down_arrow_x_f + arrow_size_f, down_arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(down_arrow_x_f, down_arrow_y_f + arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(down_arrow_x_f, down_arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(down_arrow_x_f - arrow_size_f, down_arrow_y_f - arrow_size_f / 2.0),
            arrow_color,
        );
        // Up button
        context.fill_rect(
            Rect::from_f32(up_button_x_f, rect_y_f, button_width_f, rect_height_f),
            button_bg,
        );
        context.draw_rect(
            Rect::from_f32(up_button_x_f, rect_y_f, button_width_f, rect_height_f),
            button_border,
        );
        // Up arrow
        let up_arrow_x_f = up_button_x_f + button_width_f / 2.0;
        let up_arrow_y_f = rect_y_f + rect_height_f / 2.0;
        context.draw_line(
            Point::from_f32(up_arrow_x_f - arrow_size_f, up_arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(up_arrow_x_f + arrow_size_f, up_arrow_y_f + arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(up_arrow_x_f + arrow_size_f, up_arrow_y_f + arrow_size_f / 2.0),
            Point::from_f32(up_arrow_x_f, up_arrow_y_f - arrow_size_f / 2.0),
            arrow_color,
        );
        context.draw_line(
            Point::from_f32(up_arrow_x_f, up_arrow_y_f - arrow_size_f / 2.0),
            Point::from_f32(up_arrow_x_f - arrow_size_f, up_arrow_y_f + arrow_size_f / 2.0),
            arrow_color,
        );
        // Draw text
        let display_text = self.display_text();
        if !display_text.is_empty() {
            context.draw_text(
                Point::new(text_x, text_y as i32),
                &display_text,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
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
    /// exactly the decimals the caller asked for.
    #[test]
    fn spinbox_size_hint_accounts_for_decimals() {
        let mut sb = SpinBox::new(Rect::new(0, 0, 100, 24));
        sb.set_value(1234);
        let integer_width = sb.size_hint().width;
        sb.set_decimals(2);
        assert!(
            sb.size_hint().width > integer_width,
            "a two-decimal box showing 1234.00 must be wider than one showing 1234"
        );
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
