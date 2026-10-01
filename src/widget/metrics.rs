// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Content-driven control metrics — the "implicit size" system.
//!
//! # Why this module exists
//!
//! A control is given a rectangle by whatever placed it (a layout, a designer, a
//! census cell), and that rectangle is its **available area** — how much room it
//! may occupy, not how big it should draw. Those are two different questions, and
//! conflating them is what turned `switch.svg` into a 240x120 stadium: the census
//! hands every control a 240x120 box, and a control that treats that box as a
//! *drawing instruction* paints a 240-wide track.
//!
//! The shared design-system table answers the second question with
//!
//! ```text
//! implicitWidth = max(implicitBackgroundWidth + leftInset + rightInset,
//!                     implicitContentWidth   + leftPadding + rightPadding)
//! ```
//!
//! and the key term is the `max`: the **background is a minimum tappable
//! floor**, not decoration. That is why a button labelled with 5 px text is
//! still 100x40 rather than 100x18.
//!
//! [`ControlMetrics`] is the Rust-native spelling of that formula: no trait
//! objects, no virtual dispatch, just pure functions over value types.
//!
//! The [`metrics`] constants alongside it are the numeric half of the same
//! system — the agreed size of a switch track, a checkbox box, a progress bar.
//! They are constants rather than per-control literals so that the answer to
//! "how thick is a progress bar" exists in exactly one place (rule #101).
//!
//! # Removed: `ControlMetrics::drawn_box`
//!
//! There used to be a `drawn_box(rect, intrinsic)` that returned
//! `center_in(rect, intrinsic)` verbatim — an alias with **zero call sites** in the crate.
//! Rule #51 asks a shared abstraction to earn its place by eliminating a real
//! duplication; a second name for one function eliminates nothing and only leaves
//! two spellings of the same derivation to drift apart (rule #101). It is therefore
//! deleted rather than kept "for symmetry". A control that wants a fixed piece of
//! chrome centred reaches for [`ControlMetrics::center_in`]; a **panel** reaches for
//! [`ControlMetrics::painted_box`], which differs in the one way a panel needs — it is
//! clamped up to a one-pixel floor so a squeezed dialog stays visible instead of
//! collapsing to a zero-extent, invisible rect.

use crate::core::{Rect, Size};
use crate::render::text::{estimate_cluster_advance, for_each_cluster};
use crate::style::EdgeOffsets;

/// The width `text` occupies in `font` at `scale`, without a render context.
///
/// # Why a widget needs this
///
/// A control's `size_hint` answers "how wide am I?" and is called by layouts that hold no
/// [`RenderContext`](crate::render::RenderContext) — a `Layout::arrange` asks a child its
/// size before anything is painted. The renderer's own measurement needs a backend, so a
/// control that wants the honest answer there had to write its own arithmetic instead, and
/// that is how `text.len() * 8 + 4` kept appearing: a private copy of a public fact.
///
/// # What model this uses, and why the answer has two tiers
///
/// **With a face** (a build that ships vector/bitmap faces — every `desktop`/`tablet`/`mobile`
/// profile) the cluster advances come from the face's own layout tables, through the same
/// [`crate::render::text::shaping::cluster_advances`] the renderer's `shape_line` uses. That is
/// the renderer's real ruler, so a `size_hint` and the paint agree glyph for glyph.
///
/// **Without a face** it falls back to the same grapheme traversal (`for_each_cluster`) and the
/// same advance function (`estimate_cluster_advance`) the renderer uses in that case, so the two
/// still cannot disagree about which scalars are wide or how a cluster is segmented.
///
/// # Why the fallback is not enough on its own
///
/// The flat per-cluster model puts every character at ~0.6 em. On a proportional face that is
/// wrong for most glyphs (`i` and `W` are not the same width), and the errors only cancel in
/// aggregate: the crate measured `i` as 8.40 px against a real 3.53 px. The totals often look
/// close, which is exactly why this hid — a *centred* string could still sit off centre.
///
/// # Cost
///
/// The face path parses (and caches) a shaper face per active face — see
/// [`crate::render::text::shaping::cluster_advances`]; the fallback reuses one cluster buffer
/// per call and allocates nothing per cluster, so it stays safe on the layout hot path.
pub fn estimate_text_width(text: &str, font: &crate::core::Font, scale: f32) -> u32 {
    let size = font.size().max(0.0);
    if text.is_empty() || size == 0.0 {
        return 0;
    }

    // Collect the clusters once, so both tiers measure the same segmentation.
    let mut ranges: crate::compat::Vec<(usize, usize)> = crate::compat::Vec::new();
    let mut clusters = 0usize;
    for_each_cluster(text, |_cluster, range| {
        ranges.push(range);
        clusters += 1;
    });

    // Real metrics when a face is enabled, exactly as `shape_line` does: the advance is what
    // the face's `hmtx` and `GPOS` kerning say. `None` — no face, or a build with no vector
    // data — leaves the flat model as the whole answer, so a default build's numbers do not
    // change when this exists.
    #[cfg(feature = "text-shaping")]
    // `cluster_advances` returns `None` rather than an empty vector when no face applies,
    // so the flattening is only about the inner `with_shaper_face` result.
    let real_advances: Option<crate::compat::Vec<f32>> =
        crate::render::text::shaping::cluster_advances(text, &ranges, font, scale);
    #[cfg(not(feature = "text-shaping"))]
    let real_advances: Option<crate::compat::Vec<f32>> = None;

    let mut advance = match &real_advances {
        // A length mismatch means the shaper returned an answer for a different segmentation;
        // falling back is safer than crediting the wrong cluster.
        Some(advances) if advances.len() == clusters => advances.iter().sum::<f32>(),
        _ => {
            let mut total = 0.0f32;
            for_each_cluster(text, |cluster, _range| {
                total += estimate_cluster_advance(cluster, size, scale);
            });
            total
        }
    };
    // `letter_spacing` is the gap *between* clusters, so `n` clusters pay `n - 1` gaps. The
    // renderer counts them the same way and for the same reason: counting them after the
    // last cluster would make a centred label sit left of centre.
    let tracking = font.letter_spacing() * scale;
    if tracking != 0.0 && clusters > 1 {
        advance += tracking * (clusters - 1) as f32;
    }
    advance.round().max(0.0) as u32
}

/// The line box height `font` occupies at `scale`.
///
/// The font's **effective** line height — an explicit [`Font::line_height`] when one is set,
/// otherwise the point size — which is what the surface and the SVG backend both derive
/// their `TextMetrics::height` from. A control that sized itself from `size()` alone would
/// disagree with every `text_line` it draws.
pub fn estimate_line_height(font: &crate::core::Font, scale: f32) -> u32 {
    (font.effective_line_height().max(1.0) * scale).round().max(1.0) as u32
}

/// `t` of the way from `from` to `to`, for a geometry quantity.
///
/// # Why the crate interpolates in these three shapes and not with a generic `Lerp`
///
/// A control that animates between two layouts needs the *same* fraction applied to a height
/// (`u32`), a coordinate (`i32`) and a point size (`f32`). This crate's `PropertyDriver` is a
/// `f32` progress value, so every consumer already holds `t`; what it lacked was one agreed
/// spelling for "apply `t` to this quantity", and each control had been writing its own closure
/// for it (differently rounded, and differently clamped). These three are that spelling.
///
/// `t` is clamped to `0.0..=1.0` and the ends are **exact**: `t == 0.0` returns `from` and
/// `t == 1.0` returns `to` without a round trip through floating point. That is what lets a
/// settled control be byte-identical to the un-animated one, which is the safety rope the
/// animation snapshots rely on.
pub fn lerp_u32(from: u32, to: u32, t: f32) -> u32 {
    lerp_f32(from as f32, to as f32, t).round().max(0.0) as u32
}

/// [`lerp_u32`] for a signed coordinate (a `Rect`'s `x`/`y`).
pub fn lerp_i32(from: i32, to: i32, t: f32) -> i32 {
    lerp_f32(from as f32, to as f32, t).round() as i32
}

/// [`lerp_u32`] for a continuous quantity such as a font point size.
///
/// The `t == 0.0`/`t == 1.0` ends are returned verbatim rather than through the arithmetic, so
/// a point size of `34.0` is exactly `34.0` at the end of a collapse and not `33.999998`.
pub fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    from + (to - from) * t
}

/// Content-driven sizing for a single control.
///
/// Both functions are pure, and the width one walks clusters through one reused buffer, so
/// calling them from a hot layout path costs no per-cluster allocation.
pub struct ControlMetrics;

/// The implicit size of a [`Switch`](crate::widget::display_widgets::switch::Switch).
///
/// # Why this is in the shared table and not the control
///
/// A switch reports its *floor* rather than its drawn track: a control that can show a focus ring
/// needs room for the ring's inset on both sides, or `size_hint` describes a control whose focus
/// state cannot be rendered. The floor is therefore the track widened by one
/// [`FOCUS_RING_WIDTH`] on each side, at the track's own height — the relation the control used
/// to restate as `track.width + FOCUS_RING_WIDTH * 2` inline. It lives here so the geometry the
/// control carries and the answer its hint reports come from one derivation (rule #101).
pub fn switch_hint_size(track: crate::core::Size, focus_ring_width: u32) -> crate::core::Size {
    crate::core::Size::new(
        track.width.saturating_add(focus_ring_width.saturating_mul(2)),
        track.height,
    )
}

impl ControlMetrics {
    /// Intrinsic size = `max(floor, content + padding)`, component-wise.
    ///
    /// This is the whole point of the shared implicit-size formula: the **floor is a
    /// minimum tappable area**, so small content does not shrink the control below
    /// what a finger can address, while large content still grows it past the
    /// floor. A control whose content is tiny and whose floor is `64x40` is
    /// `64x40`; a control whose content plus padding exceeds the floor is exactly
    /// that much.
    ///
    /// `padding` is applied to both sides of an axis, so a uniform horizontal
    /// padding of 12 adds 24 to the width.
    pub fn implicit_size(content: Size, padding: EdgeOffsets, floor: Size) -> Size {
        let padded_width = content.width.saturating_add(padding.horizontal_total());
        let padded_height = content.height.saturating_add(padding.vertical_total());
        Size::new(padded_width.max(floor.width), padded_height.max(floor.height))
    }

    /// The box left for content once `padding` is removed from `rect`.
    ///
    /// The width available to content after insets.
    ///
    /// The result is never negative: padding larger than the rectangle collapses
    /// the content box to zero rather than inverting it, because a negative extent
    /// is a drawing instruction that would paint outside the control.
    pub fn content_box(rect: Rect, padding: EdgeOffsets) -> Rect {
        Rect::new(
            rect.x.saturating_add(padding.left as i32),
            rect.y.saturating_add(padding.top as i32),
            rect.width.saturating_sub(padding.horizontal_total()),
            rect.height.saturating_sub(padding.vertical_total()),
        )
    }

    /// Centres a `floor`-sized box inside `rect`, clamped to fit.
    ///
    /// A control that owns a fixed-size piece of chrome (a 52x32 switch track, an
    /// 18x18 checkbox indicator) draws that chrome centred in its available area
    /// rather than stretched across it. When the available area is smaller than the
    /// chrome the result is clamped, never expanded: nothing clips a widget at this
    /// layer, so painting outside the rectangle would be a layout violation rather
    /// than a graceful degradation.
    pub fn center_in(rect: Rect, floor: Size) -> Rect {
        let width = floor.width.min(rect.width);
        let height = floor.height.min(rect.height);
        Rect::new(
            rect.x + (rect.width.saturating_sub(width) / 2) as i32,
            rect.y + (rect.height.saturating_sub(height) / 2) as i32,
            width,
            height,
        )
    }

    /// The box a fixed-size leading affordance occupies, vertically centred on `line_y`'s row.
    ///
    /// # Why the box is not clamped to the row's height
    ///
    /// A checkbox indicator is 18x18 whatever the line is: its height is part of the control's
    /// identity, not a fraction of the text beside it. Clamping to the line box produced an
    /// **18x14** indicator on a 14 px font — a rectangle pretending to be a square — which is
    /// visible in the snapshot as a squashed box. The row's height decides *where* the box sits;
    /// the box's own size decides how big it is. Only the control's own rectangle can clamp it.
    pub fn leading_box(rect: Rect, size: Size, padding: EdgeOffsets) -> Rect {
        let width = size.width.min(rect.width);
        let height = size.height;
        // Centre on `rect`'s own row, which the caller has already sized to the line.
        let y = rect.y + (rect.height.saturating_sub(height) / 2) as i32;
        let x = rect.x.saturating_add(padding.left as i32);
        // Keep the box inside the rectangle when the left padding alone would push it out; a
        // leading affordance that is not visible is not an affordance.
        let x = x.min(rect.x + rect.width.saturating_sub(width) as i32);
        Rect::new(x, y, width, height)
    }

