// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DataGrid` event dispatch: pointer, wheel and keyboard gestures.

use super::*;

impl crate::event::EventHandler for DataGrid {
    fn handle_event(&mut self, event: &Event) {
        // A page click on the bound bar is a shared-cell write, so it is drained here before the
        // event is routed: the frame that follows the click already shows the new page.
        self.poll_pagination();
        // A chooser click is likewise a shared write, drained before routing (V-015).
        self.poll_column_chooser();
        // A filter-row edit is a shared write as well (chapter 3, gap 2).
        self.poll_filter_row();
        // A companion request is likewise a shared write, so it is drained before the event is
        // routed and the frame that follows already reflects it (COMP-A8).
        self.poll_grid_status_bar();
        self.poll_summary_row();
        self.poll_header_menu();
        // A row the bound find bar landed on is applied before the event is routed, so the frame
        // that follows the match already shows the moved current cell (U3-41).
        self.poll_find_bar();
        // The base keeps the control-level hover/press facts and answers `widget_state()`. This
        // handler did not forward to it, so `"data_grid:hover"` could never fire.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::Wheel { delta, modifiers } = event {
            // One notch is one row. `WheelScroll` owns the notch normalization, so a
            // legacy 120-unit delta and a modern notch delta both mean a single row.
            let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
            let lines = scroll.lines_y.unsigned_abs() as usize;
            let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                .contains(crate::shortcut::Modifiers::SHIFT);
            if shift {
                // Shift makes the wheel horizontal: it **replaces** the axis rather than adding
                // a second scroll, which used to move the grid on both axes for one notch.
                if scroll.lines_y < 0 {
                    let next = self.scroll_column.saturating_add(lines);
                    self.set_scroll_column(next);
                } else if scroll.lines_y > 0 {
                    let up = self.scroll_column.saturating_sub(lines);
                    self.set_scroll_column(up);
                }
            } else {
                if scroll.lines_y < 0 {
                    let next = self.scroll_row.saturating_add(lines);
                    self.set_scroll_row(next);
                } else if scroll.lines_y > 0 {
                    let up = self.scroll_row.saturating_sub(lines);
                    self.set_scroll_row(up);
                }
                if scroll.columns_x != 0 {
                    let next = self.scroll_column.saturating_add_signed(scroll.columns_x as isize);
                    self.set_scroll_column(next);
                }
            }
        }

        // A stylus press is a press: the base records it identically (D09-POINTER-01), and this
        // control used to answer only the mouse variant, so a pen could not select a cell.
        let press = match event {
            Event::MousePress { pos, button, modifiers } => Some((*pos, *button, *modifiers)),
            Event::PointerPress { pos, button, .. } => Some((*pos, *button, 0)),
            _ => None,
        };
        if let Some((pos, button, modifiers)) = press {
            if button != 1 {
                return;
            }
            if let Some(column) = self.resize_handle_at(pos) {
                self.resizing_column = Some(column);
                self.resize_start_x = pos.x;
                self.resize_start_width = self.column_width_at(column);
                return;
            }
            if let Some(column) = self.header_column_at(pos) {
                self.header_clicked.emit(column);
                // `ColumnSpec::sortable` is the section's declared capability, and the reusable
                // header refuses to cycle a section that does not declare it. The grid consulted
                // no spec at all, so a click sorted a column the host had marked inert.
                if !self.header_columns.get(column).is_none_or(|spec| spec.sortable) {
                    return;
                }
                // The header cycles ascending -> descending -> unsorted, matching
                // `GridViewHeader::toggle_sort` (V-003). The third state restores the source
                // order, which a two-state toggle could never reach.
                let existing = self.sort_specs.iter().position(|spec| spec.column == column);
                let descending = match existing {
                    Some(index) => {
                        if self.sort_specs[index].descending {
                            self.sort_specs.remove(index);
                            None
                        } else {
                            self.sort_specs[index].descending = true;
                            Some(true)
                        }
                    }
                    None => {
                        self.sort_specs.push(SortSpec { column, descending: false });
                        Some(false)
                    }
                };
                self.invalidate_projection_cache();
                self.clear_cache();
                // `sort_changed` still carries `(column, descending)`; the unsorted state is
                // observable through `sort_specs()` and the arrow's absence. It is deliberately
                // not signalled as `(column, false)`, which would claim an ascending sort.
                if let Some(descending) = descending {
                    self.sort_changed.emit((column, descending));
                }
                self.base.request_redraw();
                return;
            }
            // The press resolves through `cell_at`, which reads `cell_rect` — the same
            // derivation the draw pass uses — so a selection can only name a cell that was
            // actually painted. A press in the column-title strip or in a partial trailing
            // column resolves to nothing.
            let hit = self.cell_at(pos);
            self.current_cell = hit;
            match hit {
                Some((row, column)) => {
                    self.cell_clicked.emit((row, column));
                    self.apply_selection_click(row, column, modifiers);
                    self.current_changed.emit((row, column));
                }
                None => {
                    if self.selection.is_some() || !self.selected_cells.is_empty() {
                        self.clear_selection();
                    }
                }
            }
            self.base.request_redraw();
        }

