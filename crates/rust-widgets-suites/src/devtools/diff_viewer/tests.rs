// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! Behavioural tests for the diff viewer and its file list.

use super::file_list::*;
use super::model::*;
use super::viewer::*;
use rust_widgets::core::{Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::special_widgets::code_editor::{LanguageId, TokenKind};
use rust_widgets::widget::Draw;
use std::sync::{Arc, Mutex};

/// The exact-comparison entry point, for tests that pin the algorithm without a whitespace
/// policy; the widget itself goes through `build_diff_lines_with`.
fn build_diff_lines(a: &[&str], b: &[&str]) -> Vec<DiffLine> {
    build_diff_lines_with(a, b, a, b, false)
}

#[test]
fn recompute_detects_changes() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("a\nb\nc", "a\nB\nc\nd");

    assert_eq!(viewer.lines().len(), 4);
    assert_eq!(viewer.change_count(), 2);
    assert_eq!(viewer.lines()[1].kind, DiffKind::Changed);
    assert_eq!(viewer.lines()[3].kind, DiffKind::Added);
}

#[test]
fn compared_signal_emits_change_count() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    let counts = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = counts.clone();
    viewer.compared.connect(move |count| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*count);
        }
    });

    viewer.set_texts("x", "y");

    let got = counts.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got, vec![1]);
}

#[test]
fn arrow_keys_move_selected_line() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("a\nb", "a\nb\nc");
    assert_eq!(viewer.selected_index(), Some(0));

    viewer.handle_event(&Event::key_press(40, 0));
    assert_eq!(viewer.selected_index(), Some(1));

    viewer.handle_event(&Event::key_press(40, 0));
    assert_eq!(viewer.selected_index(), Some(2));

    viewer.handle_event(&Event::key_press(38, 0));
    assert_eq!(viewer.selected_index(), Some(1));
}

#[test]
fn leading_insertion_leaves_the_rest_equal() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("A\nB\nC", "X\nA\nB\nC");

    let lines = viewer.lines();
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].kind, DiffKind::Added);
    assert_eq!(lines[0].left, None);
    assert_eq!(lines[0].right.as_deref(), Some("X"));
    assert!(lines[1..].iter().all(|line| line.kind == DiffKind::Equal));
    assert_eq!(lines[1].left.as_deref(), Some("A"));
    assert_eq!(viewer.change_count(), 1);
}

#[test]
fn identical_inputs_are_all_equal() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("alpha\nbeta\ngamma", "alpha\nbeta\ngamma");

    assert_eq!(viewer.lines().len(), 3);
    assert!(viewer.lines().iter().all(|line| line.kind == DiffKind::Equal));
    assert_eq!(viewer.change_count(), 0);
}

#[test]
fn change_count_matches_non_equal_rows() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("one\ntwo\nthree\nfour", "one\n2\nthree\nfour\nfive");

    let non_equal = viewer.lines().iter().filter(|line| line.kind != DiffKind::Equal).count();
    assert_eq!(viewer.change_count(), non_equal);
    assert!(non_equal > 0);
}

/// The wheel scrolls the visible diff window and clamps at both ends.
///
/// # The defect this pins
///
/// The paint path stopped at a fixed twelve rows, so a longer comparison could not be
/// reached. This drives the window the paint path now starts from.
#[test]
fn diff_viewer_wheel_scrolls_and_clamps() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let joined = text.join("\n");
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
    viewer.set_texts(joined.clone(), joined);
    assert_eq!(viewer.lines().len(), 20);
    assert_eq!(viewer.first_visible_line, 0);

    // One notch toward the end reveals three rows.
    viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    assert_eq!(viewer.first_visible_line, 3, "a downward notch moves the viewport down");

    // The other direction returns to the top and clamps there.
    viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
    assert_eq!(viewer.first_visible_line, 0);
    viewer.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
    assert_eq!(viewer.first_visible_line, 0, "the viewport clamps at the first row");

    // Scrolling on stops at the last full window.
    let max_first = viewer.lines().len() - viewer.visible_line_count();
    for _ in 0..20 {
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    }
    assert_eq!(viewer.first_visible_line, max_first, "the viewport clamps at the last window");
}

