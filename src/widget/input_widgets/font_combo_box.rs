use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Font combo box widget for font selection.
pub struct FontComboBox {
    base: BaseWidget,
    current_font: Font,
    fonts: Vec<String>,
    current_index: i32,
    editable: bool,
    max_visible_items: i32,
    /// Emitted when the current font changes.
    pub current_font_changed: Signal1<Font>,
    /// Emitted when the current index changes.
    pub current_index_changed: Signal1<i32>,
    /// Emitted when the combo box is activated.
    pub activated: Signal1<i32>,
    /// Emitted when the text is edited (if editable).
    pub text_edited: Signal1<String>,
    /// Emitted when the popup is shown.
    pub popup_shown: GenericSignal,
    /// Emitted when the popup is hidden.
    pub popup_hidden: GenericSignal,
}
impl FontComboBox {
    pub fn new(geometry: Rect) -> Self {
        let default_font = Font::default();
        Self {
            base: BaseWidget::new(WidgetKind::FontComboBox, geometry, "FontComboBox"),
            current_font: default_font.clone(),
            fonts: Vec::new(),
            current_index: -1,
            editable: false,
            max_visible_items: 10,
            current_font_changed: Signal1::new(),
            current_index_changed: Signal1::new(),
            activated: Signal1::new(),
            text_edited: Signal1::new(),
            popup_shown: GenericSignal::new(),
            popup_hidden: GenericSignal::new(),
        }
    }
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }
    pub fn count(&self) -> i32 {
        self.fonts.len() as i32
    }
    pub fn set_current_font(&mut self, font: Font) {
        if self.current_font != font {
            self.current_font = font.clone();
            self.current_font_changed.emit(font);
            self.base.request_redraw();
        }
    }
    pub fn set_current_index(&mut self, index: i32) {
        let clamped = index.clamp(-1, self.fonts.len() as i32 - 1);
        if self.current_index != clamped {
            self.current_index = clamped;
            self.current_index_changed.emit(clamped);
            if clamped >= 0 && clamped < self.fonts.len() as i32 {
                // Update current font based on selection
                if let Some(font_name) = self.fonts.get(clamped as usize) {
                    let new_font = Font::new(font_name, self.current_font.size, false, false);
                    self.set_current_font(new_font);
                }
            }
            self.base.request_redraw();
        }
    }
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        self.base.request_redraw();
    }
    pub fn set_max_visible_items(&mut self, max_items: i32) {
        self.max_visible_items = max_items.max(1);
    }
    pub fn add_font(&mut self, font_name: String) {
        self.fonts.push(font_name);
        self.base.request_redraw();
    }
    pub fn remove_font(&mut self, index: i32) {
        if index >= 0 && index < self.fonts.len() as i32 {
            self.fonts.remove(index as usize);
            if self.current_index == index {
                self.set_current_index(-1);
            } else if self.current_index > index {
                self.current_index -= 1;
            }
            self.base.request_redraw();
        }
    }
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.set_current_index(-1);
        self.base.request_redraw();
    }
    pub fn show_popup(&mut self) {
        self.popup_shown.emit();
    }
    pub fn hide_popup(&mut self) {
        self.popup_hidden.emit();
    }
    pub fn current_text(&self) -> String {
        if self.current_index >= 0 && self.current_index < self.fonts.len() as i32 {
            self.fonts[self.current_index as usize].clone()
        } else {
            String::new()
        }
    }
}
impl Widget for FontComboBox {
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
impl EventHandler for FontComboBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } => {
                if button == &1 {
                    // Show the dropdown list
                    self.show_popup();
                    self.base.clicked.emit();
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if button == &1 {
                    // Simulate selection: cycle to next font
                    if self.fonts.len() > 0 {
                        let new_index = (self.current_index + 1) % self.fonts.len() as i32;
                        self.set_current_index(new_index);
                        self.activated.emit(new_index);
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    13 | 32 => {
                        // Enter or Space
                        if self.fonts.len() > 0 && self.current_index >= 0 {
                            self.activated.emit(self.current_index);
                        }
                        self.base.clicked.emit();
                    }
                    27 => {
                        // Escape
                        self.hide_popup();
                    }
                    38 => {
                        // Up arrow
                        if self.fonts.len() > 0 {
                            let new_index = if self.current_index <= 0 {
                                self.fonts.len() as i32 - 1
                            } else {
                                self.current_index - 1
                            };
                            self.set_current_index(new_index);
                        }
                    }
                    40 => {
                        // Down arrow
                        if self.fonts.len() > 0 {
                            let new_index = (self.current_index + 1) % self.fonts.len() as i32;
                            self.set_current_index(new_index);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for FontComboBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::WHITE);
        let border_color = style.border_color.unwrap_or(Color::GRAY);
        let text_color = style.text_color.unwrap_or(Color::BLACK);
        let border_width = style.border_width;
        // Draw background
        context.fill_rect(rect, bg_color);
        // Draw border
        if border_width > 0 {
            context.draw_rect_stroke(rect, border_color, border_width);
        }
        // Draw current text
        let padding = &style.padding;
        let text_rect = Rect::new(
            rect.x + padding.left as i32,
            rect.y + padding.top as i32,
            rect.width - padding.left - padding.right - 24,
            rect.height - padding.top - padding.bottom,
        );
        let current_text = self.current_text();
        if !current_text.is_empty() {
            let font = &self.current_font;
            context.draw_text(
                Point::new(text_rect.x, text_rect.y + text_rect.height as i32 / 2),
                &current_text,
                font,
                text_color,
            );
        }
        // Draw dropdown arrow button
        let arrow_rect = Rect::new(
            rect.x + rect.width as f32 as i32 - 24,
            rect.y,
            24,
            rect.height,
        );
        let arrow_color = if self.base.is_enabled() {
            text_color
        } else {
            Color::GRAY
        };
        // Draw arrow background
        context.fill_rect(arrow_rect, Color::rgba(240, 240, 240, 255));
        // Draw arrow using lines
        let arrow_x = arrow_rect.x + arrow_rect.width as i32 / 2;
        let arrow_y = arrow_rect.y + arrow_rect.height as i32 / 2;
        let arrow_size = 6;
        context.draw_line(
            Point::new(arrow_x - arrow_size / 2, arrow_y - arrow_size / 2),
            Point::new(arrow_x + arrow_size / 2, arrow_y - arrow_size / 2),
            arrow_color,
        );
        context.draw_line(
            Point::new(arrow_x + arrow_size / 2, arrow_y - arrow_size / 2),
            Point::new(arrow_x, arrow_y + arrow_size / 2),
            arrow_color,
        );
        context.draw_line(
            Point::new(arrow_x, arrow_y + arrow_size / 2),
            Point::new(arrow_x - arrow_size / 2, arrow_y - arrow_size / 2),
            arrow_color,
        );
    }
}