    /// The square box a fixed-diameter disc occupies, centred horizontally in `rect`.
    ///
    /// # Why the diameter is not clamped to the row
    ///
    /// A radio's ring is a fixed size for the same reason a checkbox's box is: it is chrome the
    /// control owns. Clamping it to the label's line height made the ring smaller than the
    /// checkbox box it sits beside in the same form.
    pub fn centered_disc(rect: Rect, diameter: u32) -> Rect {
        // Only the rectangle's own extent can clamp a disc: a diameter larger than the control
        // would paint outside it, and nothing clips a widget at this layer.
        let diameter = diameter.min(rect.width).min(rect.height);
        Rect::new(
            rect.x + (rect.width.saturating_sub(diameter) / 2) as i32,
            rect.y + (rect.height.saturating_sub(diameter) / 2) as i32,
            diameter,
            diameter,
        )
    }

    /// A band of `height` centred vertically in `rect`.
    ///
    /// Progress bars, sliders, dividers and scrollbar tracks are all "a line of
    /// this thickness across the middle of my area". Centring here means the band
    /// does not depend on the container's height, which is what stops a 240x120
    /// census cell from turning a 4 px bar into a 120 px slab.
    pub fn centered_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(
            rect.x,
            rect.y + (rect.height.saturating_sub(height) / 2) as i32,
            rect.width,
            height,
        )
    }

    /// The drawn box for a control whose chrome is a single horizontal band.
    ///
    /// A field, a toolbar or a row is *supposed* to span its width and take a fixed
    /// height; only the height is a control fact. `size_hint` reports that height when
    /// the width is not content-driven, so this keeps the full width and centres the
    /// band vertically — which is exactly what stops a 48 px field from drawing as a
    /// 120 px slab.
    pub fn full_width_band(rect: Rect, height: u32) -> Rect {
        Self::centered_band(rect, height)
    }

    /// A square box of `size` centred in `rect`.
    pub fn centered_square(rect: Rect, size: u32) -> Rect {
        Self::center_in(rect, Size::new(size, size))
    }

    /// A full-width band of `height` pinned to the **top** of `rect`.
    ///
    /// # Why this is not [`Self::full_width_band`]
    ///
    /// A tab strip, a navigation bar, a menu-bar entry row and a page header are not
    /// centered chrome: they are pinned to one edge by contract, because whatever the
    /// layout handed them, the content they label begins at their trailing edge. A nav
    /// bar centred in a 120 px census cell would put its titles halfway down the control
    /// with a strip of nothing above them, and `content_rect` — which starts at
    /// `band.bottom()` — would place the page *over* the bar.
    ///
    /// The band never leaves `rect`: when `height` exceeds the rectangle the result is
    /// clamped to it, because nothing clips a widget at this layer and painting outside
    /// the given area is a layout violation rather than a graceful degradation.
    pub fn top_band(rect: Rect, height: u32) -> Rect {
        Rect::new(rect.x, rect.y, rect.width, height.min(rect.height))
    }

    /// A full-width band of `height` pinned to the **bottom** of `rect`.
    ///
    /// The mirror of [`Self::top_band`], for the strip a layout pins to the bottom edge
    /// (a status bar, a bottom tab strip). `content_rect` then ends at `band.y`.
    pub fn bottom_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(rect.x, rect.y + rect.height.saturating_sub(height) as i32, rect.width, height)
    }

    /// The region inside `band` once `inset` is removed from each of the band's own edges.
    ///
    /// # Why an inset is measured from the band and not from the control's rectangle
    ///
    /// A tab's fill, a toolbar item's hover square and a page header's label are all "my
    /// band, minus its own edging". Deriving that from the *control's* rectangle is what
    /// made a 24 px `TabWidget` tab sit at y 96 in a 120 px cell: the strip was placed
    /// correctly and then re-anchored to the control's bottom edge, moving every tab away
    /// from the strip it belongs to. Taking the band as the input makes that impossible to
    /// express.
    ///
    /// `inset` is applied to all four edges, so an inset of 2 removes 4 from each axis.
    /// The result is never inverted.
    pub fn band_inset(band: Rect, inset: u32) -> Rect {
        Rect::new(
            band.x.saturating_add(inset as i32),
            band.y.saturating_add(inset as i32),
            band.width.saturating_sub(2 * inset),
            band.height.saturating_sub(2 * inset),
        )
    }

    /// The rectangle a bottom-pinned strip of `height` leaves for content above it.
    ///
    /// The companion to [`Self::bottom_band`]: both are derived from the same `height`, so
    /// the strip and the content it pushed up cannot overlap or leave a gap between them.
    pub fn content_above_bottom_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(height))
    }

    /// The rectangle a top-pinned strip of `height` leaves for content below it.
    ///
    /// The companion to [`Self::top_band`]. Returning `rect` unchanged when the strip is
    /// taller than the rectangle is deliberate: the alternative, a negative extent, is a
    /// drawing instruction that would paint outside the control.
    pub fn content_below_top_band(rect: Rect, height: u32) -> Rect {
        let height = height.min(rect.height);
        Rect::new(
            rect.x,
            rect.y.saturating_add(height as i32),
            rect.width,
            rect.height.saturating_sub(height),
        )
    }

    /// The box a **dialog or panel** actually paints: at most `intrinsic` in each axis,
    /// centred in `rect`, and never zero in either axis.
    ///
    /// # Why a panel does not fill its rectangle
    ///
    /// A dialog is handed a rectangle by whatever placed it (a census cell, a layout slot),
    /// and that rectangle is its **available area**, not a drawing instruction. SwiftUI's
    /// `.alert` has an intrinsic size and is centred
    /// in the room it is offered; a dialog that stretches to a 240x120 census cell draws
    /// a frame shaped like a dialog rather than a dialog. This is the same rule
    /// [`ControlMetrics::center_in`] applies to a fixed piece of chrome, with one addition that matters for
    /// a panel: the result is clamped *up* to one pixel, because a zero-extent rect is an
    /// **invisible** element, and a dialog that paints nothing is a defect rather than a
    /// tight fit.
    ///
    /// # Why the height is not forced to the floor
    ///
    /// Unlike a button's tappable floor, a dialog has no minimum size of its own: a panel
    /// that ignores the caller's height would paint outside the area it was given, since
    /// nothing clips a widget at this layer. `intrinsic` is therefore a *cap*, not a floor
    /// in the vertical axis — `DIALOG_MIN_WIDTH` is the one dimension a dialog does claim,
    /// and it is clamped to `rect` like every other size here.
    ///
    /// The one-pixel floor is applied **after** the centring, not before it: `center_in`
    /// clamps its result to `rect`, so a caller that handed this a zero-extent rectangle
    /// would have had the floor clamped back to zero. A panel that is one pixel on screen
    /// is visible; one that is zero pixels is the defect this exists to prevent.
    pub fn painted_box(rect: Rect, intrinsic: Size) -> Rect {
        let boxed = Self::center_in(rect, intrinsic);
        Rect::new(boxed.x, boxed.y, boxed.width.max(1), boxed.height.max(1))
    }

    /// The rectangle a focus ring occupies for a control of `rect`.
    ///
    /// # Why the ring is drawn *inside* the control's rectangle
    ///
    /// The drawn frame surrounds the control's background, growing outward by the
    /// padding.
    /// This crate does not clip a child to its layout slot, so a ring drawn outside
    /// the rectangle would overlap whatever the layout placed next to the control —
    /// and on a toolbar, where controls sit `spacing` px apart, that overlap would be
    /// visible. Insetting by [`FOCUS_RING_WIDTH`] keeps the ring within the area the
    /// control was given.
    pub fn focus_ring_rect(rect: Rect) -> Rect {
        let inset = FOCUS_RING_WIDTH as i32;
        Rect::new(
            rect.x.saturating_add(inset),
            rect.y.saturating_add(inset),
            rect.width.saturating_sub(2 * FOCUS_RING_WIDTH),
            rect.height.saturating_sub(2 * FOCUS_RING_WIDTH),
        )
    }

    /// The corner radius a focus ring should follow for a control drawn with `radius`.
    ///
    /// The ring sits inside the control's edge, so its corners must be correspondingly
    /// tighter; reusing the control's own radius would make the ring bulge past the
    /// corners it is supposed to follow. The width is subtracted once rather than
    /// scaled, so a square control (radius 0) keeps square corners and a stadium
    /// stays a stadium.
    pub fn focus_ring_radius(radius: u32) -> u32 {
        radius.saturating_sub(FOCUS_RING_WIDTH)
    }
}

/// Thickness of a keyboard focus ring: 2 logical px.
///
/// Two is the smallest width that reads as deliberate rather than as a rendering
/// artefact on a 1x display, and it is what the reference compact style uses for its focus
/// frame. Named here rather than at the draw site so every control that draws a ring
/// agrees on how thick it is.
pub const FOCUS_RING_WIDTH: u32 = 2;

/// Builds the focus ring geometry for `rect` drawn with corner radius `radius`.
///
/// # Why a constructor rather than two calls at each draw site
///
/// Every control that shows keyboard focus needs the *same pair* — an inset rectangle
/// and a correspondingly tighter radius — and getting one of them wrong is invisible in
/// isolation (the ring simply hugs the corner slightly wrong). `ControlMetrics` owns the
/// geometry; this struct is what a draw site passes around so it cannot take one without
/// the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    /// The rectangle the ring's stroke follows.
    pub rect: Rect,
    /// The corner radius that rectangle should be drawn with.
    pub radius: u32,
}

impl FocusRing {
    /// The ring for a control occupying `rect` with corner radius `radius`.
    pub fn for_control(rect: Rect, radius: u32) -> Self {
        Self {
            rect: ControlMetrics::focus_ring_rect(rect),
            radius: ControlMetrics::focus_ring_radius(radius),
        }
    }

    /// Whether the ring has any area to paint.
    ///
    /// A control too small to hold two ring widths cannot show focus this way; the
    /// caller skips the stroke rather than painting a zero-sized one.
    pub fn is_drawable(&self) -> bool {
        self.rect.width > 0 && self.rect.height > 0
    }
}

/// The colour a focus ring takes.
///
/// # Why this is not the control's border colour
///
/// A ring must announce itself. Reading the same token a border reads is what made a
/// focused control indistinguishable from a merely bordered one (BLUE21 §7 B). This
/// function therefore takes the control's own colours and returns the contrast colour of
/// `fallback`'s surface, which is legible by construction on both appearances.
///
/// # Why there is no theme rung
///
/// The BLUE22 design called for a theme-provided `outline` token and this function used to
/// read `theme.colors.outline`. That field does not exist on [`Colors`], so the module never
/// compiled and the ring was unreachable code. Deriving the ring from the control's own ink
/// is the mechanism the crate actually has; a named `outline` token can be added to
/// [`Colors`] later, at which point this is the single site that reads it.
///
/// Takes the colour rather than a theme handle so this module stays free of the theme
/// layer and so a control with no theme can still ask.
pub fn focus_ring_color(fallback: crate::core::Color) -> crate::core::Color {
    fallback.contrast_color()
}

/// Device-independent control dimensions.
///
/// # Why these are constants and not per-control literals
///
/// Every value below is referenced by more than one control or by more than one
/// draw site within a control (a track's height and its corner radius, a
/// checkbox's box and its inset). Spreading them as literals meant the same fact
/// was derived in several places and could drift — the failure mode rule #101
/// names. The numbers themselves follow the shared design-system table,
/// which agrees closely across sources; where the numbers differ the comment records both.
pub mod dimensions {
    use crate::core::Size;

    /// The smallest area a finger can reliably address on a touch device.
    ///
    /// Material's `kMinInteractiveDimension`.
    pub const TOUCH_TARGET_MIN: u32 = 48;

    /// A push button's minimum size: `64x40`.
    ///
    /// Material M3's text-button sizing. A roomier `100x40` floor is the alternative;
    /// M3's is the tighter of the two and is what a
    /// desktop form wants.
    pub const BUTTON_MIN: Size = Size { width: 64, height: 40 };

    /// Horizontal padding of a push button's label: 12.
    pub const BUTTON_PADDING_H: u32 = 12;

    /// Vertical padding of a push button's label: 8.
    pub const BUTTON_PADDING_V: u32 = 8;

    /// Gap between a button's icon and its label: 6.
    pub const BUTTON_ICON_SPACING: u32 = 6;

    /// A button's icon box: 18 (Material M3's text-button sizing). A 24 px icon box is the alternative.
    pub const BUTTON_ICON_SIZE: u32 = 18;

