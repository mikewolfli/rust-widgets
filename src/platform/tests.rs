use crate::core::PlatformFamily;
use crate::platform::{Platform, StubPlatform, WidgetTriggerEvent, WidgetTriggerKind};
#[test]
fn consistency_menu_trigger_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    let menu_bar = platform.create_menu_bar(window, 0, 0, 100, 20);
    let menu = platform.create_menu(menu_bar, "File", 0, 0, 50, 20);
    let item = platform.menu_add_item(menu, "Open", None);
    assert!(platform.inject_menu_trigger(item));
    assert_eq!(platform.poll_menu_triggered(), Some(item));
}
#[test]
fn consistency_typed_widget_trigger_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    let button = platform.create_button(window, "btn", 0, 0, 80, 30);
    assert!(platform.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent {
            widget_id: button,
            kind: WidgetTriggerKind::Clicked,
        })
    );
}
#[test]
fn consistency_compat_poll_widget_triggered_is_single_delivery_shim() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    let button = platform.create_button(window, "btn", 0, 0, 80, 30);
    assert!(platform.inject_widget_trigger_event(button, WidgetTriggerKind::Clicked));
    assert_eq!(platform.poll_widget_triggered(), Some(button));
    assert_eq!(platform.poll_widget_triggered(), None);
    assert_eq!(platform.poll_widget_trigger_event(), None);
}
#[test]
fn consistency_list_box_data_path_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    let list_box = platform.create_list_box(window, 0, 0, 120, 80);
    assert!(platform.list_box_add_item(list_box, "A"));
    assert!(platform.list_box_add_item(list_box, "B"));
    assert_eq!(platform.list_box_item_count(list_box), 2);
    assert_eq!(
        platform.list_box_item_text(list_box, 1).as_deref(),
        Some("B")
    );
    assert!(platform.list_box_set_current_index(list_box, 1));
    assert_eq!(platform.list_box_current_index(list_box), Some(1));
    assert!(platform.list_box_remove_item(list_box, 0));
    assert_eq!(platform.list_box_item_count(list_box), 1);
    assert_eq!(platform.list_box_current_index(list_box), Some(0));
    assert!(platform.list_box_clear_items(list_box));
    assert_eq!(platform.list_box_item_count(list_box), 0);
    assert_eq!(platform.list_box_current_index(list_box), None);
}
#[test]
fn consistency_combo_box_data_and_event_path_roundtrip() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let window = platform.create_window("w", 0, 0, 100, 100);
    let combo = platform.create_combo_box(window, 0, 0, 120, 24);
    assert!(platform.combo_box_add_item(combo, "A"));
    assert!(platform.combo_box_add_item(combo, "B"));
    assert_eq!(platform.combo_box_item_count(combo), 2);
    assert_eq!(platform.combo_box_item_text(combo, 0).as_deref(), Some("A"));
    assert!(platform.combo_box_set_current_index(combo, 1));
    assert_eq!(platform.combo_box_current_index(combo), Some(1));
    assert!(platform.inject_widget_trigger_event(combo, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent {
            widget_id: combo,
            kind: WidgetTriggerKind::SelectionChanged,
        })
    );
    assert!(platform.combo_box_clear_items(combo));
    assert_eq!(platform.combo_box_item_count(combo), 0);
    assert_eq!(platform.combo_box_current_index(combo), None);
}
#[test]
fn consistency_capability_contract_by_profile() {
    let desktop = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let embedded = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    assert!(desktop.native_capability_contract().is_some());
    assert!(desktop.embedded_capability_contract().is_none());
    assert!(embedded.native_capability_contract().is_none());
    assert!(embedded.embedded_capability_contract().is_some());
}
#[test]
fn embedded_profile_core_controls_have_non_placeholder_create_paths() {
    let platform = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    let window = platform.create_window("w", 0, 0, 200, 120);
    assert_ne!(window, 0);
    assert_ne!(platform.create_button(window, "b", 0, 0, 80, 24), 0);
    assert_ne!(platform.create_checkbox(window, "c", 0, 0, 80, 24), 0);
    assert_ne!(platform.create_radio_button(window, "r", 0, 0, 80, 24), 0);
    assert_ne!(platform.create_label(window, "l", 0, 0, 80, 24), 0);
    assert_ne!(platform.create_line_edit(window, "e", 0, 0, 120, 24), 0);
    assert_ne!(platform.create_slider(window, 0, 30, 120, 24), 0);
    assert_ne!(platform.create_progress_bar(window, 0, 60, 120, 24), 0);
    assert_ne!(platform.create_panel(window, 0, 0, 120, 80), 0);
    assert_ne!(platform.create_combo_box(window, 0, 0, 120, 24), 0);
    assert_ne!(platform.create_list_box(window, 0, 0, 120, 80), 0);
}
#[test]
fn embedded_profile_host_controls_are_explicitly_unsupported() {
    let platform = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    let window = platform.create_window("w", 0, 0, 200, 120);
    let menu_bar = platform.create_menu_bar(window, 0, 0, 200, 24);
    assert_eq!(menu_bar, 0);
    assert_eq!(platform.create_menu(window, "File", 0, 0, 80, 24), 0);
    assert_eq!(platform.menu_add_item(window, "Open", None), 0);
    assert_eq!(platform.create_tool_bar(window, 0, 24, 200, 24), 0);
    assert_eq!(
        platform.create_status_bar(window, "ready", 0, 96, 200, 24),
        0
    );
    assert!(!platform.attach_menu_bar_to_window(window, menu_bar));
    assert!(!platform.inject_menu_trigger(1));
}
#[test]
fn embedded_profile_combo_list_state_event_data_roundtrip() {
    let platform = StubPlatform::new("test-embedded", PlatformFamily::Embedded);
    let window = platform.create_window("w", 0, 0, 220, 160);
    let combo = platform.create_combo_box(window, 0, 0, 120, 24);
    assert_ne!(combo, 0);
    assert!(platform.combo_box_add_item(combo, "A"));
    assert!(platform.combo_box_add_item(combo, "B"));
    assert!(platform.combo_box_set_current_index(combo, 1));
    assert_eq!(platform.combo_box_current_index(combo), Some(1));
    assert_eq!(platform.combo_box_item_count(combo), 2);
    assert_eq!(platform.combo_box_item_text(combo, 0).as_deref(), Some("A"));
    assert!(platform.inject_widget_trigger_event(combo, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent {
            widget_id: combo,
            kind: WidgetTriggerKind::SelectionChanged,
        })
    );
    let list = platform.create_list_box(window, 0, 30, 120, 80);
    assert_ne!(list, 0);
    assert!(platform.list_box_add_item(list, "L1"));
    assert!(platform.list_box_add_item(list, "L2"));
    assert!(platform.list_box_set_current_index(list, 0));
    assert_eq!(platform.list_box_current_index(list), Some(0));
    assert_eq!(platform.list_box_item_count(list), 2);
    assert_eq!(platform.list_box_item_text(list, 1).as_deref(), Some("L2"));
    assert!(platform.inject_widget_trigger_event(list, WidgetTriggerKind::SelectionChanged));
    assert_eq!(
        platform.poll_widget_trigger_event(),
        Some(WidgetTriggerEvent {
            widget_id: list,
            kind: WidgetTriggerKind::SelectionChanged,
        })
    );
}
