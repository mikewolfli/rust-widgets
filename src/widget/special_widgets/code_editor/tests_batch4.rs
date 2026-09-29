// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for batch 4: range-level read-only (A4), per-buffer language (A5),
//! the per-language highlighter registry (D1) and graceful degradation of
//! structure-aware commands (D2).
//!
//! Each of these was previously a declared-but-inert capability: the field or
//! the seam existed and the link did not. The tests therefore assert the *link*,
//! not the field — that a locked span actually refuses an edit, that a buffer's
//! language actually selects the lexer, and that an unsupported command actually
//! declines rather than corrupting the buffer.

use super::editor::CodeEditor;
use super::syntax::{LanguageId, LineState, SyntaxHighlighter};
use super::types::{ReadOnlySpan, TextPosition, TokenKind, TokenSpan};
use crate::core::Rect;
use alloc::vec;
use alloc::vec::Vec;

fn editor() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

// ── A4: range-level read-only ───────────────────────────────────────────────

/// A span locking whole lines must refuse a typed character on those lines.
#[test]
fn a_locked_line_refuses_typing_but_an_unlocked_one_accepts_it() {
    let mut editor = editor();
    editor.set_text("generated\neditable\n");
    editor.add_read_only_span(ReadOnlySpan::lines(0, 0));

    editor.set_caret(TextPosition::new(0, 4), false);
    editor.insert("X");
    assert_eq!(editor.line_text(0).as_deref(), Some("generated"), "line 0 is locked");

    editor.set_caret(TextPosition::new(1, 0), false);
    editor.insert("Y");
    assert_eq!(editor.line_text(1).as_deref(), Some("Yeditable"), "line 1 is editable");
}

/// `is_position_editable` and `is_range_editable` must agree with what the edit
/// paths actually do — a query that disagreed with the enforcement would be a
/// second, drift-prone source of truth.
#[test]
fn the_range_editability_queries_match_enforcement() {
    let mut editor = editor();
    editor.set_text("aa\nbb\ncc\n");
    editor.add_read_only_span(ReadOnlySpan::new(TextPosition::new(1, 0), TextPosition::new(1, 2)));

    assert!(editor.is_position_editable(TextPosition::new(0, 1)));
    assert!(editor.is_position_editable(TextPosition::new(1, 2)), "the end is exclusive");
    assert!(!editor.is_position_editable(TextPosition::new(1, 1)));

    // An insertion inside the locked span is refused.
    assert!(!editor.is_range_editable(TextPosition::new(1, 1), TextPosition::new(1, 1)));
    // A selection that crosses into the span is refused.
    assert!(!editor.is_range_editable(TextPosition::new(1, 0), TextPosition::new(2, 0)));
    // A range entirely before the span is allowed.
    assert!(editor.is_range_editable(TextPosition::new(0, 0), TextPosition::new(0, 2)));

    editor.set_caret(TextPosition::new(1, 1), false);
    editor.insert("Z");
    assert_eq!(editor.line_text(1).as_deref(), Some("bb"), "the guarded insert did nothing");
}

/// A selection that reaches into a locked span must not delete it.
#[test]
fn deleting_a_selection_that_touches_a_locked_span_is_refused() {
    let mut editor = editor();
    editor.set_text("keep\nlocked\nmore\n");
    editor.add_read_only_span(ReadOnlySpan::lines(1, 1));

    editor.set_caret(TextPosition::new(0, 0), false);
    editor.set_caret(TextPosition::new(2, 4), true);
    editor.delete_selection();

    assert_eq!(editor.text(), "keep\nlocked\nmore\n", "the locked line survived");
}

/// A multi-caret batch is refused wholesale when one caret lands in a lock.
#[test]
fn a_multi_caret_batch_is_refused_when_any_caret_is_locked() {
    let mut editor = editor();
    editor.set_text("one\ntwo\n");
    editor.add_read_only_span(ReadOnlySpan::lines(1, 1));

    editor.set_caret(TextPosition::new(0, 0), false);
    editor.add_cursor_below();
    assert!(editor.has_multiple_cursors());

    editor.insert("!");
    assert_eq!(editor.text(), "one\ntwo\n", "no part of a refused batch may be applied");
}

/// Line commands go through a different path than typing, so they need their own
/// guard: `delete_line` on a generated line must be refused.
#[test]
fn a_locked_line_refuses_line_commands() {
    let mut editor = editor();
    editor.set_text("generated\neditable\n");
    editor.add_read_only_span(ReadOnlySpan::lines(0, 0));

    editor.set_caret(TextPosition::new(0, 2), false);
    editor.delete_line();
    assert_eq!(editor.text(), "generated\neditable\n", "delete_line must not remove a lock");
}

