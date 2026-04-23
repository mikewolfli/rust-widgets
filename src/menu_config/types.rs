use crate::gpu::GpuType;
/// User override settings for menu features.
#[derive(Debug, Clone, Default)]
pub struct UserOverrides {
    /// User override for animations.
    pub animations: Option<bool>,
    /// User override for transparency.
    pub transparency: Option<bool>,
    /// User override for shadows.
    pub shadows: Option<bool>,
    /// User override for blur.
    pub blur: Option<bool>,
    /// User override for animation speed.
    pub animation_speed: Option<f32>,
    /// User override for max visible items.
    pub max_visible_items: Option<u32>,
    /// User override for hardware acceleration.
    pub hardware_acceleration: Option<bool>,
}
/// Hardware capabilities detected at runtime.
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// GPU type (Discrete, Integrated, CPU).
    pub gpu_type: GpuType,
    /// GPU memory in MB.
    pub gpu_memory_mb: u32,
    /// Estimated GPU performance score (0-100).
    pub gpu_performance_score: u32,
    /// System RAM in MB.
    pub system_ram_mb: u64,
    /// CPU performance score (0-100).
    pub cpu_performance_score: u32,
    /// Whether running on battery (laptops).
    pub on_battery: bool,
    /// Current performance level.
    pub performance_level: PerformanceLevel,
}
/// Performance level for adaptive feature selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceLevel {
    /// Low-end hardware - minimal effects.
    Low,
    /// Mid-range hardware - balanced effects.
    Medium,
    /// High-end hardware - all effects enabled.
    High,
}
impl Default for PerformanceLevel {
    fn default() -> Self {
        Self::Medium
    }
}
