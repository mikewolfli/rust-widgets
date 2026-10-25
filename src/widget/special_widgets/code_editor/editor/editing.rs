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
    ///
    /// # Why this is in-place
    ///
    /// The batch used to clone the whole document, split it into lines, splice the edits
    /// in a projected copy and write the result back through `set_text`. On a large file
    /// that made every multi-cursor keystroke `O(document)` instead of `O(carets x line)`.
    /// Each edit is now applied through the model's own splice (the same primitive a single
    /// caret uses), back-to-front so a splice never moves an edit that has not run yet.
    /// The cost is proportional to the edited lines (J-P02).
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
        // Every caret produced an empty insertion at a collapsed position: there is nothing
        // to write, and reporting success keeps the caller from falling through to the
        // single-caret path, which would apply the edit twice.
        if edits.iter().all(|edit| edit.insert.is_empty() && edit.start == edit.end) {
            return true;
        }
        // Back-to-front: the last edit in the document is applied first, so the
        // coordinates of the remaining (earlier) edits are still valid.
        let mut order: Vec<usize> = (0..edits.len()).collect();
        order.sort_by_key(|index| {
            let edit = &edits[*index];
            (edit.start.line, edit.start.column, edit.end.line, edit.end.column)
        });
        order.reverse();
        let Some(&lowest) = order.last() else { return false };
        let Some(&highest) = order.first() else { return false };
        let span_start = self.clamp_position(edits[lowest].start);
        let mut span_end = self.clamp_position(edits[highest].end);
        let before_span = self.model.borrow().extract(span_start, span_end);
        let span_offset = self.model.borrow_mut().total_byte_offset(span_start);

        let mut positions: Vec<Option<TextPosition>> = vec![None; edits.len()];
        let original: Vec<TextPosition> = carets.iter().map(|caret| caret.head).collect();
        // The lowest start already written. An edit that ends past it would rewrite text a
        // previous edit already replaced, so it is dropped (and its caret left in place),
        // matching the overlap rule the old projection had.
        let mut lowest_applied: Option<TextPosition> = None;
        for &index in &order {
            let edit = &edits[index];
            let start = self.clamp_position(edit.start);
            let end = self.clamp_position(edit.end);
            if lowest_applied.is_some_and(|lowest| end > lowest) {
                continue;
            }
            lowest_applied = Some(start);
            let insert = edit.insert.clone();
            let _ = self.model.borrow_mut().splice_with_range(start, end, &insert);
            let caret = caret_inside_insert(start, &insert, edit.caret_in_insert);
            // Shift everything already placed that sits at or after this edit. The
            // column rule is "distance from the edit's end added to the length of the
            // inserted text's last line", which is exact for a plain insertion and for a
            // replacement that spans lines.
            let line_delta =
                insert.matches('\n').count() as isize - (end.line - start.line) as isize;
            let last_line_len = trailing_line_len(&insert);
            let shift = |position: &mut TextPosition| {
                if position.line == end.line && position.column >= end.column {
                    position.column = last_line_len + (position.column - end.column);
                }
                if position.line >= end.line {
                    position.line = (position.line as isize + line_delta).max(0) as usize;
                }
            };
            for position in positions.iter_mut().flatten() {
                shift(position);
            }
            if span_end.line > end.line
                || (span_end.line == end.line && span_end.column >= end.column)
            {
                shift(&mut span_end);
            }
            positions[index] = Some(caret);
        }

        let primary_index = positions.len() - 1;
        let primary =
            positions.get(primary_index).copied().flatten().unwrap_or(original[primary_index]);
        positions.pop();
        let new_carets: Vec<Cursor> = positions
            .iter()
            .enumerate()
            .map(|(index, position)| position.unwrap_or(original[index]))
            .map(|head| Cursor {
                head: self.clamp_position(head),
                anchor: self.clamp_position(head),
            })
            .collect();
        self.cursor =
            Cursor { head: self.clamp_position(primary), anchor: self.clamp_position(primary) };
        self.extra_cursors = new_carets;
        self.goal.active = false;
        // One checkpoint over the region the batch touched, rather than a whole-document
        // snapshot: the text above and below the outermost carets is unchanged, so it
        // cannot be part of the edit. `set_text` is deliberately not used here, since it
        // would push a second history entry of its own.
        if !self.restoring_history {
            let after_span = self.model.borrow().extract(span_start, span_end);
            let target = self.model.borrow().text.clone();
            self.undo_stack.push(Box::new(TextRangeCommand::new(
                target,
                span_offset,
                before_span,
                after_span,
                "code_editor_edit",
            )));
        }
        self.active_typing = None;
        self.revision = self.revision.wrapping_add(1);
        // Several disjoint word changes cannot be applied to the completion index one
        // range at a time, so it is dropped and rebuilt on the next trigger.
        self.completion_index = None;
        self.refresh_derived_state();
        // A multi-caret batch rewrites several disjoint regions, so there is no single
        // line range to report; it is described conservatively as a whole-document change
        // (a subscriber re-reads what it needs) rather than pretending to one span.
        self.text_change.emit(TextChange::document(self.line_count()));
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
        let hits = self.find_all_with(
            &selected,
            SearchOptions { case_sensitive: true, whole_word: true, ..SearchOptions::default() },
        );
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
        let hits = self.find_all_with(
            &selected,
            SearchOptions { case_sensitive: true, whole_word: true, ..SearchOptions::default() },
        );
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

    /// Inserts an empty line below the caret line and moves the caret onto it.
    pub fn insert_line_below(&mut self) {
        if !self.is_editable() {
            return;
        }
        let line = self.cursor.head.line;
        let end = self.line_len(line);
        let indent = self.line_indent(line);
        let width = indent.chars().count();
        let payload = format!("\n{indent}");
        let head = TextPosition::new(line + 1, width);
        self.splice_range(
            TextPosition::new(line, end),
            TextPosition::new(line, end),
            &payload,
            head,
        );
    }

    /// Inserts an empty line above the caret line and keeps the caret on the
    /// original text.
    pub fn insert_line_above(&mut self) {
        if !self.is_editable() {
            return;
        }
        let line = self.cursor.head.line;
        let indent = self.line_indent(line);
        let width = indent.chars().count();
        let payload = format!("\n{indent}");
        let head = TextPosition::new(line + 1, width);
        self.splice_range(TextPosition::new(line, 0), TextPosition::new(line, 0), &payload, head);
    }

    /// Swaps the two characters around the caret.
    ///
    /// At the end of a line the last two characters are swapped, which is the
    /// behaviour a reader expects from `Ctrl+T`; there is no useful transposition
    /// of a one-character line, so that case is a no-op.
    pub fn transpose_characters(&mut self) {
        if !self.is_editable() {
            return;
        }
        let line = self.cursor.head.line;
        let text = self.line_text(line).unwrap_or_default();
        let mut chars: Vec<char> = text.chars().collect();
        if chars.len() < 2 {
            return;
        }
        let left = self.cursor.head.column.min(chars.len()).saturating_sub(1);
        if left + 1 >= chars.len() {
            return;
        }
        chars.swap(left, left + 1);
        let rebuilt: String = chars.into_iter().collect();
        let head = TextPosition::new(line, (left + 2).min(self.line_len(line)));
        self.splice_range(
            TextPosition::new(line, 0),
            TextPosition::new(line, self.line_len(line)),
            &rebuilt,
            head,
        );
    }

    /// Moves the selected text to `target`, in one undoable edit.
    ///
    /// This is the in-editor half of text drag and drop: the pointer layer only
    /// reports where the drag ended. A target inside the selection is a no-op, so
    /// picking a selection up and dropping it back cannot delete it.
    pub fn move_selection_to(&mut self, target: TextPosition) -> bool {
        if !self.is_editable() || self.cursor.is_collapsed() {
            return false;
        }
        let (start, end) = self.cursor.bounds();
        let target = self.clamp_position(target);
        if target >= start && target <= end {
            return false;
        }
        let before = self.text();
        let lines = split_lines(&before);
        let first = multicursor::char_offset(&lines, start);
        let last = multicursor::char_offset(&lines, end);
        let at = multicursor::char_offset(&lines, target);
        let chars: Vec<char> = before.chars().collect();
        let selected: Vec<char> = chars[first..last].to_vec();
        let mut out: Vec<char> = Vec::with_capacity(chars.len());
        let caret_offset = if at <= first {
            out.extend_from_slice(&chars[..at]);
            out.extend_from_slice(&selected);
            out.extend_from_slice(&chars[at..first]);
            out.extend_from_slice(&chars[last..]);
            at + selected.len()
        } else {
            out.extend_from_slice(&chars[..first]);
            out.extend_from_slice(&chars[last..at]);
            out.extend_from_slice(&selected);
            out.extend_from_slice(&chars[at..]);
            at
        };
        let after: String = out.into_iter().collect();
        let new_lines = split_lines(&after);
        let head = multicursor::position_at_offset(&new_lines, caret_offset);
        self.commit_edit(before, after, head);
        self.cursor = Cursor { head, anchor: head };
        self.emit_cursor_and_selection();
        true
    }

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
        self.sort_selected_lines(false, false);
    }

    /// Sorts the selected lines in descending order.
    pub fn sort_lines_descending(&mut self) {
        self.sort_selected_lines(true, false);
    }

    /// Sorts the selected lines and removes duplicate lines.
    pub fn sort_lines_unique(&mut self) {
        self.sort_selected_lines(false, true);
    }

    fn sort_selected_lines(&mut self, descending: bool, unique: bool) {
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
            if descending {
                lines[first..=last].reverse();
            }
            if unique {
                let mut seen = Vec::new();
                let mut deduped = Vec::new();
                for line in lines[first..=last].iter() {
                    if !seen.contains(line) {
                        seen.push(line.clone());
                        deduped.push(line.clone());
                    }
                }
                lines.splice(first..=last, deduped);
            }
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

    /// Wraps or unwraps the selection in a block comment.
    ///
    /// Languages without block-comment syntax (Markdown, plain text) are a no-op,
    /// exactly like [`Self::toggle_line_comment`]. A collapsed caret comments the
    /// whole line, which is what the gesture means when there is no selection.
    pub fn toggle_block_comment(&mut self) {
        if !self.is_editable() {
            return;
        }
        if !self.active_language().supports_block_comments() {
            return;
        }
        let before = self.text();
        let mut lines: Vec<String> = before.split('\n').map(|line| line.to_string()).collect();
        let (start, end) = self.cursor.bounds();
        let last = end.line.min(lines.len().saturating_sub(1));
        if !self.has_selection() {
            let Some(line) = lines.get_mut(start.line) else { return };
            let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
            let body: String = line.chars().skip(indent).collect();
            let trimmed = body.trim();
            let rebuilt = if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
                let inner = trimmed
                    .strip_prefix("/*")
                    .unwrap_or(trimmed)
                    .strip_suffix("*/")
                    .unwrap_or(trimmed)
                    .trim();
                format!("{}{}", line.chars().take(indent).collect::<String>(), inner)
            } else if body.is_empty() {
                return;
            } else {
                format!("{}/* {} */", line.chars().take(indent).collect::<String>(), body)
            };
            *line = rebuilt;
        } else {
            // Wrap the selected span in place: an opener at the start of the first
            // selected line and a closer at the end of the last one.
            let first = start.line.min(lines.len().saturating_sub(1));
            let head_line = last;
            if let Some(line) = lines.get_mut(first) {
                let column = start.column.min(line.chars().count());
                let byte =
                    line.char_indices().nth(column).map(|(byte, _)| byte).unwrap_or(line.len());
                line.insert_str(byte, "/* ");
            }
            if let Some(line) = lines.get_mut(head_line) {
                let column = end.column.min(line.chars().count());
                let byte =
                    line.char_indices().nth(column).map(|(byte, _)| byte).unwrap_or(line.len());
                line.insert_str(byte, " */");
            }
        }
        let after = join_lines(&lines);
        if after == before {
            return;
        }
        let head = self.cursor.head;
        self.commit_edit(before, after, head);
    }

    /// Inserts `snippet`, resolving `$0`…`$9` placeholders.
    ///
    /// The markers are removed and the caret lands on `$0` (or on the first marker
    /// when there is no `$0`). It is the minimal snippet contract: enough for an
    /// editor to expand a template without a second document model.
    pub fn insert_snippet(&mut self, snippet: &str) {
        if !self.is_editable() || snippet.is_empty() {
            return;
        }
        let start = self.cursor.bounds().0;
        let mut resolved = String::with_capacity(snippet.len());
        let mut caret_offset: Option<usize> = None;
        let mut first_offset: Option<usize> = None;
        let mut chars = snippet.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '$' {
                if let Some(digit) = chars.peek().copied().filter(|ch| ch.is_ascii_digit()) {
                    chars.next();
                    let offset = resolved.chars().count();
                    if first_offset.is_none() {
                        first_offset = Some(offset);
                    }
                    if digit == '0' {
                        caret_offset = Some(offset);
                    }
                    continue;
                }
            }
            resolved.push(ch);
        }
        let marker = caret_offset.or(first_offset).unwrap_or(resolved.chars().count());
        self.insert(&resolved);
        let newlines = resolved
            [..resolved.char_indices().nth(marker).map(|(byte, _)| byte).unwrap_or(resolved.len())]
            .matches('\n')
            .count();
        let column_in_line =
            resolved.char_indices().nth(marker).map(|(byte, _)| byte).unwrap_or(resolved.len());
        let line_start = resolved[..column_in_line].rfind('\n').map(|index| index + 1).unwrap_or(0);
        let column = resolved[line_start..column_in_line].chars().count();
        let head = TextPosition::new(
            start.line + newlines,
            if newlines == 0 { start.column + column } else { column },
        );
        self.move_caret_to(head, false);
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

    /// Moves the lines the selection touches up (`direction < 0`) or down.
    ///
    /// A multi-line selection moves as a block, not as the caret's single line:
    /// moving a block is what `Alt+Up/Down` means once a range is selected.
    pub fn move_line(&mut self, direction: isize) {
        if !self.is_editable() || direction == 0 {
            return;
        }
        let (start, end) = self.cursor.bounds();
        let last = self.line_count().saturating_sub(1);
        let block_start = start.line;
        let block_end = end.line.min(last);
        let before = self.text();
        let mut lines = before.split('\n').map(|text| text.to_string()).collect::<Vec<_>>();
        if block_start >= lines.len() || block_end >= lines.len() {
            return;
        }
        let (new_start, new_end) = if direction < 0 {
            if block_start == 0 {
                return;
            }
            let above = lines.remove(block_start - 1);
            lines.insert(block_end, above);
            (block_start - 1, block_end - 1)
        } else {
            if block_end + 1 >= lines.len() {
                return;
            }
            let below = lines.remove(block_end + 1);
            lines.insert(block_start, below);
            (block_start + 1, block_end + 1)
        };
        let after = join_lines(&lines);
        let head_column = self.line_len(block_end).min(self.cursor.head.column);
        let anchor_column = self.line_len(block_start).min(self.cursor.anchor.column);
        let head = TextPosition::new(new_end, head_column);
        self.commit_edit(before, after, head);
        self.cursor.anchor = TextPosition::new(new_start, anchor_column);
        self.cursor.head = TextPosition::new(new_end, self.line_len(new_end).min(head_column));
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

    pub(crate) fn char_at(&self, position: TextPosition) -> Option<char> {
        self.model.borrow().line(position.line).chars().nth(position.column)
    }

    // ── History ─────────────────────────────────────────────────────────────

    /// Undoes the most recent edit. Returns `false` when there is nothing to do.
    pub fn undo(&mut self) -> bool {
        if !self.is_editable() || !self.config.undo_redo_enabled {
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
        if !self.is_editable() || !self.config.undo_redo_enabled {
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
        self.active_typing = None;
        let restored = self.model.borrow().text.borrow().clone();
        self.restoring_history = true;
        self.model.borrow_mut().set_text(restored.clone());
        self.restoring_history = false;
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        self.completion_index = None;
        self.scale = DocumentScale::for_lines(self.line_count());
        self.clamp_cursor_to_document();
        self.refresh_derived_state();
        self.text_changed.emit(restored);
        self.text_change.emit(TextChange::document(self.line_count()));
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }
}

/// Returns the caret position after writing `insert` at `start`.
///
/// `inside` counts characters from the beginning of the inserted text (the position a
/// newline command wants inside its auto-indent), defaulting to the end. Newlines in the
/// payload advance the line and reset the column, which is what makes a multi-line
/// insertion land its caret on the last line it wrote.
fn caret_inside_insert(start: TextPosition, insert: &str, inside: Option<usize>) -> TextPosition {
    let total = insert.chars().count();
    let inside = inside.unwrap_or(total).min(total);
    let mut line = start.line;
    let mut column = start.column;
    for (index, ch) in insert.chars().enumerate() {
        if index >= inside {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    TextPosition::new(line, column)
}
