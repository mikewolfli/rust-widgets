// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MenuPanel` — a reusable drop-down menu panel (COMP-A01).
//!
//! # Why this is a companion rather than a control
//!
//! `Button` and `ToggleButton` each hand-paint their own pop-up menu, and `Pagination`
//! paints a page-size list; the rows, highlight movement and hit testing were three
//! copies of one idea. This control is the shared panel: a host gives it a rectangle
//! and a row list, anchors it where it wants, and receives the chosen index. It owns
//! no trigger, which is what lets a button, a toolbar item and a page-size cell all
//! use the same panel.

use crate::compat::{String, ToString, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_draw_bridge, impl_widget_property_hooks, property_names_of};

use super::helpers::surface_ink;

/// Height of one menu row, in logical pixels.
const MENU_ROW_HEIGHT: u32 = 32;
/// Inset from the panel's edge to a row's label.
const MENU_INSET: u32 = 12;
/// The most rows a panel shows before it scrolls.
const MENU_DEFAULT_MAX_ROWS: usize = 8;

/// One row in a [`MenuPanel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuPanelItem {
    /// The row's label.
    pub text: String,
    /// An optional icon token shown before the label.
    pub icon: Option<String>,
    /// Whether the row can be activated.
    pub enabled: bool,
}

impl MenuPanelItem {
    /// Creates an enabled row with no icon.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), icon: None, enabled: true }
    }

    /// Adds an icon token.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets the initial enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A drop-down menu panel that a host anchors itself.
pub struct MenuPanel {
    base: BaseWidget,
    items: Vec<MenuPanelItem>,
    selected_index: Option<usize>,
    highlighted_index: Option<usize>,
    max_visible_rows: usize,
    /// The first row currently shown when the list is longer than the panel.
    scroll_offset: usize,
    open: bool,
    /// Emitted with the activated row's index.
    pub item_clicked: Signal1<usize>,
    /// Emitted when the highlighted row changes.
    pub highlighted_changed: Signal1<Option<usize>>,
    /// Emitted when the panel is dismissed by Escape or an outside press.
    pub cancelled: GenericSignal,
}

impl MenuPanel {
    /// Creates a closed panel occupying `geometry`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "MenuPanel"),
            items: Vec::new(),
            selected_index: None,
            highlighted_index: None,
            max_visible_rows: MENU_DEFAULT_MAX_ROWS,
            scroll_offset: 0,
            open: false,
            item_clicked: Signal1::new(),
            highlighted_changed: Signal1::new(),
            cancelled: GenericSignal::new(),
        }
    }

    /// Opens the panel.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.scroll_offset = 0;
            self.base.request_redraw();
        }
    }

    /// Closes the panel without reporting a cancellation.
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.highlighted_index = None;
            self.base.request_redraw();
        }
    }

    /// Toggles the panel.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Whether the panel is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Replaces the rows, clamping the selection, highlight and scroll offset.
    pub fn set_items(&mut self, items: Vec<MenuPanelItem>) {
        self.items = items;
        if self.selected_index.is_some_and(|index| index >= self.items.len()) {
            self.selected_index = None;
        }
        if self.highlighted_index.is_some_and(|index| index >= self.items.len()) {
            self.highlighted_index = None;
        }
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// Returns the rows.
    pub fn items(&self) -> &[MenuPanelItem] {
        &self.items
    }

    /// The selected row, when one has been activated.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sets the selected row, reporting whether the index names a row.
    pub fn set_selected_index(&mut self, index: Option<usize>) -> bool {
        if index.is_some_and(|index| index >= self.items.len()) {
            return false;
        }
        if self.selected_index != index {
            self.selected_index = index;
            self.base.request_redraw();
        }
        true
    }

    /// The highlighted row, when one is under the pointer or the keyboard.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted_index
    }

    /// Sets the highlighted row, publishing the change.
    pub fn set_highlighted_index(&mut self, index: Option<usize>) -> bool {
        if index.is_some_and(|index| index >= self.items.len()) {
            return false;
        }
        if self.highlighted_index == index {
            return true;
        }
        self.highlighted_index = index;
        self.ensure_highlight_visible();
        self.highlighted_changed.emit(index);
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
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The rectangle of `index`, or `None` when it is outside the visible window.
    ///
    /// A host uses this to anchor a submenu or to paint its own decoration on the row.
    pub fn item_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let visible = self.visible_rows();
        if index < self.scroll_offset || index >= self.scroll_offset + visible {
            return None;
        }
        let rect = self.geometry();
        let row = (index - self.scroll_offset) as u32;
        Some(Rect::new(
            rect.x,
            rect.y + (row * MENU_ROW_HEIGHT) as i32,
            rect.width,
            MENU_ROW_HEIGHT.min(rect.height),
        ))
    }

    /// The row under `pos`, when there is one.
    pub fn highlight_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row = ((pos.y - rect.y) as u32 / MENU_ROW_HEIGHT) as usize;
        let index = self.scroll_offset + row;
        (index < self.items.len()).then_some(index)
    }

    /// Moves the highlight by `delta` rows, wrapping and skipping disabled rows.
    ///
    /// A panel with no highlight starts from the selected row, or from the first row.
    pub fn move_highlight(&mut self, delta: i32) {
        if self.items.is_empty() {
            return;
        }
        let count = self.items.len();
        let step = if delta >= 0 { 1 } else { -1 };
        let start = self
            .highlighted_index
            .or(self.selected_index)
            .map(|index| index as i32)
            .unwrap_or(if step > 0 { -1 } else { 0 });
        let mut cursor = start;
        for _ in 0..count {
            cursor = (cursor + step).rem_euclid(count as i32);
            if self.items[cursor as usize].enabled {
                let _ = self.set_highlighted_index(Some(cursor as usize));
                return;
            }
        }
    }

    /// Activates `index`, reporting whether the row was enabled.
    pub fn activate(&mut self, index: usize) -> bool {
        let Some(item) = self.items.get(index) else {
            return false;
        };
        if !item.enabled {
            return false;
        }
        self.selected_index = Some(index);
        self.item_clicked.emit(index);
        self.close();
        true
    }

    /// The number of rows that fit the panel's window.
    fn visible_rows(&self) -> usize {
        self.items
            .len()
            .min(self.max_visible_rows.max(1))
            .min((self.geometry().height / MENU_ROW_HEIGHT).max(1) as usize)
    }

    /// Keeps the scroll window valid after the list or its window changed.
    fn clamp_scroll(&mut self) {
        let visible = self.visible_rows();
        let max_offset = self.items.len().saturating_sub(visible);
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Scrolls the window so the highlighted row is visible.
    fn ensure_highlight_visible(&mut self) {
        let Some(index) = self.highlighted_index else {
            return;
        };
        let visible = self.visible_rows();
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + visible {
            self.scroll_offset = index + 1 - visible;
        }
        self.clamp_scroll();
    }

    /// Dismisses the panel and reports the cancellation.
    fn cancel(&mut self) {
        if self.open {
            self.close();
            self.cancelled.emit();
            self.base.request_redraw();
        }
    }
}

