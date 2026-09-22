// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TextArea widget — multi-line text input (BLUE13 R2.5).
use crate::compat::{Box, Rc, RefCell, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
#[cfg(test)]
use crate::event::FocusReason;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::text_utils::floor_char_boundary;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Multi-line text input widget.
///
/// Supports multi-line text storage with a byte-index cursor, character-level
/// insertion and deletion, placeholder display, read-only mode, and word-wrap
/// toggling. Emits a `changed` signal whenever the text content changes.
pub struct TextArea {
    base: BaseWidget,
    /// Text content (lines separated by '\n').
    text: String,
    /// Cursor position (byte index into text).
    cursor_pos: usize,
    /// Maximum text length (0 = unlimited).
    max_length: usize,
    /// Whether the widget is read-only.
    read_only: bool,
    /// Placeholder text when empty.
    placeholder: String,
    /// Whether this widget currently holds keyboard focus.
    focused: bool,
    /// Signal emitted when text changes.
    pub changed: GenericSignal,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl TextArea {
    /// Creates a new `TextArea` with the given initial text and geometry.
    pub fn new(text: String, rect: Rect) -> Self {
        let cursor_pos = text.len();
        let history_target = Rc::new(RefCell::new(text.clone()));
        Self {
            base: BaseWidget::new(WidgetKind::TextArea, rect, "TextArea"),
            text,
            cursor_pos,
            max_length: 0,
            read_only: false,
            placeholder: String::new(),
            focused: false,
            changed: GenericSignal::new(),
            undo_stack: UndoStack::new(),
            history_target,
            restoring_history: false,
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the entire text content.
    ///
    /// Emits the `changed` signal if the new text differs from the current text.
    /// The cursor is moved to the end of the new text.
    /// Replaces the entire text content.
    ///
    /// Emits the `changed` signal if the new text differs from the current text.
    /// The cursor is moved to the end of the new text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        let max = self.max_length;
        let next = if max > 0 && text.len() > max {
            // Use floor_char_boundary to avoid splitting a multi-byte UTF-8 char
            let boundary = floor_char_boundary(&text, max);
            text[..boundary].to_string()
        } else {
            text
        };
        let before = self.text.clone();
        self.text = next;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "text_area_text",
            )));
        }
        self.cursor_pos = self.text.len();
        self.changed.emit();
        self.base.request_redraw();
    }

    /// Inserts a single character at the current cursor position.
    ///
    /// If `max_length` is greater than zero and the text already meets or exceeds
    /// that limit, the character is not inserted.
    ///
    /// SAFETY: Ensures `cursor_pos` is on a valid UTF-8 char boundary before
    /// inserting. If not, snaps to the nearest char boundary via `floor_char_boundary`.
    pub fn insert(&mut self, ch: char) {
        if self.max_length > 0 && self.text.len() >= self.max_length {
            return;
        }
        let boundary = floor_char_boundary(&self.text, self.cursor_pos);
        self.cursor_pos = boundary;
        let mut next = self.text.clone();
        next.insert(self.cursor_pos, ch);
        let new_cursor_pos = self.cursor_pos + ch.len_utf8();
        self.set_text(next);
        self.cursor_pos = new_cursor_pos.min(self.text.len());
    }

    /// Deletes the character immediately before the cursor.
    ///
    /// If the cursor is at position 0, this is a no-op.
    pub fn delete_char(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }
        let prev = self.text[..self.cursor_pos].char_indices().last().map(|(i, c)| {
            if c == '\n' {
                (i, 1)
            } else {
                (i, c.len_utf8())
            }
        });
        if let Some((start, len)) = prev {
            let mut next = self.text.clone();
            next.replace_range(start..start + len, "");
            self.set_text(next);
            self.cursor_pos = start;
        }
    }

    /// Returns the current cursor position (byte index into `text`).
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Sets the cursor position, clamping it to the text length.
    pub fn set_cursor_pos(&mut self, pos: usize) {
        let clamped = pos.min(self.text.len());
        if self.cursor_pos != clamped {
            self.cursor_pos = clamped;
            self.base.request_redraw();
        }
    }

    /// Sets the maximum text length.
    ///
    /// A value of `0` means unlimited. If the current text exceeds the new limit
    /// it is truncated and the cursor is adjusted accordingly.
    pub fn set_max_length(&mut self, max: usize) {
        let previous = self.max_length;
        self.max_length = max;
        if max > 0 && self.text.len() > max {
            self.text.truncate(max);
            self.cursor_pos = self.cursor_pos.min(max);
            self.changed.emit();
            self.base.request_redraw();
        } else if previous != max {
            self.base.request_redraw();
        }
    }

    /// Returns whether the text area is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    pub fn set_read_only(&mut self, ro: bool) {
        if self.read_only != ro {
            self.read_only = ro;
            self.base.request_redraw();
        }
    }

    /// Returns the placeholder text shown when the text is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text.
    pub fn set_placeholder(&mut self, text: String) {
        if self.placeholder != text {
            self.placeholder = text;
            self.base.request_redraw();
        }
    }

    /// Reverts the most recent text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `changed` and clamps the caret to the restored length.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `changed` and clamps the caret to the restored length.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns whether there is an undone text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.cursor_pos = self.cursor_pos.min(self.text.len());
        self.restoring_history = false;
        self.changed.emit();
        self.base.request_redraw();
    }
}

