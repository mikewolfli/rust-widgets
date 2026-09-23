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
#[cfg(feature = "text-shaping")]
mod font_assets;
#[cfg(feature = "text-shaping")]
pub mod shaping;

// The one derivation of a line's clusters, shared by both renderers.
mod line;

// Bidirectional reordering (UAX #9). Always compiled: it is logic, not data.
pub mod bidi;

// The generated CJK table is 75 KB of static data with no consumers unless the feature is on;
// compiling it unconditionally would be dead weight and a `dead_code` warning in one move.
#[cfg(feature = "fonts-cjk-bitmap")]
mod cjk_bitmap_data;

pub use glyph_source::{
    active_stack, resolve, source_for, BitOrder, Font8x8Source, FontStack, GlyphBitmap,
    GlyphSource, TOFU,
};

// The wide-scalar table is a property of the characters, needed by the renderer's advance
// model *and* by a control's implicit-size estimate, so it is exported rather than copied.
// `estimate_cluster_advance` is exported for the same reason: it *is* the crate's advance
// model, and a second spelling of it would drift.
pub use line::{estimate_cluster_advance, is_wide_scalar};

// The cluster helpers and the line-shaping entry point stay inside the crate: their consumers
// are the two backends and a control's implicit-size estimate, not public API.
pub(crate) use line::{for_each_cluster, is_combining_mark, is_variation_selector, shape_line};

// The greedy shaper is always present; the face-backed one only when a face is.
#[cfg(feature = "text-shaping")]
pub use shaping::RustybuzzShaper;

#[cfg(feature = "fonts-cjk-bitmap")]
pub use glyph_source::cjk;
