// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DropDownPopupHost` — the shared anchored pop-up surface for the selector family
//! (COMP-A21).
//!
//! # Why a companion
//!
//! The six selector controls (combo box, drop-down, editable combo box, multi-select combo
//! box, cascader and the recent-values popup) each painted their own overlay. The rows looked
//! similar but the clamping, the flip direction, the grab and the "which row is under the
//! pointer" arithmetic were six independent copies, so the same widget family behaved
//! differently depending on which selector a screen happened to use. This host owns that
//! surface once.
//!
//! # What it owns and what it does not
//!
//! The host owns the pop-up state: the row list, the anchor, the placement, the highlight and
//! the scroll window. It owns no trigger and no selected value — a selector calls
//! [`DropDownPopupHost::open_at`], receives [`DropDownPopupHost::item_clicked`] and applies the
//! chosen row to its own model. That split is what lets one host serve six selectors without
//! knowing any of them.
//!
//! # Placement
//!
//! [`PopupPlacement::Automatic`] opens below the anchor when the pop-up fits there and flips
//! above when it does not and the space above is larger. A forced [`PopupPlacement::Below`] or
//! [`PopupPlacement::Above`] keeps the requested side; the final rectangle is then clamped into
//! the host's geometry, so a pop-up anchored at the window edge stays on screen rather than
//! being clipped. The host's own rectangle is the available viewport.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::{
    PropertyValueKind, RecordField, RecordList, RecordSchema, RecordValue, WidgetProperties,
};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, clamp_axis, surface_ink};

/// Height of one pop-up row, in logical pixels.
const POPUP_ROW_HEIGHT: u32 = 32;
/// Inset from a row's edge to its label.
const POPUP_INSET: u32 = 12;
/// The most rows a pop-up shows before it scrolls.
const POPUP_DEFAULT_MAX_ROWS: usize = 8;
/// The narrowest pop-up a selector produces, even when its anchor is narrower.
const POPUP_MIN_WIDTH: u32 = 120;
/// Width of the painted scroll indicator.
const POPUP_SCROLLBAR_WIDTH: u32 = 4;

/// The direction of one highlight step.
///
/// Shared by the selector pop-up, the command palette and the header menu so that "move to the
/// next row" has one spelling across the companion set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Move toward the end of the list, wrapping at the last row.
    Next,
    /// Move toward the start of the list, wrapping at the first row.
    Previous,
}

/// Where a pop-up is placed relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupPlacement {
    /// Always open below the anchor, then clamp into the viewport.
    Below,
    /// Always open above the anchor, then clamp into the viewport.
    Above,
    /// Open below when it fits, otherwise above when that side has more room.
    Automatic,
}

impl Default for PopupPlacement {
    fn default() -> Self {
        Self::Automatic
    }
}

impl PopupPlacement {
    /// The token used by the property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Below => "below",
            Self::Above => "above",
            Self::Automatic => "automatic",
        }
    }

    /// Parses the token used by the property contract.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "below" => Some(Self::Below),
            "above" => Some(Self::Above),
            "automatic" => Some(Self::Automatic),
            _ => None,
        }
    }
}

/// One row in a [`DropDownPopupHost`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupItem {
    /// The row's stable identity; this is what a selector stores as its current value.
    pub id: String,
    /// The text painted in the row.
    pub label: String,
    /// Whether the row can be highlighted and activated.
    pub enabled: bool,
}

impl PopupItem {
    /// Creates an enabled row.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), enabled: true }
    }

    /// Sets the row's enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// One row of the `items` records property: `id`, `label` and an optional `enabled` flag.
///
/// The list is flat because the property vocabulary has no record-of-record kind. The host
/// addresses rows by index internally, but `id` is the identity a selector persists, so the
/// schema declares it required and unique.
pub const POPUP_ITEM_SCHEMA: RecordSchema = RecordSchema {
    fields: &[
        RecordField::required("id", PropertyValueKind::String),
        RecordField::required("label", PropertyValueKind::String),
        RecordField::optional("enabled", PropertyValueKind::Bool),
    ],
    id_field: "id",
    ids_unique: true,
    ids_required: true,
};

