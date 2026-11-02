// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-owned overlays and document-coordinate lifecycles.
//!
//! Three facts live here because they share one property — the editor cannot
//! derive them from the text:
//!
//! * inlay hints and code lenses are language intelligence a host pushes in;
//! * bookmarks and breakpoints are user marks expressed in line coordinates, so
//!   an edit above them must move them rather than clear them;
//! * the inline-diff baseline is the *other* document version, which only the
//!   host holds.
//!
//! The line/position translation is shared so every anchor moves by the same
//! rule after an edit.

use super::*;

impl CodeEditor {
    // ── Inlay hints ─────────────────────────────────────────────────────────

    /// The host-supplied inline annotations.
    pub fn inlay_hints(&self) -> &[InlayHint] {
        &self.inlay_hints
    }

    /// Replaces the inline annotations, sorted by `(line, column)`.
    ///
    /// The editor draws them at the position they name and never moves them: an
    /// edit changes the text, and the host re-pushes hints for the new text.
    pub fn set_inlay_hints(&mut self, mut hints: Vec<InlayHint>) {
        hints.sort_by_key(|hint| (hint.line, hint.column));
        if self.inlay_hints == hints {
            return;
        }
        self.inlay_hints = hints;
        self.base.request_redraw();
    }

    /// Whether the paint pass draws inlay hints.
    pub fn inlay_hints_enabled(&self) -> bool {
        self.config.show_inlay_hints
    }

    /// Enables or disables inlay-hint painting without discarding the hints.
    pub fn set_inlay_hints_enabled(&mut self, enabled: bool) {
        if self.config.show_inlay_hints == enabled {
            return;
        }
        self.config.show_inlay_hints = enabled;
        self.base.request_redraw();
    }

    // ── Code lenses ─────────────────────────────────────────────────────────

    /// The host-supplied line affordances.
    pub fn code_lenses(&self) -> &[CodeLens] {
        &self.code_lenses
    }

    /// Replaces the line affordances, sorted by line.
    ///
    /// The widget paints the labels; activating one is the host's command, which
    /// is why [`CodeLens::command`] carries the id rather than a callback.
    pub fn set_code_lenses(&mut self, mut lenses: Vec<CodeLens>) {
        lenses.sort_by_key(|lens| lens.line);
        if self.code_lenses == lenses {
            return;
        }
        self.code_lenses = lenses;
        self.base.request_redraw();
    }

    // ── Bookmarks ───────────────────────────────────────────────────────────

    /// Replaces the bookmarked lines, clamping to the document.
    ///
    /// Out-of-range lines are clamped rather than refused: a host restoring a
    /// session file should not fail because the file was edited shorter since.
    /// The result is sorted and deduplicated, which is the order the getter and
    /// the gutter already promise.
    pub fn set_bookmarks(&mut self, lines: Vec<usize>) {
        let last = self.line_count().saturating_sub(1);
        let mut lines: Vec<usize> = lines.into_iter().map(|line| line.min(last)).collect();
        lines.sort_unstable();
        lines.dedup();
        if self.bookmarks == lines {
            return;
        }
        self.bookmarks = lines;
        self.base.request_redraw();
    }

    /// Removes every bookmark.
    pub fn clear_bookmarks(&mut self) {
        if self.bookmarks.is_empty() {
            return;
        }
        self.bookmarks.clear();
        self.base.request_redraw();
    }

    /// Moves the caret to the previous bookmark, wrapping to the last.
    ///
    /// The mirror of [`Self::goto_next_bookmark`]; together they make the two
    /// directions one symmetric command pair.
    pub fn previous_bookmark(&mut self) -> bool {
        let Some(target) = self
            .bookmarks
            .iter()
            .rev()
            .find(|line| **line < self.cursor.head.line)
            .or_else(|| self.bookmarks.last())
            .copied()
        else {
            return false;
        };
        self.move_caret_to(TextPosition::new(target, 0), false);
        true
    }

    // ── Inline diff ─────────────────────────────────────────────────────────

