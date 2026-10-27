// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pixel-level operations: blend_painted_glyph, glyph_rects, fill_pixels,
//! blend_pixel, set_pixel, pixel_bytes_len, and anti-aliased coverage/geometry helpers.
use crate::core::{Color, Point, Rect, Size};
use crate::render::core::command::BlendMode;

/// Where one glyph is painted, and into what.
///
/// # Why this is not owned by the glyph
///
/// The cell is the caller's decision (a label draws a cluster in a box as wide as its own advance),
/// not a property of the face — a vector face can rasterise at any cell size. So position and cell
/// travel together in the caller's struct and the face is asked only "paint into this".
///
/// # Why the position is not a field
///
/// It **is** the caller's `x`/`y`, and it used to be copied in here as well: `glyph_rects` read the
/// copies while the caller read the originals, so the two could disagree without anything noticing.
/// [`blend_painted_glyph`] takes the position as arguments instead, which makes that impossible —
/// there is one place a glyph's position is written, so there is nothing to keep in step.
pub(crate) struct GlyphDrawConfig<'a> {
    /// Cell width in pixels.
    pub w: u32,
    /// Cell height in pixels.
    pub h: u32,
    /// Glyph color.
    pub color: Color,
    /// Canvas width.
    pub canvas_width: u32,
    /// Canvas height.
    pub canvas_height: u32,
    /// Canvas pixel buffer (RGBA8).
    pub canvas: &'a mut [u8],
    /// Active render clip, if any.
    pub clip: Option<(i32, i32, u32, u32)>,
    /// The text weight the glyph is drawn at, in the conventional `100..=900` scale.
    ///
    /// Carried here because the paint call is where a variable face's instance is chosen: the cell
    /// the caller measured came from the same weight, so passing it on is what keeps the pen
    /// positions and the ink describing one face. A build with no shaper has no `fvar` reader, so
    /// the field is deliberately not consulted there (see [`blend_painted_glyph`]).
    pub weight: u16,
    /// Whether the glyph should be drawn with a synthetic oblique slant.
    ///
    /// The rasteriser has no italic axis, so an italic request used to draw upright ink while the
    /// measurement fingerprint still hashed `italic` (G-18). A shear keeps the request visible and
    /// leaves advances untouched, which is the usual synthetic-oblique contract.
    pub italic: bool,
}

/// The solid rectangles one glyph's bitmap produces inside its own box.
///
/// # Why this is a function and not a loop body
///
/// The software rasteriser drew glyphs by walking the `font8x8` bitmap and filling one
/// rectangle per set bit, computing each rectangle as
///
/// ```text
/// x0 = x + gx * w / 8,  x1 = x + (gx + 1) * w / 8
/// y0 = y + gy * h / 8,  y1 = y + (gy + 1) * h / 8
/// ```
///
/// while the SVG backend emitted a `<text>` element and let the viewer's font engine pick a
/// font. Those are two different renderers of the same string: different glyph shapes,
/// different advances, different ink boxes. Neither can be reconciled with the other by
/// adjusting a coordinate.
///
/// So the geometry lives here, once, and **both** backends read it: the rasteriser fills the
/// rectangles, the SVG backend emits them as one `<path>`. The two outputs are then the same
/// drawing by construction — not by resemblance, and not by two implementations that have to
/// be kept in step.
///
/// Returns `(x0, y0, x1, y1)` with `x1`/`y1` **exclusive**, matching the rasteriser's
/// half-open fill. A set bit always produces a non-empty rectangle: the `(gx + 1) * w / 8`
/// term can equal `gx * w / 8` when `w < 8`, and a zero-extent rectangle is a drawing command
/// that paints nothing, so it is widened to one pixel (which is also what the rasteriser's
/// own guard did).
///
/// The cell walked here is the one the **active font stack** answers with — 8x8 for Latin,
/// 16x16 for a CJK character when a CJK face is enabled — and the division is by that cell's
/// own dimensions, not by a hardcoded 8. The default build's stack is the 8x8 face, so every
/// coordinate this produced before the stack existed is reproduced bit for bit.
pub(crate) fn glyph_rects(
    ch: char,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> impl Iterator<Item = (i32, i32, i32, i32)> {
    // A whitespace glyph and a zero-extent box produce no ink; the rasteriser returns early for
    // both, and so does this.
    let glyph = if !ch.is_whitespace() && w != 0 && h != 0 {
        // A resolved glyph is always a real cell (8x8 or 16x16); the guard is belt-and-braces so
        // a hypothetical zero-sized face divides nothing.
        let (glyph, _) = crate::render::text::resolve(ch);
        (glyph.width > 0 && glyph.height > 0).then_some(glyph)
    } else {
        None
    };
    GlyphRectScan {
        glyph,
        x,
        y,
        width: w as i32,
        height: h as i32,
        gw: glyph.map_or(0, |glyph| glyph.width as i32),
        gh: glyph.map_or(0, |glyph| glyph.height as i32),
        gx: 0,
        gy: 0,
    }
}

/// The lazy state behind [`glyph_rects`].
///
/// The rectangles used to be collected into a `Vec` first, so every glyph cost one allocation even
/// when the caller consumed all of them immediately — which every caller does (the rasteriser and
/// the SVG backend both walk the set once, per glyph, per frame). Scanning the bits in `next()`
/// yields the same rectangles in the same order with no intermediate buffer (G-16).
struct GlyphRectScan {
    /// The face's 1-bit cell, or `None` when the character has no ink.
    glyph: Option<crate::render::text::GlyphBitmap>,
    /// Destination origin.
    x: i32,
    /// Destination origin.
    y: i32,
    /// Destination cell width.
    width: i32,
    /// Destination cell height.
    height: i32,
    /// Source cell width, in bitmap pixels.
    gw: i32,
    /// Source cell height, in bitmap pixels.
    gh: i32,
    /// Next source column to inspect.
    gx: i32,
    /// Next source row to inspect.
    gy: i32,
}

impl Iterator for GlyphRectScan {
    type Item = (i32, i32, i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        let glyph = self.glyph?;
        while self.gy < self.gh {
            let y0 = self.y + (self.gy * self.height) / self.gh;
            let mut y1 = self.y + ((self.gy + 1) * self.height) / self.gh;
            if y1 <= y0 {
                y1 = y0 + 1;
            }
            while self.gx < self.gw {
                let gx = self.gx;
                self.gx += 1;
                if !glyph.bit(gx as u32, self.gy as u32) {
                    continue;
                }
                let x0 = self.x + (gx * self.width) / self.gw;
                let mut x1 = self.x + ((gx + 1) * self.width) / self.gw;
                if x1 <= x0 {
                    x1 = x0 + 1;
                }
                return Some((x0, y0, x1, y1));
            }
            self.gx = 0;
            self.gy += 1;
        }
        None
    }
}

/// Blend one glyph's ink into a canvas, from a face's painted coverage.
///
/// # Why a renderer calls this and not `glyph_rects`
///
/// `glyph_rects` answers "which rectangles does this glyph's 1-bit bitmap produce?" — which is
/// what a *vector* backend wants (one subpath per set source pixel) and what the rasteriser used
/// to want. It is the wrong question for a rasteriser the moment a face can produce partial
/// coverage: antialiased ink is a value per destination pixel, not a set of full-intensity
/// rectangles, and no set of rectangles can express it.
///
/// So the rasteriser asks the face to **paint** and blends what it gets, byte per byte. For a
/// 1-bit face the two are the same picture: [`paint_bitmap`] writes `255` on exactly the pixels
/// `glyph_rects` would have filled, so every existing snapshot is unchanged — and a vector face
/// gets antialiasing through the same blend, with no second code path.
///
/// `coverage` is a caller-owned scratch of at least `cell.area()` bytes, reused across glyphs so
/// no per-glyph allocation happens on a paint path (constraint: a glyph is never resident).
/// Returns whether any ink was blended.
pub(crate) fn blend_painted_glyph(
    ch: char,
    x: i32,
    y: i32,
    /* the cell is taken from `config`, so the two cannot disagree */
    coverage: &mut [u8],
    config: &mut GlyphDrawConfig,
) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    let cell = crate::render::text::Cell::new(config.w, config.h);
    if cell.is_empty() || coverage.len() < cell.area() {
        return false;
    }
    // The face reports *what* it produced, and the two cases blend differently:
    //
    // * coverage (1-bit or a vector ramp) is a per-pixel **alpha** for the caller's text colour,
    //   which is the blend this function has always done;
    // * a colour glyph carries its own colour, so the text colour must **not** be applied — it
    //   replaces the destination instead of tinting it.
    //
    // A colour face needs `cell.area() * 4` bytes, so a caller that reserved one byte per pixel is
    // told `None` rather than having its buffer overrun.
    // A variable face is one face with a `wght` axis, so the weight is part of *which glyph* this
    // is. The cell above was sized from the line the shaper measured at the same weight, so the two
    // halves cannot disagree: `paint_active_at_weight` moves the axis through the same cache the
    // measurement used. Without the shaper there is no `fvar` reader and no face can vary, so the
    // weight is not consulted and the original path runs.
    #[cfg(feature = "text-shaping")]
    let painted = crate::render::text::paint_active_at_weight(ch, cell, coverage, config.weight);
    #[cfg(not(feature = "text-shaping"))]
    let painted = {
        let _ = config.weight;
        crate::render::text::paint_active(ch, cell, coverage)
    };
    let Some(painted) = painted else {
        return false;
    };
    if painted.is_color() {
        // Colour ink needs a four-byte-per-pixel buffer, which this path does not provide — see
        // `blend_color_glyph`, which is the caller that reserved one. Reporting "nothing drawn" is
        // the honest answer: the alternative is to read four-byte pixels as coverage and paint noise.
        return false;
    }
    let italic = config.italic;
    blend_coverage_bitmap(cell.width, cell.height, x, y, coverage, config, italic)
}

