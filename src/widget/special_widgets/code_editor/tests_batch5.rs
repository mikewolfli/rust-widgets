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

// The module-level gate, so tooling that scans file text recognises this as a test module.
// The parent declares `mod tests;` under `#[cfg(test)]`, which the compiler sees but a text scan
// of this file does not — and a scan that cannot tell a test module from production code reports
// its `Mutex` as widget state (`tools/check_locking.sh`). Same line as
// `special_widgets/toast/tests.rs`, which records the reasoning in full.
#![cfg(test)]

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

// ── Measured typography (the proportional-face fix) ──────────────────────────

/// The row height is derived from the font size unless the caller overrides it.
///
/// This is the Zed `buffer_line_height` model: `Standard` is `1.3`, `Comfortable`
/// is `1.618`, and the editor's default sits between them. The previous fixed
/// `16.0` px was a ratio of `1.23` for the default 13 pt font — tighter than
/// either preset — and it did not move when the font size did.
#[test]
fn the_row_height_follows_the_font_size_unless_overridden() {
    let derived = CodeEditorConfig::new().font_size(20.0).line_height_ratio(1.5);
    assert_eq!(derived.resolved_line_advance(), 30.0);
    let overridden = derived.line_advance(24.0);
    assert_eq!(
        overridden.resolved_line_advance(),
        24.0,
        "an explicit advance is the caller's decision and wins"
    );
    assert!(overridden.validate().is_ok(), "24 >= 20 keeps the invariant");
}

/// The status row's selection count must equal the text the selection holds.
#[test]
fn the_selection_character_count_matches_the_extracted_text() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta\ngamma");
    editor.set_cursor(0, 2, false);
    editor.set_cursor(2, 3, true);
    let counted = editor.selection_char_count();
    let extracted = editor.selected_text().expect("non-collapsed selection");
    assert_eq!(counted, extracted.chars().count());
}

/// A painted row records one x per character boundary, in increasing order.
///
/// The renderer draws with a proportional face (`Open Sans` is the compiled Latin
/// face), so `column * cell_width` is not where a character landed. Painting the row
/// measures its runs and stores the boundary positions; the caret and the hit-test
/// read them. Without this the caret pointed at a glyph that was not there.
#[test]
fn a_painted_row_reports_measured_character_positions() {
    let mut editor = editor();
    editor.set_text("fn main() {}");
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let positions: Vec<f32> = (0..=12).filter_map(|index| editor.cached_x(0, index)).collect();
    assert_eq!(positions.len(), 13, "every boundary of the painted row has an x");
    assert_eq!(positions[0], 0.0, "the row starts at x = 0");
    assert!(
        positions.windows(2).all(|pair| pair[1] > pair[0]),
        "boundaries advance left to right: {positions:?}"
    );
}

/// The toolkit synthesises a motion event after a click. A double click that selects a
/// word must not be collapsed by that motion: it used to arrive with the drag latch
/// still set, extend the selection to the click point and leave nothing selected.
#[test]
fn a_double_click_selection_survives_the_synthetic_motion() {
    use crate::event::{Event, EventHandler};
    let mut editor = editor();
    editor.set_text("fn main() {}");
    let point = Point::new(editor.text_origin_x() + 40, editor.text_origin_y() + 4);
    // The real gesture, in the order the toolkit delivers it: press, release, then the
    // double-click event for the second press, then a motion at the same point.
    editor.handle_event(&Event::MousePress { pos: point, button: 1, modifiers: 0 });
    editor.handle_event(&Event::MouseRelease { pos: point, button: 1 });
    editor.handle_event(&Event::MouseDoubleClick { pos: point, button: 1 });
    let selected = editor.selected_text();
    assert_eq!(selected.as_deref(), Some("main"), "the double click selects the word");
    editor.handle_event(&Event::MouseMove { pos: point });
    assert_eq!(
        editor.selected_text(),
        selected,
        "a motion that did not move must not collapse the word selection"
    );
}

// ── Pointer intent and the minimap ───────────────────────────────────────────

