// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core rendering data types for text and geometry.

use crate::compat::{String, Vec};

/// Measured dimensions of a laid-out text run, in logical pixels.
///
/// Every field is a whole number of pixels rounded by the measuring backend,
/// so `ascent + descent` may differ from `height` by a rounding remainder.
pub struct TextMetrics {
    /// Measured text width in logical pixels.
    pub width: u32,
    /// Measured text height in logical pixels.
    pub height: u32,
    /// Baseline ascent in logical pixels.
    pub ascent: u32,
    /// Baseline descent in logical pixels.
    pub descent: u32,
}

impl TextMetrics {
    /// The line-box metrics `font` resolves to at `scale` device pixels per logical pixel.
    ///
    /// # The defect this exists to remove
    ///
    /// The software surface and the SVG backend each derived these four numbers **inline**, and
    /// they had drifted: the SVG backend clamped the font's effective leading with
    /// `.max(1.0)` before using it, the software surface did not. Both then computed the ascent as
    /// `line_height * 0.8`, so for a small leading they disagreed about **where the baseline is**
    /// (`line_height = 0.2`: ascent `0` on one backend, `1` on the other) while agreeing on the
    /// height. A control sized against one backend's line box therefore rendered a line off-centre
    /// in the other — the exact "the two backends must agree" failure both files' own comments
    /// warn about.
    ///
    /// The clamp belongs in one place, and the place is here: the two backends differ in *how they
    /// rasterise*, not in what a line box is. `width` is not part of this because it depends on the
    /// text, which the caller has already shaped.
    pub fn for_font(font: &crate::core::Font, scale: f32) -> Self {
        // A font with no size and no explicit leading resolves to a zero-height line, which would
        // make every line box collapse. One device pixel is the floor, and it is applied to the
        // leading (not only to the rounded height) so the ascent is derived from the same value.
        let line_height = font.effective_line_height().max(1.0) * scale;
        let height = line_height.round().max(1.0) as u32;
        // The usual typographic split: about four fifths of a line box is ascender.
        let ascent = (line_height * 0.8).round() as u32;
        let descent = height.saturating_sub(ascent);
        Self { width: 0, height, ascent, descent }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;

    /// A zero-sized font still produces a usable line box, not a collapsed one.
    ///
    /// The floor exists because a theme or a caller can hand over a `Font` with no size, and a
    /// zero-height line box makes every text measurement zero — which lays text out at a point.
    #[test]
    fn a_degenerate_font_still_yields_a_one_pixel_line_box() {
        let metrics = TextMetrics::for_font(&Font::simple("sans-serif", 0.0), 1.0);
        assert_eq!(metrics.height, 1, "a line box must never collapse to zero height");
        assert_eq!(metrics.ascent, 1, "the ascent is derived from the same floored leading");
        assert!(metrics.descent <= metrics.height, "descent + ascent must fit the box");
    }

    /// The metrics scale with the device pixel ratio.
    #[test]
    fn the_line_box_scales_with_the_device_ratio() {
        let font = Font::simple("sans-serif", 10.0);
        let at_1x = TextMetrics::for_font(&font, 1.0);
        let at_2x = TextMetrics::for_font(&font, 2.0);
        assert_eq!(at_1x.height, 10);
        assert_eq!(at_2x.height, 20, "a 2x surface must measure a 2x line box");
    }

    /// An explicit line height is what the box uses, not the point size.
    ///
    /// This is the field whose absence of a consumer BLUE22 · F-10 recorded; the measurement is
    /// where it has to take effect or setting it changes nothing.
    #[test]
    fn an_explicit_leading_wins_over_the_point_size() {
        let mut font = Font::simple("sans-serif", 10.0);
        font.set_line_height(18.0);
        let metrics = TextMetrics::for_font(&font, 1.0);
        assert_eq!(metrics.height, 18, "`set_line_height` must reach the measured line box");
    }
}
/// One shaped text cluster produced by the render text shaper.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    /// Cluster source text (one or more unicode scalars).
    pub text: String,
    /// Logical horizontal advance in pixels.
    pub advance: f32,
}
/// Shaped text run composed from ordered clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    pub(crate) clusters: Vec<TextCluster>,
    pub(crate) advance: f32,
}
impl ShapedText {
    /// Returns ordered text clusters in this shaped run.
    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }
    /// Returns cluster count in this shaped run.
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }
    /// Returns total horizontal advance in logical pixels.
    pub fn advance(&self) -> f32 {
        self.advance
    }
}
