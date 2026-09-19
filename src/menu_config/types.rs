// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
    /// GPU memory in MB, or `None` when it could not be measured.
    ///
    /// This crate has no portable way to query VRAM: wgpu's `AdapterInfo` carries no
    /// memory size, and the platform backends expose total *system* memory only. The
    /// field is therefore an `Option` so "not measurable" is representable, and
    /// [`Self::gpu_memory_is_measured`] says whether the figure came from a real
    /// source or from the conservative default.
    ///
    /// It used to be a bare `u32` that silently defaulted to `512`, which
    /// `MenuConfigDialog::gpu_description` printed as if it were detected. Callers
    /// must not present a defaulted value as a measurement (principle #37/#12).
    pub gpu_memory_mb: Option<u32>,
    /// Whether [`Self::gpu_memory_mb`] came from a real source.
    ///
    /// `false` means the value is the conservative assumption for the GPU type, not
    /// a probe result. A UI that describes the hardware must say which it is.
    pub gpu_memory_is_measured: bool,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerformanceLevel {
    /// Low-end hardware - minimal effects.
    Low,
    /// Mid-range hardware - balanced effects.
    #[default]
    Medium,
    /// High-end hardware - all effects enabled.
    High,
}
