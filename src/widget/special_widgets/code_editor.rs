//! CodeEditor widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Diagnostic marker severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerSeverity {
    Info,
    Warning,
    Error,
}

/// One diagnostic marker bound to a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticMarker {
    pub line: usize,
    pub message: String,
    pub severity: MarkerSeverity,
}

/// Lightweight code editor with line numbers and diagnostics.
pub struct CodeEditor {
    base: BaseWidget,
    text: String,
    cursor_line: usize,
    cursor_column: usize,
    markers: Vec<DiagnosticMarker>,
    /// Emitted when text changes.
    pub text_changed: Signal1<String>,
    /// Emitted when cursor position changes `(line,column)`.
    pub cursor_moved: Signal1<(usize, usize)>,
}

impl CodeEditor {
    /// Creates empty code editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "CodeEditor"),
            text: String::new(),
            cursor_line: 0,
            cursor_column: 0,
            markers: Vec::new(),
            text_changed: Signal1::new(),
            cursor_moved: Signal1::new(),
        }
    }

    /// Returns text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let next = text.into();
        if self.text == next {
            return;
        }
        self.text = next.clone();
        let max_line = self.line_count().saturating_sub(1);
        self.cursor_line = self.cursor_line.min(max_line);
        self.cursor_column = self.current_line_len().min(self.cursor_column);
        self.text_changed.emit(next);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one line.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(line.as_ref());
        self.cursor_line = self.line_count().saturating_sub(1);
        self.cursor_column = self.current_line_len();
        self.text_changed.emit(self.text.clone());
        self.cursor_moved.emit((self.cursor_line, self.cursor_column));
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Sets diagnostic markers.
    pub fn set_markers(&mut self, markers: Vec<DiagnosticMarker>) {
        self.markers = markers;
        self.base.request_redraw();
    }

    /// Returns markers.
    pub fn markers(&self) -> &[DiagnosticMarker] {
        &self.markers
    }

    /// Returns line count.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            0
        } else {
            self.text.lines().count()
        }
    }

    /// Returns cursor location.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_column)
    }

    fn current_line_len(&self) -> usize {
        self.text.lines().nth(self.cursor_line).map(|line| line.chars().count()).unwrap_or(0)
    }

    fn move_cursor_line(&mut self, delta: isize) {
        let lines = self.line_count();
        if lines == 0 {
            self.cursor_line = 0;
            self.cursor_column = 0;
            return;
        }
        let next =
            (self.cursor_line as isize + delta).clamp(0, lines.saturating_sub(1) as isize) as usize;
        if next != self.cursor_line {
            self.cursor_line = next;
            self.cursor_column = self.cursor_column.min(self.current_line_len());
            self.cursor_moved.emit((self.cursor_line, self.cursor_column));
            self.base.request_redraw();
        }
    }

    fn move_cursor_column(&mut self, delta: isize) {
        let len = self.current_line_len();
        let next = (self.cursor_column as isize + delta).clamp(0, len as isize) as usize;
        if next != self.cursor_column {
            self.cursor_column = next;
            self.cursor_moved.emit((self.cursor_line, self.cursor_column));
            self.base.request_redraw();
        }
    }
}

