//! Central widget registry — maps `ObjectId` to widget metadata.
//!
//! This is the "index" that allows any part of the system to look up
//! a widget by its raw `ObjectId` at runtime.

use std::collections::HashMap;

#[cfg(not(feature = "mini"))]
use serde::{Deserialize, Serialize};

use crate::core::ObjectId;

/// Kinds of widgets tracked by the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
pub enum WidgetKind {
    Window,
    Button,
    Label,
    CheckBox,
    RadioButton,
    LineEdit,
    TextEdit,
    ComboBox,
    ListBox,
    Slider,
    ScrollBar,
    ProgressBar,
    Panel,
    TabWidget,
    GridWidget,
    Frame,
    Dialog,
    SpinBox,
    ListView,
    ScrollArea,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
}

/// Metadata stored for each registered widget.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
pub struct WidgetEntry {
    /// The widget's unique identifier.
    pub id: ObjectId,
    /// The kind of widget.
    pub kind: WidgetKind,
    /// Optional parent [`ObjectId`].
    pub parent: Option<ObjectId>,
    /// Human-readable label (mirrors widget text).
    pub label: String,
}

/// A bi-directional registry that maps `ObjectId` ↔ widget metadata.
///
/// This is the runtime "index" for all widgets created through the
/// `app` module or the raw `create_*` functions.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "mini"), derive(Serialize, Deserialize))]
pub struct WidgetRegistry {
    entries: HashMap<ObjectId, WidgetEntry>,
    by_kind: HashMap<WidgetKind, Vec<ObjectId>>,
}

impl WidgetRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { entries: HashMap::new(), by_kind: HashMap::new() }
    }

    /// Register (or update) a widget entry.
    pub fn register(&mut self, entry: WidgetEntry) {
        let kind = entry.kind;
        let id = entry.id;

        // Remove old entry for this ID if it exists
        if let Some(old) = self.entries.get(&id) {
            if let Some(list) = self.by_kind.get_mut(&old.kind) {
                list.retain(|&x| x != id);
            }
        }

        self.by_kind.entry(kind).or_default().push(id);
        self.entries.insert(id, entry);
    }

    /// Unregister a widget by its [`ObjectId`].
    pub fn unregister(&mut self, id: ObjectId) {
        if let Some(entry) = self.entries.remove(&id) {
            if let Some(list) = self.by_kind.get_mut(&entry.kind) {
                list.retain(|&x| x != id);
            }
        }
    }

    /// Look up a widget entry by [`ObjectId`].
    pub fn get(&self, id: ObjectId) -> Option<&WidgetEntry> {
        self.entries.get(&id)
    }

    /// Return all [`ObjectId`]s of a given [`WidgetKind`].
    pub fn find_by_kind(&self, kind: WidgetKind) -> &[ObjectId] {
        self.by_kind.get(&kind).map_or(&[], |v| v.as_slice())
    }

    /// Return all entries whose parent matches the given [`ObjectId`].
    pub fn children_of(&self, parent_id: ObjectId) -> Vec<&WidgetEntry> {
        self.entries.values().filter(|entry| entry.parent == Some(parent_id)).collect()
    }

    /// Serialize the registry to a JSON file at `path`.
    ///
    /// Returns `Ok(())` on success, or an error message if serialization
    /// or file writing fails.
    #[cfg(not(feature = "mini"))]
    pub fn save(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialization error: {}", e))?;
        std::fs::write(path, &json).map_err(|e| format!("write error: {}", e))
    }

    /// Deserialize widget registry data from a JSON file at `path`.
    ///
    /// Replaces all current entries with the data from the file.
    /// Returns `Ok(())` on success, or an error message if reading
    /// or parsing fails.
    #[cfg(not(feature = "mini"))]
    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("read error: {}", e))?;
        let loaded: WidgetRegistry =
            serde_json::from_str(&json).map_err(|e| format!("parse error: {}", e))?;
        *self = loaded;
        Ok(())
    }

    /// Return all registered [`ObjectId`]s.
    pub fn all_ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.entries.keys().copied()
    }

    /// Total number of registered widgets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_kind.clear();
    }
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
    }
}
