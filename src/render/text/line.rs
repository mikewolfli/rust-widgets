// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Turning one line of text into the clusters a renderer paints — in **visual** order.
//!
//! # Why this is one function and not a method on each backend
//!
//! The software rasteriser and the SVG backend both need "the clusters of this string at this
//! font size", and both must agree: a control that measures with one and draws with the other
//! would wrap differently than it paints. They used to hold a copy of this derivation each,
//! which is a promise that the two copies stay identical — and the moment one of them learned
//! about bidirectional reordering, that promise would have been broken silently, with the
//! raster frame right-to-left and the `<path>` output left-to-right.
//!
//! So the derivation lives here, once, and a backend supplies only the two things it actually
//! knows: the string, the font, and the device scale.
//!
//! # The four steps
//!
//! 1. **Grapheme clustering** — a base plus its combining marks, variation selectors and
//!    ZWJ-joined emoji is one cluster, because it is one glyph's worth of ink and one unit a
//!    bidirectional reorder must not split.
//! 2. **Advances** — one advance per cluster from the crate's single advance model
//!    ([`estimate_cluster_advance`]), plus the font's tracking between clusters.
//! 3. **Reordering** — clusters are permuted into visual order
//!    ([`crate::render::text::bidi`]), which is the identity for a left-to-right line.
//! 4. **Reporting** — the total advance, which is invariant under reordering (a sum), so the
//!    width a control reserved before the reorder still holds after it.

use crate::compat::{String, Vec};
use crate::core::Font;
use crate::render::{ShapedText, TextCluster};

pub(crate) use crate::render::grapheme::{is_combining_mark, is_variation_selector};

/// Call `visit` once per grapheme cluster of `text`, in **logical** order.
///
/// Each call receives the cluster's text and its `(start, end)` byte range in `text`.
///
/// # Why this is the crate's one clustering rule
///
/// "What counts as one cluster" is asked by three callers that must agree: the renderer, which
/// paints one cluster at a time; the bidirectional reorder, which must not split one; and a
/// control's implicit-size estimate, which must count the same clusters the renderer advances.
/// They used to answer it separately — the estimate's copy did not merge an emoji's
/// zero-width-joiner continuation at all, so a family emoji measured two ems and painted one.
/// Sharing the traversal makes agreement structural instead of a test's job.
///
/// # Why it visits rather than returns
///
/// The two consumers want different things from the same walk — a cluster *vector* for painting,
/// a running *sum* for measuring — and the measuring one must not allocate a vector per call.
/// A callback gives both the traversal without picking a side.
///
/// `buffer` is reused across clusters, so the walk costs one allocation regardless of length.
pub(crate) fn for_each_cluster(text: &str, mut visit: impl FnMut(&str, (usize, usize))) {
    let mut buffer = String::new();
    let mut start = 0usize;
    for (index, scalar) in text.char_indices() {
        // A scalar joins the previous cluster when the latter is still open and either it ends
        // in a zero-width joiner (so this scalar is what it joins to) or this scalar is itself a
        // mark that has no advance of its own.
        let joins = !buffer.is_empty()
            && (buffer.ends_with('\u{200D}')
                || scalar == '\u{200D}'
                || is_combining_mark(scalar)
                || is_variation_selector(scalar));
        if joins {
            buffer.push(scalar);
        } else {
            if !buffer.is_empty() {
                visit(&buffer, (start, index));
                buffer.clear();
            }
            start = index;
            buffer.push(scalar);
        }
    }
    if !buffer.is_empty() {
        visit(&buffer, (start, text.len()));
    }
}

/// Shape one line of text for a backend at `scale`.
///
/// See the module docs for the four steps. `scale` is the device scale the advances are
/// expressed in; it is not applied twice, and it is the same value the backend uses for every
/// other metric, so a line measures and paints at one scale.
pub(crate) fn shape_line(text: &str, font: &Font, scale: f32) -> ShapedText {
    let mut clusters: Vec<TextCluster> = Vec::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for_each_cluster(text, |cluster_text, range| {
        clusters.push(TextCluster {
            text: cluster_text.into(),
            advance: estimate_cluster_advance(cluster_text, font.size(), scale),
        });
        ranges.push(range);
    });

    // The tracking is added to each cluster's advance **here**, in the one function both the
    // measure path and the draw path go through. Adding it only in `draw_text` would make a
    // tracked font measure narrower than it paints — the "measure with A, draw with B" defect
    // this crate has paid for once already (§G.5), and a `letter_spacing` that silently changed
    // how much room a label reserves is worse than no tracking at all.
    let tracking = font.letter_spacing() * scale;
    // Real metrics when a face is enabled: the advance is what the face's `hmtx` and its `GPOS`
    // kerning say, rather than the model's flat factor. `None` — no face, or a build with no
    // vector data — leaves the model as the whole answer, which is why the default build's
    // output does not change when this exists.
    #[cfg(feature = "text-shaping")]
    let real_advances = crate::render::text::shaping::cluster_advances(text, &ranges, font, scale);
    #[cfg(not(feature = "text-shaping"))]
    let real_advances: Option<Vec<f32>> = None;

    let count = clusters.len();
    for (index, cluster) in clusters.iter_mut().enumerate() {
        cluster.advance = match &real_advances {
            Some(advances) if advances.len() == count => advances[index].max(0.0),
            _ => estimate_cluster_advance(&cluster.text, font.size(), scale),
        };
    }
    let mut total_advance: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    // `n` clusters have `n - 1` inter-cluster gaps, so a trailing tracking would extend past
    // the end of the run and make a centred label sit visibly left of centre.
    if tracking != 0.0 && !clusters.is_empty() {
        total_advance += tracking * (clusters.len() - 1) as f32;
    }

    let order = crate::render::text::bidi::visual_order(text, &ranges);
    // Reordering is the identity for every left-to-right line, so the permutation is checked
    // before a second vector is built: the common path allocates nothing extra.
    if order.iter().enumerate().any(|(visual, &logical)| visual != logical) {
        let mut reordered: Vec<TextCluster> = Vec::with_capacity(clusters.len());
        for &logical in &order {
            reordered.push(clusters[logical].clone());
        }
        clusters = reordered;
    }

    ShapedText { clusters, advance: total_advance }
}

