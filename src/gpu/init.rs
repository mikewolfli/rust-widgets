// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GPU initialization and subsystem summary functions.

use crate::compat::{format, String};
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

/// Reports whether the `wgpu` GPU backend is compiled in.
///
/// This is a compile-time fact, not a runtime probe: adapter selection is
/// asynchronous (it may enumerate devices, apply a backend ladder, and consult
/// `WGPU_BACKEND`), so a synchronous boolean cannot know whether a usable
/// adapter exists. For a genuine runtime answer, call [`GpuManager::new`]
/// (via [`init`]) and inspect the resulting manager's backend tier.
#[cfg(feature = "wgpu")]
pub fn is_gpu_available() -> bool {
    true
}

/// Reports whether the `wgpu` GPU backend is compiled in.
#[cfg(not(feature = "wgpu"))]
pub fn is_gpu_available() -> bool {
    false
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
        "GPU backend compiled in: {}\n\n",
        if is_gpu_available() { "yes" } else { "no" }
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
