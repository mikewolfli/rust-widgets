// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Cached lane geometry and per-card lane slots.
///
/// Lane mode derived each lane's height by walking every column's cards and each card's y by
/// re-walking its column — and the hover path repeated that work on every pointer move. Both are
/// pure functions of the card/lane structure, so they are built once per revision.
pub(super) struct LaneLayout {
    /// The revision this snapshot describes.
    pub(super) revision: u64,
    /// Band height per lane.
    pub(super) heights: Vec<i32>,
    /// Band top per lane, relative to the board's own top edge.
    pub(super) tops: Vec<i32>,
    /// For each column, the lane each card is painted in.
    pub(super) card_lane: Vec<Vec<usize>>,
    /// For each column, each card's ordinal inside its lane.
    pub(super) card_ordinal: Vec<Vec<usize>>,
}

impl KanbanBoard {
    // ── Swimlanes ───────────────────────────────────────────────────────────

    /// Adds a lane, returning its index.
    pub fn add_lane(&mut self, lane: KanbanLane) -> usize {
        let index = self.lanes.len();
        self.lanes.push(lane);
        self.invalidate_layout();
        self.base.request_redraw();
        index
    }

    /// Removes the lane with `id`, clearing every card assigned to it.
    ///
    /// A removed lane's cards fall back to the first lane rather than disappearing: an assignment
    /// is display state, and removing the band must not delete the card.
    pub fn remove_lane(&mut self, id: &str) -> Option<KanbanLane> {
        let index = self.lanes.iter().position(|lane| lane.id == id)?;
        let removed = self.lanes.remove(index);
        self.card_lanes.retain(|_, lane| lane != &removed.id);
        self.invalidate_layout();
        self.base.request_redraw();
        Some(removed)
    }

    /// The lanes, in display order.
    pub fn lanes(&self) -> &[KanbanLane] {
        &self.lanes
    }

    /// The lane with `id`.
    pub fn lane(&self, id: &str) -> Option<&KanbanLane> {
        self.lanes.iter().find(|lane| lane.id == id)
    }

    /// How many lanes the board has.
    pub fn lane_count(&self) -> usize {
        self.lanes.len()
    }

    /// Whether cards are grouped into lanes.
    pub fn lane_mode(&self) -> KanbanLaneMode {
        self.lane_mode
    }

    /// Turns swimlanes on or off. Lanes still need at least one [`KanbanLane`] to lay out.
    pub fn set_lane_mode(&mut self, mode: KanbanLaneMode) {
        if self.lane_mode == mode {
            return;
        }
        self.lane_mode = mode;
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Assigns a card to a lane, returning whether both ids resolved.
    ///
    /// A card with no assignment is painted in the first lane, which keeps a board usable while
    /// the caller is still filling in assignments.
    pub fn set_card_lane(&mut self, card_id: &str, lane_id: &str) -> bool {
        if self.position_of(card_id).is_none() || self.lane(lane_id).is_none() {
            return false;
        }
        let next = lane_id.to_string();
        if self.card_lanes.get(card_id) == Some(&next) {
            return true;
        }
        self.card_lanes.insert(card_id.to_string(), next);
        self.invalidate_layout();
        self.base.request_redraw();
        true
    }

    /// The lane a card is assigned to, if any.
    ///
    /// This is the documented `lane_of` spelling of [`Self::card_lane`]; a card with no assignment
    /// returns `None` and is painted in the first lane.
    pub fn lane_of(&self, card_id: &str) -> Option<&str> {
        self.card_lanes.get(card_id).map(String::as_str)
    }

    /// Assigns `card_id` to `lane_id`, returning whether both resolved.
    ///
    /// The `set_lane` spelling of [`Self::set_card_lane`]; it refuses an unknown card or lane
    /// rather than storing an assignment that can never paint.
    pub fn set_lane(&mut self, card_id: &str, lane_id: &str) -> bool {
        self.set_card_lane(card_id, lane_id)
    }

    /// The lane a card is assigned to, if any.
    pub fn card_lane(&self, card_id: &str) -> Option<&str> {
        self.card_lanes.get(card_id).map(String::as_str)
    }

    /// The index of the lane a card is painted in: its assignment, or the first lane.
    pub(super) fn card_lane_index(&self, card_id: &str) -> Option<usize> {
        if self.lanes.is_empty() {
            return None;
        }
        match self.card_lanes.get(card_id) {
            Some(lane_id) => self.lanes.iter().position(|lane| &lane.id == lane_id).or(Some(0)),
            None => Some(0),
        }
    }

    /// Whether lanes affect layout at all.
    pub(super) fn lanes_active(&self) -> bool {
        self.lane_mode == KanbanLaneMode::Lanes && !self.lanes.is_empty()
    }

    /// The top of lane `index`'s band in board coordinates.
    pub(super) fn lane_top(&self, index: usize) -> i32 {
        let rect = self.geometry();
        let cache = self.lane_cache();
        rect.y + cache.tops.get(index).copied().unwrap_or(HEADER_HEIGHT as i32)
    }

    /// The height of lane `index`'s band: its caption plus as many card rows as the fullest
    /// column holds in that lane.
    pub(super) fn lane_height(&self, index: usize) -> i32 {
        self.lane_cache().heights.get(index).copied().unwrap_or(LANE_HEADER_HEIGHT + LANE_GAP)
    }

    /// The lane geometry snapshot for the current structure.
    ///
    /// Built at most once per structural change; the hover path and the paint path both only
    /// read it, which is what removes the column×card×lane walk from pointer movement.
    pub(super) fn lane_cache(&self) -> core::cell::Ref<'_, LaneLayout> {
        let fresh = self
            .lane_layout
            .borrow()
            .as_ref()
            .is_some_and(|cache| cache.revision == self.layout_revision);
        if !fresh {
            let built = self.build_lane_layout();
            *self.lane_layout.borrow_mut() = Some(built);
        }
        core::cell::Ref::map(self.lane_layout.borrow(), |slot| {
            slot.as_ref().expect("lane_cache() populates the snapshot")
        })
    }

