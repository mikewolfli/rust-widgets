// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// The live link between a [`KanbanBoard`] and a [`DragGhostLayer`].
///
/// The board owns the layer so it can write its drag session into it; `poll_drag_ghost_layer`
/// compares the board's existing `drag`/`drag_origin` fields with the layer on every poll, so the
/// proxy starts, follows and hides without a second drag model.
pub(super) struct DragGhostBinding {
    /// The proxy kept in step with the board's drag session.
    layer: DragGhostLayer,
}

impl KanbanBoard {
    /// Advances the board while a dragged card is held near/over an edge.
    ///
    /// Without this a card could only be dropped inside the viewport it was picked up in; a long
    /// board forced the reader to drop, scroll, and pick the card up again. The step is the same
    /// clamp-respecting scroll the wheel uses.
    pub(super) fn autoscroll_drag(&mut self, pos: Point) {
        if !self.is_dragging_card() {
            return;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let band = DRAG_AUTOSCROLL_BAND;
        if pos.x < rect.x + band {
            self.scroll_horizontally_by(-HORIZONTAL_SCROLL_STEP);
        } else if pos.x > rect.x + rect.width as i32 - band {
            self.scroll_horizontally_by(HORIZONTAL_SCROLL_STEP);
        }
        let column = self.hovered_column.or(self.drag_origin.map(|origin| origin.column));
        if let Some(column) = column {
            let top = rect.y + HEADER_HEIGHT as i32 + band;
            let bottom = rect.y + rect.height as i32 - band;
            if pos.y < top {
                self.scroll_column(column, -1);
            } else if pos.y > bottom {
                self.scroll_column(column, 1);
            }
        }
    }

    /// Follows a [`DragGhostLayer`] with this board's drag session.
    ///
    /// This is the companion binding for COMP-A8. The layer is pushed from the board's existing
    /// `drag`/`drag_origin` fields — the card id is the title, the source card's rectangle is the
    /// origin, and the board's own geometry is the drop bound — so there is no second drag model.
    /// Attaching adopts an active drag immediately; [`Self::poll_drag_ghost_layer`] starts the
    /// proxy when the pointer crosses the drag threshold, follows it, and hides it when the drag
    /// ends or is cancelled. Attaching replaces any previous layer.
    pub fn attach_drag_ghost_layer(&mut self, mut layer: DragGhostLayer) {
        layer.set_drop_bounds(self.geometry());
        match self.drag_ghost_state() {
            Some((origin, title, pointer)) => {
                layer.begin(origin, title);
                layer.follow(pointer);
            }
            None => {
                let _ = layer.cancel();
            }
        }
        self.drag_ghost = Some(DragGhostBinding { layer });
        self.base.request_redraw();
    }

    /// Stops following the attached drag proxy and returns it.
    pub fn detach_drag_ghost_layer(&mut self) -> Option<DragGhostLayer> {
        let binding = self.drag_ghost.take()?;
        self.base.request_redraw();
        Some(binding.layer)
    }

    /// Whether a [`DragGhostLayer`] is attached.
    pub fn is_drag_ghost_layer_attached(&self) -> bool {
        self.drag_ghost.is_some()
    }

    /// The attached drag proxy, if any.
    pub fn drag_ghost_layer(&self) -> Option<&DragGhostLayer> {
        self.drag_ghost.as_ref().map(|binding| &binding.layer)
    }

    /// The attached drag proxy for editing, if any.
    pub fn drag_ghost_layer_mut(&mut self) -> Option<&mut DragGhostLayer> {
        self.drag_ghost.as_mut().map(|binding| &mut binding.layer)
    }

    /// Pushes the board's drag session into the attached proxy.
    ///
    /// Called from `draw` and `handle_event`; returns whether the layer's active state, title,
    /// origin, bounds or pointer moved. With no drag in progress the proxy is hidden once and
    /// later polls return `false`.
    pub fn poll_drag_ghost_layer(&mut self) -> bool {
        let bounds = self.geometry();
        let state = self.drag_ghost_state();
        let Some(binding) = self.drag_ghost.as_mut() else {
            return false;
        };
        let mut changed = false;
        if binding.layer.drop_bounds() != Some(bounds) {
            binding.layer.set_drop_bounds(bounds);
            changed = true;
        }
        match state {
            Some((origin, title, pointer)) => {
                if !binding.layer.is_active()
                    || binding.layer.origin_rect() != origin
                    || binding.layer.title() != title
                {
                    binding.layer.begin(origin, title);
                    changed = true;
                }
                if binding.layer.pointer() != pointer {
                    binding.layer.follow(pointer);
                    changed = true;
                }
            }
            None => {
                if binding.layer.is_active() {
                    binding.layer.cancel();
                    changed = true;
                }
            }
        }
        changed
    }

    /// The board's drag session expressed as a proxy state: source card rectangle, card id and
    /// pointer. `None` when no drag has crossed the activation threshold.
    pub(super) fn drag_ghost_state(&self) -> Option<(Rect, String, Point)> {
        if !self.is_dragging_card() {
            return None;
        }
        let origin = self.card_rect(self.drag_origin?)?;
        let title = self.dragged_card_id()?.to_string();
        let pointer = self.drag.as_ref()?.current();
        Some((origin, title, pointer))
    }

    /// Abandons the drag in progress.
    pub fn cancel_drag(&mut self) {
        if self.drag.take().is_some() {
            self.drag_origin = None;
            self.hovered_column = None;
            self.base.request_redraw();
        }
    }

    /// Begins a drag on the card at `position`.
    pub(super) fn begin_card_drag(&mut self, position: CardPosition, pos: Point) {
        let Some(column) = self.columns.get(position.column) else {
            return;
        };
        let Some(card) = column.cards.get(position.card) else {
            return;
        };
        let payload = DragPayload::new(CARD_PAYLOAD_TYPE, card.id.clone())
            .with_label(card.title.clone())
            .with_origin(pos);
        self.drag = Some(DragSession::begin(payload, pos));
        self.drag_origin = Some(position);
        self.hovered_column = Some(position.column);
        self.base.request_redraw();
    }

    /// Applies a pointer move to the drag in progress.
    pub(super) fn update_card_drag(&mut self, pos: Point) {
        let Some(session) = self.drag.as_mut() else {
            return;
        };
        session.update(pos, CARD_DRAG_THRESHOLD);
        if !session.is_active() {
            return;
        }
        // The hovered column drives the drop preview, so it is tracked for the whole
        // active drag rather than only on release.
        let hovered = self.column_at(pos);
        if hovered != self.hovered_column {
            self.hovered_column = hovered;
            self.base.request_redraw();
        }
    }

    /// Commits the drag in progress at `pos`.
    ///
    /// Returns whether the board moved anything, so the caller can tell a real drop
    /// from a cancelled one.
    ///
    /// A session that never crossed the drag threshold is a **click**, not a cancelled
    /// drag, and it fires [`KanbanBoard::card_activated`]. Before this distinction was
    /// made, a click on a card produced no signal at all: the press opened a session,
    /// the release took the `!effect.is_accepted()` branch, and the card's id was
    /// dropped on the floor — so the documented `card_activated` event could never fire
    /// from a real pointer.
    pub(super) fn finish_card_drag(&mut self, pos: Point) -> bool {
        let Some(session) = self.drag.take() else {
            return false;
        };
        // Read `drag_origin` **without** taking it yet: the click branch below needs it,
        // and the drop branch must still see it. Taking it up front forced an `Option`
        // unwrap into the middle of the drop arithmetic, which is how an earlier
        // revision of this function ended up shadowing the value it still needed.
        let origin = self.drag_origin.take();
        self.hovered_column = None;

        // The press/release never became a drag, so the gesture is a click on the card
        // the session was started from. `is_active()` is the shared machine's own
        // verdict on "did the pointer travel far enough", so this stays in step with
        // the threshold constant rather than re-deriving it here.
        if !session.is_active() {
            let activated = origin
                .and_then(|start| {
                    self.columns.get(start.column).and_then(|c| c.cards.get(start.card))
                })
                .map(|card| card.id.clone());
            self.base.request_redraw();
            if let Some(card_id) = activated {
                self.card_activated.emit(card_id);
            }
            return false;
        }

        let mut session = session;

        // The shared machine decides whether this drop may happen at all; the board
        // only supplies the placement.
        let effect = session.drop_on(self, pos);
        if !effect.is_accepted() {
            self.base.request_redraw();
            return false;
        }
        let Some(origin) = origin else {
            return false;
        };
        let Some(column) = self.column_at(pos) else {
            return false;
        };
        let card_id = session.payload().item_id.clone();

        // Where the card should land, computed **before** the removal so the index
        // refers to the list the user saw while dragging.
        let raw_index = self.insert_index_at(column, pos);
        // Removing the card first shifts every later index in the same column down
        // by one; compensating here is what makes a small downward nudge move the
        // card by one rather than landing it one slot too high.
        let target_index = if origin.column == column && raw_index > origin.card {
            raw_index - 1
        } else {
            raw_index
        };
        // Re-read the card, since `drop_on` may have changed the board.
        let Some(current) = self.position_of(&card_id) else {
            return false;
        };
        let target = CardPosition { column, card: target_index };
        // The lane is the second destination coordinate: it is resolved from the release point once
        // the column is known, so a drop into another band changes both at once.
        let target_lane = if self.lanes_active() {
            self.lane_at(pos).and_then(|lane| self.lanes.get(lane)).map(|lane| lane.id.clone())
        } else {
            None
        };
        let current_lane = self
            .card_lane(&card_id)
            .map(str::to_string)
            .or_else(|| self.lanes.first().map(|lane| lane.id.clone()));
        let lane_changed = target_lane.is_some() && target_lane != current_lane;
        if current == target && !lane_changed {
            // A drop that changes nothing is accepted (the user did place the card)
            // but moves nothing, so no signal is emitted for a no-op.
            self.base.request_redraw();
            return false;
        }
        let moved = self.move_card(current, target).is_some();
        if let Some(lane) = target_lane {
            self.set_card_lane(&card_id, &lane);
        }
        // A drop that carried a selected group moves the rest of the group with it, which is the
        // batch operation a multi-selection exists for. The batch is reported separately so the
        // published single-card `card_moved` payload keeps its shape.
        if moved && self.selected_cards.len() > 1 && self.selected_cards.contains(&card_id) {
            let batch: Vec<String> =
                self.selected_cards.iter().filter(|id| **id != card_id).cloned().collect();
            let mut destination = target.card + 1;
            let mut moved_ids = vec![card_id.clone()];
            for id in batch {
                if let Some(position) = self.position_of(&id) {
                    if self
                        .move_card(position, CardPosition { column, card: destination })
                        .is_some()
                    {
                        moved_ids.push(id);
                        destination += 1;
                    }
                }
            }
            if moved_ids.len() > 1 {
                self.cards_moved.emit(moved_ids);
            }
        }
        moved || lane_changed
    }
}
