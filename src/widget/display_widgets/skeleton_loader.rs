// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SkeletonLoader widget — animated placeholder content while data loads.
//!
//! The SkeletonLoader renders a gray rounded-rect placeholder with a shimmer
//! pulse effect that oscillates the fill opacity between ~0.1 and ~0.3,
//! providing clear visual feedback that data is still loading.
//!
//! Supported shapes:
//! - `Rect(w, h)` — rectangular block
//! - `Circle(r)` — circular avatar/image placeholder
//! - `TextLine(w)` — three stacked text line placeholders

use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Shape variants for the skeleton placeholder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    /// Rectangular placeholder (width, height).
    Rect(u32, u32),
    /// Circular placeholder (radius).
    Circle(u32),
    /// Single text line placeholder (width).
    TextLine(u32),
}

/// The string spelling of a [`SkeletonShape`] discriminant, published by the
/// `shape` property.
///
/// The `Rect`/`Circle`/`TextLine` payloads are deliberately left out: the
/// property names the *kind* of placeholder, matching the token style the other
/// enum properties use.
pub fn skeleton_shape_to_str(shape: SkeletonShape) -> &'static str {
    match shape {
        SkeletonShape::Rect(..) => "rect",
        SkeletonShape::Circle(..) => "circle",
        SkeletonShape::TextLine(..) => "text_line",
    }
}

/// Parses the [`skeleton_shape_to_str`] token back into a shape.
///
/// A `Rect` without dimensions is not representable, so the parse accepts only
/// the tokens that name a bare kind and rebuilds the default sizes for them.
fn expect_skeleton_shape(value: CapabilityValue) -> Result<SkeletonShape, CapabilityAccessError> {
    match expect_string(value)?.as_str() {
        "rect" => Ok(SkeletonShape::Rect(200, 20)),
        "circle" => Ok(SkeletonShape::Circle(20)),
        "text_line" => Ok(SkeletonShape::TextLine(200)),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Timer ID used to drive the shimmer animation.
const SKELETON_ANIMATION_TIMER_ID: u32 = 0x534B;

/// How long one full shimmer pulse takes, in milliseconds.
///
/// The pulse is a loop, so it is priced from the theme's `slow` token scaled up by
/// [`PULSE_SLOW_MULTIPLE`] — the same rule `progress_circle`'s revolution follows, because
/// both are "an indefinite wait, animated" and a theme that slows everything down should
/// slow them together. The literal is the no-theme fallback.
const DEFAULT_PULSE_PERIOD_MS: u32 = 1200;

/// How many `slow` tokens one shimmer pulse is worth.
const PULSE_SLOW_MULTIPLE: u32 = 4;

/// SkeletonLoader widget — renders a shimmering placeholder shape while data loads.
pub struct SkeletonLoader {
    base: BaseWidget,
    shape: SkeletonShape,
    animated: bool,
    /// The shimmer's phase, in turns: `0.0` and `1.0` are the same point in the pulse.
    ///
    /// # Why this is a phase and not a frame counter
    ///
    /// The pulse used to advance one step per `Event::Timer`, which made its *rate* a
    /// property of how often the host happened to send that event: the same placeholder
    /// pulsed at one speed on a 60 Hz host and twice as fast on a 120 Hz one, and it sat
    /// outside the animation bus entirely. A pulse is a duration, so it is driven by
    /// [`SkeletonLoader::tick`] from `delta_ms` — the same contract every other animation in
    /// the crate uses (BLUE23 \u00a73).
    phase: f32,
}

impl SkeletonLoader {
    /// Creates a new SkeletonLoader with the given geometry.
    ///
    /// The default shape is a 200×20 `Rect` and the shimmer animation is enabled
    /// by default.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SkeletonLoader, geometry, "SkeletonLoader"),
            shape: SkeletonShape::Rect(200, 20),
            animated: true,
            phase: 0.0,
        }
    }

    /// Sets the skeleton placeholder shape.
    pub fn set_shape(&mut self, shape: SkeletonShape) {
        self.shape = shape;
    }

    /// Returns the current skeleton placeholder shape.
    pub fn shape(&self) -> SkeletonShape {
        self.shape
    }

    /// Enables or disables the shimmer animation.
    ///
    /// When disabled the placeholder draws at a static mid-opacity of 0.2.
    pub fn set_animated(&mut self, animated: bool) {
        self.animated = animated;
    }

    /// Returns whether the shimmer animation is enabled.
    pub fn is_animated(&self) -> bool {
        self.animated
    }

    /// Computes the current shimmer opacity from the pulse phase.
    ///
    /// A triangle wave over the phase: `0.0` and `1.0` are the dimmest point, `0.5` the
    /// brightest. A disabled animation returns the static mid-point, which is also the wave's
    /// own mean — so switching the animation off dims the shimmer to its average brightness
    /// rather than jumping to one extreme.
    fn current_opacity(&self) -> f32 {
        if !self.animated {
            return 0.2;
        }
        // `1 - |2p - 1|` is the triangle: 0 at both ends, 1 at the middle.
        let triangle = 1.0 - (2.0 * self.phase - 1.0).abs();
        0.1 + 0.2 * triangle
    }

    /// Advances the shimmer by `delta_ms` and reports whether another frame is owed.
    ///
    /// Only the animated state owes frames: a static placeholder is drawn once, so it must
    /// not keep the frame bus alive (\u00a73.2).
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animated {
            return false;
        }
        // Wrapped rather than clamped, so a long-running app cannot drift into a large value
        // and lose float precision on the phase.
        self.phase = (self.phase + delta_ms as f32 / self.pulse_period_ms() as f32).fract();
        self.base.request_redraw();
        true
    }

    /// The duration of one full pulse, priced by the theme.
    fn pulse_period_ms(&self) -> u32 {
        let slow = crate::style::motion_tokens().2;
        if slow == 0 {
            DEFAULT_PULSE_PERIOD_MS
        } else {
            slow.saturating_mul(PULSE_SLOW_MULTIPLE).max(1)
        }
    }
}

