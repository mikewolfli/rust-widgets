//! Bridge between `ShortcutManager` and `ActionManager`.
//!
//! `ActionRouter` connects keyboard shortcut events → action commands,
//! completing the event→shortcut→action chain.
//!
//! Connect `ShortcutManager::shortcut_triggered` → `ActionRouter::route_action_id`.

use crate::action::ActionManager;
use crate::shortcut::Shortcut;

/// Routes keyboard shortcuts to registered actions, bridging `shortcut` and `action` modules.
pub struct ActionRouter<'a> {
    shortcut_mgr: &'a mut crate::shortcut::ShortcutManager,
    action_mgr: &'a mut ActionManager,
}

impl<'a> ActionRouter<'a> {
    /// Creates a new router that connects the two managers.
    pub fn new(
        shortcut_mgr: &'a mut crate::shortcut::ShortcutManager,
        action_mgr: &'a mut ActionManager,
    ) -> Self {
        Self {
            shortcut_mgr,
            action_mgr,
        }
    }

    /// Registers an action AND its keyboard shortcut in one step.
    /// Returns `true` if both registration and binding succeeded.
    pub fn connect(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        text: impl Into<String>,
    ) -> bool {
        let action_id = action_id.into();
        if !self.action_mgr.register_action(action_id.clone(), text) {
            return false;
        }
        if !self
            .shortcut_mgr
            .register(action_id.clone(), shortcut.clone(), "")
        {
            return false;
        }
        // Also register in ActionManager's shortcut map
        //
        // NOTE(P4-19): The shortcut is registered in two places
        // intentionally for consistency:
        //   1. `shortcut_mgr.register(...)` — enables shortcut detection
        //      from keyboard input in the shortcut system.
        //   2. `action_mgr.bind_shortcut_type(...)` — enables reverse
        //      lookup (shortcut → action id) inside ActionManager.
        // Both registrations are required for the router to work
        // correctly in both directions.
        self.action_mgr.bind_shortcut_type(&shortcut, action_id)
    }

    /// Registers a shortcut binding to an existing action.
    pub fn bind_shortcut(&mut self, action_id: impl Into<String>, shortcut: Shortcut) -> bool {
        let action_id = action_id.into();
        if self.action_mgr.action(&action_id).is_none() {
            return false;
        }
        if !self
            .shortcut_mgr
            .register(action_id.clone(), shortcut.clone(), "")
        {
            return false;
        }
        self.action_mgr.bind_shortcut_type(&shortcut, action_id)
    }

    /// Routes a triggered shortcut action id to ActionManager.trigger_action().
    /// Designed to be called from `ShortcutManager::shortcut_triggered` signal.
    pub fn route_action_id(&mut self, action_id: &str) -> bool {
        self.action_mgr.trigger_action(action_id)
    }
}
