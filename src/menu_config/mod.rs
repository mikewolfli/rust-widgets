//! Menu system configuration with hardware-adaptive features.
//!
//! This module provides automatic feature detection based on hardware capabilities,
//! while allowing users to override settings.

use crate::gpu::{GpuAdapter, GpuType};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

/// Menu system configuration with hardware-adaptive features.
#[derive(Debug, Clone)]
pub struct MenuConfig {
    /// Whether animations are enabled
    animations_enabled: bool,
    /// Whether transparency effects are enabled
    transparency_enabled: bool,
    /// Whether shadows are enabled
    shadows_enabled: bool,
    /// Whether blur effects are enabled
    blur_enabled: bool,
    /// Animation duration multiplier (1.0 = normal, 0.5 = fast, 2.0 = slow)
    animation_speed: f32,
    /// Maximum menu items before scrolling
    max_visible_items: u32,
    /// Whether to use hardware-accelerated rendering
    hardware_acceleration: bool,
    /// Whether to enable menu caching
    caching_enabled: bool,
    /// User override flags
    user_overrides: UserOverrides,
    /// Auto-detected hardware capabilities
    hardware_caps: HardwareCapabilities,
}

/// User override settings for menu features.
#[derive(Debug, Clone, Default)]
pub struct UserOverrides {
    /// User override for animations
    pub animations: Option<bool>,
    /// User override for transparency
    pub transparency: Option<bool>,
    /// User override for shadows
    pub shadows: Option<bool>,
    /// User override for blur
    pub blur: Option<bool>,
    /// User override for animation speed
    pub animation_speed: Option<f32>,
    /// User override for max visible items
    pub max_visible_items: Option<u32>,
    /// User override for hardware acceleration
    pub hardware_acceleration: Option<bool>,
}

/// Hardware capabilities detected at runtime.
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// GPU type (Discrete, Integrated, CPU)
    pub gpu_type: GpuType,
    /// GPU memory in MB
    pub gpu_memory_mb: u32,
    /// Estimated GPU performance score (0-100)
    pub gpu_performance_score: u32,
    /// System RAM in MB
    pub system_ram_mb: u64,
    /// CPU performance score (0-100)
    pub cpu_performance_score: u32,
    /// Whether running on battery (laptops)
    pub on_battery: bool,
    /// Current performance level
    pub performance_level: PerformanceLevel,
}

/// Performance level for adaptive feature selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceLevel {
    /// Low-end hardware - minimal effects
    Low,
    /// Mid-range hardware - balanced effects
    Medium,
    /// High-end hardware - all effects enabled
    High,
}

impl Default for PerformanceLevel {
    fn default() -> Self {
        Self::Medium
    }
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

    /// Detects hardware capabilities at runtime.
    fn detect_hardware_capabilities() -> HardwareCapabilities {
        // Try to detect GPU information
        let gpu_type = GpuAdapter::detect_primary_gpu_type()
            .unwrap_or(GpuType::Integrated);
        
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

    /// Detects GPU memory (simplified implementation).
    fn detect_gpu_memory() -> u32 {
        // In a real implementation, this would query the GPU driver
        // For now, use conservative estimates based on common configurations
        512 // Default to 512MB
    }

    /// Estimates GPU performance score (0-100).
    fn estimate_gpu_performance(gpu_type: &GpuType, memory_mb: u32) -> u32 {
        match gpu_type {
            GpuType::Discrete => {
                // Discrete GPUs typically have good performance
                let base_score = 70;
                let memory_bonus = (memory_mb / 1024).min(20);
                base_score + memory_bonus
            }
            GpuType::Integrated => {
                // Integrated GPUs vary widely
                let base_score = 40;
                let memory_bonus = (memory_mb / 512).min(20);
                base_score + memory_bonus
            }
            GpuType::Cpu => {
                // CPU rendering is slowest
                20
            }
        }
    }

    /// Detects system memory in MB.
    fn detect_system_memory() -> u64 {
        // In a real implementation, query system info
        // Conservative default: 4GB
        4096
    }

    /// Estimates CPU performance score (0-100).
    fn estimate_cpu_performance() -> u32 {
        // Simplified - in reality would check CPU model, cores, frequency
        50
    }

    /// Detects if running on battery.
    fn detect_battery_status() -> bool {
        // In a real implementation, query power management
        // Default to false (on AC power)
        false
    }

    /// Applies hardware-appropriate default settings.
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

    // Getters
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

    // Setters with user override tracking
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
            if self.transparency_enabled { "On" } else { "Off" },
            if self.shadows_enabled { "On" } else { "Off" },
            if self.blur_enabled { "On" } else { "Off" },
            self.animation_speed,
            self.max_visible_items,
            if self.hardware_acceleration { "On" } else { "Off" },
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
}

impl Default for MenuConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Global menu configuration manager.
pub struct MenuConfigManager {
    config: MenuConfig,
    auto_adjust: bool,
}

impl MenuConfigManager {
    /// Creates a new configuration manager.
    pub fn new() -> Self {
        Self {
            config: MenuConfig::new(),
            auto_adjust: true,
        }
    }

