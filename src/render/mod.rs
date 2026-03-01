//! Rendering primitives and software surface baseline.

use crate::core::{Color, Font, Point, Rect, Size};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use std::sync::{Mutex, OnceLock};

/// Text measurement result for width, height, and baseline metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextMetrics {
    /// Measured text width in logical pixels.
    pub width: u32,
    /// Measured text height in logical pixels.
    pub height: u32,
    /// Baseline ascent in logical pixels.
    pub ascent: u32,
    /// Baseline descent in logical pixels.
    pub descent: u32,
}

/// One shaped text cluster produced by the render text shaper.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    /// Cluster source text (one or more unicode scalars).
    pub text: String,
    /// Logical horizontal advance in pixels.
    pub advance: f32,
}

/// Shaped text run composed from ordered clusters.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedText {
    clusters: Vec<TextCluster>,
    advance: f32,
}

impl ShapedText {
    /// Returns ordered text clusters in this shaped run.
    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }

    /// Returns cluster count in this shaped run.
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Returns total horizontal advance in logical pixels.
    pub fn advance(&self) -> f32 {
        self.advance
    }
}

/// Double-buffered RGBA pixel storage used by software rendering.
#[derive(Debug, Clone)]
pub struct BackBuffer {
    size: Size,
    dpi_scale: f32,
    front: Vec<u8>,
    back: Vec<u8>,
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
    buffer: BackBuffer,
    aa_samples_per_axis: u8,
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
    surface: SoftwareSurface,
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
            RenderCommand::DrawRectStroke {
                rect,
                color,
                width,
            } => self.surface.draw_rect_with_width(*rect, *color, *width),
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
            RenderCommand::DrawLine { from, to, color } => self.surface.draw_line(*from, *to, *color),
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
            } => self.surface.draw_line_with_width(*from, *to, *color, *width),
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