/// Whether a scalar occupies a full em rather than a fraction of one.
///
/// This is a **property of the character**, not of any font: the CJK blocks, Hangul, the
/// fullwidth forms and emoji are drawn one em wide because their scripts say so, and every
/// face in this crate (and in practise every face a host will supply) lays them out that way.
///
/// # Why the table lives here
///
/// The identical question is asked in two places that must never disagree — the renderer's
/// advance model (below) and a control's implicit-size estimate
/// ([`crate::widget::metrics`]). A second copy of a range table is a copy that drifts, and the
/// drift is invisible: a slightly narrow advance makes a label wrap one word late, which no
/// assertion notices. One table, exported, answers both.
pub fn is_wide_scalar(scalar: char) -> bool {
    matches!(
        scalar as u32,
        0x1100..=0x115F        // Hangul Jamo initial consonants
            | 0x2329..=0x232A  // angle brackets
            | 0x2E80..=0xA4CF  // CJK radicals … Yi syllables
            | 0xAC00..=0xD7A3  // Hangul syllables
            | 0xF900..=0xFAFF  // CJK compatibility ideographs
            | 0xFE10..=0xFE19  // vertical forms
            | 0xFE30..=0xFE6F  // CJK compatibility forms
            | 0xFF00..=0xFF60  // fullwidth forms
            | 0xFFE0..=0xFFE6  // fullwidth signs
            | 0x1F300..=0x1FAFF // emoji and pictographs
            | 0x20000..=0x3FFFD // CJK extensions B and beyond
    )
}

/// One cluster's advance under the crate's single advance model.
///
/// A cluster is charged a full em if it contains a wide scalar, a third of an em if it is
/// blank (whitespace still advances, it just paints nothing), and a fraction of an em
/// otherwise. `scale` is the device scale: an advance is a device-space distance, which is why
/// it is multiplied by it here and not by the caller.
pub fn estimate_cluster_advance(cluster: &str, font_size: f32, scale: f32) -> f32 {
    if cluster.trim().is_empty() {
        return (font_size * 0.33 * scale).max(1.0);
    }
    let has_wide = cluster.chars().any(is_wide_scalar);
    let factor = if has_wide { 1.0 } else { 0.6 };
    (font_size * factor * scale).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;

    fn font() -> Font {
        Font::new("sans", 10.0, false, false)
    }

    #[test]
    fn a_left_to_right_line_keeps_its_logical_order() {
        let shaped = shape_line("abc", &font(), 1.0);
        let texts: Vec<&str> = shaped.clusters().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["a", "b", "c"]);
    }

    #[test]
    fn an_arabic_line_is_reordered_for_painting() {
        // Logical order in, visual order out: the last logical character is painted first.
        let shaped = shape_line("\u{05e9}\u{05dc}\u{05d5}\u{05dd}", &font(), 1.0);
        let texts: Vec<&str> = shaped.clusters().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["\u{05dd}", "\u{05d5}", "\u{05dc}", "\u{05e9}"]);
    }

    #[test]
    fn reordering_does_not_change_the_measured_width() {
        // The width a control reserved must survive the reorder, or an RTL label would overrun
        // the box an LTR one fits.
        let text = "\u{05e9}\u{05dc}\u{05d5}\u{05dd} abc";
        let shaped = shape_line(text, &font(), 1.0);
        let summed: f32 = shaped.clusters().iter().map(|c| c.advance).sum();
        assert!((shaped.advance() - summed).abs() < 0.01, "the advance is the sum, in any order");
    }

    #[test]
    fn a_combining_mark_merges_into_its_base_cluster() {
        let shaped = shape_line("a\u{0301}b", &font(), 1.0);
        let texts: Vec<&str> = shaped.clusters().iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["a\u{0301}", "b"], "the accent must not become its own cluster");
    }

    #[test]
    fn a_wide_cluster_is_charged_a_full_em() {
        assert!(is_wide_scalar('\u{4e2d}'), "a CJK ideograph");
        assert!(is_wide_scalar('\u{ac00}'), "a Hangul syllable");
        assert!(is_wide_scalar('\u{ff21}'), "a fullwidth form");
        assert!(is_wide_scalar('\u{1f600}'), "an emoji");
        assert!(is_wide_scalar('\u{20000}'), "a CJK extension-B ideograph");
        assert!(!is_wide_scalar('A'));
        assert!(!is_wide_scalar('\u{0416}'), "Cyrillic is narrow");
    }
}
