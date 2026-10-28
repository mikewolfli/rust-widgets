// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `HeaderMenu` — the sort/filter/hide/column-chooser menu a table header opens (COMP-A32).
//!
//! # Why a companion
//!
//! A grid header is the one place where four unrelated intents meet: change the sort, filter the
//! column, hide it, or open the column chooser. Each table family built its own menu for those
//! four actions, so the same column could offer "Sort ascending" in one screen and an icon-only
//! toggle in the next. This menu owns the small pop-up once.
//!
//! # Relationship to `MenuPanel`
//!
//! The rows, highlight and hit testing are the [`MenuPanel`](super::MenuPanel)
//! interaction model, specialised to a fixed action list: a header menu is not a data-driven
//! menu, because its rows are derived from `sortable`, `filterable` and the current sort state.
//! The panel's own model is reused as the design rather than as a dependency so this control
//! stays a small, self-contained companion.
//!
//! [`SortState::None`] is a real state: clearing a sort is an action a header offers, and it is
//! reported through the same [`HeaderMenu::sort_requested`] signal as ascending and descending so
//! the grid has one path for all three.

use crate::compat::{lock, Arc, Mutex, String, ToString, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::view_widgets::filter_expr::FilterOperator;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, clamp_axis, surface_ink};

pub use super::drop_down_popup_host::Direction;

/// Height of one header-menu row.
const HEADER_ROW_HEIGHT: u32 = 28;
/// Horizontal inset of a row's label.
const HEADER_INSET: u32 = 12;
/// The narrowest header menu, even when its header cell is narrower.
const HEADER_MENU_MIN_WIDTH: u32 = 168;

/// The sort a column is currently under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortState {
    /// The column is not part of the sort.
    #[default]
    None,
    /// The column sorts ascending.
    Asc,
    /// The column sorts descending.
    Desc,
}

impl SortState {
    /// The token used by the property contract and the sort signal.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }

    /// Parses the token used by the property contract.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "asc" => Some(Self::Asc),
            "desc" => Some(Self::Desc),
            _ => None,
        }
    }
}

/// One row a [`HeaderMenu`] can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMenuAction {
    /// Sort the column ascending.
    SortAscending,
    /// Sort the column descending.
    SortDescending,
    /// Remove the column from the sort.
    ClearSort,
    /// Filter the column through `FilterOperator`.
    Filter(FilterOperator),
    /// Hide the column.
    HideColumn,
    /// Open the column chooser for this table.
    ChooseColumns,
}

impl HeaderMenuAction {
    /// The text painted for this row.
    pub fn label(self) -> &'static str {
        match self {
            Self::SortAscending => "Sort ascending",
            Self::SortDescending => "Sort descending",
            Self::ClearSort => "Clear sort",
            Self::Filter(_) => "Filter",
            Self::HideColumn => "Hide column",
            Self::ChooseColumns => "Choose columns",
        }
    }
}

/// The menu a header cell opens for sort, filter, hide and column-chooser intents.
pub struct HeaderMenu {
    base: BaseWidget,
    column_token: String,
    anchor_rect: Rect,
    open: bool,
    selected_index: usize,
    sortable: bool,
    filterable: bool,
    sort_state: SortState,
    filter_text: String,
    /// The `(column token, filter text)` a bound table reads when it applies a request.
    ///
    /// `sort_requested`, `hide_column_requested` and `column_chooser_requested` carry the token
    /// themselves, but `filter_requested` carries only the operator, so the grid needs the menu's
    /// current column and operand. The pair is mirrored here because a table attaches with
    /// `&HeaderMenu` and cannot call the `&mut self` setters.
    context: Arc<Mutex<(String, String)>>,
    /// Emitted as `(column token, requested state)`; `None` in the state clears the sort.
    pub sort_requested: Signal1<(String, SortState)>,
    /// Emitted with the operator the Filter row requests.
    pub filter_requested: Signal1<FilterOperator>,
    /// Emitted with the column token when Hide column is chosen.
    pub hide_column_requested: Signal1<String>,
    /// Emitted with the column token when Choose columns is chosen.
    pub column_chooser_requested: Signal1<String>,
    /// Emitted when the menu is dismissed without choosing a row.
    pub cancelled: GenericSignal,
}

