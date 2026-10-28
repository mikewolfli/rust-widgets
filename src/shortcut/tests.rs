// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;
// `mini` is `no_std`: `Vec`/`Box` come from `compat`, not the prelude.
use crate::compat::{Box, MiniToString, Vec};
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
    // `format_shortcut` is the canonical, platform-independent form, and is an inverse of
    // `from_string`. A *physical* Control shortcut renders as `Control` so it is distinguishable
    // from the `Ctrl` input alias of PRIMARY (N-K-03).
    let shortcut = Shortcut::ctrl(Key::A);
    assert_eq!(shortcut.to_string(), "Control+A");
    let shortcut = Shortcut::new(Key::F1, Modifiers::CTRL | Modifiers::ALT);
    assert_eq!(shortcut.to_string(), "Control+Alt+F1");
    let shortcut = Shortcut::primary(Key::Z);
    assert_eq!(shortcut.to_string(), "Primary+Z");
}

/// Every representable key's canonical spelling must round-trip through
/// `from_string`. The page keys used to format as `PgUp`/`PgDn`, which the parser
/// did not accept, so a bound chord's own canonical spelling could not be parsed back.
#[test]
fn every_key_canonical_spelling_round_trips() {
    use Key::*;
    let keys = [
        None,
        A,
        B,
        C,
        D,
        E,
        F,
        G,
        H,
        I,
        J,
        K,
        L,
        M,
        N,
        O,
        P,
        Q,
        R,
        S,
        T,
        U,
        V,
        W,
        X,
        Y,
        Z,
        Num0,
        Num1,
        Num2,
        Num3,
        Num4,
        Num5,
        Num6,
        Num7,
        Num8,
        Num9,
        F1,
        F2,
        F3,
        F4,
        F5,
        F6,
        F7,
        F8,
        F9,
        F10,
        F11,
        F12,
        Escape,
        Tab,
        Enter,
        Space,
        Backspace,
        Delete,
        Insert,
        Home,
        End,
        PageUp,
        PageDown,
        Left,
        Right,
        Up,
        Down,
        Minus,
        Equals,
        LeftBracket,
        RightBracket,
        Semicolon,
        Quote,
        Comma,
        Period,
        Slash,
        Backslash,
    ];
    for key in keys {
        let spelling = key.format_key();
        assert_eq!(
            Key::from_string(spelling),
            Some(key),
            "canonical spelling {spelling:?} must round-trip",
        );
    }
}

/// Page keys format as `PageUp`/`PageDown` and parse back; the platform display
/// glyphs are display-only and are not accepted as canonical input.
#[test]
fn page_keys_round_trip_without_glyphs() {
    assert_eq!(Key::PageUp.format_key(), "PageUp");
    assert_eq!(Key::from_string("PageUp"), Some(Key::PageUp));
    assert_eq!(Key::PageDown.format_key(), "PageDown");
    assert_eq!(Key::from_string("PageDown"), Some(Key::PageDown));

    // A shortcut using a page key stringifies and parses back to itself.
    let shortcut = Shortcut::primary(Key::PageUp);
    assert_eq!(shortcut.to_string(), "Primary+PageUp");
    assert_eq!(Shortcut::from_string(&shortcut.to_string()), Some(shortcut));

    assert_eq!(Key::from_string("⇞"), None, "the page-up glyph is not canonical input");
    assert_eq!(Key::from_string("⇟"), None, "the page-down glyph is not canonical input");
}

