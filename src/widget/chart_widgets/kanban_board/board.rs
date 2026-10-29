// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// KanbanBoard — columns of draggable cards.
pub struct KanbanBoard {
    pub(super) base: BaseWidget,
    pub(super) columns: Vec<KanbanColumn>,
    /// The drag in progress, if any.
    pub(super) drag: Option<DragSession>,
    /// Where the dragged card came from, so a dropped card can be removed from its
    /// old place after it is inserted into its new one.
    pub(super) drag_origin: Option<CardPosition>,
    /// The column the pointer is currently over, for the drop preview.
    pub(super) hovered_column: Option<usize>,
    /// The card under the pointer, when no drag is in progress.
    pub(super) hovered_card: Option<CardPosition>,
    /// The card keyboard navigation has reached.
    pub(super) focused_card: Option<CardPosition>,
    /// Per-column scroll offsets, in cards.
    pub(super) column_scroll: Vec<usize>,
    /// Width of one column, in logical pixels.
    pub(super) column_width: u32,
    /// Per-column width overrides, keyed by column id.
    ///
    /// Keyed by id rather than index so an override follows its column across a reorder. A column
    /// without an entry uses [`Self::column_width`], which keeps every existing board at the one
    /// global width it had before overrides existed.
    pub(super) column_widths: HashMap<String, u32>,
    /// How far the columns are scrolled left, in pixels.
    pub(super) horizontal_scroll: i32,
    /// The swimlanes, in display order.
    pub(super) lanes: Vec<KanbanLane>,
    /// Card id to lane id. Cards without an entry render in the first lane.
    pub(super) card_lanes: HashMap<String, String>,
    /// Lanes the user has collapsed, by id (SP-001). A collapsed lane keeps its header strip and
    /// hides its card rows.
    pub(super) collapsed_lanes: Vec<String>,
    /// Whether cards are grouped into lanes.
    pub(super) lane_mode: KanbanLaneMode,
    /// Bumped whenever the board's structure (columns, cards, lanes, lane assignments) changes.
    /// The lane geometry and the card index are keyed on it.
    pub(super) layout_revision: u64,
    /// Lane band geometry and the lane/ordinal of every card, rebuilt at most once per revision.
    pub(super) lane_layout: RefCell<Option<LaneLayout>>,
    /// Card id to its position, rebuilt lazily after a structural change.
    pub(super) card_index: RefCell<Option<HashMap<String, CardPosition>>>,
    /// Explicitly selected cards, in display order.
    pub(super) selected_cards: Vec<String>,
    /// The same selection as `selected_cards`, for O(1) membership on the paint path.
    ///
    /// Every card draw asked the ordered list whether it contained the card's id, which is
    /// `O(selected)` string comparisons per card per frame. The set is rebuilt from the list
    /// whenever the selection is mutated, so the list keeps its display-order contract
    /// (SPX-X13).
    pub(super) selected_set: HashSet<String>,
    /// Emitted after a card moves, with the card's id and its new position.
    pub card_moved: Signal1<(String, CardPosition)>,
    /// Emitted when a card is activated (clicked without dragging), with its id.
    ///
    /// A press followed by a release that never crossed `CARD_DRAG_THRESHOLD` is a
    /// *click*, not a drag: the session is discarded and this fires instead of
    /// [`KanbanBoard::card_moved`]. That is what makes the board usable for opening a
    /// card, and it is the case the sub-threshold drag test pins.
    pub card_activated: Signal1<String>,
    /// Emitted when the card under the pointer changes: its id on entry, `None` on exit.
    pub card_hovered: Signal1<Option<String>>,
    /// Emitted with every selected card id, in display order, after a selection change.
    pub selection_changed: Signal1<Vec<String>>,
    /// Emitted with the ids moved by one drop, when a drop moved more than one card.
    ///
    /// [`Self::card_moved`] keeps its published single-card payload; this companion reports the
    /// batch a multi-selection drop produced without redefining that signal.
    pub cards_moved: Signal1<Vec<String>>,
    /// The drag proxy this board keeps in step, when one is attached.
    pub(super) drag_ghost: Option<DragGhostBinding>,
}