impl HeaderMenu {
    /// Creates a closed menu whose rows are all available.
    ///
    /// The base widget starts hidden, the same way a `Menu` does: a header menu is opened by a
    /// header cell rather than displayed with the grid. That also makes the `visible` property
    /// agree with the default the shared `WidgetKind::Menu` declares for it.
    pub fn new(geometry: Rect) -> Self {
        let mut base = BaseWidget::new(WidgetKind::Menu, geometry, "HeaderMenu");
        base.hide();
        Self {
            base,
            column_token: String::new(),
            anchor_rect: Rect::new(geometry.x, geometry.y, 0, 0),
            open: false,
            selected_index: 0,
            sortable: true,
            filterable: true,
            sort_state: SortState::None,
            filter_text: String::new(),
            context: Arc::new(Mutex::new((String::new(), String::new()))),
            sort_requested: Signal1::new(),
            filter_requested: Signal1::new(),
            hide_column_requested: Signal1::new(),
            column_chooser_requested: Signal1::new(),
            cancelled: GenericSignal::new(),
        }
    }

    /// Opens the menu for `column_token`, anchored at the header cell's rectangle.
    pub fn open_for(&mut self, column_token: impl Into<String>, anchor: Rect) {
        self.column_token = column_token.into();
        self.anchor_rect = anchor;
        self.open = true;
        self.selected_index = 0;
        self.clamp_selection();
        self.sync_context();
        self.base.show();
        self.base.request_redraw();
    }

