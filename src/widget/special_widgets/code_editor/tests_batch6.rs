// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the audit batch covering document model, input, editing,
//! find/completion, folding, the viewport and painting.
//!
//! Each test names the behaviour it pins, so a regression points at the row it
//! belongs to.

// The module-level gate, so tooling that scans file text recognises this as a test module.
#![cfg(test)]

use super::editor::CodeEditor;
use super::syntax::LanguageId;
use super::types::{
    CaretStyle, CodeEditorConfig, ExtraSelection, LineEnding, MarkerSeverity, SearchOptions,
    TextPosition,
};
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler, FocusReason};
use crate::widget::svg::render_to_svg;
use crate::widget::Widget;
use alloc::string::{String, ToString};

fn make() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

fn make_with(config: CodeEditorConfig) -> CodeEditor {
    CodeEditor::with_config(Rect::new(0, 0, 800, 600), config).expect("valid configuration")
}

/// Widget-local point of a document position.
fn point_at(editor: &CodeEditor, position: TextPosition) -> Point {
    let rect = editor.rect_for_range(position, position).expect("position is visible");
    Point::new(rect.x + 1, rect.y + 2)
}

/// Widget-local point on a gutter line.
fn gutter_point(editor: &CodeEditor, line: usize) -> Point {
    Point::new(4, editor.text_origin_y() + line as i32 * editor.line_height() as i32 + 4)
}

/// A document of `lines` numbered lines.
fn numbered(lines: usize) -> String {
    let mut text = String::new();
    for index in 0..lines {
        text.push_str("line ");
        text.push_str(&index.to_string());
        text.push('\n');
    }
    text
}

// ── Document model and performance ──────────────────────────────────────────

/// A cross-line edit rebuilds only the affected lines and keeps the mirror in
/// sync; the document is no longer joined and split around every Enter.
#[test]
fn a_cross_line_edit_keeps_the_index_and_mirror_in_step() {
    let mut editor = make();
    editor.set_text("alpha\nbeta\ngamma");
    editor.set_cursor(1, 2, false);
    editor.insert("\ninserted");
    assert_eq!(editor.text(), "alpha\nbe\ninsertedta\ngamma");
    assert_eq!(editor.line_count(), 4);
    // A second cross-line edit exercises the incremental path again.
    editor.insert_newline();
    assert_eq!(editor.text(), "alpha\nbe\ninserted\nta\ngamma");
    assert_eq!(editor.line_of_byte_offset(0), 0);
}

/// `\r\n` input is normalised and the original terminator is reported.
#[test]
fn crlf_documents_are_normalised_and_remembered() {
    let mut editor = make();
    editor.set_text("first\r\nsecond\r\nthird");
    assert_eq!(editor.line_count(), 3);
    assert_eq!(editor.line_text(0).as_deref(), Some("first"));
    assert_eq!(editor.line_text(1).as_deref(), Some("second"));
    assert!(!editor.text().contains('\r'));
    assert_eq!(editor.line_ending(), LineEnding::Crlf);
    editor.set_text("lf\nonly");
    assert_eq!(editor.line_ending(), LineEnding::Crlf, "an explicit change is a host decision");
    editor.set_line_ending(LineEnding::Lf);
    assert_eq!(editor.line_ending(), LineEnding::Lf);
}

/// A long document stays editable and the offsets remain correct after many
/// single-line splices (the Fenwick prefix index).
#[test]
fn many_single_line_edits_keep_offsets_correct() {
    let mut editor = make();
    editor.set_text(numbered(400));
    editor.set_cursor(200, 0, false);
    for _ in 0..200 {
        editor.insert("x");
    }
    assert_eq!(editor.line_len(200), 8 + 200);
    assert_eq!(editor.line_of_byte_offset(1), 0);
    assert!(editor.text().starts_with("line 0\n"));
}

