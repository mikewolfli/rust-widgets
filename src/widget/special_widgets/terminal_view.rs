// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TerminalView widget.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

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
        self.lines.push(format!("> {cmd}"));
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

    /// Paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(600, 300)
    }

    impl_widget_property_hooks!();
}

/// `TerminalView`'s property contract, published under the `TextEdit` kind.
///
/// The capability layer makes `WidgetKind::TextEdit` the kind of this control
/// (`terminal_view_capability`), and this impl reproduces exactly what the old
/// `read_input_props` / `write_input_props` arms answered for that kind.
impl WidgetProperties for TerminalView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "output_line_count" => Ok(CapabilityValue::UInt(self.lines().len() as u64)),
            "input_line" => Ok(CapabilityValue::String(self.input_line().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "input_line" => {
                self.set_input_line(expect_string(value)?);
                Ok(())
            }
            // Derived from the output buffer. The old writer had no arm for it.
            "output_line_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TERMINAL_VIEW_PROPERTIES`.
        property_names_of!["output_line_count", "input_line", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `terminal_view` publishes.
    ///
    /// `submit` maps onto the widget's real `submit`, which reports `false` for
    /// an empty input line; that is reported as needing a payload rather than as
    /// a submitted command. `append_output` carries the line to append, so a bare
    /// invocation is answered the same way rather than being called unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "submit" => {
                if self.submit() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_output" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("terminal_view")
            .or_else(|| crate::style::resolved_theme_style("text_edit"));
        // `terminal_view` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical to the
        // window behind it. That is why the field is derived rather than taken from the
        // resolved style when the two coincide:
        //
        // The previous form stepped the resolved fill 8 % toward the ink, where the
        // resolved fill had itself come from `text_edit` (`rgb(180,180,180)` on the light
        // appearance). Two mid-grey steps of 8 % land at `rgb(166,166,166)` — a dark slab
        // on a light theme, and the 1.13:1 background the success-green prompt was then
        // drawn onto. The worst text ratio in the snapshot set was therefore not a text
        // defect at all: the *surface* was wrong, and the text was a symptom.
        //
        // A terminal body is a field, so it is derived the same way every other text
        // field in the crate derives one: from the window fill, stepped toward the ink.
        // The step is large enough to read as a distinct surface (a terminal is *not* the
        // window) and is applied to the window colour rather than to a value that may
        // already be a step. A caller's own colour still wins (rule #21).
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let field_from_theme = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&Color::BLACK, 0.14));
        let resolved = style.background_color.unwrap_or(field_from_theme);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.2));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(217, 224, 236));
        let background = resolved;
        // Output text is the *theme's* ink or, failing that, whatever contrasts with the
        // terminal body itself — not the resolved fill's own contrast, because the terminal
        // body is a deliberate step away from the window and the two must stay legible
        // together.
        let output_color = if style.text_color.is_some() || theme.is_some() {
            text_color
        } else {
            background.contrast_color()
        };
        // The prompt line is a *success* state — the shell is ready for input — so it reads
        // the theme's success token rather than a literal green. The token is then pushed
        // away from the terminal body until it clears the legibility floor: at full
        // saturation the green was 1.13:1 on a mid-grey body, which is a colour that exists
        // in the file and not on the screen.
        let prompt_color = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .map(|token| nudge_apart(token, background))
            .unwrap_or_else(|| background.blend(&text_color, 0.6));

        context.fill_rect(rect, background);
        context.draw_rect(rect, border);

        // The prompt row is anchored to the control's bottom edge, but it has to be a
        // *whole* row: the origin is the glyph's top-left, so `y = height - 10` placed the
        // line's top 10 px above the edge and let the glyphs paint 4 px below it. Reserving
        // one line height plus a margin keeps the prompt inside the frame, and it also
        // subtracts that row from the output area below so the two cannot overlap.
        let prompt_font = Font::default();
        let prompt_line_height = context.measure_text("M", &prompt_font).height.max(1);
        let prompt_row_height = prompt_line_height + 6;
        let body_height = rect.height.saturating_sub(prompt_row_height);

        let max_lines = (body_height.saturating_sub(16) / prompt_line_height) as usize;
        let start = self.lines.len().saturating_sub(max_lines);
        for (idx, line) in self.lines.iter().skip(start).enumerate() {
            let y = rect.y + 16 + idx as i32 * prompt_line_height as i32;
            let line_bounds =
                Rect::new(rect.x + 8, y, rect.width.saturating_sub(16), prompt_line_height);
            context.draw_text_fitted(
                line_bounds,
                line,
                &prompt_font,
                output_color,
                HorizontalAlignment::Left,
            );
        }

        let prompt_y = (rect.y + rect.height as i32 - prompt_row_height as i32 + 3).max(rect.y);
        let prompt_text = format!("> {}", self.input_line);
        context.draw_text_fitted(
            Rect::new(rect.x + 8, prompt_y, rect.width.saturating_sub(16), prompt_line_height),
            &prompt_text,
            &prompt_font,
            prompt_color,
            HorizontalAlignment::Left,
        );
    }
}

/// Pushes `ink` away from `surface` until it is legible on it, preserving the hue.
///
/// Retained as a named call site for the prompt colour: the rule itself lives on
/// [`Color::legible_on`] so the terminal, the calendar and the text editors share one
/// implementation rather than three drifting copies.
fn nudge_apart(ink: Color, surface: Color) -> Color {
    ink.legible_on(surface, 4.5)
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