/// Trimming the common head and tail keeps a middle edit to its own hunk.
#[test]
fn a_middle_edit_changes_only_its_own_hunk() {
    let before: Vec<String> = (0..600).map(|index| alloc::format!("line {index}")).collect();
    let mut after = before.clone();
    after[300] = "line three hundred".to_string();
    let before_refs: Vec<&str> = before.iter().map(String::as_str).collect();
    let after_refs: Vec<&str> = after.iter().map(String::as_str).collect();

    let lines = build_diff_lines(&before_refs, &after_refs);
    assert_eq!(lines.len(), before.len());
    assert_eq!(lines[0].kind, DiffKind::Equal);
    assert_eq!(lines[599].kind, DiffKind::Equal);
    assert_eq!(lines[300].kind, DiffKind::Changed);
    assert_eq!(
        lines.iter().filter(|line| line.kind != DiffKind::Equal).count(),
        1,
        "the unchanged head and tail must not be reported as changes"
    );
}

/// Unified mode paints a changed row as a removal and an addition, so the old text survives.
///
/// The viewer used to paint only the new text with a `~` prefix, which is a diff that cannot
/// answer "what changed".
#[test]
fn unified_mode_shows_the_old_text_of_a_changed_row() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("a\nold\nb", "a\nnew\nb");
    viewer.set_view_mode(DiffViewMode::Unified);
    assert_eq!(viewer.display_row_count(), 4, "the changed row takes two painted rows");

    assert!(matches!(
        viewer.display_rows[1].source,
        RowSource::Line { index: 1, side: RowSide::Left }
    ));
    assert!(matches!(
        viewer.display_rows[2].source,
        RowSource::Line { index: 1, side: RowSide::Right }
    ));
    assert_eq!(viewer.display_rows[1].left_number, Some(2));
    assert_eq!(viewer.display_rows[2].right_number, Some(2));
    assert_eq!(viewer.lines()[1].left.as_deref(), Some("old"));
}

/// The pair expansion is optional, and disabling it restores the single `~` row.
#[test]
fn unified_changed_pair_can_be_disabled() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("a\nold\nb", "a\nnew\nb");
    viewer.set_view_mode(DiffViewMode::Unified);
    viewer.set_unified_changed_pair(false);
    assert_eq!(viewer.display_row_count(), 3);
    assert!(matches!(
        viewer.display_rows[1].source,
        RowSource::Line { index: 1, side: RowSide::Right }
    ));
}

/// Selection, view-mode and fold changes are published.
#[test]
fn view_state_signals_are_emitted() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    let selections = Arc::new(Mutex::new(Vec::<Option<usize>>::new()));
    let sink = selections.clone();
    viewer.selection_changed.connect(move |index| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*index);
        }
    });
    let modes = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = modes.clone();
    viewer.view_mode_changed.connect(move |mode| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(mode.as_ref().clone());
        }
    });
    let folds = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = folds.clone();
    viewer.fold_changed.connect(move |fold| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*fold);
        }
    });

    viewer.set_texts("a\nb", "a\nb\nc");
    assert!(viewer.select_index(1));
    assert!(viewer.select_index(1), "re-selecting the same row is not a change");
    viewer.set_view_mode(DiffViewMode::Unified);
    viewer.set_fold_unchanged(true);

    let selections = selections.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(selections, vec![Some(0), Some(1)], "the reset and the click are both reported");
    let modes = modes.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(modes, vec!["unified".to_string()]);
    let folds = folds.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(folds, vec![true]);
}

/// A fold marker is clickable and expands the run it stands for.
#[test]
fn clicking_the_fold_marker_expands_the_hunk() {
    let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
    let mut changed = text.clone();
    changed[25] = "line twenty-five".to_string();
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
    viewer.set_texts(text.join("\n"), changed.join("\n"));
    viewer.set_fold_unchanged(true);

    let expanded = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = expanded.clone();
    viewer.hunk_expanded.connect(move |start| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*start);
        }
    });

    assert_eq!(viewer.display_row_count(), 6, "one marker + the change + the tail");
    assert!(matches!(viewer.display_rows[0].source, RowSource::Fold { run: (0, 25) }));

    // The click row is derived from the same constants the paint uses.
    viewer.handle_event(&Event::mouse_press(10, ROW_TOP + 2, 1));
    assert_eq!(viewer.display_row_count(), 30, "expanding restores every row");
    let expanded = expanded.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(expanded, vec![0]);
}

/// Whitespace policy changes what counts as a change without rewriting the visible text.
#[test]
fn whitespace_modes_change_what_counts_as_a_change() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("a", "a ");
    assert_eq!(viewer.change_count(), 1);
    assert!(viewer.lines()[0].blank_only, "a trailing space is a blank-only change");

    viewer.set_whitespace_mode(WhitespaceMode::IgnoreEol);
    assert_eq!(viewer.change_count(), 0, "trailing whitespace is ignored");
    assert_eq!(viewer.lines()[0].left.as_deref(), Some("a"), "the display text is raw");
    assert_eq!(viewer.lines()[0].right.as_deref(), Some("a "));

    viewer.set_whitespace_mode(WhitespaceMode::IgnoreAll);
    viewer.set_texts("a b", "ab");
    assert_eq!(viewer.change_count(), 0, "all whitespace is ignored");

    viewer.set_whitespace_mode(WhitespaceMode::MarkBlankChanges);
    viewer.set_texts("a", "a ");
    assert_eq!(viewer.change_count(), 1, "blank-only rows are still reported");
    assert!(viewer.lines()[0].blank_only);
}