/// A multi-caret edit still projects correctly through a large document.
#[test]
fn multi_caret_projection_handles_a_large_document() {
    let mut editor = make();
    let mut text = String::from("start\n");
    for _ in 0..5_000 {
        text.push_str("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n");
    }
    text.push_str("end");
    editor.set_text(text);
    editor.set_cursor(10, 0, false);
    editor.add_cursor_at(TextPosition::new(20, 0));
    editor.add_cursor_at(TextPosition::new(30, 0));
    editor.insert("Y");
    assert_eq!(editor.line_text(10).as_deref(), Some("Yxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
    assert_eq!(editor.line_text(20).as_deref(), Some("Yxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
    assert_eq!(editor.line_text(30).as_deref(), Some("Yxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"));
}

// ── Input and events ───────────────────────────────────────────────────────

/// A secondary click reaches the context menu, which was previously unreachable.
#[test]
fn a_secondary_click_opens_the_context_menu() {
    let mut editor = make();
    editor.set_text("text");
    editor.handle_event(&Event::MousePress {
        pos: Point::new(200, 120),
        button: crate::event::mouse_button::SECONDARY,
        modifiers: 0,
    });
    assert!(editor.context_menu().visible);
    assert!(!editor.context_menu().items.is_empty());
}

/// The Meta bit is the primary accelerator, so `Cmd+A` selects all.
#[test]
fn the_meta_modifier_is_recognised_as_the_primary_accelerator() {
    let mut editor = make();
    editor.set_text("alpha\nbeta");
    editor.handle_key_press(65, 8);
    assert!(editor.has_selection(), "Cmd+A must select all");
    assert_eq!(editor.selected_text().as_deref(), Some("alpha\nbeta"));
}

/// The newly bound editing keys do what their names promise.
#[test]
fn the_extended_key_table_edits_the_buffer() {
    let mut editor = make();
    editor.set_text("alpha beta");
    editor.set_cursor(0, 10, false);
    // Ctrl+Backspace deletes the word.
    editor.handle_key_press(8, 2);
    assert_eq!(editor.text(), "alpha ");

    // Insert toggles overwrite.
    editor.set_cursor(0, 0, false);
    editor.handle_key_press(45, 0);
    assert!(editor.overwrite_mode());
    editor.input_text("X");
    assert_eq!(editor.line_text(0).as_deref(), Some("Xlpha "));

    // Ctrl+Enter inserts a line below.
    let mut second = make();
    second.set_text("a\nb");
    second.set_cursor(0, 1, false);
    second.handle_key_press(13, 2);
    assert_eq!(second.text(), "a\n\nb");

    // Ctrl+T transposes around the caret.
    let mut third = make();
    third.set_text("ab");
    third.set_cursor(0, 1, false);
    third.handle_key_press(84, 2);
    assert_eq!(third.text(), "ba");

    // Ctrl+L selects the line.
    let mut fourth = make();
    fourth.set_text("one\ntwo");
    fourth.set_cursor(0, 1, false);
    fourth.handle_key_press(76, 2);
    assert_eq!(fourth.selected_text().as_deref(), Some("one\n"));
}

/// Ctrl+Shift+Z redoes, and F3 advances the find.
#[test]
fn redo_and_find_next_have_dedicated_bindings() {
    let mut editor = make();
    editor.set_text("abc");
    editor.set_cursor(0, 3, false);
    editor.insert("d");
    assert!(editor.undo());
    assert_eq!(editor.text(), "abc");
    editor.handle_key_press(90, 3);
    assert_eq!(editor.text(), "abcd", "Ctrl+Shift+Z must redo");

    let mut second = make();
    second.set_text("x x x");
    second.set_cursor(0, 0, false);
    second.set_search_query("x");
    second.open_find(false);
    let first = second.find_state().current;
    second.handle_key_press(1122, 0);
    assert_ne!(second.find_state().current, first, "F3 advances the current hit");
}

/// Alt+Down moves the caret line; Ctrl+Alt+Down adds a caret.
#[test]
fn line_move_and_add_cursor_use_the_expected_chords() {
    let mut editor = make();
    editor.set_text("a\nb");
    editor.set_cursor(0, 0, false);
    editor.handle_key_press(40, 4);
    assert_eq!(editor.text(), "b\na", "Alt+Down moves the line");

    let mut second = make();
    second.set_text("a\nb\nc");
    second.set_cursor(0, 0, false);
    second.handle_key_press(40, 4 | 2);
    assert!(second.has_multiple_cursors(), "Ctrl+Alt+Down adds a caret");
    assert_eq!(second.cursor().0, 1);
}

/// PageDown moves a whole page in one operation.
#[test]
fn paging_moves_by_a_viewport_in_one_step() {
    let mut editor = make();
    editor.set_text(numbered(400));
    editor.base_mut().set_geometry(Rect::new(0, 0, 800, 200));
    editor.handle_event(&Event::Resize { size: crate::core::Size::new(800, 200) });
    editor.set_cursor(0, 0, false);
    editor.handle_key_press(34, 0);
    assert!(editor.cursor().0 > 1, "a page is more than one line");
}

/// Triple-click selects the line, Shift+click extends, and a drag moves text.
#[test]
fn pointer_gestures_select_lines_extend_and_move_text() {
    let mut editor = make();
    editor.set_text("first\nsecond\nthird");
    let point = point_at(&editor, TextPosition::new(1, 1));
    editor.handle_event(&Event::MouseDoubleClick { pos: point, button: 1 });
    editor.handle_event(&Event::MousePress { pos: point, button: 1, modifiers: 0 });
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("second\n"),
        "the third press selects the whole line"
    );

    // Shift+click extends from the existing anchor.
    let mut second = make();
    second.set_text("one\ntwo\nthree");
    second.set_cursor(0, 0, false);
    let target = point_at(&second, TextPosition::new(2, 1));
    second.handle_event(&Event::MousePress { pos: target, button: 1, modifiers: 1 });
    let (start, end) = second.selection_bounds();
    assert_eq!(start.line, 0);
    assert_eq!(end.line, 2, "Shift+click extends across lines");

    // Dragging a selection moves it.
    let mut third = make();
    third.set_text("abc def");
    third.set_caret(TextPosition::new(0, 0), false);
    third.set_caret(TextPosition::new(0, 3), true);
    let grab = point_at(&third, TextPosition::new(0, 1));
    let drop = point_at(&third, TextPosition::new(0, 7));
    third.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });
    third.handle_event(&Event::MouseMove { pos: drop });
    third.handle_event(&Event::MouseRelease { pos: drop, button: 1 });
    assert_eq!(third.text(), " defabc");
}

/// Losing focus stops the blink; regaining it restarts without hiding the caret.
#[test]
fn focus_controls_the_caret_blink() {
    let mut editor = make();
    editor.handle_event(&Event::FocusLost);
    assert!(!editor.is_focused());
    assert!(!editor.tick(500), "a blurred editor asks for no frames");
    assert!(editor.is_caret_visible(), "the caret stays visible while blurred");
    editor.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
    assert!(editor.is_focused());
    assert!(editor.tick(10));
}

// ── Editing ────────────────────────────────────────────────────────────────

/// `insert_with_indent` expands leading tabs on every inserted line.
#[test]
fn insert_with_indent_expands_each_lines_leading_tab() {
    let mut editor = make_with(CodeEditorConfig::new().tab_width(4).insert_spaces(true));
    editor.insert_with_indent("\tone\n\t\ttwo");
    assert_eq!(editor.text(), "    one\n        two");
}

/// Each buffer keeps its own undo history and caret/scroll state.
#[test]
fn buffers_keep_their_own_history_and_view() {
    let mut editor = make();
    editor.set_text("aaaa");
    editor.set_cursor(0, 2, false);
    editor.insert("X");
    assert!(editor.can_undo());

    let second = editor.open_buffer("second", "bbbb");
    assert!(!editor.can_undo(), "the new buffer starts with its own empty history");
    editor.set_cursor(0, 4, false);
    editor.insert("Y");

    assert!(editor.activate_buffer(0));
    assert_eq!(editor.cursor(), (0, 3), "the first buffer's caret is restored");
    assert!(editor.undo());
    assert_eq!(editor.text(), "aaaa", "undo affects the first buffer only");
    assert!(editor.activate_buffer(second));
    assert_eq!(editor.text(), "bbbbY");
}

/// A typing run is one undo step, and moving the caret starts a new run.
#[test]
fn consecutive_typing_merges_into_one_undo_step() {
    let mut editor = make();
    editor.set_text("");
    editor.input_text("a");
    editor.input_text("b");
    editor.input_text("c");
    assert_eq!(editor.text(), "abc");
    assert!(editor.undo());
    assert_eq!(editor.text(), "", "one undo removes the whole typed run");

    editor.input_text("x");
    editor.handle_key_press(37, 0);
    editor.input_text("y");
    assert_eq!(editor.text(), "yx");
    assert!(editor.undo());
    assert_eq!(editor.text(), "x", "the second run is a separate step");
}

/// Line commands: block move, sort variants, block comments, transpose, snippet.
#[test]
fn the_remaining_line_commands_are_complete() {
    // Moving a block of lines.
    let mut editor = make();
    editor.set_text("a\nb\nc\nd");
    editor.set_caret(TextPosition::new(1, 0), false);
    editor.set_caret(TextPosition::new(2, 1), true);
    editor.move_line(-1);
    assert_eq!(editor.text(), "b\nc\na\nd");

    // Descending sort and duplicate removal.
    let mut second = make();
    second.set_text("a\nc\nb");
    second.sort_lines_descending();
    assert_eq!(second.text(), "c\nb\na");
    let mut third = make();
    third.set_text("b\na\nb");
    third.sort_lines_unique();
    assert_eq!(third.text(), "a\nb");

    // Block comments for a language that supports them.
    let mut fourth = make_with(CodeEditorConfig::new().language(LanguageId::Rust));
    fourth.set_text("let x = 1;");
    fourth.set_cursor(0, 0, false);
    fourth.toggle_block_comment();
    assert_eq!(fourth.text(), "/* let x = 1; */");
    fourth.toggle_block_comment();
    assert_eq!(fourth.text(), "let x = 1;");

    // Snippets resolve their placeholders.
    let mut fifth = make();
    fifth.insert_snippet("fn $0() {}");
    assert_eq!(fifth.text(), "fn () {}");
    assert_eq!(fifth.cursor(), (0, 3));
}

/// Arrow keys move every caret, not just the primary one.
#[test]
fn movement_applies_to_the_whole_caret_set() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    editor.move_cursor_with_selection(1, 0, false);
    let lines: Vec<usize> = editor.cursors().carets().iter().map(|caret| caret.head.line).collect();
    assert_eq!(lines, vec![1, 2], "both carets moved down");
    editor.move_word(1, false);
    assert_eq!(editor.caret().line, 2);
}

