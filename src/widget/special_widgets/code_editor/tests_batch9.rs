// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the host seams requested in `feedback-20261101.md` §R2.3 and RW-61.
//!
//! Each test pins one behaviour a host previously had to reimplement: snippet
//! sessions, structural selection, inline overlays, bookmark/navigation
//! lifecycles, the inline-diff baseline and the context-menu request signal.

// The module-level gate, so tooling that scans file text recognises this as a test module.
#![cfg(test)]

use super::editor::CodeEditor;
use super::overlays::{CodeLens, InlayHint, InlayHintKind};
use super::syntax::{LineState, StructuralKind, SyntaxHighlighter};
use super::types::{CodeEditorConfig, TextPosition, TokenSpan};
use crate::core::{Point, Rect};
use crate::event::{mouse_button, Event, EventHandler};
use crate::widget::svg::{render_to_svg, text_ink_boxes};
use crate::widget::Widget;
use alloc::vec;
use alloc::vec::Vec;
use std::sync::Arc;
use std::sync::Mutex;

fn make() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

fn make_with(config: CodeEditorConfig) -> CodeEditor {
    CodeEditor::with_config(Rect::new(0, 0, 800, 600), config).expect("valid configuration")
}

// ── RW-19: snippet sessions ─────────────────────────────────────────────────

#[test]
fn a_snippet_session_selects_the_first_placeholder_and_tabs_through_the_stops() {
    let mut editor = make();
    let session = editor.insert_snippet_session("let ${1:x} = $2; $0").expect("a session");
    assert_eq!(editor.text(), "let x = ; ");
    assert_eq!(session.len(), 3);
    assert_eq!(session.current(), 0);
    assert!(session.stops()[0].is_placeholder());
    assert_eq!(editor.selected_text().as_deref(), Some("x"));
    assert!(editor.is_in_snippet());

    assert!(editor.next_placeholder());
    assert_eq!(editor.cursor(), (0, 8), "the bare $2 stop is the caret");
    assert!(!editor.has_selection());

    assert!(editor.next_placeholder());
    assert_eq!(editor.cursor(), (0, 10), "the final $0 stop");
    assert!(!editor.next_placeholder(), "past the final stop the session ends");
    assert!(!editor.is_in_snippet());
}

#[test]
fn a_placeholder_edit_translates_the_later_stops() {
    let mut editor = make();
    editor.insert_snippet_session("let ${1:x} = $2;").expect("a session");
    editor.insert("count");
    assert_eq!(editor.text(), "let count = ;");
    assert!(editor.is_in_snippet(), "typing into a placeholder keeps the session alive");
    assert!(editor.next_placeholder());
    assert_eq!(editor.cursor(), (0, 12), "the following stop moved with the text");
}

#[test]
fn previous_placeholder_walks_back_and_stops_at_the_first() {
    let mut editor = make();
    editor.insert_snippet_session("a ${1:one} b ${2:two}").expect("a session");
    assert!(editor.next_placeholder());
    assert_eq!(editor.selected_text().as_deref(), Some("two"));
    assert!(editor.previous_placeholder());
    assert_eq!(editor.selected_text().as_deref(), Some("one"));
    assert!(!editor.previous_placeholder(), "the first stop has no predecessor");
}

#[test]
fn cancelling_a_snippet_keeps_the_inserted_text() {
    let mut editor = make();
    editor.insert_snippet_session("fn ${1:name}() {}").expect("a session");
    assert!(editor.cancel_snippet_session());
    assert!(!editor.is_in_snippet());
    assert_eq!(editor.text(), "fn name() {}");
    assert!(!editor.cancel_snippet_session(), "there is nothing left to cancel");
}

#[test]
fn a_marker_free_snippet_inserts_and_reports_an_empty_session() {
    let mut editor = make();
    let session = editor.insert_snippet_session("plain text").expect("text is inserted");
    assert!(session.is_empty());
    assert_eq!(editor.text(), "plain text");
    assert!(!editor.next_placeholder());
    assert!(!editor.is_in_snippet());
}

