// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The wgpu backend's CPU rasteriser: the fallback for streams the GPU does not claim, and the
//! byte reference the GPU tests are compared against.
//!
//! Split by responsibility: `walk` owns the command loop and the derived clip state, `fills` owns
//! the per-shape kernels together with the blending and shadow-layer helpers, and `text` owns glyph
//! coverage. The geometry helpers the GPU also needs are re-exported here, so a caller never has to
//! know which file a corner arc or a clip intersection came from.
mod fills;
mod text;
mod walk;

pub use fills::align_to;
pub use walk::{rasterize_draw_commands_into_rgba8, rasterize_draw_commands_rgba8};

pub(crate) use fills::{
    bbox_for_circle, bbox_for_path, clip_segment_to_rect, for_each_line_stamp, gradient_ramp,
    path_bounds, path_fan_vertices, rect_for_line, rounded_rect_row_spans, stroke_rect_edges,
    GradientAxis,
};
pub(crate) use text::text_coverage_into;
pub(crate) use walk::{combine, effective_clip};
