//! Software rendering surface: back buffer, surface, and configuration.
use crate::core::{Color, Font, Point, Rect, Size};
use crate::render::{PaintBackend, RenderCommand, TextMetrics, ShapedText};
use crate::render::pixel_bytes_len;
use std::sync::{Mutex, OnceLock};

/// Double-buffered RGBA pixel storage used by software rendering.
#[derive(Debug, Clone)]
pub struct BackBuffer {
    size: Size,
    dpi_scale: f32,
    front: Vec<u8>,
    pub(crate) back: Vec<u8>,
}
impl BackBuffer {
    /// Creates a new back buffer for size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let bytes = pixel_bytes_len(size);
        Self {
            size,
            dpi_scale: dpi_scale.max(0.1),
            front: vec![0; bytes],
            back: vec![0; bytes],
        }
    }
    /// Resizes front/back buffers to the new size.
    pub fn resize(&mut self, size: Size) {
        self.size = size;
        let bytes = pixel_bytes_len(size);
        self.front.resize(bytes, 0);
        self.back.resize(bytes, 0);
    }
    /// Returns logical buffer size.
    pub fn size(&self) -> Size {
        self.size
    }
    /// Returns current logical DPI scale.
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
    /// Updates logical DPI scale.
    pub fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale.max(0.1);
    }
    /// Returns mutable reference to the back buffer pixels.
    pub fn back_mut(&mut self) -> &mut [u8] {
        &mut self.back
    }
    /// Returns immutable reference to the front buffer pixels.
    pub fn front(&self) -> &[u8] {
        &self.front
    }
    /// Swaps back and front buffers.
    pub fn present(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
    }
}
/// Software raster surface with quality controls and RGBA frame output.
pub struct SoftwareSurface {
    pub(crate) buffer: BackBuffer,
    pub(crate) aa_samples_per_axis: u8,
}
/// Public software render configuration for quality-related knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRenderConfig {
    /// Anti-aliasing sample grid size per axis (clamped to 1..=8).
    pub aa_samples_per_axis: u8,
}
impl Default for SoftwareRenderConfig {
    fn default() -> Self {
        Self {
            aa_samples_per_axis: 4,
        }
    }
}
impl SoftwareRenderConfig {
    /// Build a config with normalized value bounds.
    pub fn normalized(self) -> Self {
        Self {
            aa_samples_per_axis: self.aa_samples_per_axis.clamp(1, 8),
        }
    }
}
fn global_software_render_config() -> &'static Mutex<SoftwareRenderConfig> {
    static CONFIG: OnceLock<Mutex<SoftwareRenderConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(SoftwareRenderConfig::default()))
}
/// Set process-wide default software render configuration.
pub fn set_default_software_render_config(config: SoftwareRenderConfig) {
    *global_software_render_config()
        .lock()
        .expect("software render config lock poisoned") = config.normalized();
}
/// Get process-wide default software render configuration.
pub fn default_software_render_config() -> SoftwareRenderConfig {
    *global_software_render_config()
        .lock()
        .expect("software render config lock poisoned")
}
/// Render context for custom widget drawing.
pub struct RenderContext<'a> {
    backend: &'a mut dyn PaintBackend,
}
impl<'a> RenderContext<'a> {
    pub fn new(backend: &'a mut dyn PaintBackend) -> Self {
        Self { backend }
    }
    pub fn backend(&mut self) -> &mut dyn PaintBackend {
        self.backend
    }
    pub fn size(&self) -> Size {
        self.backend.size()
    }
    pub fn dpi_scale(&self) -> f32 {
        self.backend.dpi_scale()
    }
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.backend
            .execute_command(&RenderCommand::FillRect { rect, color });
    }
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.backend
            .execute_command(&RenderCommand::DrawRect { rect, color });
    }
    pub fn draw_rect_stroke(&mut self, rect: Rect, color: Color, width: u32) {
        self.backend
            .execute_command(&RenderCommand::DrawRectStroke { rect, color, width });
    }
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        self.backend
            .execute_command(&RenderCommand::FillRoundedRect {
                rect,
                radius,
                color,
            });
    }
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        self.backend
            .execute_command(&RenderCommand::FillRoundedRectAA {
                rect,
                radius,
                color,
            });
    }
    pub fn draw_rounded_rect_stroke(&mut self, rect: Rect, radius: u32, color: Color, width: u32) {
        self.backend
            .execute_command(&RenderCommand::DrawRoundedRectStroke {
                rect,
                radius,
                color,
                width,
            });
    }
    pub fn draw_rounded_rect_stroke_aa(
        &mut self,
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    ) {
        self.backend
            .execute_command(&RenderCommand::DrawRoundedRectStrokeAA {
                rect,
                radius,
                color,
                width,
            });
    }
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.backend
            .execute_command(&RenderCommand::DrawLine { from, to, color });
    }
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.backend
            .execute_command(&RenderCommand::DrawLineAA { from, to, color });
    }
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.backend
            .execute_command(&RenderCommand::DrawLineStroke {
                from,
                to,
                color,
                width,
            });
    }
    pub fn draw_line_stroke_aa(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.backend
            .execute_command(&RenderCommand::DrawLineStrokeAA {
                from,
                to,
                color,
                width,
            });
    }
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillCircle {
            center,
            radius,
            color,
        });
    }
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillCircleAA {
            center,
            radius,
            color,
        });
    }
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawCircle {
            center,
            radius,
            color,
        });
    }
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        self.backend
            .execute_command(&RenderCommand::DrawCircleStroke {
                center,
                radius,
                color,
                width,
            });
    }
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &Font, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
        });
    }
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.backend.measure_text(text, font)
    }
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        self.backend.shape_text(text, font)
    }
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        // Clip to the given rectangle by filling the area outside with a clipping rect.
        // Since we have no real clip stack, we use a workaround: draw nothing, but mark it.
        // The software renderer already handles per-pixel bounds.
        let _ = (x, y, width, height);
    }
    pub fn pop_clip(&mut self) {
        // Pop the clip rect. No-op since we don't have a real clip stack.
    }
}
