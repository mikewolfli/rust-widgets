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
            let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
            if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                let next = self.scroll_column.saturating_add_signed(-(scroll.lines_y as isize));
                self.set_scroll_column(next);
            }
        }

        if let Event::MousePress { pos, button, modifiers } = event {
            if *button == 3 {
                self.context_menu_requested.emit(*pos);
                return;
            }
            if *button != 1 {
                return;
            }
            if let Some(column) = self.resize_handle_at(*pos) {
                self.resizing_column = Some(column);
                self.resize_start_x = pos.x;
                self.resize_start_width = self.column_width_at(column);
                return;
            }
            if let Some(column) = self.header_column_at(*pos) {
                self.header_clicked.emit(column);
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
            let hit = self.cell_at(*pos);
            self.current_cell = hit;
            match hit {
                Some((row, column)) => {
                    self.cell_clicked.emit((row, column));
                    self.apply_selection_click(row, column, *modifiers);
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
            Event::ContextMenu { pos } => self.context_menu_requested.emit(*pos),
            Event::KeyPress { key, modifiers } if self.base.focus_reason().is_some() => {
                let page = self.visible_row_capacity().max(1);
                let last_row = self.row_count().saturating_sub(1);
                let last_column = self.column_count().saturating_sub(1);
                if self.row_count() == 0 || self.column_count() == 0 {
                    return;
                }
                let (mut row, mut column) = self
                    .current_cell
                    .or(self.selection)
                    .unwrap_or((self.scroll_row, self.scroll_column));
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
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
            Event::MouseMove { pos } => {
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
            Event::MouseRelease { button: 1, .. } => {
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