    /// Gets the current configuration.
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }

    /// Gets mutable access to configuration.
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }

    /// Enables or disables automatic adjustment based on hardware changes.
    pub fn set_auto_adjust(&mut self, enabled: bool) {
        self.auto_adjust = enabled;
    }

    /// Returns true if auto-adjustment is enabled.
    pub fn auto_adjust(&self) -> bool {
        self.auto_adjust
    }

    /// Re-detects hardware and updates configuration if auto-adjust is enabled.
    pub fn refresh_hardware_detection(&mut self) {
        if self.auto_adjust && !self.config.has_user_overrides() {
            self.config = MenuConfig::new();
        }
    }

    /// Adapts settings for battery power (reduces effects).
    pub fn adapt_for_battery_power(&mut self) {
        if self.config.hardware_caps.on_battery && self.auto_adjust {
            // Reduce effects when on battery
            if self.config.user_overrides.animations.is_none() {
                self.config.animations_enabled = false;
            }
            if self.config.user_overrides.transparency.is_none() {
                self.config.transparency_enabled = false;
            }
            if self.config.user_overrides.blur.is_none() {
                self.config.blur_enabled = false;
            }
        }
    }
}

impl Default for MenuConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration persistence manager for saving/loading user preferences.
pub struct ConfigPersistence {
    config_dir: PathBuf,
}

impl ConfigPersistence {
    /// Creates a new persistence manager with default config directory.
    pub fn new() -> Self {
        let config_dir = Self::default_config_dir();
        Self { config_dir }
    }