/// Home/End stop at the visual row under wrapping.
#[test]
fn home_and_end_follow_the_visual_row_when_wrapping() {
    let mut editor = make_with(CodeEditorConfig::new().word_wrap(true));
    let long = "a".repeat(200);
    editor.set_text(&long);
    editor.set_cursor(0, 0, false);
    editor.handle_key_press(35, 0);
    let column = editor.cursor().1;
    assert!(column > 0, "End reaches the end of the visible wrap segment");
    assert!(column < 200, "End does not jump to a column on another row");
}

/// The caret shape is selectable; the default stays the line.
#[test]
fn caret_style_is_configurable() {
    let mut editor = make();
    assert_eq!(editor.caret_style(), CaretStyle::Line);
    editor.set_caret_style(CaretStyle::Block);
    assert_eq!(editor.caret_style(), CaretStyle::Block);
    editor.set_caret_style(CaretStyle::Underline);
    assert_eq!(editor.caret_style(), CaretStyle::Underline);
}

// ── Find and completion ────────────────────────────────────────────────────

/// A fresh hit list is anchored to the caret instead of jumping to the top.
#[test]
fn find_anchors_its_first_hit_to_the_caret() {
    let mut editor = make();
    editor.set_text("x x x");
    editor.set_cursor(0, 2, false);
    editor.set_search_query("x");
    assert_eq!(editor.find_state().current, 1, "the hit at or after the caret is current");
}

