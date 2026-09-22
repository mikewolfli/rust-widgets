// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MarkdownEditor widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

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
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
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
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
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
        let before = self.text.clone();
        self.text = next.clone();
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "markdown_editor_text",
            )));
        }
        self.cursor_line = self.cursor_line.min(self.line_count().saturating_sub(1));
        self.text_changed.emit(next);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one line to markdown.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        let mut next = self.text.clone();
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str(line.as_ref());
        self.set_text(next);
        self.cursor_line = self.line_count().saturating_sub(1);
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

    /// Reverts the most recent markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise requests layout and redraw and clamps `cursor_line`.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise requests layout and redraw.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a markdown text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone markdown text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.cursor_line = self.cursor_line.min(self.line_count().saturating_sub(1));
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MarkdownEditor`'s property contract.
///
/// `text` writes go through `set_text`, which keeps the undo stack and the
/// emitted `text_changed` signal in step with the stored document; writing the
/// field directly would make the editor's own history wrong. `cursor_line` is
/// read-only because the editor owns caret placement.
impl WidgetProperties for MarkdownEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "preview_mode" => Ok(CapabilityValue::Bool(self.preview_mode())),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count() as u64)),
            "word_count" => Ok(CapabilityValue::UInt(self.word_count() as u64)),
            "heading_count" => Ok(CapabilityValue::UInt(self.heading_count() as u64)),
            "cursor_line" => Ok(CapabilityValue::UInt(self.cursor_line() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "preview_mode" => match value {
                CapabilityValue::Bool(enabled) => {
                    self.set_preview_mode(enabled);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "word_count" | "heading_count" | "cursor_line" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "preview_mode",
            "line_count",
            "word_count",
            "heading_count",
            "cursor_line",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `markdown_editor` publishes.
    ///
    /// `toggle_preview_mode` and `undo` are the genuine zero-argument actions here;
    /// `undo` reports `false` when there is nothing to undo, which is the "could not
    /// handle it" case, so it is answered with
    /// [`CapabilityAccessError::OutOfRange`] rather than a success that did nothing.
    /// `append_line` takes the line to append and `set_text` / `set_preview_mode`
    /// assign state through the property route, so those are refused the same way —
    /// the names are right and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle_preview_mode" => {
                self.toggle_preview_mode();
                Ok(())
            }
            "undo" => {
                if self.undo() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_line" | "set_text" | "set_preview_mode" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
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
                90 if *modifiers == 2 => {
                    let _ = self.undo();
                }
                89 if *modifiers == 2 => {
                    let _ = self.redo();
                }
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

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the surface, the border
        // and every piece of text were hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(252, 252, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(194, 201, 213));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(59, 72, 92));
        // The header is the dimmest ink that still reads, and the caret line is the
        // accent: both derived from the resolved ink and surface, so they move with
        // the appearance instead of being fixed literals.
        //
        // The caret line is the accent colour because it *means* "the line being edited",
        // so the hue is part of the information and must survive. It cannot be pushed
        // straight onto the surface unchecked, though: on the light appearance the editor
        // surface resolves to a mid grey and `colors.primary` is the saturated Material
        // blue, which measured 1.51:1 — the very line the caret is on became the least
        // readable line in the editor. `legible_on` keeps the hue and guarantees the ratio.
        let header_ink = ink.blend(&surface, 0.35);
        let cursor_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary.legible_on(surface, 4.5))
            .unwrap_or_else(|| ink.contrast_color());

        context.fill_rect(rect, surface);
        context.draw_rect(rect, border);

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
        // Fitted to the control's width: the header is a long line of counts, and a narrow
        // editor previously drew it straight past its own right edge.
        let header_band = Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), 14);
        context.draw_text_fitted(
            header_band,
            &header,
            &Font::default(),
            header_ink,
            HorizontalAlignment::Left,
        );

        for (idx, line) in self.text.lines().take(10).enumerate() {
            let y = rect.y + 36 + (idx as i32) * 16;
            if y > rect.y + rect.height as i32 - 8 {
                break;
            }
            let color = if idx == self.cursor_line { cursor_ink } else { ink };
            let rendered =
                if self.preview_mode { line.trim_start_matches('#').trim_start() } else { line };
            context.draw_text(
                Point::new(rect.x + 12, y),
                rendered,
                &Font::default(),
                color,
                HorizontalAlignment::Left,
            );
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

    #[test]
    fn undo_redo_restores_markdown_text() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One");
        editor.append_line("body");

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.text(), "# One");
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.text(), "# One\nbody");
    }

    #[test]
    fn keyboard_shortcuts_drive_markdown_history() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("draft");
        editor.set_text("final");

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(editor.text(), "draft");
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
        assert_eq!(editor.text(), "final");
    }
}