    /// Closes the menu without reporting a cancellation.
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.selected_index = 0;
            self.base.hide();
            self.base.request_redraw();
        }
    }

    /// Whether the menu is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The token of the column this menu was opened for.
    pub fn column_token(&self) -> &str {
        &self.column_token
    }

    /// The anchor rectangle last passed to [`Self::open_for`].
    pub fn anchor_rect(&self) -> Rect {
        self.anchor_rect
    }

    /// The current sort state of the column.
    pub fn sort_state(&self) -> SortState {
        self.sort_state
    }

    /// Sets the sort state without emitting a request; a host mirrors its grid's state here.
    pub fn set_sort_state(&mut self, state: SortState) {
        if self.sort_state != state {
            self.sort_state = state;
            self.clamp_selection();
            self.base.request_redraw();
        }
    }

    /// The filter text painted beside the Filter row.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    /// Sets the filter text without emitting a request.
    pub fn set_filter_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.filter_text != text {
            self.filter_text = text;
            self.sync_context();
            self.base.request_redraw();
        }
    }

    /// The shared `(column token, filter text)` pair a bound table reads when it applies a
    /// reported request (COMP-A8).
    ///
    /// The filter signal carries only the operator, so this is how a table learns which column the
    /// menu was opened for and what operand the user typed. The pair is updated by [`Self::open_for`]
    /// and [`Self::set_filter_text`], so a table sees the values in force when the row was
    /// activated. A writer reads it when a `HeaderMenu` signal arrives; it must not hold the lock
    /// across its own signal work.
    pub fn context_handle(&self) -> Arc<Mutex<(String, String)>> {
        Arc::clone(&self.context)
    }

    /// Mirrors the mutable fields a bound table reads through [`Self::context_handle`].
    fn sync_context(&self) {
        let mut context = lock(&self.context);
        *context = (self.column_token.clone(), self.filter_text.clone());
    }

    /// Whether sort rows are offered.
    pub fn sortable(&self) -> bool {
        self.sortable
    }

    /// Enables or removes the sort rows.
    pub fn set_sortable(&mut self, sortable: bool) {
        if self.sortable != sortable {
            self.sortable = sortable;
            self.clamp_selection();
            self.base.request_redraw();
        }
    }

    /// Whether the Filter row is offered.
    pub fn filterable(&self) -> bool {
        self.filterable
    }

    /// Enables or removes the Filter row.
    pub fn set_filterable(&mut self, filterable: bool) {
        if self.filterable != filterable {
            self.filterable = filterable;
            self.clamp_selection();
            self.base.request_redraw();
        }
    }

    /// The rows this menu currently offers, in paint order.
    ///
    /// The list is derived rather than stored: the sort rows depend on `sortable` and the current
    /// state, the filter row on `filterable`, and Hide/Choose are always available. A host that
    /// needs to mirror the menu in another surface calls this rather than reconstructing it.
    pub fn items(&self) -> Vec<HeaderMenuAction> {
        let mut items = Vec::new();
        if self.sortable {
            items.push(HeaderMenuAction::SortAscending);
            items.push(HeaderMenuAction::SortDescending);
            if self.sort_state != SortState::None {
                items.push(HeaderMenuAction::ClearSort);
            }
        }
        if self.filterable {
            items.push(HeaderMenuAction::Filter(FilterOperator::Contains));
        }
        items.push(HeaderMenuAction::HideColumn);
        items.push(HeaderMenuAction::ChooseColumns);
        items
    }

    /// The selected row's index in [`Self::items`].
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Moves the selection one row in `direction`, wrapping at the ends.
    pub fn move_selection(&mut self, direction: Direction) {
        let count = self.items().len();
        if count == 0 {
            return;
        }
        let current = self.selected_index.min(count - 1);
        let next = match direction {
            Direction::Next => (current + 1) % count,
            Direction::Previous => (current + count - 1) % count,
        };
        if self.selected_index != next {
            self.selected_index = next;
            self.base.request_redraw();
        }
    }

    /// Activates the selected row, reporting whether there was one.
    ///
    /// Activation emits the row's intent and closes the menu.
    pub fn activate_selected(&mut self) -> bool {
        if !self.open {
            return false;
        }
        let items = self.items();
        let Some(action) = items.get(self.selected_index).copied() else {
            return false;
        };
        match action {
            HeaderMenuAction::SortAscending => {
                self.set_sort_state(SortState::Asc);
                self.sort_requested.emit((self.column_token.clone(), SortState::Asc));
            }
            HeaderMenuAction::SortDescending => {
                self.set_sort_state(SortState::Desc);
                self.sort_requested.emit((self.column_token.clone(), SortState::Desc));
            }
            HeaderMenuAction::ClearSort => {
                self.set_sort_state(SortState::None);
                self.sort_requested.emit((self.column_token.clone(), SortState::None));
            }
            HeaderMenuAction::Filter(operator) => self.filter_requested.emit(operator),
            HeaderMenuAction::HideColumn => {
                self.hide_column_requested.emit(self.column_token.clone());
            }
            HeaderMenuAction::ChooseColumns => {
                self.column_chooser_requested.emit(self.column_token.clone());
            }
        }
        self.close();
        true
    }

    /// The rectangle the menu occupies, flipped and clamped into the viewport.
    pub fn panel_rect(&self) -> Rect {
        let viewport = self.geometry();
        let rows = self.items().len().max(1) as u32;
        let requested_width = self.anchor_rect.width.max(HEADER_MENU_MIN_WIDTH);
        let width = if viewport.width == 0 { 0 } else { requested_width.min(viewport.width) };
        let requested_height = rows * HEADER_ROW_HEIGHT;
        let height = if viewport.height == 0 { 0 } else { requested_height.min(viewport.height) };
        if width == 0 || height == 0 {
            return Rect::new(viewport.x, viewport.y, width, height);
        }
        let anchor = self.anchor_rect;
        let space_below = (viewport.bottom() - anchor.bottom()).max(0) as u32;
        let space_above = (anchor.y - viewport.y).max(0) as u32;
        let height_i32 = i32::try_from(height).unwrap_or(i32::MAX);
        let width_i32 = i32::try_from(width).unwrap_or(i32::MAX);
        // A header menu opens below the cell and flips only when below cannot hold it and the
        // space above is larger, which is the same rule the selector pop-up uses.
        let place_above = space_below < height && space_above > space_below;
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

    /// Keeps the selection inside the current row list.
    fn clamp_selection(&mut self) {
        let count = self.items().len();
        if count == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    /// Whether a row names the state the column is already in.
    fn action_is_current(&self, action: HeaderMenuAction) -> bool {
        match action {
            HeaderMenuAction::SortAscending => self.sort_state == SortState::Asc,
            HeaderMenuAction::SortDescending => self.sort_state == SortState::Desc,
            HeaderMenuAction::Filter(_) => !self.filter_text.is_empty(),
            HeaderMenuAction::ClearSort
            | HeaderMenuAction::HideColumn
            | HeaderMenuAction::ChooseColumns => false,
        }
    }

    /// The row index under `pos`, when the pointer is over the menu.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.panel_rect();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = ((pos.y - rect.y) as u32 / HEADER_ROW_HEIGHT) as usize;
        (row < self.items().len()).then_some(row)
    }

    /// Dismisses the menu and reports the cancellation.
    fn cancel(&mut self) {
        if self.open {
            self.close();
            self.cancelled.emit();
            self.base.request_redraw();
        }
    }
}

