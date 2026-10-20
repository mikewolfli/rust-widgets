// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A panel's child layout must survive being positioned by a parent layout.
//!
//! # The defect this pins
//!
//! `apply_panel_layout` used to read the panel's rect from the mirrored `PANEL_STATES`
//! table, which only `PanelHandle::set_geometry` writes. The window layout positions its
//! children through `crate::set_widget_geometry`, which does **not** write that table — so
//! a panel created at `(0, 0, 0, 0)` and then sized by the window layout laid its children
//! out inside a zero-area rect and never recomputed. The children were created, registered
//! and reachable, but every one of them had zero width and height: the control demo's grid
//! was invisible.
//!
//! The assertions below are all about *observable geometry*, because "the control exists"
//! was already true and told us nothing.

// Same gate as the other window/`App`-level suites: the handle constructors this exercises
// are compiled only in a device profile.
#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Orientation, Rect};
use rust_widgets::layout::{BorderLayout, BorderRegion, BoxLayout, GridLayout, Layout};

/// Builds a window with a panel hosting a grid, plus the panel host in a vertical layout.
///
/// Returns `(grid_cell_ids, window)` so the caller can inspect the cells afterwards.
fn window_with_panel_hosted_grid() -> (Vec<u64>, App, WindowHandle) {
    let mut app = App::new();
    app.init();
    let win = app.new_window("panel layout", 0, 0, 800, 600);

    // The panel is created with a placeholder rect, exactly as a caller normally does.
    let panel = win.new_panel(0, 0, 0, 0);

    // Three labels inside the panel's own grid.
    let mut grid = GridLayout::new(1, 3, 4, 0);
    let mut cell_ids = Vec::new();
    for col in 0..3 {
        let label = win.new_label(&format!("cell {col}"), 0, 0, 0, 0);
        grid.set_widget(0, col, label.raw_id());
        cell_ids.push(label.raw_id());
    }
    panel.set_layout(Box::new(grid));

    // The window layout gives the panel its real box.
    let mut outer = BoxLayout::new(Orientation::Vertical, 0, 0);
    outer.add_widget(panel.raw_id(), 1);
    win.set_layout(outer);

    (cell_ids, app, win)
}

#[test]
fn a_panel_sized_by_the_window_layout_lays_out_its_children() {
    let (cell_ids, _app, _win) = window_with_panel_hosted_grid();

    for id in &cell_ids {
        let geometry = rust_widgets::widget::runtime::geometry_of(*id)
            .unwrap_or_else(|| panic!("cell {id} must be mounted"));
        assert!(
            geometry.width > 0 && geometry.height > 0,
            "cell {id} must be laid out with a real size, got {geometry:?}"
        );
        assert!(
            geometry.x >= 0 && geometry.y >= 0,
            "cell {id} must be inside the window, got {geometry:?}"
        );
    }

    // The three cells must be side by side and not overlap.
    let mut rects: Vec<Rect> = cell_ids
        .iter()
        .map(|id| rust_widgets::widget::runtime::geometry_of(*id).expect("mounted"))
        .collect();
    rects.sort_by_key(|rect| rect.x);
    for pair in rects.windows(2) {
        assert!(
            pair[0].x + pair[0].width as i32 <= pair[1].x,
            "cells overlap: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn resizing_the_window_re_lays_out_the_panels_children() {
    let (cell_ids, _app, win) = window_with_panel_hosted_grid();

    let before: Vec<u32> = cell_ids
        .iter()
        .map(|id| rust_widgets::widget::runtime::geometry_of(*id).unwrap().width)
        .collect();

    // A wider window must widen the panel and therefore its cells.
    win.set_geometry(0, 0, 1600, 600);

    let after: Vec<u32> = cell_ids
        .iter()
        .map(|id| rust_widgets::widget::runtime::geometry_of(*id).unwrap().width)
        .collect();

    assert!(
        after.iter().sum::<u32>() > before.iter().sum::<u32>(),
        "the cells must grow with the window: {before:?} -> {after:?}"
    );
    for (index, width) in after.iter().enumerate() {
        assert!(*width > 0, "cell {index} lost its width after the resize: {after:?}");
    }
}

/// A frame is a panel under another name, so it must host a layout the same way.
///
/// # Why this is a separate test
///
/// `WindowHandle::new_frame` also calls `create_panel`, but before this test the frame
/// handle had no `set_layout`/`set_geometry` of its own: a caller could create a frame and
/// children, and the frame would never lay them out or re-lay them after a move. A frame
/// that silently behaves differently from a panel is the same implicit-contract defect the
/// panel factories remove, one spelling over.
#[test]
fn a_frame_hosts_a_layout_and_re_lays_it_out_when_moved() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("frame layout", 0, 0, 800, 600);

    let frame = win.new_frame(0, 0, 0, 0);
    frame.set_title("Group");
    let label = frame.new_label("inside", 0, 0, 0, 0);

    let mut frame_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    frame_layout.add_widget(label.raw_id(), 1);
    frame.set_layout(Box::new(frame_layout));

    let mut window_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    window_layout.add_widget(frame.raw_id(), 1);
    win.set_layout(window_layout);

    assert_eq!(
        rust_widgets::widget::runtime::with_widget(label.raw_id(), |widget| widget.parent())
            .flatten(),
        Some(frame.raw_id()),
        "a frame factory must parent the child to the frame"
    );
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(label.raw_id()),
        Some(Rect::new(0, 0, 800, 600)),
        "the frame's layout must run after the window places it"
    );

    frame.set_geometry(0, 0, 400, 300);
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(label.raw_id()),
        Some(Rect::new(0, 0, 400, 300)),
        "moving the frame must re-run its child layout"
    );
    assert_eq!(frame.as_panel().raw_id(), frame.raw_id());
}

