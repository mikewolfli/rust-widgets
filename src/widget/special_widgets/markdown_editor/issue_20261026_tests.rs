// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the markdown editor.

use super::*;
use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{Draw, Widget};

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

/// SP-002: a press resolves the row through the line index, and a drag extends it from the
/// same table — multi-byte text included.
#[test]
fn pointer_hit_testing_resolves_columns_through_the_line_index() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    let mut text = String::new();
    for index in 0..59 {
        text.push_str(&format!("line {index}\n"));
    }
    text.push_str("中文abc");
    editor.set_text(text);
    assert!(editor.goto_line(59), "the multi-byte line is reachable");

    let row = 59 - editor.visible_start();
    let font = effective_font(editor.base().style());
    // The press is measured the way the hit test measures glyphs, so the column is the
    // boundary it is meant to be rather than a string-level shaping result.
    let mut after_two = 12i32;
    for ch in "中文".chars() {
        let mut buffer = [0u8; 4];
        after_two += estimate_text_width(ch.encode_utf8(&mut buffer), font, 1.0) as i32;
    }
    editor.handle_event(&Event::mouse_press_with(
        after_two,
        TEXT_TOP + row as i32 * TEXT_ROW_HEIGHT,
        1,
        0,
    ));
    assert_eq!(editor.cursor_line(), 59, "the press lands on the multi-byte line");
    assert_eq!(editor.cursor_col(), 2, "two whole glyphs precede the caret");

    editor.handle_event(&Event::mouse_move(12, TEXT_TOP + row as i32 * TEXT_ROW_HEIGHT));
    assert_eq!(
        editor.selection(),
        Some((editor.doc_char_offset(59, 0), editor.doc_char_offset(59, 2))),
        "the drag extends from the line start to the press column"
    );
}

/// SP-003: the selected text is taken from its own offsets, and `select_all` reads the
/// cached character count.
#[test]
fn the_selection_is_taken_from_its_own_offsets() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("añb\n世界");
    assert_eq!(editor.total_char_count(), 6);

    editor.select_byte_range(1, 3);
    assert_eq!(editor.selection(), Some((1, 2)));
    assert_eq!(editor.selected_text().as_deref(), Some("ñ"));

    editor.select_byte_range(0, 8);
    assert_eq!(editor.selection(), Some((0, 5)));
    assert_eq!(editor.selected_text().as_deref(), Some("añb\n世"));

    editor.select_all();
    assert_eq!(editor.selection(), Some((0, 6)));
    assert_eq!(editor.selected_text().as_deref(), Some("añb\n世界"));
}

/// SP-004: the preview cache is keyed by what a row parses to, so identical rows share an
/// entry and inserting a line above does not evict the rows below it.
#[test]
fn the_preview_cache_is_keyed_by_content_not_position() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_preview_mode(true);
    editor.set_text("alpha\nalpha\nalpha");
    draw_once(&mut editor);
    assert_eq!(editor.preview_cache.len(), 1, "identical rows share one parse");

    editor.set_text("inserted\nalpha\nalpha\nalpha");
    draw_once(&mut editor);
    assert_eq!(
        editor.preview_cache.len(),
        2,
        "the shifted rows keep their entries instead of re-parsing"
    );
}

/// SP-005: the header is formatted into the editor's own buffer, so a still frame does not
/// allocate a fresh `String`.
#[test]
fn the_header_buffer_is_reused_across_frames() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("one\ntwo");
    draw_once(&mut editor);
    let capacity = editor.header_buffer.capacity();
    let start = editor.header_buffer.as_ptr();
    assert!(editor.header_buffer.starts_with("Markdown Edit"));
    draw_once(&mut editor);
    assert_eq!(editor.header_buffer.capacity(), capacity, "the buffer is not reallocated");
    assert_eq!(editor.header_buffer.as_ptr(), start, "the same buffer is reused");
}

/// SP-001: the published signal carries the changed run, not a copy of the document.
#[test]
fn text_changed_reports_the_changed_run_to_a_subscriber() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("alpha");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<MarkdownTextChange>::new()));
    let sink = seen.clone();
    editor.text_changed.connect(move |change| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(change.as_ref().clone());
        }
    });
    editor.set_text("alpha beta");
    let got = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].offset, 5, "the common prefix is left out");
    assert_eq!(got[0].removed, "");
    assert_eq!(got[0].inserted, " beta");
    assert_eq!(got[0].len, 10);
}

/// Undoing a whole-document restore still reports the run that changed.
#[test]
fn undo_reports_the_run_it_restored() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
    editor.set_text("draft");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<MarkdownTextChange>::new()));
    let sink = seen.clone();
    editor.text_changed.connect(move |change| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(change.as_ref().clone());
        }
    });
    assert!(editor.undo());
    assert_eq!(editor.text(), "");
    let got = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].offset, 0);
    assert_eq!(got[0].removed, "draft");
    assert_eq!(got[0].inserted, "");
    assert_eq!(got[0].len, 0);
}

/// SP-006: a blink frame in a damage-tracking mode damages only the caret's rectangle.
#[cfg(not(alloc_frugal))]
#[test]
fn a_blink_frame_damages_only_the_caret_rect() {
    use crate::widget::runtime;

    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    editor.set_text("one\ntwo");
    let caret = editor.caret_rect().expect("the caret is on screen");
    assert_eq!(caret.width, 1, "the blink owns a one-pixel bar");
    let id = runtime::register(crate::compat::Box::new(editor)).expect("registry");
    assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));
    assert!(runtime::with_widget_mut(id, |widget| widget.tick(600)).unwrap_or(false));
    assert_eq!(runtime::dirty_rects(id), vec![caret], "only the caret rectangle is repainted");
    runtime::unregister(id);
}

/// SP-006: the default repaint mode still takes the whole-control path.
#[cfg(not(alloc_frugal))]
#[test]
fn a_blink_frame_in_full_mode_keeps_the_whole_control_repaint() {
    use crate::widget::runtime;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let editor = MarkdownEditor::new(Rect::new(0, 0, 420, 300));
    let id = runtime::register(crate::compat::Box::new(editor)).expect("registry");
    assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Full));
    let redraws = Arc::new(AtomicUsize::new(0));
    let counter = redraws.clone();
    let _ = runtime::with_widget(id, |widget| {
        widget.base().redraw_requested.connect(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        })
    });

    assert_eq!(runtime::with_widget_mut(id, |widget| widget.tick(600)), Some(true));
    assert_eq!(
        redraws.load(Ordering::Relaxed),
        1,
        "Full mode keeps the whole-control redraw request"
    );
    assert!(runtime::dirty_rects(id).is_empty(), "no damage rectangle is recorded");
    runtime::unregister(id);
}
