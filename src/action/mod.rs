//! Action/shortcut/command framework.
mod app;
mod manager;
mod types;
pub use app::ActionRouter;
pub use manager::ActionManager;
pub(crate) use types::normalize_shortcut;
pub use types::{Action, ActionBinding, ActionHostKind};
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
        mgr.action("save").expect("action exists").connect_triggered(move || {
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
    #[test]
    fn checkable_action_toggles_on_trigger() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("pin", "Pin"));
        let toggles = Arc::new(AtomicUsize::new(0));
        {
            let action = mgr.action_mut("pin").expect("action exists");
            action.set_checkable(true);
            let toggles_ref = Arc::clone(&toggles);
            action.connect_toggled(move |_| {
                toggles_ref.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert!(mgr.trigger_action("pin"));
        assert_eq!(mgr.action("pin").map(|a| a.is_checked()), Some(true));
        assert!(mgr.trigger_action("pin"));
        assert_eq!(mgr.action("pin").map(|a| a.is_checked()), Some(false));
        assert_eq!(toggles.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn action_can_bind_to_button_host() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("run", "Run"));
        assert!(mgr.bind_action_to_button("run", 42));
        let bindings = mgr.bindings_for_host(42);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].kind, ActionHostKind::Button);
    }
    #[test]
    fn enabled_changed_signal_emits_on_state_transition_only() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("fmt", "Format"));
        let hits = Arc::new(AtomicUsize::new(0));
        {
            let action = mgr.action("fmt").expect("action exists");
            let hits_ref = Arc::clone(&hits);
            action.connect_enabled_changed(move |_| {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert!(mgr.set_action_enabled("fmt", false));
        assert!(mgr.set_action_enabled("fmt", false));
        assert!(mgr.set_action_enabled("fmt", true));
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn action_can_bind_to_menu_and_toolbar_hosts() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("copy", "Copy"));
        assert!(mgr.bind_action_to_menu("copy", 100));
        assert!(mgr.bind_action_to_toolbar("copy", 200));
        let menu_bindings = mgr.bindings_for_host(100);
        assert_eq!(menu_bindings.len(), 1);
        assert_eq!(menu_bindings[0].kind, ActionHostKind::Menu);
        let toolbar_bindings = mgr.bindings_for_host(200);
        assert_eq!(toolbar_bindings.len(), 1);
        assert_eq!(toolbar_bindings[0].kind, ActionHostKind::ToolBar);
    }
}