    /// Corner radius of a rectangular button: 4 (Material M2's text-button sizing).
    pub const BUTTON_RADIUS: u32 = 4;

    /// A switch's track: `52x32`.
    ///
    /// Material M3's switch sizing. A `56x28` track is the alternative; M3's
    /// proportion (a 14 px thumb radius in a 32 px track) is the shape this crate
    /// draws.
    pub const SWITCH_TRACK: Size = Size { width: 52, height: 32 };

    /// A switch thumb's radius: 14, i.e. a 28 px disc in a 32 px track.
    pub const SWITCH_THUMB_RADIUS: u32 = 14;

    /// Inset of the switch thumb from the track edge: 2.
    pub const SWITCH_THUMB_INSET: u32 = 2;

    /// An iOS (Cupertino) switch's track: `51x31`.
    ///
    /// Apple's `UISwitch` measures `51x31` at 1x, which is the geometry
    /// `cupertino_switch` draws. It is deliberately *narrower* than
    /// [`SWITCH_TRACK`](Self::SWITCH_TRACK) rather than a scaled copy of it: a
    /// Material switch and an iOS switch are two different shapes, and rendering
    /// the iOS name with the Material proportions is what made `cupertino_switch`
    /// a second spelling of `switch` rather than a control of its own.
    pub const CUPERTINO_SWITCH_TRACK: Size = Size { width: 51, height: 31 };

    /// An iOS switch thumb's radius: 13, i.e. a 26 px disc in a 31 px track.
    ///
    /// `UISwitch`'s thumb is a 27 pt disc; a 26 px one keeps the track's own
    /// `2` px inset arithmetic ([`SWITCH_THUMB_INSET`](Self::SWITCH_THUMB_INSET))
    /// exact at integer pixels, which is what a bitmap renderer needs.
    pub const CUPERTINO_SWITCH_THUMB_RADIUS: u32 = 13;

    /// How far an iOS switch thumb stretches sideways while it is held: 7.
    ///
    /// `UISwitch` widens its thumb into a capsule on touch-down so the user can
    /// see the control has taken the gesture. Expressed as a horizontal *delta*
    /// the thumb's radius grows by, so the thumb becomes `26 + 7` px wide.
    pub const CUPERTINO_SWITCH_PRESS_STRETCH: u32 = 7;

    /// A collapsible pane's header bar height: 44.
    ///
    /// Material's minimum touch-target height, which is what an expander's own bar has to be
    /// to be tappable on a phone. It was `24` — a desktop-strip height that makes the one
    /// control the user touches hardest to hit, and half the 44/48 that
    /// Material's `ExpansionTile` and `ListTile` both use.
    pub const COLLAPSIBLE_HEADER_HEIGHT: u32 = 44;

    /// A checkbox indicator's box: `18x18` (Material's checkbox sizing).
    pub const CHECKBOX_BOX: u32 = 18;

    /// A checkbox indicator's corner radius: 2.
    pub const CHECKBOX_RADIUS: u32 = 2;

    /// A checkbox tick / border stroke: 2.
    pub const CHECKBOX_STROKE: u32 = 2;

    /// A radio button's outer radius: 8 (Material's radio sizing).
    ///
    /// An outer *diameter* of 16, matching the 18 px checkbox box closely enough
    /// that the two indicators read as a set.
    pub const RADIO_OUTER_RADIUS: u32 = 8;

    /// A radio button's inner dot radius: `4.5`, rounded to 5 — the same inner/outer
    /// ratio as the shared table.
    pub const RADIO_DOT_RADIUS: u32 = 5;

    /// A radio ring's stroke width: 2, matching [`CHECKBOX_STROKE`].
    pub const RADIO_STROKE: u32 = 2;

    /// The gap between an indicator and its label: 6.
    ///
    /// This is the spacing of the shared table, and it means exactly
    /// this — indicator to text, never sibling to sibling (rule: `spacing` is not
    /// a sibling layout parameter).
    pub const INDICATOR_TEXT_SPACING: u32 = 6;

    /// A progress bar's height: 4 (Material M3's progress-indicator sizing).
    pub const PROGRESS_HEIGHT: u32 = 4;

    /// A progress bar's corner radius: 2, i.e. fully rounded at 4 px thick.
    pub const PROGRESS_RADIUS: u32 = 2;

    /// A circular progress indicator's stroke width: 4.
    pub const PROGRESS_CIRCLE_STROKE: u32 = 4;

    /// A spinner's diameter: 48 — the size its own `size_hint` reports, so the two
    /// cannot describe different controls.
    ///
    /// A spinner is a **fixed-size indicator**, not a fraction of the area it is
    /// given. Deriving the diameter from `rect` (`min(w, h) / 2 * size_ratio`) drew a
    /// 90 px ring in the 240x120 census cell and a 36 px one in a 48 px row — the same
    /// control at two sizes, and the census image showed a circle filling most of the
    /// cell. Material's `CircularProgressIndicator` is 48 at its `medium` size, and
    /// that is also [`TOUCH_TARGET_MIN`], so an idle spinner is exactly one tap target.
    pub const SPINNER_DIAMETER: u32 = 48;

    /// The height of the star row a `rating` control draws: one 24 px star.
    ///
    /// A rating is a **row of fixed-size glyphs**, not a panel: the height is the
    /// star cell's own, so a 240x120 census cell gets the same row a 24 px list row
    /// gets instead of a 120 px-tall fill with five glyphs floating in the middle of
    /// it. It is the star size [`RATING_STAR_SIZE`] because a star's line box is the
    /// cell it needs.
    pub const RATING_ROW_HEIGHT: u32 = 24;

    /// The size of one star cell in a `rating` control: 24.
    ///
    /// The cell every star's advance and its [`RATING_STAR_GAP`] are measured from,
    /// so the stars are the same distance apart in any rectangle. Spreading them
    /// around the control's own centre made the pitch a function of the caller's
    /// width rather than of the star.
    pub const RATING_STAR_SIZE: u32 = 24;

    /// The gap between two star cells in a `rating` control: 4.
    pub const RATING_STAR_GAP: u32 = 4;

    /// A slider track's height: 4 (rounded from M2's 2 so the thumb reads as
    /// sitting *on* the track rather than floating above it).
    pub const SLIDER_TRACK_HEIGHT: u32 = 4;

    /// A slider track's corner radius: 2.
    pub const SLIDER_TRACK_RADIUS: u32 = 2;

    /// A slider thumb's radius: 10 (Material's slider thumb sizing), i.e. a
    /// 20 px diameter thumb on a 4 px track.
    pub const SLIDER_THUMB_RADIUS: u32 = 10;

    /// A text field's content height floor: 48.
    ///
    /// The same touch-sized value as [`TOUCH_TARGET_MIN`]; a 40 px background is the
    /// alternative. A field is a tap target, so the
    /// touch-sized value wins.
    pub const TEXT_FIELD_MIN_HEIGHT: u32 = 48;

    /// A text field's horizontal content padding: 12.
    pub const TEXT_FIELD_PADDING_H: u32 = 12;

    /// A card's corner radius: 12 (Material's card sizing).
    pub const CARD_RADIUS: u32 = 12;

    /// A dialog's corner radius: 28 (Material M3's dialog sizing).
    pub const DIALOG_RADIUS: u32 = 28;

    /// A dialog's minimum width: 280 (Material M3's dialog sizing).
    pub const DIALOG_MIN_WIDTH: u32 = 280;

    /// A dialog's content padding: 12 (the shared table's dialog sizing).
    pub const DIALOG_PADDING: u32 = 12;

    /// The strip a dialog draws across its top to carry its title: 28.
    ///
    /// Eight dialog controls (`dialog`, `message_box`, `file_dialog`, `input_dialog`,
    /// `font_dialog`, `color_dialog`, `progress_dialog`, `find_replace_dialog`) each
    /// declared their own `TITLE_BAR_HEIGHT`/`28`, which is how the same visual object
    /// acquired several values (rule #101). It is the height that makes a 14 px title fit
    /// with [`DIALOG_PADDING`] above and below it.
    pub const DIALOG_TITLE_BAR_HEIGHT: u32 = 28;

    /// The height of a dialog's action-button row: 28.
    ///
    /// Shared with [`DIALOG_TITLE_BAR_HEIGHT`]'s reasoning: the OK/Cancel pair appears in
    /// every dialog above, and each spelled the button height itself.
    pub const DIALOG_BUTTON_HEIGHT: u32 = 28;

    /// A dialog's intrinsic height: 240 — the same as [`super::super::WidgetKind`]'s
    /// `Dialog`/`MessageBox` size hint family, and the cap a panel is centred at when it is
    /// given a taller area.
    pub const DIALOG_MIN_HEIGHT: u32 = 240;

    /// A toolbar's height: 56 (Material M3's toolbar sizing).
    pub const TOOLBAR_HEIGHT: u32 = 56;

    /// A toolbar's inter-item spacing: 6.
    pub const TOOLBAR_SPACING: u32 = 6;

    /// A toolbar item's edging inset from the bar's own edges: 2 on each side.
    ///
    /// The item band is `TOOLBAR_HEIGHT - 2 * TOOLBAR_ITEM_INSET` tall, which is what
    /// keeps a hover/checked fill from touching the strip's border. Named rather than
    /// repeated as `+ 2` / `- 4` at the three places that place an item (the draw path
    /// and the hit test both derive from `ToolBar::item_rect`).
    pub const TOOLBAR_ITEM_INSET: u32 = 2;

    /// A menu bar's height: 28 at 100% scale; a Material M3 toolbar is 56, which is the
    /// *app bar*, not a menu bar's own `File Edit View`
    /// strip.
    ///
    /// One fact for both ends of a menu bar: the height it paints its band at, and the
    /// height a layout is told it wants. They were `28` in `size_hint` and `rect.height`
    /// in `draw`, so a 240x120 census cell drew a 120 px menu bar.
    pub const MENU_BAR_HEIGHT: u32 = 28;

    /// A status bar's height: 24 (the default status-bar band).
    ///
    /// The band `status_bar` paints and the height its `size_hint` reports, so the two
    /// cannot disagree about how thick the strip at the bottom of a window is.
    pub const STATUS_BAR_HEIGHT: u32 = 24;

    /// A tab's height in a tab strip (tab bar, tab widget, tab view): 24.
    ///
    /// One fact for five files. `tab_widget`, `tab_bar` and `tab_view` each carried their
    /// own `TAB_HEIGHT`/`40`/`24` literal, which is how the three tab controls came to draw
    /// three different tab heights for the same visual object (rule #101).
    pub const TAB_HEIGHT: u32 = 24;

    /// The narrowest a tab may become: 40 (`tabwidget`'s `MIN_TAB_WIDTH`).
    pub const TAB_MIN_WIDTH: u32 = 40;

    /// The widest a tab's *measured* width may become: 200 (its `MAX_TAB_WIDTH`).
    pub const TAB_MAX_WIDTH: u32 = 200;

    /// The gap between adjacent tabs: 2 (its `TAB_SPACING`).
    pub const TAB_SPACING: u32 = 2;

    /// The horizontal space a tab reserves for its own label: 24.
    ///
    /// `tabwidget`'s `TAB_TEXT_PADDING`: the label's advance is added to this before the
    /// result is clamped to `[TAB_MIN_WIDTH, TAB_MAX_WIDTH]`.
    pub const TAB_TEXT_PADDING: u32 = 24;

    /// A single-line page-navigation bar's height: 32 (Material's `Pagination` row).
    pub const PAGINATION_HEIGHT: u32 = 32;

    /// An app bar's height: 56 (Material M3's app-bar sizing, the same idea as
    /// [`TOOLBAR_HEIGHT`] — Material treats the app bar and the toolbar as one object).
    pub const APP_BAR_HEIGHT: u32 = 56;

    /// A bottom navigation bar's height: 56 (Material M3's navigation-bar sizing).
    pub const BOTTOM_NAV_HEIGHT: u32 = 56;

    /// A splitter handle's thickness: 5 (the shared table's default handle width at
    /// 100% scale).
    ///
    /// It was `5` in `draw` and a second `HANDLE_WIDTH: f32 = 5.0` in `begin_handle_drag`,
    /// so the band the user sees and the band the pointer must hit could drift apart.
    pub const SPLITTER_HANDLE_THICKNESS: u32 = 5;

    /// The strip a menu draws above its popup body to carry its title: 20.
    ///
    /// A closed menu still paints this strip, which is what makes the control visible at
    /// rest (see `Menu::draw`).
    pub const MENU_HEADING_HEIGHT: u32 = 20;

    /// A menu popup's body padding, above the first entry row and below the last: 2.
    pub const MENU_POPUP_PADDING: u32 = 2;