/// How a glyph-coverage buffer is laid out.
///
/// # Why this is a type and not a `channels: usize` argument
///
/// The layout is a property of what the rasteriser wrote, and the two values have different
/// *meanings* as well as different sizes: one byte is the pixel's whole coverage, three bytes are
/// the red, green and blue emitters' coverage in that order. Naming them keeps the `3` from being
/// read as "subpixel addressing" by one caller and "three grey samples" by another.
///
/// # Why the subpixel member may be unconstructed in a build
///
/// It is produced by the rasteriser's `AntiAliasMode::Subpixel` and consumed here; the crate's
/// frame path asks for [`Self::Grayscale`] because that is what the active text loop paints. The
/// blend itself is real and tested — see `blend_coverage_bitmap_layout`.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoverageLayout {
    /// One byte per pixel: a single linear coverage, the default and the only layout the active
    /// text path produces.
    Grayscale,
    /// Three bytes per pixel — `R`, `G`, `B` — from the rasteriser's horizontal subpixel grid.
    Subpixel,
}

impl CoverageLayout {
    /// Bytes of coverage this layout stores per device pixel.
    const fn channels(self) -> usize {
        match self {
            Self::Grayscale => 1,
            Self::Subpixel => 3,
        }
    }
}

/// Blends one coverage bitmap of `cell_width` x `cell_height` pixels into the canvas at `(x, y)`.
///
/// Split out of [`blend_painted_glyph`] so a caller holding bytes that were painted **earlier** —
/// the glyph atlas — blends them through exactly the same loop, weight curve and clip test. The
/// arithmetic is unchanged: a glyph's ink is a linear area fraction, so it goes through
/// [`glyph_coverage_weight`] and then [`blend_pixel`], byte for byte as before.
///
/// `apply_italic` is the synthetic oblique. The atlas slants its bitmap when it rasterises, so a
/// caller passing those bytes asks for `false`; a direct paint passes `config.italic`. Doing the
/// shear twice would move the ink two cells' worth (G-202).
///
/// Returns whether any ink was blended.
pub(crate) fn blend_coverage_bitmap(
    cell_width: u32,
    cell_height: u32,
    x: i32,
    y: i32,
    coverage: &[u8],
    config: &mut GlyphDrawConfig,
    apply_italic: bool,
) -> bool {
    blend_coverage_bitmap_layout(
        cell_width,
        cell_height,
        x,
        y,
        coverage,
        config,
        apply_italic,
        CoverageLayout::Grayscale,
    )
}

/// [`blend_coverage_bitmap`] for either coverage layout.
///
/// [`CoverageLayout::Grayscale`] reproduces the single-channel loop exactly: one weight per pixel,
/// one [`blend_pixel`] call, the same clip spans. That equivalence is pinned by a test that blends a
/// three-channel buffer whose channels all equal a grayscale buffer and gets byte-identical pixels.
///
/// # What `CoverageLayout::Subpixel` does, and what it does not claim
///
/// Each of the three bytes is the coverage of one horizontal third of the pixel (red, green, blue),
/// and each is blended through [`glyph_coverage_weight`] and then **into its own frame channel** by
/// [`blend_pixel_per_channel`]. The frame is straight-alpha `RGBA8`, so a per-channel alpha is
/// expressible: each channel composites source-over against the backdrop independently.
///
/// This is genuinely per-channel blending — not three grey samples averaged — but it is not a claim
/// that the display's subpixel order or its gamma are known. The order is the rasteriser's (R, G,
/// B from the left), which is the common stripe arrangement; a BGR panel would need the first and
/// third channels swapped, and nothing here detects that.
///
/// The synthetic oblique shifts whole pixels, exactly as in grayscale: shearing each of the three
/// channels separately is a different (and unmeasured) effect, so it is deliberately not attempted.
fn blend_coverage_bitmap_layout(
    cell_width: u32,
    cell_height: u32,
    x: i32,
    y: i32,
    coverage: &[u8],
    config: &mut GlyphDrawConfig,
    apply_italic: bool,
    layout: CoverageLayout,
) -> bool {
    let (width, height) = (cell_width as i32, cell_height as i32);
    let channels = layout.channels();
    if width <= 0 || height <= 0 || coverage.len() < (width as usize) * (height as usize) * channels
    {
        return false;
    }
    // Synthetic oblique: slide each row horizontally around the cell's vertical centre. Top rows
    // move right and bottom rows move left, so the slant stays inside the cell and the advance is
    // unchanged (G-18).
    let italic_shift = |py: i32| -> i32 {
        if apply_italic {
            (((height as f32 * 0.5) - py as f32) * 0.2).round() as i32
        } else {
            0
        }
    };
    let mut any = false;
    // The clip∩bitmap row/column spans, resolved once instead of testing every pixel (G-010).
    // Each bound mirrors the check it replaces exactly — including `pixel_visible`'s wrapping
    // `clip_width as i32` cast and the saturating sum it feeds, and the wrapped `canvas_* as i32`
    // casts — so the loops visit precisely the pixels the per-pixel tests admitted. The bounds
    // are derived in `i64` because the glyph origin is a public `i32` and a bound outside the
    // bitmap or canvas simply empties its span, which is what the old `continue` did for every
    // pixel of that row or column. A missing clip is the full `i32` range rather than an
    // infinite sentinel: it admits everything the canvas checks allow, and it keeps the
    // subtractions below overflow-free.
    let canvas_height = config.canvas_height as i32 as i64;
    let canvas_width = config.canvas_width as i32 as i64;
    let (clip_x, clip_right, clip_y, clip_bottom) = match config.clip {
        Some((clip_x, clip_y, clip_width, clip_height)) => (
            clip_x as i64,
            clip_x.saturating_add(clip_width as i32) as i64,
            clip_y as i64,
            clip_y.saturating_add(clip_height as i32) as i64,
        ),
        None => (i32::MIN as i64, i32::MAX as i64, i32::MIN as i64, i32::MAX as i64),
    };
    // Rows whose canvas row `y + py` is a visible canvas row inside the clip.
    let row_lower = 0i64.max(-(y as i64)).max(clip_y - y as i64);
    let row_upper = (height as i64).min(canvas_height - y as i64).min(clip_bottom - y as i64);
    let row_lo = row_lower.min(row_upper.max(0)) as i32;
    let row_hi = row_upper.max(0) as i32;
    for py in row_lo..row_hi {
        let cy = y + py;
        let shift = italic_shift(py);
        // Columns of this row whose canvas column `x + px` is visible, inside the clip, and whose
        // shifted source column `px - shift` stays inside the bitmap. The old loop's `px < width`
        // and `source_px < width` bounds both survive here as the `width` and `width + shift`
        // terms.
        let col_lower = 0i64.max(-(x as i64)).max(clip_x - x as i64).max(shift as i64);
        let col_upper = (width as i64)
            .min(width as i64 + shift as i64)
            .min(canvas_width - x as i64)
            .min(clip_right - x as i64);
        let col_lo = col_lower.min(col_upper.max(0)) as i32;
        let col_hi = col_upper.max(0) as i32;
        for px in col_lo..col_hi {
            let source_px = px - shift;
            let cx = x + px;
            match layout {
                CoverageLayout::Grayscale => {
                    let value = coverage[(py * width + source_px) as usize];
                    if value == 0 {
                        continue;
                    }
                    // The coverage is a **linear area fraction**, so it goes through the display's
                    // transfer function before it becomes an alpha. Blending it raw is what made
                    // small text read as thin: measured, a 0.25-coverage edge pixel was written at
                    // 64 instead of 136. See `srgb_ramp` for the numbers and for why only glyph
                    // ink is corrected.
                    blend_pixel(
                        config.canvas,
                        config.canvas_width,
                        cx as u32,
                        cy as u32,
                        config.color,
                        glyph_coverage_weight(value),
                    );
                }
                CoverageLayout::Subpixel => {
                    let base = ((py * width + source_px) as usize) * channels;
                    let [red, green, blue] =
                        [coverage[base], coverage[base + 1], coverage[base + 2]];
                    if red == 0 && green == 0 && blue == 0 {
                        continue;
                    }
                    // Same transfer function per channel, then a per-channel source-over. A pixel
                    // whose three emitters are equally covered lands on exactly the grayscale
                    // result; the difference only appears on the column boundaries.
                    blend_pixel_per_channel(
                        config.canvas,
                        config.canvas_width,
                        cx as u32,
                        cy as u32,
                        config.color,
                        [
                            glyph_coverage_weight(red),
                            glyph_coverage_weight(green),
                            glyph_coverage_weight(blue),
                        ],
                    );
                }
            }
            any = true;
        }
    }
    any
}

