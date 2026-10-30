// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The CPU raster kernels: one shape per function, plus the blending, colour-space and shadow-layer
//! helpers they share.
//!
//! These are the software renderer's primitives *and* the reference the GPU pipelines agree with:
//! several GPU paths already consume their geometry helpers (`rounded_rect_row_spans`,
//! `path_fan_vertices`, `gradient_ramp`, `stroke_rect_edges`, `effective_clip`, `combine`) so that a
//! corner arc, a fan or a clip intersection is computed exactly once in the crate.

thread_local! {
    /// Reusable shadow layer buffer. Shadows are drawn per command, so a per-command zeroed
    /// layer allocation made every shadowed control pay twice (G-11). Fully rewritten on each
    /// use, so a same-sized buffer needs no clear (G-67).
    static SHADOW_LAYER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Reusable horizontal-pass buffer for [`box_blur_layer_rgba8`]. Separate from the layer so
    /// a nested shadow still has its own scratch; reused at the same size without a zero fill
    /// because the pass overwrites every pixel (G-67).
    static SHADOW_SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
// The blend-mode enum is defined once in the render layer and reused here rather than
// mirrored, so the two backends cannot disagree about the set of modes (principle #54).

use super::super::types::{PixelRect, Rgba8};
use crate::render::BlendMode;
use std::cell::RefCell;

pub fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
pub(crate) fn clear_cpu_rgba8(pixels: &mut [u8], color: Rgba8) {
    for pixel in pixels.as_chunks_mut::<4>().0 {
        *pixel = [color.r, color.g, color.b, color.a];
    }
}
pub(crate) fn fill_rect_cpu_rgba8(pixels: &mut [u8], width: u32, rect: PixelRect, color: Rgba8) {
    if width == 0 || rect.width == 0 || rect.height == 0 {
        return;
    }
    // Callers hand this helper a framebuffer-clipped rectangle; an unclipped one must be a
    // no-op rather than an out-of-range slice index.
    if rect.x < 0 || rect.y < 0 {
        return;
    }
    let row_bytes = width as usize * 4;
    let x_start = rect.x as usize;
    let y_start = rect.y as usize;
    let span = rect.width as usize * 4;
    let first = y_start * row_bytes + x_start * 4;
    let first_end = first.checked_add(span).filter(|end| *end <= pixels.len());
    let Some(first_end) = first_end else { return };
    // Expand the colour into one row, then copy that row into the following rows. A whole-row
    // `copy_within` replaces `width` four-byte stores per line with one run copy (G-69).
    for pixel in pixels[first..first_end].as_chunks_mut::<4>().0 {
        *pixel = [color.r, color.g, color.b, color.a];
    }
    let y_end = y_start + rect.height as usize;
    for y in (y_start + 1)..y_end {
        let dst = y * row_bytes + x_start * 4;
        if dst + span > pixels.len() {
            return;
        }
        pixels.copy_within(first..first_end, dst);
    }
}
pub(crate) fn stroke_rect_edges(rect: PixelRect, thickness: u32) -> [PixelRect; 4] {
    let t = thickness.min(rect.width).min(rect.height);
    [
        PixelRect { x: rect.x, y: rect.y, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.bottom() - t as i32, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.y, width: t, height: rect.height },
        PixelRect { x: rect.right() - t as i32, y: rect.y, width: t, height: rect.height },
    ]
}
pub(crate) fn draw_image_scaled_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    source_rgba8: &[u8],
    source_width: u32,
    source_height: u32,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let Some(clip_rect) = clip_rect else {
        return;
    };
    let mut sink = PixelSink { pixels, width, mode: blend };
    let (x_start, y_start, x_end, y_end) =
        (clip_rect.x, clip_rect.y, clip_rect.right(), clip_rect.bottom());
    let (rw, sw) = (rect.width as u64, source_width as u64);
    let start = (x_start - rect.x) as u64 * sw;
    let (src_x_start, acc_start) = ((start / rw) as u32, start % rw);
    let last_y = source_height.saturating_sub(1) as u64;
    for y in y_start..y_end {
        let local_y = (y - rect.y) as u32;
        let src_y =
            ((local_y as u64 * source_height as u64) / rect.height as u64).min(last_y) as u32;
        let mut acc = acc_start;
        let mut src_x = src_x_start;
        for x in x_start..x_end {
            let src_offset = ((src_y * source_width + src_x) * 4) as usize;
            let pixel = &source_rgba8[src_offset..src_offset + 4];
            let color = Rgba8 { r: pixel[0], g: pixel[1], b: pixel[2], a: pixel[3] };
            sink.set(x as u32, y as u32, color);
            acc += sw;
            while acc >= rw {
                acc -= rw;
                src_x = src_x.saturating_add(1);
            }
        }
    }
}
/// Bounding box of a line segment, inclusive of both endpoints and the stroke: the pixel-centre
/// rule makes the extent `max - min + 1`, widened by half the line width per side and at least
/// 1px per axis (N-S-65).
///
/// Clamp a segment to `rect` (Liang–Barsky), returning the trimmed endpoints. An endpoint far
/// outside the framebuffer would walk billions of invisible pixels, so this bounds the walk;
/// inside endpoints are unchanged (t₀=0, t₁=1) and a miss is `None`.
pub(crate) fn clip_segment_to_rect(
    from: (i32, i32),
    to: (i32, i32),
    rect: &PixelRect,
) -> Option<((i32, i32), (i32, i32))> {
    let (x0, y0) = (from.0 as f64, from.1 as f64);
    let (dx, dy) = (to.0 as f64 - x0, to.1 as f64 - y0);
    // Inclusive pixel bounds: `right()`/`bottom()` are exclusive.
    let (left, top) = (rect.x as f64, rect.y as f64);
    let (right, bottom) = (rect.right() as f64 - 1.0, rect.bottom() as f64 - 1.0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    // (p, q) per boundary: the segment stays inside while p*t <= q.
    for (p, q) in [(-dx, x0 - left), (dx, right - x0), (-dy, y0 - top), (dy, bottom - y0)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    let clamp = |v: f64| v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    let start = (clamp(x0 + t0 * dx), clamp(y0 + t0 * dy));
    let end = (clamp(x0 + t1 * dx), clamp(y0 + t1 * dy));
    Some((start, end))
}

pub(crate) fn rect_for_line(from: (i32, i32), to: (i32, i32), line_width: u32) -> PixelRect {
    let (min_x, max_x) = (from.0.min(to.0), from.0.max(to.0));
    let (min_y, max_y) = (from.1.min(to.1), from.1.max(to.1));
    // `draw_line_cpu_rgba8` spreads the stroke over `(center - half + 1 ..= center + half)`,
    // so the geometry reaches `half` pixels past the endpoint on each side.
    let half = (line_width as i64 / 2).max(0);
    let x0 = min_x as i64 - half;
    let y0 = min_y as i64 - half;
    // Inclusive extent: `max - min + 1`, plus the stroke on both sides.
    let w = (max_x as i64 - min_x as i64) + 1 + half * 2;
    let h = (max_y as i64 - min_y as i64) + 1 + half * 2;
    PixelRect {
        x: (x0.clamp(i32::MIN as i64, i32::MAX as i64)) as i32,
        y: (y0.clamp(i32::MIN as i64, i32::MAX as i64)) as i32,
        width: w.clamp(1, u32::MAX as i64) as u32,
        height: h.clamp(1, u32::MAX as i64) as u32,
    }
}
/// Bounding box of a circle *including* its stroke width: `DrawCircle` strokes `width`
/// pixels outside the radius, so the box must grow by `width` per side (the old `2r` clipped
/// the outer half of the ring away), plus one pixel so the inclusive far edge
/// (`center + r + width`) is covered (N-S-65).
pub(crate) fn bbox_for_circle(center: (i32, i32), radius: u32, stroke_width: u32) -> PixelRect {
    // Widened to i64 so a large radius/stroke cannot overflow before clamping.
    let reach = radius as i64 + stroke_width as i64 + 1;
    let r = reach.clamp(0, i32::MAX as i64) as i32;
    PixelRect {
        x: center.0.saturating_sub(r),
        y: center.1.saturating_sub(r),
        width: (r as i64 * 2).clamp(1, u32::MAX as i64) as u32,
        height: (r as i64 * 2).clamp(1, u32::MAX as i64) as u32,
    }
}
/// Bounding box of a polyline, inclusive of both endpoints and the stroke: the extent is
/// `max - min + 1`, widened by half the stroke per side, and at least 1px per axis so a single
/// point stays drawable (N-S-65).
pub(crate) fn bbox_for_path(points: &[(i32, i32)], stroke_width: u32) -> PixelRect {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for &(px, py) in points {
        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
    }
    if min_x > max_x || min_y > max_y {
        return PixelRect { x: 0, y: 0, width: 0, height: 0 };
    }
    let half = (stroke_width as i64 / 2).max(0);
    let w = (max_x as i64 - min_x as i64) + 1 + half * 2;
    let h = (max_y as i64 - min_y as i64) + 1 + half * 2;
    let x = (min_x as i64 - half).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y = (min_y as i64 - half).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    PixelRect {
        x,
        y,
        width: w.clamp(1, u32::MAX as i64) as u32,
        height: h.clamp(1, u32::MAX as i64) as u32,
    }
}
/// The horizontal span the filled rounded rectangle covers on each row it touches.
///
/// The shape is convex and horizontal, so every row is one contiguous span: rows between the two
/// corner bands are the full width, and a corner row's span is `[left + r - k, right - r + k)`
/// with `k = floor(sqrt(r² - dy²))` — the integer predicate the per-pixel form evaluated, resolved
/// once per row (G-803). This is the **single** definition of that rule: the CPU fill and the GPU
/// instance packer both consume it, which is what makes the two paths pixel-identical.
pub(crate) fn rounded_rect_row_spans(
    rect: PixelRect,
    radius: u32,
) -> impl Iterator<Item = PixelRect> {
    let r = radius.min(rect.width / 2).min(rect.height / 2) as i32;
    let r2 = r * r;
    let (top, bottom) = (rect.y, rect.bottom());
    (top..bottom).filter_map(move |y| {
        let (start, end) = if r <= 0 || (y >= rect.y + r && y < bottom - r) {
            (rect.x, rect.right())
        } else {
            let dy = if y < rect.y + r { rect.y + r - y } else { y - (bottom - r - 1) };
            let remaining = r2 - dy * dy;
            if remaining < 0 {
                return None;
            }
            let k = (remaining as u32).isqrt() as i32;
            (rect.x + r - k, rect.right() - r + k)
        };
        if end <= start {
            return None;
        }
        Some(PixelRect { x: start, y, width: (end - start) as u32, height: 1 })
    })
}

pub(crate) fn fill_rounded_rect_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    radius: u32,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    let x_start = clip_rect.x;
    let y_start = clip_rect.y;
    let x_end = clip_rect.right();
    let y_end = clip_rect.bottom();
    for span in rounded_rect_row_spans(rect, radius) {
        if span.y < y_start || span.y >= y_end {
            continue;
        }
        let from = span.x.max(x_start);
        let to = span.right().min(x_end);
        for x in from..to {
            sink.set(x as u32, span.y as u32, color);
        }
    }
}

/// Whether pixel `(x, y)` is inside `rect` rounded with corner radius `r`. One predicate shared
/// by the fill and the outline, so the ring's straight sections and corner arcs cannot disagree.
fn inside_rounded_rect(rect: PixelRect, r: i32, x: i32, y: i32) -> bool {
    // Bounds first: the corner branches below also assume the point is inside, so `true` from the
    // middle branch would otherwise mean "inside" for a point above or left of the rectangle —
    // which is exactly the shape the outline's *inner* test is asked about.
    if x < rect.x || x >= rect.right() || y < rect.y || y >= rect.bottom() {
        return false;
    }
    if r <= 0 {
        return true;
    }
    let r2 = r * r;
    // Check if pixel is inside the rounded rect:
    // top-left corner
    if x < rect.x + r && y < rect.y + r {
        let dx = rect.x + r - x;
        let dy = rect.y + r - y;
        dx * dx + dy * dy <= r2
    } else if x >= rect.right() - r && y < rect.y + r {
        let dx = x - (rect.right() - r - 1);
        let dy = rect.y + r - y;
        dx * dx + dy * dy <= r2
    } else if x < rect.x + r && y >= rect.bottom() - r {
        let dx = rect.x + r - x;
        let dy = y - (rect.bottom() - r - 1);
        dx * dx + dy * dy <= r2
    } else if x >= rect.right() - r && y >= rect.bottom() - r {
        let dx = x - (rect.right() - r - 1);
        let dy = y - (rect.bottom() - r - 1);
        dx * dx + dy * dy <= r2
    } else {
        true
    }
}
/// Paints `rect`'s outline as the **inside** band — `rect` minus `rect` inset by `thickness` —
/// the same shape the software and SVG backends paint. Deriving it from one predicate keeps the
/// straight sections and corner arcs on the same pixels, so a rounded outline has no open
/// corners.
#[allow(clippy::nonminimal_bool)]
pub(crate) fn stroke_rounded_rect_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    radius: u32,
    color: Rgba8,
    thickness: u32,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    let r = radius.min(rect.width / 2).min(rect.height / 2) as i32;
    let t = thickness as i32;
    let inner_width = rect.width as i32 - 2 * t;
    let inner_height = rect.height as i32 - 2 * t;
    let inner = (inner_width > 0 && inner_height > 0).then_some(PixelRect {
        x: rect.x + t,
        y: rect.y + t,
        width: inner_width as u32,
        height: inner_height as u32,
    });
    let inner_r = (r - t).max(0);
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    for y in y_start..y_end {
        for x in x_start..x_end {
            if !inside_rounded_rect(rect, r, x, y) {
                continue;
            }
            if let Some(inner) = inner {
                if inside_rounded_rect(inner, inner_r, x, y) {
                    continue;
                }
            }
            sink.set(x as u32, y as u32, color);
        }
    }
}
pub(crate) fn draw_line_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    from: (i32, i32),
    to: (i32, i32),
    color: Rgba8,
    line_width: u32,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    let x_lo = clip_rect.x as i64;
    let x_hi = clip_rect.right() as i64;
    let y_lo = clip_rect.y as i64;
    let y_hi = clip_rect.bottom() as i64;
    for_each_line_stamp(from, to, line_width, |x0, y0, half_w| {
        // Draw a small square around each point for thickness, pre-clamped to the clip so an
        // extreme endpoint cannot overflow `y0 ± half_w` and the inner loop needs no
        // per-pixel bounds check.
        let thick_y = (y0 - half_w + 1).max(y_lo)..=((y0 + half_w).min(y_hi - 1));
        let thick_x = (x0 - half_w + 1).max(x_lo)..=((x0 + half_w).min(x_hi - 1));
        for thick_y in thick_y {
            for thick_x in thick_x.clone() {
                sink.set(thick_x as u32, thick_y as u32, color);
            }
        }
    });
}

