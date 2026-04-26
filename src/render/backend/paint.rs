//! Paint backend trait and software implementation.
use super::batch::BatchState;
use crate::core::{Color, Font, Size};
use crate::render::{
    RenderCommand, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextMetrics,
};

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
    pub(crate) batch_state: BatchState,
}
impl SoftwarePaintBackend {
    /// Creates a software paint backend with a target size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        Self {
            surface: SoftwareSurface::new(size, dpi_scale),
            batch_state: BatchState::new(),
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
            RenderCommand::DrawImage {
                x,
                y,
                width,
                height,
                data,
            } => self.surface.draw_image(*x, *y, *width, *height, data),
            RenderCommand::PushClip {
                x,
                y,
                width,
                height,
            } => self.surface.push_clip(*x, *y, *width, *height),
            RenderCommand::PopClip => self.surface.pop_clip(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Point, Rect, Size};

    // ── SoftwarePaintBackend construction ───────────────────────────────

    #[test]
    fn software_paint_backend_new_creates_surface() {
        let size = Size::new(100, 100);
        let backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), size);
        assert!((backend.dpi_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_zero_size() {
        let backend = SoftwarePaintBackend::new(Size::new(0, 0), 1.0);
        assert_eq!(backend.size(), Size::new(0, 0));
        let rgba = backend.frame_rgba();
        assert!(rgba.is_empty());
    }

    #[test]
    fn software_paint_backend_high_dpi() {
        let backend = SoftwarePaintBackend::new(Size::new(50, 50), 2.0);
        assert!((backend.dpi_scale() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn software_paint_backend_minimum_dpi_scale() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 0.0);
        assert!((backend.dpi_scale() - 0.1).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend surface access ─────────────────────────────

    #[test]
    fn software_paint_backend_surface_accessor() {
        let backend = SoftwarePaintBackend::new(Size::new(30, 30), 1.0);
        let surface = backend.surface();
        assert_eq!(surface.size(), Size::new(30, 30));
    }

    #[test]
    fn software_paint_backend_surface_mut_accessor() {
        let mut backend = SoftwarePaintBackend::new(Size::new(40, 40), 1.0);
        {
            let surface = backend.surface_mut();
            assert_eq!(surface.size(), Size::new(40, 40));
        }
    }

    // ── PaintBackend trait - frame lifecycle ────────────────────────────

    #[test]
    fn paint_backend_begin_end_frame_clears() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);

        backend.begin_frame(Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255); // R
            assert_eq!(chunk[1], 0); // G
            assert_eq!(chunk[2], 0); // B
            assert_eq!(chunk[3], 255); // A
        }
    }

    #[test]
    fn paint_backend_execute_fill_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(2, 2, 10, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 5 * stride + 5 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn paint_backend_execute_draw_rect_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRectStroke {
            rect: Rect::new(0, 0, 20, 20),
            color: Color::GREEN,
            width: 1,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_line() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawLine {
            from: Point::new(0, 0),
            to: Point::new(9, 9),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_push_pop_clip() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::PushClip {
            x: 0,
            y: 0,
            width: 5,
            height: 5,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::PopClip);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_circle() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillCircleAA {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn paint_backend_size_set_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        assert_eq!(backend.size(), Size::new(10, 10));

        let new_size = Size::new(50, 50);
        backend.set_size(new_size);
        assert_eq!(backend.size(), new_size);
    }

    #[test]
    fn paint_backend_dpi_scale_set_dpi_scale() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.set_dpi_scale(1.5);
        assert!((backend.dpi_scale() - 1.5).abs() < 1e-6);
    }

    // ── SoftwarePaintBackend config ─────────────────────────────────────

    #[test]
    fn paint_backend_render_config_default() {
        let backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = backend.render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn paint_backend_apply_render_config() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig {
            aa_samples_per_axis: 2,
        };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 2);
    }

    #[test]
    fn paint_backend_apply_render_config_clamps_to_normalized_range() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig {
            aa_samples_per_axis: 99,
        };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 8);

        let config = SoftwareRenderConfig {
            aa_samples_per_axis: 0,
        };
        backend.apply_render_config(config);
        assert_eq!(backend.render_config().aa_samples_per_axis, 1);
    }

    #[test]
    fn paint_backend_default_render_config() {
        let config = <SoftwarePaintBackend as PaintBackend>::render_config(
            &SoftwarePaintBackend::new(Size::new(1, 1), 1.0),
        );
        assert_eq!(config, SoftwareRenderConfig::default());
    }

    #[test]
    fn paint_backend_execute_draw_text_does_not_panic() {
        let size = Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let font = Font::simple("Arial", 12.0);
        backend.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 20),
            text: "Hello".to_string(),
            font,
            color: Color::BLACK,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn paint_backend_execute_draw_image() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let data = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ];
        backend.execute_command(&RenderCommand::DrawImage {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            data,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn paint_backend_execute_draw_rect() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn paint_backend_execute_fill_rounded_rect() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRoundedRect {
            rect: Rect::new(2, 2, 16, 16),
            radius: 4,
            color: Color::GREEN,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx + 1], 255); // G
    }

    #[test]
    fn paint_backend_execute_draw_circle_stroke() {
        let size = Size::new(20, 20);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::DrawCircleStroke {
            center: Point::new(10, 10),
            radius: 5,
            color: Color::RED,
            width: 2,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── PaintBackend measure_text / shape_text ──────────────────────────

    #[test]
    fn paint_backend_measure_text_returns_metrics() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let metrics = backend.measure_text("Hello", &font);
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
    }

    #[test]
    fn paint_backend_shape_text_returns_shaped() {
        let backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let font = Font::simple("Arial", 12.0);
        let shaped = backend.shape_text("Hi", &font);
        assert!(!shaped.clusters.is_empty());
    }

    #[test]
    fn paint_backend_frame_rgba_after_clear() {
        let size = Size::new(5, 5);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::rgb(128, 64, 32));
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let expected_len = 5 * 5 * 4;
        assert_eq!(rgba.len(), expected_len);

        assert_eq!(rgba[0], 128);
        assert_eq!(rgba[1], 64);
        assert_eq!(rgba[2], 32);
        assert_eq!(rgba[3], 255);
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn paint_backend_execute_fill_rect_out_of_bounds() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(100, 100, 50, 50),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
            assert_eq!(chunk[1], 255);
            assert_eq!(chunk[2], 255);
        }
    }

    #[test]
    fn paint_backend_fill_rect_zero_size() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 0, 0),
            color: Color::RED,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
        }
    }

    #[test]
    fn paint_backend_multiple_commands_in_frame() {
        let size = Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 5, 10),
            color: Color::RED,
        });
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(5, 0, 5, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let left_idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[left_idx], 255); // R
        assert_eq!(rgba[left_idx + 2], 0); // B

        let right_idx = 5 * stride + 7 * 4;
        assert_eq!(rgba[right_idx], 0); // R
        assert_eq!(rgba[right_idx + 2], 255); // B
    }
}
