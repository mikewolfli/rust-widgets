// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Pixel-level operations: blend_painted_glyph, glyph_rects, fill_pixels,
//! blend_pixel, set_pixel, pixel_bytes_len, and anti-aliased coverage/geometry helpers.
use crate::core::{Color, Point, Rect, Size};

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
    let width = w as i32;
    let height = h as i32;
    let mut rects = crate::compat::Vec::new();
    // A whitespace glyph and a zero-extent box produce no ink; the rasteriser returns early for
    // both, and so does this.
    if !ch.is_whitespace() && w != 0 && h != 0 {
        let (glyph, _) = crate::render::text::resolve(ch);
        let gw = glyph.width as i32;
        let gh = glyph.height as i32;
        // A resolved glyph is always a real cell (8x8 or 16x16); the guard is belt-and-braces so
        // a hypothetical zero-sized face divides nothing.
        if gw > 0 && gh > 0 {
            for gy in 0..gh {
                let y0 = y + (gy * height) / gh;
                let mut y1 = y + ((gy + 1) * height) / gh;
                if y1 <= y0 {
                    y1 = y0 + 1;
                }
                for gx in 0..gw {
                    if !glyph.bit(gx as u32, gy as u32) {
                        continue;
                    }
                    let x0 = x + (gx * width) / gw;
                    let mut x1 = x + ((gx + 1) * width) / gw;
                    if x1 <= x0 {
                        x1 = x0 + 1;
                    }
                    rects.push((x0, y0, x1, y1));
                }
            }
        }
    }
    rects.into_iter()
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
    let Some(painted) = crate::render::text::paint_active(ch, cell, coverage) else {
        return false;
    };
    if painted.is_color() {
        // Colour ink needs the four-byte buffer this path does not provide. Reporting "nothing
        // drawn" is the honest answer: the alternative is to misinterpret four-byte pixels as
        // coverage and paint noise. The colour path is reached through `paint_color_glyph` below,
        // which is the caller that reserved the right buffer.
        return false;
    }
    let (width, height) = (cell.width as i32, cell.height as i32);
    let mut any = false;
    for py in 0..height {
        let cy = y + py;
        if cy < 0 || cy >= config.canvas_height as i32 {
            continue;
        }
        for px in 0..width {
            let value = coverage[(py * width + px) as usize];
            if value == 0 {
                continue;
            }
            let cx = x + px;
            if cx < 0 || cx >= config.canvas_width as i32 {
                continue;
            }
            if pixel_visible(config.clip, cx, cy) {
                blend_pixel(
                    config.canvas,
                    config.canvas_width,
                    cx as u32,
                    cy as u32,
                    config.color,
                    value as f32 / 255.0,
                );
                any = true;
            }
        }
    }
    any
}

pub(crate) fn pixel_bytes_len(size: Size) -> usize {
    size.width.saturating_mul(size.height).saturating_mul(4) as usize
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
pub fn fill_pixels(pixels: &mut [u8], color: Color) {
    let chunk_size = 4;
    let color_arr = [color.r, color.g, color.b, color.a];
    for chunk in pixels.chunks_mut(chunk_size) {
        if chunk.len() == chunk_size {
            chunk.copy_from_slice(&color_arr);
        } else {
            chunk.copy_from_slice(&color_arr[..chunk.len()]);
        }
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
/// Alpha-blends `color` over the pixel at `(x, y)` of a row-major RGBA frame
/// buffer, using `coverage` as an extra multiplier on the source alpha.
///
/// `frame` must be laid out with `width` pixels per row in RGBA order. The call
/// is a no-op when `coverage` is non-positive or when `(x, y)` falls outside
/// `frame`. `coverage` is clamped to `[0, 1]`.
pub fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a <= 0.0 {
        frame[idx] = 0;
        frame[idx + 1] = 0;
        frame[idx + 2] = 0;
        frame[idx + 3] = 0;
        return;
    }
    let dst = &mut frame[idx..idx + 4];
    let src = [color.r, color.g, color.b, color.a];
    let src_f: [f32; 4] = [
        src[0] as f32 / 255.0,
        src[1] as f32 / 255.0,
        src[2] as f32 / 255.0,
        src[3] as f32 / 255.0,
    ];
    let dst_f: [f32; 4] = [
        dst[0] as f32 / 255.0,
        dst[1] as f32 / 255.0,
        dst[2] as f32 / 255.0,
        dst[3] as f32 / 255.0,
    ];
    let out_a = src_a + dst_f[3] * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let out_r = (src_f[0] * src_a + dst_f[0] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_g = (src_f[1] * src_a + dst_f[1] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_b = (src_f[2] * src_a + dst_f[2] * dst_f[3] * (1.0 - src_a)) / out_a;
    dst[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
pub(crate) fn circle_fill_coverage(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (radius + 1.0 - distance).clamp(0.0, 1.0)
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
            let distance = (dx * dx + dy * dy).sqrt();
            coverage_sum += circle_fill_coverage(distance, radius);
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
            let distance = (dx * dx + dy * dy).sqrt();
            // Ring coverage: outside inner radius and inside outer radius
            let inner_coverage = circle_fill_coverage(distance, inner_radius);
            let outer_coverage = circle_fill_coverage(distance, outer_radius);
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
pub(crate) fn rounded_rect_coverage(px: i32, py: i32, rect: Rect, radius: u32) -> f32 {
    rounded_rect_coverage_grid(px, py, rect, radius, 2)
}
pub(crate) fn rounded_rect_coverage_grid(
    px: i32,
    py: i32,
    rect: Rect,
    radius: u32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let mut covered = 0u32;
    let total = sample_count * sample_count;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = (sy as f32 + 0.5) / sample_count as f32;
            if point_in_rounded_rect_f32(px as f32 + sample_x, py as f32 + sample_y, rect, radius) {
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
