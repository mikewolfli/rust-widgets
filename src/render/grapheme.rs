// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unicode cluster **predicates** used while shaping and rasterising text.
//!
//! # What this module is, and what it deliberately is not
//!
//! It holds the two range predicates that decide whether a scalar continues the cluster
//! before it — [`is_combining_mark`] and [`is_variation_selector`]. They are the shared
//! vocabulary of [`crate::render::text::line::for_each_cluster`], which is the crate's
//! **only** cluster-splitting rule, and of the SVG backend's per-scalar positioning.
//!
//! It deliberately does **not** carry a second splitter. A `GraphemeProcessor` with its own
//! `split_graphemes` / `GraphemeCluster` used to live here, and it was a **parallel
//! implementation of one concept** (principle #101): `line.rs` split text one way and this
//! type split it another, with different answers for the same input, while `line.rs`'s module
//! note already claimed to be "the crate's only splitting rule". Nothing consumed the
//! processor — its only references were its own tests and this module's re-export — so it was
//! removed rather than kept as an alternative nobody chose. Adding cluster handling means
//! extending `for_each_cluster`, not this file.

/// Returns `true` if `c` is a combining mark (zero-width diacritic).
///
/// The ranges are the scripts the crate ships glyphs for; a mark outside them is treated as a
/// base character, which shapes as a separate cluster rather than silently merging.
pub(crate) fn is_combining_mark(c: char) -> bool {
    let code = c as u32;
    matches!(code,
        // Combining Diacritical Marks
        0x0300..=0x036F
        // Combining Diacritical Marks Extended
        | 0x1AB0..=0x1AFF
        // Combining Diacritical Marks Supplement
        | 0x1DC0..=0x1DFF
        // Combining Half Marks
        | 0xFE20..=0xFE2F
        // Devanagari combining marks (subset)
        | 0x0901..=0x0903
        | 0x093E..=0x094D
        // Thai combining marks
        | 0x0E31..=0x0E3A
        | 0x0E47..=0x0E4E
        // General combining range for Indic scripts
        | 0x0981..=0x0983
        | 0x09BE..=0x09CD
        | 0x0A01..=0x0A03
        | 0x0A3E..=0x0A4D
        | 0x0B01..=0x0B03
        | 0x0B3E..=0x0B4D
        // Tibetan combining marks
        | 0x0F82..=0x0F84
        | 0x0F86..=0x0F8B
    )
}

/// Returns `true` if `c` is a variation selector (U+FE00..U+FE0F).
pub(crate) fn is_variation_selector(c: char) -> bool {
    let code = c as u32;
    matches!(code, 0xFE00..=0xFE0F)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The predicates cover the ranges they document and reject a plain base character.
    #[test]
    fn the_predicates_cover_their_documented_ranges() {
        assert!(is_combining_mark('\u{0301}'), "U+0301 is a combining acute");
        assert!(is_combining_mark('\u{093E}'), "U+093E is a Devanagari matra");
        assert!(is_variation_selector('\u{FE0F}'), "U+FE0F is the emoji variation selector");

        assert!(!is_combining_mark('a'), "a base letter is not a mark");
        assert!(!is_variation_selector('a'), "a base letter is not a selector");
        // U+FE10 is a vertical-form punctuation, one past the selector range's end.
        assert!(!is_variation_selector('\u{FE10}'), "the selector range must not over-reach");
    }
}
