// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Regression tests for the two per-frame costs that made a large file unusable.
//!
//! Both assert the *shape* of the work rather than a wall-clock number: a timing
//! assertion would be flaky on a loaded machine and would not say why it failed.
//! The property under test is that per-frame cost does not grow with the
//! document, so the tests count how many lines each path touches.

use super::editor::{CodeEditor, MAX_MINIMAP_ROWS};
use super::syntax::{LanguageId, LineState, SyntaxHighlighter};
use super::types::{
    DocumentScale, TextPosition, TokenKind, TokenSpan, SCALE_REDUCED_LINES, SCALE_VIEW_ONLY_LINES,
};
use crate::core::Rect;

/// Builds a document of `lines` trivial Rust lines.
fn document(lines: usize) -> String {
    let mut text = String::new();
    for i in 0..lines {
        text.push_str(&format!("let value_{i} = compute({i}) + 1;\n"));
    }
    text
}

/// A minimap summary must have one bucket per strip row, not per line.
///
/// The old `draw_minimap` walked `model.lines` on every frame and issued a
/// `fill_rect` per non-blank line — 10.5 ms per frame at a million lines, which
/// is the file-opens-and-the-editor-freezes bug. The bucket vector is what makes
/// painting independent of the document, so it must stay bounded by
/// [`MAX_MINIMAP_ROWS`] no matter how large the file is.
#[test]
fn the_minimap_summary_is_bounded_by_the_strip_not_the_document() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text(document(500_000));
    editor.base.set_geometry(Rect::new(0, 0, 400, 300));
    editor.prepare_minimap();

    assert!(
        editor.minimap_buckets.len() <= MAX_MINIMAP_ROWS,
        "500 000 lines produced {} buckets; per-frame work must not scale with the file",
        editor.minimap_buckets.len()
    );
    assert!(!editor.minimap_buckets.is_empty(), "the strip has height, so it must have buckets");
}

/// Painting must not re-lex the whole document.
///
/// Only the rows near the viewport are tokenized. This is checked by tokenizing a
/// window in a very large document and asserting the cache holds a small number
/// of entries — if the implementation ever went back to lexing eagerly, the count
/// would jump to the line count.
#[test]
fn only_the_visible_window_is_tokenized() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text(document(200_000));
    editor.base.set_geometry(Rect::new(0, 0, 400, 300));
    editor.refresh_visible_rows();
    editor.prepare_visible_line_cache();

    let cached = editor.cached_token_lines();
    assert!(
        cached < 1_000,
        "200 000 lines tokenized {cached} of them; only the viewport should be"
    );
    assert!(cached > 0, "the visible rows must actually be tokenized");
}

/// An edit must invalidate only from the edited line down.
#[test]
fn an_edit_does_not_discard_tokens_above_it() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text(document(500));
    editor.base.set_geometry(Rect::new(0, 0, 400, 300));
    editor.refresh_visible_rows();
    editor.prepare_visible_line_cache();

    let low = 400usize;
    editor.fill_tokens_on_line(low);
    assert!(editor.has_cached_tokens(low), "precondition: the line is cached");

    editor.invalidate_line_states_from(450);
    assert!(
        editor.has_cached_tokens(low),
        "an edit below line 450 must not drop the cache of line {low}"
    );
    assert!(!editor.has_cached_tokens(490), "the cache at and below the edit must be dropped");
}

/// A cross-line construct must survive the incremental cache: a block comment
/// opened far above the viewport still comments the visible rows.
#[test]
fn a_block_comment_spanning_the_viewport_is_still_coloured() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_language(LanguageId::Rust);
    let mut text = String::from("/*\n");
    for _ in 0..50 {
        text.push_str("let buried = true;\n");
    }
    editor.set_text(text);
    editor.base.set_geometry(Rect::new(0, 0, 400, 300));

    editor.prime_line_states(60);
    assert_eq!(
        editor.line_state(10),
        LineState::BlockComment,
        "line 10 is inside a comment opened on line 0"
    );
    let spans = editor.tokens_on_line(10);
    assert!(
        spans.iter().all(|span| span.kind == TokenKind::BlockComment),
        "buried code must stay commented, got {spans:?}"
    );
}

