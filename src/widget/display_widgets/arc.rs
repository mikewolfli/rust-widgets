//! Arc widget — circular progress/indicator (BLUE13 R2.1).
use crate::core::{Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{RenderCommand, RenderContext};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Converts degrees to radians.
fn deg_to_rad(deg: f32) -> f32 {
    deg * std::f32::consts::PI / 180.0
}

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

    /// Sets the value, clamped between min and max.
    ///
    /// Emits the `changed` signal when the value actually changes.
    pub fn set_value(&mut self, value: u32) {
        let clamped = value.clamp(self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.base.changed.emit();
    }

    /// Sets both minimum and maximum values in one call.
    ///
    /// The current value is re-clamped to the new range.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.min = min.min(max);
        self.max = max.max(min);
        let clamped = self.value.clamp(self.min, self.max);
        if self.value != clamped {
            self.value = clamped;
            self.base.changed.emit();
        }
    }

    /// Sets the total sweep angle in degrees (e.g., 360 for full circle, 270 for gauge).
    pub fn set_sweep_angle(&mut self, angle: u16) {
        self.sweep_angle = angle;
    }

    /// Sets the arc thickness in pixels.
    pub fn set_thickness(&mut self, thickness: u32) {
        self.thickness = thickness.max(1);
    }

    /// Sets whether the arc ends are rounded.
    pub fn set_rounded(&mut self, rounded: bool) {
        self.rounded = rounded;
    }

    /// Sets whether the value text is shown in the center of the arc.
    pub fn set_show_value(&mut self, show: bool) {
        self.show_value = show;
    }

    /// Sets whether the arc is in indeterminate (spinning indicator) mode.
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.indeterminate = indeterminate;
    }

    /// Returns a reference to the base widget state.
    pub fn base(&self) -> &BaseWidget {
        &self.base
    }

    /// Returns a mutable reference to the base widget state.
    pub fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
        let track_color = self.style().background_color.unwrap_or(Color::from_rgb(220, 220, 220));

        let arc_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 120, 215));

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
            // Use a neutral light fill that blends with the parent background.
            // Since we don't have compositing, draw over with a white-ish color
            // that matches the typical canvas background.
            context.fill_circle(center, inner_radius, Color::WHITE);
        }

        // Draw the value text in the center of the arc, if enabled.
        let text = self.format_value_text();
        if !text.is_empty() {
            let font = Font::default();
            let metrics = context.measure_text(&text, &font);
            let text_width = metrics.width;
            let text_height = metrics.height;

            let text_x = center.x - (text_width as i32 / 2);
            let text_y = center.y - (text_height as i32 / 2);

            let text_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0));
            context.draw_text(Point::new(text_x, text_y), &text, &font, text_color);
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
