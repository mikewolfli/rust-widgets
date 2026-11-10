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
use rust_widgets::widget::special_widgets::editor::syntax::{LanguageId, TokenKind};
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
    viewer.handle_event(&Event::FocusGained { reason: rust_widgets::event::FocusReason::Tab });
    assert_eq!(viewer.selected_index(), Some(0));

    viewer.handle_event(&Event::key_press(40, 0));
    assert_eq!(viewer.selected_index(), Some(1));

    viewer.handle_event(&Event::key_press(40, 0));
    assert_eq!(viewer.selected_index(), Some(2));

    viewer.handle_event(&Event::key_press(38, 0));
    assert_eq!(viewer.selected_index(), Some(1));
}

/// A keyboard selection past the painted window scrolls the window to it.
#[test]
fn arrow_keys_scroll_the_selected_row_into_view() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let joined = text.join("\n");
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
    viewer.set_texts(joined.clone(), joined);
    viewer.handle_event(&Event::FocusGained { reason: rust_widgets::event::FocusReason::Tab });
    assert_eq!(viewer.first_visible_line, 0);

    for _ in 0..5 {
        viewer.handle_event(&Event::key_press(40, 0));
    }
    assert_eq!(viewer.selected_index(), Some(5));
    assert_eq!(
        viewer.first_visible_line,
        5 + 1 - viewer.visible_line_count(),
        "the selected row is the last painted one instead of staying below the window"
    );
}

/// Only the rows the paint path actually draws are hit targets.
#[test]
fn a_click_outside_the_painted_rows_selects_nothing() {
    let text: Vec<String> = (0..40).map(|n| format!("line{n}")).collect();
    let joined = text.join("\n");
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
    viewer.set_texts(joined.clone(), joined);
    assert!(viewer.select_index(2));

    // The column heading is not a row.
    viewer.handle_event(&Event::mouse_press(10, 20, 1));
    assert_eq!(viewer.selected_index(), Some(2), "a heading click is not a row click");

    // The first row's fill starts ten pixels above its text origin; a press below the last
    // painted band must not reach an off-screen row.
    let below = ROW_TOP - 10 + viewer.visible_line_count() as i32 * 16 + 2;
    viewer.handle_event(&Event::mouse_press(10, below, 1));
    assert_eq!(viewer.selected_index(), Some(2), "a click below the window selects nothing");
}

/// A granularity switch must discard the advances measured for the previous spans.
#[test]
fn toggling_word_granularity_invalidates_the_measured_spans() {
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
    viewer.set_texts("alpha beta", "alphabet beta");
    rust_widgets::widget::svg::render_to_svg(&mut viewer);
    assert!(viewer.measured_span_rows() > 0, "painting a changed row measures its spans");

    viewer.set_word_granularity(true);
    assert_eq!(viewer.measured_span_rows(), 0, "the widened span is measured afresh");
}

/// A rebuilt comparison selects the first row, so the viewport returns to it.
#[test]
fn a_recomparison_returns_the_viewport_to_the_first_row() {
    let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
    let mut changed = text.clone();
    changed[20] = "line twenty".to_string();
    let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
    viewer.set_texts(text.join("\n"), changed.join("\n"));

    for _ in 0..2 {
        viewer.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    }
    assert!(viewer.first_visible_line > 0, "the wheel moved the viewport");

    viewer.set_whitespace_mode(WhitespaceMode::IgnoreEol);
    assert_eq!(viewer.selected_index(), Some(0), "the comparison selects its first row");
    assert_eq!(viewer.first_visible_line, 0, "the viewport shows the row it selected");
}