    /// A menu separator's row height: 6 (the rule sits on the row's centre line, so the
    /// space above and below it is `(MENU_SEPARATOR_HEIGHT - DIVIDER_THICKNESS) / 2`).
    pub const MENU_SEPARATOR_HEIGHT: u32 = 6;

    /// A page/tab content header's minimum height: 24.
    ///
    /// `collapsible_pane`'s header and `toolbox`'s tab share this "one line of chrome I can
    /// click" shape.
    pub const PANE_HEADER_HEIGHT: u32 = 24;

    /// A scrollbar's thickness: 8 (the shared table's scrollbar sizing).
    pub const SCROLLBAR_THICKNESS: u32 = 8;

    /// A scrollbar thumb's minimum length: 48. A proportional thumb with no floor
    /// disappears on a very long document.
    pub const SCROLLBAR_MIN_LENGTH: u32 = 48;

    /// A horizontal divider's thickness: 1.
    pub const DIVIDER_THICKNESS: u32 = 1;

    /// The vertical space a divider reserves: 16 (the shared table's divider spacing).
    pub const DIVIDER_SPACING: u32 = 16;

    /// A tooltip's box height: 24 (the shared table's desktop tooltip sizing).
    pub const TOOLTIP_HEIGHT: u32 = 24;

    /// A tooltip's horizontal padding: 8.
    pub const TOOLTIP_PADDING_H: u32 = 8;

    /// A tooltip's vertical padding: 4.
    pub const TOOLTIP_PADDING_V: u32 = 4;

    /// The base font size: 14 (the shared table's base text size).
    pub const FONT_SIZE_BASE: u32 = 14;

    /// The diameter (or side) of a `avatar` control: 40, the size its own `new`
    /// falls back to for a degenerate rectangle and the size its `size_hint`
    /// reports.
    ///
    /// An avatar is a **fixed-size disc**, not a fraction of the area it is given.
    /// Deriving it from `rect` and anchoring it at the rectangle's top-left corner
    /// drew a half-clipped circle in the 240x120 census cell (`circle cx=60 cy=60
    /// r=60`, i.e. running from x 0 to x 120 with the left half of the cell empty).
    /// One fact for the constructor's fallback, the hint a layout reads and the
    /// centred disc the draw path paints, so all three describe the same avatar.
    pub const AVATAR_SIZE: u32 = 40;

    /// A chip's height: 32 (Material M3's chip height).
    ///
    /// A chip is chrome: it is the same height whoever hands it the row, so a 240x120
    /// census cell must not draw a 112 px chip. `chip` and every list that hosts a chip
    /// read this one value.
    pub const CHIP_HEIGHT: u32 = 32;

    /// A chip's horizontal label padding: 8 on each side.
    pub const CHIP_PADDING_H: u32 = 8;

    /// A segmented control's track height: 32.
    ///
    /// One fact for three controls — `segmented_control`, `segmented_button` and
    /// `cupertino_segmented_control` each declared `32` in their own `size_hint`, and each
    /// painted the track at `rect.height` instead, so the drawn pill and the reported size
    /// disagreed on every one of them.
    pub const SEGMENTED_CONTROL_HEIGHT: u32 = 32;

    /// One row of a wheel picker: 32.
    ///
    /// A date picker's drum is a *fixed row* list, not a fraction of its area. Deriving the
    /// row from `rect.height / 5` made a 120 px cell show five 24 px rows and a 300 px panel
    /// five 60 px rows — the same control at two different densities. iOS `UIPickerView`'s
    /// date mode uses 32 pt rows at every container size.
    pub const PICKER_ROW_HEIGHT: u32 = 32;

    /// The number of rows a wheel picker keeps visible: 5 (the selection plus two either
    /// side), which is what `UIPickerView` shows at rest.
    pub const PICKER_VISIBLE_ROWS: u32 = 5;

    /// A Cupertino navigation bar's compact height: 44 (`UINavigationBar`'s standard
    /// height).
    pub const NAV_BAR_HEIGHT: u32 = 44;

    /// A Cupertino navigation bar's large-title height: 96.
    ///
    /// The large-title bar is the compact bar plus the title's own row, which is the shape
    /// `UINavigationBar` adopts when `prefersLargeTitles` is set.
    pub const NAV_BAR_LARGE_HEIGHT: u32 = 96;

    /// A browser URL bar's height: 28 (Chrome's toolbar strip at 100 % zoom).
    pub const WEB_URL_BAR_HEIGHT: u32 = 28;

    /// A breadcrumb trail's height: 28 (one line of 14 px links plus 7 px of air above
    /// and below).
    ///
    /// It matches [`SPLIT_BUTTON_HEIGHT`] because both are "one compact row of chrome".
    pub const BREADCRUMB_HEIGHT: u32 = 28;

    /// A split button's height: 28, the same compact row as [`BREADCRUMB_HEIGHT`].
    pub const SPLIT_BUTTON_HEIGHT: u32 = 28;

    /// The trailing arrow column's width: 22.
    ///
    /// # Why this is a constant and not derived from the glyph
    ///
    /// BLUE22 §B.9 asks a sub-part's box to be derived from its sibling, and this is the one
    /// place in `split_button` where the derivation is **the constant itself**: the column is
    /// the arrow's own width, so the trigger's width is whatever the column leaves. The two
    /// boxes therefore tile the band — a wider column *pushes* the trigger narrower instead of
    /// overlapping it — which is the property that matters. Deriving the number from the 'v'
    /// glyph's advance instead would tie the column to one character's metric in one font, and
    /// a 22 px column holding a 8 px glyph is deliberately roomier than the glyph (a 8 px target
    /// is not a target).
    ///
    /// It is a named constant rather than the field initialiser it used to be so the hit test,
    /// the separator line and the band derivation all read one number (rule #101).
    pub const SPLIT_ARROW_COLUMN_WIDTH: u32 = 22;

    /// Horizontal padding of a split button's label and a menu row's label: 8.
    ///
    /// The shared table's compact button uses `padding: 6` and its menu item uses
    /// `padding: 6` with a `leftPadding` that adds the indicator. 8 is the crate's existing
    /// value for both, kept as a name so the label and the menu rows cannot drift apart — they
    /// were two independent `x + 8` literals.
    pub const SPLIT_BUTTON_PADDING_H: u32 = 8;

    /// A menu row's leading inset: 8, the same compact row padding the shared table uses.
    pub const MENU_ROW_PADDING_H: u32 = 8;

    /// The width a menu row reserves for its shortcut and its submenu arrow: 28.
    ///
    /// One number for both trailing affordances, because they are drawn in one column area: a
    /// row shows at most one of them at a time (a submenu entry's shortcut is not useful), so
    /// two separate reserves would only mean two numbers to keep in step. It is a constant so
    /// the label's box and the arrow's origin read the same value (rule #101).
    pub const MENU_ROW_TRAILING_WIDTH: u32 = 28;

    /// A menu row's height: 22 (one 14 px line plus 4 px of air above and below).
    pub const MENU_ROW_HEIGHT: u32 = 22;

    /// A status bar's horizontal padding: 6.
    ///
    /// The distance from the strip's own edge to its first message, and the distance the size
    /// grip keeps from the strip's corner. One number for both, because they are the same fact:
    /// how far this control's content sits from its own edge. The grip's box and the message's
    /// inset were previously two unrelated literals (`- 14` and `+ 6`/`+ 12`), so the room
    /// reserved for the grip and the room it used could not be kept in agreement.
    pub const STATUS_BAR_PADDING_H: u32 = 6;

    /// A status bar's size grip: 12 x 12, the classic three-diagonal resize affordance.
    ///
    /// Also the answer to "how far from the corner does the grip sit", because the reserve the
    /// permanent message leaves is derived from this box.
    pub const STATUS_GRIP_SIZE: u32 = 12;

    /// A snackbar's height: 48 (Material's single-line snackbar).
    pub const SNACKBAR_HEIGHT: u32 = 48;

    /// A toast's height: 48, the snackbar's own bar height.
    pub const TOAST_HEIGHT: u32 = 48;

    /// A toast's leading accent stripe: 4 wide, running the toast's own height.
    pub const TOAST_ACCENT_WIDTH: u32 = 4;

    /// The label column a timeline or Gantt chart reserves at its left edge: 120.
    ///
    /// One fact for two charts. They carried `120`/`130` and `150`/`160` as four literals —
    /// two per file, one for the text and one for the track — so each chart's track began at
    /// a different x from the label gutter it belonged to, and the two charts disagreed about
    /// how wide a task name column is.
    pub const CHART_LABEL_GUTTER: i32 = 120;

    /// A timeline or Gantt chart's trailing margin after its track: 10.
    pub const CHART_TRACK_MARGIN: i32 = 10;

    /// A video player's transport bar height: 36.
    ///
    /// A fixed overlay strip pinned to the bottom of the video surface, not a fraction of it.
    pub const VIDEO_CONTROL_BAR_HEIGHT: u32 = 36;

    /// A video player's seek bar height: 8, the scrollbar's own thickness.
    pub const VIDEO_SEEK_BAR_HEIGHT: u32 = 8;

    /// The number of rows a `roller` wheel keeps visible at once: 5 (the selection
    /// plus two either side), the same reading as [`PICKER_VISIBLE_ROWS`].
    ///
    /// The roller's `visible_count` is already clamped to 5, so this names the height
    /// the wheel occupies. Sizing the wheel from the control's own rectangle instead
    /// drew a 120 px-tall fill in the census cell — a surface, not a wheel.
    pub const ROLLER_VISIBLE_ROWS: u32 = 5;

    /// The height of one `roller` row: 28.
    ///
    /// A wheel's rows are a **fixed row** list, like a picker's drum: deriving the
    /// row from the visible count and the control's height made the same control show
    /// 24 px rows in a short box and 60 px rows in a tall one. 28 is the row the
    /// control's own default 16 pt face needs — see `Roller::item_height`, which this
    /// is the base of — and it is the same row `emoji_picker` uses at the same size.
    pub const ROLLER_ROW_HEIGHT: u32 = 28;

    /// The height of a `roller`'s wheel: five rows, the [`PICKER_VISIBLE_ROWS`] a
    /// picker's drum shows.
    ///
    /// A wheel's whole extent is therefore 140 px, which is why a wheel in the
    /// 240x120 census cell is clamped to the cell: the control fills the rectangle it
    /// is given while it is smaller than that, and is centred when it is not. Sizing
    /// the wheel from the control's own rectangle instead drew the census cell as a
    /// 120 px-tall surface with a single 24 px selection band in it — a panel, not a
    /// wheel.
    pub const ROLLER_WHEEL_HEIGHT: u32 = ROLLER_VISIBLE_ROWS * ROLLER_ROW_HEIGHT;

    /// The height of a `stepper`'s increment/decrement row: 48.
    ///
    /// The row the +/− buttons and the value live in, so all three are centred on the
    /// control's middle line together. The buttons were painted `rect.height - 2`
    /// tall, which made a 240x120 census cell draw **118 px buttons** — two full-height
    /// slabs with a number between them rather than a stepper.
    pub const STEPPER_ROW_HEIGHT: u32 = TOUCH_TARGET_MIN;

    /// The width of one `stepper` button: 48.
    ///
    /// A square button, so it is the same object in a wide row as in a narrow one.
    /// The old `rect.height.min(rect.width / 3).max(20)` read the *control's* height,
    /// which is why the buttons stretched with the cell.
    pub const STEPPER_BUTTON_WIDTH: u32 = TOUCH_TARGET_MIN;

    /// The padding a `stepper` leaves between its own edge and its row: 2.
    pub const STEPPER_PADDING: u32 = 2;

    /// The width of one `spin_box` step button: 20.
    ///
    /// # Why this is in the shared table and not in the widget
    ///
    /// It is read by three things that must not disagree: the assembled step column's width, the
    /// floor `size_hint` reports, and the arrow's own box. It used to be `SPIN_BOX_BUTTON_WIDTH`
    /// inside `spinbox.rs`, which was fine while the widget owned the arithmetic — but the moment
    /// the value box is "whatever the column leaves", the column's width is the relation rather
    /// than a local detail, and the hint cannot restate it without becoming a second derivation.
    ///
    /// Tiny by desktop standards and deliberately so: this is the value the control's drawing has
    /// always used, and the migration's contract is that the geometry becomes *derivable* without
    /// becoming *different*. [`TOUCH_TARGET_MIN`] (48) would be the tappable floor a finger needs,
    /// but that is a project-wide metric change rather than a geometry fix.
    pub const SPIN_BOX_STEP_BUTTON_WIDTH: u32 = 20;

    /// The number of buttons in a `spin_box`'s step column: 2 (up and down).
    pub const SPIN_BOX_STEP_BUTTONS: u32 = 2;

