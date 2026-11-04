// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Getting a finished frame onto a window: the path `Default` presentation does not use.
//!
//! # What is here and what is not
//!
//! The host states an intent ([`FramePresentation`]); the platform backend answers whether it can
//! honour it; the presenter then owns one presentation target per window. Everything in this
//! module is about *delivery*: the same frame the software path would blit is uploaded instead of
//! blitted, and the `Smooth` intent additionally draws the frame's command stream where that is
//! faithful.
//!
//! The platform-specific half — obtaining a native handle, knowing when to repaint, and creating
//! the target from a handle — lives in `src/platform/`. No native type appears here (principle
//! #39), and no public name says `gpu`/`surface`/`swapchain` (principle #52): the host asks for
//! "exact" or "smooth", which is what it wants, not how it is achieved.
//!
//! # Layer layout
//!
//! | Module | Responsibility | Needs a device |
//! |---|---|---|
//! | `intent` | the host's request, per window | no |
//! | `damage` | which rows changed and whether a present is owed | no |
//! | `status` | failure meaning, escalation and counters | no |
//! | `config` | size/scale/format/present-mode selection | no (`gpu-wgpu` only because of the quality type) |
//! | `smooth` | the command recorder and its audit | no |
//! | `presenter` | the target, the upload, the present | yes |
//!
//! The first three are compiled in every profile, so a build without the GPU feature still
//! answers `WindowHandle::set_frame_presentation` honestly (`false`) and still has the types the
//! host-facing API mentions.

mod damage;
mod intent;
mod status;

pub use damage::{DamageBand, PresentDamage, MAX_PRESENT_BANDS};
pub use intent::{forget, request, requested, FramePresentation};
pub use status::{
    PresentHealth, PresentationStatus, RecoveryAction, SurfaceFailure, PRESENT_RECOVERY_BUDGET,
    PRESENT_TIMEOUT_BUDGET,
};

#[cfg(feature = "gpu-wgpu")]
mod config;
#[cfg(feature = "gpu-wgpu")]
pub use config::{
    choose_config, fallback_present_mode, is_supported_by, needs_channel_swap,
    needs_premultiplication, needs_srgb_decode,
};

#[cfg(all(feature = "gpu-wgpu", feature = "raw-window-handle"))]
mod presenter;
#[cfg(all(feature = "gpu-wgpu", feature = "raw-window-handle"))]
pub use presenter::{FramePresenter, PresentStats};

#[cfg(all(feature = "gpu-wgpu", feature = "raw-window-handle"))]
mod sink;
#[cfg(all(feature = "gpu-wgpu", feature = "raw-window-handle"))]
pub use sink::{
    forget_window, frame_is_shown, has_target, last_outcome, last_refusal, note_frame_shown,
    present_recorded_commands, present_window_frame, presented_command_frames, presented_frames,
    smooth_commands, smooth_fallback_reason, window_intent, SmoothGap, WindowPresentOutcome,
};

#[cfg(feature = "gpu-wgpu")]
mod smooth;
#[cfg(feature = "gpu-wgpu")]
pub use smooth::{
    census, census_report, classify, with_gpu_paint_backend, CommandClass, FrameCensus,
    GpuPaintBackend,
};
