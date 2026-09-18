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
use rust_widgets::layout::{BoxLayout, GridLayout, Layout};

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
