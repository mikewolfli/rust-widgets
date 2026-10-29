// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the markdown editor.

use super::outline::OUTLINE_ROW_HEIGHT;
use super::*;
use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::{
    non_primary_modifier_event_bits, primary_modifier_event_bits,
};
use crate::widget::metrics::effective_font;
use crate::widget::special_widgets::code_editor::{LanguageId, LineState, TokenKind};
use crate::widget::{Draw, Widget};
use std::sync::{Arc, Mutex};

#[test]
fn metrics_follow_text_changes() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# Title\nhello world\n## Details");

    assert_eq!(editor.line_count(), 3);
    assert_eq!(editor.word_count(), 6);
    assert_eq!(editor.heading_count(), 2);
}

#[test]
fn preview_toggle_emits_signal() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    let states = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = states.clone();
    editor.preview_mode_changed.connect(move |value| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*value);
        }
    });

    editor.toggle_preview_mode();
    editor.toggle_preview_mode();

    let got = states.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got, vec![true, false]);
}

#[test]
fn arrow_keys_move_cursor() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("a\nb\nc");

    editor.handle_event(&Event::key_press(40, 0));
    editor.handle_event(&Event::key_press(40, 0));
    assert_eq!(editor.cursor_line(), 2);

    editor.handle_event(&Event::key_press(38, 0));
    assert_eq!(editor.cursor_line(), 1);
}

#[test]
fn undo_redo_restores_markdown_text() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# One");
    editor.append_line("body");

    assert!(editor.can_undo());
    assert!(editor.undo());
    assert_eq!(editor.text(), "# One");
    assert!(editor.can_redo());
    assert!(editor.redo());
    assert_eq!(editor.text(), "# One\nbody");
}

#[test]
fn keyboard_shortcuts_drive_markdown_history() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("draft");
    editor.set_text("final");

    editor.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
    assert_eq!(editor.text(), "draft");
    editor.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
    assert_eq!(editor.text(), "final");
}

#[test]
fn keyboard_shortcuts_reject_shifted_and_non_primary_chords() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("draft");
    editor.set_text("final");

    let primary = primary_modifier_event_bits();
    // Shift+Primary+Z is a different chord, so it is not plain undo.
    editor.handle_event(&Event::key_press(90, primary | 0b0001));
    assert_eq!(editor.text(), "final");

    // The accelerator that is not primary on this host does not undo.
    editor.handle_event(&Event::key_press(90, non_primary_modifier_event_bits()));
    assert_eq!(editor.text(), "final");

    editor.handle_event(&Event::key_press(90, primary));
    assert_eq!(editor.text(), "draft");
    editor.handle_event(&Event::key_press(89, primary));
    assert_eq!(editor.text(), "final");
}

#[test]
fn text_input_inserts_text_and_emits_signal() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    let seen = Arc::new(Mutex::new(Vec::<MarkdownTextChange>::new()));
    let sink = seen.clone();
    editor.text_changed.connect(move |value| {
        if let Ok(mut guard) = sink.lock() {
            guard.push((*value).clone());
        }
    });

    editor.handle_event(&Event::text_input("# Hello"));

    assert_eq!(editor.text(), "# Hello");
    let got = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].offset, 0);
    assert_eq!(got[0].removed, "");
    assert_eq!(got[0].inserted, "# Hello");
    assert_eq!(got[0].len, 7);
}

#[test]
fn ime_commit_inserts_cjk_text() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.handle_event(&Event::ime_commit("你好"));

    assert_eq!(editor.text(), "你好");
    assert_eq!(editor.cursor_col(), 2);
}

#[test]
fn typed_text_can_be_undone_and_redone() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.handle_event(&Event::text_input("first"));
    editor.handle_event(&Event::ime_commit(" second"));
    assert_eq!(editor.text(), "first second");

    assert!(editor.undo());
    assert_eq!(editor.text(), "first");
    assert!(editor.undo());
    assert_eq!(editor.text(), "");

    assert!(editor.redo());
    assert!(editor.redo());
    assert_eq!(editor.text(), "first second");
}

