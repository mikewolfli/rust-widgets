//! Group box widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Group box widget.
pub struct GroupBox {
    base: BaseWidget,
    title: String,
    alignment: Alignment,
    checkable: bool,
    checked: bool,
    pub toggled: Signal1<bool>,
    /// Cached title width computed in draw() via RenderContext::measure_text().
    cached_title_width: Option<u32>,
}
impl GroupBox {
    /// Creates a group box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GroupBox, geometry, "GroupBox"),
            title: String::new(),
            alignment: Alignment::Left,
            checkable: false,
            checked: true,
            toggled: Signal1::new(),
            cached_title_width: None,
        }
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }
    /// Returns whether group box is checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    /// Sets checkable state.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }
    /// Returns whether group box is checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Sets checked state.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.toggled.emit(checked);
    }
    /// Toggles checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }
    /// Returns title rectangle.
    fn title_rect(&self) -> Rect {
        let rect = self.geometry();
        let text_width = self.cached_title_width.unwrap_or_else(|| {
            // Fallback approximate measurement if draw() hasn't run yet.
            self.title.len() as u32 * 8
        });
        let text_height = 16i32;
        let x = match self.alignment {
            Alignment::Left => rect.x + 10,
            Alignment::Center => rect.x + ((rect.width - text_width) / 2) as i32,
            Alignment::Right => rect.x + rect.width as i32 - text_width as i32 - 10,
            Alignment::Top | Alignment::Bottom => rect.x + 10,
        };
        Rect::new(x, rect.y - text_height / 2, text_width, text_height as u32)
    }
    /// Returns checkbox rectangle if checkable.
    fn checkbox_rect(&self) -> Option<Rect> {
        if !self.checkable {
            return None;
        }
        let title_rect = self.title_rect();
        let checkbox_size: i32 = 12;
        Some(Rect::new(
            title_rect.x - checkbox_size - 5,
            title_rect.y + (title_rect.height as i32 - checkbox_size) / 2,
            checkbox_size as u32,
            checkbox_size as u32,
        ))
    }
}
// Implement Widget trait
impl Widget for GroupBox {
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
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
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
        self.base.is_enabled()
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
impl EventHandler for GroupBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.checkable {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(checkbox_rect) = self.checkbox_rect() {
                        if checkbox_rect.contains(*pos) {
                            self.toggle();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
impl Draw for GroupBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Cache actual title width from render context.
        if !self.title.is_empty() {
            let metrics = context.measure_text(&self.title, &Font::default());
            self.cached_title_width = Some(metrics.width);
        }
        // Draw base widget
        let rect = self.geometry();
        let title_rect = self.title_rect();
        // Draw border
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        // Draw title background to hide border
        let title_bg_width = title_rect.width + 20;
        let title_bg_x = title_rect.x - 10;
        context.fill_rect(
            Rect::new(title_bg_x, rect.y, title_bg_width as u32, 2),
            Color::from_rgb(255, 255, 255),
        );
        // Draw checkbox if checkable
        if self.checkable {
            if let Some(checkbox_rect) = self.checkbox_rect() {
                // Draw checkbox border
                context.draw_rect(checkbox_rect, Color::from_rgb(100, 100, 100));
                // Draw checkmark if checked
                if self.checked {
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + 2.0,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 * 0.5,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        Color::from_rgb(0, 0, 0),
                    );
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 - 2.0,
                            checkbox_rect.y as f32 + 2.0,
                        ),
                        Color::from_rgb(0, 0, 0),
                    );
                }
            }
        }
        // Draw title text
        if !self.title.is_empty() {
            let text_color = if self.base.is_enabled() {
                Color::from_rgb(0, 0, 0)
            } else {
                Color::from_rgb(150, 150, 150)
            };
            context.draw_text(
                Point::from_f32(title_rect.x as f32, title_rect.y as f32),
                &self.title,
                &Font::default(),
                text_color,
            );
        }
    }
}
