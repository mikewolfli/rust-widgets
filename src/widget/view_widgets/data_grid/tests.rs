// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DataGrid` behaviour tests.

use super::*;
use crate::event::types::EventHandler;
use crate::signal::GenericSignal;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

struct StaticSource {
    rows: usize,
    cols: usize,
    data: Vec<Vec<String>>,
}

impl IncrementalTableDataSource for StaticSource {
    fn row_count(&self) -> usize {
        self.rows
    }

    fn column_count(&self) -> usize {
        self.cols
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.data.get(row).and_then(|line| line.get(column)).cloned()
    }
}

fn static_grid(rows: usize, cols: usize) -> DataGrid {
    let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows,
        cols,
        data: (0..rows).map(|r| (0..cols).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));
    grid
}

/// V-002: `begin_edit`'s editor is a mounted, painted control, not just a value carrier.
///
/// `DataGridCellEditor` is built only by [`DataGrid::begin_edit`] and read back only by
/// [`DataGrid::commit_edit`], and no test exercised either: the registration and contract gates
/// passed for a control that nothing ever mounted. This mounts it, paints it, and commits
/// through it, so the three facts a host relies on — it lands on the cell, it draws, and the
/// committed value is the cell's text — are asserted rather than assumed.
#[test]
fn the_cell_editor_paints_its_box_and_commits_its_text() {
    use crate::core::Size;
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use crate::widget::draw_bridge::draw_of;

    // The two halves of this control's lifecycle: built directly, and built by the grid.
    let mut direct = DataGridCellEditor::new(Rect::new(0, 0, 60, 20), "seed");
    assert_eq!(direct.text(), "seed", "the constructor seeds the value it was given");
    direct.set_text("typed");
    assert_eq!(direct.text(), "typed", "a replacement is read back verbatim");

    let mut grid = static_grid(2, 2);
    let mut editor = grid.begin_edit(1, 0).expect("the coordinate is inside the model");
    assert_eq!(grid.editing_cell(), Some((1, 0)));
    // Naming the concrete type here is also what the control-coverage gate reads: the editor
    // is the widget `begin_edit` hands back, so this is the downcast a host performs.
    let seeded = (editor.as_ref() as &dyn core::any::Any)
        .downcast_ref::<DataGridCellEditor>()
        .expect("begin_edit builds a DataGridCellEditor")
        .text()
        .to_string();
    assert_eq!(seeded, "1:0", "the editor is seeded with the cell's current text");
    let cell = editor.geometry();
    assert_eq!(cell, grid.cell_rect(1, 0), "the editor is placed over the cell it edits");

    // The editor is positioned in the grid's coordinates, so the backend has to cover the
    // grid rather than only the cell — a cell-sized frame would clip the paint at its origin
    // and report an empty `draw`.
    let size = Size::new(600, 300);
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    let probe = Color { r: 255, g: 0, b: 255, a: 255 };
    backend.begin_frame(probe);
    {
        let drawable = draw_of(editor.as_mut()).expect("the editor is a drawable widget");
        let mut context = RenderContext::new(&mut backend);
        drawable.draw(&mut context);
    }
    backend.end_frame();
    let painted = backend
        .frame_rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|px| (px[0], px[1], px[2]) != (probe.r, probe.g, probe.b));
    assert!(painted, "a `draw` that paints nothing is an empty implementation");

    let committed = grid.commit_edit(editor.as_ref()).expect("the edit is open");
    assert_eq!(committed, "1:0", "the editor is seeded with the cell's current text");
    assert_eq!(grid.editing_cell(), None, "committing closes the edit");
}

/// V-015: a chooser toggle reaches the grid's hidden-column state.
#[test]
fn a_chooser_toggle_hides_the_column_in_the_grid() {
    use crate::widget::view_widgets::column_chooser::ColumnChooser;

    let mut grid = static_grid(2, 3);
    let mut chooser = ColumnChooser::new(Rect::new(0, 0, 220, 260));
    chooser.set_columns(vec![
        ColumnSpec::new("Name", 100),
        ColumnSpec::new("Size", 80),
        ColumnSpec::new("Owner", 120),
    ]);
    let tokens: Vec<String> = chooser.columns().iter().map(ColumnSpec::to_token).collect();

    grid.attach_column_chooser(&chooser);
    assert!(grid.is_column_chooser_attached());
    assert!(!grid.is_column_hidden(1));

    assert!(chooser.set_column_visible(&tokens[1], false));
    assert!(grid.poll_column_chooser(), "the reported change is applied");
    assert!(grid.is_column_hidden(1), "the chooser hid the second column");

    assert!(chooser.set_column_visible(&tokens[1], true));
    assert!(grid.poll_column_chooser());
    assert!(!grid.is_column_hidden(1), "showing the column restores it");

    // A reorder is reported as source indices as well (V-015).
    assert!(chooser.move_column(0, 2));
    assert!(grid.poll_column_chooser());
    assert_eq!(grid.column_display_order(), vec![1, 2, 0]);

    grid.detach_column_chooser();
    assert!(!grid.is_column_chooser_attached());
    assert!(chooser.set_column_visible(&tokens[1], false));
    assert!(!grid.poll_column_chooser(), "a detached chooser reports nothing");
    assert!(!grid.is_column_hidden(1));
}

/// Chapter 3, gap 2: a condition committed on the bound row filters the grid, and detaching
/// stops the wiring without undoing the last filter.
#[test]
fn a_condition_from_the_filter_row_filters_the_grid() {
    let mut grid = static_grid(4, 2);
    let mut row = FilterRow::new(Rect::new(0, 0, 300, 28));
    row.set_columns(vec!["Id".to_string(), "Name".to_string()]);

    grid.attach_filter_row(&row);
    assert!(grid.is_filter_row_attached());
    assert_eq!(grid.fetch_visible_cells().len(), 4, "an empty row filters nothing");

    assert!(row.set_condition(0, FilterCondition::contains(0, "2")));
    assert!(grid.poll_filter_row(), "the reported conditions are applied");
    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 1, "the condition filters the grid rows");
    assert_eq!(rows[0][0], Some("2:0".to_string()));
    assert!(!grid.poll_filter_row(), "an unchanged report is not re-applied");

    grid.detach_filter_row();
    assert!(!grid.is_filter_row_attached());
    assert!(row.set_condition(0, FilterCondition::contains(0, "3")));
    assert!(!grid.poll_filter_row(), "a detached row reports nothing");
    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 1, "the last applied filter stays");
    assert_eq!(rows[0][0], Some("2:0".to_string()));
}

#[test]
fn header_sections_split_by_width_and_report_clicks() {
    let mut header = GridViewHeader::new(Rect::new(0, 0, 300, 28));
    header.set_columns(vec![
        ColumnSpec::new("Name", 100).with_sortable(true),
        ColumnSpec::new("Size", 100).with_filterable(true),
        ColumnSpec::new("Owner", 100),
    ]);
    let clicked = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = clicked.clone();
    header.section_clicked.connect(move |index| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*index);
        }
    });

    assert_eq!(header.section_rect(1), Some(Rect::new(100, 0, 100, 28)));
    let middle = header.section_rect(1).expect("section 1");
    header.handle_event(&Event::mouse_press(middle.x + 10, middle.y + 10, 1));
    assert_eq!(*clicked.lock().expect("clicked lock"), vec![1]);
}