// ── RW-20: structural selection ─────────────────────────────────────────────

#[test]
fn expand_selection_walks_from_word_to_brackets_to_block() {
    let mut editor = make();
    editor.set_text("fn main() {\n    let x = (1 + 2);\n}");
    editor.set_cursor(1, 13, false);

    assert!(editor.expand_selection(StructuralKind::Word));
    assert_eq!(editor.selected_text().as_deref(), Some("1"));

    assert!(editor.expand_selection(StructuralKind::Bracket));
    assert_eq!(editor.selected_text().as_deref(), Some("(1 + 2)"));

    assert!(editor.expand_selection(StructuralKind::Bracket));
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("{\n    let x = (1 + 2);\n}"),
        "the second expansion selects the enclosing block"
    );
}

#[test]
fn expand_selection_selects_the_line_and_the_paragraph() {
    let mut editor = make();
    editor.set_text("first\n\nsecond line\ncontinued\n\nthird");
    editor.set_cursor(2, 3, false);

    assert!(editor.expand_selection(StructuralKind::Line));
    assert_eq!(editor.selected_text().as_deref(), Some("second line\n"));

    editor.set_cursor(3, 3, false);
    assert!(editor.expand_selection(StructuralKind::Block));
    assert_eq!(editor.selected_text().as_deref(), Some("second line\ncontinued\n"));
}

#[test]
fn a_highlighter_can_supply_a_finer_word_range() {
    struct NarrowWord;

    impl SyntaxHighlighter for NarrowWord {
        fn highlight_line(&self, _line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
            (Vec::new(), state)
        }

        fn selection_ranges(
            &self,
            _text: &str,
            _position: TextPosition,
            kind: StructuralKind,
        ) -> Vec<(TextPosition, TextPosition)> {
            if kind == StructuralKind::Word {
                vec![(TextPosition::new(1, 5), TextPosition::new(1, 6))]
            } else {
                Vec::new()
            }
        }

        fn language_name(&self) -> &str {
            "Narrow"
        }
    }

    let mut editor = make();
    editor.set_text("fn main() {\n    let x = 1;\n}");
    editor.set_highlighter(Box::new(NarrowWord));
    editor.set_cursor(1, 5, false);
    assert!(editor.expand_selection(StructuralKind::Word));
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("e"),
        "the plugin's range is preferred over the textual word"
    );
}

// ── RW-21: inlay hints and code lenses ──────────────────────────────────────

#[test]
fn inlay_hints_and_code_lenses_round_trip_and_render() {
    let mut editor = make();
    editor.set_text("let answer = 42;");
    editor.set_inlay_hints(vec![
        InlayHint::new(0, 12, ": i32").with_kind(InlayHintKind::Type),
        InlayHint::new(0, 3, "name"),
    ]);
    editor.set_code_lenses(vec![CodeLens::new(0, "2 references", "references")]);

    let hints = editor.inlay_hints();
    assert_eq!(hints.len(), 2);
    assert_eq!((hints[0].column, hints[0].line), (3, 0), "hints are sorted by column");
    assert!(editor.inlay_hints_enabled());
    assert_eq!(editor.code_lenses().len(), 1);

    let svg = render_to_svg(&mut editor);
    let with_hints = text_ink_boxes(&svg).len();
    assert!(with_hints > 0, "the line has text ink");

    editor.set_inlay_hints_enabled(false);
    assert!(!editor.inlay_hints_enabled());
    let svg = render_to_svg(&mut editor);
    let without_hints = text_ink_boxes(&svg).len();
    assert!(
        with_hints >= without_hints + 2,
        "both hints are painted as extra text runs: {with_hints} vs {without_hints}"
    );

    // The lens is independent of the hint toggle; dropping it removes exactly one run.
    editor.set_code_lenses(Vec::new());
    let without_lens = text_ink_boxes(&render_to_svg(&mut editor)).len();
    assert_eq!(without_lens + 1, without_hints, "the lens is painted");
}