/// Composites one pixel with a **per-channel** coverage, source-over against a straight-alpha
/// backdrop.
///
/// # Why this is not [`blend_pixel`] called three times
///
/// Three calls would composite the source over the backdrop three times, so the second and third
/// would see a backdrop the first had already changed — the red emitter would darken the pixel the
/// green and blue emitters then blend into. One call per pixel, with all three alphas known, is the
/// only form that composites each channel against the *original* backdrop.
///
/// # The arithmetic, and why it reduces to [`blend_pixel_with_mode`]'s `Normal` case
///
/// A straight-alpha destination has one alpha but a per-channel coverage gives three source alphas,
/// so the output alpha is the largest of them composited over the backdrop:
///
/// ```text
/// out_a   = max(a_r, a_g, a_b) + dst_a * (1 - max(a_r, a_g, a_b))
/// out_c   = (src_c * a_c + dst_c * dst_a * (1 - a_c)) / out_a
/// ```
///
/// When the three alphas are equal this is exactly the single-alpha formula `blend_pixel`
/// evaluates, byte for byte (same rounding, same clamp), which is what makes a subpixel blend of an
/// equally covered pixel indistinguishable from a grayscale one.
///
/// Only `BlendMode::Normal` is applied: the glyph path has always blended with it, and a subpixel
/// glyph under a separable blend mode is a separate question with its own evidence to gather.
fn blend_pixel_per_channel(
    frame: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Color,
    coverage: [f32; 3],
) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    let base_alpha = color.a as f32 / 255.0;
    let mut alphas = [0.0f32; 3];
    let mut any = false;
    for (alpha, channel) in alphas.iter_mut().zip(coverage.iter()) {
        *alpha = base_alpha * channel.clamp(0.0, 1.0);
        any |= *alpha > 0.0;
    }
    if !any {
        // Nothing to composite. Erasing is a separate, explicit operation (see `blend_pixel`).
        return;
    }
    let dst = &mut frame[idx..idx + 4];
    let dst_a = dst[3] as f32 / 255.0;
    let alpha_max = alphas.iter().cloned().fold(0.0f32, f32::max);
    let out_a = alpha_max + dst_a * (1.0 - alpha_max);
    if out_a <= f32::EPSILON {
        return;
    }
    let src = [color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0];
    for channel in 0..3 {
        let src_a = alphas[channel];
        let dst_c = dst[channel] as f32 / 255.0;
        let out = (src[channel] * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
        dst[channel] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Blend one glyph's **colour** ink into a canvas.
///
/// # Why this is a separate function from [`blend_painted_glyph`]
///
/// The two differ in what the face's bytes *mean*, and there is no way to tell from the bytes
/// alone. Coverage ink is one byte per pixel and is an **alpha** for the caller's text colour; a
/// colour glyph is four bytes per pixel and already carries its own colour, so the text colour must
/// not be applied at all. Folding both into one function would mean either a branch on a flag
/// passed alongside the buffer (which the caller can get out of step with what it allocated) or
/// reading the fourth byte of every coverage glyph as alpha (which paints every antialiased glyph
/// as garbage).
///
/// # Why this is gated on `fonts-emoji-color`
///
/// Without that feature no face in the crate can ever return [`InkKind::Color`], so a caller of
/// this function would be drawing nothing on every call. Gating it removes the function — and its
/// call site in the text path — rather than leaving dead code that looks like it works.
///
/// # The buffer is the caller's proof
///
/// `pixels` must hold at least `cell.area() * 4` bytes. `paint_active` reports `None` when the
/// stack's only source for `ch` is a colour face and the buffer is too small, so a caller holding a
/// 1-bit scratch gets `false` rather than a partial colour glyph. That is the same contract
/// [`blend_painted_glyph`] states, expressed once at the point the buffer is chosen.
///
/// Returns whether any ink was blended.
#[cfg(feature = "fonts-emoji-color")]
pub(crate) fn blend_color_glyph(
    ch: char,
    x: i32,
    y: i32,
    /* RGBA scratch of at least `cell.area() * 4` bytes, reused across glyphs */
    pixels: &mut [u8],
    config: &mut GlyphDrawConfig,
) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    let cell = crate::render::text::Cell::new(config.w, config.h);
    if cell.is_empty() || pixels.len() < cell.area() * 4 {
        return false;
    }
    let Some(painted) = crate::render::text::paint_active(ch, cell, pixels) else {
        return false;
    };
    // A face that answered with coverage here means the stack resolved `ch` through a 1-bit or
    // vector source — there is no colour to blend, and reading the buffer as RGBA would be wrong.
    // The caller that wants those uses `blend_painted_glyph`, so refusing is the correct answer.
    if !painted.is_color() {
        return false;
    }
    blend_rgba_over_canvas(pixels, cell, x, y, config)
}

/// Composites a `cell`-sized straight-RGBA buffer into a canvas at `(x, y)`, honouring the clip.
///
/// Split out from [`blend_color_glyph`] so the pixel loop can be tested with a hand-built buffer,
/// without a colour font being present in the build.
#[cfg(feature = "fonts-emoji-color")]
fn blend_rgba_over_canvas(
    pixels: &[u8],
    cell: crate::render::text::Cell,
    x: i32,
    y: i32,
    config: &mut GlyphDrawConfig,
) -> bool {
    let (width, height) = (cell.width as i32, cell.height as i32);
    // The clip∩bitmap row/column spans, resolved once instead of testing every pixel (G-010).
    // The bounds mirror `blend_coverage_bitmap`'s, which states the exactness argument; here the
    // same derivation covers the straight-RGBA blit, including the italic shift's source bounds.
    let canvas_height = config.canvas_height as i32 as i64;
    let canvas_width = config.canvas_width as i32 as i64;
    let (clip_x, clip_right, clip_y, clip_bottom) = match config.clip {
        Some((clip_x, clip_y, clip_width, clip_height)) => (
            clip_x as i64,
            clip_x.saturating_add(clip_width as i32) as i64,
            clip_y as i64,
            clip_y.saturating_add(clip_height as i32) as i64,
        ),
        None => (i32::MIN as i64, i32::MAX as i64, i32::MIN as i64, i32::MAX as i64),
    };
    let mut any = false;
    let row_lower = 0i64.max(-(y as i64)).max(clip_y - y as i64);
    let row_upper = (height as i64).min(canvas_height - y as i64).min(clip_bottom - y as i64);
    let row_lo = row_lower.min(row_upper.max(0)) as i32;
    let row_hi = row_upper.max(0) as i32;
    for py in row_lo..row_hi {
        let cy = y + py;
        let shift = if config.italic {
            (((height as f32 * 0.5) - py as f32) * 0.2).round() as i32
        } else {
            0
        };
        let col_lower = 0i64.max(-(x as i64)).max(clip_x - x as i64).max(shift as i64);
        let col_upper = (width as i64)
            .min(width as i64 + shift as i64)
            .min(canvas_width - x as i64)
            .min(clip_right - x as i64);
        let col_lo = col_lower.min(col_upper.max(0)) as i32;
        let col_hi = col_upper.max(0) as i32;
        for px in col_lo..col_hi {
            let source_px = px - shift;
            let si = ((py * width + source_px) * 4) as usize;
            let Some(src) = pixels.get(si..si + 4) else {
                continue;
            };
            // A fully transparent source pixel contributes nothing, and skipping it is what keeps a
            // glyph's rectangular bounding box from darkening the canvas around it.
            if src[3] == 0 {
                continue;
            }
            let cx = x + px;
            blend_pixel(
                config.canvas,
                config.canvas_width,
                cx as u32,
                cy as u32,
                Color::rgba(src[0], src[1], src[2], src[3]),
                1.0,
            );
            any = true;
        }
    }
    any
}

pub(crate) fn pixel_bytes_len(size: Size) -> usize {
    size.width.saturating_mul(size.height).saturating_mul(4) as usize
}

/// Offset a coordinate by a signed delta without wrapping.
///
/// Public `RenderCommand` fields (shadow offsets, spreads, stroke insets) are `i32`, so a caller can
/// legally hand in a combination whose sum leaves `i32`'s range. That used to be a debug panic and a
/// release wrap; both are wrong answers, and on a render hot path the caller has no way to report
/// them. Saturating keeps the sign and the direction of the offset — the shadow still lies the way
/// it was asked to — instead of flipping to the far edge of the coordinate space.
pub(crate) fn sat_add_i32(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

/// Negate an offset without wrapping; `i32::MIN` saturates to `i32::MAX`.
///
/// The spread is subtracted from the rect's origin, and `-i32::MIN` is itself out of range — the one
/// value a plain unary minus cannot represent. Saturating keeps the sign flip meaningful (the shadow
/// moves the way the spread says) instead of aborting in a debug build.
pub(crate) fn sat_neg_i32(value: i32) -> i32 {
    value.saturating_neg()
}

/// `value * factor` in `i32` space, saturating instead of wrapping.
///
/// `factor` is the fixed doubling used by spread rects and inner bevels; `value` is a public `i32`,
/// so the product can leave range. Saturating multiplication preserves the sign, which is what lets
/// the caller's subsequent `.max(0)` clamp mean "collapse to zero" rather than "wrap to a huge
/// positive rectangle".
pub(crate) fn sat_mul_i32(value: i32, factor: i32) -> i32 {
    value.saturating_mul(factor)
}

/// Widen a `u32` extent to `i32`, saturating at `i32::MAX`.
///
/// Rect dimensions are `u32` but every consumer here arithmetic's in `i32`. A bare `as i32` turns an
/// extent above `i32::MAX` negative, which a later `.max(0)` then reports as "zero size" even though
/// the caller asked for the largest one. Clamping keeps the magnitude honestly large.
pub(crate) fn u32_to_i32_saturating(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

/// BLUE23 §0A.2 — the text-coverage boundary, asserted where it is decided.
///
/// The crate-level docs claim "the default build draws Latin/ASCII only" and explain
/// that anything else becomes the fallback glyph. A claim about what is *not* supported
/// decays silently — the day a font is added the docs go stale and no test notices. This
/// pins the other direction: the tofu path must be what a CJK character takes, on a
/// default build.
///
/// Moved to the end of the file so no production item follows a test module (the crate's
/// own lint posture: tests last).
/// Writes `color` into `pixels` as consecutive RGBA quads.
///
/// `pixels` must be a row-major RGBA buffer. Trailing bytes that do not form a
/// complete quad are filled with the first bytes of `color[r,g,b,a]` — i.e. a
/// non-multiple-of-four length is tolerated rather than rejected.
///
/// # Why the quads are walked with `as_chunks_mut`
///
/// The previous form asked `chunks_mut(4)` for a slice and then re-tested its length **inside**
/// the loop, so every pixel paid a bounds/branch check and the copy could not be vectorised. This
/// function is the surface clear, so it runs once per frame over the whole window: measured at
/// 1280×960 it was 9.5 ms (≈8 ns per quad) instead of the ≈1 ms the memory bandwidth allows. The
/// body writes **four pixels at a time** through a fixed-size chunk, which is what lets the
/// backend emit wide stores; a per-pixel chunk only ever produced four-byte ones.
pub fn fill_pixels(pixels: &mut [u8], color: Color) {
    let [r, g, b, a] = [color.r, color.g, color.b, color.a];
    let block = [r, g, b, a, r, g, b, a, r, g, b, a, r, g, b, a];
    let blocks = pixels.len() / block.len();
    let blocked = blocks * block.len();
    for chunk in pixels[..blocked].as_chunks_mut::<16>().0 {
        *chunk = block;
    }
    // The remainder is fewer than four pixels: whole quads first, then the partial tail the
    // function's contract says to fill from the leading colour bytes.
    let mut index = blocked;
    while index + 4 <= pixels.len() {
        pixels[index..index + 4].copy_from_slice(&[r, g, b, a]);
        index += 4;
    }
    for (offset, byte) in pixels[index..].iter_mut().enumerate() {
        *byte = [r, g, b, a][offset];
    }
}
pub(crate) fn set_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    frame[idx] = color.r;
    frame[idx + 1] = color.g;
    frame[idx + 2] = color.b;
    frame[idx + 3] = color.a;
}

/// Returns whether a logical pixel lies inside the active render clip.
pub(crate) fn pixel_visible(clip: Option<(i32, i32, u32, u32)>, x: i32, y: i32) -> bool {
    let Some((clip_x, clip_y, clip_width, clip_height)) = clip else {
        return true;
    };
    x >= clip_x
        && y >= clip_y
        && x < clip_x.saturating_add(clip_width as i32)
        && y < clip_y.saturating_add(clip_height as i32)
}
/// Applies the display's transfer function to a **glyph coverage** value, turning linear area into
/// the luminance a viewer perceives.
///
/// # The defect this corrects, measured
///
/// Antialiasing produces a *linear* coverage: a pixel half-covered by a glyph's outline is `0.5`.
/// Blending that value straight into an 8-bit frame treats the frame's bytes as if they were
/// linear light, but they are sRGB — and the eye's response to sRGB is roughly a `1/2.2` power. So
/// a `0.5` coverage that should read as mid-grey is written as `128/255`, which the eye receives as
/// about `0.22` — **less than half the intended weight**.
///
/// Measured on this crate's own glyph coverage output:
///
/// ```text
/// coverage 0.25: linear blend ->  64, gamma-correct -> 136   (difference 72)
/// coverage 0.50: linear blend -> 128, gamma-correct -> 186   (difference 59)
/// coverage 0.75: linear blend -> 191, gamma-correct -> 224   (difference 33)
/// ```
///
/// Every antialiased glyph edge is therefore too dark, and the error is *largest where coverage is
/// smallest* — which is precisely the thin-stem and small-size case. That is why 11–14 px text (97 %
/// of this crate's font sizes) reads as thin and washed out rather than merely small: a glyph's
/// one-pixel strokes are drawn from coverage values in the range this correction moves most.
///
/// # Why this is confined to glyph ink and not applied to every blend
///
/// The correction belongs to **glyph coverage**, and the reason is that a glyph's coverage is the
/// one value in this renderer that is genuinely a *linear area fraction*: it comes from counting
/// how many sub-samples of a pixel a font outline enclosed. A stroked line or a circle's edge is
/// computed differently — its coverage is a distance-derived ramp that has already been shaped to
/// look right against the pixels it lands on — so correcting *those* is a separate question with
/// its own evidence to gather, not a consequence of this one.
///
/// Applying it to the shared [`blend_pixel`] would also silently restyle every geometric primitive
/// in the crate, and it broke two tests that were pinning real AA behaviour rather than a glyph
/// value. Confining the correction to the text path keeps the change equal to the measurement.
///
/// # Why the correction is a lookup
///
/// `255` entries, computed once. The exponent is irrational and this runs per pixel of every glyph;
/// a `powf` in that loop would be the single most expensive instruction in text rendering. A table
/// is 256 bytes, fits a cache line several times over, and makes the correction free.
fn srgb_ramp() -> &'static [u8; 256] {
    use core::sync::atomic::{AtomicU8, Ordering};

    // The table is published through a single `AtomicU8` word per entry, so there is no window in
    // which one thread can observe a half-written ramp.
    //
    // # Why not `static mut` + a ready flag
    //
    // That was the first form, and it is the shape that is *easy to get wrong*: the flag and the data
    // are two independent locations, so correctness rests on the reader having the right ordering with
    // respect to the writer — and on exactly one writer being allowed to run. Two threads that both
    // observe "not ready" both run the initialiser, which is benign here only because the values are
    // identical. An `AtomicU8` per entry has neither problem: every write is atomic on its own, the
    // values are computed on first read rather than written in a separate pass, and any thread may
    // run the initialisation at any time.
    static RAMP: [AtomicU8; 256] = [const { AtomicU8::new(0) }; 256];

    // Compute every entry unconditionally on first access. ``swap`` returns the previous value, so
    // the thread that sees `0` is the one that writes — and `0` is a legal *result* (coverage 0 maps
    // to 0), which is why the sentinel is a separate "not yet computed" state rather than a value.
    //
    // Reading the table back through `load` after the loop is what orders the whole write against
    // every later read: the loop's writes are `Release` and the readers are `Acquire`.
    static READY: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if !READY.load(Ordering::Acquire) {
        for (index, slot) in RAMP.iter().enumerate() {
            let linear = index as f32 / 255.0;
            let value = (linear.powf(1.0 / 2.2) * 255.0).round().clamp(0.0, 255.0) as u8;
            slot.store(value, Ordering::Relaxed);
        }
        READY.store(true, Ordering::Release);
    }

    // SAFETY: every entry was stored before `READY` was published, and the entries are only ever
    // written by the loop above with the same values. A `u8` array has the same layout as
    // `[AtomicU8; N]` in this crate's target set (no interior padding for `u8`), and no reference to
    // the atomic view outlives this call.
    unsafe { &*(core::ptr::addr_of!(RAMP) as *const [u8; 256]) }
}

/// The perceptual weight of a linear glyph coverage in `0..=255`.
///
/// See [`srgb_ramp`] for why glyph coverage is corrected and geometric coverage is not. Coverage `0`
/// and `255` are fixed points, so a fully covered pixel and an uncovered one come out bit-for-bit as
/// they always did — only the partial values move, which is exactly the set antialiasing produces.
pub(crate) fn glyph_coverage_weight(coverage: u8) -> f32 {
    srgb_ramp()[coverage as usize] as f32 / 255.0
}

/// Alpha-blends `color` over the pixel at `(x, y)` of a row-major RGBA frame
/// buffer, using `coverage` as an extra multiplier on the source alpha.
///
/// `frame` must be laid out with `width` pixels per row in RGBA order. The call
/// is a no-op when `coverage` is non-positive or when `(x, y)` falls outside
/// `frame`. `coverage` is clamped to `[0, 1]`.
///
/// This applies `coverage` **as given** — it is the general-purpose blend, used by geometry as well
/// as by text. Callers blending glyph ink should use [`blend_pixel`]'s coverage after passing it
/// through `glyph_coverage_weight`.
pub fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: f32) {
    blend_pixel_with_mode(frame, width, x, y, color, coverage, BlendMode::Normal);
}

