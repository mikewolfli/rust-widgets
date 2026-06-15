//! Render scene composition and auto-backend selection.
use crate::core::Color;
#[cfg(feature = "quality-management")]
use crate::quality::QualityManager;
use crate::render::{
    PaintBackend, RenderCommand, SoftwarePaintBackend, SoftwareRenderConfig, SoftwareSurface,
};
#[cfg(feature = "gpu-wgpu")]
use crate::wgpu_backend::WgpuRenderer;
use std::sync::{Mutex, OnceLock};

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
    *global_last_auto_render_backend().lock().expect("auto render backend lock poisoned") = backend;
}
/// Returns last backend selected by `RenderScene::compose_to_config_auto`.
pub fn last_auto_render_backend() -> AutoRenderBackend {
    *global_last_auto_render_backend().lock().expect("auto render backend lock poisoned")
}
#[cfg(feature = "quality-management")]
/// Returns the current rendering quality level.
pub fn current_quality_level() -> crate::quality::QualityLevel {
    global_quality_manager().lock().expect("quality manager lock poisoned").quality_level()
}
#[cfg(feature = "quality-management")]
/// Sets the rendering quality level manually.
pub fn set_quality_level(level: crate::quality::QualityLevel) {
    let mut quality_manager =
        global_quality_manager().lock().expect("quality manager lock poisoned");
    quality_manager.set_quality_level(level);
}
#[cfg(feature = "quality-management")]
/// Returns the current frame rate.
pub fn current_fps() -> f32 {
    global_quality_manager().lock().expect("quality manager lock poisoned").current_fps()
}
#[cfg(feature = "quality-management")]
/// Returns the average frame time in seconds.
pub fn average_frame_time() -> f32 {
    global_quality_manager().lock().expect("quality manager lock poisoned").average_frame_time()
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
        Self { z_index, commands: Vec::new() }
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
    // Proper double-buffering: copy rendered content into the back buffer,
    // then present (swap back→front) instead of replacing the entire BackBuffer.
    surface.buffer.back.copy_from_slice(&backend.surface.buffer.back);
    surface.buffer.present();
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
}
#[cfg(feature = "gpu-wgpu")]
impl std::fmt::Display for GpuRenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuRenderError::SurfaceSizeZero => {
                write!(f, "surface size must be > 0 for gpu compose")
            }
            GpuRenderError::RendererUnavailable => write!(f, "wgpu renderer unavailable"),
            GpuRenderError::UploadFailed(e) => write!(f, "upload failed: {e}"),
        }
    }
}
#[cfg(feature = "gpu-wgpu")]
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
        let mut quality_manager =
            global_quality_manager().lock().expect("quality manager lock poisoned");
        quality_manager.finish_frame(frame_duration);
    }
    Ok(())
}
#[cfg(feature = "gpu-wgpu")]
fn cached_wgpu_renderer() -> Option<&'static WgpuRenderer> {
    static RENDERER: OnceLock<Option<WgpuRenderer>> = OnceLock::new();
    RENDERER.get_or_init(|| WgpuRenderer::new().ok()).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Point, Rect, Size};
    use crate::render::RenderCommand;

    // ── SceneLayer ──────────────────────────────────────────────────────

    #[test]
    fn scene_layer_new_creates_empty_layer() {
        let layer = SceneLayer::new(5);
        assert_eq!(layer.z_index(), 5);
        assert!(layer.commands().is_empty());
    }

    #[test]
    fn scene_layer_negative_z_index() {
        let layer = SceneLayer::new(-3);
        assert_eq!(layer.z_index(), -3);
    }

    #[test]
    fn scene_layer_zero_z_index() {
        let layer = SceneLayer::new(0);
        assert_eq!(layer.z_index(), 0);
    }

    #[test]
    fn scene_layer_push_and_commands() {
        let mut layer = SceneLayer::new(1);
        assert!(layer.commands().is_empty());

        let cmd = RenderCommand::FillRect { rect: Rect::new(0, 0, 100, 100), color: Color::RED };
        layer.push(cmd.clone());
        assert_eq!(layer.commands().len(), 1);

        let cmd2 = RenderCommand::DrawLine {
            from: Point::new(0, 0),
            to: Point::new(50, 50),
            color: Color::BLUE,
        };
        layer.push(cmd2.clone());
        assert_eq!(layer.commands().len(), 2);

        // Verify insertion order
        assert!(matches!(layer.commands()[0], RenderCommand::FillRect { .. }));
        assert!(matches!(layer.commands()[1], RenderCommand::DrawLine { .. }));
    }

    #[test]
    fn scene_layer_clone_is_independent() {
        let mut layer = SceneLayer::new(10);
        layer.push(RenderCommand::PopClip);
        let cloned = layer.clone();
        assert_eq!(cloned.commands().len(), 1);
        assert_eq!(cloned.z_index(), 10);
    }

    // ── RenderScene ─────────────────────────────────────────────────────

    #[test]
    fn render_scene_new_is_empty() {
        let scene = RenderScene::new();
        assert!(scene.layers().is_empty());
    }

    #[test]
    fn render_scene_default_is_empty() {
        let scene = RenderScene::default();
        assert!(scene.layers().is_empty());
    }

    #[test]
    fn render_scene_clear_removes_all_layers() {
        let mut scene = RenderScene::new();
        scene.add_layer(SceneLayer::new(0));
        scene.add_layer(SceneLayer::new(1));
        assert_eq!(scene.layers().len(), 2);

        scene.clear();
        assert!(scene.layers().is_empty());
    }

    #[test]
    fn render_scene_add_layer_and_retrieve() {
        let mut scene = RenderScene::new();
        let layer = SceneLayer::new(42);
        scene.add_layer(layer);

        assert_eq!(scene.layers().len(), 1);
        assert_eq!(scene.layers()[0].z_index(), 42);
    }

    #[test]
    fn render_scene_multiple_layers_in_insertion_order() {
        let mut scene = RenderScene::new();
        scene.add_layer(SceneLayer::new(10));
        scene.add_layer(SceneLayer::new(5));
        scene.add_layer(SceneLayer::new(20));

        // layers() returns insertion order, not sorted by z
        assert_eq!(scene.layers()[0].z_index(), 10);
        assert_eq!(scene.layers()[1].z_index(), 5);
        assert_eq!(scene.layers()[2].z_index(), 20);
    }

    #[test]
    fn render_scene_add_layer_with_zero_commands() {
        let mut scene = RenderScene::new();
        scene.add_layer(SceneLayer::new(0));
        assert_eq!(scene.layers()[0].commands().len(), 0);
    }

    // ── compose_with_backend ────────────────────────────────────────────

    #[test]
    fn compose_with_backend_empty_scene_clears() {
        let scene = RenderScene::new();
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        scene.compose_with_backend(&mut backend, Color::BLACK);

        // compose_with_backend already calls end_frame internally,
        // so frame_rgba() shows the cleared frame
        let rgba = backend.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 0, "R channel should be 0");
            assert_eq!(chunk[1], 0, "G channel should be 0");
            assert_eq!(chunk[2], 0, "B channel should be 0");
            assert_eq!(chunk[3], 255, "A channel should be 255");
        }
    }

    #[test]
    fn compose_with_backend_layers_by_z_index() {
        let mut backend = SoftwarePaintBackend::new(Size::new(50, 50), 1.0);

        let mut scene = RenderScene::new();

        // Layer with z=1 draws red region (higher z = on top)
        let mut layer_high = SceneLayer::new(1);
        layer_high
            .push(RenderCommand::FillRect { rect: Rect::new(0, 0, 25, 25), color: Color::RED });
        scene.add_layer(layer_high);

        // Layer with z=0 draws larger blue region (lower z = drawn first)
        let mut layer_low = SceneLayer::new(0);
        layer_low
            .push(RenderCommand::FillRect { rect: Rect::new(0, 0, 50, 50), color: Color::BLUE });
        scene.add_layer(layer_low);

        scene.compose_with_backend(&mut backend, Color::BLACK);

        // Pixel at (10,10) should be RED (higher z-index renders on top)
        let rgba = backend.frame_rgba();
        let stride = 50 * 4;
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 0); // B
    }

    #[test]
    fn compose_with_backend_config_temporary_override() {
        let scene = RenderScene::new();
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);

        let original = backend.render_config();

        let custom = SoftwareRenderConfig { aa_samples_per_axis: 1 };
        scene.compose_with_backend_config(&mut backend, Color::BLACK, Some(custom));

        // Config should be restored after compose
        assert_eq!(backend.render_config(), original);
    }

    #[test]
    fn compose_with_backend_config_none_preserves_config() {
        let scene = RenderScene::new();
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);
        let original = backend.render_config();
        scene.compose_with_backend_config(&mut backend, Color::BLACK, None);
        assert_eq!(backend.render_config(), original);
    }

    // ── compose_to / compose_to_config ──────────────────────────────────

    #[test]
    fn compose_to_empty_scene_clears_surface() {
        let scene = RenderScene::new();
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        scene.compose_to(&mut surface, Color::GREEN);

        // Surface should be cleared to green
        let rgba = surface.frame_rgba();
        for chunk in rgba.chunks(4) {
            assert_eq!(chunk[0], 0); // R
            assert_eq!(chunk[1], 255); // G
            assert_eq!(chunk[2], 0); // B
            assert_eq!(chunk[3], 255); // A
        }
    }

    #[test]
    fn compose_to_config_with_custom_config() {
        let scene = RenderScene::new();
        let mut surface = SoftwareSurface::new(Size::new(10, 10), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 2 };
        scene.compose_to_config(&mut surface, Color::BLACK, Some(config));
        let rgba = surface.frame_rgba();
        assert!(!rgba.is_empty());
    }

    // ── compose_to_config_auto ──────────────────────────────────────────

    #[test]
    fn compose_to_config_auto_returns_backend() {
        let scene = RenderScene::new();
        let mut surface = SoftwareSurface::new(Size::new(5, 5), 1.0);
        let backend = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
        // Either GpuWgpu (when gpu-wgpu feature available and runtime succeeds)
        // or CpuSoftware (fallback)
        assert!(
            backend == AutoRenderBackend::CpuSoftware || backend == AutoRenderBackend::GpuWgpu,
            "expected either CpuSoftware or GpuWgpu, got {:?}",
            backend
        );
    }

    #[test]
    fn compose_to_config_auto_with_zero_size_surface() {
        let scene = RenderScene::new();
        let mut surface = SoftwareSurface::new(Size::new(0, 0), 1.0);
        let backend = scene.compose_to_config_auto(&mut surface, Color::BLACK, None);
        assert_eq!(backend, AutoRenderBackend::CpuSoftware);
    }

    #[test]
    fn compose_to_config_auto_with_config() {
        let scene = RenderScene::new();
        let mut surface = SoftwareSurface::new(Size::new(5, 5), 1.0);
        let config = SoftwareRenderConfig { aa_samples_per_axis: 8 };
        let backend = scene.compose_to_config_auto(&mut surface, Color::WHITE, Some(config));
        assert!(
            backend == AutoRenderBackend::CpuSoftware || backend == AutoRenderBackend::GpuWgpu,
            "expected either CpuSoftware or GpuWgpu, got {:?}",
            backend
        );
    }

    // ── AutoRenderBackend global functions ──────────────────────────────

    #[test]
    fn last_auto_render_backend_default_is_cpu() {
        // Reset to default before checking (other tests may have modified global state)
        set_last_auto_render_backend(AutoRenderBackend::CpuSoftware);
        let backend = last_auto_render_backend();
        assert_eq!(backend, AutoRenderBackend::CpuSoftware);
    }

    #[cfg(feature = "quality-management")]
    #[test]
    fn current_quality_level_returns_reasonable_default() {
        let level = current_quality_level();
        let _ = level;
    }

    // ── Z-order edge cases ──────────────────────────────────────────────

    #[test]
    fn compose_with_backend_layer_order_by_negative_z() {
        let mut scene = RenderScene::new();
        let mut backend = SoftwarePaintBackend::new(Size::new(20, 20), 1.0);

        // z=1 draws red over whole surface (on top after sort)
        let mut top = SceneLayer::new(1);
        top.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 20, 20), color: Color::RED });
        scene.add_layer(top);

        // z=-1 draws blue over whole surface (lower priority, drawn first)
        let mut bottom = SceneLayer::new(-1);
        bottom.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 20, 20), color: Color::BLUE });
        scene.add_layer(bottom);

        // Sorted: z=-1 first (blue), then z=1 (red) -> red wins
        scene.compose_with_backend(&mut backend, Color::BLACK);

        let rgba = backend.frame_rgba();
        let idx = 0; // top-left pixel
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 3], 255); // A
    }

    #[test]
    fn compose_with_backend_multiple_layers_same_z_draws_in_input_order() {
        let mut scene = RenderScene::new();
        let mut backend = SoftwarePaintBackend::new(Size::new(10, 10), 1.0);

        // Both layers have same z=0, so insertion order determines draw order
        let mut first = SceneLayer::new(0);
        first.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED });
        scene.add_layer(first);

        let mut second = SceneLayer::new(0);
        second.push(RenderCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::GREEN });
        scene.add_layer(second);

        scene.compose_with_backend(&mut backend, Color::BLACK);

        // Green was drawn second (same z), so it should be on top
        let rgba = backend.frame_rgba();
        assert_eq!(rgba[0], 0); // R = 0
        assert_eq!(rgba[1], 255); // G = 255
        assert_eq!(rgba[2], 0); // B = 0
    }
}
