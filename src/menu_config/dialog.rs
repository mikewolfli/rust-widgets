use std::io;

use super::{ConfigPersistence, MenuConfig, PerformanceLevel};

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
        self.config.hardware_caps().performance_level
    }

    /// Returns a description of the detected GPU.
    pub fn gpu_description(&self) -> String {
        format!(
            "{} ({} MB)",
            self.config.hardware_caps().gpu_type.description(),
            self.config.hardware_caps().gpu_memory_mb
        )
    }
}
