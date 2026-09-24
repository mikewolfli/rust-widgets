// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The **bevel** primitive: a face's light/shade pair, and the two lines it draws.
//!
//! # Why this exists as its own module
//!
//! A raised edge is not a colour, it is a **direction**: one pair of edges catches the light and
//! the opposite pair falls into shade. Three places in this crate had worked that out
//! independently — `frame`'s `draw_box_frame` (raised *and* sunken), `frame`'s
//! `draw_win_panel_frame` and several of the older container controls — and each spelled the
//! relationship as its own pair of literals:
//!
//! ```text
//! border.blend(&Color::rgb(255, 255, 255), 0.5)   // "light"
//! border.blend(&Color::rgb(0, 0, 0), 0.5)         // "dark"
//! ```
//!
//! Those two expressions are *correct* — a highlight and a shadow derived from the border colour
//! is what keeps a bevel legible on a dark surface instead of glowing white. What was missing is
//! that they are **one relationship stated twice**, and that the *direction* (does the light come
//! from the top-left, or is this an inset well?) was carried by which pair got which colour —
//! i.e. by the order of two nearly identical blocks of code. Reversing an inset was a copy-paste
//! edit that nothing could check.
//!
//! # What this module does and does not decide
//!
//! It decides **which edges are lit and which are shaded, given a direction**, and it derives the
//! two tones from a caller's base colour. It does **not** choose the base colour, the weight, or
//! whether a bevel happens at all — those stay the caller's, because they are the difference
//! between a flat theme and a skeuomorphic one, and that difference belongs in a theme file
//! rather than in a renderer.
//!
//! # Coordinate convention
//!
//! The crate's screen system: origin top-left, `y` increases **downward**. So "light from above"
//! means the **top** edge is lit and the **bottom** edge is shaded — the opposite of a
//! maths-plot `y` axis, and the single most likely place to get a bevel backwards. The
//! [`BevelDirection::Raised`] doc states it explicitly for that reason.
//!
//! # Usage
//!
//! ```text
//! let bevel = Bevel::from_base(border_color).with_direction(BevelDirection::Raised);
//! bevel.stroke(context, rect, 1);
//! ```

use crate::core::{Color, Point, Rect};
use crate::render::RenderContext;

/// How far a bevel's highlight and shade are stepped from the caller's base colour.
///
/// The value the three existing call sites already used, kept as one constant so they cannot
/// diverge: `0.5` takes the base halfway to white for the lit edges and halfway to black for the
/// shaded ones. Halfway is what makes the pair symmetric — a bevel whose highlight is stronger than
/// its shadow reads as a light source rather than as a raised edge.
pub const BEVEL_WEIGHT: f32 = 0.5;

/// A second, softer pair for the **inner** lines of a double bevel.
///
/// A raised edge in the platform conventions is often two lines: a bright outer highlight with a
/// gentler inner one beneath it, which is what gives the edge thickness. Reusing [`BEVEL_WEIGHT`]
/// for both lines would just draw the same edge twice.
///
/// # Why this pair is symmetric
///
/// The `frame` code these constants come from used `0.25` toward white for its mid-light and `0.75`
/// toward black for its mid-dark. Those are **not** a pair — `0.75` is a *stronger* step than
/// [`BEVEL_WEIGHT`]'s `0.5`, so the "inner" dark line came out darker than the outer one. The reason
/// is that they were not a pair where they came from: `0.25`/`0.75` were the **third and fourth**
/// lines of a four-line groove (outer highlight, outer shadow, then a deeper inner shadow), which is
/// a different shape from a two-line bevel.
///
/// Copying the numbers without the shape is how a constant ends up meaning the opposite of its name,
/// and the test that caught it is `the_inner_pair_follows_the_outer_pair`. The rule this module
/// states instead is the one that is true of a pair: both inner tones are **softer** than the outer
/// ones. `0.25` does that symmetrically on both sides.
pub const BEVEL_INNER_WEIGHT: f32 = 0.25;

/// The shade's counterpart to [`BEVEL_INNER_WEIGHT`].
///
/// Equal to it, and named separately because the two are separate decisions that happen to agree — a
/// theme may want an asymmetric pair (a soft highlight with a firm shade reads as a groove), and
/// having one name for both is what makes that impossible to express later.
pub const BEVEL_INNER_SHADE_WEIGHT: f32 = 0.25;