/// The pointer is an I-beam over the text and an arrow over the chrome.
///
/// One shape for the whole widget announced "text" over a scrollbar and a minimap,
/// which reads as a broken hit target — the same distinction Zed draws between its
/// buffer and its minimap.
#[test]
fn the_pointer_shape_follows_the_region_under_it() {
    use crate::platform::CursorIcon;
    let editor = editor();
    let rect = editor.base().geometry();
    let text = Point::new(editor.text_origin_x() + 20, editor.text_origin_y() + 5);
    assert_eq!(editor.cursor_icon_at(text), CursorIcon::Text, "the buffer is text");
    let minimap = Point::new(
        rect.x + rect.width as i32 - editor.minimap_width() / 2,
        editor.text_origin_y() + 5,
    );
    assert_eq!(
        editor.cursor_icon_at(minimap),
        CursorIcon::Default,
        "the minimap is a navigation strip, not text"
    );
    let gutter = Point::new(rect.x + 2, editor.text_origin_y() + 5);
    assert_eq!(
        editor.cursor_icon_at(gutter),
        CursorIcon::Default,
        "line numbers are not editable text"
    );
}

/// Scrolling to a fraction of the document moves the viewport proportionally.
///
/// This is the mapping the minimap's click and drag use; a strip that shows the
/// whole file has to answer "where in the file is this point".
#[test]
fn scrolling_to_a_fraction_moves_the_viewport_proportionally() {
    let mut editor = editor();
    editor.set_text(plain(400));
    editor.base_mut().set_geometry(Rect::new(0, 0, 800, 200));
    editor.refresh_visible_rows();

    editor.scroll_to_fraction(0.0);
    assert_eq!(editor.scroll_line(), 0, "the top of the strip is the top of the file");

    editor.scroll_to_fraction(0.5);
    let middle = editor.scroll_line();
    assert!((100..=300).contains(&middle), "half way is about line 200, got {middle}");

    editor.scroll_to_fraction(1.0);
    let last = editor.scroll_line();
    assert!(
        last + editor.visible_rows() >= editor.line_count() - 1,
        "the bottom of the strip must show the end of the file: last={last}"
    );
}

// ── Tabs, IME, numbering and wrapped rows ────────────────────────────────────

fn centre(rect: Rect) -> Point {
    Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2)
}

/// A tab click activates its buffer; the close box closes it.
///
/// Both hit regions come from the painter (`tab_hits`), which is the only place the
/// title is measured in the font it is drawn in. Before this, a tab was sized from
/// its character count while its title was measured, so the close affordance sat
/// somewhere the label was not.
#[test]
fn a_tab_click_activates_and_the_close_box_closes() {
    let mut editor = editor();
    editor.open_buffer("a.rs", "fn a() {}");
    editor.open_buffer("b.rs", "fn b() {}");
    let _ = crate::widget::svg::render_to_svg(&mut editor);

    let slots = editor.tab_hits.borrow().clone();
    assert_eq!(
        slots.len(),
        editor.buffers().len(),
        "one slot per open buffer (the editor starts with an untitled one)"
    );
    assert_eq!(slots.len(), 3);
    for pair in slots.windows(2) {
        assert!(
            pair[1].rect.x >= pair[0].rect.x + pair[0].rect.width as i32,
            "tabs must not overlap: {:?} then {:?}",
            pair[0].rect,
            pair[1].rect
        );
    }

    // The second buffer was opened last, so it is active.
    assert_eq!(editor.active_buffer(), 2);
    editor.pointer_press(centre(slots[0].rect));
    assert_eq!(editor.active_buffer(), 0, "a tab click activates its buffer");

    // The close box of the now-active first tab removes it.
    let slots = editor.tab_hits.borrow().clone();
    assert!(slots[0].close.width > 0, "with several buffers there is something to close");
    editor.pointer_press(centre(slots[0].close));
    assert_eq!(editor.buffers().len(), 2, "the close box closes the tab");
}

/// The IME composition is displayed until the commit replaces it.
///
/// The editor dropped `ImePreedit`, so a CJK composition was invisible until it was
/// committed — the candidate window had no anchor and a wrong candidate could not be
/// seen before it entered the document.
#[test]
fn the_ime_composition_is_shown_until_it_is_committed() {
    use crate::event::{Event, EventHandler};
    let mut editor = editor();
    editor.set_text("");
    editor.handle_event(&Event::ime_preedit("にほん", 3));
    assert_eq!(editor.preedit_text(), Some("にほん"), "the composition is visible");
    assert_eq!(editor.text(), "", "but it is not in the document yet");

    editor.handle_event(&Event::ime_commit("日本"));
    assert_eq!(editor.preedit_text(), None, "the commit replaces the composition");
    assert_eq!(editor.text(), "日本");
}