/// The declared `selected_index` default is the value a fresh control reports.
#[test]
fn the_declared_selected_index_default_matches_a_fresh_viewer() {
    let viewer = DiffViewer::new(Rect::new(0, 0, 10, 10));
    assert_eq!(
        crate::devtools::default_property_value("diff_viewer", "selected_index"),
        Some(CapabilityValue::Null)
    );
    assert_eq!(viewer.get("selected_index"), Ok(CapabilityValue::Null));
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

/// A press outside the painted rows is not a selection, even when the list holds more files.
#[test]
fn file_list_ignores_clicks_outside_the_painted_rows() {
    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 60));
    list.set_files(
        (0..10)
            .map(|index| FileDiffEntry::new(format!("f{index}.rs"), FileDiffStatus::Modified))
            .collect(),
    );

    // The gap above the first row is part of the control but not of a row.
    list.handle_event(&Event::mouse_press(10, 2, 1));
    assert_eq!(list.selected_path(), None, "the top gap is not row zero");

    // Two 22 px rows fit below the six-pixel gap; the press below them must not reach row two.
    list.handle_event(&Event::mouse_press(10, 6 + 2 * 22 + 2, 1));
    assert_eq!(list.selected_path(), None, "an unpainted row is not a hit target");

    list.handle_event(&Event::mouse_press(10, 6 + 2, 1));
    assert_eq!(list.selected_path(), Some("f0.rs"));
}

