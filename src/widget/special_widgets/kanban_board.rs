// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! KanbanBoard — columns of cards, reordered and moved between columns by drag.
//!
//! # Why this is a control of its own
//!
//! The data model is a **two-level ordered structure with cross-container moves**:
//! columns hold cards, a card's position is `(column, index)`, and a drag can change
//! both coordinates at once. Nothing else in this crate holds that shape:
//!
//! * `ListView` / `ListBox` are one flat ordered list — no second level, and a drag
//!   can only reorder within the one list;
//! * `TreeView` is a tree — its parent/child relation is *containment*, and a node
//!   cannot be a member of two parents or be positioned between two siblings by
//!   index alone;
//! * `MasonryLayout` and `Grid` place children but have no model to reorder.
//!
//! So this is a genuine gap rather than a `ListView` variant (rule #78's test:
//! could an existing control's data model carry it? No).
//!
//! # This is the first consumer of `event::dnd`
//!
//! The drag state machine is [`DragSession`], not a hand-written press/move/release
//! loop. The board supplies the two domain decisions the shared machine cannot make:
//! **which card was grabbed** (a geometry question about this control's layout) and
//! **what a drop means** (an index inside a column). `KanbanBoard` implements
//! [`DropTarget`] on itself, so a card dropped anywhere over the board resolves
//! through the same `can_accept` / `on_drop` path a foreign drag would.
//!
//! # Reachability
//!
//! Registered in the widget factory as `kanban_board` (aliases `kanban`, `board`), so
//! it is reachable by name from the declarative JSON path (`"kanbanboard"`), from a
//! CSS selector (`KanbanBoard`), and through the typed `create_kanban_board` on
//! `ControlBackend`.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::dnd::{DragPayload, DragSession, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The drag payload type every card of this control carries.
///
/// Namespaced with `kanban_` so a board that shares a window with another
/// drag source cannot mistake an unrelated payload for one of its cards.
const CARD_PAYLOAD_TYPE: &str = "kanban_card";

/// How far the pointer must travel before a press on a card becomes a drag.
///
/// Four pixels: large enough that a click on a card's label is not read as a
/// one-pixel drag, small enough that a deliberate move registers immediately.
const CARD_DRAG_THRESHOLD: i32 = 4;

/// How wide each column is, in logical pixels, and the gap between them.
const COLUMN_WIDTH: u32 = 220;
const COLUMN_GAP: i32 = 12;

/// The height of a column's header strip.
const HEADER_HEIGHT: u32 = 32;

/// The height of one card, and the vertical gap between cards.
const CARD_HEIGHT: u32 = 48;
const CARD_GAP: i32 = 8;

/// The horizontal padding of a card's or a column header's text from its own box edge: 10.
///
/// One value for two labels that share a leading edge, so a column's header title and the card
/// titles below it line up rather than each deriving its own inset.
const CARD_PADDING: i32 = 10;

/// A card on the board.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanCard {
    /// Stable identifier, unique across the whole board (not just its column).
    ///
    /// Unique board-wide because a move changes the card's column: an id that was
    /// only unique within a column would stop identifying the card the moment it
    /// moved, and the caller's own data would have no way back to it.
    pub id: String,
    /// Text drawn on the card.
    pub title: String,
    /// Free-form text drawn under the title, in a smaller face.
    pub description: String,
    /// Whether the card is drawn marked as done.
    pub done: bool,
}

impl KanbanCard {
    /// Creates a card with `id` and `title`.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), description: String::new(), done: false }
    }

    /// Sets the description line.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Sets the done flag.
    pub fn with_done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

/// A column of the board.
#[derive(Debug, Clone, PartialEq)]
pub struct KanbanColumn {
    /// Stable identifier, unique across the board.
    pub id: String,
    /// Header text.
    pub title: String,
    /// The cards in order, top to bottom.
    pub cards: Vec<KanbanCard>,
    /// Whether the column is collapsed to its header.
    pub collapsed: bool,
    /// The column's work-in-progress limit, if it has one.
    ///
    /// A limit is a **soft** cap: the board refuses new cards past it through
    /// [`KanbanBoard::can_accept`], but a caller that pushes cards in directly
    /// through [`KanbanBoard::add_card`] is not stopped. The distinction is
    /// deliberate — a drag from the user must be refused (otherwise the limit is
    /// decorative), while programmatic seeding should not have to fight it.
    pub wip_limit: Option<usize>,
}

impl KanbanColumn {
    /// Creates an empty column with `id` and `title`.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            cards: Vec::new(),
            collapsed: false,
            wip_limit: None,
        }
    }

    /// Sets the work-in-progress limit.
    pub fn with_wip_limit(mut self, limit: usize) -> Self {
        self.wip_limit = Some(limit);
        self
    }

    /// Whether this column is at or over its limit.
    pub fn is_at_wip_limit(&self) -> bool {
        self.wip_limit.is_some_and(|limit| self.cards.len() >= limit)
    }
}

/// Where a card is: which column, and its index within that column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardPosition {
    /// Index of the column.
    pub column: usize,
    /// Index of the card within the column.
    pub card: usize,
}