/// Regex, selection-scoped search and the wrap notice are all implemented.
#[test]
fn find_supports_regex_selection_scope_and_wrap_notice() {
    let mut editor = make();
    editor.set_text("a1 b22 c333");
    let matches = editor.find_all_with(
        "\\d+",
        SearchOptions { use_regex: true, case_sensitive: true, ..SearchOptions::default() },
    );
    assert_eq!(matches.len(), 3);
    assert_eq!((matches[1].start_column, matches[1].end_column), (4, 6));

    let mut second = make();
    second.set_text("a1\nb2");
    second.set_caret(TextPosition::new(0, 0), false);
    second.set_caret(TextPosition::new(0, 2), true);
    let scoped = second.find_all_with(
        "2",
        SearchOptions { in_selection: true, case_sensitive: true, ..SearchOptions::default() },
    );
    assert!(scoped.is_empty(), "the hit outside the selection is excluded");

    let mut third = make();
    third.set_text("x x");
    third.set_cursor(0, 3, false);
    third.set_search_query("x");
    third.find_next(false);
    third.find_next(false);
    assert!(third.find_wrapped(), "navigating past the last hit wraps");
}

/// The built-in completion provider uses a revision-keyed index and scrolls.
#[test]
fn completion_uses_an_index_and_scrolls_a_capped_popup() {
    let mut editor = make_with(CodeEditorConfig::new().max_visible_completions(2));
    editor.set_text("alpha alphabet alphanumeric beta alphanumeric");
    editor.set_cursor(0, 3, false);
    let revision = editor.revision();
    let count = editor.trigger_completion();
    assert!(count >= 2, "the index offers the longer identifiers");
    assert_eq!(editor.completion_index_revision(), Some(revision));
    assert_eq!(editor.completion_visible_rows(), 2, "the popup is capped");
    editor.move_completion_selection(2);
    assert!(editor.completion_state().scroll > 0, "the selection scrolls into view");
}

