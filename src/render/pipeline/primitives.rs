// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Software rendering primitives: rect, rounded rect, circle, line, arc, path,
//! polygon, triangle, text, and image drawing methods.
//!
//! Methods are implemented as `impl SoftwareSurface` blocks.

#[cfg(any(
    test,
    feature = "fonts-emoji-color",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::arc_helpers::{for_each_arc_sample, normalized_arc_sweep};
use crate::render::pipeline::pixel_ops::{
    blend_painted_glyph, circle_fill_coverage_grid, circle_stroke_coverage_grid, fill_pixels,
    inset_rect, line_stroke_coverage_grid, pixel_visible, rounded_rect_coverage,
    rounded_rect_coverage_grid, rounded_rect_effective_radii, rounded_rect_effective_radius,
    u32_to_i32_saturating, uniform_radii, write_pixel_with_mode, GlyphDrawConfig,
};
use crate::render::BlendMode;
// Imported separately because it only exists when a colour face does — see its own docs.
#[cfg(feature = "fonts-emoji-color")]
use crate::render::pipeline::pixel_ops::blend_color_glyph;
// The atlas fast path (G-202) is only reachable when shaping is compiled in.
#[cfg(feature = "text-shaping")]
use crate::render::pipeline::pixel_ops::blend_coverage_bitmap;
use crate::render::text::{is_combining_mark, is_variation_selector};
use crate::render::SoftwareSurface;

/// Shrinks a pixel bounding box to the current clip rectangle.
///
/// Coverage loops that evaluate a primitive per pixel were iterating the primitive's full
/// bounding box and letting `blend_pixel_clipped!` reject each out-of-clip sample. A small glyph
/// or circle inside a large clip therefore paid for thousands of samples nobody could see
/// (G-007). Returning the intersection lets the loop skip them outright.
#[inline]
fn clip_pixel_box(
    clip: Option<(i32, i32, u32, u32)>,
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
) -> (i32, i32, i32, i32) {
    match clip {
        Some((x, y, width, height)) => {
            let clip_max_x = x.saturating_add(u32_to_i32_saturating(width)).saturating_sub(1);
            let clip_max_y = y.saturating_add(u32_to_i32_saturating(height)).saturating_sub(1);
            (min_x.max(x), min_y.max(y), max_x.min(clip_max_x), max_y.min(clip_max_y))
        }
        None => (min_x, min_y, max_x, max_y),
    }
}

macro_rules! set_pixel_clipped {
    ($blend:expr, $clip:expr, $frame:expr, $width:expr, $x:expr, $y:expr, $color:expr) => {
        if pixel_visible($clip, $x as i32, $y as i32) {
            // Every opaque primitive funnels through here, so this is where the backend's current
            // `SetBlendMode` is applied. `Normal` composites the source verbatim (the old
            // `set_pixel`, bit for bit); any other mode runs the W3C blend function first. Without
            // this the software backend stored the mode and never read it, so a `SetBlendMode` frame
            // was a no-op on the rasteriser while the SVG snapshot honoured it.
            write_pixel_with_mode($blend, $frame, $width, $x as u32, $y as u32, $color, 1.0);
        }
    };
}

macro_rules! blend_pixel_clipped {
    ($blend:expr, $clip:expr, $frame:expr, $width:expr, $x:expr, $y:expr, $color:expr, $coverage:expr) => {
        if pixel_visible($clip, $x as i32, $y as i32) {
            write_pixel_with_mode($blend, $frame, $width, $x as u32, $y as u32, $color, $coverage);
        }
    };
}

/// Which crossings of a scanline count as interior.
///
/// The two rules differ only in how they read the running winding: a non-zero winding fills
/// every pixel with at least one net turn, while even-odd fills every pixel reached by an odd
/// number of crossings. Both are produced by the same scan-conversion loop, selected here.
#[derive(Clone, Copy)]
enum FillRule {
    /// Interior where the signed winding number is non-zero.
    NonZero,
    /// Interior where the number of crossed edges is odd.
    EvenOdd,
}

impl FillRule {
    #[inline]
    fn contains(self, winding: i32) -> bool {
        match self {
            Self::NonZero => winding != 0,
            // Every crossing contributes ±1, so the parity of the signed sum is the parity of
            // the number of crossings (two's-complement `& 1` is the parity for negatives too).
            Self::EvenOdd => (winding & 1) != 0,
        }
    }
}

