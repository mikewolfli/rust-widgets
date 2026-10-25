// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry};
use crate::compat::{format, HashMap, String, Vec};
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
        let Some(action_id) = self.action_for_key(key, modifiers) else {
            return false;
        };
        self.shortcut_triggered.emit(action_id);
        true
    }

    /// Resolves the action registered for a key press.
    ///
    /// This is the public query the rest of the crate uses instead of re-implementing the
    /// platform modifier rule. `None` means no **enabled** action owns this chord; a disabled
    /// entry is deliberately indistinguishable from an absent one, because dispatching either
    /// would be wrong.
    pub fn action_for_key(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        let action_id = self.lookup(key, modifiers)?;
        let entry = self.entries.get(&action_id)?;
        entry.enabled.then_some(action_id)
    }

    /// Whether a typed binding matches a key press, using the same rule as [`Self::lookup`].
    ///
    /// # Why this is the single decision point
    ///
    /// Three callers need the answer — the global registry, a menu item's registered binding,
    /// and a standalone `Action` widget — and they used to each carry a copy of the platform
    /// normalisation. Copies drift, so a binding could fire on one path and not another. This
    /// function is the one implementation; the others delegate to it.
    pub fn matches_binding(shortcut: &Shortcut, key: Key, modifiers: Modifiers) -> bool {
        if shortcut.key != key {
            return false;
        }
        let style = crate::platform::platform_facts().shortcut_style();
        let normalized = Self::normalize_event_modifiers(modifiers, style);
        if shortcut.modifiers == normalized {
            return true;
        }
        // Windows/Linux: Control is the platform accelerator, so a `primary` binding answers
        // to a Control press. This is the same lift `lookup` applies to the registry key.
        style == PlatformShortcutStyle::Desktop
            && normalized.contains(Modifiers::CTRL)
            && !normalized.contains(Modifiers::PRIMARY)
            && shortcut.modifiers == normalized.without(Modifiers::CTRL) | Modifiers::PRIMARY
    }

    /// Dispatches a key press through the registry, returning the action id that fired.
    ///
    /// This is the host entry point shared by `Action` widgets and native menu items: a host
    /// calls it once per key press and routes the returned id to [`crate::action::ActionManager`].
    /// Keeping it here means conflict detection and matching see every binding, including the
    /// menu accelerators that used to be handled only inside the menu code.
    pub fn dispatch_shortcut(&mut self, key: Key, modifiers: Modifiers) -> Option<String> {
        let action_id = self.action_for_key(key, modifiers)?;
        self.shortcut_triggered.emit(action_id.clone());
        Some(action_id)
    }

    /// Resolves the action registered for a key press, applying the matching
    /// rule documented on [`ShortcutManager::matches_binding`].
    ///
    /// The lookup is a linear scan over the entries **on purpose**: the hash map is still the
    /// reverse index (`get_action`), but a second matching path would be a second rule. Every
    /// entry is judged by the one function, so registry dispatch and an unregistered widget
    /// action cannot disagree about a chord.
    fn lookup(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        self.entries
            .values()
            .find(|entry| Self::matches_binding(&entry.shortcut, key, modifiers))
            .map(|entry| entry.action_id.clone())
    }

    /// Removes the Control bit that macOS reports alongside Command.
    ///
    /// `⌘C` and `⌃C` are different chords, but macOS sets the Control flag on
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
    ///
    /// This is the host's entry point: no framework event loop calls it. The
    /// application owns its shortcut manager and feeds key presses here from
    /// its own dispatch path (see the module docs for the wiring and why the
    /// framework does not pump this itself).
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

impl Shortcut {
    /// Serializes this binding as a stable, single-key JSON object.
    ///
    /// The value is the same canonical spelling [`Shortcut::format_shortcut`] produces, which
    /// [`Shortcut::from_string`] parses back. A stable single field is enough because a shortcut
    /// has exactly one fact; the field name is the versioning seam, not a second schema.
    pub fn to_json(&self) -> String {
        format!("{{\"shortcut\":\"{}\"}}", json_escape(&self.format_shortcut()))
    }

    /// Reads the binding written by [`Shortcut::to_json`].
    ///
    /// Unknown keys are ignored (an older reader must not fail on a newer document), but a
    /// missing or unparsable `shortcut` is an error rather than a silently empty binding.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let spelling = json_string_field(text, "shortcut")
            .ok_or_else(|| String::from("shortcut JSON has no `shortcut` string field"))?;
        Self::from_string(&spelling)
            .ok_or_else(|| format!("`{spelling}` is not a shortcut this build understands"))
    }
}

/// Escapes the four characters a JSON string may not carry raw.
fn json_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Reads one flat string field from a single-level JSON object.
///
/// Deliberately small: the caller's schema is flat and versioned by field name, so a full JSON
/// parser would be more machinery than the one fact being read.
fn json_string_field(text: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let start = text.find(&key)? + key.len();
    let rest = &text[start..];
    let colon = rest.find(':')? + 1;
    let after_colon = rest[colon..].trim_start();
    let mut chars = after_colon.strip_prefix('"')?.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}
