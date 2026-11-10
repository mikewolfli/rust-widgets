// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the markdown block-renderer seam.
//!
//! Every test injects its own renderer, so no diagram or formula engine is involved. The
//! assertions cover the three contracts the seam promises: a registered renderer receives the
//! whole block once and its artifact is drawn at the block's first row; a `Pending`,
//! `Unsupported` or malformed answer falls back to the source rows; and replacing or removing a
//! renderer never reuses the previous registry's artifact.

use super::*;
use crate::core::{Color, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use crate::widget::Draw;
use std::sync::{Arc, Mutex};

/// What a [`TestRenderer`] answers.
#[derive(Clone, Copy)]
enum Reply {
    Image {
        height: u32,
        color: [u8; 4],
    },
    Pending,
    Unsupported,
    /// An RGBA length that does not match `width * height * 4`.
    Malformed,
}

struct TestRenderer {
    reply: Arc<Mutex<Reply>>,
    calls: Calls,
}

type Calls = Arc<Mutex<Vec<(String, MarkdownBlockSpec)>>>;

impl TestRenderer {
    fn with_reply(reply: Reply) -> (Self, Arc<Mutex<Reply>>, Calls) {
        let reply = Arc::new(Mutex::new(reply));
        let calls = Arc::new(Mutex::new(Vec::new()));
        (Self { reply: reply.clone(), calls: calls.clone() }, reply, calls)
    }
}

impl MarkdownBlockRenderer for TestRenderer {
    fn render(&self, source: &str, spec: MarkdownBlockSpec) -> MarkdownBlockRender {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push((source.to_string(), spec));
        }
        let reply = *self.reply.lock().expect("reply lock");
        match reply {
            Reply::Image { height, color } => {
                let width = 8u32;
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for _ in 0..width * height {
                    rgba.extend_from_slice(&color);
                }
                MarkdownBlockRender::Image { width, height, rgba }
            }
            Reply::Pending => MarkdownBlockRender::Pending,
            Reply::Unsupported => MarkdownBlockRender::Unsupported,
            Reply::Malformed => {
                MarkdownBlockRender::Image { width: 2, height: 2, rgba: vec![0; 15] }
            }
        }
    }
}

fn draw(editor: &mut MarkdownEditor) -> Vec<u8> {
    let mut backend = SoftwarePaintBackend::new(Size::new(400, 300), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        editor.draw(&mut context);
    }
    backend.end_frame();
    backend.frame_rgba().to_vec()
}

fn pixel(frame: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = ((y * width + x) * 4) as usize;
    [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
}

fn editor_with_mermaid_block() -> MarkdownEditor {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("before\n```mermaid\nalpha\nbeta\n```\nafter");
    editor.set_preview_mode(true);
    editor
}

#[test]
fn a_registered_renderer_draws_one_artifact_for_the_whole_block() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, _reply, calls) =
        TestRenderer::with_reply(Reply::Image { height: 24, color: [255, 0, 255, 255] });
    editor.register_block_renderer("mermaid", Box::new(renderer));

    let frame = draw(&mut editor);
    // The fence opens on line 1, so its image starts one row below the text top.
    let top = (TEXT_TOP + TEXT_ROW_HEIGHT) as u32;
    assert_eq!(pixel(&frame, 400, 14, top + 2), [255, 0, 255, 255]);
    assert_eq!(pixel(&frame, 400, 14, top + 23), [255, 0, 255, 255]);
    assert_ne!(
        pixel(&frame, 400, 14, top + 24),
        [255, 0, 255, 255],
        "the next row starts below the artifact, not on top of it"
    );

    {
        let calls = calls.lock().expect("calls lock");
        assert_eq!(calls.len(), 1, "the whole block is one render call");
        assert_eq!(calls[0].0, "alpha\nbeta\n", "fence lines are not part of the source");
    }
    let _ = draw(&mut editor);
    assert_eq!(
        calls.lock().expect("calls lock").len(),
        1,
        "a still frame reuses the cached artifact"
    );
}

#[test]
fn an_unregistered_fence_is_never_handed_to_a_renderer() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, _reply, calls) =
        TestRenderer::with_reply(Reply::Image { height: 16, color: [255, 0, 255, 255] });
    editor.register_block_renderer("latex", Box::new(renderer));

    let frame = draw(&mut editor);
    assert!(calls.lock().expect("calls lock").is_empty());
    assert!(
        !frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()),
        "no artifact was drawn for a language with no renderer"
    );
}

#[test]
fn an_unsupported_block_falls_back_to_its_source_rows() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, _reply, calls) = TestRenderer::with_reply(Reply::Unsupported);
    editor.register_block_renderer("mermaid", Box::new(renderer));

    let frame = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock").len(), 1);
    assert!(!frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()));
}

#[test]
fn a_malformed_image_falls_back_instead_of_panicking() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, _reply, calls) = TestRenderer::with_reply(Reply::Malformed);
    editor.register_block_renderer("mermaid", Box::new(renderer));

    let frame = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock").len(), 1);
    assert!(!frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()));
}