#[test]
fn header_sort_indicator_cycles_and_reports_the_order() {
    let mut header = GridViewHeader::new(Rect::new(0, 0, 200, 28));
    header.set_columns(vec![
        ColumnSpec::new("Name", 100).with_sortable(true),
        ColumnSpec::new("Fixed", 100),
    ]);
    assert!(header.set_sort_indicator(0, Some(SortOrder::Ascending)));
    assert_eq!(header.sort_indicator(0), Some(SortOrder::Ascending));
    assert_eq!(header.toggle_sort(0), Some(SortOrder::Descending));
    assert_eq!(header.toggle_sort(0), None);
    assert_eq!(header.sort_indicator(0), None);
    assert_eq!(header.toggle_sort(1), None, "an unsortable section never sorts");
    assert!(!header.set_sort_indicator(5, None), "there is no section 5");
}

#[test]
fn header_resize_emits_the_new_width() {
    let mut header = GridViewHeader::new(Rect::new(0, 0, 300, 28));
    header.set_columns(vec![ColumnSpec::new("Name", 100)]);
    header.set_resize_mode(ColumnWidthMode::Interactive);
    let resized = Arc::new(Mutex::new(Vec::<(usize, u32)>::new()));
    let sink = resized.clone();
    header.section_resized.connect(move |pair| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*pair);
        }
    });

    header.handle_event(&Event::mouse_press(99, 10, 1));
    header.handle_event(&Event::mouse_move(139, 10));
    header.handle_event(&Event::mouse_release(139, 10, 1));

    assert_eq!(header.section_width(0), 140);
    assert_eq!(*resized.lock().expect("resized lock"), vec![(0, 140)]);
}

#[test]
fn set_header_adopts_titles_widths_and_arrows() {
    let mut grid = static_grid(3, 3);
    let mut header = GridViewHeader::new(Rect::new(0, 0, 400, 28));
    header.set_columns(vec![
        ColumnSpec::new("Alpha", 90).with_sortable(true),
        ColumnSpec::new("Beta", 110).with_filterable(true),
        ColumnSpec::new("Gamma", 120),
    ]);
    header.set_sort_indicator(1, Some(SortOrder::Descending));

    grid.set_header(&header);

    assert_eq!(grid.header_columns().len(), 3);
    assert_eq!(grid.header_columns()[0].title, "Alpha");
    assert_eq!(grid.column_width_at(1), 110);
    assert_eq!(grid.sort_specs().len(), 1);
    assert_eq!(grid.sort_specs()[0], SortSpec { column: 1, descending: true });
    assert_eq!(grid.get("summary_text"), Ok(CapabilityValue::String("3 rows".to_string())));
}

#[test]
fn a_page_click_on_the_bound_bar_reaches_the_grid() {
    let mut grid = static_grid(100, 2);
    let mut pagination =
        crate::widget::nav_widgets::pagination::Pagination::new(Rect::new(0, 0, 300, 32));
    pagination.set_page_size(10);
    grid.set_pagination(&mut pagination);
    assert_eq!(pagination.total(), 100, "the bar learns the row count");
    assert_eq!(grid.pagination_state(), Some((0, 10)));

    let applied = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = applied.clone();
    grid.page_applied.connect(move |page| {
        if let Ok(mut guard) = sink.lock() {
            guard.push(*page);
        }
    });

    assert!(pagination.next_page());
    assert_eq!(grid.poll_pagination(), Some(1));
    assert_eq!(grid.scroll_row(), 10);
    assert_eq!(*applied.lock().expect("applied lock"), vec![1]);
    assert_eq!(grid.poll_pagination(), None, "an unchanged page is not re-applied");

    // Pushing the grid's page back into the bar is the other half of the binding.
    grid.set_scroll_row(30);
    assert!(grid.sync_pagination(&mut pagination));
    assert_eq!(pagination.page(), 3);
}

#[test]
fn summary_text_reports_rows_selection_and_filter() {
    let mut grid = static_grid(10, 2);
    assert_eq!(grid.summary_text(), "10 rows");
    grid.set_current_cell(0, 0);
    assert!(grid.summary_text().contains("selected"));
    grid.set_filters(vec![ColumnFilter { column: 0, query: "1".to_string() }]);
    assert!(grid.summary_text().contains("filtered"));
}

#[test]
fn stretch_last_gives_the_remaining_room_to_the_last_column() {
    let mut grid = static_grid(3, 3);
    grid.set_column_width_mode(ColumnWidthMode::StretchLast);
    let width = grid.column_width_at(2);
    assert!(width > 0);
    // The first two columns keep the base width, so the last takes the rest.
    assert_eq!(grid.column_width_at(0), grid.default_column_width());
    assert_eq!(grid.column_width_at(1), grid.default_column_width());
    assert_eq!(
        width as i32,
        grid.cells_box().width as i32 - 2 * grid.default_column_width() as i32
    );
}

/// A cell's box must be a pure function of its own projected indices.
///
/// The draw loop used to advance its own `x += column_width` cursor, so a cell's box
/// depended on how many cells happened to precede it in the fetched window. Both the draw
/// pass and `cell_at` now read `cell_rect`, so this pins the closed form.
#[test]
fn a_cells_box_is_a_function_of_its_own_indices() {
    let grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    let cells = grid.cells_box();

    assert_eq!(grid.cell_rect(0, 0), Rect::new(cells.x, cells.y, 120, 20));
    assert_eq!(grid.cell_rect(0, 2).x, cells.x + 240);
    assert_eq!(grid.cell_rect(3, 0).y, cells.y + 60);
    assert_eq!(grid.cell_rect(3, 0).width, 120);
    assert_eq!(grid.cell_rect(3, 0).height, 20);

    // Idempotent: asking twice names the same box.
    assert_eq!(grid.cell_rect(1, 1), grid.cell_rect(1, 1));

    // The cells start below the column-title strip, never on the frame's own stroke.
    assert!(grid.cells_box().y > 0, "cells must clear the column-title strip");
    assert!(grid.cells_box().height < 300, "the title strip must be removed");
}

/// The box a press resolves to must be the box that was painted for that cell.
#[test]
fn the_cell_a_press_resolves_to_is_the_cell_that_was_painted() {
    // The grid is wider than four 120 px columns so that column 3's centre falls inside
    // the control — a cell whose centre is past the right edge cannot be probed at all,
    // because nothing is painted there.
    let mut grid = DataGrid::new(Rect::new(0, 0, 900, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 4,
        data: (0..4).map(|r| (0..4).map(|c| format!("{}:{}", r, c)).collect()).collect(),
    }));

    let target = grid.cell_rect(2, 3);
    let centre =
        Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);
    assert_eq!(grid.cell_at(centre), Some((2, 3)));
    assert_eq!(grid.selection(), None, "merely probing must not select");

    // The box that *was* painted is the box a press resolves to, so the same point names
    // the same cell for probing and for pressing.
    grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
    assert_eq!(grid.selection(), Some((2, 3)));

    // A press in the column-title strip or outside the grid resolves to nothing rather
    // than to a phantom cell.
    assert_eq!(grid.cell_at(Point::new(centre.x, 1)), None);
    assert_eq!(grid.cell_at(Point::new(-5, -5)), None);

    let below = Point::new(centre.x, target.y + 20 * 100);
    grid.handle_event(&Event::MousePress { pos: below, button: 1, modifiers: 0 });
    assert_eq!(grid.selection(), None, "a press on empty space must clear the selection");
}

