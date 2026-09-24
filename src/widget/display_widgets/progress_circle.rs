// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ProgressCircle widget — a circular progress indicator.
//!
//! The ProgressCircle widget displays a circular progress track with a filled arc
//! representing the current progress value (0.0 to 1.0). It supports both determinate
//! mode (showing actual progress) and indeterminate mode (animated spinning arc).
//! The track and progress colors, as well as stroke width, are customizable.

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_bool, expect_f64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The gap left where the progress arc meets the track, in logical pixels.
///
/// Material's `ProgressIndicator.trackGap` (`progress_indicator.dart:1636`). It is the
/// difference between "this ring is 0%" and "this ring is broken": with a gap, a value of
/// zero still shows two distinct ends, while without one the arc and the track are the same
/// stroke at the same angle and the reader cannot tell the two apart at all.
const TRACK_GAP: f32 = 4.0;

/// How long one full revolution of the indeterminate sweep takes, in milliseconds.
///
/// # Why this is a multiple of the theme's `slow` token
///
/// The sweep is a *loop*, not a state change: `Motion::slow` is sized for a transition that
/// ends (300 ms in the preset), and a ring completing a revolution that fast reads as a
/// stutter rather than as progress. The revolution is therefore paced from `slow` scaled up
/// by [`SPIN_SLOW_MULTIPLE`], so a theme that slows everything down slows the ring with it,
/// and the no-theme fallback is this constant — the value the preset produces.
const DEFAULT_SPIN_PERIOD_MS: u32 = 1400;

/// How many `slow` tokens one revolution is worth.
const SPIN_SLOW_MULTIPLE: u32 = 5;

/// ProgressCircle widget — a Material-style circular progress indicator.
///
/// In determinate mode, draws a track circle and a progress arc.
/// In indeterminate mode, draws a spinning arc segment that cycles around the circle.
///
/// The widget uses the `stroke_width` to determine the thickness of both the
/// track and progress arcs. The progress arc is drawn using closely-spaced
/// line segments approximating a circular arc.
pub struct ProgressCircle {
    base: BaseWidget,
    value: f32,
    indeterminate: bool,
    track_color: Color,
    progress_color: Color,
    stroke_width: f32,
    diameter: u32,
    /// The indeterminate sweep's rotation, in turns: `0.0` and `1.0` are the same angle.
    ///
    /// # Why this is a field and not `SystemTime::now()`
    ///
    /// The sweep used to read the wall clock inside `draw`. That made the control's appearance
    /// a function of something no test could set, so the only way to assert the animation moved
    /// was to sleep; it also meant the ring kept rotating in a frame nobody had asked for, and
    /// that it could not participate in `Widget::tick` at all. A turning ring is state, and state
    /// belongs to the control: the frame bus advances it through [`ProgressCircle::tick`], and a
    /// paused or idle app simply does not advance it.
    sweep: f32,
}

