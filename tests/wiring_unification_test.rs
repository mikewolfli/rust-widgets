// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! End-to-end checks for the wiring/unification items (W-09/W-12..W-21, COMP-25..27).
//!
//! These are integration tests rather than unit tests because the behaviour under test crosses
//! module boundaries: a command resolved from `ActionManager` must reach a menu row, a toolbar
//! item, a menu-bar action and a button; the shortcut registry must be the one that dispatches;
//! and the alias registry must be the one the designer manifest exports.

use rust_widgets::action::{ActionManager, ActionRouter};
use rust_widgets::core::Rect;
use rust_widgets::data_binding::Binding;
use rust_widgets::shortcut::{Key, Modifiers, Shortcut, ShortcutManager};
use rust_widgets::widget::capability::{
    aliases_of, capability_manifest_json, resolve, CapabilityValue, WidgetFactory,
};
use rust_widgets::widget::{Button, LineEdit, Menu, MenuBar, Slider, SpinBox, ToolBar, Widget};
use std::sync::{Arc, Mutex};

fn rect() -> Rect {
    Rect::new(0, 0, 200, 24)
}

#[test]
fn the_alias_registry_resolves_both_directions() {
    assert_eq!(resolve("context_menu"), Some("menu"));
    assert_eq!(resolve("split_layout"), Some("splitter"));
    assert_eq!(resolve("menu"), None);
    let line_edit_aliases: Vec<&str> = aliases_of("line_edit").collect();
    assert_eq!(line_edit_aliases, vec!["email_input", "password_edit", "phone_input"]);
}

#[test]
fn the_designer_manifest_carries_alias_of() {
    let factory = WidgetFactory::new_with_defaults();
    let alias_document = capability_manifest_json(&factory, "email_input").expect("manifest");
    assert!(
        alias_document.contains("\"alias_of\": \"line_edit\""),
        "an alias row must name its canonical control in the manifest"
    );
    let canonical_document = capability_manifest_json(&factory, "line_edit").expect("manifest");
    assert!(
        canonical_document.contains("\"alias_of\": \"\""),
        "a canonical row carries an empty alias_of rather than a missing key"
    );
}

#[test]
fn action_manager_publishes_trigger_toggle_and_enabled_changes() {
    let mut manager = ActionManager::new();
    assert!(manager.register_action("pin", "Pin"));
    manager.action_mut("pin").expect("pin").set_checkable(true);

    let triggered = Arc::new(Mutex::new(Vec::<String>::new()));
    let toggled = Arc::new(Mutex::new(Vec::<(String, bool)>::new()));
    let enabled = Arc::new(Mutex::new(Vec::<(String, bool)>::new()));
    let t = Arc::clone(&triggered);
    let g = Arc::clone(&toggled);
    let e = Arc::clone(&enabled);
    manager.action_triggered.connect(move |id| t.lock().unwrap().push((*id).clone()));
    manager.action_toggled.connect_ref(move |value| g.lock().unwrap().push(value.clone()));
    manager.action_enabled_changed.connect_ref(move |value| e.lock().unwrap().push(value.clone()));

    assert!(manager.trigger_action("pin"));
    assert_eq!(*triggered.lock().unwrap(), vec!["pin".to_string()]);
    assert_eq!(*toggled.lock().unwrap(), vec![("pin".to_string(), true)]);

    assert!(manager.set_action_enabled("pin", false));
    assert!(manager.set_action_enabled("pin", false));
    assert_eq!(*enabled.lock().unwrap(), vec![("pin".to_string(), false)]);
}

#[test]
fn a_direct_chord_collision_is_reported_but_shortcut_manager_judges() {
    let mut manager = ActionManager::new();
    assert!(manager.register_action("save", "Save"));
    assert!(manager.register_action("open", "Open"));
    let conflicts = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let sink = Arc::clone(&conflicts);
    manager.shortcut_conflict.connect_ref(move |pair| sink.lock().unwrap().push(pair.clone()));

    assert!(manager.bind_shortcut("Ctrl+S", "save"));
    assert!(manager.bind_shortcut("Ctrl+S", "open"));
    assert_eq!(
        *conflicts.lock().unwrap(),
        vec![("open".to_string(), "save".to_string())],
        "the registry observes the collision; ShortcutManager remains the judge"
    );
}

