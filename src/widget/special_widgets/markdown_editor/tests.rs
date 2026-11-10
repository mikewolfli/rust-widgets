// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the markdown editor.

use super::outline::OUTLINE_ROW_HEIGHT;
use super::*;
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::{
    non_primary_modifier_event_bits, primary_modifier_event_bits,
};
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::special_widgets::editor::syntax::{LanguageId, LineState, TokenKind};
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

/// Paints one frame of `editor` and returns its RGBA bytes.
fn draw_frame(editor: &mut MarkdownEditor, width: u32, height: u32) -> Vec<u8> {
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(width, height), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        editor.draw(&mut context);
    }
    backend.end_frame();
    backend.frame_rgba().to_vec()
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
///
/// The assertion is about the *real* lexer, so it carries the same feature the
/// lexer does; with `builtin-languages` off the honest result is plain body text
/// (BLUE31 §1.8).
#[cfg(feature = "builtin-languages")]
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

/// A multibyte match used to trap `find_next`: the next origin was `current + 1` bytes,
/// inside the character, so `str::get` failed and the wrap-around search found the same
/// match forever.
#[test]
fn find_next_advances_past_a_multibyte_match() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("é foo é");

    assert!(editor.find_next("é"));
    let first = editor.cursor_col();
    assert!(editor.find_next("é"));

    assert!(editor.cursor_col() > first, "the second match is selected");
    assert_eq!(editor.cursor_col(), 7, "the caret sits after the second é");
}

/// A replace request is data from the host. A range that is not on a character boundary
/// used to reach `String::replace_range` and panic; the contract is "applies nothing".
#[test]
fn replace_request_on_non_boundary_range_replaces_nothing() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("é");
    let request = ReplaceRequest { ranges: vec![(0, 1)], text: "x".to_string() };

    assert_eq!(editor.apply_replace_request(&request), 0);
    assert_eq!(editor.text(), "é");
}

/// Jumping through the outline is a move, not an edit: the old selection must not stay
/// behind (the next keystroke would replace text that is no longer on screen).
#[test]
fn outline_jump_clears_a_stale_selection() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("# One\nalpha\n# Two");
    editor.select_all();

    assert!(editor.goto_outline(1));
    editor.insert_text_at_cursor("X");

    assert!(editor.text().contains("alpha"), "the old selection was not replaced");
    assert!(editor.text().contains("# One"));
}

/// Typing while the replacement row is focused belongs to that row.
///
/// The editor used to append every `TextInput` to its own query state, so a host could show
/// the replacement row but never fill it.
#[test]
fn typing_into_the_replacement_row_edits_the_replacement() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha beta");
    editor.show_find_bar();
    editor.find_bar_mut().set_replace_visible(true);

    let band = editor.find_bar_rect();
    let press = Point::new(band.x + 10, band.y + band.height as i32 - 5);
    editor.handle_event(&Event::mouse_press(press.x, press.y, 1));
    editor.handle_event(&Event::text_input("beta"));

    assert_eq!(editor.find_query(), "", "the query field was not focused");
    assert_eq!(editor.find_bar().replace_text(), "beta");
    assert_eq!(editor.text(), "alpha beta", "the document is untouched");
}

/// The outline, the metrics and the preview share one heading rule.
#[test]
fn headings_mean_the_same_thing_to_the_outline_metrics_and_preview() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    // `#tag` has no space and seven hashes is not a heading (CommonMark/GFM).
    editor.set_text("# One\n#tag\n####### seven\n## Two");

    assert_eq!(editor.heading_count(), 2);
    assert_eq!(editor.outline().len(), 2);

    let parsed = with_context(|context| {
        parse_preview_line(
            "#tag",
            BlockStart::Normal,
            true,
            true,
            false,
            effective_font(editor.base.style()),
            context,
        )
    });
    assert!(!matches!(parsed.row, PreviewRow::Heading(_)));
}

/// A fence closes only on its own marker, at least as long as the opener.
#[test]
fn a_longer_fence_is_not_closed_by_an_inner_shorter_one() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("````rust\n```\nlet x = 1;\n````\n");
    editor.ensure_blocks();

    assert!(matches!(editor.blocks[0], BlockStart::Fence));
    assert!(matches!(editor.blocks[1], BlockStart::Code(_, _)), "``` cannot close a ```` block");
    assert!(matches!(editor.blocks[2], BlockStart::Code(_, _)));
    assert!(matches!(editor.blocks[3], BlockStart::Fence), "the four-backtick line closes it");
}