/// A pop-up row list anchored by a host, with keyboard and pointer selection.
pub struct DropDownPopupHost {
    base: BaseWidget,
    items: Vec<PopupItem>,
    open: bool,
    highlighted_index: Option<usize>,
    /// The first visible row, stored as a float so a caller may animate toward a row. Painting
    /// and hit testing use the nearest whole row.
    scroll_offset: f32,
    max_visible_rows: usize,
    anchor_rect: Rect,
    placement: PopupPlacement,
    /// Emitted with the chosen row; the selector applies it to its own value.
    pub item_clicked: Signal1<PopupItem>,
    /// Emitted when the highlighted row changes, including keyboard movement.
    pub highlighted_changed: Signal1<usize>,
    /// Emitted when the pop-up is dismissed by Escape or a press outside it.
    pub cancelled: GenericSignal,
}

impl DropDownPopupHost {
    /// Creates a closed pop-up with no rows.
    ///
    /// The host's `geometry` is the available viewport; the pop-up itself is a rectangle
    /// computed inside it by [`Self::panel_rect`].
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "DropDownPopupHost"),
            items: Vec::new(),
            open: false,
            highlighted_index: None,
            scroll_offset: 0.0,
            max_visible_rows: POPUP_DEFAULT_MAX_ROWS,
            anchor_rect: Rect::new(geometry.x, geometry.y, 0, 0),
            placement: PopupPlacement::Automatic,
            item_clicked: Signal1::new(),
            highlighted_changed: Signal1::new(),
            cancelled: GenericSignal::new(),
        }
    }

    /// Opens the pop-up against `anchor`, rewinding the scroll window and highlighting the first
    /// enabled row.
    pub fn open_at(&mut self, anchor: Rect) {
        self.anchor_rect = anchor;
        self.open = true;
        self.scroll_offset = 0.0;
        self.highlighted_index = self.first_enabled();
        self.base.request_redraw();
    }

    /// Closes the pop-up without reporting a cancellation.
    pub fn close(&mut self) {
        if self.open || self.highlighted_index.is_some() {
            self.open = false;
            self.highlighted_index = None;
            self.base.request_redraw();
        }
    }

    /// Whether the pop-up is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Replaces the rows, clamping the highlight and the scroll window into the new list.
    pub fn set_items(&mut self, items: Vec<PopupItem>) {
        self.items = items;
        if self.highlighted_index.is_some_and(|index| index >= self.items.len()) {
            self.highlighted_index = None;
        }
        if self.open {
            // A replacement list may have disabled or removed the row that was highlighted.
            let usable = self
                .highlighted_index
                .and_then(|index| self.items.get(index))
                .is_some_and(|item| item.enabled);
            if !usable {
                self.highlighted_index = self.first_enabled();
            }
        }
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// Returns the rows.
    pub fn items(&self) -> &[PopupItem] {
        &self.items
    }

    /// Returns the anchor rectangle last passed to [`Self::open_at`].
    pub fn anchor_rect(&self) -> Rect {
        self.anchor_rect
    }

    /// Returns the configured placement policy.
    pub fn placement(&self) -> PopupPlacement {
        self.placement
    }

    /// Sets the placement policy.
    pub fn set_placement(&mut self, placement: PopupPlacement) {
        if self.placement != placement {
            self.placement = placement;
            self.base.request_redraw();
        }
    }

    /// The highlighted row, when one is enabled.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Sets the highlighted row, publishing the change.
    ///
    /// Returns `false` when the index names no row; the highlight is left untouched in that case.
    pub fn set_highlighted_index(&mut self, index: Option<usize>) -> bool {
        if index.is_some_and(|value| value >= self.items.len()) {
            return false;
        }
        if self.highlighted_index == index {
            return true;
        }
        self.highlighted_index = index;
        if let Some(value) = index {
            self.ensure_highlight_visible();
            self.highlighted_changed.emit(value);
        }
        self.base.request_redraw();
        true
    }

    /// The maximum number of rows shown at once.
    pub fn max_visible_rows(&self) -> usize {
        self.max_visible_rows.max(1)
    }

    /// Sets the maximum number of rows shown at once; longer lists scroll.
    pub fn set_max_visible_rows(&mut self, rows: usize) {
        let rows = rows.max(1);
        if self.max_visible_rows == rows {
            return;
        }
        self.max_visible_rows = rows;
        self.ensure_highlight_visible();
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The first visible row, as a float.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Sets the first visible row, clamped into the list and rejecting non-finite values.
    pub fn set_scroll_offset(&mut self, offset: f32) {
        let offset = if offset.is_finite() { offset } else { 0.0 };
        let max = self.max_scroll() as f32;
        let clamped = offset.clamp(0.0, max);
        if self.scroll_offset != clamped {
            self.scroll_offset = clamped;
            self.base.request_redraw();
        }
    }

    /// Moves the highlight one row in `direction`, wrapping and skipping disabled rows.
    pub fn move_highlight(&mut self, direction: Direction) {
        if self.items.is_empty() {
            return;
        }
        let count = self.items.len();
        let step = match direction {
            Direction::Next => 1,
            Direction::Previous => -1,
        };
        let start = match self.highlighted_index {
            Some(index) => index as i32,
            None => {
                if step > 0 {
                    -1
                } else {
                    0
                }
            }
        };
        let mut cursor = start;
        for _ in 0..count {
            cursor = (cursor + step).rem_euclid(count as i32);
            if self.items[cursor as usize].enabled {
                let _ = self.set_highlighted_index(Some(cursor as usize));
                return;
            }
        }
        // Every row is disabled: there is no row a highlight could name, so clear it rather than
        // leaving the highlight on a row the user cannot choose.
        let _ = self.set_highlighted_index(None);
    }

    /// Activates `index`, reporting whether it named an enabled row.
    ///
    /// A successful activation emits [`Self::item_clicked`] and closes the pop-up.
    pub fn activate(&mut self, index: usize) -> bool {
        if !self.open {
            return false;
        }
        let Some(item) = self.items.get(index) else {
            return false;
        };
        if !item.enabled {
            return false;
        }
        let chosen = item.clone();
        self.item_clicked.emit(chosen);
        self.close();
        true
    }

    /// Activates the highlighted row, when there is one.
    pub fn activate_highlighted(&mut self) -> bool {
        match self.highlighted_index {
            Some(index) => self.activate(index),
            None => false,
        }
    }

    /// The rectangle the pop-up occupies, already flipped and clamped into the viewport.
    ///
    /// A host uses this to anchor a submenu, to paint a shadow, or to decide whether a press
    /// belongs to the pop-up.
    pub fn panel_rect(&self) -> Rect {
        let viewport = self.geometry();
        let (width, height) = self.panel_size();
        if width == 0 || height == 0 {
            return Rect::new(viewport.x, viewport.y, width, height);
        }
        let anchor = self.anchor_rect;
        let space_below = (viewport.bottom() - anchor.bottom()).max(0) as u32;
        let space_above = (anchor.y - viewport.y).max(0) as u32;
        let place_above = match self.placement {
            PopupPlacement::Above => true,
            PopupPlacement::Below => false,
            PopupPlacement::Automatic => space_below < height && space_above > space_below,
        };
        let height_i32 = i32::try_from(height).unwrap_or(i32::MAX);
        let width_i32 = i32::try_from(width).unwrap_or(i32::MAX);
        let raw_y = if place_above { anchor.y.saturating_sub(height_i32) } else { anchor.bottom() };
        let max_x = viewport.right().saturating_sub(width_i32);
        let max_y = viewport.bottom().saturating_sub(height_i32);
        Rect::new(
            clamp_axis(anchor.x, viewport.x, max_x),
            clamp_axis(raw_y, viewport.y, max_y),
            width,
            height,
        )
    }

    /// The number of rows that fit the pop-up's window.
    fn visible_rows(&self) -> usize {
        let by_height = (self.geometry().height / POPUP_ROW_HEIGHT) as usize;
        self.items.len().min(self.max_visible_rows.max(1)).min(by_height.max(1))
    }

    /// The first enabled row, if any.
    fn first_enabled(&self) -> Option<usize> {
        self.items.iter().position(|item| item.enabled)
    }

    /// The largest valid scroll offset, in rows.
    fn max_scroll(&self) -> usize {
        self.items.len().saturating_sub(self.visible_rows())
    }

    /// The scroll offset rounded to the row painting and hit testing use.
    fn effective_scroll(&self) -> usize {
        if self.scroll_offset.is_finite() {
            self.scroll_offset.round().max(0.0) as usize
        } else {
            0
        }
    }

    /// The first row in the painted window.
    fn first_visible_row(&self) -> usize {
        self.effective_scroll().min(self.max_scroll())
    }

    /// Keeps the scroll offset inside the list after the rows or the window changed.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll() as f32;
        let offset = if self.scroll_offset.is_finite() { self.scroll_offset } else { 0.0 };
        self.scroll_offset = offset.clamp(0.0, max);
    }

    /// Scrolls the window so the highlighted row is visible.
    fn ensure_highlight_visible(&mut self) {
        let Some(index) = self.highlighted_index else {
            return;
        };
        let visible = self.visible_rows().max(1);
        let first = self.effective_scroll();
        if index < first {
            self.scroll_offset = index as f32;
        } else if index >= first + visible {
            self.scroll_offset = (index + 1 - visible) as f32;
        }
        self.clamp_scroll();
    }

    /// The pop-up's requested size, clamped to the viewport.
    fn panel_size(&self) -> (u32, u32) {
        let viewport = self.geometry();
        let requested_width = self.anchor_rect.width.max(POPUP_MIN_WIDTH);
        let width = if viewport.width == 0 { 0 } else { requested_width.min(viewport.width) };
        let rows = self.visible_rows().max(1) as u32;
        let requested_height = rows * POPUP_ROW_HEIGHT;
        let height = if viewport.height == 0 { 0 } else { requested_height.min(viewport.height) };
        (width, height)
    }

    /// The row under `pos`, when the pointer is inside the painted window.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.panel_rect();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = ((pos.y - rect.y) as u32 / POPUP_ROW_HEIGHT) as usize;
        if row >= self.visible_rows() {
            return None;
        }
        let index = self.first_visible_row() + row;
        (index < self.items.len()).then_some(index)
    }

    /// Dismisses the pop-up and reports the cancellation.
    fn cancel(&mut self) {
        if self.open {
            self.close();
            self.cancelled.emit();
            self.base.request_redraw();
        }
    }

    /// Paints the scroll indicator when the list is longer than the window.
    fn draw_scrollbar(&self, context: &mut RenderContext, rect: Rect, surface: Color, ink: Color) {
        let track = Rect::new(
            rect.x + rect.width as i32 - POPUP_SCROLLBAR_WIDTH as i32 - 2,
            rect.y + 2,
            POPUP_SCROLLBAR_WIDTH,
            rect.height.saturating_sub(4),
        );
        if track.height == 0 {
            return;
        }
        context.fill_rect(track, surface.blend(&ink, 0.08));
        let total = self.items.len();
        if total == 0 {
            return;
        }
        let visible = self.visible_rows();
        let thumb_height = ((track.height as u64 * visible as u64) / total as u64).max(12) as u32;
        let thumb_height = thumb_height.min(track.height);
        let max_scroll = self.max_scroll();
        let progress =
            if max_scroll == 0 { 0.0 } else { self.first_visible_row() as f32 / max_scroll as f32 };
        let travel = track.height.saturating_sub(thumb_height);
        let thumb_y = track.y + (travel as f32 * progress).round() as i32;
        context.fill_rect(
            Rect::new(track.x, thumb_y, track.width, thumb_height),
            surface.blend(&ink, 0.35),
        );
    }
}