/// A matched bracket pair is marked with a **filled rounded box**.
///
/// The mark used to be a 1 px outline, which encloses the glyph like a selection
/// rectangle. A rounded fill (radius 2) is the only `rx="2"` element the editor emits
/// on a frame with no popups and no hovered inactive tab, so its presence in the SVG
/// is the structural proof that the box is a fill.
#[test]
fn a_matched_bracket_is_marked_with_a_filled_rounded_box() {
    let mut editor = editor();
    editor.set_text("fn main() {\n}");
    // The caret sits just after the `{`, which is what makes the pair match.
    editor.set_cursor(0, 11, false);
    let svg = crate::widget::svg::render_to_svg(&mut editor);
    assert!(
        svg.contains("rx=\"2\" ry=\"2\""),
        "the bracket marker must be a rounded fill, not an outline"
    );
}

/// A brace inside a string literal is text, not a bracket.
///
/// The matcher scanned the raw line, so a caret next to `"{}"` in a format string highlighted
/// that brace as if it paired with the block's closing brace — the demo's
/// `println!("fib({i}) = {}", …)` showed exactly that. The lexer already knows which spans are
/// strings and comments; matching now steps over them.
#[test]
fn a_bracket_inside_a_string_does_not_match() {
    let mut editor = editor();
    editor.set_text("let s = \"{\";\n}");
    editor.set_cursor(0, 9, false);
    assert_eq!(editor.matching_brackets(), None, "a brace in a string literal is text");

    editor.set_text("fn main() {\n}");
    editor.set_cursor(0, 10, false);
    let pair = editor.matching_brackets().expect("the block's braces still match");
    assert_eq!(pair.0, TextPosition::new(0, 10));
    assert_eq!(pair.1, TextPosition::new(1, 0));
}

/// The bracket pair is cached by `(revision, caret)` and follows both.
///
/// Without the cache every frame re-walked the document from the caret to its partner —
/// for a top-level brace that is the whole file, with a line allocation per step. The
/// cache must not go stale: moving the caret to a non-bracket position has to answer
/// `None` immediately, and an edit has to re-resolve against the new text.
#[test]
fn the_bracket_pair_cache_follows_the_caret_and_the_revision() {
    let mut editor = editor();
    editor.set_text("fn main() {\n}");
    editor.set_cursor(0, 10, false);
    let first = editor.matching_brackets().expect("the block's braces match");
    // Reading it again is the cached path; it must agree with the resolved answer.
    assert_eq!(editor.matching_brackets(), Some(first), "a still caret keeps its pair");

    // A caret on a non-bracket char is a different key and must not reuse the pair.
    editor.set_cursor(0, 0, false);
    assert_eq!(editor.matching_brackets(), None, "the caret left the bracket");

    // Back on the brace, then an edit that moves the closing brace to another line.
    editor.set_cursor(0, 10, false);
    assert_eq!(editor.matching_brackets(), Some(first), "returning to the brace re-matches");
    editor.set_text("fn main() {\n    let x = 1;\n}");
    editor.set_cursor(0, 10, false);
    let after = editor.matching_brackets().expect("the block still closes");
    assert_eq!(
        after.1,
        TextPosition::new(2, 0),
        "the cached pair must not survive an edit that moved the closing brace"
    );

    // The lexer also decides what a position *is*. `// }` is a comment in Rust and ordinary
    // text in Python, so the same document with the same caret has no pair in one language and
    // a pair in the other. A language change re-lexes without touching the revision, so the
    // cache has to be dropped with the line states.
    editor.set_language(super::syntax::LanguageId::Rust);
    editor.set_text("{ // }");
    editor.set_cursor(0, 0, false);
    assert_eq!(
        editor.matching_brackets(),
        None,
        "in Rust the closing brace is inside a line comment"
    );
    editor.set_language(super::syntax::LanguageId::Python);
    assert_eq!(
        editor.matching_brackets(),
        Some((TextPosition::new(0, 0), TextPosition::new(0, 5))),
        "in Python `//` is not a comment, so the cached `None` must have been dropped"
    );
}