/// Calls `visit` once per Bresenham step of a line, with the step's centre and the half width of
/// the square the CPU stamps there.
///
/// This is the **one** definition of a line's geometry: the CPU blends the square, and the GPU
/// packs one instanced quad for it, which is why a diagonal line is byte-identical instead of
/// being approximated by a capsule. Coordinates are `i64` so an extreme endpoint cannot overflow
/// the error term or the `centre ± half` arithmetic; the caller decides how to bound them.
pub(crate) fn for_each_line_stamp(
    from: (i32, i32),
    to: (i32, i32),
    line_width: u32,
    mut visit: impl FnMut(i64, i64, i64),
) {
    let (mut x0, mut y0) = (from.0 as i64, from.1 as i64);
    let (x1, y1) = (to.0 as i64, to.1 as i64);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let half_w = (line_width as i64 / 2).max(1);
    loop {
        visit(x0, y0, half_w);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
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
/// Largest integer whose square is at most `value` (integer square root).
///
/// The scanline circle paths need it once per row to turn `dx² + dy² <= r²` into an inclusive x
/// interval; `f64::sqrt` seeds it and correction loops make the answer exact.
fn isqrt_i128(value: i128) -> i128 {
    if value <= 0 {
        return 0;
    }
    let mut root = (value as f64).sqrt() as i128;
    while (root + 1) * (root + 1) <= value {
        root += 1;
    }
    while root * root > value {
        root -= 1;
    }
    root
}
pub(crate) fn fill_circle_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    center: (i32, i32),
    radius: u32,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    // All circle math in `i128`: `radius` is `u32`, so `r * r` exceeds `i64` near the top of
    // that range. The clip bounds the visible rows, but the implicit `dx * dx + dy * dy` test
    // is evaluated with the same widened type.
    let r = radius as i128;
    let r2 = r * r;
    let cx = center.0 as i128;
    let cy = center.1 as i128;
    let x_start = clip_rect.x.max((cx - r).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_start = clip_rect.y.max((cy - r).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_end =
        clip_rect.right().min((cx + r + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_end =
        clip_rect.bottom().min((cy + r + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_start = x_start as i128;
    let x_end = x_end as i128;
    // One square root per row yields both endpoints of the painted run, so the inner loop is a
    // contiguous fill instead of a per-pixel distance test (G-68).
    for y in y_start..y_end {
        let dy = y as i128 - cy;
        let remaining = r2 - dy * dy;
        if remaining < 0 {
            continue;
        }
        let half = isqrt_i128(remaining);
        let row_x0 = (cx - half).max(x_start);
        let row_x1 = (cx + half).min(x_end - 1);
        for x in row_x0..=row_x1 {
            sink.set(x as u32, y as u32, color);
        }
    }
}
pub(crate) fn draw_circle_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    center: (i32, i32),
    radius: u32,
    color: Rgba8,
    width: u32,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    let r = radius as i128;
    let outer = r + width as i128;
    let inner = (r - width as i128).max(0);
    let outer_r2 = outer * outer;
    let inner_r2 = inner * inner;
    let cx = center.0 as i128;
    let cy = center.1 as i128;
    let x_start = clip_rect.x.max((cx - outer).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_start = clip_rect.y.max((cy - outer).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_end =
        clip_rect.right().min((cx + outer + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_end =
        clip_rect.bottom().min((cy + outer + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_start = x_start as i128;
    let x_end = x_end as i128;
    for y in y_start..y_end {
        let dy = y as i128 - cy;
        let dy2 = dy * dy;
        let remaining_outer = outer_r2 - dy2;
        if remaining_outer < 0 {
            continue;
        }
        let outer_half = isqrt_i128(remaining_outer);
        let row_x0 = (cx - outer_half).max(x_start);
        let row_x1 = (cx + outer_half).min(x_end - 1);
        if row_x0 > row_x1 {
            continue;
        }
        // The hole is the row of pixels with `dx * dx < inner_r2 - dy * dy`. For integer
        // `dx` that is `dx * dx <= remaining_inner - 1`, whose largest solution is
        // `isqrt(remaining_inner - 1)` (G-68).
        let remaining_inner = inner_r2 - dy2;
        if remaining_inner <= 0 {
            for x in row_x0..=row_x1 {
                sink.set(x as u32, y as u32, color);
            }
        } else {
            let inner_excl = isqrt_i128(remaining_inner - 1);
            let left_hi = (cx - inner_excl - 1).min(row_x1);
            if left_hi >= row_x0 {
                for x in row_x0..=left_hi {
                    sink.set(x as u32, y as u32, color);
                }
            }
            let right_lo = (cx + inner_excl + 1).max(row_x0);
            if right_lo <= row_x1 {
                for x in right_lo..=row_x1 {
                    sink.set(x as u32, y as u32, color);
                }
            }
        }
    }
}
/// Arc drawing parameters for `draw_arc_cpu_rgba8`.
pub(crate) struct ArcParams {
    /// Center of the arc.
    pub center: (i32, i32),
    /// Radius of the arc.
    pub radius: u32,
    /// Start angle in radians.
    pub start_angle: f32,
    /// End angle in radians.
    pub end_angle: f32,
    /// Fill color.
    pub color: Rgba8,
    /// Whether the arc is filled (vs stroked).
    pub filled: bool,
}

pub(crate) fn draw_arc_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    arc: &ArcParams,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
    // The same scanline kernel as `fill_circle_cpu_rgba8`/`draw_circle_cpu_rgba8` (G-68):
    // one square root per row yields the inside-circle run, so the angle test runs only on
    // candidate pixels instead of every bounding-box pixel (G-223). Circle math stays in
    // `i128` so `(r + 1) * (r + 1)` cannot overflow near the top of `u32`.
    let r = arc.radius as i128;
    let r2 = r * r;
    let outer_r2 = (r + 1) * (r + 1);
    let inner_r2 = (r - 1).max(0) * (r - 1).max(0);
    let cx = arc.center.0 as i128;
    let cy = arc.center.1 as i128;
    let bound = |v: i128| v.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
    let x_start = clip_rect.x.max(bound(cx - r)) as i128;
    let y_start = clip_rect.y.max(bound(cy - r)) as i128;
    let x_end = clip_rect.right().min(bound(cx + r + 1)) as i128;
    let y_end = clip_rect.bottom().min(bound(cy + r + 1)) as i128;
    let (sa, ea) = if (arc.start_angle - arc.end_angle).abs() < 0.001 {
        (0.0, std::f32::consts::TAU) // full circle: the angle test is skipped entirely
    } else {
        let sa = arc.start_angle.rem_euclid(std::f32::consts::TAU);
        let mut ea = arc.end_angle.rem_euclid(std::f32::consts::TAU);
        if ea <= sa {
            ea += std::f32::consts::TAU; // normalise to [sa, sa + TAU) so ea > sa
        }
        (sa, ea)
    };
    let is_full_circle = (ea - sa) >= std::f32::consts::TAU - 0.001;
    // Paints one row span, skipping pixels outside the arc's angular window. The test is
    // exactly the per-pixel arithmetic it replaces, so the painted set is unchanged; it now
    // runs only on pixels already inside the circle.
    let mut paint_span = |x0: i128, x1: i128, y: i128| {
        for x in x0..=x1 {
            if !is_full_circle {
                let angle = ((y - cy) as f64).atan2((x - cx) as f64) as f32;
                let angle = angle.rem_euclid(std::f32::consts::TAU);
                let angle = if angle < sa { angle + std::f32::consts::TAU } else { angle };
                if angle < sa || angle > ea {
                    continue;
                }
            }
            sink.set(x as u32, y as u32, arc.color);
        }
    };
    for y in y_start..y_end {
        let dy2 = (y - cy) * (y - cy);
        // Row candidates: the filled disc's run `|dx| <= isqrt(r2 - dy2)`, or for a stroke
        // the outer circle's run `|dx| <= isqrt(outer_r2 - dy2)`.
        let remaining = (if arc.filled { r2 } else { outer_r2 }) - dy2;
        if remaining < 0 {
            continue;
        }
        let half = isqrt_i128(remaining);
        let row_x0 = (cx - half).max(x_start);
        let row_x1 = (cx + half).min(x_end - 1);
        if row_x0 > row_x1 {
            continue;
        }
        // A stroke excludes its hole `dx * dx < inner_r2 - dy2`; the row paints the two run
        // segments flanking it (its largest excluded dx is `isqrt(remaining_inner - 1)`, G-68).
        let remaining_inner = if arc.filled { 0 } else { inner_r2 - dy2 };
        if remaining_inner > 0 {
            let inner_excl = isqrt_i128(remaining_inner - 1);
            paint_span(row_x0, (cx - inner_excl - 1).min(row_x1), y);
            paint_span((cx + inner_excl + 1).max(row_x0), row_x1, y);
        } else {
            paint_span(row_x0, row_x1, y);
        }
    }
}
pub(crate) struct CpuPathDraw<'a> {
    pub(crate) points: &'a [(i32, i32)],
    pub(crate) closed: bool,
    pub(crate) color: Rgba8,
    pub(crate) filled: bool,
    pub(crate) path_width: u32,
    pub(crate) clip_rect: Option<PixelRect>,
    pub(crate) blend: BlendMode,
}

pub(crate) fn draw_path_cpu_rgba8(pixels: &mut [u8], fb_width: u32, path: CpuPathDraw<'_>) {
    let CpuPathDraw { points, closed, color, filled, path_width, clip_rect, blend } = path;
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    if filled && points.len() >= 3 {
        let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
        // Simple scanline fill for convex polygons
        let min_y = clip_rect.y.max(points.iter().map(|&(_, y)| y).min().unwrap_or(0));
        let max_y = clip_rect.bottom().min(points.iter().map(|&(_, y)| y).max().unwrap_or(0) + 1);
        let n = points.len();
        // One intersection buffer for the whole fill, cleared per scanline — the shape of the
        // sibling `draw_path_non_zero_cpu_rgba8` just below. A fresh `Vec` per scanline meant one
        // allocation per row of every filled outline (an icon, a vector decoration, a glyph
        // fallback), which is hundreds of allocations for one picture (REND-08).
        let mut intersections: Vec<i32> = Vec::with_capacity(n);
        for y in min_y..max_y {
            intersections.clear();
            for i in 0..n {
                let (x1, y1) = points[i];
                let (x2, y2) = points[(i + 1) % n];
                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    let t = (y - y1) as f64 / (y2 - y1) as f64;
                    let ix = x1 as f64 + t * (x2 - x1) as f64;
                    intersections.push(ix as i32);
                }
            }
            intersections.sort_unstable();
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    let x_start = pair[0].max(clip_rect.x);
                    let x_end = pair[1].min(clip_rect.right());
                    for x in x_start..x_end {
                        sink.set(x as u32, y as u32, color);
                    }
                }
            }
        }
    } else {
        // Draw line segments. The sink's borrow of `pixels` has ended with the
        // `if` branch, so the line helper can take the same framebuffer.
        let iter_len = if closed { points.len() } else { points.len() - 1 };
        for i in 0..iter_len {
            let from = points[i];
            let to = points[(i + 1) % points.len()];
            let line_clip = if path_width > 0 {
                let r = rect_for_line(from, to, path_width);
                r.intersect(clip_rect)
            } else {
                None
            };
            draw_line_cpu_rgba8(pixels, fb_width, from, to, color, path_width, line_clip, blend);
        }
    }
}

pub(crate) fn draw_path_non_zero_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    points: &[(i32, i32)],
    contours: &[(usize, usize)],
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let Some(clip_rect) = clip_rect else {
        return;
    };
    let min_y = clip_rect.y.max(
        contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|&(_, y)| y)
            .min()
            .unwrap_or(0),
    );
    let max_y = clip_rect.bottom().min(
        contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|&(_, y)| y)
            .max()
            .unwrap_or(-1)
            .saturating_add(1),
    );
    let mut intersections: Vec<(f64, i32)> =
        Vec::with_capacity(contours.iter().map(|&(start, end)| end - start).sum());
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };

    for y in min_y..max_y {
        intersections.clear();
        for &(start, end) in contours {
            for index in start..end {
                let (x1, y1) = points[index];
                let (x2, y2) = points[if index + 1 == end { start } else { index + 1 }];
                let winding = if y1 <= y && y2 > y {
                    1
                } else if y2 <= y && y1 > y {
                    -1
                } else {
                    continue;
                };
                let t = (y - y1) as f64 / (y2 - y1) as f64;
                intersections.push((x1 as f64 + t * (x2 - x1) as f64, winding));
            }
        }
        intersections.sort_by(|left, right| {
            left.0.partial_cmp(&right.0).unwrap_or(core::cmp::Ordering::Equal)
        });

        let mut winding = 0;
        let mut previous_x: Option<f64> = None;
        let mut index = 0;
        while index < intersections.len() {
            let x = intersections[index].0;
            if winding != 0 {
                if let Some(previous) = previous_x {
                    let x_start = (previous.ceil() as i32).max(clip_rect.x);
                    let x_end = (x.floor() as i32).min(clip_rect.right().saturating_sub(1));
                    for pixel_x in x_start..=x_end {
                        sink.set(pixel_x as u32, y as u32, color);
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
}

/// Selects the nearest colour stop of `gradient_data` for a position `t` in `0.0..=1.0`.
///
/// Stops are evenly spaced and picked by their left edge, exactly as the previous per-pixel
/// expression did; only the arithmetic that produces `t` changed (G-68).
#[inline]
fn gradient_stop(gradient_data: &[u8], stops: usize, t: f32) -> Rgba8 {
    let idx = ((stops as f32 - 1.0) * t.clamp(0.0, 1.0)).clamp(0.0, (stops - 1) as f32) as usize;
    let base = idx * 4;
    Rgba8 {
        r: gradient_data[base],
        g: gradient_data[base + 1],
        b: gradient_data[base + 2],
        a: gradient_data[base + 3],
    }
}

/// The triangle fan that fills a path, as `(x, y)` pixel pairs.
///
/// Each contour becomes a fan around its first point, which is the standard way to turn a polygon
/// into geometry the stencil buffer can count: a fan's triangles overlap exactly as often as the
/// polygon covers each pixel, so the even-odd and non-zero rules both come out of the stencil
/// operations rather than any CPU-side winding analysis.
///
/// Contours are `(start, end)` ranges into `points`; a contour with fewer than three points
/// contributes no triangles.
pub(crate) fn path_fan_vertices(
    points: &[(i32, i32)],
    contours: &[(usize, usize)],
) -> Vec<(f32, f32)> {
    let mut fan = Vec::new();
    for &(start, end) in contours {
        let end = end.min(points.len());
        if end.saturating_sub(start) < 3 {
            continue;
        }
        let origin = points[start];
        for index in start + 1..end - 1 {
            let (a, b) = (points[index], points[index + 1]);
            fan.push((origin.0 as f32, origin.1 as f32));
            fan.push((a.0 as f32, a.1 as f32));
            fan.push((b.0 as f32, b.1 as f32));
        }
    }
    fan
}

/// Bounding box of a set of path points, clamped to `framebuffer`.
pub(crate) fn path_bounds(points: &[(i32, i32)], framebuffer: PixelRect) -> Option<PixelRect> {
    let mut iter = points.iter();
    let (first_x, first_y) = *iter.next()?;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first_x, first_y, first_x, first_y);
    for &(x, y) in iter {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    // The cover quad must contain the filled area: the fan's triangles reach at most one pixel
    // beyond the extreme vertices, so the extent is widened by one before clipping.
    let rect = PixelRect {
        x: min_x,
        y: min_y,
        width: (max_x - min_x + 2).max(1) as u32,
        height: (max_y - min_y + 2).max(1) as u32,
    };
    rect.intersect(framebuffer)
}

/// One colour per destination pixel along an axis-aligned gradient's ramp.
///
/// `Vertical` yields `rect.height` entries (one per row), `Horizontal` `rect.width` (one per
/// column). The `t` and the stop lookup are the ones [`draw_gradient_cpu_rgba8`] uses, so a GPU
/// gradient built from this ramp **is** the CPU picture, byte for byte.
///
/// This is also what makes the GPU path cheaper rather than merely different: the CPU rasteriser
/// costs one lookup per *pixel*, while the ramp costs one per row (or column) and the GPU repeats
/// it across the span.
pub(crate) fn gradient_ramp(rect: PixelRect, gradient_data: &[u8], axis: GradientAxis) -> Vec<u8> {
    let stops = gradient_data.len() / 4;
    let (span, origin) = match axis {
        GradientAxis::Vertical => ((rect.height as f32 - 1.0).max(1.0), rect.y as f32),
        GradientAxis::Horizontal => ((rect.width as f32 - 1.0).max(1.0), rect.x as f32),
    };
    let inv_span = 1.0 / span;
    let count = match axis {
        GradientAxis::Vertical => rect.height,
        GradientAxis::Horizontal => rect.width,
    } as usize;
    let mut ramp = Vec::with_capacity(count * 4);
    for index in 0..count {
        let position = match axis {
            GradientAxis::Vertical => rect.y + index as i32,
            GradientAxis::Horizontal => rect.x + index as i32,
        };
        let t = ((position as f32 - origin) * inv_span).clamp(0.0, 1.0);
        let color = gradient_stop(gradient_data, stops, t);
        ramp.extend_from_slice(&[color.r, color.g, color.b, color.a]);
    }
    ramp
}

pub(crate) fn draw_gradient_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    gradient_data: &[u8],
    clip_rect: Option<PixelRect>,
    axis: GradientAxis,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    // gradient_data is treated as a colour-stop table: [r,g,b,a, r,g,b,a, ...]
    // interpolated across the rect along `axis`.
    let stops = gradient_data.len() / 4;
    if stops == 0 {
        return;
    }
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());

    // The span the ramp is spread over, and the reciprocal that replaces a per-pixel division.
    // `f32` is enough for screen-sized ramps and halves the arithmetic width (G-68).
    let (span, origin) = match axis {
        GradientAxis::Vertical => ((rect.height as f32 - 1.0).max(1.0), rect.y as f32),
        GradientAxis::Horizontal => ((rect.width as f32 - 1.0).max(1.0), rect.x as f32),
    };
    let inv_span = 1.0 / span;

    for y in y_start..y_end {
        match axis {
            GradientAxis::Vertical => {
                // A vertical ramp is constant across the row: one stop lookup serves the whole run.
                let t = ((y as f32 - origin) * inv_span).clamp(0.0, 1.0);
                let color = gradient_stop(gradient_data, stops, t);
                for x in x_start..x_end {
                    blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
                }
            }
            GradientAxis::Horizontal => {
                // The reciprocal is applied per pixel instead of dividing by the span; the
                // product is bounded per pixel, so it does not accumulate rounding like a
                // running sum would (G-68).
                for x in x_start..x_end {
                    let t = ((x as f32 - origin) * inv_span).clamp(0.0, 1.0);
                    let color = gradient_stop(gradient_data, stops, t);
                    blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
                }
            }
        }
    }
}

/// Which direction a gradient ramp advances along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GradientAxis {
    /// Top to bottom — what [`WgpuDrawCommand::DrawGradient`] means.
    Vertical,
    /// Left to right — what [`WgpuDrawCommand::FillLinearGradient`] means.
    Horizontal,
}

/// Fills `rect` with a radial ramp of `gradient_data` stop colours. The variant carries no
/// centre or radius, so the ramp is centred in `rect` and reaches the nearest edge; pixels
/// beyond it take the **last** stop so the fill always covers `rect` completely.
pub(crate) fn draw_radial_gradient_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    gradient_data: &[u8],
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let stops = gradient_data.len() / 4;
    if stops == 0 {
        return;
    }
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());

    let centre_x = rect.x as f32 + (rect.width as f32 - 1.0) / 2.0;
    let centre_y = rect.y as f32 + (rect.height as f32 - 1.0) / 2.0;
    // Nearest edge, so the gradient fits inside the rectangle on both axes.
    let radius = ((rect.width as f32).min(rect.height as f32) / 2.0).max(1.0);
    let inv_radius = 1.0 / radius;

    for y in y_start..y_end {
        // `dy` is constant for the row; only `dx` changes, so one `dy * dy` serves the row.
        let dy = y as f32 - centre_y;
        let dy2 = dy * dy;
        for x in x_start..x_end {
            let dx = x as f32 - centre_x;
            // The per-pixel division became a multiply by the reciprocal (G-68).
            let t = ((dx * dx + dy2).sqrt() * inv_radius).clamp(0.0, 1.0);
            let color = gradient_stop(gradient_data, stops, t);
            blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
        }
    }
}

/// Combines a source colour with the destination pixel using `mode`.
///
/// `Normal` writes the source directly, which is what every other draw helper does,
/// so it stays on the fast path and existing streams are unaffected.
pub(crate) fn blend_pixel_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    source: Rgba8,
    mode: BlendMode,
) {
    if mode == BlendMode::Normal {
        set_pixel_cpu_rgba8(pixels, width, x, y, source);
        return;
    }
    // Widened to `usize` before multiplying, for the same reason
    // `set_pixel_cpu_rgba8` documents: in `u32` the product wraps for a large `y`,
    // which in debug panics and in release writes a correctly-in-range but wrong
    // pixel. This is the read-modify-write sibling of that function and had been
    // left on the wrapping form.
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    // `offset + 3` can itself overflow `usize` on a 32-bit target, so the check is
    // on the last written index rather than on the offset.
    let Some(last) = offset.checked_add(3) else {
        return;
    };
    if last >= pixels.len() {
        return;
    }
    let dest = Rgba8 {
        r: pixels[offset],
        g: pixels[offset + 1],
        b: pixels[offset + 2],
        a: pixels[offset + 3],
    };
    let blended = blend_channels(source, dest, mode);
    pixels[offset] = blended.r;
    pixels[offset + 1] = blended.g;
    pixels[offset + 2] = blended.b;
    pixels[offset + 3] = blended.a;
}

/// Maps the numeric blend-mode tag carried by the command stream to its enum. The tag is the
/// enum's own `as u8` discriminant, but the stream is plain bytes so an unknown value is
/// possible: it falls back to [`BlendMode::Normal`] and is logged, because substituting a
/// blend mode silently would change pixels the caller asked for.
pub(crate) fn blend_mode_from_id(mode: u8) -> BlendMode {
    const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];
    ALL.iter().copied().find(|m| *m as u8 == mode).unwrap_or_else(|| {
        log::warn!("[wgpu-raster] unknown blend mode tag {mode}; falling back to Normal");
        BlendMode::Normal
    })
}

/// Applies the blend formula to one source/destination pair. The separable modes are
/// evaluated per channel over 0..=1; the four non-separable ones (`Hue`, `Saturation`,
/// `Color`, `Luminosity`) are defined on the triple's luminance and are handled together in
/// the `Luminosity`-family branch below rather than inside the channel closure.
fn blend_channels(source: Rgba8, dest: Rgba8, mode: BlendMode) -> Rgba8 {
    /// Per-channel separable blend functions, on the 0..=1 scale.
    fn separable(mode: BlendMode, s: f32, d: f32) -> f32 {
        match mode {
            BlendMode::Normal => s,
            BlendMode::Multiply => s * d,
            BlendMode::Screen => s + d - s * d,
            BlendMode::Overlay => {
                if d <= 0.5 {
                    2.0 * s * d
                } else {
                    1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                }
            }
            BlendMode::Darken => s.min(d),
            BlendMode::Lighten => s.max(d),
            BlendMode::ColorDodge => {
                if d >= 1.0 {
                    1.0
                } else {
                    (s / (1.0 - d)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if d <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - s) / d).min(1.0)
                }
            }
            BlendMode::HardLight => {
                if s <= 0.5 {
                    2.0 * s * d
                } else {
                    1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                }
            }
            BlendMode::SoftLight => {
                // W3C compositing spec: the d <= 0.25 branch is a plain product,
                // above it a softened dodge with a square-root step at d > 0.5.
                if d <= 0.25 {
                    let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                    d - (1.0 - 2.0 * s) * delta
                } else if d <= 0.5 {
                    let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                    d + (2.0 * s - 1.0) * (delta - d)
                } else {
                    d + (2.0 * s - 1.0) * (d.sqrt() - d)
                }
            }
            BlendMode::Difference => (s - d).abs(),
            BlendMode::Exclusion => s + d - 2.0 * s * d,
            // Reached only for the non-separable four, which `blend_channels`
            // intercepts before calling this.
            BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => s,
        }
    }

    let to_f = |v: u8| v as f32 / 255.0;
    let (sr, sg, sb) = (to_f(source.r), to_f(source.g), to_f(source.b));
    let (dr, dg, db) = (to_f(dest.r), to_f(dest.g), to_f(dest.b));

    let (r, g, b) = match mode {
        // In Photoshop's naming, `Luminosity` takes the *source* luminance with the
        // destination hue/saturation; `Color` takes source hue/saturation with the
        // destination luminance; `Hue` and `Saturation` each take one component.
        BlendMode::Luminosity => set_luminosity((dr, dg, db), luminance(sr, sg, sb)),
        BlendMode::Color => set_luminosity((sr, sg, sb), luminance(dr, dg, db)),
        BlendMode::Hue => {
            set_luminosity(clip_color((sr, sg, sb), luminance(dr, dg, db)), luminance(dr, dg, db))
        }
        BlendMode::Saturation => set_luminosity(
            set_saturation((dr, dg, db), saturation(sr, sg, sb)),
            luminance(dr, dg, db),
        ),
        _ => (separable(mode, sr, dr), separable(mode, sg, dg), separable(mode, sb, db)),
    };

    // Alpha follows `Normal` semantics for every mode: the crate's blend modes
    // describe colour composition, and making them alter coverage as well would
    // change what is drawn, not just how it is coloured.
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba8 { r: q(r), g: q(g), b: q(b), a: source.a }
}

