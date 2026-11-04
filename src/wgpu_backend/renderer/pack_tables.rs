// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The command-stream packer's per-frame side tables, kept across frames (RD-02).
//!
//! Split out of `encode.rs`, which the tables pushed past the source-structure limit
//! (principle #9). The seam is data vs encoder: this file describes *what* a frame's draws are
//! recorded in, `encode.rs` fills and reads them while packing.
//!
//! # Why the tables live here rather than as locals
//!
//! Each was a fresh `Vec` per encoded frame, regrown from zero on every frame of a steady stream.
//! They are cleared and refilled instead, so the capacity a frame needed survives to the next one —
//! the same reason [`super::resources::PackScratch`] holds the byte buffers beside them.
//!
//! The entries own GPU handles (a bind group per draw, which keeps its texture, view and uniform
//! alive). [`PackTables::release_gpu_owned`] drops those at the **end** of the frame, so the handles
//! live exactly as long as they did when the vectors were locals, while their capacity is retained.

use super::super::types::PixelRect;
use super::resources::ShadowDraw;
use super::uploads::{GradientDraw, ImageDraw, PathDraw, RadialDraw, ShapeDraw, TextDraw};

/// One frame's draw tables.
#[derive(Default)]
pub(crate) struct PackTables {
    pub(super) image_draws: Vec<ImageDraw>,
    pub(super) text_draws: Vec<TextDraw>,
    pub(super) shape_draws: Vec<ShapeDraw>,
    pub(super) radial_draws: Vec<RadialDraw>,
    pub(super) gradient_draws: Vec<GradientDraw>,
    pub(super) path_draws: Vec<PathDraw>,
    pub(super) shadow_draws: Vec<ShadowDraw>,
    /// One entry per `DrawStep::BlurRegion`, filled while the plan is packed.
    pub(super) frame_blur_specs: Vec<(PixelRect, u32)>,
    /// The layer rectangle of each entry in `shadow_draws`, in the same order.
    pub(super) shadow_specs: Vec<PixelRect>,
}

impl PackTables {
    /// Empties every table, keeping its capacity.
    pub(crate) fn reset(&mut self) {
        self.image_draws.clear();
        self.text_draws.clear();
        self.shape_draws.clear();
        self.radial_draws.clear();
        self.gradient_draws.clear();
        self.path_draws.clear();
        self.shadow_draws.clear();
        self.frame_blur_specs.clear();
        self.shadow_specs.clear();
    }

    /// Drops the GPU handles a frame's draws owned, keeping the vectors' capacity.
    pub(crate) fn release_gpu_owned(&mut self) {
        self.image_draws.clear();
        self.text_draws.clear();
        self.shape_draws.clear();
        self.radial_draws.clear();
        self.gradient_draws.clear();
        self.path_draws.clear();
    }
}
