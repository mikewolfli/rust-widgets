// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for `ToastStack` and `Toast`.

// The module-level gate, so tooling that scans file text recognises this as a test
// module. `mod.rs` declares `mod tests;` under `#[cfg(test)]`, which the compiler
// sees but a text scan of this file does not.
#![cfg(test)]

use super::item::{ToastItem, ToastLevel};
use super::single::Toast;
use super::stack::ToastStack;
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use crate::widget::capability::types::CapabilityValue;
use crate::widget::capability::WidgetProperties;
use crate::widget::Widget;
use std::sync::{Arc, Mutex};

#[test]
fn push_and_dismiss_update_len() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "Saved", ToastLevel::Success, 2500));
    stack.push(ToastItem::new("t2", "Build failed", ToastLevel::Error, 3500));

    assert_eq!(stack.toasts().len(), 2);
    assert_eq!(stack.selected_id(), Some("t2"));

    assert!(stack.dismiss_selected());
    assert_eq!(stack.toasts().len(), 1);
    assert_eq!(stack.selected_id(), Some("t1"));
}

#[test]
fn activate_emits_signal() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "Saved", ToastLevel::Success, 2500));

    let activated = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = activated.clone();
    stack.toast_activated.connect(move |id| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(id.as_ref().clone());
        }
    });

    assert!(stack.activate_selected());
    let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got, vec!["t1".to_string()]);
}

#[test]
fn delete_key_dismisses_selected() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
    stack.push(ToastItem::new("t2", "B", ToastLevel::Warning, 1000));

    stack.handle_event(&Event::key_press(46, 0));
    assert_eq!(stack.toasts().len(), 1);
    assert_eq!(stack.selected_id(), Some("t1"));
}

#[test]
fn new_creates_default_state() {
    let stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    assert!(stack.toasts().is_empty());
    assert_eq!(stack.selected_id(), None);
}

#[test]
fn clear_removes_all() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
    stack.push(ToastItem::new("t2", "B", ToastLevel::Success, 2000));
    assert_eq!(stack.toasts().len(), 2);

    stack.clear();
    assert!(stack.toasts().is_empty());
    assert_eq!(stack.selected_id(), None);
}

#[test]
fn select_index_invalid_returns_false() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    assert!(!stack.select_index(0));

    stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
    assert!(!stack.select_index(5));
    assert_eq!(stack.selected_id(), Some("t1")); // still defaults to last
}

#[test]
fn activate_selected_on_empty_returns_false() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    assert!(!stack.activate_selected());
}

#[test]
fn dismiss_selected_on_empty_returns_false() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    assert!(!stack.dismiss_selected());
}

#[test]
fn push_sets_selected_to_new_item() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 800, 600));
    stack.push(ToastItem::new("t1", "First", ToastLevel::Info, 1000));
    assert_eq!(stack.selected_id(), Some("t1"));

    stack.push(ToastItem::new("t2", "Second", ToastLevel::Info, 1000));
    assert_eq!(stack.selected_id(), Some("t2"));
}

#[test]
fn keyboard_navigation_up_down() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "First", ToastLevel::Info, 1000));
    stack.push(ToastItem::new("t2", "Second", ToastLevel::Warning, 1000));
    stack.push(ToastItem::new("t3", "Third", ToastLevel::Error, 1000));

    // Default selected is last pushed (t3)
    assert_eq!(stack.selected_id(), Some("t3"));

    // Up arrow to t2
    stack.handle_event(&Event::key_press(38, 0));
    assert_eq!(stack.selected_id(), Some("t2"));

    // Up again to t1
    stack.handle_event(&Event::key_press(38, 0));
    assert_eq!(stack.selected_id(), Some("t1"));

    // Up at top stays
    stack.handle_event(&Event::key_press(38, 0));
    assert_eq!(stack.selected_id(), Some("t1"));

    // Down arrow to t2
    stack.handle_event(&Event::key_press(40, 0));
    assert_eq!(stack.selected_id(), Some("t2"));

    // Down to t3
    stack.handle_event(&Event::key_press(40, 0));
    assert_eq!(stack.selected_id(), Some("t3"));

    // Down at bottom stays
    stack.handle_event(&Event::key_press(40, 0));
    assert_eq!(stack.selected_id(), Some("t3"));
}