impl ProgressCircle {
    /// Creates a new ProgressCircle widget with the given geometry.
    ///
    /// Defaults: value 0.0, determinate, light gray track, and the theme's value-indicator
    /// colour for the arc, stroke width 4.
    ///
    /// `progress_color` starts as the token `progress_bar` uses for its fill — the theme's
    /// `Accent` role colour — rather than the `Color::PRIMARY` literal it carried before.
    /// The two progress controls are the same reading of the same data, so a caller who
    /// left the colour alone must not get a blue ring beside an orange bar. The literal is
    /// still the no-theme fallback, so a build without a theme renders what it used to.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressCircle, geometry, "ProgressCircle"),
            value: 0.0,
            indeterminate: false,
            track_color: Color::rgba(220, 220, 220, 200),
            progress_color: crate::style::resolved_theme_style("progress_bar")
                .and_then(|resolved| resolved.background_color)
                .unwrap_or(Color::PRIMARY),
            stroke_width: 4.0,
            diameter: geometry.width.min(geometry.height),
            sweep: 0.0,
        }
    }

    /// Returns the current progress value (0.0 to 1.0).
    pub fn value(&self) -> f32 {
        self.value
    }

    /// Sets the progress value. Clamped to 0.0..=1.0.
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
        self.base.request_redraw();
    }

    /// Returns whether the indicator is in indeterminate (spinning) mode.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Sets whether the indicator shows indeterminate (spinning) animation.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.indeterminate = indeterminate;
        // Entering the spinning state starts from the top rather than wherever the last sweep
        // stopped, so two rings switched on at different times still read as one animation.
        if indeterminate {
            self.sweep = 0.0;
        }
        self.base.request_redraw();
    }

    /// The indeterminate sweep's current angle, in radians.
    ///
    /// Exposed so a test can assert the ring actually turns across frames without sleeping:
    /// the value is a function of the ticks the control was given, not of the wall clock.
    pub fn sweep_angle(&self) -> f32 {
        self.sweep * 2.0 * core::f32::consts::PI
    }

    /// Advances the indeterminate sweep by `delta_ms` and reports whether another frame is owed.
    ///
    /// Only the indeterminate state animates: a determinate ring is a still picture that is
    /// redrawn when its value changes, so it must not keep the frame bus alive (\u00a73.2's
    /// "no animation, no repaint").
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.indeterminate {
            return false;
        }
        let period = self.spin_period_ms();
        // Wrapped rather than clamped: the sweep is a revolution, so `1.0` is `0.0` and the
        // fraction is kept in `0.0..1.0` so a long-running app cannot drift into a large value
        // and lose float precision on the angle.
        self.sweep = (self.sweep + delta_ms as f32 / period as f32).fract();
        self.base.request_redraw();
        true
    }

    /// The duration of one full revolution, priced by the theme.
    fn spin_period_ms(&self) -> u32 {
        let slow = crate::style::motion_tokens().2;
        if slow == 0 {
            DEFAULT_SPIN_PERIOD_MS
        } else {
            slow.saturating_mul(SPIN_SLOW_MULTIPLE).max(1)
        }
    }

    /// Returns the current track color.
    pub fn track_color(&self) -> Color {
        self.track_color
    }

    /// Sets the track (background circle) color.
    pub fn set_track_color(&mut self, color: Color) {
        self.track_color = color;
        self.base.request_redraw();
    }

    /// Returns the current progress (foreground arc) color.
    pub fn progress_color(&self) -> Color {
        self.progress_color
    }

    /// Sets the progress (foreground arc) color.
    pub fn set_progress_color(&mut self, color: Color) {
        self.progress_color = color;
        self.base.request_redraw();
    }

    /// Returns the current stroke width in logical pixels.
    pub fn stroke_width(&self) -> f32 {
        self.stroke_width
    }

    /// Returns the arc thickness published as the `thickness` property.
    ///
    /// The widget draws both the track and the progress arc with a single stroke
    /// width, so `thickness` and [`ProgressCircle::stroke_width`] are the same
    /// quantity read through the name the schema declares.
    pub fn thickness(&self) -> f32 {
        self.stroke_width
    }

    /// Returns the diameter of the progress circle.
    pub fn diameter(&self) -> u32 {
        self.diameter
    }

    /// Sets the diameter of the progress circle.
    pub fn set_diameter(&mut self, diameter: u32) {
        self.diameter = diameter.max(1);
        self.base.request_redraw();
    }

    /// Sets the stroke width for both track and progress arcs.
    pub fn set_stroke_width(&mut self, width: f32) {
        self.stroke_width = width.max(0.5);
        self.base.request_redraw();
    }

    /// Computes the center point and radius of the progress circle.
    fn center_and_radius(&self) -> Option<(Point, f32)> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        let cx = rect.x + (rect.width as i32) / 2;
        let cy = rect.y + (rect.height as i32) / 2;
        let radius = (rect.width.min(rect.height) as f32 / 2.0) - self.stroke_width / 2.0 - 1.0;
        if radius <= 0.0 {
            return None;
        }
        Some((Point::new(cx, cy), radius))
    }
}

