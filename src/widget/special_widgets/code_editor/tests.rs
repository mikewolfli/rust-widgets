// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Behavioural test-suite for the [`CodeEditor`] widget.
//!
//! The module-level tests cover the widget as a whole: lifecycle, mutation,
//! history, multi-caret editing, auto-pairing, line commands, find/replace,
//! folding, tabs and rendering. Pure algorithms are tested next to their
//! implementation in the sibling modules.

use super::*;
use crate::core::{Point, Rect};
use crate::widget::svg::render_to_svg;

fn editor() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

fn editor_with(config: CodeEditorConfig) -> CodeEditor {
    CodeEditor::with_config(Rect::new(0, 0, 800, 600), config).expect("valid configuration")
}

// ── 1. Lifecycle & configuration ────────────────────────────────────────────

/// The caret blinks in an editable buffer and is steady in a read-only one.
///
/// A caret that never changed was indistinguishable from a frozen marker, which is why the
/// negative half of this test matters as much as the positive one: a read-only editor must not keep
/// asking its host for frames to animate a caret it never draws.
#[test]
fn the_caret_blinks_when_editable_and_not_when_read_only() {
    let mut editor = editor();

    assert!(editor.tick(0), "an editable buffer blinks its caret");
    assert!(editor.is_caret_visible(), "it starts visible");
    assert!(editor.tick(500), "a blink is periodic and never settles");
    assert!(!editor.is_caret_visible(), "the half-period boundary flips it");
    assert!(editor.tick(500));
    assert!(editor.is_caret_visible(), "and it flips back");

    editor.set_read_only(true);
    assert!(!editor.tick(500), "a read-only editor has no caret to animate");
}

#[test]
fn default_state_is_an_empty_untitled_buffer() {
    let editor = editor();
    assert_eq!(editor.text(), "");
    assert_eq!(editor.cursor(), (0, 0));
    assert_eq!(editor.line_count(), 1, "a document always owns one line");
    assert!(editor.markers().is_empty());
    assert_eq!(editor.active_buffer(), 0);
    assert_eq!(editor.title(), "untitled");
    assert!(!editor.find_visible());
    assert!(!editor.completion_state().visible);
    assert!(!editor.context_menu().visible);
    assert!(!editor.has_multiple_cursors());
    assert_eq!(editor.language_name(), "Plain Text");
}

#[test]
fn config_validation_rejects_impossible_values() {
    assert!(CodeEditorConfig::new().tab_width(0).validate().is_err());
    assert!(CodeEditorConfig::new().tab_width(99).validate().is_err());
    assert!(CodeEditorConfig::new().font_size(0.0).validate().is_err());
    assert!(CodeEditorConfig::new().font_size(f32::NAN).validate().is_err());
    assert!(CodeEditorConfig::new().font_family("").validate().is_err());
    assert!(CodeEditorConfig::new().space_advance(0.0).validate().is_err());
    assert!(CodeEditorConfig::new().font_size(20.0).line_advance(10.0).validate().is_err());
    assert!(CodeEditorConfig::new().column_ruler(20_000).validate().is_err());
    assert!(CodeEditorConfig::new().validate().is_ok());
}

#[test]
fn with_config_propagates_validation_errors() {
    let geometry = Rect::new(0, 0, 400, 300);
    assert!(CodeEditor::with_config(geometry, CodeEditorConfig::new().tab_width(0)).is_err());
    let editor =
        CodeEditor::with_config(geometry, CodeEditorConfig::new().language(LanguageId::Rust));
    assert!(editor.is_ok());
    assert_eq!(editor.expect("valid config").language_name(), "Rust");
}

#[test]
fn language_selection_drives_tab_width_and_keywords() {
    let mut editor = editor();
    editor.set_language(LanguageId::Python);
    assert_eq!(editor.tab_width(), LanguageId::Python.default_tab_width());
    assert!(editor.keywords().contains(&"def"));
    assert!(!editor.keywords().contains(&"fn"));
    editor.set_language(LanguageId::Rust);
    assert!(editor.keywords().contains(&"fn"));
}