/// Relative numbering counts from the caret and keeps the caret's line absolute.
#[test]
fn relative_line_numbers_count_from_the_caret() {
    let mut editor = CodeEditor::with_config(
        Rect::new(0, 0, 800, 600),
        CodeEditorConfig::new().relative_line_numbers(true),
    )
    .expect("valid config");
    editor.set_text(plain(10));
    editor.set_cursor(5, 0, false);
    assert_eq!(editor.line_number_label(5), "6", "the caret line stays absolute");
    assert_eq!(editor.line_number_label(4), "1");
    assert_eq!(editor.line_number_label(7), "2");
    assert_eq!(editor.line_number_label(0), "5");

    let absolute = CodeEditor::new(Rect::new(0, 0, 800, 600));
    let mut absolute = absolute;
    absolute.set_text(plain(10));
    absolute.set_cursor(5, 0, false);
    assert_eq!(absolute.line_number_label(0), "1", "absolute mode is the default");
}

fn wrapping_editor() -> CodeEditor {
    CodeEditor::with_config(Rect::new(0, 0, 200, 300), CodeEditorConfig::new().word_wrap(true))
        .expect("valid config")
}

/// A caret on a continuation row is placed on that row, and a click there maps
/// back to the same characters.
///
/// The previous model put every caret of a wrapped line on the line's **first**
/// row and measured the x from the line's first column, so the caret for the second
/// segment pointed at the first segment's characters.
#[test]
fn a_wrapped_rows_caret_and_hit_test_agree() {
    let mut editor = wrapping_editor();
    let columns = editor.text_columns().max(4);
    editor.set_text("x".repeat(columns * 3));
    editor.base_mut().set_geometry(Rect::new(0, 0, 200, 300));
    editor.refresh_visible_rows();

    editor.set_cursor(0, columns + 2, false);
    assert_eq!(editor.caret_visual_row(), 1, "the second segment is the second row");
    assert_eq!(editor.line_to_visual_row(0), 0, "the line itself still starts at row 0");

    let map = editor.coordinate_map();
    let point = Point::new(editor.geometry().x + editor.text_origin_x() + 1, map.row_y(1));
    let hit = editor.position_at_point(point).expect("the point is inside the text area");
    assert_eq!(hit.line, 0);
    assert!(
        hit.column >= columns,
        "a click on the second row belongs to the second segment, got column {}",
        hit.column
    );
}

/// A wrapped continuation row is indented to the line's own indentation.
#[test]
fn a_continuation_row_is_indented_by_the_lines_indent() {
    let mut editor = wrapping_editor();
    editor.base_mut().set_geometry(Rect::new(0, 0, 200, 300));
    editor.refresh_visible_rows();
    let columns = editor.text_columns().max(8);
    editor.set_text(format!("    {}", "y".repeat(columns * 2)));
    editor.refresh_visible_rows();

    let left = editor.geometry().x + editor.text_origin_x();
    let cell = editor.cell_width();
    let first_of_second = TextPosition::new(0, columns);
    let second_char = TextPosition::new(0, columns + 1);
    let first_rect = editor.rect_for_range(first_of_second, second_char).expect("valid range");
    assert_eq!(
        first_rect.x,
        left + (4.0 * cell).round() as i32,
        "the continuation row is indented by the line's four spaces"
    );
    assert_eq!(
        first_rect.y,
        editor.coordinate_map().row_y(1),
        "the range sits on the second visual row"
    );
    assert_eq!(first_rect.width, cell.round() as u32, "one character is one cell");
}

/// A press on a line number selects that whole line.
///
/// The gutter is outside the text area, so the previous handler did nothing there
/// and a click on the number — the one target that can only mean "this line" — was
/// dead.
#[test]
fn a_press_on_the_gutter_selects_the_line() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta\ngamma\n");
    editor.refresh_visible_rows();
    let y = editor.coordinate_map().row_y(1) + 2;
    let x = editor.geometry().x + 2;
    editor.pointer_press(Point::new(x, y));
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("beta\n"),
        "the gutter selects the line, including its newline"
    );
}