/// Which way a face is turned.
///
/// The direction is the whole of the primitive: the two tones are the same two tones either way,
/// and only *which edges receive them* changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BevelDirection {
    /// The face bulges **toward** the viewer: light from the top-left, shade on the bottom-right.
    ///
    /// In this crate's coordinate system `y` grows downward, so "light from above" is the **top**
    /// edge. A button, a raised toolbar, a card.
    #[default]
    Raised,
    /// The face is cut **into** the surface: shade on the top-left, light on the bottom-right.
    ///
    /// Exactly [`BevelDirection::Raised`] with the two tones exchanged, which is the point — an
    /// inset well and a raised button are one shape seen from two sides, and stating that as a
    /// swap (rather than as a second copy of the drawing code) is what makes "this should be
    /// sunken" a one-token edit.
    Inset,
}

impl BevelDirection {
    /// The published token for this direction.
    pub fn as_str(self) -> &'static str {
        match self {
            BevelDirection::Raised => "raised",
            BevelDirection::Inset => "inset",
        }
    }

    /// Parses a published token, or answers `None`.
    ///
    /// `None` rather than a default: a theme that misspells `"inset"` should be told, not silently
    /// given a raised edge. The same rule the crate's other token parsers follow.
    ///
    /// Implemented as `FromStr` rather than as an inherent `from_str`, because an inherent method of
    /// that name shadows the trait's and silently makes `"inset".parse::<BevelDirection>()`
    /// uncallable — clippy's `should_implement_trait` catches exactly this, and it was right.
    pub fn parse(token: &str) -> Option<Self> {
        token.parse().ok()
    }
}

impl core::str::FromStr for BevelDirection {
    type Err = ();

    /// The token parser. `Err(())` because a caller matching on a token has nothing to report but
    /// "not one of mine", and the crate's other token parsers return `Option` — this keeps the
    /// `?`-friendly shape available through [`BevelDirection::parse`].
    fn from_str(token: &str) -> Result<Self, Self::Err> {
        match token {
            "raised" => Ok(BevelDirection::Raised),
            "inset" => Ok(BevelDirection::Inset),
            _ => Err(()),
        }
    }
}

/// A resolved bevel: the two tones and which edges they belong to.
///
/// Constructed from the face's own colour, so a bevel cannot disagree with the surface it is cut
/// into. See the module docs for why that matters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bevel {
    /// The tone painted on the **lit** edges.
    pub light: Color,
    /// The tone painted on the **shaded** edges.
    pub shade: Color,
    /// The colour the four tones are derived from.
    ///
    /// Retained rather than discarded after deriving `light`/`shade`, because the **inner** pair
    /// steps from the base rather than from the outer tones — see [`Bevel::inner_tones`] for why.
    pub base: Color,
    /// Which way the face is turned.
    pub direction: BevelDirection,
}

impl Bevel {
    /// Builds a bevel whose tones are `base` stepped toward white and black.
    ///
    /// `base` is the face's own colour — normally the resolved border colour, so a bevel on a
    /// themed face follows that theme.
    pub fn from_base(base: Color) -> Self {
        Self {
            light: base.blend(&Color::WHITE, BEVEL_WEIGHT),
            shade: base.blend(&Color::BLACK, BEVEL_WEIGHT),
            base,
            direction: BevelDirection::default(),
        }
    }

    /// Builds a bevel from an explicit base **and** explicit tones.
    ///
    /// # Why the base is still required when the tones are given
    ///
    /// The inner pair steps from the base (see [`Self::inner_tones`]), so a constructor that took
    /// only the two tones would have to guess one — and any guess would make a double bevel's inner
    /// lines silently wrong while the outer edge looked right. Requiring it means the caller who
    /// wants hand-picked tones states the whole relationship.
    ///
    /// # Why this exists at all
    ///
    /// A theme may want to state the tones rather than have them derived — that is the difference
    /// between "a bevel on this palette" and "a Mac OS 9 bevel on any palette".
    pub fn from_tones(base: Color, light: Color, shade: Color) -> Self {
        Self { light, shade, base, direction: BevelDirection::default() }
    }

    /// Returns this bevel turned the other way.
    pub fn with_direction(mut self, direction: BevelDirection) -> Self {
        self.direction = direction;
        self
    }

    /// The tone on the **top and left** edges for this direction.
    pub fn leading_tone(&self) -> Color {
        match self.direction {
            BevelDirection::Raised => self.light,
            BevelDirection::Inset => self.shade,
        }
    }

    /// The tone on the **bottom and right** edges for this direction.
    pub fn trailing_tone(&self) -> Color {
        match self.direction {
            BevelDirection::Raised => self.shade,
            BevelDirection::Inset => self.light,
        }
    }

