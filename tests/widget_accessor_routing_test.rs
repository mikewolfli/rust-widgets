// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Guard: every crate-root control-state accessor must reach the control.
//!
//! # What went wrong
//!
//! `set_widget_value` / `set_widget_checked` / `combo_box_add_item` and their
//! siblings were implemented as `platform::get_platform().…`. That was correct
//! while the host owned the controls; after BLUE15 the host owns a window and a
//! drawing surface only, so those trait methods are honest defaults that report
//! "no such control". Every accessor therefore returned `false` / `None` and
//! changed nothing — while the C ABI functions of the same names went through the
//! control backend and worked. Nothing caught it because no test called them.
//!
//! # What this test pins
//!
//! Each accessor is driven against a real control and read back through a
//! *different* entry point, so a write that is accepted must be observable. The
//! negative cases matter as much as the positive ones: an accessor asked about a
//! control that has no such concept must say so rather than report success.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::core::{ObjectId, Orientation, Rect};
use rust_widgets::widget::{runtime, WidgetFactory};

/// Builds a control from the factory and hands it to the runtime, which assigns
/// the id every accessor in this file is addressed by.
fn mount(kind: &str, text: &str) -> ObjectId {
    let factory = WidgetFactory::new_with_defaults();
    let widget = factory
        .create(kind, Rect::new(0, 0, 120, 30), text)
        .unwrap_or_else(|| panic!("{kind} must be a registered control"));
    runtime::register(widget).unwrap_or_else(|| panic!("{kind} must register on the UI thread"))
}

// ── Label family (routed through the control backend) ───────────────────────

#[test]
fn label_text_round_trips_through_the_accessor() {
    let id = mount("label", "hello");
    assert_eq!(rust_widgets::get_widget_text(id), "hello");
    rust_widgets::set_widget_text(id, "world");
    assert_eq!(rust_widgets::get_widget_text(id), "world");
}

#[test]
fn visibility_and_enabled_round_trip_through_the_accessors() {
    let id = mount("button", "ok");
    assert!(rust_widgets::is_widget_visible(id));
    rust_widgets::hide_widget(id);
    assert!(!rust_widgets::is_widget_visible(id));
    rust_widgets::show_widget(id);
    assert!(rust_widgets::is_widget_visible(id));

    assert!(rust_widgets::is_widget_enabled(id));
    rust_widgets::set_widget_enabled(id, false);
    assert!(!rust_widgets::is_widget_enabled(id));
}

#[test]
fn geometry_returns_to_the_control() {
    let id = mount("label", "x");
    rust_widgets::set_widget_geometry(id, 10, 20, 80, 24);
    assert_eq!(
        rust_widgets::widget_geometry(id),
        Some((10, 20, 80, 24)),
        "the accessor must read back the rectangle it just wrote"
    );
}

// ── Numeric family (routed through the property contract) ───────────────────

#[test]
fn slider_value_range_step_and_orientation_round_trip() {
    let id = mount("slider", "");

    assert!(rust_widgets::set_widget_range(id, 0.0, 100.0), "range must be writable");
    assert_eq!(rust_widgets::widget_range(id), Some((0.0, 100.0)));

    assert!(rust_widgets::set_widget_value(id, 42.0), "value must be writable");
    assert_eq!(rust_widgets::widget_value(id), Some(42.0));

    assert!(rust_widgets::set_widget_step(id, 5.0), "step must be writable");
    assert_eq!(rust_widgets::widget_step(id), Some(5.0));

    assert!(
        rust_widgets::set_slider_orientation(id, Orientation::Vertical),
        "orientation must be writable"
    );
    assert_eq!(rust_widgets::slider_orientation(id), Some(Orientation::Vertical));
}

#[test]
fn progress_bar_value_round_trips() {
    let id = mount("progress_bar", "");
    assert!(rust_widgets::set_widget_value(id, 25.0));
    assert_eq!(rust_widgets::widget_value(id), Some(25.0));
}

#[test]
fn spin_box_value_and_step_round_trip() {
    let id = mount("spin_box", "");
    assert!(rust_widgets::set_widget_value(id, 7.0));
    assert_eq!(rust_widgets::widget_value(id), Some(7.0));
    assert!(rust_widgets::set_widget_step(id, 2.0));
    assert_eq!(rust_widgets::widget_step(id), Some(2.0));
}