/// Draw command recorded by a render layer.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    FillRect { rect: Rect, color: Color },
    DrawRect { rect: Rect, color: Color },
    DrawRectStroke { rect: Rect, color: Color, width: u32 },
    FillRoundedRect {
        rect: Rect,
        radius: u32,
        color: Color,
    },
    FillRoundedRectAA {
        rect: Rect,
        radius: u32,
        color: Color,
    },
    DrawRoundedRectStroke {
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawRoundedRectStrokeAA {
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawLine { from: Point, to: Point, color: Color },
    DrawLineAA { from: Point, to: Point, color: Color },
    DrawLineStrokeAA {
        from: Point,
        to: Point,
        color: Color,
        width: u32,
    },
    DrawLineStroke {
        from: Point,
        to: Point,
        color: Color,
        width: u32,
    },
    FillCircle { center: Point, radius: u32, color: Color },
    FillCircleAA { center: Point, radius: u32, color: Color },
    DrawCircle { center: Point, radius: u32, color: Color },
    DrawCircleStroke {
        center: Point,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawText {
        origin: Point,
        text: String,
        font: Font,
        color: Color,
    },
}

/// One scene layer that stores ordered draw commands.
#[derive(Debug, Clone)]
pub struct SceneLayer {
    z_index: i32,
    commands: Vec<RenderCommand>,
}

impl SceneLayer {
    /// Creates an empty layer with the provided z-index.
    pub fn new(z_index: i32) -> Self {
        Self {
            z_index,
            commands: Vec::new(),
        }
    }

    /// Returns layer z-index.
    pub fn z_index(&self) -> i32 {
        self.z_index
    }

    /// Appends a draw command to this layer.
    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }

    /// Returns recorded commands in insertion order.
    pub fn commands(&self) -> &[RenderCommand] {
        &self.commands
    }
}

/// Lightweight scene model composed from layered command lists.
#[derive(Debug, Clone, Default)]
pub struct RenderScene {
    layers: Vec<SceneLayer>,
}

impl RenderScene {
    /// Creates an empty render scene.
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Removes all layers and commands from the scene.
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    /// Adds one scene layer.
    pub fn add_layer(&mut self, layer: SceneLayer) {
        self.layers.push(layer);
    }

    /// Returns all scene layers.
    pub fn layers(&self) -> &[SceneLayer] {
        &self.layers
    }

    /// Compose scene layers into an arbitrary paint backend.
    pub fn compose_with_backend<B: PaintBackend>(&self, backend: &mut B, clear: Color) {
        self.compose_with_backend_config(backend, clear, None);
    }

    /// Compose scene layers with an optional temporary render config.
    ///
    /// The backend configuration is restored after composition so callers can
    /// apply one-off quality overrides without mutating global backend state.
    pub fn compose_with_backend_config<B: PaintBackend>(
        &self,
        backend: &mut B,
        clear: Color,
        config: Option<SoftwareRenderConfig>,
    ) {
        let previous_config = config.map(|_| backend.render_config());
        if let Some(next) = config {
            backend.apply_render_config(next);
        }

        backend.begin_frame(clear);
        let mut order = self.layers.iter().collect::<Vec<_>>();
        order.sort_by_key(|layer| layer.z_index());
        for layer in order {
            for command in layer.commands() {
                backend.execute_command(command);
            }
        }
        backend.end_frame();

        if let Some(previous) = previous_config {
            backend.apply_render_config(previous);
        }
    }

    /// Compose scene layers into target surface back buffer.
    pub fn compose_to(&self, surface: &mut SoftwareSurface, clear: Color) {
        self.compose_to_config(surface, clear, None);
    }

    /// Compose scene layers into target surface using optional temporary config.
    pub fn compose_to_config(
        &self,
        surface: &mut SoftwareSurface,
        clear: Color,
        config: Option<SoftwareRenderConfig>,
    ) {
        let mut backend = SoftwarePaintBackend::new(surface.size(), surface.dpi_scale());
        backend.set_size(surface.size());
        backend.apply_render_config(surface.render_config());
        self.compose_with_backend_config(&mut backend, clear, config);
        surface.buffer = backend.surface.buffer;
    }
}

impl SoftwareSurface {
    /// Creates a software surface with size and DPI scale.
    pub fn new(size: Size, dpi_scale: f32) -> Self {
        let config = default_software_render_config();
        Self {
            buffer: BackBuffer::new(size, dpi_scale),
            aa_samples_per_axis: config.aa_samples_per_axis,
        }
    }

    /// Get current software render configuration.
    pub fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig {
            aa_samples_per_axis: self.aa_samples_per_axis,
        }
    }

    /// Apply software render configuration.
    pub fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        let normalized = config.normalized();
        self.aa_samples_per_axis = normalized.aa_samples_per_axis;
    }

    /// Set anti-aliasing sample grid size per axis for high-sample raster paths.
    pub fn set_aa_samples_per_axis(&mut self, samples: u8) {
        self.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: samples,
        });
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
        TextMetrics {
            width,
            height: line_height.round() as u32,
            ascent,
            descent,
        }
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
                clusters.push(TextCluster {
                    text: scalar.to_string(),
                    advance: 0.0,
                });
            }
        }

        let mut total_advance = 0.0f32;
        for cluster in &mut clusters {
            cluster.advance = estimate_cluster_advance(&cluster.text, font.size, scale);
            total_advance += cluster.advance;
        }

        ShapedText {
            clusters,
            advance: total_advance,
        }
    }

    /// Fills a rectangle with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        let size = self.buffer.size();
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width as i32).max(0) as u32;
        let y1 = (rect.y + rect.height as i32).max(0) as u32;
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
        let x1 = rect.x + rect.width as i32 - 1;
        let y1 = rect.y + rect.height as i32 - 1;

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
        let x1 = (rect.x + rect.width as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as i32 - 1).min(height - 1);

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
        let x1 = (rect.x + rect.width as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as i32 - 1).min(height - 1);

        let effective_radius = rounded_rect_effective_radius(rect, radius);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let coverage = rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
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
        let x1 = (rect.x + rect.width as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as i32 - 1).min(height - 1);

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
        let x1 = (rect.x + rect.width as i32 - 1).min(width - 1);
        let y1 = (rect.y + rect.height as i32 - 1).min(height - 1);

        let effective_radius = rounded_rect_effective_radius(rect, radius);
        let inner = inset_rect(rect, stroke_width as i32);
        let has_inner = inner.width > 0 && inner.height > 0;
        let inner_radius = effective_radius.saturating_sub(stroke_width);

        for py in y0..=y1 {
            for px in x0..=x1 {
                let outer_coverage = rounded_rect_coverage_grid(px, py, rect, effective_radius, sample_grid);
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
    pub fn draw_line_with_width(&mut self, from: Point, to: Point, color: Color, stroke_width: u32) {
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
    pub fn draw_line_aa_with_width(&mut self, from: Point, to: Point, color: Color, stroke_width: u32) {
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
    pub fn draw_circle_with_width(&mut self, center: Point, radius: u32, color: Color, stroke_width: u32) {
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
        let ring_half_width = stroke_width as f32 / 2.0;
        let x0 = (center.x - radius as i32 - 1).max(0);
        let y0 = (center.y - radius as i32 - 1).max(0);
        let x1 = (center.x + radius as i32 + 1).min(width - 1);
        let y1 = (center.y + radius as i32 + 1).min(height - 1);

        for py in y0..=y1 {
            for px in x0..=x1 {
                let stroke_coverage =
                    circle_stroke_coverage_grid(px, py, center, ring_radius, ring_half_width, sample_grid);
                if stroke_coverage > 0.0 {
                    blend_pixel(frame, size.width, px as u32, py as u32, color, stroke_coverage);
                }
            }
        }
    }

    /// Draws text using the current text raster fallback path.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &Font, color: Color) {
        let metrics = self.measure_text(text, font);
        if metrics.width == 0 || metrics.height == 0 {
            return;
        }

        let shaped = self.shape_text(text, font);
        let mut pen_x = origin.x as f32;
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
                draw_bitmap_glyph(
                    frame,
                    size.width,
                    size.height,
                    ch,
                    pen_x.round() as i32,
                    origin.y,
                    glyph_width,
                    glyph_height,
                    color,
                );
            }

            pen_x += cluster.advance;
        }
    }
}

