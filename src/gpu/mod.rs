// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Hardware-adaptive GPU management module.
//!
//! This module provides automatic GPU adapter selection, hardware-specific
//! buffer pool configuration, and performance monitoring with dynamic
//! quality adjustment.
//!
//! # Features
//!
//! - **Automatic GPU Detection**: Automatically selects the best available GPU
//!   (discrete > integrated > CPU) with fallback chain
//! - **Hardware-Adaptive Buffer Pools**: Configures buffer pool parameters
//!   based on detected GPU type
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
pub mod adapter;
/// Backend degradation ladder (`Primary` → OpenGL ES → software).
///
/// Gated on `gpu` because the module names `wgpu` types directly; builds without
/// that feature inherit no GPU path at all, so there is nothing to select.
#[cfg(feature = "gpu")]
pub mod backend_ladder;
pub mod buffer_pool;
pub mod manager;
pub mod performance;
pub mod texture_atlas;
// Re-export main types
pub use adapter::{
    AdapterInfo, AdapterSelectionError, AdapterSelectionStrategy, AdapterSelector, GpuAdapter,
    GpuDeviceType, GpuType,
};
#[cfg(feature = "gpu")]
pub use backend_ladder::{
    backends_from_env, instance_backends, ladder_for_target, select_adapter_with_gl_fallback,
    GpuBackendSelection, GpuBackendTier, BACKEND_LADDER,
};
pub use buffer_pool::{
    GpuBufferAllocation, GpuBufferPoolStats, GpuMemoryProfile, GpuStagingBufferPool,
    GpuUploadBatcher, MappingStrategy, StagingBufferPoolConfig,
};
pub use init::{init, init_with_strategy, is_gpu_available, subsystem_summary};
pub use manager::{
    GpuManager, GpuManagerAction, GpuManagerBuilder, GpuManagerError, GpuOperationMode,
};
pub use performance::{
    AdaptivePerformanceMonitor, AdaptivePerformanceThresholds, PerformanceMonitorStrategy,
    PerformanceSample, PerformanceStats, PerformanceTrap, PerformanceTrapDetector,
};
pub use texture_atlas::TextureAtlas;
pub mod init;
