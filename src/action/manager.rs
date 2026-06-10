use super::{normalize_shortcut, Action, ActionBinding, ActionHostKind};
use crate::compat::HashMap;
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
    pub fn bind_shortcut(
        &mut self,
        shortcut: impl Into<String>,
        action_id: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.shortcut_to_action.insert(normalize_shortcut(&shortcut.into()), action_id);
        true
    }
    /// Binds a `Shortcut` type to an existing action id. Bridges `shortcut` module with `action`.
    pub fn bind_shortcut_type(
        &mut self,
        shortcut: &Shortcut,
        action_id: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.shortcut_to_action.insert(shortcut.to_string().to_lowercase(), action_id);
        true
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
impl Default for ActionManager {
    fn default() -> Self {
        Self::new()
    }
}