    /// Creates a new persistence manager with custom config directory.
    pub fn with_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Returns the default configuration directory.
    fn default_config_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".config").join("rust-widgets")
    }

    /// Ensures the config directory exists.
    fn ensure_dir(&self) -> io::Result<()> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)?;
        }
        Ok(())
    }

    /// Returns the path to the menu config file.
    fn config_file_path(&self) -> PathBuf {
        self.config_dir.join("menu_config.json")
    }

    /// Saves menu configuration to disk.
    pub fn save(&self, config: &MenuConfig) -> io::Result<()> {
        self.ensure_dir()?;
        
        let mut data = HashMap::new();
        
        // Only save user overrides
        if let Some(animations) = config.user_overrides.animations {
            data.insert("animations_enabled".to_string(), animations.to_string());
        }
        if let Some(transparency) = config.user_overrides.transparency {
            data.insert("transparency_enabled".to_string(), transparency.to_string());
        }
        if let Some(shadows) = config.user_overrides.shadows {
            data.insert("shadows_enabled".to_string(), shadows.to_string());
        }
        if let Some(blur) = config.user_overrides.blur {
            data.insert("blur_enabled".to_string(), blur.to_string());
        }
        if let Some(speed) = config.user_overrides.animation_speed {
            data.insert("animation_speed".to_string(), speed.to_string());
        }
        if let Some(max_items) = config.user_overrides.max_visible_items {
            data.insert("max_visible_items".to_string(), max_items.to_string());
        }
        if let Some(hw_accel) = config.user_overrides.hardware_acceleration {
            data.insert("hardware_acceleration".to_string(), hw_accel.to_string());
        }

        // Serialize to simple key=value format
        let mut content = String::new();
        content.push_str("# Rust Widgets Menu Configuration\n");
        content.push_str("# This file contains user overrides for menu settings\n");
        content.push_str("# Delete this file to reset to hardware defaults\n\n");
        
        for (key, value) in &data {
            content.push_str(&format!("{}={}\n", key, value));
        }

        let path = self.config_file_path();
        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        
        Ok(())
    }

    /// Loads menu configuration from disk.
    pub fn load(&self) -> io::Result<UserOverrides> {
        let path = self.config_file_path();
        
        if !path.exists() {
            return Ok(UserOverrides::default());
        }

        let mut file = fs::File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let mut overrides = UserOverrides::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "animations_enabled" => {
                        overrides.animations = value.parse().ok();
                    }
                    "transparency_enabled" => {
                        overrides.transparency = value.parse().ok();
                    }
                    "shadows_enabled" => {
                        overrides.shadows = value.parse().ok();
                    }
                    "blur_enabled" => {
                        overrides.blur = value.parse().ok();
                    }
                    "animation_speed" => {
                        overrides.animation_speed = value.parse().ok();
                    }
                    "max_visible_items" => {
                        overrides.max_visible_items = value.parse().ok();
                    }
                    "hardware_acceleration" => {
                        overrides.hardware_acceleration = value.parse().ok();
                    }
                    _ => {}
                }
            }
        }

        Ok(overrides)
    }

    /// Deletes the saved configuration file.
    pub fn clear(&self) -> io::Result<()> {
        let path = self.config_file_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Checks if a saved configuration exists.
    pub fn exists(&self) -> bool {
        self.config_file_path().exists()
    }
}

impl Default for ConfigPersistence {
    fn default() -> Self {
        Self::new()
    }
}

/// Menu configuration dialog for user preferences.
pub struct MenuConfigDialog {
    config: MenuConfig,
    persistence: ConfigPersistence,
}

impl MenuConfigDialog {
    /// Creates a new configuration dialog.
    pub fn new(config: MenuConfig) -> Self {
        Self {
            config,
            persistence: ConfigPersistence::new(),
        }
    }

    /// Creates a dialog with custom persistence.
    pub fn with_persistence(config: MenuConfig, persistence: ConfigPersistence) -> Self {
        Self {
            config,
            persistence,
        }
    }

    /// Gets the current configuration.
    pub fn config(&self) -> &MenuConfig {
        &self.config
    }

    /// Gets mutable configuration.
    pub fn config_mut(&mut self) -> &mut MenuConfig {
        &mut self.config
    }

    /// Toggles animations on/off.
    pub fn toggle_animations(&mut self) {
        let new_value = !self.config.animations_enabled();
        self.config.set_animations_enabled(new_value);
    }

    /// Toggles transparency on/off.
    pub fn toggle_transparency(&mut self) {
        let new_value = !self.config.transparency_enabled();
        self.config.set_transparency_enabled(new_value);
    }

    /// Toggles shadows on/off.
    pub fn toggle_shadows(&mut self) {
        let new_value = !self.config.shadows_enabled();
        self.config.set_shadows_enabled(new_value);
    }

    /// Toggles blur on/off.
    pub fn toggle_blur(&mut self) {
        let new_value = !self.config.blur_enabled();
        self.config.set_blur_enabled(new_value);
    }