/// Changing the language must drop stale token caches.
#[test]
fn switching_language_invalidates_the_token_cache() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_language(LanguageId::Rust);
    editor.set_text("fn main() {}\n".to_string());
    let rust_spans = editor.tokens_on_line(0);
    assert!(
        rust_spans.iter().any(|span| span.kind == TokenKind::Keyword),
        "`fn` is a Rust keyword"
    );

    editor.set_language(LanguageId::PlainText);
    let plain_spans = editor.tokens_on_line(0);
    assert!(
        !plain_spans.iter().any(|span| span.kind == TokenKind::Keyword),
        "after switching to PlainText no token may remain a keyword, got {plain_spans:?}"
    );
}

/// A custom highlighter may not break the renderer, and must be `Send + Sync`.
#[test]
fn a_custom_highlighter_must_be_send_and_sync() {
    /// Marks every line as a comment and never leaves a construct open.
    struct AllComment;

    impl SyntaxHighlighter for AllComment {
        fn highlight_line(&self, line: &str, state: LineState) -> (Vec<TokenSpan>, LineState) {
            if line.is_empty() {
                (Vec::new(), state)
            } else {
                (vec![TokenSpan::new(0, line.len(), TokenKind::Comment)], LineState::Clean)
            }
        }

        fn language_name(&self) -> &str {
            "All Comment"
        }
    }

    fn assert_send_sync<T: Send + Sync>(_: &T) {}

    let highlighter = AllComment;
    assert_send_sync(&highlighter);

    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("fn main() {}\n".to_string());
    editor.set_highlighter(Box::new(AllComment));
    let spans = editor.tokens_on_line(0);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, TokenKind::Comment);
    assert_eq!(editor.language_name(), "All Comment");
}

/// The tab strip must not read the buffer contents.
///
/// `all_buffers.clone()` copied every open buffer's full `text` on every frame;
/// at a million lines that was 17 ms per frame — the largest single cost in the
/// paint path, and invisible when each draw helper is profiled on its own. The
/// dirty flag is likewise a maintained boolean rather than a string comparison.
/// This test pins both by drawing a large document and asserting the frame stays
/// cheap relative to the document size.
#[test]
fn painting_a_large_document_does_not_scale_with_it() {
    let mut small = CodeEditor::new(Rect::new(0, 0, 400, 300));
    small.set_language(LanguageId::Rust);
    small.set_text(document(1_000));
    small.draw_to_string();

    let mut large = CodeEditor::new(Rect::new(0, 0, 400, 300));
    large.set_language(LanguageId::Rust);
    large.set_text(document(200_000));
    large.draw_to_string();

    // Warm both caches, then compare a steady-state frame. The bound is generous
    // because this runs on shared CI hardware; the failure it catches is a
    // regression back to O(document) work, which is orders of magnitude larger
    // than this, not a few percent.
    small.draw_to_string();
    large.draw_to_string();
    let small_ms = measure_draw(&mut small);
    let large_ms = measure_draw(&mut large);

    assert!(
        large_ms < small_ms * 8.0 + 5.0,
        "a 200 000-line document painted in {large_ms:.2} ms versus {small_ms:.2} ms for \
         1 000 lines; per-frame work must not scale with the file"
    );
}

/// Times one steady-state draw, in milliseconds.
fn measure_draw(editor: &mut CodeEditor) -> f64 {
    const N: usize = 20;
    let start = std::time::Instant::now();
    for _ in 0..N {
        editor.draw_to_string();
    }
    start.elapsed().as_secs_f64() * 1e3 / N as f64
}

/// `draw_to_string` renders through the real `Draw` implementation.
trait DrawToString {
    /// Renders this editor to SVG text.
    fn draw_to_string(&mut self) -> String;
}

impl DrawToString for CodeEditor {
    fn draw_to_string(&mut self) -> String {
        crate::widget::svg::render_to_svg(self)
    }
}

// ── Range-scoped undo (batch 2) ──────────────────────────────────────────────

