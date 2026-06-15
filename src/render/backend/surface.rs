//! Software rendering surface: back buffer, surface, and configuration.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::pixel_bytes_len;
use crate::render::{PaintBackend, RenderCommand, ShapedText, TextMetrics};
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
        Self { size, dpi_scale: dpi_scale.max(0.1), front: vec![0; bytes], back: vec![0; bytes] }
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
    /// Active clip rectangle stack. An empty stack means no clipping.
    pub(crate) clip_stack: Vec<(i32, i32, u32, u32)>,
}
/// Public software render configuration for quality-related knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRenderConfig {
    /// Anti-aliasing sample grid size per axis (clamped to 1..=8).
    pub aa_samples_per_axis: u8,
}
impl Default for SoftwareRenderConfig {
    fn default() -> Self {
        Self { aa_samples_per_axis: 4 }
    }
}
impl SoftwareRenderConfig {
    /// Build a config with normalized value bounds.
    pub fn normalized(self) -> Self {
        Self { aa_samples_per_axis: self.aa_samples_per_axis.clamp(1, 8) }
    }
}
fn global_software_render_config() -> &'static Mutex<SoftwareRenderConfig> {
    static CONFIG: OnceLock<Mutex<SoftwareRenderConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(SoftwareRenderConfig::default()))
}

