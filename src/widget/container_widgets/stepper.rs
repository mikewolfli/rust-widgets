// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stepper widget — a numeric increment/decrement control with +/- buttons.
//!
//! The Stepper widget displays a numeric value with minus (-) and plus (+)
//! buttons on either side for incrementing or decrementing the value.
//! It supports configurable minimum, maximum, step size, and emits a
//! `value_changed` signal whenever the value changes.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_i64;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Stepper widget for numeric increment/decrement with +/- buttons.
pub struct Stepper {
    base: BaseWidget,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    /// Emitted when the value changes.
    pub value_changed: Signal1<i32>,
}

impl Stepper {
    /// Creates a new Stepper widget with the given geometry.
    /// Default value is 0, min=0, max=100, step=1.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Stepper, geometry, "Stepper"),
            value: 0,
            min: 0,
            max: 100,
            step: 1,
            value_changed: Signal1::new(),
        }
    }

    /// Sets the current value, clamped to [min, max].
    /// Emits `value_changed` signal if the value actually changes.
    pub fn set_value(&mut self, value: i32) {
        let clamped = ordered_clamp_i32(value, self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Sets the minimum value (inclusive).
    pub fn set_min(&mut self, min: i32) {
        self.min = min.min(self.max);
        // Re-clamp current value to new bounds
        self.set_value(self.value);
    }

    /// Sets the maximum value (inclusive).
    pub fn set_max(&mut self, max: i32) {
        self.max = max.max(self.min);
        // Re-clamp current value to new bounds
        self.set_value(self.value);
    }

    /// Returns the minimum value (inclusive).
    pub fn min(&self) -> i32 {
        self.min
    }

    /// Returns the maximum value (inclusive).
    pub fn max(&self) -> i32 {
        self.max
    }

    /// Sets the step increment/decrement amount.
    pub fn set_step(&mut self, step: i32) {
        self.step = step.max(1);
    }

    /// Returns the step increment/decrement amount.
    pub fn step(&self) -> i32 {
        self.step
    }

    /// Increments the value by the step amount, clamped to max.
    pub fn increment(&mut self) {
        self.set_value(self.value.saturating_add(self.step));
    }

    /// Decrements the value by the step amount, clamped to min.
    pub fn decrement(&mut self) {
        self.set_value(self.value.saturating_sub(self.step));
    }

    /// The band the whole control is drawn in: full width, one button tall, centred.
    ///
    /// # Why the control is not its own rectangle
    ///
    /// A stepper is **one row of chrome**: a minus button, a value, a plus button.
    /// Every piece of it used to be measured from `rect`, so the 240x120 census cell
    /// drew a 240x120 pill two **118 px** buttons wide and a number floating between
    /// them — a column shaped like a stepper rather than a stepper. The band is centred
    /// and clamped to the control, so a stepper in a 30 px form row and one in a 120 px
    /// cell are the same object, and nothing is ever painted outside the given area.
    fn row_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::STEPPER_ROW_HEIGHT)
    }

    /// The minus button: the leading edge of the row band.
    fn minus_rect(&self) -> Rect {
        let band = self.row_band();
        let width = dimensions::STEPPER_BUTTON_WIDTH.min(band.width);
        let height = band.height.saturating_sub(dimensions::STEPPER_PADDING * 2);
        Rect::new(
            band.x + dimensions::STEPPER_PADDING as i32,
            band.y + dimensions::STEPPER_PADDING as i32,
            width,
            height,
        )
    }

    /// The plus button: the trailing edge of the row band.
    ///
    /// Derived from the same band as [`Self::minus_rect`], so the two cannot drift
    /// apart — they used to be two independent `rect.width - btn_width - 1`
    /// expressions in `draw` and again in the hit test.
    fn plus_rect(&self) -> Rect {
        let band = self.row_band();
        let width = dimensions::STEPPER_BUTTON_WIDTH.min(band.width);
        let height = band.height.saturating_sub(dimensions::STEPPER_PADDING * 2);
        Rect::new(
            band.x + band.width as i32 - width as i32 - dimensions::STEPPER_PADDING as i32,
            band.y + dimensions::STEPPER_PADDING as i32,
            width,
            height,
        )
    }
}