/// The separable blend function of a [`BlendMode`], applied per RGB channel to the
/// **backdrop** (`dst`) and the **source** (`src`), both in `0.0..=1.0`.
///
/// These are the formulas CSS `mix-blend-mode` and the PDF/`SVG` compositing spec define, which is
/// the same table the SVG backend's `mix-blend-mode` maps onto — so a `SetBlendMode { Multiply }`
/// frame composites identically through either backend. The non-separable modes (`Hue`,
/// `Saturation`, `Color`, `Luminosity`) need the full RGB triple and are handled in
/// [`blend_pixel_with_mode`] rather than here.
///
/// Returns `None` for the non-separable modes.
fn separable_blend(mode: BlendMode, dst: f32, src: f32) -> Option<f32> {
    Some(match mode {
        BlendMode::Normal => src,
        BlendMode::Multiply => dst * src,
        BlendMode::Screen => dst + src - dst * src,
        BlendMode::Overlay => hard_light(src, dst),
        BlendMode::Darken => dst.min(src),
        BlendMode::Lighten => dst.max(src),
        BlendMode::ColorDodge => {
            if dst <= 0.0 {
                0.0
            } else if src >= 1.0 {
                1.0
            } else {
                (dst / (1.0 - src)).min(1.0)
            }
        }
        BlendMode::ColorBurn => {
            if dst >= 1.0 {
                1.0
            } else if src <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - dst) / src).min(1.0)
            }
        }
        BlendMode::HardLight => hard_light(dst, src),
        BlendMode::SoftLight => soft_light(dst, src),
        BlendMode::Difference => (dst - src).abs(),
        BlendMode::Exclusion => dst + src - 2.0 * dst * src,
        // Non-separable: handled by the caller with all three channels.
        BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => {
            return None
        }
    })
}

