// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `PopupItemList` — a small bordered item list a host opens at a point (COMP-L06).
//!
//! A toolbar's overflow control and a source list's context menu both need the same thing: a
//! rounded panel of rows that reports which row was chosen and dismisses itself. Nothing in the
//! crate carries that shape, so this companion does.
//!
//! Registration surface (out of scope for this file): canonical name `popup_item_list`, kind
//! [`WidgetKind::PopupWindow`].

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// Which side of the anchor the panel opens toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupAnchor {
    /// Below the anchor point.
    #[default]
    Below,
    /// Above the anchor point.
    Above,
    /// To the right of the anchor point.
    Right,
    /// To the left of the anchor point.
    Left,
}

/// One row of a popup list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupItem {
    /// The row's caption.
    pub label: String,
    /// An optional leading icon.
    pub icon: Option<IconName>,
    /// Whether the row can be chosen.
    pub enabled: bool,
}

impl PopupItem {
    /// A plain, enabled row.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), icon: None, enabled: true }
    }

    /// The same row with a leading icon.
    pub fn with_icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The same row, disabled.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// A dismissible list of items positioned at a point.
pub struct PopupItemList {
    base: BaseWidget,
    items: Vec<PopupItem>,
    anchor: PopupAnchor,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    visible: bool,
    /// Emitted with the chosen row's index.
    pub item_activated: Signal1<usize>,
    /// Emitted when the panel closes for any reason.
    pub dismissed: GenericSignal,
}

impl PopupItemList {
    /// Creates a hidden list.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupItemList"),
            items: Vec::new(),
            anchor: PopupAnchor::Below,
            selected_index: None,
            hovered_index: None,
            visible: false,
            item_activated: Signal1::new(),
            dismissed: GenericSignal::new(),
        }
    }

    /// Replaces the rows.
    pub fn set_items(&mut self, items: Vec<PopupItem>) {
        if self.items == items {
            return;
        }
        self.items = items;
        self.hovered_index = self.hovered_index.filter(|index| *index < self.items.len());
        self.selected_index = self.selected_index.filter(|index| *index < self.items.len());
        self.resize_to_items();
        self.base.request_redraw();
    }

    /// The rows.
    pub fn items(&self) -> &[PopupItem] {
        &self.items
    }

    /// Sets which side of the anchor the panel grows toward.
    pub fn set_anchor(&mut self, anchor: PopupAnchor) {
        self.anchor = anchor;
    }

    /// The side of the anchor the panel grows toward.
    pub fn anchor(&self) -> PopupAnchor {
        self.anchor
    }

    /// Whether the panel is open.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Opens the panel at `point`, anchored according to [`Self::anchor`].
    pub fn popup_at(&mut self, point: Point) {
        self.resize_to_items();
        let rect = self.geometry();
        let origin = match self.anchor {
            PopupAnchor::Below => Point::new(point.x, point.y),
            PopupAnchor::Above => Point::new(point.x, point.y - rect.height as i32),
            PopupAnchor::Right => Point::new(point.x, point.y),
            PopupAnchor::Left => Point::new(point.x - rect.width as i32, point.y),
        };
        self.base.set_geometry(Rect::new(origin.x, origin.y, rect.width, rect.height));
        self.visible = true;
        self.hovered_index = None;
        self.base.request_redraw();
    }

    /// Closes the panel, emitting `dismissed`.
    pub fn dismiss(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.hovered_index = None;
        self.base.request_redraw();
        self.dismissed.emit();
    }

    /// The selected row, when one was chosen.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sizes the panel to its rows.
    fn resize_to_items(&mut self) {
        let width = self.base.geometry().width.max(160);
        let height = (self.items.len() as u32 * POPUP_ROW_HEIGHT + 8).max(POPUP_ROW_HEIGHT + 8);
        self.base.set_geometry(Rect::new(
            self.base.geometry().x,
            self.base.geometry().y,
            width,
            height,
        ));
    }

    /// The row at `index`.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x + 4,
            rect.y + 4 + index as i32 * POPUP_ROW_HEIGHT as i32,
            rect.width.saturating_sub(8),
            POPUP_ROW_HEIGHT,
        ))
    }

    /// The row under `pos`.
    fn row_at(&self, pos: Point) -> Option<usize> {
        (0..self.items.len())
            .find(|index| self.row_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }
}

/// The height of one popup row.
const POPUP_ROW_HEIGHT: u32 = 26;

impl Widget for PopupItemList {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(180, POPUP_ROW_HEIGHT * 3 + 8)
    }
    impl_draw_bridge!();
}

impl Draw for PopupItemList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(250, 250, 250, 255));
        let accent = ink;
        let font = &role_font(style, 12.0);
        let opacity = if self.visible { 255 } else { 90 };
        context.fill_rounded_rect(rect, 6, surface.with_alpha(opacity));
        context.draw_rounded_rect_stroke(rect, 6, ink.with_alpha(60), 1);
        if !self.visible {
            return;
        }
        for index in 0..self.items.len() {
            let Some(row) = self.row_rect(index) else {
                break;
            };
            let item = &self.items[index];
            if self.hovered_index == Some(index) && item.enabled {
                context.fill_rounded_rect(row, 4, accent.with_alpha(24));
            }
            let row_ink = if item.enabled { ink } else { ink.with_alpha(90) };
            let mut text_x = row.x + 8;
            if let Some(icon) = item.icon {
                let icon_rect = Rect::new(row.x + 6, row.y + (row.height as i32 - 14) / 2, 14, 14);
                crate::widget::draw_icon_at(context, icon_rect, row_ink, icon);
                text_x += 18;
            }
            context.draw_text_fitted_cow(
                Rect::new(
                    text_x,
                    row.y + 4,
                    row.right().saturating_sub(text_x) as u32,
                    row.height.saturating_sub(8),
                ),
                &item.label,
                font,
                row_ink,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for PopupItemList {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let hovered = self.row_at(*pos);
                if hovered != self.hovered_index {
                    self.hovered_index = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if !self.geometry().contains_point(*pos) {
                    // A click outside is the standard way to dismiss a popup.
                    self.dismiss();
                    return;
                }
                if let Some(index) = self.row_at(*pos) {
                    if self.items[index].enabled {
                        self.selected_index = Some(index);
                        self.item_activated.emit(index);
                        self.dismiss();
                    }
                }
            }
            Event::KeyPress { key: 27, .. } => self.dismiss(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_popup_opens_on_its_rows_and_dismisses_without_a_selection() {
        let mut list = PopupItemList::new(Rect::new(0, 0, 160, 0));
        list.set_items(crate::compat::Vec::from([
            PopupItem::new("Open"),
            PopupItem::new("Close").disabled(),
        ]));
        assert_eq!(list.items().len(), 2);
        assert!(!list.items()[1].enabled);
        list.popup_at(Point::new(10, 20));
        assert!(list.is_visible());
        list.dismiss();
        assert!(!list.is_visible());
        list.dismiss();
        assert!(list.selected_index().is_none());
    }
}