/// N-K-02 / N-K-03: the canonical form round-trips for physical Control, and a second key token is
/// rejected rather than silently dropping the first.
#[test]
fn shortcut_canonical_form_round_trips_and_rejects_multiple_keys() {
    use crate::compat::format;
    use std::collections::HashMap;

    // Physical Control keeps its identity through format -> parse.
    for shortcut in [
        Shortcut::ctrl(Key::Z),
        Shortcut::ctrl_shift(Key::Z),
        Shortcut::ctrl_alt(Key::F1),
        Shortcut::primary(Key::Z),
        Shortcut::new(Key::A, Modifiers::CTRL | Modifiers::META),
    ] {
        let formatted = shortcut.to_string();
        let parsed = Shortcut::from_string(&formatted)
            .unwrap_or_else(|| panic!("{formatted:?} must parse back"));
        assert_eq!(parsed, shortcut, "{formatted:?} did not round-trip to an equal value");
        assert_eq!(
            format!("{parsed:?}"),
            format!("{shortcut:?}"),
            "Eq implies Hash, so Debug must match too"
        );
    }

    // The hash-identity regression is the point: a shortcut must find itself.
    let mut map: HashMap<Shortcut, u32> = HashMap::new();
    map.insert(Shortcut::ctrl(Key::Z), 1);
    let reparsed = Shortcut::from_string(&Shortcut::ctrl(Key::Z).to_string()).unwrap();
    assert_eq!(
        map.get(&reparsed).copied(),
        Some(1),
        "a reparsed shortcut must hash to the same key"
    );

    // A second key token is a malformed declaration, not a silent overwrite.
    assert_eq!(Shortcut::from_string("Ctrl+A+B"), None);
    assert_eq!(Shortcut::from_string("A+B"), None);
    // One key token still parses.
    assert!(Shortcut::from_string("Ctrl+A").is_some());
    assert!(Shortcut::from_string("Control+A").is_some());
    // `Ctrl` (input alias) and `Control` (physical) are distinct values.
    assert_ne!(Shortcut::from_string("Ctrl+A"), Shortcut::from_string("Control+A"));
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

/// W-201: a scoped binding shadows the global binding on the same chord while its scope is active,
/// and the two do not conflict at registration time.
#[test]
fn a_scoped_binding_shadows_the_global_one() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register("global_save", Shortcut::primary(Key::S), "Global save"));
    assert!(
        manager.register_in("dialog", "dialog_save", Shortcut::primary(Key::S), "Dialog save"),
        "the same chord is allowed in a different scope"
    );

    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(),
        Some("global_save"),
        "with no scope active only the global binding resolves"
    );

    manager.push_scope("dialog");
    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(),
        Some("dialog_save"),
        "the innermost scope wins"
    );
    assert!(manager.has_scoped_shortcut("dialog", &Shortcut::primary(Key::S)));
    assert!(manager.has_shortcut(&Shortcut::primary(Key::S)), "the global view is untouched");

    manager.pop_scope();
    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(),
        Some("global_save"),
        "popping the scope restores the global binding"
    );
    assert!(manager.unregister("dialog_save"));
    assert!(!manager.has_scoped_shortcut("dialog", &Shortcut::primary(Key::S)));
}

/// W-201: a modal scope shields every layer below it, including global bindings it does not
/// shadow itself.
#[test]
fn a_modal_scope_shields_the_layers_below() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register("global_save", Shortcut::primary(Key::S), "Global save"));
    manager.push_modal_scope("dialog");

    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY),
        None,
        "the modal dialog did not bind Ctrl+S, and it must not reach the global binding"
    );

    assert!(manager.register_in("dialog", "dialog_ok", Shortcut::primary(Key::Enter), "OK"));
    assert_eq!(
        manager.action_for_key(Key::Enter, Modifiers::PRIMARY).as_deref(),
        Some("dialog_ok")
    );

    manager.pop_scope();
    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(),
        Some("global_save"),
        "closing the modal restores the layer below"
    );
}

/// W-201: conflicts are judged inside a scope, so two scopes may bind one chord to different
/// actions while a duplicate within one scope is still refused.
#[test]
fn shortcut_conflicts_are_scoped() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register_in("window_a", "a_save", Shortcut::primary(Key::S), "A"));
    assert!(manager.register_in("window_b", "b_save", Shortcut::primary(Key::S), "B"));
    assert!(
        !manager.register_in("window_a", "a_other", Shortcut::primary(Key::S), "Other"),
        "a second action for one chord in the same scope is still a conflict"
    );
    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY),
        None,
        "neither window is active, so no scoped binding resolves"
    );
    manager.push_scope("window_b");
    assert_eq!(manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(), Some("b_save"));
}

