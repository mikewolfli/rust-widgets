// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{
    canonicalize_shortcut_type, normalize_shortcut, Action, ActionBinding, ActionDefinition,
    ActionHostKind, ActionLiveState,
};
use crate::compat::{lock, HashMap, Mutex, String, Vec};
use crate::core::ObjectId;
use crate::shortcut::Shortcut;
use crate::signal::Signal1;
use alloc::sync::Arc;
use core::fmt;
/// Registry for actions, shortcuts, and menu/toolbar bindings.
///
/// # Signals are the live-state channel
///
/// A host (menu row, toolbar item, button) subscribes once per linked command instead of polling
/// the registry. The four signals below are the complete set of state transitions a host can
/// present: trigger, checked toggle, enabled change and a chord collision. Keeping them on the
/// registry is what lets several hosts of one command update together.
pub struct ActionManager {
    actions: HashMap<String, Action>,
    shortcut_to_action: HashMap<String, String>,
    bindings: Vec<ActionBinding>,
    /// Emitted with the action id after an enabled action fired.
    pub action_triggered: Signal1<String>,
    /// Emitted with `(action id, new checked state)` after a checkable action toggled.
    pub action_toggled: Signal1<(String, bool)>,
    /// Emitted with `(action id, new enabled state)` after an enabled transition.
    pub action_enabled_changed: Signal1<(String, bool)>,
    /// Emitted with `(incoming action id, action that already owns the chord)`.
    ///
    /// # Why this does not reject the binding
    ///
    /// `ShortcutManager` is the only conflict *judge* (rule: one decision point). This signal is
    /// the observation side: it reports that the registry's reverse index already mapped a chord
    /// to another action, so a host can surface the collision even when it bound directly instead
    /// of going through `ActionRouter` (which asks `ShortcutManager` first).
    pub shortcut_conflict: Signal1<(String, String)>,
}
impl fmt::Debug for ActionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionManager")
            .field("actions", &self.actions.keys())
            .field("shortcut_to_action", &self.shortcut_to_action)
            .field("bindings", &self.bindings)
            .field("action_triggered", &self.action_triggered.slot_count())
            .field("action_toggled", &self.action_toggled.slot_count())
            .field("action_enabled_changed", &self.action_enabled_changed.slot_count())
            .field("shortcut_conflict", &self.shortcut_conflict.slot_count())
            .finish()
    }
}
impl ActionManager {
    /// Creates an empty action manager.
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            shortcut_to_action: HashMap::new(),
            bindings: Vec::new(),
            action_triggered: Signal1::new(),
            action_toggled: Signal1::new(),
            action_enabled_changed: Signal1::new(),
            shortcut_conflict: Signal1::new(),
        }
    }
    /// Registers a new action and returns false if the id already exists.
    pub fn register_action(&mut self, id: impl Into<String>, text: impl Into<String>) -> bool {
        let id = id.into();
        if self.actions.contains_key(&id) {
            return false;
        }
        self.actions.insert(id.clone(), Action::new(id, text));
        true
    }

    /// Removes an action and all of its shortcut/host bindings.
    pub fn unregister_action(&mut self, id: &str) -> bool {
        let removed = self.actions.remove(id).is_some();
        if removed {
            self.shortcut_to_action.retain(|_, action_id| action_id != id);
            self.bindings.retain(|binding| binding.action_id != id);
        }
        removed
    }
    /// Returns an immutable action reference by id.
    pub fn action(&self, id: &str) -> Option<&Action> {
        self.actions.get(id)
    }
    /// Returns a mutable action reference by id.
    pub fn action_mut(&mut self, id: &str) -> Option<&mut Action> {
        self.actions.get_mut(id)
    }
    /// Sets an action's enabled state and returns false when id is unknown.
    pub fn set_action_enabled(&mut self, id: &str, enabled: bool) -> bool {
        let Some(action) = self.actions.get_mut(id) else {
            return false;
        };
        let before = action.is_enabled();
        action.set_enabled(enabled);
        let changed = before != action.is_enabled();
        if changed {
            let id = String::from(id);
            self.action_enabled_changed.emit((id, enabled));
        }
        true
    }
    /// Binds a keyboard shortcut to an existing action id (string-based).
    ///
    /// Rebinding an action **moves** it: any chord previously bound to this action is
    /// released first. Without that, `bind_shortcut("Ctrl+S", "save")` followed by
    /// `bind_shortcut("Ctrl+Shift+S", "save")` left both chords live, so the stale
    /// accelerator kept firing after the rebind. `ShortcutManager::register` already
    /// releases the previous chord for exactly this reason; this is the same rule.
    pub fn bind_shortcut(
        &mut self,
        shortcut: impl Into<String>,
        action_id: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        let chord = normalize_shortcut(&shortcut.into());
        self.observe_chord_collision(&chord, &action_id);
        self.release_shortcuts_for(&action_id);
        self.shortcut_to_action.insert(chord, action_id);
        true
    }
    /// Binds a `Shortcut` type to an existing action id. Bridges `shortcut` module with `action`.
    ///
    /// See [`bind_shortcut`](Self::bind_shortcut) for why the action's previous chord is
    /// released first.
    pub fn bind_shortcut_type(
        &mut self,
        shortcut: &Shortcut,
        action_id: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        let chord = canonicalize_shortcut_type(shortcut);
        self.observe_chord_collision(&chord, &action_id);
        self.release_shortcuts_for(&action_id);
        // `canonicalize_shortcut_type`, not `normalize_shortcut`: the typed bridge
        // keeps [`Modifiers::CTRL`](crate::shortcut::Modifiers::CTRL) distinct from
        // [`Modifiers::PRIMARY`](crate::shortcut::Modifiers::PRIMARY), so
        // `Shortcut::ctrl(S)` and `Shortcut::primary(S)` occupy different keys
        // instead of colliding. The string API (`bind_shortcut`/`trigger_shortcut`)
        // keeps its own convention of folding `Ctrl` onto `Primary`.
        self.shortcut_to_action.insert(chord, action_id);
        true
    }

    /// Reports a chord that is already routed to a different action.
    ///
    /// The observation is emitted only; the caller's binding still replaces the entry because
    /// `ShortcutManager` owns the accept/reject decision. Emitting rather than ignoring is what
    /// makes a direct `ActionManager::bind_shortcut` collision visible at all.
    fn observe_chord_collision(&self, chord: &str, action_id: &str) {
        let Some(existing) = self.shortcut_to_action.get(chord) else {
            return;
        };
        if existing != action_id {
            self.shortcut_conflict.emit((String::from(action_id), existing.clone()));
        }
    }

    /// Emits [`Self::shortcut_conflict`] on behalf of the authoritative judge.
    ///
    /// `ActionRouter` calls this after `ShortcutManager::register` refuses a chord, so the signal
    /// carries the same verdict the registry gave without `ActionManager` making a second one.
    pub fn report_shortcut_conflict(
        &self,
        incoming: impl Into<String>,
        existing: impl Into<String>,
    ) {
        self.shortcut_conflict.emit((incoming.into(), existing.into()));
    }

    /// Drop every chord currently routed to `action_id`.
    ///
    /// Called before a (re)bind so an action holds one chord rather than accumulating
    /// every chord it has ever been bound to.
    fn release_shortcuts_for(&mut self, action_id: &str) {
        let stale: crate::compat::Vec<String> = self
            .shortcut_to_action
            .iter()
            .filter(|(_, bound)| bound.as_str() == action_id)
            .map(|(chord, _)| chord.clone())
            .collect();
        for chord in stale {
            self.shortcut_to_action.remove(&chord);
        }
    }
    /// Resolves and triggers an action by shortcut string.
    pub fn trigger_shortcut(&mut self, shortcut: &str) -> bool {
        let Some(action_id) = self.shortcut_to_action.get(&normalize_shortcut(shortcut)).cloned()
        else {
            return false;
        };
        self.trigger_action(&action_id)
    }
    /// Triggers an action bound to a typed [`Shortcut`], preserving the physical
    /// [`Modifiers::CTRL`](crate::shortcut::Modifiers::CTRL) bit (unlike
    /// [`trigger_shortcut`], whose string input folds `Ctrl` onto `Primary`). This
    /// is the typed reverse-lookup counterpart of
    /// [`bind_shortcut_type`](Self::bind_shortcut_type).
    pub fn trigger_shortcut_type(&mut self, shortcut: &Shortcut) -> bool {
        let Some(action_id) =
            self.shortcut_to_action.get(&canonicalize_shortcut_type(shortcut)).cloned()
        else {
            return false;
        };
        self.trigger_action(&action_id)
    }
    /// Triggers an action directly by id and publishes the state transition.
    ///
    /// A disabled action still reports `false` and emits nothing, which is the same answer a host
    /// gets from a disabled menu row: the command exists but cannot run.
    pub fn trigger_action(&mut self, action_id: &str) -> bool {
        let Some(action) = self.actions.get_mut(action_id) else {
            return false;
        };
        let was_checked = action.is_checked();
        if !action.trigger() {
            return false;
        }
        let checkable = action.is_checkable();
        let now_checked = action.is_checked();
        let toggled = checkable && now_checked != was_checked;
        let id = String::from(action_id);
        self.action_triggered.emit(id.clone());
        if toggled {
            self.action_toggled.emit((id, now_checked));
        }
        true
    }
    /// Returns every registered action id, in map order.
    ///
    /// A host building a menu or toolbar asks the registry what commands exist instead of
    /// keeping a second list; that is what makes `bind_action_to_*` the one attachment path.
    pub fn action_ids(&self) -> Vec<&String> {
        self.actions.keys().collect()
    }

    /// Every registered command with its current chord, sorted by id.
    ///
    /// Sorted because a document writer must be stable: two exports of one registry have to be
    /// byte-identical, and `HashMap` iteration order is not.
    pub fn definitions(&self) -> Vec<ActionDefinition> {
        let mut definitions: Vec<ActionDefinition> =
            self.actions.keys().filter_map(|id| self.action_definition(id)).collect();
        definitions.sort_by(|left, right| left.id.cmp(&right.id));
        definitions
    }

    /// Writes every command (and its chord) to `path` as the JSON array a project document uses.
    ///
    /// The document shape comes from `crate::json`, not from a second writer here: a keymap file
    /// and a project's `actions` section must be the same bytes for the same registry.
    #[cfg(full_widgets)]
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        crate::json::save_actions_to_file(path, &self.definitions())
    }

    /// Replaces nothing and adds the commands in `path`, rolling the batch back on a duplicate.
    ///
    /// Returns the error text from the file read/parse or from the registry, so a caller can show
    /// the same reason a designer would see. An existing action with the same id is a refusal, not
    /// an overwrite: a keymap file must not silently redefine a command the host registered.
    #[cfg(full_widgets)]
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let definitions = crate::json::load_actions_from_file(path)?;
        self.register_definitions(&definitions)
    }

    /// Registers a batch of serialized commands, rolling the batch back on a duplicate id.
    ///
    /// A partially applied document would leave a registry that no longer matches the file it was
    /// read from, so the failure path unregisters exactly the rows this call added.
    pub fn register_definitions(&mut self, definitions: &[ActionDefinition]) -> Result<(), String> {
        let mut added: Vec<String> = Vec::new();
        for definition in definitions {
            if !self.register_definition(definition) {
                for id in added.iter().rev() {
                    self.unregister_action(id);
                }
                return Err(crate::compat::format!(
                    "action `{}` could not be registered (duplicate id or conflicting chord)",
                    definition.id
                ));
            }
            added.push(definition.id.clone());
        }
        Ok(())
    }

    /// The canonical chord currently bound to `action_id`, if any.
    pub fn shortcut_for_action(&self, action_id: &str) -> Option<&String> {
        self.shortcut_to_action
            .iter()
            .find(|(_, bound)| bound.as_str() == action_id)
            .map(|(chord, _)| chord)
    }

    /// Exports one command together with the chord the registry holds for it.
    pub fn action_definition(&self, action_id: &str) -> Option<ActionDefinition> {
        let action = self.actions.get(action_id)?;
        let mut definition = ActionDefinition::from_action(action);
        definition.shortcut = self.shortcut_for_action(action_id).cloned().unwrap_or_default();
        Some(definition)
    }

    /// Subscribes one host to an action's live state.
    ///
    /// The returned cell starts at the command's current values and is updated from the action's
    /// own `toggled` / `enabled_changed` signals. This is the "live subscription" half of
    /// COMP-25: a host does not poll the registry, and several hosts of one command each get
    /// their own cell so dropping a host leaves no subscriber pointing at a destroyed widget.
    pub fn watch_action(&self, action_id: &str) -> Option<Arc<Mutex<ActionLiveState>>> {
        let definition = self.action_definition(action_id)?;
        let state = Arc::new(Mutex::new(ActionLiveState::from_definition(&definition)));
        if let Some(action) = self.actions.get(action_id) {
            let enabled_state = Arc::clone(&state);
            action.connect_enabled_changed_ref(move |enabled| {
                lock(&enabled_state).enabled = *enabled;
            });
            let checked_state = Arc::clone(&state);
            action.connect_toggled_ref(move |checked| {
                let mut state = lock(&checked_state);
                state.checked = *checked;
                state.checkable = true;
            });
        }
        Some(state)
    }

    /// Registers a serialized command, including its chord and flags.
    ///
    /// This is the inverse of [`Self::action_definition`], so a document's `actions` section can
    /// rebuild the same registry state that produced it.
    pub fn register_definition(&mut self, definition: &ActionDefinition) -> bool {
        if !self.register_action(definition.id.clone(), definition.text.clone()) {
            return false;
        }
        if let Some(action) = self.actions.get_mut(&definition.id) {
            action.set_enabled(definition.enabled);
            action.set_checkable(definition.checkable);
            if definition.checkable {
                action.set_checked(definition.checked);
            }
        }
        if !definition.shortcut.is_empty()
            && !self.bind_shortcut(definition.shortcut.clone(), definition.id.clone())
        {
            self.unregister_action(&definition.id);
            return false;
        }
        true
    }

    /// Binds an action to a menu host.
    pub fn bind_action_to_menu(&mut self, action_id: impl Into<String>, menu_id: ObjectId) -> bool {
        self.bind_action(action_id.into(), menu_id, ActionHostKind::Menu)
    }
    /// Binds an action to a toolbar host.
    pub fn bind_action_to_toolbar(
        &mut self,
        action_id: impl Into<String>,
        toolbar_id: ObjectId,
    ) -> bool {
        self.bind_action(action_id.into(), toolbar_id, ActionHostKind::ToolBar)
    }
    /// Binds an action to a button host.
    pub fn bind_action_to_button(
        &mut self,
        action_id: impl Into<String>,
        button_id: ObjectId,
    ) -> bool {
        self.bind_action(action_id.into(), button_id, ActionHostKind::Button)
    }
    /// Returns all bindings associated with a host object id.
    pub fn bindings_for_host(&self, host_id: ObjectId) -> Vec<&ActionBinding> {
        self.bindings.iter().filter(|binding| binding.host_id == host_id).collect()
    }
    fn bind_action(&mut self, action_id: String, host_id: ObjectId, kind: ActionHostKind) -> bool {
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.bindings.push(ActionBinding { action_id, host_id, kind });
        true
    }
}
crate::impl_default_via_new!(ActionManager);