    /// The height of a `search_box`'s field: 48, the same value [`TEXT_FIELD_MIN_HEIGHT`]
    /// names for every other text entry control.
    ///
    /// A search box is a text field with a magnifier in it, so it must be the same
    /// height as one: this control drew `rect.height` (120 px in the census cell) while
    /// `lineedit` drew a 48 px band, so the two entry controls in the same form were
    /// different objects.
    pub const SEARCH_BOX_FIELD_HEIGHT: u32 = TEXT_FIELD_MIN_HEIGHT;

    /// A `badge` pill's height: 18, the value its own geometry derivation already
    /// clamps to; naming it makes the pill's box and its label's line box come from
    /// one fact.
    pub const BADGE_PILL_HEIGHT: u32 = 18;

    /// A `badge` pill's horizontal label padding: 6 on each side.
    pub const BADGE_PILL_PADDING_H: u32 = 6;

    /// A `badge` pill's vertical label padding: 2 above and below.
    ///
    /// Small on purpose: a badge is a compact marker, so the pill wraps its label
    /// closely. Named rather than written as `* 2` at the draw site because the pill's
    /// height is derived from it, and a bare `2` there is the same constant the old
    /// `padding_y` was.
    pub const BADGE_PILL_PADDING_V: u32 = 2;

    /// The label font size a `badge` draws its count in: 11 (Material's `labelSmall`).
    pub const BADGE_LABEL_FONT_SIZE: u32 = 11;

    /// A `skeleton_loader` placeholder row's height: 20.
    ///
    /// One shimmer line is a fixed-height row, not a fraction of the control — the
    /// `Rect` shape's own `(w, h)` is the caller's datum, but the `TextLine` shape
    /// stacks rows of *this* height, so a three-line placeholder is 76 px tall in any
    /// rectangle.
    pub const SKELETON_ROW_HEIGHT: u32 = 20;

    /// The gap between two `skeleton_loader` placeholder rows: 8.
    pub const SKELETON_ROW_GAP: u32 = 8;

    /// A `color_well`'s swatch size: 60, the sample a colour control exists to show.
    ///
    /// The well is a **fixed-size affordance**, not a panel: sizing the checkerboard
    /// and the swatch from the control's rectangle drew a 240x120 chequerboard whose
    /// swatch covered most of the cell (`color_well.svg` was 1808 rects). The shared
    /// table's colour-well sample is 60 px square at its default density.
    pub const COLOR_WELL_SIZE: u32 = 60;

    /// The number of swatches a `color_history` lays out per row: 5.
    ///
    /// The grid is measured from the swatch's own size, so the panel is
    /// `5 * (20 + 4)` wide in any rectangle rather than as many swatches as the
    /// caller's width happens to fit.
    pub const COLOR_HISTORY_PER_ROW: u32 = 5;

    /// One `color_history` swatch: 20x20.
    pub const COLOR_HISTORY_SWATCH: u32 = 20;

    /// The gap between two `color_history` swatches: 4.
    pub const COLOR_HISTORY_PADDING: u32 = 4;

    /// The height of one `color_history` row: 20, the swatch's own box.
    pub const COLOR_HISTORY_ROW_HEIGHT: u32 = COLOR_HISTORY_SWATCH;

    /// A `refresh_control`'s pull indicator height: 40.
    ///
    /// The reveal panel is a **fixed strip** at the top of the control, so a pull in
    /// a 400 px list and a pull in the census cell open the same 40 px band. It was
    /// `40` when refreshing and `0` otherwise, with the control's own rectangle
    /// stretching beneath it.
    pub const REFRESH_INDICATOR_HEIGHT: u32 = 40;

    /// The step between visual-density levels: 4 logical px per unit.
    pub const DENSITY_STEP: u32 = 4;

    /// The widest viewport still classified as [`crate::view::Breakpoint::Compact`]: 599 px.
    ///
    /// The upper bound is **exclusive** — a viewport 600 px wide is `Medium`, not `Compact` —
    /// which is the convention every breakpoint table uses and the one the tests pin, so a
    /// boundary case has exactly one answer.
    ///
    /// Named here rather than at the classification site (principle #44 / plan §5.1 "thresholds
    /// from dimensions table, not literals") so a theme or a device profile can move the boundary in
    /// one place instead of at each reader.
    pub const BREAKPOINT_COMPACT_MAX: u32 = 599;

    /// The widest viewport still classified as [`crate::view::Breakpoint::Medium`]: 839 px.
    ///
    /// Above this is `Expanded` (desktop). The 600/840 pair is the window the three-tier
    /// classification is universally cut at; what matters here is that it is a *data* fact with
    /// one home, not that those two numbers are special in themselves.
    pub const BREAKPOINT_MEDIUM_MAX: u32 = 839;

    /// A `calendar`'s intrinsic width: 260.
    ///
    /// The month grid's own box rather than a fraction of the area the control is given, so a
    /// calendar dropped into a 240x120 census cell draws the same month as one in a dialog. Named
    /// rather than left as the `260` its `size_hint` carried.
    pub const CALENDAR_DEFAULT_WIDTH: u32 = 260;

    /// A `calendar`'s intrinsic height: 240 (a six-week month of 40 px rows).
    pub const CALENDAR_DEFAULT_HEIGHT: u32 = 240;

    /// A `date_edit`'s intrinsic width: 120 — the field plus its calendar affordance.
    pub const DATE_EDIT_DEFAULT_WIDTH: u32 = 120;

    /// A `date_edit`'s intrinsic height: 28, one compact row of chrome.
    pub const DATE_EDIT_DEFAULT_HEIGHT: u32 = 28;

    /// A `pie_menu`'s intrinsic width: 200 — the box the ring of actions is centred in.
    pub const PIE_MENU_DEFAULT_WIDTH: u32 = 200;

    /// A `pie_menu`'s intrinsic height: 200, square with the width so the ring is a circle.
    pub const PIE_MENU_DEFAULT_HEIGHT: u32 = 200;

    /// A `frame`'s intrinsic width: 200.
    ///
    /// A frame is a container: it has no content of its own to measure, so its hint is the panel
    /// box its `draw` outlines. Named so the box and the claimed size are one fact.
    pub const FRAME_DEFAULT_WIDTH: u32 = 200;

    /// A `frame`'s intrinsic height: 200, square with the width like every other panel box.
    pub const FRAME_DEFAULT_HEIGHT: u32 = 200;

    /// A generic container/panel's intrinsic width: 400 (MDI areas and every view-shaped control).
    pub const PANEL_DEFAULT_WIDTH: u32 = 400;

    /// A generic container/panel's intrinsic height: 300 (a 4:3 desktop box).
    pub const PANEL_DEFAULT_HEIGHT: u32 = 300;

    /// A `collapsible_pane`'s intrinsic width: 200 — the box its header and body live in.
    pub const COLLAPSIBLE_PANE_DEFAULT_WIDTH: u32 = 200;

    /// A `collapsible_pane`'s intrinsic height: 100, room for its header plus a body row.
    pub const COLLAPSIBLE_PANE_DEFAULT_HEIGHT: u32 = 100;

    /// A `dock_widget`'s intrinsic width: 250 — a docked panel's own column.
    pub const DOCK_WIDGET_DEFAULT_WIDTH: u32 = 250;

    /// A `dock_widget`'s intrinsic height: 200, square enough to hold its title bar and content.
    pub const DOCK_WIDGET_DEFAULT_HEIGHT: u32 = 200;

    /// A `masonry_layout`'s intrinsic width: 300 — the box its columns are measured against.
    pub const MASONRY_DEFAULT_WIDTH: u32 = 300;

    /// A `masonry_layout`'s intrinsic height: 300, square with the width.
    pub const MASONRY_DEFAULT_HEIGHT: u32 = 300;

    /// A `scroll_area`'s intrinsic width: 300 — a viewport's own box.
    pub const SCROLL_AREA_DEFAULT_WIDTH: u32 = 300;

    /// A `scroll_area`'s intrinsic height: 200, the viewport a scroll bar is measured against.
    pub const SCROLL_AREA_DEFAULT_HEIGHT: u32 = 200;

    /// A Cupertino bar's intrinsic width: 300 — `UINavigationBar`'s content box.
    pub const CUPERTINO_BAR_DEFAULT_WIDTH: u32 = 300;

    /// A Cupertino bar's intrinsic height: 48; the drawn bar itself is [`NAV_BAR_HEIGHT`].
    ///
    /// The 48 is the round's pre-existing `size_hint` value, and naming it keeps the claimed box
    /// and the painted [`NAV_BAR_HEIGHT`] band separate facts — changing the bar's thickness is a
    /// paint change, not a re-derivation of the hint.
    pub const CUPERTINO_BAR_DEFAULT_HEIGHT: u32 = 48;

    /// A Cupertino panel's intrinsic width: 270 — a compact iOS surface, not the 400 px desktop
    /// panel.
    pub const CUPERTINO_PANEL_DEFAULT_WIDTH: u32 = 270;

    /// A Cupertino panel's intrinsic height: 150, `270x150` being the `3:5` proportion iOS uses.
    pub const CUPERTINO_PANEL_DEFAULT_HEIGHT: u32 = 150;

    /// A Cupertino row's intrinsic width: 200 — one `UITableViewCell` row.
    pub const CUPERTINO_ROW_DEFAULT_WIDTH: u32 = 200;

    /// A Cupertino row's intrinsic height: 28, the same compact chrome row
    /// [`SPLIT_BUTTON_HEIGHT`] names.
    pub const CUPERTINO_ROW_DEFAULT_HEIGHT: u32 = 28;

    /// A Cupertino tab bar's intrinsic width: 72 — one `UITabBarItem`'s square slot.
    pub const CUPERTINO_TAB_BAR_DEFAULT_WIDTH: u32 = 72;

    /// A Cupertino tab bar's intrinsic height: 400 — the strip plus the content above it.
    pub const CUPERTINO_TAB_BAR_DEFAULT_HEIGHT: u32 = 400;

    /// A Cupertino segmented control's intrinsic width: 300.
    ///
    /// The same box as the Material [`SEGMENTED_CONTROL_DEFAULT_WIDTH`]; only the track's drawn
    /// form differs between the two platform names.
    pub const CUPERTINO_SEGMENTED_DEFAULT_WIDTH: u32 = 300;

    /// A `bottom_sheet`'s intrinsic width: 300 — the sheet spans this before it is clamped.
    pub const BOTTOM_SHEET_DEFAULT_WIDTH: u32 = 300;

    /// A `bottom_sheet`'s intrinsic height: 200 — the collapsed sheet's own box.
    pub const BOTTOM_SHEET_DEFAULT_HEIGHT: u32 = 200;

    /// A `dialog` widget's intrinsic width: 320.
    ///
    /// Narrower than [`DIALOG_MIN_WIDTH`]'s 280-driven content because this hint is the *box*
    /// a dialog occupies before its own title bar and buttons are laid inside it, and it is the
    /// value the control's `size_hint` has always reported.
    pub const DIALOG_WIDGET_DEFAULT_WIDTH: u32 = 320;

    /// A `find_replace_dialog`'s intrinsic width: 350 — the find field plus its replace field.
    pub const FIND_REPLACE_DIALOG_DEFAULT_WIDTH: u32 = 350;

    /// A `find_replace_dialog`'s intrinsic height: 200.
    pub const FIND_REPLACE_DIALOG_DEFAULT_HEIGHT: u32 = 200;

    /// A `message_box`'s intrinsic width: 350 — a short message's panel.
    pub const MESSAGE_BOX_DEFAULT_WIDTH: u32 = 350;

    /// A `message_box`'s intrinsic height: 150 — the message plus its action row.
    pub const MESSAGE_BOX_DEFAULT_HEIGHT: u32 = 150;

    /// A `popover`'s intrinsic width: 200 — the arrow plus its content box.
    pub const POPOVER_DEFAULT_WIDTH: u32 = 200;

    /// A `popover`'s intrinsic height: 150.
    pub const POPOVER_DEFAULT_HEIGHT: u32 = 150;

    /// A `tooltip`'s intrinsic width: 100 — the narrowest a tooltip's text is allowed to be.
    pub const TOOLTIP_DEFAULT_WIDTH: u32 = 100;

    /// A `floating_label`'s intrinsic width: 200 — the box the label floats above.
    pub const FLOATING_LABEL_DEFAULT_WIDTH: u32 = 200;

    /// A `floating_label`'s intrinsic height: 40 — the field plus the label row above it.
    pub const FLOATING_LABEL_DEFAULT_HEIGHT: u32 = 40;

    /// A `font_preview`'s intrinsic width: 300 — enough for a pangram at the base size.
    pub const FONT_PREVIEW_DEFAULT_WIDTH: u32 = 300;