fn draw_bitmap_glyph(
    frame: &mut [u8],
    surface_width: u32,
    surface_height: u32,
    ch: char,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: Color,
) {
    if ch.is_whitespace() || width <= 0 || height <= 0 {
        return;
    }

    let glyph = glyph_bitmap(ch);
    for gy in 0..8i32 {
        let row = glyph[gy as usize];
        for gx in 0..8i32 {
            if row & (1u8 << gx) == 0 {
                continue;
            }

            let x0 = x + (gx * width) / 8;
            let mut x1 = x + ((gx + 1) * width) / 8;
            let y0 = y + (gy * height) / 8;
            let mut y1 = y + ((gy + 1) * height) / 8;
            if x1 <= x0 {
                x1 = x0 + 1;
            }
            if y1 <= y0 {
                y1 = y0 + 1;
            }

            for py in y0.max(0)..y1.min(surface_height as i32) {
                for px in x0.max(0)..x1.min(surface_width as i32) {
                    blend_pixel(frame, surface_width, px as u32, py as u32, color, 1.0);
                }
            }
        }
    }
}

fn glyph_bitmap(ch: char) -> [u8; 8] {
    if let Some(bitmap) = BASIC_FONTS.get(ch) {
        return bitmap;
    }
    if let Some(bitmap) = BASIC_FONTS.get(ch.to_ascii_uppercase()) {
        return bitmap;
    }
    if let Some(bitmap) = BASIC_FONTS.get(ch.to_ascii_lowercase()) {
        return bitmap;
    }
    [
        0b11111111,
        0b10000001,
        0b10111101,
        0b10100101,
        0b10111101,
        0b10000001,
        0b11111111,
        0b00000000,
    ]
}

fn pixel_bytes_len(size: Size) -> usize {
    size.width
        .saturating_mul(size.height)
        .saturating_mul(4) as usize
}

fn fill_pixels(pixels: &mut [u8], color: Color) {
    for px in pixels.chunks_exact_mut(4) {
        px[0] = color.r;
        px[1] = color.g;
        px[2] = color.b;
        px[3] = color.a;
    }
}

fn set_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    frame[idx] = color.r;
    frame[idx + 1] = color.g;
    frame[idx + 2] = color.b;
    frame[idx + 3] = color.a;
}

fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }

    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }

    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a <= 0.0 {
        return;
    }

    let dst_r = frame[idx] as f32 / 255.0;
    let dst_g = frame[idx + 1] as f32 / 255.0;
    let dst_b = frame[idx + 2] as f32 / 255.0;
    let dst_a = frame[idx + 3] as f32 / 255.0;

    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        frame[idx] = 0;
        frame[idx + 1] = 0;
        frame[idx + 2] = 0;
        frame[idx + 3] = 0;
        return;
    }

    let src_r = color.r as f32 / 255.0;
    let src_g = color.g as f32 / 255.0;
    let src_b = color.b as f32 / 255.0;

    let out_r = (src_r * src_a + dst_r * dst_a * (1.0 - src_a)) / out_a;
    let out_g = (src_g * src_a + dst_g * dst_a * (1.0 - src_a)) / out_a;
    let out_b = (src_b * src_a + dst_b * dst_a * (1.0 - src_a)) / out_a;

    frame[idx] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
    frame[idx + 1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
    frame[idx + 2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
    frame[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn circle_fill_coverage(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }

    (radius + 1.0 - distance).clamp(0.0, 1.0)
}

fn circle_fill_coverage_grid(px: i32, py: i32, center: Point, radius: f32, grid: u8) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;

    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            coverage_sum += circle_fill_coverage(distance, radius);
        }
    }

    (coverage_sum / total as f32).clamp(0.0, 1.0)
}

fn circle_stroke_coverage_grid(
    px: i32,
    py: i32,
    center: Point,
    radius: f32,
    half_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;

    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            coverage_sum += (half_width + 0.5 - (distance - radius).abs()).clamp(0.0, 1.0);
        }
    }

    (coverage_sum / total as f32).clamp(0.0, 1.0)
}