    /// Sets the document version changed lines are compared against.
    ///
    /// `None` disables the comparison even when
    /// [`CodeEditorConfig::show_inline_diff`] is on. The text is stored as given;
    /// the editor never normalises it, because the host's baseline is whatever
    /// the other side of the diff produced.
    pub fn set_diff_baseline(&mut self, baseline: Option<&str>) {
        let baseline = baseline.map(|text| text.to_string());
        if self.diff_baseline == baseline {
            return;
        }
        self.diff_baseline_lines = baseline
            .as_deref()
            .map(|text| text.split('\n').map(|line| line.to_string()).collect())
            .unwrap_or_default();
        self.diff_baseline = baseline;
        self.base.request_redraw();
    }

    /// The configured diff baseline, if any.
    pub fn diff_baseline(&self) -> Option<&str> {
        self.diff_baseline.as_deref()
    }

    /// Whether the inline diff marker is enabled.
    pub fn show_inline_diff(&self) -> bool {
        self.config.show_inline_diff
    }

    /// Enables or disables the inline diff marker.
    pub fn set_show_inline_diff(&mut self, show: bool) {
        if self.config.show_inline_diff == show {
            return;
        }
        self.config.show_inline_diff = show;
        self.base.request_redraw();
    }

    /// The zero-based lines whose text differs from the baseline.
    ///
    /// A line past the end of the baseline is a change (it was added). This is
    /// the same predicate the paint pass uses for its tint, exposed so a host can
    /// build a companion UI or test the comparison without rendering.
    pub fn inline_diff_lines(&self) -> Vec<usize> {
        if self.diff_baseline.is_none() {
            return Vec::new();
        }
        (0..self.line_count())
            .filter(|line| {
                let current = self.line_text(*line).unwrap_or_default();
                self.diff_baseline_lines.get(*line).is_none_or(|base| *base != current)
            })
            .collect()
    }

    /// Whether one line differs from the baseline.
    pub(crate) fn line_differs_from_baseline(&self, line: usize) -> bool {
        if self.diff_baseline.is_none() {
            return false;
        }
        let current = self.line_text(line).unwrap_or_default();
        self.diff_baseline_lines.get(line).is_none_or(|base| *base != current)
    }

    // ── Edit-time translation ───────────────────────────────────────────────

    /// Moves line/position anchors through a committed edit.
    ///
    /// Called from [`Self::record_and_settle_inner`], the single choke point every
    /// edit passes through, so a bookmark cannot be left behind by an edit path
    /// that forgot to call it. Line marks (bookmarks, breakpoints) move by the net
    /// line delta; snippet stops move by the full position delta. An edit that
    /// cannot describe itself ends the snippet session rather than guessing.
    pub(crate) fn migrate_document_anchors(
        &mut self,
        dirty: Option<(usize, usize, usize)>,
        edit: Option<(TextPosition, TextPosition, TextPosition)>,
    ) {
        if let Some((first, lines_now, lines_before)) = dirty {
            let delta = lines_now as isize - lines_before as isize;
            let last = self.line_count().saturating_sub(1);
            migrate_line_set(&mut self.bookmarks, first, lines_before, delta, last);
            migrate_line_set(&mut self.breakpoints, first, lines_before, delta, last);
        }
        self.translate_snippet_session(edit);
    }
}

/// Moves a sorted line set through an edit, clamping and re-sorting the result.
///
/// A mark above the edit is untouched, one below moves by `delta`, and one
/// *inside* the replaced region stays on the edit's first line: the line it
/// named was rewritten, but dropping the mark would silently lose a user's
/// bookmark for an edit made somewhere inside the same block.
fn migrate_line_set(
    lines: &mut Vec<usize>,
    first_line: usize,
    lines_before: usize,
    delta: isize,
    last_line: usize,
) {
    if lines.is_empty() {
        return;
    }
    let edit_end = first_line.saturating_add(lines_before);
    for line in lines.iter_mut() {
        if *line >= edit_end {
            *line = shift_index(*line, delta);
        } else if *line >= first_line {
            *line = first_line;
        }
        *line = (*line).min(last_line);
    }
    lines.sort_unstable();
    lines.dedup();
}