/// Rec. 601 luminance of an RGB triple on the 0..=1 scale.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.3 * r + 0.59 * g + 0.11 * b
}

/// Maximum-minus-minimum of an RGB triple, i.e. the saturation proxy the
/// non-separable blend modes are defined over.
fn saturation(r: f32, g: f32, b: f32) -> f32 {
    r.max(g).max(b) - r.min(g).min(b)
}

/// Rescales `rgb` so its luminance becomes `target`, preserving hue and saturation.
///
/// This is the standard "set luminance" primitive the non-separable modes are built
/// from. A flat triple has no colour to preserve, so every channel takes the target —
/// the usual fallback, and it also avoids dividing by a zero channel spread.
fn set_luminosity(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let current = luminance(r, g, b);
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let spread = max - min;
    if spread.abs() < f32::EPSILON {
        return (target, target, target);
    }
    // Shift the whole triple so its luminance lands on `target`; the offset keeps
    // the channel differences (hue and saturation) unchanged.
    let shift = target - current;
    (r + shift, g + shift, b + shift)
}

/// Pulls each channel of `rgb` back into 0..=1 without changing its luminance.
///
/// Needed after a non-separable blend, which can leave a channel out of range. The
/// comparison is against the triple's own luminance rather than a fixed pivot, so
/// the luminance survives the correction.
fn clip_color(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    if min < 0.0 {
        // Scale about the luminance: everything below it moves up proportionally.
        let denom = target - min;
        if denom.abs() < f32::EPSILON {
            return (target, target, target);
        }
        let adjust = |v: f32| target + (v - target) * (target / denom);
        return (adjust(r), adjust(g), adjust(b));
    }
    if max > 1.0 {
        let denom = max - target;
        if denom.abs() < f32::EPSILON {
            return (target, target, target);
        }
        let adjust = |v: f32| target + (v - target) * ((1.0 - target) / denom);
        return (adjust(r), adjust(g), adjust(b));
    }
    rgb
}

