//! Single-line text edit widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// Single-line text edit widget.
pub struct LineEdit {
    base: BaseWidget,
    text: String,
    placeholder_text: String,
    max_length: Option<usize>,
    echo_mode: EchoMode,
    cursor_position: usize,
    selection_start: Option<usize>,
    pub text_changed: Signal1<String>,
    pub editing_finished: GenericSignal,
    pub return_pressed: GenericSignal,
}
/// Text echo mode for password fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoMode {
    /// Display characters as entered (default)
    Normal,
    /// Display asterisks for password fields
    Password,
    /// Display nothing (for sensitive data)
    NoEcho,
    /// Display asterisks only when editing
    PasswordEchoOnEdit,
}
impl Default for EchoMode {
    fn default() -> Self {
        Self::Normal
    }
}
impl LineEdit {
    /// Creates an empty line edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            echo_mode: EchoMode::Normal,
            cursor_position: 0,
            selection_start: None,
            text_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
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
        self.cursor_position = self.text.len();
        self.selection_start = None;
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
                self.cursor_position = self.cursor_position.min(max);
                if let Some(start) = &mut self.selection_start {
                    *start = (*start).min(max);
                }
                self.text_changed.emit(self.text.clone());
            }
        }
    }
    /// Returns echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }
    /// Sets echo mode.
    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        self.echo_mode = mode;
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        self.cursor_position = position.min(self.text.len());
        self.selection_start = None;
    }
    /// Returns selection start position.
    pub fn selection_start(&self) -> Option<usize> {
        self.selection_start
    }
    /// Returns selected text.
    pub fn selected_text(&self) -> String {
        if let Some(start) = self.selection_start {
            let start = start.min(self.text.len());
            let end = self.cursor_position.min(self.text.len());
            let (start, end) = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            self.text[start..end].to_string()
        } else {
            String::new()
        }
    }
    /// Selects all text.
    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor_position = self.text.len();
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
    }
    /// Inserts text at cursor position.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Check max length
        if let Some(max) = self.max_length {
            let available = max.saturating_sub(self.text.len());
            if available == 0 {
                return;
            }
            let text = if text.len() > available {
                &text[..available]
            } else {
                text
            };
        }
        // Handle selection
        let mut new_text = self.text.clone();
        if let Some(start) = self.selection_start {
            let start = start.min(new_text.len());
            let end = self.cursor_position.min(new_text.len());
            let (start, end) = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            new_text.replace_range(start..end, text);
            self.cursor_position = start + text.len();
        } else {
            new_text.insert_str(self.cursor_position, text);
            self.cursor_position += text.len();
        }
        self.selection_start = None;
        self.set_text(new_text);
    }
    /// Deletes selected text or character before cursor.
    pub fn backspace(&mut self) {
        if let Some(start) = self.selection_start {
            // Delete selection
            let start = start.min(self.text.len());
            let end = self.cursor_position.min(self.text.len());
            let (start, end) = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            let mut new_text = self.text.clone();
            new_text.replace_range(start..end, "");
            self.cursor_position = start;
            self.selection_start = None;
            self.set_text(new_text);
        } else if self.cursor_position > 0 {
            // Delete character before cursor
            let mut new_text = self.text.clone();
            new_text.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            self.set_text(new_text);
        }
    }
    /// Deletes selected text or character after cursor.
    pub fn delete(&mut self) {
        if let Some(start) = self.selection_start {
            // Delete selection
            self.backspace(); // Same logic
        } else if self.cursor_position < self.text.len() {
            // Delete character after cursor
            let mut new_text = self.text.clone();
            new_text.remove(self.cursor_position);
            self.set_text(new_text);
        }
    }
    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }
    /// Returns display text based on echo mode.
    fn display_text(&self) -> String {
        match self.echo_mode {
            EchoMode::Normal => self.text.clone(),
            EchoMode::Password => "*".repeat(self.text.len()),
            EchoMode::NoEcho => String::new(),
            EchoMode::PasswordEchoOnEdit => {
                // In real implementation, would track edit state
                "*".repeat(self.text.len())
            }
        }
    }
}
// Implement Widget trait
impl Widget for LineEdit {
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
impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, modifiers } => {
                match *key {
                    8 => {
                        // Backspace
                        self.backspace();
                    }
                    46 => {
                        // Delete
                        self.delete();
                    }
                    13 => {
                        // Enter/Return
                        self.return_pressed.emit();
                        self.editing_finished.emit();
                    }
                    27 => {
                        // Escape
                        self.editing_finished.emit();
                    }
                    37 => {
                        // Left arrow
                        if self.cursor_position > 0 {
                            if modifiers.shift {
                                if self.selection_start.is_none() {
                                    self.selection_start = Some(self.cursor_position);
                                }
                            } else {
                                self.selection_start = None;
                            }
                            self.cursor_position -= 1;
                        }
                    }
                    39 => {
                        // Right arrow
                        if self.cursor_position < self.text.len() {
                            if modifiers.shift {
                                if self.selection_start.is_none() {
                                    self.selection_start = Some(self.cursor_position);
                                }
                            } else {
                                self.selection_start = None;
                            }
                            self.cursor_position += 1;
                        }
                    }
                    36 => {
                        // Home
                        if modifiers.shift {
                            if self.selection_start.is_none() {
                                self.selection_start = Some(self.cursor_position);
                            }
                        } else {
                            self.selection_start = None;
                        }
                        self.cursor_position = 0;
                    }
                    35 => {
                        // End
                        if modifiers.shift {
                            if self.selection_start.is_none() {
                                self.selection_start = Some(self.cursor_position);
                            }
                        } else {
                            self.selection_start = None;
                        }
                        self.cursor_position = self.text.len();
                    }
                    65 if modifiers.ctrl => {
                        // Ctrl+A: Select all
                        self.select_all();
                    }
                    86 if modifiers.ctrl => {
                        // Ctrl+V: Paste (would need clipboard integration)
                        // For now, just emit signal
                        self.base.redraw_requested.emit();
                    }
                    67 if modifiers.ctrl => {
                        // Ctrl+C: Copy (would need clipboard integration)
                        // For now, just emit signal
                    }
                    88 if modifiers.ctrl => {
                        // Ctrl+X: Cut (would need clipboard integration)
                        // For now, just emit signal
                        self.base.redraw_requested.emit();
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.insert_text(&ch.to_string());
                            }
                        }
                    }
                }
            }
            Event::FocusLost => {
                self.editing_finished.emit();
            }
            _ => {}
        }
    }
}
impl Draw for LineEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let padding = 4;
        let text_x = rect.x + padding;
        let text_y = rect.y + rect.height as f32 / 2;
        // Draw background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(255, 255, 255),
        );
        // Draw border
        context.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(200, 200, 200),
        );
        // Draw text or placeholder
        let display_text = if self.text.is_empty() && !self.placeholder_text.is_empty() {
            &self.placeholder_text
        } else {
            &self.display_text()
        };
        if !display_text.is_empty() {
            context.draw_text(
                text_x,
                text_y,
                display_text,
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                Alignment::Left,
            );
        }
        // Draw cursor if focused
        // Note: Would need focus state tracking
    }
}