#[cfg(test)]
pub(crate) fn software_render_config_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
/// Set process-wide default software render configuration.
pub fn set_default_software_render_config(config: SoftwareRenderConfig) {
    *global_software_render_config().lock().expect("software render config lock poisoned") =
        config.normalized();
}
/// Get process-wide default software render configuration.
pub fn default_software_render_config() -> SoftwareRenderConfig {
    *global_software_render_config().lock().expect("software render config lock poisoned")
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
        self.backend.execute_command(&RenderCommand::FillRect { rect, color });
    }
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawRect { rect, color });
    }
    pub fn draw_rect_stroke(&mut self, rect: Rect, color: Color, width: u32) {
        self.backend.execute_command(&RenderCommand::DrawRectStroke { rect, color, width });
    }
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillRoundedRect { rect, radius, color });
    }
    pub fn fill_rounded_rect_aa(&mut self, rect: Rect, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillRoundedRectAA { rect, radius, color });
    }
    pub fn draw_rounded_rect_stroke(&mut self, rect: Rect, radius: u32, color: Color, width: u32) {
        self.backend.execute_command(&RenderCommand::DrawRoundedRectStroke {
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
        self.backend.execute_command(&RenderCommand::DrawRoundedRectStrokeAA {
            rect,
            radius,
            color,
            width,
        });
    }
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawLine { from, to, color });
    }
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawLineAA { from, to, color });
    }
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.backend.execute_command(&RenderCommand::DrawLineStroke { from, to, color, width });
    }
    pub fn draw_line_stroke_aa(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.backend.execute_command(&RenderCommand::DrawLineStrokeAA { from, to, color, width });
    }
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillCircle { center, radius, color });
    }
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::FillCircleAA { center, radius, color });
    }
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.backend.execute_command(&RenderCommand::DrawCircle { center, radius, color });
    }
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        self.backend.execute_command(&RenderCommand::DrawCircleStroke {
            center,
            radius,
            color,
            width,
        });
    }
    pub fn draw_text(
        &mut self,
        origin: Point,
        text: &str,
        font: &Font,
        color: Color,
        alignment: HorizontalAlignment,
    ) {
        self.backend.execute_command(&RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
            alignment,
        });
    }
    pub fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.backend.measure_text(text, font)
    }
    pub fn shape_text(&self, text: &str, font: &Font) -> ShapedText {
        self.backend.shape_text(text, font)
    }
    pub fn push_clip(&mut self, x: i32, y: i32, width: u32, height: u32) {
        self.backend.execute_command(&RenderCommand::PushClip { x, y, width, height });
    }
    pub fn pop_clip(&mut self) {
        self.backend.execute_command(&RenderCommand::PopClip);
    }

    pub fn draw_image(&mut self, x: i32, y: i32, width: u32, height: u32, data: &[u8]) {
        self.backend.execute_command(&RenderCommand::DrawImage {
            x,
            y,
            width,
            height,
            data: data.to_vec(),
        });
    }

    /// Execute an arbitrary render command directly.
    ///
    /// Useful for widgets like `Canvas` that store a command buffer
    /// and replay it during the draw pass.
    pub fn execute_command(&mut self, command: RenderCommand) {
        self.backend.execute_command(&command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Font, Point, Rect, Size};
    use crate::render::RenderCommand;
    use crate::render::SoftwarePaintBackend;

    struct SoftwareRenderConfigTestGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        original: SoftwareRenderConfig,
    }

    impl SoftwareRenderConfigTestGuard {
        fn new() -> Self {
            let lock = software_render_config_test_lock()
                .lock()
                .expect("software render config test lock poisoned");
            let original = default_software_render_config();
            Self { _lock: lock, original }
        }
    }

    impl Drop for SoftwareRenderConfigTestGuard {
        fn drop(&mut self) {
            set_default_software_render_config(self.original);
        }
    }

    // ── BackBuffer ──────────────────────────────────────────────────────

    #[test]
    fn back_buffer_new_creates_buffers() {
        let bb = BackBuffer::new(Size::new(10, 10), 1.0);
        assert_eq!(bb.size(), Size::new(10, 10));
        assert!((bb.dpi_scale() - 1.0).abs() < 1e-6);
        assert_eq!(bb.front().len(), 10 * 10 * 4);
        assert_eq!(bb.back.len(), 10 * 10 * 4);
    }

    #[test]
    fn back_buffer_zero_size() {
        let bb = BackBuffer::new(Size::new(0, 0), 1.0);
        assert_eq!(bb.size(), Size::new(0, 0));
        assert!(bb.front().is_empty());
        assert!(bb.back.is_empty());
    }

    #[test]
    fn back_buffer_resize() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.resize(Size::new(20, 20));
        assert_eq!(bb.size(), Size::new(20, 20));
        assert_eq!(bb.front().len(), 20 * 20 * 4);
        assert_eq!(bb.back.len(), 20 * 20 * 4);
    }

    #[test]
    fn back_buffer_resize_to_zero() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.resize(Size::new(0, 0));
        assert_eq!(bb.size(), Size::new(0, 0));
        assert!(bb.front().is_empty());
        assert!(bb.back.is_empty());
    }

    #[test]
    fn back_buffer_set_dpi_scale() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.set_dpi_scale(2.5);
        assert!((bb.dpi_scale() - 2.5).abs() < 1e-6);
    }

    #[test]
    fn back_buffer_set_dpi_scale_clamps_minimum() {
        let mut bb = BackBuffer::new(Size::new(10, 10), 1.0);
        bb.set_dpi_scale(0.0);
        assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn back_buffer_back_mut_returns_writable_slice() {
        let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
        let back = bb.back_mut();
        assert_eq!(back.len(), 5 * 5 * 4);
        back[0] = 255;
        assert_eq!(bb.back[0], 255);
    }

    #[test]
    fn back_buffer_present_swaps_front_and_back() {
        let mut bb = BackBuffer::new(Size::new(5, 5), 1.0);
        // Write to back
        bb.back_mut()[0] = 42;
        assert_ne!(bb.front()[0], 42);

        bb.present();

        // After present, back is now front
        assert_eq!(bb.front()[0], 42);
        // The old front (zeros) is now back
        assert_eq!(bb.back[0], 0);
    }

    #[test]
    fn back_buffer_present_swaps_again() {
        let mut bb = BackBuffer::new(Size::new(2, 2), 1.0);
        bb.back_mut()[0] = 10;
        bb.present();
        bb.back_mut()[0] = 20;
        bb.present();

        // Second present should show 20 on front
        assert_eq!(bb.front()[0], 20);
    }

    #[test]
    fn back_buffer_dpi_scale_initial_clamp() {
        let bb = BackBuffer::new(Size::new(1, 1), 0.0);
        assert!((bb.dpi_scale() - 0.1).abs() < 1e-6);
    }

    // ── SoftwareSurface ─────────────────────────────────────────────────

    #[test]
    fn software_surface_new_creates_surface() {
        let surface = SoftwareSurface::new(Size::new(100, 100), 1.0);
        assert_eq!(surface.size(), Size::new(100, 100));
        assert!((surface.dpi_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn software_surface_zero_size() {
        let surface = SoftwareSurface::new(Size::new(0, 0), 1.0);
        assert_eq!(surface.size(), Size::new(0, 0));
    }

    #[test]
    fn software_surface_begin_frame_clears() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::RED);
        surface.end_frame();

        let rgba = surface.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255);
            assert_eq!(chunk[1], 0);
            assert_eq!(chunk[2], 0);
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn software_surface_fill_rect() {
        let mut surface = SoftwareSurface::new(Size::new(20, 20), 1.0);
        surface.begin_frame(Color::WHITE);

        surface.fill_rect(Rect::new(2, 2, 10, 10), Color::BLUE);

        surface.end_frame();

        let rgba = surface.frame_rgba();
        let stride = 20 * 4;
        let idx = 5 * stride + 5 * 4;
        assert_eq!(rgba[idx], 0); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn software_surface_fill_rect_zero_size() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::WHITE);
        surface.fill_rect(Rect::new(0, 0, 0, 0), Color::RED);
        surface.end_frame();

        let rgba = surface.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 255); // Still white, no fill
        }
    }

    #[test]
    fn software_surface_draw_rect() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.begin_frame(Color::WHITE);

        surface.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);

        surface.end_frame();

        let rgba = surface.frame_rgba();
        // Top-left corner should be green (stroke edge)
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0); // B
    }

    #[test]
    fn software_surface_resize() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.resize(Size::new(50, 50));
        assert_eq!(surface.size(), Size::new(50, 50));
    }

    #[test]
    fn software_surface_set_dpi_scale() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.set_dpi_scale(1.5);
        assert!((surface.dpi_scale() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn software_surface_aa_samples_per_axis_default() {
        let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        assert_eq!(surface.aa_samples_per_axis(), 4);
    }

    #[test]
    fn software_surface_set_aa_samples() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.set_aa_samples_per_axis(2);
        assert_eq!(surface.aa_samples_per_axis(), 2);
    }

    #[test]
    fn software_surface_apply_render_config_clamps() {
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        surface.apply_render_config(SoftwareRenderConfig { aa_samples_per_axis: 99 });
        assert_eq!(surface.aa_samples_per_axis(), 8);
    }

    #[test]
    fn software_surface_render_config_roundtrip() {
        let surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        let config = surface.render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    // ── RenderContext ───────────────────────────────────────────────────

    #[test]
    fn render_context_new_wraps_backend() {
        let mut backend = SoftwarePaintBackend::new(Size::new(50, 50), 1.0);
        let ctx = RenderContext::new(&mut backend);
        assert_eq!(ctx.size(), Size::new(50, 50));
    }

    #[test]
    fn render_context_backend_accessor() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        let be = ctx.backend();
        assert_eq!(be.size(), Size::new(10, 10));
    }

    #[test]
    fn render_context_fill_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        // Frame is still zero-initialized (not presented yet)
        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0);

        // After begin/end frame via the backend directly
        let mut backend2 = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend2.begin_frame(Color::WHITE);
        let mut ctx2 = RenderContext::new(&mut backend2);
        ctx2.fill_rect(Rect::new(0, 0, 5, 5), Color::BLUE);
        backend2.end_frame();

        let rgba = backend2.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[2], 255); // B
    }

    #[test]
    fn render_context_draw_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rect(Rect::new(0, 0, 10, 10), Color::GREEN);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R
        assert_eq!(rgba[1], 255); // G
    }

    #[test]
    fn render_context_draw_rect_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rect_stroke(Rect::new(0, 0, 10, 10), Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R on edge
    }

    #[test]
    fn render_context_fill_rounded_rect() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rounded_rect(Rect::new(1, 1, 8, 8), 2, Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_fill_rounded_rect_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_rounded_rect_aa(Rect::new(1, 1, 8, 8), 2, Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_rounded_rect_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rounded_rect_stroke(Rect::new(1, 1, 8, 8), 2, Color::GREEN, 1);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_rounded_rect_stroke_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_rounded_rect_stroke_aa(Rect::new(1, 1, 8, 8), 2, Color::BLUE, 1);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_line() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line(Point::new(0, 0), Point::new(9, 9), Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255);
    }

    #[test]
    fn render_context_draw_line_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_aa(Point::new(0, 0), Point::new(9, 9), Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_line_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_stroke(Point::new(0, 0), Point::new(9, 9), Color::GREEN, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[1], 255);
    }

    #[test]
    fn render_context_draw_line_stroke_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_line_stroke_aa(Point::new(0, 0), Point::new(9, 9), Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_fill_circle() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_circle(Point::new(10, 10), 5, Color::BLUE);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 20 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx + 2], 255); // B
    }

    #[test]
    fn render_context_fill_circle_aa() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.fill_circle_aa(Point::new(10, 10), 5, Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255);
    }

    #[test]
    fn render_context_draw_circle() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_circle(Point::new(10, 10), 5, Color::GREEN);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_circle_stroke() {
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.draw_circle_stroke(Point::new(10, 10), 5, Color::RED, 2);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_draw_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        ctx.draw_text(Point::new(10, 20), "Hello", &font, Color::BLACK, HorizontalAlignment::Left);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }

    #[test]
    fn render_context_measure_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        let metrics = ctx.measure_text("Hello", &font);
        assert!(metrics.width > 0);
        assert!(metrics.height > 0);
    }

    #[test]
    fn render_context_shape_text() {
        let mut backend = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        let ctx = RenderContext::new(&mut backend);
        let font = Font::simple("Arial", 12.0);
        let shaped = ctx.shape_text("Hi", &font);
        assert!(!shaped.clusters.is_empty());
    }

    #[test]
    fn render_context_push_clip() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_clip(0, 0, 5, 5);
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        ctx.pop_clip();
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 2 * stride + 2 * 4;
        assert_eq!(rgba[idx], 255); // R inside clip region
    }

    #[test]
    fn render_context_pop_clip() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        ctx.push_clip(0, 0, 5, 5);
        ctx.pop_clip();
        // After pop, subsequent commands are not clipped
        ctx.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        let stride = 10 * 4;
        let idx = 7 * stride + 7 * 4; // Outside original clip
        assert_eq!(rgba[idx], 255); // R should be visible
    }

    #[test]
    fn render_context_draw_image() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        let data = vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255];
        ctx.draw_image(0, 0, 2, 2, &data);
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 255); // R from top-left pixel
    }

    #[test]
    fn render_context_size_and_dpi_scale() {
        let mut backend = SoftwarePaintBackend::new(Size::new(80, 60), 1.5);
        let ctx = RenderContext::new(&mut backend);
        assert_eq!(ctx.size(), Size::new(80, 60));
        assert!((ctx.dpi_scale() - 1.5).abs() < 1e-6);
    }

    // ── SoftwareRenderConfig ─────────────────────────────────────────────

    #[test]
    fn software_render_config_default_values() {
        let config = SoftwareRenderConfig::default();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn software_render_config_normalized_clamps_low() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 0 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 1);
    }

    #[test]
    fn software_render_config_normalized_clamps_high() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 100 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 8);
    }

    #[test]
    fn software_render_config_normalized_preserves_valid() {
        let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        let norm = config.normalized();
        assert_eq!(norm.aa_samples_per_axis, 2);
    }

    #[test]
    fn software_render_config_normalized_edge_cases() {
        let config_low = SoftwareRenderConfig { aa_samples_per_axis: 1 };
        assert_eq!(config_low.normalized().aa_samples_per_axis, 1);

        let config_high = SoftwareRenderConfig { aa_samples_per_axis: 8 };
        assert_eq!(config_high.normalized().aa_samples_per_axis, 8);
    }

    #[test]
    fn software_render_config_equality() {
        let a = SoftwareRenderConfig { aa_samples_per_axis: 4 };
        let b = SoftwareRenderConfig { aa_samples_per_axis: 4 };
        let c = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── Global config functions ──────────────────────────────────────────

    #[test]
    fn default_software_render_config_returns_default() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        set_default_software_render_config(SoftwareRenderConfig::default());
        let config = default_software_render_config();
        assert_eq!(config.aa_samples_per_axis, 4);
    }

    #[test]
    fn set_default_software_render_config_updates_global() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        let custom = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        set_default_software_render_config(custom);
        let retrieved = default_software_render_config();
        assert_eq!(retrieved.aa_samples_per_axis, 2);
    }

    #[test]
    fn set_default_software_render_config_clamps() {
        let _guard = SoftwareRenderConfigTestGuard::new();
        let custom = SoftwareRenderConfig { aa_samples_per_axis: 99 };
        set_default_software_render_config(custom);
        let retrieved = default_software_render_config();
        assert_eq!(retrieved.aa_samples_per_axis, 8);
    }

    // ── PaintBackend integration via RenderContext ───────────────────────

    #[test]
    fn render_context_dpi_scale_propagates() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 2.0);
        let ctx = RenderContext::new(&mut backend);
        assert!((ctx.dpi_scale() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn render_context_backend_mutability() {
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        {
            let mut ctx = RenderContext::new(&mut backend);
            ctx.fill_rect(Rect::new(0, 0, 5, 5), Color::RED);
        }
        // After ctx drops, backend can still be used
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::BLUE,
        });
        backend.end_frame();

        let rgba = backend.frame_rgba();
        assert!(!rgba.is_empty());
    }
}