impl EventHandler for DropDownPopupHost {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.open {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                if let Some(index) = self.row_at(*pos) {
                    let _ = self.set_highlighted_index(Some(index));
                }
            }
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                match self.row_at(*pos) {
                    Some(index) => {
                        let _ = self.activate(index);
                    }
                    None => {
                        if !self.panel_rect().contains_point(*pos) {
                            self.cancel();
                        }
                    }
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y == 0 || self.items.len() <= self.visible_rows() {
                    return;
                }
                let step = if delta.y < 0 { 1.0 } else { -1.0 };
                self.set_scroll_offset(self.scroll_offset.round() + step);
            }
            Event::KeyPress { key, .. } => match *key {
                key_codes::UP => self.move_highlight(Direction::Previous),
                key_codes::DOWN => self.move_highlight(Direction::Next),
                key_codes::HOME => {
                    if let Some(index) = self.items.iter().position(|item| item.enabled) {
                        let _ = self.set_highlighted_index(Some(index));
                    }
                }
                key_codes::END => {
                    if let Some(index) = self.items.iter().rposition(|item| item.enabled) {
                        let _ = self.set_highlighted_index(Some(index));
                    }
                }
                key_codes::ENTER | key_codes::LINE_FEED => {
                    let _ = self.activate_highlighted();
                }
                key_codes::ESCAPE => self.cancel(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for DropDownPopupHost {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }
        let rect = self.panel_rect();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        let border = surface.blend(&ink, 0.22);
        let highlight = surface.blend(&accent, 0.18);

        context.fill_rect(rect, surface);
        context.draw_rect(rect, border);

        if self.items.is_empty() {
            context.draw_text_fitted_cow(
                rect,
                "No options",
                font,
                surface.blend(&ink, 0.55),
                HorizontalAlignment::Left,
            );
            return;
        }

        let visible = self.visible_rows();
        let first = self.first_visible_row();
        for offset in 0..visible {
            let index = first + offset;
            let Some(item) = self.items.get(index) else {
                break;
            };
            let row = Rect::new(
                rect.x,
                rect.y + (offset as u32 * POPUP_ROW_HEIGHT) as i32,
                rect.width,
                POPUP_ROW_HEIGHT,
            );
            if self.highlighted_index == Some(index) {
                context.fill_rect(row, highlight);
            }
            let row_ink = if item.enabled { ink } else { ink.blend(&surface, 0.5) };
            let text = Rect::new(
                row.x + POPUP_INSET as i32,
                row.y,
                row.width.saturating_sub(POPUP_INSET * 2),
                row.height,
            );
            context.draw_text_fitted_cow(
                text,
                &item.label,
                font,
                row_ink,
                HorizontalAlignment::Left,
            );
        }
        if self.items.len() > visible {
            self.draw_scrollbar(context, rect, surface, ink);
        }
    }
}

impl Widget for DropDownPopupHost {
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
        let rows = self.visible_rows().max(1) as u32;
        Size::new(self.anchor_rect.width.max(POPUP_MIN_WIDTH), rows * POPUP_ROW_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            // The chosen row's identity; a selector persists `id`, not the painted label.
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |item| {
                    CapabilityValue::String(item.id.clone())
                }))
            }
            "highlighted_changed" => Some(EventSignalRef::mapped(
                "highlighted_changed",
                &self.highlighted_changed,
                |index| CapabilityValue::UInt(*index as u64),
            )),
            "cancelled" => Some(EventSignalRef::unit("cancelled", &self.cancelled)),
            _ => None,
        }
    }
}