impl Widget for TextArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        let line_count = if self.text.is_empty() {
            1
        } else {
            self.text.chars().filter(|&c| c == '\n').count() + 1
        };
        let max_line_width = self.text.lines().map(|l| l.len() as u32).max().unwrap_or(0);
        let w = (max_line_width * 8 + 10).max(120);
        let h = (line_count as u32 * 16 + 10).max(60);
        Size::new(w, h)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TextArea`'s property contract.
impl WidgetProperties for TextArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TEXT_AREA_PROPERTIES`.
        property_names_of!["text", "placeholder", "read_only", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `text_area` publishes.
    ///
    /// `insert` and `delete_char` are the editing actions: they act at the caret, which
    /// the control owns, so they take no argument. On a fresh control `delete_char` is a
    /// no-op (the caret is already at position 0), but the command still ran, so it
    /// answers `Ok(())` rather than manufacturing an error for a legal invocation.
    ///
    /// `set_text`, `set_placeholder` and `set_read_only` assign state and need a
    /// payload, so they are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "insert" => {
                self.insert(' ');
                Ok(())
            }
            "delete_char" => {
                self.delete_char();
                Ok(())
            }
            "set_text" | "set_placeholder" | "set_read_only" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TextArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::FocusGained { .. } => {
                self.focused = true;
                self.request_redraw();
            }
            Event::FocusLost => {
                self.focused = false;
                self.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                if *modifiers == 2 && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if *modifiers == 2 && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                if self.read_only {
                    return;
                }
                match *key {
                    8 => {
                        // Backspace
                        self.delete_char();
                        self.request_redraw();
                    }
                    13 => {
                        // Enter — insert newline
                        self.insert('\n');
                        self.request_redraw();
                    }
                    37 => {
                        // Left arrow — move to previous char boundary
                        if self.cursor_pos > 0 {
                            let mut new_pos = self.cursor_pos;
                            while new_pos > 0 {
                                new_pos -= 1;
                                if self.text.is_char_boundary(new_pos) {
                                    break;
                                }
                            }
                            self.cursor_pos = new_pos;
                            self.request_redraw();
                        }
                    }
                    39 => {
                        // Right arrow — move to next char boundary
                        if self.cursor_pos < self.text.len() {
                            let mut new_pos = self.cursor_pos + 1;
                            while new_pos <= self.text.len() && !self.text.is_char_boundary(new_pos)
                            {
                                new_pos += 1;
                            }
                            self.cursor_pos = new_pos.min(self.text.len());
                            self.request_redraw();
                        }
                    }
                    _ => {
                        // Regular character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.insert(ch);
                                self.request_redraw();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Approximate pixel advance per character for `Font::default()`.
const CHAR_W: i32 = 8;
/// Approximate line height in pixels for `Font::default()`.
const LINE_H: i32 = 16;

impl Draw for TextArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // A text area is the one field whose height is **not** a fixed band: it is
        // multi-line, so the lines legitimately span whatever height the caller gives it.
        // `rect.height` therefore stays. Everything *inside* is measured from the field's
        // own padding rather than from a local literal, so the first line starts where a
        // single-line field's does instead of at a second, unshared `y = 4`.
        let rect = self.geometry();
        let origin_x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32;

        // -- Background --
        let bg = self.style().background_color.unwrap_or(Color::rgb(255, 255, 255));
        context.fill_rect(rect, bg);

        // -- Border --
        let border = self.style().border_color.unwrap_or(Color::rgb(200, 200, 200));
        context.draw_rect(rect, border);

        // -- Text --
        let text_color = self.style().text_color.unwrap_or(Color::rgb(0, 0, 0));
        let placeholder_color = Color::rgb(180, 180, 180);

        // The first line's own glyph box, centred on a line-height band below the top edge.
        // The origin of a text run is the *top-left corner of its glyph box*, so the old
        // `rect.y + padding` put that corner 4 px below the border and drew the first line
        // half a line high — the same off-by-a-half-line defect the single-line fields had,
        // and the reason the first line sat at `y = 4` in `text_area.svg` while its own
        // placeholder sat beside it on a different baseline.
        //
        // The band is one line tall and starts one inset below the top border, where a
        // single-line field's content starts, so a text area's first line and a `line_edit`'s
        // value sit on the same row when the two are laid out at the same height. Deriving
        // the band from the *first line* rather than from the whole control is what keeps a
        // 120 px text area from putting its first line halfway down the control.
        const FIRST_LINE_INSET: i32 = 4;
        let first_band = Rect::new(origin_x, rect.y + FIRST_LINE_INSET, rect.width, LINE_H as u32);
        let first_line = context.text_line(first_band, &Font::default());
        let text_top = first_line.y;

        if self.text.is_empty() && !self.placeholder.is_empty() && !self.focused {
            // Draw placeholder in gray, on the same first-line box the value would use.
            context.draw_text(
                Point::new(origin_x, text_top),
                &self.placeholder,
                &Font::default(),
                placeholder_color,
                HorizontalAlignment::Left,
            );
        } else if !self.text.is_empty() {
            let mut y = text_top;
            for line in self.text.lines() {
                if y + LINE_H > rect.y + rect.height as i32 {
                    break;
                }
                context.draw_text(
                    Point::new(origin_x, y),
                    line,
                    &Font::default(),
                    text_color,
                    HorizontalAlignment::Left,
                );
                y += LINE_H;
            }
            // Handle the case where text ends with '\n' (lines() strips trailing newline)
            if self.text.ends_with('\n') {
                // Draw an empty visual line so the cursor can be on the last line
                // (just advance y — nothing to draw for the empty line)
            }
        }

        // -- Cursor --
        if self.focused {
            let cursor_x = self.cursor_screen_x(origin_x);
            let cursor_y = self.cursor_screen_y(text_top);
            context.draw_line(
                Point::new(cursor_x, cursor_y),
                Point::new(cursor_x, cursor_y + LINE_H),
                text_color,
            );
        }
    }
}

impl TextArea {
    /// Computes the screen-space X coordinate of the cursor.
    fn cursor_screen_x(&self, origin_x: i32) -> i32 {
        // Find the character position within the current line
        let text_before = &self.text[..self.cursor_pos];
        // Find the last newline before cursor
        let line_start = text_before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = self.text[line_start..self.cursor_pos].len();
        origin_x + col as i32 * CHAR_W
    }

    /// Computes the screen-space Y coordinate of the cursor (top of cursor line).
    fn cursor_screen_y(&self, origin_y: i32) -> i32 {
        let text_before = &self.text[..self.cursor_pos];
        let lines_before = text_before.chars().filter(|&c| c == '\n').count();
        origin_y + lines_before as i32 * LINE_H
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn textarea_creation_defaults() {
        let ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.text(), "");
        assert_eq!(ta.cursor_pos(), 0);
        assert_eq!(ta.max_length, 0);
        assert!(!ta.is_read_only());
        assert!(ta.placeholder().is_empty());
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_set_text() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_text("Hello\nWorld".to_string());
        assert_eq!(ta.text(), "Hello\nWorld");
        assert_eq!(ta.cursor_pos(), 11); // cursor at end
    }

    #[test]
    fn textarea_insert_char() {
        let mut ta = TextArea::new("Helo".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(3);
        ta.insert('l');
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(5);
        ta.delete_char();
        assert_eq!(ta.text(), "Hell");
        assert_eq!(ta.cursor_pos(), 4);
    }

    #[test]
    fn textarea_delete_char_at_start() {
        let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        ta.set_cursor_pos(0);
        ta.delete_char();
        assert_eq!(ta.text(), "Hello");
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_delete_char_with_newline() {
        let mut ta = TextArea::new("A\nB".to_string(), Rect::new(0, 0, 300, 200));
        // "A\nB" — cursor at 3 (end), delete_char removes 'B', cursor at 2
        ta.set_cursor_pos(3);
        ta.delete_char();
        assert_eq!(ta.text(), "A\n");
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_cursor_movement() {
        let mut ta = TextArea::new("Hi".to_string(), Rect::new(0, 0, 300, 200));
        assert_eq!(ta.cursor_pos(), 2);
        ta.set_cursor_pos(0);
        assert_eq!(ta.cursor_pos(), 0);
        ta.set_cursor_pos(5); // clamp
        assert_eq!(ta.cursor_pos(), 2);
    }

    #[test]
    fn textarea_placeholder() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(ta.placeholder().is_empty());
        ta.set_placeholder("Enter text...".to_string());
        assert_eq!(ta.placeholder(), "Enter text...");
    }

    #[test]
    fn textarea_read_only() {
        let mut ta = TextArea::new("Readable".to_string(), Rect::new(0, 0, 300, 200));
        assert!(!ta.is_read_only());
        ta.set_read_only(true);
        assert!(ta.is_read_only());
    }

    #[test]
    fn textarea_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(5);
        assert_eq!(ta.max_length, 5);
        // insert beyond limit
        ta.set_text("Hello World".to_string());
        assert_eq!(ta.text().len(), 5);
        assert_eq!(ta.text(), "Hello");
    }

    #[test]
    fn textarea_draw_does_not_panic() {
        use crate::render::RenderContext;
        use crate::render::SoftwarePaintBackend;

        let mut ta = TextArea::new("Line1\nLine2".to_string(), Rect::new(0, 0, 200, 100));
        let mut backend = SoftwarePaintBackend::new(Size::new(200, 100), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        // Should not panic
        ta.draw(&mut ctx);
    }

    /// The first line is padded on the **field's** own inset from the top, not at `y = 4`.
    ///
    /// The origin of a text run is its glyph box's top-left corner, so `rect.y + 4` put that
    /// corner four pixels below the border and drew the first line half a line high — the
    /// placeholder and the value that replaced it therefore sat on different baselines, and
    /// `text_area.svg` showed a 14 px line at `y = 4`. The first line now takes the same
    /// line box every other text-bearing control uses.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn the_first_line_is_padded_consistently_with_every_other_field() {
        let mut ta = TextArea::new("Line1".to_string(), Rect::new(0, 0, 240, 120));
        let svg = crate::widget::svg::render_to_svg(&mut ta);
        let line = svg.lines().find(|l| l.contains("<text")).expect("a text element");
        let x: i32 = line
            .split(" x=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .expect("an x attribute");
        let y: i32 = line
            .split(" y=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .and_then(|value| value.parse().ok())
            .expect("a y attribute");

        assert_eq!(x, dimensions::TEXT_FIELD_PADDING_H as i32, "the shared horizontal inset");
        // The line box is centred inside the first line's band, which is inset from the
        // border; the assertion is that the glyph box sits clear of the top edge and still
        // inside the first row, rather than pinned to a bare literal.
        assert!(y > 0, "the first line clears the border: {y}");
        assert!(y < LINE_H, "and stays inside the first row: {y}");
    }

    #[test]
    fn textarea_set_text_truncates_on_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("Hello".to_string());
        assert_eq!(ta.text(), "Hel");
        assert_eq!(ta.cursor_pos(), 3);
    }

    #[test]
    fn textarea_insert_respects_max_length() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.set_max_length(3);
        ta.set_text("ABC".to_string());
        ta.insert('D');
        // Should not insert because max_length reached
        assert_eq!(ta.text(), "ABC");
        assert_eq!(ta.cursor_pos(), 3);
    }

    #[test]
    fn textarea_empty_text_events_no_panic() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace on empty
        assert_eq!(ta.text(), "");
        ta.handle_event(&Event::KeyPress { key: 37, modifiers: 0 }); // left arrow at 0
        assert_eq!(ta.cursor_pos(), 0);
        ta.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // right arrow at 0 (no change)
        assert_eq!(ta.cursor_pos(), 0);
    }

    #[test]
    fn textarea_focus_events() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        assert!(!ta.focused);
        ta.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
        assert!(ta.focused);
        ta.handle_event(&Event::FocusLost);
        assert!(!ta.focused);
    }

    #[test]
    fn textarea_keypress_insertion() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 72, modifiers: 0 }); // 'H'
        ta.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
        assert_eq!(ta.text(), "Hi");
    }

    #[test]
    fn textarea_keypress_enter_inserts_newline() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        ta.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
        ta.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
        ta.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
        assert_eq!(ta.text(), "A\nB");
    }

    #[test]
    fn textarea_setters_request_redraw() {
        let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        ta.base.redraw_requested.connect({
            let flag = std::sync::Arc::clone(&fired);
            move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        ta.set_text("hello");
        ta.set_cursor_pos(2);
        ta.set_max_length(3);
        ta.set_read_only(true);
        ta.set_placeholder("hint".to_string());

        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }
}
