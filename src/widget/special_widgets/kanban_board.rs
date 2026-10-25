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
//! # Swimlanes
//!
//! A lane is a second coordinate beside the column. [`KanbanLane`]s are board-level
//! values, and [`KanbanBoard::set_card_lane`] assigns a card (by its board-wide id) to
//! one lane; [`KanbanBoard::lane_mode`] decides whether lanes affect layout at all.
//! With lanes on, each lane is a horizontal band whose height is set by the fullest
//! column in that lane, lane labels are painted in a left strip, and a card's
//! `CardPosition` still refers to its column and index while its y is derived from its
//! lane. A card with no assignment renders in the first lane. Drag releases inside a
//! band reassign the card to that lane, so the displayed band and the model agree.
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

use core::fmt::Write as _;

use crate::compat::HashMap;
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::dnd::{DragPayload, DragSession, DropEffect, DropTarget};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_string_list};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::role_font;
use crate::widget::{draw_icon_at, BaseWidget, Draw, IconName, Widget, WidgetKind};
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

/// The default width of each column, in logical pixels, and the gap between them.
const DEFAULT_COLUMN_WIDTH: u32 = 220;
const COLUMN_GAP: i32 = 12;

/// The narrowest a column may be made: below this a card's title is one glyph and a dash.
const MIN_COLUMN_WIDTH: u32 = 80;

/// How far one horizontal wheel notch scrolls the board, in pixels.
const HORIZONTAL_SCROLL_STEP: i32 = 40;

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

/// Width of the lane-label strip on the left of a swimlane board.
const LANE_HEADER_WIDTH: u32 = 96;
/// Height of a lane's own caption row, inside its band.
const LANE_HEADER_HEIGHT: i32 = 24;
/// Vertical gap between two lane bands.
const LANE_GAP: i32 = 6;

/// A swimlane: a horizontal band that groups cards across every column.
///
/// Lanes are board-level data, not column data: a card's lane is a second coordinate beside its
/// column, which is why the assignment lives in [`KanbanBoard::set_card_lane`] keyed by the card's
/// board-wide id rather than inside [`KanbanColumn`]. A lane has no effect until
/// [`KanbanBoard::set_lane_mode`] turns lanes on; the default is the flat board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanbanLane {
    /// Stable identifier used by [`KanbanBoard::set_card_lane`].
    pub id: String,
    /// Header text drawn in the lane's left strip.
    pub title: String,
}

impl KanbanLane {
    /// Creates a lane with `id` and `title`.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into() }
    }
}

/// Whether the board groups cards into swimlanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KanbanLaneMode {
    /// Every card is laid out in one flat column, in column order.
    #[default]
    None,
    /// Cards are grouped by [`KanbanBoard::card_lane`] into vertically stacked bands.
    Lanes,
}

