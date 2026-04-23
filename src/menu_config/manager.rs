use super::MenuConfig;
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
        if self.config.hardware_caps().on_battery && self.auto_adjust {
            self.config.apply_battery_adaptive_reduction();
        }
    }
}
impl Default for MenuConfigManager {
    fn default() -> Self {
        Self::new()
    }
}