impl Widget for KanbanBoard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Wide enough for two columns, which is the smallest useful board. An overridden column
        // contributes its own width, so the hint does not under-report a widened board.
        Size::new(
            self.column_width_at(0) + self.column_width_at(1) + COLUMN_GAP as u32,
            HEADER_HEIGHT + (CARD_HEIGHT + CARD_GAP as u32) * 3,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "card_moved" => Some(EventSignalRef::mapped("card_moved", &self.card_moved, |v| {
                // The second component is a pair of indices, not its `Debug` spelling: a subscriber
                // that declares `(String, String)` cannot read `"CardPosition { column: 1, card: 0 }"`
                // as a destination, which made the event unusable for the one thing it reports.
                CapabilityValue::Tuple(alloc::vec![
                    CapabilityValue::String(v.0.clone()),
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.1.column as u64),
                        CapabilityValue::UInt(v.1.card as u64),
                    ]),
                ])
            })),
            "card_activated" => {
                Some(EventSignalRef::mapped("card_activated", &self.card_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "card_hovered" => {
                Some(EventSignalRef::mapped("card_hovered", &self.card_hovered, |v| match v {
                    Some(id) => CapabilityValue::String(id.clone()),
                    None => CapabilityValue::Null,
                }))
            }
            "cards_moved" => Some(EventSignalRef::mapped("cards_moved", &self.cards_moved, |v| {
                CapabilityValue::Tuple(
                    v.iter().map(|id| CapabilityValue::String(id.clone())).collect(),
                )
            })),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::Tuple(
                        v.iter().map(|id| CapabilityValue::String(id.clone())).collect(),
                    )
                }))
            }
            _ => None,
        }
    }
}

