// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Arc widget — circular progress/indicator (BLUE13 R2.1).
use crate::compat::{format, String};
use crate::core::{deg_to_rad, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::capability::coercion::{expect_bool, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::numeric::ordered_clamp_u32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Arc widget for displaying circular progress or angular values.
pub struct Arc {
    base: BaseWidget,
    /// Current value (0-100 range mapped to arc angle).
    value: u32,
    /// Minimum value.
    min: u32,
    /// Maximum value.
    max: u32,
    /// Start angle in degrees (0 = top, clockwise).
    start_angle: i16,
    /// Total sweep angle in degrees (default 360 for full circle, 270 for gauge).
    sweep_angle: u16,
    /// Arc thickness in pixels.
    thickness: u32,
    /// Whether the arc is rounded at ends.
    rounded: bool,
    /// Whether to show the value as text in the center.
    show_value: bool,
    /// Whether the arc is in indeterminate (spinning) mode.
    indeterminate: bool,
}

impl Arc {
    /// Creates a new arc widget with the given geometry and default settings.
    ///
    /// Default range is 0-100, sweep angle is 360 degrees (full circle),
    /// thickness is 10 pixels, start angle is 0 (top), and the value text
    /// is visible.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Arc, rect, "Arc"),
            value: 0,
            min: 0,
            max: 100,
            start_angle: 0,
            sweep_angle: 360,
            thickness: 10,
            rounded: false,
            show_value: true,
            indeterminate: false,
        }
    }

    /// Returns the current value.
    pub fn value(&self) -> u32 {
        self.value
    }

    /// Returns the minimum value of the range.
    pub fn minimum(&self) -> u32 {
        self.min
    }

    /// Returns the maximum value of the range.
    pub fn maximum(&self) -> u32 {
        self.max
    }

    /// Returns the total sweep angle in degrees.
    pub fn sweep_angle(&self) -> u16 {
        self.sweep_angle
    }

    /// Returns the arc thickness in pixels.
    pub fn thickness(&self) -> u32 {
        self.thickness
    }

    /// Returns whether the arc is drawn in indeterminate (spinning) mode.
    pub fn is_indeterminate(&self) -> bool {
        self.indeterminate
    }

    /// Sets the value, clamped between min and max.
    ///
    /// Emits the `changed` signal when the value actually changes.
    ///
    /// `changed` is deliberately not gated by `enabled`: it reports a *value* transition
    /// from `set_value`, not a user action. A host that disables a progress indicator and
    /// keeps feeding it data still needs the signal to track the value, and a disabled
    /// `Arc` accepts no pointer input anyway, so there is no user-driven path to silence
    /// here (the round-52 `MiniCanvas` defect was the opposite: a disabled control
    /// responding to presses).
    pub fn set_value(&mut self, value: u32) {
        let clamped = ordered_clamp_u32(value, self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.base.changed.emit();
        self.base.request_redraw();
    }

    /// Sets the minimum value of the range, re-clamping the current value.
    pub fn set_minimum(&mut self, minimum: u32) {
        self.set_range(minimum, self.max);
    }

    /// Sets the maximum value of the range, re-clamping the current value.
    pub fn set_maximum(&mut self, maximum: u32) {
        self.set_range(self.min, maximum);
    }

    /// Sets both minimum and maximum values in one call.
    ///
    /// The current value is re-clamped to the new range.
    ///
    /// Like [`Arc::set_value`], `changed` is deliberately not gated by `enabled`.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.min = min.min(max);
        self.max = max.max(min);
        let clamped = ordered_clamp_u32(self.value, self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.base.changed.emit();
        }
        self.base.request_redraw();
    }

    /// Sets the total sweep angle in degrees (e.g., 360 for full circle, 270 for gauge).
    pub fn set_sweep_angle(&mut self, angle: u16) {
        self.sweep_angle = angle;
        self.base.request_redraw();
    }

    /// Sets the arc thickness in pixels.
    pub fn set_thickness(&mut self, thickness: u32) {
        self.thickness = thickness.max(1);
        self.base.request_redraw();
    }

    /// Sets whether the arc ends are rounded.
    pub fn set_rounded(&mut self, rounded: bool) {
        self.rounded = rounded;
        self.base.request_redraw();
    }

    /// Sets whether the value text is shown in the center of the arc.
    pub fn set_show_value(&mut self, show: bool) {
        self.show_value = show;
        self.base.request_redraw();
    }

    /// Sets whether the arc is in indeterminate (spinning indicator) mode.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.indeterminate = indeterminate;
        self.base.request_redraw();
    }

    /// Normalizes the current value to a fraction in [0.0, 1.0].
    fn normalized_value(&self) -> f32 {
        if self.max <= self.min {
            return 0.0;
        }
        (self.value - self.min) as f32 / (self.max - self.min) as f32
    }

    /// Returns the start and end angles in radians for the progress arc.
    ///
    /// The widget API uses degrees with 0 = top, clockwise. Internally the
    /// angles are converted to radians offset by -PI/2 so that 0 radians
    /// corresponds to the 3 o'clock position (standard math convention for
    /// the rendering backend).
    fn arc_angles(&self) -> (f32, f32) {
        let start_deg = self.start_angle as f32;
        let sweep_deg = self.sweep_angle as f32;
        let progress = self.normalized_value();
        let end_deg = start_deg + sweep_deg * progress;
        // Offset by -90 degrees so that 0 (top) → 3 o'clock convention.
        let offset = -90.0_f32;
        (deg_to_rad(start_deg + offset), deg_to_rad(end_deg + offset))
    }

    /// Formats the current value as a percentage string for center display.
    fn format_value_text(&self) -> String {
        if !self.show_value {
            return String::new();
        }
        let pct = self.normalized_value() * 100.0;
        format!("{}%", pct.round() as u32)
    }
}