/// KanbanBoard — columns of draggable cards.
pub struct KanbanBoard {
    base: BaseWidget,
    columns: Vec<KanbanColumn>,
    /// The drag in progress, if any.
    drag: Option<DragSession>,
    /// Where the dragged card came from, so a dropped card can be removed from its
    /// old place after it is inserted into its new one.
    drag_origin: Option<CardPosition>,
    /// The column the pointer is currently over, for the drop preview.
    hovered_column: Option<usize>,
    /// Emitted after a card moves, with the card's id and its new position.
    pub card_moved: Signal1<(String, CardPosition)>,
    /// Emitted when a card is activated (clicked without dragging), with its id.
    ///
    /// A press followed by a release that never crossed `CARD_DRAG_THRESHOLD` is a
    /// *click*, not a drag: the session is discarded and this fires instead of
    /// [`KanbanBoard::card_moved`]. That is what makes the board usable for opening a
    /// card, and it is the case the sub-threshold drag test pins.
    pub card_activated: Signal1<String>,
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
            card_moved: Signal1::new(),
            card_activated: Signal1::new(),
        }
    }

    /// Adds a column, returning its index.
    pub fn add_column(&mut self, column: KanbanColumn) -> usize {
        let index = self.columns.len();
        self.columns.push(column);
        self.base.request_redraw();
        index
    }

    /// Removes the column at `index`, returning it when the index was valid.
    pub fn remove_column(&mut self, index: usize) -> Option<KanbanColumn> {
        if index >= self.columns.len() {
            return None;
        }
        let removed = self.columns.remove(index);
        // A drag whose origin no longer exists is abandoned, so the release does
        // not try to remove a card from a column that is gone.
        if self.drag_origin.is_some_and(|origin| origin.column >= self.columns.len()) {
            self.cancel_drag();
        }
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
        for (column, entry) in self.columns.iter().enumerate() {
            if let Some(card) = entry.cards.iter().position(|c| c.id == card_id) {
                return Some(CardPosition { column, card });
            }
        }
        None
    }

    /// Removes a card by id, returning it with the position it occupied.
    pub fn remove_card(&mut self, card_id: &str) -> Option<(KanbanCard, CardPosition)> {
        let position = self.position_of(card_id)?;
        let column = self.columns.get_mut(position.column)?;
        let card = column.cards.remove(position.card);
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
        self.base.request_redraw();
        self.card_moved.emit((card_id, target));
        Some(target)
    }

    // ── Geometry ────────────────────────────────────────────────────────────

    /// The rectangle of the column at `index`.
    ///
    /// Columns are laid out left to right at a fixed width, which is what makes the
    /// board scrollable horizontally by its container rather than reflowing.
    pub fn column_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.columns.len() {
            return None;
        }
        let rect = self.geometry();
        let x = rect.x + (index as i32) * (COLUMN_WIDTH as i32 + COLUMN_GAP);
        Some(Rect::new(x, rect.y, COLUMN_WIDTH, rect.height))
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
        let y = column_rect.y
            + HEADER_HEIGHT as i32
            + (position.card as i32) * (CARD_HEIGHT as i32 + CARD_GAP);
        Some(Rect::new(column_rect.x, y, COLUMN_WIDTH, CARD_HEIGHT))
    }

    /// The column containing `pos`, when the pointer is over one.
    fn column_at(&self, pos: Point) -> Option<usize> {
        (0..self.columns.len())
            .find(|index| self.column_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// The card under `pos`, when the pointer is over one.
    fn card_at(&self, pos: Point) -> Option<CardPosition> {
        let column = self.column_at(pos)?;
        let entries = self.columns.get(column)?;
        for index in 0..entries.cards.len() {
            let position = CardPosition { column, card: index };
            if self.card_rect(position).is_some_and(|rect| rect.contains_point(pos)) {
                return Some(position);
            }
        }
        None
    }

    /// The index a card dropped at `pos` would take within `column`.
    ///
    /// Computed from the pointer's y against the laid-out cards, so dropping in the
    /// gap between two cards inserts between them rather than being snapped to one
    /// of their indices. A drop below the last card appends.
    fn insert_index_at(&self, column: usize, pos: Point) -> usize {
        let Some(entries) = self.columns.get(column) else {
            return 0;
        };
        let Some(column_rect) = self.column_rect(column) else {
            return 0;
        };
        // Cards start below the header; a drop in the header goes to the top.
        let first_card_y = column_rect.y + HEADER_HEIGHT as i32;
        if pos.y <= first_card_y {
            return 0;
        }
        // The midpoint rule: a pointer past a card's vertical centre inserts after
        // it. This is what makes the drop land where the user's cursor is rather
        // than always above the card under it.
        let stride = CARD_HEIGHT as i32 + CARD_GAP;
        let offset = pos.y - first_card_y;
        let index = (offset + stride / 2) / stride;
        // Clamped to the column's length: a pointer below the last card appends.
        (index as usize).min(entries.cards.len())
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

    /// Abandons the drag in progress.
    pub fn cancel_drag(&mut self) {
        if self.drag.take().is_some() {
            self.drag_origin = None;
            self.hovered_column = None;
            self.base.request_redraw();
        }
    }

    /// Begins a drag on the card at `position`.
    fn begin_card_drag(&mut self, position: CardPosition, pos: Point) {
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
    fn update_card_drag(&mut self, pos: Point) {
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
    fn finish_card_drag(&mut self, pos: Point) -> bool {
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
        if current == target {
            // A drop that changes nothing is accepted (the user did place the card)
            // but moves nothing, so no signal is emitted for a no-op.
            self.base.request_redraw();
            return false;
        }
        self.move_card(current, target).is_some()
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
        self.column_rect(column)
    }
}

impl Widget for KanbanBoard {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Wide enough for two columns, which is the smallest useful board.
        Size::new(
            COLUMN_WIDTH * 2 + COLUMN_GAP as u32,
            HEADER_HEIGHT + (CARD_HEIGHT + CARD_GAP as u32) * 3,
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `KanbanBoard`'s property contract.
///
/// The columns and cards are two-level lists, written through `add_column` /
/// `add_card`; the property layer reports the derived counts and the one display
/// flag, matching `Meter`'s and `RadarChart`'s convention for list-valued state.
impl WidgetProperties for KanbanBoard {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "card_count" => Ok(CapabilityValue::UInt(self.cards().len() as u64)),
            "dragging_card_id" => Ok(match self.dragged_card_id() {
                Some(id) => CapabilityValue::String(id.to_string()),
                None => CapabilityValue::Null,
            }),
            "hovered_column" => Ok(match self.hovered_column() {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "column_width" => Ok(CapabilityValue::UInt(COLUMN_WIDTH as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The card count is derived from the columns; the drag state is written
            // by the pointer. Neither has a meaningful value a caller could set.
            "column_count" | "card_count" | "dragging_card_id" | "hovered_column" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            // Reported so a caller can size a container against it, but the layout
            // constant is not per-instance state.
            "column_width" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `KANBAN_BOARD_PROPERTIES`.
        property_names_of![
            "column_count",
            "card_count",
            "dragging_card_id",
            "hovered_column",
            "column_width",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `kanban_board` publishes.
    ///
    /// `cancel_drag` is the one published command whose whole effect is to discard
    /// state, so it is the only one that can run with no argument and it executes
    /// here. The other three all describe *what* to add or where to move it — a
    /// `KanbanColumn`, a `KanbanCard`, a `CardPosition` pair — and the control has no
    /// defensible default for any of them, so they are refused as
    /// [`CapabilityAccessError::OutOfRange`]: the names are valid and the payloads
    /// are what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "cancel_drag" => {
                self.cancel_drag();
                Ok(())
            }
            "add_column" | "add_card" | "move_card" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for KanbanBoard {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(position) = self.card_at(*pos) {
                    self.begin_card_drag(position, *pos);
                }
            }
            Event::MouseMove { pos } if self.drag.is_some() => {
                self.update_card_drag(*pos);
            }
            Event::MouseRelease { pos, button } if *button == 1 && self.drag.is_some() => {
                self.finish_card_drag(*pos);
            }
            // A drag that loses the pointer (the window lost focus, the pointer left
            // the surface) must not leave a card half-attached to the cursor.
            Event::MouseLeave { .. } => self.cancel_drag(),
            _ => {}
        }
    }
}

impl Draw for KanbanBoard {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("kanban_board");
        // `kanban_board` is not a control kind in the role table, so it classifies as
        // `Surface` — whose background is `theme.colors.background`, the very colour a
        // window paints. Filling the whole rect with it would make the board
        // indistinguishable from the window behind it (the census reports that as
        // "painted nothing"), so the board's own fill is a step toward the foreground
        // and the window keeps its colour.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        context.fill_rect(rect, background);

        // Every colour the board paints below is derived from this one resolved
        // triple, so a theme switch moves the whole board rather than the frame
        // only. The column, its header strip, its cards and the drag feedback are
        // all chrome states of the same surface.
        let chrome = BoardChrome { background, border, text_color };

        for index in 0..self.columns.len() {
            self.draw_column(context, index, &chrome);
        }
        // The drag overlay is painted last so the card follows the pointer over
        // every column rather than being clipped to the one it started in.
        self.draw_drag_overlay(context, &chrome);
    }
}

/// The board's resolved chrome colours, threaded through the drawing helpers.
///
/// Gathered into one value so every column, card and drag indicator reads the
/// same appearance-resolved colours instead of re-resolving the theme per row,
/// and so no helper can drift back to a literal.
#[derive(Debug, Clone, Copy)]
struct BoardChrome {
    /// The board's own fill, and the base every other colour is derived from.
    background: Color,
    /// The resolved border colour, used for card outlines.
    border: Color,
    /// The resolved foreground, used for text and as the tint direction.
    text_color: Color,
}

impl BoardChrome {
    /// A column's fill: a step away from the board so the columns read as lanes.
    ///
    /// The step is large enough to survive 8-bit rounding: at the default palette a
    /// 4% step rounds to the board's own bytes, which leaves the columns invisible.
    fn column(&self) -> Color {
        self.background.blend(&self.text_color, 0.18)
    }

    /// The column a dragged card is hovering over, tinted toward the text colour
    /// so the drop target is visible in either appearance.
    fn drop_target(&self) -> Color {
        self.background.blend(&self.text_color, 0.3)
    }

    /// The column header strip, a second step away from the column body.
    fn header(&self) -> Color {
        self.background.blend(&self.text_color, 0.24)
    }

    /// Header text, and the ordinary card title.
    fn text(&self) -> Color {
        self.text_color
    }

    /// The card count and other secondary labels.
    fn muted_text(&self) -> Color {
        self.text_color.blend(&self.background, 0.3)
    }

    /// The card surface, one step toward the text colour from the board so a card
    /// reads as raised above its column.
    fn card(&self) -> Color {
        self.background.blend(&self.text_color, 0.1)
    }

    /// A card that has been marked done: title text muted, like the count badge.
    fn done_text(&self) -> Color {
        self.text_color.blend(&self.background, 0.45)
    }

    /// The WIP limit is a *state* the user has to act on, so it reads the theme's
    /// error token rather than a literal red.
    fn at_limit(&self) -> Color {
        crate::style::semantic_color(crate::style::SemanticColor::Error)
            .map(|token| token.blend(&self.background, 0.2))
            .unwrap_or_else(|| self.text_color.blend(&self.background, 0.3))
    }

    /// The insertion indicator shown where a dragged card would land. This is the
    /// theme's primary slot, read from the board's resolved border colour so it
    /// moves with the appearance instead of staying a fixed blue.
    fn insertion(&self) -> Color {
        self.border.blend(&self.text_color, 0.3)
    }

    /// The colour a card's own drag ghost is filled with.
    fn ghost(&self) -> Color {
        self.background.blend(&self.text_color, 0.08)
    }
}

impl KanbanBoard {
    /// Draws one column: its background, header, cards and the drop preview.
    fn draw_column(&mut self, context: &mut RenderContext, index: usize, chrome: &BoardChrome) {
        let Some(column_rect) = self.column_rect(index) else {
            return;
        };
        let (title, collapsed, cards, at_limit) = {
            let Some(column) = self.columns.get(index) else {
                return;
            };
            (column.title.clone(), column.collapsed, column.cards.clone(), column.is_at_wip_limit())
        };

        // The drop preview is drawn under the cards, so a highlighted column still
        // shows its contents.
        let is_drop_target = self.is_dragging_card() && self.hovered_column == Some(index);
        let background = if is_drop_target { chrome.drop_target() } else { chrome.column() };
        context.fill_rounded_rect(column_rect, 8, background);

        // Header height is `HEADER_HEIGHT`; the title and the count badge are centred in it
        // through the shared primitive rather than at a literal `+ 21`, which was a third of a
        // 32 px header and moved neither with the header nor with the font.
        let header_rect = Rect::new(column_rect.x, column_rect.y, column_rect.width, HEADER_HEIGHT);
        context.fill_rounded_rect(header_rect, 8, chrome.header());
        // A WIP limit is only worth showing when it is reached, which is the state a
        // user has to act on.
        let header_color = if at_limit { chrome.at_limit() } else { chrome.text() };
        let header_font = Font::simple("Sans", 13.0);
        let header_line = context.text_line(header_rect, &header_font);
        context.draw_text(
            Point::new(header_rect.x + CARD_PADDING, header_line.y),
            &title,
            &header_font,
            header_color,
            HorizontalAlignment::Left,
        );
        // Card count on the right of the header, or the collapsed marker.
        let badge = if collapsed {
            format!("({})", cards.len())
        } else if let Some(limit) = self.columns.get(index).and_then(|c| c.wip_limit) {
            format!("{}/{}", cards.len(), limit)
        } else {
            cards.len().to_string()
        };
        let badge_font = Font::simple("Sans", 11.0);
        let badge_metrics = context.measure_text(&badge, &badge_font);
        let badge_line = context.text_line(header_rect, &badge_font);
        context.draw_text(
            Point::new(
                header_rect.x + header_rect.width as i32
                    - CARD_PADDING
                    - badge_metrics.width as i32,
                badge_line.y,
            ),
            &badge,
            &badge_font,
            chrome.muted_text(),
            HorizontalAlignment::Left,
        );

        if collapsed {
            return;
        }

        for (card_index, card) in cards.iter().enumerate() {
            let position = CardPosition { column: index, card: card_index };
            let Some(card_rect) = self.card_rect(position) else {
                continue;
            };
            // Clipped to the column so a card taller than the remaining space does
            // not paint over the column beside it.
            context.push_clip(column_rect.x, column_rect.y, column_rect.width, column_rect.height);
            self.draw_card(context, card, card_rect, position, chrome);
            context.pop_clip();
        }
    }

    /// Draws one card, marking it when it is the one being dragged.
    fn draw_card(
        &self,
        context: &mut RenderContext,
        card: &KanbanCard,
        card_rect: Rect,
        position: CardPosition,
        chrome: &BoardChrome,
    ) {
        let is_dragged = self.drag_origin == Some(position) && self.is_dragging_card();
        // The dragged card stays drawn in place but faded, so the user can see both
        // where it came from and where it is going.
        let alpha = if is_dragged { 90 } else { 255 };
        context.fill_rounded_rect(card_rect, 6, chrome.card().with_alpha(alpha));
        context.draw_rounded_rect_stroke(card_rect, 6, chrome.border.with_alpha(alpha), 1);

        let title_color = if card.done {
            chrome.done_text().with_alpha(alpha)
        } else {
            chrome.text().with_alpha(alpha)
        };
        let title = if card.done { format!("✓ {}", card.title) } else { card.title.clone() };
        // The title and the description are placed from the card's own box: the literals
        // `+ 20` and `+ 38` described a 48 px card, so any other `CARD_HEIGHT` drew the
        // description past the card's bottom edge. A card is `CARD_HEIGHT` tall by contract.
        let title_font = Font::simple("Sans", 12.0);
        let desc_font = Font::simple("Sans", 10.0);
        let title_line = context.text_line(card_rect, &title_font);
        if !title.is_empty() {
            context.draw_text(
                Point::new(card_rect.x + CARD_PADDING, title_line.y),
                &title,
                &title_font,
                title_color,
                HorizontalAlignment::Left,
            );
        }
        if !card.description.is_empty() {
            // The description occupies the card's lower half, so the two lines never overlap
            // whatever the card's height is.
            let desc_band = Rect::new(
                card_rect.x,
                card_rect.y + (card_rect.height / 2) as i32,
                card_rect.width,
                card_rect.height / 2,
            );
            let desc_line = context.text_line(desc_band, &desc_font);
            context.draw_text(
                Point::new(card_rect.x + CARD_PADDING, desc_line.y),
                &card.description,
                &desc_font,
                chrome.muted_text().with_alpha(alpha),
                HorizontalAlignment::Left,
            );
        }
    }

    /// Draws the card following the pointer.
    ///
    /// Uses [`DropTarget::preview_rect`] on this board to find the landing column,
    /// so the preview and the commit read one answer rather than two rules.
    fn draw_drag_overlay(&mut self, context: &mut RenderContext, chrome: &BoardChrome) {
        let Some(session) = self.drag.as_ref() else {
            return;
        };
        if !session.is_active() {
            return;
        }
        let current = session.current();
        let preview = self.preview_rect(session.payload(), current);

        // A landing indicator at the resolved position: an insertion line where the
        // card would go, which is the feedback that makes the drop predictable.
        if let Some(column_rect) = preview {
            let insert_index = self.insert_index_at(self.hovered_column.unwrap_or(0), current);
            let line_y = column_rect.y
                + HEADER_HEIGHT as i32
                + (insert_index as i32) * (CARD_HEIGHT as i32 + CARD_GAP)
                - CARD_GAP / 2;
            context.draw_line_stroke(
                Point::new(column_rect.x + 6, line_y),
                Point::new(column_rect.x + column_rect.width as i32 - 6, line_y),
                chrome.insertion(),
                3,
            );
        }

        // The card itself, centred on the pointer and semi-transparent so the board
        // underneath stays readable. It is clamped to the board, because a ghost centred on a
        // pointer near the edge would otherwise paint over whatever the layout placed beside
        // the board — nothing clips a widget at this layer.
        let label = session.payload().label.clone();
        let board = self.geometry();
        let ghost_x = (current.x - (COLUMN_WIDTH as i32 / 2))
            .clamp(board.x, board.x + board.width.saturating_sub(COLUMN_WIDTH) as i32);
        let ghost_y = (current.y - (CARD_HEIGHT as i32 / 2))
            .clamp(board.y, board.y + board.height.saturating_sub(CARD_HEIGHT) as i32);
        let ghost = Rect::new(ghost_x, ghost_y, COLUMN_WIDTH, CARD_HEIGHT);
        context.fill_rounded_rect(ghost, 6, chrome.ghost().with_alpha(230));
        context.draw_rounded_rect_stroke(ghost, 6, chrome.insertion(), 2);
        if !label.is_empty() {
            let ghost_font = Font::simple("Sans", 12.0);
            let ghost_line = context.text_line(ghost, &ghost_font);
            context.draw_text(
                Point::new(ghost.x + CARD_PADDING, ghost_line.y),
                &label,
                &ghost_font,
                chrome.text(),
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    // The activation tests observe delivery through a shared sink, so they need the
    // same Arc/Mutex the signal machinery uses.
    use crate::compat::{Arc, Mutex};

    /// A board with three columns and one card in the first.
    fn board() -> KanbanBoard {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_column(KanbanColumn::new("doing", "Doing"));
        board.add_column(KanbanColumn::new("done", "Done"));
        board.add_card(0, KanbanCard::new("a", "First"));
        board
    }

    /// Renders the board and returns the RGBA frame.
    fn render(board: &mut KanbanBoard, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        board.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The centre of the card at `position`.
    fn card_center(board: &KanbanBoard, position: CardPosition) -> Point {
        let rect = board.card_rect(position).expect("card is laid out");
        Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2)
    }

    #[test]
    fn kanban_creation_defaults() {
        let board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        assert_eq!(board.kind(), WidgetKind::KanbanBoard);
        assert_eq!(board.column_count(), 0);
        assert!(board.cards().is_empty());
        assert!(!board.is_dragging_card());
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_add_and_remove_columns() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        board.add_column(KanbanColumn::new("a", "A"));
        board.add_column(KanbanColumn::new("b", "B"));
        assert_eq!(board.column_count(), 2);
        assert_eq!(board.column(0).map(|c| c.title.as_str()), Some("A"));

        let removed = board.remove_column(0).expect("column 0");
        assert_eq!(removed.id, "a");
        assert_eq!(board.column_count(), 1);
        assert!(board.remove_column(9).is_none());
    }

    #[test]
    fn kanban_add_card_ignores_wip_limit() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        board.add_column(KanbanColumn::new("wip", "WIP").with_wip_limit(1));
        // Seeding is not an interactive drag; the limit is a display constraint on
        // drags, not a hard invariant.
        assert!(board.add_card(0, KanbanCard::new("a", "A")).is_some());
        assert!(board.add_card(0, KanbanCard::new("b", "B")).is_some());
        assert_eq!(board.column(0).unwrap().cards.len(), 2);
        assert!(board.column(0).unwrap().is_at_wip_limit());
    }

    #[test]
    fn kanban_add_card_rejects_unknown_column() {
        let mut board = board();
        assert!(board.add_card(9, KanbanCard::new("x", "X")).is_none());
    }

    #[test]
    fn kanban_position_of_finds_a_card_anywhere() {
        let mut board = board();
        board.add_card(1, KanbanCard::new("b", "Second"));
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
        assert_eq!(board.position_of("b"), Some(CardPosition { column: 1, card: 0 }));
        assert_eq!(board.position_of("missing"), None);
    }

    #[test]
    fn kanban_move_card_across_columns() {
        let mut board = board();
        let moved = board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 1, card: 0 });
        assert_eq!(moved, Some(CardPosition { column: 1, card: 0 }));
        assert!(board.column(0).unwrap().cards.is_empty());
        assert_eq!(board.column(1).unwrap().cards[0].id, "a");
    }

    #[test]
    fn kanban_move_card_within_a_column() {
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_card(0, KanbanCard::new("c", "Third"));

        // Move the first card to the end.
        let moved = board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 0, card: 2 });
        assert!(moved.is_some());
        let ids: Vec<&str> = board.column(0).unwrap().cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn kanban_move_card_down_by_one_lands_one_step_lower() {
        // The removal-first rule is what makes this true. Computing the target index
        // against the pre-removal list would leave the card where it started.
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_card(0, KanbanCard::new("c", "Third"));

        board.move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 0, card: 1 });
        let ids: Vec<&str> = board.column(0).unwrap().cards.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a", "c"], "a moved down exactly one place");
    }

    #[test]
    fn kanban_move_card_rejects_bad_indices() {
        let mut board = board();
        // Target column does not exist.
        assert!(board
            .move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 9, card: 0 })
            .is_none());
        // Source card does not exist.
        assert!(board
            .move_card(CardPosition { column: 0, card: 9 }, CardPosition { column: 1, card: 0 })
            .is_none());
        // Nothing was lost.
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    #[test]
    fn kanban_move_card_emits_the_signal() {
        let mut board = board();
        let moved =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, usize, usize)>::new()));
        let sink = moved.clone();
        board.card_moved.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((value.0.clone(), value.1.column, value.1.card));
            }
        });

        board.move_card(CardPosition { column: 0, card: 0 }, CardPosition { column: 2, card: 0 });
        assert_eq!(*moved.lock().expect("signal lock poisoned"), vec![("a".to_string(), 2, 0)]);
    }

    #[test]
    fn kanban_remove_card_returns_it_with_its_position() {
        let mut board = board();
        let (card, position) = board.remove_card("a").expect("card a");
        assert_eq!(card.title, "First");
        assert_eq!(position, CardPosition { column: 0, card: 0 });
        assert!(board.remove_card("a").is_none());
    }

    // ── Drag and drop (the `event::dnd` integration) ─────────────────────────

    #[test]
    fn kanban_press_on_a_card_opens_a_drag() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        assert_eq!(board.dragged_card_id(), Some("a"));
        // Not yet active: a press alone is a candidate click.
        assert!(!board.is_dragging_card());
    }

    #[test]
    fn kanban_press_away_from_a_card_opens_no_drag() {
        let mut board = board();
        // Below the only card, still inside the column.
        board.handle_event(&Event::mouse_press(100, 300, 1));
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_drag_below_threshold_stays_a_click() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        board.handle_event(&Event::mouse_move(centre.x + 1, centre.y));
        assert!(!board.is_dragging_card());
    }

    /// A sub-threshold press/release is a click, and a click must be observable.
    ///
    /// Before this was fixed the board produced `card_activated` from **nowhere**:
    /// the press opened a drag session, the release discarded it, and the documented
    /// signal never fired from a real pointer. The assertion is deliberately on
    /// *delivery* (the id arriving), not on the gesture merely being non-dragging, so
    /// it fails if the emit is removed again.
    #[test]
    fn kanban_click_without_drag_emits_card_activated() {
        let mut board = board();
        let centre = card_center(&board, CardPosition { column: 0, card: 0 });
        let seen = Arc::new(Mutex::new(Option::<String>::None));
        let sink = Arc::clone(&seen);
        board.card_activated.connect(move |id: Arc<String>| {
            *sink.lock().expect("activation sink poisoned") = Some(id.as_str().to_string());
        });

        board.handle_event(&Event::mouse_press(centre.x, centre.y, 1));
        board.handle_event(&Event::mouse_move(centre.x + 1, centre.y));
        board.handle_event(&Event::mouse_release(centre.x + 1, centre.y, 1));

        assert_eq!(
            seen.lock().expect("activation sink poisoned").as_deref(),
            Some("a"),
            "a click on card 'a' must emit card_activated with its id"
        );
        // A click is not a move: the card stays where it was.
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    /// A real drag must **not** report an activation, or a drop would look like a click.
    #[test]
    fn kanban_drag_does_not_emit_card_activated() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let activations = Arc::new(Mutex::new(0usize));
        let counter = Arc::clone(&activations);
        board.card_activated.connect(move |_id: Arc<String>| {
            *counter.lock().expect("activation counter poisoned") += 1;
        });

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 10));
        board.handle_event(&Event::mouse_release(start.x + 40, start.y + 10, 1));

        assert_eq!(
            *activations.lock().expect("activation counter poisoned"),
            0,
            "a drag that moved the card must not also report an activation"
        );
    }

    #[test]
    fn kanban_drag_across_columns_moves_the_card() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let target_column = board.column_rect(2).expect("column 2");
        let target = Point::new(target_column.x + 100, target_column.y + HEADER_HEIGHT as i32 + 20);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 10));
        assert!(board.is_dragging_card(), "the drag must be active before the drop");
        board.handle_event(&Event::mouse_move(target.x, target.y));
        board.handle_event(&Event::mouse_release(target.x, target.y, 1));

        assert_eq!(board.position_of("a"), Some(CardPosition { column: 2, card: 0 }));
        assert!(board.column(0).unwrap().cards.is_empty());
    }

    #[test]
    fn kanban_drop_outside_any_column_leaves_the_card_alone() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        // Far to the right of the last column.
        board.handle_event(&Event::mouse_release(5000, 200, 1));

        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
        assert!(!board.is_dragging_card());
    }

    #[test]
    fn kanban_wip_limit_refuses_a_card_from_another_column() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_column(KanbanColumn::new("full", "Full").with_wip_limit(1));
        board.add_card(0, KanbanCard::new("dragged", "Dragged"));
        // Fill the limited column to its cap.
        board.add_card(1, KanbanCard::new("resident", "Resident"));

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        let target_column = board.column_rect(1).expect("column 1");
        let target = Point::new(target_column.x + 100, target_column.y + HEADER_HEIGHT as i32 + 20);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        board.handle_event(&Event::mouse_move(target.x, target.y));
        board.handle_event(&Event::mouse_release(target.x, target.y, 1));

        assert_eq!(
            board.position_of("dragged"),
            Some(CardPosition { column: 0, card: 0 }),
            "a full column must refuse an incoming card"
        );
        assert_eq!(board.column(1).unwrap().cards.len(), 1);
    }

    #[test]
    fn kanban_wip_limit_allows_reordering_within_the_full_column() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("full", "Full").with_wip_limit(1));
        board.add_card(0, KanbanCard::new("only", "Only"));

        // A limit caps *arrivals*, not movement. `can_accept` is the
        // position-independent question and a full board answers `false` to it — the
        // destination's own limit is what `on_drop` consults, where the column is
        // known. Asserting both halves pins which method owns which rule.
        let payload = DragPayload::new(CARD_PAYLOAD_TYPE, "only");
        assert!(
            !board.can_accept(&payload),
            "a board whose only column is full has nowhere to put a card"
        );

        // And the full column still accepts a card that is already inside it.
        let column_rect = board.column_rect(0).expect("column");
        let inside = Point::new(column_rect.x + 10, column_rect.y + HEADER_HEIGHT as i32 + 10);
        board.drag_origin = Some(CardPosition { column: 0, card: 0 });
        assert_eq!(
            board.on_drop(&payload, inside),
            DropEffect::Move,
            "a card already in the full column may be reordered within it"
        );
    }

    #[test]
    fn kanban_rejects_a_payload_of_another_type() {
        let board = board();
        assert!(!board.can_accept(&DragPayload::new("file", "x")));
    }

    #[test]
    fn kanban_empty_board_accepts_nothing() {
        let board = KanbanBoard::new(Rect::new(0, 0, 400, 300));
        // Nowhere to put a card is a refusal, not a silent no-op.
        assert!(!board.can_accept(&DragPayload::new(CARD_PAYLOAD_TYPE, "x")));
    }

    #[test]
    fn kanban_preview_rect_is_the_hovered_column() {
        let board = board();
        let payload = DragPayload::new(CARD_PAYLOAD_TYPE, "a");
        let column = board.column_rect(1).expect("column 1");
        let inside = Point::new(column.x + 10, column.y + 10);
        assert_eq!(board.preview_rect(&payload, inside), Some(column));
        // Outside every column there is no landing zone.
        assert_eq!(board.preview_rect(&payload, Point::new(5000, 5000)), None);
    }

    #[test]
    fn kanban_cancel_drag_restores_the_state() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));

        board.cancel_drag();
        assert!(!board.is_dragging_card());
        assert_eq!(board.dragged_card_id(), None);
        assert_eq!(board.hovered_column(), None);
        assert_eq!(board.position_of("a"), Some(CardPosition { column: 0, card: 0 }));
    }

    #[test]
    fn kanban_mouse_leave_cancels_the_drag() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y));
        assert!(board.is_dragging_card());

        board.handle_event(&Event::MouseLeave { pos: start });
        assert!(!board.is_dragging_card(), "losing the pointer must not strand a drag");
    }

    #[test]
    fn kanban_disabled_ignores_drags() {
        let mut board = board();
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.set_enabled(false);

        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(board.dragged_card_id(), None);
    }

    #[test]
    fn kanban_removing_a_dragged_card_column_cancels_the_drag() {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 800, 400));
        board.add_column(KanbanColumn::new("a", "A"));
        board.add_card(0, KanbanCard::new("card", "Card"));
        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(board.dragged_card_id(), Some("card"));

        board.remove_column(0);
        assert_eq!(board.dragged_card_id(), None, "the dragged card's column is gone");
    }

    // ── Layout and drawing ──────────────────────────────────────────────────

    #[test]
    fn kanban_columns_lay_out_side_by_side() {
        let board = board();
        let first = board.column_rect(0).expect("column 0");
        let second = board.column_rect(1).expect("column 1");
        assert_eq!(second.x - first.x, COLUMN_WIDTH as i32 + COLUMN_GAP);
        assert_eq!(first.width, COLUMN_WIDTH);
        assert!(board.column_rect(9).is_none());
    }

    #[test]
    fn kanban_collapsed_column_lays_out_no_cards() {
        let mut board = board();
        assert!(board.card_rect(CardPosition { column: 0, card: 0 }).is_some());
        board.column_mut(0).unwrap().collapsed = true;
        assert_eq!(
            board.card_rect(CardPosition { column: 0, card: 0 }),
            None,
            "a collapsed column shows no cards"
        );
    }

    #[test]
    fn kanban_draw_paints_without_panicking() {
        let mut board = board();
        let rgba = render(&mut board, Size::new(800, 400));
        assert!(!rgba.is_empty());
        // The board background is a light grey, so a frame that painted something
        // must differ from pure white.
        let painted = rgba
            .chunks_exact(4)
            .filter(|px| !(px[0] == 255 && px[1] == 255 && px[2] == 255))
            .count();
        assert!(painted > 0, "the board must paint its columns");
    }

    #[test]
    fn kanban_draw_zero_geometry_does_not_panic() {
        let mut board = board();
        let rgba = render(&mut board, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn kanban_drag_overlay_differs_from_the_resting_frame() {
        let mut board = board();
        let resting = render(&mut board, Size::new(800, 400));

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        board.handle_event(&Event::mouse_move(start.x + 40, start.y + 20));
        let dragging = render(&mut board, Size::new(800, 400));

        assert_ne!(resting, dragging, "a drag must be visible");
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn kanban_derived_properties_are_read_only() {
        let mut board = board();
        for name in
            ["column_count", "card_count", "dragging_card_id", "hovered_column", "column_width"]
        {
            assert_eq!(
                board.set(name, CapabilityValue::UInt(3)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
        assert_eq!(board.get("column_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(board.get("card_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(board.get("column_width").unwrap(), CapabilityValue::UInt(COLUMN_WIDTH as u64));
    }

    #[test]
    fn kanban_drag_state_is_reported_through_properties() {
        let mut board = board();
        assert_eq!(board.get("dragging_card_id").unwrap(), CapabilityValue::Null);

        let start = card_center(&board, CardPosition { column: 0, card: 0 });
        board.handle_event(&Event::mouse_press(start.x, start.y, 1));
        assert_eq!(
            board.get("dragging_card_id").unwrap(),
            CapabilityValue::String("a".to_string())
        );
        assert_eq!(board.get("hovered_column").unwrap(), CapabilityValue::UInt(0));
    }

    #[test]
    fn kanban_insert_card_clamps_an_over_large_index() {
        let mut board = board();
        let position = board.insert_card(0, 99, KanbanCard::new("z", "Z")).expect("insert");
        // Appended rather than refused, because "past the end" has one sensible
        // reading and the caller's intent is unambiguous.
        assert_eq!(position, CardPosition { column: 0, card: 1 });
        assert_eq!(board.column(0).unwrap().cards[1].id, "z");
    }

    #[test]
    fn kanban_insert_index_uses_the_midpoint_rule() {
        let mut board = board();
        board.add_card(0, KanbanCard::new("b", "Second"));
        let first = board.card_rect(CardPosition { column: 0, card: 0 }).expect("card 0");
        let column_x = first.x + 10;

        // Just into the first card: before it.
        assert_eq!(board.insert_index_at(0, Point::new(column_x, first.y + 2)), 0);
        // Past the first card's centre: after it.
        assert_eq!(
            board.insert_index_at(0, Point::new(column_x, first.y + CARD_HEIGHT as i32 - 2)),
            1
        );
        // A drop in the header goes to the top.
        let column_rect = board.column_rect(0).expect("column");
        assert_eq!(board.insert_index_at(0, Point::new(column_x, column_rect.y + 2)), 0);
        // Far below everything appends.
        assert_eq!(board.insert_index_at(0, Point::new(column_x, column_rect.y + 2000)), 2);
    }
}
