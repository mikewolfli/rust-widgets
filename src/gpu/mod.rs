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

// Re-export main types
pub use adapter::{
    AdapterInfo, AdapterSelectionError, AdapterSelectionStrategy, AdapterSelector,
    GpuAdapter, GpuDeviceType, GpuType,
};
pub use buffer_pool::{
    GpuBufferAllocation, GpuBufferPoolStats, GpuMemoryProfile, GpuStagingBufferPool,
    GpuUploadBatcher, MappingStrategy, StagingBufferPoolConfig,
};
pub use manager::{GpuManager, GpuManagerAction, GpuManagerBuilder, GpuManagerError, GpuOperationMode};
pub use performance::{
    AdaptivePerformanceMonitor, AdaptivePerformanceThresholds, PerformanceSample,
    PerformanceStats, PerformanceTrap, PerformanceTrapDetector, PerformanceMonitorStrategy,
};

/// Initialize the GPU subsystem with automatic hardware detection
pub async fn init() -> Result<GpuManager, GpuManagerError> {
    GpuManager::new().await
}

/// Initialize with specific strategy
pub async fn init_with_strategy(
    strategy: AdapterSelectionStrategy,
) -> Result<GpuManager, GpuManagerError> {
    GpuManager::with_strategy(strategy).await
}

/// Check if GPU is available
pub fn is_gpu_available() -> bool {
    // This is a simplified check - in production, you'd actually try to create
    // a GPU context
    cfg!(feature = "gpu-wgpu")
}

/// Get a summary of the GPU subsystem
pub fn subsystem_summary() -> String {
    let mut summary = String::new();
    summary.push_str("GPU Subsystem Summary\n");
    summary.push_str("====================\n\n");

    summary.push_str(&format!("GPU support: {}\n", if is_gpu_available() { "enabled" } else { "disabled" }));
    
    summary.push_str("\nSupported features:\n");
    summary.push_str("  - Automatic GPU adapter selection\n");
    summary.push_str("  - Hardware-adaptive buffer pools\n");
    summary.push_str("  - Performance monitoring\n");
    summary.push_str("  - Dynamic quality adjustment\n");
    summary.push_str("  - Performance trap detection\n");

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subsystem_summary() {
        let summary = subsystem_summary();
        assert!(summary.contains("GPU Subsystem"));
        assert!(summary.contains("Automatic GPU"));
    }
}
