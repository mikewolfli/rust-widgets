// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TableWidget tests.

// The module-level gate, so tooling that scans file text recognises this as a test module. The
// parent declares `mod tests;` under `#[cfg(test)]`, which the compiler sees but a text scan of
// this file does not — and a scan that cannot tell a test module from production code reports the
// test delegate's `size_hint` as a hand-rolled hint (`tools/check_implicit_size_uses_metrics.sh`).
// Same marker as `widget/capability/tests.rs` and `special_widgets/toast/tests.rs`.
#![cfg(test)]

use super::*;
use crate::core::Point;
use crate::event::types::EventHandler;
use crate::event::Event;
use std::sync::Arc;
use std::sync::Mutex;

/// A multi-row `Extended` selection survives a model change.
///
/// # What this pins
///
/// `normalize_projection_state` re-added the surviving rows one `select_row` at a time, and in
/// `Extended` mode that call *replaces* the selection: every model/source change collapsed the
/// selection to the last surviving row without announcing the loss.
#[test]
fn an_extended_selection_survives_a_model_change() {
    struct FiveRows;
    impl TableModel for FiveRows {
        fn row_count(&self) -> usize {
            5
        }
        fn column_count(&self) -> usize {
            1
        }
        fn data(&self, row: usize, _column: usize) -> Option<String> {
            (row < 5).then(|| "x".to_string())
        }
    }

    let mut table = TableWidget::new(Rect::new(0, 0, 200, 200));
    table.set_selection_mode(crate::widget::input_widgets::listbox::SelectionMode::Extended);
    table.set_model(Arc::new(FiveRows));
    let ctrl = crate::shortcut::Modifiers::CTRL;
    table.select_row_with_modifiers(1, ctrl);
    table.select_row_with_modifiers(2, ctrl);
    table.select_row_with_modifiers(3, ctrl);
    assert_eq!(table.selected_rows(), vec![1, 2, 3]);

    // Re-binding the same model runs the normalisation pass again.
    table.set_model(Arc::new(FiveRows));
    assert_eq!(
        table.selected_rows(),
        vec![1, 2, 3],
        "re-validating the rows must restore the whole set, not just the last row"
    );
}

struct StaticTableModel;

impl TableModel for StaticTableModel {
    fn row_count(&self) -> usize {
        2
    }

    fn column_count(&self) -> usize {
        2
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        match (row, column) {
            (0, 0) => Some("r0c0".to_string()),
            (0, 1) => Some("r0c1".to_string()),
            (1, 0) => Some("r1c0".to_string()),
            (1, 1) => Some("r1c1".to_string()),
            _ => None,
        }
    }
}

struct NoopDelegate;

impl ItemDelegate for NoopDelegate {
    fn create_editor(
        &self,
        _parent: &mut BaseWidget,
        _row: usize,
        _column: usize,
    ) -> Option<Box<dyn Widget>> {
        None
    }

    fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

    fn get_editor_data(&self, _editor: &dyn Widget, _row: usize, _column: usize) -> Option<String> {
        None
    }
}

#[test]
fn table_widget_model_delegate_and_size_accessors() {
    let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));

    assert!(!table.has_model());
    assert!(table.model_ref().is_none());
    assert!(!table.has_delegate());
    assert!(table.delegate_ref().is_none());

    table.set_model(Arc::new(StaticTableModel));
    assert!(table.has_model());
    assert!(table.model_ref().is_some());
    assert_eq!(table.item(99, 0), None);

    table.set_column_width(2, 140);
    table.set_row_height(1, 28);
    assert_eq!(table.column_width(2), Some(140));
    assert_eq!(table.row_height(1), Some(28));
    assert_eq!(table.column_width(999), None);
    assert_eq!(table.row_height(999), None);

    table.set_delegate(Arc::new(NoopDelegate));
    assert!(table.has_delegate());
    assert!(table.delegate_ref().is_some());
}

// ── Tests moved from deprecated `TableView` alias ────────────────

struct TestTableModel {
    rows: usize,
    cols: usize,
    data_accessed: std::sync::atomic::AtomicBool,
}

impl TestTableModel {
    fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, data_accessed: std::sync::atomic::AtomicBool::new(false) }
    }
}

impl TableModel for TestTableModel {
    fn row_count(&self) -> usize {
        self.rows
    }

    fn column_count(&self) -> usize {
        self.cols
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.data_accessed.store(true, std::sync::atomic::Ordering::SeqCst);
        Some(format!("r{}c{}", row, column))
    }
}

/// A menu request selects the row under the pointer before it is emitted (C-9).
#[test]
fn a_menu_request_selects_the_row_under_the_pointer() {
    let mut table = TableWidget::new(Rect::new(0, 0, 400, 200));
    table.set_model(Arc::new(TestTableModel::new(4, 3)));
    let seen = Arc::new(std::sync::Mutex::new(None));
    let sink = Arc::clone(&seen);
    table.context_menu_requested.connect(move |pos| {
        if let Ok(mut slot) = sink.lock() {
            *slot = Some(*pos);
        }
    });

    let row = table.row_rect(1).expect("row 1 is visible");
    let point = crate::core::Point::new(row.x + 2, row.y + 2);
    assert_eq!(table.row_index_at(point), Some(1));
    assert_eq!(table.selected_row(), None);
    assert!(Widget::context_menu_event(&mut table, point));
    assert_eq!(table.selected_row(), Some(1));
    assert_eq!(*seen.lock().expect("the test slot locks"), Some(point));
}

#[test]
fn test_default_creation() {
    let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    assert_eq!(tv.kind(), WidgetKind::Table);
    assert_eq!(tv.geometry(), Rect::new(0, 0, 400, 300));
    assert!(!tv.has_model());
    assert!(tv.model_ref().is_none());
    assert_eq!(tv.row_count(), 0);
    assert_eq!(tv.column_count(), 0);
    assert!(tv.selected_row().is_none());
    assert!(tv.selected_rows().is_empty());
    assert!(tv.focused_row().is_none());
    assert!(!tv.has_delegate());
    assert!(tv.delegate_ref().is_none());
    assert!(tv.is_visible());
    assert!(tv.is_enabled());
}