/// CRLF is normalised on load, so a line merge cannot leave a `\r` behind.
#[test]
fn a_crlf_document_stores_one_terminator_per_line() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("one\r\ntwo\r\n");
    assert_eq!(editor.text(), "one\ntwo\n");
    assert!(!editor.text().contains('\r'));
}

/// An unmatched backtick is literal text, not an unclosed code span.
#[test]
fn an_unclosed_backtick_does_not_style_the_rest_of_the_row_as_code() {
    let editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    let parsed = with_context(|context| {
        parse_preview_line(
            "a ` b",
            BlockStart::Normal,
            true,
            true,
            false,
            effective_font(editor.base.style()),
            context,
        )
    });
    assert!(
        parsed.runs.iter().all(|run| run.style != PreviewStyle::Code),
        "an unmatched backtick must not paint the rest of the line as code"
    );
}

/// Only rows that fit fully are counted, painted and pointer-mapped.
#[test]
fn only_rows_that_fully_fit_are_counted_and_mapped() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 500, 300));
    // The text area is 300 - 36 - 8 = 256 px, which holds exactly 16 rows of 16 px.
    assert_eq!(editor.visible_line_count(), 16);

    let text: Vec<String> = (0..100).map(|index| format!("line {index}")).collect();
    editor.set_text(text.join("\n"));
    assert!(editor.scroll_lines(1000));
    let last = editor.edit_line_count() - 1;
    assert_eq!(
        editor.visible_start() + editor.visible_line_count() - 1,
        last,
        "the final line is on screen at the last scroll position"
    );
    assert_eq!(
        editor.line_at_viewport_y(TEXT_TOP + 16 * TEXT_ROW_HEIGHT),
        None,
        "the row past the clip maps to no line"
    );
}

/// A click in the header strip or the bottom inset does not resolve to a text row.
#[test]
fn pointer_rows_reject_the_header_strip_and_the_bottom_inset() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    let text: Vec<String> = (0..20).map(|index| format!("line {index}")).collect();
    editor.set_text(text.join("\n"));

    assert_eq!(editor.line_at_viewport_y(TEXT_TOP - 1), None, "the header strip is not text");
    assert_eq!(editor.line_at_viewport_y(TEXT_TOP), Some(0));
    assert_eq!(
        editor.line_at_viewport_y(300 - TEXT_BOTTOM_INSET),
        None,
        "the bottom inset is not a row"
    );
    assert_eq!(
        editor.line_at_viewport_y(300 - TEXT_BOTTOM_INSET - 1),
        Some(15),
        "the last full row still maps"
    );
}

/// An edit that moves the caret off the painted rows scrolls the new row into view.
#[test]
fn a_commit_reveals_the_caret_it_moves() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    let text: Vec<String> = (0..20).map(|index| format!("line {index}")).collect();
    editor.set_text(text.join("\n"));
    assert!(editor.goto_line(15));
    assert_eq!(editor.visible_start(), 0);

    editor.handle_event(&Event::key_press(13, 0));
    assert_eq!(editor.cursor_line(), 16);
    assert!(
        editor.cursor_line() < editor.visible_start() + editor.visible_line_count(),
        "the committed new line is on screen"
    );
    assert_eq!(editor.visible_start(), 1);
}

/// The mounted bar's policy, not a hardcoded one, decides what a keyboard find matches.
#[test]
fn find_next_honours_the_mounted_bar_policy() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("Alpha alpha");
    assert!(editor.find_next("alpha"));
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("Alpha"),
        "the bar is case-insensitive by default"
    );

    let mut strict = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    strict.set_text("Alpha alpha");
    strict.find_bar_mut().set_case_sensitive(true);
    assert!(strict.find_next("alpha"));
    assert_eq!(
        strict.selected_text().as_deref(),
        Some("alpha"),
        "the bar's case-sensitive policy is honoured"
    );
}

/// A user caret move invalidates the previous hit, so the next search starts there.
#[test]
fn a_caret_move_restarts_the_search_from_the_caret() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha alpha alpha");
    assert!(editor.find_next("alpha"));
    assert!(editor.find_next("alpha"));
    assert_eq!(editor.selection(), Some((6, 11)));

    assert!(editor.goto_line(0));
    editor.move_to_line_edge(true);
    assert!(editor.find_next("alpha"));
    assert_eq!(
        editor.selection(),
        Some((0, 5)),
        "the moved caret is the origin, not the stale hit"
    );
}

