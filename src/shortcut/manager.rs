// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry};
use crate::compat::{HashMap, String, Vec};
use crate::event::Event;
use crate::signal::Signal1;
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

    /// Returns whether an action ID is already registered.
    pub fn contains_action(&self, action_id: &str) -> bool {
        self.entries.contains_key(action_id)
    }
    /// Handles a key event and triggers the associated action if a shortcut matches.
    ///
    /// # Matching rule
    ///
    /// A key event's [`Modifiers::PRIMARY`] bit means "the platform's primary
    /// accelerator". The physical Control bit that macOS also reports alongside
    /// Command is **not** part of the command's identity, so it is cleared before
    /// the lookup. On Windows/Linux a Control press is lifted the other way, into
    /// PRIMARY, so `Shortcut::primary` bindings answer to Ctrl as the platform
    /// convention requires. Physical-Control bindings (`Shortcut::ctrl`) are
    /// deliberately left alone on macOS: there Command and Control are different
    /// keys, and collapsing them would fire `⌘C` for a `Control+C` binding.
    pub fn handle_key_event(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let Some(action_id) = self.lookup(key, modifiers) else {
            return false;
        };
        let Some(entry) = self.entries.get(&action_id) else {
            return false;
        };
        if !entry.enabled {
            return false;
        }
        self.shortcut_triggered.emit(action_id);
        true
    }

    /// Resolves the action registered for a key press, applying the matching
    /// rule documented on [`ShortcutManager::handle_key_event`].
    fn lookup(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        // Ask the backend which convention this host uses, rather than testing
        // `target_os` here (principles #35/#36/#37). The question is about
        // *behaviour on the host that is running*, not about which OS this was
        // compiled for — a macOS-style replay on a Linux CI host must resolve the
        // way the backend says, and a new platform must not have to edit a list
        // of OS names in this file.
        let style = crate::platform::platform_facts().shortcut_style();
        let normalized = Self::normalize_event_modifiers(modifiers, style);
        if let Some(action_id) = self.shortcuts.get(&Shortcut::new(key, normalized)) {
            return Some(action_id.clone());
        }
        // Windows/Linux: the OS shortcut convention is Control, so a
        // `Shortcut::primary` binding resolves against a Control press there.
        if style == PlatformShortcutStyle::Desktop
            && normalized.contains(Modifiers::CTRL)
            && !normalized.contains(Modifiers::PRIMARY)
        {
            let as_primary =
                Shortcut::new(key, normalized.without(Modifiers::CTRL) | Modifiers::PRIMARY);
            if let Some(action_id) = self.shortcuts.get(&as_primary) {
                return Some(action_id.clone());
            }
        }
        None
    }

    /// Removes the Control bit that macOS reports alongside Command.
    ///
    /// `⌘C` and `⌃C` are different chords, but AppKit sets the Control flag on
    /// some Command events; leaving it in place would make `Shortcut::primary`
    /// bindings unreachable. On other platforms the input is already canonical.
    ///
    /// The deciding fact is the backend's shortcut style, passed in by the
    /// caller — this function does not sniff the OS (principle #37).
    fn normalize_event_modifiers(modifiers: Modifiers, style: PlatformShortcutStyle) -> Modifiers {
        if style == PlatformShortcutStyle::Mac
            && modifiers.contains(Modifiers::PRIMARY)
            && modifiers.contains(Modifiers::CTRL)
        {
            return modifiers.without(Modifiers::CTRL);
        }
        modifiers
    }
    /// Handles a framework event and triggers a shortcut when it is a key press.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Event::KeyPress { key, modifiers } = event {
            if let Some(key) = Key::from_key_code(*key) {
                return self.handle_key_event(key, Modifiers::from_event_bits(*modifiers));
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
        self.entries.values().filter(|e| e.shortcut.conflicts_with(shortcut)).collect()
    }
}
crate::impl_default_via_new!(ShortcutManager);
