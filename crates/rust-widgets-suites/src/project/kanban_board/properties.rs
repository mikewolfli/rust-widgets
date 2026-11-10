// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::board::remap_by_column_id;
use super::*;

/// `KanbanBoard`'s property contract.
///
/// The two levels of the model are two flat `Records` properties (`BLUE26 B26.09`): `columns`
/// carries column metadata, `cards` carries every card with its `column` index and optional `lane`
/// assignment. Each has a published `set_*` command that is the payload-free spelling of the write;
/// `add_column` / `add_card` / `move_card` are Rust-side APIs whose payloads are what those records
/// now transport, so they are not published (B26.03).
impl WidgetProperties for KanbanBoard {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "columns" => Ok(CapabilityValue::Records(
                rust_widgets::widget::capability::RecordList::from_items(
                    &KANBAN_COLUMN_SCHEMA,
                    self.columns().iter().map(kanban_column_record).collect(),
                ),
            )),
            "cards" => {
                let mut items = Vec::new();
                for (column, entry) in self.columns().iter().enumerate() {
                    for (card, value) in entry.cards.iter().enumerate() {
                        items.push(kanban_card_record(
                            CardPosition { column, card },
                            value,
                            self.lane_of(&value.id),
                        ));
                    }
                }
                Ok(CapabilityValue::Records(
                    rust_widgets::widget::capability::RecordList::from_items(
                        &KANBAN_CARD_SCHEMA,
                        items,
                    ),
                ))
            }
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
                // Every paint and hit path clamps on read; the property reports that same offset
                // rather than the raw field a resize can leave past the content.
                let offset = self.horizontal_scroll().max(0).min(self.max_horizontal_scroll());
                Ok(CapabilityValue::UInt(offset as u64))
            }
            "lane_mode" => Ok(CapabilityValue::String(self.lane_mode.token().to_string())),
            "swimlanes" => Ok(CapabilityValue::Bool(self.lane_mode == KanbanLaneMode::Lanes)),
            "lane_count" => Ok(CapabilityValue::UInt(self.lane_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // A column write replaces column state; cards are adopted **by column id** from the
            // existing board, because a card belongs to a column identity, not to a slot. A column
            // that disappears takes its cards with it, and their lane assignments are dropped.
            "columns" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&KANBAN_COLUMN_SCHEMA).map_err(|violation| {
                    log::warn!("kanban_board.columns refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: rust_widgets::compat::String::from("columns"),
                        reason: rust_widgets::compat::format!("{violation}"),
                    }
                })?;
                let mut columns = list
                    .items()
                    .iter()
                    .map(kanban_column_from_record)
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                // Index-keyed state (focus/hover/drag/scroll) names a slot in the *old* column
                // list; capture the identities before the slots move, so each piece of state can
                // follow the column it referred to.
                let old_ids: Vec<String> =
                    self.columns.iter().map(|column| column.id.clone()).collect();
                let old_scroll = self.column_scroll.clone();
                for column in &mut columns {
                    if let Some(existing) = self.columns.iter().find(|entry| entry.id == column.id)
                    {
                        column.cards = existing.cards.clone();
                    }
                }
                // A width override belongs to the column that owns it: a write that drops a column
                // must drop its override too, or the stale width resurrects if that id returns
                // (the rule `remove_column` already follows).
                let live_columns: HashSet<String> =
                    columns.iter().map(|column| column.id.clone()).collect();
                self.column_widths.retain(|id, _| live_columns.contains(id));
                self.columns = columns;
                let live: HashSet<String> = self
                    .columns
                    .iter()
                    .flat_map(|entry| entry.cards.iter().map(|card| card.id.clone()))
                    .collect();
                self.card_lanes.retain(|card_id, _| live.contains(card_id));
                // Per-column scroll offsets are keyed by index; carry each one to the column id it
                // belonged to. An identity write keeps them as they are, so a round-trip no longer
                // scrolled every column back to row 0.
                let order_unchanged = old_ids.len() == self.columns.len()
                    && old_ids.iter().zip(self.columns.iter()).all(|(old, new)| *old == new.id);
                if order_unchanged {
                    self.column_scroll.truncate(self.columns.len());
                    self.column_scroll.resize(self.columns.len(), 0);
                } else {
                    let mut carried = vec![0usize; self.columns.len()];
                    for (new_index, column) in self.columns.iter().enumerate() {
                        if let Some(old_index) = old_ids.iter().position(|id| *id == column.id) {
                            carried[new_index] = old_scroll.get(old_index).copied().unwrap_or(0);
                        }
                    }
                    self.column_scroll = carried;
                }
                // Focus, hover, drag origin and the hovered column follow their column ids; a
                // column that left takes its state with it (and reports the hover exit).
                self.focused_card = remap_by_column_id(self.focused_card, &old_ids, &self.columns);
                let hover_was_set = self.hovered_card.is_some();
                self.hovered_card = remap_by_column_id(self.hovered_card, &old_ids, &self.columns);
                if hover_was_set && self.hovered_card.is_none() {
                    self.card_hovered.emit(None);
                }
                self.drag_origin = remap_by_column_id(self.drag_origin, &old_ids, &self.columns);
                self.hovered_column = self.hovered_column.and_then(|old| {
                    old_ids
                        .get(old)
                        .and_then(|id| self.columns.iter().position(|column| column.id == *id))
                });
                self.invalidate_layout();
                self.prune_card_state();
                // The write can reorder the columns, and the selection's documented order is
                // display order. A pure reorder keeps the same ids, so length is not the test —
                // the `cards` arm's rule is.
                let selection_before = self.selected_cards.clone();
                self.sort_selection();
                if self.selected_cards != selection_before {
                    self.selection_changed.emit(self.selected_cards.clone());
                }
                self.clamp_horizontal_scroll();
                self.base.request_redraw();
                Ok(())
            }
            // A card write replaces the whole board's cards. Every row is checked before anything
            // is written: an unknown column or lane refuses the write with the board unchanged.
            "cards" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&KANBAN_CARD_SCHEMA).map_err(|violation| {
                    log::warn!("kanban_board.cards refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: rust_widgets::compat::String::from("cards"),
                        reason: rust_widgets::compat::format!("{violation}"),
                    }
                })?;
                let column_count = self.columns.len();
                // The whole rows list is being replaced, so index-keyed state has to be re-resolved
                // by card id: a slot can now hold a different card (or none).
                let card_id_at = |columns: &[KanbanColumn], position: CardPosition| {
                    columns
                        .get(position.column)
                        .and_then(|column| column.cards.get(position.card))
                        .map(|card| card.id.clone())
                };
                let focused_id =
                    self.focused_card.and_then(|position| card_id_at(&self.columns, position));
                let hovered_id =
                    self.hovered_card.and_then(|position| card_id_at(&self.columns, position));
                let origin_id =
                    self.drag_origin.and_then(|position| card_id_at(&self.columns, position));
                let selection_before = self.selected_cards.clone();
                let mut buckets: Vec<Vec<KanbanCard>> = vec![Vec::new(); column_count];
                let mut lanes: HashMap<String, String> = HashMap::new();
                for record in list.items() {
                    let (column, lane, card) = kanban_card_from_record(record)?;
                    let Some(bucket) = buckets.get_mut(column) else {
                        return Err(CapabilityAccessError::ConstraintViolation {
                            property: rust_widgets::compat::String::from("cards"),
                            reason: rust_widgets::compat::format!(
                                "card '{}' names column {column}, but the board has {column_count} column(s)",
                                card.id
                            ),
                        });
                    };
                    if let Some(lane) = &lane {
                        if !self.lanes.iter().any(|entry| entry.id == *lane) {
                            return Err(CapabilityAccessError::ConstraintViolation {
                                property: rust_widgets::compat::String::from("cards"),
                                reason: rust_widgets::compat::format!(
                                    "card '{}' names lane '{lane}', which the board does not have",
                                    card.id
                                ),
                            });
                        }
                        lanes.insert(card.id.clone(), lane.clone());
                    }
                    bucket.push(card);
                }
                for (entry, cards) in self.columns.iter_mut().zip(buckets) {
                    entry.cards = cards;
                }
                self.card_lanes = lanes;
                self.invalidate_layout();
                // The rows were replaced whole: the focus, hover, drag origin and selection can
                // now name a card (or a slot) that the new list does not contain.
                let hover_was_set = self.hovered_card.is_some();
                self.focused_card = focused_id.as_deref().and_then(|id| self.position_of(id));
                self.hovered_card = hovered_id.as_deref().and_then(|id| self.position_of(id));
                self.drag_origin = origin_id.as_deref().and_then(|id| self.position_of(id));
                if hover_was_set && self.hovered_card.is_none() {
                    self.card_hovered.emit(None);
                }
                self.prune_card_state();
                // A write may reorder the cards; the selection's documented order is display order,
                // so a pure reorder is republished (drops were already reported by `prune`).
                self.sort_selection();
                if self.selected_cards != selection_before
                    && self.selected_cards.len() == selection_before.len()
                {
                    self.selection_changed.emit(self.selected_cards.clone());
                }
                self.base.request_redraw();
                Ok(())
            }
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
            "columns",
            "cards",
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
    /// `cancel_drag` is the one command whose whole effect is to discard state, so it runs with
    /// no argument. The list-valued operations are payload-free spellings of a property write
    /// (B26.03): `set_columns` / `set_cards` name their payload through the `columns` / `cards`
    /// records properties, so a bare invocation reports that the value is what is missing. The
    /// write itself goes through the property route, and `add_column` / `add_card` / `move_card`
    /// are the Rust-side methods that same route calls; they are not published, because their
    /// payloads are exactly the records those properties now carry.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "cancel_drag" => {
                self.cancel_drag();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}