/// The part of a destination rectangle that is inside the surface and the active clip.
///
/// Returns `(left, top, right, bottom)` with `right`/`bottom` exclusive, or `None` when nothing
/// is visible. The rectangle is intersected **once** rather than tested per pixel: the clip test
/// costs about as much as the 4-byte write it guards, and a fill or a blit is the hottest loop a
/// software backend runs.
fn visible_bounds(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    screen: (u32, u32),
    clip: Option<(i32, i32, u32, u32)>,
) -> Option<(i32, i32, i32, i32)> {
    let mut left = x.max(0);
    let mut top = y.max(0);
    // Computed in `i64` so a rectangle wider than `i32::MAX` (a caller's arithmetic error, but one
    // that must not turn into a wrapped *negative* right edge) saturates instead of folding.
    let mut right = (x as i64 + width as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let mut bottom = (y as i64 + height as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    right = right.min(screen.0 as i32);
    bottom = bottom.min(screen.1 as i32);
    if let Some((clip_x, clip_y, clip_width, clip_height)) = clip {
        left = left.max(clip_x);
        top = top.max(clip_y);
        right = right.min(clip_x.saturating_add(clip_width as i32));
        bottom = bottom.min(clip_y.saturating_add(clip_height as i32));
    }
    if right <= left || bottom <= top {
        return None;
    }
    Some((left, top, right, bottom))
}

/// Writes a rectangle through `blend`, intersecting the surface and the active clip once.
///
/// Every axis-aligned primitive here reduces to "some rows of one colour": a wide horizontal or
/// vertical line, the middle band of a rounded rectangle, one brush stamp of a diagonal line.
/// Routing them all through one function keeps the clip arithmetic and the `Normal`-and-opaque
/// `memcpy` fast path in one place instead of once per primitive. The bytes are the ones a
/// per-pixel `write_pixel_with_mode(.., 1.0)` produces: a plain store for an opaque `Normal`, the
/// same composite for anything else.
fn write_solid_bounds(
    blend: BlendMode,
    clip: Option<(i32, i32, u32, u32)>,
    frame: &mut [u8],
    screen: (u32, u32),
    rect: (i32, i32, u32, u32),
    color: Color,
) {
    let (x, y, width, height) = rect;
    let Some((left, top, right, bottom)) = visible_bounds(x, y, width, height, screen, clip) else {
        return;
    };
    write_solid_rows(blend, frame, screen.0, top, bottom, left, right, color);
}

/// Writes `color` across `left..right` on every row of `top..bottom`.
///
/// The bounds must already be inside the surface **and** the clip; the callers get them from
/// [`visible_bounds`] or intersect them with the clip's own rectangle. Doing it that way is what
/// removes the per-pixel clip test and the per-pixel blend dispatch from the fill loops.
fn write_solid_rows(
    blend: BlendMode,
    frame: &mut [u8],
    width: u32,
    top: i32,
    bottom: i32,
    left: i32,
    right: i32,
    color: Color,
) {
    if right <= left || bottom <= top {
        return;
    }
    if matches!(blend, BlendMode::Normal) && color.a == 255 {
        let stride = width as usize * 4;
        let row_len = (right - left) as usize * 4;
        for y in top..bottom {
            let start = y as usize * stride + left as usize * 4;
            fill_pixels(&mut frame[start..start + row_len], color);
        }
        return;
    }
    for y in top..bottom {
        for x in left..right {
            write_pixel_with_mode(blend, frame, width, x as u32, y as u32, color, 1.0);
        }
    }
}

/// The columns a rounded-rectangle **corner row** can ink, as `(first, last)`.
///
/// A corner row's ink is the union of two circles, one radius in from each side, so the horizontal
/// half-extent at that row is `sqrt(r^2 - dy^2)` around each centre. `dy` is the smallest distance
/// from the corner centre to either sample row of this pixel row, which maximises the extent, and
/// the limits are rounded outwards — so this trims only columns whose every sample would have
/// scored `0.0`. It never changes a pixel; it only skips the empty corners of a large radius (G-08).
fn corner_row_reach(
    py: i32,
    corner_cy: i32,
    left_cx: i32,
    right_cx: i32,
    radius: i32,
) -> (i32, i32) {
    // Above this radius the `f32` spacing no longer resolves a pixel, so the trim would be guessing.
    // A radius that large cannot put the arc inside the surface anyway, so sampling the whole band
    // is both correct and no slower than this function's caller used to be.
    if radius <= 0 || radius > (1 << 20) {
        return (i32::MIN, i32::MAX);
    }
    // `point_in_rounded_rect_f32` samples at 0.25 and 0.75 within the pixel; the nearest sample to
    // the centre is half a pixel from the row's midline minus a quarter. Using a full half pixel
    // keeps the bound conservative.
    let dy = (((py as f32 + 0.5) - corner_cy as f32).abs() - 0.5).max(0.0);
    let r = radius as f32;
    let half_extent = (r * r - dy * dy).max(0.0).sqrt();
    let first = (left_cx as f32 - half_extent).floor() as i32;
    let last = (right_cx as f32 + half_extent).ceil() as i32;
    (first, last)
}

impl SoftwareSurface {
    /// Fills a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let size = self.buffer.size();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let clip = self.current_clip();
        let Some((left, top, right, bottom)) = visible_bounds(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            (size.width, size.height),
            clip,
        ) else {
            return;
        };
        let blend = self.blend_mode;
        let width = size.width;
        let frame = self.buffer.back_mut();
        // An opaque fill under `Normal` blending is a store, so the rectangle is one pattern-filled
        // row followed by row copies. This is the background every window paints each frame; the
        // per-pixel clip test and the per-pixel blend dispatch were pure overhead over a `memcpy`.
        if matches!(blend, BlendMode::Normal) && color.a == 255 {
            let pattern = [color.r, color.g, color.b, color.a];
            let row_len = (right - left) as usize * 4;
            let stride = width as usize * 4;
            let mut start = top as usize * stride + left as usize * 4;
            for chunk in frame[start..start + row_len].as_chunks_mut::<4>().0 {
                chunk.copy_from_slice(&pattern);
            }
            for _ in top + 1..bottom {
                frame.copy_within(start..start + row_len, start + stride);
                start += stride;
            }
            return;
        }
        for y in top..bottom {
            for x in left..right {
                write_pixel_with_mode(blend, frame, width, x as u32, y as u32, color, 1.0);
            }
        }
    }
    /// Draws a 1px rectangle stroke.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.draw_rect_with_width(rect, color, 1);
    }
    /// Draws a rectangle stroke with explicit width.
    pub fn draw_rect_with_width(&mut self, rect: Rect, color: Color, stroke_width: u32) {
        if stroke_width == 0 {
            return;
        }
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let x0 = rect.x;
        let y0 = rect.y;
        let x1 = rect.right() - 1;
        let y1 = rect.bottom() - 1;
        self.draw_line_with_width(
            Point { x: x0, y: y0 },
            Point { x: x1, y: y0 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x0, y: y1 },
            Point { x: x1, y: y1 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x0, y: y0 },
            Point { x: x0, y: y1 },
            color,
            stroke_width,
        );
        self.draw_line_with_width(
            Point { x: x1, y: y0 },
            Point { x: x1, y: y1 },
            color,
            stroke_width,
        );
    }
    /// Fills a rounded rectangle using coverage blending.
    ///
    /// # Why each row is split into bands
    ///
    /// `point_in_rounded_rect_f32` answers "inside" as soon as a pixel is between the corner
    /// circles horizontally (`left + r <= px < right - r`) or vertically, so the interior of the
    /// shape is fully covered no matter where it is sampled. It used to be sampled 2x2 anyway:
    /// four geometry tests for a pixel in the middle of a button, where the answer is always
    /// `1.0`. Each row is now one of two shapes (G-08):
    ///
    /// * a **middle row** (`top + r <= py < bottom - r`): every column is coverage `1.0`, written
    ///   as one solid span through the same blend-aware write a covered pixel already used;
    /// * a **corner row**: the middle band is still solid, and only the left and right corner
    ///   bands run the per-pixel 2x2 sampler. Their ends come from the corner-circle equation, so
    ///   a row near the top of the shape samples only the few columns its arc can reach.
    ///
    /// The corner bands keep the exact sampling they had, and the solid spans write the `1.0`
    /// that sampling returns for those pixels, so the frame is bit-identical.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let blend = self.blend_mode;
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        if x1 < x0 || y1 < y0 {
            return;
        }
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        // The band edges saturate: a radius past `i32::MAX` puts every column in a corner band, and
        // the trim below then declines to narrow it, so an out-of-range radius degrades to the old
        // per-pixel sampling rather than to a wrong picture.
        let r = u32_to_i32_saturating(effective_radius);
        let middle_first = rect.x.saturating_add(r);
        let middle_after_last = rect.right().saturating_sub(r);
        let top_band_last = rect.y.saturating_add(r).saturating_sub(1);
        let bottom_band_first = rect.bottom().saturating_sub(r);
        // The clip's own rectangle, resolved once: a solid span is intersected with it instead of
        // paying a `pixel_visible` test per pixel. The corner bands keep the per-pixel test.
        let (clip_left, clip_right, clip_top, clip_bottom) = match clip {
            Some((clip_x, clip_y, clip_w, clip_h)) => (
                clip_x,
                clip_x.saturating_add(u32_to_i32_saturating(clip_w)),
                clip_y,
                clip_y.saturating_add(u32_to_i32_saturating(clip_h)),
            ),
            None => (i32::MIN, i32::MAX, i32::MIN, i32::MAX),
        };
        for py in y0..=y1 {
            let top_corner = py <= top_band_last;
            let bottom_corner = py >= bottom_band_first;
            let row_visible = py >= clip_top && py < clip_bottom;
            if !top_corner && !bottom_corner {
                let start = x0.max(clip_left);
                let end = x1.min(clip_right.saturating_sub(1));
                if row_visible {
                    write_solid_rows(blend, frame, size.width, py, py + 1, start, end + 1, color);
                }
                continue;
            }
            let corner_cy =
                if top_corner { rect.y.saturating_add(r) } else { rect.bottom().saturating_sub(r) };
            let (reach_first, reach_last) =
                corner_row_reach(py, corner_cy, middle_first, middle_after_last, r);
            // Left corner band: the columns before the first fully covered one.
            let left_band_start = x0.max(reach_first);
            let left_band_end = middle_first.saturating_sub(1).min(x1);
            if left_band_start <= left_band_end {
                for px in left_band_start..=left_band_end {
                    let coverage = rounded_rect_coverage(px, py, rect, [effective_radius; 4]);
                    if coverage > 0.0 {
                        blend_pixel_clipped!(
                            blend, clip, frame, size.width, px, py, color, coverage
                        );
                    }
                }
            }
            // Middle band: fully covered even on a corner row.
            let middle_start = middle_first.max(x0).max(clip_left);
            let middle_end =
                middle_after_last.saturating_sub(1).min(x1).min(clip_right.saturating_sub(1));
            if row_visible {
                write_solid_rows(
                    blend,
                    frame,
                    size.width,
                    py,
                    py + 1,
                    middle_start,
                    middle_end + 1,
                    color,
                );
            }
            // Right corner band. Clamped to `middle_first` so a saturated radius, which collapses
            // the middle band, cannot make this band overlap the left one.
            let right_band_start = middle_after_last.max(middle_first).max(x0);
            let right_band_end = x1.min(reach_last);
            if right_band_start <= right_band_end {
                for px in right_band_start..=right_band_end {
                    let coverage = rounded_rect_coverage(px, py, rect, [effective_radius; 4]);
                    if coverage > 0.0 {
                        blend_pixel_clipped!(
                            blend, clip, frame, size.width, px, py, color, coverage
                        );
                    }
                }
            }
        }
    }
    /// Fill rounded-rectangle with stronger anti-aliasing sampling.
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage =
                    rounded_rect_coverage_grid(px, py, rect, [effective_radius; 4], sample_grid);
                if coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        coverage
                    );
                }
            }
        }
    }
    /// Draws a rounded rectangle stroke with explicit width.
    pub fn draw_rounded_rect_with_width(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radius = effective_radius.saturating_sub(stroke_width);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage = rounded_rect_coverage(px, py, rect, [effective_radius; 4]);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage(px, py, inner, [inner_radius; 4])
                } else {
                    0.0
                };
                let stroke_coverage = (outer_coverage - inner_coverage).clamp(0.0, 1.0);
                if stroke_coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        stroke_coverage
                    );
                }
            }
        }
    }
    /// Draw rounded-rectangle stroke with stronger anti-aliasing sampling.
    pub fn draw_rounded_rect_aa_with_width(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radius = effective_radius.saturating_sub(stroke_width);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage =
                    rounded_rect_coverage_grid(px, py, rect, [effective_radius; 4], sample_grid);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage_grid(px, py, inner, [inner_radius; 4], sample_grid)
                } else {
                    0.0
                };
                let stroke_coverage = (outer_coverage - inner_coverage).clamp(0.0, 1.0);
                if stroke_coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        stroke_coverage
                    );
                }
            }
        }
    }
    /// Fills a rounded rectangle whose corners may differ.
    ///
    /// STY-01: four equal radii are the historical [`Self::fill_rounded_rect`]
    /// byte-for-byte (the band-optimized solver runs unchanged); distinct corners
    /// take the per-pixel coverage path with the same 2×2 sampler the uniform
    /// corner bands use.
    pub fn fill_rounded_rect_radii(&mut self, rect: Rect, radii: [u32; 4], color: Color) {
        match uniform_radii(radii) {
            Some(radius) => self.fill_rounded_rect(rect, radius, color),
            None => self.fill_radii_sampled(rect, radii, color, 2),
        }
    }
    /// Anti-aliased [`Self::fill_rounded_rect_radii`].
    pub fn fill_rounded_rect_radii_aa(&mut self, rect: Rect, radii: [u32; 4], color: Color) {
        match uniform_radii(radii) {
            Some(radius) => self.fill_rounded_rect_aa(rect, radius, color),
            None => {
                let grid = self.aa_samples_per_axis;
                self.fill_radii_sampled(rect, radii, color, grid);
            }
        }
    }
    /// The per-pixel body shared by the non-uniform fill paths.
    fn fill_radii_sampled(&mut self, rect: Rect, radii: [u32; 4], color: Color, grid: u8) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = rounded_rect_coverage_grid(px, py, rect, radii, grid);
                if coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        coverage
                    );
                }
            }
        }
    }
    /// Strokes a rounded rectangle whose corners may differ.
    ///
    /// Four equal radii are the historical [`Self::draw_rounded_rect_with_width`]
    /// byte-for-byte; distinct corners resolve their radii per CSS
    /// `border-radius` on the outer rect and inset each corner by the stroke
    /// width for the inner shape, matching the uniform inner-radius arithmetic.
    pub fn draw_rounded_rect_radii_with_width(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        stroke_width: u32,
    ) {
        match uniform_radii(radii) {
            Some(radius) => self.draw_rounded_rect_with_width(rect, radius, color, stroke_width),
            None => self.stroke_radii_sampled(rect, radii, color, stroke_width, 2),
        }
    }
    /// Anti-aliased [`Self::draw_rounded_rect_radii_with_width`].
    pub fn draw_rounded_rect_radii_aa_with_width(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        stroke_width: u32,
    ) {
        match uniform_radii(radii) {
            Some(radius) => self.draw_rounded_rect_aa_with_width(rect, radius, color, stroke_width),
            None => {
                let grid = self.aa_samples_per_axis;
                self.stroke_radii_sampled(rect, radii, color, stroke_width, grid);
            }
        }
    }
    /// The per-pixel body shared by the non-uniform stroke paths.
    fn stroke_radii_sampled(
        &mut self,
        rect: Rect,
        radii: [u32; 4],
        color: Color,
        stroke_width: u32,
        grid: u8,
    ) {
        if stroke_width == 0 || rect.width == 0 || rect.height == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.right() - 1).min(width - 1);
        let y1 = (rect.bottom() - 1).min(height - 1);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radii =
            rounded_rect_effective_radii(rect, radii).map(|r| r.saturating_sub(stroke_width));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage = rounded_rect_coverage_grid(px, py, rect, radii, grid);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage_grid(px, py, inner, inner_radii, grid)
                } else {
                    0.0
                };
                let stroke_coverage = (outer_coverage - inner_coverage).clamp(0.0, 1.0);
                if stroke_coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        stroke_coverage
                    );
                }
            }
        }
    }
    /// Draws a 1px line segment.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.draw_line_with_width(from, to, color, 1);
    }
    /// Draws a line segment with explicit stroke width.
    ///
    /// # Why the axis-aligned cases do not walk Bresenham's stamps
    ///
    /// The brush is a `stroke_width`-square stamped at every point of the Bresenham walk, so a wide
    /// line used to pay `stroke_width^2` clipped, blend-dispatched pixel writes per step — a 20 px
    /// line cost 400 writes for each pixel of its length (G-09). When the segment is horizontal or
    /// vertical each step moves exactly one pixel along that axis, so consecutive stamps overlap and
    /// the union is the brush box of the first point unioned with the brush box of the last.
    ///
    /// That union is only the same picture as the stamps when one write is idempotent — an opaque
    /// `Normal` write — because the overlap means the old loop painted those pixels **twice**, and a
    /// second composite (a translucent colour, a `Screen`/`Multiply` backdrop) does not repeat the
    /// first. So the span path is taken for the idempotent case only, and everything else runs the
    /// stamp walk below, which paints each stamp once in the same order as before. The stamp walk
    /// itself is not per-pixel any more: one stamp is one solid rectangle, so the clip and the blend
    /// dispatch are resolved once per stamp rather than once per brush pixel.
    pub fn draw_line_with_width(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 {
            return;
        }
        let size = self.buffer.size();
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let blend = self.blend_mode;
        let brush_start = -(stroke_width as i32 / 2);
        let idempotent = matches!(blend, BlendMode::Normal) && color.a == 255;
        if idempotent && (from.y == to.y || from.x == to.x) {
            // The brush covers `[brush_start, brush_start + stroke_width)` around each point, and
            // consecutive points are one pixel apart, so the union runs from the first stamp's near
            // edge to the last stamp's far edge.
            let x_start = from.x.min(to.x);
            let x_end = from.x.max(to.x);
            let y_start = from.y.min(to.y);
            let y_end = from.y.max(to.y);
            let span_width = (x_end as i64 - x_start as i64)
                .saturating_add(stroke_width as i64)
                .clamp(0, u32::MAX as i64) as u32;
            let span_height = (y_end as i64 - y_start as i64)
                .saturating_add(stroke_width as i64)
                .clamp(0, u32::MAX as i64) as u32;
            write_solid_bounds(
                blend,
                clip,
                frame,
                (size.width, size.height),
                (
                    x_start.saturating_add(brush_start),
                    y_start.saturating_add(brush_start),
                    span_width,
                    span_height,
                ),
                color,
            );
            return;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            write_solid_bounds(
                blend,
                clip,
                frame,
                (size.width, size.height),
                (
                    x0.saturating_add(brush_start),
                    y0.saturating_add(brush_start),
                    stroke_width,
                    stroke_width,
                ),
                color,
            );
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err * 2;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
    /// Draw anti-aliased line using configurable sample-grid coverage.
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.draw_line_aa_with_width(from, to, color, 1);
    }
    /// Draw anti-aliased line with configurable stroke width.
    pub fn draw_line_aa_with_width(
        &mut self,
        from: Point,
        to: Point,
        color: Color,
        stroke_width: u32,
    ) {
        if stroke_width == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let half = stroke_width as f32 / 2.0;
        let pad = half.ceil() as i32 + 1;
        let min_x = from.x.min(to.x).saturating_sub(pad).max(0);
        let max_x = (from.x.max(to.x) + pad).min(width - 1);
        let min_y = from.y.min(to.y).saturating_sub(pad).max(0);
        let max_y = (from.y.max(to.y) + pad).min(height - 1);
        let (min_x, min_y, max_x, max_y) = clip_pixel_box(clip, min_x, min_y, max_x, max_y);
        let ax = from.x as f32;
        let ay = from.y as f32;
        let bx = to.x as f32;
        let by = to.y as f32;
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let coverage = line_stroke_coverage_grid(px, py, ax, ay, bx, by, half, sample_grid);
                if coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        coverage
                    );
                }
            }
        }
    }
    /// Fills a circle with a solid color.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        if radius == 0 {
            return;
        }
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let r = radius as i32;
        // `r * r` overflows `i32` for radii above ~46k, which a caller can reach through the
        // public `FillCircle` command. Compute the squared radius once in `i64` so the loop's
        // arithmetic matches `fill_circle_aa`, which already works in floating point.
        let rr = r as i64 * r as i64;
        for y in -r..=r {
            let y2 = y as i64 * y as i64;
            if y2 > rr {
                continue;
            }
            let span = ((rr - y2) as f64).sqrt() as i32;
            let py = center.y + y;
            if py < 0 || py >= height {
                continue;
            }
            for x in -span..=span {
                let px = center.x + x;
                if px < 0 || px >= width {
                    continue;
                }
                set_pixel_clipped!(self.blend_mode, clip, frame, size.width, px, py, color);
            }
        }
    }
    /// Fills a circle using anti-aliased coverage.
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        if radius == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let r = radius as f32;
        let x0 = (center.x - radius as i32 - 1).max(0);
        let y0 = (center.y - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let y1 = (center.y + radius as i32 + 1).min(height - 1);
        let (x0, y0, x1, y1) = clip_pixel_box(clip, x0, y0, x1, y1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = circle_fill_coverage_grid(px, py, center, r, sample_grid);
                if coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        coverage
                    );
                }
            }
        }
    }
    /// Draws a 1px circle stroke.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.draw_circle_with_width(center, radius, color, 1);
    }
    /// Draws a circle stroke with explicit width.
    pub fn draw_circle_with_width(
        &mut self,
        center: Point,
        radius: u32,
        color: Color,
        stroke_width: u32,
    ) {
        if radius == 0 {
            return;
        }
        if stroke_width == 0 {
            return;
        }
        let sample_grid = self.aa_samples_per_axis;
        let size = self.buffer.size();
        let width = size.width as i32;
        let height = size.height as i32;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let ring_radius = radius as f32;
        let x0 = (center.x - radius as i32 - 1).max(0);
        let y0 = (center.y - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let y1 = (center.y + radius as i32 + 1).min(height - 1);
        let (x0, y0, x1, y1) = clip_pixel_box(clip, x0, y0, x1, y1);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let stroke_coverage = circle_stroke_coverage_grid(
                    px,
                    py,
                    center,
                    ring_radius,
                    stroke_width as f32,
                    sample_grid,
                );
                if stroke_coverage > 0.0 {
                    blend_pixel_clipped!(
                        self.blend_mode,
                        clip,
                        frame,
                        size.width,
                        px,
                        py,
                        color,
                        stroke_coverage
                    );
                }
            }
        }
    }
    /// Draws an arc (partial circle).
    ///
    /// The command-level angles are normalised to `(start, sweep)` and the arc is then walked
    /// by [`for_each_arc_sample`], the same kernel [`crate::render::draw_arc_segments`] uses.
    /// The old path sampled with a fixed `0.02` rad step into a `Vec` allocated on every call;
    /// it also read a whole turn (start 0, end 2π) as a zero sweep and painted a single line
    /// instead of a circle. Both defects are gone because there is no second sampler left to
    /// drift: a stroked arc is the polyline through the kernel's samples, a filled arc is the
    /// triangle fan from the centre over those same samples.
    pub fn draw_arc(
        &mut self,
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        filled: bool,
    ) {
        if radius == 0 {
            return;
        }
        let (start, sweep) = normalized_arc_sweep(start_angle, end_angle);
        if sweep < 0.001 {
            return;
        }
        let end = start + sweep;
        let full_turn = sweep >= core::f32::consts::TAU - 0.001;

        let mut first: Option<Point> = None;
        let mut previous: Option<Point> = None;
        for_each_arc_sample(center, radius as f32, start, end, |current| {
            if first.is_none() {
                first = Some(current);
            }
            if let Some(previous) = previous {
                if filled {
                    self.fill_triangle(center, previous, current, color);
                } else {
                    self.draw_line_with_width(previous, current, color, 1);
                }
            }
            previous = Some(current);
        });

        // A whole turn closes on itself. The kernel samples the end angle at `start + 2π`,
        // which rounds onto the first pixel and is therefore dropped as a degenerate chord;
        // close the loop explicitly so a full circle has neither a one-chord gap nor a missing
        // wedge in the fan.
        if full_turn {
            if let (Some(first), Some(previous)) = (first, previous) {
                if first != previous {
                    if filled {
                        self.fill_triangle(center, previous, first, color);
                    } else {
                        self.draw_line_with_width(previous, first, color, 1);
                    }
                }
            }
        }
    }
    /// Draws a path defined by a list of points.
    pub fn draw_path(
        &mut self,
        points: &[Point],
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        if points.len() < 2 {
            return;
        }
        if filled {
            self.fill_polygon(points, color);
        } else {
            for i in 0..points.len().saturating_sub(1) {
                self.draw_line_with_width(points[i], points[i + 1], color, width);
            }
            if closed {
                self.draw_line_with_width(points[points.len() - 1], points[0], color, width);
            }
        }
    }
    /// Fills all packed contours as one non-zero winding shape.
    pub fn fill_path_non_zero(&mut self, path: &crate::render::CompoundPath, color: Color) {
        self.fill_path_non_zero_borrowed(path.points(), path.contours(), color, 0, 0)
            .expect("CompoundPath owns validated contour ranges");
    }

    /// Fills borrowed contours after applying a translation, without building an owning path.
    pub(crate) fn fill_path_non_zero_borrowed(
        &mut self,
        points: &[Point],
        contours: &[(usize, usize)],
        color: Color,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), crate::render::CompoundPathError> {
        self.fill_scanlines(points, contours, color, offset_x, offset_y, FillRule::NonZero)
    }

    /// Fills a polygon using a scanline algorithm.
    ///
    /// A polygon is one contour, so it goes through the same [`Self::fill_scanlines`] kernel
    /// as `fill_path_non_zero_borrowed`; only the rule differs. `fill_polygon` is the
    /// even-odd entry point (`draw_path(.., filled: true)`), while compound paths use the
    /// non-zero rule. Keeping one edge/intersection/span loop means a fix or a speed-up in
    /// scan conversion lands in both callers instead of having to be made twice.
    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        if points.len() < 3 {
            return;
        }
        self.fill_scanlines(points, &[(0, points.len())], color, 0, 0, FillRule::EvenOdd)
            .expect("a single in-range contour is a valid path");
    }

    /// Scan-converts `contours` with the chosen [`FillRule`].
    ///
    /// Crossings are collected once per scanline into the reused `nonzero_intersections`
    /// scratch buffer as `(x, winding_delta)`, sorted, and swept. Grouping equal `x` values
    /// before advancing the winding is what makes a vertex that lands exactly on a scanline
    /// count once rather than twice: a tangent at a local minimum contributes `+1` and `-1`
    /// at the same `x`, so the net winding — and the even-odd parity — both stay unchanged and
    /// no zero-width span is painted.
    fn fill_scanlines(
        &mut self,
        points: &[Point],
        contours: &[(usize, usize)],
        color: Color,
        offset_x: i32,
        offset_y: i32,
        rule: FillRule,
    ) -> Result<(), crate::render::CompoundPathError> {
        crate::render::CompoundPath::validate_ranges(points.len(), contours)?;
        let size = self.buffer.size();
        let clip = self.current_clip();
        let intersections = &mut self.nonzero_intersections;
        if intersections.capacity() < points.len() {
            intersections.reserve(points.len());
        }
        let frame = self.buffer.back_mut();
        let min_y = contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|point| point.y + offset_y)
            .min()
            .unwrap_or(0)
            .max(0);
        let max_y = contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|point| point.y + offset_y)
            .max()
            .unwrap_or(0)
            .min(size.height as i32 - 1);

        for y in min_y..=max_y {
            intersections.clear();
            for &(start, end) in contours {
                for index in start..end {
                    let first = points[index];
                    let second = points[if index + 1 == end { start } else { index + 1 }];
                    let first_y = first.y + offset_y;
                    let second_y = second.y + offset_y;
                    let winding = if first_y <= y && second_y > y {
                        1
                    } else if second_y <= y && first_y > y {
                        -1
                    } else {
                        continue;
                    };
                    let first_x = first.x + offset_x;
                    let second_x = second.x + offset_x;
                    let t = (y - first_y) as f32 / (second_y - first_y) as f32;
                    let x = first_x as f32 + (second_x - first_x) as f32 * t;
                    intersections.push((x, winding));
                }
            }
            intersections.sort_by(|left, right| {
                left.0.partial_cmp(&right.0).unwrap_or(core::cmp::Ordering::Equal)
            });

            let mut index = 0;
            let mut winding = 0;
            let mut previous_x: Option<f32> = None;
            while index < intersections.len() {
                let x = intersections[index].0;
                if rule.contains(winding) {
                    if let Some(previous) = previous_x {
                        let x_start = previous.ceil() as i32;
                        let x_end = x.floor() as i32;
                        for pixel_x in x_start.max(0)..=x_end.min(size.width as i32 - 1) {
                            set_pixel_clipped!(
                                self.blend_mode,
                                clip,
                                frame,
                                size.width,
                                pixel_x as u32,
                                y as u32,
                                color
                            );
                        }
                    }
                }

                let mut delta = 0;
                while index < intersections.len() && intersections[index].0 == x {
                    delta += intersections[index].1;
                    index += 1;
                }
                winding += delta;
                previous_x = Some(x);
            }
        }
        Ok(())
    }
    /// Fills a quadrilateral with per-pixel coverage taken from its projected edges.
    ///
    /// This is the rasteriser the pseudo-3D `3d` path uses for a perspective-divided quad: the
    /// four corners come from the batch replay's `FillQuadAA`, where opposite edges need not be
    /// parallel. The quad is split along the `0–2` diagonal into two triangles, each sampled on a
    /// [`QUAD_AA_SAMPLES`]-by-[`QUAD_AA_SAMPLES`] grid; a sample counts when it lies inside the
    /// triangle under a top-left fill rule, so the shared diagonal is covered exactly once. The
    /// resulting coverage is applied through the same blend-aware pixel write every other
    /// primitive uses.
    ///
    /// # Why not reuse `fill_polygon`
    ///
    /// `fill_polygon` is a scanline fill: it rounds a span's ends to whole pixels, so a sloped
    /// projected edge becomes a stair-step. That is correct for an affine quad whose corners are
    /// already rounded, and visibly wrong for the near and far edges of a turned card, which are
    /// the two edges the perspective divide makes non-parallel.
    #[cfg(feature = "3d")]
    pub fn fill_quad_aa(&mut self, corners: [Point; 4], color: Color) {
        self.fill_triangle_aa(corners[0], corners[1], corners[2], color);
        self.fill_triangle_aa(corners[0], corners[2], corners[3], color);
    }

    /// One antialiased triangle of [`Self::fill_quad_aa`].
    #[cfg(feature = "3d")]
    fn fill_triangle_aa(&mut self, a: Point, b: Point, c: Point, color: Color) {
        let size = self.buffer.size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let ax = a.x as f32;
        let ay = a.y as f32;
        let mut bx = b.x as f32;
        let mut by = b.y as f32;
        let mut cx = c.x as f32;
        let mut cy = c.y as f32;
        let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        if area.abs() < f32::EPSILON {
            return;
        }
        // Normalise the winding so "inside" is `edge >= 0` for all three edges. The sign only
        // decides which triangle owns a shared boundary sample, never which pixels are inside.
        if area < 0.0 {
            core::mem::swap(&mut bx, &mut cx);
            core::mem::swap(&mut by, &mut cy);
        }
        let min_x = a.x.min(b.x).min(c.x).max(0);
        let min_y = a.y.min(b.y).min(c.y).max(0);
        let max_x = a.x.max(b.x).max(c.x).min(size.width as i32 - 1);
        let max_y = a.y.max(b.y).max(c.y).min(size.height as i32 - 1);
        if max_x < min_x || max_y < min_y {
            return;
        }
        let clip = self.current_clip();
        let blend = self.blend_mode;
        let width = size.width;
        let frame = self.buffer.back_mut();
        let sample_step = 1.0 / QUAD_AA_SAMPLES as f32;
        let sample_count = (QUAD_AA_SAMPLES * QUAD_AA_SAMPLES) as f32;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if !pixel_visible(clip, x, y) {
                    continue;
                }
                let mut inside = 0u32;
                for sy in 0..QUAD_AA_SAMPLES {
                    let py = y as f32 + (sy as f32 + 0.5) * sample_step;
                    for sx in 0..QUAD_AA_SAMPLES {
                        let px = x as f32 + (sx as f32 + 0.5) * sample_step;
                        if inside_top_left(ax, ay, bx, by, cx, cy, px, py) {
                            inside += 1;
                        }
                    }
                }
                if inside == 0 {
                    continue;
                }
                let coverage = inside as f32 / sample_count;
                write_pixel_with_mode(blend, frame, width, x as u32, y as u32, color, coverage);
            }
        }
    }

    /// Fills a triangle using scanline rasterization (no AA).
    fn fill_triangle(&mut self, v0: Point, v1: Point, v2: Point, color: Color) {
        let mut vs = [v0, v1, v2];
        vs.sort_by_key(|v| v.y);
        let [a, b, c] = vs;
        let size = self.buffer.size();
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();

        if a.y == c.y {
            let min_x = a.x.min(b.x).min(c.x);
            let max_x = a.x.max(b.x).max(c.x);
            if max_x > min_x {
                for x in min_x.max(0)..=max_x.min(size.width as i32 - 1) {
                    set_pixel_clipped!(self.blend_mode, clip, frame, size.width, x, a.y, color);
                }
            }
            return;
        }
        let total_height = c.y - a.y;
        if total_height <= 0 {
            return;
        }
        let lerp_x =
            |p1: Point, p2: Point, t: f32| -> f32 { p1.x as f32 + (p2.x - p1.x) as f32 * t };
        for y in a.y..=c.y {
            let (x1, x2) = if y < b.y {
                let sub_h = b.y - a.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(a, b, (y - a.y) as f32 / sub_h as f32),
                )
            } else {
                let sub_h = c.y - b.y;
                if sub_h == 0 {
                    continue;
                }
                (
                    lerp_x(a, c, (y - a.y) as f32 / total_height as f32),
                    lerp_x(b, c, (y - b.y) as f32 / sub_h as f32),
                )
            };
            if y < 0 || y as u32 >= size.height {
                continue;
            }
            let x_start = x1.min(x2).ceil() as i32;
            let x_end = x1.max(x2).floor() as i32;
            if x_end >= x_start {
                let x_start = x_start.max(0);
                let x_end = x_end.min(size.width as i32 - 1);
                for x in x_start..=x_end {
                    set_pixel_clipped!(self.blend_mode, clip, frame, size.width, x, y, color);
                }
            }
        }
    }
    /// Draws text using the current text raster fallback path.
    pub fn draw_text(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        // Opt-in glyph-level painting. It is tried first and falls back to the cluster renderer
        // when the build has no face for the named family, so the default path (and every
        // existing snapshot) is untouched.
        #[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
        if self.glyph_run_text && self.draw_text_glyph_run(origin, text, font, color, alignment) {
            return;
        }
        // # One shaping pass, not two
        //
        // This used to call `measure_text` (which shapes the line) and then `shape_text` again,
        // so every label shaped its clusters twice per frame. The line box below comes from the
        // same `TextMetrics::for_font` `measure_text` uses, and the width is the shaped advance
        // rounded the same way, so the numbers are identical to the previous form.
        let shaped = self.shape_text(text, font);
        let scale = self.dpi_scale();
        let line = crate::render::TextMetrics::for_font(font, scale);
        let width = shaped.advance().round() as u32;
        if width == 0 || line.height == 0 {
            return;
        }
        let metrics = crate::render::TextMetrics { width, ..line };
        let adjusted_origin_x = match alignment {
            HorizontalAlignment::Left => origin.x,
            HorizontalAlignment::Center => origin.x - (metrics.width / 2) as i32,
            HorizontalAlignment::Right => origin.x - metrics.width as i32,
        };
        let mut pen_x = adjusted_origin_x as f32;
        let glyph_height = metrics.height.max(1) as i32;
        let size = self.buffer.size();
        let clip = self.current_clip();
        // The tracking is a **gap between clusters**, not a trailing space: paying it after the last
        // cluster would make a centred label sit left of centre and a right-aligned one fall short of
        // its box. `shape_text` counts `clusters.len() - 1` gaps for the same reason, so the two
        // readings of one run agree.
        let tracking = font.letter_spacing() * scale;
        let last_index = shaped.clusters().len().saturating_sub(1);
        // One scratch for the whole line, and for every line after it: a glyph is rasterised into
        // it, blended, and overwritten by the next — so no glyph is ever resident. It is grown
        // **when a wider cell appears** instead of being sized by a second pass over the clusters:
        // that scan computed a maximum the loop then re-derived per glyph, so every line paid two
        // walks over its clusters to size one buffer (G-15).
        let mut coverage = core::mem::take(&mut self.text_coverage_scratch);
        // The largest cell painted into the buffer so far. It only ever grows, so a short cell never
        // shrinks the allocation and the next line — and the next frame — reuses its capacity.
        let mut max_cell = coverage.len();
        let frame = self.buffer.back_mut();
        // A **second** scratch, four bytes per pixel, used only when the active stack resolves a
        // cluster through a colour face. It lives in a thread-local slot, so it keeps its capacity
        // across lines and frames rather than being allocated once per line (G-15). Keeping it
        // separate from `coverage` rather than sizing `coverage` at four bytes per pixel
        // unconditionally is what keeps the 8x8 path's allocation unchanged — every existing
        // snapshot stays byte-identical.
        #[cfg(feature = "fonts-emoji-color")]
        let mut color_scratch = take_color_scratch();
        for (index, cluster) in shaped.clusters().iter().enumerate() {
            let glyph_width = cluster.advance.max(1.0).round() as i32;
            let display_char = cluster
                .text
                .chars()
                .find(|ch| !is_combining_mark(*ch) && !is_variation_selector(*ch));
            if let Some(ch) = display_char {
                let cell_len = (glyph_width as usize).saturating_mul(glyph_height as usize);
                if cell_len > max_cell {
                    coverage.resize(cell_len, 0);
                    max_cell = cell_len;
                }
                #[cfg(feature = "fonts-emoji-color")]
                {
                    let color_len = cell_len.saturating_mul(4);
                    if color_scratch.len() < color_len {
                        color_scratch.resize(color_len, 0);
                    }
                }
                let mut config = GlyphDrawConfig {
                    canvas: &mut *frame,
                    canvas_width: size.width,
                    canvas_height: size.height,
                    w: glyph_width as u32,
                    h: glyph_height as u32,
                    color,
                    clip,
                    // The line above measured its clusters at this weight, so the glyph must be
                    // painted at it too — a variable face is one face with a `wght` axis. Italic
                    // has no axis in the rasteriser, so the paint path applies a synthetic
                    // oblique (G-18).
                    weight: font.weight(),
                    italic: font.is_italic(),
                };
                // Colour ink needs the four-byte scratch, and the two calls are mutually exclusive
                // by what each one's buffer means: `blend_color_glyph` refuses coverage ink and
                // `blend_painted_glyph` refuses colour ink, so trying one and then the other draws
                // each cluster exactly once whichever kind of face answered for it.
                #[cfg(feature = "fonts-emoji-color")]
                let drawn = blend_color_glyph(
                    ch,
                    pen_x.round() as i32,
                    origin.y,
                    &mut color_scratch,
                    &mut config,
                );
                #[cfg(not(feature = "fonts-emoji-color"))]
                let drawn = false;
                // The glyph atlas answers only the requests it can reproduce byte for byte — see
                // `blend_glyph_from_atlas` for exactly which. Everything else falls through to the
                // direct paint below, which is unchanged (G-202).
                #[cfg(feature = "text-shaping")]
                let drawn = drawn
                    || blend_glyph_from_atlas(
                        ch,
                        pen_x.round() as i32,
                        origin.y,
                        font.family(),
                        self.frame_number,
                        &mut config,
                    );
                if !drawn {
                    blend_painted_glyph(
                        ch,
                        pen_x.round() as i32,
                        origin.y,
                        &mut coverage,
                        &mut config,
                    );
                }
            }
            pen_x += cluster.advance;
            if index < last_index {
                pen_x += tracking;
            }
        }
        // Give both scratches back for the next line and the next frame.
        self.text_coverage_scratch = coverage;
        #[cfg(feature = "fonts-emoji-color")]
        return_color_scratch(color_scratch);
    }

    /// Paints `text` as a run of shaped glyph ids, with the face's ligatures enabled.
    ///
    /// Returns `false` when this build has no face for `font.family()`; the caller then uses the
    /// cluster renderer. Returns `true` for a degenerate run as well (an empty string or a
    /// zero-height line box draws nothing but has still been handled).
    ///
    /// Each glyph is outlined at the run's line-box height and filled through the same non-zero
    /// polygon path [`Self::fill_path_non_zero`] uses, translated to the position the shaper gave
    /// it. That is what makes a ligature possible: the shaper may return one glyph for several
    /// characters, and painting glyph ids draws that one shape instead of the characters.
    /// `letter_spacing` is added between glyphs, so a tracked font keeps its spacing.
    #[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
    fn draw_text_glyph_run(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) -> bool {
        use crate::render::text::{outline_glyph_at_weight, shaping, Cell};

        let scale = self.dpi_scale();
        let weight = font.weight();
        let Some(run) = shaping::glyph_run_for_family_at_weight(text, font, scale, weight) else {
            return false;
        };
        // G-019: this path outlines each shaped glyph instead of consulting the coverage atlas,
        // because ligatures are glyph-id shapes the atlas (keyed by character) cannot reproduce.
        // It is opt-in (default off) and bounded: an oversized run falls back to the cluster path,
        // which does use the atlas, rather than paying an unbounded outline cost.
        if run.glyph_ids.len() > GLYPH_RUN_MAX_GLYPHS {
            return false;
        }
        let line = crate::render::TextMetrics::for_font(font, scale);
        if line.height == 0 {
            return true;
        }
        let tracking = font.letter_spacing() * scale;
        let glyph_count = run.glyph_ids.len();
        let total_width = run.width + tracking * glyph_count.saturating_sub(1) as f32;
        let width = total_width.round().max(0.0) as u32;
        let origin_x = match alignment {
            HorizontalAlignment::Left => origin.x,
            HorizontalAlignment::Center => origin.x - (width / 2) as i32,
            HorizontalAlignment::Right => origin.x - width as i32,
        };
        let cell = Cell::new(width.max(1), line.height);
        // Taken from the thread's outline slot and put back when the run ends: the three buffers used
        // to be allocated per call, i.e. three per label per frame, for data that is not per-call
        // (G-14).
        let mut scratch = take_outline_scratch();
        for (index, (&glyph_id, &(x_offset, y_offset))) in
            run.glyph_ids.iter().zip(run.positions.iter()).enumerate()
        {
            let Some(count) = outline_glyph_at_weight(
                glyph_id as u16,
                cell,
                font.family(),
                weight,
                &mut scratch.points,
                &mut scratch.contours,
            ) else {
                continue;
            };
            let vertex_count =
                scratch.contours[..count].iter().map(|&(_, end)| end).max().unwrap_or(0);
            if vertex_count == 0 {
                continue;
            }
            scratch.polygon.clear();
            scratch.polygon.extend(scratch.points[..vertex_count].iter().map(|point| {
                let x = origin_x as f32 + x_offset + tracking * index as f32 + point.x;
                let y = origin.y as f32 + y_offset + point.y;
                Point::new(x.round() as i32, y.round() as i32)
            }));
            let _ = self.fill_path_non_zero_borrowed(
                &scratch.polygon,
                &scratch.contours[..count],
                color,
                0,
                0,
            );
        }
        return_outline_scratch(scratch);
        true
    }
    /// Draws an RGBA image at the specified position and size.
    /// Pixels are copied directly from the source data, performing alpha blending.
    ///
    /// # Why the destination rectangle is intersected first
    ///
    /// Every pixel used to re-test the surface bounds and the clip. A blit is the hottest loop a
    /// software backend has — the tree painter composites one per widget layer per frame — and the
    /// per-pixel tests cost more than the blend itself for an opaque image. The visible destination
    /// rectangle is therefore computed once, and only that rectangle is walked.
    ///
    /// # Why an opaque row is a `memcpy`
    ///
    /// An opaque run has no compositing to do: the result is the source bytes. Testing the row's
    /// alpha channel costs one byte compare per pixel and short-circuits on the first translucent
    /// one, so the common case (an icon, a widget layer captured over a background) becomes one
    /// `copy_from_slice` per row. The blend path is unchanged for everything else, so output is
    /// byte-identical either way.
    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        let size = self.buffer.size();
        if width == 0 || height == 0 {
            return;
        }
        // The expected length is computed with checked `usize` arithmetic: a bare
        // `width * height * 4` overflowed on `(1 << 31, 1 << 31)` in debug (panic)
        // and wrapped to 0 in release, so the length guard passed an under-sized
        // buffer and the row loop below indexed out of bounds. `None` means the
        // rectangle cannot describe any real buffer, so the whole blit is skipped —
        // the same degradation an over-short buffer already gets, with no change to
        // the normal path.
        let Some(expected_len) =
            (width as usize).checked_mul(height as usize).and_then(|pixels| pixels.checked_mul(4))
        else {
            return;
        };
        if data.len() < expected_len {
            return;
        }
        // From here `data.len() >= width * height * 4`, and `visible_bounds` keeps
        // `row - y < height`, `left - x <= width` and `left - x + columns <= width`,
        // so every source slice below is provably inside `data`.
        let clip = self.current_clip();
        let screen_width = size.width;
        let screen_height = size.height;
        let Some((left, top, right, bottom)) =
            visible_bounds(x, y, width, height, (screen_width, screen_height), clip)
        else {
            return;
        };
        let stride = width as usize;
        let frame = self.buffer.back_mut();
        for row in top..bottom {
            let columns = (right - left) as usize;
            let source_start = (row - y) as usize * stride + (left - x) as usize;
            let source = &data[source_start * 4..(source_start + columns) * 4];
            let destination_start = (row as u32 * screen_width + left as u32) as usize * 4;
            let destination = &mut frame[destination_start..destination_start + columns * 4];
            // Walk the row in **runs**: an opaque run is a plain copy and only the translucent
            // pixels pay for the composite. A captured widget layer is transparent at its edges and
            // opaque inside, so the interior of the row takes the copy path while the outline
            // still blends — one test per pixel either way, but no blend arithmetic for the bulk.
            let source_pixels = source.as_chunks::<4>().0;
            let destination_pixels = destination.as_chunks_mut::<4>().0;
            let mut index = 0;
            while index < source_pixels.len() {
                if source_pixels[index][3] == 255 {
                    let mut end = index;
                    while end < source_pixels.len() && source_pixels[end][3] == 255 {
                        end += 1;
                    }
                    for (target, pixel) in
                        destination_pixels[index..end].iter_mut().zip(&source_pixels[index..end])
                    {
                        *target = *pixel;
                    }
                    index = end;
                } else {
                    blend_source_over(&mut destination_pixels[index], source_pixels[index]);
                    index += 1;
                }
            }
        }
    }

    /// Draws an RGBA image mapped onto the quadrilateral spanned by `corners`.
    ///
    /// `corners` are the destination positions of the source grid's top-left, top-right,
    /// bottom-right and bottom-left corners, in perimeter order. `source_width` and
    /// `source_height` describe the grid the data was laid out in before the transform, which is
    /// the rectangle the batch path drew under a 2D transform.
    ///
    /// # Parallelograms and projective quads
    ///
    /// An affine transform maps a rectangle to a parallelogram, so the destination point
    /// `p = p00 + u * (p10 - p00) + v * (p01 - p00)` inverts with the determinant of the two edge
    /// vectors. A perspective divide produces a general quadrilateral instead; when the fourth
    /// corner is not implied by the other three, the mapping is solved as a homography from the
    /// unit square so the texture follows the projected edges (a turned card's texture narrows
    /// toward its far edge, as it should). Both paths sample the source grid bilinearly.
    ///
    /// Sampling is **bilinear**: a rotated or scaled image is resampled, not block-copied. At an
    /// identity transform the destination pixel centres land exactly on source texel centres, so
    /// the weights are `0`/`1` and the result is bit-for-bit the axis-aligned copy — the
    /// interpolation can only soften a picture the transform actually moved.
    pub fn draw_image_quad(
        &mut self,
        corners: [Point; 4],
        source_width: u32,
        source_height: u32,
        data: &[u8],
    ) {
        if source_width == 0 || source_height == 0 {
            return;
        }
        let expected_len = (source_width as usize) * (source_height as usize) * 4;
        if data.len() < expected_len {
            return;
        }
        if !corners_are_parallelogram(&corners) {
            self.draw_image_quad_projective(corners, source_width, source_height, data);
            return;
        }
        let [p00, p10, _, p01] = corners;
        let ux = (p10.x - p00.x) as f32;
        let uy = (p10.y - p00.y) as f32;
        let vx = (p01.x - p00.x) as f32;
        let vy = (p01.y - p00.y) as f32;
        let det = ux * vy - uy * vx;
        // A zero-area parallelogram has no interior to fill, and the inverse below would divide
        // by zero and write NaNs into the sampling index.
        if det.abs() < f32::EPSILON {
            return;
        }
        let min_x = corners.iter().map(|p| p.x).min().unwrap_or(0);
        let min_y = corners.iter().map(|p| p.y).min().unwrap_or(0);
        let max_x = corners.iter().map(|p| p.x).max().unwrap_or(0);
        let max_y = corners.iter().map(|p| p.y).max().unwrap_or(0);

        let size = self.buffer.size();
        let clip = self.current_clip();
        let screen_width = size.width;
        let screen_height = size.height;
        let frame = self.buffer.back_mut();
        // Intersect the quad's bounding box with the screen and the clip once, rather than
        // testing every sample of a mostly-offscreen quad (G-007/G-012).
        let (min_x, min_y, max_x, max_y) = clip_pixel_box(
            clip,
            min_x.max(0),
            min_y.max(0),
            max_x.min(screen_width as i32 - 1),
            max_y.min(screen_height as i32 - 1),
        );
        if max_x < min_x || max_y < min_y {
            return;
        }
        // Pre-multiply the inverse determinant, then step `u`/`v` across a row: the old form
        // evaluated two divides and four multiplies per pixel, for the same linear map (G-012).
        let inv_det = 1.0 / det;
        let u_dx = vy * inv_det;
        let u_dy = -vx * inv_det;
        let v_dx = -uy * inv_det;
        let v_dy = ux * inv_det;
        let row_dx = (min_x - p00.x) as f32 + 0.5;
        // Pixel centres, so a corner pixel samples its own centre rather than its top-left edge.
        for y in min_y..=max_y {
            let dy = (y - p00.y) as f32 + 0.5;
            let mut u = row_dx * u_dx + dy * u_dy;
            let mut v = row_dx * v_dx + dy * v_dy;
            for x in min_x..=max_x {
                if (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v) {
                    let [r, g, b, a] = sample_bilinear(data, source_width, source_height, u, v);
                    blend_source_pixel(frame, screen_width, x as u32, y as u32, r, g, b, a);
                }
                u += u_dx;
                v += v_dx;
            }
        }
    }

    /// Draws an image onto a general quadrilateral with a homography.
    ///
    /// The four corners define a map from the unit square to the quad. Inverting that map once
    /// and evaluating it per destination pixel is what makes the texture follow a perspective
    /// divide: the pixels near the short (far) edge read a wider band of the source, exactly the
    /// way a photographed card does. The parallelogram path in [`Self::draw_image_quad`] is kept
    /// because it is cheaper and exact, and because every affine transform produces it.
    fn draw_image_quad_projective(
        &mut self,
        corners: [Point; 4],
        source_width: u32,
        source_height: u32,
        data: &[u8],
    ) {
        let Some(inverse) = inverse_quad_homography(&corners) else {
            return;
        };
        let size = self.buffer.size();
        let clip = self.current_clip();
        let screen_width = size.width;
        let max_x = corners.iter().map(|p| p.x).max().unwrap_or(0);
        let max_y = corners.iter().map(|p| p.y).max().unwrap_or(0);
        let min_x = corners.iter().map(|p| p.x).min().unwrap_or(0);
        let min_y = corners.iter().map(|p| p.y).min().unwrap_or(0);
        let frame = self.buffer.back_mut();
        for y in min_y..=max_y {
            if y < 0 || y as u32 >= size.height {
                continue;
            }
            for x in min_x..=max_x {
                if x < 0 || x as u32 >= screen_width {
                    continue;
                }
                if !pixel_visible(clip, x, y) {
                    continue;
                }
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let w = inverse[6] * px + inverse[7] * py + inverse[8];
                if w.abs() < f32::EPSILON {
                    continue;
                }
                let u = (inverse[0] * px + inverse[1] * py + inverse[2]) / w;
                let v = (inverse[3] * px + inverse[4] * py + inverse[5]) / w;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let [r, g, b, a] = sample_bilinear(data, source_width, source_height, u, v);
                blend_source_pixel(frame, screen_width, x as u32, y as u32, r, g, b, a);
            }
        }
    }
}

