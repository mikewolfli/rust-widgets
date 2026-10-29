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

use core::cell::RefCell;
use core::fmt::Write as _;
use std::collections::HashSet;

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
use crate::widget::companions::DragGhostLayer;
use crate::widget::metrics::role_font;
use crate::widget::{draw_icon_at, BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

mod board;
mod card;
mod column_header;
mod drag;
mod events;
mod lane;
mod layout;
mod model;
mod paint;
mod properties;

#[cfg(test)]
mod tests;

pub use self::board::KanbanBoard;
pub use self::card::KanbanCardView;
pub use self::column_header::KanbanColumnHeader;
pub use self::model::{
    CardPosition, KanbanCard, KanbanColumn, KanbanLane, KanbanLaneMode, KANBAN_CARD_SCHEMA,
    KANBAN_COLUMN_SCHEMA,
};

use self::card::*;
use self::drag::*;
use self::lane::*;
use self::model::*;
