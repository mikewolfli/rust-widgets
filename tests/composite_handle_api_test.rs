// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The composite controls' handle layer must expose their content model.
//!
//! # What this pins
//!
//! `TabWidget`, `GridWidget`, `Splitter`, `Dialog` and `ScrollArea` are containers with a
//! content model that is **not** a generic `Layout`: tabs, matrix dimensions, panes, one
//! content child, one scrolling child. Their widget types already owned those operations,
//! but for several of them the public `Handle` had no method at all, so an application
//! built with `App`/`WindowHandle` could create the control and then do nothing with it.
//! These tests drive the handle API, not the widget API, so a missing delegation fails here.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, TabPosition, TabShape, WidgetHandle};
use rust_widgets::core::{Alignment, Orientation};

#[test]
fn a_tab_handle_owns_its_tabs_and_selection() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("tabs", 0, 0, 800, 600);

    let first_page = win.new_label("first", 0, 0, 0, 0);
    let second_page = win.new_label("second", 0, 0, 0, 0);
    let tabs = win.new_tab_widget(0, 0, 400, 300);

    // The shared `create_tab_widget` constructor seeds two tabs so a freshly created strip
    // has a body (`check_control_feature_visible_in_own_snapshot.py` pins that). The handle
    // API starts from that state rather than pretending the control is empty.
    assert_eq!(tabs.tab_count(), 2, "the constructor seeds two placeholder tabs");
    assert_eq!(tabs.add_tab("One", Some(first_page.raw_id())), Some(2));
    assert_eq!(tabs.add_tab("Two", Some(second_page.raw_id())), Some(3));
    assert_eq!(tabs.tab_count(), 4);
    assert_eq!(tabs.current_index(), Some(0));

    // Selecting a tab shows *its* page and hides the others, and the selected page occupies
    // the content box below the 24px strip. Both halves are the control's content model: a
    // page that is linked but never placed paints at its construction rect, and a page that
    // is never hidden paints over whichever tab is selected.
    assert!(tabs.set_current_index(2));
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(first_page.raw_id()),
        Some(rust_widgets::core::Rect::new(0, 24, 400, 276)),
        "the selected page fills the content box"
    );
    assert!(rust_widgets::is_widget_visible(first_page.raw_id()));
    assert!(
        !rust_widgets::is_widget_visible(second_page.raw_id()),
        "a page that is not selected must not paint over the selected one"
    );

    assert!(tabs.set_current_index(1));
    assert_eq!(tabs.current_index(), Some(1));
    assert!(!tabs.set_current_index(9), "an out-of-range index must be refused");

    assert!(tabs.set_tab_text(0, "Renamed"));
    assert!(tabs.set_closable(true));
    assert!(tabs.set_movable(true));
    assert!(tabs.set_tab_position(TabPosition::South));
    assert!(tabs.set_tab_shape(TabShape::Triangular));

    // A south strip leaves the content above it: moving the strip must move the page with it.
    assert!(tabs.set_current_index(2));
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(first_page.raw_id()),
        Some(rust_widgets::core::Rect::new(0, 0, 400, 276)),
        "the selected page follows a move of the tab strip"
    );

    assert!(tabs.remove_tab(3));
    assert_eq!(tabs.tab_count(), 3);
    assert!(!tabs.remove_tab(9), "removing past the end must be refused");
}

#[test]
fn a_grid_widget_handle_shapes_the_visual_matrix() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("grid", 0, 0, 800, 600);
    let grid = win.new_grid_widget(0, 0, 200, 200);

    assert_eq!(grid.rows(), Some(1));
    assert_eq!(grid.columns(), Some(1));
    assert_eq!(grid.spacing(), Some(0));

    assert!(grid.set_rows(3));
    assert!(grid.set_columns(4));
    assert!(grid.set_spacing(6));
    assert_eq!(grid.rows(), Some(3));
    assert_eq!(grid.columns(), Some(4));
    assert_eq!(grid.spacing(), Some(6));

    // `set_rows(0)` clamps to the control's minimum rather than producing a zero-row grid.
    assert!(grid.set_rows(0));
    assert_eq!(grid.rows(), Some(1));
}

#[test]
fn a_splitter_handle_owns_its_panes_and_ratios() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("splitter", 0, 0, 800, 600);
    let left = win.new_label("left", 0, 0, 0, 0);
    let right = win.new_label("right", 0, 0, 0, 0);
    let splitter = win.new_splitter(0, 0, 400, 300);

    assert_eq!(splitter.pane_count(), 0);
    assert_eq!(splitter.add_pane(left.raw_id(), 1), Some(0));
    assert_eq!(splitter.add_pane(right.raw_id(), 2), Some(1));
    assert_eq!(splitter.pane_count(), 2);
    assert_eq!(splitter.orientation(), Some(Orientation::Horizontal));

    assert!(splitter.set_ratio(0, 0.25));
    assert_eq!(splitter.ratio(0), Some(0.25));
    assert!(!splitter.set_ratio(9, 0.5), "an unknown pane must be refused");

    assert!(splitter.set_orientation(Orientation::Vertical));
    assert_eq!(splitter.orientation(), Some(Orientation::Vertical));
    assert!(splitter.remove_pane(left.raw_id()));
    assert_eq!(splitter.pane_count(), 1);
}