/// Whether the four corners form a parallelogram.
fn corners_are_parallelogram(corners: &[Point; 4]) -> bool {
    let [p00, p10, p20, p01] = corners;
    let implied_x = p10.x as i64 + p01.x as i64 - p00.x as i64;
    let implied_y = p10.y as i64 + p01.y as i64 - p00.y as i64;
    (p20.x as i64 - implied_x).abs() <= 1 && (p20.y as i64 - implied_y).abs() <= 1
}

/// The inverse of the homography that maps the unit square to `corners`.
///
/// The returned array is row-major `[[a, b, c], [d, e, f], [g, h, i]]`, so a destination point
/// `(x, y)` maps to `((a x + b y + c) / w, (d x + e y + f) / w)` with `w = g x + h y + i`. The
/// construction follows the standard unit-square-to-quad solution; a degenerate quad (zero area)
/// answers `None` rather than producing NaNs.
fn inverse_quad_homography(corners: &[Point; 4]) -> Option<[f32; 9]> {
    let [p00, p10, p20, p01] = *corners;
    let (x0, y0) = (p00.x as f32, p00.y as f32);
    let (x1, y1) = (p10.x as f32, p10.y as f32);
    let (x2, y2) = (p20.x as f32, p20.y as f32);
    let (x3, y3) = (p01.x as f32, p01.y as f32);
    let dx1 = x1 - x2;
    let dx2 = x3 - x2;
    let dx3 = x0 - x1 + x2 - x3;
    let dy1 = y1 - y2;
    let dy2 = y3 - y2;
    let dy3 = y0 - y1 + y2 - y3;
    let (a, b, c, d, e, f, g, h);
    if dx3.abs() < f32::EPSILON && dy3.abs() < f32::EPSILON {
        // Affine: the fourth corner is implied, so there is no perspective term.
        a = x1 - x0;
        b = x3 - x0;
        c = x0;
        d = y1 - y0;
        e = y3 - y0;
        f = y0;
        g = 0.0;
        h = 0.0;
    } else {
        let denominator = dx1 * dy2 - dx2 * dy1;
        if denominator.abs() < f32::EPSILON {
            return None;
        }
        g = (dx3 * dy2 - dx2 * dy3) / denominator;
        h = (dx1 * dy3 - dx3 * dy1) / denominator;
        a = x1 - x0 + g * x1;
        b = x3 - x0 + h * x3;
        c = x0;
        d = y1 - y0 + g * y1;
        e = y3 - y0 + h * y3;
        f = y0;
    }
    // Inverse of [[a, b, c], [d, e, f], [g, h, 1]] by the cofactor formula.
    let determinant = a * (e - f * h) - b * (d - f * g) + c * (d * h - e * g);
    if determinant.abs() < f32::EPSILON {
        return None;
    }
    let scale = 1.0 / determinant;
    Some([
        (e - f * h) * scale,
        (c * h - b) * scale,
        (b * f - c * e) * scale,
        (f * g - d) * scale,
        (a - c * g) * scale,
        (c * d - a * f) * scale,
        (d * h - e * g) * scale,
        (b * g - a * h) * scale,
        (a * e - b * d) * scale,
    ])
}

