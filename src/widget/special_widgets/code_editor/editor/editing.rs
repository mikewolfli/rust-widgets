// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Multi-caret edits, line commands and typing.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Multi-caret infrastructure ──────────────────────────────────────────

    /// Returns the caret set used for editing: secondaries sorted, primary last.
    pub(super) fn carets_for_edit(&self) -> Vec<Cursor> {
        let mut list = self.extra_cursors.clone();
        list.sort_by_key(|caret| {
            (caret.head.line, caret.head.column, caret.anchor.line, caret.anchor.column)
        });
        list.push(self.cursor);
        list
    }

    /// Applies one edit per caret in a single pass, then rebuilds the caret set.
    pub(super) fn apply_caret_edits(&mut self, carets: &[Cursor], edits: Vec<CaretEdit>) -> bool {
        if !self.is_editable() || carets.is_empty() || carets.len() != edits.len() {
            return false;
        }
        // Multi-caret is a distinct write path from `splice_and_record`, so the
        // locked-span guard has to be repeated here rather than inherited. The
        // whole batch is refused when *any* caret's edit touches a locked range:
        // applying the rest would leave the carets out of step with the document.
        if edits.iter().any(|edit| !self.is_range_editable(edit.start, edit.end)) {
            return false;
        }
        let before = self.text();
        let lines = self.model.borrow().lines.clone();
        let offset_edits: Vec<OffsetEdit> = edits
            .iter()
            .map(|edit| OffsetEdit {
                start: multicursor::char_offset(&lines, self.clamp_position(edit.start)),
                end: multicursor::char_offset(&lines, self.clamp_position(edit.end)),
                insert: edit.insert.clone(),
            })
            .collect();
        let (after, offsets) = multicursor::project(&before, &offset_edits);
        if after == before {
            return false;
        }
        let new_lines = split_lines(&after);
        let mut new_carets: Vec<Cursor> = Vec::with_capacity(carets.len());
        for (index, caret) in carets.iter().enumerate() {
            let position = match (offsets.get(index).copied().flatten(), edits.get(index)) {
                (Some(end_offset), Some(edit)) => {
                    let insert_len = edit.insert.chars().count();
                    let inside = edit.caret_in_insert.unwrap_or(insert_len).min(insert_len);
                    let offset = end_offset.saturating_sub(insert_len - inside);
                    multicursor::position_at_offset(&new_lines, offset)
                }
                // A dropped (overlapping) edit leaves its caret where it was.
                _ => caret.head,
            };
            new_carets.push(Cursor { head: position, anchor: position });
        }
        let primary = new_carets.pop().unwrap_or(self.cursor);
        // Record exactly one checkpoint. `set_text` below pushes its own entry when
        // history is live, so pushing here as well would make one multi-caret edit
        // undo in two steps — the buffer would come back half-edited. The range is
        // derived from the before/after pair because a multi-caret projection
        // rewrites several disjoint spans at once and has no single offset to
        // report.
        let range = diff_range(&before, &after);
        let checkpoint = if self.restoring_history {
            None
        } else {
            let target = self.model.borrow().text.clone();
            Some(match range {
                Some((start, removed_len, inserted)) => {
                    let removed =
                        before.get(start..start + removed_len).unwrap_or_default().to_string();
                    Box::new(TextRangeCommand::new(
                        target,
                        start,
                        removed,
                        inserted,
                        "code_editor_edit",
                    )) as Box<dyn UndoCommand>
                }
                None => Box::new(ValueCommand::new(
                    target,
                    before.clone(),
                    after.clone(),
                    "Edit text",
                    "code_editor_edit",
                )),
            })
        };
        // `set_text` would add a second entry of its own, so suppress it and push
        // the single checkpoint computed above instead.
        let was_restoring = self.restoring_history;
        self.restoring_history = true;
        self.model.borrow_mut().set_text(after.clone());
        self.restoring_history = was_restoring;
        if let Some(command) = checkpoint {
            self.undo_stack.push(command);
        }
        self.cursor = Cursor {
            head: self.clamp_position(primary.head),
            anchor: self.clamp_position(primary.anchor),
        };
        self.extra_cursors = new_carets
            .into_iter()
            .map(|caret| Cursor {
                head: self.clamp_position(caret.head),
                anchor: self.clamp_position(caret.anchor),
            })
            .collect();
        self.goal.active = false;
        self.refresh_derived_state();
        self.text_changed.emit(after);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Builds the per-caret edits for inserting `payload` at every caret.
    pub(super) fn insertion_edits(&self, carets: &[Cursor], payload: &str) -> Vec<CaretEdit> {
        carets
            .iter()
            .map(|caret| {
                let (start, end) = caret.bounds();
                CaretEdit::new(start, end, payload)
            })
            .collect()
    }

    /// Builds a delete-aware edit for one caret (selection, pair, or one char).
    pub(super) fn backspace_edit_for(&self, caret: Cursor) -> CaretEdit {
        if !caret.is_collapsed() {
            let (start, end) = caret.bounds();
            return CaretEdit::new(start, end, "");
        }
        let head = caret.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) && head.column > 0 {
            let start = TextPosition::new(head.line, head.column - 1);
            let end = if head.column < self.line_len(head.line) {
                TextPosition::new(head.line, head.column + 1)
            } else {
                head
            };
            return CaretEdit::new(start, end, "");
        }
        let start = if head.column > 0 {
            TextPosition::new(head.line, head.column - 1)
        } else if head.line > 0 {
            TextPosition::new(head.line - 1, self.line_len(head.line - 1))
        } else {
            return CaretEdit::new(head, head, "");
        };
        CaretEdit::new(start, head, "")
    }

    /// Builds a delete-forward edit for one caret.
    pub(super) fn forward_delete_edit_for(&self, caret: Cursor) -> CaretEdit {
        if !caret.is_collapsed() {
            let (start, end) = caret.bounds();
            return CaretEdit::new(start, end, "");
        }
        let head = caret.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) {
            let start =
                if head.column > 0 { TextPosition::new(head.line, head.column - 1) } else { head };
            let end = after.map(|_| TextPosition::new(head.line, head.column + 1)).unwrap_or(head);
            return CaretEdit::new(start, end, "");
        }
        let end = self.step_horizontal(head, 1);
        CaretEdit::new(head, end, "")
    }

    /// Builds the newline insert (auto-indent + block opening) for one caret.
    pub(super) fn newline_edit_for(&self, caret: Cursor) -> CaretEdit {
        let (start, end) = caret.bounds();
        let indent = self.line_indent(start.line);
        let char_before = self.char_before(start);
        let char_after = self.char_at(end);
        let opens_block = matches!(char_before, Some('{' | '(' | '['));
        let closes_block = matches!(char_after, Some('}' | ')' | ']'));
        let unit = self.indent_unit();
        let mut payload = String::from("\n");
        payload.push_str(&indent);
        if opens_block {
            payload.push_str(&unit);
        }
        if opens_block && closes_block {
            payload.push('\n');
            payload.push_str(&indent);
        }
        let caret_index =
            1 + indent.chars().count() + if opens_block { unit.chars().count() } else { 0 };
        CaretEdit::new(start, end, payload).with_caret(caret_index)
    }

    // ── Multi-caret public API ──────────────────────────────────────────────

    /// Returns the full caret set (primary plus secondaries).
    pub fn cursors(&self) -> MultiCursor {
        MultiCursor::from_parts(self.cursor, &self.extra_cursors)
    }

    /// Returns `true` when more than one caret is active.
    pub fn has_multiple_cursors(&self) -> bool {
        !self.extra_cursors.is_empty()
    }

    /// Returns the number of secondary carets.
    pub fn extra_cursor_count(&self) -> usize {
        self.extra_cursors.len()
    }

    /// Collapses every secondary caret, keeping the primary one.
    pub fn collapse_cursors(&mut self) -> bool {
        if self.extra_cursors.is_empty() {
            return false;
        }
        self.extra_cursors.clear();
        self.base.request_redraw();
        true
    }

    /// Adds a caret one line above the primary caret.
    pub fn add_cursor_above(&mut self) -> bool {
        self.add_caret_by(-1)
    }

    /// Adds a caret one line below the primary caret.
    pub fn add_cursor_below(&mut self) -> bool {
        self.add_caret_by(1)
    }

    pub(super) fn add_caret_by(&mut self, direction: isize) -> bool {
        let head = self.cursor.head;
        let last = self.line_count().saturating_sub(1) as isize;
        let target = head.line as isize + direction;
        if target < 0 || target > last {
            return false;
        }
        let line = target as usize;
        let column = self.line_len(line).min(head.column);
        let caret = Cursor {
            head: TextPosition::new(line, column),
            anchor: TextPosition::new(line, column),
        };
        if caret == self.cursor || self.extra_cursors.contains(&caret) {
            return false;
        }
        self.extra_cursors.push(self.cursor);
        self.cursor = caret;
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Adds a caret at `position`, keeping the existing carets.
    ///
    /// This is the second half of Zed's `Cmd`/`Ctrl` + click multi-cursor gesture;
    /// the platform reports the modifier, the pointer layer calls this. Returns
    /// `false` when there is already a caret there (including the primary one), so a
    /// double press cannot create two carets that would edit twice.
    pub fn add_cursor_at(&mut self, position: TextPosition) -> bool {
        let position = self.clamp_position(position);
        let caret = Cursor { head: position, anchor: position };
        if caret == self.cursor || self.extra_cursors.contains(&caret) {
            return false;
        }
        self.extra_cursors.push(self.cursor);
        self.cursor = caret;
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Selects the next occurrence of the current selection and adds a caret.
    ///
    /// With no selection this selects the word under the caret, mirroring the
    /// first press of Zed/VS Code `Cmd+D`.
    pub fn select_next_occurrence(&mut self) -> bool {
        let Some(selected) = self.selected_text() else {
            self.select_word_at(self.cursor.head);
            return self.has_selection();
        };
        if selected.is_empty() || selected.contains('\n') || selected.chars().count() > 128 {
            return false;
        }
        let hits =
            self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true });
        if hits.is_empty() {
            return false;
        }
        let primary = self.cursor.bounds();
        let set = self.cursors();
        let start_index = hits
            .iter()
            .position(|hit| {
                TextPosition::new(hit.line, hit.start_column) == primary.0
                    && TextPosition::new(hit.line, hit.end_column) == primary.1
            })
            .map(|index| index + 1)
            .unwrap_or(0);
        let mut chosen = None;
        for step in 0..hits.len() {
            let hit = &hits[(start_index + step) % hits.len()];
            let candidate = Cursor {
                head: TextPosition::new(hit.line, hit.end_column),
                anchor: TextPosition::new(hit.line, hit.start_column),
            };
            if set.carets().contains(&candidate) {
                continue;
            }
            chosen = Some(candidate);
            break;
        }
        let Some(candidate) = chosen else { return false };
        if !self.extra_cursors.contains(&self.cursor) {
            self.extra_cursors.push(self.cursor);
        }
        self.cursor = candidate;
        self.goal.active = false;
        self.ensure_caret_visible();
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        true
    }

    /// Selects every occurrence of the selection (or the caret word) at once.
    ///
    /// Returns the resulting caret count.
    pub fn select_all_occurrences(&mut self) -> usize {
        if !self.has_selection() {
            self.select_word_at(self.cursor.head);
        }
        let Some(selected) = self.selected_text() else { return 0 };
        if selected.is_empty() || selected.contains('\n') {
            return 0;
        }
        let hits =
            self.find_all_with(&selected, SearchOptions { case_sensitive: true, whole_word: true });
        if hits.is_empty() {
            return 0;
        }
        let primary = self.cursor.bounds();
        let mut extras = Vec::new();
        let mut primary_kept = false;
        for hit in &hits {
            let bounds = (
                TextPosition::new(hit.line, hit.start_column),
                TextPosition::new(hit.line, hit.end_column),
            );
            if bounds == primary {
                primary_kept = true;
                continue;
            }
            extras.push(Cursor { head: bounds.1, anchor: bounds.0 });
        }
        if !primary_kept {
            if let Some(first) = extras.first().copied() {
                self.cursor = first;
                extras.remove(0);
            }
        }
        self.extra_cursors = extras;
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
        self.cursors().len()
    }

    // ── Line commands ───────────────────────────────────────────────────────

    /// Duplicates the lines the selection touches, below the original block.
    pub fn duplicate_line(&mut self) {
        if !self.is_editable() {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let last = end.line.min(lines.len().saturating_sub(1));
        let block: Vec<String> = lines[start.line..=last].to_vec();
        let insert_at = last + 1;
        for (offset, line) in block.into_iter().enumerate() {
            lines.insert(insert_at + offset, line);
        }
        let after = join_lines(&lines);
        let first_copy = insert_at;
        let copy_count = end.line - start.line;
        self.commit_edit(before, after, TextPosition::new(first_copy, 0));
        // Select the duplicated block using the freshly written model.
        let select_last = first_copy + copy_count;
        let head = TextPosition::new(select_last, self.line_len(select_last));
        self.cursor = Cursor { head, anchor: TextPosition::new(first_copy, 0) };
        self.emit_cursor_and_selection();
    }

    /// Deletes the lines the selection touches.
    pub fn delete_line(&mut self) {
        if !self.is_editable() {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let last = end.line.min(lines.len().saturating_sub(1));
        lines.drain(start.line..=last);
        if lines.is_empty() {
            lines.push(String::new());
        }
        let after = join_lines(&lines);
        let head_line = start.line.min(lines.len().saturating_sub(1));
        let head = TextPosition::new(head_line, 0);
        self.commit_edit(before, after, head);
    }

    /// Joins the caret line with the following one, trimming its indentation.
    pub fn join_lines(&mut self) {
        if !self.is_editable() {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let line = end.line.min(lines.len().saturating_sub(1));
        if line + 1 >= lines.len() {
            return;
        }
        let head_column = self.line_len(line);
        let next = lines[line + 1].trim_start().to_string();
        let needs_space =
            !next.is_empty() && !lines[line].ends_with(' ') && !lines[line].ends_with('\t');
        if needs_space {
            lines[line].push(' ');
        }
        lines[line].push_str(&next);
        lines.remove(line + 1);
        let after = join_lines(&lines);
        let head = TextPosition::new(line, head_column);
        self.commit_edit(before, after, head);
        let _ = start;
    }

    /// Sorts the selected lines, or the whole document when none are selected.
    ///
    /// A **collapsed** caret counts as "none selected", which is what the summary above
    /// promises and what every editor does. Reading the caret's bounds literally instead
    /// made a collapsed caret sort the single line it sat on — a no-op for text that is
    /// already one line, and a silent partial sort otherwise, which is worse than either
    /// sorting everything or refusing.
    pub fn sort_lines(&mut self) {
        if !self.is_editable() {
            return;
        }
        let has_selection = self.has_selection();
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        // A trailing newline produces a final empty element. Sorting it would move
        // that empty line to the top of the document — `"b\na\n"` came back as
        // `"\na\nb"` — so it is held back and appended unchanged. It is a line
        // terminator's artefact, not a line the user can see or reorder.
        let trailing_blank = lines.last().is_some_and(|line| line.is_empty());
        let sortable = if trailing_blank { lines.len() - 1 } else { lines.len() };
        if sortable > 0 {
            // No selection: the whole sortable document. Otherwise the lines the
            // selection touches, inclusive of both ends.
            let (first, last) = if has_selection {
                (start.line.min(sortable - 1), end.line.min(sortable - 1))
            } else {
                (0, sortable - 1)
            };
            lines[first..=last].sort();
        }
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = TextPosition::new(sortable.saturating_sub(1), 0);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(0, 0);
        self.emit_cursor_and_selection();
    }

    /// Removes trailing whitespace from every line.
    pub fn trim_trailing_whitespace(&mut self) {
        if !self.is_editable() {
            return;
        }
        let before = self.text();
        let lines: Vec<String> =
            before.split('\n').map(|line| line.trim_end().to_string()).collect();
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = self.cursor.head;
        self.commit_edit(before, after, head);
    }

    /// Moves the caret to the first non-blank column of `line`.
    pub fn goto_line(&mut self, line: usize) {
        let last = self.line_count().saturating_sub(1);
        let line = line.min(last);
        let text = self.line_text(line).unwrap_or_default();
        let column = text.chars().take_while(|ch| ch.is_whitespace()).count();
        self.extra_cursors.clear();
        self.move_caret_to(TextPosition::new(line, column), false);
    }

    // ── Auto-pairing ────────────────────────────────────────────────────────

    /// Tries to apply auto-pairing for a single typed character.
    ///
    /// Returns `true` when the character was fully handled. Pairing is governed by
    /// the explicit [`CodeEditorConfig::auto_close_brackets`] switch rather than by
    /// language: it is a formatting preference, not a claim about grammar, so it is
    /// not among the commands D2 degrades.
    pub(crate) fn try_auto_pair(&mut self, typed: char) -> bool {
        if self.has_multiple_cursors() {
            return false;
        }
        let head = self.cursor.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        let has_selection = self.has_selection();
        match pairs::plan_typing(typed, before, after, has_selection) {
            PairAction::Plain => false,
            PairAction::SkipOver => {
                self.move_cursor(0, 1);
                true
            }
            PairAction::Pair => {
                let close = pairs::closing_for(typed).unwrap_or(typed);
                if has_selection {
                    self.wrap_selection(typed, close);
                } else {
                    let payload = [typed, close].iter().collect::<String>();
                    self.insert(&payload);
                    self.move_cursor(0, -1);
                }
                true
            }
        }
    }

    /// Wraps the selection in `open`/`close`, keeping the inner text selected.
    pub(super) fn wrap_selection(&mut self, open: char, close: char) {
        let (start, end) = self.cursor.bounds();
        let selected = self.model.borrow().extract(start, end);
        let newlines = selected.matches('\n').count();
        let payload = [open.to_string(), selected.clone(), close.to_string()].concat();
        let inner_column = if newlines == 0 {
            start.column + 1 + selected.chars().count()
        } else {
            trailing_line_len(&selected)
        };
        let head = TextPosition::new(start.line + newlines, inner_column);
        self.splice_range(start, end, &payload, head);
        let inner_start = TextPosition::new(start.line, start.column + 1);
        let inner_end = TextPosition::new(start.line + newlines, inner_column);
        self.cursor = Cursor {
            head: self.clamp_position(inner_end),
            anchor: self.clamp_position(inner_start),
        };
        self.emit_cursor_and_selection();
    }

    /// Deletes the character before the caret, or the selection.
    pub fn backspace(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.backspace_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let head = self.cursor.head;
        let before = self.char_before(head);
        let after = self.char_at(head);
        if pairs::should_delete_pair(before, after) && head.column > 0 {
            let start = TextPosition::new(head.line, head.column - 1);
            let end = if head.column < self.line_len(head.line) {
                TextPosition::new(head.line, head.column + 1)
            } else {
                head
            };
            self.splice_range(start, end, "", start);
            return;
        }
        let start = if head.column > 0 {
            TextPosition::new(head.line, head.column - 1)
        } else if head.line > 0 {
            TextPosition::new(head.line - 1, self.line_len(head.line - 1))
        } else {
            return;
        };
        self.splice_range(start, head, "", start);
    }

    /// Deletes the character after the caret, or the selection.
    pub fn delete_forward(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.forward_delete_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let head = self.cursor.head;
        let end = self.step_horizontal(head, 1);
        if end == head {
            return;
        }
        self.splice_range(head, end, "", head);
    }

    /// Deletes the whole word before the caret.
    pub fn delete_word_backward(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        let start = self.previous_word_boundary(self.cursor.head);
        if start == self.cursor.head {
            return;
        }
        self.splice_range(start, self.cursor.head, "", start);
    }

    /// Deletes the current selection.
    pub fn delete_selection(&mut self) {
        if !self.is_editable() || self.cursor.is_collapsed() {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets
                .iter()
                .map(|caret| {
                    let (start, end) = caret.bounds();
                    CaretEdit::new(start, end, "")
                })
                .collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        let (start, _) = self.cursor.bounds();
        let (_, end) = self.cursor.bounds();
        self.splice_range(start, end, "", start);
    }

    /// Inserts a newline, carrying the indentation over and opening blocks.
    pub fn insert_newline(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = carets.iter().map(|caret| self.newline_edit_for(*caret)).collect();
            self.apply_caret_edits(&carets, edits);
            return;
        }
        let (start, end) = self.cursor.bounds();
        let indent = self.line_indent(start.line);
        let char_before = self.char_before(start);
        let char_after = self.char_at(end);
        let opens_block = matches!(char_before, Some('{' | '(' | '['));
        let closes_block = matches!(char_after, Some('}' | ')' | ']'));
        let unit = self.indent_unit();

        let mut payload = String::from("\n");
        payload.push_str(&indent);
        if opens_block {
            payload.push_str(&unit);
        }
        let head = if opens_block && closes_block {
            payload.push('\n');
            payload.push_str(&indent);
            TextPosition::new(start.line + 1, indent.chars().count() + unit.chars().count())
        } else {
            TextPosition::new(
                start.line + 1,
                indent.chars().count() + if opens_block { unit.chars().count() } else { 0 },
            )
        };
        self.splice_range(start, end, &payload, head);
    }

    /// Inserts the indent unit, or indents the selection when it spans lines.
    pub fn insert_tab(&mut self) {
        if !self.is_editable() {
            return;
        }
        if self.has_selection()
            && self.cursor.line_span() > 1
            && self.selection_aligns_with_indent()
        {
            self.indent_selection();
            return;
        }
        let unit = self.indent_unit();
        self.insert(&unit);
    }

    /// Removes up to one indent unit of leading whitespace before the caret.
    pub fn outdent(&mut self) {
        if !self.is_editable() {
            return;
        }
        let line = self.cursor.head.line;
        let provider = self.model.borrow();
        let chars: Vec<char> = provider.line(line).chars().collect();
        drop(provider);
        let removable = chars
            .iter()
            .take_while(|ch| **ch == ' ' || **ch == '\t')
            .count()
            .min(self.config.tab_width)
            .min(self.cursor.head.column);
        if removable == 0 {
            return;
        }
        self.splice_range(
            TextPosition::new(line, 0),
            TextPosition::new(line, removable),
            "",
            TextPosition::new(line, self.cursor.head.column - removable),
        );
    }

    /// Indents every line the selection touches, in one undo step.
    pub fn indent_selection(&mut self) {
        if !self.is_editable() {
            return;
        }
        let unit = self.indent_unit();
        self.transform_selected_lines(
            |line, _| format!("{unit}{line}"),
            |column| column + unit.chars().count(),
        );
    }

    /// Outdents every line the selection touches, in one undo step.
    pub fn outdent_selection(&mut self) {
        if !self.is_editable() {
            return;
        }
        let width = self.config.tab_width;
        self.transform_selected_lines(
            |line, _| {
                let removable =
                    line.chars().take_while(|ch| *ch == ' ' || *ch == '\t').count().min(width);
                line.chars().skip(removable).collect()
            },
            |column| column.saturating_sub(width),
        );
    }

    /// Toggles line comments across the selection, in one undo step.
    ///
    /// Degrades explicitly for languages with no line-comment syntax: rather than
    /// inserting `// ` into Markdown or plain text, the command does nothing. The
    /// language is the *active buffer's*, not the editor default, so a Markdown tab
    /// and a Rust tab in the same widget behave differently.
    pub fn toggle_line_comment(&mut self) {
        if !self.is_editable() {
            return;
        }
        let Some(token) = self.active_language().line_comment_prefix() else {
            return;
        };
        let bare = token.trim_end();
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        let last = end.line.min(lines.len().saturating_sub(1));
        let range: Vec<usize> = (start.line..=last).collect();
        let all_commented = range
            .iter()
            .filter(|index| lines.get(**index).map(|line| !line.trim().is_empty()).unwrap_or(false))
            .all(|index| {
                lines.get(*index).map(|line| line.trim_start().starts_with(bare)).unwrap_or(false)
            });

        let delta: isize = if all_commented {
            -(token.chars().count() as isize)
        } else {
            token.chars().count() as isize
        };
        for index in &range {
            let Some(line) = lines.get_mut(*index) else { continue };
            if line.trim().is_empty() {
                continue;
            }
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            let body: String = line.chars().skip(indent).collect();
            let stripped =
                body.strip_prefix(token).or_else(|| body.strip_prefix(bare)).unwrap_or(&body);
            let rebuilt = if all_commented {
                format!("{}{stripped}", line.chars().take(indent).collect::<String>())
            } else {
                format!("{}{token}{stripped}", line.chars().take(indent).collect::<String>())
            };
            *line = rebuilt;
        }
        let after = join_lines(&lines);
        let shift = |column: usize, line_index: usize| {
            let length = lines.get(line_index).map(|line| line.chars().count()).unwrap_or(0);
            ((column as isize + delta).clamp(0, length as isize)) as usize
        };
        let head = TextPosition::new(end.line, shift(end.column, end.line));
        let before_anchor = TextPosition::new(start.line, shift(start.column, start.line));
        self.commit_edit(before, after, head);
        self.cursor.anchor = before_anchor;
        self.emit_cursor_and_selection();
    }

    /// Moves the caret line up (`direction < 0`) or down, preserving text.
    pub fn move_line(&mut self, direction: isize) {
        if !self.is_editable() || direction == 0 {
            return;
        }
        let line = self.cursor.head.line;
        let last = self.line_count().saturating_sub(1);
        let target = line as isize + direction;
        if target < 0 || target > last as isize {
            return;
        }
        let target = target as usize;
        let before = self.text();
        let mut lines = before.split('\n').map(|text| text.to_string()).collect::<Vec<_>>();
        if line >= lines.len() || target >= lines.len() {
            return;
        }
        lines.swap(line, target);
        let after = join_lines(&lines);
        let head = TextPosition::new(target, self.cursor.head.column);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(target, 0);
        self.cursor.head = TextPosition::new(target, self.line_len(target).min(head.column));
        self.emit_cursor_and_selection();
    }

    /// Applies `rewrite` to every line the selection touches.
    ///
    /// `adjust` maps a caret column through the same transformation so the
    /// selection survives the edit.
    pub(super) fn transform_selected_lines<F, G>(&mut self, rewrite: F, adjust: G)
    where
        F: Fn(&str, usize) -> String,
        G: Fn(usize) -> usize,
    {
        let (start, end) = self.cursor.bounds();
        let before = self.text();
        let mut lines = before.split('\n').map(|line| line.to_string()).collect::<Vec<_>>();
        let last = end.line.min(lines.len().saturating_sub(1));
        for (index, line) in lines.iter_mut().enumerate() {
            if index >= start.line && index <= last {
                *line = rewrite(line, index);
            }
        }
        let after = join_lines(&lines);
        let head = TextPosition::new(end.line, adjust(end.column));
        let anchor = TextPosition::new(start.line, adjust(start.column));
        self.commit_edit(before, after, head);
        self.cursor.anchor = self.clamp_position(anchor);
        self.emit_cursor_and_selection();
    }

    pub(super) fn indent_unit(&self) -> String {
        if self.config.insert_spaces {
            " ".repeat(self.config.tab_width)
        } else {
            "\t".to_string()
        }
    }

    pub(super) fn line_indent(&self, line: usize) -> String {
        self.model.borrow().line(line).chars().take_while(|ch| *ch == ' ' || *ch == '\t').collect()
    }

    pub(super) fn char_before(&self, position: TextPosition) -> Option<char> {
        let provider = self.model.borrow();
        let text = provider.line(position.line);
        let mut chars = text.chars().take(position.column);
        let mut last = chars.next()?;
        for ch in chars {
            last = ch;
        }
        Some(last)
    }

    pub(super) fn char_at(&self, position: TextPosition) -> Option<char> {
        self.model.borrow().line(position.line).chars().nth(position.column)
    }

    // ── History ─────────────────────────────────────────────────────────────

    /// Undoes the most recent edit. Returns `false` when there is nothing to do.
    pub fn undo(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let before_tab = self.active_buffer();
        if self.undo_stack.undo().is_err() {
            return false;
        }
        if before_tab != self.active_buffer() {
            // Undoing a tab switch restored a different document; the locked
            // ranges belonged to the previous document and must not leak into
            // this one.
            self.read_only_spans.clear();
        }
        self.restore_history_text();
        true
    }

    /// Redoes the most recently undone edit.
    pub fn redo(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let before_tab = self.active_buffer();
        if self.undo_stack.redo().is_err() {
            return false;
        }
        if before_tab != self.active_buffer() {
            self.read_only_spans.clear();
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` when an undo is available.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns `true` when a redo is available.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Clears the edit history without touching the text.
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
    }

    pub(super) fn restore_history_text(&mut self) {
        // The undo command already wrote the buffer directly (a range command
        // splices, a snapshot assigns), so `set_text` here would see an unchanged
        // document and return early — leaving the derived state stale. Rebuild the
        // line index from the buffer instead of routing through `set_text`.
        let restored = self.model.borrow().text.borrow().clone();
        self.restoring_history = true;
        self.model.borrow_mut().set_text(restored.clone());
        self.restoring_history = false;
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        self.scale = DocumentScale::for_lines(self.line_count());
        self.clamp_cursor_to_document();
        self.refresh_derived_state();
        self.text_changed.emit(restored);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }
}
