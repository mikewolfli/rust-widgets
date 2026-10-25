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
            // `is_hidden` binary-searches the fold list, so its sort order is an invariant of
            // the model rather than something the callers happen to preserve. Folding line 10
            // and then line 3 would otherwise leave the list unsorted.
            model.normalize_folds();
        }
        self.after_fold_change();
    }

    /// Folds the innermost bracket region enclosing `line`.
    pub fn fold_block_at(&mut self, line: usize) -> bool {
        let Some((start, end)) = self.enclosing_bracket_pair(line) else {
            return false;
        };
        self.fold(start, end);
        true
    }

    /// Folds every multi-line bracket block in the document.
    ///
    /// One pass over the cached bracket-pair index, so the cost is proportional to
    /// the number of blocks rather than one backwards scan per line.
    pub fn fold_all(&mut self) -> usize {
        let pairs = self.bracket_pairs().to_vec();
        let mut folded = 0usize;
        for (open, close) in pairs {
            if close.line > open.line {
                self.fold(open.line, close.line);
                folded += 1;
            }
        }
        if folded > 0 {
            self.after_fold_change();
        }
        folded
    }

    /// Folds the blocks whose opening line is indented at `level` indent units.
    ///
    /// A level is measured in the configured tab width, so "level 1" means the
    /// direct children of the top-level block. This is the hierarchy control the
    /// audit asked for; `fold_all` is the same pass without the depth filter.
    pub fn fold_level(&mut self, level: usize) -> usize {
        let width = self.config.tab_width.max(1);
        let target = level.saturating_mul(width);
        let pairs = self.bracket_pairs().to_vec();
        let mut folded = 0usize;
        for (open, close) in pairs {
            if close.line <= open.line {
                continue;
            }
            let indent = self.line_indent(open.line).chars().count();
            if indent == target {
                self.fold(open.line, close.line);
                folded += 1;
            }
        }
        if folded > 0 {
            self.after_fold_change();
        }
        folded
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

    /// Returns how many lines the fold starting at `line` hides, or 0.
    pub fn hidden_lines_at(&self, line: usize) -> usize {
        self.model
            .borrow()
            .folds
            .iter()
            .find(|region| region.start_line == line && region.folded)
            .map(FoldRegion::hidden_lines)
            .unwrap_or(0)
    }

    pub(super) fn after_fold_change(&mut self) {
        // Every fold mutation lands here, so the model's merged visibility ranges are
        // refreshed in one place rather than at each of the five call sites (J-P04).
        self.model.borrow_mut().normalize_folds();
        if self.config.animate_folds {
            self.fold_reveal = 0.0;
        }
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

    /// Finds the line range of the innermost block enclosing `line`.
    ///
    /// Answered from the cached pair index. The previous implementation scanned
    /// upwards to the start of the document and then downwards to its end on every
    /// call, which made folding a block in a large file an O(file) operation per
    /// fold.
    fn enclosing_bracket_pair(&mut self, line: usize) -> Option<(usize, usize)> {
        let probe = line.min(self.line_count().saturating_sub(1));
        let pairs = self.bracket_pairs();
        for (open, close) in pairs.iter().rev() {
            // An opener later on the same line still encloses the line: the
            // question is "which block does this line belong to", not "which
            // block starts before the line's first column".
            if open.line <= probe && close.line >= probe && close.line > open.line {
                return Some((open.line, close.line));
            }
        }
        None
    }

    /// Returns the cached bracket-pair index, rebuilding it when the text moved.
    ///
    /// Pairs are matched with a stack over the whole document once per revision;
    /// every fold query afterwards is a binary search or a short backwards scan.
    fn bracket_pairs(&mut self) -> &[(TextPosition, TextPosition)] {
        let stale =
            self.bracket_index.as_ref().is_none_or(|(revision, _)| *revision != self.revision);
        if stale {
            let mut stack: Vec<(TextPosition, char)> = Vec::new();
            let mut matched: Vec<(TextPosition, TextPosition)> = Vec::new();
            for line in 0..self.line_count() {
                let text = self.line_text(line).unwrap_or_default();
                for (column, ch) in text.chars().enumerate() {
                    if !matches!(ch, '(' | '[' | '{' | ')' | ']' | '}') {
                        continue;
                    }
                    let position = TextPosition::new(line, column);
                    if matches!(ch, '(' | '[' | '{') {
                        stack.push((position, ch));
                    } else if let Some(index) =
                        stack.iter().rposition(|(_, open)| pairs::closing_for(*open) == Some(ch))
                    {
                        let (open, _) = stack.remove(index);
                        matched.push((open, position));
                    }
                }
            }
            matched.sort_by_key(|(open, close)| (*open, *close));
            self.bracket_index = Some((self.revision, matched));
        }
        self.bracket_index.as_ref().map(|(_, pairs)| pairs.as_slice()).unwrap_or(&[])
    }

    /// Number of matched bracket pairs in the cache, for tests.
    #[cfg(test)]
    pub(crate) fn cached_bracket_pairs(&self) -> usize {
        self.bracket_index.as_ref().map(|(_, pairs)| pairs.len()).unwrap_or(0)
    }
}