#[test]
fn the_router_is_the_one_keyboard_dispatch_path() {
    let mut shortcuts = ShortcutManager::new();
    let mut actions = ActionManager::new();
    {
        let mut router = ActionRouter::new(&mut shortcuts, &mut actions);
        assert!(router.connect("save", Shortcut::primary(Key::S), "Save"));
        assert_eq!(router.dispatch_key(Key::S, Modifiers::PRIMARY), Some(String::from("save")));
        assert_eq!(router.dispatch_key(Key::O, Modifiers::PRIMARY), None);
    }
    assert_eq!(
        actions.action("save").map(|action| action.text.clone()),
        Some(String::from("Save"))
    );
    assert!(actions.trigger_shortcut("Ctrl+S"));
}

#[test]
fn registry_commands_reach_menu_toolbar_menu_bar_and_button() {
    let mut manager = ActionManager::new();
    assert!(manager.register_action("copy", "Copy"));
    manager.action_mut("copy").expect("copy").set_checkable(true);

    let mut menu = Menu::new("Edit", rect());
    assert!(menu.add_action_from_manager(&mut manager, "copy").is_some());
    assert_eq!(menu.items()[0].action_id(), Some("copy"));

    let mut toolbar = ToolBar::new(rect());
    assert!(toolbar.add_action_from_manager(&mut manager, "copy").is_some());
    assert_eq!(toolbar.items()[0].id(), "copy");

    let mut menu_bar = MenuBar::new(rect());
    assert!(menu_bar.add_action_from_manager(&mut manager, "copy").is_some());
    assert_eq!(menu_bar.actions()[0].command_id(), Some("copy"));

    let mut button = Button::new(String::from("placeholder"), rect());
    assert!(button.bind_action(&mut manager, "copy"));
    assert_eq!(button.action_id(), Some("copy"));
    assert_eq!(button.text(), "Copy");

    // Disabling the command and toggling it must reach every host after a refresh.
    manager.action_mut("copy").expect("copy").set_checked(true);
    assert!(manager.set_action_enabled("copy", false));
    menu.refresh_action_states(&manager);
    toolbar.refresh_action_states(&manager);
    menu_bar.refresh_action_states(&manager);
    button.refresh_action_state(&manager);

    assert!(!menu.items()[0].is_enabled());
    assert!(menu.items()[0].is_checked());
    assert!(!toolbar.items()[0].is_enabled());
    assert!(toolbar.items()[0].is_checked());
    assert!(!menu_bar.actions()[0].cmd().is_enabled());
    assert!(!button.is_enabled());
}

#[test]
fn actions_and_shortcuts_round_trip_through_a_file() {
    let mut manager = ActionManager::new();
    assert!(manager.register_action("save", "Save"));
    assert!(manager.bind_shortcut("Ctrl+S", "save"));
    manager.action_mut("save").expect("save").set_checkable(true);

    let path = std::env::temp_dir().join("rw_actions_wiring_test.json");
    manager.save_to_file(&path).expect("save commands");
    let mut loaded = ActionManager::new();
    loaded.load_from_file(&path).expect("load commands");
    let _ = std::fs::remove_file(&path);

    let definition = loaded.action_definition("save").expect("loaded command");
    assert_eq!(definition.text, "Save");
    assert_eq!(definition.shortcut, "primary+s");
    assert!(definition.checkable);
    assert!(loaded.trigger_shortcut("Ctrl+S"));
}

#[test]
fn controls_consume_the_property_bridge() {
    let slider_binding = Binding::new(CapabilityValue::Int(42));
    let mut slider = Slider::new(rect());
    slider.bind_value(&slider_binding);
    assert_eq!(slider.value(), 42);
    slider.set_value(7);
    assert!(slider.sync_to_binding());
    assert_eq!(slider_binding.get(), CapabilityValue::Int(7));

    let text_binding = Binding::new(CapabilityValue::String(String::from("hello")));
    let mut line = LineEdit::new(rect());
    line.bind_text(&text_binding);
    assert_eq!(line.text(), "hello");
    line.set_text("world");
    assert!(line.sync_to_binding());
    assert_eq!(text_binding.get(), CapabilityValue::String(String::from("world")));

    let number_binding = Binding::new(CapabilityValue::Int(5));
    let mut spin = SpinBox::new(rect());
    spin.bind_value(&number_binding);
    assert_eq!(spin.value(), 5);
    spin.set_value(9);
    assert!(spin.sync_to_binding());
    assert_eq!(number_binding.get(), CapabilityValue::Int(9));
}

#[test]
fn the_spin_box_formatter_keeps_the_declared_precision() {
    let mut spin = SpinBox::new(rect());
    spin.set_decimals(2);
    spin.set_value_f64(1.5);
    assert_eq!(spin.formatted_value(), "1.50");
    spin.set_value_f64(2.25);
    assert_eq!(spin.formatted_value(), "2.25");
}
