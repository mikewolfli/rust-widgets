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
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one diagnostic marker.
    pub fn add_marker(&mut self, marker: DiagnosticMarker) {
        self.markers.push(marker);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every marker on `line`; returns how many were removed.
    pub fn clear_markers_on_line(&mut self, line: usize) -> usize {
        let before = self.markers.len();
        self.markers.retain(|marker| marker.line != line);
        let removed = before - self.markers.len();
        if removed > 0 {
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
        self.base.request_redraw();
    }

    /// Returns the top severity on `line`, where errors outrank warnings.
    pub(crate) fn highest_severity_on_line(&self, line: usize) -> Option<MarkerSeverity> {
        self.markers
            .iter()
            .filter(|marker| marker.line == line)
            .map(|marker| marker.severity)
            .min_by_key(|severity| severity.rank())
    }

    pub(crate) fn inline_diagnostics(&self) -> Vec<InlineDiagnostic> {
        if self.markers.is_empty() {
            return Vec::new();
        }
        let mut result: Vec<InlineDiagnostic> = if self.folded_lines.is_empty() {
            // Nothing is hidden, so a marker's visual row is its line number when
            // wrapping is off. Building a line-to-row table here would walk the
            // whole document on every frame for no reason.
            if !self.config.word_wrap {
                self.markers.iter().map(|marker| (marker.line, marker.clone())).collect()
            } else {
                let columns = self.text_columns().max(1);
                self.markers
                    .iter()
                    .map(|marker| {
                        let row: usize = (0..marker.line)
                            .map(|line| self.line_len(line).max(1).div_ceil(columns))
                            .sum();
                        (row, marker.clone())
                    })
                    .collect()
            }
        } else {
            let mut rows_by_line: Vec<(usize, usize)> = Vec::new();
            let mut row = 0usize;
            for line in self.visible_document_lines() {
                rows_by_line.push((line, row));
                row += self.wrap_segments(line);
            }
            self.markers
                .iter()
                .filter_map(|marker| {
                    rows_by_line
                        .iter()
                        .find(|(line, _)| *line == marker.line)
                        .map(|(_, row)| (*row, marker.clone()))
                })
                .collect()
        };
        result.sort_by_key(|(row, marker)| (*row, marker.severity.rank(), marker.start_column()));
        result
    }
}