#[test]
fn display_options_are_builder_settable() {
    let config =
        CodeEditorConfig::new().auto_close_brackets(false).show_whitespace(true).column_ruler(80);
    assert!(!config.auto_close_brackets);
    assert!(config.show_whitespace);
    assert_eq!(config.column_ruler, 80);
    assert!(config.validate().is_ok());
}

// ── 2. Text mutation ────────────────────────────────────────────────────────

#[test]
fn set_text_get_text_roundtrip() {
    let mut editor = editor();
    editor.set_text("hello world");
    assert_eq!(editor.text(), "hello world");
    editor.set_text("hello world");
    assert_eq!(editor.text(), "hello world");
}

#[test]
fn append_line_grows_document_and_moves_caret() {
    let mut editor = editor();
    editor.append_line("first");
    assert_eq!(editor.line_count(), 1);
    editor.append_line("second");
    assert_eq!(editor.line_count(), 2);
    assert_eq!(editor.text(), "first\nsecond");
    assert_eq!(editor.cursor().0, 1);
}

#[test]
fn insert_at_caret_splices_mid_line() {
    let mut editor = editor();
    editor.set_text("fn main() {}\n");
    editor.set_cursor(0, 11, false);
    editor.insert("let x = 1;");
    assert_eq!(editor.line_text(0).as_deref(), Some("fn main() {let x = 1;}"));
    assert_eq!(editor.cursor(), (0, 21));
}

#[test]
fn insert_with_indent_expands_tabs() {
    let mut editor = editor_with(CodeEditorConfig::new().tab_width(2).insert_spaces(true));
    editor.insert_with_indent("a\tb");
    assert_eq!(editor.text(), "a  b");
}

#[test]
fn read_only_blocks_every_mutation() {
    let mut editor = editor_with(CodeEditorConfig::new().read_only(true));
    editor.set_text("locked");
    let before = editor.text();
    editor.insert("x");
    editor.insert_newline();
    editor.backspace();
    editor.delete_forward();
    editor.insert_tab();
    editor.outdent();
    editor.indent_selection();
    editor.toggle_line_comment();
    editor.delete_selection();
    editor.move_line(1);
    editor.duplicate_line();
    editor.delete_line();
    editor.join_lines();
    editor.sort_lines();
    assert_eq!(editor.text(), before);
    assert!(!editor.undo());
    assert!(!editor.redo());
}

#[test]
fn backspace_and_delete_forward_handle_line_boundaries() {
    let mut editor = editor();
    editor.set_text("ab\ncd");
    editor.set_cursor(1, 0, false);
    editor.backspace();
    assert_eq!(editor.text(), "abcd");
    editor.set_cursor(0, 2, false);
    editor.delete_forward();
    assert_eq!(editor.text(), "abd");
    editor.set_cursor(0, 0, false);
    editor.backspace();
    assert_eq!(editor.text(), "abd", "backspace at the document start is a no-op");
}

#[test]
fn delete_word_backward_removes_whole_identifier() {
    let mut editor = editor();
    editor.set_text("let counter_value = 1;");
    editor.set_cursor(0, 18, false);
    editor.delete_word_backward();
    assert_eq!(editor.text(), "let = 1;");
}

#[test]
fn newline_auto_indents_and_expands_braces() {
    let mut editor = editor_with(CodeEditorConfig::new().tab_width(4).insert_spaces(true));
    editor.set_text("fn main() {");
    editor.set_cursor(0, 11, false);
    editor.insert_newline();
    assert_eq!(editor.text(), "fn main() {\n    ");
    assert_eq!(editor.cursor(), (1, 4));
}

#[test]
fn newline_between_braces_opens_a_block() {
    let mut editor = editor_with(CodeEditorConfig::new().tab_width(4).insert_spaces(true));
    editor.set_text("{}");
    editor.set_cursor(0, 1, false);
    editor.insert_newline();
    assert_eq!(editor.text(), "{\n    \n}");
    assert_eq!(editor.cursor(), (1, 4));
}

