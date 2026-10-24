// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Persistent application settings — the `settings store` equivalent.
//!
//! Controls do **not** save themselves: choosing what state is worth persisting (window
//! geometry, splitter weights, a selected tab) is a product decision, and runtime object ids
//! are not stable across sessions. A host therefore reads a value before showing a control and
//! writes it back when the application exits, exactly as the reference toolkit's explicit `saveState` call sites do.
//!
//! The key model is flat and slash-separated (`window/main/geometry`), following the 参考工具包 group
//! convention. [`Settings::window_geometry`] and [`Settings::split_ratios`] are typed helpers
//! over that model; they encode their payloads as strings so the on-disk format stays readable
//! and needs no per-type JSON schema.
//!
//! # Gating
//!
//! This module is compiled with the same profile gate as the existing
//! [`crate::menu_config::ConfigPersistence`] precedent (`advanced-widgets`), and additionally
//! requires `serde_json` because the on-disk format is JSON. It is absent from `alloc_frugal`
//! (`mini`/`embedded`) builds, where `std::fs` is not part of the target.
//!
//! # Reachability
//!
//! **State:** Reserved: public host API with no internal caller by design. The decision recorded
//! in the plan (C25=A) is that a host reads a value before showing a control and writes it back on
//! exit; a control that saved itself would need stable ids across sessions and would make every
//! persisted byte a versioned contract. [`Settings`] is therefore reached through the crate root
//! re-export (`crate::Settings`), not from the widget layer.

use crate::compat::{Box, HashMap, String, Vec};
use crate::core::Rect;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// File name written under `config_dir/{org}/{app}/`.
const SETTINGS_FILE_NAME: &str = "settings.json";

/// One value stored in a [`SettingsStore`].
///
/// The variant is part of the persisted format: [`JsonSettings`] writes a type tag beside the
/// payload so an `I64` read back is an `I64` rather than a `F64`, and a `Bytes` value is not
/// confused with a `String`.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    /// A boolean flag.
    Bool(bool),
    /// A signed 64-bit integer.
    I64(i64),
    /// A 64-bit floating-point number.
    F64(f64),
    /// A UTF-8 string.
    String(crate::compat::String),
    /// An opaque byte payload.
    Bytes(Vec<u8>),
}