#[test]
fn test_set_columns() {
    let model = Arc::new(TestTableModel::new(3, 5));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    tv.set_model(model);

    assert!(tv.has_model());
    assert_eq!(tv.column_count(), 5);

    tv.set_column_width(0, 120);
    assert_eq!(tv.column_width(0), Some(120));
    assert_eq!(tv.column_width(99), None);
}

/// A column width the caller set actually **narrows the neighbours**.
///
/// # The defect this pins
///
/// `column_widths` was stored, published, and never read by the layout: the draw computed an
/// equal share per column and multiplied it by the index, so a wide column **overlapped** its
/// neighbour instead of pushing it. `set_column_width(0, 180)` on a three-column table therefore
/// drew column 1 at the same x as before, on top of column 0's last 60 pixels.
///
/// The assertion is on the **composited geometry**: column 1 must start where column 0 ends, and
/// the table without an override must still share equally -- the second half is what makes this
/// about the override rather than about any layout change.
#[test]
fn a_column_width_the_caller_set_pushes_its_neighbours() {
    use crate::widget::svg::render_to_svg;
    let _theme_guard = crate::style::theme_test_guard();

    let build = |width: Option<u32>| {
        let model = Arc::new(TestTableModel::new(3, 3));
        let mut tv = TableWidget::new(Rect::new(0, 0, 400, 200));
        tv.set_model(model);
        if let Some(width) = width {
            tv.set_column_width(0, width);
        }
        tv
    };

    // With no override the three columns share equally, which is the pre-existing layout.
    let plain = build(None);
    let content = plain.rows_band();
    assert_eq!(
        plain.column_offset(1, content.width),
        plain.resolved_column_width(0, content.width),
        "an even share still tiles"
    );

    // With an override the first column takes exactly what it asked for, and the second starts
    // where it ends -- the property the multiplication could not express.
    let wide = build(Some(180));
    assert_eq!(wide.resolved_column_width(0, content.width), 180, "the request is honoured");
    assert_eq!(
        wide.column_offset(1, content.width),
        180,
        "and column 1 begins where column 0 ends, rather than overlapping it"
    );
    assert_eq!(
        wide.column_offset(2, content.width),
        180 + wide.resolved_column_width(1, content.width),
        "the offsets accumulate"
    );

    // And it is visible: the two tables must not paint the same cells.
    let mut plain = plain;
    let mut wide = wide;
    assert_ne!(
        render_to_svg(&mut plain),
        render_to_svg(&mut wide),
        "a column width that is read must move the grid"
    );
}

/// A row height the caller set moves every row below it.
///
/// The same defect as the column widths, on the other axis: `row_heights` was stored and never
/// consulted, so `set_row_height(0, 40)` drew row 0 at 20 px and then overlapped it with row 1.
/// The click must follow the paint, which is why this asserts the hit test too.
#[test]
fn a_row_height_the_caller_set_moves_the_rows_below_it() {
    let model = Arc::new(TestTableModel::new(6, 2));
    let mut tv = TableWidget::new(Rect::new(0, 0, 300, 300));
    tv.set_model(model);

    // No override: the rows tile at the default height, which is what the arithmetic path gives.
    let first = tv.row_rect(0).expect("row 0 is visible");
    let second = tv.row_rect(1).expect("row 1 is visible");
    assert_eq!(second.y, first.y + TABLE_ROW_HEIGHT as i32, "the default rows tile");

    // Overriding row 0's height pushes row 1 down by exactly that height.
    tv.set_row_height(0, 44);
    let tall = tv.row_rect(0).expect("row 0 is still visible");
    let pushed = tv.row_rect(1).expect("row 1 is still visible");
    assert_eq!(tall.height, 44, "the request is honoured");
    assert_eq!(pushed.y, tall.y + 44, "row 1 begins where row 0 ends");

    // And the hit test agrees with the paint: a point in the middle of row 1 selects row 1, not
    // whichever row the default-height division would have named.
    let probe = crate::core::Point::new(pushed.x + 2, pushed.y + pushed.height as i32 / 2);
    assert_eq!(
        tv.row_at_point(probe),
        Some(1),
        "the row under the pointer is the row that was drawn there"
    );
}

#[test]
fn test_rows_via_model() {
    let model = Arc::new(TestTableModel::new(4, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    assert_eq!(tv.row_count(), 0);

    tv.set_model(model);
    assert_eq!(tv.row_count(), 4);
}

#[test]
fn test_cell_values() {
    let model = Arc::new(TestTableModel::new(2, 2));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    tv.set_model(model);

    assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));
    assert_eq!(tv.item(0, 1), Some("r0c1".to_string()));
    assert_eq!(tv.item(1, 0), Some("r1c0".to_string()));
    assert_eq!(tv.item(1, 1), Some("r1c1".to_string()));
}

#[test]
fn test_cell_values_out_of_bounds() {
    let model = Arc::new(TestTableModel::new(2, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    tv.set_model(model);

    assert_eq!(tv.item(5, 0), Some("r5c0".to_string()));
    assert_eq!(tv.item(0, 10), Some("r0c10".to_string()));
    assert_eq!(tv.item(0, 0), Some("r0c0".to_string()));

    assert_eq!(TableWidget::new(Rect::new(0, 0, 100, 100)).item(0, 0), None);
}

#[test]
fn test_column_and_row_counts() {
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    assert_eq!(tv.row_count(), 0);
    assert_eq!(tv.column_count(), 0);

    let model = Arc::new(TestTableModel::new(10, 4));
    tv.set_model(model);

    assert_eq!(tv.row_count(), 10);
    assert_eq!(tv.column_count(), 4);
}

#[test]
fn test_selection_mode() {
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    assert_eq!(tv.selection_mode(), crate::widget::view_widgets::list_view::SelectionMode::Single);

    tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Multi);
    assert_eq!(tv.selection_mode(), crate::widget::view_widgets::list_view::SelectionMode::Multi);

    tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Extended);
    assert_eq!(
        tv.selection_mode(),
        crate::widget::view_widgets::list_view::SelectionMode::Extended
    );

    tv.set_selection_mode(crate::widget::view_widgets::list_view::SelectionMode::Single);
    assert_eq!(tv.selection_mode(), crate::widget::view_widgets::list_view::SelectionMode::Single);
}