    /// Increases animation speed.
    pub fn increase_animation_speed(&mut self) {
        let current = self.config.animation_speed();
        let new_speed = (current + 0.1).min(3.0);
        self.config.set_animation_speed(new_speed);
    }

    /// Decreases animation speed.
    pub fn decrease_animation_speed(&mut self) {
        let current = self.config.animation_speed();
        let new_speed = (current - 0.1).max(0.1);
        self.config.set_animation_speed(new_speed);
    }

    /// Increases max visible items.
    pub fn increase_max_items(&mut self) {
        let current = self.config.max_visible_items();
        self.config.set_max_visible_items(current + 5);
    }

    /// Decreases max visible items.
    pub fn decrease_max_items(&mut self) {
        let current = self.config.max_visible_items();
        if current > 5 {
            self.config.set_max_visible_items(current - 5);
        }
    }

    /// Resets all settings to hardware defaults.
    pub fn reset_to_defaults(&mut self) {
        self.config.reset_to_defaults();
    }

    /// Saves the current configuration.
    pub fn save(&self) -> io::Result<()> {
        self.persistence.save(&self.config)
    }

    /// Loads configuration from disk.
    pub fn load(&mut self) -> io::Result<()> {
        let overrides = self.persistence.load()?;
        self.config.user_overrides = overrides;
        self.config.apply_user_overrides();
        Ok(())
    }

    /// Returns a summary of current settings for display.
    pub fn settings_summary(&self) -> String {
        self.config.settings_summary()
    }

    /// Returns true if any settings have been overridden.
    pub fn has_overrides(&self) -> bool {
        self.config.has_user_overrides()
    }

    /// Returns the hardware-detected performance level.
    pub fn hardware_performance_level(&self) -> PerformanceLevel {
        self.config.hardware_caps.performance_level
    }