#[test]
fn a_pending_artifact_is_retried_after_the_host_signals_ready() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, reply, calls) = TestRenderer::with_reply(Reply::Pending);
    editor.register_block_renderer("mermaid", Box::new(renderer));

    let frame = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock").len(), 1);
    assert!(!frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()));

    let _ = draw(&mut editor);
    assert_eq!(
        calls.lock().expect("calls lock").len(),
        1,
        "a cached Pending answer does not call the renderer on every frame"
    );

    *reply.lock().expect("reply lock") = Reply::Image { height: 16, color: [0, 255, 0, 255] };
    editor.notify_block_render_ready();
    let frame = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock").len(), 2);
    let top = (TEXT_TOP + TEXT_ROW_HEIGHT) as u32;
    assert_eq!(pixel(&frame, 400, 14, top + 2), [0, 255, 0, 255]);
}

#[test]
fn replacing_a_renderer_invalidates_the_cached_artifact() {
    let mut editor = editor_with_mermaid_block();
    let (magenta, _reply, _calls) =
        TestRenderer::with_reply(Reply::Image { height: 16, color: [255, 0, 255, 255] });
    editor.register_block_renderer("mermaid", Box::new(magenta));
    let frame = draw(&mut editor);
    let top = (TEXT_TOP + TEXT_ROW_HEIGHT) as u32;
    assert_eq!(pixel(&frame, 400, 14, top + 2), [255, 0, 255, 255]);

    let (green, _reply, _calls) =
        TestRenderer::with_reply(Reply::Image { height: 16, color: [0, 255, 0, 255] });
    editor.register_block_renderer("mermaid", Box::new(green));
    let frame = draw(&mut editor);
    assert_eq!(
        pixel(&frame, 400, 14, top + 2),
        [0, 255, 0, 255],
        "the previous registry's artifact must not survive"
    );
}

#[test]
fn unregistering_a_renderer_restores_the_source_rows() {
    let mut editor = editor_with_mermaid_block();
    let (renderer, _reply, _calls) =
        TestRenderer::with_reply(Reply::Image { height: 16, color: [255, 0, 255, 255] });
    editor.register_block_renderer("mermaid", Box::new(renderer));
    let frame = draw(&mut editor);
    assert!(frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()));

    assert!(editor.unregister_block_renderer("mermaid"));
    assert!(!editor.unregister_block_renderer("mermaid"));
    let frame = draw(&mut editor);
    assert!(!frame.windows(4).any(|window| window == [255, 0, 255, 255].as_slice()));
}

#[test]
fn registration_names_are_normalised_and_listed_in_order() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 300));
    let (first, _reply, _calls) = TestRenderer::with_reply(Reply::Unsupported);
    editor.register_block_renderer("  Mermaid  ", Box::new(first));
    let (second, _reply, _calls) = TestRenderer::with_reply(Reply::Unsupported);
    editor.register_block_renderer("latex", Box::new(second));

    assert!(editor.has_block_renderer("MERMAID"));
    assert!(!editor.has_block_renderer("dot"));
    assert_eq!(editor.registered_block_renderers(), vec!["latex", "mermaid"]);
}

#[test]
fn a_tall_block_does_not_make_the_tail_unreachable() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 120));
    editor.set_text("```mermaid\nalpha\n```\ntail");
    editor.set_preview_mode(true);
    let (renderer, _reply, _calls) =
        TestRenderer::with_reply(Reply::Image { height: 200, color: [255, 0, 255, 255] });
    editor.register_block_renderer("mermaid", Box::new(renderer));
    let _ = draw(&mut editor);

    assert!(editor.scroll_lines(1000));
    assert_eq!(editor.first_visible_line, editor.edit_line_count() - 1);
    // Painting from the last source line must not need the block whose rows are above it.
    let _ = draw(&mut editor);
}

#[test]
fn pointer_mapping_follows_a_rendered_block() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("```mermaid\nalpha\n```\ntail");
    editor.set_preview_mode(true);
    let (renderer, _reply, _calls) =
        TestRenderer::with_reply(Reply::Image { height: 64, color: [255, 0, 255, 255] });
    editor.register_block_renderer("mermaid", Box::new(renderer));
    let _ = draw(&mut editor);

    let tail_y = TEXT_TOP + 64 + 2;
    editor.handle_event(&Event::mouse_press(14, tail_y, 1));
    assert_eq!(editor.cursor_line(), 3, "the tail row maps below the artifact");

    editor.handle_event(&Event::mouse_press(14, TEXT_TOP + 30, 1));
    assert_eq!(editor.cursor_line(), 0, "a press inside the artifact maps to its opening line");
}

#[test]
fn an_unclosed_fence_does_not_gain_a_synthetic_trailing_line() {
    let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 300));
    editor.set_text("```mermaid\nalpha\n");
    editor.set_preview_mode(true);
    let (renderer, _reply, calls) = TestRenderer::with_reply(Reply::Unsupported);
    editor.register_block_renderer("mermaid", Box::new(renderer));
    let _ = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock")[0].0, "alpha\n");

    editor.set_text("```mermaid\nalpha");
    let (renderer, _reply, calls) = TestRenderer::with_reply(Reply::Unsupported);
    editor.register_block_renderer("mermaid", Box::new(renderer));
    let _ = draw(&mut editor);
    assert_eq!(calls.lock().expect("calls lock")[0].0, "alpha\n");
}
