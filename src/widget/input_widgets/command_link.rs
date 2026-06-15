use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Command link widget for command link buttons.
pub struct CommandLink {
    base: BaseWidget,
    text: String,
    description: String,
    is_hovered: bool,
    /// Emitted when command link is clicked.
    pub clicked: GenericSignal,
    /// Emitted when command link is hovered.
    pub hovered: Signal1<bool>,
}
impl CommandLink {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CommandLink, geometry, "CommandLink"),
            text: "Command".to_string(),
            description: "".to_string(),
            is_hovered: false,
            clicked: GenericSignal::new(),
            hovered: Signal1::new(),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    pub fn set_description(&mut self, description: String) {
        self.description = description;
        self.base.request_redraw();
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
        self.base.request_redraw();
    }
    pub fn click(&self) {
        if self.base.is_enabled() {
            self.clicked.emit();
        }
    }
}
impl Widget for CommandLink {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
}
impl EventHandler for CommandLink {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { button: 1, .. } if self.base.is_enabled() => {
                self.clicked.emit();
            }
            Event::MouseEnter { .. } => {
                self.is_hovered = true;
                self.hovered.emit(true);
            }
            Event::MouseLeave { .. } => {
                self.is_hovered = false;
                self.hovered.emit(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for CommandLink {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::TRANSPARENT);
        let text_color = style.text_color.unwrap_or(Color::rgb(0, 102, 204));
        let hover_color = Color::rgb(0, 0, 255);
        let disabled_color = Color::GRAY;
        let is_hovered = self.is_hovered;
        let is_enabled = self.base.is_enabled();
        // Draw background (transparent by default)
        if bg_color != Color::TRANSPARENT {
            context.fill_rect(rect, bg_color);
        }
        // Determine text color based on state
        let current_text_color = if !is_enabled {
            disabled_color
        } else if is_hovered {
            hover_color
        } else {
            text_color
        };
        // Draw main text
        let padding = &style.padding;
        let text_font = Font::new("Arial", 12.0, false, true);
        let text_x = rect.x + padding.left as i32;
        let text_y = rect.y + padding.top as i32 + 12;
        context.draw_text(Point::new(text_x, text_y), &self.text, &text_font, current_text_color, HorizontalAlignment::Left);
        // Draw description if present
        if !self.description.is_empty() {
            let desc_font = Font::new("Arial", 10.0, false, false);
            let desc_color = if !is_enabled { disabled_color } else { Color::GRAY };
            let desc_x = text_x;
            let desc_y = text_y + 16;
            context.draw_text(
                Point::new(desc_x, desc_y),
                &self.description,
                &desc_font,
                desc_color,
                HorizontalAlignment::Left,
            );
        }
        // Draw underline for hover state
        if is_hovered && is_enabled {
            let text_metrics = context.measure_text(&self.text, &text_font);
            let underline_y = text_y + text_metrics.height as i32 + 2;
            context.draw_line(
                Point::new(text_x, underline_y),
                Point::new(text_x + text_metrics.width as i32, underline_y),
                current_text_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn commandlink_creation_defaults() {
        let cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert_eq!(cl.text(), "Command");
        assert!(cl.description().is_empty());
        assert!(cl.is_enabled());
    }

    #[test]
    fn commandlink_set_text() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_text("Save".to_string());
        assert_eq!(cl.text(), "Save");
    }

    #[test]
    fn commandlink_set_description() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_description("Save the current document".to_string());
        assert_eq!(cl.description(), "Save the current document");
        cl.set_description(String::new());
        assert!(cl.description().is_empty());
    }

    #[test]
    fn commandlink_set_enabled() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.is_enabled());
        cl.set_enabled(false);
        assert!(!cl.is_enabled());
        cl.set_enabled(true);
        assert!(cl.is_enabled());
    }

    #[test]
    fn commandlink_set_enabled_updates_base_state() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.base().is_enabled());
        cl.set_enabled(false);
        assert!(!cl.base().is_enabled());
        cl.set_enabled(true);
        assert!(cl.base().is_enabled());
    }

    #[test]
    fn commandlink_click() {
        let cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.click(); // Should not panic
    }

    #[test]
    fn commandlink_geometry_delegation() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        cl.set_geometry(Rect::new(10, 10, 400, 80));
        assert_eq!(cl.geometry(), Rect::new(10, 10, 400, 80));
    }

    #[test]
    fn commandlink_visibility() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.is_visible());
        cl.hide();
        assert!(!cl.is_visible());
        cl.show();
        assert!(cl.is_visible());
    }

    #[test]
    fn commandlink_tooltip_roundtrip() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert!(cl.tooltip().is_empty());
        cl.set_tooltip("Click here".to_string());
        assert_eq!(cl.tooltip(), "Click here");
        cl.set_tooltip(String::new());
        assert!(cl.tooltip().is_empty());
    }

    #[test]
    fn commandlink_style_roundtrip() {
        let mut cl = CommandLink::new(Rect::new(0, 0, 300, 60));
        assert_eq!(*cl.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(240, 240, 240));
        cl.set_style(custom.clone());
        assert_eq!(*cl.style(), custom);
    }

    #[test]
    fn commandlink_id_kind() {
        let cl_a = CommandLink::new(Rect::new(0, 0, 100, 50));
        let cl_b = CommandLink::new(Rect::new(0, 0, 100, 50));
        assert_ne!(cl_a.id(), cl_b.id());
        assert_eq!(cl_a.kind(), WidgetKind::CommandLink);
        assert_eq!(cl_b.kind(), WidgetKind::CommandLink);
    }

    #[test]
    fn commandlink_signal_accessors() {
        let cl = CommandLink::new(Rect::new(0, 0, 100, 50));
        let _clicked = &cl.clicked;
        let _hovered = &cl.hovered;
    }
}
