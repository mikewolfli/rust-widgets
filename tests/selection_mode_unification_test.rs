// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Regression tests for the unified `SelectionMode`.
//!
//! Three structurally similar enums used to exist: `app::SelectionMode` (with a
//! `None` variant), `view_widgets::list_view::SelectionMode` (without one) and
//! `input_widgets::listbox::SelectionMode` (same four states under different
//! variant names — `NoSelection`/`SingleSelection`/…). They are now one type that
//! the other two paths re-export, so a mode read from a `ListBox`, a `ListView`
//! or a handle is interchangeable.
//!
//! These tests pin that identity from the consumer side: if the re-exports were
//! split back into separate types, the assignments below would stop compiling.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

/// All three public paths must name the *same* type. This is a compile-time
/// assertion: distinct types would make these assignments ill-typed.
#[test]
fn all_selection_mode_paths_are_one_type() {
    use rust_widgets::app::SelectionMode as AppMode;
    use rust_widgets::widget::input_widgets::listbox::SelectionMode as ListBoxMode;
    use rust_widgets::widget::view_widgets::list_view::SelectionMode as ListViewMode;

    let from_app: AppMode = AppMode::Extended;
    let from_list_view: ListViewMode = from_app;
    let from_list_box: ListBoxMode = from_list_view;
    assert_eq!(from_list_box, AppMode::Extended);

    // And the reverse direction, so the re-export chain is not one-way.
    let back: AppMode = ListBoxMode::None;
    assert_eq!(back, ListViewMode::None);
}

/// The canonical four modes must all exist and stay distinct — including `None`,
/// which is what the list-view copy was missing.
#[test]
fn selection_mode_has_all_four_distinct_states() {
    use rust_widgets::widget::input_widgets::listbox::SelectionMode;

    let all =
        [SelectionMode::Single, SelectionMode::Multi, SelectionMode::Extended, SelectionMode::None];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b, "modes {i} and {j} must be distinct");
            }
        }
    }
    assert_eq!(SelectionMode::default(), SelectionMode::Single);
}

/// `None` must mean "this view accepts no selection", not merely "nothing is
/// selected": a `ListBox` in `None` mode must refuse `select`.
#[test]
fn list_box_in_none_mode_refuses_selection() {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::input_widgets::listbox::{ListBox, SelectionMode};

    let mut lb = ListBox::new(Rect::new(0, 0, 200, 200));
    lb.add_item("a".to_string());
    lb.add_item("b".to_string());

    lb.set_selection_mode(SelectionMode::None);
    lb.select(0);
    assert!(!lb.is_selected(0), "a non-selectable list box must not select");
    assert!(lb.selected_indices().is_empty());

    // Switching to a selectable mode restores normal behaviour.
    lb.set_selection_mode(SelectionMode::Single);
    lb.select(1);
    assert!(lb.is_selected(1));
}

/// The selection model backing the view widgets must honour the same `None`
/// semantics, so the shared enum cannot mean two different things per widget.
#[test]
fn selection_model_honours_none_mode() {
    use rust_widgets::widget::view_widgets::list_view::{SelectionMode, SelectionModel};

    let mut model = SelectionModel::new();
    model.set_mode(SelectionMode::None);
    assert!(!model.is_selectable());

    model.select_row(3);
    assert!(model.rows().is_empty(), "select_row must be a no-op in None mode");
    assert_eq!(model.current_row(), None);
}

/// The `ListView` *producer* must honour `None` too: a non-selectable view
/// reports no `selection_changed` and no success, but a click still focuses the
/// row — focus and selection are separate concerns.
#[test]
fn list_view_in_none_mode_reports_no_selection_change() {
    use rust_widgets::core::Rect;
    use rust_widgets::widget::view_widgets::list_view::{ListView, SelectionMode, VecListModel};
    use std::sync::{Arc, Mutex};

    let mut view = ListView::new(Rect::new(0, 0, 200, 200));
    view.set_model(Arc::new(VecListModel::new((0..4).map(|n| n.to_string()).collect())));
    view.set_selection_mode(SelectionMode::None);

    let emitted = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = Arc::clone(&emitted);
    view.selection_changed.connect(move |row| sink.lock().unwrap().push(*row));

    assert!(!view.select_row(1), "a non-selectable view must report no selection success");
    assert!(view.selected_rows().is_empty(), "no rows may be selected under None");
    assert_eq!(view.focused_row(), Some(1), "but the click still focuses the row");
    assert!(emitted.lock().unwrap().is_empty(), "no selection_changed may fire under None mode");
}
