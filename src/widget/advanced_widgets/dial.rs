// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Dial (knob) widget.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f64, expect_i64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Dial (rotary knob) widget.
///
/// Holds an integer value in an inclusive `minimum ..= maximum` range and
/// renders it as a needle on a circle. The widget has no drag handling of its
/// own: it changes value in response to keyboard events and to explicit
/// [`Dial::set_value`] calls from the caller.
///
pub struct Dial {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    notches_visible: bool,
    notch_target: f64,
    wrapping: bool,
    /// Emitted with the new value whenever [`Dial::set_value`] actually changes
    /// it. Redundant sets do not fire it.
    pub value_changed: Signal1<i32>,
    /// Emitted when the primary mouse button is pressed while the dial is
    /// enabled. Purely a notification — the press does not change the value.
    pub slider_pressed: GenericSignal,
    /// Emitted when the primary mouse button is released while the dial is
    /// enabled. Like `slider_pressed`, it does not change the value.
    pub slider_released: GenericSignal,
}
impl Dial {
    /// Creates a dial ranging over `0 ..= 99` with value `0`, a single step of
    /// `1`, a page step of `10`, notches hidden, wrapping off, and a notch
    /// target of `3.7`.
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is 64x64.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dial, geometry, "Dial"),
            minimum: 0,
            maximum: 99,
            value: 0,
            single_step: 1,
            page_step: 10,
            notches_visible: false,
            notch_target: 3.7,
            wrapping: false,
            value_changed: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
        }
    }
    /// Returns the inclusive lower bound. Defaults to `0`.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Returns the inclusive upper bound. Defaults to `99`.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Returns the current value, always inside the configured range (modulo
    /// wrapping, which is still range-mapped).
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Returns the increment applied by one arrow-key press (in value units).
    /// Always at least `1`; defaults to `1`.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Returns the increment applied by Page Up / Page Down (in value units).
    /// Always at least `1`; defaults to `10`.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Returns whether the notch ring is drawn around the dial's body. Defaults to
    /// `false`.
    ///
    /// The ring is the scale the needle reads against; with it off the dial is a
    /// bare disc, which is why [`Dial::notch_target`] has no effect until this is
    /// `true`.
    pub fn notches_visible(&self) -> bool {
        self.notches_visible
    }
    /// Returns the notch target, in **pixels of arc between adjacent notches**.
    /// Defaults to `3.7`.
    ///
    /// This is Qt's own semantic for `QDial::notchTarget`, and it is the unit the
    /// rendering uses: the notch count is the dial's sweep measured in pixels and
    /// divided by this value. A value that would put fewer than two notches on the
    /// sweep is raised to the two-notch minimum, so turning the target up
    /// coarsens the scale up to that floor and then stops.
    ///
    /// Writing a value that is not finite, or not positive, leaves the target
    /// unchanged — a zero target would make the spacing, and with it the notch
    /// count, undefined.
    pub fn notch_target(&self) -> f64 {
        self.notch_target
    }
    /// Returns whether the value wraps around the range instead of clamping.
    /// Defaults to `false`.
    pub fn wrapping(&self) -> bool {
        self.wrapping
    }
    /// Sets the lower bound and re-applies it to the current value through
    /// [`Dial::set_value`], so the value will be clamped or wrapped into the
    /// new range and `value_changed` may fire.
    ///
    /// `min` above the current `maximum` leaves an inverted range in which the
    /// clamp saturates unpredictably; use [`Dial::set_range`] instead, which
    /// keeps `maximum >= minimum`.
    pub fn set_minimum(&mut self, min: i32) {
        self.minimum = min;
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets the upper bound and re-applies it to the current value through
    /// [`Dial::set_value`]. See [`Dial::set_minimum`] for range caveats.
    pub fn set_maximum(&mut self, max: i32) {
        self.maximum = max;
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call, raising `maximum` to `min`
    /// if it is lower, so the range is never inverted. The current value is
    /// then re-applied through [`Dial::set_value`].
    ///
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.minimum = min;
        self.maximum = max.max(min);
        self.set_value(self.value);
        self.base.request_redraw();
    }
    /// Sets the value, clamping into `minimum ..= maximum` — or wrapping
    /// modulo the range when [`Dial::wrapping`] is on, in which case the value
    /// is mapped back into the range rather than rejected.
    ///
    /// A no-op when the resulting value equals the current one: no signal and
    /// no redraw.
    pub fn set_value(&mut self, value: i32) {
        let clamped = if self.wrapping {
            // The span is computed in `i64`: `maximum - minimum + 1` overflows `i32`
            // for a legal range such as `i32::MIN ..= i32::MAX`, and the overflow
            // check panics in debug builds — aborting the host process on a property
            // write (`write_property(w, "minimum", ..)` reaches this). Widening keeps
            // the wrapping contract intact for every in-range span while making the
            // extreme case saturate instead of panic.
            let span = self.maximum as i64 - self.minimum as i64 + 1;
            let offset = (value as i64 - self.minimum as i64).rem_euclid(span);
            (self.minimum as i64 + offset) as i32
        } else {
            ordered_clamp_i32(value, self.minimum, self.maximum)
        };
        if self.value != clamped {
            self.value = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }
    /// Sets the arrow-key increment, floored at `1` so the value can always
    /// move. Requests a redraw.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
        self.base.request_redraw();
    }
    /// Sets the Page Up / Page Down increment, floored at `1`. Requests a
    /// redraw.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
        self.base.request_redraw();
    }
    /// Shows or hides the notch ring. See [`Dial::notches_visible`] — the ring is
    /// hidden by default. Requests a redraw.
    pub fn set_notches_visible(&mut self, visible: bool) {
        self.notches_visible = visible;
        self.base.request_redraw();
    }
    /// Sets the notch target in pixels of arc between adjacent notches. See
    /// [`Dial::notch_target`] for the unit and the minimum. A non-finite or
    /// non-positive value is ignored, so the stored target always describes a
    /// spacing the geometry can divide by. Requests a redraw.
    pub fn set_notch_target(&mut self, target: f64) {
        if target.is_finite() && target > 0.0 {
            self.notch_target = target;
            self.base.request_redraw();
        }
    }
    /// Enables or disables wrap-around behaviour.
    ///
    /// Takes effect on the *next* call to [`Dial::set_value`]; the current
    /// value is not re-mapped. Turning wrapping off therefore leaves a value
    /// that was produced by wrapping in place, which is fine because wrapped
    /// values are always inside the range. Requests a redraw.
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
        self.base.request_redraw();
    }
    /// Returns value as angle in radians (from -135° to +135°, or full circle if wrapping).
    fn value_angle(&self) -> f64 {
        Self::angle_at_fraction(self.ratio(), self.wrapping)
    }
    /// Returns the current value's position in the range as a `0.0 ..= 1.0` fraction.
    ///
    /// Zero for a degenerate range, which is the fraction whose angle both the needle and the
    /// notch ring then use — so a dial with `minimum == maximum` still draws them *in phase*
    /// rather than falling back to two different defaults.
    fn ratio(&self) -> f64 {
        let range = (self.maximum - self.minimum) as f64;
        if range == 0.0 {
            0.0
        } else {
            (self.value - self.minimum) as f64 / range
        }
    }
    /// The angle, in radians, at a `0.0 ..= 1.0` position along the dial's own sweep.
    ///
    /// # Why this is a function of the fraction and not of the value
    ///
    /// The needle and the notch ring are two readings of one scale, so they have to be placed
    /// by the *same* mapping. When each derived its own — or, as `meter.rs` did, when one of
    /// them omitted the sweep's phase offset — the ticks sat 90° out of phase with the needle
    /// they annotate, which is a scale pointing at nothing. Every caller therefore comes here.
    ///
    /// A wrapping dial sweeps the full circle from `-π` at the bottom of the range; a
    /// non-wrapping one sweeps 270° starting at `-135°`, leaving the gap at the bottom of the
    /// face.
    fn angle_at_fraction(fraction: f64, wrapping: bool) -> f64 {
        if wrapping {
            fraction * 2.0 * std::f64::consts::PI - std::f64::consts::PI
        } else {
            -std::f64::consts::PI * 0.75 + fraction * std::f64::consts::PI * 1.5
        }
    }
    /// The angular sweep the dial's scale occupies, in radians.
    fn sweep_radians(wrapping: bool) -> f64 {
        if wrapping {
            2.0 * std::f64::consts::PI
        } else {
            std::f64::consts::PI * 1.5
        }
    }
}
impl Widget for Dial {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(64, 64)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Dial`'s property contract.
impl WidgetProperties for Dial {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "single_step" => Ok(CapabilityValue::Int(self.single_step() as i64)),
            "page_step" => Ok(CapabilityValue::Int(self.page_step() as i64)),
            "notches_visible" => Ok(CapabilityValue::Bool(self.notches_visible())),
            "notch_target" => Ok(CapabilityValue::Float(self.notch_target())),
            "wrapping" => Ok(CapabilityValue::Bool(self.wrapping())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "minimum" => {
                self.set_minimum(expect_i64(value)? as i32);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i64(value)? as i32);
                Ok(())
            }
            "value" => {
                self.set_value(expect_i64(value)? as i32);
                Ok(())
            }
            "single_step" => {
                self.set_single_step(expect_i64(value)? as i32);
                Ok(())
            }
            "page_step" => {
                self.set_page_step(expect_i64(value)? as i32);
                Ok(())
            }
            "notches_visible" => {
                self.set_notches_visible(expect_bool(value)?);
                Ok(())
            }
            "notch_target" => {
                self.set_notch_target(expect_f64(value)?);
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
        // Mirrors `DIAL_PROPERTIES`.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "page_step",
            "notches_visible",
            "notch_target",
            "wrapping",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Reports the one published command that names no property.
    ///
    /// `set_range` sets two properties at once (`minimum` and `maximum`) through the
    /// control's own [`Self::set_range`], so neither property name alone stands for
    /// it. The default `set_foo` convention would have resolved `range` against
    /// `property_names` and, finding nothing, answered `UnknownCommand` for a command
    /// the control really implements. Answering `OutOfRange` says what is true: the
    /// command exists and needs the caller to supply a value.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => self.default_command(name),
        }
    }
}

impl EventHandler for Dial {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => {
                self.slider_pressed.emit();
            }
            Event::MouseRelease { button, .. } if *button == 1 => {
                self.slider_released.emit();
            }
            Event::KeyPress { key, .. } => match *key {
                37 | 40 => self.set_value(self.value - self.single_step), // Left/Down
                38 | 39 => self.set_value(self.value + self.single_step), // Up/Right
                33 => self.set_value(self.value - self.page_step),
                34 => self.set_value(self.value + self.page_step),
                36 => self.set_value(self.minimum),
                35 => self.set_value(self.maximum),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}
impl Draw for Dial {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let center = Point {
            x: rect.x + rect.width as f32 as i32 / 2,
            y: rect.y + rect.height as f32 as i32 / 2,
        };
        let radius = (rect.width.min(rect.height) / 2).saturating_sub(4);
        if radius == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step a
        // light/dark switch changed nothing on screen: the face, its rim, the needle and
        // the hub were all fixed greys, which the rendering census reported as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("dial");
        // Read as its own lock acquisition and copied out as values, so the guard is
        // dropped before anything else touches the theme. The accent is the dial's value
        // colour: the needle is a value indicator, the same role a progress bar's fill
        // plays, and reading the token is what makes the indicator move with the theme.
        let (window_fill, foreground, accent, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.accent,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The face is one step from the window fill toward the text colour, so the dial reads
        // as a raised disc in either appearance and is never byte-identical to the window
        // behind it. The filter is on the **resolved** value, not only on the theme's: the
        // active theme is applied to every control before it is drawn, so
        // `style.background_color` already holds the resolved fill. A caller's own colour
        // still wins.
        let face_from_theme = window_fill.blend(&ink, 0.10);
        let face = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => face_from_theme,
        };
        // A dial is a `Surface`-role control: the resolver knows its background and text but
        // has no border colour for it, so the rim is derived one visible step from the face.
        let rim = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != face)
            .unwrap_or_else(|| face.blend(&muted, 0.55));

        context.fill_circle(center, radius, face);
        context.draw_circle(center, radius, rim);

        // ── Notch ring ──
        //
        // `notches_visible` and `notch_target` were fully declared — field, accessors,
        // setters, `get`/`set` and `property_names` — and `draw` read neither, so the control
        // rendered a blank face whatever a caller asked for. This is the ring Qt's
        // `QDial::notchesVisible` / `notchTarget` draws, and both properties mean what Qt means
        // by them: the target is the *pixel spacing* between adjacent notches, and the count
        // follows from the sweep's arc length at this radius. A dial therefore gets a coarser
        // scale by rendering at a smaller size, which is Qt's behaviour and the reason the
        // target is a pixel quantity rather than a count.
        //
        // The arithmetic is done in `f64` and clamped before it is converted, because the
        // count becomes the bound of a loop: a rounded `NaN` is UB in that conversion, and a
        // sub-pixel target on a large radius would overflow `usize`. The clamp also supplies
        // the two-notch floor, so a target a caller has set very high still draws a scale.
        const MIN_NOTCHES: usize = 2;
        const MAX_NOTCHES: usize = 128;
        if self.notches_visible {
            let radius_f = radius as f64;
            let sweep = Self::sweep_radians(self.wrapping);
            // A zero target would make the division undefined; the setter rejects one, and
            // this keeps a target written through a future path from reaching the divide.
            let target_px =
                if self.notch_target.is_finite() { self.notch_target.max(0.5) } else { 0.5 };
            let count_f = (sweep * radius_f / target_px).round();
            let count = if count_f.is_finite() {
                (count_f as i64).clamp(MIN_NOTCHES as i64, MAX_NOTCHES as i64) as usize
            } else {
                MIN_NOTCHES
            };
            // # Why the ring's two radii are derived from the *rim* and not from each other
            //
            // The outer edge is pulled in from the rim so the ring reads as a scale drawn *on*
            // the face rather than as a second circle whose spokes cross the rim; the inner edge
            // is a fraction of the outer radius, so the tick length is proportional to the dial
            // and cannot degenerate. (Deriving the inner radius from the face's own radius
            // instead is what made the first version of this draw zero-length marks: at the
            // default target the inset and the inner fraction landed on the same pixel.)
            //
            // The inset is proportional with a proportional cap, so the ring still fits inside a
            // small dial that has a large target. Painting ticks outside the control is the
            // bounded-extent defect the census checks for, so the outer edge is also floored at
            // 1 px and the inner edge at 2 px less, which keeps `inner < outer` by construction.
            let inset = (radius_f * 0.20).round().clamp(4.0, radius_f * 0.35);
            let tick_outer = ((radius_f - inset).round() as i32).max(1);
            let tick_inner = ((tick_outer as f64 * 0.55).round() as i32).clamp(1, tick_outer - 1);

            // Every tick is placed from the *same* fraction-to-angle mapping the needle uses
            // ([`Self::angle_at_fraction`]), so the ring cannot land out of phase with the
            // needle it annotates — the defect `meter.rs` already had to fix once. Both ends
            // come from that one direction, which keeps a tick the same radial length in every
            // direction instead of two independently-rounded points whose separation varies.
            for i in 0..count {
                let tick_angle =
                    Self::angle_at_fraction(i as f64 / (count - 1) as f64, self.wrapping);
                let (dir_x, dir_y) = (tick_angle.cos(), tick_angle.sin());
                let outer = Point {
                    x: center.x + (tick_outer as f64 * dir_x).round() as i32,
                    y: center.y + (tick_outer as f64 * dir_y).round() as i32,
                };
                let inner = Point {
                    x: center.x + (tick_inner as f64 * dir_x).round() as i32,
                    y: center.y + (tick_inner as f64 * dir_y).round() as i32,
                };
                context.draw_line(outer, inner, rim);
            }
        }

        // Draw the value needle: an accent-coloured indicator, black hub beneath it.
        let angle = self.value_angle();
        let needle_len = (radius as f32 * 0.7) as i32;
        let to = Point {
            x: center.x + (needle_len as f32 * angle.cos() as f32) as i32,
            y: center.y + (needle_len as f32 * angle.sin() as f32) as i32,
        };
        context.draw_line(center, to, accent);
        context.fill_circle(center, 3, ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn dial_creation_defaults() {
        let d = Dial::new(Rect::new(0, 0, 64, 64));
        assert_eq!(d.minimum(), 0);
        assert_eq!(d.maximum(), 99);
        assert_eq!(d.value(), 0);
        assert_eq!(d.single_step(), 1);
        assert_eq!(d.page_step(), 10);
        assert!(!d.notches_visible());
        assert!(!d.wrapping());
    }

    #[test]
    fn dial_set_value_clamps() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        assert_eq!(d.value(), 50);
        d.set_value(200);
        assert_eq!(d.value(), 99);
        d.set_value(-10);
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_set_range_reclamps_value() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        d.set_range(60, 80);
        assert_eq!(d.value(), 60);
        assert_eq!(d.minimum(), 60);
        assert_eq!(d.maximum(), 80);
    }

    #[test]
    fn dial_wrapping() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_range(0, 9);
        d.set_wrapping(true);
        assert!(d.wrapping());
        d.set_value(9);
        assert_eq!(d.value(), 9);
        d.set_value(10);
        // wrapping: 10 % 10 = 0
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_steps() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_single_step(5);
        assert_eq!(d.single_step(), 5);
        d.set_single_step(0);
        assert_eq!(d.single_step(), 1); // floors at 1
        d.set_page_step(25);
        assert_eq!(d.page_step(), 25);
        d.set_page_step(0);
        assert_eq!(d.page_step(), 1); // floors at 1
    }

    #[test]
    fn dial_notches_visible() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        assert!(!d.notches_visible());
        d.set_notches_visible(true);
        assert!(d.notches_visible());
        d.set_notches_visible(false);
        assert!(!d.notches_visible());
    }

    #[test]
    fn dial_notch_target() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        assert!((d.notch_target() - 3.7).abs() < 1e-9);
        d.set_notch_target(5.0);
        assert!((d.notch_target() - 5.0).abs() < 1e-9);
    }

    /// A non-positive or non-finite target would make the notch count undefined, so the setter
    /// keeps the previous value rather than storing a spacing the geometry cannot divide by.
    #[test]
    fn dial_notch_target_rejects_a_non_positive_or_non_finite_value() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_notch_target(7.5);
        d.set_notch_target(0.0);
        assert!((d.notch_target() - 7.5).abs() < 1e-9, "a zero target must be refused");
        d.set_notch_target(-2.0);
        assert!((d.notch_target() - 7.5).abs() < 1e-9, "a negative target must be refused");
        d.set_notch_target(f64::NAN);
        assert!((d.notch_target() - 7.5).abs() < 1e-9, "a NaN target must be refused");
        d.set_notch_target(f64::INFINITY);
        assert!((d.notch_target() - 7.5).abs() < 1e-9, "an infinite target must be refused");
    }

    /// The defect: `notches_visible` and `notch_target` were fully declared and `draw` read
    /// neither, so the ring was absent no matter what a caller asked for.
    ///
    /// The expectation comes from the properties' own contract rather than from a tick count
    /// copied out of the implementation: the ring draws one mark per notch, so the `<line>` count
    /// in the rendered SVG — which is the needle plus the notches — must grow as the target gets
    /// smaller, and must fall to just the needle when the ring is hidden.
    #[test]
    fn dial_notch_ring_is_drawn_and_follows_the_target() {
        fn line_count(visible: bool, target: f64) -> usize {
            let mut d = Dial::new(Rect::new(0, 0, 240, 120));
            d.set_notches_visible(visible);
            d.set_notch_target(target);
            crate::widget::svg::render_to_svg(&mut d).matches("<line").count()
        }

        let bare = line_count(false, 3.7);
        let fine = line_count(true, 3.7);
        let coarse = line_count(true, 40.0);
        assert_eq!(bare, 1, "with notches hidden the only line is the needle");
        assert!(fine > bare, "notches_visible must actually draw the ring");
        assert!(fine > coarse, "a smaller target must place more notches");
        assert!(coarse > bare, "the caller's explicit target must still draw a ring");
    }

    /// The ring and the needle are two readings of one scale, so they must be placed by the same
    /// mapping — a ring out of phase with its own needle is the defect `meter.rs` had to fix.
    #[test]
    fn dial_notch_ring_is_in_phase_with_the_needle() {
        // The first notch of an unwrapped dial sits at the start of its 270 degree sweep, which
        // the accessor documents as `-135 deg`; the needle at `minimum` sits there too.
        let start = Dial::angle_at_fraction(0.0, false);
        let end = Dial::angle_at_fraction(1.0, false);
        assert!((start - (-std::f64::consts::PI * 0.75)).abs() < 1e-9);
        assert!((end - (std::f64::consts::PI * 0.75)).abs() < 1e-9);
        assert!((Dial::sweep_radians(false) - std::f64::consts::PI * 1.5).abs() < 1e-9);

        // A wrapping dial spans the whole circle, so its first notch is opposite its last.
        let wrap_start = Dial::angle_at_fraction(0.0, true);
        let wrap_end = Dial::angle_at_fraction(1.0, true);
        assert!((wrap_end - wrap_start - 2.0 * std::f64::consts::PI).abs() < 1e-9);

        // And the needle at `minimum` must land exactly on the ring's first notch.
        let mut d = Dial::new(Rect::new(0, 0, 240, 120));
        d.set_range(0, 100);
        d.set_value(0);
        assert!((d.value_angle() - Dial::angle_at_fraction(0.0, false)).abs() < 1e-9);
        d.set_value(100);
        assert!((d.value_angle() - Dial::angle_at_fraction(1.0, false)).abs() < 1e-9);
    }

    /// A degenerate range must not send the needle and the ring to two different fallbacks.
    #[test]
    fn dial_degenerate_range_keeps_needle_and_ring_in_phase() {
        let mut d = Dial::new(Rect::new(0, 0, 240, 120));
        d.set_range(5, 5);
        assert!((d.value_angle() - Dial::angle_at_fraction(0.0, false)).abs() < 1e-9);
    }

    /// Zero-area geometry must not divide by zero, and the ring must stay inside the control.
    #[test]
    fn dial_notches_stay_inside_the_control_at_every_size() {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

        for (w, h) in [(0u32, 0u32), (1, 1), (4, 4), (9, 9), (64, 64), (240, 120)] {
            let mut d = Dial::new(Rect::new(0, 0, w, h));
            d.set_notches_visible(true);
            d.set_notch_target(0.5);
            let mut backend = SoftwarePaintBackend::new(Size::new(w.max(1), h.max(1)), 1.0);
            backend.begin_frame(Color::WHITE);
            let mut ctx = RenderContext::new(&mut backend);
            d.draw(&mut ctx);

            // Every notch is placed from the ring's own radius, which is derived from the face's;
            // the face is inset from the control, so no mark can reach its edge.
            let svg = crate::widget::svg::render_to_svg(&mut d);
            for line in svg.lines().filter(|l| l.contains("<line")) {
                let nums: Vec<i32> = line
                    .split(|c: char| !(c.is_ascii_digit() || c == '-'))
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for value in nums {
                    assert!(value >= 0, "a notch escaped the control at {w}x{h}: {line}");
                }
            }
        }
    }

    #[test]
    fn dial_keyboard_navigation() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        // Left arrow (key 37) decreases by single step
        d.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(d.value(), 49);
        // Right arrow (key 39) increases by single step
        d.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(d.value(), 50);
        // PageUp (key 33) decreases by page step
        d.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(d.value(), 40);
        // PageDown (key 34) increases by page step
        d.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(d.value(), 50);
        // Home (key 36) goes to min
        d.handle_event(&Event::KeyPress { key: 36, modifiers: 0 });
        assert_eq!(d.value(), 0);
        // End (key 35) goes to max
        d.handle_event(&Event::KeyPress { key: 35, modifiers: 0 });
        assert_eq!(d.value(), 99);
    }

    #[test]
    fn dial_mouse_events() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.handle_event(&Event::MousePress { pos: Point::new(32, 32), button: 1 });
        // value should not change; only signal emitted
        assert_eq!(d.value(), 0);
        d.handle_event(&Event::MouseRelease { pos: Point::new(32, 32), button: 1 });
        assert_eq!(d.value(), 0);
    }

    #[test]
    fn dial_signal_accessors() {
        let d = Dial::new(Rect::new(0, 0, 64, 64));
        let _ = &d.value_changed;
        let _ = &d.slider_pressed;
        let _ = &d.slider_released;
    }

    #[test]
    fn dial_geometry_delegation() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_geometry(Rect::new(10, 10, 80, 80));
        assert_eq!(d.geometry(), Rect::new(10, 10, 80, 80));
    }

    #[test]
    fn dial_disabled_blocks_events() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_value(50);
        d.set_enabled(false);
        d.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        // Should stay unchanged because disabled
        assert_eq!(d.value(), 50);
    }

    /// A wrapping dial over the full `i32` range must not overflow.
    ///
    /// `maximum - minimum + 1` is `i32::MAX - i32::MIN + 1`, which does not fit in
    /// `i32`; the previous implementation panicked with `attempt to add with
    /// overflow` — killing the host process, since `set_minimum` is reachable from
    /// `write_property(w, "minimum", ..)` (the JSON declarative layer, the C ABI and
    /// every language binding all route through it).
    #[test]
    fn dial_wrapping_survives_full_i32_range() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_wrapping(true);
        d.set_maximum(i32::MAX);
        d.set_minimum(i32::MIN);
        // Every value is in range, so the value is preserved exactly.
        d.set_value(1234);
        assert_eq!(d.value(), 1234);
        d.set_value(i32::MIN);
        assert_eq!(d.value(), i32::MIN);
        d.set_value(i32::MAX);
        assert_eq!(d.value(), i32::MAX);
    }

    /// Wrapping still maps out-of-range values modulo the span.
    #[test]
    fn dial_wrapping_maps_out_of_range_values() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_range(0, 9);
        d.set_wrapping(true);
        d.set_value(13);
        assert_eq!(d.value(), 3);
        d.set_value(-1);
        assert_eq!(d.value(), 9);
    }

    /// A span wide enough to overflow `i32` when offset by one still wraps rather
    /// than aborting: the span is computed in `i64` so the modulo is well-defined.
    #[test]
    fn dial_wrapping_handles_near_maximal_span() {
        let mut d = Dial::new(Rect::new(0, 0, 64, 64));
        d.set_wrapping(true);
        d.set_range(-1, i32::MAX - 1);
        d.set_value(0);
        assert_eq!(d.value(), 0);
    }
}
