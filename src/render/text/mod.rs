// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The text layer: where glyphs come from, and which face wins.
//!
//! # Two questions, two types
//!
//! Rendering one character needs two answers that are easy to entangle and expensive to
//! separate later:
//!
//! 1. *"What are this character's pixels?"* — a [`GlyphSource`].
//! 2. *"I have several faces; which one answers?"* — a [`FontStack`], resolved first-hit.
//!
//! # Data is opt-in
//!
//! The default build carries **no font data** beyond the crate's historical 8x8 face, so it
//! draws Latin/ASCII and the fallback glyph for everything else — the same bytes it always
//! did. A face that covers more (today: `fonts-cjk-bitmap`, a generated 16x16 CJK subset) is
//! added by enabling a feature, which appends one entry to the stack. Nothing in this module
//! allocates, and no glyph table is ever resident in RAM: a resolved glyph borrows its rows
//! from the binary's read-only section.

mod glyph_source;

// The opt-in vector faces (generated data) and the shaper that reads them. Both are gated by
// the shaping feature, because a face with no shaper to read it has no consumer.
#[cfg(any(feature = "text-shaping", feature = "fonts-emoji-color"))]
pub mod font_assets;
#[cfg(feature = "text-shaping")]
pub mod shaping;

// Host-registered faces (`runtime-fonts`). Gated on the feature, and on `text-shaping` because a
// runtime face is only useful to a shaper or the vector rasteriser, both of which need the engine.
// A host that registers a face on a build with no vector path would hand in bytes nothing reads.
#[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
pub mod runtime_fonts;

// The public entry point for a host's own font bytes. Re-exported at `text::` because that is
// where the crate's other text-surface items live, and because the alternative — reaching into
// `text::runtime_fonts::register_face` — exposes a module the caller has no other reason to name.
#[cfg(all(feature = "runtime-fonts", feature = "text-shaping"))]
pub use runtime_fonts::{clear_faces, register_face, registered_face_count, MAX_RUNTIME_FACES};

// The one derivation of a line's clusters, shared by both renderers.
mod line;

// Vector outline rasterisation (G-5): the coverage-producing implementation of `GlyphSource`.
//
// Gated on the shaping feature because it reads the same `ttf-parser` face data the shaper does —
// a rasteriser with no face to rasterise is dead weight, and `ttf-parser` is the shaper's own
// dependency, so this adds none.
#[cfg(feature = "text-shaping")]
mod raster;

#[cfg(feature = "text-shaping")]
pub use raster::{
    outline, outline_by_coverage, OutlinePoint, VectorSource, OUTLINE_MAX_CONTOURS,
    OUTLINE_MAX_POINTS,
};

// Colour bitmap faces (G-6): `CBDT`/`CBLC` plus the PNG decoder that reads what they point at.
//
// The two share one gate because they are one capability: a `CBDT` index is useless without a
// decoder for its payload, and a decoder with no index has nothing to decode. It also keeps
// `miniz_oxide` out of a build with no colour face.
#[cfg(feature = "fonts-emoji-color")]
mod color_bitmap;
#[cfg(feature = "fonts-emoji-color")]
mod png;
#[cfg(feature = "fonts-emoji-color")]
pub use color_bitmap::ColorBitmapFace;

// Bidirectional reordering (UAX #9). Always compiled: it is logic, not data.
pub mod bidi;

// The generated CJK table is 75 KB of static data with no consumers unless the feature is on;
// compiling it unconditionally would be dead weight and a `dead_code` warning in one move.
#[cfg(feature = "fonts-cjk-bitmap")]
mod cjk_bitmap_data;

pub use glyph_source::{
    active_stack, paint_active, resolve, source_for, BitOrder, Cell, Font8x8Source, FontStack,
    GlyphBitmap, GlyphSource, InkKind, Painted, TOFU,
};

// The face lookup the backends share, exposed so a host can ask which face would draw a character
// (and therefore whether its own registered face is the one answering).
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub use font_assets::face_for_char;

// The un-cached control for the coverage-cache benchmark. `#[doc(hidden)]` on the item itself;
// re-exported here so a benchmark reaches it without naming the `font_assets` module path.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
#[doc(hidden)]
pub use font_assets::face_for_char_uncached;

#[cfg(feature = "fonts-emoji-color")]
pub use glyph_source::ColorBitmapSource;

// The wide-scalar table is a property of the characters, needed by the renderer's advance
// model *and* by a control's implicit-size estimate, so it is exported rather than copied.
// `estimate_cluster_advance` is exported for the same reason: it *is* the crate's advance
// model, and a second spelling of it would drift.
pub use line::{estimate_cluster_advance, is_wide_scalar};
// Only the `text-shaping` build has a face to measure against, so the helper is only reachable
// there; gating it the same way keeps a face-less build from warning about an unused import.
#[cfg(all(test, feature = "text-shaping"))]
pub(crate) use line::measure_text_width_for_test;

// The cluster helpers and the line-shaping entry point stay inside the crate: their consumers
// are the two backends and a control's implicit-size estimate, not public API.
pub(crate) use line::{for_each_cluster, is_combining_mark, is_variation_selector, shape_line};

// The greedy shaper is always present; the face-backed one only when a face is.
#[cfg(feature = "text-shaping")]
pub use shaping::RustybuzzShaper;

#[cfg(feature = "fonts-cjk-bitmap")]
pub use glyph_source::cjk;
