//! Render scene composition and auto-backend selection.
use crate::core::{Color, Point, Rect, Size};
use crate::render::{PaintBackend, RenderCommand, SoftwarePaintBackend, SoftwareSurface, SoftwareRenderConfig};
#[cfg(feature = "quality-management")]
use crate::quality::QualityManager;
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;

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