// ── 3. History ──────────────────────────────────────────────────────────────

#[test]
fn undo_redo_round_trips_insertions() {
    let mut editor = editor();
    editor.set_text("one");
    editor.append_line("two");
    let after = editor.text();
    assert!(editor.can_undo());
    assert!(editor.undo());
    assert_eq!(editor.text(), "one");
    assert!(editor.redo());
    assert_eq!(editor.text(), after);
}

#[test]
fn multi_caret_edit_is_one_undo_step() {
    let mut editor = editor();
    editor.set_text("a\nb\nc");
    editor.clear_history();
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    assert!(editor.add_cursor_below());
    editor.insert(">");
    assert_eq!(editor.text(), ">a\n>b\n>c");
    assert!(editor.undo());
    assert_eq!(editor.text(), "a\nb\nc");
    assert!(!editor.can_undo());
}

// ── 4. Multi-caret editing ──────────────────────────────────────────────────

#[test]
fn added_carets_track_the_primary_and_insert_everywhere() {
    let mut editor = editor();
    editor.set_text("a\nbb\nccc");
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    assert!(editor.add_cursor_below());
    assert_eq!(editor.cursors().len(), 3);
    assert!(editor.has_multiple_cursors());
    editor.insert("x");
    assert_eq!(editor.text(), "xa\nxbb\nxccc");
    assert_eq!(editor.cursors().len(), 3);
}

#[test]
fn select_all_occurrences_edits_every_match() {
    let mut editor = editor();
    editor.set_text("let a = 1;\nlet b = 1;\nlet c = 1;");
    editor.set_cursor(0, 0, false);
    editor.set_cursor(0, 8, false);
    editor.select_next_occurrence();
    let count = editor.select_all_occurrences();
    assert_eq!(count, 3);
    editor.insert("2");
    assert_eq!(editor.text(), "let a = 2;\nlet b = 2;\nlet c = 2;");
}

#[test]
fn escape_style_collapse_keeps_only_the_primary() {
    let mut editor = editor();
    editor.set_text("a\nb\nc");
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    assert!(editor.add_cursor_below());
    assert_eq!(editor.cursors().len(), 3);
    assert!(editor.collapse_cursors());
    assert_eq!(editor.cursors().len(), 1);
    assert!(!editor.has_multiple_cursors());
    assert!(!editor.collapse_cursors(), "collapsing again is a no-op");
}

#[test]
fn backspace_applies_at_every_caret() {
    let mut editor = editor();
    editor.set_text("xa\nxb\nxc");
    editor.set_cursor(0, 1, false);
    assert!(editor.add_cursor_below());
    assert!(editor.add_cursor_below());
    editor.backspace();
    assert_eq!(editor.text(), "a\nb\nc");
}

// ── 5. Auto-pairing ─────────────────────────────────────────────────────────

#[test]
fn typing_an_opener_inserts_the_pair_and_centres_the_caret() {
    let mut editor = editor();
    editor.input_text("(");
    assert_eq!(editor.text(), "()");
    assert_eq!(editor.cursor(), (0, 1));
}

#[test]
fn typing_a_closer_skips_over_the_existing_partner() {
    let mut editor = editor();
    editor.set_text("()");
    editor.set_cursor(0, 1, false);
    editor.input_text(")");
    assert_eq!(editor.text(), "()");
    assert_eq!(editor.cursor(), (0, 2));
}

#[test]
fn typing_an_opener_wraps_the_selection() {
    let mut editor = editor();
    editor.set_text("hello world");
    editor.set_cursor(0, 0, false);
    editor.move_cursor_with_selection(0, 5, true);
    editor.input_text("(");
    assert_eq!(editor.text(), "(hello) world");
    assert_eq!(editor.selected_text().as_deref(), Some("hello"));
}