impl EventHandler for HeaderMenu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.open {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                if let Some(row) = self.row_at(*pos) {
                    if self.selected_index != row {
                        self.selected_index = row;
                        self.base.request_redraw();
                    }
                }
            }
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                match self.row_at(*pos) {
                    Some(row) => {
                        self.selected_index = row;
                        let _ = self.activate_selected();
                    }
                    None => {
                        if !self.panel_rect().contains_point(*pos) {
                            self.cancel();
                        }
                    }
                }
            }
            Event::KeyPress { key, .. } => match *key {
                key_codes::UP => self.move_selection(Direction::Previous),
                key_codes::DOWN => self.move_selection(Direction::Next),
                key_codes::HOME => {
                    self.selected_index = 0;
                    self.base.request_redraw();
                }
                key_codes::END => {
                    self.selected_index = self.items().len().saturating_sub(1);
                    self.base.request_redraw();
                }
                key_codes::ENTER | key_codes::LINE_FEED => {
                    let _ = self.activate_selected();
                }
                key_codes::ESCAPE => self.cancel(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for HeaderMenu {
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
        let items = self.items();
        let highlight = surface.blend(&accent, 0.18);

        context.fill_rect(rect, surface);

        let mut separated = false;
        for (index, action) in items.iter().enumerate() {
            let row = Rect::new(
                rect.x,
                rect.y + (index as u32 * HEADER_ROW_HEIGHT) as i32,
                rect.width,
                HEADER_ROW_HEIGHT,
            );
            if row.y >= rect.bottom() {
                break;
            }
            if self.selected_index == index {
                context.fill_rect(row, highlight);
            }
            // The destructive/column-wide actions are separated from the sort/filter group.
            if !separated && matches!(action, HeaderMenuAction::HideColumn) {
                context.fill_rect(
                    Rect::new(
                        row.x + HEADER_INSET as i32,
                        row.y,
                        row.width.saturating_sub(HEADER_INSET * 2),
                        1,
                    ),
                    surface.blend(&ink, 0.18),
                );
                separated = true;
            }
            let text_x = row.x + HEADER_INSET as i32 + 12;
            if self.action_is_current(*action) {
                context.fill_rect(
                    Rect::new(
                        row.x + HEADER_INSET as i32,
                        row.y + 10,
                        4,
                        HEADER_ROW_HEIGHT.saturating_sub(20),
                    ),
                    accent,
                );
            }
            let text_width =
                if matches!(action, HeaderMenuAction::Filter(_)) && !self.filter_text.is_empty() {
                    (row.width / 2).saturating_sub(HEADER_INSET)
                } else {
                    (row.x + row.width as i32 - HEADER_INSET as i32 - text_x).max(0) as u32
                };
            if text_width > 0 {
                context.draw_text_fitted_cow(
                    Rect::new(text_x, row.y, text_width, row.height),
                    action.label(),
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
            if matches!(action, HeaderMenuAction::Filter(_)) && !self.filter_text.is_empty() {
                let width = (row.width / 2).saturating_sub(HEADER_INSET);
                if width > 0 {
                    let hint = Rect::new(
                        row.right() - HEADER_INSET as i32 - width as i32,
                        row.y,
                        width,
                        row.height,
                    );
                    context.draw_text_fitted_cow(
                        hint,
                        &self.filter_text,
                        font,
                        surface.blend(&ink, 0.55),
                        HorizontalAlignment::Right,
                    );
                }
            }
        }

        context.draw_rect(rect, surface.blend(&ink, 0.22));
    }
}

impl Widget for HeaderMenu {
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
        Size::new(HEADER_MENU_MIN_WIDTH, self.items().len().max(1) as u32 * HEADER_ROW_HEIGHT)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "sort_requested" => {
                Some(EventSignalRef::mapped("sort_requested", &self.sort_requested, |value| {
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::String(value.0.clone()),
                        CapabilityValue::String(value.1.as_str().to_string()),
                    ])
                }))
            }
            "filter_requested" => Some(EventSignalRef::mapped(
                "filter_requested",
                &self.filter_requested,
                |operator| CapabilityValue::String(operator.as_str().to_string()),
            )),
            "hide_column_requested" => Some(EventSignalRef::mapped(
                "hide_column_requested",
                &self.hide_column_requested,
                |token| CapabilityValue::String(token.clone()),
            )),
            "column_chooser_requested" => Some(EventSignalRef::mapped(
                "column_chooser_requested",
                &self.column_chooser_requested,
                |token| CapabilityValue::String(token.clone()),
            )),
            "cancelled" => Some(EventSignalRef::unit("cancelled", &self.cancelled)),
            _ => None,
        }
    }
}