#[test]
fn test_select_and_clear_row() {
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    assert!(tv.selected_row().is_none());
    assert!(tv.selected_rows().is_empty());
    assert!(tv.focused_row().is_none());

    assert!(tv.select_row(2));
    assert_eq!(tv.selected_row(), Some(2));
    assert_eq!(tv.focused_row(), Some(2));
    assert_eq!(tv.selected_rows(), vec![2]);

    assert!(!tv.select_row(99));
    assert_eq!(tv.selected_row(), Some(2));

    tv.clear_selection();
    assert!(tv.selected_row().is_none());
    assert!(tv.selected_rows().is_empty());
}

#[test]
fn test_focused_row() {
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    assert!(tv.focused_row().is_none());

    assert!(tv.set_focused_row(3));
    assert_eq!(tv.focused_row(), Some(3));

    assert!(tv.set_focused_row(3));
    assert_eq!(tv.focused_row(), Some(3));

    assert!(!tv.set_focused_row(99));
    assert_eq!(tv.focused_row(), Some(3));

    tv.clear_focused_row();
    assert!(tv.focused_row().is_none());

    tv.clear_focused_row();
    assert!(tv.focused_row().is_none());
}

#[test]
fn test_selection_changed_signal() {
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    let captured = Arc::new(Mutex::new(None::<usize>));
    tv.selection_changed.connect({
        let captured = Arc::clone(&captured);
        move |val: Arc<usize>| {
            *captured.lock().unwrap() = Some(*val);
        }
    });

    tv.select_row(1);
    assert_eq!(*captured.lock().unwrap(), Some(1));

    tv.select_row(3);
    assert_eq!(*captured.lock().unwrap(), Some(3));
}

#[test]
fn test_focused_row_changed_signal() {
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    let captured = Arc::new(Mutex::new(None));
    tv.focused_row_changed.connect({
        let captured = Arc::clone(&captured);
        move |val: Arc<Option<usize>>| {
            *captured.lock().unwrap() = *val;
        }
    });

    tv.set_focused_row(2);
    assert_eq!(*captured.lock().unwrap(), Some(2));

    tv.clear_focused_row();
    assert_eq!(*captured.lock().unwrap(), None);
}

#[test]
fn test_geometry_delegation() {
    let mut tv = TableWidget::new(Rect::new(10, 20, 400, 300));

    assert_eq!(tv.geometry(), Rect::new(10, 20, 400, 300));

    tv.set_geometry(Rect::new(0, 0, 500, 400));
    assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
    assert_eq!(tv.geometry(), Rect::new(0, 0, 500, 400));
    assert_eq!(tv.position(), Point::new(0, 0));
    assert_eq!(tv.size(), crate::core::Size::new(500, 400));
}

#[test]
fn test_widget_id_and_kind() {
    let tv = TableWidget::new(Rect::new(0, 0, 400, 300));

    assert_eq!(tv.kind(), WidgetKind::Table);
    assert_ne!(tv.id(), 0);

    let tv2 = TableWidget::new(Rect::new(0, 0, 200, 100));
    assert_ne!(tv.id(), tv2.id());
}

#[test]
fn test_svg_output() {
    let mut tv = TableWidget::new(Rect::new(0, 0, 200, 100));

    // Without model
    let svg = crate::widget::svg::render_to_svg(&mut tv);
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("width=\"200\""));
    assert!(svg.contains("height=\"100\""));

    // With model
    let model = Arc::new(TestTableModel::new(2, 3));
    let mut tv2 = TableWidget::new(Rect::new(0, 0, 300, 150));
    tv2.set_model(model);
    let svg2 = crate::widget::svg::render_to_svg(&mut tv2);
    assert!(svg2.starts_with("<svg"));
}

#[test]
fn test_disabled_state_blocks_events() {
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    tv.set_enabled(false);
    assert!(!tv.is_enabled());

    let captured = Arc::new(Mutex::new(None::<usize>));
    tv.selection_changed.connect({
        let captured = Arc::clone(&captured);
        move |val: Arc<usize>| {
            *captured.lock().unwrap() = Some(*val);
        }
    });

    // Mouse press inside widget should not trigger selection when disabled.
    //
    // The point is on the **first content row**, which is what makes this a press "inside the
    // widget" that a table is expected to act on. It used to be `(10, 15)` — inside the
    // *header* band under the corrected geometry — and it selected row 0 only because the press
    // arm measured rows from the control's top edge instead of from below the header. Fixing
    // that derivation is what turned this into a real pointer on a real row.
    let on_first_row = crate::core::Point::new(10, 30);
    tv.handle_event(&crate::event::Event::MousePress {
        pos: on_first_row,
        button: 1,
        modifiers: 0,
    });
    assert!(captured.lock().unwrap().is_none());

    // Re-enable and verify it works
    tv.set_enabled(true);
    tv.handle_event(&crate::event::Event::MousePress {
        pos: on_first_row,
        button: 1,
        modifiers: 0,
    });
    assert_eq!(*captured.lock().unwrap(), Some(0));
}

/// A press selects the row the paint loop draws at that point, and a hover highlights it.
///
/// # The defect this pins
///
/// The paint loop measured rows from the content box (below the header) and the press arm
/// measured from the control's top edge, with `20` written out twice. A click therefore selected
/// the row **above** the one under the pointer — the exact defect `ListView::row_rect` was
/// extracted to fix, still present in this sibling. Both directions now read `row_rect`.
///
/// # What makes this a real assertion
///
/// It is written as the **inverse pair**: the point taken from `row_rect`'s midpoint must
/// resolve, through `row_at_point`, back to the same index — and the drawn row's fill at that
/// point must be the one that changed. A click that selects "some row" would pass the first half
/// and fail the second.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn a_click_selects_the_row_that_is_painted_there() {
    // Same guard and pin as the hover test: both reads of a row's fill have to come from one
    // appearance for the comparison to be about selection.
    let _guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
    let model = Arc::new(TestTableModel::new(5, 3));
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    tv.set_model(model);

    // Row 1, not row 0: row 0 is the one a top-edge-relative hit test would also land on for a
    // point near the header, so it cannot distinguish the two derivations.
    let row = 1usize;
    let box1 = tv.row_rect(row).expect("the row must be visible");
    let point = crate::core::Point::new(box1.x + 10, box1.y + box1.height as i32 / 2);
    assert_eq!(tv.row_at_point(point), Some(row), "the point must name its own row");

    let before = row_fill(&mut tv, row).expect("the row paints a fill");
    tv.handle_event(&crate::event::Event::MousePress { pos: point, button: 1, modifiers: 0 });
    assert_eq!(tv.focused_row(), Some(row), "the press must select the row it is on");
    let after = row_fill(&mut tv, row).expect("the row still paints a fill");
    assert_ne!(before, after, "the selected row must be visibly selected");
}