    /// A `font_preview`'s intrinsic height: 100 — several lines of sample text.
    pub const FONT_PREVIEW_DEFAULT_HEIGHT: u32 = 100;

    /// An `lcd_number`'s intrinsic width: 80 — a fixed digit window.
    pub const LCD_NUMBER_DEFAULT_WIDTH: u32 = 80;

    /// An `lcd_number`'s intrinsic height: 30 — a seven-segment line box.
    pub const LCD_NUMBER_DEFAULT_HEIGHT: u32 = 30;

    /// A `progress_circle`'s default (and floor) diameter: 60.
    ///
    /// A ring, so its hint is square. The diameter is a *field* rather than a shared constant
    /// because a caller may set it; 60 is the value the control falls back to and the smallest a
    /// legible ring is drawn at.
    pub const PROGRESS_CIRCLE_DEFAULT_DIAMETER: u32 = 60;

    /// A `progressbar` / `slider` bar's long axis: 120.
    ///
    /// One number for two controls that both report "a bar I can drag": the extent along the
    /// direction of travel. Named so the horizontal and vertical arms read one fact rather than
    /// two literals.
    pub const BAR_DEFAULT_EXTENT: u32 = 120;

    /// A `progressbar` / `slider` bar's thickness: 20.
    ///
    /// The short axis, which includes the thumb's [`SLIDER_THUMB_RADIUS`] on both sides of its
    /// [`SLIDER_TRACK_HEIGHT`] track — the row is the thumb, not the line.
    pub const BAR_DEFAULT_THICKNESS: u32 = 20;

    /// A `scrollbar`'s intrinsic long axis: 100.
    ///
    /// Distinct from [`SCROLLBAR_MIN_LENGTH`] (48), which is the floor a *proportional* thumb is
    /// clamped to once the control has been laid out; this is the length the hint claims before
    /// any container gives it one.
    pub const SCROLLBAR_DEFAULT_LENGTH: u32 = 100;

    /// A `skeleton_loader`'s intrinsic width: 300 — a placeholder block's own box.
    pub const SKELETON_DEFAULT_WIDTH: u32 = 300;

    /// A `cascader`'s intrinsic width: 200 — one link of the column chain.
    pub const CASCADER_DEFAULT_WIDTH: u32 = 200;

    /// A `cascader`'s intrinsic height: 32, the [`SEGMENTED_CONTROL_HEIGHT`]-sized row each link
    /// occupies.
    pub const CASCADER_DEFAULT_HEIGHT: u32 = 32;

    /// An `inplace_editor`'s intrinsic width: 200 — the cell the label is edited in.
    pub const INPLACE_EDITOR_DEFAULT_WIDTH: u32 = 200;

    /// An `inplace_editor`'s intrinsic height: 28, one compact row of chrome.
    pub const INPLACE_EDITOR_DEFAULT_HEIGHT: u32 = 28;

    /// A `keyboard`'s intrinsic width: 320 — ten keys across at their own advance.
    pub const KEYBOARD_DEFAULT_WIDTH: u32 = 320;

    /// A `keyboard`'s intrinsic height: 160 — four key rows.
    pub const KEYBOARD_DEFAULT_HEIGHT: u32 = 160;

    /// A `range_slider`'s intrinsic width: 200 — the track plus its two thumbs.
    pub const RANGE_SLIDER_DEFAULT_WIDTH: u32 = 200;

    /// A `range_slider`'s intrinsic height: 28, one row holding a [`SLIDER_THUMB_RADIUS`] thumb
    /// either side of its track.
    pub const RANGE_SLIDER_DEFAULT_HEIGHT: u32 = 28;

    /// A multi-line `text_edit`'s intrinsic width: 200 — the box its lines are laid in.
    pub const TEXT_EDIT_DEFAULT_WIDTH: u32 = 200;

    /// A multi-line `text_edit`'s intrinsic height: 24 — one line's box, which the control grows
    /// from once it has content rather than a fixed number of lines.
    pub const TEXT_EDIT_DEFAULT_HEIGHT: u32 = 24;

    /// An `audio_visualizer`'s intrinsic width: 200 — the time-axis box its bars are drawn in.
    pub const AUDIO_VISUALIZER_DEFAULT_WIDTH: u32 = 200;

    /// An `audio_visualizer`'s intrinsic height: 60 — the amplitude axis the bars span.
    pub const AUDIO_VISUALIZER_DEFAULT_HEIGHT: u32 = 60;

    /// A `tool_button`'s side: 28 (Material M3's icon-button sizing).
    ///
    /// Square, so one number serves both axes: the icon box plus its own padding.
    pub const TOOL_BUTTON_SIZE: u32 = 28;

    /// Horizontal padding between a tool button's frame and its content box: 4.
    ///
    /// Tighter than [`BUTTON_PADDING_H`] because a tool button is compact chrome: its label is
    /// often a single glyph and its icon already carries the button's own margin.
    pub const TOOL_BUTTON_PADDING: u32 = 4;

    /// The square a tool button's icon occupies when its style asks for one: 16.
    ///
    /// # Why this is its own number rather than [`BUTTON_ICON_SIZE`]
    ///
    /// A tool button is a 28 px square; an 18 px icon leaves it 5 px of air per side, which reads
    /// as a push button squeezed into a tool bar. Sixteen keeps the same 6 px margin the compact
    /// row's own controls use, so a tool bar's icon-only buttons look like one family. The value
    /// is named here rather than derived because the derivation would be "28 minus a margin", and
    /// the margin is the thing being chosen.
    pub const TOOL_BUTTON_ICON_SIZE: u32 = 16;

    /// The gap between a tool button's icon and its label when the style puts them on one line: 4.
    pub const TOOL_BUTTON_ICON_SPACING: u32 = 4;

    /// Width reserved at a tool button's trailing edge for its popup indicator arrow: 12.
    ///
    /// The arrow is drawn by the button itself, so the room has to be taken out of the content box
    /// before the icon and the label are laid out in it -- otherwise the label runs under the
    /// glyph. Twelve is the arrow's own advance plus a small gap.
    pub const TOOL_BUTTON_POPUP_ARROW_RESERVE: u32 = 12;

    /// A `bezier_curve_editor`'s intrinsic width: 300.
    ///
    /// The graph's own box rather than a fraction of the area it is given: a curve editor in a
    /// 240x120 census cell must draw the same control points as one in a dialog.
    pub const BEZIER_EDITOR_DEFAULT_WIDTH: u32 = 300;

    /// A `bezier_curve_editor`'s intrinsic height: 200.
    pub const BEZIER_EDITOR_DEFAULT_HEIGHT: u32 = 200;

    /// A `qr_code`'s side: 150 logical px.
    ///
    /// A QR symbol is a **fixed-size matrix**: its module count is a property of the data it
    /// encodes, so scaling the whole symbol to the rectangle would change its scannability. Named
    /// so the hint's square and the drawn matrix are one number.
    pub const QR_CODE_SIZE: u32 = 150;

    /// A `navigation_drawer`'s intrinsic width: 300 — the panel that slides in from the edge.
    pub const NAVIGATION_DRAWER_DEFAULT_WIDTH: u32 = 300;

    /// A `navigation_drawer`'s intrinsic height: 400 — the viewport it covers when open.
    pub const NAVIGATION_DRAWER_DEFAULT_HEIGHT: u32 = 400;

    /// A `refresh_control`'s intrinsic width: 400 — the list it wraps.
    pub const REFRESH_CONTROL_DEFAULT_WIDTH: u32 = 400;

    /// A `refresh_control`'s intrinsic height: 400.
    pub const REFRESH_CONTROL_DEFAULT_HEIGHT: u32 = 400;

    /// A `splash_screen`'s intrinsic width: 480 — the brand panel shown while a host starts.
    pub const SPLASH_SCREEN_DEFAULT_WIDTH: u32 = 480;

    /// A `splash_screen`'s intrinsic height: 320, a `3:2` panel.
    pub const SPLASH_SCREEN_DEFAULT_HEIGHT: u32 = 320;

    /// A `swipe_to_dismiss`'s intrinsic width: 400 — the row that can be swiped away.
    pub const SWIPE_TO_DISMISS_DEFAULT_WIDTH: u32 = 400;

    /// A `swipe_to_dismiss`'s intrinsic height: 60, one tall dismissal row.
    pub const SWIPE_TO_DISMISS_DEFAULT_HEIGHT: u32 = 60;

    /// A `chart`'s intrinsic width: 400 — the plot box plus its label gutter.
    pub const CHART_DEFAULT_WIDTH: u32 = 400;

    /// A `chart`'s intrinsic height: 300, a `4:3` plot.
    pub const CHART_DEFAULT_HEIGHT: u32 = 300;

    /// A `gantt_widget`'s intrinsic width: 600 — the label gutter plus a week of track.
    pub const GANTT_DEFAULT_WIDTH: u32 = 600;

    /// A `gantt_widget`'s intrinsic height: 200 — its task rows.
    pub const GANTT_DEFAULT_HEIGHT: u32 = 200;

    /// The side of one `grid` cell: 20 logical px.
    ///
    /// The grid's hint is `columns * this + spacing`, so the cell size is the unit its whole
    /// measurement is counted in rather than a detail of its paint.
    pub const GRID_CELL_SIZE: u32 = 20;

    /// The `grid`'s own floor: 40 — two cells, since a one-cell grid is not a grid.
    pub const GRID_DEFAULT_FLOOR: u32 = 40;

    /// A `radar_chart`'s own box: 320 square.
    ///
    /// The polygon's radius is derived from it, so the hint and the plot are one fact rather than
    /// two numbers that describe the same square separately.
    pub const RADAR_CHART_DEFAULT_SIZE: u32 = 320;

    /// A `segmented_control`'s intrinsic width: 300 (the track's own height is
    /// [`SEGMENTED_CONTROL_HEIGHT`], shared by all three spellings of the control).
    pub const SEGMENTED_CONTROL_DEFAULT_WIDTH: u32 = 300;

    /// A `terminal_view`'s intrinsic width: 600 — eighty columns of the terminal font.
    pub const TERMINAL_DEFAULT_WIDTH: u32 = 600;

    /// A `terminal_view`'s intrinsic height: 300 — the viewport's own box.
    pub const TERMINAL_DEFAULT_HEIGHT: u32 = 300;

    /// A `data_grid`'s intrinsic width: 400 — its columns at their default share.
    pub const DATA_GRID_DEFAULT_WIDTH: u32 = 400;

    /// A `data_grid`'s intrinsic height: 300 — the rows its viewport shows.
    pub const DATA_GRID_DEFAULT_HEIGHT: u32 = 300;

    /// An `image_gallery`'s intrinsic width: 400 — the strip of thumbnails.
    pub const IMAGE_GALLERY_DEFAULT_WIDTH: u32 = 400;

    /// An `image_gallery`'s intrinsic height: 300.
    pub const IMAGE_GALLERY_DEFAULT_HEIGHT: u32 = 300;

    /// A `list_view`'s intrinsic width: 200 — the rows' own box.
    pub const LIST_VIEW_DEFAULT_WIDTH: u32 = 200;

    /// A `list_view`'s intrinsic height: 200, square with the width.
    pub const LIST_VIEW_DEFAULT_HEIGHT: u32 = 200;

    /// A `table_widget`'s intrinsic width: 400 — the header's columns at their default share.
    pub const TABLE_WIDGET_DEFAULT_WIDTH: u32 = 400;

    /// A `table_widget`'s intrinsic height: 300.
    pub const TABLE_WIDGET_DEFAULT_HEIGHT: u32 = 300;

    /// A `tree_table`'s intrinsic width: 400 — the tree column plus its data columns.
    pub const TREE_TABLE_DEFAULT_WIDTH: u32 = 400;

    /// A `tree_table`'s intrinsic height: 300.
    pub const TREE_TABLE_DEFAULT_HEIGHT: u32 = 300;

    /// A `tree_view`'s intrinsic width: 200 — the nodes' own box.
    pub const TREE_VIEW_DEFAULT_WIDTH: u32 = 200;

    /// A `tree_view`'s intrinsic height: 200, square with the width.
    pub const TREE_VIEW_DEFAULT_HEIGHT: u32 = 200;

    /// A `virtual_list`'s intrinsic width: 200.
    pub const VIRTUAL_LIST_DEFAULT_WIDTH: u32 = 200;

    /// A `virtual_list`'s intrinsic height: 200, square with the width.
    pub const VIRTUAL_LIST_DEFAULT_HEIGHT: u32 = 200;