/// Rescales `rgb` to carry the given saturation, preserving luminance.
fn set_saturation(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    if max <= min {
        // Degenerate (greyscale) input: nothing to scale.
        return rgb;
    }
    let current = saturation(r, g, b);
    let scale = if current.abs() < f32::EPSILON {
        return rgb;
    } else {
        target / current
    };
    let mid = luminance(r, g, b);
    (mid + (r - mid) * scale, mid + (g - mid) * scale, mid + (b - mid) * scale)
}

/// Fills `rect` with `color`, compositing through `mode`.
///
/// The blended counterpart of [`fill_rect_cpu_rgba8`], used where the current blend
/// mode must be honoured.
pub(crate) fn fill_rect_cpu_rgba8_blended(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    color: Rgba8,
    mode: BlendMode,
) {
    if mode == BlendMode::Normal {
        fill_rect_cpu_rgba8(pixels, width, rect, color);
        return;
    }
    if width == 0 {
        return;
    }
    // Intersect the source rectangle with the canvas once. The inner loop then runs entirely
    // inside the framebuffer, so the row base and the row length are its only invariants and no
    // per-pixel bounds or sign test is needed (G-69).
    let height = (pixels.len() / (width as usize * 4)) as u32;
    let Some(draw) = rect.intersect(PixelRect { x: 0, y: 0, width, height }) else { return };
    let row_bytes = width as usize * 4;
    let span = draw.width as usize * 4;
    for y in draw.y..draw.bottom() {
        let offset = y as usize * row_bytes + draw.x as usize * 4;
        for pixel in pixels[offset..offset + span].as_chunks_mut::<4>().0 {
            let dest = Rgba8 { r: pixel[0], g: pixel[1], b: pixel[2], a: pixel[3] };
            let blended = blend_channels(color, dest, mode);
            *pixel = [blended.r, blended.g, blended.b, blended.a];
        }
    }
}

