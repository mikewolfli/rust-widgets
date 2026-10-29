// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the host seams requested in `feedback-20261029.md` §6.
//!
//! Four gaps kept a host from deleting its own duplicate implementations: a shared
//! document handle, a public byte-offset ⇄ `TextPosition` pair, a read-only minimap
//! summary, and a single-point visual-row lookup that does not walk the document.
//! Each test names the seam it pins.

// The module-level gate, so tooling that scans file text recognises this as a test module
// (see `tests_batch5.rs` for the full reasoning).
#![cfg(test)]

use super::editor::CodeEditor;
use super::types::{CodeEditorConfig, TextPosition};
use crate::core::Rect;

fn editor() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

fn wrapping_editor() -> CodeEditor {
    CodeEditor::with_config(Rect::new(0, 0, 160, 300), CodeEditorConfig::new().word_wrap(true))
        .expect("valid config")
}

// ── Seam 2: byte offset ⇄ TextPosition ──────────────────────────────────────

/// A position round-trips through `offset_of` and `position_of`, multi-byte text included.
#[test]
fn offset_and_position_are_inverses() {
    let mut editor = editor();
    editor.set_text("héllo\nwörld\n日本語\n\nlast");

    for line in 0..editor.line_count() {
        for column in 0..=editor.line_len(line) {
            let position = TextPosition::new(line, column);
            let offset = editor.offset_of(position);
            assert_eq!(
                editor.position_of(offset),
                position,
                "position {position:?} must survive offset {offset}"
            );
        }
    }
}

/// The offset agrees with the line lookup the editor already exposed.
#[test]
fn offset_of_agrees_with_line_of_byte_offset() {
    let mut editor = editor();
    let text = "alpha\nbé\ngamma\n";
    editor.set_text(text);
    let end = editor.offset_of(TextPosition::new(editor.line_count() - 1, 0));
    for offset in 0..=end {
        assert_eq!(
            editor.position_of(offset).line,
            editor.line_of_byte_offset(offset),
            "offset {offset} must map to the same line both ways"
        );
    }
}

/// An offset inside a multi-byte character snaps down to that character's first byte.
#[test]
fn position_of_snaps_into_a_character() {
    let mut editor = editor();
    editor.set_text("é");
    assert_eq!(editor.offset_of(TextPosition::new(0, 1)), 2, "é is two bytes");
    assert_eq!(editor.position_of(1), TextPosition::new(0, 0), "byte 1 is inside é");
}

/// Past-the-end input is clamped on both public entry points rather than panicking.
#[test]
fn offsets_and_positions_clamp() {
    let mut editor = editor();
    editor.set_text("ab\ncd");
    let last = TextPosition::new(1, 2);
    assert_eq!(editor.offset_of(TextPosition::new(99, 99)), editor.offset_of(last));
    assert_eq!(editor.position_of(usize::MAX), last);
}

// ── Seam 4: single-point visual row ─────────────────────────────────────────

/// Without wrapping or folding the visual row is the document line.
#[test]
fn visual_row_of_is_the_line_when_nothing_wraps() {
    let mut editor = editor();
    editor.set_text("a\nb\nc\nd");
    for line in 0..editor.line_count() {
        assert_eq!(editor.visual_row_of(TextPosition::new(line, 0)), Some(line));
    }
    assert_eq!(editor.visual_row_of(TextPosition::new(4, 0)), None, "past the last line");
}

/// A wrapped document agrees with the materialised visual lines, row for row.
#[test]
fn visual_row_of_agrees_with_visual_lines() {
    let mut editor = wrapping_editor();
    let mut text = String::new();
    for i in 0..6 {
        text.push_str(&"x".repeat(20 + i * 30));
        text.push('\n');
    }
    editor.set_text(text.trim_end_matches('\n').to_string());

    let rows = editor.visual_lines();
    for (index, row) in rows.iter().enumerate() {
        let position = TextPosition::new(row.line, row.start_column);
        assert_eq!(
            editor.visual_row_of(position),
            Some(index),
            "row {index} starts at {position:?}"
        );
    }
}

