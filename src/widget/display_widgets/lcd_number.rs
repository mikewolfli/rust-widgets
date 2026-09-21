// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_f64, expect_i64, expect_lcd_mode, expect_segment_style, lcd_mode_to_str,
    segment_style_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_f64;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// LCD number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LCDNumberMode {
    /// Display hexadecimal numbers.
    Hex,
    /// Display decimal numbers.
    Dec,
    /// Display octal numbers.
    Oct,
    /// Display binary numbers.
    Bin,
}
/// LCD segment style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentStyle {
    /// Outline style.
    Outline,
    /// Filled style.
    Filled,
    /// Flat style.
    Flat,
}
/// LCD number widget.
///
/// Displays a single floating-point or integer value in a segmented style, in
/// one of four radices. The widget holds the number; the base-2/8/16 rendering
/// truncates the value to an `i64`, so fractional parts are dropped — and, for
/// values outside the `i64` range, the cast saturates rather than wrapping.
///
pub struct LCDNumber {
    base: BaseWidget,
    value: f64,
    min_value: f64,
    max_value: f64,
    num_digits: i32,
    small_decimal_point: bool,
    mode: LCDNumberMode,
    segment_style: SegmentStyle,
    /// Set when the most recent [`LCDNumber::set_value`] call received a value
    /// outside `min_value ..= max_value`; cleared by the next in-range set.
    /// Mirrors [`LCDNumber::check_overflow`].
    overflowed: bool,
    /// Emitted with the new value whenever [`LCDNumber::set_value`] actually
    /// changes it. Emits the *clamped* value, not the argument.
    pub value_changed: Signal1<f64>,
    /// Emitted when a value supplied to [`LCDNumber::set_value`] lies outside
    /// `min_value ..= max_value` (an overflow attempt). The stored value is
    /// still clamped into range; this signal notifies listeners that the
    /// requested magnitude could not be represented.
    pub overflow: GenericSignal,
}
impl LCDNumber {
    /// Creates a decimal display showing `0.0`, with the range
    /// `-999999.0 ..= 999999.0`, six digits, a normal-size decimal point, and
    /// the filled segment style.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is 80x30.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LCDNumber, geometry, "LCDNumber"),
            value: 0.0,
            min_value: -999999.0,
            max_value: 999999.0,
            num_digits: 6,
            small_decimal_point: false,
            mode: LCDNumberMode::Dec,
            segment_style: SegmentStyle::Filled,
            overflowed: false,
            value_changed: Signal1::new(),
            overflow: GenericSignal::new(),
        }
    }
    /// Returns the displayed value, always inside `min_value ..= max_value`.
    pub fn value(&self) -> f64 {
        self.value
    }
    /// Returns the lower display bound. Defaults to `-999999.0`.
    pub fn min_value(&self) -> f64 {
        self.min_value
    }
    /// Returns the upper display bound. Defaults to `999999.0`.
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
    /// Returns the configured digit count, used to size the display. Always at
    /// least `1`; defaults to `6`.
    pub fn num_digits(&self) -> i32 {
        self.num_digits
    }
    /// Returns whether a reduced-size decimal point is used. Defaults to
    /// `false`.
    pub fn is_small_decimal_point(&self) -> bool {
        self.small_decimal_point
    }
    /// Returns the display radix. Defaults to [`LCDNumberMode::Dec`].
    pub fn mode(&self) -> LCDNumberMode {
        self.mode
    }
    /// Returns the segment rendering style. Defaults to
    /// [`SegmentStyle::Filled`].
    pub fn segment_style(&self) -> SegmentStyle {
        self.segment_style
    }
    /// Sets the displayed value, clamped into `min_value ..= max_value`.
    ///
    /// When `value` is outside that range the stored value is clamped, the
    /// `overflow` signal is emitted, and [`LCDNumber::check_overflow`] reports
    /// `true` until the next in-range set clears it. When the clamped value is
    /// unchanged from the current value this is a no-op (no signal, no redraw).
    ///
    /// Both `overflow` and `value_changed` are deliberately not gated by `enabled`:
    /// `overflow` is a *diagnostic* about data that arrived from outside the control, and
    /// suppressing it while disabled would hide exactly the condition a host most needs to
    /// see. This control handles no input of its own.
    pub fn set_value(&mut self, value: f64) {
        let out_of_range = value < self.min_value || value > self.max_value;
        let clamped = ordered_clamp_f64(value, self.min_value, self.max_value);
        if out_of_range {
            self.overflowed = true;
            self.overflow.emit();
        } else {
            self.overflowed = false;
        }
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
        }
        self.base.request_redraw();
    }
    /// Sets the lower bound and re-applies it to the current value through
    /// [`LCDNumber::set_value`], so the value is clamped into the new range.
    ///
    /// Setting `min` above `max` produces an inverted range; `f64::clamp`
    /// panics in that case, so keep the bounds ordered (use
    /// [`LCDNumber::set_max_value`] first when raising both).
    pub fn set_min_value(&mut self, min: f64) {
        self.min_value = min;
        self.set_value(self.value);
    }
    /// Sets the upper bound and re-applies it to the current value through
    /// [`LCDNumber::set_value`]. See [`LCDNumber::set_min_value`] for the
    /// inverted-range caveat.
    pub fn set_max_value(&mut self, max: f64) {
        self.max_value = max;
        self.set_value(self.value);
    }
    /// Sets the digit count, floored at `1` so the display is never zero-width.
    /// Requests a redraw. The value itself is not re-clamped or truncated.
    pub fn set_num_digits(&mut self, digits: i32) {
        self.num_digits = digits.max(1);
        self.base.request_redraw();
    }
    /// Chooses between a reduced-size and a normal-size decimal point.
    /// Requests a redraw.
    pub fn set_small_decimal_point(&mut self, small: bool) {
        self.small_decimal_point = small;
        self.base.request_redraw();
    }
    /// Sets the display radix. Requests a redraw. Changing the mode does not
    /// change the stored value, only how it is rendered.
    pub fn set_mode(&mut self, mode: LCDNumberMode) {
        self.mode = mode;
        self.base.request_redraw();
    }
    /// Sets the segment rendering style. Requests a redraw.
    pub fn set_segment_style(&mut self, style: SegmentStyle) {
        self.segment_style = style;
        self.base.request_redraw();
    }
    /// Returns whether the most recent value supplied to
    /// [`LCDNumber::set_value`] overflowed `min_value ..= max_value`.
    ///
    /// This is sticky: it stays `true` after an out-of-range set until an
    /// in-range set clears it. It is the same state the `overflow` signal
    /// announces and that [`LCDNumber::draw`] renders as an overflow indicator.
    pub fn check_overflow(&self) -> bool {
        self.overflowed
    }
    /// Renders the value as text for the current mode, without any size or
    /// digit-count padding.
    ///
    /// [`LCDNumberMode::Dec`] uses the `Display` representation of the `f64`
    /// (so very large or small magnitudes may appear in exponential notation),
    /// and always includes a fractional part (for example `"3"` renders as
    /// `"3"` but `3.5` as `"3.5"`). The other three modes truncate to `i64`
    /// first, dropping any fraction.
    pub fn display_text(&self) -> String {
        match self.mode {
            LCDNumberMode::Hex => format!("{:X}", self.value as i64),
            LCDNumberMode::Dec => format!("{}", self.value),
            LCDNumberMode::Oct => format!("{:o}", self.value as i64),
            LCDNumberMode::Bin => format!("{:b}", self.value as i64),
        }
    }
}
impl Widget for LCDNumber {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(80, 30)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `LCDNumber`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `Int`/`Float` value shapes and the `i32` truncation `num_digits`
/// performed.
impl WidgetProperties for LCDNumber {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(self.value())),
            "min_value" => Ok(CapabilityValue::Float(self.min_value())),
            "max_value" => Ok(CapabilityValue::Float(self.max_value())),
            "num_digits" => Ok(CapabilityValue::Int(self.num_digits() as i64)),
            "small_decimal_point" => Ok(CapabilityValue::Bool(self.is_small_decimal_point())),
            "mode" => Ok(CapabilityValue::String(lcd_mode_to_str(self.mode()).to_string())),
            "segment_style" => {
                Ok(CapabilityValue::String(segment_style_to_str(self.segment_style()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_f64(value)?);
                Ok(())
            }
            "min_value" => {
                self.set_min_value(expect_f64(value)?);
                Ok(())
            }
            "max_value" => {
                self.set_max_value(expect_f64(value)?);
                Ok(())
            }
            "num_digits" => {
                self.set_num_digits(expect_i64(value)? as i32);
                Ok(())
            }
            "small_decimal_point" => {
                self.set_small_decimal_point(expect_bool(value)?);
                Ok(())
            }
            "mode" => {
                self.set_mode(expect_lcd_mode(value)?);
                Ok(())
            }
            "segment_style" => {
                self.set_segment_style(expect_segment_style(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "value",
            "min_value",
            "max_value",
            "num_digits",
            "small_decimal_point",
            "mode",
            "segment_style",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `lcd_number` publishes.
    ///
    /// All three assign state — a number, a display mode and a segment style — so
    /// each needs an argument a command carries none of. They are refused as
    /// [`CapabilityAccessError::OutOfRange`] (use the property route
    /// `set("value", ..)` / `set("mode", ..)` / `set("segment_style", ..)`) rather
    /// than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_mode" | "set_segment_style" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
impl EventHandler for LCDNumber {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for LCDNumber {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::BLACK);
        let fg_color = style.text_color.unwrap_or(Color::rgb(255, 0, 0));
        context.fill_rect(rect, bg_color);
        let display_text = self.display_text();
        let digit_width = rect.width / (self.num_digits as f64).max(1.0) as u32;
        let digit_height = rect.height * 7 / 10;
        let segment_width = digit_width / 8;
        let start_x =
            rect.x + ((rect.width as i32 - digit_width as i32 * display_text.len() as i32) / 2);
        let start_y = rect.y + ((rect.height as i32 - digit_height as i32) / 2);
        for (i, ch) in display_text.chars().enumerate() {
            let digit_x = (start_x + i as i32 * digit_width as i32) as u32;
            let digit_y = start_y as u32;
            self.draw_digit(
                context,
                ch,
                digit_x,
                digit_y,
                digit_width,
                digit_height,
                segment_width,
                fg_color,
            );
        }
        if self.check_overflow() {
            let overflow_color = Color::rgb(255, 255, 0);
            context.fill_circle(Point::new(rect.x + 10, rect.y + 10), 5, overflow_color);
        }
    }
}

impl LCDNumber {
    #[allow(clippy::too_many_arguments)]
    fn draw_digit(
        &self,
        context: &mut RenderContext,
        ch: char,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        segment_width: u32,
        color: Color,
    ) {
        let segments = self.get_segments(ch);
        let hw = (segment_width / 2) as i32;
        // let mid_x = x as i32 + width as i32 / 2;
        let mid_y = y as i32 + height as i32 / 2;
        let top_y = y as i32;
        let bottom_y = y as i32 + height as i32;
        let left_x = x as i32;
        let right_x = x as i32 + width as i32;
        if segments[0] {
            self.draw_horizontal_segment(
                context,
                left_x + hw,
                top_y,
                right_x - hw,
                top_y + segment_width as i32,
                color,
            );
        }
        if segments[1] {
            self.draw_vertical_segment(
                context,
                right_x - segment_width as i32,
                top_y + hw,
                right_x,
                mid_y - hw,
                color,
            );
        }
        if segments[2] {
            self.draw_vertical_segment(
                context,
                right_x - segment_width as i32,
                mid_y + hw,
                right_x,
                bottom_y - hw,
                color,
            );
        }
        if segments[3] {
            self.draw_horizontal_segment(
                context,
                left_x + hw,
                bottom_y - segment_width as i32,
                right_x - hw,
                bottom_y,
                color,
            );
        }
        if segments[4] {
            self.draw_vertical_segment(
                context,
                left_x,
                mid_y + hw,
                left_x + segment_width as i32,
                bottom_y - hw,
                color,
            );
        }
        if segments[5] {
            self.draw_vertical_segment(
                context,
                left_x,
                top_y + hw,
                left_x + segment_width as i32,
                mid_y - hw,
                color,
            );
        }
        if segments[6] {
            self.draw_horizontal_segment(
                context,
                left_x + hw,
                mid_y - hw / 2,
                right_x - hw,
                mid_y + hw / 2,
                color,
            );
        }
    }
    fn draw_horizontal_segment(
        &self,
        context: &mut RenderContext,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    ) {
        let width = (x2 - x1).max(1) as u32;
        let height = (y2 - y1).max(1) as u32;
        match self.segment_style {
            SegmentStyle::Outline => {
                context.draw_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Filled => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Flat => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
        }
    }
    fn draw_vertical_segment(
        &self,
        context: &mut RenderContext,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    ) {
        let width = (x2 - x1).max(1) as u32;
        let height = (y2 - y1).max(1) as u32;
        match self.segment_style {
            SegmentStyle::Outline => {
                context.draw_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Filled => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
            SegmentStyle::Flat => {
                context.fill_rect(Rect::new(x1, y1, width, height), color);
            }
        }
    }
    fn get_segments(&self, ch: char) -> [bool; 7] {
        match ch.to_ascii_uppercase() {
            '0' => [true, true, true, true, true, true, false],
            '1' => [false, true, true, false, false, false, false],
            '2' => [true, true, false, true, true, false, true],
            '3' => [true, true, true, true, false, false, true],
            '4' => [false, true, true, false, false, true, true],
            '5' => [true, false, true, true, false, true, true],
            '6' => [true, false, true, true, true, true, true],
            '7' => [true, true, true, false, false, false, false],
            '8' => [true, true, true, true, true, true, true],
            '9' => [true, true, true, true, false, true, true],
            'A' => [true, true, true, false, true, true, true],
            'B' => [false, false, true, true, true, true, true],
            'C' => [true, false, false, true, true, true, false],
            'D' => [false, true, true, true, true, false, true],
            'E' => [true, false, false, true, true, true, true],
            'F' => [true, false, false, false, true, true, true],
            '-' => [false, false, false, false, false, false, true],
            '.' => [false, false, false, false, false, false, false],
            ' ' => [false, false, false, false, false, false, false],
            _ => [false, false, false, false, false, false, false],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn lcd_creation_defaults() {
        let lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        assert!((lcd.value() - 0.0).abs() < f64::EPSILON);
        assert!((lcd.min_value() - (-999999.0)).abs() < f64::EPSILON);
        assert!((lcd.max_value() - 999999.0).abs() < f64::EPSILON);
        assert_eq!(lcd.num_digits(), 6);
        assert!(!lcd.is_small_decimal_point());
        assert_eq!(lcd.mode(), LCDNumberMode::Dec);
        assert_eq!(lcd.segment_style(), SegmentStyle::Filled);
        assert!(!lcd.check_overflow());
    }

    #[test]
    fn lcd_set_value() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_value(42.5);
        assert!((lcd.value() - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lcd_set_value_clamps() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_value(9999999.0);
        assert!((lcd.value() - 999999.0).abs() < f64::EPSILON);
        lcd.set_value(-9999999.0);
        assert!((lcd.value() - (-999999.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn lcd_set_min_max_value() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_min_value(-100.0);
        assert!((lcd.min_value() - (-100.0)).abs() < f64::EPSILON);
        lcd.set_max_value(500.0);
        assert!((lcd.max_value() - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lcd_set_num_digits() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_num_digits(4);
        assert_eq!(lcd.num_digits(), 4);
        lcd.set_num_digits(0); // floors at 1
        assert_eq!(lcd.num_digits(), 1);
    }

    #[test]
    fn lcd_small_decimal_point() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        assert!(!lcd.is_small_decimal_point());
        lcd.set_small_decimal_point(true);
        assert!(lcd.is_small_decimal_point());
        lcd.set_small_decimal_point(false);
        assert!(!lcd.is_small_decimal_point());
    }

    #[test]
    fn lcd_mode_roundtrip() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_mode(LCDNumberMode::Hex);
        assert_eq!(lcd.mode(), LCDNumberMode::Hex);
        lcd.set_mode(LCDNumberMode::Oct);
        assert_eq!(lcd.mode(), LCDNumberMode::Oct);
        lcd.set_mode(LCDNumberMode::Bin);
        assert_eq!(lcd.mode(), LCDNumberMode::Bin);
        lcd.set_mode(LCDNumberMode::Dec);
        assert_eq!(lcd.mode(), LCDNumberMode::Dec);
    }

    #[test]
    fn lcd_segment_style_roundtrip() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_segment_style(SegmentStyle::Outline);
        assert_eq!(lcd.segment_style(), SegmentStyle::Outline);
        lcd.set_segment_style(SegmentStyle::Flat);
        assert_eq!(lcd.segment_style(), SegmentStyle::Flat);
        lcd.set_segment_style(SegmentStyle::Filled);
        assert_eq!(lcd.segment_style(), SegmentStyle::Filled);
    }

    #[test]
    fn lcd_display_text_dec() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_value(1234.0);
        assert_eq!(lcd.display_text(), "1234");
    }

    #[test]
    fn lcd_display_text_hex() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_mode(LCDNumberMode::Hex);
        lcd.set_value(255.0);
        assert_eq!(lcd.display_text(), "FF");
    }

    #[test]
    fn lcd_display_text_oct() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_mode(LCDNumberMode::Oct);
        lcd.set_value(64.0);
        assert_eq!(lcd.display_text(), "100");
    }

    #[test]
    fn lcd_display_text_bin() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_mode(LCDNumberMode::Bin);
        lcd.set_value(5.0);
        assert_eq!(lcd.display_text(), "101");
    }

    #[test]
    fn lcd_overflow_detection() {
        let lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        // Freshly constructed, no overflow has been requested.
        assert!(!lcd.check_overflow());

        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_value(500000.0);
        assert!(!lcd.check_overflow()); // value is within default range
    }

    #[test]
    fn lcd_overflow_is_emitted_and_sticky() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        let emitted = Arc::new(AtomicUsize::new(0));
        let counter = emitted.clone();
        lcd.overflow.connect(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });

        // An out-of-range value clamps the display and raises overflow.
        lcd.set_value(9_999_999.0);
        assert!(lcd.check_overflow());
        assert_eq!(emitted.load(Ordering::SeqCst), 1);
        assert!((lcd.value() - 999_999.0).abs() < f64::EPSILON);

        // Sticky until the next in-range set clears it.
        lcd.set_value(9_999_999.0);
        assert!(lcd.check_overflow());
        assert_eq!(emitted.load(Ordering::SeqCst), 2);

        lcd.set_value(42.0);
        assert!(!lcd.check_overflow());
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn lcd_geometry_delegation() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        lcd.set_geometry(Rect::new(10, 10, 300, 60));
        assert_eq!(lcd.geometry(), Rect::new(10, 10, 300, 60));
    }

    #[test]
    fn lcd_visibility() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        assert!(lcd.is_visible());
        lcd.hide();
        assert!(!lcd.is_visible());
        lcd.show();
        assert!(lcd.is_visible());
    }

    #[test]
    fn lcd_enabled() {
        let mut lcd = LCDNumber::new(Rect::new(0, 0, 200, 50));
        assert!(lcd.is_enabled());
        lcd.set_enabled(false);
        assert!(!lcd.is_enabled());
        lcd.set_enabled(true);
        assert!(lcd.is_enabled());
    }

    #[test]
    fn lcd_id_kind() {
        let lcd_a = LCDNumber::new(Rect::new(0, 0, 100, 50));
        let lcd_b = LCDNumber::new(Rect::new(0, 0, 100, 50));
        assert_ne!(lcd_a.id(), lcd_b.id());
        assert_eq!(lcd_a.kind(), WidgetKind::LCDNumber);
        assert_eq!(lcd_b.kind(), WidgetKind::LCDNumber);
    }

    #[test]
    fn lcd_signal_accessors() {
        let lcd = LCDNumber::new(Rect::new(0, 0, 100, 50));
        let _value_changed = &lcd.value_changed;
        let _overflow = &lcd.overflow;
    }
}