/// Draws a drop shadow into a private layer the size of the clamped blur area, blurs it there,
/// then composites it back (N-S-63). `body` is the solid rectangle and `paint_region` bounds the
/// feathered paint, so the blur can spread outside the body but never outside the caller's clip;
/// blurring the layer keeps the scene behind the shadow out of the source.
pub(crate) fn box_shadow_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    body: PixelRect,
    paint_region: PixelRect,
    color: Rgba8,
    blur_radius: u32,
    blend: BlendMode,
) {
    if width == 0 || height == 0 {
        return;
    }
    // Nothing to draw if the body does not intersect the framebuffer.
    let Some(visible) = body.intersect(PixelRect { x: 0, y: 0, width, height }) else { return };

    if blur_radius == 0 {
        // No blur: a plain fill of the body, respecting the blend mode.
        fill_rect_cpu_rgba8_blended(pixels, width, visible, color, blend);
        return;
    }

    let r = blur_radius as i64;
    // The layer must cover the body expanded by the blur radius, clamped to the
    // framebuffer, so the ramp has source pixels without reading outside the
    // allocation.
    let lx0 = (visible.x as i64 - r).max(0) as usize;
    let ly0 = (visible.y as i64 - r).max(0) as usize;
    let lx1 = ((visible.right() as i64) + r).clamp(0, width as i64) as usize;
    let ly1 = ((visible.bottom() as i64) + r).clamp(0, height as i64) as usize;
    if lx1 <= lx0 || ly1 <= ly0 {
        return;
    }
    let lw = lx1 - lx0;
    let lh = ly1 - ly0;

    // Independent, transparent shadow layer, retained by capacity between calls.
    SHADOW_LAYER.with(|cell| {
        let mut layer = cell.borrow_mut();
        let needed = lw * lh * 4;
        if layer.len() != needed {
            layer.resize(needed, 0);
        }
        // Fill the body and clear the surrounding ring in one pass. Every byte of the layer is
        // written below, so the previous `clear` + `resize` cost a second full memset for nothing
        // (G-67).
        let bx0 = (visible.x as i64 - lx0 as i64).max(0) as usize;
        let by0 = (visible.y as i64 - ly0 as i64).max(0) as usize;
        let bx1 = ((visible.right() as i64 - lx0 as i64).clamp(0, lw as i64)) as usize;
        let by1 = ((visible.bottom() as i64 - ly0 as i64).clamp(0, lh as i64)) as usize;
        let body = [color.r, color.g, color.b, color.a];
        for y in 0..lh {
            let in_body_y = y >= by0 && y < by1;
            for x in 0..lw {
                let o = (y * lw + x) * 4;
                let pixel = if in_body_y && x >= bx0 && x < bx1 { body } else { [0, 0, 0, 0] };
                layer[o..o + 4].copy_from_slice(&pixel);
            }
        }

        // Blur the layer in place (separable moving average, matching the software
        // backend's kernel).
        box_blur_layer_rgba8(&mut layer[..], lw, lh, blur_radius as usize);

        // Composite the layer back, but only inside `paint_region` (the caller's
        // clip, or the framebuffer): any pixel the caller clipped away must stay
        // exactly as it was (N-S-63). The body is opaque shadow colour, so a
        // source-over composite reads the layer's alpha as coverage.
        let Some(dst) = paint_region.intersect(PixelRect { x: 0, y: 0, width, height }) else {
            return;
        };
        // Each row paints at most one contiguous span — the destination row clipped to the
        // layer — so the per-pixel layer bounds checks collapse into two span endpoints
        // (G-225). Rows outside the layer are skipped whole; the blend logic is unchanged.
        for y in dst.y..dst.bottom() {
            let lyo = y as i64 - ly0 as i64;
            if lyo < 0 || lyo >= lh as i64 {
                continue;
            }
            let x_lo = (dst.x as i64).max(lx0 as i64);
            let x_hi = (dst.right() as i64).min(lx0 as i64 + lw as i64);
            if x_lo >= x_hi {
                continue;
            }
            let row = lyo as usize * lw;
            for x in x_lo..x_hi {
                let o = (row + (x - lx0 as i64) as usize) * 4;
                let alpha = layer[o + 3];
                if alpha == 0 {
                    continue;
                }
                let src = Rgba8 { r: layer[o], g: layer[o + 1], b: layer[o + 2], a: alpha };
                if alpha == 255 {
                    blend_pixel_cpu_rgba8(pixels, width, x as u32, y as u32, src, blend);
                } else {
                    // The ramp **is** the source alpha, so it is composited once: passing it as a
                    // coverage mask as well squared it and faded the shadow twice, which the
                    // GPU/CPU shadow comparison exposed (R4-2).
                    blend_cpu_rgba8(pixels, width, x as u32, y as u32, src, 1.0);
                }
            }
        }
    });
}

