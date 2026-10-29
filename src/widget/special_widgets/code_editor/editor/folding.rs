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
        match self.fold_at(start) {
            Some(index) => {
                if let Some(region) = self.folds.get_mut(index) {
                    region.end_line = end;
                    region.folded = true;
                }
            }
            None => self.folds.push(FoldRegion::new(start, end, true)),
        }
        // `is_hidden` binary-searches the fold list, so its sort order is an invariant of
        // the view rather than something the callers happen to preserve. Folding line 10
        // and then line 3 would otherwise leave the list unsorted.
        self.normalize_folds();
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
        let changed = match self.fold_at(start_line).and_then(|index| self.folds.get_mut(index)) {
            Some(region) => {
                region.folded = false;
                true
            }
            None => false,
        };
        if changed {
            self.after_fold_change();
        }
        changed
    }

    /// Toggles the fold state of `start_line`, inferring the block when needed.
    pub fn toggle_fold(&mut self, start_line: usize) -> bool {
        let state = self
            .folds
            .iter()
            .find(|region| region.start_line == start_line)
            .map(|region| region.folded);
        match state {
            Some(true) => self.unfold(start_line),
            Some(false) => {
                let changed =
                    match self.fold_at(start_line).and_then(|index| self.folds.get_mut(index)) {
                        Some(region) => {
                            region.folded = true;
                            true
                        }
                        None => false,
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
        let mut changed = false;
        for region in &mut self.folds {
            if region.folded {
                region.folded = false;
                changed = true;
            }
        }
        if changed {
            self.after_fold_change();
        }
    }

    /// Returns the fold regions of the active buffer.
    pub fn fold_regions(&self) -> Vec<FoldRegion> {
        self.folds.clone()
    }

    /// Returns how many lines are hidden by folds.
    pub fn folded_line_count(&self) -> usize {
        self.folds.iter().map(FoldRegion::hidden_lines).sum()
    }

    /// Returns `true` when `line` starts a currently folded region.
    ///
    /// # Why this reads the view's own fold list, not the cache
    ///
    /// This used to scan `folded_lines`, a *derived* cache rebuilt by
    /// `refresh_derived_state`, while [`CodeEditor::folded_line_count`] and the
    /// render path's row visibility both answered from the fold list. Two sources for
    /// one fact is how they drift: the cache is only as fresh as the last refresh,
    /// so a state change that had not yet triggered one could be reported
    /// differently by this predicate than by the count. The fold list is the single
    /// authority; the cache is a render-path detail and no longer decides public
    /// answers.
    pub fn is_line_folded(&self, line: usize) -> bool {
        self.folds.iter().any(|region| region.folded && region.start_line == line)
    }

    /// Returns whether a line is hidden *inside* a folded region.
    ///
    /// Distinct from [`Self::is_line_folded`], which reports the fold's own start
    /// line — the one row a collapsed region still shows. This answers the other
    /// half: is the line currently invisible because some region swallowed it.
    pub fn is_line_hidden(&self, line: usize) -> bool {
        self.is_hidden(line)
    }

    /// Returns how many lines the fold starting at `line` hides, or 0.
    pub fn hidden_lines_at(&self, line: usize) -> usize {
        self.folds
            .iter()
            .find(|region| region.start_line == line && region.folded)
            .map(FoldRegion::hidden_lines)
            .unwrap_or(0)
    }

    pub(super) fn after_fold_change(&mut self) {
        // Every fold mutation lands here. `refresh_derived_state` re-derives the merged
        // visibility ranges and the row projection from the fold list, so the one call
        // below is the single place that has to remember both (J-P04).
        if self.config.animate_folds {
            self.fold_reveal = 0.0;
        }
        self.refresh_derived_state();
        self.fold_changed.emit(self.folded_line_count());
        self.base.request_layout();
        self.base.request_redraw();
    }

    pub(super) fn clamp_cursor_to_visible(&mut self) {
        let hidden_region = if self.is_hidden(self.cursor.head.line) {
            self.folds
                .iter()
                .find(|region| region.folded && self.cursor.head.line <= region.end_line)
                .map(|region| region.start_line)
        } else {
            None
        };
        if let Some(start) = hidden_region {
            let position = TextPosition::new(start, self.line_len(start));
            self.cursor = Cursor { head: position, anchor: position };
        }
    }

    // ── Fold storage ────────────────────────────────────────────────────────

    /// Returns whether a line is hidden by a folded region.
    ///
    /// Two binary searches over the merged fold ranges: the last range starting at or
    /// before `line`, then a containment test. The old form scanned every fold for every
    /// line, which made scrolling a long folded file `O(lines x folds)` (J-P04).
    pub(crate) fn is_hidden(&self, line: usize) -> bool {
        let index = self.hidden_ranges.partition_point(|(start, _)| *start <= line);
        index > 0 && self.hidden_ranges[index - 1].1 >= line
    }

    /// Returns the index of the fold region starting at `start_line`.
    pub(crate) fn fold_at(&self, start_line: usize) -> Option<usize> {
        self.folds.iter().position(|region| region.start_line == start_line)
    }

    /// Drops fold regions that no longer span more than one line after an edit.
    ///
    /// The list is also put in `start_line` order here. [`Self::is_hidden`] binary-searches
    /// it, and fold commands append regions in document order anyway — sorting is what keeps
    /// that an invariant of the type rather than a property the callers happen to have.
    pub(crate) fn normalize_folds(&mut self) {
        let last = self.line_count().saturating_sub(1);
        for region in &mut self.folds {
            region.start_line = region.start_line.min(last);
            region.end_line = region.end_line.clamp(region.start_line, last);
        }
        self.folds.retain(|region| region.end_line > region.start_line);
        self.folds.sort_by_key(|region| region.start_line);
        // Rebuild the merged view the visibility query reads. Regions are sorted by start,
        // so a single pass unions them: overlapping and adjacent ranges collapse into one
        // entry, which is what makes a nested fold queryable in `O(log folds)`.
        self.hidden_ranges.clear();
        for region in &self.folds {
            if !region.folded || region.end_line <= region.start_line {
                continue;
            }
            // `start_line` itself stays visible (it carries the fold marker).
            let start = region.start_line + 1;
            match self.hidden_ranges.last_mut() {
                Some((_, last_end)) if start <= *last_end + 1 => {
                    *last_end = (*last_end).max(region.end_line);
                }
                _ => self.hidden_ranges.push((start, region.end_line)),
            }
        }
    }

    /// Replaces the fold set and refreshes the derived visibility ranges.
    #[cfg(test)]
    pub(crate) fn set_folds(&mut self, folds: Vec<FoldRegion>) {
        self.folds = folds;
        self.normalize_folds();
    }

    /// Returns the indices of all lines that are not hidden by a fold.
    pub(crate) fn visible_lines(&self) -> Vec<usize> {
        (0..self.line_count()).filter(|line| !self.is_hidden(*line)).collect()
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
        let revision = self.revision.get();
        let stale = self.bracket_index.as_ref().is_none_or(|(cached, _)| *cached != revision);
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
            self.bracket_index = Some((revision, matched));
        }
        self.bracket_index.as_ref().map(|(_, pairs)| pairs.as_slice()).unwrap_or(&[])
    }

    /// Number of matched bracket pairs in the cache, for tests.
    #[cfg(test)]
    pub(crate) fn cached_bracket_pairs(&self) -> usize {
        self.bracket_index.as_ref().map(|(_, pairs)| pairs.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> CodeEditor {
        CodeEditor::new(Rect::new(0, 0, 800, 600))
    }

    /// A fold hides only its interior; shrinking the document clamps it away.
    ///
    /// These two cases moved here with the fold list itself: the state is now the
    /// view's, so the assertions are made through the widget rather than the model.
    #[test]
    fn folds_hide_only_interior_lines_and_normalize_after_edits() {
        let mut editor = editor();
        editor.set_text("a\nb\nc\nd");
        editor.set_folds(vec![FoldRegion::new(1, 2, true)]);
        assert!(!editor.is_hidden(0));
        assert!(!editor.is_hidden(1), "the fold header stays visible");
        assert!(editor.is_hidden(2));
        assert!(!editor.is_hidden(3));
        assert_eq!(editor.folded_line_count(), 1);
        assert_eq!(editor.visible_lines(), vec![0, 1, 3]);

        // Shrinking the document must clamp the region, not panic.
        editor.set_text("only");
        assert!(editor.fold_regions().is_empty());
    }

    /// A fold inside a fold stays hidden even when a later region ends earlier.
    #[test]
    fn nested_folds_hide_their_interiors() {
        let mut editor = editor();
        editor.set_text("a\nb\nc\nd\ne\nf");
        editor.set_folds(vec![FoldRegion::new(0, 4, true), FoldRegion::new(2, 3, true)]);
        assert!(!editor.is_hidden(0), "the outer header stays visible");
        assert!(editor.is_hidden(1));
        assert!(editor.is_hidden(2));
        assert!(editor.is_hidden(4));
        assert!(!editor.is_hidden(5));
        assert_eq!(editor.visible_lines(), vec![0, 5]);
    }
}