/// One row of [`POPUP_ITEM_SCHEMA`].
fn popup_item_record(item: &PopupItem) -> RecordValue {
    RecordValue::new()
        .with("id", CapabilityValue::String(item.id.clone()))
        .with("label", CapabilityValue::String(item.label.clone()))
        .with("enabled", CapabilityValue::Bool(item.enabled))
}

/// Decodes the `items` write into rows.
///
/// The whole list is decoded before `set_items` runs, so a refused row leaves the host
/// unchanged.
fn popup_items_from_records(list: &RecordList) -> Result<Vec<PopupItem>, CapabilityAccessError> {
    let mut items = Vec::with_capacity(list.items().len());
    for record in list.items() {
        let id = match record.get("id") {
            Some(CapabilityValue::String(value)) => value.clone(),
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let label = match record.get("label") {
            Some(CapabilityValue::String(value)) => value.clone(),
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        // An omitted `enabled` is the same default `PopupItem::new` gives a row.
        let enabled = match record.get("enabled") {
            None | Some(CapabilityValue::Null) => true,
            Some(CapabilityValue::Bool(value)) => *value,
            Some(_) => return Err(CapabilityAccessError::TypeMismatch),
        };
        items.push(PopupItem { id, label, enabled });
    }
    Ok(items)
}

impl WidgetProperties for DropDownPopupHost {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => Ok(CapabilityValue::Records(RecordList::from_items(
                &POPUP_ITEM_SCHEMA,
                self.items.iter().map(popup_item_record).collect(),
            ))),
            "open" => Ok(CapabilityValue::Bool(self.open)),
            "highlighted_index" => Ok(match self.highlighted_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "scroll_offset" => Ok(CapabilityValue::Float(f64::from(self.scroll_offset))),
            "max_visible_rows" => Ok(CapabilityValue::UInt(self.max_visible_rows() as u64)),
            "anchor_rect" => Ok(CapabilityValue::Rect(self.anchor_rect)),
            "placement" => Ok(CapabilityValue::String(self.placement.as_str().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "items" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&POPUP_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("drop_down_popup_host.items refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("items"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                self.set_items(popup_items_from_records(&list)?);
                Ok(())
            }
            "open" => {
                if expect_bool(value)? {
                    self.open_at(self.anchor_rect);
                } else {
                    self.close();
                }
                Ok(())
            }
            "highlighted_index" => match value {
                CapabilityValue::Null => {
                    let _ = self.set_highlighted_index(None);
                    Ok(())
                }
                other => {
                    let index = expect_usize(other)?;
                    if self.set_highlighted_index(Some(index)) {
                        Ok(())
                    } else {
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
            "scroll_offset" => {
                self.set_scroll_offset(expect_f32(value)?);
                Ok(())
            }
            "max_visible_rows" => {
                self.set_max_visible_rows(expect_usize(value)?);
                Ok(())
            }
            "anchor_rect" => {
                let CapabilityValue::Rect(rect) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if self.anchor_rect != rect {
                    self.anchor_rect = rect;
                    self.base.request_redraw();
                }
                Ok(())
            }
            "placement" => {
                let token = expect_string(value)?;
                self.set_placement(
                    PopupPlacement::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)?,
                );
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "open",
            "highlighted_index",
            "scroll_offset",
            "max_visible_rows",
            "anchor_rect",
            "placement",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn host() -> DropDownPopupHost {
        let mut host = DropDownPopupHost::new(Rect::new(0, 0, 200, 200));
        host.set_items(vec![
            PopupItem::new("a", "Alpha"),
            PopupItem::new("b", "Bravo").with_enabled(false),
            PopupItem::new("c", "Charlie"),
        ]);
        host
    }

    #[test]
    fn the_popup_flips_above_when_below_does_not_fit() {
        let mut host = host();

        host.open_at(Rect::new(20, 20, 100, 16));
        let below = host.panel_rect();
        assert_eq!(below.y, 36, "the panel opens below the anchor when there is room");

        host.open_at(Rect::new(20, 180, 100, 16));
        let above = host.panel_rect();
        assert!(above.bottom() <= 180, "the panel flips above the anchor");
        assert!(above.y < 180);

        // A wide anchor at the right edge is clamped rather than drawn out of the viewport.
        host.open_at(Rect::new(180, 20, 100, 16));
        assert!(host.panel_rect().right() <= 200);
    }

    #[test]
    fn the_highlight_moves_and_skips_disabled_rows() {
        let mut host = host();
        host.open_at(Rect::new(20, 20, 100, 16));
        assert_eq!(host.highlighted_index(), Some(0), "opening highlights the first enabled row");

        host.move_highlight(Direction::Next);
        assert_eq!(host.highlighted_index(), Some(2), "the disabled row is skipped");
        host.move_highlight(Direction::Next);
        assert_eq!(host.highlighted_index(), Some(0), "the walk wraps");
        host.move_highlight(Direction::Previous);
        assert_eq!(host.highlighted_index(), Some(2));
    }

    #[test]
    fn a_click_emits_the_item_and_closes() {
        let mut host = host();
        host.open_at(Rect::new(20, 20, 100, 16));
        let fired = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&fired);
        host.item_clicked.connect(move |item| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(item.id.clone());
            }
        });

        let rect = host.panel_rect();
        let pos = Point::new(rect.x + 4, rect.y + POPUP_ROW_HEIGHT as i32 * 2 + 4);
        host.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });

        assert_eq!(*fired.lock().unwrap(), vec![String::from("c")]);
        assert!(!host.is_open());
    }

    #[test]
    fn escape_cancels_and_reports_the_dismissal() {
        let mut host = host();
        host.open_at(Rect::new(20, 20, 100, 16));
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        host.cancelled.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        host.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!host.is_open());
        assert_eq!(host.highlighted_index(), None);
    }
}