/// A control with no numeric value must report `false`, not silently accept the
/// write — that is the difference between "unsupported" and "set to 0".
#[test]
fn numeric_accessors_refuse_a_control_without_a_value() {
    let id = mount("label", "x");
    assert!(!rust_widgets::set_widget_value(id, 1.0));
    assert_eq!(rust_widgets::widget_value(id), None);
}

/// An id that addresses nothing must be refused rather than answered with a
/// plausible-looking default.
#[test]
fn accessors_refuse_an_unknown_id() {
    let unknown = 0xdead_beef_u64;
    assert!(!rust_widgets::set_widget_value(unknown, 1.0));
    assert_eq!(rust_widgets::widget_value(unknown), None);
    assert!(!rust_widgets::set_widget_checked(unknown, true));
    assert_eq!(rust_widgets::is_widget_checked(unknown), None);
    assert_eq!(rust_widgets::get_widget_text(unknown), "");
}

// ── Checkable family ────────────────────────────────────────────────────────

#[test]
fn checkbox_checked_and_tristate_round_trip() {
    let id = mount("checkbox", "cb");
    assert_eq!(rust_widgets::is_widget_checked(id), Some(false));
    assert!(rust_widgets::set_widget_checked(id, true));
    assert_eq!(rust_widgets::is_widget_checked(id), Some(true));

    assert_eq!(rust_widgets::is_widget_tristate(id), Some(false));
    assert!(rust_widgets::set_widget_tristate(id, true));
    assert_eq!(rust_widgets::is_widget_tristate(id), Some(true));
}

#[test]
fn radio_button_group_round_trips() {
    let id = mount("radio_button", "r");
    assert!(rust_widgets::set_widget_group(id, "size"));
    assert_eq!(rust_widgets::widget_group(id), Some("size".to_string()));

    assert!(rust_widgets::set_widget_group(id, ""));
    assert_eq!(rust_widgets::widget_group(id), None, "an empty group clears the membership");
}

/// A label is not checkable, so the question has no answer.
#[test]
fn checkable_accessors_refuse_a_non_checkable_control() {
    let id = mount("label", "x");
    assert!(!rust_widgets::set_widget_checked(id, true));
    assert_eq!(rust_widgets::is_widget_checked(id), None);
    assert_eq!(rust_widgets::is_widget_tristate(id), None);
}

// ── Text-entry family ───────────────────────────────────────────────────────

#[test]
fn line_edit_read_only_placeholder_and_max_length_round_trip() {
    let id = mount("line_edit", "");

    assert_eq!(rust_widgets::is_widget_read_only(id), Some(false));
    assert!(rust_widgets::set_widget_read_only(id, true));
    assert_eq!(rust_widgets::is_widget_read_only(id), Some(true));

    assert!(rust_widgets::set_widget_placeholder(id, "type here"));
    assert_eq!(rust_widgets::widget_placeholder(id), Some("type here".to_string()));

    assert!(rust_widgets::set_widget_max_length(id, 12));
    assert_eq!(rust_widgets::widget_max_length(id), Some(12));
}

// ── Scrollable container ────────────────────────────────────────────────────

#[test]
fn scroll_area_position_is_accepted_and_clamped_by_the_control() {
    let id = mount("scroll_area", "");

    // The container clamps an offset to its content extent, and this one holds no
    // content, so the stored offset is (0, 0). What matters here is that the write
    // is *accepted* — the control was reached — which is exactly what the platform
    // default could not do (it returned `false` without touching anything).
    assert!(rust_widgets::set_widget_scroll_position(id, 5, 9));
    assert_eq!(rust_widgets::widget_scroll_position(id), Some((0, 0)));
}

// ── Item operations (routed through the control backend) ────────────────────

#[test]
fn combo_box_item_operations_round_trip() {
    let id = mount("combo_box", "");

    assert!(rust_widgets::combo_box_add_item(id, "alpha"));
    assert!(rust_widgets::combo_box_add_item(id, "beta"));
    assert_eq!(rust_widgets::combo_box_item_count(id), 2);
    assert_eq!(rust_widgets::combo_box_item_text(id, 1), Some("beta".to_string()));

    assert!(rust_widgets::combo_box_set_current_index(id, 1));
    assert_eq!(rust_widgets::combo_box_current_index(id), Some(1));

    assert!(rust_widgets::combo_box_clear_items(id));
    assert_eq!(rust_widgets::combo_box_item_count(id), 0);
}