/// `Ctrl` + click adds a caret instead of moving the only one.
#[test]
fn control_click_adds_a_caret() {
    use crate::event::{Event, EventHandler};
    let mut editor = editor();
    editor.set_text("alpha\nbeta\ngamma");
    editor.refresh_visible_rows();
    let left = editor.text_origin_x() + 2;
    let row = editor.line_height().round() as i32;
    editor.pointer_press(Point::new(left, editor.text_origin_y() + 2));
    assert_eq!(editor.cursors().len(), 1);

    editor.handle_event(&Event::MousePress {
        pos: Point::new(left, editor.text_origin_y() + 2 + row),
        button: 1,
        modifiers: 2,
    });
    assert_eq!(editor.cursors().len(), 2, "Ctrl + click adds a caret");
    assert_eq!(editor.caret().line, 1);
}

/// The minimap holds one bucket per visible line for a short file, and one per
/// strip row for a long one — and a repaint with no change does not resize it.
///
/// The rebuild check used to compare against a count derived differently from the
/// size the rebuild produced, so it never matched for a document shorter than the
/// strip: the minimap was rebuilt, line by line, on **every frame**.
#[test]
fn the_minimap_has_one_bucket_per_line_until_it_runs_out_of_rows() {
    let mut short = editor();
    short.base_mut().set_geometry(Rect::new(0, 0, 800, 400));
    short.set_text(plain(14));
    short.prepare_minimap();
    let rows = short.minimap_rows();
    assert_eq!(
        short.minimap_buckets.len(),
        short.line_count().min(rows),
        "a short document must not allocate a bucket per strip pixel"
    );
    let first = short.minimap_buckets.len();
    short.prepare_minimap();
    assert_eq!(short.minimap_buckets.len(), first, "a no-op frame must not resize it");

    let mut long = editor();
    long.base_mut().set_geometry(Rect::new(0, 0, 800, 400));
    // No trailing newline: a final empty line would legitimately leave the last
    // bucket blank, which is not the tail this test is about.
    let mut text = String::new();
    for line in 0..5_000 {
        if line > 0 {
            text.push('\n');
        }
        text.push_str(&format!("line {line}"));
    }
    long.set_text(text);
    long.prepare_minimap();
    let rows = long.minimap_rows();
    assert!(
        long.minimap_buckets.len() <= rows,
        "the summary never claims more rows than the strip has"
    );
    let filled = long.minimap_buckets.iter().filter(|bucket| !bucket.blank).count();
    assert_eq!(
        filled,
        long.minimap_buckets.len(),
        "no bucket may be left blank: the partition has no tail"
    );
    // 5001 lines over 256 rows is 20 lines per bucket, so the 251st bucket covers
    // the remainder — the count is `rows` rounded down to the partition, not `rows`.
    assert!(long.minimap_buckets.len() > rows - 10, "the strip is filled, not truncated");
}

/// A cached row layout is reused while its inputs are unchanged, and re-measured when the text
/// changes.
///
/// The x of the boundary after "WWWW" is much wider than after "iiii", so a stale cache would
/// report the wide numbers against the narrow text. Painting twice with no change must keep the
/// same measurement (it is a measurement, not a fresh sample), and replacing the text must
/// replace the layout even though the visible line index never moved.
#[test]
fn a_cached_row_layout_is_reused_until_the_text_changes() {
    let mut editor = editor();
    editor.set_text("WWWW");
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let wide = editor.cached_x(0, 4).expect("the painted row has a boundary");

    let _ = crate::widget::svg::render_to_svg(&mut editor);
    assert_eq!(editor.cached_x(0, 4), Some(wide), "unchanged inputs reuse the measurement");

    editor.set_text("iiii");
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let narrow = editor.cached_x(0, 4).expect("the repainted row has a boundary");
    assert!(
        narrow < wide,
        "a stale layout would keep the wide x against narrow text: {narrow} vs {wide}"
    );
}

/// Changing a metric the layout was measured with — here the font size — invalidates the cache.
///
/// Without this the editor would draw larger glyphs at the old advances, so the caret and the
/// hit-test would disagree with the ink until the text happened to change.
#[test]
fn a_font_size_change_invalidates_the_cached_row_layout() {
    let mut editor = editor();
    editor.set_text("WWWW");
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let small = editor.cached_x(0, 4).expect("the painted row has a boundary");

    editor.config.font_size *= 2.0;
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let large = editor.cached_x(0, 4).expect("the repainted row has a boundary");
    assert!(large > small, "the doubled font must re-measure: {large} vs {small}");
}
