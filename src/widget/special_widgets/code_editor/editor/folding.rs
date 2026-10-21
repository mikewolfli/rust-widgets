// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Fold regions and the visible-row projection.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Folding ─────────────────────────────────────────────────────────────

    /// Folds `start_line..=end_line` into a single visible row.
    pub fn fold(&mut self, start_line: usize, end_line: usize) {
        let last = self.line_count().saturating_sub(1);
        let start = start_line.min(last);
        let end = end_line.max(start + 1).min(last);
        if end <= start {
            return;
        }
        {
            let mut model = self.model.borrow_mut();
            match model.fold_at(start) {
                Some(index) => {
                    if let Some(region) = model.folds.get_mut(index) {
                        region.end_line = end;
                        region.folded = true;
                    }
                }
                None => model.folds.push(FoldRegion::new(start, end, true)),
            }
        }
        self.after_fold_change();
    }

    /// Folds the innermost bracket region enclosing `line`.
    pub fn fold_block_at(&mut self, line: usize) -> bool {
        let start = match self.enclosing_bracket_start(line) {
            Some(start) => start,
            None => return false,
        };
        let end = match self.matching_bracket_line(start) {
            Some(end) => end,
            None => return false,
        };
        if end <= start {
            return false;
        }
        self.fold(start, end);
        true
    }

    /// Expands the region starting at `start_line`.
    pub fn unfold(&mut self, start_line: usize) -> bool {
        let changed = {
            let mut model = self.model.borrow_mut();
            match model.fold_at(start_line).and_then(|index| model.folds.get_mut(index)) {
                Some(region) => {
                    region.folded = false;
                    true
                }
                None => false,
            }
        };
        if changed {
            self.after_fold_change();
        }
        changed
    }

    /// Toggles the fold state of `start_line`, inferring the block when needed.
    pub fn toggle_fold(&mut self, start_line: usize) -> bool {
        let state = self
            .model
            .borrow()
            .folds
            .iter()
            .find(|region| region.start_line == start_line)
            .map(|region| region.folded);
        match state {
            Some(true) => self.unfold(start_line),
            Some(false) => {
                let changed = {
                    let mut model = self.model.borrow_mut();
                    match model.fold_at(start_line).and_then(|index| model.folds.get_mut(index)) {
                        Some(region) => {
                            region.folded = true;
                            true
                        }
                        None => false,
                    }
                };
                if changed {
                    self.after_fold_change();
                }
                changed
            }
            None => self.fold_block_at(start_line),
        }
    }

    /// Expands every fold region.
    pub fn unfold_all(&mut self) {
        let changed = {
            let mut model = self.model.borrow_mut();
            let mut changed = false;
            for region in &mut model.folds {
                if region.folded {
                    region.folded = false;
                    changed = true;
                }
            }
            changed
        };
        if changed {
            self.after_fold_change();
        }
    }

    /// Returns the fold regions of the active buffer.
    pub fn fold_regions(&self) -> Vec<FoldRegion> {
        self.model.borrow().folds.clone()
    }

    /// Returns how many lines are hidden by folds.
    pub fn folded_line_count(&self) -> usize {
        self.model.borrow().folded_line_count()
    }

    /// Returns `true` when `line` starts a currently folded region.
    ///
    /// # Why this reads the model, not the cache
    ///
    /// This used to scan `folded_lines`, a *derived* cache rebuilt by
    /// `refresh_derived_state`, while [`CodeEditor::folded_line_count`] and the
    /// render path's row visibility both answered from the model. Two sources for
    /// one fact is how they drift: the cache is only as fresh as the last refresh,
    /// so a state change that had not yet triggered one could be reported
    /// differently by this predicate than by the count. The model is the single
    /// authority; the cache is a render-path detail and no longer decides public
    /// answers.
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.model.borrow().folds.iter().any(|region| region.folded && region.start_line == line)
    }

    /// Returns whether a line is hidden *inside* a folded region.
    ///
    /// Distinct from [`Self::is_line_folded`], which reports the fold's own start
    /// line — the one row a collapsed region still shows. This answers the other
    /// half: is the line currently invisible because some region swallowed it.
    pub fn is_line_hidden(&self, line: usize) -> bool {
        self.model.borrow().is_hidden(line)
    }

    pub(super) fn after_fold_change(&mut self) {
        self.refresh_derived_state();
        self.fold_changed.emit(self.folded_line_count());
        self.base.request_layout();
        self.base.request_redraw();
    }

    pub(super) fn clamp_cursor_to_visible(&mut self) {
        let hidden_region = {
            let model = self.model.borrow();
            if model.is_hidden(self.cursor.head.line) {
                model
                    .folds
                    .iter()
                    .find(|region| region.folded && self.cursor.head.line <= region.end_line)
                    .map(|region| region.start_line)
            } else {
                None
            }
        };
        if let Some(start) = hidden_region {
            let position = TextPosition::new(start, self.line_len(start));
            self.cursor = Cursor { head: position, anchor: position };
        }
    }

    /// Finds the line containing the bracket that encloses `line`.
    pub(super) fn enclosing_bracket_start(&self, line: usize) -> Option<usize> {
        let model = self.model.borrow();
        let line = line.min(model.lines.len().saturating_sub(1));
        let mut depth = 0isize;
        for index in (0..=line).rev() {
            for ch in model.line(index).chars().rev() {
                match ch {
                    '}' | ')' | ']' => depth += 1,
                    '{' | '(' | '[' => {
                        depth -= 1;
                        if depth < 0 {
                            return Some(index);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Returns the line of the bracket closing the opener at `start`.
    pub(super) fn matching_bracket_line(&self, start: usize) -> Option<usize> {
        let mut depth = 0isize;
        for line in start..self.line_count() {
            let length = self.line_len(line);
            for column in 0..length {
                match self.char_at(TextPosition::new(line, column)) {
                    Some('{') | Some('(') | Some('[') => depth += 1,
                    Some('}') | Some(')') | Some(']') => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(line);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
}