impl WidgetProperties for HeaderMenu {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "sortable" => Ok(CapabilityValue::Bool(self.sortable)),
            "filterable" => Ok(CapabilityValue::Bool(self.filterable)),
            "sort_state" => Ok(CapabilityValue::String(self.sort_state.as_str().to_string())),
            "filter_text" => Ok(CapabilityValue::String(self.filter_text.clone())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "sortable" => {
                self.set_sortable(expect_bool(value)?);
                Ok(())
            }
            "filterable" => {
                self.set_filterable(expect_bool(value)?);
                Ok(())
            }
            "sort_state" => {
                let token = expect_string(value)?;
                self.set_sort_state(
                    SortState::from_name(&token).ok_or(CapabilityAccessError::TypeMismatch)?,
                );
                Ok(())
            }
            "filter_text" => {
                self.set_filter_text(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "sortable",
            "filterable",
            "sort_state",
            "filter_text",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn menu() -> HeaderMenu {
        HeaderMenu::new(Rect::new(0, 0, 320, 240))
    }

    fn anchor() -> Rect {
        Rect::new(20, 20, 120, 24)
    }

    #[test]
    fn the_sort_rows_cycle_through_the_states() {
        let mut menu = menu();
        let fired = Arc::new(Mutex::new(Vec::<(String, SortState)>::new()));
        let sink = Arc::clone(&fired);
        menu.sort_requested.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((value.0.clone(), value.1));
            }
        });

        menu.open_for("amount", anchor());
        assert!(menu.activate_selected());
        assert_eq!(menu.sort_state(), SortState::Asc);

        menu.open_for("amount", anchor());
        menu.move_selection(Direction::Next);
        assert!(menu.activate_selected());
        assert_eq!(menu.sort_state(), SortState::Desc);

        menu.open_for("amount", anchor());
        menu.move_selection(Direction::Next);
        menu.move_selection(Direction::Next);
        assert!(menu.activate_selected());
        assert_eq!(menu.sort_state(), SortState::None);

        assert_eq!(
            *fired.lock().unwrap(),
            vec![
                (String::from("amount"), SortState::Asc),
                (String::from("amount"), SortState::Desc),
                (String::from("amount"), SortState::None),
            ]
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn the_filter_row_emits_its_operator() {
        let mut menu = menu();
        menu.set_filter_text("berlin");
        let fired = Arc::new(Mutex::new(Vec::<FilterOperator>::new()));
        let sink = Arc::clone(&fired);
        menu.filter_requested.connect(move |operator| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*operator);
            }
        });

        menu.open_for("city", anchor());
        // With no active sort the rows are Asc, Desc, Filter, Hide, Choose.
        menu.move_selection(Direction::Next);
        menu.move_selection(Direction::Next);
        assert!(menu.activate_selected());

        assert_eq!(*fired.lock().unwrap(), vec![FilterOperator::Contains]);
        assert!(!menu.is_open());
    }

    #[test]
    fn hide_and_choose_report_the_column_token() {
        let mut menu = menu();
        let hidden = Arc::new(Mutex::new(Vec::<String>::new()));
        let chosen = Arc::new(Mutex::new(Vec::<String>::new()));

        let hide_sink = Arc::clone(&hidden);
        menu.hide_column_requested.connect(move |token| {
            if let Ok(mut guard) = hide_sink.lock() {
                guard.push((*token).clone());
            }
        });
        let choose_sink = Arc::clone(&chosen);
        menu.column_chooser_requested.connect(move |token| {
            if let Ok(mut guard) = choose_sink.lock() {
                guard.push((*token).clone());
            }
        });

        menu.open_for("amount", anchor());
        menu.move_selection(Direction::Next);
        menu.move_selection(Direction::Next);
        menu.move_selection(Direction::Next);
        assert!(menu.activate_selected());

        menu.open_for("amount", anchor());
        // From the first row, Previous wraps to the last row, Choose columns.
        menu.move_selection(Direction::Previous);
        assert!(menu.activate_selected());

        assert_eq!(*hidden.lock().unwrap(), vec![String::from("amount")]);
        assert_eq!(*chosen.lock().unwrap(), vec![String::from("amount")]);
    }

    #[test]
    fn escape_cancels_the_menu() {
        let mut menu = menu();
        let count = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&count);
        menu.cancelled.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        menu.open_for("amount", anchor());
        menu.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!menu.is_open());
    }
}
