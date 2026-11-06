// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Rendering backends: software surface, paint, and scene.
/// Resampling blits shared by [`paint::PaintBackend::draw_image_scaled`] and its software path.
pub(crate) mod image_scale;
pub(crate) mod paint;
/// The box-blur kernel the software backend runs for shadows and blur layers.
pub(crate) mod paint_blur;
pub(crate) mod scene;
pub(crate) mod surface;

// The box-blur kernel the software rasteriser blurs a region with, shared with the GPU command
// pipeline so a bridged blur runs the reference's own three widths and edge rule instead of a
// lookalike. Only the GPU command path (and its CPU parity rasteriser) consumes it, so the
// re-export exists only when that path is compiled in.
#[cfg(feature = "gpu-wgpu")]
pub(crate) use paint_blur::{box_blur_region, box_blur_widths};

// Public re-exports consumed by downstream — suppress intra-crate unused warning.
// RD2-04: the batch recording surface (`BatchRenderer`, `BatchId`, `BatchError`, `BatchCommand`)
// and its image registry were removed — they had no production consumer, and replaying a second
// command vocabulary on the compose path would be a negative optimisation.
pub use paint::{PaintBackend, SoftwarePaintBackend};

#[cfg(feature = "quality-management")]
pub use scene::{average_frame_time, current_fps, current_quality_level, set_quality_level};
pub use scene::{last_auto_render_backend, AutoRenderBackend, RenderScene, SceneLayer};
pub use surface::{
    default_software_render_config, set_default_software_render_config, text_line, BackBuffer,
    RenderContext, SoftwareRenderConfig, SoftwareSurface, VerticalAlignment, TEXT_FIT_MARGIN,
};
// `fitted_origin` is re-exported at the crate-internal level only: it is the one placement rule
// the caret and the value must share, so `line_edit` reads it rather than restating the arithmetic
// (which is how the two drifted apart). It is not part of the public surface.
pub(crate) use surface::fitted_origin;

#[cfg(all(test, feature = "desktop", widgets_unstripped))]
pub(crate) use surface::software_render_config_test_lock;