/// `Extended` mode grows a contiguous range from a Shift-click or a Shift-arrow.
#[test]
fn file_list_extended_mode_selects_a_range() {
    use rust_widgets::shortcut::Modifiers;
    use rust_widgets::widget::view_widgets::list_view::SelectionMode;

    let shift = Modifiers::SHIFT.bits() as u32;
    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(
        ["a.rs", "b.rs", "c.rs", "d.rs"]
            .iter()
            .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
            .collect(),
    );
    list.set_selection_mode(SelectionMode::Extended);

    let picked = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = picked.clone();
    list.file_selected.connect(move |path| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(path.as_str().to_string());
        }
    });

    let row0 = list.row_rect(0).expect("row 0");
    let row2 = list.row_rect(2).expect("row 2");
    list.handle_event(&Event::mouse_press(row0.x + 4, row0.y + 4, 1));
    assert_eq!(list.selected_paths(), vec!["a.rs"]);

    list.handle_event(&Event::mouse_press_with(row2.x + 4, row2.y + 4, 1, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs", "b.rs", "c.rs"], "Shift-click extends");

    list.handle_event(&Event::FocusGained { reason: rust_widgets::event::FocusReason::Tab });
    list.handle_event(&Event::key_press(40, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs", "b.rs", "c.rs", "d.rs"]);

    list.handle_event(&Event::key_press(38, shift));
    list.handle_event(&Event::key_press(38, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs", "b.rs"], "Shift-Up shrinks to the anchor");
    list.handle_event(&Event::key_press(38, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs"]);
    list.handle_event(&Event::key_press(38, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs"], "the anchor row has no predecessor");

    let picked = picked.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(
        picked,
        vec!["a.rs", "c.rs", "d.rs", "c.rs", "b.rs", "a.rs"],
        "each extension reports the head it reached"
    );
}

/// Shift+Home and Shift+End extend an `Extended` selection to the list edges.
#[test]
fn file_list_extended_shift_home_and_end_reach_the_edges() {
    use rust_widgets::shortcut::Modifiers;
    use rust_widgets::widget::view_widgets::list_view::SelectionMode;

    let shift = Modifiers::SHIFT.bits() as u32;
    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(
        ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]
            .iter()
            .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
            .collect(),
    );
    list.set_selection_mode(SelectionMode::Extended);
    list.handle_event(&Event::FocusGained { reason: rust_widgets::event::FocusReason::Tab });

    let row2 = list.row_rect(2).expect("row 2");
    list.handle_event(&Event::mouse_press(row2.x + 4, row2.y + 4, 1));
    assert_eq!(list.selected_paths(), vec!["c.rs"]);

    list.handle_event(&Event::key_press(35, shift));
    assert_eq!(list.selected_paths(), vec!["c.rs", "d.rs", "e.rs"], "Shift+End");

    list.handle_event(&Event::key_press(36, shift));
    assert_eq!(list.selected_paths(), vec!["a.rs", "b.rs", "c.rs"], "Shift+Home");
}

/// Plain Home and End move the selection to the list edges; they used to do nothing.
#[test]
fn file_list_plain_home_and_end_move_the_selection() {
    use rust_widgets::event::FocusReason;

    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(
        ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]
            .iter()
            .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
            .collect(),
    );
    list.handle_event(&Event::FocusGained { reason: FocusReason::Tab });

    let row2 = list.row_rect(2).expect("row 2");
    list.handle_event(&Event::mouse_press(row2.x + 4, row2.y + 4, 1));
    assert_eq!(list.selected_paths(), vec!["c.rs"]);

    list.handle_event(&Event::key_press(35, 0));
    assert_eq!(list.selected_paths(), vec!["e.rs"], "End picks the last row");

    list.handle_event(&Event::key_press(36, 0));
    assert_eq!(list.selected_paths(), vec!["a.rs"], "Home picks the first row");
}

/// Dragging past an edge of an `Extended` selection scrolls the list, so the range can grow past
/// the rows that happened to be on screen when the drag began.
#[test]
fn file_list_extended_drag_scrolls_at_the_edges() {
    use rust_widgets::widget::view_widgets::list_view::SelectionMode;
    use rust_widgets::widget::Widget as _;

    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(
        (0..20)
            .map(|index| FileDiffEntry::new(format!("f{index}.rs"), FileDiffStatus::Modified))
            .collect(),
    );
    list.set_selection_mode(SelectionMode::Extended);

    let row6 = list.row_rect(6).expect("row 6");
    list.handle_event(&Event::mouse_press(row6.x + 4, row6.y + 4, 1));
    assert_eq!(list.selected_paths().len(), 1);

    let below = list.geometry().bottom() + 20;
    for _ in 0..3 {
        list.handle_event(&Event::mouse_move(row6.x + 4, below));
    }
    assert!(
        list.row_rect(0).is_none(),
        "the viewport scrolled under the drag, so row 0 is no longer painted"
    );
    assert_eq!(
        list.selected_paths().last().copied(),
        Some("f10.rs"),
        "each edge move scrolled one row and extended to it"
    );

    // Dragging back above the top edge scrolls back up and shrinks the range.
    let above = list.geometry().y - 20;
    for _ in 0..3 {
        list.handle_event(&Event::mouse_move(row6.x + 4, above));
    }
    assert!(list.row_rect(0).is_some(), "the viewport came back to the first row");
    assert_eq!(
        list.selected_paths(),
        vec!["f0.rs", "f1.rs", "f2.rs", "f3.rs", "f4.rs", "f5.rs", "f6.rs"],
        "the head followed the scroll back to the first row, anchored at the press"
    );
}

/// A pointer drag grows an `Extended` range and a release stops it.
#[test]
fn file_list_extended_drag_selects_a_range_until_release() {
    use rust_widgets::widget::view_widgets::list_view::SelectionMode;

    let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
    list.set_files(
        ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]
            .iter()
            .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
            .collect(),
    );
    list.set_selection_mode(SelectionMode::Extended);

    let row0 = list.row_rect(0).expect("row 0");
    let row1 = list.row_rect(1).expect("row 1");
    let row2 = list.row_rect(2).expect("row 2");
    let row3 = list.row_rect(3).expect("row 3");
    let row4 = list.row_rect(4).expect("row 4");

    list.handle_event(&Event::mouse_press(row1.x + 4, row1.y + 4, 1));
    assert_eq!(list.selected_paths(), vec!["b.rs"]);
    list.handle_event(&Event::mouse_move(row3.x + 4, row3.y + 4));
    assert_eq!(list.selected_paths(), vec!["b.rs", "c.rs", "d.rs"], "the drag extends");

    list.handle_event(&Event::mouse_release(row3.x + 4, row3.y + 4, 1));
    list.handle_event(&Event::mouse_move(row0.x + 4, row0.y + 4));
    assert_eq!(
        list.selected_paths(),
        vec!["b.rs", "c.rs", "d.rs"],
        "a move after the release does not extend"
    );

    // Leaving the control is the last signal when the release lands outside it.
    list.handle_event(&Event::mouse_press(row1.x + 4, row1.y + 4, 1));
    list.handle_event(&Event::mouse_move(row2.x + 4, row2.y + 4));
    list.handle_event(&Event::mouse_leave(row4.x + 4, row4.y + 4));
    list.handle_event(&Event::mouse_move(row4.x + 4, row4.y + 4));
    assert_eq!(list.selected_paths(), vec!["b.rs", "c.rs"], "the leave ended the drag");
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

    /// Turning syntax highlighting off (or to plain text) drops the token table.
    ///
    /// The rebuild used to return early and leave the last language's spans in place, so a
    /// plain-text view kept painting token colours — including over unrelated new text, since the
    /// spans are only clamped to the line length.
    #[test]
    fn plain_text_language_clears_the_stale_token_spans() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts("fn main() {}", "fn main() { }");
        viewer.set_syntax_highlight(true);
        viewer.set_language(LanguageId::Rust);
        viewer.ensure_syntax_spans();
        assert!(viewer.syntax_spans.is_some(), "Rust produces token spans");

        viewer.set_language(LanguageId::PlainText);
        viewer.ensure_syntax_spans();
        assert!(viewer.syntax_spans.is_none(), "no language means no token spans");
    }

    /// Unified mode's fold context rows are ordinary single-column rows.
    #[test]
    fn unified_fold_context_rows_are_single_column() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut changed = text.clone();
        changed[15] = "line fifteen".to_string();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 400));
        viewer.set_texts(text.join("\n"), changed.join("\n"));
        viewer.set_view_mode(DiffViewMode::Unified);
        viewer.set_fold_unchanged(true);
        viewer.set_fold_threshold(4);
        viewer.set_fold_context_lines(2);
        viewer.display_row_count();

        assert!(matches!(viewer.display_rows[2].source, RowSource::Fold { .. }));
        assert!(
            viewer
                .display_rows
                .iter()
                .all(|row| !matches!(row.source, RowSource::Line { side: RowSide::Both, .. })),
            "unified mode has no two-column row"
        );
    }

    /// A selection never names a line the folded view does not paint.
    #[test]
    fn a_folded_selection_never_names_a_hidden_row() {
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        let mut changed = text.clone();
        changed[25] = "line twenty-five".to_string();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 100));
        viewer.set_texts(text.join("\n"), changed.join("\n"));
        assert!(viewer.select_index(10));

        viewer.set_fold_unchanged(true);
        viewer.display_row_count();

        let selected = viewer.selected_index().expect("a selection survives the fold");
        assert!(
            viewer.display_rows.iter().any(|row| matches!(
                row.source,
                RowSource::Line { index, .. } if index == selected
            )),
            "the selected line {selected} must be painted by some row"
        );
        // Selecting a folded line directly snaps to the nearest painted one.
        assert!(viewer.select_index(5));
        let selected = viewer.selected_index().expect("the snap keeps a selection");
        assert!(viewer.display_rows.iter().any(|row| matches!(
            row.source,
            RowSource::Line { index, .. } if index == selected
        )));
    }

    /// A comparison rebuilt while folding is on reconciles its selection with the folds.
    ///
    /// `recompute` always selected line 0, and never reconciled it with the row list: a fresh
    /// comparison whose first line sits inside a fold reported a selection the paint path skips.
    #[test]
    fn a_rebuilt_comparison_reconciles_its_selection_with_the_folds() {
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_fold_unchanged(true);
        viewer.set_texts(text.join("\n"), text.join("\n"));
        assert_eq!(viewer.display_row_count(), 1, "the whole comparison is one marker");
        assert_eq!(viewer.selected_index(), None, "nothing painted, so nothing is selected");
    }

    /// A selection inside a fold snaps to the nearest painted context row.
    ///
    /// The old `nearest_painted_line` always preferred the row **below** the marker, so a selection
    /// near the top of a collapsed hunk jumped to its far side.
    #[test]
    fn a_hidden_selection_snaps_to_the_nearest_context_row() {
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        let mut changed = text.clone();
        changed[25] = "line twenty-five".to_string();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts(text.join("\n"), changed.join("\n"));
        viewer.set_fold_context_lines(2);
        viewer.set_fold_unchanged(true);
        viewer.display_row_count();

        assert!(viewer.select_index(5));
        let selected = viewer.selected_index().expect("the snap keeps a selection");
        assert!(
            selected <= 5,
            "line 5 must snap to the context row above it, not across the hunk to {selected}"
        );
    }

    /// A document folded into a single marker paints nothing, so it cannot hold a selection.
    ///
    /// `nearest_painted_line` returns `None` when the fold has no painted neighbour on either
    /// side; both `select_index` and the reconcile pass used to keep the hidden index anyway.
    #[test]
    fn folding_the_whole_document_clears_the_selection() {
        let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 640, 300));
        viewer.set_texts(text.join("\n"), text.join("\n"));
        assert!(viewer.select_index(10));

        viewer.set_fold_unchanged(true);
        assert_eq!(viewer.display_row_count(), 1, "the whole comparison is one marker");
        assert_eq!(
            viewer.selected_index(),
            None,
            "a fold with no painted neighbour cannot keep a selected line"
        );

        assert!(viewer.select_index(5));
        assert_eq!(viewer.selected_index(), None, "and a new select of a hidden line is refused");
    }

    /// A viewer shorter than one row paints nothing, so it accepts no click.
    ///
    /// `visible_line_count` used to answer 1 for a negative usable height while `draw` broke
    /// before painting row 0; a press in that band selected a row that was never on screen.
    #[test]
    fn a_viewer_too_short_to_paint_a_row_selects_nothing() {
        let mut viewer = DiffViewer::new(Rect::new(0, 0, 320, 40));
        viewer.set_texts("a\nb", "a\nB");
        assert_eq!(viewer.visible_line_count(), 0, "nothing fits below the headings");
        assert_eq!(viewer.selected_index(), None, "nothing is selected before the click");

        viewer.handle_event(&Event::mouse_press(10, 35, 1));
        assert_eq!(viewer.selected_index(), None, "no painted row, no selection");
    }

    /// `select_file` selects in every mode, and a deselection reports nothing.
    #[test]
    fn select_file_never_deselects_in_multi_mode() {
        use rust_widgets::widget::view_widgets::list_view::SelectionMode;

        let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
        list.set_files(
            ["a.rs", "b.rs", "c.rs"]
                .iter()
                .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
                .collect(),
        );
        list.set_selection_mode(SelectionMode::Multi);
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        list.file_selected.connect(move |path| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(path.as_str().to_string());
            }
        });

        assert!(list.select_file("a.rs"));
        assert!(list.select_file("c.rs"));
        assert!(list.select_file("a.rs"));
        assert_eq!(list.selected_paths(), vec!["a.rs", "c.rs"], "select_file selects");
        assert_eq!(
            *emitted.lock().unwrap(),
            vec!["a.rs", "c.rs"],
            "a path that was not newly selected is never reported"
        );
    }

    /// A `None` view holds no selection, whatever it held before.
    ///
    /// Entering `SelectionMode::None` only truncated a multi-selection to one row, so the published
    /// mode and `selected_paths()` contradicted each other and the highlight stayed painted.
    #[test]
    fn a_none_selection_mode_clears_the_selection() {
        use rust_widgets::widget::view_widgets::list_view::SelectionMode;

        let mut list = DiffFileList::new(Rect::new(0, 0, 300, 200));
        list.set_files(
            ["a.rs", "b.rs"]
                .iter()
                .map(|path| FileDiffEntry::new(*path, FileDiffStatus::Modified))
                .collect(),
        );
        assert!(list.select_file("a.rs"));
        list.set_selection_mode(SelectionMode::None);
        assert!(list.selected_paths().is_empty(), "a `None` view selects nothing");
        assert!(!list.select_file("a.rs"), "and a select request reports that it did nothing");
    }

    /// A path containing the token separator round-trips through the published property.
    #[test]
    fn a_path_containing_a_pipe_round_trips() {
        let entry = FileDiffEntry::new("src/a|b.rs", FileDiffStatus::Renamed).with_counts(2, 3);
        assert_eq!(FileDiffEntry::from_token(&entry.to_token()), Some(entry));
    }

    /// `row_rect` reports only rows the viewport paints.
    #[test]
    fn row_rect_reports_only_painted_rows() {
        let mut list = DiffFileList::new(Rect::new(0, 0, 300, 60));
        list.set_files(
            (0..10)
                .map(|index| FileDiffEntry::new(format!("f{index}.rs"), FileDiffStatus::Modified))
                .collect(),
        );
        assert!(list.row_rect(1).is_some());
        assert_eq!(list.row_rect(2), None, "a two-row viewport does not paint row 2");
    }
}
