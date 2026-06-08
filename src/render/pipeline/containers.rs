//! Container and complex widgets: tab_widget, text_edit, rich_edit, tree_view,
//! table_widget, grid_widget, chart_widget, dock_panel, group_box, splitter, mdi_area,
//! canvas, spin_box, list_view, scroll_area.
//!
//! Also contains the `impl SoftwareSurface` block with all software rendering methods.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::default_software_render_config;
use crate::render::pipeline::pixel_ops::{
    blend_pixel, circle_fill_coverage_grid, circle_stroke_coverage_grid, cluster_ends_with_zwj,
    draw_bitmap_glyph, estimate_cluster_advance, fill_pixels, inset_rect, is_combining_mark,
    is_variation_selector, line_stroke_coverage_grid, rounded_rect_coverage,
    rounded_rect_coverage_grid, rounded_rect_effective_radius, set_pixel, GlyphDrawConfig,
};
use crate::render::{
    BackBuffer, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextCluster, TextMetrics,
};
use crate::style::{Gradient, GradientType};

impl SoftwareSurface {
    /// Creates a software surface with size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let config = default_software_render_config();
        Self {
            buffer: BackBuffer::new(size, dpi_scale),
            aa_samples_per_axis: config.aa_samples_per_axis,
            clip_stack: Vec::new(),
        }
    }
    /// Get current software render configuration.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig { aa_samples_per_axis: self.aa_samples_per_axis }
    }
    /// Apply software render configuration.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        let normalized = config.normalized();
        self.aa_samples_per_axis = normalized.aa_samples_per_axis;
    }
    /// Set anti-aliasing sample grid size per axis for high-sample raster paths.
    pub fn set_aa_samples_per_axis(&mut self, samples: u8) {
        self.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: samples });
    }
    /// Get anti-aliasing sample grid size per axis.
    pub fn aa_samples_per_axis(&self) -> u8 {
        self.aa_samples_per_axis
    }
    /// Clears the current back buffer with a solid color.
    pub fn begin_frame(&mut self, clear: Color) {
        fill_pixels(self.buffer.back_mut(), clear);
    }
    /// Presents the back buffer as the current frame.
    pub fn end_frame(&mut self) {
        self.buffer.present();
    }
    /// Returns logical surface size.
    pub fn size(&self) -> Size {
        self.buffer.size()
    }
    /// Resizes the surface buffers.
    pub fn resize(&mut self, size: Size) {
        self.buffer.resize(size);
    }
    /// Sets logical DPI scale for text and geometry.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.buffer.set_dpi_scale(dpi_scale);
    }
    /// Returns logical DPI scale.
    pub fn dpi_scale(&self) -> f32 {
        self.buffer.dpi_scale()
    }
    /// Returns RGBA bytes of the presented frame.
    pub fn frame_rgba(&self) -> &[u8] {
        self.buffer.front()
    }
    /// Measures text bounds and baseline metrics.
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        let scale = self.buffer.dpi_scale();
        let line_height = (font.size * scale).max(1.0);
        let ascent = (line_height * 0.8) as u32;
        let descent = (line_height - ascent as f32).max(0.0) as u32;
        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        TextMetrics { width, height: line_height.round() as u32, ascent, descent }
    }
    /// Shape text into unicode-aware clusters with logical advances.
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        let scale = self.buffer.dpi_scale();
        let mut clusters: Vec<TextCluster> = Vec::new();
        for scalar in text.chars() {
            let should_merge = clusters
                .last()
                .map(|cluster| {
                    cluster_ends_with_zwj(cluster)
                        || scalar == '\u{200D}'
                        || is_combining_mark(scalar)
                        || is_variation_selector(scalar)
                })
                .unwrap_or(false);
            if should_merge {
                if let Some(last) = clusters.last_mut() {
                    last.text.push(scalar);
                }
            } else {
                clusters.push(TextCluster { text: scalar.to_string(), advance: 0.0 });
            }
        }
        let mut total_advance = 0.0f32;
        for cluster in &mut clusters {
            cluster.advance = estimate_cluster_advance(&cluster.text, font.size, scale);
            total_advance += cluster.advance;
        }
        ShapedText { clusters, advance: total_advance }
    }
    /// Fills a rectangle with a gradient.
    pub fn fill_rect_gradient(&mut self, rect: Rect, gradient: &Gradient) {
        let size = self.buffer.size();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width as f32 as i32).max(0) as u32;
        let y1 = (rect.y + rect.height as f32 as i32).max(0) as u32;
        let x1 = x1.min(size.width);
        let y1 = y1.min(size.height);
        let frame = self.buffer.back_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                let pos = match gradient.gradient_type {
                    GradientType::Linear => {
                        // Project pixel onto start→end line
                        let dx = gradient.end_point.x as f32 - gradient.start_point.x as f32;
                        let dy = gradient.end_point.y as f32 - gradient.start_point.y as f32;
                        let dot_len = dx * dx + dy * dy;
                        if dot_len == 0.0 {
                            0.0
                        } else {
                            let px = x as f32 - gradient.start_point.x as f32;
                            let py = y as f32 - gradient.start_point.y as f32;
                            ((px * dx + py * dy) / dot_len).clamp(0.0, 1.0)
                        }
                    }
                    GradientType::Radial => {
                        let dx = x as f32 - gradient.center.x as f32;
                        let dy = y as f32 - gradient.center.y as f32;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if gradient.radius <= 0.0 {
                            0.0
                        } else {
                            (dist / gradient.radius).clamp(0.0, 1.0)
                        }
                    }
                    GradientType::Conic => {
                        let dx = x as f32 - gradient.center.x as f32;
                        let dy = y as f32 - gradient.center.y as f32;
                        let angle = dy.atan2(dx) + std::f32::consts::PI;
                        let angle = (angle + gradient.angle) % (2.0 * std::f32::consts::PI);
                        angle / (2.0 * std::f32::consts::PI)
                    }
                };
                let color = gradient.interpolate(pos);
                set_pixel(frame, size.width, x, y, color);
            }
        }
    }

    /// Fills a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let size = self.buffer.size();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width as f32 as i32).max(0) as u32;
        let y1 = (rect.y + rect.height as f32 as i32).max(0) as u32;
        let x1 = x1.min(size.width);
        let y1 = y1.min(size.height);
        let frame = self.buffer.back_mut();
        for y in y0..y1 {
            for x in x0..x1 {
                set_pixel(frame, size.width, x, y, color);
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
        let x1 = rect.x + rect.width as f32 as i32 - 1;
        let y1 = rect.y + rect.height as f32 as i32 - 1;
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
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = rounded_rect_coverage(px, py, rect, effective_radius);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
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
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage =
                    rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
                if coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
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
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
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
                    blend_pixel(frame, size.width, px as u32, py as u32, color, stroke_coverage);
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
        let frame = self.buffer.back_mut();
        let x0 = rect.x.max(0);
        let y0 = rect.y.max(0);
        let x1 = (rect.x + rect.width as f32 as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as f32 as i32 - 1).min(height - 1);
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
                    blend_pixel(frame, size.width, px as u32, py as u32, color, stroke_coverage);
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
                        set_pixel(frame, width, px as u32, py as u32, color);
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
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
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
                set_pixel(frame, size.width, px as u32, py as u32, color);
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
                    blend_pixel(frame, size.width, px as u32, py as u32, color, coverage);
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
        let frame = self.buffer.back_mut();
        let ring_radius = radius as f32;
        // let ring_half_width = stroke_width as f32 / 2.0; // unused
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
                    blend_pixel(frame, size.width, px as u32, py as u32, color, stroke_coverage);
                }
            }
        }
    }
    /// Draws an arc (partial circle).
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
        let step = 0.02f32;
        let mut angle = start_angle;
        let mut points: Vec<Point> = Vec::new();
        if start_angle <= end_angle {
            while angle <= end_angle {
                let x = center.x + (radius as f32 * angle.cos()).round() as i32;
                let y = center.y + (radius as f32 * angle.sin()).round() as i32;
                points.push(Point::new(x, y));
                angle += step;
            }
        } else {
            while angle >= end_angle {
                let x = center.x + (radius as f32 * angle.cos()).round() as i32;
                let y = center.y + (radius as f32 * angle.sin()).round() as i32;
                points.push(Point::new(x, y));
                angle -= step;
            }
        }
        // Ensure end angle point is included
        let last_x = center.x + (radius as f32 * end_angle.cos()).round() as i32;
        let last_y = center.y + (radius as f32 * end_angle.sin()).round() as i32;
        points.push(Point::new(last_x, last_y));

        if filled {
            // Triangle fan from center to consecutive arc points
            for i in 0..points.len().saturating_sub(1) {
                self.fill_triangle(center, points[i], points[i + 1], color);
            }
        } else {
            // Line segments along the arc
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
            // Draw line segments between consecutive points
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
        let frame = self.buffer.back_mut();
        let n = points.len();
        if n < 3 {
            return;
        }

        // Find min/max y
        let min_y = points.iter().map(|p| p.y).min().unwrap_or(0).max(0);
        let max_y = points.iter().map(|p| p.y).max().unwrap_or(0).min(size.height as i32 - 1);

        for y in min_y..=max_y {
            // Build intersection list
            let mut intersections: Vec<f32> = Vec::new();
            let mut j = n - 1;
            for i in 0..n {
                let p1 = points[i];
                let p2 = points[j];
                // Check if edge crosses scanline y
                if (p1.y <= y && p2.y > y) || (p2.y <= y && p1.y > y) {
                    // Edge crosses scanline, compute x intersection
                    let t = (y - p1.y) as f32 / (p2.y - p1.y) as f32;
                    let x = p1.x as f32 + (p2.x - p1.x) as f32 * t;
                    intersections.push(x);
                }
                j = i;
            }

            // Sort intersections
            intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Fill between pairs (pairity fill)
            let mut i = 0;
            while i + 1 < intersections.len() {
                let x_start = intersections[i].ceil() as i32;
                let x_end = intersections[i + 1].floor() as i32;
                let x_start = x_start.max(0);
                let x_end = x_end.min(size.width as i32 - 1);
                for x in x_start..=x_end {
                    set_pixel(frame, size.width, x as u32, y as u32, color);
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
        let frame = self.buffer.back_mut();

        if a.y == c.y {
            let min_x = a.x.min(b.x).min(c.x);
            let max_x = a.x.max(b.x).max(c.x);
            if max_x > min_x {
                for x in min_x.max(0)..=max_x.min(size.width as i32 - 1) {
                    set_pixel(frame, size.width, x as u32, a.y as u32, color);
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
                    set_pixel(frame, size.width, x as u32, y as u32, color);
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
        let frame = self.buffer.back_mut();
        for cluster in shaped.clusters() {
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
                    ch: ch as u8,
                    x: pen_x.round() as i32,
                    y: origin.y,
                    w: glyph_width as u32,
                    h: glyph_height as u32,
                    color,
                };
                draw_bitmap_glyph(&mut config);
            }
            pen_x += cluster.advance;
        }
    }
    /// Draws an RGBA image at the specified position and size.
    /// Pixels are copied directly from the source data, performing alpha blending.
    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        let size = self.buffer.size();
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
    /// Pushes a clip rectangle onto the clip stack.
    /// The clip rectangle is intersected with any existing clip region.
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        if let Some(&(cx, cy, cw, ch)) = self.clip_stack.last() {
            // Intersect with current clip region.
            let nx = x.max(cx);
            let ny = y.max(cy);
            let nx2 = (x.saturating_add(width as i32)).min(cx.saturating_add(cw as i32));
            let ny2 = (y.saturating_add(height as i32)).min(cy.saturating_add(ch as i32));
            let nw = (nx2 - nx).max(0) as u32;
            let nh = (ny2 - ny).max(0) as u32;
            self.clip_stack.push((nx, ny, nw, nh));
        } else {
            self.clip_stack.push((x, y, width, height));
        }
    }
    /// Pops the top clip rectangle from the clip stack.
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }
    /// Returns the current effective clip rect, or `None` if the clip stack is empty.
    pub fn current_clip(&self) -> Option<(i32, i32, u32, u32)> {
        self.clip_stack.last().copied()
    }
}
