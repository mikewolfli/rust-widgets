// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Central widget registry — maps `ObjectId` to widget metadata.
//!
//! This is the "index" that allows any part of the system to look up
//! a widget by its raw `ObjectId` at runtime.

use crate::compat::{HashMap, String, Vec};

#[cfg(all(feature = "serde", widgets_unstripped))]
use serde::{Deserialize, Serialize};

use crate::core::ObjectId;

pub use crate::widget::WidgetKind;

/// Metadata stored for each registered widget.
#[derive(Debug, Clone)]
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
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
#[cfg_attr(all(feature = "serde", widgets_unstripped), derive(Serialize, Deserialize))]
pub struct WidgetRegistry {
    entries: HashMap<ObjectId, WidgetEntry>,
    by_kind: HashMap<WidgetKind, Vec<ObjectId>>,
    /// Parent → children index.
    ///
    /// `children_of` used to scan every entry on every call. The index is derived from
    /// `entries`, so it is skipped by serde and rebuilt after a load.
    #[cfg_attr(all(feature = "serde", widgets_unstripped), serde(skip))]
    children: HashMap<ObjectId, Vec<ObjectId>>,
}

impl WidgetRegistry {
    /// Rebuilds the derived indexes from `entries`.
    ///
    /// Only `load` needs it: the incremental paths maintain the indexes directly, and a build
    /// without JSON has no `load`.
    #[cfg(all(feature = "serde", feature = "serde_json", widgets_unstripped))]
    fn rebuild_indices(&mut self) {
        let mut by_kind: HashMap<WidgetKind, Vec<ObjectId>> = HashMap::new();
        let mut children: HashMap<ObjectId, Vec<ObjectId>> = HashMap::new();
        for (id, entry) in &self.entries {
            by_kind.entry(entry.kind).or_default().push(*id);
            if let Some(parent) = entry.parent {
                children.entry(parent).or_default().push(*id);
            }
        }
        self.by_kind = by_kind;
        self.children = children;
    }

    /// Create an empty registry.
    pub fn new() -> Self {
        Self { entries: HashMap::new(), by_kind: HashMap::new(), children: HashMap::new() }
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
            if let Some(parent) = old.parent {
                Self::forget_child(&mut self.children, parent, id);
            }
        }

        self.by_kind.entry(kind).or_default().push(id);
        if let Some(parent) = entry.parent {
            self.children.entry(parent).or_default().push(id);
        }
        self.entries.insert(id, entry);
    }

    /// Unregister a widget by its [`ObjectId`].
    pub fn unregister(&mut self, id: ObjectId) {
        if let Some(entry) = self.entries.remove(&id) {
            if let Some(list) = self.by_kind.get_mut(&entry.kind) {
                list.retain(|&x| x != id);
            }
            if let Some(parent) = entry.parent {
                Self::forget_child(&mut self.children, parent, id);
            }
        }
    }

    /// Drops `id` from `parent`'s child list, removing the key when that was the last child.
    ///
    /// The children index is keyed by parent id. Leaving an empty `Vec` behind after the last
    /// child left meant every parent ever seen kept a key forever, so a long-lived registry grew
    /// one entry per parent unregistered — a leak with no bound. A key with no children carries
    /// no information (`children_of` answers the same either way), so it is removed.
    fn forget_child(
        children: &mut HashMap<ObjectId, Vec<ObjectId>>,
        parent: ObjectId,
        id: ObjectId,
    ) {
        let now_empty = match children.get_mut(&parent) {
            Some(list) => {
                list.retain(|&x| x != id);
                list.is_empty()
            }
            None => false,
        };
        if now_empty {
            children.remove(&parent);
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
    ///
    /// Reads the parent → children index, so the cost is proportional to the number of children
    /// rather than to the size of the whole registry (the previous version scanned every entry).
    pub fn children_of(&self, parent_id: ObjectId) -> Vec<&WidgetEntry> {
        self.children
            .get(&parent_id)
            .map(|ids| ids.iter().filter_map(|id| self.entries.get(id)).collect())
            .unwrap_or_default()
    }

    /// Serialize the registry to a JSON file at `path`.
    ///
    /// Returns `Ok(())` on success, or an error message if serialization
    /// or file writing fails.
    #[cfg(all(feature = "serde", feature = "serde_json", widgets_unstripped))]
    pub fn save(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            format!(
                "{} widget registry entry/entries could not be serialized to JSON: {e}",
                self.len()
            )
        })?;
        std::fs::write(path, &json).map_err(|e| {
            format!(
                "widget registry ({} bytes of JSON) could not be written to '{path}': {e} \
                 (check that the directory exists and is writable)",
                json.len()
            )
        })
    }

    /// Deserialize widget registry data from a JSON file at `path`.
    ///
    /// Replaces all current entries with the data from the file.
    /// Returns `Ok(())` on success, or an error message if reading
    /// or parsing fails.
    #[cfg(all(feature = "serde", feature = "serde_json", widgets_unstripped))]
    pub fn load(&mut self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| {
            format!(
                "widget registry file '{path}' could not be read: {e} (check that the path \
                 exists and is readable)"
            )
        })?;
        let loaded: WidgetRegistry = serde_json::from_str(&json).map_err(|e| {
            format!("widget registry file '{path}' is not valid WidgetRegistry JSON: {e}")
        })?;
        *self = loaded;
        // The parent → children index is derived state and is skipped by serde, so it is empty
        // after a load. Rebuild both indexes rather than trusting the serialized `by_kind`.
        self.rebuild_indices();
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
        self.children.clear();
    }
}

