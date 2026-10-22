// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::gpu::GpuType;
/// User override settings for menu features.
#[derive(Debug, Clone, Default)]
pub struct UserOverrides {
    /// Master switch: whether the adaptive menu configuration is applied at all.
    ///
    /// `None` keeps the default (off). Everything else in this struct is irrelevant
    /// while the switch is off.
    pub enabled: Option<bool>,
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
    /// User override for the per-widget text-measurement cache.
    pub caching: Option<bool>,
}
impl HardwareCapabilities {
    /// A fixed capability record for verification paths (snapshots, census, tests).
    ///
    /// Hardware detection is deliberately **not** used here: a committed SVG must not
    /// differ between a laptop and a workstation. The values describe a mid-range host,
    /// which is also the profile the pinned effect policy models.
    pub fn verification() -> Self {
        Self {
            gpu_type: GpuType::Integrated,
            gpu_memory_mb: None,
            gpu_memory_is_measured: false,
            gpu_performance_score: 60,
            system_ram_mb: Some(8192),
            cpu_performance_score: 60,
            on_battery: false,
            performance_level: PerformanceLevel::Medium,
        }
    }
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
    ///
    /// The same distinction for system memory is carried by the *type* rather than a
    /// flag: [`Self::system_ram_mb`] is `None` when nothing measured it, so an assumed
    /// figure can never be mistaken for a measurement.
    pub gpu_memory_is_measured: bool,
    /// Estimated GPU performance score (0-100).
    pub gpu_performance_score: u32,
    /// System RAM in MB, when the active backend can measure it.
    ///
    /// `None` means the backend cannot report physical memory, which is the honest
    /// answer — [`crate::platform::Platform::total_memory_mb`]'s own contract forbids
    /// substituting a made-up constant here. A figure that is *assumed* rather than
    /// measured is supplied separately by `assumed_system_memory`, and never appears
    /// in this field, so a caller reading it can trust it. The parallel field
    /// [`Self::gpu_memory_is_measured`] carries the same distinction for the GPU.
    pub system_ram_mb: Option<u64>,
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