/// `visual_line_at` converts a byte offset and returns the same row as the position form.
#[test]
fn visual_line_at_matches_the_position_form() {
    let mut editor = wrapping_editor();
    editor.set_text("short\n".to_string() + &"y".repeat(200));
    for column in [0usize, 1, 40, 120, 200] {
        let position = TextPosition::new(1, column);
        assert_eq!(
            editor.visual_line_at(editor.offset_of(position)),
            editor.visual_row_of(position),
            "column {column} must agree through both seams"
        );
    }
}

/// A folded line reports the row the row table gives it, and never panics.
#[test]
fn visual_row_of_tolerates_folds() {
    let mut editor = editor();
    editor.set_text((0..30).map(|i| format!("line {i}\n")).collect::<String>());
    editor.fold(10, 20);
    for line in 0..editor.line_count() {
        assert_eq!(
            editor.visual_row_of(TextPosition::new(line, 0)),
            Some(editor.line_to_visual_row(line))
        );
    }
}

// ── Seam 3: minimap summary ─────────────────────────────────────────────────

/// The summary is built on demand and reports start line, indent and non-blank count.
#[test]
fn minimap_summary_describes_the_document() {
    let mut editor = editor();
    editor.set_text("fn main() {\n    println!(\"hi\");\n\n    // comment\n}\n");
    let rows = editor.minimap_rows();
    let summary = editor.minimap_summary(rows).to_vec();

    assert!(!summary.is_empty(), "a non-empty document has buckets");
    assert!(summary.len() <= rows, "never more buckets than strip rows");
    let non_blank: usize = summary.iter().map(|bucket| bucket.non_blank_lines).sum();
    assert_eq!(non_blank, 4, "one line of the five is blank");
    assert_eq!(summary[0].start_line, 0);
    for pair in summary.windows(2) {
        assert!(pair[0].start_line < pair[1].start_line, "start lines ascend");
    }
    assert!(summary.iter().any(|bucket| bucket.indent >= 4), "the indented line is recorded");
}

/// The cached accessor does not build anything; it is empty until the summary exists.
#[test]
fn minimap_summary_cached_is_empty_before_any_build() {
    let mut editor = editor();
    editor.set_text("one\ntwo\nthree");
    assert!(editor.minimap_summary_cached().is_empty());
    let rows = editor.minimap_rows();
    assert!(!editor.minimap_summary(rows).is_empty());
    assert!(!editor.minimap_summary_cached().is_empty(), "the build is now cached");
}

// ── Seam 1: shared document handle ──────────────────────────────────────────

/// Two views of one document see each other's edits and share the revision.
#[test]
fn shared_views_see_the_same_text_and_revision() {
    let document = super::CodeEditorDocument::new("alpha\nbeta");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    assert_eq!(second.text(), "alpha\nbeta");

    first.set_cursor(0, 0, false);
    first.insert("X");

    assert_eq!(first.text(), "Xalpha\nbeta");
    assert_eq!(second.text(), "Xalpha\nbeta", "the second view reads the shared text");
    assert_eq!(first.revision(), second.revision(), "one revision for the document");
    assert_eq!(document.revision(), first.revision());
}

/// Caret and scroll stay private to each view.
#[test]
fn shared_views_keep_private_caret_and_scroll() {
    let document =
        super::CodeEditorDocument::new((0..100).map(|i| format!("line {i}\n")).collect::<String>());
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    first.set_cursor(2, 1, false);
    first.scroll_by(2);

    assert_eq!(first.cursor(), (2, 1));
    assert_eq!(second.cursor(), (0, 0), "the second view's caret did not move");
    assert!(first.scroll_line() > 0);
    assert_eq!(second.scroll_line(), 0, "the second view's scroll did not move");
}

/// Replacing the text through the handle reaches every view and advances the revision.
#[test]
fn the_handle_replaces_text_for_every_view() {
    let mut document = super::CodeEditorDocument::new("old");
    let view = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let before = document.revision();
    document.set_text("new");
    assert!(document.revision() > before);
    assert_eq!(view.text(), "new");
    assert_eq!(view.revision(), document.revision());
}

/// An editor can hand out a handle and keep editing; `into_document` drops the view state.
#[test]
fn an_editor_can_export_its_document() {
    let mut original = editor();
    original.set_text("shared");

    let handle = original.document();
    let view = CodeEditor::with_document(Rect::new(0, 0, 400, 300), handle);
    assert_eq!(view.text(), "shared");

    let mut consumed = editor();
    consumed.set_text("exported");
    let document = consumed.into_document();
    assert_eq!(document.text(), "exported");
}

