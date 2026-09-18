// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::Vec;
use crate::core::{Color, Point};
/// Which geometry a [`Gradient`] interpolates along.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GradientType {
    /// Colour is interpolated along the straight line from `start_point` to
    /// `end_point`. The default.
    #[default]
    Linear,
    /// Colour is interpolated by distance from `center`, reaching the last stop
    /// at `radius`.
    Radial,
    /// Colour is interpolated by angle (swept) around `center`.
    Conic,
}
/// One colour stop on a gradient ramp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// Where the stop sits along the ramp, as a fraction in the inclusive range
    /// `0.0`..=`1.0`: `0.0` is the ramp's start, `1.0` its end. Values outside
    /// that range cannot be constructed through [`GradientStop::new`], but the
    /// field is public and a directly-constructed stop may hold one.
    pub position: f32,
    /// The colour reached exactly at `position`.
    pub color: Color,
}
impl GradientStop {
    /// Creates a stop, clamping `position` into `0.0`..=`1.0`.
    pub fn new(position: f32, color: Color) -> Self {
        Self { position: position.clamp(0.0, 1.0), color }
    }
}
/// A colour ramp, plus the geometry that says how ramp positions map to space.
///
/// Exactly one of the three geometries is in effect at a time, selected by
/// `gradient_type`; the fields belonging to the other two are left at their
/// constructor defaults and are ignored.
///
/// [`Gradient::interpolate`] samples the ramp by a normalised position and does
/// not itself depend on `gradient_type` — mapping a ramp position to a point in
/// space is the renderer's job, using the geometry fields below.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Which geometry the ramp is laid out along.
    pub gradient_type: GradientType,
    /// The ramp's colour stops. Always kept sorted by ascending `position` by
    /// the builder methods on this type; [`Gradient::interpolate`] relies on that
    /// ordering, so a vector assigned directly through this field must be sorted
    /// by the caller.
    pub stops: Vec<GradientStop>,
    /// Start of the ramp for [`GradientType::Linear`]: the point at position
    /// `0.0`. Unused by the radial and conic geometries.
    pub start_point: Point,
    /// End of the ramp for [`GradientType::Linear`]: the point at position
    /// `1.0`. Unused by the radial and conic geometries.
    pub end_point: Point,
    /// Sweep angle in **degrees**, clockwise from the positive x-axis. Settable
    /// for the linear geometry via [`Gradient::with_angle`]; for
    /// [`GradientType::Conic`] it is the angle the sweep starts at and is set by
    /// [`Gradient::conic`]. Unused by the radial geometry.
    pub angle: f32,
    /// Centre point, in the parent surface's pixel coordinates, of a radial or
    /// conic gradient. Unused by the linear geometry.
    pub center: Point,
    /// Radius in **pixels** at which a radial gradient reaches its last stop.
    /// Unused by the linear and conic geometries.
    pub radius: f32,
}
impl Gradient {
    /// Creates a linear gradient running from `start` (ramp position `0.0`) to
    /// `end` (position `1.0`).
    ///
    /// The ramp starts empty: add stops with [`Gradient::add_stop`] or
    /// [`Gradient::with_stops`]. An empty ramp interpolates to
    /// [`Color::TRANSPARENT`].
    pub fn linear(start: Point, end: Point) -> Self {
        Self {
            gradient_type: GradientType::Linear,
            stops: Vec::new(),
            start_point: start,
            end_point: end,
            angle: 0.0,
            center: Point::new(0, 0),
            radius: 0.0,
        }
    }
    /// Creates a radial gradient centred on `center` whose last stop is reached
    /// at `radius` pixels from that centre.
    ///
    /// The ramp starts empty; see [`Gradient::linear`] for how to fill it.
    pub fn radial(center: Point, radius: f32) -> Self {
        Self {
            gradient_type: GradientType::Radial,
            stops: Vec::new(),
            start_point: Point::new(0, 0),
            end_point: Point::new(0, 0),
            angle: 0.0,
            center,
            radius,
        }
    }
    /// Creates a conic (swept) gradient centred on `center`, starting its sweep
    /// at `angle` **degrees** clockwise from the positive x-axis.
    ///
    /// The ramp starts empty; see [`Gradient::linear`] for how to fill it.
    pub fn conic(center: Point, angle: f32) -> Self {
        Self {
            gradient_type: GradientType::Conic,
            stops: Vec::new(),
            start_point: Point::new(0, 0),
            end_point: Point::new(0, 0),
            angle,
            center,
            radius: 0.0,
        }
    }
    /// Sets the sweep angle in **degrees** and returns `self`, for chaining onto
    /// a constructor.
    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }
    /// Appends a colour stop and returns `self`.
    ///
    /// `position` is the fraction along the ramp, clamped into `0.0`..=`1.0`.
    /// The stop list is re-sorted by position, so stops may be added in any
    /// order. Two stops at the same position are allowed; which of them wins at
    /// exactly that position is then left to sort stability rather than to the
    /// caller.
    pub fn add_stop(mut self, position: f32, color: Color) -> Self {
        self.stops.push(GradientStop::new(position, color));
        self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self
    }
    /// Replaces the whole stop list, sorts it by ascending position, and returns
    /// `self`.
    pub fn with_stops(mut self, stops: Vec<GradientStop>) -> Self {
        self.stops = stops;
        self.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self
    }
    /// Samples the ramp at `position`, a fraction along it that is clamped into
    /// `0.0`..=`1.0`.
    ///
    /// The value is found by straight linear interpolation in non-premultiplied
    /// RGBA, one `u8` channel at a time, so partially transparent stops blend
    /// their alpha as a plain channel.
    ///
    /// Degenerate ramps return a defined colour rather than panicking:
    ///
    /// * no stops — [`Color::TRANSPARENT`];
    /// * exactly one stop — that stop's colour, at every position;
    /// * `position` before the first stop or at/after the last — the first or
    ///   last stop's colour, with no extension or wrapping.
    pub fn interpolate(&self, position: f32) -> Color {
        let position = position.clamp(0.0, 1.0);
        if self.stops.is_empty() {
            return Color::TRANSPARENT;
        }
        if self.stops.len() == 1 {
            return self.stops[0].color;
        }
        if position <= self.stops[0].position {
            return self.stops[0].color;
        }
        if position >= self.stops[self.stops.len() - 1].position {
            return self.stops[self.stops.len() - 1].color;
        }
        for i in 0..self.stops.len() - 1 {
            let current = &self.stops[i];
            let next = &self.stops[i + 1];
            if position >= current.position && position <= next.position {
                let t = (position - current.position) / (next.position - current.position);
                return Self::interpolate_color(current.color, next.color, t);
            }
        }
        self.stops[self.stops.len() - 1].color
    }
    fn interpolate_color(from: Color, to: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let r = ((1.0 - t) * from.r as f32 + t * to.r as f32) as u8;
        let g = ((1.0 - t) * from.g as f32 + t * to.g as f32) as u8;
        let b = ((1.0 - t) * from.b as f32 + t * to.b as f32) as u8;
        let a = ((1.0 - t) * from.a as f32 + t * to.a as f32) as u8;
        Color::rgba(r, g, b, a)
    }
    /// Returns a copy whose ramp runs the other way: the stop order is reversed
    /// and each stop's `position` is mirrored to `1.0 - position`.
    ///
    /// The geometry fields (`start_point`, `end_point`, `center`, `radius`,
    /// `angle`) are copied through unchanged, so this flips only the colour ramp,
    /// not the direction in space in which it is painted.
    pub fn reverse(&self) -> Self {
        let mut reversed = self.clone();
        reversed.stops.reverse();
        for stop in &mut reversed.stops {
            stop.position = 1.0 - stop.position;
        }
        reversed
    }
    /// Whether this gradient has enough stops to be worth painting.
    ///
    /// Reports `true` for two or more stops. A one-stop gradient paints a single
    /// flat colour — [`Gradient::interpolate`] handles it — so it is reported as
    /// invalid here; callers wanting a flat fill should use a solid colour.
    pub fn is_valid(&self) -> bool {
        self.stops.len() >= 2
    }
}
impl Default for Gradient {
    fn default() -> Self {
        Self::linear(Point::new(0, 0), Point::new(100, 0))
    }
}

