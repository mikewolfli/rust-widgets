// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Regression tests for the §11.4 findings of `issue-20261106.md`.
//!
//! Each test pins one gap the full-file read found: a locked replace that reported
//! success, an incremental search that ignored a selection limit, a gesture latch
//! that outlived its window, visual rows measured in cells instead of characters,
//! and the lexer's nested-comment and triple-quote handling.

// The module-level gate, so tooling that scans file text recognises this as a test module.
#![cfg(test)]

use super::editor::CodeEditor;
use super::syntax::{BuiltinHighlighter, LanguageId, LineState, SyntaxHighlighter};
use super::types::{CodeEditorConfig, ReadOnlySpan, SearchOptions, TextPosition, TokenKind};
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use crate::widget::Widget;
use alloc::string::ToString;
use alloc::vec::Vec;
use std::sync::{Arc, Mutex};

fn make() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

// ── Find / replace ──────────────────────────────────────────────────────────

#[test]
fn a_locked_hit_refuses_replace_current_without_touching_the_text() {
    let mut editor = make();
    editor.set_text("a\nb\n");
    editor.add_read_only_span(ReadOnlySpan::lines(0, 0));
    editor.set_search_query("a");
    editor.set_search_replacement("z");

    assert!(!editor.replace_current(), "a locked hit must report the refusal");
    assert_eq!(editor.text(), "a\nb\n", "nothing was written");
}

#[test]
fn replace_all_refuses_a_span_crossing_a_locked_line() {
    let mut editor = make();
    editor.set_text("a\nlocked\na\n");
    // The lock sits *between* the two hits, so each hit is editable on its own; the
    // batch's diff span still crosses it, which is the case that used to report two
    // replacements while the document stayed unchanged.
    editor.add_read_only_span(ReadOnlySpan::lines(1, 1));
    editor.set_search_query("a");
    editor.set_search_replacement("z");

    assert_eq!(editor.replace_all(), 0, "the refused batch reports no replacements");
    assert_eq!(editor.text(), "a\nlocked\na\n");
}

#[test]
fn an_edit_republishes_the_match_count() {
    let mut editor = make();
    editor.set_text("foo foo");
    let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = Arc::clone(&seen);
    editor.search_changed.connect(move |count| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*count);
        }
    });
    editor.set_search_query("foo");
    if let Ok(mut guard) = seen.lock() {
        guard.clear();
    }

    // Splitting the first `foo` leaves one hit; the navigation list changed even
    // though the query did not.
    editor.set_cursor(0, 1, false);
    editor.insert("x");
    let counts = seen.lock().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(counts.last().copied(), Some(1), "the edit publishes the new count");
}

#[test]
fn an_incremental_recompute_keeps_the_selection_limit() {
    let mut editor = make();
    editor.set_text("foo bar\nfoo baz\n");
    editor.set_search_options(SearchOptions { in_selection: true, ..SearchOptions::default() });
    // Select the first line only, then edit inside it. The incremental path used to
    // splice hits back in without re-applying `in_selection`, so the second line's
    // hit resurfaced after the keystroke.
    editor.set_cursor(0, 0, false);
    editor.set_cursor(0, 7, true);
    editor.set_search_query("foo");
    assert_eq!(editor.search_matches().len(), 1);

    editor.set_cursor(0, 0, false);
    editor.insert("f");
    assert!(
        editor.search_matches().iter().all(|hit| hit.line == 0),
        "a selection-limited search must not gain hits from outside the selection"
    );
}

// ── Pointer latches ─────────────────────────────────────────────────────────

#[test]
fn losing_focus_drops_a_pending_selection_drag() {
    let mut editor = make();
    editor.set_text("hello world\nsecond line\n");
    editor.refresh_visible_rows();
    editor.pointer_press_with_modifiers(Point::new(20, 50), 0);
    let pressed = editor.cursor();
    editor.handle_event(&Event::FocusLost);
    editor.pointer_drag(Point::new(400, 50));
    assert_eq!(editor.cursor(), pressed, "a motion after focus loss must not extend the drag");
}

// ── Viewport and coordinates ────────────────────────────────────────────────

#[test]
fn scroll_by_clamps_in_visual_rows_when_a_line_wraps() {
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 260, 120), CodeEditorConfig::new().word_wrap(true))
            .expect("valid config");
    editor.set_text("x".repeat(4000));
    editor.refresh_visible_rows();
    let total = editor.visual_row_count();
    assert!(total > editor.visible_rows(), "the wrapped line exceeds one viewport");
    editor.scroll_by(5);
    assert_eq!(editor.first_visual_row(), 5, "a single wrapped line can still scroll");
}

#[test]
fn visual_lines_report_character_columns_for_wide_text() {
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 120, 200), CodeEditorConfig::new().word_wrap(true))
            .expect("valid config");
    editor.set_text("日本語のテキストを折り返します".to_string());
    editor.refresh_visible_rows();
    let line_chars = editor.line_len(0);
    let lines = editor.visual_lines();
    assert!(lines.len() > 1);
    for line in &lines {
        assert!(
            line.start_column <= line_chars && line.end_column <= line_chars,
            "a visual row must name characters, not grid cells: {line:?}"
        );
        assert!(line.end_column >= line.start_column);
    }
}

#[test]
fn a_multi_row_range_starts_at_the_text_origin() {
    let mut editor = make();
    editor.set_text("abcdef\nghij\n");
    editor.refresh_visible_rows();
    let rect = editor
        .rect_for_range(TextPosition::new(0, 3), TextPosition::new(1, 2))
        .expect("a valid range");
    let origin = editor.geometry().x + editor.text_origin_x();
    assert_eq!(rect.x, origin, "a row-spanning band covers whole rows");
}

// ── Lexer ───────────────────────────────────────────────────────────────────

#[test]
fn a_nested_rust_block_comment_needs_two_closers() {
    let highlighter = BuiltinHighlighter::new(LanguageId::Rust);
    let (_, state) = highlighter.highlight_line("/* outer /* inner", LineState::Clean);
    assert_eq!(state, LineState::NestedBlockComment { depth: 2 });

    let (spans, state) = highlighter.highlight_line("still inner */ still outer", state);
    assert_eq!(state, LineState::BlockComment, "one closer leaves the outer level open");
    assert!(spans.iter().all(|span| span.kind == TokenKind::BlockComment));

    let (_, state) = highlighter.highlight_line("*/ let x = 1;", state);
    assert_eq!(state, LineState::Clean, "the second closer ends the comment");
}

#[test]
fn a_python_triple_quote_ignores_an_escaped_delimiter() {
    let highlighter = BuiltinHighlighter::new(LanguageId::Python);
    // `\"""` does not end the string; the final `"""` does.
    let source = "s = \"\"\"a \\\"\"\" b\"\"\"";
    let (spans, state) = highlighter.highlight_line(source, LineState::Clean);
    assert_eq!(state, LineState::Clean, "the escaped delimiter does not close the string");
    assert_eq!(spans.last().map(|span| span.kind), Some(TokenKind::String));
    assert_eq!(spans.last().map(|span| span.end), Some(source.len()));
}
