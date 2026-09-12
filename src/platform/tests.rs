//! Platform integration tests — capability negotiation, widget creation round-trips,
//! event injection, clipboard, IME, and drag-drop flows.
//!
//! All tests use `StubPlatform` (in-memory state backend) so they run on any host
//! without native platform dependencies.

use crate::core::PlatformFamily;
use crate::platform::{Platform, StubPlatform, WidgetTriggerEvent, WidgetTriggerKind};

/// The process-global backend must be the Harmony backend when the `harmony`
/// preview feature is enabled, rather than falling through to the generic
/// desktop stub. Regression guard for the missing `create_native_platform` arm.
#[cfg(feature = "harmony")]
#[test]
fn runtime_selects_harmony_backend_when_feature_enabled() {
    let platform = crate::platform::get_platform();
    assert_eq!(platform.backend_name(), "harmony-desktop");
    assert_eq!(platform.family(), PlatformFamily::Desktop);
}

/// The generic stub must never be selected merely because the host OS is
/// unrecognised; a feature-selected backend takes precedence.
#[cfg(feature = "harmony")]
#[test]
fn runtime_does_not_fall_back_to_unknown_stub_with_harmony() {
    let name = crate::platform::backend_name();
    assert_ne!(name, "unknown-runtime-stub");
}
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
        Some(WidgetTriggerEvent { widget_id: button, kind: WidgetTriggerKind::Clicked })
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
    assert_eq!(platform.list_box_item_text(list_box, 1).as_deref(), Some("B"));
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
        Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::SelectionChanged })
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
    assert_eq!(platform.create_status_bar(window, "ready", 0, 96, 200, 24), 0);
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
        Some(WidgetTriggerEvent { widget_id: combo, kind: WidgetTriggerKind::SelectionChanged })
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
        Some(WidgetTriggerEvent { widget_id: list, kind: WidgetTriggerKind::SelectionChanged })
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Platform isolation contract (principle #35–#37)
//
// OS facts must be answered by the backend through semantic `Platform` methods,
// never sniffed by a middle layer. These tests pin the default behavior so a
// backend cannot silently start fabricating values.
// ═══════════════════════════════════════════════════════════════════════════════

/// A backend with no OS to interrogate must say "unknown", not invent a number.
/// A fabricated figure would make the adaptive layers size caches against a lie.
#[test]
fn stub_reports_unknown_platform_facts_honestly() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert_eq!(platform.total_memory_mb(), None);
    assert_eq!(platform.process_memory_utilization(), None);
    assert_eq!(platform.process_cpu_utilization(), None);
}

/// `is_on_battery` defaults to `false` on backends that cannot tell. That is the
/// safe direction: a wrong `true` would strip animations from a plugged-in host.
#[test]
fn battery_unknown_defaults_to_mains_power() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(!platform.is_on_battery());
}

/// A backend with no spooler must refuse the job rather than report success, so
/// the caller can surface the failure instead of pretending it printed.
#[test]
fn stub_without_spooler_refuses_print_job() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(!platform.has_print_support());
    let result = platform.spawn_print_job(std::path::Path::new("/tmp/does-not-matter.txt"));
    assert!(result.is_err(), "a backend with no spooler must report the gap, not fake success");
}

/// Control routing asks the backend which native primitives exist instead of
/// testing `cfg(target_os)`. A backend publishing nothing therefore yields the
/// global policy verdict for every kind, including the Win32-only ones.
#[test]
fn native_widget_kinds_defaults_to_empty_so_routing_uses_global_policy() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(platform.native_widget_kinds().is_empty());
}

/// Guards the spooler probe against an exit-code false negative.
///
/// CUPS `lp` rejects `--version` with status 1 while still printing usage, so a
/// check keyed on `status.success()` reported "no spooler" on machines that had
/// one. Presence is proven by the command being spawnable, and this test pins
/// that `lp` (which ships on every macOS and most Linux installs) is detected
/// when it exists on `PATH`.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn unix_print_probe_detects_cups_client_despite_nonzero_version_exit() {
    let lp_exists = std::process::Command::new("lp").arg("--help").output().is_ok();
    let lpr_exists = std::process::Command::new("lpr").arg("--help").output().is_ok();
    if lp_exists || lpr_exists {
        assert!(
            crate::platform::types::unix_print_clients_available(),
            "a spooler client exists on PATH but was reported missing"
        );
    }
}

/// A backend with no engine must report `None` so `src/web/` uses the simulated
/// path instead of trying to drive a native view that does not exist.
#[test]
fn stub_reports_no_native_web_engine() {
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    assert!(platform.create_web_engine().is_none());
}

/// Shortcut notation is asked of the backend rather than decided by `cfg!` in the
/// shortcut layer. The stub inherits the build-target default, and on an Apple
/// host that must be the AppKit symbol style.
#[test]
fn shortcut_style_follows_the_backend_not_a_middle_layer_cfg() {
    use crate::shortcut::PlatformShortcutStyle;
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let expected = if cfg!(any(target_os = "macos", target_os = "ios")) {
        PlatformShortcutStyle::Mac
    } else {
        PlatformShortcutStyle::Desktop
    };
    assert_eq!(platform.shortcut_style(), expected);
}

/// The format used for menu labels must agree with the backend's declared style,
/// so `Shortcut::primary` renders `⌘Z` or `Ctrl+Z` as the host expects.
#[test]
fn format_shortcut_uses_the_backend_style() {
    use crate::shortcut::{format_shortcut_for_platform, Key, Shortcut};
    let platform = StubPlatform::new("test-desktop", PlatformFamily::Desktop);
    let shortcut = Shortcut::primary(Key::Z);
    assert_eq!(
        platform.format_shortcut(&shortcut),
        format_shortcut_for_platform(&shortcut, platform.shortcut_style()),
    );
}