/// Chainable builder for a [`Gradient`].
///
/// Differs from the [`Gradient`] `with_*`/`add_stop` methods in one way that
/// matters: [`GradientBuilder::build`] sorts the stops, so stops can be pushed
/// here in any order and the ordering invariant [`Gradient::interpolate`] relies
/// on is established once, at the end, instead of on every insertion.
pub struct GradientBuilder {
    gradient: Gradient,
}
impl GradientBuilder {
    /// Starts building a linear gradient from `start` (ramp position `0.0`) to
    /// `end` (position `1.0`).
    pub fn linear(start: Point, end: Point) -> Self {
        Self { gradient: Gradient::linear(start, end) }
    }
    /// Starts building a radial gradient centred on `center` with its last stop
    /// at `radius` **pixels** from that centre.
    pub fn radial(center: Point, radius: f32) -> Self {
        Self { gradient: Gradient::radial(center, radius) }
    }
    /// Starts building a conic (swept) gradient centred on `center`, sweeping
    /// from `angle` **degrees** clockwise from the positive x-axis.
    pub fn conic(center: Point, angle: f32) -> Self {
        Self { gradient: Gradient::conic(center, angle) }
    }
    /// Adds a colour stop at `position`, a fraction along the ramp clamped into
    /// `0.0`..=`1.0`, and returns the builder for chaining.
    ///
    /// Unlike [`Gradient::add_stop`], stops are not sorted here — the sort
    /// happens once in [`GradientBuilder::build`] — so this is the cheaper call
    /// in a loop.
    pub fn stop(mut self, position: f32, color: Color) -> Self {
        self.gradient.stops.push(GradientStop::new(position, color));
        self
    }
    /// Sorts the accumulated stops by ascending position and yields the
    /// gradient. Safe to call with no stops, which produces an empty
    /// (transparent) ramp.
    pub fn build(mut self) -> Gradient {
        self.gradient.stops.sort_by(|a, b| a.position.total_cmp(&b.position));
        self.gradient
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_gradient_creation() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::RED)
            .add_stop(1.0, Color::BLUE);
        assert_eq!(gradient.gradient_type, GradientType::Linear);
        assert_eq!(gradient.stops.len(), 2);
    }
    #[test]
    fn test_gradient_interpolation() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color { r: 255, g: 0, b: 0, a: 255 })
            .add_stop(1.0, Color { r: 0, g: 0, b: 255, a: 255 });
        let mid_color = gradient.interpolate(0.5);
        assert_eq!(mid_color.r, 127);
        assert_eq!(mid_color.g, 0);
        assert_eq!(mid_color.b, 127);
    }
    #[test]
    fn test_gradient_reverse() {
        let gradient = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::RED)
            .add_stop(1.0, Color::BLUE);
        let reversed = gradient.reverse();
        assert_eq!(reversed.stops[0].color, Color::BLUE);
        assert_eq!(reversed.stops[1].color, Color::RED);
    }
    #[test]
    fn gradient_linear_interpolation() {
        let g = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::BLACK)
            .add_stop(1.0, Color::WHITE);
        assert_eq!(g.interpolate(0.0), Color::BLACK);
        assert_eq!(g.interpolate(1.0), Color::WHITE);
        let mid = g.interpolate(0.5);
        assert!(mid.r >= 125 && mid.r <= 131);
    }
    #[test]
    fn gradient_radial_creation() {
        let g = Gradient::radial(Point::new(50, 50), 30.0)
            .add_stop(0.0, Color::RED)
            .add_stop(1.0, Color::BLUE);
        assert_eq!(g.gradient_type, GradientType::Radial);
        assert_eq!(g.center, Point::new(50, 50));
        assert!((g.radius - 30.0).abs() < 1e-6);
    }
    #[test]
    fn gradient_interpolate_clamps() {
        let g = Gradient::linear(Point::new(0, 0), Point::new(100, 0))
            .add_stop(0.0, Color::BLACK)
            .add_stop(1.0, Color::WHITE);
        assert_eq!(g.interpolate(-0.5), Color::BLACK);
        assert_eq!(g.interpolate(1.5), Color::WHITE);
    }
    #[test]
    fn gradient_single_stop() {
        let g = Gradient::linear(Point::new(0, 0), Point::new(100, 0)).add_stop(0.0, Color::RED);
        assert_eq!(g.interpolate(0.0), Color::RED);
        assert_eq!(g.interpolate(0.5), Color::RED);
        assert_eq!(g.interpolate(1.0), Color::RED);
    }
    #[test]
    fn gradient_empty_stops_interpolates_transparent() {
        let g = Gradient::linear(Point::new(0, 0), Point::new(100, 0));
        assert_eq!(g.interpolate(0.0), Color::TRANSPARENT);
        assert_eq!(g.interpolate(0.5), Color::TRANSPARENT);
    }
    #[test]
    fn test_gradient_builder() {
        let gradient = GradientBuilder::linear(Point::new(0, 0), Point::new(100, 100))
            .stop(0.0, Color::WHITE)
            .stop(0.5, Color::GRAY)
            .stop(1.0, Color::BLACK)
            .build();
        assert_eq!(gradient.stops.len(), 3);
    }
}