/// A selected cell is **painted** as selected, and a hovered cell is marked more faintly.
///
/// # The defect this pins
///
/// `selection` was stored and never drawn: a press set the field, re-requested a redraw, and the
/// grid came back byte-identical. A stored state with no consumer is the same shape as a declared
/// token with no consumer, and this one is a *user-visible* one — the user cannot tell the cell
/// they clicked from any other.
///
/// # Why the assertion counts accent strokes rather than comparing documents
///
/// "The document changed" would also be satisfied by the hover outline or by the frozen-column
/// rule. The marker is emitted as a `<rect ... stroke="..." stroke-width="2" />` in the accent
/// colour, so counting elements carrying that stroke names exactly the element under test.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn a_selected_cell_is_painted_as_selected() {
    let _guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);

    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 4,
        data: (0..4).map(|r| (0..4).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));

    let target = grid.cell_rect(1, 1);
    let centre =
        Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);

    let before = width_two_strokes(&mut grid);
    assert_eq!(before, 0, "an untouched grid draws no selection marker");

    grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
    assert_eq!(grid.selection(), Some((1, 1)));
    let after = width_two_strokes(&mut grid);
    assert_eq!(
        after,
        before + 1,
        "selecting a cell must paint exactly one selection marker (had {before}, now {after})"
    );
}

/// How many SVG `<rect>` strokes are drawn at width 2 — how the selection marker is emitted.
#[cfg(all(device_profile, feature = "desktop"))]
fn width_two_strokes(grid: &mut DataGrid) -> usize {
    let rect = grid.geometry();
    let svg = crate::widget::svg::render_widget_to_svg(grid, rect);
    svg.match_indices("stroke-width=\"2\"").count()
}

/// Scrolling moves the mapping from a point to a cell along with the cells themselves.
///
/// The scroll offset is applied inside `cell_rect`, so a press at a fixed point names a
/// later column once the grid has scrolled left; the painted box and the resolved cell
/// move together.
#[test]
fn scrolling_moves_the_point_to_cell_mapping_with_the_cells() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 8,
        data: (0..4).map(|r| (0..8).map(|c| format!("{}:{}", r, c)).collect()).collect(),
    }));

    let first = grid.cell_rect(0, 0);
    let probe = Point::new(first.x + 5, first.y + 5);
    assert_eq!(grid.cell_at(probe), Some((0, 0)));

    grid.set_scroll_column(2);
    // The first painted column is now projected column 2, so the same point names it.
    assert_eq!(grid.cell_at(probe), Some((0, 2)));
    // The first visible scrolling column occupies the band's leading edge; the columns
    // hidden to its left are off-screen rather than stacked on top of it.
    assert_eq!(grid.cell_rect(0, 2).x, first.x);
    assert!(grid.cell_rect(0, 0).x < first.x, "scrolled-away columns stay off-screen");

    // The projected column just right of the probe is one further along, so the mapping
    // really is continuous across the column boundary rather than pinned to a single cell.
    assert_eq!(grid.cell_at(Point::new(first.x + 120 + 5, probe.y)), Some((0, 3)));

    // A point left of the cells box — the frame margin the control does not paint in —
    // resolves to nothing. This is where truncating `usize` division used to go wrong: it
    // floored a negative distance to `0` and named a phantom column.
    assert_eq!(
        grid.cell_at(Point::new(first.x - 1, probe.y)),
        None,
        "the frame margin is not a cell"
    );
}

/// A frozen column keeps its leading position when the grid scrolls horizontally.
///
/// `cell_rect` used to apply `scroll_column` to every column, so the "frozen" prefix slid
/// away with the rest of the grid and the only trace of the feature was the divider line.
/// The pinned band is now a fixed function of the column index; the scrolling columns on
/// its right still travel the scroll.
#[test]
fn frozen_columns_ignore_horizontal_scroll() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 900, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 8,
        data: (0..4).map(|r| (0..8).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));
    grid.set_frozen_columns(2);

    let cells = grid.cells_box();
    let first = grid.cell_rect(0, 0);
    let second = grid.cell_rect(0, 1);
    assert_eq!(first.x, cells.x);
    assert_eq!(second.x, cells.x + grid.column_width as i32);

    grid.set_scroll_column(5);

    assert_eq!(grid.cell_rect(0, 0), first, "the frozen column must not travel the scroll");
    assert_eq!(grid.cell_rect(0, 1), second, "nor the next frozen column");
    // Rows still scroll: only the column offset is pinned.
    assert_eq!(grid.cell_rect(2, 0).y, cells.y + 2 * grid.row_height as i32);

    // A point over the frozen band names a frozen column, not the scrolling column the old
    // inverse arithmetic would have named.
    let frozen_centre =
        Point::new(second.x + second.width as i32 / 2, second.y + second.height as i32 / 2);
    assert_eq!(grid.cell_at(frozen_centre), Some((0, 1)));

    // To the right of the band the original scroll mapping is unchanged.
    let scrolling = grid.cell_rect(2, 7);
    assert_eq!(
        scrolling.x,
        cells.x + 2 * grid.column_width as i32 + 2 * grid.column_width as i32,
        "the frozen prefix occupies its own band before the scrolled columns"
    );
    let scrolling_centre = Point::new(
        scrolling.x + scrolling.width as i32 / 2,
        scrolling.y + scrolling.height as i32 / 2,
    );
    assert_eq!(grid.cell_at(scrolling_centre), Some((2, 7)));

    // The fetched window must include the frozen prefix, or the draw pass would have no
    // values to repaint over the scrolling cells.
    assert_eq!(grid.visible_window().2, 0);
}

/// A press inside the frozen band selects the frozen cell it is over.
#[test]
fn a_click_in_the_frozen_band_selects_the_frozen_column() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 8,
        data: (0..4).map(|r| (0..8).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));
    grid.set_frozen_columns(2);
    grid.set_scroll_column(5);

    let target = grid.cell_rect(3, 1);
    let centre =
        Point::new(target.x + target.width as i32 / 2, target.y + target.height as i32 / 2);
    grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });

    assert_eq!(grid.selection(), Some((3, 1)));
}

/// The frozen band is **painted** at the leading edge after a horizontal scroll.
///
/// The model fix alone is not enough: with the fetch window starting at `scroll - overscan`,
/// a frozen column that lies before it never reaches the draw pass at all. Only the frozen
/// cell is made non-empty, so exactly one text run must appear — and it must land in the
/// leading column.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn a_frozen_column_is_painted_at_the_leading_edge_after_scrolling() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    let mut data: Vec<Vec<String>> = vec![vec![String::new(); 8]; 4];
    data[1][0] = "P".to_string();
    grid.set_data_source(Arc::new(StaticSource { rows: 4, cols: 8, data }));
    grid.set_frozen_columns(2);
    grid.set_scroll_column(5);

    let rect = grid.geometry();
    let svg = crate::widget::svg::render_widget_to_svg(&mut grid, rect);
    let boxes = crate::widget::svg::text_ink_boxes(&svg);
    assert_eq!(
        boxes.len(),
        1,
        "only the frozen cell is non-empty; a missing frozen window paints nothing"
    );
    let frozen = grid.cell_rect(1, 0);
    let (bx, by, _, _) = boxes[0];
    assert!(
        bx >= frozen.x && bx < frozen.x + frozen.width as i32,
        "the frozen value must land in the leading column ({frozen:?}), got x={bx}"
    );
    assert!(
        by >= frozen.y && by < frozen.y + frozen.height as i32,
        "and in its own row ({frozen:?}), got y={by}"
    );
}