/// Samples an RGBA grid bilinearly at `(u, v)`, both in `0.0..=1.0`.
///
/// The sample point is offset by half a texel so `u == 0` reads the centre of the first texel
/// rather than its top-left corner; that offset is what makes an identity transform reproduce the
/// source exactly. Edge texels are clamped, so a sample at the boundary reads the edge colour
/// instead of wrapping to the opposite side.
///
/// Channels are interpolated in **straight** alpha. For an opaque source this is identical to
/// premultiplied interpolation; for a translucent one it can show a slight seam where alpha goes
/// to zero, which is the honest trade for not carrying a premultiplied copy of every image.
fn sample_bilinear(data: &[u8], width: u32, height: u32, u: f32, v: f32) -> [u8; 4] {
    let fx = u * width as f32 - 0.5;
    let fy = v * height as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let max_x = width.saturating_sub(1) as i32;
    let max_y = height.saturating_sub(1) as i32;
    let clamp_x = |value: f32| (value as i32).clamp(0, max_x) as usize;
    let clamp_y = |value: f32| (value as i32).clamp(0, max_y) as usize;
    let x0i = clamp_x(x0);
    let x1i = clamp_x(x0 + 1.0);
    let y0i = clamp_y(y0);
    let y1i = clamp_y(y0 + 1.0);
    let stride = width as usize;
    let fetch = |x: usize, y: usize| -> [u8; 4] {
        let index = (y * stride + x) * 4;
        [data[index], data[index + 1], data[index + 2], data[index + 3]]
    };
    let top_left = fetch(x0i, y0i);
    let top_right = fetch(x1i, y0i);
    let bottom_left = fetch(x0i, y1i);
    let bottom_right = fetch(x1i, y1i);
    let mut out = [0u8; 4];
    for channel in 0..4 {
        let top =
            top_left[channel] as f32 + (top_right[channel] as f32 - top_left[channel] as f32) * tx;
        let bottom = bottom_left[channel] as f32
            + (bottom_right[channel] as f32 - bottom_left[channel] as f32) * tx;
        let value = top + (bottom - top) * ty;
        out[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Alpha-composites one straight-RGBA source pixel over a row-major RGBA frame buffer.
///
/// Bounds and the clip are the caller's job: every image path checks them on the way in, and
/// checking them twice per pixel is the difference between a blit and a loop.
#[allow(clippy::too_many_arguments)]
fn blend_source_pixel(
    frame: &mut [u8],
    screen_width: u32,
    x: u32,
    y: u32,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
) {
    let index = ((y * screen_width + x) as usize) * 4;
    blend_source_over(&mut frame[index..index + 4], [r, g, b, a]);
}

/// Alpha-composites one straight-RGBA source pixel over one destination pixel.
///
/// Both sides are straight (non-premultiplied) RGBA, and the composite is the W3C source-over
/// formula in that space: the result is the source weighted by its own alpha plus the destination
/// weighted by what the source leaves uncovered, renormalised by the resulting alpha.
///
/// # Why a `&mut [u8]` and not `&mut [u8; 4]`
///
/// The callers hold the destination as a `&mut [u8]` row (or a four-byte slice of one). Taking the
/// slice form keeps the `as_chunks_mut` and `frame[..]` callers from having to convert, and the
/// four-byte precondition is the one thing they already guarantee.
#[inline]
fn blend_source_over(destination: &mut [u8], source: [u8; 4]) {
    if source[3] == 0 {
        return;
    }
    if source[3] == 255 {
        destination[..4].copy_from_slice(&source);
        return;
    }
    let src_alpha = source[3] as f32 / 255.0;
    let dst_alpha = destination[3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return;
    }
    let src_weight = src_alpha / out_alpha;
    let dst_weight = dst_alpha * (1.0 - src_alpha) / out_alpha;
    for channel in 0..3 {
        destination[channel] =
            (source[channel] as f32 * src_weight + destination[channel] as f32 * dst_weight) as u8;
    }
    destination[3] = (out_alpha * 255.0) as u8;
}

/// Samples per axis used by [`SoftwareSurface::fill_quad_aa`].
#[cfg(feature = "3d")]
const QUAD_AA_SAMPLES: u32 = 4;

/// Twice the signed area of `(a, b, p)`, i.e. the edge function of `p` for the directed edge
/// `a → b`.
#[cfg(feature = "3d")]
#[inline]
fn quad_edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Whether the directed edge `a → b` is a top or left edge.
///
/// The predicate is antisymmetric under reversing the edge: an edge and its reverse cannot both
/// answer `true`. That is what makes the shared diagonal of the two quad triangles covered by
/// exactly one of them, so a translucent quad has no double-blended seam.
#[cfg(feature = "3d")]
#[inline]
fn quad_edge_is_top_left(ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    // Screen coordinates grow downward, so an edge pointing down is a "top" edge.
    by > ay || (by == ay && bx < ax)
}

/// Whether `(px, py)` is inside the triangle `(a, b, c)` under the top-left fill rule.
///
/// The caller has already normalised the winding so every interior point has a non-negative edge
/// value. A sample exactly on an edge belongs to the triangle only when that edge is a top or left
/// edge, which is the rule that keeps a shared edge from being painted twice.
#[cfg(feature = "3d")]
#[inline]
fn inside_top_left(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32, px: f32, py: f32) -> bool {
    let edges = [(ax, ay, bx, by), (bx, by, cx, cy), (cx, cy, ax, ay)];
    for (ex0, ey0, ex1, ey1) in edges {
        let value = quad_edge(ex0, ey0, ex1, ey1, px, py);
        if value < 0.0 {
            return false;
        }
        if value == 0.0 && !quad_edge_is_top_left(ex0, ey0, ex1, ey1) {
            return false;
        }
    }
    true
}

/// Reusable outline buffers for [`SoftwareSurface::draw_text_glyph_run`].
///
/// The fixed-capacity outline arrays and the translated polygon were three allocations **per
/// call** — three per label per frame — although nothing about them depends on the label (G-14).
/// They are taken from a thread-local slot and put back when the run finishes, so the steady state
/// allocates nothing, and taking rather than borrowing makes re-entry safe: a nested glyph run finds
/// an empty slot and allocates its own buffers instead of aliasing this one.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
struct OutlineScratch {
    /// Glyph outline vertices, at the capacity the outliner requires.
    points: Vec<crate::render::text::OutlinePoint>,
    /// `(start, end)` vertex ranges of the outline's contours.
    contours: Vec<(usize, usize)>,
    /// The translated polygon handed to the scan converter; kept for its capacity.
    polygon: Vec<Point>,
}

#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
impl OutlineScratch {
    fn new() -> Self {
        Self {
            points: crate::compat::vec![
                crate::render::text::OutlinePoint { x: 0.0, y: 0.0 };
                crate::render::text::OUTLINE_MAX_POINTS
            ],
            contours: crate::compat::vec![
                (0usize, 0usize); crate::render::text::OUTLINE_MAX_CONTOURS
            ],
            polygon: Vec::new(),
        }
    }
}

#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
thread_local! {
    /// The outline slot, already at full capacity so the first glyph run does not allocate either.
    static OUTLINE_SCRATCH: core::cell::RefCell<Option<OutlineScratch>> =
        core::cell::RefCell::new(Some(OutlineScratch::new()));
}

/// Takes the thread's outline slot, allocating one only when a nested run holds it.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn take_outline_scratch() -> OutlineScratch {
    OUTLINE_SCRATCH
        .with(|slot| match slot.try_borrow_mut() {
            Ok(mut slot) => slot.take(),
            Err(_) => None,
        })
        .unwrap_or_else(OutlineScratch::new)
}

/// Returns an outline slot to the thread when the slot is still empty.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
fn return_outline_scratch(scratch: OutlineScratch) {
    OUTLINE_SCRATCH.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if slot.is_none() {
                *slot = Some(scratch);
            }
        }
    });
}

