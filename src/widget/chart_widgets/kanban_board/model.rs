// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// The drag payload type every card of this control carries.
///
/// Namespaced with `kanban_` so a board that shares a window with another
/// drag source cannot mistake an unrelated payload for one of its cards.
pub(super) const CARD_PAYLOAD_TYPE: &str = "kanban_card";

/// How far the pointer must travel before a press on a card becomes a drag.
///
/// Four pixels: large enough that a click on a card's label is not read as a
/// one-pixel drag, small enough that a deliberate move registers immediately.
pub(super) const CARD_DRAG_THRESHOLD: i32 = 4;

/// The default width of each column, in logical pixels, and the gap between them.
pub(super) const DEFAULT_COLUMN_WIDTH: u32 = 220;
pub(super) const COLUMN_GAP: i32 = 12;

/// The narrowest a column may be made: below this a card's title is one glyph and a dash.
pub(super) const MIN_COLUMN_WIDTH: u32 = 80;

/// How far one horizontal wheel notch scrolls the board, in pixels.
pub(super) const HORIZONTAL_SCROLL_STEP: i32 = 48;
/// Width of the edge band that advances the board while a card is dragged.
pub(super) const DRAG_AUTOSCROLL_BAND: i32 = 28;

/// The height of a column's header strip.
pub(super) const HEADER_HEIGHT: u32 = 32;

/// The height of one card, and the vertical gap between cards.
pub(super) const CARD_HEIGHT: u32 = 48;
pub(super) const CARD_GAP: i32 = 8;

/// The horizontal padding of a card's or a column header's text from its own box edge: 10.
///
/// One value for two labels that share a leading edge, so a column's header title and the card
/// titles below it line up rather than each deriving its own inset.
pub(super) const CARD_PADDING: i32 = 10;

/// Width of the lane-label strip on the left of a swimlane board.
pub(super) const LANE_HEADER_WIDTH: u32 = 96;
/// Height of a lane's own caption row, inside its band.
pub(super) const LANE_HEADER_HEIGHT: i32 = 24;
/// Vertical gap between two lane bands.
pub(super) const LANE_GAP: i32 = 6;

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

/// The row shape of the `columns` property (`BLUE26 B26.09`).
///
/// A column is one flat record: identity, header, collapse flag and WIP limit. The cards it holds
/// are **not** nested here — the property vocabulary has no record-of-records kind (a `Records`
/// field value is refused, see `records.rs`), so the board publishes its two levels as two flat
/// lists: `columns` (this schema) and `cards` ([`KANBAN_CARD_SCHEMA`]), whose `column` field is the
/// index into this list. Card order within a column is the order of the `cards` list.
pub const KANBAN_COLUMN_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "title",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "collapsed",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "wip_limit",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// The row shape of the `cards` property (`BLUE26 B26.09`).
///
/// A card is one flat record. `column` is the **index** into the current `columns` list, because a
/// column reference by index is what [`CardPosition`] already means; `lane` names the swimlane the
/// card is assigned to, when it has one. Both lists travel together: a `cards` write is refused
/// when any row names a column or lane that does not exist, and it never writes a half-applied
/// board.
pub const KANBAN_CARD_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "title",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "column",
                crate::widget::capability::PropertyValueKind::UInt,
            ),
            crate::widget::capability::RecordField::optional(
                "description",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "done",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "lane",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`KANBAN_COLUMN_SCHEMA`].
pub(super) fn kanban_column_record(column: &KanbanColumn) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(column.id.clone()))
        .with("title", CapabilityValue::String(column.title.clone()))
        .with("collapsed", CapabilityValue::Bool(column.collapsed));
    if let Some(limit) = column.wip_limit {
        record = record.with("wip_limit", CapabilityValue::UInt(limit as u64));
    }
    record
}

/// Rebuilds one column's metadata from a row of [`KANBAN_COLUMN_SCHEMA`].
///
/// The returned column has no cards: a `columns` write carries column state only, and the board
/// adopts the existing cards of a column with the same id (see the `columns` arm of
/// [`KanbanBoard`]'s `set`).
pub(super) fn kanban_column_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<KanbanColumn, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let mut column = KanbanColumn::new(text("id")?, text("title")?);
    match record.get("collapsed") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::Bool(collapsed)) => column.collapsed = *collapsed,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    match record.get("wip_limit") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::UInt(limit)) => {
            column.wip_limit =
                Some(usize::try_from(*limit).map_err(|_| CapabilityAccessError::TypeMismatch)?);
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    Ok(column)
}

/// One row of [`KANBAN_CARD_SCHEMA`].
pub(super) fn kanban_card_record(
    position: CardPosition,
    card: &KanbanCard,
    lane: Option<&str>,
) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(card.id.clone()))
        .with("title", CapabilityValue::String(card.title.clone()))
        .with("description", CapabilityValue::String(card.description.clone()))
        .with("done", CapabilityValue::Bool(card.done))
        .with("column", CapabilityValue::UInt(position.column as u64));
    if let Some(lane) = lane {
        record = record.with("lane", CapabilityValue::String(lane.to_string()));
    }
    record
}

/// Rebuilds `(column index, lane, card)` from a row of [`KANBAN_CARD_SCHEMA`].
pub(super) fn kanban_card_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<(usize, Option<String>, KanbanCard), CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let column = match record.get("column") {
        Some(CapabilityValue::UInt(index)) => {
            usize::try_from(*index).map_err(|_| CapabilityAccessError::TypeMismatch)?
        }
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let mut card = KanbanCard::new(text("id")?, text("title")?);
    match record.get("description") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::String(description)) => card.description = description.clone(),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    match record.get("done") {
        None | Some(CapabilityValue::Null) => {}
        Some(CapabilityValue::Bool(done)) => card.done = *done,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    }
    let lane = match record.get("lane") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(lane)) => Some(lane.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok((column, lane, card))
}

/// Where a card is: which column, and its index within that column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardPosition {
    /// Index of the column.
    pub column: usize,
    /// Index of the card within the column.
    pub card: usize,
}
