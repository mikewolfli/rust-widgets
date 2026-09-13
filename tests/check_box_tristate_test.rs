// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Regression tests for `CheckBoxHandle` tri-state semantics.
//!
//! `set_checked` used to update the tri-state `check_state` mirror **only when
//! tri-state mode was off**, so a tri-state box reported `is_checked() == true`
//! while `check_state()` stayed `Unchecked` forever — contradictory state, and
//! `PartiallyChecked` was unreachable through any API. These tests pin the
//! corrected behaviour so the guard cannot come back.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, CheckState, WidgetHandle};

/// A two-state check-box must track `set_checked` in both directions.
#[test]
fn plain_checkbox_tracks_set_checked() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("t", 0, 0, 300, 200);
    let cb = win.new_checkbox("plain", 0, 0, 100, 20);

    assert_eq!(cb.check_state(), CheckState::Unchecked);
    cb.set_checked(true);
    assert_eq!(cb.check_state(), CheckState::Checked);
    assert!(cb.is_checked());
    cb.set_checked(false);
    assert_eq!(cb.check_state(), CheckState::Unchecked);
    assert!(!cb.is_checked());
}

/// The regression: a **tri-state** box must still reflect `set_checked`, instead
/// of leaving a stale `Unchecked` behind.
#[test]
fn tristate_checkbox_reflects_set_checked_after_enabling_tristate() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("t", 0, 0, 300, 200);
    let cb = win.new_checkbox("tri", 0, 0, 100, 20);

    cb.set_tristate(true);
    assert_eq!(cb.is_tristate(), Some(true));

    cb.set_checked(true);
    assert_eq!(
        cb.check_state(),
        CheckState::Checked,
        "tri-state box must reflect set_checked(true)"
    );
    assert!(cb.is_checked());

    cb.set_checked(false);
    assert_eq!(cb.check_state(), CheckState::Unchecked);
}

/// `PartiallyChecked` must actually be reachable — it is the entire reason
/// tri-state mode exists, and it must not count as "checked".
#[test]
fn tristate_checkbox_can_reach_partially_checked() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("t", 0, 0, 300, 200);
    let cb = win.new_checkbox("tri", 0, 0, 100, 20);

    cb.set_tristate(true);
    assert!(cb.set_check_state(CheckState::PartiallyChecked), "mixed state must be settable");
    assert_eq!(cb.check_state(), CheckState::PartiallyChecked);
    assert!(!cb.is_checked(), "the mixed state is not the checked state");
}

/// The mixed state is meaningless without tri-state mode, so it must be refused
/// rather than silently degraded into one of the two real states.
#[test]
fn partially_checked_is_refused_without_tristate() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("t", 0, 0, 300, 200);
    let cb = win.new_checkbox("plain", 0, 0, 100, 20);

    cb.set_checked(true);
    assert!(!cb.set_check_state(CheckState::PartiallyChecked), "must refuse without tri-state");
    assert_eq!(cb.check_state(), CheckState::Checked, "refusal must not mutate the state");
}

/// Turning tri-state off while mixed must collapse to a state the control can
/// still display, not leave a stale mixed value behind.
#[test]
fn disabling_tristate_collapses_mixed_state() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("t", 0, 0, 300, 200);
    let cb = win.new_checkbox("tri", 0, 0, 100, 20);

    cb.set_tristate(true);
    cb.set_check_state(CheckState::PartiallyChecked);
    cb.set_tristate(false);

    assert_eq!(
        cb.check_state(),
        CheckState::Unchecked,
        "mixed must collapse when tri-state is off"
    );
    assert!(!cb.is_checked());
}

/// The handle layer and the widget layer must name the **same** three-state enum,
/// not two structurally identical ones. This is what lets a state read from a
/// handle be passed straight into a widget API with no conversion step.
#[test]
fn handle_and_widget_share_one_check_state_type() {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::base_widgets::checkbox::CheckBox as WidgetCheckBox;

    // If these were two distinct types, this assignment would not compile —
    // which is the point: the duplicate-enum defect becomes a build error.
    let state: CheckState = CheckState::PartiallyChecked;

    let mut widget = WidgetCheckBox::new(Rect::new(0, 0, 80, 20));
    widget.set_state(state);
    assert_eq!(widget.state(), state);
    assert!(widget.is_partially_checked());
}