#[test]
fn auto_pairing_can_be_disabled() {
    let mut editor = editor_with(CodeEditorConfig::new().auto_close_brackets(false));
    editor.input_text("(");
    assert_eq!(editor.text(), "(");
}

#[test]
fn backspace_removes_an_empty_pair_together() {
    let mut editor = editor();
    editor.set_text("()");
    editor.set_cursor(0, 1, false);
    editor.backspace();
    assert_eq!(editor.text(), "");
}

// ── 6. Line commands ────────────────────────────────────────────────────────

#[test]
fn duplicate_line_copies_below_and_selects_the_copy() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta");
    editor.set_cursor(0, 2, false);
    editor.duplicate_line();
    assert_eq!(editor.text(), "alpha\nalpha\nbeta");
    assert_eq!(editor.selected_text().as_deref(), Some("alpha"));
}

#[test]
fn delete_line_removes_the_caret_line() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta\ngamma");
    editor.set_cursor(1, 0, false);
    editor.delete_line();
    assert_eq!(editor.text(), "alpha\ngamma");
}

#[test]
fn join_lines_merges_with_the_following_line() {
    let mut editor = editor();
    editor.set_text("foo\n    bar");
    editor.set_cursor(0, 0, false);
    editor.join_lines();
    assert_eq!(editor.text(), "foo bar");
    assert_eq!(editor.cursor(), (0, 3));
}

#[test]
fn sort_lines_orders_the_selected_block() {
    let mut editor = editor();
    editor.set_text("c\na\nb\nd");
    editor.set_cursor(0, 0, false);
    editor.move_cursor_with_selection(2, 0, true);
    editor.sort_lines();
    assert_eq!(editor.text(), "a\nb\nc\nd");
}

#[test]
fn goto_line_moves_to_the_first_non_blank_column() {
    let mut editor = editor();
    editor.set_text("zero\n    two\nthree");
    editor.goto_line(1);
    assert_eq!(editor.cursor(), (1, 4));
    editor.goto_line(999);
    assert_eq!(editor.cursor(), (2, 0));
}

#[test]
fn trim_trailing_whitespace_cleans_every_line() {
    let mut editor = editor();
    editor.set_text("a  \nb\t\nc");
    editor.trim_trailing_whitespace();
    assert_eq!(editor.text(), "a\nb\nc");
}

// ── 7. Find & replace ───────────────────────────────────────────────────────

#[test]
fn find_bar_tracks_hits_and_navigates() {
    let mut editor = editor();
    editor.set_text("foo bar foo baz foo");
    editor.open_find(false);
    assert!(editor.find_visible());
    editor.set_search_query("foo");
    assert_eq!(editor.search_matches().len(), 3);
    editor.find_next(false);
    let first = editor.selected_text();
    editor.find_next(false);
    let second = editor.selected_text();
    assert_eq!(first, second);
    assert_eq!(first.as_deref(), Some("foo"));
    editor.close_find();
    assert!(!editor.find_visible());
}

#[test]
fn replace_all_rewrites_every_hit_in_one_step() {
    let mut editor = editor();
    editor.set_text("a-b-c");
    editor.open_find(true);
    editor.set_search_query("-");
    editor.set_search_replacement("+");
    let replaced = editor.replace_all();
    assert_eq!(replaced, 2);
    assert_eq!(editor.text(), "a+b+c");
    assert!(editor.undo());
    assert_eq!(editor.text(), "a-b-c");
}

// ── 8. Folding ──────────────────────────────────────────────────────────────

#[test]
fn folding_hides_interior_lines_and_unfolding_restores_them() {
    let mut editor = editor_with(CodeEditorConfig::new().language(LanguageId::Rust));
    editor.set_text("fn main() {\n    let x = 1;\n    let y = 2;\n}");
    editor.fold(0, 3);
    assert!(editor.is_line_folded(0));
    assert_eq!(editor.folded_line_count(), 3);
    editor.unfold_all();
    assert_eq!(editor.folded_line_count(), 0);
}

