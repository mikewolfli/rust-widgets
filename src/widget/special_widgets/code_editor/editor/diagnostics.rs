// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Diagnostic markers and inline diagnostics.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Diagnostics ─────────────────────────────────────────────────────────

    /// Replaces the diagnostic markers.
    pub fn set_markers(&mut self, markers: Vec<DiagnosticMarker>) {
        self.markers = markers;
        self.bump_layout_generation();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one diagnostic marker.
    pub fn add_marker(&mut self, marker: DiagnosticMarker) {
        self.markers.push(marker);
        self.bump_layout_generation();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every marker on `line`; returns how many were removed.
    pub fn clear_markers_on_line(&mut self, line: usize) -> usize {
        let before = self.markers.len();
        self.markers.retain(|marker| marker.line != line);
        let removed = before - self.markers.len();
        if removed > 0 {
            self.bump_layout_generation();
            self.base.request_layout();
            self.base.request_redraw();
        }
        removed
    }

    /// Returns the diagnostic markers.
    pub fn markers(&self) -> &[DiagnosticMarker] {
        &self.markers
    }

    /// Returns the number of markers of a given severity.
    pub fn marker_count(&self, severity: MarkerSeverity) -> usize {
        self.markers.iter().filter(|marker| marker.severity == severity).count()
    }

    /// Sorts markers by line, then severity, then column.
    pub fn sort_markers(&mut self) {
        self.markers.sort_by(|a, b| {
            a.line
                .cmp(&b.line)
                .then_with(|| a.severity.rank().cmp(&b.severity.rank()))
                .then_with(|| a.start_column().cmp(&b.start_column()))
        });
        self.bump_layout_generation();
        self.base.request_redraw();
    }

    /// Returns every marker paired with the visual row it belongs to, in paint order.
    ///
    /// Markers are **borrowed**, not cloned: `DiagnosticMarker` owns its message
    /// string, so the old `(row, marker.clone())` list allocated once per marker per
    /// call, and a frame calls this from both the painter and the row count. The
    /// word-wrap branch also re-summed `0..marker.line` for every marker
    /// (O(markers × line)); it now walks the document once and looks each marker's
    /// line up in the resulting table. Folding still needs the visible-line table, but
    /// a missing line (a marker inside a fold) is dropped from the paint list, as
    /// before.
    pub(crate) fn inline_diagnostics(&self) -> Vec<InlineDiagnostic<'_>> {
        if self.markers.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<InlineDiagnostic<'_>> = if self.folded_lines.is_empty() {
            if !self.config.word_wrap {
                // Nothing is hidden, so a marker's visual row is its line number when
                // wrapping is off. Building a line-to-row table here would walk the
                // whole document on every frame for no reason.
                self.markers.iter().map(|marker| (marker.line, marker)).collect()
            } else {
                // The line-to-row table is cached by the derived-layout generation:
                // rebuilding it was a full document pass on every frame while
                // wrapping was on, even when neither the text nor the markers moved.
                let generation = self.layout_generation;
                let cached = self.diagnostic_rows.borrow();
                if let Some((key, rows_by_line)) = cached.as_ref() {
                    if *key == generation {
                        return Self::markers_by_row(&self.markers, rows_by_line);
                    }
                }
                drop(cached);
                let columns = self.text_columns().max(1);
                let mut lines: Vec<usize> = self.markers.iter().map(|marker| marker.line).collect();
                lines.sort_unstable();
                lines.dedup();
                let mut rows_by_line: Vec<(usize, usize)> = Vec::with_capacity(lines.len());
                let mut row = 0usize;
                let mut previous = 0usize;
                for &line in &lines {
                    for skipped in previous..line {
                        row += self.line_len(skipped).max(1).div_ceil(columns);
                    }
                    previous = line;
                    rows_by_line.push((line, row));
                }
                #[cfg(test)]
                self.diagnostic_cache_builds.set(self.diagnostic_cache_builds.get() + 1);
                *self.diagnostic_rows.borrow_mut() = Some((generation, rows_by_line.clone()));
                Self::markers_by_row(&self.markers, &rows_by_line)
            }
        } else {
            // `visible_lines()` ascends, so the table is searchable in place.
            let mut rows_by_line: Vec<(usize, usize)> = Vec::new();
            let mut row = 0usize;
            for line in self.visible_document_lines() {
                rows_by_line.push((line, row));
                row += self.wrap_segments(line);
            }
            Self::markers_by_row(&self.markers, &rows_by_line)
        };
        result.sort_by_key(|(row, marker)| (*row, marker.severity.rank(), marker.start_column()));
        result
    }

    /// Maps markers onto the visual rows in `rows_by_line` by binary search.
    fn markers_by_row<'a>(
        markers: &'a [DiagnosticMarker],
        rows_by_line: &[(usize, usize)],
    ) -> Vec<InlineDiagnostic<'a>> {
        markers
            .iter()
            .filter_map(|marker| {
                rows_by_line
                    .binary_search_by_key(&marker.line, |(line, _)| *line)
                    .ok()
                    .map(|index| (rows_by_line[index].1, marker))
            })
            .collect()
    }
}