/// Box-blurs an RGBA buffer in place using a separable moving average. The squared kernel
/// matches the software backend, and separability keeps it O(1) per pixel instead of O(radius)
/// (G-67); the buffer is a self-contained layer, so the framebuffer is never read (N-S-63).
fn box_blur_layer_rgba8(layer: &mut [u8], width: usize, height: usize, radius: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    SHADOW_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let needed = layer.len();
        // The horizontal pass overwrites every pixel of the scratch, so a buffer that already
        // has the right length needs neither a resize nor a zero fill (G-67).
        if scratch.len() != needed {
            scratch.resize(needed, 0);
        }
        // Horizontal pass: `layer` -> `scratch`. The window starts as `[0, radius]` (clamped)
        // and slides right, adding the pixel that enters and removing the one that leaves.
        // Signed accumulators: the leaving pixel can be brighter than the entering one, so an
        // unsigned sum would underflow where the window stops growing and starts sliding.
        let half = radius;
        for y in 0..height {
            let row = y * width;
            let mut sums = [0i64; 4];
            for kx in 0..=half.min(width - 1) {
                let i = (row + kx) * 4;
                for c in 0..4 {
                    sums[c] += layer[i + c] as i64;
                }
            }
            for x in 0..width {
                let lo = x.saturating_sub(half);
                let hi = x.saturating_add(half).min(width - 1);
                let count = (hi - lo + 1) as i64;
                let di = (row + x) * 4;
                for c in 0..4 {
                    scratch[di + c] = (sums[c] / count) as u8;
                }
                if x + 1 == width {
                    break;
                }
                if x.saturating_add(1).saturating_add(half) < width {
                    let entering = (row + x + 1 + half) * 4;
                    for c in 0..4 {
                        sums[c] += layer[entering + c] as i64;
                    }
                }
                if x >= half {
                    let leaving = (row + x - half) * 4;
                    for c in 0..4 {
                        sums[c] -= layer[leaving + c] as i64;
                    }
                }
            }
        }
        // Vertical pass: `scratch` -> `layer`, with the same sliding window down each column.
        for x in 0..width {
            let mut sums = [0i64; 4];
            for ky in 0..=half.min(height - 1) {
                let i = (ky * width + x) * 4;
                for c in 0..4 {
                    sums[c] += scratch[i + c] as i64;
                }
            }
            for y in 0..height {
                let lo = y.saturating_sub(half);
                let hi = y.saturating_add(half).min(height - 1);
                let count = (hi - lo + 1) as i64;
                let di = (y * width + x) * 4;
                for c in 0..4 {
                    layer[di + c] = (sums[c] / count) as u8;
                }
                if y + 1 == height {
                    break;
                }
                if y.saturating_add(1).saturating_add(half) < height {
                    let entering = ((y + 1 + half) * width + x) * 4;
                    for c in 0..4 {
                        sums[c] += scratch[entering + c] as i64;
                    }
                }
                if y >= half {
                    let leaving = ((y - half) * width + x) * 4;
                    for c in 0..4 {
                        sums[c] -= scratch[leaving + c] as i64;
                    }
                }
            }
        }
    });
}