/// The differing spans are resolved when the row is built, not per frame.
#[test]
fn changed_spans_are_precomputed() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("x = 1", "x = 2");
    let line = &viewer.lines()[0];
    assert_eq!(line.kind, DiffKind::Changed);
    assert_eq!(line.changed_spans, Some(differing_spans("x = 1", "x = 2")));
    assert!(viewer
        .lines()
        .iter()
        .all(|line| line.kind == DiffKind::Changed || line.changed_spans.is_none()));
}

/// Character granularity is the default; word granularity widens the tint to the words the
/// changed run touches, without changing the row set.
#[test]
fn word_granularity_widens_the_inline_span() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("alpha beta", "alphabet beta");
    assert!(!viewer.word_granularity());
    let char_span = viewer.lines()[0].changed_spans.expect("changed row");
    // The changed run under character granularity is the appended `bet`.
    assert_eq!(char_span.0, (5, 5));
    assert_eq!(char_span.1, (5, 8));

    viewer.set_word_granularity(true);
    assert!(viewer.word_granularity());
    let word_span = viewer.lines()[0].changed_spans.expect("changed row");
    let ((left_start, left_end), (right_start, right_end)) = word_span;
    assert_eq!(&"alpha beta"[left_start..left_end], "alpha");
    assert_eq!(&"alphabet beta"[right_start..right_end], "alphabet");

    // Switching back restores the character span rather than leaving the widened one.
    viewer.set_word_granularity(false);
    assert_eq!(viewer.lines()[0].changed_spans, Some(char_span));
}

/// A comparison rebuilt while word granularity is on keeps the widened spans.
#[test]
fn word_granularity_survives_a_recomparison() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_word_granularity(true);
    viewer.set_texts("total = 41", "total = 42");
    let span = viewer.lines()[0].changed_spans.expect("changed row");
    assert_eq!(&"total = 41"[span.0 .0..span.0 .1], "41");
    assert_eq!(&"total = 42"[span.1 .0..span.1 .1], "42");
}

fn file_list() -> DiffFileList {
    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(vec![
        FileDiffEntry::new("src/a.rs", FileDiffStatus::Modified).with_counts(3, 1),
        FileDiffEntry::new("src/b.rs", FileDiffStatus::Added).with_counts(10, 0),
        FileDiffEntry::new("src/c.rs", FileDiffStatus::Removed).with_counts(0, 7),
    ]);
    list
}

#[test]
fn file_list_reports_selection_and_activation() {
    let mut list = file_list();
    let selected = Arc::new(Mutex::new(Vec::<String>::new()));
    let activated = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = selected.clone();
    list.file_selected.connect(move |path| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(path.as_str().to_string());
        }
    });
    let sink = activated.clone();
    list.file_activated.connect(move |path| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(path.as_str().to_string());
        }
    });

    assert!(list.select_file("src/b.rs"));
    assert!(!list.select_file("src/missing.rs"), "an unknown path is refused");
    let row = list.row_rect(1).expect("row 1 is visible");
    list.handle_event(&Event::mouse_double_click(row.x + 4, row.y + 4, 1));

    assert_eq!(list.selected_path(), Some("src/b.rs"));
    assert_eq!(
        selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
        vec!["src/b.rs".to_string()]
    );
    assert_eq!(
        activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
        vec!["src/b.rs".to_string()]
    );
}

#[test]
fn file_list_files_property_round_trips() {
    let mut list = file_list();
    let read = list.get("files").expect("files readable");
    let written = list.set("files", read.clone());
    assert!(written.is_ok());
    assert_eq!(list.get("files").expect("files readable"), read);
    assert_eq!(list.get("file_count"), Ok(CapabilityValue::UInt(3)));
}

#[test]
fn file_list_multi_selection_keeps_every_clicked_row() {
    let mut list = file_list();
    list.set_selection_mode(rust_widgets::widget::view_widgets::list_view::SelectionMode::Multi);
    list.select_index(0);
    list.select_index(2);
    assert_eq!(list.selected_paths(), vec!["src/a.rs", "src/c.rs"]);
    list.select_index(0);
    assert_eq!(list.selected_paths(), vec!["src/c.rs"]);
}

