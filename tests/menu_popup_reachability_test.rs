// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A menu bar's titles and its popups must be reachable from the public API.
//!
//! # The gap this pins
//!
//! `new_menu` created a `Menu` widget and nothing else: the bar's entry list stayed
//! empty (so the strip painted no titles), and a `Menu` is a popup that starts hidden
//! with no public way to open it. The control demo documented the first half and
//! therefore shipped without a menu bar at all.
//!
//! The assertions below are the three facts a working menu needs:
//!
//! 1. creating a menu under a bar appends its title to the bar;
//! 2. a menu opened at an entry's rectangle becomes visible there, with a non-zero
//!    size even when the caller passed the documented `0, 0` width and height;
//! 3. a press on a drawn row reaches the menu even when a control created **after**
//!    the menu covers the same pixels — the popup is raised to the top of the tree
//!    when it opens.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use std::sync::{Arc, Mutex};

use rust_widgets::app::{App, WidgetHandle};
use rust_widgets::core::{Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::runtime::{dispatch_pointer_event, with_widget_mut};

#[test]
fn a_bar_title_click_reaches_the_handle_callback_with_its_rectangle() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("menu bar callback", 0, 0, 640, 480);
    let bar = win.new_menu_bar(0, 0, 640, 28);
    let _file = win.new_menu(&bar, "File", 0, 0, 200, 0);

    let entry = bar.entry_rect(0).expect("the File entry must have a rectangle");
    let fired: Arc<Mutex<Vec<(String, Rect)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&fired);
    bar.on_entry_triggered(move |title, rect| {
        if let Ok(mut rows) = sink.lock() {
            rows.push((title, rect));
        }
    });

    let point = Point::new(entry.x + entry.width as i32 / 2, entry.y + entry.height as i32 / 2);
    assert!(dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: point, button: 1, modifiers: 0 },
        point,
    ));

    assert_eq!(
        &*fired.lock().expect("sink poisoned"),
        &[("File".to_string(), entry)],
        "the callback must receive the entry that was clicked, rectangle included"
    );
}

#[test]
fn a_press_outside_an_open_menu_dismisses_it_instead_of_reaching_the_control_below() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("menu modality", 0, 0, 640, 480);

    let bar = win.new_menu_bar(0, 0, 640, 28);
    let file = win.new_menu(&bar, "File", 0, 0, 200, 0);
    let _item = win.new_menu_item(&file, "Open", None);

    // Underneath the popup, so a press outside the menu would reach it without the grab.
    let clicks = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&clicks);
    let below = win.new_button("below", 0, 100, 300, 200);
    below.on_click(move || {
        if let Ok(mut count) = counter.lock() {
            *count += 1;
        }
    });

    assert!(file.open_at(0, 28, Rect::new(0, 0, 640, 480)));
    let outside = Point::new(100, 200);
    let handled = dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: outside, button: 1, modifiers: 0 },
        outside,
    );
    assert!(handled, "the dismissing press belongs to the open popup");
    assert!(!file.is_visible(), "an outside press must dismiss the menu");
    assert_eq!(
        *clicks.lock().expect("counter poisoned"),
        0,
        "the press must not also click the control the popup covered"
    );
}

#[test]
fn a_menu_title_reaches_its_bar_and_the_popup_opens_drawn() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("menu reachability", 0, 0, 640, 480);

    let bar = win.new_menu_bar(0, 0, 640, 28);
    assert_eq!(bar.entry_count(), 0, "a fresh bar has no entries");

    let file = win.new_menu(&bar, "File", 0, 0, 0, 0);
    let edit = win.new_menu(&bar, "Edit", 0, 0, 0, 0);
    assert_ne!(file.raw_id(), 0, "a menu must be a mounted widget");
    assert_ne!(edit.raw_id(), 0, "a second menu must be a mounted widget");
    assert_eq!(bar.entry_count(), 2, "each top-level menu appends one bar entry");

    // The second entry's box is where its popup belongs.
    let entry = bar.entry_rect(1).expect("the Edit entry must have a rectangle");
    assert!(entry.width > 0, "an entry with a title must have a width: {entry:?}");

    assert!(!file.is_visible(), "a menu is a popup and starts hidden");
    assert!(
        file.open_at(entry.x, entry.y + entry.height as i32, Rect::new(0, 0, 640, 480)),
        "opening a mounted menu must be reported as success"
    );
    assert!(file.is_visible(), "an opened menu must be visible");

    let geometry = rust_widgets::widget_geometry(file.raw_id()).expect("mounted menu geometry");
    assert!(geometry.2 > 0, "opened menu must have a drawable width: {geometry:?}");
    assert!(geometry.3 > 0, "opened menu must have a drawable height: {geometry:?}");

    assert!(file.close(), "closing a mounted menu must be reported as success");
    assert!(!file.is_visible(), "a closed menu must be hidden");
}

#[test]
fn a_click_on_a_drawn_row_beats_a_later_sibling_over_the_popup() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("menu clicks", 0, 0, 640, 480);

    let bar = win.new_menu_bar(0, 0, 640, 28);
    let file = win.new_menu(&bar, "File", 0, 0, 0, 0);
    let item = win.new_menu_item(&file, "Open", None);
    assert_ne!(item.raw_id(), 0, "the menu must contain the item");

    // Created **after** the menu, so its sibling order would put it above the popup
    // unless opening raises the menu.
    let cover = win.new_button("cover", 0, 28, 300, 300);
    assert_ne!(cover.raw_id(), 0);

    let fired: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&fired);
    with_widget_mut(file.raw_id(), |widget| {
        let menu = (widget as &mut dyn std::any::Any)
            .downcast_mut::<rust_widgets::Menu>()
            .expect("the id addresses the mounted menu");
        menu.triggered.connect(move |text: Arc<String>| {
            if let Ok(mut rows) = sink.lock() {
                rows.push((*text).clone());
            }
        });
    });

    assert!(file.open_at(0, 28, Rect::new(0, 0, 640, 480)));
    let geometry = rust_widgets::widget_geometry(file.raw_id()).expect("mounted menu geometry");

    // A bar-owned menu has no heading (the bar already shows the title), so the first row
    // starts at the popup's own top edge plus the body's padding. `Menu::draw` and the hit
    // path share that derivation.
    let row_point = Point::new(geometry.0 + 20, geometry.1 + 4);
    let handled = dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: row_point, button: 1, modifiers: 0 },
        row_point,
    );
    assert!(handled, "a press on the popup must be dispatched");

    assert_eq!(
        &*fired.lock().expect("sink poisoned"),
        &["Open".to_string()],
        "the click must activate the menu item, not the button created after it"
    );
    assert!(!file.is_visible(), "choosing an item closes the popup");

    // The choice must also reach the queue contract `poll_menu_triggered` documents, not
    // only the menu's own signal: a host that polls (the code-editor demo does) otherwise
    // sees a menu that looks dead on a self-drawn backend.
    assert_eq!(
        rust_widgets::poll_menu_triggered(),
        Some(item.raw_id()),
        "the chosen entry's id must be what poll_menu_triggered reports"
    );
}
