//! TerminalView widget.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Terminal-like text view with input line and command history.
pub struct TerminalView {
    base: BaseWidget,
    lines: Vec<String>,
    input_line: String,
    history: Vec<String>,
    history_index: Option<usize>,
    /// Emitted when command is submitted.
    pub command_submitted: Signal1<String>,
}

impl TerminalView {
    /// Creates terminal view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TerminalView"),
            lines: Vec::new(),
            input_line: String::new(),
            history: Vec::new(),
            history_index: None,
            command_submitted: Signal1::new(),
        }
    }

    /// Returns output lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns current input line.
    pub fn input_line(&self) -> &str {
        &self.input_line
    }

    /// Sets current input line.
    pub fn set_input_line(&mut self, line: impl Into<String>) {
        self.input_line = line.into();
        self.base.request_redraw();
    }

    /// Appends output line.
    pub fn append_output(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
        if self.lines.len() > 200 {
            let drop_count = self.lines.len() - 200;
            self.lines.drain(0..drop_count);
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Submits current command line.
    pub fn submit(&mut self) -> bool {
        let cmd = self.input_line.trim().to_string();
        if cmd.is_empty() {
            return false;
        }
        self.history.push(cmd.clone());
        self.history_index = None;
        self.lines.push(format!("> {}", cmd));
        self.command_submitted.emit(cmd);
        self.input_line.clear();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    fn recall_history(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }

        let next_index = match (self.history_index, up) {
            (None, true) => Some(self.history.len() - 1),
            (None, false) => None,
            (Some(index), true) => Some(index.saturating_sub(1)),
            (Some(index), false) if index + 1 < self.history.len() => Some(index + 1),
            (Some(_), false) => None,
        };

        self.history_index = next_index;
        self.input_line =
            next_index.and_then(|index| self.history.get(index).cloned()).unwrap_or_default();
        self.base.request_redraw();
    }
}

impl Widget for TerminalView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for TerminalView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::KeyPress { key, modifiers: _ } = event {
            match *key {
                13 => {
                    let _ = self.submit();
                }
                38 => self.recall_history(true),
                40 => self.recall_history(false),
                _ => { /* Other keys are not relevant */ }
            }
        }
    }
}

impl Draw for TerminalView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(22, 27, 34));
        context.draw_rect(rect, Color::from_rgb(68, 78, 92));

        let max_lines = ((rect.height.saturating_sub(28)) / 14) as usize;
        let start = self.lines.len().saturating_sub(max_lines);
        for (idx, line) in self.lines.iter().skip(start).enumerate() {
            let y = rect.y + 16 + idx as i32 * 14;
            context.draw_text(
                Point::new(rect.x + 8, y),
                line,
                &Font::default(),
                Color::from_rgb(217, 224, 236),
            );
        }

        let prompt_y = rect.y + rect.height as i32 - 10;
        context.draw_text(
            Point::new(rect.x + 8, prompt_y),
            &format!("> {}", self.input_line),
            &Font::default(),
            Color::from_rgb(140, 218, 160),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn submit_emits_and_appends_command() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("ls -la");

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        terminal.command_submitted.connect(move |cmd| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(cmd.as_ref().clone());
            }
        });

        assert!(terminal.submit());
        assert_eq!(terminal.lines().last().map(|s| s.as_str()), Some("> ls -la"));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["ls -la".to_string()]);
    }

    #[test]
    fn history_recall_works_with_arrow_keys() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        terminal.set_input_line("first");
        assert!(terminal.submit());
        terminal.set_input_line("second");
        assert!(terminal.submit());

        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "second");

        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "first");

        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "second");
    }

    #[test]
    fn append_output_keeps_recent_window() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        for i in 0..220 {
            terminal.append_output(format!("line {}", i));
        }

        assert_eq!(terminal.lines().len(), 200);
        assert_eq!(terminal.lines().first().map(|s| s.as_str()), Some("line 20"));
    }

    #[test]
    fn default_state() {
        let terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        assert!(terminal.lines().is_empty());
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn append_output_adds_lines() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.append_output("first line");
        terminal.append_output("second line");

        assert_eq!(terminal.lines().len(), 2);
        assert_eq!(terminal.lines()[0], "first line");
        assert_eq!(terminal.lines()[1], "second line");
    }

    #[test]
    fn empty_submit_returns_false() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        // Empty input line
        assert!(!terminal.submit());

        // Whitespace-only input
        terminal.set_input_line("   ");
        assert!(!terminal.submit());

        // Still no lines appended
        assert!(terminal.lines().is_empty());
    }

    #[test]
    fn clear_input_after_submit() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.set_input_line("command");
        assert!(terminal.submit());
        assert_eq!(terminal.input_line(), "", "input must be cleared after submit");
    }

    #[test]
    fn input_line_set_get() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        terminal.set_input_line("custom input");
        assert_eq!(terminal.input_line(), "custom input");

        terminal.set_input_line("");
        assert_eq!(terminal.input_line(), "");
    }

    #[test]
    fn history_recall_bounds() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 800, 600));
        // No history yet
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "");

        terminal.set_input_line("cmd1");
        assert!(terminal.submit());
        terminal.set_input_line("cmd2");
        assert!(terminal.submit());

        // Up twice: cmd2 -> cmd1
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd2");
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd1");
        // Up again stays at oldest
        terminal.handle_event(&Event::key_press(38, 0));
        assert_eq!(terminal.input_line(), "cmd1");

        // Down goes back
        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "cmd2");
        // Down again clears
        terminal.handle_event(&Event::key_press(40, 0));
        assert_eq!(terminal.input_line(), "");
    }
}
