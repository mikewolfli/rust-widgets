// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

impl KanbanBoard {
    // ── Geometry ────────────────────────────────────────────────────────────

    /// The rectangle of the column at `index`.
    ///
    /// Columns are laid out left to right at the control's own width, offset by the horizontal
    /// scroll. They used to be laid out at a fixed 220 px with no offset at all, which left a board
    /// wider than its viewport with no way to reach the columns past the edge — the documented
    /// answer was "let the container scroll it", which a control that paints itself cannot rely on.
    pub fn column_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.columns.len() {
            return None;
        }
        let rect = self.geometry();
        // Clamped on read as well as on write: a board that shrank after its offset was set would
        // otherwise paint past its content and show empty space.
        let scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
        // In lane mode the left strip names the lanes, so the columns begin after it.
        let origin = rect.x + if self.lanes_active() { LANE_HEADER_WIDTH as i32 } else { 0 };
        // Per-column widths make the stride non-uniform, so the column's left edge is the sum of
        // the widths and gaps before it rather than `index * stride`.
        let mut x = origin - scroll;
        for previous in 0..index {
            x += self.column_width_at(previous) as i32 + COLUMN_GAP;
        }
        Some(Rect::new(x, rect.y, self.column_width_at(index), rect.height))
    }

    /// The width column `index` is painted at: its override, or the board-wide default.
    pub fn column_width_at(&self, index: usize) -> u32 {
        self.columns
            .get(index)
            .and_then(|column| self.column_widths.get(&column.id))
            .copied()
            .unwrap_or(self.column_width)
    }

    /// Overrides the width of column `index`, or clears the override when `width` is `None`.
    ///
    /// Returns whether `index` named a column. The width is floored at [`MIN_COLUMN_WIDTH`], the
    /// same floor [`Self::set_column_width`] applies, so an override cannot produce a column that
    /// no longer has room for a card title.
    pub fn set_column_width_at(&mut self, index: usize, width: Option<u32>) -> bool {
        let Some(column) = self.columns.get(index) else {
            return false;
        };
        let id = column.id.clone();
        let changed = match width {
            Some(width) => {
                let width = width.max(MIN_COLUMN_WIDTH);
                self.column_widths.insert(id, width) != Some(width)
            }
            None => self.column_widths.remove(&id).is_some(),
        };
        if changed {
            self.clamp_horizontal_scroll();
            self.base.request_layout();
            self.base.request_redraw();
        }
        true
    }

    /// Whether column `index` is collapsed, or `None` when the index names no column.
    pub fn is_column_collapsed(&self, index: usize) -> Option<bool> {
        self.columns.get(index).map(|column| column.collapsed)
    }

    /// Collapses or expands column `index`; returns whether the index named a column.
    pub fn set_column_collapsed(&mut self, index: usize, collapsed: bool) -> bool {
        let Some(column) = self.columns.get_mut(index) else {
            return false;
        };
        if column.collapsed == collapsed {
            return true;
        }
        column.collapsed = collapsed;
        self.invalidate_layout();
        self.clamp_horizontal_scroll();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Flips the collapsed state of column `index`; returns whether the index named a column.
    pub fn toggle_column_collapsed(&mut self, index: usize) -> bool {
        match self.is_column_collapsed(index) {
            Some(collapsed) => self.set_column_collapsed(index, !collapsed),
            None => false,
        }
    }

    /// The painted box of a column header's collapse chevron, or `None` for an unknown index.
    ///
    /// The same leading-edge cell [`KanbanColumnHeader::collapse_rect`] uses, so the board's
    /// inline header and the standalone header control present one target at one place.
    pub fn column_collapse_rect(&self, index: usize) -> Option<Rect> {
        let column_rect = self.column_rect(index)?;
        Some(Rect::new(
            column_rect.x + 4,
            column_rect.y + (HEADER_HEIGHT as i32 - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        ))
    }

    /// The rectangle of the card at `position`, or `None` when it is not laid out.
    ///
    /// A collapsed column lays out no cards, so its cards report `None` rather than
    /// a rectangle inside the collapsed header.
    pub fn card_rect(&self, position: CardPosition) -> Option<Rect> {
        let column = self.columns.get(position.column)?;
        if column.collapsed || position.card >= column.cards.len() {
            return None;
        }
        let column_rect = self.column_rect(position.column)?;
        // In lane mode the band owns the row: the same card index inside a column can sit at a
        // different y in every lane, so the ordinal is the one the cached layout derived.
        if self.lanes_active() {
            let (lane, ordinal) = self.lane_slot(position)?;
            // A collapsed lane hides its cards; the header strip remains (SP-001).
            if self.lanes.get(lane).is_some_and(|lane| self.is_lane_collapsed(&lane.id)) {
                return None;
            }
            let band = self.lane_band_rect(lane)?;
            let y = band.y + LANE_HEADER_HEIGHT + ordinal as i32 * (CARD_HEIGHT as i32 + CARD_GAP);
            return Some(Rect::new(column_rect.x, y, column_rect.width, CARD_HEIGHT));
        }
        let scroll = self.column_scroll.get(position.column).copied().unwrap_or(0);
        let y = column_rect.y
            + HEADER_HEIGHT as i32
            + ((position.card.saturating_sub(scroll)) as i32) * (CARD_HEIGHT as i32 + CARD_GAP);
        if position.card < scroll {
            return None;
        }
        Some(Rect::new(column_rect.x, y, column_rect.width, CARD_HEIGHT))
    }

    /// The column containing `pos`, when the pointer is over one.
    pub(super) fn column_at(&self, pos: Point) -> Option<usize> {
        // Clamped to the board first: a column wider or further right than the control would
        // otherwise answer for a point outside it, which is a press on whatever the layout put
        // beside the board.
        if !self.geometry().contains_point(pos) {
            return None;
        }
        // The lane strip is not a column: a press there names a lane, not a destination card slot.
        if self.lanes_active() && pos.x < self.geometry().x + LANE_HEADER_WIDTH as i32 {
            return None;
        }
        // Walked with a running left edge rather than through `column_rect` per index: the
        // per-column widths make each rect an O(index) sum, and calling it in the loop made the
        // hit test quadratic in the column count.
        let rect = self.geometry();
        let scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
        let origin = rect.x + if self.lanes_active() { LANE_HEADER_WIDTH as i32 } else { 0 };
        let mut x = origin - scroll;
        for index in 0..self.columns.len() {
            let width = self.column_width_at(index);
            if Rect::new(x, rect.y, width, rect.height).contains_point(pos) {
                return Some(index);
            }
            x += width as i32 + COLUMN_GAP;
        }
        None
    }

    /// The card under `pos`, when the pointer is over one.
    pub(super) fn card_at(&self, pos: Point) -> Option<CardPosition> {
        let column = self.column_at(pos)?;
        let entries = self.columns.get(column)?;
        if entries.collapsed {
            return None;
        }
        // Lane mode paints each card at the ordinal the cached layout gave it, so there is no
        // closed-form slot to invert; the column is still bounded, and the lane cache makes each
        // probe constant-time.
        if self.lanes_active() {
            for index in 0..entries.cards.len() {
                let position = CardPosition { column, card: index };
                if self.card_rect(position).is_some_and(|rect| rect.contains_point(pos)) {
                    return Some(position);
                }
            }
            return None;
        }
        // The flat layout is an arithmetic grid: the slot under the pointer is its y offset from
        // the first row divided by the row pitch, offset by the column's scroll. Looking the card
        // up this way makes the hit test O(1) instead of walking every card in the column.
        let rect = self.column_rect(column)?;
        let first_y = rect.y + HEADER_HEIGHT as i32;
        if pos.y < first_y {
            return None;
        }
        let stride = CARD_HEIGHT as i32 + CARD_GAP;
        let slot = ((pos.y - first_y) / stride) as usize;
        let scroll = self.column_scroll.get(column).copied().unwrap_or(0);
        let position = CardPosition { column, card: scroll + slot };
        // The gap between two cards belongs to neither, and a slot past the column's last card
        // names no card; `card_rect` returns `None` in both cases.
        self.card_rect(position).filter(|rect| rect.contains_point(pos)).map(|_| position)
    }

    /// The inclusive/exclusive range of cards a column paints at its current scroll.
    ///
    /// The controller already exposes only the cards of one column at a time; this is that window
    /// as a range, so the paint loop stops at the viewport edge instead of walking every card that
    /// the column clip would discard. A lane board has no per-column window and keeps the full
    /// range, which is exactly the behaviour it had before.
    pub(super) fn visible_card_range(&self, column: usize) -> (usize, usize) {
        let Some(entries) = self.columns.get(column) else {
            return (0, 0);
        };
        if self.lanes_active() {
            return (0, entries.cards.len());
        }
        let scroll = self.column_scroll.get(column).copied().unwrap_or(0).min(entries.cards.len());
        // One extra card past the last fully visible row: a partially visible card still paints.
        let end = scroll
            .saturating_add(self.visible_cards_per_column())
            .saturating_add(1)
            .min(entries.cards.len());
        (scroll, end)
    }

    /// The index a card dropped at `pos` would take within `column`.
    ///
    /// Computed from the pointer's y against the laid-out cards, so dropping in the
    /// gap between two cards inserts between them rather than being snapped to one
    /// of their indices. A drop below the last card appends.
    pub(super) fn insert_index_at(&self, column: usize, pos: Point) -> usize {
        let Some(entries) = self.columns.get(column) else {
            return 0;
        };
        let Some(column_rect) = self.column_rect(column) else {
            return 0;
        };
        if self.lanes_active() {
            let lane = self.lane_at(pos).unwrap_or(0);
            let lane_indices: Vec<usize> = {
                let cache = self.lane_cache();
                cache
                    .card_lane
                    .get(column)
                    .map(|slots| {
                        slots
                            .iter()
                            .enumerate()
                            .filter(|(_, slot)| **slot == lane)
                            .map(|(index, _)| index)
                            .collect()
                    })
                    .unwrap_or_default()
            };
            if lane_indices.is_empty() {
                // The lane has no card in this column, so appending keeps the other lanes' order
                // intact and puts the new card at the end of its own band.
                return entries.cards.len();
            }
            let first_card_y = self
                .lane_band_rect(lane)
                .map(|band| band.y + LANE_HEADER_HEIGHT)
                .unwrap_or(column_rect.y + HEADER_HEIGHT as i32);
            let stride = CARD_HEIGHT as i32 + CARD_GAP;
            let offset = (pos.y - first_card_y).max(0);
            let slot = ((offset + stride / 2) / stride) as usize;
            let slot = slot.min(lane_indices.len());
            return if slot < lane_indices.len() {
                lane_indices[slot]
            } else {
                lane_indices.last().copied().unwrap_or(0) + 1
            };
        }
        // Cards start below the header; a drop in the header goes to the top.
        let first_card_y = column_rect.y + HEADER_HEIGHT as i32;
        if pos.y <= first_card_y {
            return 0;
        }
        // The midpoint rule: a pointer past a card's vertical centre inserts after
        // it. This is what makes the drop land where the user's cursor is rather
        // than always above the card under it. The pointer is relative to the painted window, so
        // the column's scroll offset is added back to name an index in the model.
        let stride = CARD_HEIGHT as i32 + CARD_GAP;
        let offset = pos.y - first_card_y;
        let scroll = self.column_scroll.get(column).copied().unwrap_or(0);
        let index = (offset + stride / 2) / stride;
        // Clamped to the column's length: a pointer below the last card appends.
        (scroll + index as usize).min(entries.cards.len())
    }
}

impl DropTarget for KanbanBoard {
    /// Accepts a card payload when it targets a column that has room.
    ///
    /// The WIP limit is enforced **here** rather than in `on_drop` because the
    /// preview must not promise a drop the commit then refuses — that is the whole
    /// reason this trait keeps the two apart.
    ///
    /// # Why the check is per-destination, not per-card
    ///
    /// An earlier version exempted any card that was already somewhere on the
    /// board, on the reasoning that reordering adds nothing. That is only true of a
    /// move *within* one column: dragging a card from a short column into a full one
    /// adds to the full column, and the exemption let it through. The limit is about
    /// what the destination holds, so the destination is what this asks.
    ///
    /// A board with no columns accepts nothing: there is nowhere to put the card.
    fn can_accept(&self, payload: &DragPayload) -> bool {
        if !payload.is_type(CARD_PAYLOAD_TYPE) {
            return false;
        }
        if self.columns.is_empty() {
            return false;
        }
        // Every column being full means there is nowhere for a new card to land.
        // Which column the pointer is over is checked at the drop site, where the
        // position is known; this is the question that must be answerable during the
        // drag, before any position is committed.
        self.columns.iter().any(|column| !column.is_at_wip_limit() || column.cards.is_empty())
    }

    /// Reports the target position without moving anything.
    ///
    /// Uses the same column resolution the commit does, so the preview rectangle
    /// and the landing place cannot disagree.
    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !payload.is_type(CARD_PAYLOAD_TYPE) {
            return DropEffect::None;
        }
        let Some(column) = self.column_at(pos) else {
            return DropEffect::None;
        };
        // The destination's own limit is checked here, where the column is known.
        // `can_accept` answers the position-independent question ("does any column
        // have room"); this answers the one that needs the drop site.
        //
        // A card arriving from *this* column does not add to it, so a move within a
        // full column stays legal — that is the case the limit must not block.
        let arriving_from_here = self.drag_origin.is_some_and(|origin| origin.column == column);
        if !arriving_from_here {
            if let Some(target) = self.columns.get(column) {
                if target.is_at_wip_limit() {
                    return DropEffect::None;
                }
            }
        }
        // The board is the target *and* the owner of the card, so the move itself
        // happens in `finish_card_drag` where the origin is known. Reporting `Move`
        // is what tells that path the placement was accepted.
        DropEffect::Move
    }

    /// Highlights the column the card would land in.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<Rect> {
        if !payload.is_type(CARD_PAYLOAD_TYPE) {
            return None;
        }
        let column = self.column_at(pos)?;
        let mut rect = self.column_rect(column)?;
        // In lane mode the landing place is a cell, not the whole column: the preview must show
        // the band the release would put the card in.
        if self.lanes_active() {
            if let Some(lane) = self.lane_at(pos) {
                if let Some(band) = self.lane_band_rect(lane) {
                    rect.y = band.y;
                    rect.height = band.height;
                }
            }
        }
        Some(rect)
    }
}
