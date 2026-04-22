use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Command link widget for command link buttons.
pub struct CommandLink {
    base: BaseWidget,
    text: String,
    description: String,
    enabled: bool,
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
            enabled: true,
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
        self.enabled
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
        self.enabled = enabled;
        self.base.request_redraw();
    }

    pub fn click(&self) {
        if self.enabled {
            self.clicked.emit();
        }
    }
}

impl Widget for CommandLink {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.enabled && self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}

impl EventHandler for CommandLink {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { button: 1, .. } => {
                if self.enabled {
                    self.clicked.emit();
                }
            }
            Event::MouseEnter { .. } => {
                self.hovered.emit(true);
            }
            Event::MouseLeave { .. } => {
                self.hovered.emit(false);
            }
            _ => {}
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

        let is_hovered = self.hovered.slot_count() > 0;
        let is_enabled = self.enabled && self.base.is_enabled();

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

        context.draw_text(
            Point::new(text_x, text_y),
            &self.text,
            &text_font,
            current_text_color,
        );

        // Draw description if present
        if !self.description.is_empty() {
            let desc_font = Font::new("Arial", 10.0, false, false);
            let desc_color = if !is_enabled {
                disabled_color
            } else {
                Color::GRAY
            };

            let desc_x = text_x;
            let desc_y = text_y + 16;

            context.draw_text(
                Point::new(desc_x, desc_y),
                &self.description,
                &desc_font,
                desc_color,
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