/// The fill of the SVG `<rect>` at the midpoint of content row `index`.
#[cfg(all(device_profile, feature = "desktop"))]
fn row_fill(tv: &mut TableWidget, index: usize) -> Option<String> {
    let rect = tv.geometry();
    let row = tv.row_rect(index)?;
    let at = crate::core::Point::new(row.x + 10, row.y + row.height as i32 / 2);
    let svg = crate::widget::svg::render_widget_to_svg(tv, rect);
    let mut best: Option<(u32, String)> = None;
    for element in svg.split("/>") {
        let Some(open) = element.find("<rect ") else { continue };
        let element = &element[open + "<rect ".len()..];
        let attr = |name: &str| -> Option<i32> {
            let key = format!("{name}=\"");
            let at = element.find(&key)? + key.len();
            let to = element[at..].find('"')? + at;
            element[at..to].parse().ok()
        };
        let (Some(x), Some(y), Some(w), Some(h)) =
            (attr("x"), attr("y"), attr("width"), attr("height"))
        else {
            continue;
        };
        if at.x < x || at.x >= x + w || at.y < y || at.y >= y + h {
            continue;
        }
        let Some(fill_at) = element.find("fill=\"") else { continue };
        let from = fill_at + "fill=\"".len();
        let Some(to) = element[from..].find('"') else { continue };
        let area = (w as u32).saturating_mul(h as u32);
        if best.as_ref().map(|(a, _)| area < *a).unwrap_or(true) {
            best = Some((area, element[from..from + to].to_string()));
        }
    }
    best.map(|(_, fill)| fill)
}

#[test]
fn test_row_height_overrides() {
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    assert_eq!(tv.row_height(0), None);
    assert_eq!(tv.row_height(99), None);

    tv.set_row_height(1, 28);
    assert_eq!(tv.row_height(1), Some(28));

    tv.set_row_height(5, 32);
    assert_eq!(tv.row_height(5), Some(32));
    assert_eq!(tv.row_height(1), Some(28));
}

#[test]
fn test_item_delegate() {
    struct TestDelegate;

    impl ItemDelegate for TestDelegate {
        fn create_editor(
            &self,
            _parent: &mut BaseWidget,
            _row: usize,
            _column: usize,
        ) -> Option<Box<dyn Widget>> {
            None
        }

        fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

        fn get_editor_data(
            &self,
            _editor: &dyn Widget,
            _row: usize,
            _column: usize,
        ) -> Option<String> {
            None
        }
    }

    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 300));
    assert!(!tv.has_delegate());
    assert!(tv.delegate_ref().is_none());

    tv.set_delegate(Arc::new(TestDelegate));
    assert!(tv.has_delegate());
    assert!(tv.delegate_ref().is_some());
}

/// The delegate's three methods are **called**, in order, by one entry point.
///
/// # The defect this pins
///
/// `ItemDelegate` declared `create_editor` / `set_editor_data` / `get_editor_data` and the widget
/// called **none** of them: the delegate was stored, published as `has_delegate`, reachable through
/// `delegate_ref`, and had no effect on anything. The test above could not see that, because
/// setting a delegate and reading it back is exactly what the broken version already supported --
/// the same blind spot the `calendar_popup` round-trip test had.
///
/// The assertion is a **call log**: the delegate records the order it was asked to do things in, so
/// a `begin_edit` that created an editor without seeding it, or a `commit_edit` that read a cell the
/// edit was not on, fails here rather than silently returning the wrong text.
#[test]
fn the_delegate_is_driven_through_begin_and_commit() {
    use std::sync::Mutex;

    #[derive(Default)]
    struct LoggingDelegate {
        calls: Mutex<Vec<String>>,
    }
    impl ItemDelegate for LoggingDelegate {
        fn create_editor(
            &self,
            _parent: &mut BaseWidget,
            row: usize,
            column: usize,
        ) -> Option<Box<dyn Widget>> {
            self.calls.lock().expect("lock").push(format!("create:{row},{column}"));
            // A real editor: the parent's own label, which the delegate then seeds.
            Some(Box::new(crate::widget::base_widgets::label::Label::new(
                String::new(),
                Rect::new(0, 0, 40, 16),
            )))
        }
        fn set_editor_data(&self, _editor: &mut dyn Widget, row: usize, column: usize) {
            self.calls.lock().expect("lock").push(format!("seed:{row},{column}"));
        }
        fn get_editor_data(
            &self,
            _editor: &dyn Widget,
            row: usize,
            column: usize,
        ) -> Option<String> {
            self.calls.lock().expect("lock").push(format!("read:{row},{column}"));
            Some(format!("{row}/{column}"))
        }
    }

    let delegate = Arc::new(LoggingDelegate::default());
    let mut tv = TableWidget::new(Rect::new(0, 0, 400, 200));
    tv.set_model(Arc::new(TestTableModel::new(4, 3)));

    // No delegate: the request is meaningless and answers `None` rather than panicking.
    assert!(tv.begin_edit(1, 2).is_none(), "no delegate, no editor");
    assert_eq!(tv.editing_cell(), None);

    tv.set_delegate(delegate.clone());
    let editor = tv.begin_edit(1, 2).expect("the delegate builds an editor");
    assert_eq!(tv.editing_cell(), Some((1, 2)), "the edit is tracked");
    assert_eq!(
        delegate.calls.lock().expect("lock").as_slice(),
        ["create:1,2", "seed:1,2"],
        "the editor is created and then seeded, in that order"
    );

    let committed = tv.commit_edit(editor.as_ref()).expect("the delegate reads it back");
    assert_eq!(committed, "1/2", "the value comes from the delegate, not a downcast");
    assert_eq!(tv.editing_cell(), None, "and the edit is closed");
    assert_eq!(
        delegate.calls.lock().expect("lock").as_slice(),
        ["create:1,2", "seed:1,2", "read:1,2"],
        "and the read is of the cell the edit was on"
    );

    // An out-of-range cell is refused before the delegate is troubled.
    let before = delegate.calls.lock().expect("lock").len();
    assert!(tv.begin_edit(99, 99).is_none(), "the cell must exist");
    assert_eq!(
        delegate.calls.lock().expect("lock").len(),
        before,
        "and the delegate was not asked"
    );

    // Cancelling drops the tracking without reading anything back.
    let _editor = tv.begin_edit(0, 0).expect("a second edit opens");
    tv.cancel_edit();
    assert_eq!(tv.editing_cell(), None, "cancelling closes it");
}