impl EventHandler for MenuPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                if self.open {
                    let hovered = self.highlight_at(*pos);
                    let _ = self.set_highlighted_index(hovered);
                }
            }
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                if !self.open {
                    return;
                }
                match self.highlight_at(*pos) {
                    Some(index) => {
                        let _ = self.activate(index);
                    }
                    None => self.cancel(),
                }
            }
            Event::Wheel { delta, .. } => {
                if !self.open || self.items.len() <= self.visible_rows() || delta.y == 0 {
                    return;
                }
                let visible = self.visible_rows();
                let max_offset = self.items.len() - visible;
                let next = if delta.y < 0 {
                    self.scroll_offset.saturating_add(1)
                } else {
                    self.scroll_offset.saturating_sub(1)
                };
                let next = next.min(max_offset);
                if next != self.scroll_offset {
                    self.scroll_offset = next;
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, .. } if self.open => match *key {
                key_codes::UP => self.move_highlight(-1),
                key_codes::DOWN => self.move_highlight(1),
                key_codes::PAGE_UP => self.move_highlight(-(self.visible_rows() as i32)),
                key_codes::PAGE_DOWN => self.move_highlight(self.visible_rows() as i32),
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
                    if let Some(index) = self.highlighted_index {
                        let _ = self.activate(index);
                    }
                }
                key_codes::ESCAPE => self.cancel(),
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for MenuPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        let border = surface.blend(&ink, 0.2);
        let highlight = surface.blend(&ink, 0.12);

        context.fill_rect(rect, surface);
        let visible = self.visible_rows();
        for offset in 0..visible {
            let index = self.scroll_offset + offset;
            let Some(item) = self.items.get(index) else {
                break;
            };
            let row = Rect::new(
                rect.x,
                rect.y + (offset as u32 * MENU_ROW_HEIGHT) as i32,
                rect.width,
                MENU_ROW_HEIGHT,
            );
            if self.highlighted_index == Some(index) {
                context.fill_rect(row, highlight);
            }
            let row_ink = if item.enabled { ink } else { ink.blend(&surface, 0.5) };
            let line = context.text_line(row, font);
            let mut text_x = rect.x + MENU_INSET as i32;
            if let Some(icon) = item.icon.as_deref() {
                let side = line.height.max(1);
                let icon_rect = Rect::new(text_x, line.y, side, side);
                if !crate::widget::draw_icon_named(context, icon_rect, row_ink, icon) {
                    context.draw_text(
                        Point::new(text_x, line.y),
                        icon,
                        font,
                        row_ink,
                        HorizontalAlignment::Left,
                    );
                }
                text_x += side as i32 + 6;
            }
            let text_width =
                (rect.x + rect.width as i32 - MENU_INSET as i32 - text_x).max(0) as u32;
            if text_width > 0 {
                context.draw_text_fitted_cow(
                    Rect::new(text_x, line.y, text_width, line.height.max(1)),
                    &item.text,
                    font,
                    row_ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        context.draw_rect(rect, border);
    }
}

impl Widget for MenuPanel {
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
        Size::new(180, (self.visible_rows() as u32 * MENU_ROW_HEIGHT).max(MENU_ROW_HEIGHT))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "highlighted_changed" => Some(EventSignalRef::mapped(
                "highlighted_changed",
                &self.highlighted_changed,
                |value| match value {
                    Some(index) => CapabilityValue::UInt(*index as u64),
                    None => CapabilityValue::Null,
                },
            )),
            "cancelled" => Some(EventSignalRef::unit("cancelled", &self.cancelled)),
            _ => None,
        }
    }
}

