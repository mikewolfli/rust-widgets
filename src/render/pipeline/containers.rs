// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Container utilities: SoftwareSurface lifecycle management, configuration,
//! frame operations, text shaping, gradient fill, and clip stack.
//!
//! Rendering primitives (rect, circle, line, text, etc.) are in the
//! `primitives` sub-module.

use crate::compat::Vec;
use crate::core::{Color, Font, Rect, Size};
use crate::render::default_software_render_config;
use crate::render::pipeline::pixel_ops::{fill_pixels, pixel_visible, write_pixel_with_mode};
use crate::render::{
    BackBuffer, BlendMode, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextMetrics,
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
            blend_mode: BlendMode::Normal,
        }
    }

    /// Sets the blend mode every subsequent pixel write composites through.
    ///
    /// Driven by [`crate::render::RenderCommand::SetBlendMode`]. The mode is frame state, like the
    /// clip stack, because it applies to everything drawn until it changes rather than to one
    /// primitive. `Normal` restores plain source-over compositing.
    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    /// The mode the next pixel write will composite through.
    pub fn blend_mode(&self) -> BlendMode {
        self.blend_mode
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
        // The line box is the font's **effective** line height, so an explicit `line_height` — the
        // field a text scale needs, and the one that had no consumer when it was added (BLUE22 ·
        // F-10) — actually sets how tall a line of this font is. Deriving it from `size()` alone made
        // the field writable and unread: a caller could set 1.8 em leading and every `text_line` in
        // the crate would still centre on the default, so the field was decoration.
        //
        // The height/ascent/descent triple comes from `TextMetrics::for_font` rather than being
        // derived here, because the SVG backend derives the same triple and the two had drifted
        // (see that function for the measured difference).
        let shaped = self.shape_text(text, font);
        let width = shaped.advance().round() as u32;
        TextMetrics { width, ..TextMetrics::for_font(font, scale) }
    }
    /// Shape text into unicode-aware clusters, in **visual** order for painting.
    ///
    /// The derivation itself lives in [`crate::render::text`] (`shape_line`), so this backend and
    /// the SVG backend cannot drift: a change to clustering, advances or bidirectional reordering
    /// reaches both at once. All this method supplies is the device scale it already knows.
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        crate::render::text::shape_line(text, font, self.buffer.dpi_scale())
    }
    /// Fills a rectangle with a gradient.
    pub fn fill_rect_gradient(&mut self, rect: Rect, gradient: &Gradient) {
        let size = self.buffer.size();
        let clip = self.current_clip();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = rect.x.saturating_add(rect.width as i32).max(0) as u32;
        let y1 = rect.y.saturating_add(rect.height as i32).max(0) as u32;
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
                        // The per-pixel angle is in **radians** (that is what `atan2` returns),
                        // while `Gradient::angle` is documented in **degrees** — so it is converted
                        // before being added. Adding it raw rotated the ramp by `angle` *radians*
                        // (~117° for the common `conic(center, 90.0)`) and made a caller's degree
                        // value meaningless. The same conversion is applied by the SVG backend's
                        // `conic_sample`, so the two agree.
                        let angle = dy.atan2(dx) + core::f32::consts::PI;
                        let start = gradient.angle.to_radians();
                        let angle = (angle + start) % core::f32::consts::TAU;
                        let angle =
                            if angle < 0.0 { angle + core::f32::consts::TAU } else { angle };
                        angle / core::f32::consts::TAU
                    }
                };
                let color = gradient.interpolate(pos);
                if pixel_visible(clip, x as i32, y as i32) {
                    // Through the blend-aware write, so a gradient drawn under a `SetBlendMode`
                    // composites the same way an opaque fill does.
                    write_pixel_with_mode(self.blend_mode, frame, size.width, x, y, color, 1.0);
                }
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