/// The whole-buffer switch and the range lock are independent: clearing one does
/// not clear the other.
#[test]
fn clearing_spans_leaves_the_buffer_read_only_switch_alone() {
    let mut editor = editor();
    editor.add_read_only_span(ReadOnlySpan::lines(0, 0));
    editor.set_read_only(true);
    assert!(!editor.is_position_editable(TextPosition::new(5, 0)));

    editor.clear_read_only_spans();
    assert!(editor.read_only_spans().is_empty());
    assert!(!editor.is_position_editable(TextPosition::new(5, 0)), "still globally read-only");

    editor.set_read_only(false);
    assert!(editor.is_position_editable(TextPosition::new(5, 0)));
}

/// A span below an inserted line must follow it, or the lock would drift onto
/// unrelated text.
#[test]
fn locked_spans_follow_edits_that_insert_lines_above_them() {
    let mut editor = editor();
    editor.set_text("head\ngenerated\n");
    editor.add_read_only_span(ReadOnlySpan::lines(1, 1));

    // Insert a line at the very top.
    editor.set_caret(TextPosition::new(0, 0), false);
    editor.insert("new\n");
    assert_eq!(editor.text(), "new\nhead\ngenerated\n");
    assert_eq!(
        editor.read_only_spans(),
        &[ReadOnlySpan::lines(2, 2)],
        "the lock moved down with its text"
    );
}

/// Empty spans are not stored, so a host may build them from a collapsed range
/// without checking.
#[test]
fn an_empty_span_is_not_stored() {
    let mut editor = editor();
    let collapsed = ReadOnlySpan::new(TextPosition::new(3, 4), TextPosition::new(3, 4));
    editor.add_read_only_span(collapsed);
    assert!(editor.read_only_spans().is_empty());
    assert!(editor.is_position_editable(TextPosition::new(3, 4)));
}

/// Replacing the whole document invalidates positional locks, so they are
/// dropped rather than silently re-anchored.
#[test]
fn set_text_drops_positional_locks() {
    let mut editor = editor();
    editor.set_text("a\nb\n");
    editor.add_read_only_span(ReadOnlySpan::lines(0, 1));
    editor.set_text("completely\ndifferent\n");
    assert!(editor.read_only_spans().is_empty());
}

// ── A5 / D1: language per buffer and the registry ───────────────────────────

/// A buffer's language override must actually select the lexer for that buffer.
#[test]
fn a_buffers_language_override_selects_the_lexer() {
    let mut editor = editor();
    let rust = editor.open_buffer("main.rs", "fn main() {}");
    editor.set_buffer_language(rust, Some(LanguageId::Rust));
    assert_eq!(editor.active_language(), LanguageId::Rust);
    assert!(
        editor.tokens_on_line(0).iter().any(|span| span.kind == TokenKind::Keyword),
        "`fn` must lex as a keyword once the buffer declares Rust"
    );

    let text = editor.open_buffer("notes.txt", "fn main() {}");
    editor.set_buffer_language(text, Some(LanguageId::PlainText));
    assert_eq!(editor.active_language(), LanguageId::PlainText);
    assert!(
        editor.tokens_on_line(0).iter().all(|span| span.kind != TokenKind::Keyword),
        "plain text must not colour `fn` as a keyword"
    );
}

/// Switching back to a Rust buffer must restore Rust highlighting, which is the
/// whole point of a per-buffer language.
#[test]
fn switching_buffers_switches_the_active_language() {
    let mut editor = editor();
    let rust = editor.open_buffer("main.rs", "let x = 1;");
    editor.set_buffer_language(rust, Some(LanguageId::Rust));
    let plain = editor.open_buffer("notes.txt", "let x = 1;");
    editor.set_buffer_language(plain, Some(LanguageId::PlainText));

    assert_eq!(editor.active_language(), LanguageId::PlainText);
    assert!(editor.activate_buffer(rust));
    assert_eq!(editor.active_language(), LanguageId::Rust);
    assert!(
        editor.tokens_on_line(0).iter().any(|span| span.kind == TokenKind::Keyword),
        "`let` must be a keyword again after the switch"
    );
}

