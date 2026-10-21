// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Interaction regressions found while exercising the demos with a real X11 window.
//!
//! Each case is a control whose model was correct but whose **pointer path** was not: the hit
//! test rejected a region the control paints, two popups could be open at once, or a scroll
//! offset moved the bar while leaving the content behind. They are grouped because the same two
//! rules answer all of them — a control owns everything it paints, and only one overlay is open.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, WidgetHandle};
use rust_widgets::core::{Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::runtime::dispatch_pointer_event;

#[test]
fn a_combo_box_row_is_clickable_even_when_a_later_control_sits_under_it() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("combo popup", 0, 0, 640, 480);

    let combo = win.new_combo_box(20, 20, 200, 26);
    combo.add_item("red");
    combo.add_item("green");
    combo.add_item("blue");
    combo.set_current_index(0);
    // Created after the combo, so sibling order alone would put it on top of the open list.
    let _cover = win.new_button("cover", 0, 40, 300, 200);

    let field = Point::new(120, 33);
    assert!(dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: field, button: 1, modifiers: 0 },
        field,
    ));
    assert!(combo.is_open(), "a press on the field opens the list");

    // The second row hangs below the field, over the button created after the combo.
    let row = rust_widgets::widget_geometry(combo.raw_id()).expect("combo geometry");
    let row_point = Point::new(row.0 + 30, row.1 + row.3 as i32 + 26 + 13);
    assert!(dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: row_point, button: 1, modifiers: 0 },
        row_point,
    ));

    assert_eq!(combo.current_index(), Some(1), "the row under the press must be taken");
    assert!(!combo.is_open(), "taking a row closes the list");
    assert_eq!(
        rect_of(combo.raw_id()).map(|r| r.width),
        Some(200),
        "the control itself must still be where it was drawn"
    );

    // Re-open and dismiss from a point outside both the field and the list: the popup owns the
    // dismissing press, so the control below must not receive it.
    assert!(dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: field, button: 1, modifiers: 0 },
        field,
    ));
    assert!(combo.is_open());
    let elsewhere = Point::new(500, 400);
    assert!(dispatch_pointer_event(
        win.raw_id(),
        &Event::MousePress { pos: elsewhere, button: 1, modifiers: 0 },
        elsewhere,
    ));
    assert!(!combo.is_open(), "an outside press must dismiss an open list");
}

#[test]
fn opening_another_menu_closes_the_previous_one_and_a_second_click_toggles_it() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("menu exclusivity", 0, 0, 640, 480);

    let bar = win.new_menu_bar(0, 0, 640, 28);
    let file = win.new_menu(&bar, "File", 0, 0, 200, 0);
    let edit = win.new_menu(&bar, "Edit", 0, 0, 200, 0);
    let viewport = Rect::new(0, 0, 640, 480);

    assert!(file.open_at(0, 28, viewport));
    assert!(file.is_visible());

    // Opening a second menu closes the first: a bar never shows two drop-downs.
    assert!(edit.open_at(64, 28, viewport));
    assert!(!file.is_visible(), "the previous menu must close");
    assert!(edit.is_visible());

    // The same title again toggles its menu closed, which is what a menu-bar title does.
    assert!(edit.open_at(64, 28, viewport));
    assert!(!edit.is_visible(), "a second click on the same title closes its menu");
}

#[test]
fn scrolling_moves_the_mounted_content_as_well_as_the_bar() {
    rust_widgets::init();
    let mut app = App::new();
    app.init();
    let win = app.new_window("scroll content", 0, 0, 640, 480);

    let area = win.new_scroll_area(20, 20, 240, 100);
    area.set_content_size(240, 400);
    // Rows mounted directly in the viewport, the shape the control demo uses.
    let first = rust_widgets::create_label(area.raw_id(), "row 0", 4, 4, 220, 18);
    let second = rust_widgets::create_label(area.raw_id(), "row 1", 4, 30, 220, 18);
    assert_ne!(first, 0);
    assert_ne!(second, 0);

    let before = rect_of(second).expect("the second row is mounted");
    assert!(
        rust_widgets::set_widget_scroll_position(area.raw_id(), 0, 20),
        "a scroll area accepts a scroll offset"
    );
    let after = rect_of(second).expect("the second row is still mounted");

    assert_eq!(after.y, before.y - 20, "the content must follow the scroll offset");
    assert_eq!(
        rust_widgets::widget_scroll_position(area.raw_id()),
        Some((0, 20)),
        "the model must report the offset that moved the content"
    );
}

fn rect_of(id: rust_widgets::core::ObjectId) -> Option<Rect> {
    rust_widgets::widget_geometry(id).map(|(x, y, width, height)| Rect::new(x, y, width, height))
}
