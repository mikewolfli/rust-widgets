//! Container utilities: SoftwareSurface lifecycle management, configuration,
//! frame operations, text shaping, gradient fill, and clip stack.
//!
//! Rendering primitives (rect, circle, line, text, etc.) are in the
//! `primitives` sub-module.

use crate::core::{Color, Font, Rect, Size};
use crate::render::default_software_render_config;
use crate::render::pipeline::pixel_ops::{
    cluster_ends_with_zwj, estimate_cluster_advance, fill_pixels, is_combining_mark,
    is_variation_selector, set_pixel,
};
use crate::render::{
    BackBuffer, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextCluster, TextMetrics,
};
use crate::style::Gradient;
use crate::style::GradientType;

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
    /// Pushes a clip rectangle onto the clip stack.
    /// The clip rectangle is intersected with any existing clip region.
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        if let Some(&(cx, cy, cw, ch)) = self.clip_stack.last() {
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