#[test]
fn enter_splits_and_backspace_merges_lines() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("ab");

    editor.handle_event(&Event::key_press(39, 0));
    editor.handle_event(&Event::key_press(39, 0));
    editor.handle_event(&Event::key_press(13, 0));
    assert_eq!(editor.text(), "ab\n");
    assert_eq!(editor.cursor_line(), 1);
    assert_eq!(editor.cursor_col(), 0);

    editor.handle_event(&Event::key_press(8, 0));
    assert_eq!(editor.text(), "ab");
    assert_eq!(editor.cursor_line(), 0);
    assert_eq!(editor.cursor_col(), 2);

    editor.handle_event(&Event::key_press(37, 0));
    editor.handle_event(&Event::key_press(8, 0));
    assert_eq!(editor.text(), "b");
    assert_eq!(editor.cursor_col(), 0);
}

#[test]
fn arrow_keys_clamp_cursor_column_to_line_length() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("long line\nx");

    editor.handle_event(&Event::key_press(39, 0));
    editor.handle_event(&Event::key_press(39, 0));
    assert_eq!(editor.cursor_col(), 2);

    editor.handle_event(&Event::key_press(40, 0));
    assert_eq!(editor.cursor_line(), 1);
    assert_eq!(editor.cursor_col(), 1);
}

/// The wheel scrolls the visible line window and clamps at both ends.
///
/// # The defect this pins
///
/// The paint path stopped at a fixed `take(10)`, so a longer document could not be reached at
/// all. This drives the same window the paint path now starts from.
#[test]
fn markdown_editor_wheel_scrolls_and_clamps() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 100));
    editor.set_text(text.join("\n"));
    assert_eq!(editor.first_visible_line, 0);

    // One notch toward the end reveals three lines.
    editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
    assert_eq!(editor.first_visible_line, 3, "a downward notch moves the viewport down");

    // The other direction returns to the top and clamps there.
    editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
    assert_eq!(editor.first_visible_line, 0);
    editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
    assert_eq!(editor.first_visible_line, 0, "the viewport clamps at the first line");

    // Scrolling on stops at the last full window.
    let max_first = editor.line_count() - editor.visible_line_count();
    for _ in 0..20 {
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
    }
    assert_eq!(editor.first_visible_line, max_first, "the viewport clamps at the last window");
}

/// A whole-text replacement still records only the changed middle for undo.
#[test]
fn set_text_undoes_a_paragraph_change_without_losing_the_rest() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha\nbeta\ngamma");
    editor.set_text("alpha\nBETA 你好\ngamma");

    assert!(editor.undo());
    assert_eq!(editor.text(), "alpha\nbeta\ngamma");
    assert!(editor.redo());
    assert_eq!(editor.text(), "alpha\nBETA 你好\ngamma");
}

/// Runs `f` with a real paint context, for the paths that need measurements.
fn with_context<T>(f: impl FnOnce(&RenderContext) -> T) -> T {
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(400, 300), 1.0);
    backend.begin_frame(Color::WHITE);
    let result = {
        let context = RenderContext::new(&mut backend);
        f(&context)
    };
    backend.end_frame();
    result
}

/// The counts are cached against the revision, so a still frame walks no document.
///
/// `line_count`/`word_count`/`heading_count` each scanned the whole document, and the paint
/// path called all three per frame.
#[test]
fn metrics_are_cached_until_the_revision_changes() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# One\ntwo words here");
    assert_eq!((editor.line_count(), editor.word_count(), editor.heading_count()), (2, 5, 1));
    let cached = *editor.metrics.borrow();
    assert_eq!(cached.map(|(revision, _)| revision), Some(editor.revision));

    editor.set_text("# One\ntwo words here\nthree");
    assert_eq!(editor.word_count(), 6, "an edit invalidates the cache");
    assert_eq!(
        editor.metrics.borrow().map(|(revision, _)| revision),
        Some(editor.revision),
        "and the next read refreshes it"
    );
}