// ── Seam 1 (continued): per-view active buffer, shared history, per-view folds ──

/// The buffer set is shared while the active index is not.
#[test]
fn two_views_can_show_different_buffers_of_one_document() {
    let document = super::CodeEditorDocument::new("first");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let mut second_view = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    let second = first.open_buffer("second", "two");
    assert_eq!(second_view.buffers().len(), 2, "the buffer list is document state");
    assert_eq!(first.active_buffer(), second);
    assert_eq!(second_view.active_buffer(), 0, "the other view kept its own tab");
    assert_eq!(first.text(), "two");
    assert_eq!(second_view.text(), "first");

    assert!(second_view.activate_buffer(second));
    assert_eq!(second_view.text(), "two");
    assert_eq!(first.text(), "two");
}

/// Undo history belongs to the buffer, so both views undo the same edit.
#[test]
fn views_share_the_undo_history_of_a_buffer() {
    let document = super::CodeEditorDocument::new("base");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let mut second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    first.set_cursor(0, 4, false);
    first.insert("!");
    assert_eq!(second.text(), "base!");
    assert!(second.can_undo(), "the edit is in the shared history");
    assert!(second.undo());
    assert_eq!(first.text(), "base", "an undo in one view is visible in the other");
}

/// Fold regions are view state: collapsing in one view leaves the other expanded.
#[test]
fn views_keep_independent_folds() {
    let document = super::CodeEditorDocument::new("a\nb\nc\nd");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    first.fold(1, 2);
    assert!(first.is_line_hidden(2));
    assert!(!second.is_line_hidden(2), "the other view has no folds");
    assert_eq!(second.fold_regions().len(), 0);
}

/// Closing a buffer in one view re-homes the other before it reads.
#[test]
fn closing_a_buffer_rehomes_the_other_view() {
    let document = super::CodeEditorDocument::new("first");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let mut second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    let extra = first.open_buffer("extra", "second");
    assert!(second.activate_buffer(extra));
    assert_eq!(second.text(), "second");

    assert!(first.close_buffer(extra));
    assert_eq!(first.text(), "first");
    // The other view still points past the end until it is told the set shrank.
    assert!(second.sync_with_document());
    assert_eq!(second.active_buffer(), 0);
    assert_eq!(second.text(), "first");
}

// ── Stable buffer ids: snapshots survive an unrelated close ─────────────────

/// Closing an earlier buffer only moves the position; the other view keeps its buffer,
/// its caret and its folds.
#[test]
fn closing_an_earlier_buffer_keeps_the_other_views_snapshot() {
    let document = super::CodeEditorDocument::new("A");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let mut second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    first.open_buffer("B", "BB");
    let last = first.open_buffer("C", "CCC");
    assert!(second.activate_buffer(last));
    assert_eq!(second.text(), "CCC");

    // Two different carets in two views on the same buffer.
    first.set_cursor(0, 1, false);
    second.set_cursor(0, 2, false);

    // Remove A, which sits before the shared buffer: its position shifts from 2 to 1.
    assert!(first.close_buffer(0));
    assert_eq!(first.text(), "CCC");
    assert_eq!(first.cursor(), (0, 1));

    assert!(!second.sync_with_document(), "the buffer itself survived the close");
    assert_eq!(second.active_buffer(), 1, "the position followed the buffer");
    assert_eq!(second.text(), "CCC");
    assert_eq!(second.cursor(), (0, 2), "the caret stayed with its buffer");
}

/// Re-opening a buffer gets a fresh snapshot rather than inheriting the closed one.
#[test]
fn a_reopened_buffer_does_not_inherit_the_closed_snapshot() {
    let mut editor = editor();
    editor.set_text("A");
    let second = editor.open_buffer("B", "BBBB");
    editor.set_cursor(0, 3, false);
    assert!(editor.close_buffer(second));

    let reopened = editor.open_buffer("C", "C");
    assert_eq!(editor.active_buffer(), reopened);
    assert_eq!(editor.text(), "C");
    assert_eq!(editor.cursor(), (0, 0), "the new buffer starts with a default view");
}

