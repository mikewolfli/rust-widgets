use super::{Key, Modifiers, Shortcut, ShortcutEntry};
use crate::signal::Signal1;
use std::collections::HashMap;
/// Global shortcut manager for registering and dispatching shortcuts.
pub struct ShortcutManager {
    /// Map from shortcut to action ID.
    shortcuts: HashMap<Shortcut, String>,
    /// Map from action ID to entry.
    entries: HashMap<String, ShortcutEntry>,
    /// Emitted when a shortcut is triggered.
    pub shortcut_triggered: Signal1<String>,
    /// Emitted when a shortcut conflict is detected.
    pub conflict_detected: Signal1<(Shortcut, String, String)>,
}
impl ShortcutManager {
    /// Creates a new shortcut manager.
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            entries: HashMap::new(),
            shortcut_triggered: Signal1::new(),
            conflict_detected: Signal1::new(),
        }
    }
    /// Registers a new shortcut.
    /// Returns true if registration succeeded, false if there was a conflict.
    pub fn register(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        let description = description.into();
        if let Some(existing_action) = self.shortcuts.get(&shortcut) {
            if existing_action != &action_id {
                self.conflict_detected.emit((
                    shortcut.clone(),
                    action_id.clone(),
                    existing_action.clone(),
                ));
                return false;
            }
        }
        if let Some(entry) = self.entries.get(&action_id) {
            self.shortcuts.remove(&entry.shortcut);
        }
        let entry = ShortcutEntry {
            action_id: action_id.clone(),
            description,
            shortcut: shortcut.clone(),
            enabled: true,
        };
        self.shortcuts.insert(shortcut, action_id.clone());
        self.entries.insert(action_id, entry);
        true
    }
    /// Unregisters a shortcut by action ID.
    pub fn unregister(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.remove(action_id) {
            self.shortcuts.remove(&entry.shortcut);
            true
        } else {
            false
        }
    }
    /// Unregisters a shortcut by key combination.
    pub fn unregister_shortcut(&mut self, shortcut: &Shortcut) -> bool {
        if let Some(action_id) = self.shortcuts.remove(shortcut) {
            self.entries.remove(&action_id);
            true
        } else {
            false
        }
    }
    /// Handles a key event and triggers the associated action if a shortcut matches.
    pub fn handle_key_event(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let shortcut = Shortcut::new(key, modifiers);
        if let Some(action_id) = self.shortcuts.get(&shortcut) {
            if let Some(entry) = self.entries.get(action_id) {
                if entry.enabled {
                    self.shortcut_triggered.emit(action_id.clone());
                    return true;
                }
            }
        }
        false
    }
    /// Enables a shortcut by action ID.
    pub fn enable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = true;
            true
        } else {
            false
        }
    }
    /// Disables a shortcut by action ID.
    pub fn disable(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(action_id) {
            entry.enabled = false;
            true
        } else {
            false
        }
    }
    /// Returns the shortcut for an action ID.
    pub fn get_shortcut(&self, action_id: &str) -> Option<&Shortcut> {
        self.entries.get(action_id).map(|e| &e.shortcut)
    }
    /// Returns the action ID for a shortcut.
    pub fn get_action(&self, shortcut: &Shortcut) -> Option<&String> {
        self.shortcuts.get(shortcut)
    }
    /// Returns all registered shortcuts.
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String> {
        &self.shortcuts
    }
    /// Returns all entries.
    pub fn all_entries(&self) -> &HashMap<String, ShortcutEntry> {
        &self.entries
    }
    /// Clears all shortcuts.
    pub fn clear(&mut self) {
        self.shortcuts.clear();
        self.entries.clear();
    }
    /// Returns true if a shortcut is registered.
    pub fn has_shortcut(&self, shortcut: &Shortcut) -> bool {
        self.shortcuts.contains_key(shortcut)
    }
    /// Returns true if an action has a shortcut registered.
    pub fn has_action(&self, action_id: &str) -> bool {
        self.entries.contains_key(action_id)
    }
    /// Finds conflicts between the given shortcut and existing shortcuts.
    pub fn find_conflicts(&self, shortcut: &Shortcut) -> Vec<&ShortcutEntry> {
        self.entries
            .values()
            .filter(|e| e.shortcut.conflicts_with(shortcut))
            .collect()
    }
}
impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}
