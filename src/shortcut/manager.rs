// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry};
use crate::compat::{format, HashMap, String, Vec};
use crate::event::Event;
use crate::signal::Signal1;
/// Global shortcut manager for registering and dispatching shortcuts.
///
/// # Scopes (W-201)
///
/// Bindings registered with [`ShortcutManager::register`] live in the **global** scope and behave
/// exactly as before. [`ShortcutManager::register_in`] attaches a binding to a named context (a
/// window or modal domain), and [`ShortcutManager::push_scope`] / `push_modal_scope` make contexts
/// active. Resolution walks the active stack from the top down and falls back to the global scope,
/// so the same chord can mean different commands in different windows and a modal scope can shield
/// the layers below it.
pub struct ShortcutManager {
    /// Map from shortcut to action ID for the **global** scope.
    ///
    /// Kept as its own map because it is the historical public view
    /// ([`Self::all_shortcuts`]); scoped bindings live in `scoped_shortcuts` so the two cannot
    /// collide by accident.
    shortcuts: HashMap<Shortcut, String>,
    /// Map from `(scope id, shortcut)` to action ID for scoped bindings.
    scoped_shortcuts: HashMap<(String, Shortcut), String>,
    /// Map from action ID to entry.
    entries: HashMap<String, ShortcutEntry>,
    /// Active scope stack, bottom first. Resolution walks it in reverse (top first).
    scopes: Vec<ActiveScope>,
    /// Emitted when a shortcut is triggered.
    pub shortcut_triggered: Signal1<String>,
    /// Emitted when a shortcut conflict is detected.
    pub conflict_detected: Signal1<(Shortcut, String, String)>,
}

/// One active context on the scope stack.
#[derive(Debug, Clone)]
struct ActiveScope {
    /// Host-chosen context name (a window id, a modal dialog id, …).
    id: String,
    /// Whether this scope shields every scope below it, modal-dialog style.
    modal: bool,
}

impl ShortcutManager {
    /// Creates a new shortcut manager.
    pub fn new() -> Self {
        Self {
            shortcuts: HashMap::new(),
            scoped_shortcuts: HashMap::new(),
            entries: HashMap::new(),
            scopes: Vec::new(),
            shortcut_triggered: Signal1::new(),
            conflict_detected: Signal1::new(),
        }
    }
    /// Registers a new shortcut in the global scope.
    /// Returns true if registration succeeded, false if there was a conflict.
    pub fn register(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        self.register_scoped(None, action_id, shortcut, description)
    }

    /// Registers a shortcut in `scope` instead of the global scope.
    ///
    /// The binding only fires while `scope` is on the active stack (see [`Self::push_scope`]), and
    /// it shadows a global binding on the same chord. The same chord may be bound in two different
    /// scopes — that is the point of scopes — so conflicts are judged **within** a scope only.
    pub fn register_in(
        &mut self,
        scope: impl Into<String>,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        self.register_scoped(Some(scope.into()), action_id, shortcut, description)
    }

    /// Shared body of [`Self::register`] and [`Self::register_in`].
    fn register_scoped(
        &mut self,
        scope: Option<String>,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        description: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        let description = description.into();
        // Conflict is judged inside the binding's own scope: a global chord and a scoped chord may
        // share a key, which is how a modal dialog rebinds Enter without fighting the main window.
        let conflict = match &scope {
            None => {
                self.shortcuts.get(&shortcut).filter(|existing| **existing != action_id).cloned()
            }
            Some(scope) => self
                .scoped_shortcuts
                .get(&(scope.clone(), shortcut.clone()))
                .filter(|existing| **existing != action_id)
                .cloned(),
        };
        if let Some(existing_action) = conflict {
            self.conflict_detected.emit((shortcut.clone(), action_id.clone(), existing_action));
            return false;
        }
        // One chord per action: re-registering an id moves its binding rather than adding one.
        if let Some(entry) = self.entries.remove(&action_id) {
            self.remove_binding(&entry);
        }
        let entry = ShortcutEntry {
            action_id: action_id.clone(),
            description,
            shortcut: shortcut.clone(),
            enabled: true,
            scope: scope.clone(),
        };
        match scope {
            Some(scope) => {
                self.scoped_shortcuts.insert((scope, shortcut), action_id.clone());
            }
            None => {
                self.shortcuts.insert(shortcut, action_id.clone());
            }
        }
        self.entries.insert(action_id, entry);
        true
    }

    /// Removes `entry`'s chord from the table its scope belongs to.
    fn remove_binding(&mut self, entry: &ShortcutEntry) {
        match &entry.scope {
            Some(scope) => {
                self.scoped_shortcuts.remove(&(scope.clone(), entry.shortcut.clone()));
            }
            None => {
                self.shortcuts.remove(&entry.shortcut);
            }
        }
    }