/// An IME composition is visible before it is committed, anchored at the caret.
#[test]
fn ime_preedit_is_visible_at_the_caret() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("ab");
    // `set_text` leaves the caret where it was, so it is moved to the end explicitly.
    editor.handle_event(&Event::key_press(35, 0));
    assert!(editor.preedit_screen_rect().is_none(), "nothing is composing yet");

    editor.handle_event(&Event::ime_preedit("你好", 3));
    let at_end = editor.preedit_screen_rect().expect("a composition has a rectangle");
    assert_eq!(editor.preedit.as_ref().map(|p| p.cursor), Some(3));

    editor.handle_event(&Event::key_press(37, 0));
    let mid_line = editor.preedit_screen_rect().expect("still composing");
    assert!(mid_line.x < at_end.x, "the composition moved back with the caret");

    editor.handle_event(&Event::ime_commit("你好"));
    assert!(editor.preedit_screen_rect().is_none(), "a commit ends the composition");
}

/// The preview recognises the GFM blocks and inline styles it used to print as source.
#[test]
fn preview_recognises_gfm_constructs() {
    with_context(|context| {
        let style = crate::style::WidgetStyle::default();
        let font = effective_font(&style);
        let heading =
            parse_preview_line("# Title", BlockStart::Normal, true, true, false, font, context);
        assert!(matches!(heading.row, PreviewRow::Heading(1)));
        assert!(heading.runs.iter().all(|run| !run.text.contains('#')));

        let task =
            parse_preview_line("- [x] done", BlockStart::Normal, true, true, false, font, context);
        assert_eq!(task.runs.first().map(|run| run.text.as_str()), Some("☑ "));

        let strike =
            parse_preview_line("~~gone~~", BlockStart::Normal, true, true, false, font, context);
        assert!(strike.runs.iter().any(|run| run.style == PreviewStyle::Strike));

        let quote =
            parse_preview_line("> quoted", BlockStart::Normal, true, true, false, font, context);
        assert_eq!(quote.runs.first().map(|run| run.style), Some(PreviewStyle::Accent));

        let rule = parse_preview_line("---", BlockStart::Normal, true, true, false, font, context);
        assert!(matches!(rule.row, PreviewRow::Rule));

        let fence =
            parse_preview_line("```rust", BlockStart::Fence, true, true, false, font, context);
        assert!(matches!(fence.row, PreviewRow::Fence));

        let table =
            parse_preview_line("| a | b |", BlockStart::Normal, true, true, false, font, context);
        assert!(matches!(table.row, PreviewRow::Table));
        assert!(table.runs.iter().any(|run| run.text.contains('│')));

        // GFM recognises a table by the delimiter row after it; a pipe in prose is not one.
        let header =
            parse_preview_line("a | b", BlockStart::Normal, true, true, true, font, context);
        assert!(matches!(header.row, PreviewRow::Table));
        let prose =
            parse_preview_line("a | b", BlockStart::Normal, true, true, false, font, context);
        assert!(!matches!(prose.row, PreviewRow::Table));

        let image = parse_preview_line(
            "![alt](x.png)",
            BlockStart::Normal,
            true,
            true,
            false,
            font,
            context,
        );
        assert!(image.runs.iter().any(|run| run.style == PreviewStyle::Link));

        // With the extensions off, the same text stays literal.
        let plain =
            parse_preview_line("~~gone~~", BlockStart::Normal, false, true, false, font, context);
        assert!(plain.runs.iter().all(|run| run.style != PreviewStyle::Strike));
        assert!(plain.runs.iter().any(|run| run.text.contains("~~")));
    });
}

