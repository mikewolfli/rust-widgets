// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::{normalize_shortcut, Action, ActionBinding, ActionHostKind};
use crate::compat::{HashMap, MiniToString, String, Vec};
use crate::core::ObjectId;
use crate::shortcut::Shortcut;
use core::fmt;
/// Registry for actions, shortcuts, and menu/toolbar bindings.
pub struct ActionManager {
    actions: HashMap<String, Action>,
    shortcut_to_action: HashMap<String, String>,
    bindings: Vec<ActionBinding>,
}
impl fmt::Debug for ActionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionManager")
            .field("actions", &self.actions.keys())
            .field("shortcut_to_action", &self.shortcut_to_action)
            .field("bindings", &self.bindings)
            .finish()
    }
}
impl ActionManager {
    /// Creates an empty action manager.
    pub fn new() -> Self {
        Self { actions: HashMap::new(), shortcut_to_action: HashMap::new(), bindings: Vec::new() }
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
        action.set_enabled(enabled);
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
        self.release_shortcuts_for(&action_id);
        self.shortcut_to_action.insert(normalize_shortcut(&shortcut.into()), action_id);
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
        self.release_shortcuts_for(&action_id);
        self.shortcut_to_action.insert(shortcut.to_string().to_lowercase(), action_id);
        true
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
        let Some(action_id) = self.shortcut_to_action.get(&normalize_shortcut(shortcut)) else {
            return false;
        };
        self.actions.get_mut(action_id).map(|action| action.trigger()).unwrap_or(false)
    }
    /// Triggers an action directly by id.
    pub fn trigger_action(&mut self, action_id: &str) -> bool {
        self.actions.get_mut(action_id).map(|action| action.trigger()).unwrap_or(false)
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