/// Scrolling, wheel paging, keyboard navigation and edit triggers are wired.
#[test]
fn scrolling_keyboard_and_edit_triggers_are_wired() {
    use crate::event::types::EventHandler;
    struct ManyRows;
    impl TableModel for ManyRows {
        fn row_count(&self) -> usize {
            40
        }
        fn column_count(&self) -> usize {
            2
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(std::format!("{row}-{column}"))
        }
    }
    let mut table = TableWidget::new(Rect::new(0, 0, 300, 120));
    table.set_model(Arc::new(ManyRows));
    table.handle_event(&Event::wheel(0, -1, 0));
    assert_eq!(table.scroll_row(), 1, "one notch is one row");
    table.set_edit_triggers(TableEditTriggers::DOUBLE_CLICKED.or(TableEditTriggers::EDIT_KEY));
    assert!(table.edit_triggers().contains(TableEditTriggers::DOUBLE_CLICKED));
    let requested = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&requested);
    table.edit_requested.connect(move |cell| {
        if let Ok(mut slot) = sink.lock() {
            *slot = Some(*cell);
        }
    });
    table.handle_event(&Event::MouseDoubleClick { pos: Point::new(20, 40), button: 1 });
    assert!(
        requested.lock().map(|slot| slot.is_some()).unwrap_or(false),
        "a double-click requests an edit"
    );

    table.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
    table.handle_event(&Event::KeyPress { key: crate::event::key_codes::END, modifiers: 0 });
    assert!(table.scroll_row() > 1, "End reaches the last row");
}

struct FourColumnModel;

impl TableModel for FourColumnModel {
    fn row_count(&self) -> usize {
        3
    }
    fn column_count(&self) -> usize {
        4
    }
    fn data(&self, row: usize, column: usize) -> Option<String> {
        Some(format!("r{row}c{column}"))
    }
}

/// V-03: a horizontal offset makes the right-hand columns paintable and clickable.
#[test]
fn scroll_column_reaches_the_right_hand_columns() {
    let mut table = TableWidget::new(Rect::new(0, 0, 200, 160));
    table.set_model(Arc::new(FourColumnModel));
    table.set_scroll_column(2);
    assert_eq!(table.scroll_column(), 2);
    table.set_scroll_column(99);
    assert_eq!(table.scroll_column(), 3, "clamped to the last column");

    table.set_scroll_column(2);
    // The first visible column starts at the content box's left edge.
    let content = ControlMetrics::band_inset(table.base.geometry(), TABLE_INSET);
    assert_eq!(table.column_at(content.x + 2), Some(2));
    assert_eq!(table.column_at(content.x + 60), Some(3));
}

/// V-04: Enter edits the column the pointer last named, not column zero.
#[test]
fn enter_edits_the_pointed_column() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 160));
    table.set_model(Arc::new(FourColumnModel));
    table.set_editable(true);
    table.set_edit_triggers(TableEditTriggers::ALL);
    let edited = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&edited);
    table.edit_requested.connect(move |cell| {
        if let Ok(mut slot) = sink.lock() {
            *slot = Some(*cell);
        }
    });
    table.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });

    let row = table.row_rect(1).expect("row 1 is visible");
    let content = ControlMetrics::band_inset(table.base.geometry(), TABLE_INSET);
    table.handle_event(&Event::MousePress {
        pos: Point::new(content.x + 60, row.y + 2),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(table.focused_row(), Some(1));
    table.handle_event(&Event::KeyPress { key: crate::event::key_codes::ENTER, modifiers: 0 });
    assert_eq!(*edited.lock().expect("edited lock"), Some((1, 1)));
}

/// V-05: fitting a column records an explicit width derived from its contents.
#[test]
fn fit_column_to_contents_records_a_measured_width() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 160));
    table.set_model(Arc::new(FourColumnModel));
    assert_eq!(table.column_width(0), None);
    assert!(table.fit_column_to_contents(0));
    let width = table.column_width(0).expect("an override was recorded");
    assert!(width >= 40, "the 40 px floor applies");
    assert!(!table.fit_column_to_contents(9), "there is no column 9");
    table.fit_columns_to_contents();
    for column in 0..4 {
        assert!(table.column_width(column).is_some());
    }
}

// ── RW-5: column structure ──

#[test]
fn column_structure_crud_emits_one_change_per_edit() {
    use crate::widget::view_widgets::data_grid::{ColumnChangeKind, ColumnDataType, ColumnSpec};

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(Arc::new(StaticTableModel));
    assert_eq!(table.column_count(), 2);

    let seen = Arc::new(Mutex::new(Vec::<(ColumnChangeKind, usize)>::new()));
    let sink = Arc::clone(&seen);
    table.column_changed.connect(move |change| {
        sink.lock().expect("lock").push((change.kind, change.index));
    });

    let at =
        table.insert_column(1, ColumnSpec::new("Extra", 60).with_type(ColumnDataType::Integer));
    assert_eq!(at, 1);
    assert_eq!(table.column_count(), 3, "the structure owns the painted count");
    assert_eq!(table.column_title(1).as_deref(), Some("Extra"));
    assert_eq!(table.column_type(1), Some(&ColumnDataType::Integer));
    assert_eq!(table.column_width(1), Some(60), "the spec width is recorded");

    assert!(table.set_column_title(0, "First"));
    assert_eq!(table.column_title(0).as_deref(), Some("First"));
    assert!(!table.set_column_title(7, "nope"), "there is no column 7");

    let removed = table.remove_column(1).expect("the inserted column exists");
    assert_eq!(removed.title, "Extra");
    assert_eq!(table.column_count(), 2);
    assert!(table.remove_column(5).is_none(), "removing past the end is refused");

    assert_eq!(
        *seen.lock().expect("lock"),
        vec![
            (ColumnChangeKind::Inserted, 1),
            (ColumnChangeKind::TitleChanged, 0),
            (ColumnChangeKind::Removed, 1),
        ],
        "the host is told exactly which structural edits happened"
    );
}