impl Widget for CodeEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for CodeEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers: _ } = event {
            match *key {
                37 => self.move_cursor_column(-1),
                39 => self.move_cursor_column(1),
                38 => self.move_cursor_line(-1),
                40 => self.move_cursor_line(1),
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}

impl Draw for CodeEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(249, 251, 254));
        context.draw_rect(rect, Color::from_rgb(188, 197, 211));

        let gutter_w = 40;
        context.fill_rect(
            Rect::new(rect.x, rect.y, gutter_w as u32, rect.height),
            Color::from_rgb(238, 243, 250),
        );
        context.draw_line(
            Point::new(rect.x + gutter_w, rect.y),
            Point::new(rect.x + gutter_w, rect.y + rect.height as i32),
            Color::from_rgb(208, 217, 230),
        );

        for (idx, line) in self.text.lines().take(14).enumerate() {
            let y = rect.y + 16 + idx as i32 * 15;
            context.draw_text(
                Point::new(rect.x + 6, y),
                &format!("{}", idx + 1),
                &Font::default(),
                Color::from_rgb(108, 120, 138),
                HorizontalAlignment::Left,
            );

            if let Some(marker) = self.markers.iter().find(|m| m.line == idx) {
                let color = match marker.severity {
                    MarkerSeverity::Info => Color::from_rgb(74, 125, 201),
                    MarkerSeverity::Warning => Color::from_rgb(216, 155, 52),
                    MarkerSeverity::Error => Color::from_rgb(206, 82, 73),
                };
                context.fill_rect(Rect::new(rect.x + gutter_w - 8, y - 6, 4, 4), color);
            }

            let text_color = if idx == self.cursor_line {
                Color::from_rgb(24, 99, 190)
            } else {
                Color::from_rgb(43, 55, 74)
            };
            context.draw_text(
                Point::new(rect.x + gutter_w + 8, y),
                line,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }

        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 130, rect.y + rect.height as i32 - 10),
            &format!("Ln {}, Col {}", self.cursor_line + 1, self.cursor_column + 1),
            &Font::default(),
            Color::from_rgb(88, 102, 124),
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn cursor_navigation_moves_position() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 500, 260));
        editor.set_text("fn main() {\n  println!(\"hi\");\n}");

        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor().0, 1);

        editor.handle_event(&Event::key_press(39, 0));
        assert_eq!(editor.cursor().1, 1);
    }

    #[test]
    fn markers_can_be_set_and_read() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 500, 260));
        editor.set_markers(vec![DiagnosticMarker {
            line: 0,
            message: "hint".to_string(),
            severity: MarkerSeverity::Info,
        }]);
        assert_eq!(editor.markers().len(), 1);
        assert_eq!(editor.markers()[0].line, 0);
    }

    #[test]
    fn text_changed_emits_new_text() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 500, 260));
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        editor.text_changed.connect(move |text| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(text.as_ref().clone());
            }
        });

        editor.set_text("abc");

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["abc".to_string()]);
    }

    #[test]
    fn default_state() {
        let editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        assert_eq!(editor.text(), "");
        assert_eq!(editor.cursor(), (0, 0));
        assert_eq!(editor.line_count(), 0);
        assert!(editor.markers().is_empty());
    }

    #[test]
    fn set_text_get_text_roundtrip() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        editor.set_text("hello world");
        assert_eq!(editor.text(), "hello world");
    }

    #[test]
    fn append_line_updates_line_count() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        editor.append_line("first");
        assert_eq!(editor.line_count(), 1);
        assert_eq!(editor.text(), "first");

        editor.append_line("second");
        assert_eq!(editor.line_count(), 2);
        assert_eq!(editor.text(), "first\nsecond");

        editor.append_line("third");
        assert_eq!(editor.line_count(), 3);
    }

    #[test]
    fn empty_text_line_count() {
        let editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        assert_eq!(editor.line_count(), 0);
    }

    #[test]
    fn duplicate_set_text_is_noop() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        editor.text_changed.connect(move |text| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(text.as_ref().clone());
            }
        });

        editor.set_text("hello");
        // Second set with same value should be no-op
        editor.set_text("hello");

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1, "duplicate set_text must not emit");
    }

    #[test]
    fn marker_severity_edge_cases() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        editor.set_text("line1\nline2\nline3");

        editor.set_markers(vec![
            DiagnosticMarker {
                line: 0,
                message: "info msg".to_string(),
                severity: MarkerSeverity::Info,
            },
            DiagnosticMarker {
                line: 1,
                message: "warning msg".to_string(),
                severity: MarkerSeverity::Warning,
            },
            DiagnosticMarker {
                line: 2,
                message: "error msg".to_string(),
                severity: MarkerSeverity::Error,
            },
        ]);

        assert_eq!(editor.markers().len(), 3);
        assert_eq!(editor.markers()[0].severity, MarkerSeverity::Info);
        assert_eq!(editor.markers()[1].severity, MarkerSeverity::Warning);
        assert_eq!(editor.markers()[2].severity, MarkerSeverity::Error);

        // Clear markers
        editor.set_markers(Vec::new());
        assert!(editor.markers().is_empty());
    }

    #[test]
    fn cursor_movement_tracking() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        let emitted = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = emitted.clone();
        editor.cursor_moved.connect(move |pos| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*pos.as_ref());
            }
        });

        editor.set_text("line one\nline two\nline three");

        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor().0, 1);

        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor().0, 2);

        editor.handle_event(&Event::key_press(38, 0));
        assert_eq!(editor.cursor().0, 1);

        let positions = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!positions.is_empty(), "cursor_moved must have been emitted");
    }

    #[test]
    fn text_change_signal_emission() {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 800, 600));
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        editor.text_changed.connect(move |text| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(text.as_ref().clone());
            }
        });

        editor.set_text("initial");
        editor.append_line("appended");
        editor.set_text("replaced");

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], "initial");
        assert_eq!(got[2], "replaced");
    }
}
