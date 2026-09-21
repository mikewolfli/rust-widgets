// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Whether a click on a window's control actually reaches that control.
//!
//! # The defect this pins
//!
//! The Linux backend paints a window's whole widget tree into one `DrawingArea`. A control
//! that is only painted — a button, a check box, a line edit — has no surface of its own,
//! so before the painter took pointer input **no click ever reached a control**: the
//! controls were visible, their geometry was correct, and their callbacks never ran.
//!
//! # What is asserted
//!
//! A press at a control's centre must run that control's click callback, and a press at a
//! point no control covers must run nothing. That pair is what distinguishes "the click
//! found the control" from "any click runs every callback".

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Point, Rect};
use rust_widgets::event::Event;

#[test]
fn a_click_reaches_the_control_under_the_pointer() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();

    let win: WindowHandle = app.new_window("input probe", 0, 0, 400, 300);

    // A button with a click callback that counts how often it ran.
    let clicks = Arc::new(AtomicUsize::new(0));
    let button = win.new_button("Click Me", 20, 20, 160, 36);
    let counter = Arc::clone(&clicks);
    button.on_click(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    // Whether the callback is connected at all: a `false` here means `on_click` found no
    // live widget to attach to, which is a different failure from "the click did not
    // arrive".
    let connected = rust_widgets::widget::runtime::with_widget(button.raw_id(), |widget| {
        widget.base().clicked.slot_count()
    });
    println!("[probe] click slots connected = {connected:?} (expected >= 1)");

    // The window is the root the router starts from, and its child list is how the
    // router finds the button (see `control_backend::custom::mount_named_widget`).
    let root = win.raw_id();
    let centre = Point::new(20 + 160 / 2, 20 + 36 / 2);

    // A press inside the button must reach it. The router runs on press (the release
    // completes the gesture), so this is the press half.
    let hit = rust_widgets::widget::runtime::dispatch_pointer_event(
        root,
        &Event::MousePress { pos: centre, button: 1 },
        centre,
    );
    assert!(hit, "a press at the button's centre must be delivered to a control");

    // The callback itself runs on release for a button.
    let release = rust_widgets::widget::runtime::dispatch_pointer_event(
        root,
        &Event::MouseRelease { pos: centre, button: 1 },
        centre,
    );
    assert!(release, "a release at the button's centre must be delivered");

    let after_hit = clicks.load(Ordering::SeqCst);
    assert!(
        after_hit > 0,
        "the button's click callback never ran for a press at its centre \
         (the tree is painted but not receiving input)"
    );

    // A press on empty space must not run it again: that is what makes the assertion
    // above evidence of *routing* rather than of a callback that always fires.
    let empty = Point::new(390, 290);
    rust_widgets::widget::runtime::dispatch_pointer_event(
        root,
        &Event::MousePress { pos: empty, button: 1 },
        empty,
    );
    rust_widgets::widget::runtime::dispatch_pointer_event(
        root,
        &Event::MouseRelease { pos: empty, button: 1 },
        empty,
    );
    assert_eq!(
        clicks.load(Ordering::SeqCst),
        after_hit,
        "a click on empty space must not run the button's callback"
    );

    // The button's geometry is what the hit test used, so report it on failure.
    let geometry = rust_widgets::widget::runtime::geometry_of(button.raw_id());
    assert_eq!(geometry, Some(Rect::new(20, 20, 160, 36)), "the button keeps its rect");
}