    /// Derives every lane band and every card's lane slot in one pass.
    pub(super) fn build_lane_layout(&self) -> LaneLayout {
        let lane_count = self.lanes.len();
        let mut card_lane: Vec<Vec<usize>> = vec![Vec::new(); self.columns.len()];
        let mut card_ordinal: Vec<Vec<usize>> = vec![Vec::new(); self.columns.len()];
        let mut rows: Vec<usize> = vec![0; lane_count];
        for (column_index, column) in self.columns.iter().enumerate() {
            let mut seen: Vec<usize> = vec![0; lane_count];
            for card in &column.cards {
                let lane =
                    self.card_lane_index(&card.id).unwrap_or(0).min(lane_count.saturating_sub(1));
                card_lane[column_index].push(lane);
                let ordinal = seen.get(lane).copied().unwrap_or(0);
                card_ordinal[column_index].push(ordinal);
                if let Some(slot) = seen.get_mut(lane) {
                    *slot += 1;
                    if let Some(row) = rows.get_mut(lane) {
                        *row = (*row).max(*slot);
                    }
                }
            }
        }
        let mut heights = Vec::with_capacity(lane_count);
        let mut tops = Vec::with_capacity(lane_count);
        let mut y = HEADER_HEIGHT as i32;
        // A collapsed lane keeps only its caption strip; its cards are hidden rather than moved,
        // so expanding it again restores the order the model already has (SP-001).
        for (index, lane) in self.lanes.iter().enumerate() {
            if self.is_lane_collapsed(&lane.id) {
                if let Some(row) = rows.get_mut(index) {
                    *row = 0;
                }
            }
        }
        for &row in rows.iter().take(lane_count) {
            tops.push(y);
            let height =
                LANE_HEADER_HEIGHT + row as i32 * (CARD_HEIGHT as i32 + CARD_GAP) + LANE_GAP;
            heights.push(height);
            y += height + LANE_GAP;
        }
        LaneLayout { revision: self.layout_revision, heights, tops, card_lane, card_ordinal }
    }

    /// The lane and in-lane ordinal a card is painted at.
    pub(super) fn lane_slot(&self, position: CardPosition) -> Option<(usize, usize)> {
        let cache = self.lane_cache();
        let lane = cache.card_lane.get(position.column)?.get(position.card).copied()?;
        let ordinal = cache.card_ordinal.get(position.column)?.get(position.card).copied()?;
        Some((lane, ordinal))
    }

    /// The band of lane `index`, spanning every column.
    pub fn lane_band_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.lanes.len() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x + LANE_HEADER_WIDTH as i32,
            self.lane_top(index),
            rect.width.saturating_sub(LANE_HEADER_WIDTH),
            self.lane_height(index).max(1) as u32,
        ))
    }

    /// The label strip of lane `index`, on the left edge of the board.
    pub fn lane_header_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.lanes.len() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            self.lane_top(index),
            LANE_HEADER_WIDTH,
            (self.lane_height(index) - LANE_GAP).max(1) as u32,
        ))
    }

    /// Whether lane `id` is collapsed (SP-001).
    pub fn is_lane_collapsed(&self, id: &str) -> bool {
        self.collapsed_lanes.iter().any(|lane| lane == id)
    }

    /// Collapses or expands lane `id`; returns whether the id named a lane.
    pub fn set_lane_collapsed(&mut self, id: &str, collapsed: bool) -> bool {
        if !self.lanes.iter().any(|lane| lane.id == id) {
            return false;
        }
        let position = self.collapsed_lanes.iter().position(|lane| lane == id);
        match (collapsed, position) {
            (true, None) => self.collapsed_lanes.push(id.to_string()),
            (false, Some(index)) => {
                self.collapsed_lanes.remove(index);
            }
            _ => return true,
        }
        self.invalidate_layout();
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Flips the collapsed state of lane `id`; returns whether the id named a lane.
    pub fn toggle_lane_collapsed(&mut self, id: &str) -> bool {
        self.set_lane_collapsed(id, !self.is_lane_collapsed(id))
    }

    /// The painted box of lane `id`'s collapse chevron, or `None` for an unknown lane.
    pub fn lane_collapse_rect(&self, id: &str) -> Option<Rect> {
        let index = self.lanes.iter().position(|lane| lane.id == id)?;
        let header = self.lane_header_rect(index)?;
        Some(Rect::new(
            header.x + 4,
            header.y + (LANE_HEADER_HEIGHT - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        ))
    }

    /// The lane whose band contains `pos`.
    pub(super) fn lane_at(&self, pos: Point) -> Option<usize> {
        if !self.lanes_active() || !self.geometry().contains_point(pos) {
            return None;
        }
        (0..self.lanes.len()).find(|index| {
            let top = self.lane_top(*index);
            pos.y >= top && pos.y < top + self.lane_height(*index)
        })
    }
}