#[test]
fn structural_column_changes_clamp_the_scroll_offset() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(Arc::new(FourColumnModel));
    table.set_scroll_column(3);
    assert_eq!(table.scroll_column(), 3);

    table.remove_column(3);
    assert_eq!(table.scroll_column(), 2, "the scroll moves to a column that exists");
}

// ── RW-7: incremental data source ──

struct CountingSource {
    rows: std::sync::atomic::AtomicUsize,
    columns: usize,
    changed: GenericSignal,
}

impl CountingSource {
    fn new(rows: usize, columns: usize) -> Self {
        Self {
            rows: std::sync::atomic::AtomicUsize::new(rows),
            columns,
            changed: GenericSignal::new(),
        }
    }
}

impl IncrementalTableDataSource for CountingSource {
    fn row_count(&self) -> usize {
        self.rows.load(Ordering::Relaxed)
    }

    fn column_count(&self) -> usize {
        self.columns
    }

    fn column_name(&self, column: usize) -> Option<String> {
        Some(format!("C{column}"))
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        Some(format!("{row}:{column}"))
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.changed)
    }
}

#[test]
fn a_data_source_supplies_rows_columns_and_cells() {
    let source = Arc::new(CountingSource::new(3, 2));
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(source.clone());

    assert!(table.has_data_source());
    assert!(!table.has_model(), "the two binder slots are exclusive");
    assert!(table.data_source_ref().is_some());
    assert_eq!(table.row_count(), 3);
    assert_eq!(table.column_count(), 2);
    assert_eq!(table.item(1, 1), Some("1:1".to_string()));

    // A source change is drained before the next paint and repairs the projection.
    table.select_row(2);
    table.set_scroll_row(2);
    source.rows.store(1, Ordering::Relaxed);
    source.changed.emit();
    table.drain_data_updates();
    assert_eq!(table.row_count(), 1);
    assert_eq!(table.scroll_row(), 0, "the scroll is clamped to the new row count");

    table.clear_data_source();
    assert!(!table.has_data_source());
    assert_eq!(table.row_count(), 0);
}

#[test]
fn a_paint_reads_the_source_through_its_window_api() {
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    let source = Arc::new(CountingSource::new(4, 2));
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(source.clone());
    assert_eq!(table.window_fetches, 0);

    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(240, 120), 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        table.draw(&mut context);
    }
    backend.end_frame();
    assert_eq!(table.window_fetches, 1, "one bounded window per frame");

    // A second frame with no change reuses the window.
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        table.draw(&mut context);
    }
    backend.end_frame();
    assert_eq!(table.window_fetches, 1, "a cached window is not re-fetched");

    source.rows.store(2, Ordering::Relaxed);
    source.changed.emit();
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        table.draw(&mut context);
    }
    backend.end_frame();
    assert_eq!(table.window_fetches, 2, "a change invalidates the window");
}

// ── RW-8: VecTableModel ──