/// The Myers trace is bounded by bytes, and an over-deep comparison is reported as such.
#[test]
fn the_myers_trace_is_bounded() {
    let left: Vec<String> = (0..2000).map(|n| format!("left{n}")).collect();
    let right: Vec<String> = (0..2000).map(|n| format!("right{n}")).collect();
    let left_refs: Vec<&str> = left.iter().map(String::as_str).collect();
    let right_refs: Vec<&str> = right.iter().map(String::as_str).collect();
    assert!(
        myers_diff(&left_refs, &right_refs).is_none(),
        "a full-distance comparison must be refused rather than allocating the trace"
    );
    // The caller still gets a usable row list from the positional fallback.
    let lines = build_diff_lines(&left_refs, &right_refs);
    assert_eq!(lines.len(), 2000);
    assert!(lines.iter().all(|line| line.kind != DiffKind::Equal));
}

/// The fold threshold and the context kept around a marker are configurable.
#[test]
fn fold_threshold_and_context_lines_are_configurable() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let mut changed = text.clone();
    changed[15] = "line fifteen".to_string();
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
    viewer.set_texts(text.join("\n"), changed.join("\n"));
    assert_eq!(viewer.fold_threshold(), 8);
    assert_eq!(viewer.fold_context_lines(), 0);

    viewer.set_fold_unchanged(true);
    viewer.set_fold_threshold(30);
    assert_eq!(viewer.display_row_count(), 20, "a threshold above the run keeps every row");

    viewer.set_fold_threshold(4);
    viewer.set_fold_context_lines(2);
    viewer.display_dirty = true;
    let rows = viewer.display_row_count();
    assert!(rows < 20, "the run is folded with context around it");
    assert!(matches!(viewer.display_rows[2].source, RowSource::Fold { .. }));
    assert!(matches!(viewer.display_rows[0].source, RowSource::Line { index: 0, .. }));
    assert!(matches!(viewer.display_rows[1].source, RowSource::Line { index: 1, .. }));
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;

    /// SP-08: the two normalisation switches remove noise the row set used to keep.
    #[test]
    fn ignore_case_and_blank_lines_remove_comparison_noise() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("Alpha\n \nbeta", "alpha\n\nbeta");
        assert!(viewer.change_count() > 0);
        viewer.set_ignore_case(true);
        viewer.set_ignore_blank_lines(true);
        assert_eq!(viewer.change_count(), 0);
        assert!(viewer.ignore_case());
        assert!(viewer.ignore_blank_lines());
    }

    /// SP-P12: the gutters are precomputed with the rows rather than formatted per frame.
    #[test]
    fn display_rows_carry_their_gutter_captions() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("one\ntwo", "one\nTWO");
        let count = viewer.display_row_count();
        assert!(count >= 2);
        let row = &viewer.display_rows[1];
        assert!(row.left_gutter.is_some() || row.right_gutter.is_some());
    }

    /// SP-016: syntax highlighting is opt-in and tokenises the compared lines in place.
    #[test]
    fn syntax_highlight_is_off_by_default_and_tokenises_when_enabled() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("fn main() {}", "fn main() { }");
        assert!(!viewer.syntax_highlight(), "the default presentation is unchanged");
        assert_eq!(viewer.language(), LanguageId::PlainText);
        viewer.ensure_syntax_spans();
        assert!(viewer.syntax_spans.is_none(), "no tokens are built while it is off");

        viewer.set_syntax_highlight(true);
        viewer.set_language(LanguageId::Rust);
        viewer.ensure_syntax_spans();
        let (language, rows) = viewer.syntax_spans.as_ref().expect("spans were built");
        assert_eq!(*language, LanguageId::Rust);
        let (left, _) = &rows[0];
        assert!(
            left.iter().any(|span| span.kind == TokenKind::Keyword
                && viewer.lines()[0]
                    .left
                    .as_deref()
                    .and_then(|text| text.get(span.start..span.end))
                    == Some("fn")),
            "the keyword is classified for its own colour"
        );
    }

    /// SP-016: the highlighted paint path runs over a real frame.
    #[test]
    fn syntax_highlighted_rows_paint() {
        use rust_widgets::core::{Color, Size};
        use rust_widgets::render::{PaintBackend, SoftwarePaintBackend};
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts("let value = 1;\nfn run() {}", "let value = 2;\nfn run() {}");
        viewer.set_language(LanguageId::Rust);
        viewer.set_syntax_highlight(true);
        let mut backend = SoftwarePaintBackend::new(Size::new(640, 300), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        viewer.draw(&mut context);
        backend.end_frame();
        assert!(!backend.frame_rgba().is_empty());
    }
}