    /// A `split_button`'s intrinsic width: 100 — the trigger plus its arrow column.
    ///
    /// Its height is [`SPLIT_BUTTON_HEIGHT`], the compact row the control paints, so the hint and
    /// the drawn band are one fact on that axis.
    pub const SPLIT_BUTTON_DEFAULT_WIDTH: u32 = 100;

    /// How opaque the **disabled veil** is, as a byte alpha.
    ///
    /// # Why this is a shared constant and not a per-control literal
    ///
    /// Two controls dim their contents when disabled — `label` and `frame` — and both used to do it
    /// by laying down a fixed mid-grey (`rgba(128,128,128,60)` and `rgba(128,128,128,80)`). A
    /// half-transparent mid-grey has **no direction**: over a light surface it darkens, over a dark
    /// one it lightens, so "disabled" came out as "more contrast" on whichever appearance was
    /// already hardest to read. That is BLUE21 B23's scrim defect in two more places.
    ///
    /// The fix is the same one the scrim needed — step the veil *toward the surface*, which is the
    /// only direction that reads as "receded" on both appearances — and the weight is here rather
    /// than in each file so the two controls recede by the same amount. `140` is the same figure
    /// the preset state overrides use for a disabled fill (`preset_states::DISABLED_FADE`, 0.55),
    /// so a disabled label, a disabled frame and a disabled button all recede alike.
    pub const DISABLED_VEIL_ALPHA: u8 = 140;

    /// Which way a visual-density level shifts metric sizes.
    ///
    /// The density level is a signed offset in [`DENSITY_STEP`] units, and
    /// Material's named levels are `comfortable = -4` and `compact = -8`. Only the
    /// *vertical* metrics shrink at the compact end — the shared table deliberately does
    /// **not** compress horizontal padding,
    /// because a compact desktop button with 0 px of side padding stops reading as
    /// a button.
    pub fn density_scale(vertical_density: i32) -> i32 {
        vertical_density.saturating_mul(DENSITY_STEP as i32)
    }
}

/// The drawn shape of a [`Switch`](crate::widget::display_widgets::switch::Switch)'s track
/// and thumb.
///
/// # Why this is a value and not two separate controls
///
/// A Material switch and an iOS switch are the same *control* — one boolean, one gesture,
/// one `checked` signal, one `travel` animation — drawn at two different sizes. Splitting
/// them into two `Widget` implementations would duplicate the gesture, the animation and
/// the contract to change three numbers (principle #28), and the two copies would then be
/// free to drift on the parts that are *not* about size.
///
/// So the size lives here, one field on the one implementation, and
/// `cupertino_switch` is that implementation carrying this preset instead of the default
/// one. The mechanism is the crate's own: `CodeEditor` carries a `SyntaxPalette`, this
/// carries a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchGeometry {
    /// The track's drawn size, centred inside the control's rectangle.
    pub track: Size,
    /// The thumb's radius at rest.
    pub thumb_radius: u32,
    /// The thumb's inset from the track's edge.
    pub thumb_inset: u32,
    /// How much wider the thumb becomes while it is held, on each side.
    ///
    /// `0` for a shape that does not stretch. iOS's `UISwitch` widens its thumb into a
    /// capsule on touch-down; Material's does not, and that difference is exactly the
    /// kind of thing a single shared implementation must be able to express rather than
    /// ignore.
    pub press_stretch: u32,
}

impl SwitchGeometry {
    /// Material's shape: a `52x32` track with a 14 px thumb radius and no stretch.
    pub const MATERIAL: Self = Self {
        track: dimensions::SWITCH_TRACK,
        thumb_radius: dimensions::SWITCH_THUMB_RADIUS,
        thumb_inset: dimensions::SWITCH_THUMB_INSET,
        press_stretch: 0,
    };

    /// iOS's shape: a `51x31` track, a 13 px thumb radius, and a 7 px press stretch.
    pub const CUPERTINO: Self = Self {
        track: dimensions::CUPERTINO_SWITCH_TRACK,
        thumb_radius: dimensions::CUPERTINO_SWITCH_THUMB_RADIUS,
        thumb_inset: dimensions::SWITCH_THUMB_INSET,
        press_stretch: dimensions::CUPERTINO_SWITCH_PRESS_STRETCH,
    };

    /// The thumb's drawn size at rest, as a diameter.
    pub const fn thumb_size(&self) -> u32 {
        self.thumb_radius * 2
    }
}

impl Default for SwitchGeometry {
    fn default() -> Self {
        Self::MATERIAL
    }
}