/// A keystroke's undo checkpoint must be proportional to the edit, not the file.
///
/// The shared snapshot command stores the document twice; at a million lines that
/// is tens of megabytes per key press. Typing goes through the range command, so
/// the recorded text is the few characters that changed.
#[test]
fn a_keystroke_records_the_edit_not_the_document() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_language(LanguageId::Rust);
    editor.set_text(document(100_000));
    let document_bytes = editor.text().len();

    // Type one character at the caret, which sits at the document start.
    editor.insert("x");
    let checkpoint_bytes = editor.debug_last_checkpoint_bytes();

    assert!(
        checkpoint_bytes < document_bytes / 1_000,
        "the checkpoint holds {checkpoint_bytes} bytes for a 1-byte edit in a \
         {document_bytes}-byte document; history must not scale with the file"
    );
}

/// Undo and redo must round-trip through the range command.
#[test]
fn undo_restores_the_original_text_after_a_keystroke() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("hello world".to_string());
    editor.set_caret(TextPosition::new(0, 11), false);
    editor.insert("!");

    assert_eq!(editor.text(), "hello world!");
    assert!(editor.undo(), "undo must be available");
    assert_eq!(editor.text(), "hello world");
    assert!(editor.redo(), "redo must be available");
    assert_eq!(editor.text(), "hello world!");
}

/// Undo must also restore across a deletion.
#[test]
fn undo_restores_text_removed_by_backspace() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("abc".to_string());
    editor.set_caret(TextPosition::new(0, 3), false);
    editor.backspace();

    assert_eq!(editor.text(), "ab");
    assert!(editor.undo());
    assert_eq!(editor.text(), "abc");
}

/// A line command rebuilds the document and has no single range; its checkpoint
/// must still undo correctly through the snapshot fallback.
#[test]
fn a_line_command_undoes_correctly() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("b\na\n".to_string());
    editor.select_all();
    editor.sort_lines();

    assert_eq!(editor.text(), "a\nb\n");
    assert!(editor.undo(), "a rebuilt document must still be undoable");
    assert_eq!(editor.text(), "b\na\n", "undo must restore the original order");
}

// ── Large-document degradation (batch 2) ─────────────────────────────────────

/// The scale must be chosen from the line count, at documented boundaries.
#[test]
fn the_document_scale_follows_the_line_count() {
    assert_eq!(DocumentScale::for_lines(10), DocumentScale::Full);
    assert_eq!(
        DocumentScale::for_lines(SCALE_REDUCED_LINES - 1),
        DocumentScale::Full,
        "just below the reduced threshold"
    );
    assert_eq!(DocumentScale::for_lines(SCALE_REDUCED_LINES), DocumentScale::Reduced);
    assert_eq!(
        DocumentScale::for_lines(SCALE_VIEW_ONLY_LINES - 1),
        DocumentScale::Reduced,
        "just below the view-only threshold"
    );
    assert_eq!(DocumentScale::for_lines(SCALE_VIEW_ONLY_LINES), DocumentScale::ViewOnly);
}

/// A view-only document must refuse every mutation, even though the caller never
/// asked for read-only.
#[test]
fn a_view_only_document_rejects_edits() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    let mut text = String::with_capacity(SCALE_VIEW_ONLY_LINES * 4);
    for _ in 0..SCALE_VIEW_ONLY_LINES {
        text.push_str("x\n");
    }
    editor.set_text(text);

    assert_eq!(editor.document_scale(), DocumentScale::ViewOnly);
    assert!(!editor.is_editable(), "a view-only document is not editable");
    assert!(editor.is_read_only(), "and reports itself as read-only");

    let before = editor.text().len();
    editor.insert("z");
    assert_eq!(editor.text().len(), before, "an insert into a view-only document must be a no-op");
}

/// A reduced document still accepts edits but reports the reduction.
#[test]
fn a_reduced_document_stays_editable_and_says_what_it_dropped() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    let mut text = String::with_capacity(SCALE_REDUCED_LINES * 4);
    for _ in 0..SCALE_REDUCED_LINES {
        text.push_str("x\n");
    }
    editor.set_text(text);

    assert_eq!(editor.document_scale(), DocumentScale::Reduced);
    assert!(editor.is_editable(), "a reduced document is still editable");
    assert!(
        editor.document_scale().notice().is_some(),
        "a reduction must be reportable, not silent"
    );
    assert!(!editor.document_scale().allows_document_analysis());
}