// ── D09-VIEW-01: the leading overscan is skipped, not painted as the first cell ──

/// After a vertical scroll the first visible box must name the scrolled-to row, and a press in
/// that box must select that same global row.
///
/// # The defect this pins
///
/// `visible_window()` fetches from `row_start = scroll - overscan`, but the draw loop enumerated
/// the fetched matrix from its local row `0` — so at row 8 the first painted row was global row 6
/// (the overscan lead), not row 8. The row also never travelled the scroll in `cell_at`. Both
/// now measure the row from `scroll_row`, so the box a press resolves to is the box that was
/// painted for the scrolled-to row.
#[test]
fn a_vertical_scroll_keeps_the_first_visible_row_at_the_scroll_position() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 30,
        cols: 6,
        data: (0..30).map(|r| (0..6).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));

    grid.set_scroll_row(8);
    // The default row overscan (2) means the fetch begins two rows early.
    assert_eq!(grid.visible_window().0, 6, "the fetch origin leads the scroll by the overscan");

    // The first visible box must name global row 8, not the overscan lead row 6.
    let first = grid.cell_rect(8, 0);
    let centre = Point::new(first.x + first.width as i32 / 2, first.y + first.height as i32 / 2);
    assert_eq!(grid.cell_at(centre), Some((8, 0)));

    // And a press there selects the same global cell.
    grid.handle_event(&Event::MousePress { pos: centre, button: 1, modifiers: 0 });
    assert_eq!(grid.selection(), Some((8, 0)));
}

/// A scrolled grid paints its visible rows, not the leading overscan rows — the model-versus-
/// pixels form. As with `virtual_table`, the SVG carries glyph outlines rather than the text
/// string, so exactly one cell is made non-empty and its ink box is checked instead.
#[test]
#[cfg(all(device_profile, feature = "desktop"))]
fn the_visible_rows_are_drawn_blocked_at_the_scroll_position() {
    // The scrolled-to cell is the only non-empty one; if the leading overscan were painted first
    // the first visible box would be blank.
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    let rows = 30usize;
    let mut data: Vec<Vec<String>> = vec![vec![String::new(); 6]; rows];
    data[8][0] = "W".to_string();
    grid.set_data_source(Arc::new(StaticSource { rows, cols: 6, data }));

    grid.set_scroll_row(8);
    let first = grid.cell_rect(8, 0);
    let rect = grid.geometry();
    let svg = crate::widget::svg::render_widget_to_svg(&mut grid, rect);
    let boxes = crate::widget::svg::text_ink_boxes(&svg);
    assert_eq!(
        boxes.len(),
        1,
        "only the scrolled-to cell must be painted as text; a missing row-scroll offset paints \
         the blank lead row instead"
    );
    let (bx, by, _, _) = boxes[0];
    assert!(
        bx >= first.x && bx < first.x + first.width as i32,
        "the value must land in the scrolled-to row's first column ({first:?}), got x={bx}"
    );
    assert!(
        by >= first.y && by < first.y + first.height as i32,
        "the value must land in the scrolled-to row's box ({first:?}), got y={by}"
    );
}

#[test]
fn visible_window_includes_overscan_and_clamps() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 10,
        cols: 6,
        data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
    }));

    assert_eq!(grid.visible_window(), (0, 7, 0, 4));

    grid.set_scroll_row(8);
    grid.set_scroll_column(5);
    let window = grid.visible_window();
    assert_eq!(window.0, 6);
    assert_eq!(window.2, 4);
    assert!(window.1 <= 4);
    assert!(window.3 <= 2);
}

#[test]
fn filter_and_sort_apply_to_window_rows() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 240, 80));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 2,
        data: vec![
            vec!["u1".to_string(), "alice".to_string()],
            vec!["u2".to_string(), "bob".to_string()],
            vec!["u3".to_string(), "bruce".to_string()],
            vec!["u4".to_string(), "zoe".to_string()],
        ],
    }));
    grid.set_filters(vec![ColumnFilter { column: 1, query: "b".to_string() }]);
    grid.set_sort_specs(vec![SortSpec { column: 1, descending: true }]);

    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][1], Some("bruce".to_string()));
    assert_eq!(rows[1][1], Some("bob".to_string()));
}

#[test]
fn frozen_columns_clamp_to_column_count() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 200, 60));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 2,
        cols: 3,
        data: vec![
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["d".to_string(), "e".to_string(), "f".to_string()],
        ],
    }));

    grid.set_frozen_columns(99);
    assert_eq!(grid.frozen_columns(), 3);

    grid.clear_data_source();
    assert_eq!(grid.frozen_columns(), 0);
}

#[test]
fn new_creates_default_state() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    assert!(!grid.has_data_source());
    assert_eq!(grid.scroll_row(), 0);
    assert_eq!(grid.scroll_column(), 0);
    assert_eq!(grid.row_height(), 20);
    assert_eq!(grid.default_column_width(), 120);
    assert_eq!(grid.frozen_columns(), 0);
    assert!(grid.sort_specs().is_empty());
    assert!(grid.filters().is_empty());
    assert_eq!(grid.row_count(), 0);
    assert_eq!(grid.column_count(), 0);
    assert!(grid.fetch_visible_cells().is_empty());
}

#[test]
fn has_data_source_before_and_after() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    assert!(!grid.has_data_source());

    grid.set_data_source(Arc::new(StaticSource {
        rows: 3,
        cols: 3,
        data: vec![
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["d".to_string(), "e".to_string(), "f".to_string()],
            vec!["g".to_string(), "h".to_string(), "i".to_string()],
        ],
    }));
    assert!(grid.has_data_source());

    grid.clear_data_source();
    assert!(!grid.has_data_source());
}

#[test]
fn row_and_column_count_queries() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    assert_eq!(grid.row_count(), 0);
    assert_eq!(grid.column_count(), 0);

    grid.set_data_source(Arc::new(StaticSource { rows: 5, cols: 4, data: vec![] }));
    assert_eq!(grid.row_count(), 5);
    assert_eq!(grid.column_count(), 4);

    grid.clear_data_source();
    assert_eq!(grid.row_count(), 0);
    assert_eq!(grid.column_count(), 0);
}

#[test]
fn scroll_position_clamping() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 10,
        cols: 6,
        data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
    }));

    grid.set_scroll_row(5);
    assert_eq!(grid.scroll_row(), 5);

    grid.set_scroll_row(999);
    assert_eq!(grid.scroll_row(), 9);

    grid.set_scroll_column(3);
    assert_eq!(grid.scroll_column(), 3);

    grid.set_scroll_column(999);
    assert_eq!(grid.scroll_column(), 5);
}

