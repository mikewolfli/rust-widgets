// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Software rendering primitives: rect, rounded rect, circle, line, arc, path,
//! polygon, triangle, text, and image drawing methods.
//!
//! Methods are implemented as `impl SoftwareSurface` blocks.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::pipeline::pixel_ops::{
    blend_painted_glyph, circle_fill_coverage_grid, circle_stroke_coverage_grid, inset_rect,
    line_stroke_coverage_grid, pixel_visible, rounded_rect_coverage, rounded_rect_coverage_grid,
    rounded_rect_effective_radius, write_pixel_with_mode, GlyphDrawConfig,
};
use crate::render::BlendMode;
// Imported separately because it only exists when a colour face does — see its own docs.
#[cfg(feature = "fonts-emoji-color")]
use crate::render::pipeline::pixel_ops::blend_color_glyph;
use crate::render::text::{is_combining_mark, is_variation_selector};
use crate::render::SoftwareSurface;

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
            for chunk in frame[start..start + row_len].chunks_exact_mut(4) {
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
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
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
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = rounded_rect_coverage(px, py, rect, effective_radius);
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
                    rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
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
                let outer_coverage = rounded_rect_coverage(px, py, rect, effective_radius);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage(px, py, inner, inner_radius)
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
                    rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
                if outer_coverage <= 0.0 {
                    continue;
                }
                let inner_coverage = if has_inner {
                    rounded_rect_coverage_grid(px, py, inner, inner_radius, sample_grid)
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
        let width = size.width;
        let height = size.height;
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let brush_start = -(stroke_width as i32 / 2);
        let brush_end = brush_start + stroke_width as i32 - 1;
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
            for oy in brush_start..=brush_end {
                for ox in brush_start..=brush_end {
                    let px = x0 + ox;
                    let py = y0 + oy;
                    if px >= 0 && py >= 0 && (px as u32) < width && (py as u32) < height {
                        set_pixel_clipped!(self.blend_mode, clip, frame, width, px, py, color);
                    }
                }
            }
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
    /// Angles are normalized to [0, 2π) and the sweep is capped at 2π to
    /// prevent excessive point generation for very large angle ranges.
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
        const TWO_PI: f32 = core::f32::consts::TAU;
        // Normalize both angles to [0, 2π)
        let mut start = start_angle % TWO_PI;
        if start < 0.0 {
            start += TWO_PI;
        }
        let mut end = end_angle % TWO_PI;
        if end < 0.0 {
            end += TWO_PI;
        }
        // Cap the sweep to at most one full circle
        let step = 0.02f32;
        let mut angle = start;
        let mut points: Vec<Point> = Vec::new();
        if start <= end {
            while angle <= end && angle <= start + TWO_PI {
                let x = center.x + (radius as f32 * angle.cos()).round() as i32;
                let y = center.y + (radius as f32 * angle.sin()).round() as i32;
                points.push(Point::new(x, y));
                angle += step;
            }
        } else {
            // Wraparound case: e.g. start=350° to end=10°
            while angle < TWO_PI && angle <= start + TWO_PI {
                let x = center.x + (radius as f32 * angle.cos()).round() as i32;
                let y = center.y + (radius as f32 * angle.sin()).round() as i32;
                points.push(Point::new(x, y));
                angle += step;
            }
            angle = 0.0;
            while angle <= end && angle <= TWO_PI {
                let x = center.x + (radius as f32 * angle.cos()).round() as i32;
                let y = center.y + (radius as f32 * angle.sin()).round() as i32;
                points.push(Point::new(x, y));
                angle += step;
            }
        }
        let last_x = center.x + (radius as f32 * end.cos()).round() as i32;
        let last_y = center.y + (radius as f32 * end.sin()).round() as i32;
        points.push(Point::new(last_x, last_y));

        if filled {
            for i in 0..points.len().saturating_sub(1) {
                self.fill_triangle(center, points[i], points[i + 1], color);
            }
        } else {
            for i in 0..points.len().saturating_sub(1) {
                self.draw_line_with_width(points[i], points[i + 1], color, 1);
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
                if winding != 0 {
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
    /// Fills a polygon using a scanline algorithm.
    fn fill_polygon(&mut self, points: &[Point], color: Color) {
        let size = self.buffer.size();
        let clip = self.current_clip();
        let frame = self.buffer.back_mut();
        let n = points.len();
        if n < 3 {
            return;
        }

        let min_y = points.iter().map(|p| p.y).min().unwrap_or(0).max(0);
        let max_y = points.iter().map(|p| p.y).max().unwrap_or(0).min(size.height as i32 - 1);

        let mut intersections: Vec<f32> = Vec::with_capacity(n);
        for y in min_y..=max_y {
            intersections.clear();
            let mut j = n - 1;
            for i in 0..n {
                let p1 = points[i];
                let p2 = points[j];
                if (p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y) {
                    let t = (y - p1.y) as f32 / (p2.y - p1.y) as f32;
                    let x = p1.x as f32 + (p2.x - p1.x) as f32 * t;
                    intersections.push(x);
                }
                j = i;
            }

            intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));

            let mut i = 0;
            while i + 1 < intersections.len() {
                let x_start = intersections[i].ceil() as i32;
                let x_end = intersections[i + 1].floor() as i32;
                let x_start = x_start.max(0);
                let x_end = x_end.min(size.width as i32 - 1);
                for x in x_start..=x_end {
                    set_pixel_clipped!(self.blend_mode, clip, frame, size.width, x, y, color);
                }
                i += 2;
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
        // One scratch for the whole line, sized for the tallest and widest cell it needs: a glyph
        // is rasterised into it, blended, and overwritten by the next — so no glyph is ever
        // resident. The buffer is owned by the surface and reused across lines and frames; a
        // per-glyph `vec![0u8; ..]` was one allocation per character per frame.
        let max_cell = shaped
            .clusters()
            .iter()
            .map(|cluster| {
                let w = cluster.advance.max(1.0).round() as u32;
                (w as usize) * (glyph_height as usize)
            })
            .max()
            .unwrap_or(0);
        let mut coverage = core::mem::take(&mut self.text_coverage_scratch);
        if coverage.len() < max_cell {
            coverage.resize(max_cell, 0);
        }
        let frame = self.buffer.back_mut();
        // A **second** scratch, four bytes per pixel, used only when the active stack resolves a
        // cluster through a colour face. It is allocated once per line and only when the build has
        // such a face, so a build without one pays nothing. Keeping it separate from `coverage`
        // rather than sizing `coverage` at `max_cell * 4` unconditionally is what keeps the 8x8
        // path's allocation exactly as it was — every existing snapshot stays byte-identical.
        #[cfg(feature = "fonts-emoji-color")]
        let mut color_scratch: crate::compat::Vec<u8> =
            crate::compat::vec![0u8; max_cell.saturating_mul(4)];
        for (index, cluster) in shaped.clusters().iter().enumerate() {
            let glyph_width = cluster.advance.max(1.0).round() as i32;
            let display_char = cluster
                .text
                .chars()
                .find(|ch| !is_combining_mark(*ch) && !is_variation_selector(*ch));
            if let Some(ch) = display_char {
                let mut config = GlyphDrawConfig {
                    canvas: &mut *frame,
                    canvas_width: size.width,
                    canvas_height: size.height,
                    w: glyph_width as u32,
                    h: glyph_height as u32,
                    color,
                    clip,
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
        // Give the scratch back for the next line. Held across the loop above so a glyph can
        // overwrite it in place without reallocating.
        self.text_coverage_scratch = coverage;
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
        let expected_len = (width as usize) * (height as usize) * 4;
        if data.len() < expected_len {
            return;
        }
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
            if source.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255) {
                frame[destination_start..destination_start + columns * 4].copy_from_slice(source);
                continue;
            }
            for (offset, pixel) in source.as_chunks::<4>().0.iter().enumerate() {
                blend_source_pixel(
                    frame,
                    screen_width,
                    (left as usize + offset) as u32,
                    row as u32,
                    pixel[0],
                    pixel[1],
                    pixel[2],
                    pixel[3],
                );
            }
        }
    }

    /// Draws an RGBA image mapped onto the parallelogram spanned by `corners`.
    ///
    /// `corners` are the destination positions of the source grid's top-left, top-right,
    /// bottom-right and bottom-left corners, in perimeter order. `source_width` and
    /// `source_height` describe the grid the data was laid out in before the transform, which is
    /// the rectangle the batch path drew under a 2D transform.
    ///
    /// # Why the inverse mapping is two divisions
    ///
    /// An affine transform maps a rectangle to a parallelogram, so the destination point
    /// `p = p00 + u * (p10 - p00) + v * (p01 - p00)` inverts with the determinant of the two edge
    /// vectors. Each destination pixel is then sampled from the source grid at `(u, v)`. The
    /// fourth corner is not read: for a parallelogram it is implied, and a projective quad is
    /// not something an affine transform can produce.
    ///
    /// Sampling is **bilinear**, which is what the plan for the affine layer specifies: a
    /// rotated or scaled image is resampled, not block-copied. At an identity transform the
    /// destination pixel centres land exactly on source texel centres, so the weights are `0`/`1`
    /// and the result is bit-for-bit the axis-aligned copy — the interpolation can only soften a
    /// picture the transform actually moved.
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
        // Pixel centres, so a corner pixel samples its own centre rather than its top-left edge.
        for y in min_y..=max_y {
            if y < 0 || y as u32 >= screen_height {
                continue;
            }
            for x in min_x..=max_x {
                if x < 0 || x as u32 >= screen_width {
                    continue;
                }
                if !pixel_visible(clip, x, y) {
                    continue;
                }
                let dx = (x - p00.x) as f32 + 0.5;
                let dy = (y - p00.y) as f32 + 0.5;
                let u = (dx * vy - dy * vx) / det;
                let v = (dy * ux - dx * uy) / det;
                if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                    continue;
                }
                let [r, g, b, a] = sample_bilinear(data, source_width, source_height, u, v);
                blend_source_pixel(frame, screen_width, x as u32, y as u32, r, g, b, a);
            }
        }
    }
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
    if a == 0 {
        return;
    }
    let dst_idx = ((y * screen_width + x) as usize) * 4;
    if a == 255 {
        frame[dst_idx] = r;
        frame[dst_idx + 1] = g;
        frame[dst_idx + 2] = b;
        frame[dst_idx + 3] = 255;
        return;
    }
    let src_alpha = a as f32 / 255.0;
    let dst_alpha = frame[dst_idx + 3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= 0.0 {
        return;
    }
    let src_weight = src_alpha / out_alpha;
    let dst_weight = dst_alpha * (1.0 - src_alpha) / out_alpha;
    frame[dst_idx] = (r as f32 * src_weight + frame[dst_idx] as f32 * dst_weight) as u8;
    frame[dst_idx + 1] = (g as f32 * src_weight + frame[dst_idx + 1] as f32 * dst_weight) as u8;
    frame[dst_idx + 2] = (b as f32 * src_weight + frame[dst_idx + 2] as f32 * dst_weight) as u8;
    frame[dst_idx + 3] = (out_alpha * 255.0) as u8;
}