/// Fenced code is tokenized with the shared lexer, so keywords are not body text.
#[test]
fn fenced_code_is_highlighted_with_the_shared_lexer() {
    with_context(|context| {
        let style = crate::style::WidgetStyle::default();
        let font = effective_font(&style);
        let parsed = parse_preview_line(
            "let x = 1;",
            BlockStart::Code(LanguageId::Rust, LineState::initial()),
            true,
            true,
            false,
            font,
            context,
        );
        assert!(matches!(parsed.row, PreviewRow::Code));
        assert!(
            parsed.runs.iter().any(|run| run.style == PreviewStyle::Token(TokenKind::Keyword)),
            "`let` must be a keyword token"
        );
    });
}

/// Selection, typing over it and cutting all work on the real document.
#[test]
fn selection_replaces_text_and_cut_deletes_it() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha beta");
    editor.select_all();
    assert_eq!(editor.selected_text().as_deref(), Some("alpha beta"));

    editor.insert_text_at_cursor("X");
    assert_eq!(editor.text(), "X", "typing replaces the selection in one step");
    assert!(editor.selection().is_none());

    editor.set_text("abc");
    editor.select_all();
    editor.cut();
    assert_eq!(editor.text(), "", "cut removes the selected run");
}

/// The selection signal reports presence, and Ctrl+A drives it from the keyboard.
#[test]
fn select_all_emits_selection_changed() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("text");
    let seen = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = seen.clone();
    editor.selection_changed.connect(move |present| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*present);
        }
    });

    editor.handle_event(&Event::key_press(65, primary_modifier_event_bits()));
    assert_eq!(editor.selected_text().as_deref(), Some("text"));

    editor.handle_event(&Event::key_press(37, 0));
    let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(seen, vec![true, false], "a plain arrow clears the selection");
}

/// Home/End/Delete/PageDown and the scrollbar reach the document.
#[test]
fn keyboard_covers_line_edges_delete_and_paging() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 100));
    let text: Vec<String> = (0..30).map(|n| format!("line{n}")).collect();
    editor.set_text(text.join("\n"));

    editor.handle_event(&Event::key_press(35, 0));
    assert_eq!(editor.cursor_col(), "line0".chars().count(), "End reaches the line's end");
    editor.handle_event(&Event::key_press(36, 0));
    assert_eq!(editor.cursor_col(), 0, "Home returns to its start");

    editor.handle_event(&Event::key_press(46, 0));
    assert_eq!(editor.text().split('\n').next(), Some("ine0"), "Delete removes forward");

    assert!(editor.scroll_page(1));
    assert!(editor.first_visible_line > 0, "PageDown moves the viewport");
    editor.handle_event(&Event::key_press(33, 0));
    assert_eq!(editor.first_visible_line, 0, "PageUp returns to the top");
}

/// The caret blinks on the ticks it is given — this control was the only text field in the
/// crate whose caret never moved.
#[test]
fn the_caret_blinks_on_the_ticks_it_is_given() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    assert!(editor.is_animating());
    assert!(editor.is_caret_visible());
    editor.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
    assert!(!editor.is_caret_visible(), "one half-period hides the caret");
    editor.tick(crate::style::animation::CURSOR_BLINK_HALF_PERIOD_MS);
    assert!(editor.is_caret_visible());

    editor.set_enabled(false);
    editor.handle_event(&Event::key_press(65, 0));
    assert!(!editor.is_animating(), "a disabled editor holds a steady caret");
}

/// The scrollbar track jumps the viewport, and clicking a preview heading announces it.
#[test]
fn scrollbar_and_heading_clicks_navigate() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 200));
    let text: Vec<String> = (0..40).map(|n| format!("line{n}")).collect();
    editor.set_text(text.join("\n"));

    editor.handle_event(&Event::mouse_press(414, 190, 1));
    assert!(editor.first_visible_line > 0, "a click near the track's bottom jumps forward");

    let mut headings = MarkdownEditor::new(Rect::new(0, 0, 420, 200));
    headings.set_text("# First\n## Second");
    headings.set_preview_mode(true);
    let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = seen.clone();
    headings.heading_activated.connect(move |index| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*index);
        }
    });
    headings.handle_event(&Event::mouse_press(40, TEXT_TOP + 2, 1));
    let seen = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(seen, vec![0], "the heading row announces its outline index");
}

