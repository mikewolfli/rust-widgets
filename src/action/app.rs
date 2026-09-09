//! Bridge between `ShortcutManager` and `ActionManager`.
//!
//! `ActionRouter` connects keyboard shortcut events → action commands,
//! completing the event→shortcut→action chain.
//!
//! Connect `ShortcutManager::shortcut_triggered` → `ActionRouter::route_action_id`.

use crate::action::ActionManager;
use crate::shortcut::Key;
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
        Self { shortcut_mgr, action_mgr }
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
        if !self.shortcut_mgr.register(action_id.clone(), shortcut.clone(), "") {
            self.action_mgr.unregister_action(&action_id);
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
        if self.action_mgr.bind_shortcut_type(&shortcut, action_id.clone()) {
            true
        } else {
            self.shortcut_mgr.unregister(&action_id);
            self.action_mgr.unregister_action(&action_id);
            false
        }
    }

    /// Registers an action, shortcut, and callback as one complete binding.
    pub fn connect_callback<F>(
        &mut self,
        action_id: impl Into<String>,
        shortcut: Shortcut,
        text: impl Into<String>,
        callback: F,
    ) -> bool
    where
        F: FnMut() + Send + Sync + 'static,
    {
        let action_id = action_id.into();
        if !self.connect(action_id.clone(), shortcut, text) {
            return false;
        }
        self.action_mgr
            .action(&action_id)
            .map(|action| {
                action.connect_triggered(callback);
                true
            })
            .unwrap_or(false)
    }

    /// Registers standard Ctrl+Z and Ctrl+Y undo/redo actions.
    pub fn connect_undo_redo<U, R>(&mut self, undo: U, redo: R) -> bool
    where
        U: FnMut() + Send + Sync + 'static,
        R: FnMut() + Send + Sync + 'static,
    {
        let undo_ok = self.connect_callback("undo", Shortcut::ctrl(Key::Z), "Undo", undo);
        let redo_ok = self.connect_callback("redo", Shortcut::ctrl(Key::Y), "Redo", redo);
        undo_ok && redo_ok
    }

    /// Registers a shortcut binding to an existing action.
    pub fn bind_shortcut(&mut self, action_id: impl Into<String>, shortcut: Shortcut) -> bool {
        let action_id = action_id.into();
        if self.action_mgr.action(&action_id).is_none() {
            return false;
        }
        if !self.shortcut_mgr.register(action_id.clone(), shortcut.clone(), "") {
            return false;
        }
        if self.action_mgr.bind_shortcut_type(&shortcut, action_id.clone()) {
            return true;
        }
        self.shortcut_mgr.unregister(&action_id);
        false
    }

    /// Routes a triggered shortcut action id to ActionManager.trigger_action().
    /// Designed to be called from `ShortcutManager::shortcut_triggered` signal.
    pub fn route_action_id(&mut self, action_id: &str) -> bool {
        self.action_mgr.trigger_action(action_id)
    }
}