/// `HardLight`'s per-channel function: `Multiply`/`Screen` chosen by the **source**.
fn hard_light(base: f32, blend: f32) -> f32 {
    if blend <= 0.5 {
        base * (2.0 * blend)
    } else {
        base + (2.0 * blend - 1.0) * (1.0 - base)
    }
}

/// `SoftLight`'s per-channel function (the W3C compositing formula).
fn soft_light(base: f32, blend: f32) -> f32 {
    if blend <= 0.5 {
        base - (1.0 - 2.0 * blend) * base * (1.0 - base)
    } else {
        let d = if base <= 0.25 { ((16.0 * base - 12.0) * base + 4.0) * base } else { base.sqrt() };
        base + (2.0 * blend - 1.0) * (d - base)
    }
}

/// Non-separable blend: `SetLum`, `SetSat`, `SetHue` … over the full RGB triple.
///
/// Returns the blended RGB for the four non-separable modes using the W3C compositing definitions.
fn non_separable_blend(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    let lum = |c: [f32; 3]| 0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2];
    let clip_color = |mut c: [f32; 3]| {
        let l = lum(c);
        let n = c.iter().cloned().fold(f32::INFINITY, f32::min);
        let x = c.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        if n < 0.0 {
            for v in &mut c {
                *v = l + (*v - l) * l / (l - n);
            }
        }
        if x > 1.0 {
            for v in &mut c {
                *v = l + (*v - l) * (1.0 - l) / (x - l);
            }
        }
        c
    };
    let set_lum =
        |c: [f32; 3], l: f32| clip_color([c[0] + l - lum(c), c[1] + l - lum(c), c[2] + l - lum(c)]);
    let sat = |c: [f32; 3]| {
        c.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
            - c.iter().cloned().fold(f32::INFINITY, f32::min)
    };
    let set_sat = |mut c: [f32; 3], s: f32| {
        let (mut lo, mut mid, mut hi) = (0usize, 1usize, 2usize);
        if c[lo] > c[mid] {
            core::mem::swap(&mut lo, &mut mid);
        }
        if c[mid] > c[hi] {
            core::mem::swap(&mut mid, &mut hi);
        }
        if c[lo] > c[mid] {
            core::mem::swap(&mut lo, &mut mid);
        }
        let (out_lo, out_hi) = (0.0, s);
        let out_mid =
            if c[hi] - c[lo] > 0.0 { (c[mid] - c[lo]) * s / (c[hi] - c[lo]) } else { 0.0 };
        let mut out = [0.0f32; 3];
        out[lo] = out_lo;
        out[mid] = out_mid;
        out[hi] = out_hi;
        c = out;
        c
    };
    match mode {
        BlendMode::Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        BlendMode::Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        BlendMode::Color => set_lum(cs, lum(cb)),
        BlendMode::Luminosity => set_lum(cb, lum(cs)),
        // The separable modes never reach here.
        _ => cs,
    }
}

