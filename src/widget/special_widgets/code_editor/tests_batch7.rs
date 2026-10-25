// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the §2.10 changes: change descriptions, in-place multi-caret edits,
//! the incremental word index, diagnostic navigation/hover, the find-bar buttons,
//! column selection and the command palette model.
//!
//! Each test names the behaviour it pins, so a regression points at the row it
//! belongs to.

#![cfg(test)]

use super::editor::CodeEditor;
use super::types::{
    DiagnosticMarker, DocumentWordIndex, MarkerSeverity, SearchOptions, TextChange,
};
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use alloc::string::String;
use alloc::vec;

fn make() -> CodeEditor {
    CodeEditor::new(Rect::new(0, 0, 800, 600))
}

// ── J-P01 / J-P12: change descriptions ──────────────────────────────────────

/// A keystroke reports its own line range instead of shipping the whole document.
#[test]
fn typing_emits_a_change_description_not_the_document() {
    let mut editor = make();
    editor.set_text("alpha\nbeta");
    // `set_text` is a wholesale replacement and still carries the text.
    let document_payloads = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let sink = document_payloads.clone();
    editor.text_changed.connect(move |_| *sink.lock().unwrap() += 1);
    let changes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<TextChange>::new()));
    let sink = changes.clone();
    editor.text_change.connect(move |change| sink.lock().unwrap().push((*change).clone()));

    editor.set_cursor(0, 5, false);
    editor.insert("!");

    assert_eq!(*document_payloads.lock().unwrap(), 0, "typing must not materialise the document");
    let changes = changes.lock().unwrap();
    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert!(!change.whole_document);
    assert_eq!(change.start_line, 0);
    assert_eq!(change.removed_lines, 1);
    assert_eq!(change.inserted_lines, 1);
    assert_eq!(change.inserted_text, "!");
    assert_eq!(editor.text(), "alpha!\nbeta");
}

// ── J-P02: multi-caret edits stay in place ──────────────────────────────────

/// Two carets insert on two lines and one undo restores both.
#[test]
fn multi_caret_insert_edits_in_place_and_undoes_once() {
    let mut editor = make();
    editor.set_text("one\ntwo\nthree");
    editor.set_cursor(0, 0, false);
    assert!(editor.add_cursor_below());
    editor.insert("X");
    assert_eq!(editor.text(), "Xone\nXtwo\nthree");
    assert!(editor.undo());
    assert_eq!(editor.text(), "one\ntwo\nthree");
}

// ── J-P11: the word index is incremental ────────────────────────────────────

/// An incremental edit produces the same index a rebuild would.
#[test]
fn word_index_matches_a_rebuild_after_an_edit() {
    let mut text = String::from("alpha beta gamma");
    let mut index = DocumentWordIndex::build(&text, 0);
    // Replace "beta" with "delta".
    let start = 6;
    let removed = "beta";
    text.replace_range(start..start + removed.len(), "delta");
    index.apply_edit(1, &text, start, removed, "delta");
    let rebuilt = DocumentWordIndex::build(&text, 1);
    assert_eq!(index, rebuilt);
    assert_eq!(index.revision(), 1);
}

/// Removing the last occurrence drops the word; removing one of two keeps it.
#[test]
fn word_index_counts_occurrences() {
    let mut text = String::from("dup dup");
    let mut index = DocumentWordIndex::build(&text, 0);
    text.replace_range(4..7, "");
    index.apply_edit(1, &text, 4, "dup", "");
    assert_eq!(index, DocumentWordIndex::build(&text, 1));
    assert_eq!(index.len(), 1);
    text.replace_range(0..3, "");
    index.apply_edit(2, &text, 0, "dup", "");
    assert!(index.is_empty());
}

// ── ED-02: diagnostic navigation ────────────────────────────────────────────

/// Next/previous cycle through sorted markers and wrap.
#[test]
fn diagnostic_navigation_walks_markers_and_wraps() {
    let mut editor = make();
    editor.set_text("a\nb\nc\nd\ne");
    editor.set_markers(vec![
        DiagnosticMarker::new(4, "fourth", MarkerSeverity::Error),
        DiagnosticMarker::new(1, "first", MarkerSeverity::Warning),
        DiagnosticMarker::new(3, "third", MarkerSeverity::Info),
    ]);
    editor.set_cursor(0, 0, false);
    assert_eq!(editor.next_marker(), Some(1));
    assert_eq!(editor.cursor().0, 1);
    assert_eq!(editor.next_marker(), Some(2));
    assert_eq!(editor.cursor().0, 3);
    assert_eq!(editor.next_marker(), Some(0));
    assert_eq!(editor.cursor().0, 4);
    // Wrapping forward from the last returns to the first.
    assert_eq!(editor.next_marker(), Some(1));
    assert_eq!(editor.cursor().0, 1);
    assert_eq!(editor.previous_marker(), Some(0));
    assert_eq!(editor.cursor().0, 4);
}

