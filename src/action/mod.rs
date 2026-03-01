//! Action/shortcut/command framework.

use std::collections::HashMap;

use crate::core::ObjectId;
use crate::signal::{ConnectionHandle, GenericSignal};

/// Represents a user-invokable command with enabled state and trigger signal.
#[derive(Clone)]
pub struct Action {
    /// Stable action identifier.
    pub id: String,
    /// Human-readable action label.
    pub text: String,
    enabled: bool,
    triggered: GenericSignal,
}

impl Action {
    /// Creates a new enabled action with the provided id and label.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            enabled: true,
            triggered: GenericSignal::new(),
        }
    }

    /// Returns whether this action can currently be triggered.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Updates whether this action can be triggered.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Connects a callback that runs when the action is triggered.
    pub fn connect_triggered<F>(&self, slot: F) -> ConnectionHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.triggered.connect(slot)
    }

    /// Triggers the action when enabled and returns whether it fired.
    pub fn trigger(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.triggered.emit();
        true
    }
}

/// Host container kinds that can expose actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionHostKind {
    /// Action is hosted by a menu.
    Menu,
    /// Action is hosted by a toolbar.
    ToolBar,
}

/// Associates an action with a concrete UI host object.
#[derive(Clone)]
pub struct ActionBinding {
    /// Identifier of the bound action.
    pub action_id: String,
    /// Object id of the host widget.
    pub host_id: ObjectId,
    /// Host kind receiving the action.
    pub kind: ActionHostKind,
}

/// Registry for actions, shortcuts, and menu/toolbar bindings.
pub struct ActionManager {
    actions: HashMap<String, Action>,
    shortcut_to_action: HashMap<String, String>,
    bindings: Vec<ActionBinding>,
}

impl ActionManager {
    /// Creates an empty action manager.
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            shortcut_to_action: HashMap::new(),
            bindings: Vec::new(),
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

    /// Binds a keyboard shortcut to an existing action id.
    pub fn bind_shortcut(&mut self, shortcut: impl Into<String>, action_id: impl Into<String>) -> bool {
        let action_id = action_id.into();
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.shortcut_to_action.insert(normalize_shortcut(&shortcut.into()), action_id);
        true
    }

    /// Resolves and triggers an action by shortcut.
    pub fn trigger_shortcut(&self, shortcut: &str) -> bool {
        let Some(action_id) = self.shortcut_to_action.get(&normalize_shortcut(shortcut)) else {
            return false;
        };
        self.actions
            .get(action_id)
            .map(|action| action.trigger())
            .unwrap_or(false)
    }

            /// Triggers an action directly by id.
    pub fn trigger_action(&self, action_id: &str) -> bool {
        self.actions
            .get(action_id)
            .map(|action| action.trigger())
            .unwrap_or(false)
    }

    /// Binds an action to a menu host.
    pub fn bind_action_to_menu(&mut self, action_id: impl Into<String>, menu_id: ObjectId) -> bool {
        self.bind_action(action_id.into(), menu_id, ActionHostKind::Menu)
    }

    /// Binds an action to a toolbar host.
    pub fn bind_action_to_toolbar(&mut self, action_id: impl Into<String>, toolbar_id: ObjectId) -> bool {
        self.bind_action(action_id.into(), toolbar_id, ActionHostKind::ToolBar)
    }

    /// Returns all bindings associated with a host object id.
    pub fn bindings_for_host(&self, host_id: ObjectId) -> Vec<&ActionBinding> {
        self.bindings
            .iter()
            .filter(|binding| binding.host_id == host_id)
            .collect()
    }

    fn bind_action(&mut self, action_id: String, host_id: ObjectId, kind: ActionHostKind) -> bool {
        if !self.actions.contains_key(&action_id) {
            return false;
        }
        self.bindings.push(ActionBinding {
            action_id,
            host_id,
            kind,
        });
        true
    }
}

impl Default for ActionManager {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_shortcut(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn shortcut_triggers_action() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);
        mgr.action("save")
            .expect("action exists")
            .connect_triggered(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        assert!(mgr.bind_shortcut("Ctrl+S", "save"));
        assert!(mgr.trigger_shortcut("ctrl+s"));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn disabled_action_does_not_trigger() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("build", "Build"));
        assert!(mgr.set_action_enabled("build", false));
        assert!(!mgr.trigger_action("build"));
    }
}
