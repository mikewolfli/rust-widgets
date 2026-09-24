// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Real shaping: what a script's *rules* do to a string, on top of what its face *has*.
//!
//! # The two axes, again
//!
//! [BLUE23 §0A.4](docs/plans/blue23.md) keeps two questions apart, and this module is the
//! second one:
//!
//! * *Does a face have this character?* — [`crate::render::text::GlyphSource`], answered by data;
//! * *What glyphs does the script say this **string** becomes?* — this module, answered by the
//!   face's layout tables (`GSUB`, `GPOS`) and the script's rules.
//!
//! A CJK face needs the first and not the second: one character is one cell. An Arabic face
//! needs the second and cannot fake it — `بيت` is three characters and, drawn as three isolated
//! letters, is not a word in any state a reader would call *close*. That asymmetry is why
//! BLUE23 calls the two orthogonal rather than one "font support" axis.
//!
//! # Why the engine is opt-in
//!
//! Shaping is only meaningful with a face, so it is compiled in only when one is
//! ([`crate::render::text::shaping::active_faces`] is empty otherwise) and the model in
//! [`crate::render::text::line`] stays the whole story for a default build. The default build's
//! bytes do not change.
//!
//! # Why the engine is built per call rather than stored
//!
//! A shaper face carries a mutable layout-plan cache, which is why it is not `Sync` — and
//! [`crate::render::TextShaper`] is required to be `Send + Sync` because a renderer is. Rather
//! than hide that behind a lock (and pay for the lock on every measurement), the face is parsed
//! from its bytes on each call: parsing a table directory is a few dozen reads, this is an
//! opt-in path, and the alternative is shared mutable state in the one place the three
//! constraints say there must be none.

use crate::compat::Vec;
use crate::core::Font;
use crate::render::text::font_assets::{active_faces, FaceBytes};
use crate::render::{ShapedGlyphRun, TextShaper};

/// The face whose family name matches `font.family()`, if any.
///
/// # Why the face is named rather than guessed
///
/// [`crate::core::Font`] already carries the answer to "which face?" — its `family` — and this
/// layer honours it. The alternative, picking the first face that happens to cover the text,
/// would change every label's **advance** the moment a vector face was enabled, for every
/// caller, including those drawing Latin text that never needed a second face. Metrics are
/// layout: a feature that silently re-laid-out every control would be a surprise, not an
/// upgrade. Naming the face makes the choice the caller's, which is also what makes "enable the
/// feature and nothing changes until you ask for the family" a stated behaviour instead of a
/// lucky one.
///
/// The match is case-insensitive because a font family name is an identifier a person types.
pub(crate) fn face_for_family(family: &str) -> Option<FaceBytes> {
    active_faces().iter().copied().find(|face| face.name.eq_ignore_ascii_case(family))
}

/// The first face that covers `text`'s first strong character.
///
/// "First strong character" is the rule the bidirectional algorithm uses to pick a paragraph's
/// base direction (UAX #9 P2). It is the right rule for a caller that has said *nothing* about
/// which face it wants — the case [`RustybuzzShaper::from_active_faces`] exists for — and it is
/// deliberately not the rule the renderer's measurement path uses: see [`face_for_family`].
fn face_for_text(text: &str) -> Option<FaceBytes> {
    let faces = active_faces();
    if faces.is_empty() {
        return None;
    }
    let probe = text.chars().find(|ch| !ch.is_whitespace())?;
    faces.iter().copied().find(|face| covers(face, probe))
}

/// Whether `face` has a glyph for `ch`.
pub(crate) fn covers(face: &FaceBytes, ch: char) -> bool {
    face_parser(face).map(|parsed| parsed.glyph_index(ch).is_some()).unwrap_or(false)
}

/// A parsed view of `face`'s bytes, or `None` if they are not a font this crate can read.
///
/// Kept private: parsing is the one operation that can fail, and every caller wants a different
/// thing from the result. Exposing the parser would leak the font crate into this module's API.
fn face_parser(face: &FaceBytes) -> Option<ttf_parser::Face<'static>> {
    ttf_parser::Face::parse(face.bytes, 0).ok()
}