#[test]
fn vec_table_model_edits_one_cell_and_one_row_at_a_time() {
    let mut model = VecTableModel::new(vec!["A".to_string(), "B".to_string()]);
    let cells = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
    let rows = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cell_sink = Arc::clone(&cells);
    let row_sink = Arc::clone(&rows);
    model.cell_changed_signal().connect(move |change| {
        cell_sink.lock().expect("lock").push((change.0, change.1));
    });
    model.rows_changed_signal().connect(move || {
        row_sink.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(model.row_count(), 0);
    assert_eq!(model.column_count(), 2);
    assert_eq!(model.push_row(vec!["r0".to_string(), "c1".to_string()]), 0);
    assert_eq!(rows.load(Ordering::Relaxed), 1);
    assert!(model.set_cell(0, 1, "changed"));
    assert_eq!(model.cell(0, 1), Some("changed"));
    assert_eq!(*cells.lock().expect("lock"), vec![(0usize, 1usize)]);
    assert!(!model.set_cell(0, 9, "nope"), "there is no column 9");
    assert!(!model.set_cell(5, 0, "nope"), "there is no row 5");

    assert!(model.remove_row(0).is_some());
    assert_eq!(model.row_count(), 0);
    assert_eq!(rows.load(Ordering::Relaxed), 2);

    // The model is a `TableModel`, so the existing adapter can project it.
    let shared: Arc<VecTableModel> =
        Arc::new(VecTableModel::with_rows(vec!["A".to_string()], vec![vec!["x".to_string()]]));
    let source = crate::widget::view_widgets::data_source::TableModelDataSource::new(shared);
    assert_eq!(source.row_count(), 1);
    assert_eq!(source.column_count(), 1);
    assert_eq!(source.data(0, 0), Some("x".to_string()));
}

// ── RW-17: empty/error captions ──

#[test]
fn empty_and_error_captions_are_stored_and_cleared() {
    use crate::widget::view_widgets::collection_state::CollectionState;

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    assert!(table.empty_text().is_none());
    table.set_empty_text("No rows");
    assert_eq!(table.empty_text(), Some("No rows"));
    table.set_error_text("Load failed");
    assert_eq!(table.error_text(), Some("Load failed"));
    assert_eq!(table.placeholder().text(), Some("Load failed"));
    table.clear_error_text();
    assert_eq!(table.error_text(), None);
    assert_eq!(table.placeholder().text(), Some("No rows"));
    table.clear_empty_text();
    assert!(table.placeholder().is_empty());
}

/// RW-6: a model's declared column type reaches the table when the structure leaves it `Auto`.
#[test]
fn a_model_can_declare_a_column_type() {
    use crate::widget::view_widgets::data_grid::ColumnDataType;

    struct TypedModel;

    impl TableModel for TypedModel {
        fn row_count(&self) -> usize {
            1
        }

        fn column_count(&self) -> usize {
            2
        }

        fn data(&self, _row: usize, column: usize) -> Option<String> {
            Some(format!("c{column}"))
        }

        fn column_type(&self, column: usize) -> Option<&ColumnDataType> {
            static INTEGER: ColumnDataType = ColumnDataType::Integer;
            (column == 0).then_some(&INTEGER)
        }
    }

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(Arc::new(TypedModel));
    assert_eq!(table.column_type(0), Some(&ColumnDataType::Integer));
    assert_eq!(table.column_type(1), None, "the default answers None");
    assert_eq!(table.column_type(9), None);
}

// ── Regression tests: composited geometry, selection and window caching ──

/// A delegate that asks for a fixed row height and builds a label editor.
///
/// Shared by the composited-offset and editor-geometry tests: both need a delegate that changes
/// the rows' resolved height, which is exactly what the empty-`row_heights` fast path ignored.
struct FixedHeightDelegate {
    height: u32,
}

impl ItemDelegate for FixedHeightDelegate {
    fn create_editor(
        &self,
        _parent: &mut BaseWidget,
        _row: usize,
        _column: usize,
    ) -> Option<Box<dyn Widget>> {
        Some(Box::new(crate::widget::base_widgets::label::Label::new(
            String::new(),
            Rect::new(0, 0, 40, 16),
        )))
    }

    fn set_editor_data(&self, _editor: &mut dyn Widget, _row: usize, _column: usize) {}

    fn get_editor_data(&self, _editor: &dyn Widget, _row: usize, _column: usize) -> Option<String> {
        None
    }

    fn size_hint(&self, _row: usize, _column: usize) -> Option<crate::core::Size> {
        Some(crate::core::Size::new(60, self.height))
    }
}

/// Row offsets compose every resolved height, including a delegate's size hint.
///
/// The fast path returned `TABLE_ROW_HEIGHT * index` whenever `row_heights` was empty, while
/// `resolved_row_height` consulted the delegate. A delegate asking for 40 px rows therefore drew
/// row 1 at the band top when `scroll_row == 1`, but `row_at_point` targeted `20 * 1` and answered
/// row 0 — the hit test and the paint disagreed. The offset and the hit test must be one geometry.
#[test]
fn a_delegate_height_composes_the_row_offset() {
    let mut table = TableWidget::new(Rect::new(0, 0, 300, 300));
    table.set_model(Arc::new(TestTableModel::new(6, 2)));
    table.set_delegate(Arc::new(FixedHeightDelegate { height: 40 }));

    assert_eq!(table.row_offset(1), 40, "the offset composes the delegate's resolved height");

    table.set_scroll_row(1);
    assert_eq!(table.scroll_row(), 1);
    let band = table.rows_band();
    let row = table.row_rect(1).expect("row 1 is visible");
    assert_eq!(row.y, band.y, "the scrolled-to row begins at the band top");
    assert_eq!(
        table.row_at_point(Point::new(row.x + 2, band.y + 5)),
        Some(1),
        "a point in the painted row must resolve back to it"
    );

    let editor = table.begin_edit(1, 0).expect("the delegate builds an editor");
    let geometry = editor.geometry();
    assert_eq!(geometry.y, band.y, "the editor sits over the row that is painted");
    assert_eq!(geometry.height, 40, "and is as tall as the resolved row");
}

/// The window the source is read through counts the columns the paint loop actually draws.
///
/// `visible_column_span` started its cumulative `left` at `horizontal_offset()` while the paint
/// loop measures every column relative to the scrolled band. The span was therefore short by the
/// columns inside the first offset pixels: with eight 40 px columns and `scroll_column == 3` the
/// paint drew columns 3..8 while the window stopped at column 5.
#[test]
fn a_scrolled_column_span_covers_every_painted_column() {
    let mut table = TableWidget::new(Rect::new(0, 0, 200, 160));
    table.set_data_source(Arc::new(CountingSource::new(3, 8)));
    table.set_scroll_column(3);
    let content = ControlMetrics::band_inset(table.base.geometry(), TABLE_INSET);
    assert_eq!(
        table.visible_column_span(content.width),
        5,
        "columns 3..8 fit the band, so all five must be fetched"
    );
}

/// `begin_edit` places the editor over the cell the paint loop draws.
///
/// The x was `content.x + sum(widths(0..column))`, which ignored `scroll_column`; with two columns
/// scrolled away the editor appeared two columns to the right of its cell.
#[test]
fn begin_edit_follows_the_horizontal_scroll() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 160));
    table.set_model(Arc::new(FourColumnModel));
    table.set_delegate(Arc::new(FixedHeightDelegate { height: 20 }));
    table.set_scroll_column(2);

    let editor = table.begin_edit(0, 2).expect("the delegate builds an editor");
    let geometry = editor.geometry();
    let content = ControlMetrics::band_inset(table.base.geometry(), TABLE_INSET);
    assert_eq!(
        geometry.x, content.x,
        "the first scrolled column starts at the content edge, so its editor does too"
    );
    assert_eq!(geometry.width, table.resolved_column_width(2, content.width));
}

/// Installing a delegate asks for both the layout and a repaint.
///
/// The delegate changes the cell ink and, through `size_hint`, the row heights, so storing it and
/// requesting nothing left the first frame at the old geometry until an unrelated event repainted.
#[test]
fn setting_a_delegate_requests_layout_and_redraw() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));
    let redraws = Arc::new(AtomicUsize::new(0));
    let layouts = Arc::new(AtomicUsize::new(0));
    {
        let sink = Arc::clone(&redraws);
        table.base.redraw_requested_signal().connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
    }
    {
        let sink = Arc::clone(&layouts);
        table.base.layout_requested_signal().connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });
    }

    table.set_delegate(Arc::new(NoopDelegate));
    assert_eq!(redraws.load(Ordering::SeqCst), 1, "the cells must be repainted");
    assert_eq!(layouts.load(Ordering::SeqCst), 1, "and the rows re-measured");
}

