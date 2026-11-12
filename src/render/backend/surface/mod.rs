// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Software rendering surface: back buffer, surface, and configuration.
use crate::compat::Arc;
use crate::compat::Rc;
use crate::compat::{lock, vec, Cow, Mutex, OnceLock, RefCell, String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::render::core::command::{LayerEffect, MatteMode};
use crate::render::pixel_bytes_len;
use crate::render::{BlendMode, PaintBackend, RenderCommand, ShapedText, TextMetrics};

mod back_buffer;
mod config;
mod context;
mod damage;
mod software;
mod state;
mod text;

pub use back_buffer::BackBuffer;
pub use config::default_software_render_config;
pub use config::set_default_software_render_config;
#[cfg(test)]
pub(crate) use config::software_render_config_test_lock;
pub use config::SoftwareRenderConfig;
pub use context::RenderContext;
pub(crate) use damage::clipped_damage;
pub(crate) use damage::contains_region;
pub(crate) use damage::regions_intersect;
pub(crate) use damage::union_region;
pub(crate) use damage::MAX_DAMAGE_REGIONS;
pub use software::SoftwareSurface;
pub(crate) use state::BoxBlurScratch;
pub(crate) use state::CachedLayer;
pub(crate) use state::LayerCapture;
pub(crate) use state::MatteCapture;
pub(crate) use state::PathClip;
#[cfg(test)]
pub(crate) use text::dash_line_segments;
pub(crate) use text::fit_text_parts;
#[cfg(test)]
pub(crate) use text::fit_text_to_width;
pub(crate) use text::fit_text_to_width_measured;
pub(crate) use text::fitted_origin;
pub(crate) use text::for_each_dash_segment;
pub(crate) use text::recorded_image;
pub use text::text_line;
pub(crate) use text::FittedLabelEntry;
pub use text::VerticalAlignment;
pub(crate) use text::FITTED_LABEL_CACHE_DEPTH;
pub(crate) use text::PATH_POINT_CACHE_DEPTH;
pub use text::TEXT_FIT_MARGIN;
pub(crate) use text::{FITTED_LABEL_CACHE, PATH_POINT_CACHE};

#[cfg(test)]
mod tests;