/// `is_line_folded` must agree with `folded_line_count` at any fold count.
///
/// They used to answer from *different* sources — this predicate scanned the
/// derived `folded_lines` cache while the count read the model. The two agree in
/// every configuration that actually compiles today, so this is a consistency
/// guard rather than a reproduction: it pins the invariant that the read paths
/// cannot diverge, and it exceeds the `MiniVec` capacity so the cache cannot hold
/// every folded start. If `code_editor` is ever enabled on an `alloc_frugal`
/// build, this is the test that would catch the drift first.
#[test]
fn folding_more_regions_than_the_cache_holds_stays_consistent() {
    let mut editor = editor_with(CodeEditorConfig::new().language(LanguageId::Rust));
    // 100 foldable regions, comfortably past the 64-element cache.
    let mut text = String::new();
    for i in 0..100 {
        text.push_str(&format!("fn f{i}() {{\n    let x = {i};\n}}\n"));
    }
    editor.set_text(&text);

    let last_line = editor.line_count().saturating_sub(1);
    let mut line = 0usize;
    while line < editor.line_count() {
        editor.fold(line, (line + 2).min(last_line));
        line += 3;
    }
    let folded = editor.fold_regions().iter().filter(|r| r.folded).count();
    assert!(folded > 64, "the test must exceed the 64-element cache, got {folded}");

    // Every folded region must be reported, including one the cache would have
    // had to drop first.
    let mut folded_starts: Vec<usize> =
        editor.fold_regions().iter().filter(|r| r.folded).map(|r| r.start_line).collect();
    folded_starts.sort_unstable();
    for start in [folded_starts[0], *folded_starts.last().expect("non-empty")] {
        assert!(
            editor.is_line_folded(start),
            "folded region at line {start} must be reported as folded"
        );
    }

    // A line that is NOT folded must not be reported as folded either — the
    // predicate has to stay exact, not just permissive.
    let unfolded = (0..editor.line_count()).find(|l| !folded_starts.contains(l));
    if let Some(line) = unfolded {
        assert!(!editor.is_line_folded(line));
    }

    editor.unfold_all();
    assert_eq!(editor.folded_line_count(), 0);
    assert!(!editor.is_line_folded(folded_starts[0]));
}

// ── 9. Tabs ─────────────────────────────────────────────────────────────────

#[test]
fn buffers_open_switch_and_close() {
    let mut editor = editor();
    let first = editor.active_buffer();
    let second = editor.open_buffer("second.rs", "fn b() {}");
    assert_eq!(editor.active_buffer(), second);
    assert_eq!(editor.text(), "fn b() {}");
    assert!(editor.activate_buffer(first));
    assert_eq!(editor.text(), "");
    assert!(editor.close_buffer(second));
    assert_eq!(editor.buffers().len(), 1);
}

// ── 10. Rendering ───────────────────────────────────────────────────────────

#[test]
fn draw_produces_svg_output() {
    let mut editor = editor();
    editor.set_text("fn main() {\n    let x = 1;\n}");
    editor.set_markers(vec![DiagnosticMarker::new(1, "unused", MarkerSeverity::Warning)]);
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
    assert!(svg.len() > 100);
}

#[test]
fn draw_with_multiple_cursors_stays_well_formed() {
    let mut editor = editor();
    editor.set_text("alpha\nbeta\ngamma");
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    assert!(editor.add_cursor_below());
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn whitespace_and_ruler_overlays_render_without_panicking() {
    let mut editor = editor_with(CodeEditorConfig::new().show_whitespace(true).column_ruler(10));
    editor.set_text("let x = 1;\n\tlet y = 2;");
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn rect_for_range_is_available_for_a_valid_selection() {
    let editor = editor();
    let rect = editor.rect_for_range(TextPosition::new(0, 0), TextPosition::new(0, 5));
    assert!(rect.is_some());
}

#[test]
fn position_at_point_is_inside_the_document() {
    let editor = editor();
    let position = editor.position_at_point(Point::new(200, 120));
    assert!(position.is_some());
}