#[test]
fn clear_data_source_resets_state() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource { rows: 5, cols: 3, data: vec![] }));
    grid.set_scroll_row(2);
    grid.set_scroll_column(1);
    grid.set_frozen_columns(2);
    grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);

    grid.clear_data_source();
    assert!(!grid.has_data_source());
    assert_eq!(grid.scroll_row(), 0);
    assert_eq!(grid.scroll_column(), 0);
    assert_eq!(grid.frozen_columns(), 0);
    assert!(grid.fetch_visible_cells().is_empty());
}

#[test]
fn sort_specs_set_get_invalidate_cache() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 3,
        cols: 2,
        data: vec![
            vec!["c".to_string(), "x".to_string()],
            vec!["a".to_string(), "y".to_string()],
            vec!["b".to_string(), "z".to_string()],
        ],
    }));

    assert!(grid.sort_specs().is_empty());

    let specs = vec![SortSpec { column: 0, descending: false }];
    grid.set_sort_specs(specs.clone());
    assert_eq!(grid.sort_specs().len(), 1);
    assert_eq!(grid.sort_specs()[0].column, 0);

    // Fetch with sort applied
    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0], Some("a".to_string()));
    assert_eq!(rows[1][0], Some("b".to_string()));
    assert_eq!(rows[2][0], Some("c".to_string()));

    // Change sort - should clear cache
    grid.set_sort_specs(vec![SortSpec { column: 0, descending: true }]);
    let rows2 = grid.fetch_visible_cells();
    assert_eq!(rows2[0][0], Some("c".to_string()));
    assert_eq!(rows2[2][0], Some("a".to_string()));

    grid.set_sort_specs(Vec::new());
    assert!(grid.sort_specs().is_empty());
}

#[test]
fn filters_set_get_invalidate_cache() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 2,
        data: vec![
            vec!["apple".to_string(), "red".to_string()],
            vec!["banana".to_string(), "yellow".to_string()],
            vec!["cherry".to_string(), "red".to_string()],
            vec!["date".to_string(), "brown".to_string()],
        ],
    }));

    assert!(grid.filters().is_empty());

    let filters = vec![ColumnFilter { column: 1, query: "red".to_string() }];
    grid.set_filters(filters.clone());
    assert_eq!(grid.filters().len(), 1);

    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0], Some("apple".to_string()));
    assert_eq!(rows[1][0], Some("cherry".to_string()));

    // Change filter - should recalc
    grid.set_filters(vec![ColumnFilter { column: 0, query: "b".to_string() }]);
    let rows2 = grid.fetch_visible_cells();
    assert_eq!(rows2.len(), 1);
    assert_eq!(rows2[0][0], Some("banana".to_string()));

    grid.set_filters(Vec::new());
    assert!(grid.filters().is_empty());
}

#[test]
fn frozen_columns_set_get() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource { rows: 2, cols: 5, data: vec![] }));

    assert_eq!(grid.frozen_columns(), 0);

    grid.set_frozen_columns(3);
    assert_eq!(grid.frozen_columns(), 3);

    // Clamped to column count
    grid.set_frozen_columns(999);
    assert_eq!(grid.frozen_columns(), 5);
}

#[test]
fn visible_window_changed_signal_emission() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 10,
        cols: 6,
        data: (0..10).map(|r| (0..6).map(|c| format!("{}:{}", r, c)).collect()).collect(),
    }));

    let emitted = Arc::new(Mutex::new(false));
    let sink = emitted.clone();
    grid.visible_window_changed.connect(move |_win| {
        *sink.lock().unwrap() = true;
    });

    grid.set_scroll_row(3);
    assert!(*emitted.lock().unwrap());
}

#[test]
fn empty_source_returns_empty_window() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource { rows: 0, cols: 0, data: vec![] }));

    assert_eq!(grid.visible_window(), (0, 0, 0, 0));
    assert!(grid.fetch_visible_cells().is_empty());
}

struct RevisionSource {
    rows: usize,
    cols: usize,
    rev: AtomicU64,
    changed: GenericSignal,
    call_count: AtomicUsize,
}

impl RevisionSource {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            rev: AtomicU64::new(1),
            changed: GenericSignal::new(),
            call_count: AtomicUsize::new(0),
        }
    }

    fn bump(&self) {
        self.rev.fetch_add(1, Ordering::Relaxed);
        self.changed.emit();
    }

    /// A revision-less source: announces change without versioning the projection.
    fn with_revision(rows: usize, cols: usize, revision: u64) -> Self {
        let source = Self::new(rows, cols);
        source.rev.store(revision, Ordering::Relaxed);
        source
    }

    fn touch(&self) {
        self.changed.emit();
    }
}

impl IncrementalTableDataSource for RevisionSource {
    fn row_count(&self) -> usize {
        self.rows
    }

    fn column_count(&self) -> usize {
        self.cols
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        Some(format!("{}:{}", row, column))
    }

    fn revision(&self) -> u64 {
        self.rev.load(Ordering::Relaxed)
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.changed)
    }
}

#[test]
fn cache_invalidation_on_revision_change() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
    let source = Arc::new(RevisionSource::new(10, 4));
    grid.set_data_source(source.clone());

    let _a = grid.fetch_visible_cells();
    let calls_after_a = source.call_count.load(Ordering::Relaxed);
    assert!(calls_after_a > 0);

    // Second fetch should hit cache
    let _b = grid.fetch_visible_cells();
    assert_eq!(source.call_count.load(Ordering::Relaxed), calls_after_a);

    // Bump revision -> cache invalidated
    source.bump();
    let _c = grid.fetch_visible_cells();
    assert!(source.call_count.load(Ordering::Relaxed) > calls_after_a);
}

/// The trait's `revision()` default is `0`; a source that reports change only through its
/// signal must still be cacheable, and that signal must invalidate the window.
#[test]
fn a_revision_less_source_is_cached_and_invalidated_by_its_signal() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 240, 60));
    let source = Arc::new(RevisionSource::with_revision(10, 4, 0));
    grid.set_data_source(source.clone());

    let first = grid.fetch_visible_cells_shared();
    let calls_after_first = source.call_count.load(Ordering::Relaxed);
    assert!(calls_after_first > 0);

    let second = grid.fetch_visible_cells_shared();
    assert!(Rc::ptr_eq(&first, &second), "the cached window is shared, not re-fetched");
    assert_eq!(
        source.call_count.load(Ordering::Relaxed),
        calls_after_first,
        "the signal makes this source observable"
    );

    source.touch();
    let _ = grid.fetch_visible_cells_shared();
    assert!(
        source.call_count.load(Ordering::Relaxed) > calls_after_first,
        "a reported change must invalidate the window"
    );
}

#[test]
fn row_height_and_column_width_minimum_clamp() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 800, 600));
    grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 3, data: vec![] }));

    grid.set_row_height(0);
    assert_eq!(grid.row_height(), 1);

    grid.set_row_height(25);
    assert_eq!(grid.row_height(), 25);

    grid.set_default_column_width(0);
    assert_eq!(grid.default_column_width(), 1);

    grid.set_default_column_width(100);
    assert_eq!(grid.default_column_width(), 100);
}

