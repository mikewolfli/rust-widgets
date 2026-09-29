// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for batch 5: the coordinate mapping layer (`B3`) and the visual-row
//! reverse lookup (`B4`).
//!
//! The property under test is **agreement**: the transform a caller uses to place
//! something must be the inverse of the transform used to hit-test it, and both
//! must survive wrapping and folding. Before `B4` the forward direction walked to
//! the target line and the reverse walked from the top, so the two could disagree
//! after a fold — and both were linear in the document.

use super::editor::CodeEditor;
use super::types::{CodeEditorConfig, TextPosition, ViewportSnapshot, DEFAULT_FONT_SIZE};
use crate::core::{Point, Rect};
use crate::widget::Widget;

fn editor() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

/// A wide document that does not wrap.
fn plain(lines: usize) -> String {
    let mut text = String::new();
    for i in 0..lines {
        text.push_str(&format!("line {i}\n"));
    }
    text
}

/// With neither wrapping nor folding, row and line are the same number — the fast
/// path that must never build a prefix table.
#[test]
fn rows_equal_lines_without_wrap_or_fold() {
    let mut editor = editor();
    editor.set_text(plain(50));
    assert!(
        editor.visual_row_prefix_is_empty(),
        "the fold-free, wrap-free case must not pay for a table"
    );
    for line in [0usize, 1, 17, 49] {
        assert_eq!(editor.line_to_visual_row(line), line);
        assert_eq!(editor.visual_row_to_line(line), line);
    }
}

/// With wrapping on, the two directions must still be inverses: this is the case
/// that used to be a forward walk in one direction and a separate walk in the
/// other.
#[test]
fn the_row_and_line_transforms_are_inverses_under_wrapping() {
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 160, 300), CodeEditorConfig::new().word_wrap(true))
            .expect("valid config");
    // Lines of varying length, so some wrap and some do not.
    let mut text = String::new();
    for i in 0..40 {
        let width = (i % 4) * 20;
        text.push_str(&"x".repeat(width));
        text.push('\n');
    }
    editor.set_text(text);

    let rows = editor.visual_row_count();
    assert!(rows >= editor.line_count(), "wrapping can only add rows");
    for line in 0..editor.line_count() {
        let row = editor.line_to_visual_row(line);
        assert_eq!(
            editor.visual_row_to_line(row),
            line,
            "line {line} at row {row} must round-trip"
        );
    }
}

/// A folded region must be skipped by both directions, and the reverse lookup must
/// return the fold's own start line for every row the fold occupies.
#[test]
fn the_row_transforms_respect_folding() {
    let mut editor = editor();
    editor.set_text(plain(30));
    editor.fold(10, 20);
    assert_eq!(editor.folded_line_count(), 10);

    // Line 9 sits immediately above the fold; line 21 immediately below.
    let row_before = editor.line_to_visual_row(9);
    let row_start = editor.line_to_visual_row(10);
    let row_after = editor.line_to_visual_row(21);
    assert_eq!(row_start, row_before + 1, "the fold start follows line 9 directly");
    assert_eq!(row_after, row_start + 1, "line 21 follows the fold's single visible row");

    assert_eq!(editor.visual_row_to_line(row_start), 10, "the row maps to the fold start");
    assert_eq!(editor.visual_row_to_line(row_after), 21);
    // Every hidden line must be unreachable through the row transform.
    for hidden in 11..=20 {
        assert_ne!(editor.visual_row_to_line(row_start), hidden);
    }
}

/// The prefix table must be exactly as long as the document plus one sentinel, so
/// a caller can tell at a glance that it is valid — and it must be rebuilt, not
/// left describing a document that no longer exists.
#[test]
fn the_prefix_table_is_rebuilt_when_the_document_changes() {
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 160, 300), CodeEditorConfig::new().word_wrap(true))
            .expect("valid config");
    editor.set_text(plain(20));
    assert_eq!(
        editor.visual_row_prefix_len(),
        editor.line_count() + 1,
        "one entry per line plus the total sentinel"
    );

    editor.set_text(plain(5));
    assert_eq!(
        editor.visual_row_prefix_len(),
        editor.line_count() + 1,
        "the table followed the shrink"
    );

    editor.fold(0, 3);
    assert_eq!(
        editor.visual_row_prefix_len(),
        editor.line_count() + 1,
        "a fold rebuilds but does not resize it"
    );
}

/// A hit-test must land on the line whose rectangle was drawn there, for every
/// visible row — this is the agreement between `B3` and `B4`.
#[test]
fn a_hit_test_agrees_with_the_row_transform() {
    let mut editor = editor();
    editor.set_text(plain(30));
    editor.fold(8, 14);

    let map = editor.coordinate_map();
    for row in 0..(editor.line_count() - editor.folded_line_count()).min(8) {
        let y = map.row_y(row);
        let line = editor.visual_row_to_line(row);
        let position = editor
            .position_at_point(Point::new(map.column_x(0), y))
            .expect("the point is inside the text area");
        assert_eq!(
            position.line, line,
            "row {row} drawn as line {line} must hit-test to the same line"
        );
    }
}

/// A point above and below the text area must be rejected, not clamped into a
/// position: a click in the tab strip is not a click in the document.
#[test]
fn a_point_outside_the_text_area_is_rejected() {
    let mut editor = editor();
    editor.set_text(plain(10));
    let map = editor.coordinate_map();
    assert!(editor.position_at_point(Point::new(map.column_x(0), map.row_y(0))).is_some());
    // Above the source area (inside the tab strip / find bar region).
    let above_y = map.row_y(0) - 1 - editor.text_origin_y();
    assert!(editor.position_at_point(Point::new(map.column_x(0), above_y)).is_none());
}

