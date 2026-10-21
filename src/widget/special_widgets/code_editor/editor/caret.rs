// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cursor, selection and caret movement.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Cursor and selection ────────────────────────────────────────────────

    /// Returns the caret as `(line, column)`, both zero-based.
    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor.head.line, self.cursor.head.column)
    }

    /// Returns the caret as a [`TextPosition`].
    pub fn caret(&self) -> TextPosition {
        self.cursor.head
    }

    /// The IME composition currently shown at the caret, if one is active.
    ///
    /// Exposed so a host can anchor its candidate window and so a test can assert that a
    /// preedit frame arrived and that a commit cleared it. The composition is **not**
    /// part of [`Self::text`] until the commit.
    pub fn preedit_text(&self) -> Option<&str> {
        self.preedit.as_ref().map(|preedit| preedit.text.as_str())
    }

    /// Returns the selection anchor.
    pub fn anchor(&self) -> TextPosition {
        self.cursor.anchor
    }

    /// Returns `true` when text is selected.
    pub fn has_selection(&self) -> bool {
        !self.cursor.is_collapsed()
    }

    /// Returns the selected text, or `None` when the selection is collapsed.
    pub fn selected_text(&self) -> Option<String> {
        if self.cursor.is_collapsed() {
            return None;
        }
        let (start, end) = self.cursor.bounds();
        Some(self.model.borrow().extract(start, end))
    }

    /// Number of characters in the selection, **without building the string**.
    ///
    /// The status row prints this every frame. Asking [`Self::selected_text`] for
    /// it allocated a copy of the whole selection (a select-all on a large file is
    /// megabytes) only to count it and drop it. The count is taken from the line
    /// lengths, and includes the newline at each line boundary so the number is the
    /// same one the old code displayed.
    pub fn selection_char_count(&self) -> usize {
        let (start, end) = self.cursor.bounds();
        if start >= end {
            return 0;
        }
        let model = self.model.borrow();
        if start.line == end.line {
            return end.column.saturating_sub(start.column);
        }
        let mut total = model.line_len(start.line).saturating_sub(start.column);
        for line in (start.line + 1)..end.line {
            total += model.line_len(line);
        }
        total += end.column.min(model.line_len(end.line));
        // One newline per boundary crossed, matching `extract`'s output.
        total + (end.line - start.line)
    }

    /// Returns the normalized selection bounds.
    pub fn selection_bounds(&self) -> (TextPosition, TextPosition) {
        self.cursor.bounds()
    }

    /// Places the caret, optionally extending the selection.
    pub fn set_cursor(&mut self, line: usize, column: usize, extend_selection: bool) {
        self.move_caret_to(TextPosition::new(line, column), extend_selection);
    }

    /// Sets the caret from an explicit position.
    pub fn set_caret(&mut self, position: TextPosition, extend_selection: bool) {
        self.move_caret_to(position, extend_selection);
    }

    pub(crate) fn move_caret_to(&mut self, position: TextPosition, extend_selection: bool) {
        let clamped = self.clamp_position(position);
        if clamped == self.cursor.head && self.cursor.is_collapsed() && !extend_selection {
            return;
        }
        self.cursor.head = clamped;
        if !extend_selection {
            self.cursor.anchor = clamped;
            self.goal.active = false;
        }
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the entire document.
    pub fn select_all(&mut self) {
        let last = self.line_count().saturating_sub(1);
        self.cursor.anchor = TextPosition::new(0, 0);
        self.cursor.head = TextPosition::new(last, self.line_len(last));
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects one line, including its trailing newline when present.
    pub fn select_line(&mut self, line: usize) {
        let last = self.line_count().saturating_sub(1);
        let line = line.min(last);
        self.cursor.anchor = TextPosition::new(line, 0);
        self.cursor.head = if line < last {
            TextPosition::new(line + 1, 0)
        } else {
            TextPosition::new(line, self.line_len(line))
        };
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the word under a character position.
    pub fn select_word_at(&mut self, position: TextPosition) {
        let position = self.clamp_position(position);
        let (start, end) = self.word_bounds(position);
        self.cursor.anchor = start;
        self.cursor.head = end;
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Selects the word under a widget-local point.
    pub fn select_word_at_point(&mut self, point: Point) -> bool {
        self.last_pointer_position = Some(point);
        match self.position_at_point(point) {
            Some(position) => {
                self.select_word_at(position);
                true
            }
            None => false,
        }
    }

    pub(super) fn word_bounds(&self, position: TextPosition) -> (TextPosition, TextPosition) {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut start = position.column.min(chars.len());
        let mut end = start;
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if start == end {
            return (position, position);
        }
        (TextPosition::new(position.line, start), TextPosition::new(position.line, end))
    }

    // ── Caret movement ──────────────────────────────────────────────────────

    /// Moves the caret one character left/right or one line up/down.
    pub fn move_cursor(&mut self, delta_line: isize, delta_column: isize) {
        self.move_cursor_with_selection(delta_line, delta_column, false);
    }

    /// Moves the caret, extending the selection when requested.
    pub fn move_cursor_with_selection(
        &mut self,
        delta_line: isize,
        delta_column: isize,
        extend_selection: bool,
    ) {
        let mut head = self.cursor.head;
        if delta_line != 0 {
            let lines = self.line_count();
            let target =
                (head.line as isize + delta_line).clamp(0, lines.saturating_sub(1) as isize);
            head.line = target as usize;
            let provider = self.model.borrow();
            head.column = if self.goal.active {
                provider.clamp_column(head.line, self.goal.column)
            } else {
                provider.clamp_column(head.line, head.column)
            };
            drop(provider);
            self.goal = CursorGoal { active: true, column: head.column };
        } else {
            self.goal.active = false;
            let direction: isize = if delta_column < 0 { -1 } else { 1 };
            for _ in 0..delta_column.unsigned_abs() {
                head = self.step_horizontal(head, direction);
            }
        }
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Crosses line boundaries, which is what every real editor does.
    pub(super) fn step_horizontal(&self, position: TextPosition, direction: isize) -> TextPosition {
        let provider = self.model.borrow();
        if direction < 0 {
            if position.column > 0 {
                TextPosition::new(position.line, position.column - 1)
            } else if position.line > 0 {
                let line = position.line - 1;
                TextPosition::new(line, provider.line_len(line))
            } else {
                position
            }
        } else if position.column < provider.line_len(position.line) {
            TextPosition::new(position.line, position.column + 1)
        } else if position.line + 1 < provider.lines.len() {
            TextPosition::new(position.line + 1, 0)
        } else {
            position
        }
    }

    /// Moves the caret one word left (`direction < 0`) or right.
    pub fn move_word(&mut self, direction: isize, extend_selection: bool) {
        let head = if direction < 0 {
            self.previous_word_boundary(self.cursor.head)
        } else {
            self.next_word_boundary(self.cursor.head)
        };
        if head == self.cursor.head {
            return;
        }
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    pub(super) fn previous_word_boundary(&self, position: TextPosition) -> TextPosition {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        let previous_len = position.line.checked_sub(1).map(|line| provider.line_len(line));
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut column = position.column.min(chars.len());
        while column > 0 && !is_word(chars[column - 1]) {
            column -= 1;
        }
        while column > 0 && is_word(chars[column - 1]) {
            column -= 1;
        }
        if column == 0 && position.column == 0 {
            if let Some(previous_len) = previous_len {
                return TextPosition::new(position.line - 1, previous_len);
            }
        }
        TextPosition::new(position.line, column)
    }

    pub(super) fn next_word_boundary(&self, position: TextPosition) -> TextPosition {
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(position.line).chars().collect();
        drop(provider);
        let is_word = |ch: char| self.config.language.is_word_char(ch);
        let mut column = position.column.min(chars.len());
        while column < chars.len() && is_word(chars[column]) {
            column += 1;
        }
        while column < chars.len() && !is_word(chars[column]) {
            column += 1;
        }
        TextPosition::new(position.line, column)
    }

    /// Moves the caret to the start (`direction < 0`) or end of the line.
    /// Moves the caret to the start or end of the line.
    ///
    /// # Why Home is "smart" (as in Zed)
    ///
    /// `Home` goes to the **first non-whitespace character**, and only to column 0 when
    /// the caret is already there. The plain "column 0" behaviour makes an indented line
    /// take two presses to reach the indent's start, and one press to reach a column the
    /// user rarely wants. With a selection the destination is the line edge, because the
    /// press is collapsing the selection rather than navigating within the line.
    pub fn move_to_line_edge(&mut self, direction: isize, extend_selection: bool) {
        let line = self.cursor.head.line;
        let head = if direction < 0 {
            let text = self.model.borrow().line(line).to_string();
            let first_content = text.chars().take_while(|ch| *ch == ' ' || *ch == '\t').count();
            let collapsed = self.cursor.is_collapsed();
            let column = if collapsed && self.cursor.head.column != first_content {
                first_content
            } else {
                0
            };
            TextPosition::new(line, column)
        } else {
            TextPosition::new(line, self.line_len(line))
        };
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Moves the caret to the very start or end of the document.
    pub fn move_to_document_edge(&mut self, direction: isize, extend_selection: bool) {
        let head = if direction < 0 {
            TextPosition::new(0, 0)
        } else {
            let last = self.line_count().saturating_sub(1);
            TextPosition::new(last, self.line_len(last))
        };
        self.cursor.head = head;
        if !extend_selection {
            self.cursor.anchor = head;
        }
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    pub(super) fn selection_aligns_with_indent(&self) -> bool {
        let (start, _) = self.cursor.bounds();
        let chars: Vec<char> = self.model.borrow().line(start.line).chars().collect();
        chars[..start.column.min(chars.len())].iter().all(|ch| ch.is_whitespace())
    }
}
