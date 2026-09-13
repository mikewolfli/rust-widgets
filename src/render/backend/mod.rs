// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rendering backends: software surface, paint, batch, and scene.
pub(crate) mod batch;
pub(crate) mod paint;
pub(crate) mod scene;
pub(crate) mod surface;

// Public re-exports consumed by downstream — suppress intra-crate unused warning.
#[allow(unused_imports)]
pub use batch::{BatchCommand, BatchError, BatchId, BatchRenderer};
pub use paint::{PaintBackend, SoftwarePaintBackend};

#[cfg(feature = "quality-management")]
pub use scene::{average_frame_time, current_fps, current_quality_level, set_quality_level};
pub use scene::{last_auto_render_backend, AutoRenderBackend, RenderScene, SceneLayer};
pub use surface::{
    default_software_render_config, set_default_software_render_config, BackBuffer, RenderContext,
    SoftwareRenderConfig, SoftwareSurface,
};

#[cfg(all(test, feature = "desktop", not(any(feature = "mini", feature = "embedded"))))]
pub(crate) use surface::software_render_config_test_lock;