fn point_to_segment_distance(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len2 = abx * abx + aby * aby;

    if ab_len2 <= f32::EPSILON {
        let dx = px - ax;
        let dy = py - ay;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = ((apx * abx + apy * aby) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    let dx = px - cx;
    let dy = py - cy;
    (dx * dx + dy * dy).sqrt()
}

fn line_stroke_coverage_grid(
    px: i32,
    py: i32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    half_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;

    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let distance = point_to_segment_distance(sample_x, sample_y, ax, ay, bx, by);
            coverage_sum += (half_width + 0.5 - distance).clamp(0.0, 1.0);
        }
    }

    (coverage_sum / total as f32).clamp(0.0, 1.0)
}

fn cluster_ends_with_zwj(cluster: &TextCluster) -> bool {
    cluster.text.chars().last().map(|ch| ch == '\u{200D}').unwrap_or(false)
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

fn is_variation_selector(ch: char) -> bool {
    matches!(ch as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

fn is_wide_scalar(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x115F
            | 0x2329..=0x232A
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
    )
}

fn estimate_cluster_advance(cluster: &str, font_size: f32, scale: f32) -> f32 {
    if cluster.trim().is_empty() {
        return (font_size * 0.33 * scale).max(1.0);
    }

    let has_wide = cluster.chars().any(is_wide_scalar);
    let factor = if has_wide { 1.0 } else { 0.6 };
    (font_size * factor * scale).max(1.0)
}

fn rounded_rect_effective_radius(rect: Rect, radius: u32) -> u32 {
    radius.min(rect.width / 2).min(rect.height / 2)
}

fn inset_rect(rect: Rect, inset: i32) -> Rect {
    let x = rect.x + inset;
    let y = rect.y + inset;
    let width = (rect.width as i32 - inset * 2).max(0) as u32;
    let height = (rect.height as i32 - inset * 2).max(0) as u32;
    Rect { x, y, width, height }
}

fn point_in_rounded_rect_f32(px: f32, py: f32, rect: Rect, radius: u32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }

    let left = rect.x as f32;
    let top = rect.y as f32;
    let right = rect.x as f32 + rect.width as f32;
    let bottom = rect.y as f32 + rect.height as f32;
    if px < left || px >= right || py < top || py >= bottom {
        return false;
    }

    let r = rounded_rect_effective_radius(rect, radius) as f32;
    if r <= 0.0 {
        return true;
    }

    if (px >= left + r && px < right - r) || (py >= top + r && py < bottom - r) {
        return true;
    }

    let cx = if px < left + r {
        left + r
    } else if px >= right - r {
        right - r
    } else {
        px
    };
    let cy = if py < top + r {
        top + r
    } else if py >= bottom - r {
        bottom - r
    } else {
        py
    };

    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}

fn rounded_rect_coverage(px: i32, py: i32, rect: Rect, radius: u32) -> f32 {
    rounded_rect_coverage_grid(px, py, rect, radius, 2)
}

fn rounded_rect_coverage_grid(px: i32, py: i32, rect: Rect, radius: u32, grid: u8) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let mut covered = 0u32;
    let total = sample_count * sample_count;

    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = (sy as f32 + 0.5) / sample_count as f32;
            if point_in_rounded_rect_f32(px as f32 + sample_x, py as f32 + sample_y, rect, radius) {
                covered += 1;
            }
        }
    }

    covered as f32 / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font {
            family: "Sans".to_string(),
            size: 14.0,
            bold: false,
            italic: false,
        }
    }

    #[test]
    fn text_metrics_scale_with_dpi() {
        let mut surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
        let m1 = surface.measure_text("hello", &font());
        surface.set_dpi_scale(2.0);
        let m2 = surface.measure_text("hello", &font());
        assert!(m2.width > m1.width);
        assert!(m2.height > m1.height);
    }

    #[test]
    fn double_buffer_present_swaps_frame() {
        let mut surface = SoftwareSurface::new(Size { width: 4, height: 4 }, 1.0);
        surface.begin_frame(Color::rgba(255, 0, 0, 255));
        surface.end_frame();
        assert_eq!(&surface.frame_rgba()[0..4], &[255, 0, 0, 255]);

        surface.begin_frame(Color::rgba(0, 0, 255, 255));
        surface.end_frame();
        assert_eq!(&surface.frame_rgba()[0..4], &[0, 0, 255, 255]);
    }

    #[test]
    fn fill_rect_writes_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 255));
        surface.fill_rect(
            Rect {
                x: 2,
                y: 2,
                width: 3,
                height: 3,
            },
            Color::rgba(1, 2, 3, 255),
        );
        surface.end_frame();
        let idx = ((3 * 8 + 3) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[1, 2, 3, 255]);
    }

    #[test]
    fn shaping_merges_combining_marks_into_one_cluster() {
        let surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
        let shaped = surface.shape_text("e\u{0301}", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }

    #[test]
    fn shaping_merges_zwj_sequence_into_one_cluster() {
        let surface = SoftwareSurface::new(Size { width: 100, height: 40 }, 1.0);
        let shaped = surface.shape_text("👨\u{200D}👩\u{200D}👧\u{200D}👦", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }

    #[test]
    fn scene_composition_respects_layer_order() {
        let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);

        let mut back = SceneLayer::new(0);
        back.push(RenderCommand::FillRect {
            rect: Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
            color: Color::rgba(10, 20, 30, 255),
        });

        let mut front = SceneLayer::new(10);
        front.push(RenderCommand::FillRect {
            rect: Rect {
                x: 2,
                y: 2,
                width: 2,
                height: 2,
            },
            color: Color::rgba(200, 1, 2, 255),
        });

        let mut scene = RenderScene::new();
        scene.add_layer(front);
        scene.add_layer(back);
        scene.compose_to(&mut surface, Color::rgba(0, 0, 0, 255));

        let idx = ((2 * 8 + 2) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[200, 1, 2, 255]);
    }

    #[test]
    fn scene_clear_removes_layers() {
        let mut scene = RenderScene::new();
        scene.add_layer(SceneLayer::new(1));
        assert_eq!(scene.layers().len(), 1);
        scene.clear();
        assert!(scene.layers().is_empty());
    }

    #[test]
    fn scene_composes_through_paint_backend_strategy() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            color: Color::rgba(7, 8, 9, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 255));

        let idx = ((1 * 8 + 1) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[7, 8, 9, 255]);
    }

    #[test]
    fn software_backend_delegates_text_shaping() {
        let backend = SoftwarePaintBackend::new(Size { width: 100, height: 40 }, 1.0);
        let shaped = backend.shape_text("e\u{0301}", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }

    #[test]
    fn draw_text_rasterizes_glyph_instead_of_full_rect_fill() {
        let mut surface = SoftwareSurface::new(Size { width: 80, height: 30 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_text(Point { x: 4, y: 4 }, "A", &font(), Color::rgba(255, 255, 255, 255));
        surface.end_frame();

        let metrics = surface.measure_text("A", &font());
        let mut painted = 0usize;
        for y in 4..(4 + metrics.height as i32) {
            for x in 4..(4 + metrics.width as i32) {
                let idx = ((y as u32 * surface.size().width + x as u32) * 4 + 3) as usize;
                if surface.frame_rgba()[idx] > 0 {
                    painted += 1;
                }
            }
        }

        let bbox_area = (metrics.width as usize).saturating_mul(metrics.height as usize);
        assert!(painted > 0);
        assert!(painted < bbox_area);
    }

    #[test]
    fn fill_circle_writes_center_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 255));
        surface.fill_circle(Point { x: 6, y: 6 }, 3, Color::rgba(9, 10, 11, 255));
        surface.end_frame();

        let idx = ((6 * 12 + 6) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[9, 10, 11, 255]);
    }

    #[test]
    fn scene_composition_supports_circle_commands() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillCircle {
            center: Point { x: 5, y: 5 },
            radius: 2,
            color: Color::rgba(3, 4, 5, 255),
        });
        layer.push(RenderCommand::DrawCircle {
            center: Point { x: 5, y: 5 },
            radius: 2,
            color: Color::rgba(200, 201, 202, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 255));

        let stroke_idx = ((5 * 12 + 7) * 4) as usize;
        let stroke_px = &backend.frame_rgba()[stroke_idx..stroke_idx + 4];
        assert!(stroke_px[0] > 3 && stroke_px[1] > 4 && stroke_px[2] > 5);
        assert_eq!(stroke_px[3], 255);

        let fill_idx = ((5 * 12 + 5) * 4) as usize;
        let fill_px = &backend.frame_rgba()[fill_idx..fill_idx + 4];
        assert!(fill_px[0] >= 3 && fill_px[0] < 32);
        assert!(fill_px[1] >= 4 && fill_px[1] < 32);
        assert!(fill_px[2] >= 5 && fill_px[2] < 32);
        assert_eq!(fill_px[3], 255);
    }

    #[test]
    fn draw_circle_with_width_expands_stroke_band() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_circle_with_width(
            Point { x: 8, y: 8 },
            4,
            Color::rgba(170, 171, 172, 255),
            2,
        );
        surface.end_frame();

        let outer_idx = ((8 * 16 + 12) * 4) as usize;
        assert!(surface.frame_rgba()[outer_idx + 3] > 0);

        let inner_band_idx = ((8 * 16 + 10) * 4) as usize;
        assert!(surface.frame_rgba()[inner_band_idx + 3] > 0);

        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(surface.frame_rgba()[center_idx + 3], 0);
    }

    #[test]
    fn fill_circle_aa_applies_partial_alpha_on_edge_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(190, 191, 192, 255));
        surface.end_frame();

        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[190, 191, 192, 255]);

        let edge_idx = ((8 * 16 + 12) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn scene_composition_supports_stroke_circle_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawCircleStroke {
            center: Point { x: 8, y: 8 },
            radius: 4,
            color: Color::rgba(180, 181, 182, 255),
            width: 2,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let band_idx = ((8 * 16 + 10) * 4) as usize;
        let band_px = &backend.frame_rgba()[band_idx..band_idx + 4];
        assert_eq!(band_px[0], 180);
        assert_eq!(band_px[1], 181);
        assert_eq!(band_px[2], 182);
        assert!(band_px[3] > 0 && band_px[3] < 255);
    }

    #[test]
    fn scene_composition_supports_aa_fill_circle_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillCircleAA {
            center: Point { x: 8, y: 8 },
            radius: 4,
            color: Color::rgba(200, 201, 202, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[center_idx..center_idx + 4],
            &[200, 201, 202, 255]
        );

        let edge_idx = ((8 * 16 + 12) * 4) as usize;
        let edge_alpha = backend.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn draw_line_with_width_marks_neighbor_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 255));
        surface.draw_line_with_width(
            Point { x: 2, y: 6 },
            Point { x: 9, y: 6 },
            Color::rgba(21, 22, 23, 255),
            3,
        );
        surface.end_frame();

        let center_idx = ((6 * 12 + 5) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[21, 22, 23, 255]);

        let upper_idx = ((5 * 12 + 5) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[upper_idx..upper_idx + 4], &[21, 22, 23, 255]);

        let lower_idx = ((7 * 12 + 5) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[lower_idx..lower_idx + 4], &[21, 22, 23, 255]);
    }

    #[test]
    fn scene_composition_supports_stroke_line_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x: 2, y: 6 },
            to: Point { x: 9, y: 6 },
            color: Color::rgba(31, 32, 33, 255),
            width: 3,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 255));

        let idx = ((5 * 12 + 5) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[31, 32, 33, 255]);
    }

    #[test]
    fn draw_rect_with_width_marks_neighbor_border_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 255));
        surface.draw_rect_with_width(
            Rect {
                x: 4,
                y: 4,
                width: 6,
                height: 6,
            },
            Color::rgba(41, 42, 43, 255),
            3,
        );
        surface.end_frame();

        let border_idx = ((4 * 14 + 6) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[border_idx..border_idx + 4], &[41, 42, 43, 255]);

        let neighbor_idx = ((5 * 14 + 6) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[neighbor_idx..neighbor_idx + 4],
            &[41, 42, 43, 255]
        );
    }

    #[test]
    fn scene_composition_supports_stroke_rect_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x: 4,
                y: 4,
                width: 6,
                height: 6,
            },
            color: Color::rgba(51, 52, 53, 255),
            width: 3,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 14, height: 14 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 255));

        let idx = ((5 * 14 + 6) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[51, 52, 53, 255]);
    }

    #[test]
    fn fill_rounded_rect_writes_center_and_preserves_corner() {
        let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 255));
        surface.fill_rounded_rect(
            Rect {
                x: 3,
                y: 3,
                width: 8,
                height: 8,
            },
            3,
            Color::rgba(61, 62, 63, 255),
        );
        surface.end_frame();

        let center_idx = ((7 * 14 + 7) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[61, 62, 63, 255]);

        let corner_idx = ((3 * 14 + 3) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[corner_idx..corner_idx + 4], &[0, 0, 0, 255]);
    }

    #[test]
    fn scene_composition_supports_rounded_rect_commands() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRoundedRect {
            rect: Rect {
                x: 3,
                y: 3,
                width: 8,
                height: 8,
            },
            radius: 3,
            color: Color::rgba(71, 72, 73, 255),
        });
        layer.push(RenderCommand::DrawRoundedRectStroke {
            rect: Rect {
                x: 3,
                y: 3,
                width: 8,
                height: 8,
            },
            radius: 3,
            color: Color::rgba(81, 82, 83, 255),
            width: 2,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 14, height: 14 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 255));

        let stroke_idx = ((3 * 14 + 7) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[stroke_idx..stroke_idx + 4], &[81, 82, 83, 255]);

        let fill_idx = ((7 * 14 + 7) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[fill_idx..fill_idx + 4], &[71, 72, 73, 255]);
    }

    #[test]
    fn draw_rounded_rect_aa_with_width_produces_soft_edge() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_rounded_rect_aa_with_width(
            Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            4,
            Color::rgba(230, 231, 232, 255),
            2,
        );
        surface.end_frame();

        let core_idx = ((3 * 16 + 8) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[core_idx..core_idx + 4], &[230, 231, 232, 255]);

        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn fill_rounded_rect_aa_produces_soft_corner_edge() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_rounded_rect_aa(
            Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            4,
            Color::rgba(250, 210, 170, 255),
        );
        surface.end_frame();

        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[center_idx..center_idx + 4], &[250, 210, 170, 255]);

        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn aa_sample_level_changes_rounded_rect_edge_coverage() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);

        surface.set_aa_samples_per_axis(1);
        assert_eq!(surface.aa_samples_per_axis(), 1);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_rounded_rect_aa(
            Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            4,
            Color::rgba(200, 100, 50, 255),
        );
        surface.end_frame();
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let alpha_low = surface.frame_rgba()[edge_idx + 3];

        surface.set_aa_samples_per_axis(4);
        assert_eq!(surface.aa_samples_per_axis(), 4);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_rounded_rect_aa(
            Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            4,
            Color::rgba(200, 100, 50, 255),
        );
        surface.end_frame();
        let alpha_high = surface.frame_rgba()[edge_idx + 3];

        assert_ne!(alpha_low, alpha_high);
    }

    #[test]
    fn render_config_applies_and_clamps_aa_samples() {
        let mut surface = SoftwareSurface::new(Size { width: 8, height: 8 }, 1.0);
        assert_eq!(surface.render_config().aa_samples_per_axis, 4);

        surface.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 0,
        });
        assert_eq!(surface.render_config().aa_samples_per_axis, 1);

        surface.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 12,
        });
        assert_eq!(surface.render_config().aa_samples_per_axis, 8);
    }

    #[test]
    fn backend_render_config_passthrough_clamps_values() {
        let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
        assert_eq!(backend.render_config().aa_samples_per_axis, 4);

        backend.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 0,
        });
        assert_eq!(backend.render_config().aa_samples_per_axis, 1);

        backend.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 20,
        });
        assert_eq!(backend.render_config().aa_samples_per_axis, 8);
    }

    #[test]
    fn paint_backend_trait_render_config_updates_software_backend() {
        let mut backend = SoftwarePaintBackend::new(Size { width: 8, height: 8 }, 1.0);
        PaintBackend::apply_render_config(
            &mut backend,
            SoftwareRenderConfig {
                aa_samples_per_axis: 3,
            },
        );
        assert_eq!(PaintBackend::render_config(&backend).aa_samples_per_axis, 3);
    }

    #[test]
    fn scene_compose_with_temporary_config_restores_backend_state() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRoundedRectAA {
            rect: Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            radius: 4,
            color: Color::rgba(100, 110, 120, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        backend.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });

        scene.compose_with_backend_config(
            &mut backend,
            Color::rgba(0, 0, 0, 0),
            Some(SoftwareRenderConfig {
                aa_samples_per_axis: 1,
            }),
        );

        assert_eq!(backend.render_config().aa_samples_per_axis, 4);
    }

    #[test]
    fn scene_compose_with_temporary_config_changes_aa_output() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRoundedRectAA {
            rect: Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            radius: 4,
            color: Color::rgba(150, 151, 152, 255),
        });
        scene.add_layer(layer);

        let mut backend_default = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        backend_default.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });
        scene.compose_with_backend(&mut backend_default, Color::rgba(0, 0, 0, 0));
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let alpha_default = backend_default.frame_rgba()[edge_idx + 3];

        let mut backend_temp = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        backend_temp.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });
        scene.compose_with_backend_config(
            &mut backend_temp,
            Color::rgba(0, 0, 0, 0),
            Some(SoftwareRenderConfig {
                aa_samples_per_axis: 1,
            }),
        );
        let alpha_temp = backend_temp.frame_rgba()[edge_idx + 3];

        assert_ne!(alpha_default, alpha_temp);
    }

    #[test]
    fn aa_sample_level_changes_circle_edge_coverage() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);

        surface.set_aa_samples_per_axis(1);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
        surface.end_frame();
        let edge_idx = ((8 * 16 + 12) * 4) as usize;
        let alpha_low = surface.frame_rgba()[edge_idx + 3];

        surface.set_aa_samples_per_axis(4);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
        surface.end_frame();
        let alpha_high = surface.frame_rgba()[edge_idx + 3];

        assert_ne!(alpha_low, alpha_high);
    }

    #[test]
    fn aa_sample_level_changes_line_edge_coverage() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);

        surface.set_aa_samples_per_axis(1);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_line_aa_with_width(
            Point { x: 2, y: 2 },
            Point { x: 13, y: 9 },
            Color::rgba(130, 131, 132, 255),
            3,
        );
        surface.end_frame();
        let edge_idx = ((5 * 16 + 6) * 4) as usize;
        let alpha_low = surface.frame_rgba()[edge_idx + 3];

        surface.set_aa_samples_per_axis(4);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_line_aa_with_width(
            Point { x: 2, y: 2 },
            Point { x: 13, y: 9 },
            Color::rgba(130, 131, 132, 255),
            3,
        );
        surface.end_frame();
        let alpha_high = surface.frame_rgba()[edge_idx + 3];

        assert_ne!(alpha_low, alpha_high);
    }

    #[test]
    fn circle_stroke_applies_partial_alpha_on_edge_pixels() {
        let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_circle(Point { x: 7, y: 7 }, 3, Color::rgba(100, 120, 140, 255));
        surface.end_frame();

        let edge_idx = ((8 * 14 + 10) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn rounded_rect_fill_applies_partial_alpha_at_corner_edge() {
        let mut surface = SoftwareSurface::new(Size { width: 14, height: 14 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.fill_rounded_rect(
            Rect {
                x: 3,
                y: 3,
                width: 8,
                height: 8,
            },
            3,
            Color::rgba(90, 91, 92, 255),
        );
        surface.end_frame();

        let edge_idx = ((4 * 14 + 3) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn draw_line_aa_produces_partial_alpha_on_neighbor_pixel() {
        let mut surface = SoftwareSurface::new(Size { width: 12, height: 12 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_line_aa(
            Point { x: 1, y: 1 },
            Point { x: 10, y: 8 },
            Color::rgba(110, 120, 130, 255),
        );
        surface.end_frame();

        let neighbor_idx = ((3 * 12 + 4) * 4) as usize;
        let alpha = surface.frame_rgba()[neighbor_idx + 3];
        assert!(alpha > 0 && alpha < 255);
    }

    #[test]
    fn scene_composition_supports_aa_line_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawLineAA {
            from: Point { x: 1, y: 1 },
            to: Point { x: 10, y: 8 },
            color: Color::rgba(140, 150, 160, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 12, height: 12 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let idx = ((3 * 12 + 4) * 4) as usize;
        let px = &backend.frame_rgba()[idx..idx + 4];
        assert_eq!(px[0], 140);
        assert_eq!(px[1], 150);
        assert_eq!(px[2], 160);
        assert!(px[3] > 0 && px[3] < 255);
    }

    #[test]
    fn draw_line_aa_with_width_expands_band_and_keeps_soft_edge() {
        let mut surface = SoftwareSurface::new(Size { width: 16, height: 16 }, 1.0);
        surface.begin_frame(Color::rgba(0, 0, 0, 0));
        surface.draw_line_aa_with_width(
            Point { x: 2, y: 8 },
            Point { x: 13, y: 8 },
            Color::rgba(210, 211, 212, 255),
            3,
        );
        surface.end_frame();

        let core_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[core_idx..core_idx + 4], &[210, 211, 212, 255]);

        let edge_idx = ((9 * 16 + 8) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn scene_composition_supports_aa_stroke_line_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawLineStrokeAA {
            from: Point { x: 2, y: 8 },
            to: Point { x: 13, y: 8 },
            color: Color::rgba(220, 221, 222, 255),
            width: 3,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let core_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[core_idx..core_idx + 4],
            &[220, 221, 222, 255]
        );

        let edge_idx = ((9 * 16 + 8) * 4) as usize;
        let edge_alpha = backend.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn scene_composition_supports_aa_stroke_rounded_rect_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawRoundedRectStrokeAA {
            rect: Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            radius: 4,
            color: Color::rgba(240, 241, 242, 255),
            width: 2,
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let core_idx = ((3 * 16 + 8) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[core_idx..core_idx + 4],
            &[240, 241, 242, 255]
        );

        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = backend.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }

    #[test]
    fn scene_composition_supports_aa_fill_rounded_rect_command() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRoundedRectAA {
            rect: Rect {
                x: 3,
                y: 3,
                width: 10,
                height: 10,
            },
            radius: 4,
            color: Color::rgba(120, 130, 140, 255),
        });
        scene.add_layer(layer);

        let mut backend = SoftwarePaintBackend::new(Size { width: 16, height: 16 }, 1.0);
        scene.compose_with_backend(&mut backend, Color::rgba(0, 0, 0, 0));

        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[center_idx..center_idx + 4],
            &[120, 130, 140, 255]
        );

        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = backend.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }
}