/// Alpha-blends `color` over the pixel at `(x, y)`, compositing the source through `mode`'s blend
/// function first.
///
/// This is [`blend_pixel`] with the backend's current [`RenderCommand::SetBlendMode`] applied. The
/// SVG backend maps each mode onto a `mix-blend-mode` group; this is the rasteriser half of that
/// pair, and both compute the same W3C formulas — which is what makes the two backends agree for
/// one command. Before this existed the software backend *stored* the mode and never read it, so
/// `SetBlendMode` was a silent no-op on the reference implementation while the snapshot honoured
/// it: one command, two pictures.
///
/// [`RenderCommand::SetBlendMode`]: crate::render::RenderCommand::SetBlendMode
pub fn blend_pixel_with_mode(
    frame: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Color,
    coverage: f32,
    mode: BlendMode,
) {
    if coverage <= 0.0 {
        return;
    }
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a <= 0.0 {
        // A fully transparent source contributes nothing in source-over, so the destination is left
        // untouched. This used to zero the pixel, which turned "draw nothing" into "erase whatever
        // was already there" -- a caller drawing a transparent colour over an opaque background lit
        // a black hole through it. Erasing is a separate, explicit operation; compositing is not it.
        return;
    }
    let dst = &mut frame[idx..idx + 4];
    let src_f: [f32; 4] = [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        color.a as f32 / 255.0,
    ];
    let dst_f: [f32; 4] = [
        dst[0] as f32 / 255.0,
        dst[1] as f32 / 255.0,
        dst[2] as f32 / 255.0,
        dst[3] as f32 / 255.0,
    ];
    // Blend the source against the backdrop **before** the alpha composite. For `Normal` this is
    // the identity (`B(Cb, Cs) = Cs`), so the arithmetic below is bit-for-bit the old path.
    let blended = if mode == BlendMode::Normal {
        [src_f[0], src_f[1], src_f[2]]
    } else {
        // One call decides which family the mode belongs to; the red channel's result *is* that
        // answer. It used to be computed twice — once to test and once to store — so every
        // non-`Normal` pixel paid for one extra blend function (G-11). The bytes are unchanged:
        // the discarded call returned exactly the value bound here.
        match separable_blend(mode, dst_f[0], src_f[0]) {
            Some(red) => [
                red,
                separable_blend(mode, dst_f[1], src_f[1]).unwrap_or(src_f[1]),
                separable_blend(mode, dst_f[2], src_f[2]).unwrap_or(src_f[2]),
            ],
            None => non_separable_blend(
                mode,
                [dst_f[0], dst_f[1], dst_f[2]],
                [src_f[0], src_f[1], src_f[2]],
            ),
        }
    };
    let out_a = src_a + dst_f[3] * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let out_r = (blended[0] * src_a + dst_f[0] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_g = (blended[1] * src_a + dst_f[1] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_b = (blended[2] * src_a + dst_f[2] * dst_f[3] * (1.0 - src_a)) / out_a;
    dst[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
/// Writes one pixel through `mode`, used by every rasteriser primitive.
///
/// # Why every pixel goes through here
///
/// The rasteriser's primitives (`primitives.rs`) all end in a per-pixel write, and before this
/// existed each one called `set_pixel`/`blend_pixel` directly — which meant a
/// [`RenderCommand::SetBlendMode`](crate::render::RenderCommand::SetBlendMode) command had nowhere to
/// take effect, so the software backend **stored** the mode and never read it while the SVG backend
/// honoured it. Routing the writes through one function makes "the mode applies to what is drawn
/// next" true for opaque fills, antialiased geometry, and glyphs alike.
///
/// `Normal` preserves the historical arithmetic bit for bit: a fully covered pixel is a
/// replacement, a partial one an alpha blend, with no blend function in between.
pub(crate) fn write_pixel_with_mode(
    mode: BlendMode,
    frame: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Color,
    coverage: f32,
) {
    if matches!(mode, BlendMode::Normal) {
        // A fully covered **opaque** pixel can be stored verbatim. A translucent one cannot: the
        // frame is straight (non-premultiplied) RGBA and is premultiplied only at blit time, so
        // storing the colour bytes and dropping the destination leaves the backdrop's RGB under a
        // partial alpha — a shadow or scrim drew a see-through hole in whatever it covered. Route
        // translucent pixels through the source-over composite, which is exactly what the
        // antialiased edge of the same shape already did.
        if coverage >= 1.0 && color.a == 255 {
            set_pixel(frame, width, x, y, color);
        } else {
            blend_pixel_with_mode(frame, width, x, y, color, coverage, BlendMode::Normal);
        }
        return;
    }
    blend_pixel_with_mode(frame, width, x, y, color, coverage, mode);
}

pub(crate) fn circle_fill_coverage(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (radius + 1.0 - distance).clamp(0.0, 1.0)
}

/// [`circle_fill_coverage`] taking the **squared** distance from the centre.
///
/// The clamp makes the answer exact outside a one-pixel band: a sample at distance `d <= r` is
/// fully covered and one at `d >= r + 1` is empty, so only the edge band needs the square root
/// (G-10). The thresholds are compared in `f64` because `dx * dx + dy * dy` is an `f32` value and
/// `r * r` rounds in `f32`, while the wider product of two `f32` operands is exact — that makes
/// `dist2 <= r * r` exactly equivalent to `sqrt(dist2) <= r`, so the two shortcuts below return
/// the very value the old per-sample `sqrt` + clamp returned, bit for bit. A sample that lands in
/// the band still evaluates the identical `f32` expression.
fn circle_fill_coverage_at_sq(dist2: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let squared = dist2 as f64;
    let r = radius as f64;
    if squared <= r * r {
        return 1.0;
    }
    let outer = r + 1.0;
    if squared >= outer * outer {
        return 0.0;
    }
    circle_fill_coverage_in_band(dist2, radius)
}

/// The square root the band case above needs, kept as the named entry point so the clamp lives in
/// exactly one place.
#[inline]
fn circle_fill_coverage_in_band(dist2: f32, radius: f32) -> f32 {
    circle_fill_coverage(dist2.sqrt(), radius)
}

pub(crate) fn circle_fill_coverage_grid(
    px: i32,
    py: i32,
    center: Point,
    radius: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            // The distance is no longer rooted here: the three cases the clamp distinguishes are
            // decided on the squared distance, and only an edge-band sample pays for the `sqrt`.
            coverage_sum += circle_fill_coverage_at_sq(dx * dx + dy * dy, radius);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn circle_stroke_coverage_grid(
    px: i32,
    py: i32,
    center: Point,
    radius: f32,
    stroke_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    // radius is the outer radius, stroke_width is the width of the ring
    let outer_radius = radius;
    let inner_radius = (radius - stroke_width).max(0.0);
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            // Both circles are answered from the one squared distance, so a ring's sample needs no
            // root at all unless it falls in the outer band (G-10).
            let dist2 = dx * dx + dy * dy;
            let inner_coverage = circle_fill_coverage_at_sq(dist2, inner_radius);
            let outer_coverage = circle_fill_coverage_at_sq(dist2, outer_radius);
            // Ring is outer circle minus inner circle
            coverage_sum += (outer_coverage - inner_coverage).max(0.0);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn point_to_segment_distance(
    px: f32,
    py: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len2 = abx * abx + aby * aby;
    if ab_len2 <= f32::EPSILON {
        let dx = px - ax;
        let dy = py - ay;
        return (dx * dx + dy * dy).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    let dx = px - cx;
    let dy = py - cy;
    (dx * dx + dy * dy).sqrt()
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn line_stroke_coverage_grid(
    px: i32,
    py: i32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    half_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let distance = point_to_segment_distance(sample_x, sample_y, ax, ay, bx, by);
            coverage_sum += (half_width + 0.5 - distance).clamp(0.0, 1.0);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn rounded_rect_effective_radius(rect: Rect, radius: u32) -> u32 {
    radius.min(rect.width / 2).min(rect.height / 2)
}
/// The single radius the four corners share, when they do.
///
/// STY-01: corner geometry now arrives as `[top_left, top_right, bottom_right,
/// bottom_left]`; every path that only understands one radius goes through this
/// predicate so the uniform case keeps the exact arithmetic it always had.
pub(crate) fn uniform_radii(radii: [u32; 4]) -> Option<u32> {
    (radii[0] == radii[1] && radii[1] == radii[2] && radii[2] == radii[3]).then_some(radii[0])
}
/// Per-corner radii resolved the way CSS `border-radius` resolves them.
///
/// When the two radii meeting at one edge sum past that edge's length, every
/// radius is scaled by the largest factor that makes them fit — which also caps
/// each radius at half the edge it sits on, so the shape never degenerates. A
/// zero-sized edge zeroes its corners. Order is `[top_left, top_right,
/// bottom_right, bottom_left]`, matching [`CornerRadii`](crate::style::CornerRadii)'s
/// field order.
pub(crate) fn rounded_rect_effective_radii(rect: Rect, radii: [u32; 4]) -> [u32; 4] {
    if rect.width == 0 || rect.height == 0 {
        return [0; 4];
    }
    // Integer arithmetic throughout: a candidate is `radius * edge / adjacent_sum`,
    // and the smallest candidate wins. For four equal radii this is exactly the
    // legacy clamp — `r * edge / (2 * r)` truncates to `edge / 2`, the same
    // integer division `rounded_rect_effective_radius` performs — so the uniform
    // shape never moves by a pixel even if this function is reached with one.
    let original = [radii[0] as u64, radii[1] as u64, radii[2] as u64, radii[3] as u64];
    let w = rect.width as u64;
    let h = rect.height as u64;
    let mut resolved = original;
    for (sum, edge) in [
        (original[0] + original[1], w),
        (original[1] + original[2], h),
        (original[2] + original[3], w),
        (original[3] + original[0], h),
    ] {
        if sum > edge {
            for (slot, radius) in resolved.iter_mut().zip(original) {
                let scaled = radius * edge / sum;
                if scaled < *slot {
                    *slot = scaled;
                }
            }
        }
    }
    resolved.map(|r| r as u32)
}
/// The per-corner inside test, mirroring [`point_in_rounded_rect_f32`] for
/// corners that differ.
///
/// A sample sits in a corner zone when it is past that corner's radius on both
/// axes; the zone test is the same quarter-disc equation the uniform test uses,
/// with the corner's own radius. Adjacent corner zones cannot overlap because
/// the resolved radii satisfy the CSS fit per edge, so first match wins is
/// exact, not a tie-break.
pub(crate) fn point_in_rounded_rect_corners_f32(
    px: f32,
    py: f32,
    rect: Rect,
    radii: [u32; 4],
) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let left = rect.x as f32;
    let top = rect.y as f32;
    let right = left + rect.width as f32;
    let bottom = top + rect.height as f32;
    if px < left || px >= right || py < top || py >= bottom {
        return false;
    }
    let [tl, tr, br, bl] = rounded_rect_effective_radii(rect, radii);
    let (tl, tr, br, bl) = (tl as f32, tr as f32, br as f32, bl as f32);
    if tl > 0.0 && px < left + tl && py < top + tl {
        let dx = px - (left + tl);
        let dy = py - (top + tl);
        return dx * dx + dy * dy <= tl * tl;
    }
    if tr > 0.0 && px >= right - tr && py < top + tr {
        let dx = px - (right - tr);
        let dy = py - (top + tr);
        return dx * dx + dy * dy <= tr * tr;
    }
    if br > 0.0 && px >= right - br && py >= bottom - br {
        let dx = px - (right - br);
        let dy = py - (bottom - br);
        return dx * dx + dy * dy <= br * br;
    }
    if bl > 0.0 && px < left + bl && py >= bottom - bl {
        let dx = px - (left + bl);
        let dy = py - (bottom - bl);
        return dx * dx + dy * dy <= bl * bl;
    }
    true
}
pub(crate) fn inset_rect(rect: Rect, inset: i32) -> Rect {
    let x = rect.x + inset;
    let y = rect.y + inset;
    let width = (rect.width as i32 - inset * 2).max(0) as u32;
    let height = (rect.height as i32 - inset * 2).max(0) as u32;
    Rect { x, y, width, height }
}
pub(crate) fn point_in_rounded_rect_f32(px: f32, py: f32, rect: Rect, radius: u32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let left = rect.x as f32;
    let top = rect.y as f32;
    let right = rect.x as f32 + rect.width as f32;
    let bottom = rect.y as f32 + rect.height as f32;
    if px < left || px >= right || py < top || py >= bottom {
        return false;
    }
    let r = rounded_rect_effective_radius(rect, radius) as f32;
    if r <= 0.0 {
        return true;
    }
    if (px >= left + r && px < right - r) || (py >= top + r && py < bottom - r) {
        return true;
    }
    let cx = if px < left + r {
        left + r
    } else if px >= right - r {
        right - r
    } else {
        px
    };
    let cy = if py < top + r {
        top + r
    } else if py >= bottom - r {
        bottom - r
    } else {
        py
    };
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}
pub(crate) fn rounded_rect_coverage(px: i32, py: i32, rect: Rect, radii: [u32; 4]) -> f32 {
    rounded_rect_coverage_grid(px, py, rect, radii, 2)
}
pub(crate) fn rounded_rect_coverage_grid(
    px: i32,
    py: i32,
    rect: Rect,
    radii: [u32; 4],
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let mut covered = 0u32;
    let total = sample_count * sample_count;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            // The uniform case keeps the historical single-radius test so every
            // frame a uniform radius ever produced stays byte-for-byte; only a
            // genuinely per-corner shape takes the new arithmetic.
            let inside = match uniform_radii(radii) {
                Some(radius) => point_in_rounded_rect_f32(sample_x, sample_y, rect, radius),
                None => point_in_rounded_rect_corners_f32(sample_x, sample_y, rect, radii),
            };
            if inside {
                covered += 1;
            }
        }
    }
    covered as f32 / total as f32
}

/// BLUE23 §0A.2 — the text-coverage boundary, asserted where it is decided.
///
/// The crate-level docs claim "the default build draws Latin/ASCII only" and explain that
/// anything else becomes the fallback glyph. A claim about what is *not* supported decays
/// silently — the day a font is added the docs go stale and no test notices. So the boundary
/// is pinned from both sides: the tofu path is what a non-Latin character takes on a default
/// build, and enabling a data feature moves that boundary by exactly the face it adds.
#[cfg(test)]
mod text_coverage_tests {
    use super::glyph_coverage_weight;
    use crate::render::text;

    #[test]
    fn ascii_resolves_to_a_real_glyph() {
        assert_eq!(text::source_for('A'), Some("font8x8"), "'A' is inside the base face");
        assert_eq!(text::source_for('5'), Some("font8x8"));
    }

    /// The scripts the crate docs name as unsupported: CJK, Cyrillic, Arabic, emoji. With no
    /// font data enabled, every one of them must take the fallback glyph.
    #[cfg(not(feature = "fonts-cjk-bitmap"))]
    #[test]
    fn non_latin_resolves_to_the_fallback_glyph_by_default() {
        for ch in ['\u{4e2d}', '\u{0416}', '\u{0627}', '\u{1f600}'] {
            assert_eq!(
                text::source_for(ch),
                None,
                "U+{:04X} is outside the base face and must take the fallback glyph",
                ch as u32
            );
        }
    }

    // ── The display transfer function applied to glyph coverage ──────────

    /// The ramp must be monotonically non-decreasing and must fix its endpoints.
    ///
    /// Monotonicity is what makes the correction a *contrast* operation rather than a distortion: a
    /// pixel with more of a glyph on it must never come out lighter than one with less. The endpoints
    /// being fixed is what keeps the change equal to the measurement — every fully covered pixel and
    /// every uncovered one in the crate stays bit-for-bit, so only antialiased edges move.
    #[test]
    fn the_glyph_coverage_ramp_is_monotonic_and_fixes_its_endpoints() {
        assert_eq!(glyph_coverage_weight(0), 0.0, "no coverage paints nothing");
        assert!(
            (glyph_coverage_weight(255) - 1.0).abs() < 1e-6,
            "full coverage must stay fully opaque, or every solid pixel shifts"
        );
        let mut previous = -1.0f32;
        for level in 0..=255u8 {
            let weight = glyph_coverage_weight(level);
            assert!(weight >= previous - 1e-6, "coverage {level} is lighter than {}", level - 1);
            previous = weight;
        }
    }

    /// The correction must **lift** the mid-range, which is the whole reason it exists.
    ///
    /// Measured before it was applied: a 0.25-coverage glyph edge was written at 64 instead of 136,
    /// so every antialiased stroke was more than twice as dark as intended and small text read as
    /// thin. This pins the direction and the rough magnitude, not the exact curve — the exponent is
    /// an approximation of a display's response and a future profile may change it.
    #[test]
    fn the_correction_lifts_the_mid_range_and_barely_touches_a_solid_stem() {
        let mid = glyph_coverage_weight(128);
        assert!(mid > 0.6, "mid coverage must land near the perceptual middle, got {mid}");

        // A near-solid pixel is already where the curve is flat: a vertical stem needs no help, and
        // this is why `l` gains ~5 % while a curved bowl gains ~40 %. It also means the correction
        // cannot blow out solid ink, which is what a badly-placed gamma curve would do.
        let solid = glyph_coverage_weight(240);
        assert!(solid > 0.96, "a near-solid pixel must stay near-solid, got {solid}");

        // And the lift must be strictly larger low down than high up, or it would be a brightness
        // change rather than a transfer function.
        let low_lift = glyph_coverage_weight(64) - 64.0 / 255.0;
        let high_lift = glyph_coverage_weight(200) - 200.0 / 255.0;
        assert!(low_lift > high_lift, "the lift must be largest where coverage is smallest");
    }

    /// With the CJK data enabled, the boundary moves to exactly where the feature says: Han is
    /// covered by the added face, and the scripts the feature does *not* carry still fall back.
    /// This is the half of the claim that would otherwise rot unnoticed.
    #[cfg(feature = "fonts-cjk-bitmap")]
    #[test]
    fn enabling_the_cjk_data_moves_the_boundary_by_exactly_one_face() {
        assert_eq!(text::source_for('\u{4e2d}'), Some("cjk-bitmap"));
        assert_eq!(text::source_for('\u{0416}'), None, "Cyrillic is not in the CJK subset");
        assert_eq!(text::source_for('\u{0627}'), None, "Arabic is not in the CJK subset");
        assert_eq!(text::source_for('\u{1f600}'), None, "emoji is not in the CJK subset");
    }
}

// ── STY-01: per-corner coverage ──────────────────────────────────────────────

#[cfg(test)]
mod corner_coverage_tests {
    use super::*;

    /// A uniform radii array must take the identical arithmetic the scalar test has always
    /// run: every frame a uniform radius ever produced stays byte-for-byte.
    #[test]
    fn uniform_input_matches_the_legacy_point_test() {
        let rects = [
            Rect { x: 0, y: 0, width: 1, height: 1 },
            Rect { x: 2, y: 3, width: 8, height: 8 },
            Rect { x: -2, y: 5, width: 33, height: 17 },
            Rect { x: 0, y: 0, width: 60, height: 40 },
        ];
        for rect in rects {
            for radius in [0u32, 1, 3, 6, 17, 1000] {
                for py in (rect.y - 1)..(rect.y + rect.height as i32 + 1) {
                    for px in (rect.x - 1)..(rect.x + rect.width as i32 + 1) {
                        let legacy = point_in_rounded_rect_f32(
                            px as f32 + 0.25,
                            py as f32 + 0.75,
                            rect,
                            radius,
                        );
                        let corners = point_in_rounded_rect_corners_f32(
                            px as f32 + 0.25,
                            py as f32 + 0.75,
                            rect,
                            [radius; 4],
                        );
                        assert_eq!(
                            legacy, corners,
                            "rect {rect:?} radius {radius} diverged at ({px},{py})"
                        );
                    }
                }
            }
        }
    }

    /// CSS `border-radius`: adjacent radii that overrun their edge are scaled down by the
    /// same factor until they fit, which also caps a radius at half the edge.
    #[test]
    fn overrunning_corners_are_scaled_down_proportionally() {
        // 15 + 15 > 20: the factor 20/30 shrinks both top radii to 10.
        let resolved = rounded_rect_effective_radii(Rect::new(0, 0, 20, 40), [15, 15, 0, 0]);
        assert_eq!(resolved, [10, 10, 0, 0]);
        // A single oversized corner is capped by the tightest edge it sits on:
        // top factor 20/40, left factor 10/40 — the global factor 0.25 shrinks
        // the corner to 10, per the CSS fit rule.
        let capped = rounded_rect_effective_radii(Rect::new(0, 0, 20, 10), [40, 0, 0, 0]);
        assert_eq!(capped, [10, 0, 0, 0]);
        // A zero-sized edge zeroes its corners.
        let empty = rounded_rect_effective_radii(Rect::new(0, 0, 0, 10), [8, 8, 8, 8]);
        assert_eq!(empty, [0; 4]);
    }

    /// The per-corner zones: only the declared corners round, and a pixel inside the rect
    /// but outside a rounded corner's quarter disc is coverage 0.
    #[test]
    fn only_declared_corners_round() {
        let rect = Rect::new(0, 0, 40, 40);
        let radii = [10, 0, 0, 0];
        // Inside the top-left quarter disc (centre (10, 10), radius 10).
        assert!(point_in_rounded_rect_corners_f32(4.0, 4.0, rect, radii));
        // Same offset from the top-right corner, which carries radius 0: inside the rect.
        assert!(point_in_rounded_rect_corners_f32(36.0, 4.0, rect, radii));
        // Outside the top-left disc but inside the rect: not covered.
        assert!(!point_in_rounded_rect_corners_f32(1.0, 1.0, rect, radii));
        // Centre is always inside.
        assert!(point_in_rounded_rect_corners_f32(20.0, 20.0, rect, radii));
    }
}

/// The clip∩bitmap span hoisting in the glyph blend loops (G-010): after the loops stopped
/// calling `pixel_visible` per pixel, they must still visit exactly the pixels that check
/// admitted — for clips that contain, cut, miss, or precede the bitmap, at canvas edges, and
/// under the synthetic oblique's shifted source columns.
#[cfg(test)]
mod glyph_blend_clip_tests {
    use super::*;
    // The `mini` profile is `no_std`, so `Vec` is not in the prelude; every test module that
    // builds one names it here instead of relying on `super::*` to carry it.
    use crate::compat::Vec;

    const CANVAS: u32 = 8;

    fn config<'a>(
        canvas: &'a mut [u8],
        clip: Option<(i32, i32, u32, u32)>,
        italic: bool,
    ) -> GlyphDrawConfig<'a> {
        GlyphDrawConfig {
            w: 4,
            h: 10,
            color: Color::rgb(255, 0, 0),
            canvas_width: CANVAS,
            canvas_height: CANVAS,
            canvas,
            clip,
            weight: 400,
            italic,
        }
    }

    /// The destination pixels the old per-pixel tests admitted, derived independently of the
    /// production loop: the cell closed under the canvas bounds, the clip, the oblique's
    /// source-column restriction, and the bitmap's own ink (zero-coverage source pixels never
    /// blended, before or after the hoisting). This is the spec the hoisted spans must reproduce.
    fn expected_ink(
        x: i32,
        y: i32,
        clip: Option<(i32, i32, u32, u32)>,
        cell_width: i32,
        cell_height: i32,
        italic: bool,
        coverage: &[u8],
    ) -> Vec<(i32, i32)> {
        let shift_of = |py: i32| -> i32 {
            if italic {
                (((cell_height as f32 * 0.5) - py as f32) * 0.2).round() as i32
            } else {
                0
            }
        };
        let mut pixels = Vec::new();
        for py in 0..cell_height {
            let shift = shift_of(py);
            for px in 0..cell_width {
                let source_px = px - shift;
                if source_px < 0 || source_px >= cell_width {
                    continue;
                }
                if coverage[py as usize * cell_width as usize + source_px as usize] == 0 {
                    continue;
                }
                let (cx, cy) = (x + px, y + py);
                let on_canvas = cx >= 0 && cy >= 0 && cx < CANVAS as i32 && cy < CANVAS as i32;
                let visible = clip.map_or(true, |(cl, ct, cw, ch)| {
                    cx >= cl
                        && cy >= ct
                        && cx < cl.saturating_add(cw as i32)
                        && cy < ct.saturating_add(ch as i32)
                });
                if on_canvas && visible {
                    pixels.push((cx, cy));
                }
            }
        }
        pixels
    }

    /// Every fully-red pixel of the canvas, as the set of coordinates carrying ink.
    fn ink_positions(canvas: &[u8]) -> Vec<(i32, i32)> {
        let mut found = Vec::new();
        for cy in 0..CANVAS as i32 {
            for cx in 0..CANVAS as i32 {
                let i = ((cy * CANVAS as i32 + cx) * 4) as usize;
                if canvas[i] == 255
                    && canvas[i + 1] == 0
                    && canvas[i + 2] == 0
                    && canvas[i + 3] == 255
                {
                    found.push((cx, cy));
                }
            }
        }
        found
    }

    fn assert_ink_matches(
        clip: Option<(i32, i32, u32, u32)>,
        x: i32,
        y: i32,
        italic: bool,
        coverage: &[u8],
        cell_width: u32,
        cell_height: u32,
    ) {
        let mut canvas = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let mut cfg = config(&mut canvas, clip, italic);
        cfg.w = cell_width;
        cfg.h = cell_height;
        let any = blend_coverage_bitmap(cell_width, cell_height, x, y, coverage, &mut cfg, italic);
        let expected =
            expected_ink(x, y, clip, cell_width as i32, cell_height as i32, italic, coverage);
        assert_eq!(ink_positions(&canvas), expected, "clip {clip:?} at ({x}, {y})");
        assert_eq!(any, !expected.is_empty(), "the return must report real ink");
    }

    #[test]
    fn an_unclipped_solid_bitmap_lands_exactly_on_its_cell() {
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(None, 2, 2, false, &coverage, 4, 10);
    }

    #[test]
    fn a_partial_clip_admits_only_the_intersecting_span() {
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(Some((3, 3, 2, 2)), 2, 2, false, &coverage, 4, 10);
    }

    #[test]
    fn a_disjoint_clip_draws_nothing_and_reports_no_ink() {
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(Some((6, 0, 2, 8)), 2, 2, false, &coverage, 4, 10);
    }

    #[test]
    fn a_bitmap_partly_off_canvas_is_cut_at_the_canvas_edge() {
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(None, 6, 6, false, &coverage, 4, 10);
        // A negative origin cuts the bitmap's top and left against the canvas itself.
        assert_ink_matches(None, -2, -2, false, &coverage, 4, 10);
    }

    #[test]
    fn a_clip_lying_before_the_canvas_origin_still_cuts_correctly() {
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(Some((-2, -2, 6, 6)), 2, 2, false, &coverage, 4, 10);
        // A clip whose saturating end sum is beyond `i32` bounds is not constructible here, but
        // the empty clip (zero width) must draw nothing.
        assert_ink_matches(Some((3, 3, 0, 0)), 2, 2, false, &coverage, 4, 10);
    }

    #[test]
    fn a_sparse_bitmap_blends_only_its_ink_pixels() {
        // One ink pixel at source (1, 1); everything else is zero coverage and must skip the
        // blend even where the clip admits the pixel.
        let mut coverage = vec![0u8; 4 * 10];
        coverage[1 * 4 + 1] = 255;
        assert_ink_matches(None, 2, 2, false, &coverage, 4, 10);
        assert_ink_matches(Some((3, 3, 1, 1)), 2, 2, false, &coverage, 4, 10);
    }

    #[test]
    fn the_synthetic_oblique_shifts_the_source_columns_not_the_bounds() {
        // The 10-pixel cell makes the oblique's shift cross zero within the bitmap, so the
        // source-column restriction moves rows both ways.
        let coverage = vec![255u8; 4 * 10];
        assert_ink_matches(None, 2, -3, true, &coverage, 4, 10);
        assert_ink_matches(Some((3, 0, 3, 8)), 2, -3, true, &coverage, 4, 10);
    }

    /// The straight-RGBA blit must honour the same spans: opaque source pixels land exactly
    /// where the old per-pixel checks admitted, transparent ones never darken the canvas.
    #[cfg(feature = "fonts-emoji-color")]
    #[test]
    fn the_rgba_blit_honours_the_clip_and_canvas_spans() {
        // Opaque red at source pixels (0, 0) and (3, 3); every other source pixel is fully
        // transparent and must never darken the canvas.
        let mut pixels = vec![0u8; 4 * 10 * 4];
        for source in [0usize, 3 * 4 + 3] {
            let base = source * 4;
            pixels[base..base + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        let draw = |clip: Option<(i32, i32, u32, u32)>| {
            let mut canvas = vec![0u8; (CANVAS * CANVAS * 4) as usize];
            let mut cfg = config(&mut canvas, clip, false);
            let cell = crate::render::text::Cell::new(4, 10);
            let any = blend_rgba_over_canvas(&pixels, cell, 2, 2, &mut cfg);
            (canvas.to_vec(), any)
        };
        let (canvas, any) = draw(None);
        assert_eq!(
            ink_positions(&canvas),
            vec![(2, 2), (5, 5)],
            "unclipped: the two opaque source pixels land on their cells"
        );
        assert!(any);
        let (canvas, any) = draw(Some((0, 0, 4, 3)));
        assert_eq!(ink_positions(&canvas), vec![(2, 2)], "the clip removes the second pixel");
        assert!(any);
        let (canvas, any) = draw(Some((6, 0, 2, 8)));
        assert!(ink_positions(&canvas).is_empty(), "disjoint clip draws nothing");
        assert!(!any, "and reports no ink");
    }
}

/// The two coverage layouts the rasteriser can produce must blend through one loop, and the
/// three-channel one must composite each emitter into its own frame channel.
///
/// # Why the equality case is the important one
///
/// A subpixel blend of a pixel whose three emitters are equally covered has to be *indistinguishable*
/// from the grayscale blend, or the mode changes pixels it has no information about. Comparing two
/// canvases byte for byte is what holds that: any divergence in the weight curve, the rounding or the
/// alpha composite shows up on the first partial-coverage pixel.
#[cfg(test)]
mod coverage_layout_tests {
    use super::*;
    // See `glyph_blend_clip_tests`: `Vec` is not in the `mini` prelude.
    use crate::compat::Vec;

    const CANVAS: u32 = 8;

    fn config(canvas: &mut [u8]) -> GlyphDrawConfig<'_> {
        GlyphDrawConfig {
            w: 4,
            h: 4,
            // Black ink on the white canvas below, so a fully covered channel is unmistakable.
            color: Color::rgb(0, 0, 0),
            canvas_width: CANVAS,
            canvas_height: CANVAS,
            canvas,
            clip: None,
            weight: 400,
            italic: false,
        }
    }

    /// A canvas of opaque white, the backdrop both layouts are compared on.
    fn white_canvas() -> Vec<u8> {
        let mut canvas = vec![255u8; (CANVAS * CANVAS * 4) as usize];
        for pixel in canvas.chunks_mut(4) {
            pixel[3] = 255;
        }
        canvas
    }

    /// One pixel's `RGBA` at `(x, y)`.
    fn pixel(canvas: &[u8], x: u32, y: u32) -> [u8; 4] {
        let index = ((y * CANVAS + x) * 4) as usize;
        [canvas[index], canvas[index + 1], canvas[index + 2], canvas[index + 3]]
    }

    #[test]
    fn equal_channels_reproduce_the_grayscale_blend_byte_for_byte() {
        // A ramp that exercises partial, near-solid and solid coverage.
        let coverage: Vec<u8> = (0..16).map(|index| (index * 17) as u8).collect();
        let mut three_channel = Vec::with_capacity(coverage.len() * 3);
        for value in &coverage {
            three_channel.extend_from_slice(&[*value, *value, *value]);
        }

        let mut gray_canvas = white_canvas();
        let mut cfg = config(&mut gray_canvas);
        let gray_any = blend_coverage_bitmap(4, 4, 2, 2, &coverage, &mut cfg, false);

        let mut sub_canvas = white_canvas();
        let mut cfg = config(&mut sub_canvas);
        let sub_any = blend_coverage_bitmap_layout(
            4,
            4,
            2,
            2,
            &three_channel,
            &mut cfg,
            false,
            CoverageLayout::Subpixel,
        );

        assert!(gray_any && sub_any, "both layouts must report the ink they blended");
        assert_eq!(
            gray_canvas, sub_canvas,
            "three equal channels must composite exactly like one grayscale value"
        );
    }

    #[test]
    fn each_emitter_is_blended_into_its_own_frame_channel() {
        // Red fully covered, green and blue untouched: black ink must reach only the red byte.
        let coverage = [255u8, 0, 0];
        let mut canvas = white_canvas();
        let mut cfg = config(&mut canvas);
        let any = blend_coverage_bitmap_layout(
            1,
            1,
            3,
            3,
            &coverage,
            &mut cfg,
            false,
            CoverageLayout::Subpixel,
        );
        assert!(any);
        assert_eq!(pixel(&canvas, 3, 3), [0, 255, 255, 255], "only red carried coverage");

        // And the middle emitter reaches green only.
        let coverage = [0u8, 255, 0];
        let mut canvas = white_canvas();
        let mut cfg = config(&mut canvas);
        assert!(blend_coverage_bitmap_layout(
            1,
            1,
            4,
            4,
            &coverage,
            &mut cfg,
            false,
            CoverageLayout::Subpixel,
        ));
        assert_eq!(pixel(&canvas, 4, 4), [255, 0, 255, 255], "only green carried coverage");
        // Neighbouring pixels stay white: the blend is per pixel, not a row-wide wash.
        assert_eq!(pixel(&canvas, 5, 4), [255, 255, 255, 255]);
    }

    #[test]
    fn a_buffer_too_short_for_three_channels_draws_nothing() {
        // One byte per pixel: the grayscale length, which the subpixel layout must reject.
        let coverage = [255u8; 16];
        let mut canvas = white_canvas();
        let mut cfg = config(&mut canvas);
        assert!(
            !blend_coverage_bitmap_layout(
                4,
                4,
                2,
                2,
                &coverage,
                &mut cfg,
                false,
                CoverageLayout::Subpixel,
            ),
            "a 16-byte buffer cannot describe a 4x4 three-channel cell"
        );
        assert_eq!(canvas, white_canvas(), "and it must not have touched the canvas");
    }

    #[test]
    fn an_all_zero_subpixel_cell_reports_no_ink() {
        let coverage = [0u8; 16 * 3];
        let mut canvas = white_canvas();
        let mut cfg = config(&mut canvas);
        assert!(!blend_coverage_bitmap_layout(
            4,
            4,
            2,
            2,
            &coverage,
            &mut cfg,
            false,
            CoverageLayout::Subpixel,
        ));
        assert_eq!(canvas, white_canvas());
    }
}