impl KanbanBoard {
    /// Creates an empty board.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanBoard, geometry, "KanbanBoard"),
            columns: Vec::new(),
            drag: None,
            drag_origin: None,
            hovered_column: None,
            hovered_card: None,
            focused_card: None,
            column_scroll: Vec::new(),
            column_width: DEFAULT_COLUMN_WIDTH,
            column_widths: HashMap::new(),
            horizontal_scroll: 0,
            lanes: Vec::new(),
            collapsed_lanes: Vec::new(),
            card_lanes: HashMap::new(),
            lane_mode: KanbanLaneMode::None,
            layout_revision: 0,
            lane_layout: RefCell::new(None),
            card_index: RefCell::new(None),
            selected_cards: Vec::new(),
            selected_set: HashSet::new(),
            card_moved: Signal1::new(),
            card_activated: Signal1::new(),
            card_hovered: Signal1::new(),
            selection_changed: Signal1::new(),
            cards_moved: Signal1::new(),
            drag_ghost: None,
        }
    }

    /// Adds a column, returning its index.
    pub fn add_column(&mut self, column: KanbanColumn) -> usize {
        let index = self.columns.len();
        self.columns.push(column);
        self.invalidate_layout();
        self.base.request_redraw();
        index
    }

    /// Marks the lane geometry and the card index stale.
    pub(super) fn invalidate_layout(&mut self) {
        self.layout_revision = self.layout_revision.wrapping_add(1);
        *self.card_index.borrow_mut() = None;
    }

    /// Removes the column at `index`, returning it when the index was valid.
    pub fn remove_column(&mut self, index: usize) -> Option<KanbanColumn> {
        if index >= self.columns.len() {
            return None;
        }
        let removed = self.columns.remove(index);
        // The override belongs to the column that left, not to the index it used to occupy.
        self.column_widths.remove(&removed.id);
        // A lane assignment names a card; a card that left the board must not keep one.
        let mut selection_touched = false;
        for card in &removed.cards {
            self.card_lanes.remove(&card.id);
            if self.selected_set.contains(card.id.as_str()) {
                self.selected_cards.retain(|id| *id != card.id);
                selection_touched = true;
            }
        }
        if selection_touched {
            self.rebuild_selected_set();
            self.selection_changed.emit(self.selected_cards.clone());
        }
        // A drag whose origin no longer exists is abandoned, so the release does
        // not try to remove a card from a column that is gone.
        if self.drag_origin.is_some_and(|origin| origin.column >= self.columns.len()) {
            self.cancel_drag();
        }
        self.invalidate_layout();
        self.clamp_horizontal_scroll();
        self.base.request_redraw();
        Some(removed)
    }

    /// Returns the columns.
    pub fn columns(&self) -> &[KanbanColumn] {
        &self.columns
    }

    /// Returns the number of columns.
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns the column at `index`.
    pub fn column(&self, index: usize) -> Option<&KanbanColumn> {
        self.columns.get(index)
    }

    /// Returns a mutable reference to the column at `index`.
    pub fn column_mut(&mut self, index: usize) -> Option<&mut KanbanColumn> {
        // The caller may add, remove or reorder cards through this handle, which this control
        // cannot observe; the derived tables are therefore rebuilt on the next read.
        self.invalidate_layout();
        self.columns.get_mut(index)
    }

    /// Appends `card` to the column at `index`, returning its position.
    ///
    /// Deliberately does **not** enforce the column's WIP limit: seeding a board or
    /// restoring saved state must not have to fight a display constraint owed to
    /// interactive drags.
    pub fn add_card(&mut self, column: usize, card: KanbanCard) -> Option<CardPosition> {
        let column_ref = self.columns.get_mut(column)?;
        let position = CardPosition { column, card: column_ref.cards.len() };
        column_ref.cards.push(card);
        self.invalidate_layout();
        self.base.request_redraw();
        Some(position)
    }

    /// Returns every card on the board as `(position, card)` pairs, in reading
    /// order (column by column, top to bottom).
    pub fn cards(&self) -> Vec<(CardPosition, &KanbanCard)> {
        let mut out = Vec::new();
        for (column, entry) in self.columns.iter().enumerate() {
            for (card, value) in entry.cards.iter().enumerate() {
                out.push((CardPosition { column, card }, value));
            }
        }
        out
    }

    /// Finds a card by id, returning where it is.
    pub fn position_of(&self, card_id: &str) -> Option<CardPosition> {
        self.card_positions().get(card_id).copied()
    }

    /// The id-keyed card index, rebuilt after a structural change.
    pub(super) fn card_positions(&self) -> core::cell::Ref<'_, HashMap<String, CardPosition>> {
        if self.card_index.borrow().is_none() {
            let mut map = HashMap::new();
            for (column_index, column) in self.columns.iter().enumerate() {
                for (card_index, card) in column.cards.iter().enumerate() {
                    // First occurrence wins, matching the linear scan this replaces.
                    map.entry(card.id.clone())
                        .or_insert(CardPosition { column: column_index, card: card_index });
                }
            }
            *self.card_index.borrow_mut() = Some(map);
        }
        core::cell::Ref::map(self.card_index.borrow(), |slot| {
            slot.as_ref().expect("card_positions() populates the index")
        })
    }

    /// Removes a card by id, returning it with the position it occupied.
    pub fn remove_card(&mut self, card_id: &str) -> Option<(KanbanCard, CardPosition)> {
        let position = self.position_of(card_id)?;
        let column = self.columns.get_mut(position.column)?;
        let card = column.cards.remove(position.card);
        self.card_lanes.remove(card_id);
        if self.selected_set.contains(card_id) {
            self.selected_cards.retain(|id| id != card_id);
            self.rebuild_selected_set();
            self.selection_changed.emit(self.selected_cards.clone());
        }
        self.invalidate_layout();
        self.base.request_redraw();
        Some((card, position))
    }

    /// Inserts `card` into `column` at `index`, clamped to the column's length.
    ///
    /// # Why this does not consult the WIP limit
    ///
    /// It is the primitive a *completed* move uses: by the time it runs, the card
    /// has already left its old column and the acceptance question has already been
    /// answered. Checking again here would make a legal move fail when the source
    /// and destination are the same column (where removing first frees the slot).
    pub fn insert_card(
        &mut self,
        column: usize,
        index: usize,
        card: KanbanCard,
    ) -> Option<CardPosition> {
        let column_ref = self.columns.get_mut(column)?;
        let index = index.min(column_ref.cards.len());
        column_ref.cards.insert(index, card);
        self.invalidate_layout();
        self.base.request_redraw();
        Some(CardPosition { column, card: index })
    }

    /// Moves the card at `from` to `to` in one step.
    ///
    /// Returns the card's new position. A `to` that names a nonexistent column, or a
    /// `from` that names no card, is refused rather than silently clamped, because
    /// an index that addresses nothing is a caller error and answering `None` keeps
    /// it visible.
    ///
    /// The removal happens **before** the insert so that moving a card within its
    /// own column computes the target index against the post-removal list, which is
    /// what makes "move down by one" mean one position rather than zero.
    pub fn move_card(&mut self, from: CardPosition, to: CardPosition) -> Option<CardPosition> {
        if to.column >= self.columns.len() {
            return None;
        }
        if from.column >= self.columns.len() || from.card >= self.columns[from.column].cards.len() {
            return None;
        }
        // Removing first makes an in-column move's target index refer to the list
        // the card is being inserted into, not the one it is leaving.
        let card = self.columns[from.column].cards.remove(from.card);
        let card_id = card.id.clone();
        let target = self.insert_card(to.column, to.card, card)?;
        self.invalidate_layout();
        self.base.request_redraw();
        self.card_moved.emit((card_id, target));
        Some(target)
    }

    /// Returns the width of one column.
    ///
    /// This is the board-wide default; a column with an override reports it from
    /// [`Self::column_width_at`] instead.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Sets the width of one column (floored at `MIN_COLUMN_WIDTH`).
    pub fn set_column_width(&mut self, width: u32) {
        let width = width.max(MIN_COLUMN_WIDTH);
        if self.column_width == width {
            return;
        }
        self.column_width = width;
        self.clamp_horizontal_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns how far the columns are scrolled left, in pixels.
    pub fn horizontal_scroll(&self) -> i32 {
        self.horizontal_scroll
    }

    /// Scrolls the board horizontally to `offset` pixels (clamped to the content).
    pub fn set_horizontal_scroll(&mut self, offset: i32) {
        let next = offset.max(0).min(self.max_horizontal_scroll());
        if self.horizontal_scroll == next {
            return;
        }
        self.horizontal_scroll = next;
        self.base.request_redraw();
    }

    /// Scrolls the board horizontally by `delta` pixels.
    pub fn scroll_horizontally_by(&mut self, delta: i32) {
        self.set_horizontal_scroll(self.horizontal_scroll.saturating_add(delta));
    }

    /// The largest scroll offset that still shows content.
    pub(super) fn max_horizontal_scroll(&self) -> i32 {
        if self.columns.is_empty() {
            return 0;
        }
        let content: i32 = (0..self.columns.len())
            .map(|index| self.column_width_at(index) as i32 + COLUMN_GAP)
            .sum::<i32>()
            - COLUMN_GAP;
        let lane_strip = if self.lanes_active() { LANE_HEADER_WIDTH as i32 } else { 0 };
        let viewport = (self.geometry().width as i32 - lane_strip).max(0);
        (content - viewport).max(0)
    }

    /// Re-clamps the scroll after the content or viewport width changed.
    pub(super) fn clamp_horizontal_scroll(&mut self) {
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
    }

    // ── Keyboard and column scrolling ──────────────────────────────────────

    /// The card keyboard navigation has reached.
    pub fn focused_card(&self) -> Option<CardPosition> {
        self.focused_card
    }

    /// The card under the pointer, when no drag is in progress.
    pub fn hovered_card(&self) -> Option<CardPosition> {
        self.hovered_card
    }

    /// The explicitly selected card ids, in display order.
    pub fn selected_cards(&self) -> &[String] {
        &self.selected_cards
    }

    /// Selects or deselects a card by id, returning whether the id resolved.
    pub fn set_card_selected(&mut self, card_id: &str, selected: bool) -> bool {
        if self.position_of(card_id).is_none() {
            return false;
        }
        let present = self.selected_set.contains(card_id);
        if present == selected {
            return true;
        }
        if selected {
            self.selected_cards.push(card_id.to_string());
        } else {
            self.selected_cards.retain(|id| id != card_id);
        }
        self.sort_selection();
        self.selection_changed.emit(self.selected_cards.clone());
        self.base.request_redraw();
        true
    }

    /// Toggles a card's membership in the selection.
    pub fn toggle_card_selected(&mut self, card_id: &str) -> bool {
        let present = self.selected_set.contains(card_id);
        self.set_card_selected(card_id, !present)
    }

    /// Clears the whole selection.
    pub fn clear_selection(&mut self) {
        if self.selected_cards.is_empty() {
            return;
        }
        self.selected_cards.clear();
        self.selected_set.clear();
        self.selection_changed.emit(Vec::new());
        self.base.request_redraw();
    }

    /// Re-orders the selection into reading order and drops ids that no longer name a card.
    pub(super) fn sort_selection(&mut self) {
        if !self.selected_cards.is_empty() {
            // `set_card_selected` has already mutated the list but not the set, so the set is
            // rebuilt from the list as it stands before it is consulted per card.
            self.rebuild_selected_set();
            let mut ordered = Vec::with_capacity(self.selected_cards.len());
            for column in &self.columns {
                for card in &column.cards {
                    if self.selected_set.contains(card.id.as_str()) {
                        ordered.push(card.id.clone());
                    }
                }
            }
            self.selected_cards = ordered;
        }
        // Also the point where ids that no longer name a card are dropped, so the set is rebuilt
        // from the list the ordering produced.
        self.rebuild_selected_set();
    }

    /// Rebuilds `selected_set` from `selected_cards` (O(selected)).
    ///
    /// Called at every place the ordered selection is mutated; the paint path then tests one
    /// card id in constant time (SPX-X13).
    pub(super) fn rebuild_selected_set(&mut self) {
        self.selected_set.clear();
        self.selected_set.reserve(self.selected_cards.len());
        for id in &self.selected_cards {
            self.selected_set.insert(id.clone());
        }
    }

    /// Moves keyboard focus to `position`, scrolling its column when needed.
    pub fn set_focused_card(&mut self, position: Option<CardPosition>) {
        if let Some(position) = position {
            if position.column >= self.columns.len()
                || position.card >= self.columns[position.column].cards.len()
            {
                return;
            }
        }
        if self.focused_card == position {
            return;
        }
        self.focused_card = position;
        self.ensure_focused_visible();
        self.base.request_redraw();
    }

    /// Scrolls the focused card's column so the card's lane is visible.
    pub(super) fn ensure_focused_visible(&mut self) {
        // Lanes lay out every card in its band; there is no per-column window to scroll.
        if self.lanes_active() {
            return;
        }
        let Some(position) = self.focused_card else {
            return;
        };
        let visible = self.visible_cards_per_column();
        if self.column_scroll.len() <= position.column {
            self.column_scroll.resize(position.column + 1, 0);
        }
        let scroll = self.column_scroll[position.column];
        if position.card < scroll {
            self.column_scroll[position.column] = position.card;
        } else if position.card >= scroll + visible {
            self.column_scroll[position.column] = position.card + 1 - visible;
        }
    }

    /// The number of cards a column shows at once.
    pub(super) fn visible_cards_per_column(&self) -> usize {
        let rect = self.geometry();
        ((rect.height.saturating_sub(HEADER_HEIGHT)) / (CARD_HEIGHT + CARD_GAP as u32)).max(1)
            as usize
    }

    /// Scrolls `column` by `delta` cards.
    pub fn scroll_column(&mut self, column: usize, delta: isize) {
        // Lane mode gives each band all the height its busiest column needs, so a column is never
        // clipped and has nothing to scroll.
        if self.lanes_active() {
            return;
        }
        let Some(entry) = self.columns.get(column) else {
            return;
        };
        let max = entry.cards.len().saturating_sub(self.visible_cards_per_column());
        if self.column_scroll.len() <= column {
            self.column_scroll.resize(column + 1, 0);
        }
        let next = (self.column_scroll[column] as isize + delta).clamp(0, max as isize) as usize;
        if self.column_scroll[column] == next {
            return;
        }
        self.column_scroll[column] = next;
        self.base.request_redraw();
    }

    // ── Drag handling ───────────────────────────────────────────────────────

    /// Whether a card drag is in progress.
    pub fn is_dragging_card(&self) -> bool {
        self.drag.as_ref().is_some_and(DragSession::is_active)
    }

    /// The id of the card being dragged, if any.
    pub fn dragged_card_id(&self) -> Option<&str> {
        self.drag.as_ref().map(|session| session.payload().item_id.as_str())
    }

    /// The column the pointer is currently over during a drag.
    ///
    /// Exposed so a container can auto-scroll toward it when the board is wider
    /// than its viewport — the one piece of drag feedback this control cannot
    /// provide for itself.
    pub fn hovered_column(&self) -> Option<usize> {
        self.hovered_column
    }
}