    /// Unregisters a shortcut by action ID.
    pub fn unregister(&mut self, action_id: &str) -> bool {
        if let Some(entry) = self.entries.remove(action_id) {
            self.remove_binding(&entry);
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
    /// # Resolution order (W-201)
    ///
    /// The active scope stack is walked from the top down. A matching binding in a scope wins over
    /// every lower scope and over the global scope. A **modal** scope terminates the walk whether
    /// or not it matched, which is what shields the layers below a modal dialog. After the stack is
    /// exhausted, global bindings are consulted exactly as before scopes existed.
    ///
    /// The global fallback is still a linear scan over the entries **on purpose**: the hash map is
    /// the reverse index ([`Self::get_action`]), but a second matching path would be a second rule.
    /// Every entry is judged by the one function, so registry dispatch and an unregistered widget
    /// action cannot disagree about a chord.
    fn lookup(&self, key: Key, modifiers: Modifiers) -> Option<String> {
        for active in self.scopes.iter().rev() {
            let matched = self
                .scoped_shortcuts
                .iter()
                .find(|((scope, shortcut), _)| {
                    scope == &active.id && Self::matches_binding(shortcut, key, modifiers)
                })
                .map(|(_, action_id)| action_id.clone());
            if matched.is_some() {
                return matched;
            }
            if active.modal {
                return None;
            }
        }
        self.entries
            .values()
            .find(|entry| {
                entry.scope.is_none() && Self::matches_binding(&entry.shortcut, key, modifiers)
            })
            .map(|entry| entry.action_id.clone())
    }

    /// Pushes a non-modal scope onto the active stack.
    ///
    /// Bindings registered in this scope now win over global ones on the same chord. A scope id may
    /// be pushed more than once; the stack then contains it more than once, which is harmless but
    /// rarely useful.
    pub fn push_scope(&mut self, scope: impl Into<String>) {
        self.scopes.push(ActiveScope { id: scope.into(), modal: false });
    }

    /// Pushes a **modal** scope: it wins like any other scope and also stops the lookup from
    /// reaching the scopes and global bindings beneath it.
    pub fn push_modal_scope(&mut self, scope: impl Into<String>) {
        self.scopes.push(ActiveScope { id: scope.into(), modal: true });
    }

    /// Pops the top scope, returning its id.
    pub fn pop_scope(&mut self) -> Option<String> {
        self.scopes.pop().map(|scope| scope.id)
    }

    /// The innermost active scope, or `None` when only the global scope is active.
    pub fn active_scope(&self) -> Option<&str> {
        self.scopes.last().map(|scope| scope.id.as_str())
    }

    /// Whether `scope` is currently on the stack.
    pub fn is_scope_active(&self, scope: &str) -> bool {
        self.scopes.iter().any(|active| active.id == scope)
    }

    /// The active scope ids in activation order (bottom first).
    pub fn active_scopes(&self) -> Vec<String> {
        self.scopes.iter().map(|scope| scope.id.clone()).collect()
    }

    /// Removes every active scope, returning to the global-only behaviour.
    pub fn clear_scopes(&mut self) {
        self.scopes.clear();
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
    /// Returns the shortcut for an action ID, in any scope.
    pub fn get_shortcut(&self, action_id: &str) -> Option<&Shortcut> {
        self.entries.get(action_id).map(|e| &e.shortcut)
    }
    /// Returns the action ID for a **global** shortcut.
    ///
    /// Scoped bindings are not part of this view: the same chord may map to a different action in
    /// each scope, so a scope-free key cannot name one answer.
    pub fn get_action(&self, shortcut: &Shortcut) -> Option<&String> {
        self.shortcuts.get(shortcut)
    }
    /// Returns all registered **global** shortcuts.
    ///
    /// See [`Self::get_action`] for why scoped bindings are excluded.
    pub fn all_shortcuts(&self) -> &HashMap<Shortcut, String> {
        &self.shortcuts
    }
    /// Returns all entries, global and scoped.
    pub fn all_entries(&self) -> &HashMap<String, ShortcutEntry> {
        &self.entries
    }
    /// Clears all shortcuts, scoped or global, and every active scope.
    pub fn clear(&mut self) {
        self.shortcuts.clear();
        self.scoped_shortcuts.clear();
        self.entries.clear();
        self.scopes.clear();
    }
    /// Returns true if a **global** shortcut is registered.
    pub fn has_shortcut(&self, shortcut: &Shortcut) -> bool {
        self.shortcuts.contains_key(shortcut)
    }
    /// Returns true if `scope` has `shortcut` registered.
    pub fn has_scoped_shortcut(&self, scope: &str, shortcut: &Shortcut) -> bool {
        self.scoped_shortcuts.contains_key(&(String::from(scope), shortcut.clone()))
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