impl Widget for Arc {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let diameter = self.thickness * 4;
        Size::new(diameter.max(60), diameter.max(60))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Arc`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch. Every name the
/// `ARC_PROPERTIES` schema publishes as readable is answered here, so the schema
/// and the contract cannot disagree about what exists.
impl WidgetProperties for Arc {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::UInt(self.value() as u64)),
            "minimum" => Ok(CapabilityValue::UInt(self.minimum() as u64)),
            "maximum" => Ok(CapabilityValue::UInt(self.maximum() as u64)),
            "thickness" => Ok(CapabilityValue::UInt(self.thickness() as u64)),
            "sweep_angle" => Ok(CapabilityValue::UInt(self.sweep_angle() as u64)),
            "indeterminate" => Ok(CapabilityValue::Bool(self.is_indeterminate())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                self.set_value(expect_u32(value)?);
                Ok(())
            }
            "minimum" => {
                self.set_minimum(expect_u32(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_u32(value)?);
                Ok(())
            }
            "thickness" => {
                self.set_thickness(expect_u32(value)?);
                Ok(())
            }
            "sweep_angle" => {
                self.set_sweep_angle(expect_u32(value)? as u16);
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
        property_names_of![
            "value",
            "minimum",
            "maximum",
            "thickness",
            "sweep_angle",
            "indeterminate",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `arc` publishes.
    ///
    /// Both published names (`set_value`, `set_range`) assign a number and are
    /// answered through the property path (`set("value", ..)`), so neither can be a
    /// zero-argument action; they are refused as
    /// [`CapabilityAccessError::OutOfRange`] rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Arc {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Arc is non-interactive by default; subclasses can override.
    }
}

impl Draw for Arc {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);

        // Resolve colors from style overrides, falling back to defaults.
        let track_color = self.style().background_color.unwrap_or(Color::rgb(220, 220, 220));

        let arc_color = self.style().text_color.unwrap_or(Color::rgb(0, 120, 215));

        // Radius is half the smaller dimension minus a small inset.
        let outer_radius = rect.width.min(rect.height).saturating_sub(2) / 2;
        if outer_radius < self.thickness {
            // Not enough space to draw anything meaningful.
            return;
        }

        // Draw the background track as a filled circle (pancake style).
        context.fill_circle(center, outer_radius, track_color);

        // Draw the progress indicator arc.
        let (start_rad, end_rad) = self.arc_angles();
        let inner_radius = outer_radius.saturating_sub(self.thickness);

        if self.indeterminate {
            // In indeterminate mode, draw a quarter-circle arc as a spinning
            // indicator. The arc covers 90 degrees starting at the computed
            // start angle (which should be animated externally by advancing
            // start_angle over time).
            let indet_sweep = deg_to_rad(90.0);
            context.execute_command(RenderCommand::DrawArc {
                center,
                radius: outer_radius,
                start_angle: start_rad,
                end_angle: start_rad + indet_sweep,
                color: arc_color,
                filled: true,
            });
        } else {
            // Draw the progress arc from start_angle to start_angle + sweep * progress.
            if (end_rad - start_rad).abs() > 0.001 {
                context.execute_command(RenderCommand::DrawArc {
                    center,
                    radius: outer_radius,
                    start_angle: start_rad,
                    end_angle: end_rad,
                    color: arc_color,
                    filled: true,
                });
            }

            // When rounded ends are enabled, draw small filled circles at the
            // start and end of the arc to create a rounded cap appearance.
            if self.rounded && (end_rad - start_rad).abs() > 0.001 {
                let half_thick = (self.thickness as f32 / 2.0).max(1.0) as u32;
                let cap_radius = half_thick.max(2);

                // Cap at the start of the arc.
                let sx = center.x + (outer_radius as f32 * start_rad.cos()) as i32;
                let sy = center.y + (outer_radius as f32 * start_rad.sin()) as i32;
                context.fill_circle(Point::new(sx, sy), cap_radius, arc_color);

                // Cap at the end of the arc.
                let ex = center.x + (outer_radius as f32 * end_rad.cos()) as i32;
                let ey = center.y + (outer_radius as f32 * end_rad.sin()) as i32;
                context.fill_circle(Point::new(ex, ey), cap_radius, arc_color);
            }
        }

        // Draw the inner hole (clear center) so the arc looks like a ring.
        // We draw a filled circle in the background color over the center,
        // simulating a donut punch-out.
        if inner_radius > 0 {
            // Use the style background color, or fall back to the track color
            // (which was already drawn as the full background circle). This
            // creates a visible donut hole rather than relying on TRANSPARENT
            // which can't erase the filled circle behind it.
            let bg = self.style().background_color.unwrap_or(Color::WHITE);
            context.fill_circle(center, inner_radius, bg);
        }

        // Draw the value text in the centre of the arc, if enabled.
        //
        // The label belongs in the **ring's hole**, not in the middle of the control: the
        // hole's diameter is `2 * inner_radius`, and an arc drawn as a ring only has room for
        // text narrower and shorter than that hole. The old placement centred the text in the
        // whole control, so on a ring whose hole is smaller than the label the digits were
        // painted over the ring itself. It is also dropped when it cannot fit, rather than
        // straddling the arc: a reading that covers the progress it reports is worse than no
        // reading, and the raster backend hides the overflow while the SVG one shows it.
        let text = self.format_value_text();
        if !text.is_empty() && inner_radius > 0 {
            let font = Font::default();
            let metrics = context.measure_text(&text, &font);
            let hole = inner_radius.saturating_mul(2);
            // A one-pixel margin on each side of the hole, so the glyphs do not touch the
            // ring they sit inside.
            let fits =
                metrics.width <= hole.saturating_sub(2) && metrics.height <= hole.saturating_sub(2);
            if fits {
                let text_x = center.x - (metrics.width as i32 / 2);
                let text_y = center.y - (metrics.height as i32 / 2);
                let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
                context.draw_text(
                    Point::new(text_x, text_y),
                    &text,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    #[test]
    fn arc_creation_defaults() {
        let arc = Arc::new(Rect::new(0, 0, 200, 200));
        assert_eq!(arc.value(), 0);
        assert_eq!(arc.min, 0);
        assert_eq!(arc.max, 100);
        assert_eq!(arc.sweep_angle, 360);
        assert_eq!(arc.thickness, 10);
        assert!(!arc.rounded);
        assert!(arc.show_value);
        assert!(!arc.indeterminate);
        assert_eq!(arc.start_angle, 0);
    }

    #[test]
    fn arc_set_value_clamps() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_value(50);
        assert_eq!(arc.value(), 50);

        // Above max should clamp to 100.
        arc.set_value(200);
        assert_eq!(arc.value(), 100);

        // Below min should clamp to 0.
        arc.set_value(0);
        assert_eq!(arc.value(), 0);
    }

    #[test]
    fn arc_set_range() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_range(10, 50);
        assert_eq!(arc.min, 10);
        assert_eq!(arc.max, 50);

        // Value should be re-clamped to the new range.
        arc.set_value(30);
        assert_eq!(arc.value(), 30);

        // Value below new min clamps up.
        arc.set_value(5);
        assert_eq!(arc.value(), 10);

        // Value above new max clamps down.
        arc.set_value(60);
        assert_eq!(arc.value(), 50);
    }

    #[test]
    fn arc_indeterminate_mode() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        assert!(!arc.indeterminate);

        arc.set_indeterminate(true);
        assert!(arc.indeterminate);

        arc.set_indeterminate(false);
        assert!(!arc.indeterminate);
    }

    #[test]
    fn arc_draw_does_not_panic() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_value(65);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        arc.draw(&mut context);
        backend.end_frame();
    }

    /// The `0%` reading is drawn in the ring's **hole**, and dropped when it cannot fit there.
    ///
    /// The label used to be centred on the control and printed whatever its size, so on a ring
    /// whose hole is smaller than the text the digits were painted over the arc they annotate —
    /// invisible in a raster (clipped) and visible in the SVG. This pins the two halves of the
    /// fix: a roomy ring still shows the reading, and a ring with no hole at all (a disc, below
    /// the thickness) lays down no text ink.
    ///
    /// # Why the measure is ink and not a `<text>` element
    ///
    /// The backend no longer hands the string to the viewer's font engine: it emits the same
    /// `font8x8` rectangles the software rasteriser fills, inside a single `<path>` (see
    /// `text_subpath_count`). The string is therefore absent from the document in every form,
    /// and "was the reading drawn?" is a question about **ink** — subpath count and ink box —
    /// rather than about the presence of an element.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn arc_value_is_drawn_only_when_it_fits_in_the_ring_hole() {
        /// The ink the reading leaves, as `(subpaths, ink box)`.
        fn reading_ink(arc: &mut Arc, side: u32) -> (usize, Option<(i32, i32, i32, i32)>) {
            let mut backend = SoftwarePaintBackend::new(Size::new(side, side), 1.0);
            backend.begin_frame(Color::WHITE);
            {
                let mut context = RenderContext::new(&mut backend);
                arc.draw(&mut context);
            }
            backend.end_frame();
            let svg = crate::widget::svg::render_to_svg(arc);
            (crate::widget::svg::text_subpath_count(&svg), crate::widget::svg::text_ink_box(&svg))
        }

        // A 200 px ring with the default 20 px thickness has a 140 px hole: the 0% fits.
        let mut roomy = Arc::new(Rect::new(0, 0, 200, 200));
        roomy.set_show_value(true);
        let (subpaths, ink) = reading_ink(&mut roomy, 200);
        assert!(subpaths > 0, "a reading that fits inside the ring hole must still be drawn");
        // …and it is drawn in the hole, not over the ring. `0%` is `0`'s bitmap followed by
        // `%`'s, so the run's ink is one pixel narrower on each side than the glyph box the
        // width test above accepted. A label left at the arc's own radius, or one wider than
        // the hole, overflows these bounds.
        let (left, top, right, _) = ink.expect("the drawn reading has an ink box");
        let field = roomy.geometry();
        let outer_radius = field.width.min(field.height).saturating_sub(2) / 2;
        let inner_radius = outer_radius.saturating_sub(roomy.thickness);
        let center_x = field.x + field.width as i32 / 2;
        let center_y = field.y + field.height as i32 / 2;
        // The run is drawn on the hole's **own** centre line and its ink fits inside the hole:
        // `inner_radius` is that hole's half-width, so both edges of the ink are within
        // `inner_radius - 1` of `center_x`. A reading placed at the arc's own radius (the
        // defect this pins) overshoots that bound; one positioned from the field rather than
        // from the hole lands off the hole's centre line entirely.
        assert!(
            left >= center_x - inner_radius as i32 && right <= center_x + inner_radius as i32,
            "the reading's ink {left}..{right} must lie within the hole's centre ± {inner_radius}"
        );
        assert_eq!(
            left + right,
            2 * center_x - 1,
            "the reading hangs on the hole's own centre line, not the field's"
        );
        assert_eq!(top, center_y - 7, "and is centred on the hole's middle line");

        // A ring whose thickness equals its radius has no hole for the label, so there is
        // nowhere legal to put it — it is dropped rather than overprinted on the arc.
        let mut solid = Arc::new(Rect::new(0, 0, 200, 200));
        solid.set_show_value(true);
        solid.set_thickness(99);
        assert_eq!(
            reading_ink(&mut solid, 200),
            (0, None),
            "with no ring hole the reading must be dropped, not drawn over the arc"
        );
    }

    #[test]
    fn arc_normalized_value_returns_zero_for_empty_range() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        arc.set_range(50, 50);
        assert!((arc.normalized_value() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn arc_normalized_value_half() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        arc.set_value(50);
        assert!((arc.normalized_value() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn arc_set_sweep_angle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert_eq!(arc.sweep_angle, 360);
        arc.set_sweep_angle(270);
        assert_eq!(arc.sweep_angle, 270);
    }

    #[test]
    fn arc_set_thickness() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert_eq!(arc.thickness, 10);
        arc.set_thickness(20);
        assert_eq!(arc.thickness, 20);
        // Thickness should not drop below 1.
        arc.set_thickness(0);
        assert_eq!(arc.thickness, 1);
    }

    #[test]
    fn arc_rounded_toggle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert!(!arc.rounded);
        arc.set_rounded(true);
        assert!(arc.rounded);
    }

    #[test]
    fn arc_show_value_toggle() {
        let mut arc = Arc::new(Rect::new(0, 0, 100, 100));
        assert!(arc.show_value);
        arc.set_show_value(false);
        assert!(!arc.show_value);
    }

    #[test]
    fn arc_geometry_delegation() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_geometry(Rect::new(10, 10, 150, 150));
        assert_eq!(arc.geometry(), Rect::new(10, 10, 150, 150));
    }

    #[test]
    fn arc_size_hint() {
        let arc = Arc::new(Rect::new(0, 0, 200, 200));
        let hint = arc.size_hint();
        assert_eq!(hint, Size::new(60, 60));
    }

    #[test]
    fn arc_draw_indeterminate_does_not_panic() {
        let mut arc = Arc::new(Rect::new(0, 0, 200, 200));
        arc.set_indeterminate(true);

        let mut backend = SoftwarePaintBackend::new(Size::new(200, 200), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        arc.draw(&mut context);
        backend.end_frame();
    }
}