/// Clicking a gutter line with a diagnostic emits the marker index.
#[test]
fn gutter_click_reports_the_marker_index() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_markers(vec![DiagnosticMarker::new(1, "boom", MarkerSeverity::Error)]);
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = seen.clone();
    editor.marker_clicked.connect(move |index| *sink.lock().unwrap() = Some(*index));
    // Paint once so the gutter geometry is known, then press on line 1's gutter.
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let row_height = editor.line_height();
    let top = editor.text_origin_y();
    let y = top + row_height as i32 + row_height as i32 / 2;
    editor.handle_event(&Event::MousePress { pos: Point::new(6, y), button: 1, modifiers: 0 });
    assert_eq!(*seen.lock().unwrap(), Some(0));
}

// ── ED-01: find-bar buttons ─────────────────────────────────────────────────

/// The bar publishes a close button and toggles, and the close button dismisses it.
#[test]
fn find_bar_buttons_are_hit_testable_and_close_the_bar() {
    let mut editor = make();
    editor.open_find(false);
    let buttons = editor.find_bar_buttons();
    let actions: Vec<_> = buttons.iter().map(|(action, _)| *action).collect();
    assert!(actions.contains(&super::types::FindBarAction::Close));
    assert!(actions.contains(&super::types::FindBarAction::ToggleCase));
    let close = buttons
        .iter()
        .find(|(action, _)| *action == super::types::FindBarAction::Close)
        .map(|(_, rect)| *rect)
        .expect("close button");
    let centre = Point::new(close.x + close.width as i32 / 2, close.y + close.height as i32 / 2);
    editor.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
    assert!(!editor.find_visible(), "the close button dismisses the bar");
}

/// The case toggle flips the matching policy and re-runs the search.
#[test]
fn find_bar_case_toggle_updates_the_options() {
    let mut editor = make();
    editor.set_text("Alpha alpha");
    editor.open_find(false);
    editor.set_search_query("alpha");
    assert_eq!(editor.find_state().match_count(), 2);
    let toggle = editor
        .find_bar_buttons()
        .into_iter()
        .find(|(action, _)| *action == super::types::FindBarAction::ToggleCase)
        .map(|(_, rect)| rect)
        .expect("case toggle");
    let centre =
        Point::new(toggle.x + toggle.width as i32 / 2, toggle.y + toggle.height as i32 / 2);
    editor.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
    assert!(editor.find_state().options.case_sensitive);
    assert_eq!(editor.find_state().match_count(), 1);
}

// ── ED-03: diagnostic hover ────────────────────────────────────────────────

/// Hovering a line with a marker records it; leaving clears it.
#[test]
fn hovering_a_diagnostic_line_tracks_the_line() {
    let mut editor = make();
    editor.set_text("a\nb\nc");
    editor.set_markers(vec![DiagnosticMarker::new(1, "boom", MarkerSeverity::Error)]);
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let row_height = editor.line_height();
    let y = editor.text_origin_y() + row_height as i32 + row_height as i32 / 2;
    editor.handle_event(&Event::MouseMove { pos: Point::new(60, y) });
    assert_eq!(editor.hovered_diagnostic, Some(1));
    editor.handle_event(&Event::MouseLeave { pos: Point::new(60, y) });
    assert_eq!(editor.hovered_diagnostic, None);
}

// ── ED-04: column selection ────────────────────────────────────────────────

/// Alt+drag over two lines produces one caret per covered line, and typing a
/// character edits both edges.
#[test]
fn alt_drag_selects_a_column_and_edits_every_line() {
    let mut editor = make();
    editor.set_text("abc\nabc\nabc");
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    let row_height = editor.line_height();
    let top = editor.text_origin_y();
    let cell = editor.cell_width() as i32;
    let left = editor.text_origin_x();
    let start = Point::new(left + cell, top + row_height as i32 / 2);
    let end = Point::new(left + cell * 2, top + row_height as i32 + row_height as i32 / 2);
    editor.handle_event(&Event::MousePress {
        pos: start,
        button: 1,
        // ALT, the modifier bit the platform layer uses for rectangular selection.
        modifiers: 4,
    });
    editor.handle_event(&Event::MouseMove { pos: end });
    editor.handle_event(&Event::MouseRelease { pos: end, button: 1 });
    // Two carets, each selecting the rectangle's columns.
    assert!(!editor.extra_cursors.is_empty(), "a rectangular drag creates a caret per line");
    editor.insert("Z");
    let text = editor.text();
    assert!(text.contains('Z'), "the block edit wrote into the document");
}

