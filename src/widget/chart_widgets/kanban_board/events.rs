// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

impl EventHandler for KanbanBoard {
    fn handle_event(&mut self, event: &Event) {
        // The proxy follows the board's own drag session, so it is drained before routing: the
        // frame that follows the gesture already shows the updated proxy.
        let _ = self.poll_drag_ghost_layer();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, modifiers } if *button == 1 => {
                // The collapse chevron is a header target, checked before any card hit: a press on
                // it must not start a drag or change the selection.
                let collapse = (0..self.columns.len()).find(|index| {
                    self.column_collapse_rect(*index).is_some_and(|rect| rect.contains_point(*pos))
                });
                if let Some(index) = collapse {
                    self.toggle_column_collapsed(index);
                    return;
                }
                // The lane chevron sits in the left strip, one level above the columns (SP-001).
                let lane_collapse = self
                    .lanes
                    .iter()
                    .find(|lane| {
                        self.lane_collapse_rect(&lane.id)
                            .is_some_and(|rect| rect.contains_point(*pos))
                    })
                    .map(|lane| lane.id.clone());
                if let Some(id) = lane_collapse {
                    self.toggle_lane_collapsed(&id);
                    return;
                }
                if let Some(position) = self.card_at(*pos) {
                    let card_id = self
                        .columns
                        .get(position.column)
                        .and_then(|column| column.cards.get(position.card))
                        .map(|card| card.id.clone());
                    let pressed = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                    // A modified click extends the selection instead of starting a drag, which is
                    // how a batch of cards is gathered for one move. The primary test goes through
                    // the shared predicate: on a Desktop host Control is the primary accelerator,
                    // and only the Command bit sets `Modifiers::PRIMARY` on its own.
                    if crate::shortcut::event_carries_primary(*modifiers)
                        || pressed.contains(crate::shortcut::Modifiers::SHIFT)
                    {
                        self.focused_card = Some(position);
                        if let Some(card_id) = card_id {
                            self.toggle_card_selected(&card_id);
                        }
                        return;
                    }
                    self.focused_card = Some(position);
                    if self.selected_cards.len() > 1
                        && card_id
                            .as_ref()
                            .is_some_and(|id| self.selected_cards.iter().any(|s| s == id))
                    {
                        // Pressing one member of a group keeps the group: the drop moves all of it.
                    } else {
                        self.clear_selection();
                    }
                    self.begin_card_drag(position, *pos);
                }
            }
            Event::MouseMove { pos } if self.drag.is_some() => {
                // Edge auto-scroll first: `update_card_drag` then resolves the column and the drop
                // index against the viewport the scroll just produced.
                self.autoscroll_drag(*pos);
                self.update_card_drag(*pos);
            }
            Event::MouseMove { pos } => {
                let hovered = self.card_at(*pos);
                let column = self.column_at(*pos);
                let changed = hovered != self.hovered_card || column != self.hovered_column;
                let hover_changed = hovered != self.hovered_card;
                self.hovered_card = hovered;
                self.hovered_column = column;
                if changed {
                    self.base.request_redraw();
                }
                if hover_changed {
                    let id = hovered.and_then(|position| {
                        self.columns
                            .get(position.column)
                            .and_then(|entry| entry.cards.get(position.card))
                            .map(|card| card.id.clone())
                    });
                    self.card_hovered.emit(id);
                }
            }
            Event::MouseRelease { pos, button } if *button == 1 && self.drag.is_some() => {
                self.finish_card_drag(*pos);
            }
            Event::Wheel { delta, modifiers } => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                // Shift, or a horizontal wheel, scrolls the board itself. Without this a board
                // wider than its viewport could only be scrolled by its container.
                if modifiers.contains(crate::shortcut::Modifiers::SHIFT) || delta.x != 0 {
                    let step = if delta.x != 0 {
                        delta.x.signum() * HORIZONTAL_SCROLL_STEP
                    } else if delta.y > 0 {
                        -HORIZONTAL_SCROLL_STEP
                    } else if delta.y < 0 {
                        HORIZONTAL_SCROLL_STEP
                    } else {
                        0
                    };
                    self.scroll_horizontally_by(step);
                    return;
                }
                if delta.y != 0 {
                    // Scroll the column under the pointer when one is known, otherwise the
                    // first column: a board with several columns scrolls them independently.
                    let column = self.hovered_column.unwrap_or(0);
                    self.scroll_column(column, if delta.y > 0 { 1 } else { -1 });
                }
            }
            Event::KeyPress { key, .. } => {
                let key = crate::shortcut::Key::from_key_code(*key);
                let current = self.focused_card.or_else(|| {
                    self.columns
                        .iter()
                        .position(|column| !column.cards.is_empty())
                        .map(|column| CardPosition { column, card: 0 })
                });
                let Some(current) = current else {
                    return;
                };
                match key {
                    Some(crate::shortcut::Key::Left) => {
                        let mut column = current.column;
                        while column > 0 {
                            column -= 1;
                            if !self.columns[column].cards.is_empty() {
                                let card = current.card.min(self.columns[column].cards.len() - 1);
                                self.set_focused_card(Some(CardPosition { column, card }));
                                break;
                            }
                        }
                    }
                    Some(crate::shortcut::Key::Right) => {
                        let mut column = current.column;
                        while column + 1 < self.columns.len() {
                            column += 1;
                            if !self.columns[column].cards.is_empty() {
                                let card = current.card.min(self.columns[column].cards.len() - 1);
                                self.set_focused_card(Some(CardPosition { column, card }));
                                break;
                            }
                        }
                    }
                    Some(crate::shortcut::Key::Up) => {
                        if current.card > 0 {
                            self.set_focused_card(Some(CardPosition {
                                column: current.column,
                                card: current.card - 1,
                            }));
                        }
                    }
                    Some(crate::shortcut::Key::Down) => {
                        if current.card + 1 < self.columns[current.column].cards.len() {
                            self.set_focused_card(Some(CardPosition {
                                column: current.column,
                                card: current.card + 1,
                            }));
                        }
                    }
                    Some(crate::shortcut::Key::PageUp) => self.scroll_column(current.column, -1),
                    Some(crate::shortcut::Key::PageDown) => self.scroll_column(current.column, 1),
                    Some(crate::shortcut::Key::Enter) | Some(crate::shortcut::Key::Space) => {
                        if let Some(card) = self
                            .columns
                            .get(current.column)
                            .and_then(|column| column.cards.get(current.card))
                        {
                            let id = card.id.clone();
                            self.card_activated.emit(id);
                        }
                    }
                    _ => {}
                }
            }
            // A drag that loses the pointer (the window lost focus, the pointer left
            // the surface) must not leave a card half-attached to the cursor.
            Event::MouseLeave { .. } => {
                self.cancel_drag();
                let hover_changed = self.hovered_card.take().is_some();
                if hover_changed {
                    self.card_hovered.emit(None);
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
    }
}
