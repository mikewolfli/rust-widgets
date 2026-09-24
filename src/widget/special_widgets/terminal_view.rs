// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TerminalView widget.

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::animation::CursorBlink;
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
    /// The caret at the end of the input line, which blinks while the terminal is enabled.
    ///
    /// # Why a terminal needs one more than most fields
    ///
    /// A terminal's whole appearance is *text that has stopped arriving*. Without a caret there
    /// is nothing on screen that distinguishes "the shell is waiting for you" from "the shell
    /// has hung", and a reader's first instinct on a frozen prompt is to kill the process. This
    /// control drew no caret at all — no field, no blink, no `tick` — so it was the one control
    /// in the crate whose animation a user is most likely to be *waiting on*. The blink is the
    /// crate's shared [`CursorBlink`], the same primitive `line_edit` and `code_editor` use.
    cursor: CursorBlink,
    /// Emitted when command is submitted.
    pub command_submitted: Signal1<String>,
}

impl TerminalView {
    /// Creates terminal view.
    pub fn new(geometry: Rect) -> Self {
        let mut cursor = CursorBlink::new();
        // Started at construction rather than on `FocusGained`: a terminal is an *input device*
        // by definition, so a freshly mounted one is already waiting for a command. Waiting for a
        // focus event that a host may never send would put the caret back where this control was.
        cursor.start();
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TerminalView"),
            lines: Vec::new(),
            input_line: String::new(),
            history: Vec::new(),
            history_index: None,
            cursor,
            command_submitted: Signal1::new(),
        }
    }

    /// Whether the caret is drawn on the current frame.
    ///
    /// Exposed so the blink can be asserted without sleeping: the value is a function of the
    /// ticks the control was given, not of the wall clock.
    pub fn is_caret_visible(&self) -> bool {
        self.cursor.is_visible()
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is owed.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let owes_frame = self.cursor.tick(delta_ms);
        if owes_frame {
            // The caret's two halves are different pictures, so the frame that flips it must
            // repaint. Without this the blink would advance and never be drawn.
            self.base.request_redraw();
        }
        owes_frame
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

    /// One frame of the caret's blink. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        TerminalView::tick(self, delta_ms)
    }

    /// A blinking caret always owes frames while it runs; a disabled terminal's steady caret
    /// does not, which is what `CursorBlink::stop` is for.
    fn is_animating(&self) -> bool {
        self.cursor.is_running()
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
        // A disabled terminal holds a steady caret (the shared rule for a control that cannot be
        // typed into), and an enabled one blinks. This is the only place the two states differ.
        if !self.base.is_enabled() {
            self.cursor.stop();
            return;
        }
        if !self.cursor.is_running() {
            self.cursor.start();
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
        let prompt_bounds =
            Rect::new(rect.x + 8, prompt_y, rect.width.saturating_sub(16), prompt_line_height);
        context.draw_text_fitted(
            prompt_bounds,
            &prompt_text,
            &prompt_font,
            prompt_color,
            HorizontalAlignment::Left,
        );

        // The caret sits immediately after the input text, in the prompt's own ink — it is the
        // same line, continued. Its x is the measured advance of the text already typed, so it
        // tracks the input rather than sitting at a fixed offset from the field's left edge.
        //
        // Drawn only when the blink's own half-period says so: a caret that ignores
        // `cursor.is_visible()` is a caret that never blinks, which is the state this control was
        // in before it had one at all.
        if self.base.is_enabled() && self.cursor.is_visible() {
            let typed = context.measure_text(&prompt_text, &prompt_font);
            let caret_x = (rect.x + 8 + typed.width as i32)
                .min(rect.x + rect.width as i32 - 2)
                .max(rect.x + 8);
            // A caret is a thin, full-height block rather than a stroke: it is the position the
            // *next* character will occupy, so it is as tall as the line and as wide as one
            // pixel column of it.
            context.fill_rect(
                Rect::new(caret_x, prompt_y, 1, prompt_line_height.max(1)),
                prompt_color,
            );
        }
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
    /// The caret blinks on the ticks it is given, and stops when the terminal is disabled.
    ///
    /// Regression (this plan's A.8.1b P0): the control drew no caret at all — no field, no blink,
    /// no `tick` — so a terminal whose output had stopped was indistinguishable from one that had
    /// hung. The blink is asserted through `is_caret_visible` rather than a pixel, because the
    /// whole contract is that one boolean flipping on `delta_ms`.
    #[test]
    fn the_caret_blinks_on_the_ticks_it_is_given() {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 400, 200));
        assert!(terminal.is_animating(), "a fresh terminal waits for input, so it blinks");
        assert!(terminal.is_caret_visible(), "a blink starts visible");

        // A quarter period does not flip it; crossing the half-period does.
        assert!(terminal.tick(100), "a running caret owes another frame");
        assert!(terminal.is_caret_visible(), "100 ms is inside the visible half");
        terminal.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(!terminal.is_caret_visible(), "one half-period later it is hidden");
        terminal.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(terminal.is_caret_visible(), "and the next half brings it back");

        // A disabled terminal holds a steady caret and stops owing frames, so an idle disabled
        // control does not repaint forever.
        terminal.handle_event(&Event::key_press(65, 0));
        terminal.set_enabled(false);
        terminal.handle_event(&Event::key_press(65, 0));
        assert!(!terminal.is_animating(), "a disabled terminal's caret is steady");
        assert!(terminal.is_caret_visible(), "a steady caret is visible, not hidden");
        assert!(!terminal.tick(100), "and it owes no frames");
    }
}
