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

/// Separates a scope key from the action id inside the registry map.
///
/// U+001F (unit separator) cannot occur in an identifier a host writes, so a
/// scoped entry can never collide with a global id spelled
/// `"scope<SEP>id"` by hand.
const SCOPE_SEPARATOR: char = '\u{001f}';

/// The map key one registration occupies.
///
/// A global action keeps exactly the id it always had, so every existing
/// lookup path is unchanged. A scoped action is keyed by
/// `scope + SEP + id`, which lets two hosts hold the same command id without
/// one shadowing the other (M-018).
fn scoped_key(scope: Option<&str>, id: &str) -> String {
    match scope.filter(|scope| !scope.is_empty()) {
        Some(scope) => {
            let mut key = String::with_capacity(scope.len() + 1 + id.len());
            key.push_str(scope);
            key.push(SCOPE_SEPARATOR);
            key.push_str(id);
            key
        }
        None => String::from(id),
    }
}
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
    /// W-P05: reverse index of [`Self::shortcut_to_action`] — the one chord an action currently
    /// holds.
    ///
    /// Every binder releases the action's previous chord before inserting, so an action owns at
    /// most one chord and this is a 1:1 map. That is what makes `release_shortcuts_for` and
    /// `shortcut_for_action` O(1) instead of full-table scans, and it is maintained only by
    /// [`Self::set_chord`], [`Self::release_shortcuts_for`] and [`Self::unregister_action`].
    action_to_chord: HashMap<String, String>,
    /// W-P05: host → indices into [`Self::bindings`], in insertion order, so
    /// `bindings_for_host` is a bucket walk instead of a full-list filter. Appends maintain it
    /// incrementally; the one in-place filter (`unregister_action`) rebuilds it.
    bindings_by_host: HashMap<ObjectId, Vec<usize>>,
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
/// One disagreement between the action registry's reverse index and `ShortcutManager`.
///
/// # Why a structured report (W-08)
///
/// `ActionRouter::connect` writes the same binding to two tables in one call. Two writes in one
/// function body stay in step only as long as nobody adds a third write point, and there is no
/// event-side reconciliation gate for these tables. This report is the runtime check that closes
/// that gap: [`ActionManager::shortcut_reconciliation`] compares the tables and
/// [`crate::action::ActionRouter`] asserts it is empty after every registration it performs, so a
/// future bypass is caught at the call that broke the invariant rather than at the next dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutDisagreement {
    /// The action id one or both tables claim the chord routes to.
    pub action_id: String,
    /// The chord in canonical form (`canonicalize_shortcut_type`).
    pub chord: String,
    /// Whether the action registry's reverse index holds this chord.
    pub action_manager_has: bool,
    /// Whether the shortcut registry's global table holds this chord.
    pub shortcut_manager_has: bool,
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
            action_to_chord: HashMap::new(),
            bindings_by_host: HashMap::new(),
            action_triggered: Signal1::new(),
            action_toggled: Signal1::new(),
            action_enabled_changed: Signal1::new(),
            shortcut_conflict: Signal1::new(),
        }
    }
    /// Registers a new action and returns false if the id already exists.
    pub fn register_action(&mut self, id: impl Into<String>, text: impl Into<String>) -> bool {
        self.register_action_in(None, id.into(), text.into())
    }

    /// Registers an action inside a host scope and returns false when that key exists.
    ///
    /// The scope is a caller-chosen key (a window id, a dialog id, a designer
    /// document id). A scoped registration with the same id as a global one does
    /// not shadow or replace it: [`Self::action_in_scope`] prefers the scoped
    /// entry and falls back to the global one. An empty scope is the global
    /// registration, which keeps this method additive (M-018).
    pub fn register_action_in_scope(
        &mut self,
        scope: impl AsRef<str>,
        id: impl Into<String>,
        text: impl Into<String>,
    ) -> bool {
        self.register_action_in(Some(scope.as_ref()), id.into(), text.into())
    }

    /// The one registration path both public entry points share.
    fn register_action_in(&mut self, scope: Option<&str>, id: String, text: String) -> bool {
        let key = scoped_key(scope, &id);
        if self.actions.contains_key(&key) {
            return false;
        }
        self.actions.insert(key, Action::new(id, text));
        true
    }

    /// Removes a scoped action, leaving the global entry with the same id intact.
    pub fn unregister_action_in_scope(&mut self, scope: &str, id: &str) -> bool {
        let key = scoped_key(Some(scope), id);
        self.actions.remove(&key).is_some()
    }

    /// Removes an action and all of its shortcut/host bindings.
    pub fn unregister_action(&mut self, id: &str) -> bool {
        let removed = self.actions.remove(id).is_some();
        if removed {
            // W-P05: the chord leaves through the reverse index (the forward-index check keeps
            // the removal exact), and the filtered binding list re-derives the host index.
            if let Some(chord) = self.action_to_chord.remove(id) {
                if self.shortcut_to_action.get(&chord).is_some_and(|bound| bound == id) {
                    self.shortcut_to_action.remove(&chord);
                }
            }
            self.bindings.retain(|binding| binding.action_id != id);
            self.rebuild_binding_index();
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

    /// Returns the action a scope resolves to, falling back to the global entry.
    ///
    /// The fallback is what makes a scope an override layer rather than a
    /// second registry: a window-local command shadows the global one only when
    /// it exists, so every host keeps seeing a command it could already see.
    pub fn action_in_scope(&self, scope: &str, id: &str) -> Option<&Action> {
        self.actions.get(&scoped_key(Some(scope), id)).or_else(|| self.actions.get(id))
    }

    /// Mutable counterpart of [`Self::action_in_scope`].
    pub fn action_mut_in_scope(&mut self, scope: &str, id: &str) -> Option<&mut Action> {
        let key = scoped_key(Some(scope), id);
        if self.actions.contains_key(&key) {
            self.actions.get_mut(&key)
        } else {
            self.actions.get_mut(id)
        }
    }

    /// Sets the scoped action's enabled state, falling back to the global entry.
    pub fn set_action_enabled_in_scope(&mut self, scope: &str, id: &str, enabled: bool) -> bool {
        let Some(action) = self.action_mut_in_scope(scope, id) else {
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

    /// Sets the scoped action's visible state, falling back to the global entry.
    ///
    /// The transition is published through the action's own
    /// [`connect_visible_changed`](Action::connect_visible_changed) signal, so a
    /// host repaints from the signal rather than polling the registry.
    pub fn set_action_visible_in_scope(&mut self, scope: &str, id: &str, visible: bool) -> bool {
        let Some(action) = self.action_mut_in_scope(scope, id) else {
            return false;
        };
        action.set_visible(visible);
        true
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
        let shortcut = shortcut.into();
        // EV-X08: a malformed chord is rejected rather than registered under a silently
        // truncated/synthesised spelling, so both string paths give the same verdict.
        let Some(chord) = normalize_shortcut(&shortcut) else {
            log::warn!(
                "[action] refusing malformed shortcut '{shortcut}' for action '{action_id}'"
            );
            return false;
        };
        self.observe_chord_collision(&chord, &action_id);
        self.release_shortcuts_for(&action_id);
        self.set_chord(chord, action_id);
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
        self.set_chord(chord, action_id);
        true
    }

    /// Routes `chord` to `action_id`, keeping the reverse index in step (W-P05).
    ///
    /// If the chord was previously routed to a different action, that action loses it: the
    /// forward index now maps the chord to `action_id` only, so the previous owner's index entry
    /// would otherwise be a stale claim on a chord it no longer answers to.
    fn set_chord(&mut self, chord: String, action_id: String) {
        if let Some(previous) = self.shortcut_to_action.insert(chord.clone(), action_id.clone()) {
            if previous != action_id
                && self.action_to_chord.get(&previous).is_some_and(|bound| *bound == chord)
            {
                self.action_to_chord.remove(&previous);
            }
        }
        self.action_to_chord.insert(action_id, chord);
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
    /// every chord it has ever been bound to. W-P05: the reverse index answers in O(1) where
    /// the old path filtered the whole forward table; the forward-index check keeps the removal
    /// exact even if the two tables were ever to disagree about which chord the action holds.
    fn release_shortcuts_for(&mut self, action_id: &str) {
        if let Some(chord) = self.action_to_chord.remove(action_id) {
            if self.shortcut_to_action.get(&chord).is_some_and(|bound| bound == action_id) {
                self.shortcut_to_action.remove(&chord);
            }
        }
    }
    /// Resolves and triggers an action by shortcut string.
    pub fn trigger_shortcut(&mut self, shortcut: &str) -> bool {
        // EV-X08: a malformed chord can never match a registered one, so it is rejected with the
        // same verdict the typed path gives rather than normalised into a different key.
        let Some(chord) = normalize_shortcut(shortcut) else {
            log::warn!("[action] refusing malformed shortcut '{shortcut}'");
            return false;
        };
        let Some(action_id) = self.shortcut_to_action.get(&chord).cloned() else {
            return false;
        };
        self.trigger_action(&action_id)
    }
    /// Triggers an action bound to a typed [`Shortcut`], preserving the physical
    /// [`Modifiers::CTRL`](crate::shortcut::Modifiers::CTRL) bit (unlike
    /// `trigger_shortcut`, whose string input folds `Ctrl` onto `Primary`). This
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
        self.trigger_key(action_id, action_id)
    }

    /// Triggers the scoped action `id`, falling back to the global entry.
    ///
    /// The published payload is always the command id (never the internal
    /// scoped key), so listeners receive the same id they would for a global
    /// action.
    pub fn trigger_action_in_scope(&mut self, scope: &str, id: &str) -> bool {
        let scoped = scoped_key(Some(scope), id);
        let key = if self.actions.contains_key(&scoped) { scoped } else { String::from(id) };
        self.trigger_key(&key, id)
    }

    /// The one trigger path, keyed by the map key and publishing `publish_id`.
    fn trigger_key(&mut self, key: &str, publish_id: &str) -> bool {
        let Some(action) = self.actions.get_mut(key) else {
            return false;
        };
        let was_checked = action.is_checked();
        if !action.trigger() {
            return false;
        }
        let checkable = action.is_checkable();
        let now_checked = action.is_checked();
        let toggled = checkable && now_checked != was_checked;
        let id = String::from(publish_id);
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
        // A scoped entry lives under an internal key and is deliberately not a
        // second published command: `action_ids` names the global registry, as
        // it did before scopes existed.
        self.actions.keys().filter(|key| !key.contains(SCOPE_SEPARATOR)).collect()
    }

    /// Every action id registered in `scope`, in map order.
    pub fn action_ids_in_scope(&self, scope: &str) -> Vec<&String> {
        let prefix = String::from(scope);
        self.actions
            .iter()
            .filter_map(|(key, action)| {
                key.strip_prefix(prefix.as_str())
                    .and_then(|rest| rest.strip_prefix(SCOPE_SEPARATOR))
                    .map(|_| &action.id)
            })
            .collect()
    }

    /// Every registered command with its current chord, sorted by id.
    ///
    /// Sorted because a document writer must be stable: two exports of one registry have to be
    /// byte-identical, and `HashMap` iteration order is not.
    pub fn definitions(&self) -> Vec<ActionDefinition> {
        let mut definitions: Vec<ActionDefinition> =
            self.action_ids().into_iter().filter_map(|id| self.action_definition(id)).collect();
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

    /// Compares the reverse index against [`crate::shortcut::ShortcutManager`]'s global table.
    ///
    /// An empty result is the invariant `ActionRouter::connect` maintains: every chord it registers
    /// is present in both tables and routes to the same action id. A non-empty result names each
    /// disagreement, so the reconciliation can be asserted (debug builds) and reported (release
    /// builds) instead of silently drifting.
    ///
    /// Scoped shortcut registrations are excluded on purpose: this registry has no scope dimension,
    /// so a chord bound in a window scope is not a disagreement with the global table.
    pub fn shortcut_reconciliation(
        &self,
        shortcuts: &crate::shortcut::ShortcutManager,
    ) -> Vec<ShortcutDisagreement> {
        let mut reported: Vec<String> = Vec::new();
        let mut disagreements = Vec::new();

        // Registry -> action index: the registry is authoritative for *which* chords exist.
        for (shortcut, action_id) in shortcuts.all_shortcuts() {
            let chord = canonicalize_shortcut_type(shortcut);
            if reported.contains(&chord) {
                continue;
            }
            reported.push(chord.clone());
            let indexed = self.shortcut_to_action.get(&chord);
            if indexed != Some(action_id) {
                disagreements.push(ShortcutDisagreement {
                    action_id: action_id.clone(),
                    chord,
                    action_manager_has: indexed.is_some(),
                    shortcut_manager_has: true,
                });
            }
        }

        // Action index -> registry: catches a chord the reverse index holds that the registry
        // never accepted (the shape a bypassed `connect` produces).
        for (chord, action_id) in &self.shortcut_to_action {
            if reported.contains(chord) {
                continue;
            }
            disagreements.push(ShortcutDisagreement {
                action_id: action_id.clone(),
                chord: chord.clone(),
                action_manager_has: true,
                shortcut_manager_has: false,
            });
        }

        disagreements
    }

    /// The canonical chord currently bound to `action_id`, if any.
    pub fn shortcut_for_action(&self, action_id: &str) -> Option<&String> {
        // W-P05: the reverse index locates the action's chord in O(1), and `get_key_value`
        // hands back the forward table's actual key — the exact chord `trigger_shortcut`
        // resolves, which is what the old linear scan returned. A missing forward entry
        // degrades to `None` exactly as the scan's miss did.
        let chord = self.action_to_chord.get(action_id)?;
        self.shortcut_to_action.get_key_value(chord).map(|(key, _)| key)
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
            action.set_visible(definition.visible);
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
    ///
    /// W-P05: answered from the per-host index, whose buckets are appended in insertion order —
    /// the same order the full-list filter produced.
    pub fn bindings_for_host(&self, host_id: ObjectId) -> Vec<&ActionBinding> {
        match self.bindings_by_host.get(&host_id) {
            Some(indices) => indices.iter().map(|&index| &self.bindings[index]).collect(),
            None => Vec::new(),
        }
    }
    fn bind_action(&mut self, action_id: String, host_id: ObjectId, kind: ActionHostKind) -> bool {
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.bindings.push(ActionBinding { action_id, host_id, kind });
        self.bindings_by_host.entry(host_id).or_default().push(self.bindings.len() - 1);
        true
    }

    /// Rebuilds the per-host binding index from [`Self::bindings`] in one pass (W-P05). Only
    /// needed when the list is filtered in place (`unregister_action`); appends maintain it
    /// incrementally, and the pass preserves insertion order within each bucket.
    fn rebuild_binding_index(&mut self) {
        self.bindings_by_host.clear();
        for (index, binding) in self.bindings.iter().enumerate() {
            self.bindings_by_host.entry(binding.host_id).or_default().push(index);
        }
    }
}
crate::impl_default_via_new!(ActionManager);
