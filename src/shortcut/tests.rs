use super::*;
use crate::event::Event;
#[test]
fn test_shortcut_parsing() {
    let shortcut = Shortcut::from_string("Ctrl+A").unwrap();
    assert_eq!(shortcut.key, Key::A);
    assert!(shortcut.modifiers.contains(Modifiers::CTRL));
    assert!(!shortcut.modifiers.contains(Modifiers::ALT));
    let shortcut = Shortcut::from_string("Alt+F4").unwrap();
    assert_eq!(shortcut.key, Key::F4);
    assert!(shortcut.modifiers.contains(Modifiers::ALT));
    let shortcut = Shortcut::from_string("Ctrl+Shift+S").unwrap();
    assert_eq!(shortcut.key, Key::S);
    assert!(shortcut.modifiers.contains(Modifiers::CTRL));
    assert!(shortcut.modifiers.contains(Modifiers::SHIFT));
}
#[test]
fn test_shortcut_to_string() {
    let shortcut = Shortcut::ctrl(Key::A);
    assert_eq!(shortcut.to_string(), "Ctrl+A");
    let shortcut = Shortcut::new(Key::F1, Modifiers::CTRL | Modifiers::ALT);
    assert_eq!(shortcut.to_string(), "Ctrl+Alt+F1");
}
#[test]
fn test_shortcut_manager_register() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register("action1", Shortcut::ctrl(Key::A), "Action 1"));
    assert!(manager.has_action("action1"));
    assert!(manager.has_shortcut(&Shortcut::ctrl(Key::A)));
    assert!(manager.register("action1", Shortcut::ctrl(Key::A), "Action 1"));
    assert!(!manager.register("action2", Shortcut::ctrl(Key::A), "Action 2"));
}
#[test]
fn test_shortcut_manager_trigger() {
    let mut manager = ShortcutManager::new();
    manager.register("action1", Shortcut::ctrl(Key::A), "Action 1");
    assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
    assert!(!manager.handle_key_event(Key::B, Modifiers::CTRL));
    assert!(!manager.handle_key_event(Key::A, Modifiers::ALT));
}

#[test]
fn test_shortcut_manager_handles_framework_key_press_event() {
    let mut manager = ShortcutManager::new();
    manager.register("save", Shortcut::ctrl(Key::S), "Save");

    assert!(manager.handle_event(&Event::KeyPress { key: 83, modifiers: 2 }));
    assert!(manager.handle_event(&Event::KeyPress { key: 115, modifiers: 2 }));
    assert!(!manager.handle_event(&Event::KeyPress { key: 83, modifiers: 4 }));
    assert!(!manager.handle_event(&Event::KeyRelease { key: 83, modifiers: 2 }));
}

#[test]
fn test_shortcut_key_code_conversion_covers_navigation_keys() {
    assert_eq!(Key::from_key_code(37), Some(Key::Left));
    assert_eq!(Key::from_key_code(39), Some(Key::Right));
    assert_eq!(Key::from_key_code(13), Some(Key::Enter));
    assert_eq!(Key::from_key_code(9999), None);
}
#[test]
fn test_shortcut_manager_enable_disable() {
    let mut manager = ShortcutManager::new();
    manager.register("action1", Shortcut::ctrl(Key::A), "Action 1");
    assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
    manager.disable("action1");
    assert!(!manager.handle_key_event(Key::A, Modifiers::CTRL));
    manager.enable("action1");
    assert!(manager.handle_key_event(Key::A, Modifiers::CTRL));
}
#[test]
fn test_shortcut_conflict_detection() {
    let shortcut1 = Shortcut::ctrl(Key::A);
    let shortcut2 = Shortcut::ctrl(Key::A);
    let shortcut3 = Shortcut::alt(Key::A);
    assert!(shortcut1.conflicts_with(&shortcut2));
    assert!(!shortcut1.conflicts_with(&shortcut3));
}