crate::impl_default_via_new!(WidgetRegistry);

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: ObjectId, parent: Option<ObjectId>) -> WidgetEntry {
        WidgetEntry { id, kind: WidgetKind::Button, parent, label: String::new() }
    }

    /// The parent → children index must follow re-registration and removal, not only insertion.
    #[test]
    fn children_index_tracks_parent_changes_and_removal() {
        let mut registry = WidgetRegistry::new();
        registry.register(entry(1, None));
        registry.register(entry(2, Some(1)));
        registry.register(entry(3, Some(1)));
        assert_eq!(registry.children_of(1).len(), 2);

        // Re-registering with a different parent must move the id, not duplicate it.
        registry.register(entry(2, Some(3)));
        assert_eq!(registry.children_of(1).len(), 1);
        assert_eq!(registry.children_of(3).len(), 1);

        registry.unregister(3);
        assert!(
            registry.children_of(1).is_empty(),
            "removing child 3 must also leave parent 1's list"
        );
        assert_eq!(
            registry.children_of(3).len(),
            1,
            "child 2 still names 3 as its parent; the index mirrors the entries, it does not cascade"
        );
    }

    /// W6-48: removing the last child must remove the parent's key, not leave an empty `Vec`.
    ///
    /// The old `unregister` retained the child list but never removed the key, so every parent
    /// that ever had a child kept an entry in the children index forever. A host that rebuilds its
    /// widget tree repeatedly grew that map one key per unregistered child.
    #[test]
    fn repeated_parent_unregister_register_does_not_grow_the_children_index() {
        let mut registry = WidgetRegistry::new();
        registry.register(entry(1, None));

        // One round trip: register the child, unregister it, and the key is gone.
        registry.register(entry(2, Some(1)));
        assert_eq!(registry.children.len(), 1);
        registry.unregister(2);
        assert_eq!(registry.children.len(), 0, "the last child leaving must remove the parent key");

        // Repeated mounting/unmounting must stay flat rather than accumulating one key per pass.
        for _ in 0..100 {
            registry.register(entry(2, Some(1)));
            registry.unregister(2);
        }
        assert_eq!(
            registry.children.len(),
            0,
            "repeated unregister/register must not grow the map"
        );
    }

    /// Re-registering a child under a new parent must also drop the old parent's now-empty key.
    #[test]
    fn moving_the_last_child_removes_the_old_parent_key() {
        let mut registry = WidgetRegistry::new();
        registry.register(entry(3, None));
        registry.register(entry(2, Some(1)));

        registry.register(entry(2, Some(3)));

        assert_eq!(registry.children.len(), 1, "parent 1 has no children left, so its key is gone");
        assert_eq!(registry.children_of(3).len(), 1);
    }
}
