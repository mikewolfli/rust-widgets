use super::{MenuConfig, UserOverrides};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
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
    fn default_config_dir() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".config").join("rust-widgets")
    }
    fn ensure_dir(&self) -> io::Result<()> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)?;
        }
        Ok(())
    }
    fn config_file_path(&self) -> PathBuf {
        self.config_dir.join("menu_config.json")
    }
    /// Saves menu configuration to disk.
    pub fn save(&self, config: &MenuConfig) -> io::Result<()> {
        self.ensure_dir()?;
        let mut data = HashMap::new();
        if let Some(animations) = config.user_overrides().animations {
            data.insert("animations_enabled".to_string(), animations.to_string());
        }
        if let Some(transparency) = config.user_overrides().transparency {
            data.insert("transparency_enabled".to_string(), transparency.to_string());
        }
        if let Some(shadows) = config.user_overrides().shadows {
            data.insert("shadows_enabled".to_string(), shadows.to_string());
        }
        if let Some(blur) = config.user_overrides().blur {
            data.insert("blur_enabled".to_string(), blur.to_string());
        }
        if let Some(speed) = config.user_overrides().animation_speed {
            data.insert("animation_speed".to_string(), speed.to_string());
        }
        if let Some(max_items) = config.user_overrides().max_visible_items {
            data.insert("max_visible_items".to_string(), max_items.to_string());
        }
        if let Some(hw_accel) = config.user_overrides().hardware_acceleration {
            data.insert("hardware_acceleration".to_string(), hw_accel.to_string());
        }
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
                    "animations_enabled" => overrides.animations = value.parse().ok(),
                    "transparency_enabled" => overrides.transparency = value.parse().ok(),
                    "shadows_enabled" => overrides.shadows = value.parse().ok(),
                    "blur_enabled" => overrides.blur = value.parse().ok(),
                    "animation_speed" => overrides.animation_speed = value.parse().ok(),
                    "max_visible_items" => overrides.max_visible_items = value.parse().ok(),
                    "hardware_acceleration" => overrides.hardware_acceleration = value.parse().ok(),
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