/// W-P01: the chord index must reproduce the linear scan's outcomes.
///
/// One dispatch matrix over every path the index touches: the direct bucket, the desktop
/// `primary` lift bucket, re-registration (a moved binding must leave its old bucket), scoped
/// shadowing through the same buckets, and unregister.
#[test]
fn the_chord_index_preserves_dispatch_outcomes() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register("save", Shortcut::primary(Key::S), "Save"));
    assert!(manager.register("copy", Shortcut::ctrl(Key::C), "Copy"));

    // The binding's own modifier set answers from the direct bucket, on every platform.
    assert_eq!(manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(), Some("save"));
    assert_eq!(manager.action_for_key(Key::C, Modifiers::CTRL).as_deref(), Some("copy"));
    // A modifier miss must not resolve: no bucket exists for (S, SHIFT), and `copy` is not
    // indexed under (C, PRIMARY) nor lifted to it.
    assert_eq!(manager.action_for_key(Key::S, Modifiers::SHIFT), None);
    assert_eq!(manager.action_for_key(Key::C, Modifiers::PRIMARY), None);
    // The lift bucket: a `primary` binding answers to a Control press on desktop only, and a
    // physical-Control binding never answers to a primary press anywhere.
    match PlatformShortcutStyle::current() {
        PlatformShortcutStyle::Desktop => {
            assert_eq!(manager.action_for_key(Key::S, Modifiers::CTRL).as_deref(), Some("save"));
        }
        PlatformShortcutStyle::Mac => {
            assert_eq!(manager.action_for_key(Key::S, Modifiers::CTRL), None);
            assert_eq!(manager.action_for_key(Key::C, Modifiers::PRIMARY), None);
        }
    }

    // Re-registration moves the binding: the old chord must die with its index entries.
    assert!(manager.register("save", Shortcut::primary(Key::D), "Save"));
    assert_eq!(manager.action_for_key(Key::S, Modifiers::PRIMARY), None);
    assert_eq!(manager.action_for_key(Key::D, Modifiers::PRIMARY).as_deref(), Some("save"));

    // Scoped shadowing walks the same buckets: the innermost active scope wins, and popping
    // restores the global binding.
    assert!(manager.register_in("dialog", "dialog_ok", Shortcut::primary(Key::D), "OK"));
    manager.push_scope("dialog");
    assert_eq!(manager.action_for_key(Key::D, Modifiers::PRIMARY).as_deref(), Some("dialog_ok"));
    manager.pop_scope();
    assert_eq!(manager.action_for_key(Key::D, Modifiers::PRIMARY).as_deref(), Some("save"));

    // Unregistering removes the bucket entry with the binding, and leaves the scoped one alone.
    assert!(manager.unregister("save"));
    assert_eq!(manager.action_for_key(Key::D, Modifiers::PRIMARY), None);
    manager.push_scope("dialog");
    assert_eq!(manager.action_for_key(Key::D, Modifiers::PRIMARY).as_deref(), Some("dialog_ok"));
    manager.pop_scope();
}

/// W-P01 order-sensitive case: a physical-Control binding and a `primary` binding for the same
/// key can both answer to one press on desktop (the lift). The bucket keeps their registration
/// order, so the winner is deterministic instead of hash-order dependent.
#[test]
fn overlapping_chords_resolve_in_registration_order_within_their_bucket() {
    let mut manager = ShortcutManager::new();
    assert!(manager.register("first", Shortcut::ctrl(Key::S), "First"));
    assert!(manager.register("second", Shortcut::primary(Key::S), "Second"));

    // A primary press reaches only the `primary` binding: the physical-Control binding is
    // neither indexed under PRIMARY nor lifted to it.
    assert_eq!(manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(), Some("second"));

    match PlatformShortcutStyle::current() {
        PlatformShortcutStyle::Desktop => {
            // Both bindings are genuine candidates for the Control press — `matches_binding`
            // accepts each — so the bucket order decides: first-registered wins.
            assert!(ShortcutManager::matches_binding(
                &Shortcut::ctrl(Key::S),
                Key::S,
                Modifiers::CTRL
            ));
            assert!(ShortcutManager::matches_binding(
                &Shortcut::primary(Key::S),
                Key::S,
                Modifiers::CTRL
            ));
            assert_eq!(manager.action_for_key(Key::S, Modifiers::CTRL).as_deref(), Some("first"));
        }
        PlatformShortcutStyle::Mac => {
            // The two chords stay distinct on macOS: the Control press reaches only the physical
            // binding, because `matches_binding` rejects the lift bucket there.
            assert_eq!(manager.action_for_key(Key::S, Modifiers::CTRL).as_deref(), Some("first"));
        }
    }

    // A disabled entry stays indistinguishable from an absent one: the resolution does not fall
    // through to the second binding in the bucket.
    manager.disable("first");
    assert_eq!(manager.action_for_key(Key::S, Modifiers::CTRL), None);
}