/// Why a [`SettingsStore`] could not persist its values.
#[derive(Debug)]
pub enum SettingsError {
    /// The settings file could not be created, written, or renamed.
    Io(std::io::Error),
    /// The in-memory map could not be serialized to JSON.
    Json(serde_json::Error),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Io(error) => write!(f, "settings I/O error: {error}"),
            SettingsError::Json(error) => write!(f, "settings JSON error: {error}"),
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SettingsError::Io(error) => Some(error),
            SettingsError::Json(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for SettingsError {
    fn from(error: std::io::Error) -> Self {
        SettingsError::Io(error)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(error: serde_json::Error) -> Self {
        SettingsError::Json(error)
    }
}

/// A key/value backend behind [`Settings`].
///
/// Implementations hold the values in memory and persist them on [`Self::flush`]. `write` is
/// therefore cheap and may be called while building a UI; a host calls `flush` once at shutdown
/// (or after a change it wants durable immediately).
pub trait SettingsStore {
    /// Reads the current value for `key`, if one has been written.
    fn read(&self, key: &str) -> Option<SettingValue>;

    /// Writes `value` under `key`, replacing any previous value.
    fn write(&mut self, key: &str, value: SettingValue);

    /// Persists every value. Backends without durable storage return `Ok(())`.
    fn flush(&mut self) -> Result<(), SettingsError>;
}

/// The application-facing settings handle.
///
/// [`Settings::open`] creates the JSON backend under the platform config directory;
/// [`Settings::with_store`] injects any backend, such as [`MemorySettings`] in tests.
pub struct Settings {
    store: Box<dyn SettingsStore>,
}

impl Settings {
    /// Opens (or creates) `config_dir/{org}/{app}/settings.json`.
    ///
    /// The directory is created when the first [`Settings::flush`] writes the file. A missing or
    /// corrupt file is not an error: the store starts empty, so an application always has usable
    /// defaults.
    pub fn open(org: &str, app: &str) -> Self {
        Self::with_store(Box::new(JsonSettings::new(org, app)))
    }

    /// Builds a settings handle over an injected backend.
    pub fn with_store(store: Box<dyn SettingsStore>) -> Self {
        Self { store }
    }

    /// Reads a raw value by key.
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        self.store.read(key)
    }

    /// Writes a raw value by key. Nothing reaches disk until [`Self::flush`].
    pub fn set(&mut self, key: &str, value: SettingValue) {
        self.store.write(key, value);
    }

    /// Persists every pending write through the backend.
    pub fn flush(&mut self) -> Result<(), SettingsError> {
        self.store.flush()
    }

    /// Reads the saved geometry for a named window.
    ///
    /// The key is `window/{name}/geometry`.
    pub fn window_geometry(&self, name: &str) -> Option<Rect> {
        let key = geometry_key(name);
        match self.store.read(&key)? {
            SettingValue::String(text) => decode_rect(&text),
            _ => None,
        }
    }

    /// Saves the geometry of a named window.
    pub fn set_window_geometry(&mut self, name: &str, rect: Rect) {
        self.store.write(&geometry_key(name), SettingValue::String(encode_rect(rect)));
    }

    /// Reads the saved pane weights for a named splitter.
    ///
    /// The key is `splitter/{name}/ratios`. The values are the splitter's relative weights, in
    /// pane order — the same representation a `Splitter`'s `ratios()` returns.
    pub fn split_ratios(&self, name: &str) -> Option<Vec<f32>> {
        let key = ratios_key(name);
        match self.store.read(&key)? {
            SettingValue::String(text) => decode_ratios(&text),
            _ => None,
        }
    }

    /// Saves the pane weights of a named splitter.
    pub fn set_split_ratios(&mut self, name: &str, ratios: &[f32]) {
        self.store.write(&ratios_key(name), SettingValue::String(encode_ratios(ratios)));
    }
}

/// An in-memory [`SettingsStore`] for tests and embedded hosts that manage persistence themselves.
#[derive(Debug, Clone, Default)]
pub struct MemorySettings {
    values: HashMap<String, SettingValue>,
}

impl MemorySettings {
    /// Creates an empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SettingsStore for MemorySettings {
    fn read(&self, key: &str) -> Option<SettingValue> {
        self.values.get(key).cloned()
    }

    fn write(&mut self, key: &str, value: SettingValue) {
        self.values.insert(key.to_string(), value);
    }

    fn flush(&mut self) -> Result<(), SettingsError> {
        Ok(())
    }
}

/// A JSON-file [`SettingsStore`].
///
/// Values are decoded once when the store is created, so reads never touch the filesystem. The
/// file is written atomically — to a sibling temporary file, then renamed over the target — which
/// is the same protection [`crate::menu_config::ConfigPersistence`] uses: an interrupted write
/// leaves the previous settings intact rather than a half-written file.
pub struct JsonSettings {
    path: PathBuf,
    values: HashMap<String, SettingValue>,
}

impl JsonSettings {
    /// Creates a store at `config_dir/{org}/{app}/settings.json`.
    ///
    /// A missing or unreadable file yields an empty store; only an explicit [`Self::flush`]
    /// reports I/O failures.
    pub fn new(org: &str, app: &str) -> Self {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let path = base.join(org).join(app).join(SETTINGS_FILE_NAME);
        Self::at_path(path)
    }

    /// Creates a store at `dir/settings.json`.
    ///
    /// Exposed primarily so tests and hosts can use a temporary or application-chosen directory
    /// without touching the platform config location, mirroring
    /// [`crate::menu_config::ConfigPersistence::with_dir`].
    pub fn with_dir(dir: PathBuf) -> Self {
        Self::at_path(dir.join(SETTINGS_FILE_NAME))
    }

    /// Creates a store backed by an explicit file path.
    pub fn at_path(path: PathBuf) -> Self {
        let values = Self::load(&path);
        Self { path, values }
    }

    /// The file this store reads and writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads and decodes the file, returning an empty map on any failure.
    fn load(path: &Path) -> HashMap<String, SettingValue> {
        let Ok(content) = fs::read_to_string(path) else {
            return HashMap::new();
        };
        let Ok(serde_json::Value::Object(object)) = serde_json::from_str(&content) else {
            return HashMap::new();
        };
        let mut values = HashMap::new();
        for (key, raw) in object {
            if let Some(value) = value_from_json(&raw) {
                values.insert(key, value);
            }
        }
        values
    }
}

impl SettingsStore for JsonSettings {
    fn read(&self, key: &str) -> Option<SettingValue> {
        self.values.get(key).cloned()
    }

    fn write(&mut self, key: &str, value: SettingValue) {
        self.values.insert(key.to_string(), value);
    }

    fn flush(&mut self) -> Result<(), SettingsError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut object = serde_json::Map::new();
        for (key, value) in &self.values {
            object.insert(key.clone(), value_to_json(value));
        }
        let content = serde_json::to_string_pretty(&serde_json::Value::Object(object))?;

        // The temporary file lives beside the target so the rename stays on one filesystem and
        // is atomic. Any failure removes it, leaving the old settings file untouched.
        let temp_path = self.path.with_extension("tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            if let Err(error) = file.write_all(content.as_bytes()).and_then(|()| file.sync_all()) {
                drop(file);
                let _ = fs::remove_file(&temp_path);
                return Err(SettingsError::Io(error));
            }
        }
        if let Err(error) = fs::rename(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(SettingsError::Io(error));
        }
        Ok(())
    }
}