/// The find bar takes typed text and Enter selects the next match.
#[test]
fn find_bar_selects_matches() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha beta alpha");
    assert!(editor.find_next("alpha"));
    assert_eq!(editor.selection(), Some((0, 5)));
    assert!(editor.find_next("alpha"));
    assert_eq!(editor.selection(), Some((11, 16)), "the next call moves past the match");
    assert!(editor.find_previous("alpha"));
    assert_eq!(editor.selection(), Some((0, 5)));

    editor.open_find();
    assert!(editor.find_bar_visible());
    editor.handle_event(&Event::text_input("beta"));
    assert_eq!(editor.find_query(), "beta", "typing goes to the query, not the document");
    editor.handle_event(&Event::key_press(13, 0));
    assert_eq!(editor.selected_text().as_deref(), Some("beta"));
    editor.handle_event(&Event::key_press(27, 0));
    assert!(!editor.find_bar_visible());
}

#[test]
fn outline_panel_follows_the_editor_caret() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# One\ntext\n## Two\nmore");
    let mut panel = MarkdownOutlinePanel::new(Rect::new(0, 0, 180, 200));
    panel.sync_with_editor(&editor);
    assert_eq!(panel.entries().len(), 2);
    assert_eq!(panel.entries()[0].title, "One");
    assert_eq!(panel.active_index(), Some(0));

    assert!(editor.goto_line(2));
    panel.sync_with_editor(&editor);
    assert_eq!(panel.active_index(), Some(1), "the caret is inside the second heading");

    let selected = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
    let sink = selected.clone();
    panel.entry_selected.connect(move |index| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*index);
        }
    });
    assert_eq!(panel.goto(0), Some(0), "goto reports the line it addresses");
    assert_eq!(*selected.lock().expect("selected lock"), vec![0]);
    assert_eq!(panel.goto(9), None);
}

#[test]
fn find_bar_navigates_case_and_whole_word_matches() {
    let mut bar = FindBar::new(Rect::new(0, 0, 360, 26));
    bar.set_text("Alpha alpha alphabet alpha");
    bar.set_query("alpha");
    assert_eq!(bar.match_count(), 4, "case-insensitive by default, substrings included");
    assert!(bar.find_next());
    assert_eq!(bar.current_range(), Some((0, 5)));
    assert!(bar.find_next());
    assert_eq!(bar.current_range(), Some((6, 11)));

    bar.set_whole_word(true);
    assert_eq!(bar.match_count(), 3);
    bar.set_case_sensitive(true);
    assert_eq!(bar.match_count(), 2, "only the lowercase, whole-word hits remain");
    bar.set_query("zzz");
    assert!(!bar.find_next(), "an empty match list reports no match");
}

#[test]
fn find_bar_replace_request_reaches_the_editor_and_is_undoable() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("cat dog cat");
    let mut bar = FindBar::new(Rect::new(0, 0, 360, 52));
    bar.set_replace_visible(true);
    bar.set_replace_text("fox");
    editor.sync_find_bar(&mut bar);
    bar.set_query("cat");
    assert_eq!(bar.match_count(), 2);

    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::<ReplaceRequest>::new()));
    let sink = requests.clone();
    bar.replace_requested.connect(move |request| {
        if let Ok(mut guard) = sink.lock() {
            guard.push((*request).clone());
        }
    });
    assert!(bar.replace_all());
    let request = requests.lock().expect("requests lock")[0].clone();
    assert_eq!(request.ranges, vec![(0, 3), (8, 11)]);

    assert_eq!(editor.apply_replace_request(&request), 2);
    assert_eq!(editor.text(), "fox dog fox");
    assert!(editor.undo());
    assert_eq!(editor.text(), "cat dog cat");
}