/// W-02: unregistering a chord removes the **global** binding only. A scoped binding on the same
/// chord stays registered, stays resolvable while its scope is active, and keeps its entry.
#[test]
fn unregistering_a_chord_leaves_the_same_chord_in_scopes_alive() {
    let chord = Shortcut::primary(Key::S);
    let mut manager = ShortcutManager::new();
    assert!(manager.register("global_save", chord.clone(), "Global"));
    assert!(manager.register_in("window", "window_save", chord.clone(), "Window"));
    assert!(manager.register_for_control(77, "control_save", chord.clone(), "Control"));

    assert!(
        manager.unregister_shortcut(&chord),
        "the global binding exists, so the chord removal reports true"
    );
    assert!(!manager.has_shortcut(&chord), "the global table lost the chord");
    assert!(!manager.contains_action("global_save"), "and the global action's entry is gone");

    // Every other binding on the chord is untouched — no dangling entry, no lost table row.
    assert!(manager.has_scoped_shortcut("window", &chord));
    assert!(manager.has_control_shortcut(77, &chord));
    assert!(manager.contains_action("window_save"));
    assert!(manager.contains_action("control_save"));

    manager.push_scope("window");
    assert_eq!(
        manager.action_for_key(Key::S, Modifiers::PRIMARY).as_deref(),
        Some("window_save"),
        "the window binding still resolves after the global one was removed"
    );
    manager.pop_scope();

    // The chord is no longer in the global table, so a second removal reports false and changes
    // nothing (in particular it must not take the scoped entries with it).
    assert!(!manager.unregister_shortcut(&chord));
    assert!(manager.has_scoped_shortcut("window", &chord));
    assert!(manager.has_control_shortcut(77, &chord));
}

/// W-02: removal matches `(key, scope)` exactly, so removing one scope's binding neither removes
/// another scope's binding on the same chord nor the control-scoped one.
#[test]
fn a_scoped_chord_unregisters_only_in_its_own_scope() {
    let chord = Shortcut::primary(Key::P);
    let mut manager = ShortcutManager::new();
    assert!(manager.register_in("window_a", "a_print", chord.clone(), "A"));
    assert!(manager.register_in("window_b", "b_print", chord.clone(), "B"));
    assert!(manager.register_for_control(5, "control_print", chord.clone(), "C"));

    assert!(manager.unregister_shortcut_in("window_a", &chord));
    assert!(!manager.has_scoped_shortcut("window_a", &chord));
    assert!(!manager.contains_action("a_print"), "and only its own entry is removed");

    assert!(manager.has_scoped_shortcut("window_b", &chord));
    assert!(manager.has_control_shortcut(5, &chord));
    assert!(manager.contains_action("b_print"));
    assert!(manager.contains_action("control_print"));

    manager.push_scope("window_b");
    assert_eq!(
        manager.action_for_key(Key::P, Modifiers::PRIMARY).as_deref(),
        Some("b_print"),
        "the sibling window's binding is the one that resolves"
    );
    manager.pop_scope();

    // A scope that does not hold the chord reports false without touching anything.
    assert!(!manager.unregister_shortcut_in("window_a", &chord));
    assert!(manager.has_scoped_shortcut("window_b", &chord));
    assert!(manager.has_control_shortcut(5, &chord));

    // The control-scoped convenience entry removes exactly that control's binding.
    assert!(manager.unregister_for_control(5, &chord));
    assert!(!manager.has_control_shortcut(5, &chord));
    assert!(!manager.contains_action("control_print"));
    assert!(manager.has_scoped_shortcut("window_b", &chord), "window_b is unaffected");
}