/// RW-115: hiding code lenses keeps their data and only changes the paint.
#[test]
fn code_lenses_can_be_hidden_without_losing_them() {
    let mut editor = make();
    editor.set_text("let answer = 42;");
    editor.set_code_lenses(vec![CodeLens::new(0, "2 references", "references")]);
    assert!(editor.code_lenses_enabled());
    let visible_runs = text_ink_boxes(&render_to_svg(&mut editor)).len();

    editor.set_code_lenses_enabled(false);
    assert!(!editor.code_lenses_enabled());
    assert_eq!(editor.code_lenses().len(), 1, "the data is not dropped");
    let hidden_runs = text_ink_boxes(&render_to_svg(&mut editor)).len();
    assert_eq!(hidden_runs + 1, visible_runs, "the lens text is not painted");

    editor.set_code_lenses_enabled(true);
    let shown_runs = text_ink_boxes(&render_to_svg(&mut editor)).len();
    assert_eq!(shown_runs, visible_runs, "the same data comes back");
}

// ── RW-22: bookmark lifecycle ───────────────────────────────────────────────

#[test]
fn bookmark_set_is_clamped_sorted_and_deduplicated() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_bookmarks(vec![2, 0, 2, 99]);
    assert_eq!(editor.bookmarks(), &[0, 2]);
    editor.clear_bookmarks();
    assert!(editor.bookmarks().is_empty());
}

#[test]
fn bookmarks_and_breakpoints_move_with_an_edit() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_bookmarks(vec![0, 2]);
    editor.add_breakpoint(2);

    editor.set_cursor(0, 0, false);
    editor.insert_newline();
    assert_eq!(editor.text(), "\na\nb\nc");
    assert_eq!(editor.bookmarks(), &[0, 3], "the mark below the insertion moved");
    assert_eq!(editor.breakpoints(), &[3], "breakpoints use the same translation");
}

#[test]
fn previous_bookmark_walks_backwards_and_wraps() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_bookmarks(vec![0, 2]);
    editor.set_cursor(2, 0, false);
    assert!(editor.previous_bookmark());
    assert_eq!(editor.cursor(), (0, 0));
    assert!(editor.previous_bookmark(), "from the first mark it wraps to the last");
    assert_eq!(editor.cursor(), (2, 0));

    editor.clear_bookmarks();
    assert!(!editor.previous_bookmark());
}

// ── RW-23: navigation stack ─────────────────────────────────────────────────

#[test]
fn navigation_history_walks_back_and_forward() {
    let mut editor = make();
    editor.set_text("a\nb\nc");

    editor.set_cursor(0, 0, false);
    assert!(editor.push_navigation_position());
    editor.set_cursor(1, 0, false);
    assert!(editor.push_navigation_position());
    editor.set_cursor(2, 0, false);
    assert!(editor.push_navigation_position());
    assert_eq!(editor.navigation_history_len(), 3);

    assert!(editor.go_back());
    assert_eq!(editor.cursor(), (1, 0));
    assert!(editor.go_back());
    assert_eq!(editor.cursor(), (0, 0));
    assert!(!editor.go_back());

    assert!(editor.go_forward());
    assert_eq!(editor.cursor(), (1, 0));
    assert!(editor.go_forward());
    assert_eq!(editor.cursor(), (2, 0));
    assert!(!editor.go_forward());
}