/// Builds the JSON object for one value, preserving its variant in a `type` tag.
fn value_to_json(value: &SettingValue) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    match value {
        SettingValue::Bool(flag) => {
            object.insert("type".to_string(), serde_json::Value::String("bool".to_string()));
            object.insert("value".to_string(), serde_json::Value::Bool(*flag));
        }
        SettingValue::I64(number) => {
            object.insert("type".to_string(), serde_json::Value::String("i64".to_string()));
            object.insert("value".to_string(), serde_json::Value::Number((*number).into()));
        }
        SettingValue::F64(number) => {
            object.insert("type".to_string(), serde_json::Value::String("f64".to_string()));
            match serde_json::Number::from_f64(*number) {
                Some(json_number) => {
                    object.insert("value".to_string(), serde_json::Value::Number(json_number));
                }
                // JSON has no NaN or infinity literal; store it as the string the decoder
                // understands rather than silently changing the value to zero.
                None => {
                    object
                        .insert("value".to_string(), serde_json::Value::String(number.to_string()));
                }
            }
        }
        SettingValue::String(text) => {
            object.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            object.insert("value".to_string(), serde_json::Value::String(text.clone()));
        }
        SettingValue::Bytes(bytes) => {
            object.insert("type".to_string(), serde_json::Value::String("bytes".to_string()));
            let items =
                bytes.iter().map(|byte| serde_json::Value::Number((*byte).into())).collect();
            object.insert("value".to_string(), serde_json::Value::Array(items));
        }
    }
    serde_json::Value::Object(object)
}

/// Decodes one tagged value; `None` means the entry is malformed and is skipped.
fn value_from_json(value: &serde_json::Value) -> Option<SettingValue> {
    let object = value.as_object()?;
    let tag = object.get("type")?.as_str()?;
    let raw = object.get("value")?;
    match tag {
        "bool" => raw.as_bool().map(SettingValue::Bool),
        "i64" => raw.as_i64().map(SettingValue::I64),
        "f64" => match raw {
            serde_json::Value::Number(_) => raw.as_f64().map(SettingValue::F64),
            serde_json::Value::String(text) => text.parse::<f64>().ok().map(SettingValue::F64),
            _ => None,
        },
        "string" => raw.as_str().map(|text| SettingValue::String(text.to_string())),
        "bytes" => {
            let items = raw.as_array()?;
            let mut bytes = Vec::with_capacity(items.len());
            for item in items {
                bytes.push(u8::try_from(item.as_u64()?).ok()?);
            }
            Some(SettingValue::Bytes(bytes))
        }
        _ => None,
    }
}

/// The storage key for a window's geometry.
fn geometry_key(name: &str) -> String {
    format!("window/{name}/geometry")
}

/// The storage key for a splitter's weights.
fn ratios_key(name: &str) -> String {
    format!("splitter/{name}/ratios")
}

/// Encodes a rectangle as `x,y,width,height`.
fn encode_rect(rect: Rect) -> String {
    format!("{},{},{},{}", rect.x, rect.y, rect.width, rect.height)
}

/// Decodes `x,y,width,height`; anything else is `None`.
fn decode_rect(text: &str) -> Option<Rect> {
    let mut parts = text.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    let width = parts.next()?.trim().parse::<u32>().ok()?;
    let height = parts.next()?.trim().parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(Rect::new(x, y, width, height))
}

/// Encodes pane weights as a comma-separated list.
fn encode_ratios(ratios: &[f32]) -> String {
    let mut text = String::new();
    for (index, ratio) in ratios.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&ratio.to_string());
    }
    text
}

