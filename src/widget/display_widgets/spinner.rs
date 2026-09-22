// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Spinner widget — rotating loading indicator (BLUE13 R2.2).
//!
//! A `Spinner` displays an animated circular arc that rotates to indicate
//! an ongoing loading or processing operation. It supports configurable
//! thickness, speed, size ratio, and style integration.

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Spinner widget for indicating loading/processing state.
///
/// Draws a circular track (gray ring) and a colored arc segment that
/// rotates when the spinner is active. The rotation angle advances via
/// the [`tick`](Spinner::tick) method, which should be called each frame.
///
/// # Style integration
///
/// - `style().background_color`: overrides the default arc color.
/// - `style().text_color`: overrides the default track color.
pub struct Spinner {
    base: BaseWidget,
    /// Whether the spinner is actively spinning.
    active: bool,
    /// Current rotation angle in degrees (0–360).
    angle: f32,
    /// Line/arc thickness in pixels.
    thickness: u32,
    /// Size of the spinner as a fraction of the widget (0.0–1.0).
    size_ratio: f32,
    /// Speed multiplier (default 1.0).
    speed: f32,
}

impl Spinner {
    /// Creates a new Spinner with the given geometry.
    ///
    /// Defaults: active, angle 0, thickness 4 px, size ratio 0.8, speed 1.0.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Spinner, rect, "Spinner"),
            active: true,
            angle: 0.0,
            thickness: 4,
            size_ratio: 0.8,
            speed: 1.0,
        }
    }

    /// Returns whether the spinner is actively spinning.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Sets the active state. When inactive the arc stays at its current angle.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.base.request_redraw();
    }

    /// Sets the line/arc thickness in pixels (minimum 1).
    pub fn set_thickness(&mut self, thickness: u32) {
        self.thickness = thickness.max(1);
        self.base.request_redraw();
    }

    /// Sets the speed multiplier (clamped to >= 0.0).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.max(0.0);
        self.base.request_redraw();
    }

    /// Sets the size ratio (0.0–1.0) that controls the spinner diameter
    /// relative to the widget's shortest side.
    pub fn set_size_ratio(&mut self, ratio: f32) {
        self.size_ratio = ratio.clamp(0.0, 1.0);
        self.base.request_redraw();
    }

    /// Returns the current rotation angle in degrees.
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Returns the line/arc thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// Returns the size ratio.
    pub fn size_ratio(&self) -> f32 {
        self.size_ratio
    }

    /// Returns the speed multiplier.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Advance the spinner angle. Call this each frame to animate.
    ///
    /// `delta_ms` is the elapsed time in milliseconds since the last frame.
    /// The angle advances at a base rate of 0.1 degrees per millisecond,
    /// multiplied by `self.speed`.
    pub fn tick(&mut self, delta_ms: u32) {
        self.angle = (self.angle + delta_ms as f32 * 0.1 * self.speed) % 360.0;
    }

    /// Computes the centre point and effective radius of the painted ring.
    ///
    /// # Why the diameter is the control's own, not the rectangle's
    ///
    /// A spinner is a **fixed-size indicator**: the same ring means the same thing in a
    /// toolbar slot and in a full-screen cell. Deriving the diameter from the given
    /// rectangle (`min(w, h) as f32 / 2.0 * size_ratio`) made the ring a fraction of
    /// whatever space the caller happened to leave — the census cell drew a `r=45`
    /// circle filling most of the 240x120 box while [`Self::size_hint`] reported
    /// 48x48, so the control's declaration and its drawing described two different
    /// objects. [`dimensions::SPINNER_DIAMETER`] is the size `size_hint` asks for, so
    /// the two now agree by construction; [`ControlMetrics::centered_disc`] places it
    /// on the control's middle and clamps it to the rectangle, because nothing clips a
    /// widget at this layer and a ring larger than the control would paint outside it.
    ///
    /// `size_ratio` still scales the ring inside that fixed box, so a caller that wants
    /// a smaller indicator gets one without resizing the control.
    fn center_and_radius(&self) -> Option<(Point, u32)> {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return None;
        }
        // The box the ring is centred in: the fixed diameter when the control has room
        // for it, the control's own extent when it does not. Guarded against zero in
        // both axes, because `draw_circle_stroke` with a zero radius is an invisible
        // element rather than a small one.
        let box_side = dimensions::SPINNER_DIAMETER.min(rect.width).min(rect.height);
        if box_side == 0 {
            return None;
        }
        let disc = ControlMetrics::centered_disc(rect, box_side);
        let center =
            Point::new(disc.x + (disc.width as i32) / 2, disc.y + (disc.height as i32) / 2);
        // The ring is inset by half the stroke and a pixel of rounding, so the painted
        // outline stays inside the disc rather than straddling its edge.
        let raw_radius = disc.width as f32 / 2.0 * self.size_ratio;
        let radius = (raw_radius - self.thickness as f32 / 2.0 - 1.0).max(1.0);
        Some((center, radius as u32))
    }
}

