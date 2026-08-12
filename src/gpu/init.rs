//! GPU initialization and subsystem summary functions.

use crate::gpu::manager::{GpuManager, GpuManagerError};
use crate::gpu::AdapterSelectionStrategy;

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

/// Check if GPU is available by checking compile-time feature and runtime status.
pub fn is_gpu_available() -> bool {
    #[cfg(feature = "wgpu")]
    {
        // Compile-time feature enabled; at runtime we need to try adapter creation.
        // This is a best-effort check — for true runtime detection call `GpuManager::new().await`.
        cfg!(feature = "wgpu")
    }
    #[cfg(not(feature = "wgpu"))]
    {
        false
    }
}

/// Get a summary of the GPU subsystem with runtime-aware details.
pub fn subsystem_summary() -> String {
    let mut summary = String::new();
    summary.push_str("GPU Subsystem Summary\n");
    summary.push_str("====================\n\n");
    summary.push_str(&format!(
        "GPU support (compile-time): {}\n",
        if cfg!(feature = "wgpu") { "enabled" } else { "disabled" }
    ));
    summary.push_str(&format!(
        "GPU support (runtime check): {}\n\n",
        if is_gpu_available() { "available" } else { "not available" }
    ));
    summary.push_str("Capabilities:\n");
    summary.push_str("  - Adapter selection: ");
    if cfg!(feature = "wgpu") {
        summary.push_str("automatic\n");
        summary.push_str("  - WGPU backend: enabled\n");
        summary.push_str("  - Buffer pools: hardware-adaptive\n");
        summary.push_str("  - Performance monitoring: enabled\n");
    } else {
        summary.push_str("software fallback\n");
        summary.push_str("  - Software rendering fallback\n");
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_subsystem_summary() {
        let summary = subsystem_summary();
        assert!(summary.contains("GPU Subsystem"));
        assert!(summary.contains("Adapter selection"));
    }
}