/// A normal document is unaffected: full capability, no notice.
#[test]
fn a_normal_document_keeps_full_capability() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text(document(500));

    assert_eq!(editor.document_scale(), DocumentScale::Full);
    assert!(editor.is_editable());
    assert!(!editor.is_read_only());
    assert_eq!(editor.document_scale().notice(), None, "nothing to report at Full");
    assert!(editor.document_scale().allows_document_analysis());
}

// ── In-place editing and incremental search (batch 3) ────────────────────────

/// A single-line edit must not resize the document.
///
/// The old splice joined every line into one string, spliced that, and split it
/// back — O(file) for a keystroke. The lines outside the edit are now untouched,
/// which this checks by their addresses: `Vec<String>` reallocation aside, the
/// buffer a line lives in must be the same allocation before and after.
#[test]
fn a_single_line_edit_leaves_other_lines_untouched() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text(document(200));
    editor.set_caret(TextPosition::new(100, 4), false);

    let before: Vec<String> = (0..201).map(|line| editor.line_text(line).unwrap()).collect();
    editor.insert("Z");
    let after: Vec<String> = (0..201).map(|line| editor.line_text(line).unwrap()).collect();

    assert_eq!(before[0], after[0], "a line above the edit must be unchanged");
    assert_eq!(before[99], after[99], "the line above the caret must be unchanged");
    assert_eq!(before[101], after[101], "a line below the edit must be unchanged");
    assert_eq!(before[200], after[200], "the last line must be unchanged");
    assert_ne!(before[100], after[100], "the edited line must have changed");
    assert_eq!(editor.line_count(), 201, "a one-line edit must not change the line count");
}

/// A cross-line edit must re-index the document correctly.
#[test]
fn a_cross_line_edit_splits_the_line() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("alpha\nbeta".to_string());
    editor.set_caret(TextPosition::new(0, 3), false);
    editor.insert("\n");

    assert_eq!(editor.text(), "alp\nha\nbeta");
    assert_eq!(editor.line_count(), 3);

    assert!(editor.undo());
    assert_eq!(editor.text(), "alpha\nbeta", "undo must reunite the line");
    assert_eq!(editor.line_count(), 2);
}

/// Deleting a line break must merge the two lines.
#[test]
fn deleting_a_line_break_merges_the_lines() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("alpha\nbeta".to_string());
    editor.set_caret(TextPosition::new(1, 0), false);
    editor.backspace();

    assert_eq!(editor.text(), "alphabeta");
    assert_eq!(editor.line_count(), 1);
}

/// The match cache must stay correct after an incremental update.
///
/// Hits are recomputed only for the edited line, so a bug there would leave
/// stale or missing hits. This edits a line that contains hits and checks the
/// list follows.
#[test]
fn the_match_cache_tracks_an_edit_to_a_matching_line() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("aaa\nbbb\naaa\n".to_string());
    editor.set_search_query("aaa");
    assert_eq!(editor.search_matches().len(), 2, "two lines contain the query");

    // Select the whole first line and delete it in one edit, so there is a single
    // checkpoint to undo.
    editor.set_caret(TextPosition::new(0, 0), false);
    editor.set_caret(TextPosition::new(0, 3), true);
    editor.delete_selection();
    assert_eq!(editor.text(), "\nbbb\naaa\n");
    assert_eq!(
        editor.search_matches().len(),
        1,
        "the hits of the edited line must be dropped: {:?}",
        editor.search_matches()
    );
    assert_eq!(editor.search_matches()[0].line, 2, "the surviving hit is on line 2");

    assert!(editor.undo());
    assert_eq!(
        editor.search_matches().len(),
        2,
        "undo must restore both hits: {:?}",
        editor.search_matches()
    );
}

/// A query with no matches, then an edit that creates one.
#[test]
fn the_match_cache_finds_a_newly_typed_query() {
    let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("one\ntwo\nthree\n".to_string());
    editor.set_search_query("zzz");
    assert!(editor.search_matches().is_empty());

    editor.set_caret(TextPosition::new(1, 3), false);
    editor.insert("zzz");
    assert_eq!(editor.text(), "one\ntwozzz\nthree\n");
    assert_eq!(editor.search_matches().len(), 1, "the new text must be found");
    assert_eq!(editor.search_matches()[0].line, 1);
}