impl Widget for Stepper {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // One row of chrome: the two buttons and the value between them, at the row's
        // own fixed height. Reporting anything else would let a layout hand the control
        // a box its buttons do not fit in.
        Size::new(
            dimensions::STEPPER_BUTTON_WIDTH * 2 + dimensions::STEPPER_PADDING * 2,
            dimensions::STEPPER_ROW_HEIGHT,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Stepper`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before, including the `i32`
/// truncation and the `min`/`max` clamping the setters perform.
impl WidgetProperties for Stepper {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "minimum" => Ok(CapabilityValue::Int(self.min() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.max() as i64)),
            "step" => Ok(CapabilityValue::Int(self.step() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_i64(value)? as i32);
                Ok(())
            }
            "minimum" => {
                self.set_min(expect_i64(value)? as i32);
                Ok(())
            }
            "maximum" => {
                self.set_max(expect_i64(value)? as i32);
                Ok(())
            }
            "step" => {
                self.set_step(expect_i64(value)? as i32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["value", "minimum", "maximum", "step", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Stepper {
    fn draw(&mut self, context: &mut RenderContext) {
        let is_enabled = self.base.is_enabled();
        let value_text = self.value.to_string();
        let font = crate::core::Font::default_ui();
        let text_metrics = context.measure_text(&value_text, &font);

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("stepper");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A disabled control is a chrome state, so it reads a dimmed pair derived
        // from the resolved colours rather than a second literal grey.
        let bg_color = if !is_enabled { background.blend(&text_color, 0.06) } else { background };
        let button_color = background.blend(&text_color, 0.08);
        let disabled_button = button_color.blend(&background, 0.6);
        let disabled_text = text_color.blend(&background, 0.5);

        // ── The row band actually painted ──
        //
        // `rect` is the area the control was *given*; a stepper's own chrome is one row
        // of a fixed height. Painting the surface across the whole rectangle made a
        // 240x120 census cell a full-bleed pill whose buttons stood the control's full
        // height — the defect this replaces — and left the value sitting in empty space
        // between them. The band is the single derivation the surface and both buttons
        // are placed from, and the hit test reads the same buttons.
        let band = self.row_band();
        context.fill_rounded_rect(band, 4, bg_color);
        context.draw_rounded_rect_stroke(band, 4, border, 1);
        // A band too thin for buttons has no button row to paint: emitting a zero-height
        // button would be an invisible element rather than a small one.
        if band.height <= dimensions::STEPPER_PADDING * 2 {
            return;
        }

        // --- Minus button (left) ---
        let minus_rect = self.minus_rect();
        let minus_color = if !is_enabled { disabled_button } else { button_color };
        context.fill_rounded_rect(minus_rect, 3, minus_color);
        context.draw_rounded_rect_stroke(minus_rect, 3, border, 1);
        // Draw "-" symbol centered in the minus button through the shared line box: a
        // glyph origin is the box's **top** edge, so `(height + metrics.height) / 2 -
        // descent` placed that edge near the button's middle and drew the sign low.
        let minus_label = "\u{2212}";
        let minus_font = crate::core::Font::bold("Arial", 16.0);
        let minus_metrics = context.measure_text(minus_label, &minus_font);
        let minus_x = minus_rect.x + (minus_rect.width as i32 - minus_metrics.width as i32) / 2;
        let minus_line = context.text_line(minus_rect, &minus_font);
        context.draw_text(
            Point::new(minus_x, minus_line.y),
            minus_label,
            &minus_font,
            if !is_enabled { disabled_text } else { text_color },
            HorizontalAlignment::Left,
        );

        // --- Plus button (right) ---
        let plus_rect = self.plus_rect();
        let plus_color = if !is_enabled { disabled_button } else { button_color };
        context.fill_rounded_rect(plus_rect, 3, plus_color);
        context.draw_rounded_rect_stroke(plus_rect, 3, border, 1);
        // Draw "+" symbol centered in the plus button, same line box as the sign above.
        let plus_label = "+";
        let plus_font = crate::core::Font::bold("Arial", 16.0);
        let plus_metrics = context.measure_text(plus_label, &plus_font);
        let plus_x = plus_rect.x + (plus_rect.width as i32 - plus_metrics.width as i32) / 2;
        let plus_line = context.text_line(plus_rect, &plus_font);
        context.draw_text(
            Point::new(plus_x, plus_line.y),
            plus_label,
            &plus_font,
            if !is_enabled { disabled_text } else { text_color },
            HorizontalAlignment::Left,
        );

        // --- Value text (center of the row, not of the control) ---
        let text_x = band.x + (band.width as i32 - text_metrics.width as i32) / 2;
        let value_line = context.text_line(band, &font);
        let value_color = if !is_enabled { disabled_text } else { text_color };
        context.draw_text(
            Point::new(text_x, value_line.y),
            &value_text,
            &font,
            value_color,
            HorizontalAlignment::Left,
        );
    }
}