/// Writes pixels into the framebuffer through the rasteriser's active blend mode.
///
/// `SetBlendMode` is sticky and promised to affect *every* following draw, but only rects
/// and gradients honoured it — text, images, rounded rects, lines, circles, arcs and paths
/// all called `set_pixel_cpu_rgba8` directly and always wrote source-over. Threading this
/// sink through the helpers gives every variant the same blend behaviour (N-S-64).
pub(crate) struct PixelSink<'a> {
    pub(crate) pixels: &'a mut [u8],
    pub(crate) width: u32,
    pub(crate) mode: BlendMode,
}

impl PixelSink<'_> {
    /// Writes `color` at `(x, y)`, compositing with the active blend mode. `Normal` takes
    /// the fast direct-write path, so an unaware caller's output is byte-for-byte unchanged.
    #[inline]
    pub(crate) fn set(&mut self, x: u32, y: u32, color: Rgba8) {
        blend_pixel_cpu_rgba8(self.pixels, self.width, x, y, color, self.mode);
    }
}

pub(crate) fn set_pixel_cpu_rgba8(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Rgba8) {
    // Widened to `usize` before multiplying: in `u32` the product wraps for a
    // large `y`, which in debug panics and in release silently writes a
    // correctly-in-range but wrong pixel. The bounds check then makes the
    // remaining out-of-range case (geometry beyond the framebuffer) a no-op
    // instead of a panic, matching the callers' pre-clipping expectation.
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    if offset + 3 >= pixels.len() {
        return;
    }
    pixels[offset] = color.r;
    pixels[offset + 1] = color.g;
    pixels[offset + 2] = color.b;
    pixels[offset + 3] = color.a;
}