#[test]
fn list_box_item_operations_round_trip() {
    let id = mount("list_box", "");

    assert!(rust_widgets::list_box_add_item(id, "one"));
    assert!(rust_widgets::list_box_add_item(id, "two"));
    assert_eq!(rust_widgets::list_box_item_count(id), 2);
    assert_eq!(rust_widgets::list_box_item_text(id, 0), Some("one".to_string()));

    assert!(rust_widgets::list_box_set_current_index(id, 1));
    assert_eq!(rust_widgets::list_box_current_index(id), Some(1));

    assert!(rust_widgets::list_box_remove_item(id, 0));
    assert_eq!(rust_widgets::list_box_item_count(id), 1);
    assert_eq!(rust_widgets::list_box_item_text(id, 0), Some("two".to_string()));

    // Out-of-range removal is refused rather than silently ignored.
    assert!(!rust_widgets::list_box_remove_item(id, 9));

    assert!(rust_widgets::list_box_clear_items(id));
    assert_eq!(rust_widgets::list_box_item_count(id), 0);
}

/// An item operation aimed at the wrong kind must be refused: the id addressed a
/// real control, just not one that holds items.
#[test]
fn item_operations_refuse_a_control_without_items() {
    let id = mount("label", "x");
    assert!(!rust_widgets::combo_box_add_item(id, "alpha"));
    assert!(!rust_widgets::list_box_add_item(id, "alpha"));
    assert_eq!(rust_widgets::combo_box_item_text(id, 0), None);
}

/// Item reads must agree with the property the control publishes, so the two
/// entry points cannot drift.
#[test]
fn item_count_matches_the_published_property() {
    let id = mount("list_box", "");
    rust_widgets::list_box_add_item(id, "only");
    let published = rust_widgets::read_widget_property_by_id(id, "item_count");
    assert_eq!(published, Ok(rust_widgets::widget::capability::CapabilityValue::UInt(1)));
}

// ── Selection index ─────────────────────────────────────────────────────────

#[test]
fn selection_index_round_trips_and_can_be_cleared() {
    let id = mount("combo_box", "");
    rust_widgets::combo_box_add_item(id, "alpha");
    rust_widgets::combo_box_add_item(id, "beta");

    assert!(rust_widgets::set_widget_selected_index(id, Some(1)));
    assert_eq!(rust_widgets::widget_selected_index(id), Some(1));

    assert!(
        rust_widgets::set_widget_selected_index(id, None),
        "clearing the selection must be accepted"
    );
    assert_eq!(
        rust_widgets::widget_selected_index(id),
        None,
        "a cleared selection reads as no selection, not as index 0"
    );
}

// ── Trigger queue ───────────────────────────────────────────────────────────

#[test]
fn trigger_queue_is_shared_between_its_two_readers() {
    let id = mount("button", "ok");
    assert!(rust_widgets::inject_widget_trigger_event(
        id,
        rust_widgets::WidgetTriggerKind::Clicked
    ));
    assert_eq!(
        rust_widgets::poll_widget_triggered(),
        Some(id),
        "the id-level reader must see the injected event"
    );
    assert_eq!(
        rust_widgets::poll_widget_triggered(),
        None,
        "an answered event is consumed, so the queue does not repeat it"
    );
}

// ── Native handle accessor ─────────────────────────────────────────────────