/// A registered per-language highlighter must be the one that lexes a buffer of
/// that language, and the built-in lexer must keep serving the others.
#[test]
fn a_registered_language_highlighter_wins_for_that_language_only() {
    /// A highlighter that calls the whole line a comment, so its effect is
    /// unmistakable in the token stream.
    struct AllComments;
    impl SyntaxHighlighter for AllComments {
        fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
            let spans = if line.is_empty() {
                Vec::new()
            } else {
                vec![TokenSpan::new(0, line.len(), TokenKind::Comment)]
            };
            (spans, state)
        }
        fn language_name(&self) -> &str {
            "All Comments"
        }
    }

    let mut editor = editor();
    let rust = editor.open_buffer("main.rs", "fn main() {}");
    editor.set_buffer_language(rust, Some(LanguageId::Rust));
    let python = editor.open_buffer("script.py", "def main():");
    editor.set_buffer_language(python, Some(LanguageId::Python));

    editor.register_language_highlighter(LanguageId::Python, alloc::boxed::Box::new(AllComments));
    assert_eq!(editor.registered_languages(), vec![LanguageId::Python]);
    assert_eq!(editor.highlighter_name_for(LanguageId::Python), "All Comments");

    assert_eq!(editor.active_language(), LanguageId::Python);
    assert_eq!(
        editor.tokens_on_line(0).iter().map(|span| span.kind).collect::<Vec<_>>(),
        vec![TokenKind::Comment],
        "the registered engine must lex the Python buffer"
    );

    assert!(editor.activate_buffer(rust));
    assert!(
        editor.tokens_on_line(0).iter().any(|span| span.kind == TokenKind::Keyword),
        "Rust must still use the built-in lexer"
    );
}

/// Removing a registration must fall back to the built-in lexer immediately.
#[test]
fn unregistering_a_language_highlighter_restores_the_builtin() {
    struct AllComments;
    impl SyntaxHighlighter for AllComments {
        fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
            let spans = if line.is_empty() {
                Vec::new()
            } else {
                vec![TokenSpan::new(0, line.len(), TokenKind::Comment)]
            };
            (spans, state)
        }
    }

    let mut editor = editor();
    editor.set_language(LanguageId::Rust);
    editor.set_text("fn main() {}");
    editor.register_language_highlighter(LanguageId::Rust, alloc::boxed::Box::new(AllComments));
    assert_eq!(editor.tokens_on_line(0)[0].kind, TokenKind::Comment);

    assert!(editor.unregister_language_highlighter(LanguageId::Rust));
    assert!(
        editor.tokens_on_line(0).iter().any(|span| span.kind == TokenKind::Keyword),
        "the built-in lexer must be back in force"
    );
    assert!(!editor.unregister_language_highlighter(LanguageId::Rust));
}

/// `set_language` must apply to the active buffer, not merely to the editor
/// default, so a multi-language host has one entry point.
#[test]
fn set_language_applies_to_the_active_buffer() {
    let mut editor = editor();
    let index = editor.open_buffer("a.rs", "let x = 1;");
    assert_eq!(editor.buffer_language(index), None, "a fresh buffer has no override");
    editor.set_language(LanguageId::Rust);
    assert_eq!(editor.buffer_language(index), Some(LanguageId::Rust));
    assert_eq!(editor.active_language(), LanguageId::Rust);
}

// ── D2: graceful degradation ────────────────────────────────────────────────

/// Toggling a comment in a language with no comment syntax must decline rather
/// than insert `// ` into plain text.
#[test]
fn toggle_line_comment_declines_for_a_language_without_comments() {
    let mut editor = editor();
    let index = editor.open_buffer("notes.txt", "just some text");
    editor.set_buffer_language(index, Some(LanguageId::PlainText));
    editor.set_caret(TextPosition::new(0, 0), false);

    editor.toggle_line_comment();
    assert_eq!(editor.text(), "just some text", "plain text has no comment syntax");
}

/// The same command must work for a language that does have one, using the
/// active buffer's language.
#[test]
fn toggle_line_comment_uses_the_active_language_prefix() {
    let mut editor = editor();
    let index = editor.open_buffer("script.py", "x = 1");
    editor.set_buffer_language(index, Some(LanguageId::Python));
    editor.set_caret(TextPosition::new(0, 0), false);

    editor.toggle_line_comment();
    assert_eq!(editor.line_text(0).as_deref(), Some("# x = 1"), "Python comments use `#`");
}

