//! Rich text editor widget.
use crate::core::Rect;
use crate::signal::Signal1;
use crate::render::RenderContext;
use crate::widget::base::{BaseWidget, Widget, WidgetKind};
/// Rich text/code editor baseline widget contract.
pub struct RichEdit {
    base: BaseWidget,
    text: String,
    selection: Option<(usize, usize)>,
    read_only: bool,
    pub text_changed: Signal1<String>,
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    pub read_only_changed: Signal1<bool>,
    pub cursor_position_changed: Signal1<usize>,
}
impl RichEdit {
    /// Creates an empty rich editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "RichEdit"),
            text: String::new(),
            selection: None,
            read_only: false,
            text_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            read_only_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
        }
    }
    /// Returns current editor text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Replaces editor text and resets selection/cursor to end.
    pub fn set_text(&mut self, text: String) {
        if self.read_only || self.text == text {
            return;
        }
        self.text = text;
        self.selection = None;
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(self.text.len());
    }
    /// Returns current selection range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }
    /// Sets selection range.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        if self.read_only {
            return;
        }
        let start = start.min(self.text.len());
        let end = end.min(self.text.len());
        if self.selection == Some((start, end)) {
            return;
        }
        self.selection = Some((start, end));
        self.selection_changed.emit(self.selection);
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        if self.selection.is_none() {
            return;
        }
        self.selection = None;
        self.selection_changed.emit(None);
    }
    /// Returns read-only state.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Sets read-only state.
    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        self.read_only_changed.emit(read_only);
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.selection.map_or(0, |(start, _)| start)
    }
    /// Sets cursor position.
    pub fn set_cursor_position(&mut self, position: usize) {
        if self.read_only {
            return;
        }
        let position = position.min(self.text.len());
        self.selection = Some((position, position));
        self.cursor_position_changed.emit(position);
    }
}
impl Widget for RichEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl crate::widget::base::Draw for RichEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        // Default drawing implementation
        // Rich edit is drawn by the renderer
    }
}
impl crate::event::EventHandler for RichEdit {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // Default event handling
        let _ = event;
    }
}