#[test]
fn a_dialog_handle_owns_content_and_lifecycle() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("dialog", 0, 0, 800, 600);
    let content = win.new_label("body", 0, 0, 0, 0);
    let dialog = win.new_dialog("Title", 100, 100, 400, 300);

    assert!(dialog.set_title("New title"));
    assert!(dialog.set_modal(false));
    assert!(dialog.set_content_widget(Some(content.raw_id())));
    assert_eq!(dialog.content_widget(), Some(content.raw_id()));
    assert!(dialog.content_rect().is_some(), "a dialog places its content");
    assert!(!dialog.set_content_widget(Some(0xDEAD_BEEF)), "an unmounted child must be refused");

    assert!(dialog.open());
    assert!(dialog.close());
    assert!(dialog.set_content_widget(None));
    assert_eq!(dialog.content_widget(), None);
}

#[test]
fn a_dialog_handle_places_the_content_it_owns() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("dialog placement", 0, 0, 800, 600);
    let content = win.new_label("body", 0, 0, 0, 0);
    let dialog = win.new_dialog("Title", 40, 40, 400, 300);

    assert!(dialog.set_content_widget(Some(content.raw_id())));
    let placed = rust_widgets::widget::runtime::geometry_of(content.raw_id())
        .expect("the content is mounted");
    assert_eq!(
        Some(placed),
        dialog.content_rect(),
        "the child must occupy the rectangle the dialog calls its content box"
    );

    // A layout positions the dialog through the same geometry entry point, so moving the
    // dialog must move the content with it rather than leaving it behind.
    dialog.set_geometry(100, 80, 500, 360);
    let moved = dialog.content_rect().expect("a dialog answers its content box");
    assert_ne!(placed, moved, "a new dialog rectangle implies a new content box");
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(content.raw_id()),
        Some(moved),
        "the content follows the dialog after a move"
    );
}

#[test]
fn a_scroll_area_handle_content_size_reaches_the_control() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("scroll", 0, 0, 800, 600);
    let content = win.new_label("long content", 0, 0, 0, 0);
    let area = win.new_scroll_area(0, 0, 400, 300);

    assert!(area.set_content_widget(Some(content.raw_id())));
    assert_eq!(area.content_widget(), Some(content.raw_id()));
    assert!(!area.set_content_widget(Some(0xDEAD_BEEF)));

    area.set_content_size(1000, 1000);
    assert_eq!(area.content_size(), (1000, 1000));

    area.scroll_to_bottom();
    let (_, y) = area.scroll_position();
    assert_eq!(
        y, 700,
        "the control clamps the requested 1000 against a 300px viewport; the handle must report \
         the accepted offset, not the request"
    );
}

#[test]
fn a_scroll_area_places_its_content_and_moves_it_with_the_scroll() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("scroll placement", 0, 0, 800, 600);
    let content = win.new_label("content", 0, 0, 0, 0);
    let area = win.new_scroll_area(10, 20, 200, 150);

    assert!(area.set_content_widget(Some(content.raw_id())));
    area.set_content_size(1000, 1000);
    // Both bars are owed (1000 > 200 and 1000 > 150), so the content box is the control
    // minus the two 8px bands.
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(content.raw_id()),
        Some(rust_widgets::core::Rect::new(10, 20, 192, 142)),
        "the content fills the viewport, not the control's whole rectangle"
    );

    area.set_scroll_position(30, 40);
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(content.raw_id()),
        Some(rust_widgets::core::Rect::new(-20, -20, 192, 142)),
        "the scroll offset is part of the content geometry, not a paint-time shift"
    );

    // The content is a tree child, so a press inside the viewport reaches it directly — and
    // the clip the scroll area reports keeps its painting inside the same box.
    assert_eq!(
        rust_widgets::widget::runtime::widget_at(
            win.raw_id(),
            rust_widgets::core::Point::new(20, 30)
        ),
        Some(content.raw_id())
    );
}