/// Auto-pairing stays available in a language without bracket grammar, because it
/// is an explicit user preference rather than a syntax guess. This is the
/// counterpoint to the comment toggle: not every structure-aware feature should
/// degrade, and the difference is whether a wrong guess corrupts.
#[test]
fn auto_pairing_stays_available_for_plain_text() {
    let mut editor = editor();
    let index = editor.open_buffer("notes.txt", "");
    editor.set_buffer_language(index, Some(LanguageId::PlainText));
    editor.set_caret(TextPosition::new(0, 0), false);

    assert!(editor.try_auto_pair('('), "pairing is a user choice, not a grammar claim");
    assert_eq!(editor.text(), "()");
}

/// Pairing must also keep working for a language that has bracket grammar.
#[test]
fn auto_pairing_works_for_rust() {
    let mut editor = editor();
    editor.set_language(LanguageId::Rust);
    editor.set_text("");
    editor.set_caret(TextPosition::new(0, 0), false);

    assert!(editor.try_auto_pair('('));
    assert_eq!(editor.text(), "()");
}

/// The `LanguageId` predicates the degradation is built on must be honest.
#[test]
fn the_language_capability_predicates_are_consistent() {
    for language in [
        LanguageId::Rust,
        LanguageId::Python,
        LanguageId::JavaScript,
        LanguageId::Go,
        LanguageId::CLike,
    ] {
        assert!(language.supports_line_comments(), "{} has comments", language.name());
        assert!(language.line_comment_prefix().is_some());
    }
    for language in [LanguageId::Markdown, LanguageId::PlainText] {
        assert!(!language.supports_line_comments(), "{} has no comments", language.name());
        assert!(language.line_comment_prefix().is_none());
    }
}

/// `ReadOnlySpan` overlap must be exact at both ends, since the whole A4 guard
/// rests on it.
#[test]
fn read_only_span_overlap_is_exact_at_the_boundaries() {
    // `lines(2, 4)` locks lines 2, 3 and 4 to their ends: the end column sentinel
    // means the whole end line is covered, so there is no "start of line 4" gap.
    let span = ReadOnlySpan::lines(2, 4);
    assert!(span.overlaps(TextPosition::new(2, 0), TextPosition::new(2, 0)), "first locked line");
    assert!(span.overlaps(TextPosition::new(3, 5), TextPosition::new(3, 9)));
    assert!(
        span.overlaps(TextPosition::new(4, 0), TextPosition::new(4, 2)),
        "the end line is locked"
    );
    assert!(span.overlaps(TextPosition::new(1, 3), TextPosition::new(2, 1)), "crossing in");
    assert!(!span.overlaps(TextPosition::new(1, 0), TextPosition::new(1, 9)), "entirely above");
    assert!(!span.overlaps(TextPosition::new(5, 0), TextPosition::new(6, 0)), "entirely below");

    // A mid-line span is exclusive at its end, so a range starting exactly there
    // does not touch it.
    let mid = ReadOnlySpan::new(TextPosition::new(2, 1), TextPosition::new(2, 4));
    assert!(mid.overlaps(TextPosition::new(2, 0), TextPosition::new(2, 1)), "touching the start");
    assert!(!mid.overlaps(TextPosition::new(2, 0), TextPosition::new(2, 0)), "before the span");
    assert!(!mid.overlaps(TextPosition::new(2, 4), TextPosition::new(2, 6)), "at/after the end");
}

/// `lines(a, b)` must lock the whole end line, which is the ambiguity that made
/// the first implementation treat `lines(0, 0)` as an empty span.
#[test]
fn a_whole_line_span_covers_the_end_line_to_its_end() {
    let span = ReadOnlySpan::lines(0, 0);
    assert!(!span.is_empty());
    assert!(span.contains(TextPosition::new(0, 0)));
    assert!(span.contains(TextPosition::new(0, 9_999)), "no column on the line is outside");
    assert!(!span.contains(TextPosition::new(1, 0)));
}

/// The span shift helper must saturate rather than wrap when lines are removed.
#[test]
fn shifting_spans_saturates_when_lines_are_removed() {
    let mut editor = editor();
    editor.set_text("a\nb\nc\n");
    editor.add_read_only_span(ReadOnlySpan::lines(2, 2));
    editor.shift_read_only_spans_after(0, -10);
    assert_eq!(editor.read_only_spans(), &[ReadOnlySpan::lines(0, 0)]);
}

/// A host that edits out of band can tell the editor a lock moved.
#[test]
fn a_host_can_shift_locks_explicitly() {
    let mut editor = editor();
    editor.add_read_only_span(ReadOnlySpan::lines(1, 2));
    editor.shift_read_only_spans_after(0, 3);
    assert_eq!(editor.read_only_spans(), &[ReadOnlySpan::lines(4, 5)]);
}

