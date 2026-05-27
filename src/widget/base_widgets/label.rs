//! Label widget implementation.
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Label widget for displaying text.
pub struct Label {
    base: BaseWidget,
    text: String,
    alignment: crate::core::Alignment,
}
impl Label {
    /// Creates a label with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "Label"),
            text,
            alignment: crate::core::Alignment::Left,
        }
    }
    /// Returns label text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets label text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    /// Returns text alignment.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }
    /// Sets text alignment.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        self.alignment = alignment;
        self.base.request_redraw();
    }
}
impl Widget for Label {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let text_w = self.text().len() as u32 * 8 + 4;
        Size::new(text_w.max(16), 20)
    }
}
impl EventHandler for Label {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for Label {
    fn draw(&mut self, context: &mut RenderContext) {
        // Label rendering logic
        let rect = self.geometry();
        // Draw background if specified
        if let Some(bg_color) = self.style().background_color {
            context.fill_rect(rect, bg_color);
        }
        // Draw text
        if !self.text.is_empty() {
            let text_color = self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0));
            context.draw_text(
                Point::new(rect.x, rect.y),
                &self.text,
                &self.font().cloned().unwrap_or_default(),
                text_color,
            );
        }
        // Draw border if specified
        if let Some(border_color) = self.style().border_color {
            context.draw_rect(rect, border_color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Alignment, Color, Font, ObjectId, Rect, Size};
    use crate::style::WidgetStyle;

    // ------------------------------------------------------------------
    // 1. Label creation (text, geometry)
    // ------------------------------------------------------------------

    #[test]
    fn label_creation_sets_text_and_geometry() {
        let rect = Rect::new(10, 20, 200, 30);
        let label = Label::new("Hello, World!".to_string(), rect);

        assert_eq!(label.text(), "Hello, World!");
        assert_eq!(label.geometry(), rect);
    }

    #[test]
    fn label_creation_default_alignment_is_left() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_creation_with_empty_text() {
        let label = Label::new(String::new(), Rect::new(0, 0, 50, 16));
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_creation_with_zero_geometry() {
        let label = Label::new("Zero".to_string(), Rect::new(0, 0, 0, 0));
        assert_eq!(label.geometry(), Rect::new(0, 0, 0, 0));
    }

    // ------------------------------------------------------------------
    // 2. Text set / get
    // ------------------------------------------------------------------

    #[test]
    fn label_set_text_updates_stored_text() {
        let mut label = Label::new("Initial".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Updated".to_string());
        assert_eq!(label.text(), "Updated");
    }

    #[test]
    fn label_set_text_overwrites_previous() {
        let mut label = Label::new("First".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text("Second".to_string());
        label.set_text("Third".to_string());
        assert_eq!(label.text(), "Third");
    }

    #[test]
    fn label_set_text_empty() {
        let mut label = Label::new("Something".to_string(), Rect::new(0, 0, 100, 20));
        label.set_text(String::new());
        assert!(label.text().is_empty());
    }

    #[test]
    fn label_set_text_long_string() {
        let long = "a".repeat(10_000);
        let mut label = Label::new(String::new(), Rect::new(0, 0, 100, 20));
        label.set_text(long.clone());
        assert_eq!(label.text(), long);
    }

    // ------------------------------------------------------------------
    // 3. Alignment set / get (default Left, set Center / Right)
    // ------------------------------------------------------------------

    #[test]
    fn label_alignment_default_is_left() {
        let label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_center() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Center);
        assert_eq!(label.alignment(), Alignment::Center);
    }

    #[test]
    fn label_set_alignment_right() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Right);
        assert_eq!(label.alignment(), Alignment::Right);
    }

    #[test]
    fn label_set_alignment_left_explicitly() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        // Start with Center, then go back to Left
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Left);
        assert_eq!(label.alignment(), Alignment::Left);
    }

    #[test]
    fn label_set_alignment_top_and_bottom() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
        label.set_alignment(Alignment::Bottom);
        assert_eq!(label.alignment(), Alignment::Bottom);
    }

    #[test]
    fn label_alignment_set_multiple_times_keeps_last() {
        let mut label = Label::new("Hello".to_string(), Rect::new(0, 0, 100, 20));
        label.set_alignment(Alignment::Left);
        label.set_alignment(Alignment::Right);
        label.set_alignment(Alignment::Center);
        label.set_alignment(Alignment::Top);
        assert_eq!(label.alignment(), Alignment::Top);
    }

    // ------------------------------------------------------------------
    // 4. Widget trait delegation (geometry, visibility, enabled, parent,
    //    children, min/max size, tooltip, style, id, kind)
    // ------------------------------------------------------------------

    #[test]
    fn widget_geometry_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(5, 10, 150, 25));
        assert_eq!(label.geometry(), Rect::new(5, 10, 150, 25));

        label.set_geometry(Rect::new(20, 30, 300, 50));
        assert_eq!(label.geometry(), Rect::new(20, 30, 300, 50));
    }

    #[test]
    fn widget_visibility_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_visible(), "Label should be visible by default");

        label.hide();
        assert!(!label.is_visible(), "Label should be hidden after hide()");

        label.show();
        assert!(label.is_visible(), "Label should be visible after show()");
    }

    #[test]
    fn widget_enabled_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.is_enabled(), "Label should be enabled by default");

        label.set_enabled(false);
        assert!(!label.is_enabled(), "Label should be disabled");

        label.set_enabled(true);
        assert!(label.is_enabled(), "Label should be re-enabled");
    }

    #[test]
    fn widget_parent_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert_eq!(label.parent(), None);

        let parent_id: ObjectId = 42;
        label.set_parent(Some(parent_id));
        assert_eq!(label.parent(), Some(parent_id));

        label.set_parent(None);
        assert_eq!(label.parent(), None);
    }

    #[test]
    fn widget_children_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.children().is_empty());

        let child_a: ObjectId = 100;
        let child_b: ObjectId = 200;

        label.add_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_a);

        label.add_child(child_b);
        assert_eq!(label.children().len(), 2);

        label.remove_child(child_a);
        assert_eq!(label.children().len(), 1);
        assert_eq!(label.children()[0], child_b);

        label.remove_child(child_b);
        assert!(label.children().is_empty());
    }

    #[test]
    fn widget_min_max_size_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default min/max sizes
        assert_eq!(label.min_size(), None);
        assert_eq!(label.max_size(), None);

        // Set min size
        let min = Size::new(80, 16);
        label.set_min_size(Some(min));
        assert_eq!(label.min_size(), Some(min));

        // Set max size
        let max = Size::new(400, 100);
        label.set_max_size(Some(max));
        assert_eq!(label.max_size(), Some(max));

        // Clear min size
        label.set_min_size(None);
        assert_eq!(label.min_size(), None);
    }

    #[test]
    fn widget_tooltip_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        assert!(label.tooltip().is_empty());

        label.set_tooltip("Helpful tip".to_string());
        assert_eq!(label.tooltip(), "Helpful tip");

        label.set_tooltip(String::new());
        assert!(label.tooltip().is_empty());
    }

    #[test]
    fn widget_style_delegation() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));

        // Default style
        assert_eq!(*label.style(), WidgetStyle::default());

        // Set a custom style
        let custom_style = WidgetStyle::default().with_background(Color::from_rgb(240, 240, 240));
        label.set_style(custom_style.clone());
        assert_eq!(*label.style(), custom_style);
    }

    #[test]
    fn widget_id_is_unique_and_kind_is_label() {
        let label_a = Label::new("A".to_string(), Rect::new(0, 0, 100, 20));
        let label_b = Label::new("B".to_string(), Rect::new(0, 0, 100, 20));

        // Each widget gets a unique ObjectId
        assert_ne!(label_a.id(), label_b.id());

        // Kind must be Label
        assert_eq!(label_a.kind(), WidgetKind::Label);
        assert_eq!(label_b.kind(), WidgetKind::Label);
    }

    // ------------------------------------------------------------------
    // 5. Style properties (background_color, text_color, font,
    //    border_color / width / radius)
    // ------------------------------------------------------------------

    #[test]
    fn label_style_default_values() {
        let label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = label.style();

        assert_eq!(style.background_color, None);
        assert_eq!(style.text_color, None);
        assert_eq!(style.font, None);
        assert_eq!(style.border_color, None);
        assert_eq!(style.border_width, 0);
        assert_eq!(style.border_radius, 0);
    }

    #[test]
    fn label_style_background_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let bg = Color::from_rgb(200, 210, 220);

        let style = WidgetStyle::default().with_background(bg);
        label.set_style(style);

        assert_eq!(label.style().background_color, Some(bg));
    }

    #[test]
    fn label_style_text_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let tc = Color::from_rgb(50, 80, 200);

        let style = WidgetStyle::default().with_text_color(tc);
        label.set_style(style);

        assert_eq!(label.style().text_color, Some(tc));
    }

    #[test]
    fn label_style_font() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let font = Font::simple("Helvetica", 16.0);

        let style = WidgetStyle::default().with_font(font.clone());
        label.set_style(style);

        assert_eq!(label.style().font, Some(font));
    }

    #[test]
    fn label_style_border_color() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let border = Color::from_rgb(255, 0, 0);

        let style = WidgetStyle::default().with_border(border, 2, 4);
        label.set_style(style);

        assert_eq!(label.style().border_color, Some(border));
    }

    #[test]
    fn label_style_border_width() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::from_rgb(0, 0, 0), 5, 0);
        label.set_style(style);

        assert_eq!(label.style().border_width, 5);
    }

    #[test]
    fn label_style_border_radius() {
        let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 20));
        let style = WidgetStyle::default().with_border(Color::from_rgb(0, 0, 0), 1, 8);
        label.set_style(style);

        assert_eq!(label.style().border_radius, 8);
    }

    #[test]
    fn label_style_combined_properties() {
        let mut label = Label::new("Styled".to_string(), Rect::new(0, 0, 200, 40));
        let bg = Color::from_rgb(240, 248, 255);
        let tc = Color::from_rgb(0, 51, 102);
        let font = Font::simple("Georgia", 18.0);
        let bc = Color::from_rgb(0, 102, 204);

        let style = WidgetStyle::default()
            .with_background(bg)
            .with_text_color(tc)
            .with_font(font.clone())
            .with_border(bc, 2, 6);

        label.set_style(style);

        let s = label.style();
        assert_eq!(s.background_color, Some(bg));
        assert_eq!(s.text_color, Some(tc));
        assert_eq!(s.font, Some(font));
        assert_eq!(s.border_color, Some(bc));
        assert_eq!(s.border_width, 2);
        assert_eq!(s.border_radius, 6);
    }
}
