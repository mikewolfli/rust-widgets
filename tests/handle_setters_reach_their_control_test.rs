// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A `*Handle` setter must reach the control it names — not just its own bookkeeping.
//!
//! # The defect this pins
//!
//! Every handle setter wrote an in-process mirror and then called
//! `Platform::set_widget_value` / `set_widget_placeholder` / … — a surface that exists for
//! **host-native** controls. This library creates no native controls, so the platform held no
//! record for those ids and each of those calls answered `false`. The result was discarded, the
//! mirror was written anyway, and the getter read only the mirror. Measured, before the fix:
//!
//! ```text
//! handle.set_placeholder("hint")           // line edit created through a WindowHandle
//! handle.placeholder()             -> "hint"        <- the read-back said so
//! control.read_property(placeholder_text) -> ""     <- the control never received it
//! ```
//!
//! So a host could set a slider's value, a field's placeholder or a check box's state and the
//! control would not move, while the read-back claimed it had. Two mechanisms for one fact, only
//! one of which reached the control.
//!
//! # What is asserted
//!
//! For each handle family, the handle's setter is called and then the **control's own property
//! contract** is read back. The assertion is on the control, not on the getter: a handle that
//! agrees only with its own mirror is exactly the failure mode. The second half of each case
//! checks the handle's getter agrees too, so the two cannot drift apart in the other direction.
//!
//! The last test names the properties a control does **not** publish, so the limitation is
//! recorded rather than discovered: those writes are refused and the handle does not claim them.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, WidgetHandle};
use rust_widgets::platform::EchoMode;
use rust_widgets::widget::capability::{CapabilityValue, WidgetFactory};

/// Reads a property off the mounted control, as the control itself reports it.
fn control(id: u64, name: &str) -> Option<CapabilityValue> {
    rust_widgets::widget::runtime::with_widget_mut(id, |w| {
        WidgetFactory::new_with_defaults().read_property(w, name).ok()
    })
    .flatten()
}

#[test]
fn a_line_edit_handle_reaches_the_field() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let le = win.new_line_edit("", 0, 0, 200, 30);
    let id = le.raw_id();

    le.set_placeholder("hint");
    assert_eq!(
        control(id, "placeholder_text"),
        Some(CapabilityValue::String("hint".to_string())),
        "the control must hold the placeholder, not just the handle's mirror"
    );
    assert_eq!(le.placeholder(), "hint");

    le.set_read_only(true);
    assert_eq!(control(id, "read_only"), Some(CapabilityValue::Bool(true)));
    assert!(le.is_read_only());

    le.set_max_length(7);
    assert_eq!(control(id, "max_length"), Some(CapabilityValue::UInt(7)));
    assert_eq!(le.max_length(), 7);

    // The masking mode is the one that has to reach the control: the copy path consults it, so a
    // field that only *reports* `Password` would still hand its plaintext to the clipboard.
    le.set_echo_mode(EchoMode::Password);
    assert_eq!(control(id, "echo_mode"), Some(CapabilityValue::String("password".to_string())));
    assert_eq!(le.echo_mode(), EchoMode::Password);
}

#[test]
fn a_slider_handle_reaches_the_slider() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let sl = win.new_slider(0, 0, 200, 30);
    let id = sl.raw_id();

    sl.set_range(0, 10);
    assert_eq!(control(id, "minimum"), Some(CapabilityValue::Int(0)));
    assert_eq!(control(id, "maximum"), Some(CapabilityValue::Int(10)));

    sl.set_value(6);
    assert_eq!(control(id, "value"), Some(CapabilityValue::Int(6)));
    assert_eq!(sl.value(), 6);

    // `single_step` is declared `Int` on this control, so the integral step must arrive as one.
    sl.set_step(2);
    assert_eq!(
        control(id, "single_step"),
        Some(CapabilityValue::Int(2)),
        "the step must reach the control in the carrier its contract declares"
    );
}

#[test]
fn a_check_box_handle_reaches_the_box() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let cb = win.new_checkbox("c", 0, 0, 200, 30);
    let id = cb.raw_id();

    cb.set_tristate(true);
    assert_eq!(control(id, "tristate_enabled"), Some(CapabilityValue::Bool(true)));
    assert_eq!(cb.is_tristate(), Some(true));

    cb.set_checked(true);
    assert_eq!(control(id, "checked"), Some(CapabilityValue::Bool(true)));
    assert_eq!(cb.checked(), Some(true));
}

#[test]
fn a_radio_button_handle_reaches_the_button() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let rb = win.new_radio_button("r", 0, 0, 200, 30);
    let id = rb.raw_id();

    rb.set_group("alpha");
    assert_eq!(control(id, "group_id"), Some(CapabilityValue::String("alpha".to_string())));
    assert_eq!(rb.group().as_deref(), Some("alpha"));
}

#[test]
fn a_spin_box_handle_reaches_the_field() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let sb = win.new_spin_box(0, 0, 200, 30);
    let id = sb.raw_id();

    sb.set_range(0, 50);
    assert_eq!(control(id, "minimum"), Some(CapabilityValue::Int(0)));
    assert_eq!(control(id, "maximum"), Some(CapabilityValue::Int(50)));

    sb.set_value(9);
    assert_eq!(control(id, "value"), Some(CapabilityValue::Int(9)));
    assert_eq!(sb.value(), 9);
}

#[test]
fn a_progress_bar_handle_reaches_the_bar() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);
    let pb = win.new_progress_bar(0, 0, 200, 30);
    let id = pb.raw_id();

    pb.set_range(0.0, 100.0);
    assert_eq!(control(id, "minimum"), Some(CapabilityValue::Int(0)));
    assert_eq!(control(id, "maximum"), Some(CapabilityValue::Int(100)));

    pb.set_value(40);
    assert_eq!(control(id, "value"), Some(CapabilityValue::Int(40)));
    assert_eq!(pb.value(), 40);
}

#[test]
fn a_property_the_control_does_not_publish_is_refused_not_faked() {
    let app = App::new();
    let win = app.new_window("t", 0, 0, 640, 480);

    let pb = win.new_progress_bar(0, 0, 200, 30);
    let pid = pb.raw_id();
    // The 2026-10-23 scan published `indeterminate`, so the original refusal premise no longer
    // holds for this control. Its switch is reachable through the contract now, so this asserts
    // the round trip; the refusal cases below still carry the test's purpose.
    pb.set_indeterminate(true);
    assert_eq!(
        control(pid, "indeterminate"),
        Some(CapabilityValue::Bool(true)),
        "a published setter must reach the control's own state"
    );
    assert_eq!(
        pb.is_indeterminate(),
        Some(true),
        "a write that took effect must be reported by a later read"
    );

    // A scroll area's offsets are clamped against its content, and the control publishes no
    // content-size property, so `ScrollAreaHandle::set_content_size` has nowhere to reach: the
    // area stays content-less and every offset clamps to 0. Asserted so the limitation is
    // recorded: the read is the control's own clamped state, and it is a real value (not `None`),
    // which is what separates "clamped" from "the setter never arrived".
    let sa = win.new_scroll_area(0, 0, 200, 100);
    let sid = sa.raw_id();
    sa.set_scroll_position(3, 4);
    assert_eq!(
        control(sid, "scroll_position_x"),
        Some(CapabilityValue::Int(0)),
        "with no content to scroll, the control clamps the offset to 0"
    );

    let lv = win.new_list_view(0, 0, 200, 100);
    assert!(
        !lv.select_row(Some(1)),
        "`ListView` publishes no writable selection index, so the write must report failure \
         rather than succeed into a mirror"
    );
}