/// N-V-03 / N-V-04: a full-width control with a full-height row must not overflow the
/// `height + row_height - 1` capacity computation.
#[test]
fn extreme_dimensions_do_not_overflow_visible_capacity() {
    // A very wide/tall control with an enormous row/column height: the old
    // `height + row_height - 1` and `width + column_width - 1` overflowed (debug panic).
    let mut grid = DataGrid::new(Rect::new(0, 0, u32::MAX, u32::MAX));
    grid.set_row_height(u32::MAX);
    grid.set_default_column_width(u32::MAX);
    // Reaching the assertions means neither capacity calculation panicked.
    assert_eq!(grid.visible_row_capacity(), 1);
    assert_eq!(grid.visible_column_capacity(), 1);
}

/// One wheel notch scrolls one row, whether the host sends the documented
/// notch delta or the legacy 120-unit tick.
#[test]
fn wheel_scrolls_one_row_per_notch() {
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    grid.set_data_source(Arc::new(StaticSource { rows: 10, cols: 2, data: vec![] }));

    grid.handle_event(&Event::wheel(0, -120, 0));
    assert_eq!(grid.scroll_row(), 1, "one legacy notch is one row");

    grid.handle_event(&Event::wheel(0, -1, 0));
    assert_eq!(grid.scroll_row(), 2, "one documented notch is one row");

    grid.handle_event(&Event::wheel(0, 120, 0));
    assert_eq!(grid.scroll_row(), 1, "the opposite notch scrolls back one row");
}

/// Full-model projection, header sorting, column modes, alternating rows and keyboard
/// navigation are wired.
#[test]
fn full_model_projection_header_sort_and_keyboard_are_wired() {
    let data = vec![vec!["b".to_string()], vec!["c".to_string()], vec!["a".to_string()]];
    let mut grid = DataGrid::new(Rect::new(0, 0, 300, 120));
    grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 1, data }));
    grid.set_full_model_projection(true);
    grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);
    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.first().and_then(|row| row.first().cloned()), Some(Some("a".to_string())));

    grid.set_column_width_mode(ColumnWidthMode::Stretch);
    assert_eq!(grid.column_width_mode(), ColumnWidthMode::Stretch);
    grid.set_alternating_rows(true);
    assert!(grid.alternating_rows());
    grid.set_show_scrollbars(true);
    assert!(grid.show_scrollbars());

    let header = grid.header_band();
    let clicked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
    let sink = std::sync::Arc::clone(&clicked);
    grid.header_clicked.connect(move |column| {
        sink.store(*column, std::sync::atomic::Ordering::SeqCst);
    });
    grid.handle_event(&Event::MousePress {
        pos: Point::new(header.x + 1, header.y + 1),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(
        clicked.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "a header click names its column"
    );

    grid.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
    grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
    assert_eq!(grid.current_cell(), Some((1, 0)), "Down moves the current cell one row");
    grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
    assert_eq!(grid.current_cell(), Some((2, 0)), "Down reaches the last row");
    let _ = grid.fetch_visible_cells();
}

/// F-P01: the column prefix table is built once per width revision, not summed per cell.
#[test]
fn column_prefix_width_is_cached_until_a_width_changes() {
    let mut grid = static_grid(50, 5);
    grid.set_column_width_mode(ColumnWidthMode::Interactive);
    assert!(grid.set_column_width(1, 80));
    let expected = 120 + 80 + 120 + 120;
    assert_eq!(grid.column_prefix_width(4), expected);
    let builds = grid.column_prefix_rebuilds.get();
    // Repeated reads are table lookups: no rebuild.
    for _ in 0..100 {
        assert_eq!(grid.column_prefix_width(4), expected);
        assert_eq!(grid.column_prefix_width(2), 200);
    }
    assert_eq!(grid.column_prefix_rebuilds.get(), builds, "the table was reused");

    // A width change rebuilds exactly once, and the new widths are visible immediately.
    assert!(grid.set_column_width(1, 50));
    assert_eq!(grid.column_prefix_width(4), 120 + 50 + 120 + 120);
    assert_eq!(grid.column_prefix_rebuilds.get(), builds + 1);
}

/// F-P02: the draw path's view addresses the cached projection instead of cloning the window.
#[test]
fn draw_view_shares_the_projection_across_scroll_positions() {
    let mut grid = static_grid(200, 4);
    // The shared-projection view is the explicit full-model mode; the default virtualized
    // mode fetches only the window (V-001).
    grid.set_full_model_projection(true);
    let first = grid.fetch_visible_window_view();
    assert!(!first.is_empty());
    let same_projection = Rc::clone(&first.rows);

    grid.set_scroll_row(10);
    let second = grid.fetch_visible_window_view();
    assert!(
        Rc::ptr_eq(&same_projection, &second.rows),
        "a scroll reuses the same projected matrix instead of slicing a copy"
    );
    assert_ne!(first.row_start, second.row_start, "the window did move");
    // The window starts at `scroll - overscan`, so the viewport's first row is index 2.
    assert_eq!(second.row(2).and_then(|row| row[0].clone()), Some("10:0".to_string()));
}

/// F-P03: a source with neither a revision nor a signal is projected once, and
/// `refresh_data` is the explicit invalidation point.
#[test]
fn non_observable_projection_is_cached_until_refresh_data() {
    struct CountingSource {
        rows: usize,
        columns: usize,
        fetches: Arc<AtomicU64>,
    }
    impl IncrementalTableDataSource for CountingSource {
        fn row_count(&self) -> usize {
            self.rows
        }
        fn column_count(&self) -> usize {
            self.columns
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{row}:{column}"))
        }
        fn fetch_window(
            &self,
            row_start: usize,
            row_len: usize,
            column_start: usize,
            column_len: usize,
        ) -> Vec<Vec<Option<String>>> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            (row_start..row_start.saturating_add(row_len).min(self.rows))
                .map(|row| {
                    (column_start..column_start.saturating_add(column_len).min(self.columns))
                        .map(|column| Some(format!("{row}:{column}")))
                        .collect()
                })
                .collect()
        }
    }

    let fetches = Arc::new(AtomicU64::new(0));
    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 160));
    // The full-model projection is what caches a non-observable source; without it the
    // virtualized mode refetches its window by design (V-001).
    grid.set_full_model_projection(true);
    grid.set_data_source(Arc::new(CountingSource {
        rows: 500,
        columns: 3,
        fetches: Arc::clone(&fetches),
    }));
    assert!(!grid.data_is_observable, "the fixture has no invalidation contract");

    let _ = grid.fetch_visible_cells();
    let after_first = fetches.load(Ordering::Relaxed);
    assert!(after_first > 0);

    grid.set_scroll_row(5);
    let _ = grid.fetch_visible_cells();
    grid.set_scroll_row(9);
    let _ = grid.fetch_visible_cells();
    assert_eq!(
        fetches.load(Ordering::Relaxed),
        after_first,
        "scrolling must hit the cached projection"
    );

    grid.refresh_data();
    let _ = grid.fetch_visible_cells();
    assert!(
        fetches.load(Ordering::Relaxed) > after_first,
        "refresh_data is how a non-observable source reports change"
    );
}

/// F-P04: elided header labels are cached and reused across frames.
#[test]
fn header_labels_are_elided_once_and_cached() {
    let mut grid = static_grid(4, 3);
    grid.set_header_labels(&["Alpha", "Beta", "Gamma"]);
    let _ = crate::widget::svg::render_to_svg(&mut grid);
    let cached = grid.header_text_cache.borrow();
    assert_eq!(cached.entries.get(&(0, 120)).map(String::as_str), Some("Alpha"));
    assert_eq!(cached.entries.get(&(2, 120)).map(String::as_str), Some("Gamma"));

    // A label change invalidates the cache; the next paint stores the new title.
    drop(cached);
    grid.set_header_labels(&["Renamed", "Beta", "Gamma"]);
    let _ = crate::widget::svg::render_to_svg(&mut grid);
    let cached = grid.header_text_cache.borrow();
    assert_eq!(cached.entries.get(&(0, 120)).map(String::as_str), Some("Renamed"));
}