/// Decodes a comma-separated weight list. The empty string is an empty vector.
fn decode_ratios(text: &str) -> Option<Vec<f32>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Some(Vec::new());
    }
    let mut ratios = Vec::new();
    for part in trimmed.split(',') {
        ratios.push(part.trim().parse::<f32>().ok()?);
    }
    Some(ratios)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique directory under the OS temp directory, removed by the caller.
    fn temp_dir(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir()
            .join(format!("rust-widgets-settings-{tag}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&dir).expect("the temporary directory is creatable");
        dir
    }

    #[test]
    fn memory_store_round_trips_window_geometry() {
        let mut settings = Settings::with_store(Box::new(MemorySettings::new()));
        let rect = Rect::new(12, 34, 800, 600);

        assert_eq!(settings.window_geometry("main"), None);
        settings.set_window_geometry("main", rect);
        assert_eq!(settings.window_geometry("main"), Some(rect));
        assert_eq!(
            settings.get("window/main/geometry"),
            Some(SettingValue::String("12,34,800,600".to_string()))
        );
        assert!(settings.flush().is_ok(), "the memory backend always flushes");
    }

    #[test]
    fn memory_store_round_trips_split_ratios() {
        let mut settings = Settings::with_store(Box::new(MemorySettings::new()));
        let ratios = [0.25f32, 0.5, 0.25];

        assert_eq!(settings.split_ratios("editor"), None);
        settings.set_split_ratios("editor", &ratios);
        assert_eq!(settings.split_ratios("editor"), Some(ratios.to_vec()));
        assert_eq!(
            settings.get("splitter/editor/ratios"),
            Some(SettingValue::String("0.25,0.5,0.25".to_string()))
        );
    }

    #[test]
    fn json_settings_writes_and_rereads_identically() {
        let dir = temp_dir("json-round-trip");
        let path = dir.join(SETTINGS_FILE_NAME);
        let rect = Rect::new(-10, 20, 1024, 768);
        let ratios = [0.2f32, 0.3, 0.5];

        {
            let mut settings = Settings::with_store(Box::new(JsonSettings::with_dir(dir.clone())));
            settings.set_window_geometry("main", rect);
            settings.set_split_ratios("main", &ratios);
            settings.set("ui/theme/dark", SettingValue::Bool(true));
            settings.set("app/version", SettingValue::I64(7));
            settings.set("app/scale", SettingValue::F64(1.25));
            settings.set("app/label", SettingValue::String("Widgets".to_string()));
            settings.set("app/blob", SettingValue::Bytes(vec![0, 1, 255]));
            settings.flush().expect("settings are written");
        }

        assert!(path.exists(), "flush creates the settings file");

        let settings = Settings::with_store(Box::new(JsonSettings::with_dir(dir.clone())));
        assert_eq!(settings.window_geometry("main"), Some(rect));
        assert_eq!(settings.split_ratios("main"), Some(ratios.to_vec()));
        assert_eq!(settings.get("ui/theme/dark"), Some(SettingValue::Bool(true)));
        assert_eq!(settings.get("app/version"), Some(SettingValue::I64(7)));
        assert_eq!(settings.get("app/scale"), Some(SettingValue::F64(1.25)));
        assert_eq!(settings.get("app/label"), Some(SettingValue::String("Widgets".to_string())));
        assert_eq!(settings.get("app/blob"), Some(SettingValue::Bytes(vec![0, 1, 255])));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_file_starts_empty_and_flush_replaces_it() {
        let dir = temp_dir("corrupt");
        let path = dir.join(SETTINGS_FILE_NAME);
        fs::write(&path, b"{ this is not json").expect("the corrupt fixture is writable");

        let mut settings = Settings::with_store(Box::new(JsonSettings::with_dir(dir.clone())));
        assert_eq!(settings.get("ui/theme"), None, "a corrupt file starts empty");

        settings.set("ui/theme", SettingValue::String("dark".to_string()));
        settings.flush().expect("a corrupt file is replaced on flush");

        let reread = Settings::with_store(Box::new(JsonSettings::with_dir(dir.clone())));
        assert_eq!(reread.get("ui/theme"), Some(SettingValue::String("dark".to_string())));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_path_follows_the_org_app_convention() {
        let expected = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("acme")
            .join("widgets")
            .join(SETTINGS_FILE_NAME);
        let store = JsonSettings::new("acme", "widgets");
        assert_eq!(store.path(), expected);
    }
}
