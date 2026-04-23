//! Rendering backends: software surface, paint, batch, and scene.
pub(crate) mod surface;
pub(crate) mod paint;
pub(crate) mod batch;
pub(crate) mod scene;

pub use surface::{BackBuffer, RenderContext, SoftwareSurface, SoftwareRenderConfig,
    set_default_software_render_config, default_software_render_config};
pub use paint::{PaintBackend, SoftwarePaintBackend};
pub use batch::BatchId;
pub use scene::{SceneLayer, RenderScene, AutoRenderBackend,
    last_auto_render_backend,
    current_quality_level, set_quality_level, current_fps, average_frame_time};