#[cfg(feature = "fonts-emoji-color")]
thread_local! {
    /// Four-byte-per-pixel glyph scratch for [`SoftwareSurface::draw_text`], reused across lines and
    /// frames. It used to be allocated once per line (G-15); it lives here rather than on the
    /// surface because a colour face is the only thing that needs it, and the surface's own
    /// coverage scratch is one byte per pixel by contract.
    static TEXT_COLOR_SCRATCH: core::cell::RefCell<Vec<u8>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// Takes the thread's colour-glyph scratch, leaving an empty buffer behind.
#[cfg(feature = "fonts-emoji-color")]
fn take_color_scratch() -> Vec<u8> {
    TEXT_COLOR_SCRATCH.with(|slot| match slot.try_borrow_mut() {
        Ok(mut slot) => core::mem::take(&mut *slot),
        Err(_) => Vec::new(),
    })
}

/// Returns a colour-glyph scratch, keeping the larger of the two buffers.
#[cfg(feature = "fonts-emoji-color")]
fn return_color_scratch(scratch: Vec<u8>) {
    TEXT_COLOR_SCRATCH.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if scratch.capacity() > slot.capacity() {
                *slot = scratch;
            }
        }
    });
}

/// The key face for a request whose family names no shipped face. Such a key is rasterised through
/// the active stack, so its bitmap is identified by the character and the cell alone — every unnamed
/// family shares the entry, and the leading control scalar keeps the name from colliding with one.
#[cfg(feature = "text-shaping")]
const ATLAS_STACK_RESOLVED_FACE: &str = "\u{1}";