/// V-001/V-002: the default is a virtualized index projection, and its visible order is the
/// same as the explicit full-model projection.
#[test]
fn the_default_projection_is_virtualized_and_matches_the_full_order() {
    let data = vec![vec!["c".to_string()], vec!["a".to_string()], vec!["b".to_string()]];
    let mut grid = DataGrid::new(Rect::new(0, 0, 300, 120));
    grid.set_data_source(Arc::new(StaticSource { rows: 3, cols: 1, data }));
    assert!(!grid.full_model_projection(), "virtualized rendering is the default");
    grid.set_sort_specs(vec![SortSpec { column: 0, descending: false }]);

    let virtualized = grid.fetch_visible_cells();
    assert!(grid.full_projection_cache.is_none(), "no whole-model row matrix is held");
    let index = Rc::clone(&grid.index_projection_cache.as_ref().expect("index cache").1);
    assert_eq!(*index, vec![1, 2, 0], "the projection is an index, not cloned rows");
    assert_eq!(virtualized[0][0], Some("a".to_string()));

    grid.set_full_model_projection(true);
    let materialised = grid.fetch_visible_cells();
    assert_eq!(virtualized, materialised, "both strategies paint the same rows in the same order");
}

/// V-003: a header click cycles ascending -> descending -> unsorted.
#[test]
fn a_header_click_cycles_three_sort_states() {
    let mut grid = static_grid(3, 1);
    let header = grid.header_band();
    let pos = Point::new(header.x + 2, header.y + 2);

    grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
    assert_eq!(grid.sort_specs(), &[SortSpec { column: 0, descending: false }]);
    grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
    assert_eq!(grid.sort_specs(), &[SortSpec { column: 0, descending: true }]);
    grid.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
    assert!(grid.sort_specs().is_empty(), "the third state restores the source order");
}

/// V-004: row and column membership is a set lookup, and the stored unit is the row or the
/// column rather than every covered cell.
#[test]
fn row_and_column_selection_are_one_unit_lookups() {
    let mut grid = static_grid(4, 3);
    grid.set_selection_behavior(DataGridSelectionBehavior::Rows);
    grid.apply_selection_click(1, 2, 0);
    assert_eq!(grid.selected_cells(), vec![(1, 0)], "a row unit is (row, 0)");
    assert!(grid.selection_contains(1, 0));
    assert!(grid.selection_contains(1, 2), "every column of the row is selected");
    assert!(!grid.selection_contains(2, 0));

    grid.set_selection_behavior(DataGridSelectionBehavior::Columns);
    grid.apply_selection_click(2, 1, 0);
    assert_eq!(grid.selected_cells(), vec![(0, 1)], "a column unit is (0, column)");
    assert!(grid.selection_contains(0, 1));
    assert!(grid.selection_contains(3, 1));
    assert!(!grid.selection_contains(0, 2));
}

/// V-014: a hidden column occupies no width, closes the gap, and is skipped by the hit test.
#[test]
fn hiding_a_column_removes_its_slot_without_renumbering() {
    let mut grid = static_grid(3, 3);
    assert_eq!(grid.column_view_x(2), 240);
    assert!(grid.set_column_hidden(1, true));
    assert_eq!(grid.column_width_at(1), 0);
    assert_eq!(grid.column_view_x(2), 120, "the following column closes the gap");
    assert_eq!(grid.column_at_dx(130), Some(2), "a press past the gap names column 2");
    assert_eq!(grid.hidden_columns(), vec![1]);
    assert!(grid.set_column_hidden(1, false));
    assert_eq!(grid.column_view_x(2), 240);
    assert_eq!(grid.hidden_columns(), Vec::<usize>::new());
}

/// V-014: a merged run answers as its origin, at the paint geometry and the hit test.
#[test]
fn a_merged_run_resolves_to_its_origin() {
    let mut grid = static_grid(3, 3);
    assert!(grid.set_cell_span(0, 0, 2, 2));
    assert_eq!(grid.cell_spans().len(), 1);
    let inside = grid.cell_rect(1, 1);
    let centre = Point::new(inside.x + 1, inside.y + 1);
    assert_eq!(grid.cell_at(centre), Some((0, 0)));
    assert_eq!(grid.cell_span(1, 1), Some(CellSpan::new(0, 0, 2, 2)));
    assert_eq!(grid.cell_span(2, 2), None);
    assert!(!grid.set_cell_span(99, 0, 1, 1), "an anchor outside the model is refused");
    grid.clear_cell_spans();
    assert!(grid.cell_spans().is_empty());
}

/// V-014: each extra header level takes another title strip, and removing it restores the
/// historical geometry.
#[test]
fn extra_header_levels_take_their_own_strips() {
    let mut grid = static_grid(3, 3);
    let base_y = grid.cells_box().y;
    grid.set_extra_header_rows(vec![vec!["Top".to_string()]]);
    assert_eq!(grid.header_strip_height(), GRID_HEADER_HEIGHT * 2);
    assert_eq!(grid.cells_box().y, base_y + GRID_HEADER_HEIGHT as i32);
    assert_eq!(grid.header_band().height, GRID_HEADER_HEIGHT * 2);
    grid.set_extra_header_rows(Vec::new());
    assert_eq!(grid.cells_box().y, base_y);
    assert_eq!(grid.header_band().height, GRID_HEADER_HEIGHT);
}

/// V-014: the tooltip callback is asked per cell and is removed by `clear_cell_tooltip`.
#[test]
fn a_cell_tooltip_comes_from_the_installed_callback() {
    let mut grid = static_grid(3, 3);
    grid.set_cell_tooltip(Arc::new(|row, column| {
        (row == 1 && column == 2).then(|| "the tip".to_string())
    }));
    assert_eq!(grid.cell_tooltip(1, 2).as_deref(), Some("the tip"));
    assert_eq!(grid.cell_tooltip(0, 0), None);
    grid.clear_cell_tooltip();
    assert_eq!(grid.cell_tooltip(1, 2), None);
}