pub(crate) fn blend_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Rgba8,
    coverage: f32,
) {
    blend_cpu_rgba8_with_mode(pixels, width, x, y, color, coverage, BlendMode::Normal)
}

/// As [`blend_cpu_rgba8`], but first composites `color` with the destination through
/// `mode`. The coverage path serves anti-aliased glyph edges; without the mode it would
/// bypass `SetBlendMode` entirely (opaque glyph interiors honoured it, their edges did
/// not), so a Multiply text draw came out half-applied (N-S-64). The blended colour is
/// computed against the current destination, then source-over composited by `coverage`.
pub(crate) fn blend_cpu_rgba8_with_mode(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Rgba8,
    coverage: f32,
    mode: BlendMode,
) {
    // The same source-over arithmetic `render::blend_pixel` performs, on this module's flat RGBA
    // byte order. It exists because this path blends un-premultiplied coverage from a glyph's
    // paint, which the opaque `set_pixel_cpu_rgba8` cannot express.
    if coverage <= 0.0 {
        return;
    }
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    if offset + 3 >= pixels.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a >= 1.0 {
        pixels[offset] = color.r;
        pixels[offset + 1] = color.g;
        pixels[offset + 2] = color.b;
        pixels[offset + 3] = 255;
        return;
    }
    let dst = [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]];
    // Apply the blend mode against the destination first, then composite the
    // result by coverage. `Normal` leaves the source unchanged.
    let color = if mode == BlendMode::Normal {
        color
    } else {
        let dest = Rgba8 { r: dst[0], g: dst[1], b: dst[2], a: dst[3] };
        blend_channels(color, dest, mode)
    };
    let mix = |src: u8, dst: u8| (src as f32 * src_a + dst as f32 * (1.0 - src_a)).round() as u8;
    pixels[offset] = mix(color.r, dst[0]);
    pixels[offset + 1] = mix(color.g, dst[1]);
    pixels[offset + 2] = mix(color.b, dst[2]);
    pixels[offset + 3] =
        (dst[3] as f32 + 255.0 * src_a * (1.0 - dst[3] as f32 / 255.0)).round() as u8;
}