/// Fuzzy completion finds a subsequence, and the ghost hint is exposed.
#[test]
fn completion_matches_subsequences_and_reports_the_hint() {
    let mut editor = make();
    editor.set_text("alphanumeric\nzzz");
    editor.set_cursor(1, 0, false);
    editor.input_text("aln");
    let count = editor.trigger_completion();
    assert!(count > 0, "a subsequence prefix still matches");
    assert!(editor.completion_state().items.iter().any(|item| item.label == "alphanumeric"));
    editor.set_inline_suggestion(Some("alphanumeric".to_string()));
    assert_eq!(editor.inline_suggestion(), Some("alphanumeric"));
}

// ── Folding, viewport and painting ────────────────────────────────────────

/// Fold-all, level folding and the cached bracket index.
#[test]
fn folding_covers_all_blocks_and_levels() {
    let mut editor = make();
    editor.set_text("fn a() {\n    x;\n}\nfn b() {\n    y;\n}");
    assert_eq!(editor.fold_all(), 2);
    assert!(editor.is_line_folded(0));
    assert!(editor.is_line_folded(3));
    assert!(editor.cached_bracket_pairs() > 0);
    editor.unfold_all();

    assert_eq!(editor.fold_level(0), 2, "both top-level blocks fold at level 0");
    assert!(editor.hidden_lines_at(0) > 0);
    let svg = render_to_svg(&mut make_with(CodeEditorConfig::new().show_fold_summary(true)));
    assert!(svg.starts_with("<svg"));

    // The fold reveal fade is opt-in and driven by `tick`.
    let mut animated = make_with(CodeEditorConfig::new().animate_folds(true));
    animated.set_text("fn a() {\n    x;\n}");
    animated.set_focused(false);
    assert!(animated.fold_block_at(0));
    assert!(animated.tick(16), "the fade is still running");
    assert!(animated.tick(200));
    assert!(!animated.tick(1), "the fade settles");
}

/// Diagnostics under wrapping rebuild their row table once per layout change.
#[test]
fn wrapped_diagnostics_reuse_their_row_table() {
    let mut editor = make_with(CodeEditorConfig::new().word_wrap(true));
    editor.set_text("first\nsecond\nthird");
    editor.set_markers(vec![
        super::types::DiagnosticMarker::new(1, "one", MarkerSeverity::Warning),
        super::types::DiagnosticMarker::new(2, "two", MarkerSeverity::Error),
    ]);
    let first = editor.inline_diagnostics();
    let second = editor.inline_diagnostics();
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert_eq!(editor.diagnostic_cache_builds.get(), 1, "the second call is a cache hit");
    editor.insert("x");
    editor.inline_diagnostics();
    assert_eq!(editor.diagnostic_cache_builds.get(), 2, "an edit invalidates the table");
}

/// Wrapped rows are measured, so the caret reads a real glyph position.
#[test]
fn wrapped_rows_are_measured() {
    let mut editor = make_with(CodeEditorConfig::new().word_wrap(true));
    editor.set_text("the quick brown fox jumps over the lazy dog ".repeat(4));
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
    assert!(editor.cached_x(0, 0).is_some(), "the row was measured");
    assert!(editor.cached_x(0, 1).is_some());
}

/// Smooth scrolling eases toward the caret instead of jumping.
#[test]
fn smooth_scrolling_eases_toward_the_caret() {
    let mut editor = make_with(CodeEditorConfig::new().smooth_scroll(true));
    editor.set_text(numbered(400));
    editor.base_mut().set_geometry(Rect::new(0, 0, 800, 200));
    editor.refresh_visible_rows();
    editor.goto_line(80);
    assert_eq!(editor.scroll_line(), 0, "the viewport has not jumped yet");
    for _ in 0..10 {
        editor.tick(16);
    }
    assert!(editor.scroll_line() > 0, "the viewport is catching up");
}

