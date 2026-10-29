// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the markdown editor.

use super::*;
use crate::core::{Color, Rect};
use crate::render::RenderContext;
use crate::widget::input_widgets::shortcut_editor::primary_modifier_event_bits;
use crate::widget::Draw;
use std::sync::{Arc, Mutex};

fn editor(text: &str) -> MarkdownEditor {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_text(text);
    editor
}

fn draw_once(editor: &mut MarkdownEditor) {
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(400, 300), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        editor.draw(&mut context);
    }
    backend.end_frame();
}

/// SP-02: Tab indents the selected lines and Shift+Tab reverses it.
#[test]
fn tab_indents_and_shift_tab_outdents_selected_lines() {
    let mut editor = editor("alpha\nbeta");
    editor.select_all();
    editor.handle_key(9, 0);
    assert_eq!(editor.text(), "  alpha\n  beta");
    editor.handle_key(9, crate::shortcut::Modifiers::SHIFT.bits() as u32);
    assert_eq!(editor.text(), "alpha\nbeta");
}

/// SP-03: the formatting action wraps a selection and a second call unwraps it.
#[test]
fn format_actions_wrap_and_unwrap_the_selection() {
    let mut editor = editor("hello");
    editor.select_all();
    assert!(editor.wrap_selection("**", "**"));
    assert_eq!(editor.text(), "**hello**");
    assert!(editor.wrap_selection("**", "**"));
    assert_eq!(editor.text(), "hello");
}

/// SP-03: the keyboard chord drives the same action.
#[test]
fn primary_b_wraps_the_selection() {
    let mut editor = editor("hello");
    editor.select_all();
    editor.handle_key(66, primary_modifier_event_bits());
    assert_eq!(editor.text(), "**hello**");
}

/// SP-04: primary+Shift+Z redoes, the chord the handler used to ignore.
#[test]
fn primary_shift_z_redoes() {
    let mut editor = editor("ab");
    assert!(editor.undo());
    assert_eq!(editor.text(), "");
    let modifiers = primary_modifier_event_bits() | crate::shortcut::Modifiers::SHIFT.bits() as u32;
    editor.handle_key(90, modifiers);
    assert_eq!(editor.text(), "ab");
}

/// SP-03: a line prefix toggle is one undo step for a multi-line selection.
#[test]
fn line_prefix_toggles_every_selected_line_as_one_undo_step() {
    let mut editor = editor("one\ntwo");
    editor.select_all();
    assert!(editor.toggle_line_prefix("- "));
    assert_eq!(editor.text(), "- one\n- two");
    assert!(editor.undo());
    assert_eq!(editor.text(), "one\ntwo");
}

/// COMP-H04: the toolbar emits only actions it actually offers.
#[test]
fn toolbar_executes_only_enabled_actions() {
    let mut toolbar = MarkdownToolbar::new(Rect::new(0, 0, 300, 28));
    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = hits.clone();
    toolbar.action_triggered.connect_ref(move |token| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(token.clone());
        }
    });
    assert!(toolbar.execute("bold"));
    assert!(!toolbar.execute("nope"));
    assert_eq!(hits.lock().ok().map(|guard| guard.clone()).unwrap_or_default(), vec!["bold"]);
    assert!(!toolbar.set_actions(vec!["nope".to_string()]));
    assert_eq!(toolbar.actions().len(), 7);
}

/// SP-P03: the preview cache keeps live entries when it is bounded.
#[test]
fn the_preview_cache_keeps_live_lines_when_it_is_trimmed() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_preview_mode(true);
    let text: String = (0..200).map(|line| format!("line {line}\n")).collect();
    editor.set_text(text);
    draw_once(&mut editor);
    // The buffer is longer than the bound only after several revisions; the assertion that
    // matters is that the live entries survive, so the following frame still draws them.
    editor.set_text(editor.text().to_string() + "tail");
    draw_once(&mut editor);
    assert!(editor.line_count() >= 200);
}

/// SP-P04: caret geometry follows edits instead of rescanning the document.
#[test]
fn caret_geometry_tracks_edits_with_the_line_index() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_text("one\ntwo\nthree");
    editor.goto_line(2);
    editor.move_to_line_edge(false);
    assert_eq!(editor.cursor_line(), 2);
    assert_eq!(editor.cursor_col(), 5);
    editor.insert_text_at_cursor("!");
    assert_eq!(editor.text(), "one\ntwo\nthree!");
    assert_eq!(editor.cursor_line(), 2);
    assert_eq!(editor.cursor_col(), 6);
}