// ── Public stable-id API ────────────────────────────────────────────────────

/// Ids survive a close that shifts positions, and an id that is gone is refused.
#[test]
fn stable_ids_survive_a_close_and_are_the_identity() {
    let document = super::CodeEditorDocument::new("A");
    let mut editor = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document);
    let a_id = editor.active_buffer_id();
    assert_eq!(editor.buffer_ids(), vec![a_id]);

    let b = editor.open_buffer("B", "BB");
    let b_id = editor.buffer_ids()[b];
    let c = editor.open_buffer("C", "CCC");
    let c_id = editor.buffer_ids()[c];
    assert_ne!(a_id, b_id);
    assert_ne!(b_id, c_id);
    assert_eq!(editor.active_buffer_id(), c_id);

    // Close the first buffer: positions shift down, the ids do not.
    assert!(editor.close_buffer(0));
    assert_eq!(editor.buffer_ids(), vec![b_id, c_id]);
    assert_eq!(editor.active_buffer_id(), c_id);
    assert_eq!(editor.active_buffer(), 1, "the position moved, the identity did not");
    assert_eq!(editor.text(), "CCC");

    assert!(!editor.activate_buffer_id(a_id), "a closed id cannot be activated");
    assert!(editor.activate_buffer_id(b_id));
    assert_eq!(editor.text(), "BB");
    assert_eq!(editor.active_buffer_id(), b_id);
    assert!(!editor.activate_buffer_id(b_id), "activating the current id is a no-op");
}

/// Two views agree on the id set, and an id taken from one activates the same buffer in the other.
#[test]
fn an_id_identifies_the_same_buffer_in_both_views() {
    let document = super::CodeEditorDocument::new("A");
    let mut first = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());
    let mut second = CodeEditor::with_document(Rect::new(0, 0, 400, 300), document.clone());

    first.open_buffer("B", "BB");
    let b_id = first.active_buffer_id();
    assert_eq!(second.buffer_ids(), first.buffer_ids());

    assert!(second.activate_buffer_id(b_id));
    assert_eq!(second.text(), "BB");
    assert_eq!(second.active_buffer_id(), b_id);
    assert_eq!(first.text(), "BB", "both views are on the same buffer");
}

/// Closing by id removes the named buffer and leaves the active one alone; unknown ids are
/// refused.
#[test]
fn close_buffer_id_closes_the_named_buffer() {
    let mut editor = editor();
    editor.set_text("A");
    editor.open_buffer("B", "BB");
    editor.open_buffer("C", "CCC");
    let ids = editor.buffer_ids();
    let (a_id, b_id, c_id) = (ids[0], ids[1], ids[2]);
    assert_eq!(editor.active_buffer_id(), c_id);

    assert!(editor.close_buffer_id(b_id));
    assert_eq!(editor.buffer_ids(), vec![a_id, c_id]);
    assert_eq!(editor.text(), "CCC");
    assert_eq!(editor.active_buffer_id(), c_id);

    assert!(!editor.close_buffer_id(b_id), "the id is already gone");
    assert!(!editor.close_buffer_id(9_999), "an id that never existed");
}

/// Closing the active buffer by id re-homes the view to a survivor.
#[test]
fn close_buffer_id_of_the_active_buffer_rehomes() {
    let mut editor = editor();
    editor.set_text("A");
    editor.open_buffer("B", "BB");
    let b_id = editor.active_buffer_id();

    assert!(editor.close_buffer_id(b_id));
    assert_eq!(editor.buffer_ids().len(), 1);
    assert_ne!(editor.active_buffer_id(), b_id);
    assert_eq!(editor.active_buffer_id(), editor.buffer_ids()[0]);
    assert_eq!(editor.text(), "A");
}

/// The last buffer is emptied, not removed, so the document and its id survive.
#[test]
fn close_buffer_id_of_the_last_buffer_empties_it() {
    let mut editor = editor();
    editor.set_text("only");
    let id = editor.active_buffer_id();

    assert!(editor.close_buffer_id(id));
    assert_eq!(editor.text(), "");
    assert_eq!(editor.buffer_ids(), vec![id], "the document keeps its one buffer");
}
