// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;
use crate::event::Event;
#[test]
fn test_shortcut_parsing() {
    // `Ctrl` and `Cmd` both parse to PRIMARY: one declaration, every platform.
    let shortcut = Shortcut::from_string("Ctrl+A").unwrap();
    assert_eq!(shortcut.key, Key::A);
    assert!(shortcut.modifiers.contains(Modifiers::PRIMARY));
    assert!(!shortcut.modifiers.contains(Modifiers::ALT));
    assert_eq!(Shortcut::from_string("Cmd+A").unwrap(), shortcut);
    assert_eq!(Shortcut::from_string("Primary+A").unwrap(), shortcut);
    let shortcut = Shortcut::from_string("Alt+F4").unwrap();
    assert_eq!(shortcut.key, Key::F4);
    assert!(shortcut.modifiers.contains(Modifiers::ALT));
    let shortcut = Shortcut::from_string("Ctrl+Shift+S").unwrap();
    assert_eq!(shortcut.key, Key::S);
    assert!(shortcut.modifiers.contains(Modifiers::PRIMARY));
    assert!(shortcut.modifiers.contains(Modifiers::SHIFT));
    // "Meta"/"Win" stay a distinct, physical modifier.
    let shortcut = Shortcut::from_string("Meta+X").unwrap();
    assert!(shortcut.modifiers.contains(Modifiers::META));
    assert!(!shortcut.modifiers.contains(Modifiers::PRIMARY));
}
#[test]
fn test_shortcut_to_string() {
    // `format_shortcut` is the canonical, platform-independent form.
    let shortcut = Shortcut::ctrl(Key::A);
    assert_eq!(shortcut.to_string(), "Ctrl+A");
    let shortcut = Shortcut::new(Key::F1, Modifiers::CTRL | Modifiers::ALT);
    assert_eq!(shortcut.to_string(), "Ctrl+Alt+F1");
    let shortcut = Shortcut::primary(Key::Z);
    assert_eq!(shortcut.to_string(), "Primary+Z");
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

/// `PRIMARY` renders as macOS glyphs on Mac and as spelled-out modifiers
/// elsewhere. Both branches are asserted on every host, so a change to either
/// convention is caught without a Windows or macOS machine.
#[test]
fn primary_shortcut_renders_per_platform() {
    use super::PlatformShortcutStyle::{Desktop, Mac};

    let undo = Shortcut::primary(Key::Z);
    assert_eq!(format_shortcut_for_platform(&undo, Mac), "⌘Z");
    assert_eq!(format_shortcut_for_platform(&undo, Desktop), "Ctrl+Z");

    let redo = Shortcut::primary_shift(Key::Z);
    assert_eq!(format_shortcut_for_platform(&redo, Mac), "⇧⌘Z");
    assert_eq!(format_shortcut_for_platform(&redo, Desktop), "Ctrl+Shift+Z");

    let save = Shortcut::primary(Key::S);
    assert_eq!(format_shortcut_for_platform(&save, Mac), "⌘S");
    assert_eq!(format_shortcut_for_platform(&save, Desktop), "Ctrl+S");
}

/// A physical-Control shortcut stays `Ctrl` on desktop and `⌃` on macOS, and is
/// not conflated with `PRIMARY`.
#[test]
fn physical_control_is_not_primary() {
    use super::PlatformShortcutStyle::{Desktop, Mac};

    let control_c = Shortcut::ctrl(Key::C);
    assert_eq!(format_shortcut_for_platform(&control_c, Desktop), "Ctrl+C");
    assert_eq!(format_shortcut_for_platform(&control_c, Mac), "⌃C");

    let primary_c = Shortcut::primary(Key::C);
    assert_ne!(primary_c, control_c);
}

/// macOS menus print arrow/enter keys as glyphs rather than words.
#[test]
fn mac_style_uses_glyphs_for_named_keys() {
    use super::PlatformShortcutStyle::{Desktop, Mac};

    let enter = Shortcut::new(Key::Enter, Modifiers::PRIMARY);
    assert_eq!(format_shortcut_for_platform(&enter, Mac), "⌘↩");
    assert_eq!(format_shortcut_for_platform(&enter, Desktop), "Ctrl+Enter");

    let up = Shortcut::new(Key::Up, Modifiers::ALT);
    assert_eq!(format_shortcut_for_platform(&up, Mac), "⌥↑");
    assert_eq!(format_shortcut_for_platform(&up, Desktop), "Alt+Up");
}

/// The style the library picks by default must match the host it is built for.
#[test]
fn current_style_matches_host() {
    // The compile-target mapping lives in `src/platform/` (principle #36); this
    // test only asserts that `current()` surfaces it faithfully.
    let expected = crate::platform::types::compile_target_shortcut_style();
    assert_eq!(PlatformShortcutStyle::current(), expected);
}

/// A `KeyPress` carrying the Meta/Command bit must match a `Primary` shortcut.
/// This is the link that makes typing Cmd+Z on macOS hit `Shortcut::primary`.
///
/// The Meta bit is the *macOS* primary accelerator. Under the desktop conventions
/// the primary accelerator is Control instead, and the lookup lifts a Control
/// press into `PRIMARY` to reach the same binding. The style is read at runtime
/// (principle #68: no `cfg(target_os)` outside `src/platform/`), so the expected
/// mapping is asserted per style rather than assuming one host's convention.
#[test]
fn command_modifier_bit_matches_primary_shortcut() {
    let mut manager = ShortcutManager::new();
    manager.register("undo", Shortcut::primary(Key::Z), "Undo");

    // Establish each link of the chain explicitly, so a failure names the step.
    assert_eq!(Key::from_key_code(90), Some(Key::Z));
    // Modifier bit 0b1000 is the Meta/Command slot. It lifts into PRIMARY so a
    // Command key event matches `Shortcut::primary`, and it also implies CTRL:
    // a widget that only looks at the control bit (as the editor's own key
    // handling does) still recognises `Cmd+C` as copy.
    let command = Modifiers::from_event_bits(0b1000);
    assert!(command.contains(Modifiers::PRIMARY));
    assert!(command.contains(Modifiers::CTRL));
    assert_eq!(command, Modifiers::PRIMARY | Modifiers::CTRL);

    // Shift+Command must not fire the unshifted command. This holds under both
    // conventions: the extra SHIFT bit survives normalisation either way.
    assert!(!manager.handle_event(&Event::KeyPress { key: 90, modifiers: 0b1001 }));

    match PlatformShortcutStyle::current() {
        // macOS: the Meta/Command bit *is* the primary accelerator.
        PlatformShortcutStyle::Mac => {
            assert!(manager.handle_event(&Event::KeyPress { key: 90, modifiers: 0b1000 }));
        }
        // Windows/Linux: Control is the primary accelerator, so a Control press is
        // lifted into PRIMARY and matches, while a bare Meta press is not this
        // platform's primary accelerator and must not fire the binding.
        PlatformShortcutStyle::Desktop => {
            assert!(manager.handle_event(&Event::KeyPress { key: 90, modifiers: 0b0010 }));
            assert!(!manager.handle_event(&Event::KeyPress { key: 90, modifiers: 0b1000 }));
        }
    }

    // A physical-Control key event always matches a `Shortcut::ctrl` binding.
    let mut control_manager = ShortcutManager::new();
    control_manager.register("physical", Shortcut::ctrl(Key::Z), "Control only");
    assert!(control_manager.handle_event(&Event::KeyPress { key: 90, modifiers: 0b0010 }));
}