#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::style::EdgeOffsets;

    #[test]
    fn small_content_still_gets_the_floor() {
        // 5 px of text in a button: the background floor wins. This is the
        // assertion behind "a control must not shrink below what a finger can
        // address", and it is the regression guard for the 240x120 stadium.
        let size = ControlMetrics::implicit_size(
            Size::new(5, 5),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, dimensions::BUTTON_MIN);
    }

    #[test]
    fn large_content_wins_over_the_floor() {
        // Content + padding exceeds the floor on both axes, so it decides.
        let size = ControlMetrics::implicit_size(
            Size::new(200, 60),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, Size::new(224, 76));
    }

    #[test]
    fn the_two_arms_are_compared_component_wise() {
        // Content loses on width but wins on height: each axis decides alone.
        let size = ControlMetrics::implicit_size(
            Size::new(10, 90),
            EdgeOffsets::symmetric(8, 12),
            dimensions::BUTTON_MIN,
        );
        assert_eq!(size, Size::new(64, 106));
    }

    #[test]
    fn content_box_removes_padding_from_both_sides() {
        let rect = Rect::new(10, 20, 100, 50);
        let content = ControlMetrics::content_box(rect, EdgeOffsets::symmetric(4, 6));
        assert_eq!(content, Rect::new(16, 24, 88, 42));
    }

    #[test]
    fn a_panel_is_capped_at_its_intrinsic_size_and_centred() {
        // The census case: a 240x120 cell offered to a panel whose intrinsic size is
        // 280x240. The width cannot exceed the cell, so the panel is the whole width; the
        // height is capped at the intrinsic 240 (which is larger than the cell) so the
        // panel is the whole height too. Neither axis is stretched beyond what the panel
        // asked for, which is the ``switch``-shaped-rectangle defect inverted.
        let cell = crate::widget::census::CENSUS_RECT;
        let panel = ControlMetrics::painted_box(
            cell,
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        );
        assert_eq!(panel, Rect::new(0, 0, 240, 120));

        // Given *more* room than its intrinsic size, the panel keeps that size and is
        // centred, so the frame reads as a dialog rather than as a wall of fill.
        let roomy = Rect::new(0, 0, 400, 300);
        let centred = ControlMetrics::painted_box(
            roomy,
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        );
        assert_eq!(centred, Rect::new(60, 30, 280, 240));
    }

    #[test]
    fn a_panel_never_collapses_to_zero_extent() {
        // A zero-extent rectangle is an invisible element, which is a defect rather than a
        // tight fit: a panel squeezed to nothing has to keep one pixel of itself visible.
        let degenerate = Rect::new(5, 5, 0, 0);
        let panel = ControlMetrics::painted_box(degenerate, Size::new(280, 240));
        assert_eq!(panel.width, 1);
        assert_eq!(panel.height, 1);
    }

    #[test]
    fn oversized_padding_collapses_the_content_box_instead_of_inverting_it() {
        let rect = Rect::new(0, 0, 10, 10);
        let content = ControlMetrics::content_box(rect, EdgeOffsets::all(50));
        assert_eq!(content.width, 0);
        assert_eq!(content.height, 0);
        assert_eq!(content.x, 50);
        assert_eq!(content.y, 50);
    }

    #[test]
    fn centered_chrome_is_centred_and_clamped_never_expanded() {
        // Roomier than the chrome: centred, chrome keeps its own size.
        let roomy = ControlMetrics::center_in(Rect::new(0, 0, 240, 120), dimensions::SWITCH_TRACK);
        assert_eq!(roomy, Rect::new(94, 44, 52, 32));

        // Smaller than the chrome: clamped to the available area, not expanded.
        let tight = ControlMetrics::center_in(Rect::new(5, 7, 20, 10), dimensions::SWITCH_TRACK);
        assert_eq!(tight, Rect::new(5, 7, 20, 10));
    }

    #[test]
    fn a_disc_stays_square_in_a_non_square_area() {
        let disc = ControlMetrics::centered_disc(Rect::new(0, 0, 100, 20), 18);
        assert_eq!(disc.width, disc.height);
        assert_eq!(disc, Rect::new(41, 1, 18, 18));
    }

    #[test]
    fn a_leading_box_sits_at_the_padding_edge_and_never_leaves_the_rect() {
        // A checkbox indicator: 18x18, 2 px in from the left, centred vertically.
        let indicator = ControlMetrics::leading_box(
            Rect::new(0, 0, 100, 40),
            Size::new(dimensions::CHECKBOX_BOX, dimensions::CHECKBOX_BOX),
            EdgeOffsets::all(2),
        );
        assert_eq!(indicator, Rect::new(2, 11, 18, 18));

        // Padding so large the box would fall out: it is pinned to the right edge
        // of the rectangle instead of being painted past it.
        let pinned = ControlMetrics::leading_box(
            Rect::new(0, 0, 20, 40),
            Size::new(dimensions::CHECKBOX_BOX, dimensions::CHECKBOX_BOX),
            EdgeOffsets::all(50),
        );
        assert_eq!(pinned.x, 2);
        assert_eq!(pinned.x + pinned.width as i32, 20);
    }

    #[test]
    fn a_band_is_centred_and_keeps_its_thickness() {
        // The whole point: a 4 px progress bar in a 120 px cell stays 4 px.
        let band =
            ControlMetrics::centered_band(Rect::new(0, 0, 240, 120), dimensions::PROGRESS_HEIGHT);
        assert_eq!(band, Rect::new(0, 58, 240, 4));

        // A band thicker than the rectangle is clamped to it.
        let clamped = ControlMetrics::centered_band(Rect::new(0, 10, 100, 2), 4);
        assert_eq!(clamped, Rect::new(0, 10, 100, 2));
    }

    /// The metric table's own invariants, extended with the constants added for the
    /// controls that used to derive their chrome from `rect`.
    ///
    /// Each of these is a "one fact, two consumers" value: a diameter that must agree
    /// with a `size_hint`, a row that must be its own height, a padding that must not
    /// vanish. Asserting the relation here means a later edit to one of the pair cannot
    /// silently break it in a control whose snapshot nobody re-read.
    /// The dimensions table is self-consistent, checked **at compile time**.
    ///
    /// # Why these are `const` assertions rather than ordinary ones
    ///
    /// Every claim below relates two constants to each other — a thumb's diameter against
    /// its track, a pill's height against its label's line box, a field's height against
    /// the touch floor. They are properties of the table rather than of any execution, so
    /// stating them in a `const` block makes a contradictory pair a **build failure** rather
    /// than a test failure. A test can be filtered out, or run against a stale binary; a
    /// `const` assertion cannot, which is exactly the guarantee a table like this needs.
    #[test]
    fn the_dimensions_table_is_internally_consistent() {
        const {
            // ── Sliders: the handle is wider than its groove ──
            // A handle no wider than the track it rides on is a line, not a grip.
            assert!(dimensions::SLIDER_THUMB_RADIUS * 2 > dimensions::SLIDER_TRACK_HEIGHT);
            assert!(dimensions::SLIDER_TRACK_RADIUS * 2 <= dimensions::SLIDER_TRACK_HEIGHT);

            // ── The switch: thumb plus insets exactly fill the track's height ──
            assert!(
                dimensions::SWITCH_THUMB_RADIUS * 2 + dimensions::SWITCH_THUMB_INSET * 2
                    == dimensions::SWITCH_TRACK.height
            );
            assert!(dimensions::SWITCH_THUMB_RADIUS * 2 < dimensions::SWITCH_TRACK.width);

            // ── The bar family is a stadium at its own height ──
            assert!(dimensions::PROGRESS_RADIUS * 2 == dimensions::PROGRESS_HEIGHT);
            assert!(dimensions::PROGRESS_HEIGHT <= dimensions::SLIDER_TRACK_HEIGHT * 2);

            // ── A scrollbar's floor is longer than its trough is thick ──
            // A proportional thumb shrunk to the floor must still be a grabbable grip
            // rather than a dot.
            assert!(dimensions::SCROLLBAR_MIN_LENGTH > dimensions::SCROLLBAR_THICKNESS);

            // ── The indicator pair reads as a set ──
            // A checkbox box and a radio ring sit side by side in every form; letting them
            // differ by more than a couple of pixels makes them look like different control
            // families rather than two spellings of one choice.
            assert!(dimensions::CHECKBOX_BOX >= dimensions::RADIO_OUTER_RADIUS * 2);
            assert!(dimensions::CHECKBOX_BOX - dimensions::RADIO_OUTER_RADIUS * 2 <= 2);
            assert!(dimensions::RADIO_DOT_RADIUS < dimensions::RADIO_OUTER_RADIUS);

            // ── The row-like controls are bands, not panels ──
            assert!(dimensions::MENU_BAR_HEIGHT < dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::STATUS_BAR_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::TAB_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::PAGINATION_HEIGHT <= dimensions::TOOLBAR_HEIGHT);
            assert!(dimensions::BREADCRUMB_HEIGHT <= dimensions::TOOLBAR_HEIGHT);

            // ── A stepper's buttons fit inside its row ──
            assert!(dimensions::STEPPER_BUTTON_WIDTH <= dimensions::STEPPER_ROW_HEIGHT);
            assert!(dimensions::STEPPER_ROW_HEIGHT - dimensions::STEPPER_PADDING * 2 > 0);

            // ── The touch floor is the strongest floor ──
            // A field is the smallest control a finger must address, and a button is
            // taller than the floor rather than equal to it.
            assert!(dimensions::TEXT_FIELD_MIN_HEIGHT >= dimensions::TOUCH_TARGET_MIN);
            assert!(dimensions::TOUCH_TARGET_MIN > dimensions::BUTTON_MIN.height);

            // ── A search box is a text field, by construction ──
            assert!(dimensions::SEARCH_BOX_FIELD_HEIGHT == dimensions::TEXT_FIELD_MIN_HEIGHT);

            // ── A badge's pill holds its own label's line box ──
            assert!(dimensions::BADGE_PILL_HEIGHT > dimensions::BADGE_LABEL_FONT_SIZE);

            // ── A color history row is the swatch it holds ──
            assert!(dimensions::COLOR_HISTORY_ROW_HEIGHT == dimensions::COLOR_HISTORY_SWATCH);
            assert!(dimensions::COLOR_HISTORY_PER_ROW > 0);

            // ── A skeleton's gap cannot consume its own row ──
            assert!(dimensions::SKELETON_ROW_GAP < dimensions::SKELETON_ROW_HEIGHT);

            // ── The refresh indicator is a strip, not the whole control ──
            assert!(dimensions::REFRESH_INDICATOR_HEIGHT < 120);

            // ── The roller shows the same number of rows a picker does ──
            assert!(dimensions::ROLLER_VISIBLE_ROWS == dimensions::PICKER_VISIBLE_ROWS);
            assert!(
                dimensions::ROLLER_WHEEL_HEIGHT
                    == dimensions::ROLLER_VISIBLE_ROWS * dimensions::ROLLER_ROW_HEIGHT
            );
        }
    }

    #[test]
    fn mirrored_offsets_exchange_horizontal_sides_only() {
        let offsets = EdgeOffsets::new(1, 2, 3, 4);
        let mirrored = offsets.mirrored();
        assert_eq!(mirrored.top, 1);
        assert_eq!(mirrored.bottom, 3);
        assert_eq!(mirrored.left, 2);
        assert_eq!(mirrored.right, 4);
        assert_eq!(mirrored.horizontal_total(), offsets.horizontal_total());
    }

    #[test]
    fn a_focus_ring_is_inset_so_it_never_leaves_the_control() {
        // The ring must fit inside the control: a control laid out flush against a
        // neighbour must not paint over it.
        let ring = ControlMetrics::focus_ring_rect(Rect::new(0, 0, 64, 40));
        assert_eq!(ring, Rect::new(2, 2, 60, 36));
        assert!(ring.x >= 0 && ring.y >= 0);
        assert!(ring.x + ring.width as i32 <= 64);
        assert!(ring.y + ring.height as i32 <= 40);

        // A control smaller than two ring widths collapses to zero rather than
        // inverting, the same rule `content_box` follows.
        let tiny = ControlMetrics::focus_ring_rect(Rect::new(5, 5, 2, 2));
        assert_eq!(tiny.width, 0);
        assert_eq!(tiny.height, 0);
    }

    #[test]
    fn a_focus_ring_follows_the_control_corner_tightly() {
        // A stadium stays a stadium; square corners stay square.
        assert_eq!(ControlMetrics::focus_ring_radius(0), 0);
        assert_eq!(ControlMetrics::focus_ring_radius(6), 4);
        // A radius smaller than the ring cannot produce one, so it floors at 0.
        assert_eq!(ControlMetrics::focus_ring_radius(1), 0);
    }

    #[test]
    fn horizontal_and_vertical_totals_add_both_sides() {
        let offsets = EdgeOffsets::new(3, 5, 7, 11);
        assert_eq!(offsets.horizontal_total(), 16);
        assert_eq!(offsets.vertical_total(), 10);
    }

    #[test]
    fn a_top_band_keeps_its_thickness_at_the_top_edge() {
        // A nav bar in a 120 px census cell: 44 px at y 0, not 44 px centred and not
        // 120 px tall. This is the assertion behind `pagination.svg` and `app_bar.svg`
        // no longer drawing a full-canvas chrome bar.
        let band = ControlMetrics::top_band(Rect::new(0, 0, 240, 120), dimensions::TAB_HEIGHT);
        assert_eq!(band, Rect::new(0, 0, 240, 24));

        // A band taller than the rectangle is clamped to it rather than painted past it.
        let clamped = ControlMetrics::top_band(Rect::new(5, 7, 100, 10), 40);
        assert_eq!(clamped, Rect::new(5, 7, 100, 10));
    }

    #[test]
    fn a_bottom_band_keeps_its_thickness_at_the_bottom_edge() {
        // A bottom tab strip in a 120 px cell: its height sits on the bottom edge, so
        // `TabWidget`'s South tabs are at y 96 and not y -24 as they were before.
        let band = ControlMetrics::bottom_band(Rect::new(0, 0, 240, 120), dimensions::TAB_HEIGHT);
        assert_eq!(band, Rect::new(0, 96, 240, 24));
        assert_eq!(band.y + band.height as i32, 120, "the strip must end on the control's edge");
    }

    #[test]
    fn a_bands_inset_is_measured_from_the_band_not_the_control() {
        // A toolbar item: 2 px of edging from the 56 px strip it sits in.
        let strip = ControlMetrics::top_band(Rect::new(0, 0, 240, 120), dimensions::TOOLBAR_HEIGHT);
        let item = ControlMetrics::band_inset(strip, dimensions::TOOLBAR_ITEM_INSET);
        assert_eq!(item, Rect::new(2, 2, 236, 52));

        // An inset that would invert the band collapses it instead, the same rule
        // `content_box` follows.
        let collapsed = ControlMetrics::band_inset(Rect::new(4, 4, 6, 6), 10);
        assert_eq!(collapsed.width, 0);
        assert_eq!(collapsed.height, 0);
    }

    #[test]
    fn a_top_band_and_the_content_below_it_tile_the_rectangle() {
        // The two are derived from one height, so they cannot overlap or leave a gap:
        // a nav bar that covered its own page, or floated above it, is what this pins.
        let rect = Rect::new(0, 0, 240, 120);
        let band = ControlMetrics::top_band(rect, dimensions::TAB_HEIGHT);
        let content = ControlMetrics::content_below_top_band(rect, dimensions::TAB_HEIGHT);
        assert_eq!(content.y, band.y + band.height as i32);
        assert_eq!(content.height + band.height, rect.height);
        assert_eq!(content, Rect::new(0, 24, 240, 96));
    }

    #[test]
    fn a_bottom_band_and_the_content_above_it_tile_the_rectangle() {
        let rect = Rect::new(0, 0, 240, 120);
        let band = ControlMetrics::bottom_band(rect, dimensions::TAB_HEIGHT);
        let content = ControlMetrics::content_above_bottom_band(rect, dimensions::TAB_HEIGHT);
        assert_eq!(content.y + content.height as i32, band.y);
        assert_eq!(content.height + band.height, rect.height);
        assert_eq!(content, Rect::new(0, 0, 240, 96));
    }

    #[test]
    fn a_strip_taller_than_the_control_leaves_no_content_rather_than_a_negative_one() {
        let rect = Rect::new(10, 20, 100, 10);
        assert_eq!(ControlMetrics::content_below_top_band(rect, 40).height, 0);
        assert_eq!(ControlMetrics::content_above_bottom_band(rect, 40).height, 0);
    }

    /// The estimate must agree with the renderer's own measurement of the same string.
    ///
    /// # Why this no longer pins the flat model's numbers
    ///
    /// It used to assert `estimate_text_width("abcd") == 34`, i.e. the flat `0.6 em × n`
    /// model written out longhand. That model is only the *fallback* now: with a face active
    /// (every `desktop`/`tablet`/`mobile` profile ships vector faces) both this function and
    /// the renderer's `shape_line` read the face's real advances, and "abcd" is 32 px, not 34.
    /// Pinning 34 would have forced the estimate to keep measuring with a different ruler than
    /// the renderer — the very defect the test was written to prevent.
    ///
    /// So the invariant asserted is the real one: **the estimate equals what the renderer
    /// measures**, whatever model is in force. The properties that must hold under *both*
    /// models (a wide scalar advances more than a narrow one, empty text advances nothing,
    /// the line box is one em) are asserted separately below.
    #[test]
    fn the_text_estimate_reproduces_the_renderers_advance_model() {
        let font = crate::core::Font::simple("sans-serif", 14.0);

        // The renderer's own measurement, through the same ladder `shape_line` walks.
        let renderer = crate::render::text::measure_text_width_for_test("abcd", &font, 1.0);
        assert_eq!(
            estimate_text_width("abcd", &font, 1.0),
            renderer,
            "a `size_hint` and the paint must use the same ruler"
        );

        // Empty text advances nothing at all, rather than a floor of one cluster.
        assert_eq!(estimate_text_width("", &font, 1.0), 0);

        // A CJK cluster is wider than a Latin one of the same count, which is what makes a
        // label sized this way stay wide enough for its own text.
        assert!(
            estimate_text_width("\u{4e2d}", &font, 1.0) > estimate_text_width("a", &font, 1.0),
            "a wide scalar must advance more than a narrow one"
        );

        // The line box is the font's effective line height, one em at size 14.
        assert_eq!(estimate_line_height(&font, 1.0), 14);
    }

    /// The **fallback** model — what a build with no face uses — still advances per cluster
    /// the way the renderer's own fallback does, so the two only differ when a face exists.
    #[test]
    fn the_fallback_advance_model_matches_the_renderers_fallback() {
        let font = crate::core::Font::simple("sans-serif", 14.0);
        // Whatever tier is in force, summing the renderer's per-cluster model must equal the
        // estimate for a string the face cannot supply — an unassigned private-use scalar.
        let exotic = "\u{e000}\u{e001}";
        let mut expected = 0.0f32;
        crate::render::text::for_each_cluster(exotic, |cluster, _range| {
            expected += estimate_cluster_advance(cluster, 14.0, 1.0);
        });
        assert_eq!(estimate_text_width(exotic, &font, 1.0), expected.round().max(0.0) as u32);
    }

    /// Tracking is paid on the *gaps* between clusters, never after the last one.
    ///
    /// The renderer counts `clusters - 1` gaps for exactly this reason — a trailing gap would
    /// make a centred label sit left of centre — so the measurement must count them the same way
    /// or a tracked label would reserve more room than it paints.
    #[test]
    fn letter_spacing_is_paid_between_clusters_and_not_after_the_last() {
        let plain = crate::core::Font::simple("sans-serif", 14.0);
        let mut tracked = crate::core::Font::simple("sans-serif", 14.0);
        tracked.set_letter_spacing(4.0);

        let one = estimate_text_width("a", &tracked, 1.0);
        assert_eq!(
            one,
            estimate_text_width("a", &plain, 1.0),
            "a single cluster has no gap to pay tracking on"
        );

        // Three clusters pay two gaps: the plain advance plus 2 x 4 px.
        assert_eq!(
            estimate_text_width("abc", &tracked, 1.0),
            estimate_text_width("abc", &plain, 1.0) + 8
        );
    }

    /// The line height follows an explicit `line_height` rather than the point size.
    ///
    /// The renderer derives `TextMetrics::height` from `effective_line_height`, so a control
    /// sizing itself from `size()` alone would disagree with every `text_line` it draws — the
    /// "set 1.8 em leading and every line still centres on the default" defect.
    #[test]
    fn the_line_box_follows_an_explicit_line_height() {
        let font = crate::core::Font::simple("sans-serif", 10.0);
        assert_eq!(estimate_line_height(&font, 1.0), 10);
        let mut taller = font.clone();
        taller.set_line_height(20.0);
        assert_eq!(estimate_line_height(&taller, 1.0), 20);
    }
}