/// A declarative container nested in a panel must follow the panel when it moves.
///
/// # Why this is a separate registry test
///
/// JSON documents and the C ABI store their layouts in `layout::declarative`, while a
/// `PanelHandle` stores its layout in the app layer. Both are "a container with a layout",
/// and the nested-panel rule is about the container being placed, not about which store
/// owns its layout. Before the cross-registry hook, a JSON-loaded container placed by a
/// panel was moved and never asked to lay its own children out again.
#[test]
fn a_declarative_container_inside_a_panel_re_lays_out_when_the_panel_moves() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("declarative nested", 0, 0, 800, 600);

    let panel = win.new_panel(0, 0, 0, 0);
    let inner = panel.new_panel(0, 0, 0, 0);
    let child = inner.new_label("nested", 0, 0, 0, 0);

    // The document/C-ABI path registers its layout here.
    let mut inner_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    inner_layout.add_widget(child.raw_id(), 1);
    rust_widgets::json::store_layout(inner.raw_id(), Box::new(inner_layout));

    let mut panel_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    panel_layout.add_widget(inner.raw_id(), 1);
    panel.set_layout(Box::new(panel_layout));

    let mut window_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    window_layout.add_widget(panel.raw_id(), 1);
    win.set_layout(window_layout);

    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(child.raw_id()),
        Some(Rect::new(0, 0, 800, 600)),
        "the declarative child layout must run when the panel is first placed"
    );

    panel.set_geometry(0, 0, 400, 300);
    assert_eq!(
        rust_widgets::widget::runtime::geometry_of(child.raw_id()),
        Some(Rect::new(0, 0, 400, 300)),
        "moving the panel must re-run the nested declarative layout"
    );
    rust_widgets::json::forget_layout(inner.raw_id());
}

/// A panel's own factories must parent the child to the **panel**, and a panel that is
/// itself a layout child must have its layout re-run when the parent places it.
///
/// # Why this is a separate test from the placement one
///
/// The placement test used `WindowHandle::new_label`, which works through the documented
/// backwards-compatible path (a panel layout may position a control created by the
/// enclosing window). That path leaves the widget's parent pointing at the window, so it
/// says nothing about whether the panel factories exist or whether nesting recurses. This
/// test exercises the two facts a workbench layout needs: the child is created *for* the
/// panel, and a panel nested inside another panel is laid out after its parent sizes it.
#[test]
fn a_panel_factory_parents_its_children_and_nested_panels_re_layout() {
    let mut app = App::new();
    app.init();
    let win = app.new_window("panel factories", 0, 0, 800, 600);

    let panel = win.new_panel(0, 0, 0, 0);
    let header = panel.new_label("header", 0, 0, 0, 0);
    let body = panel.new_panel(0, 0, 0, 0);
    let body_text = body.new_label("body", 0, 0, 0, 0);

    let mut body_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    body_layout.add_widget(body_text.raw_id(), 1);
    body.set_layout(Box::new(body_layout));

    let mut panel_layout = BorderLayout::new(0, 0);
    panel_layout.add(BorderRegion::North, header.raw_id(), 30);
    panel_layout.add(BorderRegion::Center, body.raw_id(), 0);
    panel.set_layout(Box::new(panel_layout));

    let mut window_layout = BoxLayout::new(Orientation::Vertical, 0, 0);
    window_layout.add_widget(panel.raw_id(), 1);
    win.set_layout(window_layout);

    let header_rect =
        rust_widgets::widget::runtime::geometry_of(header.raw_id()).expect("the header is mounted");
    assert_eq!(header_rect, Rect::new(0, 0, 800, 30), "the north track is fixed");

    let parent_of =
        |id| rust_widgets::widget::runtime::with_widget(id, |widget| widget.parent()).flatten();
    assert_eq!(
        parent_of(header.raw_id()),
        Some(panel.raw_id()),
        "a panel factory must parent the child to the panel, not the window"
    );
    assert_eq!(parent_of(body.raw_id()), Some(panel.raw_id()));
    assert_eq!(
        parent_of(body_text.raw_id()),
        Some(body.raw_id()),
        "a nested panel must own its own child"
    );

    let body_rect =
        rust_widgets::widget::runtime::geometry_of(body.raw_id()).expect("the body is mounted");
    assert_eq!(body_rect, Rect::new(0, 30, 800, 570), "the center takes the rest");

    let text_rect = rust_widgets::widget::runtime::geometry_of(body_text.raw_id())
        .expect("the nested label is mounted");
    assert_eq!(
        text_rect,
        Rect::new(0, 30, 800, 570),
        "the nested panel's own layout must run after its parent places it"
    );
}
