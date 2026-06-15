//! Progress bar widget.
use crate::core::{HorizontalAlignment, Color, Font, Orientation, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Progress bar widget.
pub struct ProgressBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    text_visible: bool,
    orientation: Orientation,
    inverted_appearance: bool,
    pub value_changed: Signal1<i32>,
}
impl ProgressBar {
    /// Creates a progress bar with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressBar, geometry, "ProgressBar"),
            minimum: 0,
            maximum: 100,
            value: 0,
            text_visible: true,
            orientation: Orientation::Horizontal,
            inverted_appearance: false,
            value_changed: Signal1::new(),
        }
    }
    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value(self.value); // Re-clamp
    }
    /// Returns maximum value.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value(self.value); // Re-clamp
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
        self.set_value(self.value); // Re-clamp
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(self.value);
    }
    /// Resets progress bar to minimum value.
    pub fn reset(&mut self) {
        self.set_value(self.minimum);
    }
    /// Returns whether text is visible.
    pub fn is_text_visible(&self) -> bool {
        self.text_visible
    }
    /// Sets text visibility.
    pub fn set_text_visible(&mut self, visible: bool) {
        self.text_visible = visible;
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }
    /// Returns whether appearance is inverted.
    pub fn is_inverted_appearance(&self) -> bool {
        self.inverted_appearance
    }
    /// Sets inverted appearance.
    pub fn set_inverted_appearance(&mut self, inverted: bool) {
        self.inverted_appearance = inverted;
    }
    /// Returns progress as percentage (0 to 1).
    pub fn progress(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0.0;
        }
        // Use saturating_sub to prevent integer overflow.
        ((self.value.saturating_sub(self.minimum)) as f32)
            / ((self.maximum.saturating_sub(self.minimum)) as f32)
    }
    /// Returns formatted text for display.
    fn format_text(&self) -> String {
        if !self.text_visible {
            return String::new();
        }
        let percentage = self.progress() * 100.0;
        format!("{}%", percentage.round() as i32)
    }
}
// Implement Widget trait
impl Widget for ProgressBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        match self.orientation() {
            Orientation::Horizontal => Size::new(120, 20),
            Orientation::Vertical => Size::new(20, 120),
        }
    }
}
impl EventHandler for ProgressBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Progress bar is usually non-interactive
    }
}
impl Draw for ProgressBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let progress = self.progress();
        let style = self.style();
        let bg = style.background_color.unwrap_or(Color::from_rgb(240, 240, 240));
        let text_color = style.text_color.unwrap_or(Color::from_rgb(0, 0, 0));
        // Draw background
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), border_color);
        }
        // Draw progress bar
        match self.orientation {
            Orientation::Horizontal => {
                let progress_width = (rect.width as f32 * progress) as u32;
                let x = if self.inverted_appearance {
                    rect.x + rect.width as i32 - progress_width as i32
                } else {
                    rect.x
                };
                context.fill_rect(
                    Rect::new(x, rect.y, progress_width, rect.height),
                    Color::from_rgb(0, 120, 215),
                );
            }
            Orientation::Vertical => {
                let progress_height = (rect.height as f32 * progress) as u32;
                let y = if self.inverted_appearance {
                    rect.y
                } else {
                    rect.y + rect.height as i32 - progress_height as i32
                };
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, progress_height),
                    Color::from_rgb(0, 120, 215),
                );
            }
        }
        // Draw text if visible
        let text = self.format_text();
        if !text.is_empty() {
            context.draw_text(
                Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2),
                &text,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Orientation, Rect, Size};
    use crate::style::WidgetStyle;

    #[test]
    fn progressbar_creation_defaults() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert_eq!(pb.value(), 0);
        assert_eq!(pb.minimum(), 0);
        assert_eq!(pb.maximum(), 100);
        assert!(pb.is_text_visible());
        assert_eq!(pb.orientation(), Orientation::Horizontal);
        assert!(!pb.is_inverted_appearance());
    }

    #[test]
    fn progressbar_set_value() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        assert_eq!(pb.value(), 50);
        pb.set_value(200); // clamp to max
        assert_eq!(pb.value(), 100);
        pb.set_value(-10); // clamp to min
        assert_eq!(pb.value(), 0);
    }

    #[test]
    fn progressbar_set_range() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_minimum(10);
        pb.set_maximum(200);
        assert_eq!(pb.minimum(), 10);
        assert_eq!(pb.maximum(), 200);
    }

    #[test]
    fn progressbar_set_range_reclamps_value() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        pb.set_range(60, 100);
        assert_eq!(pb.value(), 60);
    }

    #[test]
    fn progressbar_orientation() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_orientation(Orientation::Vertical);
        assert_eq!(pb.orientation(), Orientation::Vertical);
        pb.set_orientation(Orientation::Horizontal);
        assert_eq!(pb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn progressbar_text_visible() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_text_visible());
        pb.set_text_visible(false);
        assert!(!pb.is_text_visible());
        pb.set_text_visible(true);
        assert!(pb.is_text_visible());
    }

    #[test]
    fn progressbar_inverted_appearance() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(!pb.is_inverted_appearance());
        pb.set_inverted_appearance(true);
        assert!(pb.is_inverted_appearance());
        pb.set_inverted_appearance(false);
        assert!(!pb.is_inverted_appearance());
    }

    #[test]
    fn progressbar_reset() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(75);
        assert_eq!(pb.value(), 75);
        pb.reset();
        assert_eq!(pb.value(), 0);
    }

    #[test]
    fn progressbar_progress_percentage() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!((pb.progress() - 0.0).abs() < f32::EPSILON);

        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        assert!((pb.progress() - 0.5).abs() < f32::EPSILON);

        pb.set_value(100);
        assert!((pb.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progressbar_geometry_delegation() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(pb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn progressbar_visibility() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_visible());
        pb.hide();
        assert!(!pb.is_visible());
        pb.show();
        assert!(pb.is_visible());
    }

    #[test]
    fn progressbar_enabled() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_enabled());
        pb.set_enabled(false);
        assert!(!pb.is_enabled());
        pb.set_enabled(true);
        assert!(pb.is_enabled());
    }

    #[test]
    fn progressbar_tooltip_roundtrip() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.tooltip().is_empty());
        pb.set_tooltip("Progress info".to_string());
        assert_eq!(pb.tooltip(), "Progress info");
        pb.set_tooltip(String::new());
        assert!(pb.tooltip().is_empty());
    }

    #[test]
    fn progressbar_style_roundtrip() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert_eq!(*pb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::from_rgb(220, 220, 220));
        pb.set_style(custom.clone());
        assert_eq!(*pb.style(), custom);
    }

    #[test]
    fn progressbar_id_kind() {
        let pb_a = ProgressBar::new(Rect::new(0, 0, 100, 20));
        let pb_b = ProgressBar::new(Rect::new(0, 0, 100, 20));
        assert_ne!(pb_a.id(), pb_b.id());
        assert_eq!(pb_a.kind(), WidgetKind::ProgressBar);
        assert_eq!(pb_b.kind(), WidgetKind::ProgressBar);
    }

    #[test]
    fn progressbar_signal_accessors() {
        let pb = ProgressBar::new(Rect::new(0, 0, 100, 20));
        let _value_changed = &pb.value_changed;
    }

    #[test]
    fn progressbar_size_hint_horizontal() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        let hint = pb.size_hint();
        assert_eq!(hint, Size::new(120, 20));
    }

    #[test]
    fn progressbar_size_hint_vertical() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_orientation(Orientation::Vertical);
        let hint = pb.size_hint();
        assert_eq!(hint, Size::new(20, 120));
    }
}
