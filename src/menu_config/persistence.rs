// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{MenuConfig, UserOverrides};
use crate::compat::HashMap;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
/// Configuration persistence manager for saving/loading user preferences.
pub struct ConfigPersistence {
    config_dir: PathBuf,
}

/// Error returned when loading a menu configuration file fails.
///
/// A parse failure on a **known** field is reported with the field name and its
/// line number, so a corrupt or hand-edited file never silently drops a preference.
/// Unknown keys are deliberately ignored (a separate, extension-friendly policy).
#[derive(Debug)]
pub enum ConfigLoadError {
    /// The file could not be opened or read.
    Io(io::Error),
    /// A known field held a value that did not parse.
    Parse {
        /// 1-based line number where the bad field appeared.
        line: usize,
        /// The field name that failed to parse.
        field: String,
        /// The offending value.
        value: String,
    },
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigLoadError::Io(err) => write!(f, "failed to read menu config: {err}"),
            ConfigLoadError::Parse { line, field, value } => {
                write!(f, "menu config line {line}: field `{field}` has invalid value `{value}`")
            }
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigLoadError::Io(err) => Some(err),
            ConfigLoadError::Parse { .. } => None,
        }
    }
}

impl From<io::Error> for ConfigLoadError {
    fn from(err: io::Error) -> Self {
        ConfigLoadError::Io(err)
    }
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

    /// The directory this manager reads and writes `menu_config.json` in.
    ///
    /// Exposed so hosts can display or log where settings live; it is also what
    /// lets the platform-convention test verify the path without touching disk.
    pub fn config_dir(&self) -> &std::path::Path {
        &self.config_dir
    }
    /// Resolves the per-user config directory for this application.
    ///
    /// Uses [`dirs::config_dir`] rather than appending `.config` to the home
    /// directory by hand: the correct location is OS-specific (macOS
    /// `~/Library/Application Support`, Windows `%APPDATA%`, Linux
    /// `$XDG_CONFIG_HOME` or `~/.config`), and hand-rolling the Linux convention
    /// would scatter this app's config into the wrong place on the other two
    /// platforms. The OS knowledge stays inside the `dirs` crate (principle #36).
    fn default_config_dir() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("rust-widgets")
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
            content.push_str(&format!("{key}={value}\n"));
        }
        let path = self.config_file_path();
        // Write to a temporary file in the same directory, then rename over the target.
        //
        // `File::create(path)` truncates the existing file *before* the write, so an IO error
        // mid-write (disk full, quota, a permissions change) left the previous configuration
        // replaced by an empty or half-written file — the user lost their settings and gained a
        // corrupt one. `rename` within the same directory is atomic on every supported platform, so
        // the target is either the old content or the complete new content, never a torn mix. The
        // temp file is removed on any failure so a failed save leaves no stray file behind.
        let temp_path = path.with_extension("tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            if let Err(error) = file.write_all(content.as_bytes()).and_then(|()| file.sync_all()) {
                drop(file);
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        }
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        Ok(())
    }
    /// Loads menu configuration from disk.
    ///
    /// A known field that cannot be parsed is an explicit [`ConfigLoadError::Parse`]
    /// (with the field and line number) rather than a silently dropped `None`; a whole
    /// file that fails to parse is rejected so the caller can keep its previous
    /// configuration. Unknown keys are ignored independently of that policy.
    pub fn load(&self) -> Result<UserOverrides, ConfigLoadError> {
        let path = self.config_file_path();
        if !path.exists() {
            return Ok(UserOverrides::default());
        }
        let mut file = fs::File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let mut overrides = UserOverrides::default();
        for (index, raw_line) in content.lines().enumerate() {
            let line_number = index + 1;
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "animations_enabled" => {
                        overrides.animations = Some(Self::parse_field(key, value, line_number)?)
                    }
                    "transparency_enabled" => {
                        overrides.transparency = Some(Self::parse_field(key, value, line_number)?)
                    }
                    "shadows_enabled" => {
                        overrides.shadows = Some(Self::parse_field(key, value, line_number)?)
                    }
                    "blur_enabled" => {
                        overrides.blur = Some(Self::parse_field(key, value, line_number)?)
                    }
                    "animation_speed" => {
                        overrides.animation_speed =
                            Some(Self::parse_field(key, value, line_number)?)
                    }
                    "max_visible_items" => {
                        overrides.max_visible_items =
                            Some(Self::parse_field(key, value, line_number)?)
                    }
                    "hardware_acceleration" => {
                        overrides.hardware_acceleration =
                            Some(Self::parse_field(key, value, line_number)?)
                    }
                    _ => { /* Unknown key: ignored so files can carry forward extensions. */ }
                }
            }
        }
        Ok(overrides)
    }

    /// Parses a known field, turning a parse failure into an error that names the
    /// field and its line.
    fn parse_field<T: std::str::FromStr>(
        field: &str,
        value: &str,
        line: usize,
    ) -> Result<T, ConfigLoadError> {
        value.parse::<T>().map_err(|_| ConfigLoadError::Parse {
            line,
            field: field.to_string(),
            value: value.to_string(),
        })
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
crate::impl_default_via_new!(ConfigPersistence);
