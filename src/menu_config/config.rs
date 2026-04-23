use crate::gpu::{GpuAdapter, GpuType};
use super::{HardwareCapabilities, PerformanceLevel, UserOverrides};
/// Menu system configuration with hardware-adaptive features.
#[derive(Debug, Clone)]
pub struct MenuConfig {
    /// Whether animations are enabled.
    animations_enabled: bool,
    /// Whether transparency effects are enabled.
    transparency_enabled: bool,
    /// Whether shadows are enabled.
    shadows_enabled: bool,
    /// Whether blur effects are enabled.
    blur_enabled: bool,
    /// Animation duration multiplier (1.0 = normal, 0.5 = fast, 2.0 = slow).
    animation_speed: f32,
    /// Maximum menu items before scrolling.
    max_visible_items: u32,
    /// Whether to use hardware-accelerated rendering.
    hardware_acceleration: bool,
    /// Whether to enable menu caching.
    caching_enabled: bool,
    /// User override flags.
    pub(crate) user_overrides: UserOverrides,
    /// Auto-detected hardware capabilities.
    pub(crate) hardware_caps: HardwareCapabilities,
}
impl MenuConfig {
    /// Creates a new menu configuration with auto-detection.
    pub fn new() -> Self {
        let hardware_caps = Self::detect_hardware_capabilities();
        let mut config = Self {
            animations_enabled: false,
            transparency_enabled: false,
            shadows_enabled: false,
            blur_enabled: false,
            animation_speed: 1.0,
            max_visible_items: 20,
            hardware_acceleration: false,
            caching_enabled: false,
            user_overrides: UserOverrides::default(),
            hardware_caps,
        };
        config.apply_hardware_defaults();
        config
    }
    fn detect_hardware_capabilities() -> HardwareCapabilities {
        let gpu_type = GpuAdapter::detect_primary_gpu_type().unwrap_or(GpuType::Integrated);
        let gpu_memory_mb = Self::detect_gpu_memory();
        let gpu_performance_score = Self::estimate_gpu_performance(&gpu_type, gpu_memory_mb);
        let system_ram_mb = Self::detect_system_memory();
        let cpu_performance_score = Self::estimate_cpu_performance();
        let on_battery = Self::detect_battery_status();
        let performance_level = if gpu_performance_score >= 70 && !on_battery {
            PerformanceLevel::High
        } else if gpu_performance_score >= 40 {
            PerformanceLevel::Medium
        } else {
            PerformanceLevel::Low
        };
        HardwareCapabilities {
            gpu_type,
            gpu_memory_mb,
            gpu_performance_score,
            system_ram_mb,
            cpu_performance_score,
            on_battery,
            performance_level,
        }
    }
    fn detect_gpu_memory() -> u32 {
        512
    }
    fn estimate_gpu_performance(gpu_type: &GpuType, memory_mb: u32) -> u32 {
        match gpu_type {
            GpuType::Discrete => {
                let base_score = 70;
                let memory_bonus = (memory_mb / 1024).min(20);
                base_score + memory_bonus
            }
            GpuType::Integrated => {
                let base_score = 40;
                let memory_bonus = (memory_mb / 512).min(20);
                base_score + memory_bonus
            }
            GpuType::Cpu => 20,
        }
    }
    fn detect_system_memory() -> u64 {
        4096
    }
    fn estimate_cpu_performance() -> u32 {
        50
    }
    fn detect_battery_status() -> bool {
        false
    }
    fn apply_hardware_defaults(&mut self) {
        match self.hardware_caps.performance_level {
            PerformanceLevel::High => {
                self.animations_enabled = true;
                self.transparency_enabled = true;
                self.shadows_enabled = true;
                self.blur_enabled = true;
                self.animation_speed = 1.0;
                self.max_visible_items = 25;
                self.hardware_acceleration = true;
                self.caching_enabled = true;
            }
            PerformanceLevel::Medium => {
                self.animations_enabled = true;
                self.transparency_enabled = true;
                self.shadows_enabled = false;
                self.blur_enabled = false;
                self.animation_speed = 0.8;
                self.max_visible_items = 20;
                self.hardware_acceleration = true;
                self.caching_enabled = true;
            }
            PerformanceLevel::Low => {
                self.animations_enabled = false;
                self.transparency_enabled = false;
                self.shadows_enabled = false;
                self.blur_enabled = false;
                self.animation_speed = 0.5;
                self.max_visible_items = 15;
                self.hardware_acceleration = false;
                self.caching_enabled = false;
            }
        }
    }
    /// Applies user overrides to the configuration.
    pub fn apply_user_overrides(&mut self) {
        if let Some(animations) = self.user_overrides.animations {
            self.animations_enabled = animations;
        }
        if let Some(transparency) = self.user_overrides.transparency {
            self.transparency_enabled = transparency;
        }
        if let Some(shadows) = self.user_overrides.shadows {
            self.shadows_enabled = shadows;
        }
        if let Some(blur) = self.user_overrides.blur {
            self.blur_enabled = blur;
        }
        if let Some(speed) = self.user_overrides.animation_speed {
            self.animation_speed = speed.clamp(0.1, 3.0);
        }
        if let Some(max_items) = self.user_overrides.max_visible_items {
            self.max_visible_items = max_items.max(5);
        }
        if let Some(hw_accel) = self.user_overrides.hardware_acceleration {
            self.hardware_acceleration = hw_accel;
        }
    }
    /// Resets to hardware defaults, clearing user overrides.
    pub fn reset_to_defaults(&mut self) {
        self.user_overrides = UserOverrides::default();
        self.apply_hardware_defaults();
    }
    pub fn animations_enabled(&self) -> bool {
        self.animations_enabled
    }
    pub fn transparency_enabled(&self) -> bool {
        self.transparency_enabled
    }
    pub fn shadows_enabled(&self) -> bool {
        self.shadows_enabled
    }
    pub fn blur_enabled(&self) -> bool {
        self.blur_enabled
    }
    pub fn animation_speed(&self) -> f32 {
        self.animation_speed
    }
    pub fn max_visible_items(&self) -> u32 {
        self.max_visible_items
    }
    pub fn hardware_acceleration(&self) -> bool {
        self.hardware_acceleration
    }
    pub fn caching_enabled(&self) -> bool {
        self.caching_enabled
    }
    pub fn hardware_caps(&self) -> &HardwareCapabilities {
        &self.hardware_caps
    }
    pub fn user_overrides(&self) -> &UserOverrides {
        &self.user_overrides
    }
    pub fn set_animations_enabled(&mut self, enabled: bool) {
        self.user_overrides.animations = Some(enabled);
        self.animations_enabled = enabled;
    }
    pub fn set_transparency_enabled(&mut self, enabled: bool) {
        self.user_overrides.transparency = Some(enabled);
        self.transparency_enabled = enabled;
    }
    pub fn set_shadows_enabled(&mut self, enabled: bool) {
        self.user_overrides.shadows = Some(enabled);
        self.shadows_enabled = enabled;
    }
    pub fn set_blur_enabled(&mut self, enabled: bool) {
        self.user_overrides.blur = Some(enabled);
        self.blur_enabled = enabled;
    }
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.user_overrides.animation_speed = Some(speed.clamp(0.1, 3.0));
        self.animation_speed = speed.clamp(0.1, 3.0);
    }
    pub fn set_max_visible_items(&mut self, max: u32) {
        self.user_overrides.max_visible_items = Some(max.max(5));
        self.max_visible_items = max.max(5);
    }
    pub fn set_hardware_acceleration(&mut self, enabled: bool) {
        self.user_overrides.hardware_acceleration = Some(enabled);
        self.hardware_acceleration = enabled;
    }
    /// Returns a user-friendly description of current settings.
    pub fn settings_summary(&self) -> String {
        format!(
            "Menu Settings:\n\
             - Animations: {}\n\
             - Transparency: {}\n\
             - Shadows: {}\n\
             - Blur: {}\n\
             - Animation Speed: {:.1}x\n\
             - Max Visible Items: {}\n\
             - Hardware Acceleration: {}\n\
             - Performance Level: {:?}",
            if self.animations_enabled { "On" } else { "Off" },
            if self.transparency_enabled {
                "On"
            } else {
                "Off"
            },
            if self.shadows_enabled { "On" } else { "Off" },
            if self.blur_enabled { "On" } else { "Off" },
            self.animation_speed,
            self.max_visible_items,
            if self.hardware_acceleration {
                "On"
            } else {
                "Off"
            },
            self.hardware_caps.performance_level
        )
    }
    /// Returns true if any user overrides are active.
    pub fn has_user_overrides(&self) -> bool {
        self.user_overrides.animations.is_some()
            || self.user_overrides.transparency.is_some()
            || self.user_overrides.shadows.is_some()
            || self.user_overrides.blur.is_some()
            || self.user_overrides.animation_speed.is_some()
            || self.user_overrides.max_visible_items.is_some()
            || self.user_overrides.hardware_acceleration.is_some()
    }
    pub(crate) fn apply_battery_adaptive_reduction(&mut self) {
        if self.user_overrides.animations.is_none() {
            self.animations_enabled = false;
        }
        if self.user_overrides.transparency.is_none() {
            self.transparency_enabled = false;
        }
        if self.user_overrides.blur.is_none() {
            self.blur_enabled = false;
        }
    }
}
impl Default for MenuConfig {
    fn default() -> Self {
        Self::new()
    }
}