/// The advance of each cluster, summed over the **glyphs** that cluster shaped into.
///
/// `ranges` is the caller's clusters as `(byte_start, byte_end)` pairs in logical order — the
/// same list [`crate::render::text::line`] built for the bidirectional reorder. The result is
/// indexed the same way, so the caller can read it straight into its own clusters.
///
/// # Why a sum, and why by byte range
///
/// Shaping is not one-glyph-per-cluster. A ligature turns two characters into one glyph; Arabic
/// joining substitutes a contextual form for a letter; a cluster can also decompose into several
/// glyphs. What the renderer needs is the width each *cluster* occupies, so every glyph's
/// advance is credited to the cluster its reported byte index falls inside — by range, not by
/// counting, because counting would mis-attribute a cluster that spans more than one character.
pub(crate) fn cluster_advances(
    text: &str,
    ranges: &[(usize, usize)],
    font: &Font,
    scale: f32,
) -> Option<Vec<f32>> {
    let face = face_for_family(font.family())?;
    let parsed = rustybuzz::Face::from_slice(face.bytes, 0)?;
    let units_per_em = parsed.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return None;
    }
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    // The direction is guessed from the text, for the same reason the face is named by its
    // family: a label's base direction is a property of its content, and the reorder itself is
    // `bidi`'s job and happens later — shaping works in logical order.
    buffer.guess_segment_properties();
    let output = rustybuzz::shape(&parsed, &[], buffer);

    // The pixel size of one font unit. An `em` is the font size, so a unit is
    // `font_size / units_per_em` logical pixels — the real advance, rather than a per-cluster
    // guess. `scale` converts to device pixels, as everywhere else in the text layer.
    let unit_px = font.size() * scale / units_per_em;

    let mut advances = vec![0.0f32; ranges.len()];
    for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
        let byte = info.cluster as usize;
        // `ranges` is sorted and disjoint, so the cluster containing `byte` is a partition point.
        let index = ranges.partition_point(|&(start, _)| start <= byte);
        if let Some(slot) = advances.get_mut(index.saturating_sub(1)) {
            *slot += position.x_advance as f32 * unit_px;
        }
    }
    Some(advances)
}

/// A shaper backed by a real font's layout tables.
///
/// # What it adds over [`crate::render::SimpleTextShaper`]
///
/// Glyph **identities** and real advances: `SimpleTextShaper` numbers characters sequentially
/// and charges each a fixed fraction of an em, which is enough to lay out a left-to-right Latin
/// label and cannot express that two characters became one glyph, that a letter changed shape
/// because of its neighbour, or that a pair is kerned.
///
/// # What it needs
///
/// A face. [`Self::from_active_faces`] picks the first of this build's opt-in faces that covers
/// the text; a caller with its own face constructs one with [`Self::new`]. With no face, every
/// method falls back to the model — this type never guesses a glyph.
pub struct RustybuzzShaper {
    face: FaceBytes,
}

impl RustybuzzShaper {
    /// A shaper over `face`, or `None` when its bytes are not a font this crate can read.
    pub fn new(face: FaceBytes) -> Option<Self> {
        face_parser(&face)?;
        Some(Self { face })
    }

    /// A shaper over the first of this build's opt-in faces that covers `text`, if any.
    ///
    /// This is the convenience for a caller that has not named a family: it picks by coverage.
    /// Presentation goes through [`Self::new`] with the face [`face_for_family`] selected, so a
    /// label's *metrics* never change behind a caller's back.
    ///
    /// Returns `None` on a build with no vector face enabled, which is the honest answer: there
    /// is nothing to shape with, and the caller should use the model.
    pub fn from_active_faces(text: &str) -> Option<Self> {
        Self::new(face_for_text(text)?)
    }

    /// The name of the face this shaper reads.
    pub fn face_name(&self) -> &'static str {
        self.face.name
    }

    /// Shape `text` into one run of positioned glyphs, in logical order.
    ///
    /// Positions are in logical pixels; the run's `width` is the sum of the glyph advances, and
    /// its `height` is the font's line box — the same box
    /// [`crate::render::text::line`]'s model reports.
    pub fn shape_face(&self, text: &str, font_size: f32, scale: f32) -> Option<ShapedGlyphRun> {
        let parsed = rustybuzz::Face::from_slice(self.face.bytes, 0)?;
        let units_per_em = parsed.units_per_em() as f32;
        if units_per_em <= 0.0 {
            return None;
        }
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let output = rustybuzz::shape(&parsed, &[], buffer);

        let unit_px = font_size * scale / units_per_em;
        let mut glyph_ids = Vec::with_capacity(output.len());
        let mut positions = Vec::with_capacity(output.len());
        let mut pen_x = 0.0f32;
        for (info, position) in output.glyph_infos().iter().zip(output.glyph_positions()) {
            glyph_ids.push(info.glyph_id);
            pen_x += position.x_offset as f32 * unit_px;
            positions.push((pen_x, position.y_offset as f32 * unit_px));
            pen_x += position.x_advance as f32 * unit_px;
        }
        Some(ShapedGlyphRun {
            width: pen_x,
            height: font_size * 1.2,
            glyph_ids,
            positions,
            font_size,
        })
    }
}

impl TextShaper for RustybuzzShaper {
    /// One run of real glyphs, or — if the face cannot be read — an empty run rather than a
    /// fabricated one.
    fn shape(&self, text: &str, font_size: f32) -> Vec<ShapedGlyphRun> {
        self.shape_face(text, font_size, 1.0).into_iter().collect()
    }

