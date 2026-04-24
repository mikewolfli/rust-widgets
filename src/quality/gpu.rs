//! GPU capability detection.
//!
//! **Deprecated**: Use [`crate::gpu::adapter::AdapterInfo`] instead.
//! `AdapterInfo.device_type` provides all information previously available
//! through `GpuCapability` (see `performance_tier()`, `is_integrated()`, etc.).
use super::level::QualityLevel;
/// GPU capability detection based on adapter information.
///
/// ⚠️ **Deprecated**: Use [`crate::gpu::adapter::AdapterInfo`] instead.
#[derive(Debug, Clone, Copy)]
#[allow(deprecated)]
pub struct GpuCapability {
    /// Whether the GPU supports high-quality rendering.
    pub supports_high_quality: bool,
    /// Whether the GPU is integrated (vs discrete).
    pub is_integrated: bool,
    /// Estimated performance tier (1-5, higher is better).
    pub performance_tier: u8,
}
impl Default for GpuCapability {
    fn default() -> Self {
        Self {
            supports_high_quality: true,
            is_integrated: false,
            performance_tier: 3,
        }
    }
}
impl GpuCapability {
    /// Creates GPU capability from adapter information.
    #[cfg(feature = "gpu-wgpu")]
    pub fn from_adapter_info(adapter_info: &wgpu::AdapterInfo) -> Self {
        let supports_high_quality = matches!(
            adapter_info.device_type,
            wgpu::DeviceType::DiscreteGpu | wgpu::DeviceType::IntegratedGpu
        );
        let is_integrated = matches!(adapter_info.device_type, wgpu::DeviceType::IntegratedGpu);
        let performance_tier = match adapter_info.device_type {
            wgpu::DeviceType::DiscreteGpu => 5,
            wgpu::DeviceType::IntegratedGpu => 3,
            wgpu::DeviceType::Other => 2,
            wgpu::DeviceType::VirtualGpu => 2,
            wgpu::DeviceType::Cpu => 1,
        };
        Self {
            supports_high_quality,
            is_integrated,
            performance_tier,
        }
    }
    /// Creates a default capability when GPU info is unavailable.
    pub fn default_capability() -> Self {
        Self::default()
    }
    /// Returns the recommended initial quality level.
    pub fn recommended_initial_quality(&self) -> QualityLevel {
        if self.supports_high_quality && self.performance_tier >= 4 {
            QualityLevel::High
        } else if self.supports_high_quality && self.performance_tier >= 2 {
            QualityLevel::Medium
        } else {
            QualityLevel::Low
        }
    }
}
