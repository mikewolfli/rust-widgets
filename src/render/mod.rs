//! Rendering primitives and software surface baseline.
//!
//! # Coordinate System
//!
//! This module uses the framework's standard **screen coordinate system** with origin at **top-left**:
//!
//! - **X axis**: Increases from left to right (0 → width)
//! - **Y axis**: Increases from top to bottom (0 → height)
//!
//! All rendering operations (drawing text, shapes, images) expect coordinates in this system.
//! The rendering context automatically handles any necessary transformations when working with
//! widgets or other components that may use different coordinate systems internally.
//!
//! ## Drawing Operations
//!
//! - `draw_text()`: Draws text at the specified (x, y) position
//! - `draw_line()`: Draws a line from (x1, y1) to (x2, y2)
//! - `draw_rect()`: Draws a rectangle outline
//! - `fill_rect()`: Fills a rectangle with a solid color
//! - `draw_image()`: Draws an image at the specified position
//!
//! All coordinates are in logical pixels and use the screen coordinate system.
pub mod batch;
pub mod quality;
pub mod text_cache;
use crate::core::{Color, Font, Point, Rect, Size};
use crate::widget::{
    ActivityIndicator, Button, ButtonState, Canvas, ChartWidget, CheckBox, CheckState, ColorDialog,
    ComboBox, ContextMenu, Dialog, DirectoryDialog, DockPanel, FileDialog, FontDialog, GridWidget,
    GroupBox, Label, LineEdit, ListBox, MdiArea, Menu, MenuBar, MessageBox, Panel, PopupWindow,
    ProgressBar, RadioButton, RichEdit, ScrollBar, Slider, Splitter, StatusBar, TabWidget,
    TableWidget, TextEdit, ToolBar, TreeView, Widget,
};
use crate::window::Window;
use font8x8::{UnicodeFonts, BASIC_FONTS};
// use rayon::prelude::*;
// use std::simd::{u8x4, Simd};
use std::sync::{Mutex, OnceLock};
/// Returns true if given rect is empty (width == 0 or height == 0).
fn is_empty_rect(rect: &crate::core::Rect) -> bool {
    rect.width == 0 || rect.height == 0
}
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;
#[cfg(feature = "quality-management")]
use crate::quality::QualityManager;
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
/// Draw command recorded by a render layer.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    FillRect {
        rect: Rect,
        color: Color,
    },
    DrawRect {
        rect: Rect,
        color: Color,
    },
    DrawRectStroke {
        rect: Rect,
        color: Color,
        width: u32,
    },
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
    DrawLine {
        from: Point,
        to: Point,
        color: Color,
    },
    DrawLineAA {
        from: Point,
        to: Point,
        color: Color,
    },
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
    FillCircle {
        center: Point,
        radius: u32,
        color: Color,
    },
    FillCircleAA {
        center: Point,
        radius: u32,
        color: Color,
    },
    DrawCircle {
        center: Point,
        radius: u32,
        color: Color,
    },
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
/// Backend selected by automatic compose path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoRenderBackend {
    /// GPU path using feature-gated `wgpu` backend.
    GpuWgpu,
    /// CPU software raster path.
    CpuSoftware,
}
fn global_last_auto_render_backend() -> &'static Mutex<AutoRenderBackend> {
    static LAST_BACKEND: OnceLock<Mutex<AutoRenderBackend>> = OnceLock::new();
    LAST_BACKEND.get_or_init(|| Mutex::new(AutoRenderBackend::CpuSoftware))
}
fn set_last_auto_render_backend(backend: AutoRenderBackend) {
    *global_last_auto_render_backend()
        .lock()
        .expect("auto render backend lock poisoned") = backend;
}
/// Returns last backend selected by `RenderScene::compose_to_config_auto`.
pub fn last_auto_render_backend() -> AutoRenderBackend {
    *global_last_auto_render_backend()
        .lock()
        .expect("auto render backend lock poisoned")
}
#[cfg(feature = "quality-management")]
/// Returns the current rendering quality level.
pub fn current_quality_level() -> crate::quality::QualityLevel {
    global_quality_manager()
        .lock()
        .expect("quality manager lock poisoned")
        .quality_level()
}
#[cfg(feature = "quality-management")]
/// Sets the rendering quality level manually.
pub fn set_quality_level(level: crate::quality::QualityLevel) {
    let mut quality_manager = global_quality_manager()
        .lock()
        .expect("quality manager lock poisoned");
    quality_manager.set_quality_level(level);
}
#[cfg(feature = "quality-management")]
/// Returns the current frame rate.
pub fn current_fps() -> f32 {
    global_quality_manager()
        .lock()
        .expect("quality manager lock poisoned")
        .current_fps()
}
#[cfg(feature = "quality-management")]
/// Returns the average frame time in seconds.
pub fn average_frame_time() -> f32 {
    global_quality_manager()
        .lock()
        .expect("quality manager lock poisoned")
        .average_frame_time()
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
        let _ = self.compose_to_config_auto(surface, clear, config);
    }
    /// Compose scene layers to target surface using automatic backend strategy.
    ///
    /// Strategy is unified across desktop and embedded builds:
    /// - when `gpu-wgpu` is enabled and runtime GPU initialization succeeds,
    ///   use GPU for supported command sets;
    /// - otherwise fall back to CPU software rendering.
    pub fn compose_to_config_auto(
        &self,
        surface: &mut SoftwareSurface,
        clear: Color,
        config: Option<SoftwareRenderConfig>,
    ) -> AutoRenderBackend {
        #[cfg(feature = "gpu-wgpu")]
        {
            if compose_scene_to_surface_wgpu(self, surface, clear, config).is_ok() {
                set_last_auto_render_backend(AutoRenderBackend::GpuWgpu);
                return AutoRenderBackend::GpuWgpu;
            }
        }
        compose_scene_to_surface_software(self, surface, clear, config);
        set_last_auto_render_backend(AutoRenderBackend::CpuSoftware);
        AutoRenderBackend::CpuSoftware
    }
}
fn compose_scene_to_surface_software(
    scene: &RenderScene,
    surface: &mut SoftwareSurface,
    clear: Color,
    config: Option<SoftwareRenderConfig>,
) {
    let mut backend = SoftwarePaintBackend::new(surface.size(), surface.dpi_scale());
    backend.set_size(surface.size());
    backend.apply_render_config(surface.render_config());
    scene.compose_with_backend_config(&mut backend, clear, config);
    surface.buffer = backend.surface.buffer;
}
#[cfg(feature = "quality-management")]
fn global_quality_manager() -> &'static Mutex<QualityManager> {
    static MANAGER: OnceLock<Mutex<QualityManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Mutex::new(QualityManager::new()))
}
#[cfg(feature = "gpu-wgpu")]
#[derive(Debug)]
pub enum GpuRenderError {
    SurfaceSizeZero,
    RendererUnavailable,
    UploadFailed(String),
    Other(String),
}
impl std::fmt::Display for GpuRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuRenderError::SurfaceSizeZero => {
                write!(f, "surface size must be > 0 for gpu compose")
            }
            GpuRenderError::RendererUnavailable => write!(f, "wgpu renderer unavailable"),
            GpuRenderError::UploadFailed(e) => write!(f, "upload failed: {e}"),
            GpuRenderError::Other(e) => write!(f, "gpu error: {e}"),
        }
    }
}
impl std::error::Error for GpuRenderError {}
#[cfg(feature = "gpu-wgpu")]
fn compose_scene_to_surface_wgpu(
    scene: &RenderScene,
    surface: &mut SoftwareSurface,
    clear: Color,
    config: Option<SoftwareRenderConfig>,
) -> Result<(), GpuRenderError> {
    let size = surface.size();
    if size.width == 0 || size.height == 0 {
        return Err(GpuRenderError::SurfaceSizeZero);
    }
    let renderer = cached_wgpu_renderer().ok_or(GpuRenderError::RendererUnavailable)?;
    let start_time = std::time::Instant::now();
    let mut backend = SoftwarePaintBackend::new(size, surface.dpi_scale());
    backend.set_size(size);
    backend.apply_render_config(surface.render_config());
    scene.compose_with_backend_config(&mut backend, clear, config);
    let pixels = renderer
        .upload_rgba8_and_readback(size.width, size.height, backend.frame_rgba())
        .map_err(GpuRenderError::UploadFailed)?;
    surface.buffer.back = pixels;
    surface.buffer.present();
    let frame_duration = start_time.elapsed();
    #[cfg(feature = "quality-management")]
    {
        let mut quality_manager = global_quality_manager()
            .lock()
            .expect("quality manager lock poisoned");
        quality_manager.finish_frame(frame_duration);
    }
    Ok(())
}
#[cfg(feature = "gpu-wgpu")]
fn cached_wgpu_renderer() -> Option<&'static WgpuRenderer> {
    static RENDERER: OnceLock<Option<WgpuRenderer>> = OnceLock::new();
    RENDERER.get_or_init(|| WgpuRenderer::new().ok()).as_ref()
}
fn push_widget_fill_and_border<W: Widget>(
    layer: &mut SceneLayer,
    widget: &W,
    fallback_background: Option<Color>,
    fallback_border: Option<(Color, u32)>,
) {
    let rect = widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    if let Some(background) = widget.background_color().or(fallback_background) {
        if widget.border_radius() > 0 {
            layer.push(RenderCommand::FillRoundedRect {
                rect,
                radius: widget.border_radius(),
                color: background,
            });
        } else {
            layer.push(RenderCommand::FillRect {
                rect,
                color: background,
            });
        }
    }
    let border_color = widget
        .border_color()
        .or_else(|| fallback_border.map(|value| value.0));
    let border_width = if widget.border_width() > 0 {
        widget.border_width()
    } else {
        fallback_border.map(|value| value.1).unwrap_or(0)
    };
    if let Some(color) = border_color {
        if border_width > 0 {
            if widget.border_radius() > 0 {
                layer.push(RenderCommand::DrawRoundedRectStroke {
                    rect,
                    radius: widget.border_radius(),
                    color,
                    width: border_width,
                });
            } else {
                layer.push(RenderCommand::DrawRectStroke {
                    rect,
                    color,
                    width: border_width,
                });
            }
        }
    }
}
fn centered_text_origin(rect: Rect) -> Point {
    Point {
        x: rect.x + 6,
        y: rect.y + (rect.height as i32 / 2) - 4,
    }
}
fn normalized_progress_u32(value: u32, min: u32, max: u32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value.saturating_sub(min)) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}
fn normalized_progress_i32(value: i32, min: i32, max: i32) -> f32 {
    if max <= min {
        return 0.0;
    }
    ((value - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}
/// Append visual commands for a `Window` baseline representation.
pub fn append_window_visual_commands(layer: &mut SceneLayer, window: &Window) {
    push_widget_fill_and_border(
        layer,
        window,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = window.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: window.title().to_string(),
            font: window.font().cloned().unwrap_or_default(),
            color: window.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `Panel` baseline representation.
pub fn append_panel_visual_commands(layer: &mut SceneLayer, panel: &Panel) {
    push_widget_fill_and_border(
        layer,
        panel,
        Some(Color::rgba(232, 235, 240, 255)),
        Some((Color::rgba(146, 152, 165, 255), 1)),
    );
}
/// Append visual commands for a `Label` baseline representation.
pub fn append_label_visual_commands(layer: &mut SceneLayer, label: &Label) {
    push_widget_fill_and_border(layer, label, None, None);
    if !label.text().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(label.geometry()),
            text: label.text().to_string(),
            font: label.font().cloned().unwrap_or_default(),
            color: label
                .foreground_color()
                .unwrap_or(Color::rgba(30, 30, 30, 255)),
        });
    }
}
/// Append visual commands for a `Button` baseline representation.
pub fn append_button_visual_commands(layer: &mut SceneLayer, button: &Button) {
    let fallback_bg = match button.state() {
        ButtonState::Pressed => Color::PRIMARY,
        ButtonState::Disabled => Color::LIGHT_GRAY,
        ButtonState::Normal => Color::PRIMARY,
    };
    let fallback_fg = if matches!(button.state(), ButtonState::Disabled) {
        Color::GRAY
    } else {
        Color::WHITE
    };
    push_widget_fill_and_border(
        layer,
        button,
        Some(fallback_bg),
        Some((Color::DARK_GRAY, 1)),
    );
    if !button.text().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(button.geometry()),
            text: button.text().to_string(),
            font: button.font().cloned().unwrap_or_default(),
            color: button.foreground_color().unwrap_or(fallback_fg),
        });
    }
}
/// Append visual commands for a `CheckBox` baseline representation.
pub fn append_checkbox_visual_commands(layer: &mut SceneLayer, checkbox: &CheckBox) {
    let rect = checkbox.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let box_side = rect.width.min(rect.height).clamp(8, 16);
    let indicator = Rect {
        x: rect.x + 2,
        y: rect.y + ((rect.height as i32 - box_side as i32) / 2),
        width: box_side,
        height: box_side,
    };
    layer.push(RenderCommand::FillRect {
        rect: indicator,
        color: checkbox
            .background_color()
            .unwrap_or(Color::rgba(250, 250, 250, 255)),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: indicator,
        color: checkbox
            .border_color()
            .unwrap_or(Color::rgba(90, 98, 108, 255)),
        width: checkbox.border_width().max(1),
    });
    match checkbox.state() {
        CheckState::Checked => {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: indicator.x + 3,
                    y: indicator.y + 3,
                    width: indicator.width.saturating_sub(6),
                    height: indicator.height.saturating_sub(6),
                },
                color: checkbox
                    .foreground_color()
                    .unwrap_or(Color::rgba(40, 120, 230, 255)),
            });
        }
        CheckState::PartiallyChecked => {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: indicator.x + 2,
                    y: indicator.y + (indicator.height as i32 / 2) - 1,
                    width: indicator.width.saturating_sub(4),
                    height: 2,
                },
                color: checkbox
                    .foreground_color()
                    .unwrap_or(Color::rgba(40, 120, 230, 255)),
            });
        }
        CheckState::Unchecked => {}
    }
}
/// Append visual commands for a `RadioButton` baseline representation.
pub fn append_radiobutton_visual_commands(layer: &mut SceneLayer, radio: &RadioButton) {
    let rect = radio.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let radius = (rect.width.min(rect.height).min(16) / 2).max(4);
    let center = Point {
        x: rect.x + 2 + radius as i32,
        y: rect.y + (rect.height as i32 / 2),
    };
    layer.push(RenderCommand::DrawCircleStroke {
        center,
        radius,
        color: radio
            .border_color()
            .unwrap_or(Color::rgba(92, 98, 108, 255)),
        width: radio.border_width().max(1),
    });
    if radio.is_checked() {
        layer.push(RenderCommand::FillCircle {
            center,
            radius: radius.saturating_sub(3).max(1),
            color: radio
                .foreground_color()
                .unwrap_or(Color::rgba(45, 122, 235, 255)),
        });
    }
}
/// Append visual commands for a `LineEdit` baseline representation.
pub fn append_line_edit_visual_commands(layer: &mut SceneLayer, line_edit: &LineEdit) {
    push_widget_fill_and_border(
        layer,
        line_edit,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = line_edit.text().to_string();
    if !text.is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(line_edit.geometry()),
            text,
            font: line_edit.font().cloned().unwrap_or_default(),
            color: line_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
}
/// Append visual commands for a `ComboBox` baseline representation.
pub fn append_combo_box_visual_commands(layer: &mut SceneLayer, combo_box: &ComboBox) {
    let rect = combo_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Render main background
    layer.push(RenderCommand::FillRect {
        rect,
        color: Color::WHITE,
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect,
        color: Color::rgba(122, 128, 138, 255),
        width: 1,
    });
    let arrow_width = 14u32.min(rect.width);
    let arrow_rect = Rect {
        x: rect.x + rect.width as f32 as i32 - arrow_width as i32,
        y: rect.y,
        width: arrow_width,
        height: rect.height,
    };
    layer.push(RenderCommand::FillRect {
        rect: arrow_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: arrow_rect,
        color: Color::rgba(122, 128, 138, 255),
        width: 1,
    });
    let text = combo_box.current_text();
    if !text.is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(rect),
            text,
            font: combo_box.font().cloned().unwrap_or_default(),
            color: combo_box
                .foreground_color()
                .unwrap_or(Color::rgba(28, 30, 34, 255)),
        });
    }
    if combo_box.count() > 0 {
        let popup_rows = combo_box.count().min(4) as u32;
        let row_height = rect.height.max(16);
        let popup_rect = Rect {
            x: rect.x,
            y: rect.y + rect.height as f32 as i32,
            width: rect.width,
            height: row_height.saturating_mul(popup_rows),
        };
        layer.push(RenderCommand::FillRect {
            rect: popup_rect,
            color: Color::rgba(250, 250, 252, 255),
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: popup_rect,
            color: Color::rgba(122, 128, 138, 255),
            width: 1,
        });
        for row in 0..popup_rows as usize {
            let item_rect = Rect {
                x: popup_rect.x + 2,
                y: popup_rect.y + row as i32 * row_height as i32,
                width: popup_rect.width.saturating_sub(4),
                height: row_height,
            };
            if combo_box.current_index() == Some(row) {
                layer.push(RenderCommand::FillRect {
                    rect: item_rect,
                    color: Color::rgba(206, 226, 255, 255),
                });
            }
            if let Some(item) = combo_box.item(row) {
                layer.push(RenderCommand::DrawText {
                    origin: centered_text_origin(item_rect),
                    text: item.to_string(),
                    font: combo_box.font().cloned().unwrap_or_default(),
                    color: Color::rgba(28, 30, 34, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ListBox` baseline representation.
pub fn append_list_box_visual_commands(layer: &mut SceneLayer, list_box: &ListBox) {
    push_widget_fill_and_border(
        layer,
        list_box,
        Some(Color::rgba(252, 252, 253, 255)),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = list_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 16u32;
    let max_rows = (rect.height / row_height).clamp(1, 4) as usize;
    for row in 0..list_box.count().min(max_rows) {
        let item_rect = Rect {
            x: rect.x + 2,
            y: rect.y + 2 + row as i32 * row_height as i32,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        if let Some(item) = list_box.item(row) {
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(item_rect),
                text: item.to_string(),
                font: list_box.font().cloned().unwrap_or_default(),
                color: list_box
                    .foreground_color()
                    .unwrap_or(Color::rgba(30, 32, 36, 255)),
            });
        }
    }
}
/// Append visual commands for a `ProgressBar` value representation.
pub fn append_progress_bar_visual_commands(layer: &mut SceneLayer, progress_bar: &ProgressBar) {
    push_widget_fill_and_border(
        layer,
        progress_bar,
        Some(Color::rgba(232, 236, 243, 255)),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = progress_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let ratio = normalized_progress_i32(
        progress_bar.value(),
        progress_bar.minimum(),
        progress_bar.maximum(),
    );
    let filled_width = ((rect.width as f32) * ratio).round() as u32;
    if filled_width > 0 {
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: rect.y,
                width: filled_width.min(rect.width),
                height: rect.height,
            },
            color: progress_bar.foreground_color().unwrap_or(Color::PRIMARY),
        });
    }
}
/// Append visual commands for a `Slider` value representation.
pub fn append_slider_visual_commands(layer: &mut SceneLayer, slider: &Slider) {
    let rect = slider.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::FillRect {
        rect,
        color: slider
            .background_color()
            .unwrap_or(Color::rgba(238, 241, 246, 255)),
    });
    let ratio = normalized_progress_i32(slider.value(), slider.minimum(), slider.maximum());
    if rect.width >= rect.height {
        let track_y = rect.y + rect.height as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: rect.x + 4,
                y: track_y,
            },
            to: Point {
                x: rect.x + rect.width as f32 as i32 - 4,
                y: track_y,
            },
            color: slider
                .border_color()
                .unwrap_or(Color::rgba(126, 132, 142, 255)),
            width: 2,
        });
        let thumb_x = rect.x + ((rect.width.saturating_sub(1) as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillCircle {
            center: Point {
                x: thumb_x,
                y: track_y,
            },
            radius: (rect.height / 3).max(3),
            color: slider
                .foreground_color()
                .unwrap_or(Color::rgba(70, 140, 248, 255)),
        });
    } else {
        let track_x = rect.x + rect.width as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: track_x,
                y: rect.y + 4,
            },
            to: Point {
                x: track_x,
                y: rect.y + rect.height as f32 as i32 - 4,
            },
            color: slider
                .border_color()
                .unwrap_or(Color::rgba(126, 132, 142, 255)),
            width: 2,
        });
        let thumb_y = rect.y + ((rect.height.saturating_sub(1) as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillCircle {
            center: Point {
                x: track_x,
                y: thumb_y,
            },
            radius: (rect.width / 3).max(3),
            color: slider
                .foreground_color()
                .unwrap_or(Color::rgba(70, 140, 248, 255)),
        });
    }
}
/// Append visual commands for a `ScrollBar` value representation.
pub fn append_scroll_bar_visual_commands(layer: &mut SceneLayer, scroll_bar: &ScrollBar) {
    let rect = scroll_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::FillRect {
        rect,
        color: scroll_bar
            .background_color()
            .unwrap_or(Color::rgba(229, 233, 239, 255)),
    });
    let ratio = normalized_progress_i32(
        scroll_bar.value(),
        scroll_bar.minimum(),
        scroll_bar.maximum(),
    );
    let denom = (scroll_bar.maximum() - scroll_bar.minimum()).max(1) as f32;
    let page_ratio = (scroll_bar.page_step().max(1) as f32
        / (denom + scroll_bar.page_step().max(1) as f32))
        .clamp(0.1, 1.0);
    if rect.width >= rect.height {
        let thumb_width = ((rect.width as f32) * page_ratio).round() as u32;
        let travel = rect.width.saturating_sub(thumb_width);
        let thumb_x = rect.x + ((travel as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: thumb_x,
                y: rect.y,
                width: thumb_width.max(6).min(rect.width),
                height: rect.height,
            },
            color: scroll_bar
                .foreground_color()
                .unwrap_or(Color::rgba(144, 151, 164, 255)),
        });
    } else {
        let thumb_height = ((rect.height as f32) * page_ratio).round() as u32;
        let travel = rect.height.saturating_sub(thumb_height);
        let thumb_y = rect.y + ((travel as f32) * ratio).round() as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: thumb_y,
                width: rect.width,
                height: thumb_height.max(6).min(rect.height),
            },
            color: scroll_bar
                .foreground_color()
                .unwrap_or(Color::rgba(144, 151, 164, 255)),
        });
    }
}
/// Append visual commands for a `MenuBar` host representation.
pub fn append_menu_bar_visual_commands(layer: &mut SceneLayer, menu_bar: &MenuBar) {
    push_widget_fill_and_border(
        layer,
        menu_bar,
        Some(Color::rgba(238, 242, 248, 255)),
        Some((Color::rgba(128, 134, 144, 255), 1)),
    );
    let rect = menu_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let menu_count = menu_bar.entries().len().max(1);
    let slot_width = (rect.width / menu_count as u32).max(20);
    for index in 0..menu_count {
        let slot_rect = Rect {
            x: rect.x + (index as u32 * slot_width) as i32,
            y: rect.y,
            width: slot_width.min(rect.width),
            height: rect.height,
        };
        if menu_bar.active_index().is_some() && index == 0 {
            layer.push(RenderCommand::FillRect {
                rect: slot_rect,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        let label = menu_bar
            .entries()
            .get(index)
            .map(|entry| entry.title.clone())
            .unwrap_or_else(|| format!("Menu{}", index + 1));
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(slot_rect),
            text: label,
            font: menu_bar.font().cloned().unwrap_or_default(),
            color: menu_bar
                .foreground_color()
                .unwrap_or(Color::rgba(32, 34, 38, 255)),
        });
    }
}
/// Append visual commands for a `Menu` host representation.
pub fn append_menu_visual_commands(layer: &mut SceneLayer, menu: &Menu) {
    push_widget_fill_and_border(
        layer,
        menu,
        Some(Color::rgba(250, 250, 251, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    let rect = menu.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw title if present
    let mut content_offset = 0i32;
    if !menu.title().is_empty() {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: menu.title().to_string(),
            font: menu.font().cloned().unwrap_or_default(),
            color: menu
                .foreground_color()
                .unwrap_or(Color::rgba(22, 24, 30, 255)),
        });
        content_offset = 20;
    }
    let row_height = 24u32;
    let icon_width = 0i32;
    let shortcut_width = 60;
    for (index, item) in menu.items().iter().enumerate() {
        let row_y = rect.y + content_offset + (index as u32 * row_height) as i32;
        let row_rect = Rect {
            x: rect.x + 2,
            y: row_y,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        // Draw selection highlight
        if Some(index) == menu.hovered_index() {
            layer.push(RenderCommand::FillRect {
                rect: row_rect,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        // Handle different item types
        if item.separator {
            // Draw separator line
            layer.push(RenderCommand::DrawLine {
                from: Point {
                    x: rect.x + 8,
                    y: row_y + (row_height / 2) as i32,
                },
                to: Point {
                    x: rect.x + rect.width as f32 as i32 - 8,
                    y: row_y + (row_height / 2) as i32,
                },
                color: Color::rgba(180, 186, 196, 255),
            });
        } else {
            // Draw checkmark for checkable items
            let mut text_offset_x = rect.x + 8;
            if item.checkable {
                if item.checked {
                    layer.push(RenderCommand::DrawText {
                        origin: Point {
                            x: text_offset_x,
                            y: row_y + 4,
                        },
                        text: "✓".to_string(),
                        font: menu.font().cloned().unwrap_or_default(),
                        color: Color::rgba(32, 34, 38, 255),
                    });
                }
                text_offset_x += 16;
            }
            text_offset_x += icon_width;
            // Draw item text
            let text_color = if item.enabled {
                menu.foreground_color()
                    .unwrap_or(Color::rgba(32, 34, 38, 255))
            } else {
                Color::rgba(128, 128, 128, 255)
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: text_offset_x,
                    y: row_y + 4,
                },
                text: item.text.clone(),
                font: menu.font().cloned().unwrap_or_default(),
                color: text_color,
            });
            // Draw shortcut if present
            if !item.shortcut.is_empty() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - shortcut_width,
                        y: row_y + 4,
                    },
                    text: item.shortcut.clone(),
                    font: menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
            // Draw submenu arrow
            if item.has_submenu {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - 16,
                        y: row_y + 4,
                    },
                    text: "▶".to_string(),
                    font: menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ContextMenu` host representation.
/// Reuses the same rendering logic as Menu for consistency.
pub fn append_context_menu_visual_commands(layer: &mut SceneLayer, context_menu: &ContextMenu) {
    push_widget_fill_and_border(
        layer,
        context_menu,
        Some(Color::rgba(250, 250, 251, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    let rect = context_menu.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let icon_width = 0i32;
    let shortcut_width = 60;
    for (index, item) in context_menu.items().iter().enumerate() {
        let row_y = rect.y + (index as u32 * row_height) as i32;
        let row_rect = Rect {
            x: rect.x + 2,
            y: row_y,
            width: rect.width.saturating_sub(4),
            height: row_height,
        };
        // Draw selection highlight
        if let Some(selected_idx) = context_menu.hovered_index() {
            if selected_idx == index {
                layer.push(RenderCommand::FillRect {
                    rect: row_rect,
                    color: Color::rgba(208, 224, 249, 255),
                });
            }
        }
        // Handle different item types
        if item.separator {
            // Draw separator line
            layer.push(RenderCommand::DrawLine {
                from: Point {
                    x: rect.x + 8,
                    y: row_y + (row_height / 2) as i32,
                },
                to: Point {
                    x: rect.x + rect.width as f32 as i32 - 8,
                    y: row_y + (row_height / 2) as i32,
                },
                color: Color::rgba(180, 186, 196, 255),
            });
        } else {
            // Draw checkmark for checkable items
            let mut text_offset_x = rect.x + 8;
            if item.checkable {
                if item.checked {
                    layer.push(RenderCommand::DrawText {
                        origin: Point {
                            x: text_offset_x,
                            y: row_y + 4,
                        },
                        text: "✓".to_string(),
                        font: context_menu.font().cloned().unwrap_or_default(),
                        color: Color::rgba(32, 34, 38, 255),
                    });
                }
                text_offset_x += 16;
            }
            text_offset_x += icon_width;
            // Draw item text
            let text_color = if item.enabled {
                context_menu
                    .foreground_color()
                    .unwrap_or(Color::rgba(32, 34, 38, 255))
            } else {
                Color::rgba(128, 128, 128, 255)
            };
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: text_offset_x,
                    y: row_y + 4,
                },
                text: item.text.clone(),
                font: context_menu.font().cloned().unwrap_or_default(),
                color: text_color,
            });
            // Draw shortcut if present
            if !item.shortcut.is_empty() {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - shortcut_width,
                        y: row_y + 4,
                    },
                    text: item.shortcut.clone(),
                    font: context_menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
            // Draw submenu arrow
            if item.has_submenu {
                layer.push(RenderCommand::DrawText {
                    origin: Point {
                        x: rect.x + rect.width as f32 as i32 - 16,
                        y: row_y + 4,
                    },
                    text: "▶".to_string(),
                    font: context_menu.font().cloned().unwrap_or_default(),
                    color: Color::rgba(100, 100, 100, 255),
                });
            }
        }
    }
}
/// Append visual commands for a `ToolBar` host representation.
pub fn append_tool_bar_visual_commands(layer: &mut SceneLayer, tool_bar: &ToolBar) {
    push_widget_fill_and_border(
        layer,
        tool_bar,
        Some(Color::rgba(236, 240, 246, 255)),
        Some((Color::rgba(126, 132, 142, 255), 1)),
    );
    let rect = tool_bar.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let mut cursor_x = rect.x + 4;
    let button_width = 32u32;
    let separator_width = 4u32;
    for (index, item) in tool_bar.items().iter().enumerate() {
        // Draw separator
        if item.separator {
            let separator_rect = Rect {
                x: cursor_x,
                y: rect.y + 4,
                width: separator_width,
                height: rect.height.saturating_sub(8),
            };
            layer.push(RenderCommand::FillRect {
                rect: separator_rect,
                color: Color::rgba(180, 186, 196, 255),
            });
            cursor_x += separator_width as i32 + 4;
            continue;
        }
        // Draw action item
        let action_rect = Rect {
            x: cursor_x,
            y: rect.y + 2,
            width: button_width,
            height: rect.height.saturating_sub(4),
        };
        // Draw selection highlight
        if item.checked {
            layer.push(RenderCommand::FillRoundedRect {
                rect: action_rect,
                radius: 3,
                color: Color::rgba(208, 224, 249, 255),
            });
        }
        // Draw button background
        layer.push(RenderCommand::FillRoundedRect {
            rect: action_rect,
            radius: 3,
            color: Color::rgba(216, 225, 238, 255),
        });
        // Draw item text (if no icon or as tooltip)
        let text_color = if item.enabled {
            tool_bar
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255))
        } else {
            Color::rgba(128, 128, 128, 255)
        };
        // Show first character as compact button text
        if !item.text.is_empty() {
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(action_rect),
                text: item.text.chars().take(1).collect::<String>(),
                font: tool_bar.font().cloned().unwrap_or_default(),
                color: text_color,
            });
        }
        cursor_x += button_width as i32 + 4;
        if cursor_x >= rect.x + rect.width as f32 as i32 {
            break;
        }
    }
}
/// Append visual commands for a `StatusBar` host representation.
pub fn append_status_bar_visual_commands(layer: &mut SceneLayer, status_bar: &StatusBar) {
    push_widget_fill_and_border(
        layer,
        status_bar,
        Some(Color::rgba(232, 236, 243, 255)),
        Some((Color::rgba(124, 130, 140, 255), 1)),
    );
    if !status_bar.message().is_empty() {
        let rect = status_bar.geometry();
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(rect),
            text: status_bar.message().to_string(),
            font: status_bar.font().cloned().unwrap_or_default(),
            color: status_bar
                .foreground_color()
                .unwrap_or(Color::rgba(34, 36, 40, 255)),
        });
    }
}
/// Append visual commands for a `TabWidget` navigation representation.
pub fn append_tab_widget_visual_commands(layer: &mut SceneLayer, tab_widget: &TabWidget) {
    push_widget_fill_and_border(
        layer,
        tab_widget,
        Some(Color::rgba(245, 247, 252, 255)),
        Some((Color::rgba(126, 132, 142, 255), 1)),
    );
    let rect = tab_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let count = tab_widget.count().max(1);
    let tab_height = rect.height.min(26);
    let tab_width = (rect.width / count as u32).max(24);
    for index in 0..count {
        let tab_rect = Rect {
            x: rect.x + (index as u32 * tab_width) as i32,
            y: rect.y,
            width: tab_width.min(rect.width),
            height: tab_height,
        };
        let is_current = tab_widget.current_index() == index;
        layer.push(RenderCommand::FillRect {
            rect: tab_rect,
            color: if is_current {
                Color::rgba(210, 224, 248, 255)
            } else {
                Color::rgba(229, 234, 242, 255)
            },
        });
        layer.push(RenderCommand::DrawText {
            origin: centered_text_origin(tab_rect),
            text: format!("Tab{}", index + 1),
            font: tab_widget.font().cloned().unwrap_or_default(),
            color: tab_widget
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255)),
        });
    }
}
/// Append visual commands for a `TextEdit` multi-line text editor representation.
pub fn append_text_edit_visual_commands(layer: &mut SceneLayer, text_edit: &TextEdit) {
    push_widget_fill_and_border(
        layer,
        text_edit,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = text_edit.text();
    if !text.is_empty() {
        let rect = text_edit.geometry();
        let padding = 4i32;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: rect.width.saturating_sub(padding as u32 * 2),
            height: rect.height.saturating_sub(padding as u32 * 2),
        };
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: text_rect.x,
                y: text_rect.y + text_rect.height as i32 / 2,
            },
            text: text.to_string(),
            font: text_edit.font().cloned().unwrap_or_default(),
            color: text_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
}
/// Append visual commands for a `RichEdit` rich text editor representation.
pub fn append_rich_edit_visual_commands(layer: &mut SceneLayer, rich_edit: &RichEdit) {
    let bg_color = if rich_edit.is_read_only() {
        Color::rgba(245, 245, 245, 255)
    } else {
        Color::WHITE
    };
    push_widget_fill_and_border(
        layer,
        rich_edit,
        Some(bg_color),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let text = rich_edit.text();
    if !text.is_empty() {
        let rect = rich_edit.geometry();
        let padding = 4i32;
        let text_rect = Rect {
            x: rect.x + padding,
            y: rect.y + padding,
            width: rect.width.saturating_sub(padding as u32 * 2),
            height: rect.height.saturating_sub(padding as u32 * 2),
        };
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: text_rect.x,
                y: text_rect.y + text_rect.height as i32 / 2,
            },
            text: text.to_string(),
            font: rich_edit.font().cloned().unwrap_or_default(),
            color: rich_edit
                .foreground_color()
                .unwrap_or(Color::rgba(26, 26, 26, 255)),
        });
    }
    // Draw selection highlight if present
    if let Some((start, end)) = rich_edit.selection() {
        if start != end {
            let rect = rich_edit.geometry();
            let padding = 4i32;
            let selection_width = ((end - start) as u32).min(20);
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + padding,
                    y: rect.y + padding,
                    width: selection_width,
                    height: rect.height.saturating_sub(padding as u32 * 2),
                },
                color: Color::rgba(128, 192, 255, 128),
            });
        }
    }
}
/// Append visual commands for a `TreeView` hierarchical data display representation.
pub fn append_tree_view_visual_commands(layer: &mut SceneLayer, tree_view: &TreeView) {
    push_widget_fill_and_border(
        layer,
        tree_view,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = tree_view.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw header area
    let header_height = 20u32.min(rect.height);
    let header_rect = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: header_height,
    };
    layer.push(RenderCommand::FillRect {
        rect: header_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: header_rect,
        color: Color::rgba(200, 205, 215, 255),
        width: 1,
    });
    // Draw tree icon placeholder
    let icon_size = 12u32.min(header_height);
    if icon_size > 0 {
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x: rect.x + 4,
                y: rect.y + 4,
                width: icon_size,
                height: icon_size,
            },
            color: Color::rgba(100, 100, 100, 255),
            width: 1,
        });
    }
    // Draw placeholder text for tree structure
    layer.push(RenderCommand::DrawText {
        origin: Point {
            x: rect.x + icon_size as i32 + 12,
            y: rect.y + header_height as i32 / 2,
        },
        text: "Tree".to_string(),
        font: tree_view.font().cloned().unwrap_or_default(),
        color: tree_view
            .foreground_color()
            .unwrap_or(Color::rgba(26, 26, 26, 255)),
    });
}
/// Append visual commands for a `TableWidget` data grid representation.
pub fn append_table_widget_visual_commands(layer: &mut SceneLayer, table_widget: &TableWidget) {
    push_widget_fill_and_border(
        layer,
        table_widget,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = table_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw header row
    let header_height = 20u32.min(rect.height / 4).max(16);
    let header_rect = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: header_height,
    };
    layer.push(RenderCommand::FillRect {
        rect: header_rect,
        color: Color::rgba(235, 238, 243, 255),
    });
    layer.push(RenderCommand::DrawRectStroke {
        rect: header_rect,
        color: Color::rgba(200, 205, 215, 255),
        width: 1,
    });
    // Draw column dividers
    let column_count = 3u32;
    let column_width = rect.width / column_count;
    for i in 1..column_count {
        let x = rect.x + (i * column_width) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x, y: rect.y },
            to: Point {
                x,
                y: rect.y + header_height as i32,
            },
            color: Color::rgba(200, 205, 215, 255),
            width: 1,
        });
    }
    // Draw data rows placeholder
    let row_height = 18u32;
    let data_height = rect.height.saturating_sub(header_height);
    let visible_rows = data_height / row_height;
    for row in 0..visible_rows.min(10) {
        let y = rect.y + header_height as i32 + (row * row_height) as i32;
        if y + row_height as i32 > rect.y + rect.height as f32 as i32 {
            break;
        }
        // Row background (alternating)
        if row % 2 == 1 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y,
                    width: rect.width,
                    height: row_height,
                },
                color: Color::rgba(250, 250, 252, 255),
            });
        }
        // Row divider
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x: rect.x, y },
            to: Point {
                x: rect.x + rect.width as f32 as i32,
                y,
            },
            color: Color::rgba(230, 232, 238, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `GridWidget` layout container representation.
pub fn append_grid_widget_visual_commands(layer: &mut SceneLayer, grid_widget: &GridWidget) {
    push_widget_fill_and_border(
        layer,
        grid_widget,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::rgba(180, 185, 195, 255), 1)),
    );
    let rect = grid_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw grid lines
    let rows = 4u32;
    let cols = 4u32;
    let cell_width = rect.width / cols;
    let cell_height = rect.height / rows;
    // Horizontal lines
    for i in 1..rows {
        let y = rect.y + (i * cell_height) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x: rect.x, y },
            to: Point {
                x: rect.x + rect.width as f32 as i32,
                y,
            },
            color: Color::rgba(210, 215, 225, 255),
            width: 1,
        });
    }
    // Vertical lines
    for i in 1..cols {
        let x = rect.x + (i * cell_width) as i32;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point { x, y: rect.y },
            to: Point {
                x,
                y: rect.y + rect.height as f32 as i32,
            },
            color: Color::rgba(210, 215, 225, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `ChartWidget` data visualization representation.
pub fn append_chart_widget_visual_commands(layer: &mut SceneLayer, chart_widget: &ChartWidget) {
    push_widget_fill_and_border(
        layer,
        chart_widget,
        Some(Color::WHITE),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = chart_widget.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let padding = 20i32;
    let chart_rect = Rect {
        x: rect.x + padding,
        y: rect.y + padding,
        width: rect.width.saturating_sub(padding as u32 * 2),
        height: rect.height.saturating_sub(padding as u32 * 2),
    };
    if chart_rect.width == 0 || chart_rect.height == 0 {
        return;
    }
    // Draw chart background
    layer.push(RenderCommand::FillRect {
        rect: chart_rect,
        color: Color::rgba(248, 249, 250, 255),
    });
    // Draw axis lines
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: chart_rect.x,
            y: chart_rect.y + chart_rect.height as i32,
        },
        to: Point {
            x: chart_rect.x + chart_rect.width as i32,
            y: chart_rect.y + chart_rect.height as i32,
        },
        color: Color::rgba(100, 100, 100, 255),
        width: 2,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: chart_rect.x,
            y: chart_rect.y,
        },
        to: Point {
            x: chart_rect.x,
            y: chart_rect.y + chart_rect.height as i32,
        },
        color: Color::rgba(100, 100, 100, 255),
        width: 2,
    });
    // Draw sample bar chart bars
    let bar_count = 5u32;
    let bar_width = chart_rect.width / (bar_count * 2);
    let max_bar_height = chart_rect.height.saturating_sub(10);
    for i in 0..bar_count {
        let bar_height = max_bar_height * (i + 1) / bar_count;
        let x = chart_rect.x + (i * bar_width * 2) as i32 + bar_width as i32 / 2;
        let y = chart_rect.y + chart_rect.height as i32 - bar_height as i32;
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x,
                y,
                width: bar_width,
                height: bar_height,
            },
            color: Color::rgba(66, 133, 244, 200),
        });
        layer.push(RenderCommand::DrawRectStroke {
            rect: Rect {
                x,
                y,
                width: bar_width,
                height: bar_height,
            },
            color: Color::rgba(66, 133, 244, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `DockPanel` docking container representation.
pub fn append_dock_panel_visual_commands(layer: &mut SceneLayer, dock_panel: &DockPanel) {
    push_widget_fill_and_border(
        layer,
        dock_panel,
        Some(Color::BACKGROUND),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = dock_panel.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw dock area dividers
    let center_x = rect.x + rect.width as f32 as i32 / 2;
    let center_y = rect.y + rect.height as f32 as i32 / 2;
    // Vertical center divider
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: center_x,
            y: rect.y + 4,
        },
        to: Point {
            x: center_x,
            y: rect.y + rect.height as f32 as i32 - 4,
        },
        color: Color::rgba(200, 205, 215, 255),
        width: 2,
    });
    // Horizontal center divider
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: rect.x + 4,
            y: center_y,
        },
        to: Point {
            x: rect.x + rect.width as f32 as i32 - 4,
            y: center_y,
        },
        color: Color::rgba(200, 205, 215, 255),
        width: 2,
    });
}
/// Append visual commands for a `GroupBox` titled container representation.
pub fn append_group_box_visual_commands(layer: &mut SceneLayer, group_box: &GroupBox) {
    let rect = group_box.geometry();
    // Draw the main border with title area
    let title_height = 16i32;
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x,
            y: rect.y + title_height / 2,
            width: rect.width,
            height: rect.height.saturating_sub(title_height as u32 / 2),
        },
        color: Color::rgba(140, 145, 155, 255),
        width: 1,
    });
    // Fill title background
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: rect.x + 8,
            y: rect.y,
            width: 60,
            height: title_height as u32,
        },
        color: Color::BACKGROUND,
    });
    // Draw title text
    layer.push(RenderCommand::DrawText {
        origin: Point {
            x: rect.x + 12,
            y: rect.y + title_height / 2,
        },
        text: "Group".to_string(),
        font: group_box.font().cloned().unwrap_or_default(),
        color: group_box
            .foreground_color()
            .unwrap_or(Color::rgba(50, 52, 56, 255)),
    });
}
/// Append visual commands for a `Splitter` resizable divider representation.
pub fn append_splitter_visual_commands(layer: &mut SceneLayer, splitter: &Splitter) {
    push_widget_fill_and_border(
        layer,
        splitter,
        Some(Color::rgba(235, 238, 243, 255)),
        Some((Color::rgba(180, 185, 195, 255), 1)),
    );
    let rect = splitter.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw gripper dots/lines
    let is_horizontal = rect.width > rect.height;
    if is_horizontal {
        // Horizontal splitter - vertical gripper line
        let center_x = rect.x + rect.width as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: center_x,
                y: rect.y + 4,
            },
            to: Point {
                x: center_x,
                y: rect.y + rect.height as f32 as i32 - 4,
            },
            color: Color::rgba(160, 165, 175, 255),
            width: 2,
        });
    } else {
        // Vertical splitter - horizontal gripper line
        let center_y = rect.y + rect.height as f32 as i32 / 2;
        layer.push(RenderCommand::DrawLineStroke {
            from: Point {
                x: rect.x + 4,
                y: center_y,
            },
            to: Point {
                x: rect.x + rect.width as f32 as i32 - 4,
                y: center_y,
            },
            color: Color::rgba(160, 165, 175, 255),
            width: 2,
        });
    }
}
/// Append visual commands for an `MdiArea` multiple document interface representation.
pub fn append_mdi_area_visual_commands(layer: &mut SceneLayer, mdi_area: &MdiArea) {
    push_widget_fill_and_border(
        layer,
        mdi_area,
        Some(Color::rgba(220, 225, 232, 255)),
        Some((Color::rgba(140, 148, 160, 255), 1)),
    );
    let rect = mdi_area.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw placeholder child window frames
    let child_rect = Rect {
        x: rect.x + 10,
        y: rect.y + 10,
        width: (rect.width / 2).saturating_sub(15),
        height: (rect.height / 2).saturating_sub(15),
    };
    if child_rect.width > 0 && child_rect.height > 0 {
        // Child window background
        layer.push(RenderCommand::FillRect {
            rect: child_rect,
            color: Color::WHITE,
        });
        // Child window title bar
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: child_rect.x,
                y: child_rect.y,
                width: child_rect.width,
                height: 20u32.min(child_rect.height),
            },
            color: Color::rgba(66, 133, 244, 255),
        });
        // Child window border
        layer.push(RenderCommand::DrawRectStroke {
            rect: child_rect,
            color: Color::rgba(120, 128, 140, 255),
            width: 1,
        });
    }
}
/// Append visual commands for a `Canvas` drawing surface representation.
pub fn append_canvas_visual_commands(layer: &mut SceneLayer, canvas: &Canvas) {
    push_widget_fill_and_border(
        layer,
        canvas,
        Some(Color::WHITE),
        Some((Color::rgba(100, 108, 120, 255), 1)),
    );
    let rect = canvas.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw canvas grid pattern
    let grid_size = 20u32;
    let cols = rect.width / grid_size;
    let rows = rect.height / grid_size;
    // Light grid dots
    for row in 0..rows {
        for col in 0..cols {
            let x = rect.x + (col * grid_size) as i32 + grid_size as i32 / 2;
            let y = rect.y + (row * grid_size) as i32 + grid_size as i32 / 2;
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x,
                    y,
                    width: 1,
                    height: 1,
                },
                color: Color::rgba(220, 225, 235, 255),
            });
        }
    }
}
/// Append visual commands for a `SpinBox` numeric input control.
pub fn append_spin_box_visual_commands(layer: &mut SceneLayer, spin_box: &crate::widget::SpinBox) {
    push_widget_fill_and_border(
        layer,
        spin_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = spin_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Up/down button width
    let button_width = (rect.width / 5).clamp(16, 24);
    let value_area_width = rect.width.saturating_sub(button_width);
    // Draw value text
    let value_text = spin_box.value().to_string();
    let text_color = spin_box
        .foreground_color()
        .unwrap_or(Color::rgba(40, 44, 52, 255));
    let padding = 4i32;
    layer.push(RenderCommand::DrawText {
        text: value_text,
        origin: Point {
            x: rect.x + padding,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        font: spin_box.font().cloned().unwrap_or_default(),
        color: text_color,
    });
    // Draw up button (top half of right side)
    let button_x = rect.x + value_area_width as i32;
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: button_x,
            y: rect.y,
            width: button_width,
            height: rect.height / 2,
        },
        color: Color::rgba(240, 242, 245, 255),
    });
    // Draw up arrow
    let arrow_center_y = rect.y + rect.height as f32 as i32 / 4;
    let arrow_color = Color::rgba(80, 84, 92, 255);
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2 - 3,
            y: arrow_center_y + 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y - 2,
        },
        color: arrow_color,
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y - 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2 + 3,
            y: arrow_center_y + 2,
        },
        color: arrow_color,
        width: 1,
    });
    // Draw down button (bottom half of right side)
    layer.push(RenderCommand::FillRect {
        rect: Rect {
            x: button_x,
            y: rect.y + rect.height as f32 as i32 / 2,
            width: button_width,
            height: rect.height / 2,
        },
        color: Color::rgba(240, 242, 245, 255),
    });
    // Draw down arrow
    let arrow_center_y2 = rect.y + rect.height as f32 as i32 * 3 / 4;
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2 - 3,
            y: arrow_center_y2 - 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y2 + 2,
        },
        color: arrow_color,
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x + button_width as i32 / 2,
            y: arrow_center_y2 + 2,
        },
        to: Point {
            x: button_x + button_width as i32 / 2 + 3,
            y: arrow_center_y2 - 2,
        },
        color: arrow_color,
        width: 1,
    });
    // Button separator lines
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x,
            y: rect.y,
        },
        to: Point {
            x: button_x,
            y: rect.y + rect.height as f32 as i32,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
    layer.push(RenderCommand::DrawLineStroke {
        from: Point {
            x: button_x,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        to: Point {
            x: button_x + button_width as i32,
            y: rect.y + rect.height as f32 as i32 / 2,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
}
/// Append visual commands for a `ListView` widget representation.
pub fn append_list_view_visual_commands(
    layer: &mut SceneLayer,
    list_view: &crate::widget::ListView,
) {
    push_widget_fill_and_border(
        layer,
        list_view,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = list_view.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let padding = 8i32;
    let text_color = Color::rgba(40, 44, 52, 255);
    let selected_bg = Color::PRIMARY;
    let selected_text = Color::WHITE;
    let font = Font::default_ui();
    let visible_rows = (rect.height / row_height) as usize;
    let row_count = list_view.row_count().min(visible_rows);
    for row in 0..row_count {
        let row_y = rect.y + (row as u32 * row_height) as i32;
        let is_selected = list_view.selected_row() == Some(row);
        let is_focused = list_view.focused_row() == Some(row);
        // Draw selection background
        if is_selected {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y: row_y,
                    width: rect.width,
                    height: row_height,
                },
                color: selected_bg,
            });
        }
        // Draw focus indicator
        if is_focused && !is_selected {
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 1,
                    y: row_y + 1,
                    width: rect.width.saturating_sub(2),
                    height: row_height.saturating_sub(2),
                },
                color: Color::PRIMARY,
                width: 1,
            });
        }
        // Draw item text
        if let Some(text) = list_view.item(row) {
            layer.push(RenderCommand::DrawText {
                text,
                origin: Point {
                    x: rect.x + padding,
                    y: row_y + row_height as i32 / 2,
                },
                font: font.clone(),
                color: if is_selected {
                    selected_text
                } else {
                    text_color
                },
            });
        }
    }
}
/// Append visual commands for a `ScrollArea` scrollable container.
pub fn append_scroll_area_visual_commands(
    layer: &mut SceneLayer,
    scroll_area: &crate::widget::ScrollArea,
) {
    push_widget_fill_and_border(
        layer,
        scroll_area,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::rgba(180, 188, 200, 255), 1)),
    );
    let rect = scroll_area.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let viewport = scroll_area.viewport();
    let scroll_offset = viewport.position();
    let viewport_size = viewport.size();
    // ScrollArea does not currently expose content geometry; use viewport as a safe baseline.
    let content_size = viewport.size();
    // Calculate scrollbar visibility and sizes
    let needs_h_scroll = content_size.width > viewport_size.width;
    let needs_v_scroll = content_size.height > viewport_size.height;
    let scrollbar_size = 12u32;
    // Horizontal scrollbar
    if needs_h_scroll {
        let h_track_y = rect.y + rect.height as f32 as i32 - scrollbar_size as i32;
        let h_track_width = if needs_v_scroll {
            rect.width.saturating_sub(scrollbar_size)
        } else {
            rect.width
        };
        // Track
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x,
                y: h_track_y,
                width: h_track_width,
                height: scrollbar_size,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
        // Thumb
        let h_ratio = viewport_size.width as f32 / content_size.width as f32;
        let h_thumb_width = (h_track_width as f32 * h_ratio).max(20.0) as u32;
        let h_max_offset = content_size.width.saturating_sub(viewport_size.width) as i32;
        let h_thumb_offset = if h_max_offset > 0 {
            (scroll_offset.x as f32 / h_max_offset as f32
                * (h_track_width.saturating_sub(h_thumb_width)) as f32) as i32
        } else {
            0
        };
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + h_thumb_offset,
                y: h_track_y + 2,
                width: h_thumb_width,
                height: scrollbar_size.saturating_sub(4),
            },
            color: Color::rgba(172, 178, 188, 255),
        });
    }
    // Vertical scrollbar
    if needs_v_scroll {
        let v_track_x = rect.x + rect.width as f32 as i32 - scrollbar_size as i32;
        let v_track_height = if needs_h_scroll {
            rect.height.saturating_sub(scrollbar_size)
        } else {
            rect.height
        };
        // Track
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: v_track_x,
                y: rect.y,
                width: scrollbar_size,
                height: v_track_height,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
        // Thumb
        let v_ratio = viewport_size.height as f32 / content_size.height as f32;
        let v_thumb_height = (v_track_height as f32 * v_ratio).max(20.0) as u32;
        let v_max_offset = content_size.height.saturating_sub(viewport_size.height) as i32;
        let v_thumb_offset = if v_max_offset > 0 {
            (scroll_offset.y as f32 / v_max_offset as f32
                * (v_track_height.saturating_sub(v_thumb_height)) as f32) as i32
        } else {
            0
        };
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: v_track_x + 2,
                y: rect.y + v_thumb_offset,
                width: scrollbar_size.saturating_sub(4),
                height: v_thumb_height,
            },
            color: Color::rgba(172, 178, 188, 255),
        });
    }
    // Corner square (when both scrollbars are visible)
    if needs_h_scroll && needs_v_scroll {
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: rect.x + rect.width as f32 as i32 - scrollbar_size as i32,
                y: rect.y + rect.height as f32 as i32 - scrollbar_size as i32,
                width: scrollbar_size,
                height: scrollbar_size,
            },
            color: Color::rgba(232, 234, 238, 255),
        });
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
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
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
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
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
                    blend_pixel(
                        frame,
                        size.width,
                        px as u32,
                        py as u32,
                        color,
                        stroke_coverage,
                    );
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
#[allow(clippy::too_many_arguments)]
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
        0b11111111, 0b10000001, 0b10111101, 0b10100101, 0b10111101, 0b10000001, 0b11111111,
        0b00000000,
    ]
}
fn pixel_bytes_len(size: Size) -> usize {
    size.width.saturating_mul(size.height).saturating_mul(4) as usize
}
pub fn fill_pixels(pixels: &mut [u8], color: Color) {
    let chunk_size = 4;
    let color_arr = [color.r, color.g, color.b, color.a];
    for chunk in pixels.chunks_mut(chunk_size) {
        if chunk.len() == chunk_size {
            chunk.copy_from_slice(&color_arr);
        } else {
            chunk.copy_from_slice(&color_arr[..chunk.len()]);
        }
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
pub fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a <= 0.0 {
        frame[idx] = 0;
        frame[idx + 1] = 0;
        frame[idx + 2] = 0;
        frame[idx + 3] = 0;
        return;
    }
    let dst = &mut frame[idx..idx + 4];
    let src = [color.r, color.g, color.b, color.a];
    let src_f: [f32; 4] = [
        src[0] as f32 / 255.0,
        src[1] as f32 / 255.0,
        src[2] as f32 / 255.0,
        src[3] as f32 / 255.0,
    ];
    let dst_f: [f32; 4] = [
        dst[0] as f32 / 255.0,
        dst[1] as f32 / 255.0,
        dst[2] as f32 / 255.0,
        dst[3] as f32 / 255.0,
    ];
    let out_a = src_a + dst_f[3] * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let out_r = (src_f[0] * src_a + dst_f[0] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_g = (src_f[1] * src_a + dst_f[1] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_b = (src_f[2] * src_a + dst_f[2] * dst_f[3] * (1.0 - src_a)) / out_a;
    dst[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
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
    stroke_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    // radius is the outer radius, stroke_width is the width of the ring
    let outer_radius = radius;
    let inner_radius = (radius - stroke_width).max(0.0);
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            // Ring coverage: outside inner radius and inside outer radius
            let inner_coverage = circle_fill_coverage(distance, inner_radius);
            let outer_coverage = circle_fill_coverage(distance, outer_radius);
            // Ring is outer circle minus inner circle
            coverage_sum += (outer_coverage - inner_coverage).max(0.0);
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
#[allow(clippy::too_many_arguments)]
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
    cluster
        .text
        .chars()
        .last()
        .map(|ch| ch == '\u{200D}')
        .unwrap_or(false)
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
    Rect {
        x,
        y,
        width,
        height,
    }
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
            weight: Font::REGULAR_WEIGHT,
            bold: false,
            italic: false,
        }
    }
    #[test]
    fn text_metrics_scale_with_dpi() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 100,
                height: 40,
            },
            1.0,
        );
        let m1 = surface.measure_text("hello", &font());
        surface.set_dpi_scale(2.0);
        let m2 = surface.measure_text("hello", &font());
        assert!(m2.width > m1.width);
        assert!(m2.height > m1.height);
    }
    #[test]
    fn double_buffer_present_swaps_frame() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 4,
                height: 4,
            },
            1.0,
        );
        surface.begin_frame(Color::RED);
        surface.end_frame();
        assert_eq!(&surface.frame_rgba()[0..4], &[255, 0, 0, 255]);
        surface.begin_frame(Color::BLUE);
        surface.end_frame();
        assert_eq!(&surface.frame_rgba()[0..4], &[0, 0, 255, 255]);
    }
    #[test]
    fn fill_rect_writes_pixels() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
        surface.begin_frame(Color::BLACK);
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
        let surface = SoftwareSurface::new(
            Size {
                width: 100,
                height: 40,
            },
            1.0,
        );
        let shaped = surface.shape_text("e\u{0301}", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }
    #[test]
    fn shaping_merges_zwj_sequence_into_one_cluster() {
        let surface = SoftwareSurface::new(
            Size {
                width: 100,
                height: 40,
            },
            1.0,
        );
        let shaped = surface.shape_text("👨\u{200D}👩\u{200D}👧\u{200D}👦", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }
    #[test]
    fn scene_composition_respects_layer_order() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
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
        scene.compose_to(&mut surface, Color::BLACK);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::BLACK);
        let idx = 36;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[7, 8, 9, 255]);
    }
    #[test]
    fn software_backend_delegates_text_shaping() {
        let backend = SoftwarePaintBackend::new(
            Size {
                width: 100,
                height: 40,
            },
            1.0,
        );
        let shaped = backend.shape_text("e\u{0301}", &font());
        assert_eq!(shaped.cluster_count(), 1);
    }
    #[test]
    fn draw_text_rasterizes_glyph_instead_of_full_rect_fill() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 80,
                height: 30,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
        surface.draw_text(Point { x: 4, y: 4 }, "A", &font(), Color::WHITE);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        surface.begin_frame(Color::BLACK);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::BLACK);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
        surface.draw_circle_with_width(Point { x: 8, y: 8 }, 4, Color::rgba(170, 171, 172, 255), 2);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(190, 191, 192, 255));
        surface.end_frame();
        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[center_idx..center_idx + 4],
            &[190, 191, 192, 255]
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        surface.begin_frame(Color::BLACK);
        surface.draw_line_with_width(
            Point { x: 2, y: 6 },
            Point { x: 9, y: 6 },
            Color::rgba(21, 22, 23, 255),
            3,
        );
        surface.end_frame();
        let center_idx = ((6 * 12 + 5) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[center_idx..center_idx + 4],
            &[21, 22, 23, 255]
        );
        let upper_idx = ((5 * 12 + 5) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[upper_idx..upper_idx + 4],
            &[21, 22, 23, 255]
        );
        let lower_idx = ((7 * 12 + 5) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[lower_idx..lower_idx + 4],
            &[21, 22, 23, 255]
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::BLACK);
        let idx = ((5 * 12 + 5) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[31, 32, 33, 255]);
    }
    #[test]
    fn draw_rect_with_width_marks_neighbor_border_pixels() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        surface.begin_frame(Color::BLACK);
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
        assert_eq!(
            &surface.frame_rgba()[border_idx..border_idx + 4],
            &[41, 42, 43, 255]
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::BLACK);
        let idx = ((5 * 14 + 6) * 4) as usize;
        assert_eq!(&backend.frame_rgba()[idx..idx + 4], &[51, 52, 53, 255]);
    }
    #[test]
    fn fill_rounded_rect_writes_center_and_preserves_corner() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        surface.begin_frame(Color::BLACK);
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
        assert_eq!(
            &surface.frame_rgba()[center_idx..center_idx + 4],
            &[61, 62, 63, 255]
        );
        let corner_idx = ((3 * 14 + 3) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[corner_idx..corner_idx + 4],
            &[0, 0, 0, 255]
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::BLACK);
        let stroke_idx = ((3 * 14 + 7) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[stroke_idx..stroke_idx + 4],
            &[81, 82, 83, 255]
        );
        let fill_idx = ((7 * 14 + 7) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[fill_idx..fill_idx + 4],
            &[71, 72, 73, 255]
        );
    }
    #[test]
    fn draw_rounded_rect_aa_with_width_produces_soft_edge() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
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
        assert_eq!(
            &surface.frame_rgba()[core_idx..core_idx + 4],
            &[230, 231, 232, 255]
        );
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }
    #[test]
    fn fill_rounded_rect_aa_produces_soft_corner_edge() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
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
        assert_eq!(
            &surface.frame_rgba()[center_idx..center_idx + 4],
            &[250, 210, 170, 255]
        );
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }
    #[test]
    fn aa_sample_level_changes_rounded_rect_edge_coverage() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.set_aa_samples_per_axis(1);
        assert_eq!(surface.aa_samples_per_axis(), 1);
        surface.begin_frame(Color::TRANSPARENT);
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
        surface.begin_frame(Color::TRANSPARENT);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        backend.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });
        scene.compose_with_backend_config(
            &mut backend,
            Color::TRANSPARENT,
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
        let mut backend_default = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        backend_default.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });
        scene.compose_with_backend(&mut backend_default, Color::TRANSPARENT);
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let alpha_default = backend_default.frame_rgba()[edge_idx + 3];
        let mut backend_temp = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        backend_temp.apply_render_config(SoftwareRenderConfig {
            aa_samples_per_axis: 4,
        });
        scene.compose_with_backend_config(
            &mut backend_temp,
            Color::TRANSPARENT,
            Some(SoftwareRenderConfig {
                aa_samples_per_axis: 1,
            }),
        );
        let alpha_temp = backend_temp.frame_rgba()[edge_idx + 3];
        assert_ne!(alpha_default, alpha_temp);
    }
    #[test]
    fn aa_sample_level_changes_circle_edge_coverage() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.set_aa_samples_per_axis(1);
        surface.begin_frame(Color::TRANSPARENT);
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
        surface.end_frame();
        let edge_idx = ((8 * 16 + 12) * 4) as usize;
        let alpha_low = surface.frame_rgba()[edge_idx + 3];
        surface.set_aa_samples_per_axis(4);
        surface.begin_frame(Color::TRANSPARENT);
        surface.fill_circle_aa(Point { x: 8, y: 8 }, 4, Color::rgba(120, 121, 122, 255));
        surface.end_frame();
        let alpha_high = surface.frame_rgba()[edge_idx + 3];
        assert_ne!(alpha_low, alpha_high);
    }
    #[test]
    fn aa_sample_level_changes_line_edge_coverage() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.set_aa_samples_per_axis(1);
        surface.begin_frame(Color::TRANSPARENT);
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
        surface.begin_frame(Color::TRANSPARENT);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
        surface.draw_circle(Point { x: 7, y: 7 }, 3, Color::rgba(100, 120, 140, 255));
        surface.end_frame();
        let edge_idx = ((8 * 14 + 10) * 4) as usize;
        let edge_alpha = surface.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }
    #[test]
    fn rounded_rect_fill_applies_partial_alpha_at_corner_edge() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 14,
                height: 14,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
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
        let mut surface = SoftwareSurface::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 12,
                height: 12,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
        let idx = ((3 * 12 + 4) * 4) as usize;
        let px = &backend.frame_rgba()[idx..idx + 4];
        assert_eq!(px[0], 140);
        assert_eq!(px[1], 150);
        assert_eq!(px[2], 160);
        assert!(px[3] > 0 && px[3] < 255);
    }
    #[test]
    fn draw_line_aa_with_width_expands_band_and_keeps_soft_edge() {
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        surface.begin_frame(Color::TRANSPARENT);
        surface.draw_line_aa_with_width(
            Point { x: 2, y: 8 },
            Point { x: 13, y: 8 },
            Color::rgba(210, 211, 212, 255),
            3,
        );
        surface.end_frame();
        let core_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &surface.frame_rgba()[core_idx..core_idx + 4],
            &[210, 211, 212, 255]
        );
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
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
        let mut backend = SoftwarePaintBackend::new(
            Size {
                width: 16,
                height: 16,
            },
            1.0,
        );
        scene.compose_with_backend(&mut backend, Color::TRANSPARENT);
        let center_idx = ((8 * 16 + 8) * 4) as usize;
        assert_eq!(
            &backend.frame_rgba()[center_idx..center_idx + 4],
            &[120, 130, 140, 255]
        );
        let edge_idx = ((4 * 16 + 3) * 4) as usize;
        let edge_alpha = backend.frame_rgba()[edge_idx + 3];
        assert!(edge_alpha > 0 && edge_alpha < 255);
    }
    #[test]
    fn auto_compose_handles_draw_text_scene_with_gpu_or_cpu_backend() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::DrawText {
            origin: Point { x: 1, y: 10 },
            text: "fallback".to_string(),
            font: Font::default(),
            color: Color::rgba(250, 120, 40, 255),
        });
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 48,
                height: 24,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
    }
    #[test]
    fn auto_compose_produces_expected_pixels_for_simple_rect_scene() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: 2,
                y: 2,
                width: 6,
                height: 4,
            },
            color: Color::rgba(11, 22, 33, 255),
        });
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 16,
                height: 12,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
        let idx = ((3 * 16 + 3) * 4) as usize;
        assert_eq!(&surface.frame_rgba()[idx..idx + 4], &[11, 22, 33, 255]);
    }
    #[test]
    fn auto_compose_updates_last_backend_diagnostics() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: 1,
                y: 1,
                width: 3,
                height: 3,
            },
            color: Color::rgba(1, 2, 3, 255),
        });
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 8,
                height: 8,
            },
            1.0,
        );
        let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
        assert_eq!(selected, last_auto_render_backend());
    }
    #[test]
    fn auto_compose_falls_back_to_cpu_backend_when_gpu_path_is_rejected() {
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        layer.push(RenderCommand::FillRect {
            rect: Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            color: Color::rgba(9, 8, 7, 255),
        });
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 0,
                height: 0,
            },
            1.0,
        );
        let selected = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
        assert_eq!(selected, AutoRenderBackend::CpuSoftware);
        assert_eq!(last_auto_render_backend(), AutoRenderBackend::CpuSoftware);
    }
    #[test]
    fn base_control_visual_builders_emit_expected_command_types() {
        use crate::widget::{
            Button, CheckBox, CheckState, Label, LineEdit, Panel, RadioButton, Widget,
        };
        let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 120, 80));
        window.set_background_color(Some(Color::rgba(10, 20, 30, 255)));
        let mut panel = Panel::new(Rect::new(4, 20, 112, 52));
        panel.set_background_color(Some(Color::rgba(40, 50, 60, 255)));
        let mut button = Button::new("OK".to_string(), Rect::new(10, 26, 50, 20));
        button.set_pressed(true);
        let mut checkbox = CheckBox::new(Rect::new(70, 28, 20, 20));
        checkbox.set_state(CheckState::Checked);
        let mut radio = RadioButton::new(Rect::new(70, 52, 20, 20));
        radio.set_checked(true);
        let mut label = Label::new("Label".to_string(), Rect::new(10, 50, 60, 16));
        label.set_background_color(Some(Color::rgba(80, 90, 100, 255)));
        let mut line_edit = LineEdit::new(Rect::new(10, 54, 52, 16));
        line_edit.set_text("abc".to_string());
        let mut layer = SceneLayer::new(0);
        append_window_visual_commands(&mut layer, &window);
        append_panel_visual_commands(&mut layer, &panel);
        append_button_visual_commands(&mut layer, &button);
        append_checkbox_visual_commands(&mut layer, &checkbox);
        append_radiobutton_visual_commands(&mut layer, &radio);
        append_label_visual_commands(&mut layer, &label);
        append_line_edit_visual_commands(&mut layer, &line_edit);
        let mut saw_fill_rect = false;
        let mut saw_draw_text = false;
        let mut saw_fill_circle = false;
        for command in layer.commands() {
            match command {
                RenderCommand::FillRect { .. } => saw_fill_rect = true,
                RenderCommand::DrawText { .. } => saw_draw_text = true,
                RenderCommand::FillCircle { .. } => saw_fill_circle = true,
                _ => {}
            }
        }
        assert!(saw_fill_rect);
        assert!(saw_draw_text);
        assert!(saw_fill_circle);
    }
    #[test]
    fn auto_compose_renders_base_control_scene_with_gpu_or_cpu_backend() {
        use crate::widget::{
            Button, CheckBox, CheckState, Label, LineEdit, Panel, RadioButton, Widget,
        };
        let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 120, 80));
        window.set_background_color(Some(Color::rgba(10, 20, 30, 255)));
        let mut panel = Panel::new(Rect::new(4, 20, 112, 52));
        panel.set_background_color(Some(Color::rgba(40, 50, 60, 255)));
        let mut button = Button::new("Apply".to_string(), Rect::new(10, 26, 50, 20));
        button.set_pressed(true);
        let mut checkbox = CheckBox::new(Rect::new(70, 28, 20, 20));
        checkbox.set_state(CheckState::Checked);
        let mut radio = RadioButton::new(Rect::new(70, 52, 20, 20));
        radio.set_checked(true);
        let mut label = Label::new("Label".to_string(), Rect::new(10, 50, 60, 16));
        label.set_background_color(Some(Color::rgba(80, 90, 100, 255)));
        let mut line_edit = LineEdit::new(Rect::new(10, 54, 52, 16));
        line_edit.set_text("abc".to_string());
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        append_window_visual_commands(&mut layer, &window);
        append_panel_visual_commands(&mut layer, &panel);
        append_button_visual_commands(&mut layer, &button);
        append_checkbox_visual_commands(&mut layer, &checkbox);
        append_radiobutton_visual_commands(&mut layer, &radio);
        append_label_visual_commands(&mut layer, &label);
        append_line_edit_visual_commands(&mut layer, &line_edit);
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 128,
                height: 88,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
        let sample = |x: u32, y: u32| -> [u8; 4] {
            let idx = ((y * surface.size().width + x) * 4) as usize;
            [
                surface.frame_rgba()[idx],
                surface.frame_rgba()[idx + 1],
                surface.frame_rgba()[idx + 2],
                surface.frame_rgba()[idx + 3],
            ]
        };
        assert_eq!(sample(2, 2), [10, 20, 30, 255]);
        assert_eq!(sample(6, 22), [40, 50, 60, 255]);
        let button_px = sample(12, 28);
        assert!(button_px[3] > 0);
        let checkbox_px = sample(76, 34);
        assert!(checkbox_px[3] > 0);
        let radio_px = sample(80, 62);
        assert!(radio_px[3] > 0);
        assert_eq!(sample(12, 52), [80, 90, 100, 255]);
        let line_edit_px = sample(12, 56);
        assert!(line_edit_px[3] > 0);
    }
    #[test]
    fn data_range_control_visual_builders_emit_selection_and_value_commands() {
        use crate::widget::{ComboBox, ListBox, ProgressBar, ScrollBar, Slider};
        let mut combo = ComboBox::new(Rect::new(4, 4, 120, 20));
        combo.add_item("Alpha".to_string());
        combo.add_item("Beta".to_string());
        combo.set_current_index(Some(1));
        // combo.open_dropdown(); // Method not found
        let mut list = ListBox::new(Rect::new(4, 28, 120, 64));
        list.add_item("Row-1".to_string());
        list.add_item("Row-2".to_string());
        let mut progress = ProgressBar::new(Rect::new(140, 8, 100, 14));
        progress.set_range(0, 100);
        progress.set_value(60);
        let mut slider = Slider::new(Rect::new(140, 30, 100, 20));
        slider.set_range(0, 100);
        slider.set_value(30);
        let mut scroll = ScrollBar::new(Rect::new(140, 58, 100, 16));
        scroll.set_range(0, 100);
        scroll.set_page_step(20);
        scroll.set_value(40);
        let mut layer = SceneLayer::new(0);
        append_combo_box_visual_commands(&mut layer, &combo);
        append_list_box_visual_commands(&mut layer, &list);
        append_progress_bar_visual_commands(&mut layer, &progress);
        append_slider_visual_commands(&mut layer, &slider);
        append_scroll_bar_visual_commands(&mut layer, &scroll);
        let mut draw_text_count = 0usize;
        let mut fill_circle_count = 0usize;
        let mut fill_rect_count = 0usize;
        for command in layer.commands() {
            match command {
                RenderCommand::DrawText { .. } => draw_text_count += 1,
                RenderCommand::FillCircle { .. } => fill_circle_count += 1,
                RenderCommand::FillRect { .. } => fill_rect_count += 1,
                _ => {}
            }
        }
        assert!(draw_text_count >= 3);
        assert!(fill_circle_count >= 1);
        assert!(fill_rect_count >= 5);
    }
    #[test]
    fn auto_compose_renders_data_range_scene_with_gpu_or_cpu_backend() {
        use crate::widget::{ComboBox, ListBox, ProgressBar, ScrollBar, Slider};
        let mut combo = ComboBox::new(Rect::new(4, 4, 120, 20));
        combo.add_item("Alpha");
        combo.add_item("Beta");
        combo.set_current_index(1);
        combo.open_dropdown();
        let mut list = ListBox::new(Rect::new(4, 28, 120, 64));
        list.add_item("Row-1");
        list.add_item("Row-2");
        let mut progress = ProgressBar::new(Rect::new(140, 8, 100, 14));
        progress.set_range(0, 100);
        progress.set_value(60);
        let mut slider = Slider::new(Rect::new(140, 30, 100, 20));
        slider.set_range(0, 100);
        slider.set_value(30);
        let mut scroll = ScrollBar::new(Rect::new(140, 58, 100, 16));
        scroll.set_range(0, 100);
        scroll.set_page_step(20);
        scroll.set_value(40);
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        append_combo_box_visual_commands(&mut layer, &combo);
        append_list_box_visual_commands(&mut layer, &list);
        append_progress_bar_visual_commands(&mut layer, &progress);
        append_slider_visual_commands(&mut layer, &slider);
        append_scroll_bar_visual_commands(&mut layer, &scroll);
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 256,
                height: 128,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
        let sample = |x: u32, y: u32| -> [u8; 4] {
            let idx = ((y * surface.size().width + x) * 4) as usize;
            [
                surface.frame_rgba()[idx],
                surface.frame_rgba()[idx + 1],
                surface.frame_rgba()[idx + 2],
                surface.frame_rgba()[idx + 3],
            ]
        };
        let combo_px = sample(8, 8);
        assert!(combo_px[3] > 0);
        let list_px = sample(8, 34);
        assert!(list_px[3] > 0);
        let progress_px = sample(180, 12);
        assert_eq!(progress_px[3], 255);
        let slider_thumb_px = sample(170, 40);
        assert!(slider_thumb_px[3] > 0);
        let scroll_thumb_px = sample(178, 62);
        assert!(scroll_thumb_px[3] > 0);
    }
    #[test]
    fn host_navigation_visual_builders_emit_expected_commands() {
        use crate::widget::{Menu, MenuBar, StatusBar, TabWidget, ToolBar};
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 260, 24));
        menu_bar.add_menu("File".to_string());
        menu_bar.add_menu("Edit".to_string());
        // menu_bar.set_current_menu(1002); // Method not found
        let mut menu = Menu::new(Rect::new(0, 24, 160, 100), "File".to_string());
        // menu.set_title("File".to_string()); // Method not found
        menu.add_action("Open".to_string());
        menu.add_action("Save".to_string());
        let mut tool_bar = ToolBar::new(Rect::new(0, 128, 260, 28));
        tool_bar.add_action("cut".to_string(), "Cut".to_string());
        tool_bar.add_action("copy".to_string(), "Copy".to_string());
        tool_bar.add_action("paste".to_string(), "Paste".to_string());
        let mut status_bar = StatusBar::new(Rect::new(0, 160, 260, 22));
        // status_bar.set_message("Ready".to_string()); // Method not found
        let mut tabs = TabWidget::new(Rect::new(170, 24, 90, 70));
        tabs.add_tab("Tab 1".to_string(), None);
        tabs.add_tab("Tab 2".to_string(), None);
        tabs.set_current_index(Some(1));
        let mut layer = SceneLayer::new(0);
        append_menu_bar_visual_commands(&mut layer, &menu_bar);
        append_menu_visual_commands(&mut layer, &menu);
        append_tool_bar_visual_commands(&mut layer, &tool_bar);
        append_status_bar_visual_commands(&mut layer, &status_bar);
        append_tab_widget_visual_commands(&mut layer, &tabs);
        let mut draw_text_count = 0usize;
        let mut fill_rect_count = 0usize;
        let mut rounded_rect_count = 0usize;
        for command in layer.commands() {
            match command {
                RenderCommand::DrawText { .. } => draw_text_count += 1,
                RenderCommand::FillRect { .. } => fill_rect_count += 1,
                RenderCommand::FillRoundedRect { .. } => rounded_rect_count += 1,
                _ => {}
            }
        }
        assert!(draw_text_count >= 6);
        assert!(fill_rect_count >= 5);
        assert!(rounded_rect_count >= 1);
    }
    #[test]
    fn auto_compose_renders_host_navigation_scene_with_gpu_or_cpu_backend() {
        use crate::widget::{Menu, MenuBar, StatusBar, TabWidget, ToolBar};
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 260, 24));
        menu_bar.add_menu("File".to_string());
        menu_bar.add_menu("Edit".to_string());
        // menu_bar.set_current_menu(1002); // Method not found
        let mut menu = Menu::new(Rect::new(0, 24, 160, 100), "File".to_string());
        // menu.set_title("File".to_string()); // Method not found
        menu.add_action("Open".to_string());
        menu.add_action("Save".to_string());
        let mut tool_bar = ToolBar::new(Rect::new(0, 128, 260, 28));
        tool_bar.add_action("cut".to_string(), "Cut".to_string());
        tool_bar.add_action("copy".to_string(), "Copy".to_string());
        let mut status_bar = StatusBar::new(Rect::new(0, 160, 260, 20));
        // status_bar.set_message("Ready".to_string()); // Method not found
        let mut tabs = TabWidget::new(Rect::new(140, 120, 120, 32));
        tabs.add_tab("Tab 1".to_string(), None);
        tabs.add_tab("Tab 2".to_string(), None);
        tabs.set_current_index(Some(1));
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        append_menu_bar_visual_commands(&mut layer, &menu_bar);
        append_menu_visual_commands(&mut layer, &menu);
        append_tool_bar_visual_commands(&mut layer, &tool_bar);
        append_status_bar_visual_commands(&mut layer, &status_bar);
        append_tab_widget_visual_commands(&mut layer, &tabs);
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 280,
                height: 190,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
        let sample = |x: u32, y: u32| -> [u8; 4] {
            let idx = ((y * surface.size().width + x) * 4) as usize;
            [
                surface.frame_rgba()[idx],
                surface.frame_rgba()[idx + 1],
                surface.frame_rgba()[idx + 2],
                surface.frame_rgba()[idx + 3],
            ]
        };
        let menu_bar_px = sample(6, 6);
        assert!(menu_bar_px[3] > 0);
        let menu_px = sample(8, 36);
        assert!(menu_px[3] > 0);
        let toolbar_px = sample(8, 132);
        assert!(toolbar_px[3] > 0);
        let status_px = sample(8, 166);
        assert!(status_px[3] > 0);
        let tabs_px = sample(150, 130);
        assert!(tabs_px[3] > 0);
        let stack_px = sample(150, 90);
        assert!(stack_px[3] > 0);
    }
    #[test]
    fn gpu_parity_covered_controls_emit_non_empty_command_suite() {
        use crate::widget::{
            Button, CheckBox, CheckState, ComboBox, Label, LineEdit, ListBox, Menu, MenuBar, Panel,
            ProgressBar, RadioButton, ScrollBar, Slider, StatusBar, TabWidget, ToolBar, Widget,
        };
        let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 320, 240));
        window.set_background_color(Some(Color::rgba(18, 20, 24, 255)));
        let mut panel = Panel::new(Rect::new(8, 32, 180, 100));
        panel.set_background_color(Some(Color::rgba(38, 45, 60, 255)));
        let mut button = Button::new("OK".to_string(), Rect::new(16, 42, 70, 24));
        button.set_pressed(true);
        let mut checkbox = CheckBox::new(Rect::new(16, 74, 22, 22));
        checkbox.set_state(CheckState::Checked);
        let mut radio = RadioButton::new(Rect::new(46, 74, 22, 22));
        radio.set_checked(true);
        let mut label = Label::new("Label".to_string(), Rect::new(16, 102, 80, 18));
        label.set_background_color(Some(Color::rgba(76, 84, 98, 255)));
        let mut line_edit = LineEdit::new(Rect::new(98, 102, 82, 18));
        line_edit.set_text("text".to_string());
        let mut combo = ComboBox::new(Rect::new(200, 32, 110, 22));
        combo.add_item("A");
        combo.add_item("B");
        combo.set_current_index(1);
        let mut list = ListBox::new(Rect::new(200, 58, 110, 74));
        list.add_item("One");
        list.add_item("Two");
        let mut progress = ProgressBar::new(Rect::new(8, 142, 170, 14));
        progress.set_range(0, 100);
        progress.set_value(45);
        let mut slider = Slider::new(Rect::new(8, 162, 170, 20));
        slider.set_range(0, 100);
        slider.set_value(35);
        let mut scroll = ScrollBar::new(Rect::new(8, 186, 170, 16));
        scroll.set_range(0, 100);
        scroll.set_page_step(20);
        scroll.set_value(30);
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 320, 24));
        menu_bar.add_menu(1001);
        menu_bar.add_menu(1002);
        let mut menu = Menu::new(Rect::new(200, 136, 110, 64));
        menu.set_title("File".to_string());
        menu.add_action("open", "Open", "action_open");
        menu.add_action("save", "Save", "action_save");
        let mut tool_bar = ToolBar::new(Rect::new(0, 210, 320, 24));
        tool_bar.add_action("cut", "Cut", "action_cut");
        tool_bar.add_action("copy", "Copy", "action_copy");
        let mut status_bar = StatusBar::new(Rect::new(0, 234, 320, 20));
        status_bar.set_message("Ready".to_string());
        let mut tabs = TabWidget::new(Rect::new(192, 202, 120, 32));
        tabs.add_tab(1);
        tabs.add_tab(2);
        tabs.set_current_index(1);
        let mut layer = SceneLayer::new(0);
        append_window_visual_commands(&mut layer, &window);
        append_panel_visual_commands(&mut layer, &panel);
        append_button_visual_commands(&mut layer, &button);
        append_checkbox_visual_commands(&mut layer, &checkbox);
        append_radiobutton_visual_commands(&mut layer, &radio);
        append_label_visual_commands(&mut layer, &label);
        append_line_edit_visual_commands(&mut layer, &line_edit);
        append_combo_box_visual_commands(&mut layer, &combo);
        append_list_box_visual_commands(&mut layer, &list);
        append_progress_bar_visual_commands(&mut layer, &progress);
        append_slider_visual_commands(&mut layer, &slider);
        append_scroll_bar_visual_commands(&mut layer, &scroll);
        append_menu_bar_visual_commands(&mut layer, &menu_bar);
        append_menu_visual_commands(&mut layer, &menu);
        append_tool_bar_visual_commands(&mut layer, &tool_bar);
        append_status_bar_visual_commands(&mut layer, &status_bar);
        append_tab_widget_visual_commands(&mut layer, &tabs);
        assert!(layer.commands().len() >= 30);
    }
    #[test]
    fn gpu_parity_covered_controls_auto_compose_runs_with_gpu_or_cpu_backend() {
        use crate::widget::{
            Button, CheckBox, CheckState, ComboBox, Label, LineEdit, ListBox, Menu, MenuBar, Panel,
            ProgressBar, RadioButton, ScrollBar, Slider, StatusBar, TabWidget, ToolBar, Widget,
        };
        let mut window = Window::new("Main".to_string(), Rect::new(0, 0, 320, 240));
        window.set_background_color(Some(Color::rgba(18, 20, 24, 255)));
        let mut panel = Panel::new(Rect::new(8, 32, 180, 100));
        panel.set_background_color(Some(Color::rgba(38, 45, 60, 255)));
        let mut button = Button::new("OK".to_string(), Rect::new(16, 42, 70, 24));
        button.set_pressed(true);
        let mut checkbox = CheckBox::new(Rect::new(16, 74, 22, 22));
        checkbox.set_state(CheckState::Checked);
        let mut radio = RadioButton::new(Rect::new(46, 74, 22, 22));
        radio.set_checked(true);
        let mut label = Label::new("Label".to_string(), Rect::new(16, 102, 80, 18));
        label.set_background_color(Some(Color::rgba(76, 84, 98, 255)));
        let mut line_edit = LineEdit::new(Rect::new(98, 102, 82, 18));
        line_edit.set_text("text".to_string());
        let mut combo = ComboBox::new(Rect::new(200, 32, 110, 22));
        combo.add_item("A");
        combo.add_item("B");
        combo.set_current_index(1);
        let mut list = ListBox::new(Rect::new(200, 58, 110, 74));
        list.add_item("One");
        list.add_item("Two");
        let mut progress = ProgressBar::new(Rect::new(8, 142, 170, 14));
        progress.set_range(0, 100);
        progress.set_value(45);
        let mut slider = Slider::new(Rect::new(8, 162, 170, 20));
        slider.set_range(0, 100);
        slider.set_value(35);
        let mut scroll = ScrollBar::new(Rect::new(8, 186, 170, 16));
        scroll.set_range(0, 100);
        scroll.set_page_step(20);
        scroll.set_value(30);
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 320, 24));
        menu_bar.add_menu(1001);
        menu_bar.add_menu(1002);
        let mut menu = Menu::new(Rect::new(200, 136, 110, 64));
        menu.set_title("File".to_string());
        menu.add_action("open", "Open", "action_open");
        menu.add_action("save", "Save", "action_save");
        let mut tool_bar = ToolBar::new(Rect::new(0, 210, 320, 24));
        tool_bar.add_action("cut", "Cut", "action_cut");
        tool_bar.add_action("copy", "Copy", "action_copy");
        let mut status_bar = StatusBar::new(Rect::new(0, 234, 320, 20));
        status_bar.set_message("Ready".to_string());
        let mut tabs = TabWidget::new(Rect::new(192, 202, 120, 32));
        tabs.add_tab(1);
        tabs.add_tab(2);
        tabs.set_current_index(1);
        let mut scene = RenderScene::new();
        let mut layer = SceneLayer::new(0);
        append_window_visual_commands(&mut layer, &window);
        append_panel_visual_commands(&mut layer, &panel);
        append_button_visual_commands(&mut layer, &button);
        append_checkbox_visual_commands(&mut layer, &checkbox);
        append_radiobutton_visual_commands(&mut layer, &radio);
        append_label_visual_commands(&mut layer, &label);
        append_line_edit_visual_commands(&mut layer, &line_edit);
        append_combo_box_visual_commands(&mut layer, &combo);
        append_list_box_visual_commands(&mut layer, &list);
        append_progress_bar_visual_commands(&mut layer, &progress);
        append_slider_visual_commands(&mut layer, &slider);
        append_scroll_bar_visual_commands(&mut layer, &scroll);
        append_menu_bar_visual_commands(&mut layer, &menu_bar);
        append_menu_visual_commands(&mut layer, &menu);
        append_tool_bar_visual_commands(&mut layer, &tool_bar);
        append_status_bar_visual_commands(&mut layer, &status_bar);
        append_tab_widget_visual_commands(&mut layer, &tabs);
        scene.add_layer(layer);
        let mut surface = SoftwareSurface::new(
            Size {
                width: 340,
                height: 260,
            },
            1.0,
        );
        let backend = scene.compose_to_config_auto(&mut surface, Color::TRANSPARENT, None);
        assert!(matches!(
            backend,
            AutoRenderBackend::GpuWgpu | AutoRenderBackend::CpuSoftware
        ));
        let idx = ((20 * surface.size().width + 20) * 4) as usize;
        assert!(surface.frame_rgba()[idx + 3] > 0);
    }
}
/// Append visual commands for a `Dialog` baseline representation.
pub fn append_dialog_visual_commands(layer: &mut SceneLayer, dialog: &Dialog) {
    push_widget_fill_and_border(
        layer,
        dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Dialog".to_string(),
            font: dialog.font().cloned().unwrap_or_default(),
            color: dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `MessageBox` baseline representation.
pub fn append_message_box_visual_commands(layer: &mut SceneLayer, message_box: &MessageBox) {
    push_widget_fill_and_border(
        layer,
        message_box,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = message_box.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: message_box.title().to_string(),
            font: message_box.font().cloned().unwrap_or_default(),
            color: message_box.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 8,
                    y: rect.y + 24,
                },
                text: "Message content".to_string(),
                font: message_box.font().cloned().unwrap_or_default(),
                color: message_box.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `FileDialog` baseline representation.
pub fn append_file_dialog_visual_commands(layer: &mut SceneLayer, file_dialog: &FileDialog) {
    push_widget_fill_and_border(
        layer,
        file_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = file_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: file_dialog.title().to_string(),
            font: file_dialog.font().cloned().unwrap_or_default(),
            color: file_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "File browser".to_string(),
                font: file_dialog.font().cloned().unwrap_or_default(),
                color: file_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `ColorDialog` baseline representation.
pub fn append_color_dialog_visual_commands(layer: &mut SceneLayer, color_dialog: &ColorDialog) {
    push_widget_fill_and_border(
        layer,
        color_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = color_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Color Dialog".to_string(),
            font: color_dialog.font().cloned().unwrap_or_default(),
            color: color_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.width > 40 && rect.height > 40 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 32,
                    width: 80,
                    height: 80,
                },
                color: Color::RED,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 16,
                    y: rect.y + 32,
                    width: 80,
                    height: 80,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
        }
    }
}
/// Append visual commands for a `FontDialog` baseline representation.
pub fn append_font_dialog_visual_commands(layer: &mut SceneLayer, font_dialog: &FontDialog) {
    push_widget_fill_and_border(
        layer,
        font_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = font_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Font Dialog".to_string(),
            font: font_dialog.font().cloned().unwrap_or_default(),
            color: font_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "Font preview: ABCabc123".to_string(),
                font: font_dialog.font().cloned().unwrap_or_default(),
                color: font_dialog.foreground_color().unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for a `PopupWindow` baseline representation.
pub fn append_popup_window_visual_commands(layer: &mut SceneLayer, popup_window: &PopupWindow) {
    push_widget_fill_and_border(
        layer,
        popup_window,
        Some(Color::rgba(250, 250, 252, 255)),
        Some((Color::SECONDARY, 1)),
    );
    let rect = popup_window.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Popup Window".to_string(),
            font: popup_window.font().cloned().unwrap_or_default(),
            color: popup_window.foreground_color().unwrap_or(Color::FOREGROUND),
        });
    }
}
/// Append visual commands for a `DirectoryDialog` baseline representation.
pub fn append_directory_dialog_visual_commands(
    layer: &mut SceneLayer,
    directory_dialog: &DirectoryDialog,
) {
    push_widget_fill_and_border(
        layer,
        directory_dialog,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = directory_dialog.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Directory Dialog".to_string(),
            font: directory_dialog.font().cloned().unwrap_or_default(),
            color: directory_dialog
                .foreground_color()
                .unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 24,
                    width: rect.width - 16,
                    height: rect.height - 40,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 32,
                },
                text: "Directory browser".to_string(),
                font: directory_dialog.font().cloned().unwrap_or_default(),
                color: directory_dialog
                    .foreground_color()
                    .unwrap_or(Color::FOREGROUND),
            });
        }
    }
}
/// Append visual commands for an `ActivityIndicator` baseline representation.
pub fn append_activity_indicator_visual_commands(
    layer: &mut SceneLayer,
    activity_indicator: &ActivityIndicator,
) {
    push_widget_fill_and_border(
        layer,
        activity_indicator,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = activity_indicator.geometry();
    let center = Point {
        x: rect.x + (rect.width / 2) as i32,
        y: rect.y + (rect.height / 2) as i32,
    };
    let radius = (rect.width.min(rect.height) / 2 - 4) as f32;
    // Draw activity indicator
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI / 4.0;
        let alpha = (i as f32 / 8.0) * 255.0;
        let color = Color {
            r: 0,
            g: 128,
            b: 255,
            a: alpha as u8,
        };
        let x = center.x + (angle.cos() * radius) as i32;
        let y = center.y + (angle.sin() * radius) as i32;
        layer.push(RenderCommand::DrawCircle {
            center: Point { x, y },
            radius: 3,
            color,
        });
    }
}
/// Append visual commands for a `ToggleButton` baseline representation.
pub fn append_toggle_button_visual_commands(
    layer: &mut SceneLayer,
    toggle_button: &crate::widget::ToggleButton,
) {
    push_widget_fill_and_border(
        layer,
        toggle_button,
        Some(if toggle_button.is_checked() {
            Color::PRIMARY
        } else {
            Color::BACKGROUND
        }),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = toggle_button.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    layer.push(RenderCommand::DrawText {
        origin: centered_text_origin(rect),
        text: toggle_button.text().to_string(),
        font: toggle_button.font().cloned().unwrap_or_default(),
        color: if toggle_button.is_checked() {
            Color::WHITE
        } else {
            toggle_button
                .foreground_color()
                .unwrap_or(Color::rgba(30, 32, 36, 255))
        },
    });
}
/// Append visual commands for a `CheckListBox` baseline representation.
pub fn append_check_list_box_visual_commands(
    layer: &mut SceneLayer,
    check_list_box: &crate::widget::CheckListBox,
) {
    push_widget_fill_and_border(
        layer,
        check_list_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = check_list_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let row_height = 24u32;
    let padding = 8i32;
    let text_color = Color::rgba(40, 44, 52, 255);
    let font = Font::default_ui();
    let visible_rows = (rect.height / row_height) as usize;
    let row_count = check_list_box.count().min(visible_rows);
    for row in 0..row_count {
        let row_y = rect.y + (row as u32 * row_height) as i32;
        // Draw checkbox
        let checkbox_rect = Rect {
            x: rect.x + padding,
            y: row_y + (row_height as i32 - 16) / 2,
            width: 16,
            height: 16,
        };
        layer.push(RenderCommand::DrawRectStroke {
            rect: checkbox_rect,
            color: Color::rgba(122, 128, 138, 255),
            width: 1,
        });
        if check_list_box.is_selected(row) {
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: checkbox_rect.x + 3,
                    y: checkbox_rect.y + 3,
                    width: 10,
                    height: 10,
                },
                color: Color::PRIMARY,
            });
        }
        // Draw item text
        if let Some(item) = check_list_box.item(row) {
            layer.push(RenderCommand::DrawText {
                text: item.to_string(),
                origin: Point {
                    x: rect.x + padding + 24,
                    y: row_y + row_height as i32 / 2,
                },
                font: font.clone(),
                color: text_color,
            });
        }
    }
}
/// Append visual commands for a `DoubleSpinBox` baseline representation.
pub fn append_double_spin_box_visual_commands(
    layer: &mut SceneLayer,
    double_spin_box: &crate::widget::DoubleSpinBox,
) {
    push_widget_fill_and_border(
        layer,
        double_spin_box,
        Some(Color::WHITE),
        Some((Color::rgba(160, 168, 180, 255), 1)),
    );
    let rect = double_spin_box.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    // Draw value text
    let text_rect = Rect {
        x: rect.x + 8,
        y: rect.y,
        width: rect.width - 32,
        height: rect.height,
    };
    layer.push(RenderCommand::DrawText {
        origin: centered_text_origin(text_rect),
        text: format!("{:.2}", double_spin_box.value()),
        font: double_spin_box.font().cloned().unwrap_or_default(),
        color: double_spin_box
            .foreground_color()
            .unwrap_or(Color::rgba(30, 32, 36, 255)),
    });
    // Draw up/down buttons
    let button_width = 32u32;
    let button_height = rect.height / 2;
    // Up button
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x + rect.width as f32 as i32 - button_width as i32,
            y: rect.y,
            width: button_width,
            height: button_height,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
    // Down button
    layer.push(RenderCommand::DrawRectStroke {
        rect: Rect {
            x: rect.x + rect.width as f32 as i32 - button_width as i32,
            y: rect.y + button_height as i32,
            width: button_width,
            height: button_height,
        },
        color: Color::rgba(160, 168, 180, 255),
        width: 1,
    });
}
/// Append visual commands for a `Dial` baseline representation.
pub fn append_dial_visual_commands(layer: &mut SceneLayer, dial: &crate::widget::Dial) {
    push_widget_fill_and_border(
        layer,
        dial,
        Some(Color::BACKGROUND),
        Some((Color::rgba(122, 128, 138, 255), 1)),
    );
    let rect = dial.geometry();
    if is_empty_rect(&rect) {
        return;
    }
    let center = Point {
        x: rect.x + rect.width as f32 as i32 / 2,
        y: rect.y + rect.height as f32 as i32 / 2,
    };
    let radius = (rect.width.min(rect.height) / 2 - 4) as u32;
    // Draw dial background
    layer.push(RenderCommand::DrawCircleStroke {
        center,
        radius,
        color: Color::rgba(160, 168, 180, 255),
        width: 2,
    });
    // Draw dial needle
    let value = dial.value() as f64;
    let min = 0.0;
    let max = 100.0;
    let angle =
        (value - min) / (max - min) * std::f64::consts::PI * 2.0 - std::f64::consts::PI / 2.0;
    let needle_end = Point {
        x: center.x + (angle.cos() * radius as f64) as i32,
        y: center.y + (angle.sin() * radius as f64) as i32,
    };
    layer.push(RenderCommand::DrawLine {
        from: center,
        to: needle_end,
        color: Color::PRIMARY,
    });
    // Draw center point
    layer.push(RenderCommand::FillCircle {
        center,
        radius: 4,
        color: Color::PRIMARY,
    });
}
/// Append visual commands for a `Wizard` baseline representation.
pub fn append_wizard_visual_commands(layer: &mut SceneLayer, wizard: &crate::widget::Wizard) {
    push_widget_fill_and_border(
        layer,
        wizard,
        Some(Color::BACKGROUND),
        Some((Color::SECONDARY, 1)),
    );
    let rect = wizard.geometry();
    if rect.width > 16 && rect.height > 12 {
        layer.push(RenderCommand::DrawText {
            origin: Point {
                x: rect.x + 8,
                y: rect.y + 4,
            },
            text: "Wizard".to_string(),
            font: wizard.font().cloned().unwrap_or_default(),
            color: wizard.foreground_color().unwrap_or(Color::FOREGROUND),
        });
        if rect.height > 30 {
            // Draw header
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x,
                    y: rect.y + 24,
                    width: rect.width,
                    height: 40,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawText {
                origin: Point {
                    x: rect.x + 16,
                    y: rect.y + 40,
                },
                text: "Wizard Step 1 of 3".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
            // Draw content area
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 72,
                    width: rect.width - 16,
                    height: rect.height - 120,
                },
                color: Color::WHITE,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + 8,
                    y: rect.y + 72,
                    width: rect.width - 16,
                    height: rect.height - 120,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            // Draw buttons
            let button_width = 80u32;
            let button_height = 28u32;
            // Back button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::BACKGROUND,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::rgba(122, 128, 138, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 * 2 - 16,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                }),
                text: "Back".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: wizard
                    .foreground_color()
                    .unwrap_or(Color::rgba(30, 32, 36, 255)),
            });
            // Next button
            layer.push(RenderCommand::FillRect {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::PRIMARY,
            });
            layer.push(RenderCommand::DrawRectStroke {
                rect: Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                },
                color: Color::rgba(52, 122, 226, 255),
                width: 1,
            });
            layer.push(RenderCommand::DrawText {
                origin: centered_text_origin(Rect {
                    x: rect.x + rect.width as f32 as i32 - button_width as i32 - 8,
                    y: rect.y + rect.height as f32 as i32 - button_height as i32 - 8,
                    width: button_width,
                    height: button_height,
                }),
                text: "Next".to_string(),
                font: wizard.font().cloned().unwrap_or_default(),
                color: Color::WHITE,
            });
        }
    }
}
/// Routing logic for native vs custom widget drawing.
/// This function provides a framework for routing between native and custom drawing paths.
/// Widgets that implement the Draw trait will use custom drawing, others use native.
pub fn route_widget_drawing<W>(
    widget: &mut W,
    context: &mut RenderContext,
    custom_renderer: impl FnOnce(&mut W, &mut RenderContext),
    _native_renderer: impl FnOnce(&mut W, &mut RenderContext),
) where
    W: Widget + ?Sized,
{
    // In a real implementation, this would check if widget implements Draw trait
    // For now, we provide both paths and let the caller choose
    // This is a simplified routing mechanism
    custom_renderer(widget, context);
}
/// Check if a widget uses custom drawing.
/// This is a placeholder for future implementation with trait object system.
pub fn widget_uses_custom_drawing<W>(_widget: &W) -> bool
where
    W: Widget + ?Sized,
{
    // Placeholder: In a real implementation, this would check if widget implements Draw trait
    // For now, return false to indicate native rendering
    false
}
/// Render a widget with automatic routing between native and custom drawing.
/// This is a simplified version that delegates to the provided renderer.
pub fn render_widget<W>(
    widget: &mut W,
    backend: &mut dyn PaintBackend,
    custom_renderer: impl FnOnce(&mut W, &mut RenderContext),
) where
    W: Widget + ?Sized,
{
    let mut context = RenderContext::new(backend);
    custom_renderer(widget, &mut context);
}
/// Helper function to render widgets that implement Draw trait.
pub fn render_custom_widget<W>(widget: &mut W, context: &mut RenderContext)
where
    W: crate::widget::Draw,
{
    widget.draw(context);
}
/// Helper function to render widgets using native platform rendering.
pub fn render_native_widget<W>(widget: &W, context: &mut RenderContext)
where
    W: Widget,
{
    // Native rendering is handled by the platform backend
    // This function is a placeholder for future native rendering integration
    let rect = widget.geometry();
    let style = widget.style();
    // Draw basic widget background and border as fallback
    if let Some(bg_color) = style.background_color {
        context.fill_rect(rect, bg_color);
    }
    if style.border_width > 0 {
        if let Some(border_color) = style.border_color {
            context.draw_rect_stroke(rect, border_color, style.border_width);
        }
    }
}