    /// The softer pair, for a double bevel's inner lines.
    ///
    /// # Why these blend from the *base* and not from the outer tones
    ///
    /// A first version blended the inner tones from [`Self::light`] / [`Self::shade`], on the
    /// reasoning that the inner line continues the outer one. That is wrong, and measurably so: the
    /// outer highlight is already halfway to white, so stepping *further* toward white made the
    /// inner line **brighter** than the outer — an edge that reads as a second, hotter highlight
    /// rather than as thickness. The hand-written call sites this replaces had it right by writing
    /// `border.blend(WHITE, 0.25)`: the inner tones step from the **base**, so they sit strictly
    /// between it and the outer pair. The test that caught this is
    /// `the_inner_pair_follows_the_outer_pair`.
    ///
    /// Returned in the same order as [`Self::leading_tone`] / [`Self::trailing_tone`], so the
    /// inner lines cannot end up on the opposite edges from the outer ones — which is the defect
    /// this whole module is arranged to make impossible.
    pub fn inner_tones(&self) -> (Color, Color) {
        let (light, shade) = (
            self.base.blend(&Color::WHITE, BEVEL_INNER_WEIGHT),
            self.base.blend(&Color::BLACK, BEVEL_INNER_SHADE_WEIGHT),
        );
        match self.direction {
            BevelDirection::Raised => (light, shade),
            BevelDirection::Inset => (shade, light),
        }
    }

    /// Strokes the bevel's four edges on the inside of `rect`.
    ///
    /// `width` is the line thickness in logical pixels. The edges are drawn **on** the rectangle's
    /// own boundary rather than inset by `width`, which is what the call sites this replaces did;
    /// a caller that wants the bevel inside a border stroke should pass the inner rectangle.
    pub fn stroke(&self, context: &mut RenderContext, rect: Rect, width: u32) {
        let (leading, trailing) = (self.leading_tone(), self.trailing_tone());
        let (x0, y0) = (rect.x as f32, rect.y as f32);
        let (x1, y1) = (rect.x as f32 + rect.width as f32, rect.y as f32 + rect.height as f32);

        // Top then left, so a corner pixel is drawn by the leading pair…
        self.edge(context, Point::from_f32(x0, y0), Point::from_f32(x1, y0), leading, width);
        self.edge(context, Point::from_f32(x0, y0), Point::from_f32(x0, y1), leading, width);
        // …and bottom then right by the trailing pair. Both pairs include the corners, so a
        // 1-pixel bevel has no gap at the diagonals; the trailing line wins there, which is the
        // same precedence the hand-written call sites had.
        self.edge(context, Point::from_f32(x0, y1), Point::from_f32(x1, y1), trailing, width);
        self.edge(context, Point::from_f32(x1, y0), Point::from_f32(x1, y1), trailing, width);
    }

    /// Strokes the bevel's four edges offset inward by one line width, using the softer pair.
    ///
    /// This is the second half of a double bevel — the "thickness" beneath the highlight.
    pub fn stroke_inner(&self, context: &mut RenderContext, rect: Rect, width: u32) {
        let inset = width as i32;
        let inner = Rect::new(
            rect.x + inset,
            rect.y + inset,
            rect.width.saturating_sub(width * 2),
            rect.height.saturating_sub(width * 2),
        );
        let (leading, trailing) = self.inner_tones();
        let (x0, y0) = (inner.x as f32, inner.y as f32);
        let (x1, y1) = (inner.x as f32 + inner.width as f32, inner.y as f32 + inner.height as f32);
        self.edge(context, Point::from_f32(x0, y0), Point::from_f32(x1, y0), leading, width);
        self.edge(context, Point::from_f32(x0, y0), Point::from_f32(x0, y1), leading, width);
        self.edge(context, Point::from_f32(x0, y1), Point::from_f32(x1, y1), trailing, width);
        self.edge(context, Point::from_f32(x1, y0), Point::from_f32(x1, y1), trailing, width);
    }

