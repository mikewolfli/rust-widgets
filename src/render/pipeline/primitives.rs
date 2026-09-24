// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Software rendering primitives: rect, rounded rect, circle, line, arc, path,
//! polygon, triangle, text, and image drawing methods.
//!
//! Methods are implemented as `impl SoftwareSurface` blocks.

use crate::compat::Vec;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::render::pipeline::pixel_ops::{
    blend_painted_glyph, blend_pixel, circle_fill_coverage_grid, circle_stroke_coverage_grid,
    inset_rect, line_stroke_coverage_grid, pixel_visible, rounded_rect_coverage,
    rounded_rect_coverage_grid, rounded_rect_effective_radius, set_pixel, GlyphDrawConfig,
};
// Imported separately because it only exists when a colour face does — see its own docs.
#[cfg(feature = "fonts-emoji-color")]
use crate::render::pipeline::pixel_ops::blend_color_glyph;
use crate::render::text::{is_combining_mark, is_variation_selector};
use crate::render::SoftwareSurface;

macro_rules! set_pixel_clipped {
    ($clip:expr, $frame:expr, $width:expr, $x:expr, $y:expr, $color:expr) => {
        if pixel_visible($clip, $x as i32, $y as i32) {
            set_pixel($frame, $width, $x as u32, $y as u32, $color);
        }
    };
}

macro_rules! blend_pixel_clipped {
    ($clip:expr, $frame:expr, $width:expr, $x:expr, $y:expr, $color:expr, $coverage:expr) => {
        if pixel_visible($clip, $x as i32, $y as i32) {
            blend_pixel($frame, $width, $x as u32, $y as u32, $color, $coverage);
        }
    };
}

impl SoftwareSurface {
    /// Fills a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let size = self.buffer.size();
        let clip = self.current_clip();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = rect.right().max(0) as u32;
        let y1 = rect.bottom().max(0) as u32;
        let x1 = x1.min(size.width);
        let y1 = y1.min(size.height);
        let frame = self.buffer.back_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                set_pixel_clipped!(clip, frame, size.width, x, y, color);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, coverage);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, coverage);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, stroke_coverage);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, stroke_coverage);
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
                        set_pixel_clipped!(clip, frame, width, px, py, color);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, coverage);
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
        for y in -r..=r {
            let y2 = y * y;
            if y2 > r * r {
                continue;
            }
            let span = ((r * r - y2) as f32).sqrt() as i32;
            let py = center.y + y;
            if py < 0 || py >= height {
                continue;
            }
            for x in -span..=span {
                let px = center.x + x;
                if px < 0 || px >= width {
                    continue;
                }
                set_pixel_clipped!(clip, frame, size.width, px, py, color);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, coverage);
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
                    blend_pixel_clipped!(clip, frame, size.width, px, py, color, stroke_coverage);
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

        for y in min_y..=max_y {
            let mut intersections: Vec<f32> = Vec::new();
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
                    set_pixel_clipped!(clip, frame, size.width, x, y, color);
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
                    set_pixel_clipped!(clip, frame, size.width, x, a.y, color);
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
                    set_pixel_clipped!(clip, frame, size.width, x, y, color);
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
        let metrics = self.measure_text(text, font);
        if metrics.width == 0 || metrics.height == 0 {
            return;
        }
        let shaped = self.shape_text(text, font);
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
        //
        // Read before the frame borrow below, because `dpi_scale` borrows `self` immutably.
        let scale = self.dpi_scale();
        let tracking = font.letter_spacing() * scale;
        let last_index = shaped.clusters().len().saturating_sub(1);
        let frame = self.buffer.back_mut();
        // One scratch for the whole line, sized for the tallest and widest cell it needs: a glyph
        // is rasterised into it, blended, and overwritten by the next — so no glyph is ever
        // resident, and a line allocates at most once (and never for the 8x8 face, whose cells fit
        // a small const buffer).
        let max_cell = shaped
            .clusters()
            .iter()
            .map(|cluster| {
                let w = cluster.advance.max(1.0).round() as u32;
                (w as usize) * (glyph_height as usize)
            })
            .max()
            .unwrap_or(0);
        let mut coverage: crate::compat::Vec<u8> = crate::compat::vec![0u8; max_cell];
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
    }
    /// Draws an RGBA image at the specified position and size.
    /// Pixels are copied directly from the source data, performing alpha blending.
    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        let size = self.buffer.size();
        let clip = self.current_clip();
        if width == 0 || height == 0 {
            return;
        }
        let expected_len = (width as usize) * (height as usize) * 4;
        if data.len() < expected_len {
            return;
        }
        let frame = self.buffer.back_mut();
        let screen_width = size.width;
        let screen_height = size.height;
        for row in 0..height {
            let sy = y + row as i32;
            if sy < 0 || sy as u32 >= screen_height {
                continue;
            }
            for col in 0..width {
                let sx = x + col as i32;
                if sx < 0 || sx as u32 >= screen_width {
                    continue;
                }
                if !pixel_visible(clip, sx, sy) {
                    continue;
                }
                let src_idx = ((row as usize) * (width as usize) + (col as usize)) * 4;
                let r = data[src_idx];
                let g = data[src_idx + 1];
                let b = data[src_idx + 2];
                let a = data[src_idx + 3];
                let dst_idx = ((sy as u32) * screen_width + (sx as u32)) as usize * 4;
                if a == 255 {
                    frame[dst_idx] = r;
                    frame[dst_idx + 1] = g;
                    frame[dst_idx + 2] = b;
                    frame[dst_idx + 3] = 255;
                } else if a > 0 {
                    let src_alpha = a as f32 / 255.0;
                    let dst_alpha = frame[dst_idx + 3] as f32 / 255.0;
                    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
                    if out_alpha > 0.0 {
                        let src_weight = src_alpha / out_alpha;
                        let dst_weight = dst_alpha * (1.0 - src_alpha) / out_alpha;
                        frame[dst_idx] =
                            (r as f32 * src_weight + frame[dst_idx] as f32 * dst_weight) as u8;
                        frame[dst_idx + 1] =
                            (g as f32 * src_weight + frame[dst_idx + 1] as f32 * dst_weight) as u8;
                        frame[dst_idx + 2] =
                            (b as f32 * src_weight + frame[dst_idx + 2] as f32 * dst_weight) as u8;
                        frame[dst_idx + 3] = (out_alpha * 255.0) as u8;
                    }
                }
            }
        }
    }
}
