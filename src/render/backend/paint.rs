//! Paint backend trait and software implementation.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{RenderCommand, TextMetrics, ShapedText, SoftwareSurface, SoftwareRenderConfig};

/// Pluggable paint backend strategy used by render scene composition.
pub trait PaintBackend {
    fn begin_frame(&mut self, clear: Color);
    fn end_frame(&mut self);
    fn execute_command(&mut self, command: &RenderCommand);
    fn size(&self) -> Size;
    fn set_size(&mut self, size: Size);
    fn dpi_scale(&self) -> f32;
    fn set_dpi_scale(&mut self, dpi_scale: f32);
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics;
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText;
    fn frame_rgba(&self) -> &[u8];
    /// Apply backend-specific render quality configuration.
    fn apply_render_config(&mut self, _config: SoftwareRenderConfig) {}
    /// Read backend-specific render quality configuration.
    fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig::default()
    }
}
/// Software implementation of the paint backend strategy.
pub struct SoftwarePaintBackend {
    pub(crate) surface: SoftwareSurface,
}
impl SoftwarePaintBackend {
    /// Creates a software paint backend with a target size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        Self {
            surface: SoftwareSurface::new(size, dpi_scale),
        }
    }
    /// Returns immutable access to the underlying software surface.
    pub fn surface(&self) -> &SoftwareSurface {
        &self.surface
    }
    /// Returns mutable access to the underlying software surface.
    pub fn surface_mut(&mut self) -> &mut SoftwareSurface {
        &mut self.surface
    }
    /// Apply software render quality configuration via backend facade.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    /// Get software render quality configuration via backend facade.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }
}
impl PaintBackend for SoftwarePaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.surface.begin_frame(clear);
    }
    fn end_frame(&mut self) {
        self.surface.end_frame();
    }
    fn execute_command(&mut self, command: &RenderCommand) {
        match command {
            RenderCommand::FillRect { rect, color } => self.surface.fill_rect(*rect, *color),
            RenderCommand::DrawRect { rect, color } => self.surface.draw_rect(*rect, *color),
            RenderCommand::DrawRectStroke { rect, color, width } => {
                self.surface.draw_rect_with_width(*rect, *color, *width)
            }
            RenderCommand::FillRoundedRect {
                rect,
                radius,
                color,
            } => self.surface.fill_rounded_rect(*rect, *radius, *color),
            RenderCommand::FillRoundedRectAA {
                rect,
                radius,
                color,
            } => self.surface.fill_rounded_rect_aa(*rect, *radius, *color),
            RenderCommand::DrawRoundedRectStroke {
                rect,
                radius,
                color,
                width,
            } => self
                .surface
                .draw_rounded_rect_with_width(*rect, *radius, *color, *width),
            RenderCommand::DrawRoundedRectStrokeAA {
                rect,
                radius,
                color,
                width,
            } => self
                .surface
                .draw_rounded_rect_aa_with_width(*rect, *radius, *color, *width),
            RenderCommand::DrawLine { from, to, color } => {
                self.surface.draw_line(*from, *to, *color)
            }
            RenderCommand::DrawLineAA { from, to, color } => {
                self.surface.draw_line_aa(*from, *to, *color)
            }
            RenderCommand::DrawLineStrokeAA {
                from,
                to,
                color,
                width,
            } => self
                .surface
                .draw_line_aa_with_width(*from, *to, *color, *width),
            RenderCommand::DrawLineStroke {
                from,
                to,
                color,
                width,
            } => self
                .surface
                .draw_line_with_width(*from, *to, *color, *width),
            RenderCommand::FillCircle {
                center,
                radius,
                color,
            } => self.surface.fill_circle(*center, *radius, *color),
            RenderCommand::FillCircleAA {
                center,
                radius,
                color,
            } => self.surface.fill_circle_aa(*center, *radius, *color),
            RenderCommand::DrawCircle {
                center,
                radius,
                color,
            } => self.surface.draw_circle(*center, *radius, *color),
            RenderCommand::DrawCircleStroke {
                center,
                radius,
                color,
                width,
            } => self
                .surface
                .draw_circle_with_width(*center, *radius, *color, *width),
            RenderCommand::DrawText {
                origin,
                text,
                font,
                color,
            } => self.surface.draw_text(*origin, text, font, *color),
        }
    }
    fn size(&self) -> Size {
        self.surface.size()
    }
    fn set_size(&mut self, size: Size) {
        self.surface.resize(size);
    }
    fn dpi_scale(&self) -> f32 {
        self.surface.dpi_scale()
    }
    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.surface.set_dpi_scale(dpi_scale);
    }
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.surface.measure_text(text, font)
    }
    fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        self.surface.shape_text(text, font)
    }
    fn frame_rgba(&self) -> &[u8] {
        self.surface.frame_rgba()
    }
    fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.surface.apply_render_config(config);
    }
    fn render_config(&self) -> SoftwareRenderConfig {
        self.surface.render_config()
    }
}