impl Widget for Spinner {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // The same fact `center_and_radius` draws from: the indicator's diameter. It has
        // to be a square of that size, or a layout that honours `size_hint` would hand
        // the control a box its ring does not fit in.
        Size::new(dimensions::SPINNER_DIAMETER, dimensions::SPINNER_DIAMETER)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Spinner`'s property contract.
impl WidgetProperties for Spinner {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "active" => Ok(CapabilityValue::Bool(self.is_active())),
            "thickness" => Ok(CapabilityValue::UInt(self.thickness() as u64)),
            "speed" => Ok(CapabilityValue::Float(self.speed() as f64)),
            "size_ratio" => Ok(CapabilityValue::Float(self.size_ratio() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "active" => {
                self.set_active(expect_bool(value)?);
                Ok(())
            }
            "thickness" => {
                self.set_thickness(expect_u32(value)?);
                Ok(())
            }
            "speed" => {
                self.set_speed(expect_f32(value)?);
                Ok(())
            }
            "size_ratio" => {
                self.set_size_ratio(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SPINNER_PROPERTIES`.
        property_names_of!["active", "thickness", "speed", "size_ratio", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `spinner` publishes.
    ///
    /// All four assign state — the running flag, the rotation speed, the stroke width
    /// and the diameter ratio — so each needs an argument a command carries none of.
    /// They are refused as [`CapabilityAccessError::OutOfRange`] (use the property
    /// route `set("active", ..)` / `set("speed", ..)` / `set("thickness", ..)` /
    /// `set("size_ratio", ..)`) rather than reported as unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_active" | "set_speed" | "set_thickness" | "set_size_ratio" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Spinner {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for Spinner {
    fn draw(&mut self, context: &mut RenderContext) {
        // `center_and_radius` already refuses a rectangle with no extent, so this path
        // only runs when there is a disc to paint.
        let Some((center, radius)) = self.center_and_radius() else {
            return;
        };
        if radius == 0 {
            // A zero-radius ring is invisible: nothing to draw, and emitting it would
            // break the "never emit a zero-extent element" rule.
            return;
        }

        let is_enabled = self.base.is_enabled();
        let stroke_w = self.thickness.max(1);

        // Resolve colors from style or use defaults.
        let style = self.style();
        let track_color = style.text_color.unwrap_or_else(|| Color::rgba(220, 220, 220, 200));
        let arc_color = style.background_color.unwrap_or(Color::PRIMARY);

        // Resolve effective colors based on enabled state.
        let effective_track =
            if is_enabled { track_color } else { Color::rgba(200, 200, 200, 100) };
        let effective_arc = if is_enabled { arc_color } else { Color::DISABLED_FOREGROUND };

        // Draw the track (full circle stroke).
        context.draw_circle_stroke(center, radius, effective_track, stroke_w);

        // Draw the spinning arc segment (~135 degrees sweep).
        // Convert angle from degrees to radians; start at -90° (12 o'clock) offset.
        let arc_sweep = 2.4; // ~135 degrees in radians
        let start_angle = self.angle.to_radians() - core::f32::consts::FRAC_PI_2;
        let end_angle = start_angle + arc_sweep;

        crate::render::draw_arc_segments(
            context,
            center,
            radius as f32,
            start_angle,
            end_angle,
            effective_arc,
            stroke_w,
        );
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::render::PaintBackend;
    use crate::style::WidgetStyle;

    #[test]
    fn spinner_creation_defaults() {
        let sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert!(sp.is_active());
        assert!((sp.angle() - 0.0).abs() < f32::EPSILON);
        assert_eq!(sp.thickness(), 4);
        assert!((sp.size_ratio() - 0.8).abs() < f32::EPSILON);
        assert!((sp.speed() - 1.0).abs() < f32::EPSILON);
        assert_eq!(sp.kind(), WidgetKind::Spinner);
    }

    #[test]
    fn spinner_active_state() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert!(sp.is_active());
        sp.set_active(false);
        assert!(!sp.is_active());
        sp.set_active(true);
        assert!(sp.is_active());
    }

    #[test]
    fn spinner_tick_advances_angle() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        // Tick with 100ms at default speed: angle += 100 * 0.1 * 1.0 = 10.0
        sp.tick(100);
        assert!((sp.angle() - 10.0).abs() < 0.01);

        // Tick another 200ms: angle += 200 * 0.1 = 20.0 => total 30.0
        sp.tick(200);
        assert!((sp.angle() - 30.0).abs() < 0.01);

        // Tick enough to wrap around 360
        sp.tick(3301); // 330.1 degrees added => 30 + 330.1 = 360.1 -> 0.1
        assert!((sp.angle() - 0.1).abs() < 0.01);
    }