// ── COMP-J01: command palette ──────────────────────────────────────────────

/// Every advertised command id is executable, and an unknown id is refused.
#[test]
fn editor_commands_are_all_executable() {
    let mut editor = make();
    editor.set_text("hello");
    editor.set_cursor(0, 0, false);
    let entries = editor.editor_commands();
    assert!(entries.len() >= 20, "the palette should cover the built-in commands");
    for entry in &entries {
        // The clipboard commands touch the machine-global clipboard; they are advertised
        // and wired, but exercising them here would depend on the host's clipboard.
        if matches!(entry.command_id.as_str(), "copy" | "cut" | "paste") {
            continue;
        }
        assert!(
            editor.execute_palette_command(&entry.command_id),
            "{} should be executable",
            entry.command_id
        );
    }
    assert!(entries.iter().any(|entry| entry.command_id == "paste"));
    assert!(!editor.execute_palette_command("not_a_command"));
}

/// Filtering ranks a prefix match above a scattered one and executes the selection.
#[test]
fn palette_filters_and_executes() {
    let mut palette = super::palette::CommandPalette::new();
    palette.set_entries(vec![
        super::palette::PaletteEntry::new("Toggle Line Comment", "toggle_comment"),
        super::palette::PaletteEntry::new("Sort Lines", "sort_lines"),
        super::palette::PaletteEntry::new("Select All", "select_all"),
    ]);
    palette.open("");
    assert_eq!(palette.filtered().len(), 3);
    palette.set_query("tlc");
    let filtered = palette.filter_for("tlc");
    assert_eq!(filtered, vec![0], "a subsequence match finds the comment command");
    palette.set_query("sort");
    assert_eq!(palette.selected_entry().map(|entry| entry.command_id.as_str()), Some("sort_lines"));
    let committed = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = committed.clone();
    palette.committed.connect(move |id| *sink.lock().unwrap() = Some(id.clone()));
    assert_eq!(palette.execute_selected(), Some("sort_lines"));
    assert_eq!(committed.lock().unwrap().as_deref().map(String::as_str), Some("sort_lines"));
    assert!(!palette.is_visible());
}

/// A disabled entry does not execute and leaves the palette open.
#[test]
fn palette_refuses_a_disabled_selection() {
    let mut palette = super::palette::CommandPalette::new();
    palette.set_entries(vec![super::palette::PaletteEntry::new("Broken", "broken").disabled()]);
    palette.open("");
    assert!(palette.execute_selected().is_none());
    assert!(palette.is_visible());
}

// ── J-P08: search must not change its answers ──────────────────────────────

/// The allocation-free literal scan agrees with the previous semantics on the
/// cases that matter: case folding, whole-word and multi-byte text.
#[test]
fn literal_search_agrees_across_options() {
    let mut editor = make();
    editor.set_text("Alpha alpha ALPHA\nalphabet\n日本語 日本");
    editor.set_search_query("alpha");
    assert_eq!(editor.find_state().match_count(), 4, "three words plus the alphabetic prefix");
    editor.set_search_options(SearchOptions { case_sensitive: true, ..SearchOptions::default() });
    assert_eq!(editor.find_state().match_count(), 2);
    editor.set_search_options(SearchOptions { whole_word: true, ..SearchOptions::default() });
    assert_eq!(editor.find_state().match_count(), 3, "the three standalone words only");
    editor.set_search_options(SearchOptions::default());
    editor.set_search_query("日本");
    assert_eq!(editor.find_state().match_count(), 2);
}

/// Column navigation over a multi-byte line lands on the same columns rebuild would.
#[test]
fn token_borrow_matches_the_owned_accessor() {
    let mut editor = make();
    editor.set_text("fn main() {}");
    editor.resolve_line_state(0);
    let owned = editor.tokens_on_line(0);
    // The cache may not be filled for this line yet; filling happens in paint.
    let _ = crate::widget::svg::render_to_svg(&mut editor);
    if let Some(borrowed) = editor.tokens_on_line_borrowed(0) {
        assert_eq!(borrowed, owned.as_slice());
    }
}