#[test]
fn dismiss_selected_emits_signal() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));
    stack.push(ToastItem::new("t2", "B", ToastLevel::Warning, 1000));

    let dismissed = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = dismissed.clone();
    stack.toast_dismissed.connect(move |id| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(id.as_ref().clone());
        }
    });

    assert!(stack.dismiss_selected());
    let got = dismissed.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got, vec!["t2".to_string()]);
}

#[test]
fn enter_key_activates_selected() {
    let mut stack = ToastStack::new(Rect::new(0, 0, 360, 180));
    stack.push(ToastItem::new("t1", "A", ToastLevel::Info, 1000));

    let activated = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = activated.clone();
    stack.toast_activated.connect(move |id| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(id.as_ref().clone());
        }
    });

    stack.handle_event(&Event::key_press(13, 0));
    let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
    assert_eq!(got, vec!["t1".to_string()]);
}

#[test]
fn toast_item_ttl_min_100() {
    let item = ToastItem::new("id", "msg", ToastLevel::Info, 20);
    assert_eq!(item.ttl_ms, 100);
}

// ── `Toast` — one message, no queue ──────────────────────────────────────────

/// A fresh toast reports the message it was built with, at informational level.
///
/// `Toast` had no tests of its own until this module was split out of one file:
/// every assertion in the original `toast.rs` was about `ToastStack`, so the single
/// toast — the control most callers reach for — was covered only by the constructors
/// test that proves it exists.
#[test]
fn a_new_toast_carries_its_message_and_level() {
    let toast = Toast::new(Rect::new(0, 0, 280, 48), "Saved");
    assert_eq!(toast.message(), "Saved");
    assert_eq!(toast.level(), ToastLevel::Info);
    assert!(toast.is_dismissible());
    assert_eq!(toast.ttl_ms(), 3000);
}

/// Dismissing emits, and carries the message so a handler does not have to find the
/// control again.
#[test]
fn dismissing_emits_with_the_message() {
    let mut toast = Toast::new(Rect::new(0, 0, 280, 48), "Saved");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    toast.dismissed.connect(move |message| {
        sink.lock().expect("signal sink poisoned").push(message.as_ref().clone());
    });

    toast.dismiss();
    assert_eq!(seen.lock().expect("lock").as_slice(), ["Saved"]);

    // A second dismiss emits again: this is a signal, not a one-shot state change,
    // and a host that re-arms the toast for another expiry relies on that.
    toast.set_message("Saved again");
    toast.dismiss();
    assert_eq!(seen.lock().expect("lock").as_slice(), ["Saved", "Saved again"]);
}

/// A non-dismissible toast still expires; the flag controls the *close affordance*,
/// not whether the program may end it.
#[test]
fn a_non_dismissible_toast_can_still_be_dismissed_by_the_program() {
    let mut toast = Toast::new(Rect::new(0, 0, 280, 48), "Working");
    toast.set_dismissible(false);
    assert!(!toast.is_dismissible());

    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    toast.dismissed.connect(move |message| {
        sink.lock().expect("signal sink poisoned").push(message.as_ref().clone());
    });

    toast.dismiss();
    assert_eq!(
        seen.lock().expect("lock").as_slice(),
        ["Working"],
        "the flag hides the close button; it must not gate the program's own dismissal"
    );
}

/// The ttl is a hint with the same floor as [`ToastItem`], so the two cannot disagree
/// about what a usable duration is.
#[test]
fn the_ttl_hint_shares_toast_items_floor() {
    let mut toast = Toast::new(Rect::new(0, 0, 200, 40), "x");
    toast.set_ttl_ms(20);
    assert_eq!(toast.ttl_ms(), 100, "a duration below 100 ms is not a duration");
    assert_eq!(toast.ttl_ms(), ToastItem::new("i", "m", ToastLevel::Info, 20).ttl_ms);
}

/// Escape dismisses, and only Escape: a stray key must not make a toast vanish.
#[test]
fn escape_dismisses_but_no_other_key_does() {
    let mut toast = Toast::new(Rect::new(0, 0, 280, 48), "Saved");
    let seen = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&seen);
    toast.dismissed.connect(move |_| {
        *sink.lock().expect("signal sink poisoned") += 1;
    });

    toast.handle_event(&Event::key_press(65, 0)); // 'A'
    assert_eq!(*seen.lock().expect("lock"), 0, "a letter must not dismiss a toast");

    toast.handle_event(&Event::key_press(27, 0)); // Escape
    assert_eq!(*seen.lock().expect("lock"), 1);
}