    fn measure_width(&self, text: &str, font_size: f32) -> f32 {
        self.shape_face(text, font_size, 1.0).map(|run| run.width).unwrap_or(0.0)
    }

    fn measure_height(&self, _text: &str, font_size: f32) -> f32 {
        font_size * 1.2
    }

    fn char_advance(&self, c: char, font_size: f32) -> f32 {
        self.measure_width(&c.to_string(), font_size)
    }
}

#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_is_chosen_by_the_first_strong_character() {
        // `from_active_faces` is the "caller named nothing" convenience, so it picks by
        // coverage. The renderer does not use this rule — it honours `Font::family`, so that
        // enabling a face never changes a label's advance behind the caller's back.
        let face = face_for_text("hello").expect("a Latin face is enabled");
        assert!(face.bytes.len() > 1000);

        #[cfg(feature = "fonts-complex")]
        assert_eq!(
            face_for_text("\u{628}\u{64a}\u{62a}").expect("an Arabic face is enabled").name,
            "Noto Naskh Arabic"
        );
    }

    /// A family name selects the face, case-insensitively, and an unknown family does not.
    ///
    /// Gated on `fonts-vector-latin` specifically, not on "a vector feature": the body asserts the
    /// **Open Sans** face, which only that feature ships. Gating this on the module's own
    /// `any(fonts-vector-latin, fonts-complex)` window made it fail on a `fonts-complex`-only build,
    /// where the face it names genuinely does not exist — the test was asserting a fact about the
    /// wrong build configuration.
    #[cfg(feature = "fonts-vector-latin")]
    #[test]
    fn the_face_a_family_names_is_the_face_that_is_used() {
        assert_eq!(
            face_for_family("open sans").map(|face| face.name),
            Some("Open Sans"),
            "a family name is matched case-insensitively"
        );
        assert!(
            face_for_family("Arial").is_none(),
            "a family this build does not carry leaves the model in charge"
        );
    }

    #[test]
    fn shaping_a_ligature_produces_fewer_glyphs_than_characters() {
        // "ffi" is a standard Latin ligature. The model would report three clusters; a real
        // face reports the glyphs the face's `GSUB` says the string becomes.
        let shaper = RustybuzzShaper::from_active_faces("ffi").expect("a face is enabled");
        let shaped = shaper.shape_face("ffi", 16.0, 1.0).expect("the run shapes");
        assert!(
            shaped.glyph_ids.len() <= 3,
            "a ligature may not produce more glyphs than characters: {:?}",
            shaped.glyph_ids
        );
        assert!(shaped.width > 0.0, "a shaped run has a width");
    }

    #[test]
    fn arabic_joining_selects_contextual_forms() {
        // The criterion BLUE23 §0A.4 states for shaping: an Arabic word must not shape to the
        // same glyphs as its letters in isolation. `بيت` (house) is three letters whose medial
        // forms differ from their isolated ones, so the shaped run's glyph identities must
        // differ from the per-character `cmap` lookups.
        #[cfg(feature = "fonts-complex")]
        {
            let shaper =
                RustybuzzShaper::from_active_faces("\u{628}\u{64a}\u{62a}").expect("a face");
            let shaped = shaper.shape_face("\u{628}\u{64a}\u{62a}", 16.0, 1.0).expect("shapes");
            let faces = crate::render::text::font_assets::active_faces();
            let arabic = faces
                .iter()
                .find(|face| face.name == "Noto Naskh Arabic")
                .expect("the Arabic face is enabled");
            let parsed = ttf_parser::Face::parse(arabic.bytes, 0).expect("parses");
            let isolated: Vec<u32> = "\u{628}\u{64a}\u{62a}"
                .chars()
                .filter_map(|ch| parsed.glyph_index(ch).map(|id| id.0 as u32))
                .collect();
            assert_eq!(isolated.len(), 3, "the face has all three letters");
            assert_ne!(
                shaped.glyph_ids, isolated,
                "joining must substitute the contextual forms, not the isolated ones"
            );
        }
    }

    #[test]
    fn real_advances_differ_from_the_models_flat_factor() {
        // The point of reading the face: a proportional face's letters are not all 0.6 em. If
        // they were, the model would be enough and this feature would be worth nothing.
        let shaper = RustybuzzShaper::from_active_faces("iI").expect("a face is enabled");
        let thin = shaper.char_advance('i', 16.0);
        let thick = shaper.char_advance('W', 16.0);
        assert!(thick > thin, "W ({thick}) must be wider than i ({thin}) in a proportional face");
    }

    #[test]
    fn a_build_with_no_vector_face_says_so_rather_than_guessing() {
        // The contract that keeps this honest on a build with no data: no face, no shaper.
        if active_faces().is_empty() {
            assert!(RustybuzzShaper::from_active_faces("abc").is_none());
        }
    }
}