#[test]
fn a_scrolled_panel_does_not_paint_outside_the_viewport() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("scroll clip", 0, 0, 800, 600);
    let panel = win.new_panel(0, 0, 0, 0);
    let area = win.new_scroll_area(10, 20, 200, 150);

    assert!(area.set_content_widget(Some(panel.raw_id())));
    area.set_content_size(1000, 1000);

    // A fixed, non-theme-derived fill: the panel's face is then unambiguously red, so a
    // pixel of that colour outside the viewport can only have come from a missing clip.
    let style =
        rust_widgets::style::WidgetStyle::default().with_background(rust_widgets::core::Color::RED);
    rust_widgets::widget::runtime::with_widget_mut(panel.raw_id(), |widget| {
        widget.base_mut().set_style(style);
    })
    .expect("the panel is mounted");

    // Scroll it up by 40: the panel's rectangle now starts 20px above the window and would
    // paint over the area above the viewport if the tree painter ignored the clip.
    area.set_scroll_position(0, 40);

    let frame = rust_widgets::widget::runtime::render_frame_tree(
        win.raw_id(),
        rust_widgets::core::Size::new(800, 600),
        rust_widgets::core::Color::WHITE,
    )
    .expect("the window tree paints a frame");
    let pixel = |x: usize, y: usize| {
        let index = (y * 800 + x) * 4;
        (frame[index], frame[index + 1], frame[index + 2])
    };

    assert_eq!(pixel(20, 30), (255, 0, 0), "inside the viewport the panel paints");
    assert_ne!(
        pixel(20, 5),
        (255, 0, 0),
        "above the viewport the scrolled panel must be clipped away"
    );
    assert_ne!(pixel(205, 30), (255, 0, 0), "and it must not paint over the vertical bar's band");
}

#[test]
fn a_splitter_places_and_hosts_its_panes() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("splitter placement", 0, 0, 800, 600);
    let left = win.new_label("left", 0, 0, 0, 0);
    let right = win.new_label("right", 0, 0, 0, 0);
    let splitter = win.new_splitter(10, 20, 400, 300);

    assert_eq!(splitter.add_pane(left.raw_id(), 1), Some(0));
    assert_eq!(splitter.add_pane(right.raw_id(), 1), Some(1));

    let left_rect = rust_widgets::widget::runtime::geometry_of(left.raw_id())
        .expect("the left pane is mounted");
    let right_rect = rust_widgets::widget::runtime::geometry_of(right.raw_id())
        .expect("the right pane is mounted");
    assert_eq!((left_rect.x, left_rect.y), (10, 20), "a pane starts at the splitter origin");
    assert_eq!(left_rect.height, 300, "and spans its track");
    assert_eq!(
        right_rect.x - (left_rect.x + left_rect.width as i32),
        5,
        "the layout reserves the divider's grab area between panes"
    );
    assert_eq!(left_rect.width + right_rect.width + 5, 400, "panes and divider tile the width");

    // The panes are children now, so the hit test reaches one directly; the divider gap is
    // not inside any pane and therefore resolves to the splitter, which is what lets a drag
    // on the visible handle begin.
    assert_eq!(
        rust_widgets::widget::runtime::widget_at(
            win.raw_id(),
            rust_widgets::core::Point::new(20, 40)
        ),
        Some(left.raw_id())
    );
    let divider = left_rect.x + left_rect.width as i32 + 2;
    assert_eq!(
        rust_widgets::widget::runtime::widget_at(
            win.raw_id(),
            rust_widgets::core::Point::new(divider, 40)
        ),
        Some(splitter.raw_id()),
        "a press on the divider must land on the splitter, not on a pane"
    );

    // A layout positions the splitter through its geometry, so the panes follow it.
    splitter.set_geometry(50, 60, 400, 300);
    let moved = rust_widgets::widget::runtime::geometry_of(left.raw_id())
        .expect("the left pane is mounted");
    assert_eq!((moved.x, moved.y), (50, 60), "panes move with the splitter");
}

#[test]
fn panel_and_frame_expose_the_group_box_properties() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("group", 0, 0, 800, 600);

    let panel = win.new_panel(0, 0, 200, 100);
    assert!(panel.set_checkable(true));
    assert_eq!(panel.is_checkable(), Some(true));
    assert_eq!(panel.checked(), Some(true), "a fresh group box starts checked");
    assert!(panel.toggle());
    assert_eq!(panel.checked(), Some(false));
    assert!(panel.set_alignment(Alignment::Center));
    assert_eq!(panel.alignment(), Some(Alignment::Center));

    let frame = win.new_group_box("Group title", 0, 200, 200, 100);
    assert_eq!(frame.text(), "Group title");
    assert!(frame.set_checkable(true));
    assert_eq!(frame.is_checkable(), Some(true));
    assert!(frame.set_alignment(Alignment::Right));
    assert_eq!(frame.alignment(), Some(Alignment::Right));
    assert!(frame.toggle());
    assert_eq!(frame.checked(), Some(false));
}
