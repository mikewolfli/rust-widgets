// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Action/shortcut/command framework.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/capability/properties.rs:1` (the `action` capability's property contract).
mod app;
mod manager;
mod types;
pub use app::ActionRouter;
pub use manager::ActionManager;
pub(crate) use types::{canonicalize_shortcut_type, normalize_shortcut};
pub use types::{Action, ActionBinding, ActionHostKind};
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
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
    /// Rebinding an action releases the chord it was bound to before.
    ///
    /// `bind_shortcut` only inserted into `shortcut_to_action`, so an action kept every
    /// chord it had ever been given: binding `Ctrl+S` and then `Ctrl+Shift+S` left **both**
    /// live, and the stale accelerator kept firing after the rebind. `ShortcutManager::register`
    /// already released the previous chord; the action-side registry did not.
    #[test]
    fn rebinding_a_shortcut_releases_the_previous_chord() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);
        mgr.action("save").expect("action exists").connect_triggered(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        assert!(mgr.bind_shortcut("Ctrl+S", "save"));
        assert!(mgr.trigger_shortcut("Ctrl+S"));
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        assert!(mgr.bind_shortcut("Ctrl+Shift+S", "save"));
        assert!(!mgr.trigger_shortcut("Ctrl+S"), "the old chord must be released by the rebind");
        assert_eq!(counter.load(Ordering::SeqCst), 1, "the stale chord must not fire");

        assert!(mgr.trigger_shortcut("Ctrl+Shift+S"), "the new chord must work");
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// The type-based binder follows the same rule as the string-based one.
    #[test]
    fn rebinding_a_shortcut_type_releases_the_previous_chord() {
        use crate::shortcut::{Key, Shortcut};
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));

        assert!(mgr.bind_shortcut_type(&Shortcut::primary(Key::S), "save"));
        assert!(mgr.bind_shortcut_type(&Shortcut::primary_shift(Key::S), "save"));

        assert!(!mgr.trigger_shortcut("Ctrl+S"), "the old chord must be released by the rebind");
        assert!(mgr.trigger_shortcut("Ctrl+Shift+S"));
    }

    /// A type-bound chord must be reachable by its own canonical string spelling.
    ///
    /// Pins the defect: `bind_shortcut_type` keyed the map with `shortcut.to_string().to_lowercase()`
    /// while `trigger_shortcut` looked up through `normalize_shortcut`. `Shortcut::to_string`
    /// renders `Modifiers::PRIMARY` as the literal `"Primary"`, so a `primary`-bound chord stored
    /// `"primary+s"` and a lookup of `normalize_shortcut("Primary+S")` -> `"primary+s"` would have
    /// matched only by luck; the string spelling `"Ctrl+S"` (which `from_string` maps to the same
    /// `Modifiers::PRIMARY`) normalizes to `"primary+s"` too, so both spellings must reach it.
    #[test]
    fn a_type_bound_shortcut_is_reachable_by_its_string_spelling() {
        use crate::shortcut::{Key, Shortcut};
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));

        assert!(mgr.bind_shortcut_type(&Shortcut::primary(Key::S), "save"));
        // Both the canonical spelling and the platform-neutral `Ctrl` spelling resolve to the one
        // normalized key, which is the whole point of routing both paths through
        // `normalize_shortcut`.
        assert!(mgr.trigger_shortcut("Primary+S"), "the canonical spelling must reach it");
        assert!(mgr.trigger_shortcut("Ctrl+S"), "and so must the `Ctrl` spelling");
    }

    /// Modifier order and Option/Alt aliases must normalize to one chord key.
    ///
    /// `Shortcut::from_string` treats `"Ctrl+Shift+S"` and `"Shift+Ctrl+S"` as the
    /// same value; the action registry must key them identically too, or a binding
    /// entered with one spelling cannot be triggered with the other. `Option` is the
    /// macOS name for the Alt key, so the two spellings must agree as well.
    #[test]
    fn string_shortcuts_normalize_modifier_order_and_aliases() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);
        mgr.action("save").expect("action exists").connect_triggered(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        assert!(mgr.bind_shortcut("Ctrl+Shift+S", "save"));
        assert!(mgr.trigger_shortcut("Shift+Ctrl+S"), "modifier order must not matter");
        assert!(mgr.trigger_shortcut("ctrl+shift+s"), "case must not matter");
        assert_eq!(counter.load(Ordering::SeqCst), 2);

        assert!(mgr.bind_shortcut("Alt+A", "save"));
        assert!(mgr.trigger_shortcut("Option+A"), "Alt and Option are the same key");
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    /// `Shortcut::ctrl(S)` and `Shortcut::primary(S)` are different chords and must
    /// not overwrite each other in the action registry. The string API keeps folding
    /// `Ctrl` onto `Primary`, so a physical-Control binding is reachable through the
    /// typed trigger, not through the `"Ctrl+S"` string spelling.
    #[test]
    fn typed_ctrl_and_primary_are_distinct_bindings() {
        use crate::shortcut::{Key, Shortcut};
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("cut", "Cut"));
        assert!(mgr.register_action("save", "Save"));
        let cut = Arc::new(AtomicUsize::new(0));
        let save = Arc::new(AtomicUsize::new(0));
        {
            let cut = Arc::clone(&cut);
            mgr.action("cut").expect("cut exists").connect_triggered(move || {
                cut.fetch_add(1, Ordering::SeqCst);
            });
            let save = Arc::clone(&save);
            mgr.action("save").expect("save exists").connect_triggered(move || {
                save.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(mgr.bind_shortcut_type(&Shortcut::ctrl(Key::S), "cut"));
        assert!(mgr.bind_shortcut_type(&Shortcut::primary(Key::S), "save"));

        // Physical Control and PRIMARY are independent: each typed chord triggers only
        // its own action.
        assert!(mgr.trigger_shortcut_type(&Shortcut::ctrl(Key::S)));
        assert_eq!(cut.load(Ordering::SeqCst), 1);
        assert_eq!(save.load(Ordering::SeqCst), 0);

        assert!(mgr.trigger_shortcut_type(&Shortcut::primary(Key::S)));
        assert_eq!(cut.load(Ordering::SeqCst), 1);
        assert_eq!(save.load(Ordering::SeqCst), 1);

        // The string "Ctrl+S" means PRIMARY (the isolated string convention), so it
        // fires save, not the physical-Control action.
        assert!(mgr.trigger_shortcut("Ctrl+S"));
        assert_eq!(cut.load(Ordering::SeqCst), 1);
        assert_eq!(save.load(Ordering::SeqCst), 2);
    }

    /// Two different actions keep their own chords.
    ///
    /// The release is scoped to the action being rebound, not global.
    #[test]
    fn rebinding_one_action_does_not_disturb_another() {
        let mut mgr = ActionManager::new();
        assert!(mgr.register_action("save", "Save"));
        assert!(mgr.register_action("open", "Open"));

        assert!(mgr.bind_shortcut("Ctrl+S", "save"));
        assert!(mgr.bind_shortcut("Ctrl+O", "open"));
        assert!(mgr.bind_shortcut("Ctrl+Shift+S", "save"));

        assert!(mgr.trigger_shortcut("Ctrl+O"), "the other action's chord must survive");
        assert!(!mgr.trigger_shortcut("Ctrl+S"));
        assert!(mgr.trigger_shortcut("Ctrl+Shift+S"));
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

    #[test]
    fn action_router_connects_undo_redo_callbacks() {
        use crate::shortcut::{Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutManager};
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};

        let style = crate::platform::platform_facts().shortcut_style();

        let mut shortcut_manager = ShortcutManager::new();
        let mut action_manager = ActionManager::new();
        let undo_count = Arc::new(AtomicUsize::new(0));
        let redo_count = Arc::new(AtomicUsize::new(0));
        let undo_ref = Arc::clone(&undo_count);
        let redo_ref = Arc::clone(&redo_count);
        let mut router = ActionRouter::new(&mut shortcut_manager, &mut action_manager);
        assert!(router.connect_undo_redo(
            move || {
                undo_ref.fetch_add(1, Ordering::SeqCst);
            },
            move || {
                redo_ref.fetch_add(1, Ordering::SeqCst);
            },
        ));

        // Undo is always the platform PRIMARY + Z, never the physical Control key.
        let undo = shortcut_manager.get_shortcut("undo").expect("undo registered").clone();
        assert!(undo.modifiers.contains(Modifiers::PRIMARY), "undo must be Primary+Z");
        assert!(!undo.modifiers.contains(Modifiers::CTRL), "undo must not be physical Control");
        assert_eq!(undo.key, Key::Z);

        // Redo follows the explicit platform convention: Cmd+Shift+Z on macOS,
        // Ctrl+Y (Primary+Y) on Windows/Linux.
        let expected_redo = match style {
            PlatformShortcutStyle::Mac => Shortcut::primary_shift(Key::Z),
            PlatformShortcutStyle::Desktop => Shortcut::primary(Key::Y),
        };
        assert_eq!(
            shortcut_manager.get_shortcut("redo").expect("redo registered"),
            &expected_redo,
            "redo must follow the platform convention",
        );

        // The neutral `Ctrl` string spelling parses to PRIMARY, so it reaches undo.
        assert!(action_manager.trigger_shortcut("Ctrl+Z"));
        assert_eq!(undo_count.load(Ordering::SeqCst), 1);

        let redo_spelling = match style {
            PlatformShortcutStyle::Mac => "Ctrl+Shift+Z",
            PlatformShortcutStyle::Desktop => "Ctrl+Y",
        };
        assert!(action_manager.trigger_shortcut(redo_spelling));
        assert_eq!(redo_count.load(Ordering::SeqCst), 1);

        // A real key event with the platform PRIMARY modifier routes through the
        // shortcut manager.
        assert!(shortcut_manager.handle_key_event(Key::Z, Modifiers::PRIMARY));
        let (redo_key, redo_mods) = match style {
            PlatformShortcutStyle::Mac => (Key::Z, Modifiers::PRIMARY | Modifiers::SHIFT),
            PlatformShortcutStyle::Desktop => (Key::Y, Modifiers::PRIMARY),
        };
        assert!(shortcut_manager.handle_key_event(redo_key, redo_mods));
    }
}