    /// One edge, at the requested thickness.
    fn edge(&self, context: &mut RenderContext, from: Point, to: Point, color: Color, width: u32) {
        // Always the stroked form: a bevel is a line of a stated width, and the plain `draw_line`
        // the call sites used could only ever be one pixel — so a `line_width` of 2 on a themed
        // frame silently drew a 1-pixel bevel beside a 2-pixel border.
        context.draw_line_stroke(from, to, color, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    /// Renders a bevel and returns the pixels it painted, as `(x, y, colour)`.
    ///
    /// `end_frame` is what flushes the software backend's batched work — reading the surface before
    /// it sees a partially drawn frame, which is what the trait's own docs warn about.
    fn painted<F>(size: Size, f: F) -> Vec<(i32, i32, Color)>
    where
        F: FnOnce(&mut RenderContext),
    {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgba(0, 0, 0, 0));
        {
            let mut context = RenderContext::new(&mut backend);
            f(&mut context);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        let mut out = Vec::new();
        for y in 0..size.height as i32 {
            for x in 0..size.width as i32 {
                let at = ((y as u32 * size.width + x as u32) * 4) as usize;
                let px = Color::rgba(rgba[at], rgba[at + 1], rgba[at + 2], rgba[at + 3]);
                // The frame begins fully transparent, so anything opaque is paint.
                if px.a != 0 {
                    out.push((x, y, px));
                }
            }
        }
        out
    }

    /// A face with room on every side for a multi-pixel brush.
    ///
    /// The stroke brush is **centred on the line's coordinate** (`-width/2 ..= width/2`, see
    /// `draw_line_with_width`), so a bevel on the edge at `y = 0` paints into `y = -1` and is
    /// clipped there. The rect is therefore inset from the surface by a margin, so every edge's
    /// brush lands inside the raster — because otherwise "the top edge is lit" is untestable at any
    /// width above 1, which is how the first draft of these tests mis-measured.
    const RECT: Rect = Rect { x: 4, y: 4, width: 20, height: 10 };
    /// The surface `RECT` is drawn on, with room for the widest brush a test uses.
    const SURFACE: Size = Size { width: 28, height: 18 };

    /// The colour of the pixel at `(x, y)` among the painted ones.
    fn at(px: &[(i32, i32, Color)], x: i32, y: i32) -> Color {
        px.iter()
            .find(|(px, py, _)| *px == x && *py == y)
            .unwrap_or_else(|| panic!("no paint at {x},{y}"))
            .2
    }

    /// A raised bevel lights the **top** edge in this coordinate system, where `y` grows
    /// downward.
    ///
    /// This is the single most likely place to get a bevel backwards, so it is stated as a test
    /// rather than left to the reader: with a maths-plot convention the *bottom* would be lit, and
    /// a `y`-flip somewhere upstream would silently invert every raised edge in the library.
    #[test]
    fn a_raised_bevel_lights_the_top_and_shades_the_bottom() {
        let bevel = Bevel::from_base(Color::rgb(128, 128, 128));
        let px = painted(SURFACE, |c| bevel.stroke(c, RECT, 1));

        let mid_x = RECT.x + RECT.width as i32 / 2;
        let top = at(&px, mid_x, RECT.y);
        let bottom = at(&px, mid_x, RECT.y + RECT.height as i32);
        assert!(
            top.luminance() > bottom.luminance(),
            "the top must be lighter than the bottom: top {top:?} vs bottom {bottom:?}"
        );
    }

    /// An inset is the **same two tones with the edges exchanged** — not a second drawing.
    #[test]
    fn an_inset_is_a_raised_bevel_with_its_edges_exchanged() {
        let raised = Bevel::from_base(Color::rgb(128, 128, 128));
        let inset = raised.with_direction(BevelDirection::Inset);
        assert_eq!(raised.light, inset.light, "the tones are the same");
        assert_eq!(raised.shade, inset.shade, "the tones are the same");
        assert_eq!(raised.leading_tone(), inset.trailing_tone());
        assert_eq!(raised.trailing_tone(), inset.leading_tone());

        let px = painted(SURFACE, |c| inset.stroke(c, RECT, 1));
        let mid_x = RECT.x + RECT.width as i32 / 2;
        let top = at(&px, mid_x, RECT.y);
        let bottom = at(&px, mid_x, RECT.y + RECT.height as i32);
        assert!(
            top.luminance() < bottom.luminance(),
            "an inset must invert the pair: top {top:?} vs bottom {bottom:?}"
        );
    }

    /// The inner lines land on the **same** edges as the outer ones, one step softer.
    ///
    /// This is the relationship a hand-written double bevel gets wrong when someone copies the
    /// outer block and edits one of the four lines: the inner highlight ends up on the shaded side
    /// and the edge reads as a seam rather than as a thickness.
    ///
    /// # Why the assertion is stated against the *base*, not against "leading is lighter"
    ///
    /// The leading edge is the highlight when raised and the **shade** when inset — that is what
    /// the direction means. A first version asserted "the inner leading line is the lighter of the
    /// two", which is true only for a raised face and so failed against a correct inset. The
    /// invariant that holds for both is the nesting: each inner tone sits strictly between the base
    /// and its own outer tone.
    #[test]
    fn the_inner_pair_follows_the_outer_pair() {
        for direction in [BevelDirection::Raised, BevelDirection::Inset] {
            let base = Color::rgb(128, 128, 128);
            let bevel = Bevel::from_base(base).with_direction(direction);
            let (inner_leading, inner_trailing) = bevel.inner_tones();
            let (outer_lit, outer_shaded) = (bevel.light.luminance(), bevel.shade.luminance());
            // The inner pair must shadow the outer pair's *order*, whichever way the face faces:
            // the inner leading tone is on the same side of the base as the outer leading tone.
            let side = |c: Color| c.luminance() - base.luminance();
            assert_eq!(
                side(inner_leading).signum(),
                side(bevel.leading_tone()).signum(),
                "{direction:?}: the inner leading line must be on the same side of the base as the \
                 outer leading line"
            );
            assert_eq!(
                side(inner_trailing).signum(),
                side(bevel.trailing_tone()).signum(),
                "{direction:?}: and the inner trailing line likewise"
            );
            // Nesting, stated so it holds for both directions: each inner tone is **less extreme**
            // than its own outer tone. Otherwise the "second line" is just the first drawn again,
            // or — the defect — hotter than the outer one.
            assert!(
                side(inner_leading).abs() < side(bevel.leading_tone()).abs(),
                "{direction:?}: the inner highlight must be softer than the outer one"
            );
            assert!(
                side(inner_trailing).abs() < side(bevel.trailing_tone()).abs(),
                "{direction:?}: the inner shade must be softer than the outer one"
            );
            // And the outer pair really is a pair: one lighter than the base, one darker.
            assert!(outer_lit > base.luminance() && outer_shaded < base.luminance());
        }
    }

    /// A bevel's tones are derived from the caller's base, so it follows a themed face.
    #[test]
    fn the_tones_are_derived_from_the_base_colour() {
        let dark = Bevel::from_base(Color::rgb(30, 30, 33));
        let light = Bevel::from_base(Color::rgb(240, 240, 240));
        assert_ne!(dark.light, light.light, "the highlight must follow the base");
        assert_ne!(dark.shade, light.shade, "the shade must follow the base");
        // And the relationship holds on both: a bevel on a dark face still has a lighter and a
        // darker side, which is what a hardcoded white/black pair would get wrong.
        for bevel in [dark, light] {
            assert!(bevel.light.luminance() > bevel.shade.luminance());
        }
    }

    /// The stroke width reaches every edge, as a brush **centred on the edge's coordinate**.
    ///
    /// The call sites this replaces used `draw_line`, which is one pixel wide whatever the face's
    /// `line_width` says — so a 2-pixel themed border had a 1-pixel bevel beside it. The brush's
    /// straddle is the same fact the [`RECT`] note records, and it is asserted here rather than
    /// assumed, because a bevel drawn only outside its rectangle would sit on top of the border.
    #[test]
    fn the_stroke_width_applies_to_every_edge() {
        for width in [1u32, 2, 3] {
            let bevel = Bevel::from_base(Color::rgb(128, 128, 128));
            let px = painted(SURFACE, |c| bevel.stroke(c, RECT, width));
            let mid_x = RECT.x + RECT.width as i32 / 2;
            // `brush_start = -(width/2)`, `brush_end = brush_start + width - 1`, so the brush
            // covers exactly `width` rows from `edge - width/2`.
            let first = RECT.y - width as i32 / 2;
            for dy in 0..width as i32 {
                let y = first + dy;
                let c = at(&px, mid_x, y);
                assert!(
                    c.luminance() > 128.0 / 255.0,
                    "a {width}px bevel must light row {y} of the top edge (row {dy} of the brush), \
                     but it is {c:?}"
                );
            }
            // And it must stop there: one row past the brush on either side is unpainted.
            for y in [first - 1, first + width as i32] {
                assert!(
                    !px.iter().any(|(x, py, _)| *x == mid_x && *py == y),
                    "a {width}px bevel must not reach row {y}"
                );
            }
        }
    }

    /// A bevel narrower than its own double stroke does not panic or wrap.
    ///
    /// `stroke_inner` insets by the line width on both sides, which a small rectangle cannot
    /// always afford. Saturating rather than underflowing is what keeps a 2x2 face from panicking
    /// on a large `line_width`.
    #[test]
    fn a_face_too_small_for_its_inner_lines_degrades_rather_than_panics() {
        let bevel = Bevel::from_base(Color::rgb(128, 128, 128));
        let tiny = Rect::new(0, 0, 2, 2);
        let px = painted(Size::new(4, 4), |c| {
            bevel.stroke(c, tiny, 4);
            bevel.stroke_inner(c, tiny, 4);
        });
        // The outer stroke still painted something; no panic is the property under test.
        assert!(!px.is_empty(), "a saturated inner rect must still paint the outer bevel");
    }

    /// The two directions are the only two tokens, and an unknown token is refused.
    #[test]
    fn the_direction_tokens_round_trip_and_reject_unknown() {
        assert_eq!(BevelDirection::parse("raised"), Some(BevelDirection::Raised));
        assert_eq!(BevelDirection::parse("inset"), Some(BevelDirection::Inset));
        assert_eq!(BevelDirection::parse("sunken"), None, "not a token this crate publishes");
        assert_eq!(BevelDirection::parse(""), None);
        assert_eq!(BevelDirection::parse("Raised"), None, "tokens are lower-case");
        for direction in [BevelDirection::Raised, BevelDirection::Inset] {
            assert_eq!(BevelDirection::parse(direction.as_str()), Some(direction));
        }
        // And the `FromStr` impl behind it is reachable the idiomatic way.
        assert_eq!("inset".parse::<BevelDirection>(), Ok(BevelDirection::Inset));
        assert_eq!("nope".parse::<BevelDirection>(), Err(()));
    }

    /// Every tone is a step from the base **toward one of the two extremes**, on every channel.
    ///
    /// # The property, stated so it can actually fail
    ///
    /// A first version of this test asserted every channel of every tone was `>=` the base's, on the
    /// reasoning that a bevel is "lighter". That is wrong — half the tones are steps toward black,
    /// where every channel *decreases* — so the test failed against a correct implementation
    /// (`rgb(120,40,200)` -> `rgb(15,5,25)`), which is what a test asserting the wrong invariant
    /// looks like. The real property is per-tone: a tone is the base blended toward white or toward
    /// black, and in the first case no channel may be below the base's while in the second none may
    /// be above it.
    #[test]
    fn every_tone_is_a_step_toward_one_extreme_on_every_channel() {
        let base = Color::rgb(120, 40, 200);
        let bevel = Bevel::from_base(base);
        let (inner_light, inner_shade) = bevel.inner_tones();
        let tones = [
            ("light", bevel.light, true),
            ("shade", bevel.shade, false),
            ("inner light", inner_light, true),
            ("inner shade", inner_shade, false),
        ];
        for (name, tone, toward_white) in tones {
            for (channel, tone_c, base_c) in
                [("r", tone.r, base.r), ("g", tone.g, base.g), ("b", tone.b, base.b)]
            {
                let ok = if toward_white { tone_c >= base_c } else { tone_c <= base_c };
                assert!(
                    ok,
                    "{name}: channel {channel} of {tone:?} moved the wrong way from base {base:?} \
                     ({tone_c} vs {base_c}); a tone stepped toward white must raise every channel \
                     and one stepped toward black must lower every channel"
                );
            }
        }
    }

    /// The derived tones are exactly the two expressions the existing call sites wrote by hand.
    ///
    /// This is the compatibility claim of the whole module: a caller that adopts `Bevel` gets the
    /// **same pixels** it drew before, so replacing those call sites is a refactor rather than a
    /// visual change. Without this test the claim would live only in a doc comment, and the first
    /// caller to migrate would find out the hard way.
    #[test]
    fn the_derivation_reproduces_the_hand_written_expressions() {
        let base = Color::rgb(100, 150, 200);
        let bevel = Bevel::from_base(base);
        assert_eq!(bevel.light, base.blend(&Color::WHITE, BEVEL_WEIGHT));
        assert_eq!(bevel.shade, base.blend(&Color::BLACK, BEVEL_WEIGHT));
        let (inner_light, inner_shade) = bevel.inner_tones();
        // The inner pair steps from the **base**, not from the outer tones — see
        // `Bevel::inner_tones` for why (blending on from the outer highlight made the inner line
        // brighter than the outer one).
        assert_eq!(inner_light, base.blend(&Color::WHITE, BEVEL_INNER_WEIGHT));
        assert_eq!(inner_shade, base.blend(&Color::BLACK, BEVEL_INNER_SHADE_WEIGHT));
    }
}