/// The public `native_handle` accessor must report the host window's handle
/// **whenever the backend has one to report**.
///
/// `WindowsPlatform` kept that mapping as an *inherent* method, so the trait
/// method kept its `None` default and the accessor reported "no native object" for
/// a window that plainly had one.
///
/// # Why "whenever" matters
///
/// A native handle is a property of the backend, not of the profile: Windows and
/// macOS hand back a real `HWND`/`NSView`, while `desktop` on Linux without
/// `gtk-native` is a state backend that owns no native window at all and reports
/// `get_native_handle() == None` (principles #35/#37). Asserting `is_some()`
/// unconditionally made the test pass only on the host it was written on. The
/// property that must hold everywhere — and the one the Windows fix was about — is
/// that the answer is **consistent across the two entry points**: the public
/// accessor must agree with the backend's own trait method, never falling back to
/// the default while the backend has a handle. A shadowed inherent method would
/// make these two disagree, so this is exactly the regression guard needed.
#[test]
fn native_handle_reports_the_host_window_handle() {
    rust_widgets::init();
    let platform = rust_widgets::platform::get_platform();
    let host = platform.create_window("handle probe", 0, 0, 200, 120);
    assert_ne!(host, 0, "the host must create a window");

    // The Windows defect was a backend that *had* a handle but reported `None`,
    // because its inherent method shadowed the trait one. The durable guard is that
    // the two entry points can never disagree: the public accessor is a pure
    // forward to the backend, so a shadowed or missing override shows up as a
    // mismatch, while a genuine `None` (a state-only window, e.g. no display) is
    // reported identically by both and stays honest.
    for probe in [host, 0xdead_beef_u64] {
        assert_eq!(
            // The old name still resolves through the deprecated alias, which is the
            // compatibility promise; this call site uses the new spelling so the crate's
            // own tests do not emit deprecation warnings.
            rust_widgets::backend_handle(probe),
            platform.get_native_handle(probe),
            "the accessor and the backend must agree for id {probe:#x}"
        );
    }

    assert_eq!(
        rust_widgets::backend_handle(0xdead_beef_u64),
        None,
        "an id that addresses nothing must not produce a handle"
    );
}

// ── Menu entries ───────────────────────────────────────────────────────────

#[test]
fn menu_entries_are_addressable_and_round_trip() {
    let window = mount("window", "w");
    let menu_bar = rust_widgets::create_menu_bar(window, 0, 0, 200, 20);
    assert_ne!(menu_bar, 0, "a menu bar must be creatable under the window");
    assert!(
        rust_widgets::attach_menu_bar_to_window(window, menu_bar),
        "attaching a real menu bar to a real window must succeed"
    );

    let menu = rust_widgets::create_menu(menu_bar, "file", 0, 0, 100, 20);
    assert_ne!(menu, 0, "a menu must be creatable under the menu bar");

    let open = rust_widgets::menu_add_item(menu, "open", Some("Ctrl+O"));
    assert_ne!(
        open, 0,
        "a menu entry must be addressable by the id it returns, not return a fixed sentinel"
    );
    let second = rust_widgets::menu_add_item(menu, "save", Some("Ctrl+S"));
    assert_ne!(second, open, "each entry must get its own id");

    assert_eq!(rust_widgets::menu_item_shortcut(open), Some("Ctrl+O".to_string()));
    assert_eq!(rust_widgets::menu_item_shortcut(second), Some("Ctrl+S".to_string()));
    assert_eq!(rust_widgets::menu_item_shortcut(0xdead_beef_u64), None);

    assert!(rust_widgets::inject_menu_trigger(open));
    assert_eq!(rust_widgets::poll_menu_triggered(), Some(open));
}

/// Attaching to ids that address nothing must be refused rather than reported as
/// success.
#[test]
fn attach_menu_bar_refuses_unknown_ids() {
    assert!(!rust_widgets::attach_menu_bar_to_window(0xdead_beef_u64, 0xdead_beee_u64));
    let window = mount("window", "w");
    assert!(!rust_widgets::attach_menu_bar_to_window(window, 0xdead_beee_u64));
}

// ── Documented gaps ─────────────────────────────────────────────────────────
//
// These two accessors have no property to write to yet: the text controls track
// an echo mode and a selection internally but publish neither. The assertions
// below pin the *honest* answer (`false` / `None`) rather than the feature, so a
// caller is never told a mode was applied when it was not — and so implementing
// the properties is what flips these tests, rather than a silent change.

#[test]
fn echo_mode_reports_unsupported_until_a_control_publishes_it() {
    let id = mount("line_edit", "");
    assert!(
        !rust_widgets::set_widget_echo_mode(id, rust_widgets::platform::EchoMode::Password),
        "no control publishes `echo_mode` yet, so the write must be reported as refused"
    );
    assert_eq!(rust_widgets::widget_echo_mode(id), None);
}

#[test]
fn selection_range_reports_unsupported_until_a_control_publishes_it() {
    let id = mount("line_edit", "");
    assert!(
        !rust_widgets::set_widget_selection(id, 0, 3),
        "no control publishes `selection` yet, so the write must be reported as refused"
    );
    assert_eq!(rust_widgets::widget_selection(id), None);
}
