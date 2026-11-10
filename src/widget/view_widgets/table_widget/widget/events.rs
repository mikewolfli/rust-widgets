// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TableWidget pointer/keyboard handling.

use super::*;

impl crate::event::EventHandler for TableWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        // A source change may have invalidated the row and column indices an event addresses, so
        // the pending repair runs before the event is interpreted.
        self.drain_data_updates();
        // The base keeps the control-level hover/press facts and answers `widget_state()`. This
        // handler did not forward to it, so `"table:hover"` could never fire.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button, modifiers } if *button == 1 => {
                if let Some(column) = self.header_column_at(*pos) {
                    self.section_clicked.emit(column);
                    return;
                }
                // The row is read from `row_at_point`, the same derivation the paint loop uses.
                // The press arm used to compute its own `(pos.y - rect.y) / 20`, which measured from
                // the control instead of from the content box below the header, so a click on the
                // first content row selected the row above it.
                if let Some(index) = self.row_at_point(*pos) {
                    if let Some(column) = self.column_at(pos.x) {
                        self.last_pointed_column = Some(column);
                    }
                    self.select_row_with_modifiers(
                        index,
                        crate::shortcut::Modifiers::from_event_bits(*modifiers),
                    );
                }
            }
            crate::event::Event::MouseMove { pos } => {
                let hovered = self.row_at_point(*pos);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseLeave { .. } => {
                let had_hover = self.hovered_row.take().is_some();
                if had_hover {
                    self.base.request_redraw();
                }
            }
            crate::event::Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.row_at_point(*pos) {
                    self.select_row(index);
                    self.row_activated.emit(index);
                    // The column is the one the pointer is actually over. This used to be the
                    // literal `0`, so double-clicking any column edited the first one, which is a
                    // different cell from the one the user pointed at.
                    if let Some(column) = self.column_at(pos.x) {
                        self.last_pointed_column = Some(column);
                        self.cell_double_clicked.emit((index, column));
                        if self.editable
                            && self.edit_triggers.contains(TableEditTriggers::DOUBLE_CLICKED)
                        {
                            self.edit_requested.emit((index, column));
                        }
                    }
                }
            }
            crate::event::Event::ContextMenu { .. } => {
                // Answered by `Widget::context_menu_event`, which selects the row under the pointer
                // before emitting; an arm here would emit a second, unselected request first.
            }
            crate::event::Event::Wheel { delta, modifiers } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                let mods = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if mods.contains(crate::shortcut::Modifiers::SHIFT) {
                    let next = self.scroll_column.saturating_add_signed(-(scroll.lines_y as isize));
                    self.set_scroll_column(next);
                } else {
                    let row = self.scroll_row as isize - scroll.lines_y as isize;
                    self.set_scroll_row(row.max(0) as usize);
                }
            }
            crate::event::Event::KeyPress { key, .. } if self.base.focus_reason().is_some() => {
                let page = (self.rows_band().height / TABLE_ROW_HEIGHT.max(1)).max(1) as usize;
                let last = self.row_count().saturating_sub(1);
                match *key {
                    crate::event::key_codes::UP => {
                        // The validated accessor, not the raw field: a model that shrinks without a
                        // change signal leaves a focus row past the end, and subtracting from it
                        // produced a row that no longer exists (so the viewport jumped to the
                        // clamp while every further `UP` recomputed the same stale index).
                        let next = self.focused_row().unwrap_or(0).saturating_sub(1);
                        self.select_row(next);
                        self.set_scroll_row(next);
                    }
                    crate::event::key_codes::DOWN => {
                        let next = self
                            .focused_row()
                            .map(|row| row.saturating_add(1))
                            .unwrap_or(0)
                            .min(last);
                        self.select_row(next);
                        self.set_scroll_row(next);
                    }
                    crate::event::key_codes::HOME => {
                        self.select_row(0);
                        self.set_scroll_row(0);
                    }
                    crate::event::key_codes::END => {
                        self.select_row(last);
                        self.set_scroll_row(last);
                    }
                    crate::event::key_codes::PAGE_UP => {
                        let next = self.focused_row().unwrap_or(0).saturating_sub(page);
                        self.select_row(next);
                        self.set_scroll_row(next);
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        let next = self
                            .focused_row()
                            .map(|row| row.saturating_add(page))
                            .unwrap_or(0)
                            .min(last);
                        self.select_row(next);
                        self.set_scroll_row(next);
                    }
                    crate::event::key_codes::ENTER
                        if self.edit_triggers.contains(TableEditTriggers::EDIT_KEY) =>
                    {
                        if let Some(row) = self.focused_row() {
                            if self.editable {
                                let column = self.last_pointed_column.unwrap_or(0);
                                self.edit_requested.emit((row, column));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