/// A line-prefix toggle keeps a caret where it was in the text and reports real edits only.
#[test]
fn line_prefix_edits_keep_the_caret_and_report_only_real_changes() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("  item");
    editor.cursor_line = 0;
    editor.cursor_col = 2;

    assert!(editor.toggle_line_prefix("  "));
    assert_eq!(editor.text(), "item");
    assert_eq!(editor.cursor_col(), 0, "the caret keeps its place in the text");
    assert!(editor.selection().is_none());

    editor.set_text("item");
    editor.cursor_line = 0;
    editor.cursor_col = 2;
    assert!(editor.toggle_line_prefix("- "));
    assert_eq!(editor.text(), "- item");
    assert_eq!(editor.cursor_col(), 4, "the caret follows the inserted prefix");

    editor.set_text("\n\n");
    editor.select_all();
    assert!(!editor.toggle_line_prefix("- "), "an unchanged document reports no change");
    assert_eq!(editor.text(), "\n\n");
}

/// The open find bar is painted over the document rows, so its fields stay readable.
#[test]
fn document_rows_do_not_overprint_the_open_find_bar() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_text("M".repeat(200));
    editor.show_find_bar();
    editor.find_bar_mut().set_replace_visible(true);
    let with_text = draw_frame(&mut editor, 420, 300);
    let bar = editor.find_bar_rect();

    let mut empty = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    empty.show_find_bar();
    empty.find_bar_mut().set_replace_visible(true);
    let without_text = draw_frame(&mut empty, 420, 300);

    for y in bar.y..bar.y + bar.height as i32 {
        for x in bar.x..bar.x + bar.width as i32 {
            let index = ((y as u32 * 420 + x as u32) * 4) as usize;
            assert_eq!(
                &with_text[index..index + 4],
                &without_text[index..index + 4],
                "a document row overprints the find bar at ({x}, {y})"
            );
        }
    }
}

/// Preview lays rows out from parsed runs, so it does not paint the source-metric band.
#[test]
fn preview_does_not_paint_the_source_metric_selection_band() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_preview_mode(true);
    editor.set_text("**bold** text");
    let plain = draw_frame(&mut editor, 420, 300);

    let length = editor.text().len();
    editor.select_byte_range(0, length);
    let selected = draw_frame(&mut editor, 420, 300);

    let font = effective_font(editor.base().style());
    let source_width = estimate_text_width("**bold** text", font, 1.0) as i32;
    let x = (12 + source_width - 4) as usize;
    let y = (TEXT_TOP + TEXT_ROW_HEIGHT - 1) as usize;
    let index = (y * 420 + x) * 4;
    assert_eq!(
        &selected[index..index + 4],
        &plain[index..index + 4],
        "the preview row paints no band measured from the source line"
    );
}

/// An outline entry title may contain the wire separator and still round-trip.
#[test]
fn outline_entries_round_trip_a_title_containing_the_separator() {
    let mut panel = MarkdownOutlinePanel::new(Rect::new(0, 0, 180, 200));
    panel.set_entries(vec![MarkdownOutlineEntry { line: 3, level: 2, title: "a | b".to_string() }]);

    let encoded = panel.get("entries").expect("entries readable");
    let CapabilityValue::Tuple(rows) = encoded else { panic!("entries is a string list") };
    assert_eq!(rows.len(), 1);
    let CapabilityValue::String(token) = &rows[0] else { panic!("entry is a string") };
    assert_eq!(token, "2|a \\| b|3");

    let mut restored = MarkdownOutlinePanel::new(Rect::new(0, 0, 180, 200));
    restored.set("entries", CapabilityValue::Tuple(rows)).expect("entries writable");
    assert_eq!(restored.entries(), panel.entries());
}

/// A shift-press moves the caret and rebuilds the selection in the same event.
#[test]
fn a_shift_press_rebuilds_the_selection_from_the_anchor() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    let line = "abcdefghij".repeat(4);
    editor.set_text(line.as_str());
    editor.select_byte_range(0, 5);

    let font = effective_font(editor.base().style());
    let x = 12 + estimate_text_width(&line[..40], font, 1.0) as i32;
    editor.handle_event(&Event::mouse_press_with(x, TEXT_TOP + 2, 1, 0b0001));
    assert_eq!(editor.cursor_col(), 40);
    assert_eq!(editor.selection(), Some((0, 40)), "the shift-press rebuilds the selection");
}

/// A stale scroll offset beyond the end reports no movement and is healed.
#[test]
fn scrolling_a_shrunk_document_reports_the_clamped_position() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_text("a\nb\nc\nd");
    editor.first_visible_line = 84;
    assert_eq!(editor.visible_start(), 0);

    assert!(!editor.scroll_lines(-3), "the viewport was already at the clamped start");
    assert_eq!(editor.first_visible_line, 0, "the clamped offset is stored");
}