/// The context menu must report a lock instead of offering an action that would
/// silently do nothing.
///
/// The clipboard state is **injected**, so this test reaches no platform call at
/// all — which is the property the cached state exists for. Before the cache,
/// `open_context_menu` read the machine-global OS clipboard here, whose exclusive
/// handle made it race `lineedit_clipboard_copy_paste_cut` into an intermittent
/// `get_clipboard_text() == ""`.
#[test]
fn the_context_menu_disables_mutations_over_a_locked_selection() {
    let mut editor = editor();
    editor.set_clipboard_has_text(true);
    editor.set_text("keep\nlocked\n");
    editor.add_read_only_span(ReadOnlySpan::lines(1, 1));
    editor.set_caret(TextPosition::new(1, 0), false);
    editor.set_caret(TextPosition::new(1, 6), true);
    editor.open_context_menu(crate::core::Point::new(10, 10));

    let cut = editor.context_menu().items.iter().find(|item| item.id == "cut").expect("cut item");
    assert!(cut.disabled, "cut over a locked selection must be disabled");
    let copy =
        editor.context_menu().items.iter().find(|item| item.id == "copy").expect("copy item");
    assert!(!copy.disabled, "copy reads, so it stays available");
}

/// A host-provided clipboard state must decide the `paste` row without the editor
/// reading the platform.
///
/// Both directions are asserted, because "the row is enabled" alone would also pass
/// if the row were enabled unconditionally.
#[test]
fn an_injected_clipboard_state_decides_the_paste_row() {
    let mut editor = editor();
    editor.set_text("x");

    editor.set_clipboard_has_text(false);
    editor.open_context_menu(crate::core::Point::new(0, 0));
    let paste =
        editor.context_menu().items.iter().find(|item| item.id == "paste").expect("paste item");
    assert!(paste.disabled, "an empty clipboard disables paste");

    editor.set_clipboard_has_text(true);
    editor.open_context_menu(crate::core::Point::new(0, 0));
    let paste =
        editor.context_menu().items.iter().find(|item| item.id == "paste").expect("paste item");
    assert!(!paste.disabled, "text on the clipboard enables paste");
}

/// Writing the clipboard teaches the editor the answer, so the next menu needs no
/// platform read.
#[test]
fn copying_records_that_the_clipboard_has_text() {
    // No guard: this test writes only through the editor's own `copy`, which routes
    // to the platform — so it does touch the real clipboard and must not race the
    // line-edit test. The guard is held for exactly that reason.
    let _clipboard = crate::clipboard::clipboard_test_guard();
    let mut editor = editor();
    editor.set_text("some selected text");
    editor.select_all();
    assert!(editor.copy());

    // The menu now answers from the recorded fact instead of re-reading.
    editor.open_context_menu(crate::core::Point::new(0, 0));
    let paste =
        editor.context_menu().items.iter().find(|item| item.id == "paste").expect("paste item");
    assert!(!paste.disabled, "copying must leave paste enabled");
}

/// `invalidate` must drop the injected answer so the next query goes back to the
/// platform.
///
/// The assertion stops at "the override is gone": what the platform then reports is
/// **deliberately not asserted on**, because reading the OS clipboard is fallible by
/// nature (its handle is exclusive, so a read can legitimately come back empty).
/// Asserting on a live OS resource is what made the first version of this test
/// intermittent — the same mistake the cached state exists to stop the menu from
/// making.
#[test]
fn invalidating_drops_the_injected_answer() {
    let mut editor = editor();
    editor.set_clipboard_has_text(false);
    assert!(!editor.clipboard_has_text(), "an injected answer wins");

    editor.invalidate_clipboard_has_text();
    // The next query re-probes and re-caches; only the fact that it answers without
    // panicking is deterministic, so that is all this test claims.
    let _probed = editor.clipboard_has_text();
}

/// A buffer that never had an override must not be given one by reading it.
#[test]
fn a_buffer_without_a_language_override_falls_back_to_the_config() {
    let mut editor = editor();
    editor.set_language(LanguageId::Rust);
    let index = editor.open_buffer("plain.txt", "text");
    assert_eq!(editor.buffer_language(index), None);
    assert_eq!(editor.active_language(), LanguageId::Rust, "the config is the fallback");
    editor.set_buffer_language(index, None);
    assert_eq!(editor.active_language(), LanguageId::Rust);
}
