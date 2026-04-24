//! Rendering backends: software surface, paint, batch, and scene.
pub(crate) mod batch;
pub(crate) mod paint;
pub(crate) mod scene;
pub(crate) mod surface;

pub use paint::{PaintBackend, SoftwarePaintBackend};
pub use scene::{
    average_frame_time, current_fps, current_quality_level, last_auto_render_backend,
    set_quality_level, AutoRenderBackend, RenderScene, SceneLayer,
};
pub use surface::{
    default_software_render_config, set_default_software_render_config, BackBuffer, RenderContext,
    SoftwareRenderConfig, SoftwareSurface,
};