/// V-014: a delegate is consulted for each painted cell, and its `false` answer keeps the
/// direct rendering.
#[test]
fn a_cell_delegate_is_invoked_for_painted_cells() {
    struct CountingDelegate(std::sync::Arc<AtomicUsize>);
    impl DataGridCellDelegate for CountingDelegate {
        fn paint(
            &self,
            _context: &mut RenderContext,
            _cell: Rect,
            _row: usize,
            _column: usize,
            _text: Option<&str>,
        ) -> bool {
            self.0.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mut grid = static_grid(3, 3);
    grid.set_cell_delegate(Arc::new(CountingDelegate(Arc::clone(&calls))));
    assert!(grid.has_cell_delegate());
    let _ = crate::widget::svg::render_to_svg(&mut grid);
    assert!(calls.load(Ordering::Relaxed) > 0, "the delegate is called by the paint path");
    grid.clear_cell_delegate();
    assert!(!grid.has_cell_delegate());
}

// ─── COMP-A8: companion wiring ───

fn header_with_titles(titles: &[&str]) -> GridViewHeader {
    let mut header = GridViewHeader::new(Rect::new(0, 0, 600, 22));
    header.set_columns(titles.iter().map(|title| ColumnSpec::new(*title, 120)).collect::<Vec<_>>());
    header
}

/// COMP-A8: attaching pushes the counts, a later move is polled in, and detaching stops the
/// link without rewinding the strip.
#[test]
fn the_status_bar_follows_the_grids_counts() {
    use crate::widget::companions::GridStatusBar;

    let mut grid = static_grid(4, 2);
    let bar = GridStatusBar::new(Rect::new(0, 0, 300, 28));
    grid.attach_grid_status_bar(&bar);
    assert!(grid.is_grid_status_bar_attached());
    assert_eq!(bar.row_count(), 4, "attach pushes the current row count");
    assert_eq!(bar.selected_count(), 0);
    assert!(!grid.poll_grid_status_bar(), "a steady grid changes nothing");

    grid.select_all();
    assert!(grid.poll_grid_status_bar(), "the selection moved");
    assert_eq!(bar.selected_count(), 4);
    assert!(!grid.poll_grid_status_bar(), "the second poll has nothing to do");

    grid.detach_grid_status_bar();
    assert!(!grid.is_grid_status_bar_attached());
    grid.set_data_source(Arc::new(StaticSource {
        rows: 9,
        cols: 2,
        data: (0..9).map(|r| (0..2).map(|c| format!("{r}:{c}")).collect()).collect(),
    }));
    assert!(!grid.poll_grid_status_bar(), "a detached bar is not polled");
    assert_eq!(bar.row_count(), 4, "detach leaves the last counts");
}

/// COMP-A8: the row's aggregates come from the projected rows and follow a filter.
#[test]
fn the_summary_row_follows_the_projected_rows() {
    use crate::widget::companions::{ColumnAggregate, ColumnAggregateKind, SummaryRow};

    let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 3,
        cols: 2,
        data: vec![
            vec!["1".to_string(), "10".to_string()],
            vec!["2".to_string(), "20".to_string()],
            vec!["3".to_string(), "30".to_string()],
        ],
    }));
    let header = header_with_titles(&["Qty", "Price"]);
    grid.set_header(&header);
    let qty = ColumnSpec::new("Qty", 120).to_token();
    let price = ColumnSpec::new("Price", 120).to_token();

    let mut row = SummaryRow::new(Rect::new(0, 0, 400, 32));
    row.set_aggregates(vec![
        ColumnAggregate::new(qty.clone(), ColumnAggregateKind::Sum, "Qty"),
        ColumnAggregate::new(price.clone(), ColumnAggregateKind::Average, "Price"),
    ]);

    grid.attach_summary_row(&row);
    assert!(grid.is_summary_row_attached());
    assert_eq!(row.value(&qty), Some(6.0), "attach pushes the current sum");
    assert_eq!(row.value(&price), Some(20.0), "and the current average");
    assert!(!grid.poll_summary_row(), "a steady projection is not recomputed");

    grid.set_filters(vec![ColumnFilter { column: 0, query: "1".to_string() }]);
    assert!(grid.poll_summary_row(), "the filter moved the projection");
    assert_eq!(row.value(&qty), Some(1.0), "the sum follows the visible rows");
    assert_eq!(row.value(&price), Some(10.0));
    assert!(!grid.poll_summary_row(), "the second poll has nothing to do");

    grid.detach_summary_row();
    assert!(!grid.is_summary_row_attached());
    grid.set_filters(Vec::new());
    assert!(!grid.poll_summary_row(), "a detached row is not polled");
    assert_eq!(row.value(&qty), Some(1.0), "detach leaves the last values");
}

/// COMP-A8: sort, filter, hide and chooser requests are applied by the poll.
#[test]
fn the_header_menu_requests_reach_the_grid() {
    use crate::widget::companions::{Direction, HeaderMenu};

    let mut grid = static_grid(4, 2);
    let header = header_with_titles(&["First", "Second"]);
    grid.set_header(&header);
    let first = header.columns()[0].to_token();
    let anchor = Rect::new(100, 0, 120, 22);
    let mut menu = HeaderMenu::new(Rect::new(0, 0, 320, 240));

    grid.attach_header_menu(&menu);
    assert!(grid.is_header_menu_attached());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Idle, "nothing is queued");

    // Row 0 is "Sort ascending" while no sort is active.
    menu.open_for(first.clone(), anchor);
    assert!(menu.activate_selected());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Applied);
    let specs = grid.sort_specs();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0], SortSpec { column: 0, descending: false });
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Idle, "the queue is drained");

    // With a sort active the rows are Asc, Desc, Clear, Filter, Hide, Choose.
    menu.open_for(first.clone(), anchor);
    menu.set_filter_text("1");
    for _ in 0..3 {
        menu.move_selection(Direction::Next);
    }
    assert!(menu.activate_selected());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Applied);
    assert_eq!(grid.fetch_visible_cells().len(), 1, "only the row containing 1 survives");

    menu.open_for(first.clone(), anchor);
    for _ in 0..4 {
        menu.move_selection(Direction::Next);
    }
    assert!(menu.activate_selected());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Applied);
    assert!(grid.is_column_hidden(0), "Hide column reaches the grid");

    menu.open_for(first.clone(), anchor);
    for _ in 0..5 {
        menu.move_selection(Direction::Next);
    }
    assert!(menu.activate_selected());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::ColumnChooserRequested);
    assert!(grid.take_column_chooser_request(), "the host can drain the chooser intent");
    assert!(!grid.take_column_chooser_request(), "the intent is reported once");

    grid.detach_header_menu();
    assert!(!grid.is_header_menu_attached());
    assert!(!menu.activate_selected(), "the menu is closed already");
    menu.open_for(first, anchor);
    assert!(menu.activate_selected());
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Idle, "a detached menu reports nothing");
    assert!(grid.is_column_hidden(0), "the last applied hide stays");
}

/// COMP-A8: the menu's operator reaches the grid's filter expression unchanged.
#[test]
fn a_header_menu_filter_keeps_its_operator() {
    use crate::widget::companions::HeaderMenu;

    let mut grid = DataGrid::new(Rect::new(0, 0, 300, 200));
    grid.set_data_source(Arc::new(StaticSource {
        rows: 4,
        cols: 1,
        data: (1..=4).map(|value| vec![value.to_string()]).collect(),
    }));
    let header = header_with_titles(&["N"]);
    grid.set_header(&header);
    let token = header.columns()[0].to_token();
    let mut menu = HeaderMenu::new(Rect::new(0, 0, 320, 240));
    grid.attach_header_menu(&menu);

    menu.open_for(token, Rect::new(0, 0, 100, 22));
    menu.set_filter_text("2");
    menu.filter_requested.emit(FilterOperator::GreaterThan);
    assert_eq!(grid.poll_header_menu(), HeaderMenuPoll::Applied);
    let rows = grid.fetch_visible_cells();
    assert_eq!(rows.len(), 2, "greater than 2 keeps 3 and 4");
    assert_eq!(rows[0][0], Some("3".to_string()));
}