/// Clicking the close affordance dismisses; clicking elsewhere does not.
///
/// Asserted through the hit test, because "the toast disappeared" is only correct if
/// it disappeared for the click *on the button*.
#[test]
fn clicking_the_close_affordance_dismisses() {
    let mut toast = Toast::new(Rect::new(0, 0, 200, 40), "Saved");
    let seen = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&seen);
    toast.dismissed.connect(move |_| {
        *sink.lock().expect("signal sink poisoned") += 1;
    });

    // A press well away from the right-hand close area.
    toast.handle_event(&Event::MousePress { pos: Point::new(5, 20), button: 1 });
    assert_eq!(*seen.lock().expect("lock"), 0, "a click on the body must be ignored");

    // The close affordance sits near the right edge, vertically centred.
    toast.handle_event(&Event::MousePress { pos: Point::new(188, 20), button: 1 });
    assert_eq!(*seen.lock().expect("lock"), 1, "a click on the close button must dismiss");
}

/// A non-dismissible toast has no close affordance, so a press there is not a close.
#[test]
fn a_non_dismissible_toast_has_no_close_target() {
    let mut toast = Toast::new(Rect::new(0, 0, 200, 40), "Working");
    toast.set_dismissible(false);

    let seen = Arc::new(Mutex::new(0usize));
    let sink = Arc::clone(&seen);
    toast.dismissed.connect(move |_| {
        *sink.lock().expect("signal sink poisoned") += 1;
    });

    toast.handle_event(&Event::MousePress { pos: Point::new(188, 20), button: 1 });
    assert_eq!(
        *seen.lock().expect("lock"),
        0,
        "without an affordance the same pixels must not dismiss"
    );
}

/// The property contract round-trips, and refuses a value of the wrong kind.
#[test]
fn the_property_contract_round_trips() {
    let mut toast = Toast::new(Rect::new(0, 0, 240, 44), "Saved");

    assert_eq!(
        toast.get("message").expect("readable"),
        CapabilityValue::String("Saved".to_string())
    );
    assert_eq!(toast.get("level").expect("readable"), CapabilityValue::String("info".into()));
    assert_eq!(toast.get("ttl_ms").expect("readable"), CapabilityValue::UInt(3000));
    assert_eq!(toast.get("dismissible").expect("readable"), CapabilityValue::Bool(true));

    assert!(toast.set("level", CapabilityValue::String("error".into())).is_ok());
    assert_eq!(toast.level(), ToastLevel::Error);

    assert!(toast.set("ttl_ms", CapabilityValue::UInt(500)).is_ok());
    assert_eq!(toast.ttl_ms(), 500);

    assert!(
        toast.set("level", CapabilityValue::String("chartreuse".into())).is_err(),
        "an unrecognised level token must be refused, not defaulted"
    );
    assert_eq!(toast.level(), ToastLevel::Error, "a refused write must not partially apply");

    assert!(toast.set("dismissible", CapabilityValue::Bool(false)).is_ok());
    assert!(!toast.is_dismissible());

    assert_eq!(
        toast.set("message", CapabilityValue::Bool(true)),
        Err(crate::widget::capability::types::CapabilityAccessError::TypeMismatch)
    );
}

/// Every declared property must actually be readable, or the schema is describing a
/// control that does not exist.
#[test]
fn the_declared_property_names_match_the_contract() {
    let toast = Toast::new(Rect::new(0, 0, 200, 40), "x");
    for name in toast.property_names() {
        assert!(toast.get(name).is_ok(), "{name} is declared but the contract refuses to read it");
    }
}

/// A toast is its own kind, so the factory lookup and the accessibility role name a
/// notification rather than the stack that holds one.
#[test]
fn a_toast_declares_its_own_kind() {
    let toast = Toast::new(Rect::new(0, 0, 200, 40), "x");
    assert_eq!(toast.kind(), crate::widget::WidgetKind::Toast);
    assert_eq!(
        toast.size_hint(),
        crate::core::Size::new(320, 48),
        "the hint is one text row plus padding"
    );
}
