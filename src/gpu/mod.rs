// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hardware-adaptive GPU management module.
//!
//! This module provides automatic GPU adapter selection, hardware-derived quality budgets, and
//! performance monitoring with dynamic quality adjustment.
//!
//! # Features
//!
//! - **Automatic GPU Detection**: Automatically selects the best available GPU
//!   (discrete > integrated > CPU) with fallback chain
//! - **Performance Monitoring**: Monitors frame times and adjusts quality
//!   dynamically based on hardware capabilities
//! - **Performance Trap Detection**: Detects performance issues and provides
//!   user guidance
//!
//! # Example
//!
//! ```rust,no_run
//! use rust_widgets::gpu::{GpuManager, GpuManagerBuilder, AdapterSelectionStrategy};
//!
//! async fn setup_gpu() {
//!     // Auto-detect best GPU
//!     let manager = GpuManager::new().await.unwrap();
//!
//!     println!("Using GPU: {}", manager.adapter_info().name);
//!     println!("Mode: {:?}", manager.operation_mode());
//! }
//! ```
//!
//! # Reachability
//!
//! **State:** Production callers: `src/render/gpu/mod.rs:1` (the software renderer reads the GPU
//! capability report) and `src/wgpu_backend/renderer/build.rs` (`gpu_manager`, RD-04): the live
//! `WgpuRenderer` owns one `GpuManager` per device, built from the adapter facts `wgpu` already
//! reported, and brackets every offscreen frame with `begin_frame`/`end_frame`. Its operation mode,
//! performance samples, trap recommendations and quality advice are read through the renderer's own
//! accessors (`gpu_adapter_info`, `gpu_performance_stats`, ...).
//!
//! # Removed: the staging pool and upload batcher (RD-04, closed)
//!
//! `GpuStagingBufferPool` advanced a ring cursor every frame while owning no GPU buffer, and
//! `GpuUploadBatcher` merged uploads that this crate never hands it. Both are deleted rather than
//! described as host-facing surface: `wgpu`'s `Queue::write_buffer`/`write_texture` already stage
//! into a reusable belt and coalesce a batch into one write, so routing uploads through a second ring
//! would add a copy and a second implementation of one concept (principle #101). A pool nobody
//! allocates from is not a feature — that was the pool's own stated deletion condition.
//!
//! # Relationship to `crate::performance` (G-214)
//!
//! This module owns hardware adaptation and per-stage attribution: adapter
//! selection, hardware-adaptive buffer profiles, and
//! `performance::PerformanceStats` (frame classification, pressure, traps) used
//! to adjust quality. `crate::performance::profiler` owns per-section frame
//! timing (`FrameProfiler`/`Profiler`). The boundary is deliberate — the two
//! answer different questions — and each metric has exactly one statistics
//! endpoint: adaptive decisions read `performance::PerformanceStats`, never a
//! second frame table built here.
//!
//! # One backend answer (G-211)
//!
//! Adapter selection lives in `backend_ladder`; `GpuManager` publishes its
//! result (including `GpuBackendTier`) through
//! `publish_shared_selection`, so other GPU paths read the same answer with
//! `shared_selection` instead of running a second ladder.
pub mod adapter;
/// Backend degradation ladder (`Primary` → OpenGL ES → software).
///
/// Gated on `gpu` because the module names `wgpu` types directly; builds without
/// that feature inherit no GPU path at all, so there is nothing to select.
#[cfg(feature = "gpu")]
pub mod backend_ladder;
pub mod manager;
pub mod performance;
pub mod texture_atlas;
/// Measures whether a hand-rolled staging ring beats `Queue::write_buffer` (RD-04 evidence).
#[cfg(all(test, feature = "gpu-wgpu"))]
mod upload_path_tests;
// Re-export main types
pub use adapter::{
    AdapterInfo, AdapterSelectionError, AdapterSelectionStrategy, AdapterSelector, GpuAdapter,
    GpuDeviceType, GpuType,
};
#[cfg(feature = "gpu")]
pub use backend_ladder::{
    backends_from_env, instance_backends, ladder_for_target, publish_shared_selection,
    select_adapter_with_gl_fallback, shared_selection, GpuBackendSelection, GpuBackendTier,
    BACKEND_LADDER,
};
pub use manager::{
    GpuManager, GpuManagerAction, GpuManagerBuilder, GpuManagerError, GpuOperationMode,
};
pub use performance::{
    AdaptivePerformanceMonitor, AdaptivePerformanceThresholds, PerformanceMonitorStrategy,
    PerformanceSample, PerformanceStats, PerformanceTrap, PerformanceTrapDetector,
};
// G-228: `TextureAtlas` is deliberately NOT re-exported here. It has zero
// in-repo callers and is documented in `texture_atlas.rs` as a host-side tool;
// hosts reach it at `rust_widgets::gpu::texture_atlas::TextureAtlas`.
pub mod init;