#[test]
fn the_navigation_history_is_trimmed_and_branches_are_discarded() {
    let mut editor = make();
    editor.set_text("a\nb");
    for index in 0..(super::editor::MAX_NAVIGATION_HISTORY + 10) {
        editor.set_cursor(index % 2, 0, false);
        editor.push_navigation_position();
    }
    assert_eq!(editor.navigation_history_len(), super::editor::MAX_NAVIGATION_HISTORY);

    let mut editor = make();
    editor.set_text("a\nb");
    editor.set_cursor(0, 0, false);
    editor.push_navigation_position();
    editor.set_cursor(1, 0, false);
    editor.push_navigation_position();
    assert!(editor.go_back());
    editor.set_cursor(1, 0, false);
    editor.push_navigation_position();
    assert!(!editor.go_forward(), "a push after going back discards the forward branch");
}

// ── RW-24: public surround and inline diff ──────────────────────────────────

#[test]
fn wrap_selection_is_public_and_keeps_the_inner_text_selected() {
    let mut editor = make();
    editor.set_text("hello");
    editor.set_cursor(0, 0, false);
    editor.set_cursor(0, 5, true);
    editor.wrap_selection('(', ')');
    assert_eq!(editor.text(), "(hello)");
    assert_eq!(editor.selected_text().as_deref(), Some("hello"));
}

#[test]
fn the_inline_diff_names_the_changed_lines_and_the_baseline_round_trips() {
    let mut editor = make_with(CodeEditorConfig::new().show_inline_diff(true));
    editor.set_text("a\nc");
    assert!(editor.show_inline_diff());
    assert!(editor.diff_baseline().is_none());
    assert!(editor.inline_diff_lines().is_empty());

    editor.set_diff_baseline(Some("a\nb"));
    assert_eq!(editor.diff_baseline(), Some("a\nb"));
    assert_eq!(editor.inline_diff_lines(), vec![1]);

    editor.set_show_inline_diff(false);
    assert!(!editor.show_inline_diff());

    editor.set_diff_baseline(None);
    assert!(editor.inline_diff_lines().is_empty());

    // The paint pass agrees with the predicate: it renders with a tint and does not panic.
    editor.set_show_inline_diff(true);
    editor.set_diff_baseline(Some("a\nb"));
    let svg = render_to_svg(&mut editor);
    assert!(svg.starts_with("<svg"));
}

// ── RW-61: context-menu request signal ──────────────────────────────────────

#[test]
fn a_secondary_click_announces_the_request_before_the_built_in_menu() {
    let mut editor = make();
    editor.set_text("text");
    let seen = Arc::new(Mutex::new(Vec::<Point>::new()));
    let sink = Arc::clone(&seen);
    editor.context_menu_requested.connect(move |position| {
        if let Ok(mut slot) = sink.lock() {
            slot.push(*position);
        }
    });

    editor.handle_event(&Event::mouse_press(200, 120, mouse_button::SECONDARY));
    assert_eq!(*seen.lock().unwrap(), vec![Point::new(200, 120)]);
    assert!(editor.context_menu().visible, "the default still opens the built-in menu");
}

#[test]
fn a_host_menu_can_suppress_the_built_in_one_and_still_receive_the_request() {
    let mut editor = make_with(CodeEditorConfig::new().built_in_context_menu(false));
    editor.set_text("fn main() {}\n");
    let before = editor.text();
    let seen = Arc::new(Mutex::new(Vec::<Point>::new()));
    let sink = Arc::clone(&seen);
    editor.context_menu_requested.connect(move |position| {
        if let Ok(mut slot) = sink.lock() {
            slot.push(*position);
        }
    });

    editor.handle_event(&Event::mouse_press(40, 50, mouse_button::SECONDARY));
    assert_eq!(*seen.lock().unwrap(), vec![Point::new(40, 50)]);
    assert!(!editor.context_menu().visible, "the built-in menu is suppressed");

    assert!(
        Widget::context_menu_event(&mut editor, Point::new(7, 9)),
        "the platform's synthesized request is always handled"
    );
    assert_eq!(*seen.lock().unwrap(), vec![Point::new(40, 50), Point::new(7, 9)]);
    assert!(!editor.context_menu().visible);
    assert_eq!(editor.text(), before, "the request never mutates the document");
}