/// How many glyphs one opt-in glyph-run label may outline before falling back to the cluster path (G-019).
///
/// The glyph-run path is the opt-in ligature mode (`SoftwareSurface::set_glyph_run_text`), not a
/// glyph-atlas cache: it fills each shaped glyph's vector outline, so its cost is proportional to
/// the run. The cap keeps a pathological label from turning an enabling flag into an unbounded
/// per-frame outline storm; the caller then uses the cluster renderer, which has the coverage
/// atlas behind it.
#[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex", cjk_outline_face))]
const GLYPH_RUN_MAX_GLYPHS: usize = 4096;

#[cfg(feature = "text-shaping")]
thread_local! {
    /// The atlas the text path paints from. It is owned by the thread because no renderer state is
    /// reachable from this module.
    static TEXT_GLYPH_ATLAS: core::cell::RefCell<crate::render::text::GlyphAtlas> =
        core::cell::RefCell::new(crate::render::text::GlyphAtlas::new());
    /// The last real frame number the atlas' epoch was advanced for (G-020).
    static TEXT_GLYPH_ATLAS_FRAME: core::cell::Cell<u64> = const { core::cell::Cell::new(u64::MAX) };
}

/// Paints `ch` from the thread's glyph atlas when the atlas reproduces the request byte for byte.
///
/// # Which requests the atlas can serve (G-202)
///
/// * **Cell shape** — every device cell a label draws into is expressible: `GlyphKey::with_cell`
///   stores the width and height independently (G-202), so ordinary Latin text — an advance of
///   about 0.6 em inside a line box of about 1.2 em — is served as well as a full-em CJK glyph.
/// * **Cell size** — both dimensions must fit a `u8`, so a line box larger than 255 device pixels
///   keeps the direct path.
/// * **Weight** — the default weight only: `paint_active_at_weight` moves a variable face's `wght`
///   axis, while the atlas' stack fallback paints the face's default instance.
/// * **Face choice** — the atlas paints with the face the family names and falls back to the active
///   stack when that face does not cover the character; the direct path always paints through the
///   active stack. The two agree exactly when the family names no shipped face, or names the same
///   face the coverage lookup picks for `ch`.
///
/// The atlas applies the synthetic oblique itself when the key says `italic`, so the blend below
/// passes `apply_italic = false` — the shear must not happen twice.
///
/// Returns `false` when this request is not one the atlas can serve; the caller then runs the direct
/// paint, which is untouched.
#[cfg(feature = "text-shaping")]
fn blend_glyph_from_atlas(
    ch: char,
    x: i32,
    y: i32,
    family: &str,
    frame: u64,
    config: &mut GlyphDrawConfig,
) -> bool {
    if config.w == 0 || config.h == 0 || config.w > u8::MAX as u32 || config.h > u8::MAX as u32 {
        return false;
    }
    if config.weight != crate::core::Font::default_weight() {
        return false;
    }
    let family_face = crate::render::text::shaping::face_for_family(family);
    if let Some(face) = family_face {
        let Some(stack_face) = crate::render::text::face_for_char(ch) else {
            return false;
        };
        if !core::ptr::eq(face.bytes.as_ptr(), stack_face.bytes.as_ptr()) {
            return false;
        }
    }
    let key = crate::render::text::GlyphKey::new(
        family_face.map_or(ATLAS_STACK_RESOLVED_FACE, |face| face.name),
        ch,
        config.h as u8,
        config.weight,
    )
    // The real drawing cell, not the em square: a Latin cluster advances ~0.6 em in a ~1.2 em
    // line box, and the explicit cell lets the atlas serve that box byte for byte (G-202).
    .with_cell(config.w, config.h)
    .with_italic(config.italic);
    let advance_epoch = TEXT_GLYPH_ATLAS_FRAME.with(|last| {
        if last.get() == frame {
            false
        } else {
            last.set(frame);
            true
        }
    });
    let slot = TEXT_GLYPH_ATLAS.with(|atlas| match atlas.try_borrow_mut() {
        Ok(mut atlas) => {
            if advance_epoch {
                atlas.begin_frame();
            }
            Some(atlas.get_or_raster(key))
        }
        Err(_) => None,
    });
    let Some(slot) = slot else {
        return false;
    };
    if slot.width() != config.w || slot.height() != config.h {
        return false;
    }
    // Colour ink is four bytes per pixel and would read as noise here; the atlas reserves the
    // one-byte coverage box, so this cannot happen — refusing is cheaper than trusting that.
    if !matches!(
        slot.ink(),
        crate::render::text::InkKind::Bitmap | crate::render::text::InkKind::Coverage
    ) {
        return false;
    }
    blend_coverage_bitmap(slot.width(), slot.height(), x, y, slot.bitmap(), config, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;

    /// An overflowing `width × height × 4` must skip the blit, not wrap the guard.
    ///
    /// Before the guard used checked arithmetic, `(1 << 31) * (1 << 31) * 4` panicked in
    /// debug and wrapped to 0 in release. The wrapped zero made
    /// `data.len() < expected_len` false for a tiny buffer, and the row loop indexed out
    /// of bounds. A skipped blit is the same degradation an over-short buffer gets.
    #[test]
    fn draw_image_with_overflowing_dimensions_is_ignored() {
        let mut surface = SoftwareSurface::new(Size::new(4, 4), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.draw_image(0, 0, 1 << 31, 1 << 31, &[0, 0, 0, 0]);
        surface.draw_image(0, 0, u32::MAX, u32::MAX, &[]);
        surface.end_frame();

        let rgba = surface.frame_rgba();
        assert!(
            rgba.as_chunks::<4>().0.iter().all(|pixel| pixel.iter().all(|&byte| byte == 255)),
            "an impossible size leaves the surface untouched"
        );
    }

    /// The normal path still paints exactly the pixels the caller supplied.
    #[test]
    fn draw_image_paints_a_buffer_that_matches_its_dimensions() {
        let mut surface = SoftwareSurface::new(Size::new(2, 2), 1.0);
        surface.begin_frame(Color::WHITE);
        let data = [255u8, 0, 0, 255].repeat(4);
        surface.draw_image(0, 0, 2, 2, &data);
        surface.end_frame();
        let rgba = surface.frame_rgba();
        assert_eq!(&rgba[..4], &[255, 0, 0, 255]);
    }

    /// The banded `fill_rounded_rect` must ink exactly the pixels the per-pixel sampler inks.
    ///
    /// This is the equivalence the G-08 fast path promises: the corner bands keep the original
    /// 2x2 sampler, the middle band writes the `1.0` that sampler returns. Comparing the two
    /// against a black backdrop makes a dropped or widened span visible as a channel difference.
    #[test]
    fn the_banded_rounded_fill_matches_the_per_pixel_sampler() {
        let cases = [(40u32, 16u32), (17, 9), (100, 40), (10, 10), (7, 30)];
        for (width, height) in cases {
            for radius in 0..=12u32 {
                let rect = Rect::new(0, 0, width, height);
                let mut banded = SoftwareSurface::new(Size::new(width + 4, height + 4), 1.0);
                banded.begin_frame(Color::BLACK);
                banded.fill_rounded_rect(rect, radius, Color::WHITE);
                banded.end_frame();
                let produced = banded.frame_rgba();

                let effective = rounded_rect_effective_radius(rect, radius);
                for py in 0..height as i32 {
                    for px in 0..width as i32 {
                        let expected = (rounded_rect_coverage(px, py, rect, [effective; 4]) * 255.0)
                            .round() as u8;
                        let index = (py as usize * (width + 4) as usize + px as usize) * 4;
                        let actual = produced[index];
                        assert!(
                            actual.abs_diff(expected) <= 1,
                            "rect {width}x{height} r={radius} at ({px},{py}): banded={actual} expected={expected}"
                        );
                    }
                }
            }
        }
    }

    /// A filled polygon is one contour, so `fill_polygon` (even-odd) and
    /// `fill_path_non_zero_borrowed` (non-zero) share the same scanline kernel. On a single
    /// simple contour both rules select the same interior pixels; this pins that the two entry
    /// points cannot drift apart again, including at vertices that land on a scanline.
    #[test]
    fn fill_polygon_and_compound_path_share_one_scanline_kernel() {
        let cases: &[&[Point]] = &[
            &[Point::new(2, 2), Point::new(28, 2), Point::new(28, 28), Point::new(2, 28)],
            &[Point::new(2, 28), Point::new(16, 2), Point::new(30, 28)],
            &[Point::new(2, 2), Point::new(30, 2), Point::new(16, 16)],
            &[
                Point::new(2, 2),
                Point::new(30, 2),
                Point::new(30, 30),
                Point::new(16, 16),
                Point::new(2, 30),
            ],
            &[Point::new(16, 2), Point::new(30, 16), Point::new(16, 30), Point::new(2, 16)],
            &[
                Point::new(16, 2),
                Point::new(20, 12),
                Point::new(30, 12),
                Point::new(22, 19),
                Point::new(26, 30),
                Point::new(16, 23),
                Point::new(6, 30),
                Point::new(10, 19),
                Point::new(2, 12),
                Point::new(12, 12),
            ],
            &[Point::new(3, 4), Point::new(27, 5), Point::new(11, 29)],
        ];
        for points in cases {
            let mut polygon = SoftwareSurface::new(Size::new(32, 32), 1.0);
            polygon.begin_frame(Color::BLACK);
            polygon.fill_polygon(points, Color::WHITE);
            polygon.end_frame();
            let mut compound = SoftwareSurface::new(Size::new(32, 32), 1.0);
            compound.begin_frame(Color::BLACK);
            compound
                .fill_path_non_zero_borrowed(points, &[(0, points.len())], Color::WHITE, 0, 0)
                .unwrap();
            compound.end_frame();
            assert_eq!(
                polygon.frame_rgba(),
                compound.frame_rgba(),
                "one kernel must mean identical bytes for both fill-rule entry points"
            );
        }
    }

    /// A local-minimum vertex that lands on the first scanline is a tangential contact, not a
    /// span. The old even-odd pairing treated the two coincident crossings as a pair and inked
    /// the apex; grouping equal `x` values leaves the zero-width interval unpainted.
    #[test]
    fn the_scanline_kernel_drops_the_tangent_pixel_at_a_local_minimum() {
        let points = [Point::new(2, 28), Point::new(16, 2), Point::new(30, 28)];
        let mut surface = SoftwareSurface::new(Size::new(32, 32), 1.0);
        surface.begin_frame(Color::BLACK);
        surface.fill_polygon(&points, Color::WHITE);
        surface.end_frame();
        let rgba = surface.frame_rgba();
        let pixel = |x: u32, y: u32| rgba[((y * 32 + x) * 4) as usize];
        assert_eq!(pixel(16, 2), 0, "a tangent vertex is not a filled span");
        assert_eq!(pixel(16, 20), 255, "the interior below the apex is still filled");
    }

    /// The parameterisation is the whole point of the unification: one loop, two readings of
    /// the running winding.
    #[test]
    fn the_fill_rule_reads_winding_and_parity() {
        assert!(FillRule::NonZero.contains(1));
        assert!(FillRule::NonZero.contains(2));
        assert!(!FillRule::NonZero.contains(0));
        assert!(FillRule::EvenOdd.contains(1));
        assert!(FillRule::EvenOdd.contains(-1));
        assert!(!FillRule::EvenOdd.contains(2));
        assert!(!FillRule::EvenOdd.contains(0));
    }

    fn render_command_arc(radius: u32, start: f32, end: f32, filled: bool) -> Vec<u8> {
        let mut surface = SoftwareSurface::new(Size::new(64, 64), 1.0);
        surface.begin_frame(Color::BLACK);
        surface.draw_arc(Point::new(32, 32), radius, start, end, Color::WHITE, filled);
        surface.end_frame();
        surface.frame_rgba().to_vec()
    }

    /// The command-level arc is the shared kernel, not a second sampler: for angles that are
    /// already normalised, `draw_arc` and `draw_arc_segments` must ink exactly the same bytes.
    #[test]
    fn a_command_arc_and_the_shared_segment_kernel_are_byte_identical() {
        use crate::render::{draw_arc_segments, PaintBackend, RenderContext, SoftwarePaintBackend};
        let (start, end) = (0.5f32, 2.5f32);
        let mut backend = SoftwarePaintBackend::new(Size::new(64, 64), 1.0);
        backend.begin_frame(Color::BLACK);
        {
            let mut context = RenderContext::new(&mut backend);
            draw_arc_segments(&mut context, Point::new(32, 32), 16.0, start, end, Color::WHITE, 1);
        }
        backend.end_frame();
        assert_eq!(render_command_arc(16, start, end, false).as_slice(), backend.frame_rgba());
    }

    /// A whole turn used to normalise to `end == start` and collapse to a single line. It is
    /// one full circle, and the kernel walks every quadrant of it.
    #[test]
    fn a_full_turn_command_arc_paints_the_whole_circle() {
        let rgba = render_command_arc(16, 0.0, core::f32::consts::TAU, false);
        let pixel = |x: u32, y: u32| rgba[((y * 64 + x) * 4) as usize];
        for (x, y) in [(32u32, 16u32), (16, 32), (48, 32), (32, 48)] {
            assert_eq!(pixel(x, y), 255, "a full-circle stroke must reach ({x},{y})");
        }
    }

    /// The filled command arc is the triangle fan over the same samples, so a whole turn is a
    /// disc rather than the line the old fixed-step path left behind.
    #[test]
    fn a_filled_full_turn_command_arc_fills_the_disc() {
        let rgba = render_command_arc(16, 0.0, core::f32::consts::TAU, true);
        let pixel = |x: u32, y: u32| rgba[((y * 64 + x) * 4) as usize];
        assert_eq!(pixel(32, 32), 255, "the centre is inside the disc");
        assert_eq!(pixel(32, 20), 255, "the upper half is inside the disc");
        assert_eq!(pixel(32, 16), 255, "the rim is painted");
    }
}