impl EventHandler for Stepper {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button != 1 {
                    return;
                }
                // The hit test reads the **drawn** buttons. It used to re-derive them
                // from `rect` with its own copy of the arithmetic, so the clickable area
                // and the painted one could — and did — describe different controls.
                if self.minus_rect().contains(*pos) {
                    self.decrement();
                } else if self.plus_rect().contains(*pos) {
                    self.increment();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn stepper_default_values() {
        let s = Stepper::new(Rect::new(0, 0, 120, 30));
        assert_eq!(s.value(), 0);
        assert_eq!(s.kind(), WidgetKind::Stepper);
    }

    #[test]
    fn stepper_set_value_clamps_to_min() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(-10);
        assert_eq!(s.value(), 0); // clamped to min=0
    }

    #[test]
    fn stepper_set_value_clamps_to_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(200);
        assert_eq!(s.value(), 100); // clamped to max=100
    }

    #[test]
    fn stepper_set_value_emits_signal() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        let captured = Arc::new(Mutex::new(None));
        s.value_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<i32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        s.set_value(42);
        assert_eq!(s.value(), 42);
        assert_eq!(*captured.lock().unwrap(), Some(42));
    }

    #[test]
    fn stepper_increment_decrement() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.increment();
        assert_eq!(s.value(), 1);
        s.increment();
        assert_eq!(s.value(), 2);
        s.decrement();
        assert_eq!(s.value(), 1);
    }

    #[test]
    fn stepper_increment_clamped_to_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(100);
        s.increment();
        assert_eq!(s.value(), 100);
    }

    #[test]
    fn stepper_decrement_clamped_to_min() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_value(0);
        s.decrement();
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn stepper_set_min_max() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_min(10);
        s.set_max(50);
        // Current value should be re-clamped
        assert_eq!(s.value(), 10);
        s.set_value(30);
        assert_eq!(s.value(), 30);
        s.set_value(5);
        assert_eq!(s.value(), 10);
        s.set_value(100);
        assert_eq!(s.value(), 50);
    }

    #[test]
    fn stepper_set_step() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_step(5);
        s.increment();
        assert_eq!(s.value(), 5);
        s.increment();
        assert_eq!(s.value(), 10);
        s.decrement();
        assert_eq!(s.value(), 5);
    }

    #[test]
    fn stepper_mouse_press_minus_decrements() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        // Set value to 5 first, then click in minus area (left side)
        s.set_value(5);
        // The press is placed from the drawn button, because that is what hit-tests.
        let minus = s.minus_rect();
        s.handle_event(&Event::MousePress {
            pos: Point::new(minus.x + minus.width as i32 / 2, minus.y + minus.height as i32 / 2),
            button: 1,
        });
        assert_eq!(s.value(), 4);
    }

    #[test]
    fn stepper_mouse_press_plus_increments() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        // The plus button is the trailing edge of the row band. `TOUCH_TARGET_MIN` is 48
        // and the band is 48 tall, so the buttons are 48 wide in this 120 px control.
        let plus = s.plus_rect();
        assert_eq!(plus.x, 120 - dimensions::STEPPER_BUTTON_WIDTH as i32 - 2);
        s.handle_event(&Event::MousePress {
            pos: Point::new(plus.x + plus.width as i32 / 2, plus.y + plus.height as i32 / 2),
            button: 1,
        });
        assert_eq!(s.value(), 1);
    }

    #[test]
    fn stepper_disabled_blocks_events() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        s.set_enabled(false);
        let plus = s.plus_rect();
        s.handle_event(&Event::MousePress {
            pos: Point::new(plus.x + plus.width as i32 / 2, plus.y + plus.height as i32 / 2),
            button: 1,
        });
        assert_eq!(s.value(), 0);
    }

    #[test]
    fn stepper_svg_output() {
        let mut s = Stepper::new(Rect::new(0, 0, 120, 30));
        let svg = crate::widget::svg::render_to_svg(&mut s);
        assert!(svg.starts_with("<svg"));
    }

    /// A stepper's buttons are a fixed height, not the control's.
    ///
    /// This pins the defect the fix removes: `btn_width` was
    /// `rect.height.min(rect.width / 3).max(20)` and the button's height was the
    /// control's own `rect.height - 2`, so the 240x120 census cell drew two **118 px**
    /// buttons in a 240x120 pill while a stepper in a 30 px form row drew 28 px ones —
    /// the same control at two sizes. The row band is now `STEPPER_ROW_HEIGHT` tall
    /// whenever the control has room for it.
    #[test]
    fn the_stepper_keeps_its_row_height_in_any_rectangle() {
        for height in [48u32, 120, 300] {
            let s = Stepper::new(Rect::new(0, 0, 240, height));
            let band = s.row_band();
            assert_eq!(band.height, dimensions::STEPPER_ROW_HEIGHT, "at control height {height}");
            assert_eq!(band.width, 240, "the row spans the control's width");
            // Centred, so the row sits on the control's middle line.
            assert_eq!(band.y, (height - band.height) as i32 / 2, "at control height {height}");
            // The buttons are a fixed width and sit inside the band once padded, so they
            // are never taller than the row that holds them.
            let minus = s.minus_rect();
            let plus = s.plus_rect();
            assert_eq!(
                minus.height,
                dimensions::STEPPER_ROW_HEIGHT - dimensions::STEPPER_PADDING * 2
            );
            assert_eq!(plus.height, minus.height);
            assert_eq!(minus.width, dimensions::STEPPER_BUTTON_WIDTH);
            assert!(minus.y >= band.y && plus.y >= band.y);
            assert!(
                minus.y + minus.height as i32 <= band.y + band.height as i32,
                "the minus button stays in the row"
            );
        }
        // A control shorter than the row clamps it rather than painting outside.
        let short = Stepper::new(Rect::new(0, 0, 240, 20));
        assert_eq!(short.row_band().height, 20);
        assert_eq!(short.row_band().y, 0, "a clamped band starts at the control's edge");
    }

    /// The buttons are the hit area, so a press outside them changes nothing.
    #[test]
    fn a_press_outside_the_buttons_changes_nothing() {
        let mut s = Stepper::new(Rect::new(0, 0, 240, 120));
        s.set_value(50);
        let band = s.row_band();
        // Above the row band, still inside the control's rectangle.
        assert!(band.y > 0, "the row is centred, so there is empty space above it");
        s.handle_event(&Event::MousePress { pos: Point::new(band.x + 4, band.y - 5), button: 1 });
        assert_eq!(s.value(), 50, "a press above the row must not step");
        // On the value between the two buttons.
        s.handle_event(&Event::MousePress {
            pos: Point::new(band.x + band.width as i32 / 2, band.y + band.height as i32 / 2),
            button: 1,
        });
        assert_eq!(s.value(), 50, "a press on the value must not step");
    }

    /// The emitted fills are the row band and its buttons, not a full-canvas slab.
    #[test]
    fn the_stepper_paints_a_row_rather_than_a_panel() {
        let mut s = Stepper::new(crate::widget::census::CENSUS_RECT);
        let svg = crate::widget::svg::render_to_svg(&mut s);
        let band = s.row_band();
        assert_eq!(band.height, dimensions::STEPPER_ROW_HEIGHT);
        assert_eq!(band.y, 36, "a 48 px row centred in the 120 px cell");
        assert!(
            svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" ry=\"4\"",
                band.x, band.y, band.width, band.height
            )),
            "the row band is the control's own surface: {svg}"
        );
        // The defect's signature: a surface the control's own height. The 120 px cell
        // must not emit a 120 px-tall fill.
        assert!(
            !svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"120\"",
                band.x, band.y, band.width
            )),
            "the control's own height would mean a panel fill: {svg}"
        );
        // Both buttons are one padding in from the row's top and bottom edges.
        let minus = s.minus_rect();
        assert!(
            svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" ry=\"3\"",
                minus.x, minus.y, minus.width, minus.height
            )),
            "the minus button is a fixed-size button on the row: {svg}"
        );
    }
}