/// Encodes one row as `[text, icon-or-null, enabled]`.
fn item_value(item: &MenuPanelItem) -> CapabilityValue {
    CapabilityValue::Tuple(Vec::from([
        CapabilityValue::String(item.text.clone()),
        match item.icon.as_deref() {
            Some(icon) => CapabilityValue::String(icon.to_string()),
            None => CapabilityValue::Null,
        },
        CapabilityValue::Bool(item.enabled),
    ]))
}

/// Decodes the row-list property.
fn expect_items(value: CapabilityValue) -> Result<Vec<MenuPanelItem>, CapabilityAccessError> {
    let CapabilityValue::Tuple(rows) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let CapabilityValue::Tuple(parts) = row else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let text = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(text) => text,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let icon = match parts.next().ok_or(CapabilityAccessError::TypeMismatch)? {
            CapabilityValue::String(icon) => Some(icon),
            CapabilityValue::Null => None,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        let enabled = expect_bool(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        items.push(MenuPanelItem { text, icon, enabled });
    }
    Ok(items)
}

impl WidgetProperties for MenuPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => Ok(CapabilityValue::Tuple(self.items.iter().map(item_value).collect())),
            "selected_index" => Ok(match self.selected_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "highlighted_index" => Ok(match self.highlighted_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "max_visible_rows" => Ok(CapabilityValue::UInt(self.max_visible_rows() as u64)),
            "open" => Ok(CapabilityValue::Bool(self.open)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "items" => {
                self.set_items(expect_items(value)?);
                Ok(())
            }
            "selected_index" => match value {
                CapabilityValue::Null => {
                    let _ = self.set_selected_index(None);
                    Ok(())
                }
                other => {
                    let index = expect_usize(other)?;
                    if self.set_selected_index(Some(index)) {
                        Ok(())
                    } else {
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
            },
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
            "max_visible_rows" => {
                self.set_max_visible_rows(expect_usize(value)?);
                Ok(())
            }
            "open" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "selected_index",
            "highlighted_index",
            "max_visible_rows",
            "open",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn panel() -> MenuPanel {
        let mut panel = MenuPanel::new(Rect::new(0, 0, 180, 100));
        panel.set_items(vec![
            MenuPanelItem::new("Open"),
            MenuPanelItem::new("Disabled").with_enabled(false),
            MenuPanelItem::new("Close").with_icon("cross"),
        ]);
        panel.open();
        panel
    }

    #[test]
    fn activation_reports_the_row_and_closes() {
        let mut panel = panel();
        let fired = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&fired);
        panel.item_clicked.connect(move |index| sink.lock().unwrap().push(*index));
        assert!(panel.activate(2));
        assert_eq!(*fired.lock().unwrap(), vec![2]);
        assert_eq!(panel.selected_index(), Some(2));
        assert!(!panel.is_open());
    }

    #[test]
    fn the_highlight_wraps_and_skips_disabled_rows() {
        let mut panel = panel();
        panel.move_highlight(1);
        assert_eq!(panel.highlighted_index(), Some(0));
        panel.move_highlight(1);
        assert_eq!(panel.highlighted_index(), Some(2), "the disabled row is skipped");
        panel.move_highlight(1);
        assert_eq!(panel.highlighted_index(), Some(0), "the walk wraps");
    }

    #[test]
    fn a_long_list_scrolls_to_keep_the_highlight_visible() {
        let mut panel = panel();
        panel.set_max_visible_rows(1);
        assert!(panel.item_rect(1).is_none());
        assert!(panel.set_highlighted_index(Some(2)));
        assert!(panel.item_rect(2).is_some(), "the window followed the highlight");
        assert_eq!(panel.item_rect(2).unwrap().y, panel.geometry().y);
    }
}