/// A blink tick reports the caret rectangle as its damage region.
#[test]
fn a_blink_tick_reports_a_damage_rectangle() {
    let mut editor = make();
    editor.set_text("text");
    let _ = editor.take_damage_rect();
    assert!(editor.tick(500), "the half period flips");
    let damage = editor.take_damage_rect().expect("the caret moved");
    assert!(damage.width <= 4, "the damage is the caret, not the widget");
    assert!(editor.take_damage_rect().is_none(), "taking it clears it");
}

/// The scrollbar track is a click target.
#[test]
fn clicking_the_scrollbar_track_scrolls() {
    let mut editor = make();
    editor.set_text(numbered(400));
    editor.base_mut().set_geometry(Rect::new(0, 0, 800, 200));
    editor.refresh_visible_rows();
    let rect = editor.base().geometry();
    let x = rect.x + rect.width as i32 - editor.scrollbar_width() - editor.minimap_width() + 2;
    let y = editor.text_origin_y() + 130;
    editor.pointer_press(Point::new(x, y));
    assert!(editor.scroll_line() > 0, "a track press scrolls to that fraction");
}

/// Gutter drags select whole lines and a gutter double-click folds.
#[test]
fn gutter_drag_and_double_click_select_and_fold() {
    let mut editor = make();
    editor.set_text("one\ntwo\nthree\nfour");
    let start = gutter_point(&editor, 0);
    editor.pointer_press(start);
    editor.pointer_drag(gutter_point(&editor, 2));
    let selected = editor.selected_text().unwrap_or_default();
    assert!(selected.starts_with("one\n"), "the first line is selected: {selected:?}");
    assert!(selected.contains("three"), "the drag reaches the third line");

    let mut second = make();
    second.set_text("fn a() {\n    x;\n}");
    let line = gutter_point(&second, 0);
    second.handle_event(&Event::MouseDoubleClick { pos: line, button: 1 });
    assert!(second.is_line_folded(0), "a gutter double-click folds the block");
}

/// Optional vector gutter marks and round severity dots render.
#[test]
fn optional_gutter_visuals_render() {
    let mut editor =
        make_with(CodeEditorConfig::new().vector_gutter_marks(true).round_severity_dots(true));
    editor.set_text("fn a() {\n    x;\n}");
    editor.fold_block_at(0);
    editor.set_markers(vec![super::types::DiagnosticMarker::new(1, "bad", MarkerSeverity::Error)]);
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

/// Overwrite mode replaces the character, and the remaining new APIs behave.
#[test]
fn the_remaining_public_state_apis_round_trip() {
    let mut editor = make();
    editor.set_text("abc");
    editor.set_cursor(0, 0, false);
    editor.set_overwrite_mode(true);
    editor.input_text("X");
    assert_eq!(editor.text(), "Xbc");
    editor.set_overwrite_mode(false);
    editor.input_text("Y");
    assert_eq!(editor.text(), "XYbc");

    editor.add_extra_selection(ExtraSelection::new(
        TextPosition::new(0, 0),
        TextPosition::new(0, 2),
        Color::RED,
    ));
    assert_eq!(editor.extra_selections().len(), 1);
    editor.clear_extra_selections();
    assert!(editor.extra_selections().is_empty());

    editor.set_text("a\nb\nc");
    editor.toggle_bookmark(0);
    editor.toggle_bookmark(2);
    assert_eq!(editor.bookmarks(), &[0, 2]);
    editor.set_cursor(0, 0, false);
    assert!(editor.goto_next_bookmark());
    assert_eq!(editor.cursor().0, 2);

    editor.set_undo_redo_enabled(false);
    assert!(!editor.undo_redo_enabled());
    editor.set_undo_redo_enabled(true);

    editor.set_placeholder_text(Some("start typing".to_string()));
    assert_eq!(editor.config().placeholder_text.as_deref(), Some("start typing"));
    let svg = render_to_svg(&mut make_with(CodeEditorConfig::new().placeholder_text("type")));
    assert!(svg.starts_with("<svg"));
}

/// The command table publishes the editing actions, not just `undo`.
#[test]
fn the_command_table_covers_the_editing_actions() {
    use crate::widget::capability::WidgetProperties;
    let mut editor = make();
    editor.set_text("one\ntwo");
    assert!(editor.command("select_all").is_ok());
    assert!(editor.has_selection());
    assert!(editor.command("find").is_ok());
    assert!(editor.find_visible());
    assert!(editor.command("close_find").is_ok());
    assert!(editor.command("unfold_all").is_ok());
    assert!(editor.command("no_such_command").is_err());
}