impl Widget for SkeletonLoader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 20)
    }

    /// One frame of the shimmer. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        SkeletonLoader::tick(self, delta_ms)
    }

    /// Only a shimmering placeholder owes frames; a static one is a still picture.
    fn is_animating(&self) -> bool {
        self.animated
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SkeletonLoader`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The legacy table spelled the
/// animation flag `active`, which is kept as the published name and maps onto
/// [`SkeletonLoader::is_animated`]; `shape` is published alongside it so the
/// placeholder kind is describable too.
impl WidgetProperties for SkeletonLoader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "active" => Ok(CapabilityValue::Bool(self.is_animated())),
            "shape" => Ok(CapabilityValue::String(skeleton_shape_to_str(self.shape()).to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "active" => {
                self.set_animated(expect_bool(value)?);
                Ok(())
            }
            "shape" => {
                self.set_shape(expect_skeleton_shape(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["active", "shape", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `skeleton_loader` publishes.
    ///
    /// `set_active` is the only published name and it assigns the animation flag,
    /// which a command cannot carry, so it is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the name is right, the boolean belongs
    /// on the property route `set("active", ..)` — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_active" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for SkeletonLoader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;

        // Chrome colours resolve the explicit style first, then the theme's resolved style
        // for this control, and only then the original literal. The literal stays as the
        // fallback so an inactive theme still has a defined appearance. Previously the RGB
        // was a literal grey, so the placeholder shimmered the same in light and dark and the
        // rendering census reported the control as theme-blind.
        //
        // `resolved_theme_style` takes and releases the global manager's lock internally, so
        // no guard is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let themed = crate::style::resolved_theme_style("skeleton_loader");
        let themed_bg = themed.as_ref().and_then(|r| r.background_color);
        let themed_text = themed.as_ref().and_then(|r| r.text_color);
        // The resolved surface, re-derived one visible step towards the theme's ink: the
        // placeholder's whole job is to be *seen* against the surface it sits on, so painting
        // it in that exact surface colour would defeat it.
        let surface = style.background_color.or(themed_bg).unwrap_or(Color::WHITE);
        let ink = style.text_color.or(themed_text).unwrap_or(Color::rgb(200, 200, 200));
        let skeleton = surface.blend(&ink, 0.35);

        let opacity = self.current_opacity();
        // Only the RGB comes from the resolved colour: the alpha below is the shimmer
        // animation's own modulation and is deliberately left exactly as it was.
        let base_color =
            Color::rgba(skeleton.r, skeleton.g, skeleton.b, (opacity * 255.0).round() as u8);

        match self.shape {
            SkeletonShape::Rect(w, h) => {
                // The caller's own dimensions are the **datum** here: a skeleton stands in
                // for a specific piece of content, so its width and height are the shape
                // the caller asked for. They are still clamped to the rectangle, because
                // an oversized placeholder would paint outside the area nothing clips it
                // to — which is what `ControlMetrics::center_in` guarantees, and it is
                // also what keeps a zero-size placeholder from being emitted.
                let w = w.min(rect.width).max(1);
                let h = h.min(rect.height).max(1);
                let shape_rect = ControlMetrics::center_in(rect, crate::core::Size::new(w, h));
                context.fill_rounded_rect(shape_rect, 4, base_color);
            }
            SkeletonShape::Circle(r) => {
                // The radius is the caller's datum, clamped to the shortest side so the
                // disc stays square and inside the control.
                let r = r.min(rect.width.min(rect.height) / 2).max(1);
                let center = Point::new(cx, cy);
                context.fill_circle_aa(center, r, base_color);
            }
            SkeletonShape::TextLine(w) => {
                // Draw three stacked text-like lines to simulate paragraph text.
                //
                // The rows are a **fixed** height and gap, so the placeholder is
                // `3 * row + 2 * gap` tall in any rectangle. Every term used to be a
                // local literal, which is why the census cell drew a 12 px line in a
                // 120 px box: the stack was the only fixed part of a control whose box
                // was otherwise the caller's, so the same skeleton was a different
                // object at every size. `SKELETON_ROW_HEIGHT` and `SKELETON_ROW_GAP`
                // name the two values the caller can now predict from the shape.
                let line_h = dimensions::SKELETON_ROW_HEIGHT;
                let gap = dimensions::SKELETON_ROW_GAP;
                let total_h = 3 * line_h + 2 * gap;
                let start_y = cy - total_h as i32 / 2;
                // A row is the caller's width clamped to the control, and never zero: an
                // over-wide row would paint past the rectangle and a zero-width one would
                // be an invisible element.
                let row_w = w.clamp(1, rect.width.max(1));
                for i in 0..3 {
                    let y = start_y + i * (line_h + gap) as i32;
                    let line_rect = Rect::new(cx - row_w as i32 / 2, y, row_w, line_h);
                    context.fill_rounded_rect(line_rect, 3, base_color);
                }
            }
        }
    }
}

impl EventHandler for SkeletonLoader {
    fn handle_event(&mut self, event: &Event) {
        // A disabled control consumes nothing, so the base state stays authoritative.
        if !self.base.is_enabled() {
            return;
        }
        self.base.handle_event(event);
        // The legacy timer still advances the pulse, at a nominal frame's worth of time, so a
        // host that has not moved to the frame bus sees the shimmer it always did. It is a
        // fallback, not the driver: the pulse's *rate* now comes from the theme rather than
        // from how often this event arrives.
        if let Event::Timer { id } = event {
            if *id == SKELETON_ANIMATION_TIMER_ID && self.animated {
                self.tick(16);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn skeleton_loader_default_creation() {
        let sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert_eq!(sl.kind(), WidgetKind::SkeletonLoader);
        assert_eq!(sl.geometry(), Rect::new(0, 0, 200, 100));
        assert_eq!(sl.shape(), SkeletonShape::Rect(200, 20));
        assert!(sl.is_animated());
    }

    #[test]
    fn skeleton_loader_rect_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::Rect(100, 50));
        assert_eq!(sl.shape(), SkeletonShape::Rect(100, 50));
    }

    #[test]
    fn skeleton_loader_circle_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::Circle(30));
        assert_eq!(sl.shape(), SkeletonShape::Circle(30));
    }

    #[test]
    fn skeleton_loader_text_line_shape() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        sl.set_shape(SkeletonShape::TextLine(150));
        assert_eq!(sl.shape(), SkeletonShape::TextLine(150));
    }

    /// The shimmer oscillates between its two ends, and its rate is a *duration* — not a frame
    /// count.
    ///
    /// Regression: the pulse advanced one step per `Event::Timer`, so its rate depended on how
    /// often the host sent that event and it stood outside the frame bus. It is now driven by
    /// `tick(delta_ms)` from the theme's `slow` token, so the same control pulses at the same
    /// rate on any host. The legacy timer still nudges it, which is why the event path is
    /// exercised here as well.
    #[test]
    fn skeleton_loader_pulses_over_time_and_holds_still_when_static() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert!(sl.is_animated());

        // Collect opacities across one full period, stepping in tenths of the period.
        let period = sl.pulse_period_ms();
        let at_rest = sl.current_opacity();
        let opacities: Vec<f32> = (0..10)
            .map(|_| {
                assert!(sl.tick(period / 10), "an animated pulse owes another frame");
                sl.current_opacity()
            })
            .collect();

        // Both ends of the range are reached, so the pulse reads as a shimmer and not a wobble.
        let min_op = opacities.iter().cloned().fold(f32::MAX, f32::min);
        let max_op = opacities.iter().cloned().fold(f32::MIN, f32::max);
        assert!(min_op < 0.15, "minimum opacity should be near 0.1, got {min_op}");
        assert!(max_op > 0.25, "maximum opacity should be near 0.3, got {max_op}");

        // Ten steps of a tenth of a period is exactly one turn, so the pulse is back where it
        // started: the phase is a loop, not a counter that grows without bound.
        assert!(
            (sl.current_opacity() - at_rest).abs() < 0.02,
            "one period returns to the start: {} vs {at_rest}",
            sl.current_opacity()
        );

        // Disabling the animation freezes it at the wave's mean and stops it owing frames.
        sl.set_animated(false);
        assert!(!sl.is_animated());
        let static_op = sl.current_opacity();
        assert!((static_op - 0.2).abs() < 0.01, "static opacity should be 0.2, got {static_op}");
        assert!(!sl.tick(period), "a static placeholder must not request another frame");

        // The legacy timer path still advances the pulse for a host that has not migrated.
        sl.set_animated(true);
        let before = sl.current_opacity();
        sl.handle_event(&Event::Timer { id: SKELETON_ANIMATION_TIMER_ID });
        assert_ne!(sl.current_opacity(), before, "the legacy timer still nudges the pulse");
    }

    #[test]
    fn skeleton_loader_svg_output() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        let svg = render_to_svg(&mut sl);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"100\""));
    }

    #[test]
    fn skeleton_loader_set_animated_flag() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 200, 100));
        assert!(sl.is_animated());
        sl.set_animated(false);
        assert!(!sl.is_animated());
        sl.set_animated(true);
        assert!(sl.is_animated());
    }

    /// A `TextLine` skeleton is a fixed stack of rows, not a fraction of the control.
    ///
    /// This pins the defect the fix removes: the row height and gap were local literals
    /// and the rows were the only fixed part of a control whose box was otherwise the
    /// caller's, so a skeleton in a 100 px slot and one in a census cell were laid out
    /// against different arithmetic. The stack is now
    /// `3 * SKELETON_ROW_HEIGHT + 2 * SKELETON_ROW_GAP` tall in any rectangle, which is
    /// the one thing a caller can predict from the shape.
    #[test]
    fn a_text_line_skeleton_stacks_fixed_rows() {
        let total = 3 * dimensions::SKELETON_ROW_HEIGHT + 2 * dimensions::SKELETON_ROW_GAP;
        // The stack cannot outgrow the cell a control is measured in, or the census
        // would be reporting a clipped placeholder rather than a drawn one.
        assert!(total < 120, "the default stack is shorter than the census cell");
        for rect in [Rect::new(0, 0, 240, 120), Rect::new(0, 0, 240, 300)] {
            let mut sl = SkeletonLoader::new(rect);
            sl.set_shape(SkeletonShape::TextLine(180));
            sl.set_animated(false);
            let svg = render_to_svg(&mut sl);
            // Each of the three rows is emitted at the named height.
            assert_eq!(
                svg.matches(&format!("height=\"{}\"", dimensions::SKELETON_ROW_HEIGHT)).count(),
                3,
                "three rows at SKELETON_ROW_HEIGHT in {rect:?}: {svg}"
            );
        }
    }

    /// A `Rect` skeleton is the caller's size, clamped to the control rather than
    /// painted outside it, and never zero-extent.
    #[test]
    fn a_rect_skeleton_is_clamped_to_its_control_and_never_empty() {
        let mut oversized = SkeletonLoader::new(Rect::new(0, 0, 100, 40));
        oversized.set_shape(SkeletonShape::Rect(500, 500));
        oversized.set_animated(false);
        let svg = render_to_svg(&mut oversized);
        assert!(
            svg.contains("height=\"40\""),
            "an oversized placeholder clamps to the control: {svg}"
        );

        // A zero-size placeholder is still emitted at one pixel rather than not at all.
        let mut empty = SkeletonLoader::new(Rect::new(0, 0, 100, 40));
        empty.set_shape(SkeletonShape::Rect(0, 0));
        empty.set_animated(false);
        let svg = render_to_svg(&mut empty);
        assert!(!svg.contains("width=\"0\""), "nothing is emitted zero-wide: {svg}");
        assert!(!svg.contains("height=\"0\""), "nothing is emitted zero-tall: {svg}");
    }

    /// A `Circle` skeleton is a disc centred in the control, clamped to its short side.
    #[test]
    fn a_circle_skeleton_stays_square_and_inside() {
        let mut sl = SkeletonLoader::new(Rect::new(0, 0, 60, 120));
        sl.set_shape(SkeletonShape::Circle(200));
        sl.set_animated(false);
        // The disc is clamped to half the *shorter* side, so it cannot exceed the width.
        let svg = render_to_svg(&mut sl);
        assert!(!svg.contains("r=\"0\""), "a disc is never zero-radius: {svg}");
    }
}
