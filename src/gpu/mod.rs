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
pub mod buffer_pool;
pub mod manager;
pub mod performance;
pub mod texture_atlas;
// Re-export main types
pub use adapter::{
    AdapterInfo, AdapterSelectionError, AdapterSelectionStrategy, AdapterSelector, GpuAdapter,
    GpuDeviceType, GpuType,
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