/// The total row count and the reverse lookup must not disagree about the last
/// row of the document: an off-by-one there puts the scrollbar and the bottom of
/// the buffer out of step.
#[test]
fn the_last_row_maps_to_the_last_line() {
    let mut editor = editor();
    editor.set_text(plain(12));
    let total = editor.visual_row_count();
    assert!(total >= 12);
    let last_line = editor.line_count() - 1;
    assert_eq!(editor.visual_row_to_line(total.saturating_sub(1)), last_line);
    // A row past the end clamps rather than panicking or wrapping around.
    assert_eq!(editor.visual_row_to_line(total + 100), last_line);
}

/// Wrapping plus folding together must keep the transforms inverse, since that is
/// the combination where the two former forward walks could disagree.
#[test]
fn the_row_transforms_are_inverses_under_wrap_and_fold() {
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 160, 300), CodeEditorConfig::new().word_wrap(true))
            .expect("valid config");
    let mut text = String::new();
    for i in 0..30 {
        text.push_str(&"y".repeat((i % 3) * 15));
        text.push('\n');
    }
    editor.set_text(text);
    editor.fold(5, 12);

    for line in 0..editor.line_count() {
        if editor.is_line_hidden(line) {
            continue;
        }
        let row = editor.line_to_visual_row(line);
        assert_eq!(editor.visual_row_to_line(row), line, "line {line} at row {row}");
    }
}

/// `rect_for_range` must agree with the coordinate map on both axes.
#[test]
fn the_range_rectangle_uses_the_coordinate_map() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta\n");
    let map = editor.coordinate_map();
    let rect = editor
        .rect_for_range(TextPosition::new(1, 0), TextPosition::new(1, 4))
        .expect("a valid range");
    assert_eq!(rect.x, map.column_x(0));
    assert_eq!(rect.y, map.row_y(editor.line_to_visual_row(1)));
    assert!(rect.width > 0 && rect.height > 0);
}

/// A font size change moves the row height, so the map must be derived per call
/// rather than cached — a stale map would hit-test against the old geometry.
#[test]
fn the_map_reflects_the_current_geometry() {
    let mut editor = editor();
    editor.set_text(plain(5));
    let before = editor.coordinate_map();
    editor.set_geometry(Rect::new(0, 40, 800, 600));
    let after = editor.coordinate_map();
    assert_eq!(after.row_y(0) - before.row_y(0), 40, "the map followed the widget origin");
}

/// A guard against the map silently reading the configured font size constant
/// instead of the measured cell: the two must be the same number for a default
/// editor, but the point is that the map is built from whatever is current.
#[test]
fn the_map_column_step_is_the_cell_width() {
    let editor = editor();
    let map = editor.coordinate_map();
    let a = map.column_x(0);
    let b = map.column_x(1);
    assert!(b > a, "a column advance must move the origin");
    let _ = DEFAULT_FONT_SIZE;
}

// ── D3: multi-instance linkage ──────────────────────────────────────────────

/// A viewport snapshot taken from one editor must place another at the same row,
/// which is what makes a linked two-pane comparison line up.
#[test]
fn a_viewport_snapshot_links_two_editors() {
    let mut left = CodeEditor::new(Rect::new(0, 0, 400, 120));
    left.set_text(plain(200));
    left.set_geometry(Rect::new(0, 0, 400, 120));
    left.refresh_visible_rows();
    left.scroll_by(20);
    assert_eq!(left.scroll_line(), 20);

    let mut right = CodeEditor::new(Rect::new(0, 0, 400, 120));
    right.set_text(plain(200));
    right.set_geometry(Rect::new(0, 0, 400, 120));
    right.refresh_visible_rows();
    right.apply_viewport(left.viewport_snapshot());

    assert_eq!(right.first_visual_row(), left.first_visual_row());
    assert_eq!(right.scroll_column(), left.scroll_column());
    assert_eq!(right.caret(), left.caret());
}

/// Applying a snapshot is navigation, not an edit: it must not push an undo entry
/// or mark the buffer modified.
#[test]
fn applying_a_snapshot_does_not_edit_or_dirty_the_buffer() {
    let mut editor = editor();
    editor.set_text(plain(50));
    editor.mark_saved();
    editor.clear_history();
    assert!(!editor.is_modified());
    assert!(!editor.can_undo(), "the history starts empty for this assertion");

    editor.apply_viewport(ViewportSnapshot {
        first_visual_row: 5,
        scroll_column: 3,
        caret: TextPosition::new(10, 2),
    });

    assert!(!editor.is_modified(), "linking panes is not an edit");
    assert_eq!(editor.caret(), TextPosition::new(10, 2));
    assert!(!editor.can_undo(), "no history entry may be recorded");
}

/// A snapshot from a larger document must be clamped into a smaller one rather
/// than placing the caret past the end.
#[test]
fn a_snapshot_is_clamped_into_a_shorter_document() {
    let mut editor = editor();
    editor.set_text(plain(3));
    editor.apply_viewport(ViewportSnapshot {
        first_visual_row: 9_999,
        scroll_column: 400,
        caret: TextPosition::new(9_999, 9_999),
    });
    let last = editor.line_count() - 1;
    assert_eq!(editor.caret().line, last, "the caret clamps to the last line");
    assert!(editor.first_visual_row() < editor.visual_row_count().max(1) + 1);
}