        match event {
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some((row, column)) = self.cell_at(*pos) {
                    self.cell_double_clicked.emit((row, column));
                    self.apply_selection_click(row, column, 0);
                    self.current_cell = Some((row, column));
                    self.current_changed.emit((row, column));
                    if self.edit_triggers.contains(EditTriggers::DOUBLE_CLICKED) {
                        self.edit_requested.emit((row, column));
                    }
                    self.base.request_redraw();
                }
            }
            Event::ContextMenu { .. } => {
                // Answered by `Widget::context_menu_event`, which selects the cell under the
                // pointer before emitting; an arm here would emit a duplicate, stale request.
            }
            Event::KeyPress { key, modifiers } if self.base.focus_reason().is_some() => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                // Ctrl+F asks the host to open the attached find bar (U3-41). The grid cannot own
                // the bar, so the request travels as an event; the check precedes the row/column
                // guard so an empty grid still opens its search surface.
                if *key == crate::event::key_codes::F
                    && modifiers.contains(crate::shortcut::Modifiers::CTRL)
                {
                    self.find_requested.emit(());
                    return;
                }
                let page = self.visible_row_capacity().max(1);
                // The paint pass enumerates the filtered/sorted projection, so keyboard bounds
                // must use its length; the source count let a key walk onto rows nothing painted.
                let projected_rows = self.projected_row_count_mut();
                if projected_rows == 0 || self.column_count() == 0 {
                    return;
                }
                let last_row = projected_rows.saturating_sub(1);
                let last_column = self.column_count().saturating_sub(1);
                let (mut row, mut column) = self
                    .current_cell
                    .or(self.selection)
                    .unwrap_or((self.scroll_row, self.scroll_column));
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT)
                    && self.selection_mode
                        == crate::widget::view_widgets::list_view::SelectionMode::Extended;
                match *key {
                    crate::event::key_codes::UP => row = row.saturating_sub(1),
                    crate::event::key_codes::DOWN => row = row.saturating_add(1).min(last_row),
                    crate::event::key_codes::LEFT => column = column.saturating_sub(1),
                    crate::event::key_codes::RIGHT => {
                        column = column.saturating_add(1).min(last_column)
                    }
                    crate::event::key_codes::PAGE_UP => row = row.saturating_sub(page),
                    crate::event::key_codes::PAGE_DOWN => {
                        row = row.saturating_add(page).min(last_row)
                    }
                    crate::event::key_codes::HOME => column = 0,
                    crate::event::key_codes::END => column = last_column,
                    crate::event::key_codes::ENTER
                        if self.edit_triggers.contains(EditTriggers::EDIT_KEY) =>
                    {
                        if let Some(cell) = self.current_cell.or(self.selection) {
                            self.edit_requested.emit(cell);
                        }
                        return;
                    }
                    _ => return,
                }
                self.move_current_cell(row, column, shift);
            }
            // Cell hover, from the same `cell_at` the press reads: the cell that is outlined is the
            // cell a click would select. A pointer that leaves the cells (into the title strip, past
            // the last column, or off the control) clears it rather than latching the last cell it
            // crossed.
            Event::MouseMove { pos } | Event::PointerMove { pos, .. } => {
                if let Some(column) = self.resizing_column {
                    let delta = pos.x - self.resize_start_x;
                    let width = (self.resize_start_width as i32 + delta).max(1) as u32;
                    if self.store_column_width(column, width) {
                        self.base.request_redraw();
                    }
                    return;
                }
                let hovered = self.cell_at(*pos);
                if hovered != self.hovered_cell {
                    self.hovered_cell = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseRelease { button: 1, .. } | Event::PointerRelease { button: 1, .. } => {
                if let Some(column) = self.resizing_column.take() {
                    self.column_resized.emit((column, self.column_width_at(column)));
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                let had_hover = self.hovered_cell.take().is_some();
                if had_hover {
                    self.base.request_redraw();
                }
            }
            _ => { /* The wheel and press arms above already handled those. */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventHandler;

    struct Source {
        data: Vec<Vec<String>>,
    }

    impl IncrementalTableDataSource for Source {
        fn row_count(&self) -> usize {
            self.data.len()
        }

        fn column_count(&self) -> usize {
            self.data.first().map_or(0, Vec::len)
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.data.get(row).and_then(|line| line.get(column)).cloned()
        }
    }

    fn grid(rows: usize, columns: usize) -> DataGrid {
        let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
        grid.set_data_source(Arc::new(Source {
            data: (0..rows)
                .map(|row| (0..columns).map(|column| format!("{row}:{column}")).collect())
                .collect(),
        }));
        grid
    }

    fn centre(grid: &DataGrid, row: usize, column: usize) -> Point {
        let cell = grid.cell_rect(row, column);
        Point::new(cell.x + cell.width as i32 / 2, cell.y + cell.height as i32 / 2)
    }

    /// A stylus press/move/release is the same gesture as the mouse's (D09-POINTER-01); the grid
    /// used to answer only the mouse variants, so a pen could not select or hover a cell.
    #[test]
    fn pointer_events_drive_the_same_gestures_as_mouse_events() {
        let mut grid = grid(4, 4);
        let pressed = centre(&grid, 1, 1);
        grid.handle_event(&Event::PointerPress {
            pos: pressed,
            button: 1,
            pressure: 0.5,
            tilt_x: 0.0,
            tilt_y: 0.0,
        });
        assert_eq!(grid.selection(), Some((1, 1)), "a pen press selects like a click");
        assert_eq!(grid.current_cell(), Some((1, 1)));

        let moved = centre(&grid, 2, 2);
        grid.handle_event(&Event::PointerMove {
            pos: moved,
            pressure: 0.0,
            tilt_x: 0.0,
            tilt_y: 0.0,
        });
        assert_eq!(grid.hovered_cell, Some((2, 2)), "a pen hover outlines like the mouse");

        grid.handle_event(&Event::PointerRelease { pos: moved, button: 1, pressure: 0.0 });
        assert_eq!(grid.resizing_column, None);
        assert_eq!(grid.selection(), Some((1, 1)), "the release does not disturb the selection");
    }

    /// Shift replaces the wheel's axis instead of adding to it: the previous handler applied the
    /// vertical scroll and *then* added the same notch horizontally.
    #[test]
    fn shift_wheel_replaces_the_axis() {
        let mut grid = grid(10, 8);
        grid.handle_event(&Event::Wheel {
            delta: Point::new(0, -120),
            modifiers: crate::shortcut::Modifiers::SHIFT.bits() as u32,
        });
        assert_eq!(grid.scroll_row(), 0, "the vertical axis is not also scrolled");
        assert_eq!(grid.scroll_column(), 1, "the notch becomes a horizontal one");

        grid.handle_event(&Event::Wheel { delta: Point::new(0, -120), modifiers: 0 });
        assert_eq!(grid.scroll_row(), 1, "without shift the notch stays vertical");
    }

    /// `ColumnSpec::sortable` is the section's declared capability; a click on a section that
    /// does not declare it still reports the click but must not sort.
    #[test]
    fn a_non_sortable_header_section_reports_the_click_without_sorting() {
        let mut grid = grid(4, 2);
        let mut header = GridViewHeader::new(Rect::new(0, 0, 240, 22));
        header.set_columns(vec![
            ColumnSpec::new("A", 120).with_sortable(false),
            ColumnSpec::new("B", 120).with_sortable(true),
        ]);
        grid.set_header(&header);
        let band = grid.header_band();

        let clicked = Arc::new(AtomicU64::new(u64::MAX));
        let sink = Arc::clone(&clicked);
        grid.header_clicked.connect(move |column| {
            sink.store(*column as u64, Ordering::Relaxed);
        });

        grid.handle_event(&Event::MousePress {
            pos: Point::new(band.x + 5, band.y + 4),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(clicked.load(Ordering::Relaxed), 0, "the click is still reported");
        assert!(grid.sort_specs().is_empty(), "a non-sortable section is not sorted");

        grid.handle_event(&Event::MousePress {
            pos: Point::new(band.x + 125, band.y + 4),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.sort_specs().len(), 1, "a sortable section still cycles");
        assert_eq!(grid.sort_specs()[0].column, 1);
    }

    /// Keyboard bounds follow the painted projection: Down cannot walk onto a row the filter
    /// removed, which nothing has a box for.
    #[test]
    fn keyboard_navigation_stops_at_the_last_projected_row() {
        let mut grid = DataGrid::new(Rect::new(0, 0, 600, 300));
        grid.set_data_source(Arc::new(Source {
            data: vec![
                vec!["alpha".to_string(), "a".to_string()],
                vec!["beta".to_string(), "b".to_string()],
                vec!["gamma".to_string(), "c".to_string()],
                vec!["delta".to_string(), "d".to_string()],
            ],
        }));
        grid.set_filters(vec![ColumnFilter { column: 0, query: "alph".to_string() }]);
        grid.fetch_visible_cells();

        grid.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        grid.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
        assert_eq!(
            grid.current_cell(),
            Some((0, 0)),
            "the projection paints one row, so Down stays on it"
        );
        assert!(!grid.move_current_cell(1, 0, false), "row 1 is not in the projection");
    }
}
