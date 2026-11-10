// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The block-level renderer seam for the markdown preview.
//!
//! A fenced code block whose info string names a registered renderer is handed to that renderer
//! as a **whole block**, not as one line at a time. The renderer returns a raster image (or
//! `Pending`/`Unsupported`), and the preview paints it at the block's first line, advancing the
//! following rows by the image's height. This is what lets a host plug in a diagram or formula
//! engine without the crate depending on that engine:
//!
//! * The widget owns the block boundary, the source text and the cache;
//! * The host owns the renderer implementation (an external process, a JS runtime, a typesetting
//!   crate, a network service);
//! * A renderer that returns `Unsupported`, `Pending`, or a malformed image leaves the block
//!   rendering exactly as before: one syntax-highlighted source row per line.
//!
//! ```no_run
//! use rust_widgets::core::Rect;
//! use rust_widgets::widget::special_widgets::markdown_editor::{
//!     MarkdownBlockRender, MarkdownBlockRenderer, MarkdownBlockSpec, MarkdownEditor,
//! };
//!
//! struct Solid;
//!
//! impl MarkdownBlockRenderer for Solid {
//!     fn render(&self, _source: &str, spec: MarkdownBlockSpec) -> MarkdownBlockRender {
//!         let width = spec.max_width.max(1);
//!         let height = 24;
//!         MarkdownBlockRender::Image {
//!             width,
//!             height,
//!             rgba: vec![128; (width * height * 4) as usize],
//!         }
//!     }
//! }
//!
//! let mut editor = MarkdownEditor::new(Rect::new(0, 0, 400, 300));
//! editor.register_block_renderer("diagram", Box::new(Solid));
//! ```
//!
//! Nothing in this module is a dynamic plugin system. Renderers are ordinary trait objects held by
//! [`super::MarkdownEditor`], which keeps the seam ABI-stable and testable without loading code.
//! The pattern is the same one
//! [`crate::widget::special_widgets::code_editor::SyntaxHighlighter`] uses.
//!
//! # Source text
//!
//! [`MarkdownBlockRenderer::render`] receives the fenced body, one line per `\n`, including the
//! final newline when the body is not empty. The opening and closing fence lines are **not** part
//! of the source. This is the text a diagram or formula engine expects; trimming it is the
//! renderer's decision.
//!
//! # Async rendering
//!
//! A renderer that cannot answer synchronously returns [`MarkdownBlockRender::Pending`]. The
//! editor caches that answer so a still frame does not call the renderer again, draws the source
//! rows as the fallback, and waits for the host to call
//! [`super::MarkdownEditor::notify_block_render_ready`] when the artifact exists. The next frame
//! calls the renderer again.
//!
//! # Viewport
//!
//! The viewport is still addressed in **source lines**: a rendered block is drawn at its opening
//! fence's row and the rows below it are shifted by the artifact's height, but
//! `first_visible_line`, the wheel step and the scrollbar proportion keep their line-based model.
//! A block whose image is taller than its source rows therefore shifts the page while it is on
//! screen; it does not yet contribute a pixel-based scroll extent. To keep the tail reachable, a
//! document containing a rendered block may scroll until its last source line is at the top, and
//! pointer hit-testing searches the geometry painted for the frame instead of dividing by the
//! fixed row pitch.

use crate::core::Color;

/// The inputs a block renderer may read when it produces one artifact.
///
/// All geometry is in the editor's drawing coordinate space. A renderer that wants a sharper
/// raster can render at `max_width * dpi_scale` and downscale before returning; the returned
/// [`MarkdownBlockRender::Image`] dimensions are what the editor draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkdownBlockSpec {
    /// The width reserved for the block's text column.
    pub max_width: u32,
    /// The backend DPI scale at the time of the request.
    pub dpi_scale: f32,
    /// The editor surface the block sits on.
    pub surface: Color,
    /// The resolved body ink.
    pub ink: Color,
    /// The resolved accent ink.
    pub accent: Color,
}

/// What one block renderer produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlockRender {
    /// A raster artifact to draw at the block's first row.
    ///
    /// `rgba` must contain exactly `width * height * 4` bytes; anything else is treated as
    /// [`Self::Unsupported`], so a renderer bug degrades to source rows instead of corrupting the
    /// frame.
    Image {
        /// The width to draw, in the editor's drawing coordinates.
        width: u32,
        /// The height to draw, in the editor's drawing coordinates.
        height: u32,
        /// Row-major RGBA8 pixels, `width * height * 4` bytes.
        rgba: Vec<u8>,
    },
    /// The artifact is not ready yet.
    ///
    /// The editor caches this answer and draws the source rows until the host calls
    /// [`super::MarkdownEditor::notify_block_render_ready`].
    Pending,
    /// This renderer does not handle this source.
    ///
    /// The block falls back to the ordinary syntax-highlighted source rows.
    Unsupported,
}