/// A press cannot land in a header the paint loop refuses to draw.
///
/// `header_column_at` only checked `header_visible`, while the paint path also requires
/// `content.height > header.height`. In a 24 px table the header is not drawn, yet a press in the
/// content resolved through it and fired `section_clicked` instead of addressing the body.
#[test]
fn a_press_in_a_band_too_short_for_a_header_is_not_a_header_press() {
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 24));
    table.set_model(Arc::new(StaticTableModel));
    let fired = Arc::new(Mutex::new(None::<usize>));
    let sink = Arc::clone(&fired);
    table.section_clicked_signal().connect(move |column: Arc<usize>| {
        *sink.lock().expect("lock") = Some(*column);
    });

    let content = ControlMetrics::band_inset(table.base.geometry(), TABLE_INSET);
    table.handle_event(&Event::MousePress {
        pos: Point::new(content.x + 10, content.y + 2),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(*fired.lock().expect("lock"), None, "no header was painted, so none was pressed");
}

/// A width written through `set_column_width` is also the width the structure publishes.
///
/// The setter wrote only the override map, so `columns()[column].width` kept the share recorded
/// when the structure was materialised, contradicting the field's own contract (and the two other
/// setters that write both).
#[test]
fn set_column_width_updates_the_column_structure() {
    use crate::widget::view_widgets::data_grid::ColumnSpec;

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(Arc::new(StaticTableModel));
    table.insert_column(0, ColumnSpec::new("A", 50));

    table.set_column_width(0, 300);
    assert_eq!(table.columns()[0].width, 300, "the structure reports the painted width");
    assert_eq!(table.column_width(0), Some(300));
}

/// Host header labels still win after a structural edit materialises the column structure.
///
/// `ensure_columns`/`reconcile_columns` copied the model's names into `ColumnSpec::title`, and
/// `column_title` prefers a non-empty spec title. Any structural edit therefore froze the names
/// that existed at that moment: a later `set_header_labels` painted the model's old names instead.
#[test]
fn host_header_labels_survive_a_structural_edit() {
    use crate::widget::view_widgets::data_grid::ColumnSpec;

    let model = Arc::new(VecTableModel::with_rows(
        vec!["Alpha".to_string(), "Beta".to_string()],
        Vec::new(),
    ));
    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(model);
    table.insert_column(0, ColumnSpec::new("", 40));

    table.set_header_labels(&["X", "Y", "Z"]);
    assert_eq!(table.column_title(0).as_deref(), Some("X"));
    assert_eq!(table.column_title(1).as_deref(), Some("Y"));
    assert_eq!(table.column_title(2).as_deref(), Some("Z"));
}

/// `SelectionMode::None` accepts no selection, and switching to it clears what was there.
///
/// `select_row` emitted `selection_changed`, moved focus and reported `true` while the selection
/// model was a documented no-op, so the control painted a focus band for a selection that did not
/// exist. The mode switch left the old set behind for the same reason.
#[test]
fn selecting_in_none_mode_reports_no_selection() {
    use crate::widget::view_widgets::list_view::SelectionMode;

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_model(Arc::new(TestTableModel::new(5, 2)));
    assert!(table.select_row(1));

    table.set_selection_mode(SelectionMode::None);
    assert!(table.selected_rows().is_empty(), "the mode switch clears the selection");
    assert_eq!(table.focused_row(), None, "and the focus band with it");

    assert!(!table.select_row(3), "None accepts no selection");
    assert!(!table.select_row_with_modifiers(2, crate::shortcut::Modifiers::NONE));
    assert!(table.selected_rows().is_empty());
    assert_eq!(table.focused_row(), None, "and it does not paint a focus band");
}

/// Keyboard paging reads the validated focus row, not the stale field behind it.
///
/// A model without a change signal can shrink under the view. `UP`/`PAGE_UP` subtracted from the
/// raw `focused_row`, so a focus at row 9 in a 3-row table computed row 8: `select_row(8)` failed,
/// `set_scroll_row(8)` clamped to 2, and every further key recomputed 8.
#[test]
fn a_stale_focus_does_not_send_the_keyboard_past_the_model() {
    struct ShrinkingModel {
        rows: std::sync::atomic::AtomicUsize,
    }
    impl TableModel for ShrinkingModel {
        fn row_count(&self) -> usize {
            self.rows.load(std::sync::atomic::Ordering::Relaxed)
        }
        fn column_count(&self) -> usize {
            2
        }
        fn data(&self, row: usize, _column: usize) -> Option<String> {
            Some(format!("r{row}"))
        }
    }

    let model = Arc::new(ShrinkingModel { rows: std::sync::atomic::AtomicUsize::new(10) });
    let mut table = TableWidget::new(Rect::new(0, 0, 300, 100));
    table.set_model(model.clone());
    table.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
    assert!(table.set_focused_row(9));

    // The model shrinks without a `data_changed` signal, so the raw focus row is now out of range.
    model.rows.store(3, std::sync::atomic::Ordering::Relaxed);
    table.handle_event(&Event::KeyPress { key: crate::event::key_codes::UP, modifiers: 0 });
    assert_eq!(table.focused_row(), Some(0), "UP moves from a row that still exists");
    assert_eq!(table.scroll_row(), 0, "and does not jump the viewport to a stale clamp");

    model.rows.store(10, std::sync::atomic::Ordering::Relaxed);
    assert!(table.set_focused_row(9));
    model.rows.store(3, std::sync::atomic::Ordering::Relaxed);
    table.handle_event(&Event::KeyPress { key: crate::event::key_codes::PAGE_UP, modifiers: 0 });
    assert_eq!(table.focused_row(), Some(0), "PAGE_UP reads the same validated row");
    assert_eq!(table.scroll_row(), 0);
}

/// A different window origin is a cache miss even when the span returns to the same value.
///
/// The cache key XOR-folded `scroll_column` and `needed_columns` into the same bits: column 1 with
/// two needed columns and column 0 with three both produced key 3. A scroll back to column 0 reused
/// the window that began at column 1, so column 0 painted blank until the source changed.
#[test]
fn a_scroll_column_change_refetches_the_window() {
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn paint(table: &mut TableWidget, backend: &mut SoftwarePaintBackend) {
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(backend);
            table.draw(&mut context);
        }
        backend.end_frame();
    }

    let mut table = TableWidget::new(Rect::new(0, 0, 240, 120));
    table.set_data_source(Arc::new(CountingSource::new(4, 3)));
    let mut backend = SoftwarePaintBackend::new(crate::core::Size::new(240, 120), 1.0);

    paint(&mut table, &mut backend);
    assert_eq!(table.window_fetches, 1);

    table.set_scroll_column(1);
    paint(&mut table, &mut backend);
    assert_eq!(table.window_fetches, 2);

    table.set_scroll_column(0);
    paint(&mut table, &mut backend);
    assert_eq!(
        table.window_fetches, 3,
        "the origin changed, so the window that began at column 1 cannot be reused"
    );
    assert_eq!(
        table.window_cache.as_ref().map(|window| window.column_start),
        Some(0),
        "and the cached window begins where the paint begins"
    );
}
