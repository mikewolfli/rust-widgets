//! Multi-line text edit widget.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Multi-line text edit widget.
pub struct TextEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    read_only: bool,
    line_wrap: bool,
    pub text_changed: Signal1<String>,
    pub cursor_position_changed: Signal1<usize>,
}
impl TextEdit {
    /// Creates an empty text edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            read_only: false,
            line_wrap: true,
            text_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
        }
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text and emits text_changed signal if different.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.text_changed.emit(self.text.clone());
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        self.placeholder_text = text;
    }
    /// Returns maximum text length.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        self.max_length = max_length;
        // Truncate if needed
        if let Some(max) = max_length {
            if self.text.len() > max {
                self.text.truncate(max);
                self.text_changed.emit(self.text.clone());
            }
        }
    }
    /// Returns whether the widget is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }
    /// Returns whether line wrap is enabled.
    pub fn line_wrap(&self) -> bool {
        self.line_wrap
    }
    /// Sets line wrap state.
    pub fn set_line_wrap(&mut self, line_wrap: bool) {
        self.line_wrap = line_wrap;
    }
    /// Returns number of lines in the text.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        }
    }
    /// Returns text at specified line (0-indexed).
    pub fn line_text(&self, line: usize) -> Option<&str> {
        let mut start = 0;
        let mut current_line = 0;
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                if current_line == line {
                    return Some(&self.text[start..i]);
                }
                start = i + 1;
                current_line += 1;
            }
        }
        if current_line == line {
            Some(&self.text[start..])
        } else {
            None
        }
    }
    /// Appends text to the end.
    pub fn append(&mut self, text: &str) {
        self.text.push_str(text);
        self.text_changed.emit(self.text.clone());
    }
    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Returns whether the text edit is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}
// Implement Widget trait
impl Widget for TextEdit {
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
impl EventHandler for TextEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.read_only {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => {
                match *key {
                    8 => {
                        // Backspace
                        if !self.text.is_empty() {
                            self.text.pop();
                            self.text_changed.emit(self.text.clone());
                        }
                    }
                    13 => {
                        // Enter
                        self.text.push('\n');
                        self.text_changed.emit(self.text.clone());
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' || ch == '\t' {
                                self.text.push(ch);
                                self.text_changed.emit(self.text.clone());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
impl Draw for TextEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + padding;
        // Draw background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.text
        };
        if !display_text.is_empty() {
            // Simple text drawing - in real implementation would handle line wrapping
            context.draw_text(
                Point::new(text_x, text_y),
                display_text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
            );
        }
    }
}