/// W-02 regression: re-registering one action must not remove another action that shares the
/// chord in the same scope, and the surviving binding must still resolve.
#[test]
fn moving_one_action_never_takes_another_actions_chord_with_it() {
    let chord = Shortcut::primary(Key::R);
    let mut manager = ShortcutManager::new();
    assert!(manager.register_in("editor", "run", chord.clone(), "Run"));
    assert!(manager.register_in("editor", "rename", Shortcut::primary(Key::F2), "Rename"));

    // Move `rename` to the chord `run` does not use; `run` must survive.
    assert!(manager.register_in("editor", "rename", Shortcut::primary(Key::F3), "Rename"));
    assert!(manager.has_scoped_shortcut("editor", &chord));
    assert!(manager.has_scoped_shortcut("editor", &Shortcut::primary(Key::F3)));
    manager.push_scope("editor");
    assert_eq!(manager.action_for_key(Key::R, Modifiers::PRIMARY).as_deref(), Some("run"));
    manager.pop_scope();
}

/// W-12: a control-scoped binding resolves only while the control is in the focus chain, and it
/// wins over a window or global binding on the same chord. With no focus-chain lookup installed
/// nothing changes (the pre-W-12 behaviour).
#[test]
fn a_control_scope_resolves_only_inside_the_focus_chain() {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex as StdMutex;

    let chord = Shortcut::primary(Key::B);
    let mut manager = ShortcutManager::new();
    assert!(manager.register("bold", chord.clone(), "Bold everywhere"));
    assert!(manager.register_in("window", "window_bold", chord.clone(), "Window bold"));
    assert!(manager.register_for_control(42, "control_bold", chord.clone(), "Control bold"));

    // No lookup installed: only the global binding resolves.
    assert_eq!(manager.action_for_key(Key::B, Modifiers::PRIMARY).as_deref(), Some("bold"));

    let focused: Arc<StdMutex<Vec<crate::core::ObjectId>>> = Arc::new(StdMutex::new(Vec::new()));
    let chain = Arc::clone(&focused);
    let reads = Arc::new(AtomicUsize::new(0));
    let reads_in_closure = Arc::clone(&reads);
    manager.set_focus_chain_lookup(Box::new(move |out: &mut Vec<crate::core::ObjectId>| {
        reads_in_closure.fetch_add(1, Ordering::SeqCst);
        out.clear();
        out.extend(chain.lock().unwrap().iter().copied());
    }));

    // The control is not in the chain: the global binding still applies.
    assert_eq!(
        manager.action_for_key(Key::B, Modifiers::PRIMARY).as_deref(),
        Some("bold"),
        "a control scope is inert while the control is not focused"
    );

    *focused.lock().unwrap() = vec![42];
    assert_eq!(
        manager.action_for_key(Key::B, Modifiers::PRIMARY).as_deref(),
        Some("control_bold"),
        "the focused control's binding wins over the global one"
    );
    manager.push_scope("window");
    assert_eq!(
        manager.action_for_key(Key::B, Modifiers::PRIMARY).as_deref(),
        Some("control_bold"),
        "the focused control also wins over a window scope"
    );
    manager.pop_scope();
    assert!(reads.load(Ordering::SeqCst) > 0, "the lookup was consulted");

    // Removing the control's binding falls back to the global one even while it is focused.
    assert!(manager.unregister_for_control(42, &chord));
    assert!(!manager.has_control_shortcut(42, &chord));
    assert_eq!(manager.action_for_key(Key::B, Modifiers::PRIMARY).as_deref(), Some("bold"));

    // Clearing the lookup returns to the pre-W-12 behaviour.
    assert!(manager.clear_focus_chain_lookup());
    assert!(!manager.clear_focus_chain_lookup(), "a second clear reports nothing to remove");
}