impl KanbanLaneMode {
    /// The token this mode is published under.
    pub fn token(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lanes => "lanes",
        }
    }

    /// Parses a published token; `None` for an unknown spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "none" => Some(Self::None),
            "lanes" => Some(Self::Lanes),
            _ => None,
        }
    }
}

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
    /// The card under the pointer, when no drag is in progress.
    hovered_card: Option<CardPosition>,
    /// The card keyboard navigation has reached.
    focused_card: Option<CardPosition>,
    /// Per-column scroll offsets, in cards.
    column_scroll: Vec<usize>,
    /// Width of one column, in logical pixels.
    column_width: u32,
    /// How far the columns are scrolled left, in pixels.
    horizontal_scroll: i32,
    /// The swimlanes, in display order.
    lanes: Vec<KanbanLane>,
    /// Card id to lane id. Cards without an entry render in the first lane.
    card_lanes: HashMap<String, String>,
    /// Whether cards are grouped into lanes.
    lane_mode: KanbanLaneMode,
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
            hovered_card: None,
            focused_card: None,
            column_scroll: Vec::new(),
            column_width: DEFAULT_COLUMN_WIDTH,
            horizontal_scroll: 0,
            lanes: Vec::new(),
            card_lanes: HashMap::new(),
            lane_mode: KanbanLaneMode::None,
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
        // A lane assignment names a card; a card that left the board must not keep one.
        for card in &removed.cards {
            self.card_lanes.remove(&card.id);
        }
        // A drag whose origin no longer exists is abandoned, so the release does
        // not try to remove a card from a column that is gone.
        if self.drag_origin.is_some_and(|origin| origin.column >= self.columns.len()) {
            self.cancel_drag();
        }
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
        self.columns.get_mut(index)
    }

    // ── Swimlanes ───────────────────────────────────────────────────────────

    /// Adds a lane, returning its index.
    pub fn add_lane(&mut self, lane: KanbanLane) -> usize {
        let index = self.lanes.len();
        self.lanes.push(lane);
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
    fn card_lane_index(&self, card_id: &str) -> Option<usize> {
        if self.lanes.is_empty() {
            return None;
        }
        match self.card_lanes.get(card_id) {
            Some(lane_id) => self.lanes.iter().position(|lane| &lane.id == lane_id).or(Some(0)),
            None => Some(0),
        }
    }

    /// Whether lanes affect layout at all.
    fn lanes_active(&self) -> bool {
        self.lane_mode == KanbanLaneMode::Lanes && !self.lanes.is_empty()
    }

    /// The top of lane `index`'s band in board coordinates.
    fn lane_top(&self, index: usize) -> i32 {
        let mut y = self.geometry().y + HEADER_HEIGHT as i32;
        for lane in 0..index.min(self.lanes.len()) {
            y += self.lane_height(lane) + LANE_GAP;
        }
        y
    }

    /// The height of lane `index`'s band: its caption plus as many card rows as the fullest
    /// column holds in that lane.
    fn lane_height(&self, index: usize) -> i32 {
        let mut rows = 0usize;
        for column in &self.columns {
            let count = column
                .cards
                .iter()
                .filter(|card| self.card_lane_index(&card.id) == Some(index))
                .count();
            rows = rows.max(count);
        }
        LANE_HEADER_HEIGHT + rows as i32 * (CARD_HEIGHT as i32 + CARD_GAP) + LANE_GAP
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

    /// The lane whose band contains `pos`.
    fn lane_at(&self, pos: Point) -> Option<usize> {
        if !self.lanes_active() || !self.geometry().contains_point(pos) {
            return None;
        }
        (0..self.lanes.len()).find(|index| {
            let top = self.lane_top(*index);
            pos.y >= top && pos.y < top + self.lane_height(*index)
        })
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
        self.card_lanes.remove(card_id);
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
    /// Columns are laid out left to right at the control's own width, offset by the horizontal
    /// scroll. They used to be laid out at a fixed 220 px with no offset at all, which left a board
    /// wider than its viewport with no way to reach the columns past the edge — the documented
    /// answer was "let the container scroll it", which a control that paints itself cannot rely on.
    pub fn column_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.columns.len() {
            return None;
        }
        let rect = self.geometry();
        let stride = self.column_width as i32 + COLUMN_GAP;
        // Clamped on read as well as on write: a board that shrank after its offset was set would
        // otherwise paint past its content and show empty space.
        let scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
        // In lane mode the left strip names the lanes, so the columns begin after it.
        let origin = rect.x + if self.lanes_active() { LANE_HEADER_WIDTH as i32 } else { 0 };
        let x = origin - scroll + (index as i32) * stride;
        Some(Rect::new(x, rect.y, self.column_width, rect.height))
    }

    /// Returns the width of one column.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Sets the width of one column (floored at [`MIN_COLUMN_WIDTH`]).
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
    fn max_horizontal_scroll(&self) -> i32 {
        if self.columns.is_empty() {
            return 0;
        }
        let content =
            self.columns.len() as i32 * (self.column_width as i32 + COLUMN_GAP) - COLUMN_GAP;
        let lane_strip = if self.lanes_active() { LANE_HEADER_WIDTH as i32 } else { 0 };
        let viewport = (self.geometry().width as i32 - lane_strip).max(0);
        (content - viewport).max(0)
    }

    /// Re-clamps the scroll after the content or viewport width changed.
    fn clamp_horizontal_scroll(&mut self) {
        self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll());
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
        // different y in every lane, so the ordinal is counted among the cards of that lane.
        if self.lanes_active() {
            let card_id = column.cards.get(position.card)?.id.as_str();
            let lane = self.card_lane_index(card_id)?;
            let band = self.lane_band_rect(lane)?;
            let ordinal = column.cards[..position.card]
                .iter()
                .filter(|card| self.card_lane_index(&card.id) == Some(lane))
                .count();
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
    fn column_at(&self, pos: Point) -> Option<usize> {
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
        if self.lanes_active() {
            let lane = self.lane_at(pos).unwrap_or(0);
            let lane_indices: Vec<usize> = entries
                .cards
                .iter()
                .enumerate()
                .filter(|(_, card)| self.card_lane_index(&card.id) == Some(lane))
                .map(|(index, _)| index)
                .collect();
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
        // than always above the card under it.
        let stride = CARD_HEIGHT as i32 + CARD_GAP;
        let offset = pos.y - first_card_y;
        let index = (offset + stride / 2) / stride;
        // Clamped to the column's length: a pointer below the last card appends.
        (index as usize).min(entries.cards.len())
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
    fn ensure_focused_visible(&mut self) {
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
    fn visible_cards_per_column(&self) -> usize {
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
        moved || lane_changed
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
        // Wide enough for two columns, which is the smallest useful board.
        Size::new(
            self.column_width * 2 + COLUMN_GAP as u32,
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
            _ => None,
        }
    }
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
            "column_width" => Ok(CapabilityValue::UInt(self.column_width() as u64)),
            "horizontal_scroll_offset" => {
                Ok(CapabilityValue::UInt(self.horizontal_scroll().max(0) as u64))
            }
            "lane_mode" => Ok(CapabilityValue::String(self.lane_mode.token().to_string())),
            "swimlanes" => Ok(CapabilityValue::Bool(self.lane_mode == KanbanLaneMode::Lanes)),
            "lane_count" => Ok(CapabilityValue::UInt(self.lane_count() as u64)),
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
            // A caller may set the width, so it is a per-instance value now rather than the fixed
            // layout constant it used to report as read-only.
            "column_width" => {
                let width = match value {
                    CapabilityValue::UInt(width) => {
                        u32::try_from(width).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_column_width(width);
                Ok(())
            }
            "horizontal_scroll_offset" => {
                let offset = match value {
                    CapabilityValue::UInt(offset) => {
                        i32::try_from(offset).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_horizontal_scroll(offset);
                Ok(())
            }
            "lane_mode" => {
                let token = expect_string(value)?;
                let mode = KanbanLaneMode::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_lane_mode(mode);
                Ok(())
            }
            // Boolean alias for the same switch: `true` groups cards into the configured lanes,
            // `false` is the flat board. `lane_mode` remains the explicit spelling.
            "swimlanes" => {
                let enabled = expect_bool(value)?;
                self.set_lane_mode(if enabled {
                    KanbanLaneMode::Lanes
                } else {
                    KanbanLaneMode::None
                });
                Ok(())
            }
            // Lanes are identities; a count cannot say which band a card belongs to.
            "lane_count" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "horizontal_scroll_offset",
            "lane_mode",
            "swimlanes",
            "lane_count",
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
            Event::MousePress { pos, button, .. } if *button == 1 => {
                if let Some(position) = self.card_at(*pos) {
                    self.focused_card = Some(position);
                    self.begin_card_drag(position, *pos);
                }
            }
            Event::MouseMove { pos } if self.drag.is_some() => {
                self.update_card_drag(*pos);
            }
            Event::MouseMove { pos } => {
                let hovered = self.card_at(*pos);
                let column = self.column_at(*pos);
                let changed = hovered != self.hovered_card || column != self.hovered_column;
                self.hovered_card = hovered;
                self.hovered_column = column;
                if changed {
                    self.base.request_redraw();
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
                if self.hovered_card.take().is_some() {
                    self.base.request_redraw();
                }
            }
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
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("kanban_board");
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
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Every colour the board paints below is derived from this one resolved
        // triple, so a theme switch moves the whole board rather than the frame
        // only. The column, its header strip, its cards and the drag feedback are
        // all chrome states of the same surface.
        let chrome = BoardChrome { background, border, text_color };

        // One face per role for the whole board: these were resolved inside `draw_column`, so a
        // board of N columns re-derived (and re-allocated the family string of) four fonts per
        // column per frame.
        let fonts = ColumnFonts {
            header: role_font(self.base.style(), 13.0),
            badge: role_font(self.base.style(), 11.0),
            title: role_font(self.base.style(), 12.0),
            desc: role_font(self.base.style(), 10.0),
        };

        // Columns that the horizontal scroll has moved past either edge are skipped, and the rest
        // are clipped to the board: a column laid out at a negative x would otherwise paint over
        // whatever the layout put to the left of the board (nothing clips a widget at this layer).
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        for index in 0..self.columns.len() {
            let Some(column_rect) = self.column_rect(index) else {
                continue;
            };
            if column_rect.x + column_rect.width as i32 <= rect.x
                || column_rect.x >= rect.x + rect.width as i32
            {
                continue;
            }
            self.draw_column(context, index, &chrome, &fonts);
        }
        if self.lanes_active() {
            self.draw_lane_headers(context, &chrome, &fonts);
        }
        context.pop_clip();
        // The drag overlay is painted last so the card follows the pointer over
        // every column rather than being clipped to the one it started in. It is clamped to the
        // board inside `draw_drag_overlay`, so it stays inside the control without a clip.
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

/// The four faces a column draws with, resolved once per paint.
///
/// `role_font` re-derives the font from the style (cloning its family string), so resolving
/// these inside `draw_column` cost four allocations per column per frame.
struct ColumnFonts {
    header: crate::core::Font,
    badge: crate::core::Font,
    title: crate::core::Font,
    desc: crate::core::Font,
}

impl BoardChrome {
    /// A column's fill: the `surface_container` role when the theme offers one, so a theme can
    /// tune the lane colour independently of the board it sits on.
    ///
    /// # Why the role comes first (BLUE23 §93, M5)
    ///
    /// The lane was a local blend of the board's own colour, which is a value no theme can address:
    /// changing the board moved the lanes with it, so "the board is dark but its lanes should be
    /// lighter" was not expressible. `surface_container` is the role the crate already declares for
    /// exactly this — one step up from the page — and it is what `panel`, `group_box` and
    /// `emoji_picker` read for the same purpose. The blend survives as the fallback for a theme
    /// that predates the role.
    ///
    /// It still steps *away* from the board rather than adopting the role verbatim when the role
    /// has resolved to the board's own colour, which is the same guard `drop_zone` and `otp_input`
    /// use: a lane indistinguishable from the board is not a lane.
    fn column(&self) -> Color {
        let role = crate::style::layer_color(crate::style::LayerColor::SurfaceContainer);
        match role {
            Some(role) if role != self.background => role,
            _ => self.background.blend(&self.text_color, 0.18),
        }
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

    /// One lane band, alternating by parity so two adjacent lanes read as separate bands even
    /// when no card happens to sit in one of them.
    fn lane_band(&self, index: usize) -> Color {
        let step = if index.is_multiple_of(2) { 0.18 } else { 0.13 };
        self.background.blend(&self.text_color, step)
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
    fn draw_column(
        &mut self,
        context: &mut RenderContext,
        index: usize,
        chrome: &BoardChrome,
        fonts: &ColumnFonts,
    ) {
        let Some(column_rect) = self.column_rect(index) else {
            return;
        };
        // Borrowed rather than cloned: the tuple used to copy the title `String`, the whole
        // card vector (and every card's title/description) once per column per frame. Every
        // helper below (`column_rect`, `card_rect`, `draw_card`) takes `&self`, so the borrow
        // can live across the column.
        let Some(column) = self.columns.get(index) else {
            return;
        };
        let title = column.title.as_str();
        let collapsed = column.collapsed;
        let cards = &column.cards;
        let at_limit = column.is_at_wip_limit();
        let wip_limit = column.wip_limit;

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
        let header_font = &fonts.header;
        let badge_font = &fonts.badge;
        let title_font = &fonts.title;
        let desc_font = &fonts.desc;
        let header_line = context.text_line(header_rect, header_font);
        // The badge is measured first so the title knows where to stop: an unbounded
        // title ran underneath the count on the right.
        let mut badge = String::new();
        if collapsed {
            let _ = write!(badge, "({})", cards.len());
        } else if let Some(limit) = wip_limit {
            let _ = write!(badge, "{}/{}", cards.len(), limit);
        } else {
            let _ = write!(badge, "{}", cards.len());
        }
        let badge_metrics = context.measure_text(&badge, badge_font);
        let badge_line = context.text_line(header_rect, badge_font);
        let badge_x =
            header_rect.x + header_rect.width as i32 - CARD_PADDING - badge_metrics.width as i32;
        let title_width = ((badge_x - 6) - (header_rect.x + CARD_PADDING)).max(0) as u32;
        context.draw_text_fitted_cow(
            Rect::new(
                header_rect.x + CARD_PADDING,
                header_line.y,
                title_width,
                header_line.height.max(1),
            ),
            title,
            header_font,
            header_color,
            HorizontalAlignment::Left,
        );
        // Card count on the right of the header, or the collapsed marker. One buffer
        // reused per column: the three-branch `format!`/`to_string` chain allocated a
        // fresh `String` for every column on every frame.
        context.draw_text(
            Point::new(badge_x, badge_line.y),
            &badge,
            badge_font,
            chrome.muted_text(),
            HorizontalAlignment::Left,
        );

        if collapsed {
            return;
        }

        // In lane mode the band is the column's row grid: painting it here (rather than in the
        // board loop) keeps each band clipped to the column it belongs to, so a card cannot appear
        // to float in a neighbouring column.
        if self.lanes_active() {
            for lane in 0..self.lanes.len() {
                let Some(band) = self.lane_band_rect(lane) else {
                    continue;
                };
                let cell = Rect::new(column_rect.x, band.y, column_rect.width, band.height);
                context.fill_rect(cell, chrome.lane_band(lane));
                context.draw_line(
                    Point::new(cell.x, cell.y + cell.height as i32),
                    Point::new(cell.x + cell.width as i32, cell.y + cell.height as i32),
                    chrome.border,
                );
            }
        }

        for (card_index, card) in cards.iter().enumerate() {
            let position = CardPosition { column: index, card: card_index };
            let Some(card_rect) = self.card_rect(position) else {
                continue;
            };
            // Clipped to the column so a card taller than the remaining space does
            // not paint over the column beside it.
            context.push_clip(column_rect.x, column_rect.y, column_rect.width, column_rect.height);
            self.draw_card(context, card, card_rect, position, chrome, title_font, desc_font);
            context.pop_clip();
        }
    }

    /// Draws the lane labels down the left edge and the divider that separates them from the
    /// columns. The bands themselves are painted per column by `draw_column`.
    fn draw_lane_headers(
        &self,
        context: &mut RenderContext,
        chrome: &BoardChrome,
        fonts: &ColumnFonts,
    ) {
        let rect = self.geometry();
        let caption = Rect::new(rect.x, rect.y, LANE_HEADER_WIDTH, HEADER_HEIGHT);
        context.fill_rounded_rect(caption, 8, chrome.header());
        context.draw_text_line_cow(
            Rect::new(
                caption.x + CARD_PADDING,
                caption.y,
                caption.width.saturating_sub(2 * CARD_PADDING as u32),
                caption.height,
            ),
            "Lanes",
            &fonts.header,
            chrome.muted_text(),
            HorizontalAlignment::Left,
        );
        for index in 0..self.lanes.len() {
            let Some(header) = self.lane_header_rect(index) else {
                continue;
            };
            context.fill_rounded_rect(header, 6, chrome.lane_band(index));
            context.draw_text_line_cow(
                Rect::new(
                    header.x + CARD_PADDING,
                    header.y,
                    header.width.saturating_sub(2 * CARD_PADDING as u32),
                    LANE_HEADER_HEIGHT as u32,
                ),
                &self.lanes[index].title,
                &fonts.header,
                chrome.text(),
                HorizontalAlignment::Left,
            );
        }
        let divider_x = rect.x + LANE_HEADER_WIDTH as i32;
        context.draw_line(
            Point::new(divider_x, rect.y + HEADER_HEIGHT as i32),
            Point::new(divider_x, rect.y + rect.height as i32),
            chrome.border,
        );
    }

    /// Draws one card, marking it when it is the one being dragged.
    fn draw_card(
        &self,
        context: &mut RenderContext,
        card: &KanbanCard,
        card_rect: Rect,
        position: CardPosition,
        chrome: &BoardChrome,
        title_font: &Font,
        desc_font: &Font,
    ) {
        let is_dragged = self.drag_origin == Some(position) && self.is_dragging_card();
        // A hovered card lifts off the column with a small shadow, so the pointer's target
        // reads as a raised object before it is dragged. The shadow is drawn first and the
        // card keeps its rectangle, which is what keeps the layout (and the hit test) still.
        if self.hovered_card == Some(position) && !self.is_dragging_card() {
            context.draw_box_shadow(card_rect, Color::BLACK.with_alpha(40), 0, 3, 6, 0);
        }
        // The dragged card stays drawn in place but faded, so the user can see both
        // where it came from and where it is going.
        let alpha = if is_dragged { 90 } else { 255 };
        context.fill_rounded_rect(card_rect, 6, chrome.card().with_alpha(alpha));
        let focused = self.focused_card == Some(position);
        context.draw_rounded_rect_stroke(
            card_rect,
            6,
            if focused { chrome.text().with_alpha(alpha) } else { chrome.border.with_alpha(alpha) },
            if focused { 2 } else { 1 },
        );

        let title_color = if card.done {
            chrome.done_text().with_alpha(alpha)
        } else {
            chrome.text().with_alpha(alpha)
        };
        let title = card.title.as_str();
        // The title and the description are placed from the card's own box: the literals
        // `+ 20` and `+ 38` described a 48 px card, so any other `CARD_HEIGHT` drew the
        // description past the card's bottom edge. A card is `CARD_HEIGHT` tall by contract.
        let title_line = context.text_line(card_rect, title_font);
        // The done marker used to be a `✓` (U+2713) prefix on the title string. No bundled
        // face covers that code point, so the text path fell back to an 8x8 bitmap and drew a
        // blocky check mark. An `IconName::Check` is real geometry both backends agree on, and
        // drawing it in its own square keeps it independent of the label's glyph coverage.
        let mut title_x = card_rect.x + CARD_PADDING;
        if card.done {
            let icon_side: u32 = 10;
            let icon_rect = Rect::new(
                title_x,
                title_line.y + (title_line.height as i32 - icon_side as i32) / 2,
                icon_side,
                icon_side,
            );
            draw_icon_at(context, icon_rect, title_color, IconName::Check);
            title_x += icon_side as i32 + 4;
        }
        if !title.is_empty() {
            context.draw_text(
                Point::new(title_x, title_line.y),
                title,
                title_font,
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
            let desc_line = context.text_line(desc_band, desc_font);
            context.draw_text(
                Point::new(card_rect.x + CARD_PADDING, desc_line.y),
                &card.description,
                desc_font,
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
        let ghost_width = self.column_width;
        let ghost_x = (current.x - (ghost_width as i32 / 2))
            .clamp(board.x, board.x + board.width.saturating_sub(ghost_width) as i32);
        let ghost_y = (current.y - (CARD_HEIGHT as i32 / 2))
            .clamp(board.y, board.y + board.height.saturating_sub(CARD_HEIGHT) as i32);
        let ghost = Rect::new(ghost_x, ghost_y, ghost_width, CARD_HEIGHT);
        context.fill_rounded_rect(ghost, 6, chrome.ghost().with_alpha(230));
        context.draw_rounded_rect_stroke(ghost, 6, chrome.insertion(), 2);
        if !label.is_empty() {
            let ghost_font = role_font(self.base.style(), 12.0);
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

/// Padding inside a card, and the gap between its stacked lines.
const CARD_INSET: i32 = 8;
/// Side of a card's done box.
const CARD_DONE_BOX: i32 = 14;
/// Height of a column header strip and the side of its icon cells.
const HEADER_SIDE: i32 = 32;
const HEADER_ICON: i32 = 14;

/// The card control of a kanban board.
///
/// # Why this is a control and not the board's row
///
/// [`KanbanBoard`] paints each card as a rectangle from its own model. That is enough to drag a
/// card, and not enough for anything else: there is no child control to hit the done box, root an
/// editor for the title, or carry tags and an assignee that a real board shows. This widget is
/// that card as a control in its own right; the board's [`KanbanCard`] stays the data model.
///
/// The name is deliberately distinct: `KanbanCard` already meant the data (id, title, description,
/// done) and reusing it for the control would have broken every existing caller for a rename that
/// adds no behaviour.
pub struct KanbanCardView {
    base: BaseWidget,
    title: String,
    description: String,
    done: bool,
    tags: Vec<String>,
    assignee: String,
    draggable: bool,
    /// The in-progress title edit, when the card is being renamed.
    edit_buffer: Option<String>,
    /// Emitted when the card body is clicked (not its done box).
    pub card_clicked: GenericSignal,
    /// Emitted with the committed title after an edit.
    pub card_edited: Signal1<String>,
    /// Emitted with the new done flag when it is toggled.
    pub card_done_toggled: Signal1<bool>,
}

impl KanbanCardView {
    /// Creates a card with `title` and no other content.
    pub fn new(geometry: Rect, title: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanCardView, geometry, "KanbanCardView"),
            title: title.into(),
            description: String::new(),
            done: false,
            tags: Vec::new(),
            assignee: String::new(),
            draggable: true,
            edit_buffer: None,
            card_clicked: GenericSignal::new(),
            card_edited: Signal1::new(),
            card_done_toggled: Signal1::new(),
        }
    }

    /// The card title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title without starting an edit, reporting it as a card edit.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title.clone();
        self.card_edited.emit(title);
        self.base.request_redraw();
    }

    /// The description line.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Sets the description line.
    pub fn set_description(&mut self, description: impl Into<String>) {
        let description = description.into();
        if self.description == description {
            return;
        }
        self.description = description;
        self.base.request_redraw();
    }

    /// Whether the card is marked done.
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Sets the done flag, emitting `card_done_toggled`.
    pub fn set_done(&mut self, done: bool) {
        if self.done == done {
            return;
        }
        self.done = done;
        self.card_done_toggled.emit(done);
        self.base.request_redraw();
    }

    /// The card's tags, in display order.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Replaces the tags.
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags = tags;
        self.base.request_redraw();
    }

    /// The assignee line.
    pub fn assignee(&self) -> &str {
        &self.assignee
    }

    /// Sets the assignee line.
    pub fn set_assignee(&mut self, assignee: impl Into<String>) {
        self.assignee = assignee.into();
        self.base.request_redraw();
    }

    /// Whether a board may drag this card.
    pub fn is_draggable(&self) -> bool {
        self.draggable
    }

    /// Enables or disables dragging for this card.
    pub fn set_draggable(&mut self, draggable: bool) {
        self.draggable = draggable;
    }

    /// Whether the title is being edited.
    pub fn is_editing(&self) -> bool {
        self.edit_buffer.is_some()
    }

    /// Starts editing the title from the current text.
    pub fn begin_title_edit(&mut self) -> bool {
        if self.edit_buffer.is_some() {
            return false;
        }
        self.edit_buffer = Some(self.title.clone());
        self.base.request_redraw();
        true
    }

    /// Commits the in-progress title edit.
    pub fn commit_title_edit(&mut self) -> bool {
        let Some(next) = self.edit_buffer.take() else {
            return false;
        };
        self.base.request_redraw();
        if next == self.title {
            return false;
        }
        self.title = next.clone();
        self.card_edited.emit(next);
        true
    }

    /// Cancels the in-progress title edit, restoring the old title.
    pub fn cancel_title_edit(&mut self) -> bool {
        self.edit_buffer.take().is_some()
    }

    /// The painted box of the done box.
    pub fn done_box_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + CARD_INSET,
            rect.y + CARD_INSET,
            CARD_DONE_BOX as u32,
            CARD_DONE_BOX as u32,
        )
    }
}

impl Widget for KanbanCardView {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(220, 72)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the card's three events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "card_clicked" => Some(EventSignalRef::unit("card_clicked", &self.card_clicked)),
            "card_edited" => Some(EventSignalRef::mapped("card_edited", &self.card_edited, |v| {
                CapabilityValue::String(v.clone())
            })),
            "card_done_toggled" => {
                Some(EventSignalRef::mapped("card_done_toggled", &self.card_done_toggled, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for KanbanCardView {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "description" => Ok(CapabilityValue::String(self.description.clone())),
            "done" => Ok(CapabilityValue::Bool(self.done)),
            "tags" => Ok(CapabilityValue::Tuple(
                self.tags.iter().map(|tag| CapabilityValue::String(tag.clone())).collect(),
            )),
            "assignee" => Ok(CapabilityValue::String(self.assignee.clone())),
            "draggable" => Ok(CapabilityValue::Bool(self.draggable)),
            "editing" => Ok(CapabilityValue::Bool(self.is_editing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "description" => {
                self.set_description(expect_string(value)?);
                Ok(())
            }
            "done" => {
                self.set_done(expect_bool(value)?);
                Ok(())
            }
            "tags" => {
                let tags = expect_string_list(value)?;
                self.set_tags(tags);
                Ok(())
            }
            "assignee" => {
                self.set_assignee(expect_string(value)?);
                Ok(())
            }
            "draggable" => {
                self.set_draggable(expect_bool(value)?);
                Ok(())
            }
            "editing" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "description",
            "done",
            "tags",
            "assignee",
            "draggable",
            "editing",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for KanbanCardView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.edit_buffer.is_some() {
            match event {
                Event::TextInput { text } | Event::ImeCommit { text } => {
                    if let Some(buffer) = self.edit_buffer.as_mut() {
                        buffer.extend(text.chars().filter(|character| !character.is_control()));
                    }
                    self.base.request_redraw();
                }
                Event::KeyPress { key, .. } => match *key {
                    13 => {
                        let _ = self.commit_title_edit();
                    }
                    27 => {
                        self.cancel_title_edit();
                        self.base.request_redraw();
                    }
                    8 => {
                        if let Some(buffer) = self.edit_buffer.as_mut() {
                            buffer.pop();
                        }
                        self.base.request_redraw();
                    }
                    _ => { /* Other keys are not relevant */ }
                },
                _ => {}
            }
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.done_box_rect().contains_point(*pos) {
                    self.set_done(!self.done);
                } else if self.base.geometry().contains_point(*pos) {
                    self.card_clicked.emit();
                }
            }
            Event::MouseDoubleClick { pos, button: 1, .. }
                if self.base.geometry().contains_point(*pos) =>
            {
                let _ = self.begin_title_edit();
            }
            _ => {}
        }
    }
}

impl Draw for KanbanCardView {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("kanban_board");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let font = role_font(style, 12.0);
        let small = role_font(style, 10.0);

        context.fill_rounded_rect(rect, 6, surface);
        context.draw_rounded_rect_stroke(rect, 6, border, 1);

        let done_box = self.done_box_rect();
        context.draw_rect(done_box, border);
        if self.done {
            crate::widget::draw_icon_at(context, done_box, accent, IconName::Check);
        }

        let text_x = done_box.x + done_box.width as i32 + 6;
        let title = match &self.edit_buffer {
            Some(buffer) => buffer.as_str(),
            None => self.title.as_str(),
        };
        let strike = if self.done { ink.blend(&surface, 0.45) } else { ink };
        let fitted = crate::widget::text_utils::elide_end(
            title,
            ((rect.width as i32 - (text_x - rect.x) - CARD_INSET) / 7).max(1) as usize,
        );
        context.draw_text(
            Point::new(text_x, rect.y + CARD_INSET),
            &fitted,
            &font,
            strike,
            HorizontalAlignment::Left,
        );
        if self.done {
            let width = context.measure_text(&fitted, &font).width as i32;
            context.draw_line(
                Point::new(text_x, rect.y + CARD_INSET + 7),
                Point::new(text_x + width, rect.y + CARD_INSET + 7),
                strike,
            );
        }

        let mut y = rect.y + CARD_INSET + 18;
        if !self.description.is_empty() {
            let description = crate::widget::text_utils::elide_end(
                &self.description,
                ((rect.width as i32 - CARD_INSET * 2) / 6).max(1) as usize,
            );
            context.draw_text(
                Point::new(rect.x + CARD_INSET, y),
                &description,
                &small,
                ink.blend(&surface, 0.30),
                HorizontalAlignment::Left,
            );
            y += 16;
        }
        if !self.tags.is_empty() {
            let mut x = rect.x + CARD_INSET;
            for tag in &self.tags {
                let label =
                    crate::widget::text_utils::elide_end(tag, (rect.width as usize / 7).max(2));
                let width = context.measure_text(&label, &small).width + 10;
                if x + width as i32 > rect.x + rect.width as i32 - CARD_INSET {
                    break;
                }
                let chip = Rect::new(x, y, width, 14);
                context.fill_rounded_rect(chip, 4, surface.blend(&accent, 0.22));
                context.draw_text(
                    Point::new(x + 4, y + 1),
                    &label,
                    &small,
                    ink,
                    HorizontalAlignment::Left,
                );
                x += width as i32 + 4;
            }
        }
        if !self.assignee.is_empty() {
            let label = crate::widget::text_utils::elide_end(&self.assignee, 12);
            let width = context.measure_text(&label, &small).width as i32;
            context.draw_text(
                Point::new(
                    rect.x + rect.width as i32 - width - CARD_INSET,
                    rect.y + rect.height as i32 - 18,
                ),
                &label,
                &small,
                ink.blend(&surface, 0.30),
                HorizontalAlignment::Left,
            );
        }
    }
}

/// The header control of a kanban column.
///
/// The board paints its column titles as part of its own chrome, so a column could not be renamed,
/// its WIP limit could not be read, and its collapse chevron was a painted triangle with no
/// target. This control is that header as a widget: title, count badge, WIP limit, collapse
/// and an add-card affordance, each of which reports the gesture it received.
pub struct KanbanColumnHeader {
    base: BaseWidget,
    title: String,
    count: usize,
    wip_limit: Option<usize>,
    collapsed: bool,
    editable: bool,
    /// The in-progress title edit, when the header is being renamed.
    edit_buffer: Option<String>,
    /// Emitted with the new title after a rename.
    pub column_renamed: Signal1<String>,
    /// Emitted with the new collapsed flag.
    pub column_collapsed_changed: Signal1<bool>,
    /// Emitted when the add-card button is pressed.
    pub add_card_requested: GenericSignal,
}

impl KanbanColumnHeader {
    /// Creates a header showing `title`.
    pub fn new(geometry: Rect, title: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::KanbanColumnHeader, geometry, "KanbanColumnHeader"),
            title: title.into(),
            count: 0,
            wip_limit: None,
            collapsed: false,
            editable: true,
            edit_buffer: None,
            column_renamed: Signal1::new(),
            column_collapsed_changed: Signal1::new(),
            add_card_requested: GenericSignal::new(),
        }
    }

    /// The column title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Renames the column, emitting `column_renamed`.
    ///
    /// Returns whether the title changed; a rename to the same text is not reported, because a
    /// subscriber that persisted the column would write a change that did not happen.
    pub fn rename(&mut self, title: impl Into<String>) -> bool {
        let title = title.into();
        if self.title == title {
            return false;
        }
        self.title = title.clone();
        self.column_renamed.emit(title);
        self.base.request_redraw();
        true
    }

    /// The card count shown in the badge.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Sets the card count shown in the badge.
    pub fn set_count(&mut self, count: usize) {
        if self.count == count {
            return;
        }
        self.count = count;
        self.base.request_redraw();
    }

    /// The column's work-in-progress limit.
    pub fn wip_limit(&self) -> Option<usize> {
        self.wip_limit
    }

    /// Sets the work-in-progress limit.
    pub fn set_wip_limit(&mut self, limit: Option<usize>) {
        if self.wip_limit == limit {
            return;
        }
        self.wip_limit = limit;
        self.base.request_redraw();
    }

    /// Whether the column is collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Collapses or expands the column, emitting `column_collapsed_changed`.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.column_collapsed_changed.emit(collapsed);
        self.base.request_redraw();
    }

    /// Whether the title may be edited by the reader.
    pub fn is_editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables title editing.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        if !editable {
            self.edit_buffer = None;
        }
        self.base.request_redraw();
    }

    /// Whether the count has reached the WIP limit.
    pub fn is_at_wip_limit(&self) -> bool {
        self.wip_limit.is_some_and(|limit| self.count >= limit)
    }

    /// Whether the title is being edited.
    pub fn is_editing(&self) -> bool {
        self.edit_buffer.is_some()
    }

    /// Starts editing the title from the current text.
    pub fn begin_title_edit(&mut self) -> bool {
        if !self.editable || self.edit_buffer.is_some() {
            return false;
        }
        self.edit_buffer = Some(self.title.clone());
        self.base.request_redraw();
        true
    }

    /// Commits the in-progress rename.
    pub fn commit_title_edit(&mut self) -> bool {
        let Some(next) = self.edit_buffer.take() else {
            return false;
        };
        if next == self.title {
            self.base.request_redraw();
            return false;
        }
        let changed = self.rename(next);
        self.base.request_redraw();
        changed
    }

    /// Cancels the in-progress rename.
    pub fn cancel_title_edit(&mut self) -> bool {
        let canceled = self.edit_buffer.take().is_some();
        if canceled {
            self.base.request_redraw();
        }
        canceled
    }

    /// The painted box of the collapse chevron.
    pub fn collapse_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + 4,
            rect.y + (rect.height as i32 - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        )
    }

    /// The painted box of the add-card button.
    pub fn add_rect(&self) -> Rect {
        let rect = self.base.geometry();
        Rect::new(
            rect.x + rect.width as i32 - HEADER_ICON - 4,
            rect.y + (rect.height as i32 - HEADER_ICON) / 2,
            HEADER_ICON as u32,
            HEADER_ICON as u32,
        )
    }
}

impl Widget for KanbanColumnHeader {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(220, HEADER_SIDE as u32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the header's three events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "column_renamed" => {
                Some(EventSignalRef::mapped("column_renamed", &self.column_renamed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "column_collapsed_changed" => Some(EventSignalRef::mapped(
                "column_collapsed_changed",
                &self.column_collapsed_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            "add_card_requested" => {
                Some(EventSignalRef::unit("add_card_requested", &self.add_card_requested))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for KanbanColumnHeader {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "count" => Ok(CapabilityValue::UInt(self.count as u64)),
            "wip_limit" => Ok(match self.wip_limit {
                Some(limit) => CapabilityValue::UInt(limit as u64),
                None => CapabilityValue::Null,
            }),
            "collapsed" => Ok(CapabilityValue::Bool(self.collapsed)),
            "editable" => Ok(CapabilityValue::Bool(self.editable)),
            "at_wip_limit" => Ok(CapabilityValue::Bool(self.is_at_wip_limit())),
            "editing" => Ok(CapabilityValue::Bool(self.is_editing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.rename(expect_string(value)?);
                Ok(())
            }
            "count" => {
                let count = match value {
                    CapabilityValue::UInt(count) => {
                        usize::try_from(count).map_err(|_| CapabilityAccessError::OutOfRange)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_count(count);
                Ok(())
            }
            "wip_limit" => match value {
                CapabilityValue::Null => {
                    self.set_wip_limit(None);
                    Ok(())
                }
                CapabilityValue::UInt(limit) => {
                    self.set_wip_limit(Some(
                        usize::try_from(limit).map_err(|_| CapabilityAccessError::OutOfRange)?,
                    ));
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "collapsed" => {
                self.set_collapsed(expect_bool(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "at_wip_limit" | "editing" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "count",
            "wip_limit",
            "collapsed",
            "editable",
            "at_wip_limit",
            "editing",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for KanbanColumnHeader {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.edit_buffer.is_some() {
            match event {
                Event::TextInput { text } | Event::ImeCommit { text } => {
                    if let Some(buffer) = self.edit_buffer.as_mut() {
                        buffer.extend(text.chars().filter(|character| !character.is_control()));
                    }
                    self.base.request_redraw();
                }
                Event::KeyPress { key, .. } => match *key {
                    13 => {
                        let _ = self.commit_title_edit();
                    }
                    27 => {
                        self.cancel_title_edit();
                    }
                    8 => {
                        if let Some(buffer) = self.edit_buffer.as_mut() {
                            buffer.pop();
                        }
                        self.base.request_redraw();
                    }
                    _ => { /* Other keys are not relevant */ }
                },
                _ => {}
            }
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.collapse_rect().contains_point(*pos) {
                    self.set_collapsed(!self.collapsed);
                } else if self.add_rect().contains_point(*pos) {
                    self.add_card_requested.emit();
                }
            }
            Event::MouseDoubleClick { pos, button: 1, .. }
                if self.editable && self.base.geometry().contains_point(*pos) =>
            {
                let _ = self.begin_title_edit();
            }
            _ => {}
        }
    }
}

impl Draw for KanbanColumnHeader {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("kanban_board");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);
        let warning = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .unwrap_or(Color::PRIMARY);
        let font = role_font(style, 12.0);

        context.face_with_gradient(
            rect,
            surface.blend(&ink, 0.05),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        let chevron = if self.collapsed { IconName::ChevronRight } else { IconName::ChevronDown };
        crate::widget::draw_icon_at(context, self.collapse_rect(), ink, chevron);

        let title = match &self.edit_buffer {
            Some(buffer) => buffer.as_str(),
            None => self.title.as_str(),
        };
        let add = self.add_rect();
        let badge_width = context.measure_text(&format!("{}", self.count), &font).width as i32 + 10;
        let text_left = self.collapse_rect().x + HEADER_ICON + 6;
        let text_right = (add.x - 6 - badge_width).max(text_left + 8);
        let text = crate::widget::text_utils::elide_end(
            title,
            ((text_right - text_left).max(8) / 7) as usize,
        );
        context.draw_text(
            Point::new(text_left, rect.y + (rect.height as i32 - 12) / 2),
            &text,
            &font,
            ink,
            HorizontalAlignment::Left,
        );

        let badge =
            Rect::new(text_right, rect.y + (rect.height as i32 - 16) / 2, badge_width as u32, 16);
        context.fill_rounded_rect(badge, 8, surface.blend(&ink, 0.12));
        let badge_text = if let Some(limit) = self.wip_limit {
            format!("{}/{limit}", self.count)
        } else {
            format!("{}", self.count)
        };
        context.draw_text(
            Point::new(badge.x + 5, badge.y + 2),
            &badge_text,
            &font,
            if self.is_at_wip_limit() { warning } else { ink },
            HorizontalAlignment::Left,
        );

        crate::widget::draw_icon_at(context, add, ink.blend(&surface, 0.25), IconName::Plus);
        // The accent is read for the at-limit outline so a theme's warning colour is the one thing
        // the reader sees when a column is full; the stroke sits inside the badge.
        if self.is_at_wip_limit() {
            context.draw_rect(badge, warning.blend(&accent, 0.2));
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
    fn card_view_toggles_done_and_reports_a_committed_edit() {
        let mut card = KanbanCardView::new(Rect::new(0, 0, 220, 72), "Write spec");
        card.set_description("Owner: Ada");
        card.set_tags(vec!["ui".to_string(), "p1".to_string()]);

        let done = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = Arc::clone(&done);
        card.card_done_toggled.connect(move |flag| {
            sink.lock().expect("done sink").push(*flag);
        });
        let clicked = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&clicked);
        card.card_clicked.connect(move || {
            *sink.lock().expect("click sink") += 1;
        });
        let edited = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&edited);
        card.card_edited.connect(move |title| {
            sink.lock().expect("edit sink").push(title.as_str().to_string());
        });

        let box_rect = card.done_box_rect();
        card.handle_event(&Event::mouse_press(box_rect.x + 2, box_rect.y + 2, 1));
        assert!(card.is_done());
        assert_eq!(*done.lock().expect("done sink"), vec![true]);

        card.handle_event(&Event::mouse_press(100, 50, 1));
        assert_eq!(*clicked.lock().expect("click sink"), 1);

        assert!(card.begin_title_edit());
        card.handle_event(&Event::text_input("!"));
        card.handle_event(&Event::key_press(13, 0));
        assert_eq!(card.title(), "Write spec!");
        assert_eq!(edited.lock().expect("edit sink").as_slice(), ["Write spec!".to_string()]);
    }

    #[test]
    fn card_view_properties_round_trip() {
        let mut card = KanbanCardView::new(Rect::new(0, 0, 220, 72), "Card");
        assert_eq!(card.get("title"), Ok(CapabilityValue::String("Card".to_string())));
        assert!(card.set("done", CapabilityValue::Bool(true)).is_ok());
        assert!(card.is_done());
        assert!(card
            .set("tags", CapabilityValue::Tuple(vec![CapabilityValue::String("a".to_string())]))
            .is_ok());
        assert_eq!(card.tags(), ["a".to_string()]);
        assert_eq!(
            card.set("editing", CapabilityValue::Bool(false)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn column_header_reports_rename_collapse_and_add() {
        let mut header = KanbanColumnHeader::new(Rect::new(0, 0, 220, 32), "Todo");
        header.set_count(2);
        header.set_wip_limit(Some(2));
        assert!(header.is_at_wip_limit());

        let renamed = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&renamed);
        header.column_renamed.connect(move |title| {
            sink.lock().expect("rename sink").push(title.as_str().to_string());
        });
        let collapsed = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = Arc::clone(&collapsed);
        header.column_collapsed_changed.connect(move |flag| {
            sink.lock().expect("collapse sink").push(*flag);
        });
        let added = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&added);
        header.add_card_requested.connect(move || {
            *sink.lock().expect("add sink") += 1;
        });

        let add = header.add_rect();
        header.handle_event(&Event::mouse_press(add.x + 2, add.y + 2, 1));
        assert_eq!(*added.lock().expect("add sink"), 1);

        let collapse = header.collapse_rect();
        header.handle_event(&Event::mouse_press(collapse.x + 2, collapse.y + 2, 1));
        assert!(header.is_collapsed());
        assert_eq!(*collapsed.lock().expect("collapse sink"), vec![true]);

        assert!(header.rename("In Progress"));
        assert!(!header.rename("In Progress"), "a no-op rename is not reported");
        assert_eq!(renamed.lock().expect("rename sink").as_slice(), ["In Progress".to_string()]);
        assert_eq!(header.get("wip_limit"), Ok(CapabilityValue::UInt(2)));
    }

    #[test]
    fn column_header_edits_its_title_when_editable() {
        let mut header = KanbanColumnHeader::new(Rect::new(0, 0, 220, 32), "Todo");
        assert!(header.begin_title_edit());
        header.handle_event(&Event::text_input("s"));
        header.handle_event(&Event::key_press(13, 0));
        assert_eq!(header.title(), "Todos");

        header.set_editable(false);
        assert!(!header.begin_title_edit(), "a read-only header cannot be renamed by the reader");
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
        assert_eq!(second.x - first.x, DEFAULT_COLUMN_WIDTH as i32 + COLUMN_GAP);
        assert_eq!(first.width, DEFAULT_COLUMN_WIDTH);
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
            .as_chunks::<4>()
            .0
            .iter()
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
        for name in ["column_count", "card_count", "dragging_card_id", "hovered_column"] {
            assert_eq!(
                board.set(name, CapabilityValue::UInt(3)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
        // `column_width` used to be in that list; SP-15 makes it the one display dimension the
        // caller owns, so it is writable and only the derived counts stay read-only.
        assert!(board.set("column_width", CapabilityValue::UInt(300)).is_ok());
        assert_eq!(board.column_width(), 300);
        assert_eq!(board.get("column_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(board.get("card_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(board.get("column_width").unwrap(), CapabilityValue::UInt(300));
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

    #[test]
    fn column_width_is_writable_and_moves_the_next_column() {
        let mut board = board();
        let before = board.column_rect(1).expect("column 1").x;
        board.set_column_width(320);
        assert_eq!(board.column_width(), 320);
        assert_eq!(board.get("column_width").unwrap(), CapabilityValue::UInt(320));
        let after = board.column_rect(1).expect("column 1").x;
        assert_eq!(after - before, 100, "a wider column must displace the one beside it");
        assert_eq!(board.column_rect(0).unwrap().width, 320);

        // A width below the floor is clamped rather than producing an unusable column.
        board.set("column_width", CapabilityValue::UInt(1)).expect("writable");
        assert_eq!(board.column_width(), MIN_COLUMN_WIDTH);
    }

    #[test]
    fn horizontal_scroll_shifts_columns_and_clamps_to_content() {
        // Three 220 px columns plus gaps are narrower than this board, so the test widens them
        // first to give the content somewhere to scroll.
        let mut board = board();
        board.set_column_width(400);
        let unscrolled = board.column_rect(0).expect("column 0").x;
        board.set_horizontal_scroll(100);
        assert_eq!(board.horizontal_scroll(), 100);
        assert_eq!(board.column_rect(0).unwrap().x, unscrolled - 100);

        // Past the end is clamped to the content's own width, not left unbounded.
        board.set_horizontal_scroll(100_000);
        assert_eq!(board.horizontal_scroll(), board.max_horizontal_scroll());
        // Reset and a negative request both stop at zero.
        board.set_horizontal_scroll(-50);
        assert_eq!(board.horizontal_scroll(), 0);

        // Shift-wheel scrolls horizontally instead of scrolling a column's cards.
        board.handle_event(&Event::wheel(0, -120, 0x0001));
        assert!(board.horizontal_scroll() > 0);
    }

    #[test]
    fn horizontal_scroll_is_published_as_a_property() {
        let mut board = board();
        board.set_column_width(400);
        board.set("horizontal_scroll_offset", CapabilityValue::UInt(64)).expect("writable");
        assert_eq!(board.horizontal_scroll(), 64);
        assert_eq!(board.get("horizontal_scroll_offset").unwrap(), CapabilityValue::UInt(64));
    }

    fn lane_board() -> KanbanBoard {
        let mut board = KanbanBoard::new(Rect::new(0, 0, 600, 400));
        board.add_column(KanbanColumn::new("todo", "To Do"));
        board.add_card(0, KanbanCard::new("a", "First"));
        board.add_card(0, KanbanCard::new("b", "Second"));
        board.add_lane(KanbanLane::new("fast", "Fast"));
        board.add_lane(KanbanLane::new("slow", "Slow"));
        board.set_lane_mode(KanbanLaneMode::Lanes);
        board
    }

    /// The lane model: a board-wide card id maps to a lane id, and an unassigned card renders in
    /// the first lane rather than disappearing.
    #[test]
    fn lanes_assign_cards_and_drive_layout() {
        let mut board = lane_board();
        assert_eq!(board.lane_count(), 2);
        assert_eq!(board.lane("slow").map(|lane| lane.title.as_str()), Some("Slow"));
        assert!(board.set_card_lane("b", "slow"));
        assert_eq!(board.card_lane("b"), Some("slow"));
        assert_eq!(board.lane_of("b"), Some("slow"), "lane_of is the read alias");
        assert_eq!(board.card_lane("a"), None, "unassigned cards fall into lane 0");
        assert!(!board.set_card_lane("missing", "slow"));
        assert!(!board.set_card_lane("a", "missing"));
        assert!(board.set_lane("a", "slow"), "set_lane is the write alias");
        assert_eq!(board.lane_of("a"), Some("slow"));
        assert!(!board.set_lane("missing", "slow"));
        // Restore the fixture's layout expectation: `a` again renders in the first lane.
        assert!(board.set_lane("a", "fast"));

        let lane0 = board.lane_band_rect(0).expect("lane 0");
        let lane1 = board.lane_band_rect(1).expect("lane 1");
        assert_eq!(lane1.x, LANE_HEADER_WIDTH as i32, "bands start after the labels");
        assert!(lane1.y >= lane0.y + lane0.height as i32);
        assert_eq!(board.lane_header_rect(1).expect("header").x, 0);

        let a = board.card_rect(CardPosition { column: 0, card: 0 }).expect("a laid out");
        let b = board.card_rect(CardPosition { column: 0, card: 1 }).expect("b laid out");
        assert!(a.y >= lane0.y && a.y < lane1.y, "a sits in the first band");
        assert!(b.y >= lane1.y, "b sits in the second band");
    }

    /// Paint and hit agree about the lane: a card hit-tests inside its band, and a painted lane
    /// board produces a frame.
    #[test]
    fn lane_mode_paints_and_hit_tests_inside_the_band() {
        let mut board = lane_board();
        assert!(board.set_card_lane("b", "slow"));
        let frame = render(&mut board, Size::new(600, 400));
        assert!(!frame.is_empty());

        let b = board.card_rect(CardPosition { column: 0, card: 1 }).expect("b laid out");
        board.handle_event(&Event::mouse_move(b.x + 10, b.y + 10));
        assert_eq!(board.hovered_card(), Some(CardPosition { column: 0, card: 1 }));

        // The lane strip itself is not a card slot.
        board.handle_event(&Event::mouse_move(10, b.y + 10));
        assert_eq!(board.hovered_card(), None);
    }

    /// A release inside another band reassigns the card, so the model follows the drop.
    #[test]
    fn dropping_a_card_in_another_lane_reassigns_it() {
        let mut board = lane_board();
        assert!(board.set_card_lane("b", "slow"));
        let a = board.card_rect(CardPosition { column: 0, card: 0 }).expect("a laid out");
        let slow = board.lane_band_rect(1).expect("slow lane");
        let target_y = slow.y + LANE_HEADER_HEIGHT + 10;

        board.handle_event(&Event::mouse_press(a.x + 10, a.y + 10, 1));
        board.handle_event(&Event::mouse_move(a.x + 10, target_y));
        board.handle_event(&Event::mouse_release(a.x + 10, target_y, 1));
        assert_eq!(board.card_lane("a"), Some("slow"));
    }

    #[test]
    fn lane_mode_is_a_property() {
        let mut board = lane_board();
        assert_eq!(board.get("lane_mode").unwrap(), CapabilityValue::String("lanes".to_string()));
        assert_eq!(board.get("swimlanes").unwrap(), CapabilityValue::Bool(true));
        board.set("swimlanes", CapabilityValue::Bool(false)).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::None);
        board.set("swimlanes", CapabilityValue::Bool(true)).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::Lanes);
        board.set("lane_mode", CapabilityValue::String("none".to_string())).expect("writable");
        assert_eq!(board.lane_mode(), KanbanLaneMode::None);
        assert!(board.set("lane_mode", CapabilityValue::String("diagonal".to_string())).is_err());
        assert!(board.set("swimlanes", CapabilityValue::UInt(1)).is_err());
        assert_eq!(
            board.set("lane_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }
}