impl Widget for ProgressCircle {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(self.diameter.max(60), self.diameter.max(60))
    }

    /// One frame of the indeterminate sweep. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        ProgressCircle::tick(self, delta_ms)
    }

    /// Only the indeterminate ring owes frames; a determinate one is a still picture.
    fn is_animating(&self) -> bool {
        self.indeterminate
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ProgressCircle`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The schema publishes the arc
/// thickness as `thickness` although the widget draws through a single stroke
/// width, so the two names address the same field.
impl WidgetProperties for ProgressCircle {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(f64::from(self.value()))),
            "thickness" => Ok(CapabilityValue::Float(f64::from(self.thickness()))),
            "indeterminate" => Ok(CapabilityValue::Bool(self.is_indeterminate())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_f64(value)? as f32);
                Ok(())
            }
            "thickness" => {
                self.set_stroke_width(expect_f64(value)? as f32);
                Ok(())
            }
            "indeterminate" => {
                self.set_indeterminate(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["value", "thickness", "indeterminate", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `progress_circle` publishes.
    ///
    /// All three assign state — the progress value, the stroke width and the
    /// indeterminate flag — so each needs an argument a command carries none of. They
    /// are refused as [`CapabilityAccessError::OutOfRange`] (use the property route
    /// `set("value", ..)` / `set("thickness", ..)` / `set("indeterminate", ..)`)
    /// rather than reported as unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_thickness" | "set_indeterminate" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ProgressCircle {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let Some((center, radius)) = self.center_and_radius() else {
            return;
        };

        let is_enabled = self.base.is_enabled();
        let stroke_w = self.stroke_width as u32;

        // Draw track (full circle). The track is the ring's empty groove — chrome, not a
        // value — so it follows the theme. Chrome colours resolve the explicit style first,
        // then the theme's resolved style for this control, and only then the original
        // literal, which stays as the fallback so an inactive theme still has a defined
        // appearance. The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let themed = crate::style::resolved_theme_style("progress_circle");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        // `progress_circle` is `WidgetRole::Accent`, so the theme writes a groove colour into
        // the style's background. The control's own `track_color` literal is deliberately
        // **not** the base here: it is a near-white grey in every palette, so using it would
        // leave the track identical in light and dark — the exact defect being fixed. It
        // stays as the enabled-state literal fallback instead.
        let fallback_track = themed_bg.unwrap_or(Color::rgba(220, 220, 220, 200));
        let surface = style.background_color.unwrap_or(fallback_track);
        let track_color = if is_enabled {
            surface
        } else {
            // A disabled control is a chrome state, so the groove is derived from the
            // resolved colours rather than from a second hardcoded grey.
            surface.blend(&Color::DISABLED_FOREGROUND, 0.5)
        };

        // Draw track as a circle stroke
        context.draw_circle_stroke(center, radius as u32, track_color, stroke_w.max(1));

        // The progress arc is deliberately NOT themed: its colour can encode a value or a
        // threshold the caller chose, which is data, so its computation is left untouched.

        if self.indeterminate {
            // The arc sweeps a fixed share of the circle and the whole thing revolves, which is
            // the shape Material's indeterminate indicator describes: the arc's own length says
            // "work is happening", the revolution says "and it has not stopped".
            //
            // The angle comes from `self.sweep`, advanced by `tick`, so nothing here reads the
            // clock: two frames of the same control with the same tick history paint the same
            // ring, which is what makes the animation assertable without sleeping.
            let arc_sweep = 2.4; // ~135 degrees in radians
            let start_angle = self.sweep_angle();
            let end_angle = start_angle + arc_sweep;

            let prog_color =
                if is_enabled { self.progress_color } else { Color::DISABLED_FOREGROUND };
            crate::render::draw_arc_segments(
                context,
                center,
                radius,
                start_angle,
                end_angle,
                prog_color,
                stroke_w.max(1),
            );
        } else if self.value > 0.0 {
            // In determinate mode, draw the progress arc from 12 o'clock
            // -PI/2 (12 o'clock) to -PI/2 + 2*PI*value
            let start_angle = -std::f32::consts::FRAC_PI_2;
            // A non-zero value starts after a `TRACK_GAP`, so the arc's two ends are distinct
            // from the track's two ends even at the smallest visible value. The gap is converted
            // from pixels to radians at the drawn radius, so the visible breaking is the same
            // size on a small ring and a large one.
            let gap_radians = TRACK_GAP / radius.max(1.0);
            let start_angle = start_angle + gap_radians;
            let end_angle = -std::f32::consts::FRAC_PI_2 + 2.0 * std::f32::consts::PI * self.value;
            // The arc must never run backwards or past its own start when the value is small
            // enough that the gap would swallow it; a zero-or-negative sweep is nothing to draw.
            if end_angle <= start_angle {
                return;
            }

            // A sweep too small to advance a pixel is skipped rather than emitted as a stream
            // of zero-length lines. This guard used to be the only thing standing between a
            // tiny `value` and 40 degenerate `<line>` elements, because the helper's vertex
            // rounding collapsed every sample onto one pixel. `draw_arc_segments` now derives
            // its sample count from the arc's own pixel length and drops any chord whose
            // endpoints round together, so it is degenerate-safe on its own; the explicit
            // check stays because "no arc at all" is a decision about *this* control's
            // contract (a rounded-away sweep is nothing to draw), and stating it here keeps
            // the intent readable without relying on a helper's internals.
            let sweep_px = (radius * (end_angle - start_angle)).round() as u32;
            if sweep_px > 0 {
                let prog_color =
                    if is_enabled { self.progress_color } else { Color::DISABLED_FOREGROUND };
                crate::render::draw_arc_segments(
                    context,
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    prog_color,
                    stroke_w.max(1),
                );
            }
        }
    }
}

impl EventHandler for ProgressCircle {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_circle_default_creation() {
        let pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert_eq!(pc.kind(), WidgetKind::ProgressCircle);
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
        assert!(!pc.is_indeterminate());
        assert!((pc.stroke_width() - 4.0).abs() < f32::EPSILON);
        assert_eq!(pc.track_color(), Color::rgba(220, 220, 220, 200));
        // The arc's default is the token `progress_bar` fills with, so the two progress
        // controls agree; the literal is only the no-theme fallback. Asserting the literal
        // here would re-assert the defect the default was changed to remove.
        let expected = crate::style::resolved_theme_style("progress_bar")
            .and_then(|resolved| resolved.background_color)
            .unwrap_or(Color::PRIMARY);
        assert_eq!(pc.progress_color(), expected);
    }

    #[test]
    fn progress_circle_set_value() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.5);
        assert!((pc.value() - 0.5).abs() < f32::EPSILON);

        pc.set_value(1.5); // Should clamp to 1.0
        assert!((pc.value() - 1.0).abs() < f32::EPSILON);

        pc.set_value(-0.5); // Should clamp to 0.0
        assert!((pc.value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_indeterminate_toggle() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        assert!(!pc.is_indeterminate());

        pc.set_indeterminate(true);
        assert!(pc.is_indeterminate());

        pc.set_indeterminate(false);
        assert!(!pc.is_indeterminate());
    }

    #[test]
    fn progress_circle_colors_and_stroke() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));

        pc.set_track_color(Color::LIGHT_GRAY);
        assert_eq!(pc.track_color(), Color::LIGHT_GRAY);

        pc.set_progress_color(Color::SUCCESS);
        assert_eq!(pc.progress_color(), Color::SUCCESS);

        pc.set_stroke_width(6.0);
        assert!((pc.stroke_width() - 6.0).abs() < f32::EPSILON);

        pc.set_stroke_width(-1.0); // Should clamp to minimum
        assert!((pc.stroke_width() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_circle_svg_output_determinate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.75);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_svg_output_indeterminate() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_indeterminate(true);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn progress_circle_zero_geometry_no_crash() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 0, 0));
        // Should not panic
        let svg = crate::widget::svg::render_to_svg(&mut pc);
        assert!(svg.starts_with("<svg"));
    }

    /// Every arc segment in the stream, as `(x1, y1, x2, y2)`.
    ///
    /// The arc helper emits one `<line>` per segment at the widget's stroke width, so selecting on
    /// that pair counts the arc's own output without having to reason about the track circle.
    fn arc_segments(svg: &str) -> Vec<(i32, i32, i32, i32)> {
        svg.lines()
            .filter(|line| line.contains("<line") && line.contains("stroke-width=\"4\""))
            .map(|line| {
                let attr = |name: &str| -> i32 {
                    line.split(&std::format!("{name}=\""))
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| panic!("no {name} on: {line}"))
                };
                (attr("x1"), attr("y1"), attr("x2"), attr("y2"))
            })
            .collect()
    }

    /// A value whose arc sweep rounds away must emit **no** segment with any length.
    ///
    /// Two guards used to leave this value uncovered. The helper drops a sweep below 0.001 rad, and
    /// the draw skipped `value == 0.0` — but value 0.001 gives a 0.0063 rad sweep, which passes the
    /// first test while 40 vertex pairs all round onto the same pixel. The result was 40
    /// zero-length `<line>` elements: a stream full of drawing commands that draw nothing. The track
    /// circle must survive the guard, because it is the ring's own chrome.
    #[test]
    fn progress_circle_ignores_a_value_too_small_to_sweep_a_pixel() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.001);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(
            segments.iter().all(|(x1, y1, x2, y2)| x1 == x2 && y1 == y2),
            "no arc segment of a swept-away value may have length: {segments:?}"
        );
        assert!(
            svg.contains("<circle") && svg.contains("fill=\"none\""),
            "the track ring is chrome and must still be drawn: {svg}"
        );
    }

    /// The complement: a value that does sweep whole pixels must still draw the arc.
    #[test]
    fn progress_circle_draws_an_arc_that_sweeps_whole_pixels() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.25);

        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(
            !segments.is_empty() && segments.iter().any(|(x1, y1, x2, y2)| x1 != x2 || y1 != y2),
            "a quarter turn is a visible arc, not 40 dots: {segments:?}"
        );
    }

    #[test]
    fn progress_circle_event_forwarding() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        // Should not panic
        pc.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        pc.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
    }

    /// The indeterminate ring must turn, and it must turn on the ticks it is given.
    ///
    /// This is the \u00a70.3 three-frame judgement: the three frames' geometry differ, and the
    /// angle advances monotonically. It replaces a sweep read from `SystemTime::now()` inside
    /// `draw`, which could only be tested by sleeping and which kept the ring turning in a
    /// frame nobody had requested.
    #[test]
    fn the_indeterminate_ring_turns_on_the_ticks_it_is_given() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_indeterminate(true);
        let first = crate::widget::svg::render_to_svg(&mut pc);
        let angle_before = pc.sweep_angle();

        assert!(pc.tick(100), "an indeterminate ring owes another frame");
        let second = crate::widget::svg::render_to_svg(&mut pc);
        assert!(pc.tick(100), "and the frame after that");
        let third = crate::widget::svg::render_to_svg(&mut pc);

        assert!(pc.sweep_angle() > angle_before, "the ring must advance, not sit still");
        assert_ne!(first, second, "frame one and frame two must not be the same picture");
        assert_ne!(second, third, "nor frame two and frame three");
    }

    /// A determinate ring is a still picture: it owes no frames, and telling it so is a no-op.
    ///
    /// \u00a73.2 in one assertion: an animation that is not running must not keep the frame bus
    /// alive, or every idle progress ring repaints the whole window forever.
    #[test]
    fn a_determinate_ring_owes_no_frames() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.5);
        assert!(!pc.tick(100), "a determinate ring must not request another frame");
        let before = crate::widget::svg::render_to_svg(&mut pc);
        assert!(!pc.tick(100), "and still not after being asked twice");
        assert_eq!(before, crate::widget::svg::render_to_svg(&mut pc), "its picture is unchanged");
    }

    /// At value zero the arc and the track must still be two different strokes.
    ///
    /// Regression (BLUE21 A.3.6): without a track gap, a value just above zero draws the arc
    /// flush against the track's own start, so the two read as one unbroken ring and the reader
    /// cannot tell an empty indicator from a full one. The gap is what makes the arc's start
    /// visible.
    ///
    /// # Why this measures the first *chord* and not the first vertex
    ///
    /// The arc helper rounds every sample onto a pixel, so on a 48 px ring a 4 px gap moves the
    /// start by less than a pixel and the rounded corner is the same either way — an assertion on
    /// the vertex passes with and without the gap. The arc's *length* is what actually changes:
    /// the same value sweeps fewer pixels once the gap is taken out of it, so the gap is asserted
    /// by measuring the swept length against the value's own angle.
    #[test]
    fn the_smallest_visible_arc_starts_after_a_track_gap() {
        let mut pc = ProgressCircle::new(Rect::new(0, 0, 48, 48));
        pc.set_value(0.25);
        let svg = crate::widget::svg::render_to_svg(&mut pc);
        let segments = arc_segments(&svg);
        assert!(!segments.is_empty(), "a quarter ring is a visible arc: {svg}");

        // The radius the control draws at is `min(w, h)/2 - stroke/2 - 1` — **not** half the
        // box: the stroke is centred on the path, so the outer half of it would otherwise fall
        // outside the rectangle. The gap comes out of the sweep's own angle, so the arc's pixel
        // length is `radius * (quarter - gap/radius)` = `radius * quarter - gap`.
        let radius = 48.0 / 2.0 - 4.0 / 2.0 - 1.0;
        let gapped_px = (radius * core::f32::consts::FRAC_PI_2 - TRACK_GAP).ceil() as usize;
        let ungapped_px = (radius * core::f32::consts::FRAC_PI_2).ceil() as usize;
        assert!(
            ungapped_px > gapped_px,
            "the fixture must distinguish the two cases, or the assertion below is vacuous"
        );
        // The helper emits one chord per pixel of the gapped length (the last sample's rounding
        // can collapse it by one), and this is strictly fewer than the ungapped sweep would be.
        assert!(
            segments.len() <= gapped_px && segments.len() < ungapped_px,
            "the arc must be the gapped sweep, not the raw one: {} segments, expected <= \
             {gapped_px} (gapped) and < {ungapped_px} (ungapped)",
            segments.len()
        );
    }
}
