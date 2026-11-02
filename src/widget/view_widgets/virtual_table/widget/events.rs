// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable pointer/keyboard handling.

use super::*;

impl EventHandler for VirtualTable {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // The context menu is a request signal, exactly like the rest of the table family
            // (V-013). Both the right-button press and the platform's own menu event are
            // reported, so a host does not have to guess which one its backend delivers.
            // The context menu is a request signal. Selection happens in
            // `Widget::context_menu_event`, which the runtime calls with the synthesized
            // `Event::ContextMenu`; emitting here as well produced a duplicate request.
            Event::ContextMenu { .. } => {}
            Event::MousePress { pos, button: 1, modifiers } => {
                // A press in the column-title strip cycles that column's sort; only a body press
                // selects a cell (V-013).
                if let Some(column) = self.header_column_at(*pos) {
                    let _ = self.toggle_sort_column(column);
                    return;
                }
                if let Some((row, column)) = self.cell_at(*pos) {
                    let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                    self.select_cell_with_modifiers(row, column, modifiers);
                } else {
                    self.clear_selection();
                }
            }
            Event::MouseMove { pos, .. } => {
                // Hover is the absolute row under the pointer, from the same `cell_at`
                // derivation `cell_rect` paints through, so a highlight cannot land on a
                // row a click would not reach. `MouseMove` fires far more often than the
                // highlight changes, so the redraw is requested only on an actual change.
                let hovered = self.cell_at(*pos).map(|(row, _)| row);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, modifiers } => {
                // One shared normalisation for wheel units and modifiers, the same one the rest of
                // the table family uses. The old arm read `delta.y` raw and ignored modifiers, so a
                // model-level delta scrolled hundreds of rows and Shift could not pan sideways.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                let step = match self.wheel_scroll_mode {
                    WheelScrollMode::PerRow => 1,
                    WheelScrollMode::PerPage => self.visible_grid(self.geometry()).0.max(1),
                };
                let lines = scroll.lines_y.unsigned_abs() as usize * step;
                if scroll.lines_y < 0 {
                    self.set_scroll_row(self.scroll_row.saturating_add(lines));
                } else if scroll.lines_y > 0 {
                    self.set_scroll_row(self.scroll_row.saturating_sub(lines));
                }
                let columns = scroll.columns_x as isize * step as isize;
                if columns != 0 {
                    let next = self.scroll_column.saturating_add_signed(columns);
                    self.set_scroll_column(next);
                }
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                    let next = self
                        .scroll_column
                        .saturating_add_signed(-(scroll.lines_y as isize * step as isize));
                    self.set_scroll_column(next);
                }
            }
            Event::KeyPress { key, modifiers } => {
                let (page_rows, _) = self.visible_grid(self.geometry());
                let page = page_rows.max(1) as isize;
                let last = self.row_count().saturating_sub(1) as isize;
                match *key {
                    37 => self.move_current_cell(0, -1, *modifiers),
                    39 => self.move_current_cell(0, 1, *modifiers),
                    38 => self.move_current_cell(-1, 0, *modifiers),
                    40 => self.move_current_cell(1, 0, *modifiers),
                    crate::event::key_codes::PAGE_UP => {
                        self.move_current_cell(-page, 0, *modifiers)
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        self.move_current_cell(page, 0, *modifiers)
                    }
                    crate::event::key_codes::HOME => {
                        let current = self.selected_cell().map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(-current, 0, *modifiers);
                    }
                    crate::event::key_codes::END => {
                        let current = self.selected_cell().map_or(0, |(row, _)| row) as isize;
                        self.move_current_cell(last - current, 0, *modifiers);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