#[test]
fn find_bar_properties_round_trip() {
    let mut bar = FindBar::new(Rect::new(0, 0, 360, 26));
    bar.set("query", CapabilityValue::String("a".to_string())).expect("query writable");
    bar.set("case_sensitive", CapabilityValue::Bool(true)).expect("case writable");
    bar.set("whole_word", CapabilityValue::Bool(true)).expect("word writable");
    bar.set("replace_text", CapabilityValue::String("b".to_string())).expect("replace writable");
    bar.set("visible", CapabilityValue::Bool(true)).expect("visible writable");
    assert!(bar.is_visible());
    assert_eq!(bar.get("query"), Ok(CapabilityValue::String("a".to_string())));
    assert_eq!(bar.get("case_sensitive"), Ok(CapabilityValue::Bool(true)));
    assert_eq!(bar.get("whole_word"), Ok(CapabilityValue::Bool(true)));
    assert_eq!(bar.get("replace_text"), Ok(CapabilityValue::String("b".to_string())));
    assert_eq!(
        bar.set("match_count", CapabilityValue::UInt(0)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
}

/// The undo depth is a constructor argument and a property.
#[test]
fn undo_depth_is_configurable() {
    let mut editor = MarkdownEditor::with_undo_limit(Rect::new(0, 0, 420, 240), 2);
    assert_eq!(editor.undo_stack_limit(), 2);
    editor.set_text("a");
    editor.insert_text_at_cursor("b");
    editor.insert_text_at_cursor("c");

    assert!(editor.undo());
    assert!(editor.undo());
    assert!(!editor.undo(), "only the configured depth is retained");
    assert_eq!(editor.text(), "a");

    editor.set_undo_limit(8);
    assert_eq!(editor.undo_stack_limit(), 8);
}

/// The new properties round-trip through the property contract.
#[test]
fn new_properties_round_trip() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    assert_eq!(editor.get("gfm_extensions").ok(), Some(CapabilityValue::Bool(true)));
    editor.set("gfm_extensions", CapabilityValue::Bool(false)).expect("writable");
    assert!(!editor.gfm_extensions());
    editor.set("code_highlight", CapabilityValue::Bool(false)).expect("writable");
    assert!(!editor.code_highlight());
    editor.set("show_scrollbar", CapabilityValue::Bool(false)).expect("writable");
    assert!(!editor.show_scrollbar());
    editor.set("undo_stack_limit", CapabilityValue::UInt(7)).expect("writable");
    assert_eq!(editor.undo_stack_limit(), 7);
}

/// The editor mounts a real outline panel: a press in its side column navigates, and the
/// painted rows come from the same heading list `outline()` reports.
#[test]
fn mounted_outline_panel_navigates_and_paints() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# One\ntext\n## Two\nmore");
    editor.set_show_outline(true);
    assert_eq!(editor.outline_panel().entries().len(), 2);
    assert_eq!(editor.outline_panel().active_index(), Some(0));

    // A first paint places the companion and proves the path draws without panicking.
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(420, 240), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        editor.draw(&mut context);
    }
    backend.end_frame();

    // The panel is 180 px wide, so it starts at x = 240; the second row sits one row down.
    editor.handle_event(&Event::mouse_press(250, TEXT_TOP + OUTLINE_ROW_HEIGHT + 2, 1));
    assert_eq!(editor.cursor_line(), 2, "the second outline row navigates to line 2");
    assert_eq!(editor.outline_panel().active_index(), Some(1));
}

/// The mounted find bar is the painted form of the editor's own find state: typing goes to the
/// same query the editor's Enter handler searches, and closing mirrors both.
#[test]
fn mounted_find_bar_reflects_the_editor_query() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha beta alpha");
    editor.show_find_bar();
    assert!(editor.find_bar().is_visible());
    assert_eq!(editor.find_bar().text(), "alpha beta alpha");

    editor.handle_event(&Event::text_input("beta"));
    assert_eq!(editor.find_query(), "beta");
    assert_eq!(editor.find_bar().query(), "beta");
    assert_eq!(editor.find_bar().match_count(), 1);

    editor.handle_event(&Event::key_press(27, 0));
    assert!(!editor.find_bar_visible());
    assert!(!editor.find_bar().is_visible());
}