    /// Returns a description of the detected GPU.
    pub fn gpu_description(&self) -> String {
        format!(
            "{} ({} MB)",
            self.config.hardware_caps.gpu_type.description(),
            self.config.hardware_caps.gpu_memory_mb
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_config_default() {
        let config = MenuConfig::new();
        
        // Should have detected hardware
        assert!(config.animation_speed() > 0.0);
        assert!(config.max_visible_items() >= 5);
        
        // Should not have user overrides initially
        assert!(!config.has_user_overrides());
    }

    #[test]
    fn test_user_overrides() {
        let mut config = MenuConfig::new();
        
        // Apply user overrides
        config.set_animations_enabled(false);
        config.set_transparency_enabled(true);
        config.set_animation_speed(1.5);
        
        assert!(config.has_user_overrides());
        assert!(!config.animations_enabled());
        assert!(config.transparency_enabled());
        assert_eq!(config.animation_speed(), 1.5);
        
        // Check overrides are tracked
        assert_eq!(config.user_overrides.animations, Some(false));
        assert_eq!(config.user_overrides.transparency, Some(true));
    }

    #[test]
    fn test_reset_to_defaults() {
        let mut config = MenuConfig::new();
        
        // Store original values
        let original_animations = config.animations_enabled();
        
        // Apply override
        config.set_animations_enabled(!original_animations);
        assert!(config.has_user_overrides());
        
        // Reset
        config.reset_to_defaults();
        
        assert!(!config.has_user_overrides());
        assert_eq!(config.animations_enabled(), original_animations);
    }

    #[test]
    fn test_animation_speed_clamping() {
        let mut config = MenuConfig::new();
        
        config.set_animation_speed(0.05); // Too low
        assert_eq!(config.animation_speed(), 0.1);
        
        config.set_animation_speed(5.0); // Too high
        assert_eq!(config.animation_speed(), 3.0);
        
        config.set_animation_speed(1.5); // Valid
        assert_eq!(config.animation_speed(), 1.5);
    }

    #[test]
    fn test_max_visible_items_minimum() {
        let mut config = MenuConfig::new();
        
        config.set_max_visible_items(2); // Too low
        assert_eq!(config.max_visible_items(), 5);
        
        config.set_max_visible_items(30); // Valid
        assert_eq!(config.max_visible_items(), 30);
    }

    #[test]
    fn test_settings_summary() {
        let config = MenuConfig::new();
        let summary = config.settings_summary();
        
        assert!(summary.contains("Menu Settings:"));
        assert!(summary.contains("Animations:"));
        assert!(summary.contains("Transparency:"));
        assert!(summary.contains("Performance Level:"));
    }

    #[test]
    fn test_config_manager() {
        let mut manager = MenuConfigManager::new();
        
        assert!(manager.auto_adjust());
        
        manager.set_auto_adjust(false);
        assert!(!manager.auto_adjust());
        
        // Should be able to access config
        let _ = manager.config().animations_enabled();
    }

    #[test]
    fn test_config_persistence_roundtrip() {
        use std::env;
        
        // Create a temporary directory for testing
        let temp_dir = env::temp_dir().join("rust-widgets-test");
        let persistence = ConfigPersistence::with_dir(temp_dir.clone());
        
        // Clear any existing config
        let _ = persistence.clear();
        
        // Create config with overrides
        let mut config = MenuConfig::new();
        config.set_animations_enabled(false);
        config.set_transparency_enabled(true);
        config.set_animation_speed(1.5);
        
        // Save
        persistence.save(&config).unwrap();
        assert!(persistence.exists());
        
        // Load
        let overrides = persistence.load().unwrap();
        assert_eq!(overrides.animations, Some(false));
        assert_eq!(overrides.transparency, Some(true));
        assert_eq!(overrides.animation_speed, Some(1.5));
        
        // Cleanup
        let _ = persistence.clear();
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_config_dialog() {
        let config = MenuConfig::new();
        let mut dialog = MenuConfigDialog::new(config);
        
        // Test toggles
        let initial_animations = dialog.config().animations_enabled();
        dialog.toggle_animations();
        assert_eq!(dialog.config().animations_enabled(), !initial_animations);
        
        // Test speed adjustment
        let initial_speed = dialog.config().animation_speed();
        dialog.increase_animation_speed();
        assert!(dialog.config().animation_speed() > initial_speed);
        
        dialog.decrease_animation_speed();
        assert!(dialog.config().animation_speed() <= initial_speed + 0.1);
        
        // Test max items adjustment
        let initial_max = dialog.config().max_visible_items();
        dialog.increase_max_items();
        assert_eq!(dialog.config().max_visible_items(), initial_max + 5);
        
        dialog.decrease_max_items();
        assert_eq!(dialog.config().max_visible_items(), initial_max);
        
        // Test reset
        dialog.reset_to_defaults();
        assert!(!dialog.has_overrides());
        
        // Test summary
        let summary = dialog.settings_summary();
        assert!(summary.contains("Menu Settings:"));
        assert!(summary.contains("Performance Level:"));
        
        // Test GPU description
        let gpu_desc = dialog.gpu_description();
        assert!(!gpu_desc.is_empty());
    }

    #[test]
    fn test_performance_level_enum() {
        assert_ne!(PerformanceLevel::Low, PerformanceLevel::Medium);
        assert_ne!(PerformanceLevel::Medium, PerformanceLevel::High);
        
        // Test default
        assert!(matches!(PerformanceLevel::default(), PerformanceLevel::Medium));
    }

    #[test]
    fn test_hardware_capabilities() {
        let caps = HardwareCapabilities {
            gpu_type: GpuType::Discrete,
            gpu_memory_mb: 4096,
            gpu_performance_score: 80,
            system_ram_mb: 16384,
            cpu_performance_score: 70,
            on_battery: false,
            performance_level: PerformanceLevel::High,
        };
        
        assert!(matches!(caps.gpu_type, GpuType::Discrete));
        assert_eq!(caps.gpu_memory_mb, 4096);
        assert!(!caps.on_battery);
        assert!(matches!(caps.performance_level, PerformanceLevel::High));
    }
}
