// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scrolling, viewport linkage and caret visibility.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Viewport ────────────────────────────────────────────────────────────

    /// Returns the first visible document line.
    pub fn scroll_line(&self) -> usize {
        self.scroll_line
    }

    /// Returns the horizontal scroll offset in character columns.
    ///
    /// The companion to [`Self::scroll_line`] for a host that links two panes and
    /// needs to reproduce a viewport exactly rather than approximately.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
    }

    /// Returns the number of visible rows for the current geometry.
    pub fn visible_rows(&self) -> usize {
        if self.visible_rows == 0 {
            return self.compute_visible_rows();
        }
        self.visible_rows
    }

    /// Scrolls by a number of lines, clamped to the document.
    pub fn scroll_by(&mut self, delta_lines: isize) {
        let rows = self.visible_rows();
        let max_first = self.line_count().saturating_sub(rows);
        let target = (self.scroll_line as isize + delta_lines).clamp(0, max_first as isize);
        if target as usize != self.scroll_line {
            self.scroll_line = target as usize;
            self.scroll_visual_row = self.line_to_visual_row(self.scroll_line);
            self.base.request_redraw();
        }
    }

    /// Scrolls horizontally by a number of columns.
    pub fn scroll_columns_by(&mut self, delta_columns: isize) {
        // Wrapping means there is no off-screen column: the rows already cover the
        // full line, so a horizontal offset would shift every row against its wrap
        // points.
        if self.config.word_wrap {
            return;
        }
        let target = (self.scroll_column as isize + delta_columns).max(0);
        if target as usize != self.scroll_column {
            self.scroll_column = target as usize;
            self.base.request_redraw();
        }
    }

    /// Scrolls to a fraction of the document, `0.0` = top, `1.0` = bottom.
    ///
    /// This is what the minimap's click target uses: the strip shows the whole file,
    /// so a click maps to a position in it. Scrolling is expressed in visual rows, and
    /// `scroll_line` is derived from the target row, so wrapped and folded documents
    /// land on a real row rather than on a line number the viewport cannot show.
    pub fn scroll_to_fraction(&mut self, fraction: f32) {
        let rows = self.visible_rows().max(1);
        let max_first = self.total_visual_rows().saturating_sub(rows);
        let target = (fraction.clamp(0.0, 1.0) * max_first as f32).round() as usize;
        let target = target.min(max_first);
        let line = self.visual_row_to_line(target);
        let row = self.line_to_visual_row(line);
        if row != self.scroll_visual_row || line != self.scroll_line {
            self.scroll_visual_row = row;
            self.scroll_line = line;
            self.base.request_redraw();
        }
    }

    // ── Multi-instance linkage (D3) ─────────────────────────────────────────

    /// Captures where this editor is looking, for a host that links two panes.
    ///
    /// A side-by-side comparison needs both instances to agree on the viewport;
    /// the *policy* (follow the caret? follow the scroll only? lock the columns?)
    /// belongs to the host, which is why only the capture and the apply live here.
    pub fn viewport_snapshot(&self) -> ViewportSnapshot {
        ViewportSnapshot {
            first_visual_row: self.scroll_visual_row,
            scroll_column: self.scroll_column,
            caret: self.cursor.head,
        }
    }

    /// Applies a viewport snapshot from another instance.
    ///
    /// Scrolls and moves the caret without touching undo history or the modified
    /// flag: following a *linked* pane is not an edit of this buffer. The caret is
    /// clamped to this document, so linking a long file to a short one cannot put
    /// the caret past the end.
    pub fn apply_viewport(&mut self, snapshot: ViewportSnapshot) {
        self.scroll_column = if self.config.word_wrap { 0 } else { snapshot.scroll_column };
        // The row is clamped through the same transform a scroll uses, so a linked
        // pane whose `scroll_line` lags the row cannot desynchronise the two.
        let visible = self.visible_rows().max(1);
        let max_row = self.visual_row_count().saturating_sub(visible);
        self.scroll_visual_row = snapshot.first_visual_row.min(max_row);
        self.scroll_line = self.visual_row_to_line(self.scroll_visual_row);
        let caret = self.clamp_position(snapshot.caret);
        let moved = caret != self.cursor.head;
        self.cursor = Cursor { head: caret, anchor: caret };
        self.goal.active = false;
        if moved {
            self.emit_cursor_and_selection();
        }
        self.base.request_redraw();
    }

    /// Returns the first visible visual row, the linked-pane coordinate.
    ///
    /// Distinct from [`Self::scroll_line`], which is a *document line*: with
    /// wrapping or folding the two differ, and a host linking panes must use the
    /// row or the panes drift apart by exactly the fold they disagree about.
    pub fn first_visual_row(&self) -> usize {
        self.scroll_visual_row
    }

    /// Keeps the caret inside the viewport, scrolling only when necessary.
    pub(super) fn ensure_caret_visible(&mut self) {
        self.refresh_visible_rows();
        let rows = self.visible_rows();
        // The caret's *segment* row, not the line's first row: a caret on the fourth
        // wrapped row was scrolled to as though it were on the first, so it could be
        // off screen while the viewport claimed to show it.
        let visual = self.caret_visual_row();
        if visual < self.scroll_visual_row {
            self.scroll_visual_row = visual;
            self.scroll_line = self.visual_row_to_line(self.scroll_visual_row);
        } else if visual >= self.scroll_visual_row + rows {
            self.scroll_visual_row = visual.saturating_sub(rows.saturating_sub(1));
            self.scroll_line = self.visual_row_to_line(self.scroll_visual_row);
        }
        // With wrapping on, every character is already visible on some row, so there
        // is nothing to scroll horizontally — and scrolling would shift every row by
        // a column count the wrap points did not account for.
        if self.config.word_wrap {
            self.scroll_column = 0;
        } else {
            let columns = self.text_columns();
            if self.cursor.head.column < self.scroll_column {
                self.scroll_column = self.cursor.head.column;
            } else if self.cursor.head.column >= self.scroll_column + columns {
                self.scroll_column = self.cursor.head.column + 1 - columns;
            }
        }
        let max_first = self.line_count().saturating_sub(rows);
        self.scroll_line = self.scroll_line.min(max_first);
    }

    pub(super) fn clamp_cursor_to_document(&mut self) {
        self.cursor = Cursor {
            head: self.clamp_position(self.cursor.head),
            anchor: self.clamp_position(self.cursor.anchor),
        };
    }

    pub(crate) fn clamp_position(&self, position: TextPosition) -> TextPosition {
        self.model.borrow().clamp_position(position)
    }

    pub(super) fn emit_cursor_and_selection(&self) {
        self.cursor_moved.emit((self.cursor.head.line, self.cursor.head.column));
        if self.cursor.is_collapsed() {
            self.selection_changed.emit(None);
        } else {
            let (start, end) = self.cursor.bounds();
            self.selection_changed.emit(Some(Rect::new(
                start.column as i32,
                start.line as i32,
                (end.column.saturating_sub(start.column)).max(1) as u32,
                (end.line.saturating_sub(start.line) + 1) as u32,
            )));
        }
    }

    /// Recomputes fold hints, match cache, row budget and the cursor clamp.
    pub(super) fn refresh_derived_state(&mut self) {
        self.refresh_derived_state_after(None);
    }

    /// [`Self::refresh_derived_state`] with a hint about what the edit changed.
    ///
    /// `dirty` describes the edited line range so the match cache can rescan only
    /// those lines instead of the whole document. Callers that changed the shape
    /// of the document in a way they cannot describe pass `None`, which is always
    /// correct and merely slower.
    pub(super) fn refresh_derived_state_after(&mut self, dirty: Option<(usize, usize, usize)>) {
        {
            let model = self.model.borrow();
            self.folded_lines.clear();
            for region in &model.folds {
                if region.folded {
                    self.folded_lines.push(region.start_line);
                }
            }
        }
        // The document may have gained or lost lines since the last refresh, so
        // the per-line lexer cache is reconciled here rather than in `draw`.
        let count = self.line_count();
        if self.line_states.len() != count + 1 {
            self.line_states.resize(count + 1, LineState::Clean);
            self.line_tokens.resize(count, None);
        }
        self.clamp_cursor_to_visible();
        self.recompute_match_cache_incremental(dirty);
        self.refresh_visible_rows();
        // The document shape changed (lines, folds, or the wrap geometry the callers
        // above may have moved), so the row-prefix table is rebuilt here rather than
        // in the lookup, which only has `&self`.
        self.rebuild_visual_row_prefix();
    }
}