/// A host-provided renderer for one fenced-block language.
///
/// Registered through [`super::MarkdownEditor::register_block_renderer`], keyed by the
/// lower-cased first token of the fence's info string (` ```mermaid ` → `"mermaid"`).
///
/// No `Send`/`Sync` bound: the markdown editor is a single-threaded widget, so a renderer that
/// holds `Rc`/`RefCell` state is as usable as one that holds `Arc`/`Mutex`.
pub trait MarkdownBlockRenderer {
    /// Renders one fenced block.
    ///
    /// `source` is the block body (see the module docs). The editor calls this only while it is
    /// painting preview mode and only for a block whose info token has a registered renderer.
    fn render(&self, source: &str, spec: MarkdownBlockSpec) -> MarkdownBlockRender;
}

/// A fenced block whose info token named a renderer when the block table was last rebuilt.
#[derive(Debug)]
pub(crate) struct FencedBlock {
    /// The opening fence's line index.
    pub(crate) start_line: usize,
    /// The closing fence's line index, or the last line for an unclosed block.
    pub(crate) end_line: usize,
    /// The lower-cased info token used as the registry key.
    pub(crate) language: String,
    /// The body lines, each terminated by `\n` (empty for an empty block).
    pub(crate) source: String,
    /// Precomputed [`source`](Self::source) hash, so a still frame does not rehash the body.
    pub(crate) source_hash: u64,
    /// Precomputed [`language`](Self::language) hash.
    pub(crate) language_hash: u64,
}

/// The cache key of one rendered block.
///
/// Every input that can change the pixels is part of the key: the source hash and length, the
/// language hash and length, the available width, the DPI scale, the theme inks and the
/// renderer-registry generation. It does **not** carry the block's line index, so an edit above the
/// block keeps its cached artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct BlockKey {
    source: u64,
    /// The body length beside its hash, so a hash collision alone cannot reuse an artifact.
    source_len: usize,
    language: u64,
    language_len: usize,
    width: u32,
    dpi_bits: u32,
    generation: u64,
    surface: Color,
    ink: Color,
    accent: Color,
}

impl BlockKey {
    pub(crate) fn new(block: &FencedBlock, spec: MarkdownBlockSpec, generation: u64) -> Self {
        Self {
            source: block.source_hash,
            source_len: block.source.len(),
            language: block.language_hash,
            language_len: block.language.len(),
            width: spec.max_width,
            dpi_bits: spec.dpi_scale.to_bits(),
            generation,
            surface: spec.surface,
            ink: spec.ink,
            accent: spec.accent,
        }
    }
}

/// The cached answer for one [`BlockKey`].
pub(crate) enum BlockArtifact {
    Image { width: u32, height: u32, rgba: Vec<u8> },
    Pending,
    Unsupported,
}

/// Calls `renderer` and normalises its answer into a cacheable artifact.
pub(crate) fn render_block(
    renderer: &dyn MarkdownBlockRenderer,
    block: &FencedBlock,
    spec: MarkdownBlockSpec,
) -> BlockArtifact {
    match renderer.render(&block.source, spec) {
        MarkdownBlockRender::Image { width, height, rgba } => {
            if image_is_well_formed(width, height, rgba.len()) {
                BlockArtifact::Image { width, height, rgba }
            } else {
                BlockArtifact::Unsupported
            }
        }
        MarkdownBlockRender::Pending => BlockArtifact::Pending,
        MarkdownBlockRender::Unsupported => BlockArtifact::Unsupported,
    }
}

/// The lower-cased first token of a fence info string, which is the registry key.
pub(crate) fn block_language_key(info: &str) -> String {
    info.split_whitespace().next().unwrap_or("").to_ascii_lowercase()
}

/// Whether the renderer's raster really is `width * height` RGBA8 pixels.
fn image_is_well_formed(width: u32, height: u32, len: usize) -> bool {
    // The dimensions are drawn through `i32` coordinates, so a value that cannot fit one is not
    // an image this painter can place. The multiplication is checked because a malicious or
    // confused renderer can hand over values that overflow `usize` on a 32-bit target.
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return false;
    }
    let pixel_count = (width as usize).checked_mul(height as usize);
    pixel_count.and_then(|count| count.checked_mul(4)) == Some(len)
}
