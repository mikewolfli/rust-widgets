//! MarkdownEditor widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Lightweight markdown editor with preview toggle and metrics.
pub struct MarkdownEditor {
    base: BaseWidget,
    text: String,
    preview_mode: bool,
    cursor_line: usize,
    /// Emitted when text changes.
    pub text_changed: Signal1<String>,
    /// Emitted when preview mode changes.
    pub preview_mode_changed: Signal1<bool>,
}

impl MarkdownEditor {
    /// Creates editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "MarkdownEditor"),
            text: String::new(),
            preview_mode: false,
            cursor_line: 0,
            text_changed: Signal1::new(),
            preview_mode_changed: Signal1::new(),
        }
    }

    /// Returns markdown text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets markdown text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let next = text.into();
        if self.text == next {
            return;
        }
        self.text = next.clone();
        self.cursor_line = self.cursor_line.min(self.line_count().saturating_sub(1));
        self.text_changed.emit(next);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one line to markdown.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(line.as_ref());
        self.cursor_line = self.line_count().saturating_sub(1);
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Toggles preview mode.
    pub fn toggle_preview_mode(&mut self) {
        self.preview_mode = !self.preview_mode;
        self.preview_mode_changed.emit(self.preview_mode);
        self.base.request_redraw();
    }

    /// Sets preview mode.
    pub fn set_preview_mode(&mut self, preview_mode: bool) {
        if self.preview_mode == preview_mode {
            return;
        }
        self.preview_mode = preview_mode;
        self.preview_mode_changed.emit(preview_mode);
        self.base.request_redraw();
    }

    /// Returns whether preview mode is enabled.
    pub fn preview_mode(&self) -> bool {
        self.preview_mode
    }

    /// Returns line count.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            0
        } else {
            self.text.lines().count()
        }
    }

    /// Returns word count.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// Returns heading count.
    pub fn heading_count(&self) -> usize {
        self.text.lines().filter(|line| line.trim_start().starts_with('#')).count()
    }

    /// Returns current line index.
    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    fn move_cursor(&mut self, delta: isize) {
        let lines = self.line_count();
        if lines == 0 {
            self.cursor_line = 0;
            return;
        }
        let current = self.cursor_line as isize;
        let next = (current + delta).clamp(0, lines.saturating_sub(1) as isize) as usize;
        if next != self.cursor_line {
            self.cursor_line = next;
            self.base.request_redraw();
        }
    }
}

impl Widget for MarkdownEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for MarkdownEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers } = event {
            match *key {
                38 => self.move_cursor(-1),
                40 => self.move_cursor(1),
                80 | 112 if *modifiers != 0 => self.toggle_preview_mode(),
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}

impl Draw for MarkdownEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(252, 252, 253));
        context.draw_rect(rect, Color::from_rgb(194, 201, 213));

        let header = if self.preview_mode {
            format!(
                "Markdown Preview  lines:{} words:{} headings:{}",
                self.line_count(),
                self.word_count(),
                self.heading_count()
            )
        } else {
            format!(
                "Markdown Edit  lines:{} words:{} headings:{}",
                self.line_count(),
                self.word_count(),
                self.heading_count()
            )
        };
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 16),
            &header,
            &Font::default(),
            Color::from_rgb(41, 54, 73),
        );

        for (idx, line) in self.text.lines().take(10).enumerate() {
            let y = rect.y + 36 + (idx as i32) * 16;
            if y > rect.y + rect.height as i32 - 8 {
                break;
            }
            let color = if idx == self.cursor_line {
                Color::from_rgb(23, 110, 203)
            } else {
                Color::from_rgb(59, 72, 92)
            };
            let rendered =
                if self.preview_mode { line.trim_start_matches('#').trim_start() } else { line };
            context.draw_text(Point::new(rect.x + 12, y), rendered, &Font::default(), color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn metrics_follow_text_changes() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# Title\nhello world\n## Details");

        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.word_count(), 6);
        assert_eq!(editor.heading_count(), 2);
    }

    #[test]
    fn preview_toggle_emits_signal() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        let states = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = states.clone();
        editor.preview_mode_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });

        editor.toggle_preview_mode();
        editor.toggle_preview_mode();

        let got = states.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    #[test]
    fn arrow_keys_move_cursor() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("a\nb\nc");

        editor.handle_event(&Event::key_press(40, 0));
        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor_line(), 2);

        editor.handle_event(&Event::key_press(38, 0));
        assert_eq!(editor.cursor_line(), 1);
    }
}