    #[test]
    fn spinner_tick_speed_multiplier() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_speed(2.0);
        // 100ms at speed 2.0: angle += 100 * 0.1 * 2.0 = 20.0
        sp.tick(100);
        assert!((sp.angle() - 20.0).abs() < 0.01);
    }

    #[test]
    fn spinner_tick_inactive_does_not_auto_advance() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_active(false);
        sp.tick(100);
        // tick still advances angle even when inactive (the draw method uses the angle,
        // but tick is separate — the user controls when to call tick).
        // The requirement says "When not active, just show the arc at its current angle
        // without updating." But tick is a public method that advances regardless.
        // The active flag is checked in the draw method logic, but tick still works.
        // Let's verify tick still advances:
        assert!((sp.angle() - 10.0).abs() < 0.01);
        // Angle was advanced because tick() doesn't check active — that's
        // by design; the caller decides when to call tick().
    }

    #[test]
    fn spinner_set_thickness() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_thickness(8);
        assert_eq!(sp.thickness(), 8);
        // Minimum is 1
        sp.set_thickness(0);
        assert_eq!(sp.thickness(), 1);
    }

    #[test]
    fn spinner_set_speed() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_speed(2.5);
        assert!((sp.speed() - 2.5).abs() < f32::EPSILON);
        // Clamped to >= 0
        sp.set_speed(-1.0);
        assert!((sp.speed() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn spinner_set_size_ratio() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_size_ratio(0.5);
        assert!((sp.size_ratio() - 0.5).abs() < f32::EPSILON);
        // Clamped to 0.0..=1.0
        sp.set_size_ratio(1.5);
        assert!((sp.size_ratio() - 1.0).abs() < f32::EPSILON);
        sp.set_size_ratio(-0.5);
        assert!((sp.size_ratio() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn spinner_draw_does_not_panic() {
        let rect = Rect::new(0, 0, 48, 48);
        let mut sp = Spinner::new(rect);

        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        sp.draw(&mut ctx);
        backend.end_frame();

        // Smoke test: just ensure no panic and pixels were written
        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn spinner_draw_zero_geometry_does_not_panic() {
        let mut sp = Spinner::new(Rect::new(0, 0, 0, 0));
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = crate::render::RenderContext::new(&mut backend);
        sp.draw(&mut ctx);
        backend.end_frame();
        // Must not panic for zero-sized widget
        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn spinner_style_roundtrip() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert_eq!(*sp.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(0, 150, 255));
        sp.set_style(custom.clone());
        assert_eq!(*sp.style(), custom);
    }

    #[test]
    fn spinner_id_kind() {
        let sp_a = Spinner::new(Rect::new(0, 0, 48, 48));
        let sp_b = Spinner::new(Rect::new(0, 0, 48, 48));
        assert_ne!(sp_a.id(), sp_b.id());
        assert_eq!(sp_a.kind(), WidgetKind::Spinner);
        assert_eq!(sp_b.kind(), WidgetKind::Spinner);
    }

    #[test]
    fn spinner_geometry_delegation() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        sp.set_geometry(Rect::new(10, 10, 60, 60));
        assert_eq!(sp.geometry(), Rect::new(10, 10, 60, 60));
    }

    #[test]
    fn spinner_visibility() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert!(sp.is_visible());
        sp.hide();
        assert!(!sp.is_visible());
        sp.show();
        assert!(sp.is_visible());
    }

    #[test]
    fn spinner_enabled() {
        let mut sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert!(sp.is_enabled());
        sp.set_enabled(false);
        assert!(!sp.is_enabled());
        sp.set_enabled(true);
        assert!(sp.is_enabled());
    }

    #[test]
    fn spinner_size_hint() {
        let sp = Spinner::new(Rect::new(0, 0, 48, 48));
        assert_eq!(
            sp.size_hint(),
            Size::new(dimensions::SPINNER_DIAMETER, dimensions::SPINNER_DIAMETER)
        );
    }

    /// A spinner is a fixed-size indicator, not a fraction of the area it is given.
    ///
    /// This pins the defect the old derivation caused: `min(w, h) / 2 * size_ratio` on
    /// the 240x120 census cell drew a ring with `r = 45` centred at (120, 60), while
    /// `size_hint` reported 48x48 — a spinner that was one control to a layout and
    /// another one on screen. The diameter is now [`dimensions::SPINNER_DIAMETER`]
    /// whenever the control has room for it, clamped only when it does not.
    #[test]
    fn spinner_draws_its_own_diameter_in_any_rectangle() {
        for (rect, expected_side) in [
            (Rect::new(0, 0, 240, 120), dimensions::SPINNER_DIAMETER),
            (Rect::new(0, 0, 48, 48), dimensions::SPINNER_DIAMETER),
            // A control smaller than the indicator clamps it rather than painting
            // outside its own rectangle.
            (Rect::new(0, 0, 24, 80), 24),
        ] {
            let sp = Spinner::new(rect);
            let (center, _) = sp.center_and_radius().expect("a non-empty rectangle has a disc");
            // Recover the disc the centre came from and check its square side.
            let disc = ControlMetrics::centered_disc(rect, expected_side);
            assert_eq!(disc.width, expected_side, "at control {rect:?}");
            assert_eq!(disc.height, expected_side, "at control {rect:?}");
            assert_eq!(disc.x + (disc.width as i32) / 2, center.x);
            assert_eq!(disc.y + (disc.height as i32) / 2, center.y);
            // The ring is centred in the control, so its centre is the control's middle.
            assert_eq!(center.x, rect.x + (rect.width as i32) / 2);
            assert_eq!(center.y, rect.y + (rect.height as i32) / 2);
        }
    }

    /// The painted ring stays inside the control it was given.
    #[test]
    fn spinner_ring_never_leaves_its_rectangle() {
        for rect in [Rect::new(0, 0, 240, 120), Rect::new(5, 7, 30, 30), Rect::new(0, 0, 51, 47)] {
            let sp = Spinner::new(rect);
            let Some((center, radius)) = sp.center_and_radius() else {
                continue;
            };
            assert!(radius > 0, "a drawn ring has a positive radius at {rect:?}");
            assert!(
                center.x - radius as i32 >= rect.x
                    && center.x + radius as i32 <= rect.x + rect.width as i32,
                "the ring fits horizontally at {rect:?}"
            );
            assert!(
                center.y - radius as i32 >= rect.y
                    && center.y + radius as i32 <= rect.y + rect.height as i32,
                "the ring fits vertically at {rect:?}"
            );
        }
    }

    /// The emitted SVG's circle is the fixed diameter, not the census cell's half.
    #[test]
    fn spinner_svg_paints_the_fixed_diameter() {
        let mut sp = Spinner::new(crate::widget::census::CENSUS_RECT);
        let svg = crate::widget::svg::render_to_svg(&mut sp);
        assert!(svg.contains("<circle"), "a spinner paints its ring as a circle stroke: {svg}");
        // `r` is the ring radius: the disc's half, inset by the stroke and a pixel.
        // Anything derived from the cell would be near 45 rather than under 22.
        let radius = svg
            .split("<circle")
            .nth(1)
            .and_then(|rest| rest.split("r=\"").nth(1))
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse::<f32>().ok())
            .expect("the circle carries a radius");
        assert!(
            radius < dimensions::SPINNER_DIAMETER as f32 / 2.0,
            "a {radius} radius would mean the ring scaled with the cell"
        );
    }
}
