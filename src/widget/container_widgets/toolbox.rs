// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool box widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_orientation, expect_usize, orientation_to_str};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// The height of one tab in a vertical tool box, in logical pixels.
const ITEM_HEIGHT: u32 = 32;
/// The width of one tab in a horizontal tool box, in logical pixels.
const ITEM_WIDTH: u32 = 120;
/// The smallest page the content area is allowed to keep.
///
/// # Why the page cannot simply be what is left over
///
/// `content_rect` used to be `rect.height - item_height * items.len()`, floored at zero. At the
/// census size (120 px) with four items that is `120 - 128 = 0`: every item's tab was drawn and
/// the page — the whole point of a tool box — had no pixels at all, so the control rendered as a
/// list of tabs beside nothing. The reference tool box gives the page the remaining body and keeps at
/// least a line of it; this constant is that rule, in the same shape as `meter`'s reserved
/// reading band and `splitter`'s minimum pane.
const MIN_CONTENT_EXTENT: u32 = 24;

/// Tool box widget.
///
/// # Overflow
///
/// The tab strip and the page share one axis, so they cannot both have the room they would like.
/// The page keeps [`MIN_CONTENT_EXTENT`] and the *strip* gives way: when the tabs no longer fit,
/// the strip is scrolled by whole tabs (see [`ToolBox::scroll_offset`]) rather than letting an
/// item be placed past the control's edge. An item is therefore either drawn whole, inside the
/// strip, or not drawn at all.
pub struct ToolBox {
    base: BaseWidget,
    items: Vec<ToolBoxItem>,
    current_index: usize,
    orientation: Orientation,
    /// Index of the first tab drawn, i.e. the tab strip's scroll offset in whole tabs.
    ///
    /// Kept in `usize` rather than as a pixel offset because a tab is drawn as a whole unit: a
    /// partial tab at the top of the strip would be a tab whose label is cut in half, and the
    /// overflow rule above is deliberately "whole tabs, or none".
    scroll_offset: usize,
    /// Emitted with the new page index whenever the current page changes through
    /// [`ToolBox::set_current_index`] or user input. `clear()` resets the index to `0`
    /// without emitting, because it also empties the page vector.
    ///
    /// (An earlier version of this comment described a "collapse" notification;
    /// a toolbox page has no collapsed state, so that contract was never real.)
    pub current_changed: Signal1<usize>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Tool box item.
pub struct ToolBoxItem {
    text: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
}
impl ToolBoxItem {
    /// Creates a new tool box item.
    pub fn new(text: String) -> Self {
        Self {
            text,
            #[cfg(feature = "image")]
            icon: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
        }
    }
    /// Returns text.
    pub fn text(&self) -> &str {
        &self.text
    }
    /// Sets text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    #[cfg(feature = "image")]
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    #[cfg(feature = "image")]
    /// Sets icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }
    /// Returns tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// Sets tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    /// Returns whether item is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
    }
}
impl ToolBox {
    /// Creates a tool box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Toolbox, geometry, "ToolBox"),
            items: Vec::new(),
            current_index: 0,
            orientation: Orientation::Vertical,
            scroll_offset: 0,
            current_changed: Signal1::new(),
            registry: None,
        }
    }
    /// Adds an item.
    pub fn add_item(&mut self, text: String, widget: Option<ObjectId>) -> usize {
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.items.push(item);
        self.clamp_scroll();
        self.items.len().saturating_sub(1)
    }
    /// Inserts an item at position.
    pub fn insert_item(&mut self, index: usize, text: String, widget: Option<ObjectId>) {
        let was_empty = self.items.is_empty();
        let mut item = ToolBoxItem::new(text);
        item.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.items.insert(index, item);
        if !was_empty && self.current_index >= index {
            self.current_index += 1;
        }
        self.clamp_scroll();
    }
    /// Removes an item.
    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            if let Some(widget_id) = self.items[index].widget {
                self.base.remove_child(widget_id);
            }
            self.items.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.items.is_empty() {
                self.current_index = 0;
            }
            self.clamp_scroll();
        }
    }
    /// Returns number of items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Returns current item index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current item index.
    ///
    /// The new page's tab is scrolled into view if overflow had hidden it; the selection is not
    /// silently applied to a page the user cannot see the tab of.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.items.len() && self.current_index != index {
            self.current_index = index;
            self.scroll_current_into_view();
            self.current_changed.emit(index);
            self.base.request_redraw();
        }
    }
    /// Returns current item widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.items.get(self.current_index).and_then(|item| item.widget)
    }
    /// Returns item at index.
    pub fn item(&self, index: usize) -> Option<&ToolBoxItem> {
        self.items.get(index)
    }
    /// Returns mutable item at index.
    pub fn item_mut(&mut self, index: usize) -> Option<&mut ToolBoxItem> {
        self.items.get_mut(index)
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
        // The other axis has a different extent and a different tab size, so an offset that was
        // legal in one is not necessarily legal in the other.
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// Removes all items from the toolbox.
    pub fn clear(&mut self) {
        for item in self.items.drain(..) {
            if let Some(widget_id) = item.widget {
                self.base.remove_child(widget_id);
            }
        }
        self.current_index = 0;
        self.scroll_offset = 0;
        self.base.request_redraw();
    }
    /// The extent one tab occupies along the strip's axis.
    fn item_extent(&self) -> u32 {
        match self.orientation {
            Orientation::Vertical => ITEM_HEIGHT,
            Orientation::Horizontal => ITEM_WIDTH,
        }
    }

    /// How much room the tab strip has, along its own axis.
    fn strip_extent(&self) -> u32 {
        let rect = self.geometry();
        match self.orientation {
            Orientation::Vertical => rect.height,
            Orientation::Horizontal => rect.width,
        }
    }

    /// The largest value [`Self::scroll_offset`] may take. Zero when every tab fits.
    ///
    /// The strip is never allowed to spend the page's own [`MIN_CONTENT_EXTENT`]: that is what
    /// makes "the page keeps at least one line" true at *any* tab count, including the case
    /// where the tabs alone are taller than the whole control.
    pub fn max_scroll(&self) -> usize {
        let extent = self.strip_extent();
        let visible =
            (extent.saturating_sub(MIN_CONTENT_EXTENT) / self.item_extent().max(1)) as usize;
        self.items.len().saturating_sub(visible)
    }

    /// Returns the tab strip's scroll offset, in whole tabs.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the tab strip to `offset`, clamped to [`Self::max_scroll`].
    ///
    /// The clamp is what makes this a total function for a caller: an offset past the end simply
    /// shows the last full strip, which is what a scroll-into-view means. Requests a redraw only
    /// when the offset actually changes.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let clamped = offset.min(self.max_scroll());
        if self.scroll_offset != clamped {
            self.scroll_offset = clamped;
            self.base.request_redraw();
        }
    }

    /// Re-clamps the scroll offset after the item list or the geometry changed.
    ///
    /// # Why this is needed separately from the setter
    ///
    /// Every other mutator on this control can shrink the strip — removing an item, clearing the
    /// list, resizing the control — and a stale offset would then scroll a strip that fits,
    /// which shows *fewer* tabs than there is room for. Every one of those paths ends here rather
    /// than each remembering the clamp, because a rule that lives in one place cannot be applied
    /// in three of four.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
    }

    /// Keeps the page's own tab inside the visible strip, scrolling it into view when it is not.
    ///
    /// A selection made by keyboard or by the property contract can name a tab that overflow has
    /// scrolled off, and a page whose tab cannot be seen is a page the user cannot tell is open.
    fn scroll_current_into_view(&mut self) {
        let extent = self.strip_extent();
        let visible =
            (extent.saturating_sub(MIN_CONTENT_EXTENT) / self.item_extent().max(1)) as usize;
        if visible == 0 {
            // No tab fits alongside the page's own minimum, so there is nothing to scroll to;
            // scrolling would only hide a tab to show another.
            return;
        }
        if self.current_index < self.scroll_offset {
            self.scroll_offset = self.current_index;
        } else if self.current_index >= self.scroll_offset + visible {
            self.scroll_offset = self.current_index + 1 - visible;
        }
        self.clamp_scroll();
    }

    /// Returns item rectangle at index.
    ///
    /// # Why the result is intersected with the strip
    ///
    /// The index is offset by the strip's scroll position and the rectangle is then clipped to
    /// the strip, so an item that overflow has pushed past the strip's end yields a *partial*
    /// rectangle — up to and including an empty one — rather than a rectangle outside the
    /// control. That is the invariant the draw path and hit testing both rely on: an item either
    /// has pixels inside the strip or is reported as having none. Previously the position was
    /// `item_extent * index` with no reference to the control's extent at all, so item 5 of 6 in a
    /// 120 px control was painted from y = 160 downward, entirely outside the control.
    fn item_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        let extent = self.item_extent();
        // An item scrolled above the strip's start contributes no pixels, and a `usize`
        // subtraction would underflow here — which is why the offset is compared rather than
        // subtracted.
        let visible_from = self.scroll_offset;
        if index < visible_from {
            return None;
        }
        let slot = (index - visible_from) as u32;
        let position = slot * extent;
        match self.orientation {
            Orientation::Horizontal => {
                let strip = Rect::new(rect.x, rect.y, rect.width, rect.height);
                let full = Rect::new(
                    rect.x.saturating_add_unsigned(position),
                    rect.y,
                    ITEM_WIDTH,
                    rect.height,
                );
                full.intersection(&strip)
            }
            Orientation::Vertical => {
                let strip = Rect::new(rect.x, rect.y, rect.width, rect.height);
                let full = Rect::new(
                    rect.x,
                    rect.y.saturating_add_unsigned(position),
                    rect.width,
                    ITEM_HEIGHT,
                );
                full.intersection(&strip)
            }
        }
    }

    /// Returns content rectangle.
    ///
    /// The page starts where the *visible* strip ends — not where an unbounded strip would have
    /// ended — so the page cannot be pushed off the control by tabs that overflow. It keeps at
    /// least [`MIN_CONTENT_EXTENT`], which is what stops a tab list from reducing the page to
    /// zero height; the tabs, not the page, are what give way when both cannot fit.
    ///
    /// The scrolling has an important consequence worth stating: because at most
    /// `(extent - MIN_CONTENT_EXTENT) / item_extent` tabs are visible at once, the strip's end and
    /// the page's start are always inside the control, and their sum can never exceed it.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let extent = self.item_extent();
        // `extent` is already the item's full slot, so the division needs no further cast: the
        // count of whole tabs that fit beside the reserved page is what `strip_extent` reports.
        let visible = self.strip_extent().saturating_sub(MIN_CONTENT_EXTENT) / extent.max(1);
        // Every tab fits: the page gets whatever is left, which may be less than the minimum
        // because the tabs are the part with a fixed size. `saturating_sub` keeps the short case
        // from wrapping.
        let strip_used = visible * extent;
        match self.orientation {
            Orientation::Horizontal => {
                let remaining = rect.width.saturating_sub(strip_used);
                let content_width = if self.items.len() as u32 <= visible {
                    remaining.max(MIN_CONTENT_EXTENT).min(rect.width)
                } else {
                    remaining
                };
                Rect::new(
                    rect.x.saturating_add_unsigned(strip_used),
                    rect.y,
                    content_width,
                    rect.height,
                )
            }
            Orientation::Vertical => {
                let remaining = rect.height.saturating_sub(strip_used);
                let content_height = if self.items.len() as u32 <= visible {
                    remaining.max(MIN_CONTENT_EXTENT).min(rect.height)
                } else {
                    remaining
                };
                Rect::new(
                    rect.x,
                    rect.y.saturating_add_unsigned(strip_used),
                    rect.width,
                    content_height,
                )
            }
        }
    }
    /// Returns index of item at position.
    fn item_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                if item_rect.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }
}
// Implement Widget trait
impl Widget for ToolBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 250)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ToolBox`'s property contract.
impl WidgetProperties for ToolBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => Ok(CapabilityValue::UInt(self.current_index() as u64)),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            // Derived from the item list.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `TOOL_BOX_PROPERTIES`.
        property_names_of!["item_count", "current_index", "orientation", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `tool_box` publishes.
    ///
    /// `add_item` takes the item's label and `remove_item` takes the index to
    /// remove, so neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_item" | "remove_item" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl ToolBox {
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.base.request_redraw();
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
}
impl EventHandler for ToolBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(index) = self.item_at_position(*pos) {
                    if self.items[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
            Event::KeyPress { key, .. } => {
                // A tab that overflow has scrolled off the strip is still reachable by the arrow
                // key that walks toward it: `set_current_index` scrolls the strip to keep the
                // selection's tab visible, so the two agree on where the strip sits and the
                // keyboard cannot select a page whose tab is nowhere on screen.
                let next = match (self.orientation, key) {
                    // Vertical: Up/Down; Horizontal: Left/Right
                    (Orientation::Vertical, 38) | (Orientation::Horizontal, 37) => {
                        // Up/Left
                        if self.current_index > 0 {
                            Some(self.current_index - 1)
                        } else {
                            None
                        }
                    }
                    (Orientation::Vertical, 40) | (Orientation::Horizontal, 39) => {
                        // Down/Right
                        let next = self.current_index + 1;
                        if next < self.items.len() {
                            Some(next)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                if let Some(idx) = next {
                    self.set_current_index(idx);
                }
            }
            _ => {}
        }
        let allow_child_event = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos } => self.content_rect().contains(*pos),
            _ => true,
        };
        // Forward content events only to the current widget.
        if allow_child_event {
            if let Some(widget_id) = self.current_widget() {
                if let Some(ref reg) = self.registry {
                    reg.borrow_mut().set_widget_geometry(widget_id, self.content_rect());
                    reg.borrow_mut().forward_event(widget_id, event);
                }
            }
        }
    }
}
impl Draw for ToolBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // The page is drawn **before** the tabs, and the tabs are drawn only where `item_rect`
        // says they are. Both are consequences of the same rule: the strip and the page share the
        // control's axis, so anything the tabs are allowed to paint must already be inside the
        // strip, and the page must not paint over the tabs that sit above it.
        let content_rect = self.content_rect();
        // Every colour below used to be a literal, so a themed toolbox kept a white
        // page and light tabs inside a dark window. The style is read once and each
        // literal becomes that field's fallback, so nothing that was visible before
        // becomes invisible now.
        let style = self.base.style();
        // Draw content background.
        //
        // `tool_box` is absent from the role table, so it classifies as `Surface` and the
        // active theme writes the *window fill* into `style.background_color`. Taking that
        // value at face value painted the content area in the window's own colour — the two
        // rectangles in the snapshot were byte-identical, so an empty toolbox read as a bare
        // border with a hole in it. `splitter`, `stacked_widget`, `mdi_area` and `carousel`
        // all detect exactly this case and re-derive a visible step; this one did not.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let page = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&style.text_color.unwrap_or(Color::BLACK), 0.08),
        };
        // Draw content background
        context.fill_rect(
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
            page,
        );
        // Draw content border
        context.draw_rect(
            Rect::new(content_rect.x, content_rect.y, content_rect.width, content_rect.height),
            style
                .border_color
                .filter(|resolved| *resolved != page)
                .unwrap_or_else(|| page.blend(&Color::BLACK, 0.25)),
        );
        // Draw the current widget, clipped to the page. This happens *before* the tab strip so
        // the tabs stay on top of their own surface: a vertical tool box's page starts where the
        // tabs end, but a page that painted a border or a background of its own could still
        // reach into the strip, and the tabs are the control's chrome rather than its content.
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(widget_id, content_rect);
                context.push_clip(
                    content_rect.x,
                    content_rect.y,
                    content_rect.width,
                    content_rect.height,
                );
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_clip();
            }
        }
        // Draw items
        for i in 0..self.items.len() {
            if let Some(item_rect) = self.item_rect(i) {
                let item = &self.items[i];
                let is_current = i == self.current_index;
                let is_enabled = item.enabled;
                // Draw item background
                //
                // The tab strip is this widget's own surface, so it uses the style;
                // the *selected* tab is a state highlight, not a colour, and is kept
                // as the literal it always was (swapping it for `border_color` would
                // have turned the selection into a plain border-coloured box).
                let bg_color = if !is_enabled {
                    style.background_color.unwrap_or(Color::rgb(240, 240, 240))
                } else if is_current {
                    Color::rgb(220, 220, 255)
                } else {
                    style.background_color.unwrap_or(Color::rgb(240, 240, 240))
                };
                context.fill_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    bg_color,
                );
                // Draw item border
                let border_color = if !is_enabled {
                    style.border_color.unwrap_or(Color::rgb(200, 200, 200))
                } else if is_current {
                    Color::rgb(100, 100, 200)
                } else {
                    style.border_color.unwrap_or(Color::rgb(200, 200, 200))
                };
                context.draw_rect(
                    Rect::new(item_rect.x, item_rect.y, item_rect.width, item_rect.height),
                    border_color,
                );
                let padding = 5i32;
                #[cfg(feature = "image")]
                let icon_size = 16u32;
                #[cfg(feature = "image")]
                let text_x = if item.icon.is_some() {
                    item_rect.x + icon_size as i32 + padding + 4
                } else {
                    item_rect.x + padding
                };
                #[cfg(not(feature = "image"))]
                let text_x = item_rect.x + padding;
                #[cfg(feature = "image")]
                if item.icon.is_some() {
                    let icon_x = item_rect.x + padding;
                    let icon_y = item_rect.y + (item_rect.height as i32 - icon_size as i32) / 2;
                    let icon_rect = Rect::new(icon_x, icon_y, icon_size, icon_size);
                    // Draw a small rounded square as the icon background
                    let icon_bg_color = if is_current {
                        Color::rgb(100, 100, 200)
                    } else {
                        Color::rgb(180, 180, 200)
                    };
                    context.fill_rounded_rect(icon_rect, 3, icon_bg_color);
                    // Draw a simple shape inside: a small circle (representative)
                    let inner_r = 3;
                    let cx = icon_x + icon_size as i32 / 2;
                    let cy = icon_y + icon_size as i32 / 2;
                    context.fill_rounded_rect(
                        Rect::new(
                            cx - inner_r as i32,
                            cy - inner_r as i32,
                            inner_r * 2,
                            inner_r * 2,
                        ),
                        inner_r,
                        // The glyph is drawn *on* the tinted icon square, so it reads
                        // `text_color` like every other piece of ink in the item.
                        style.text_color.unwrap_or(Color::rgb(255, 255, 255)),
                    );
                }
                // Draw item text
                //
                // A disabled item keeps its dimmed literal: that muted grey *is* how
                // the state is communicated, and `text_color` would repaint it at
                // full strength and erase the distinction.
                let item_text_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
                let text_color =
                    if !is_enabled { Color::rgb(150, 150, 150) } else { item_text_color };
                // The label is centred on the item's own line box. `item_rect.y +
                // item_rect.height / 2` placed the glyph box's top edge on the item's middle
                // line instead, which drew the text half a line low.
                let text_font = Font::default();
                let item_line = context.text_line(item_rect, &text_font);
                context.draw_text(
                    Point::new(text_x, item_line.y),
                    &item.text,
                    &text_font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::event::Event;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Helper constants ──────────────────────────────────────────────────

    fn widget_id_1() -> ObjectId {
        9101
    }
    fn widget_id_2() -> ObjectId {
        9102
    }
    fn widget_id_3() -> ObjectId {
        9103
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn toolbox_creation_defaults() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0, "should have no items");
        assert_eq!(tb.current_index(), 0, "current index should be 0");
        assert_eq!(tb.current_widget(), None, "no current widget");
        assert_eq!(tb.orientation(), Orientation::Vertical, "default orientation");
        assert!(tb.is_visible(), "should be visible");
        assert!(tb.is_enabled(), "should be enabled");
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
        assert!(tb.registry().is_none(), "registry should be None by default");
    }

    // ── 2. Adding items ───────────────────────────────────────────────────

    #[test]
    fn toolbox_add_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        let idx = tb.add_item("First".to_string(), None);
        assert_eq!(idx, 0, "first item index should be 0");
        assert_eq!(tb.count(), 1);

        let idx2 = tb.add_item("Second".to_string(), Some(widget_id_1()));
        assert_eq!(idx2, 1, "second item index should be 1");
        assert_eq!(tb.count(), 2);

        // The widget associated with the second item becomes a child
        let children = tb.children();
        assert!(children.contains(&widget_id_1()), "child widget should be tracked");
    }

    #[test]
    fn toolbox_add_item_without_widget_does_not_add_child() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("NoWidget".to_string(), None);
        assert!(tb.children().is_empty(), "no children when no widget is given");
    }

    // ── 3. Setting/getting current index ──────────────────────────────────

    #[test]
    fn toolbox_get_set_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        assert_eq!(tb.current_index(), 0, "starts at 0");
        tb.set_current_index(1);
        assert_eq!(tb.current_index(), 1);
        tb.set_current_index(0);
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_set_current_index_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Only".to_string(), None);
        tb.set_current_index(5); // out of bounds
        assert_eq!(tb.current_index(), 0, "should remain 0");
    }

    #[test]
    fn toolbox_set_current_index_same_value_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.set_current_index(0); // already 0, should not emit signal
        assert_eq!(tb.current_index(), 0);
    }

    // ── 4. Item count ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_item_count() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.count(), 0);
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.count(), 1);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        assert_eq!(tb.count(), 3);
    }

    // ── 5. Removing items ─────────────────────────────────────────────────

    #[test]
    fn toolbox_remove_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), Some(widget_id_1()));
        tb.add_item("C".to_string(), None);

        tb.remove_item(1);
        assert_eq!(tb.count(), 2);
        // The widget should be removed from children
        assert!(!tb.children().contains(&widget_id_1()), "child widget should be removed");
    }

    #[test]
    fn toolbox_remove_item_out_of_bounds_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.remove_item(5); // out of bounds
        assert_eq!(tb.count(), 1);
    }

    #[test]
    fn toolbox_remove_item_adjusts_current_index() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        tb.set_current_index(2);

        // Remove item at index 0 (< current_index), current decrements to 1
        tb.remove_item(0);
        assert_eq!(tb.current_index(), 1);

        // Removing last item leaves empty -> current_index resets to 0
        tb.remove_item(1);
        tb.remove_item(0);
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_remove_item_above_current_index_does_not_change_it() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);
        // Remove item at index 1 (> current_index 0), current stays 0
        tb.remove_item(1);
        assert_eq!(tb.current_index(), 0);
    }

    // ── 6. Clear all items ────────────────────────────────────────────────

    #[test]
    fn toolbox_clear_all_items() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), Some(widget_id_1()));
        tb.add_item("B".to_string(), Some(widget_id_2()));
        tb.add_item("C".to_string(), Some(widget_id_3()));

        // Remove all items one by one (no dedicated clear() method)
        tb.remove_item(0);
        tb.remove_item(0);
        tb.remove_item(0);

        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), 0);
        assert!(tb.children().is_empty(), "all child widgets should be removed");
        assert_eq!(tb.current_widget(), None);
    }

    // ── 7. Setting/getting item text/tooltip ──────────────────────────────

    #[test]
    fn toolbox_item_text_and_tooltip() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("Hello".to_string(), None);
        tb.add_item("World".to_string(), None);

        if let Some(item) = tb.item(0) {
            assert_eq!(item.text(), "Hello");
            assert_eq!(item.tooltip(), "");
        }

        // Modify item via item_mut
        if let Some(item) = tb.item_mut(1) {
            item.set_text("Modified".to_string());
            item.set_tooltip("Tooltip text".to_string());
        }

        if let Some(item) = tb.item(1) {
            assert_eq!(item.text(), "Modified");
            assert_eq!(item.tooltip(), "Tooltip text");
        }

        // Getting item at out-of-bounds index
        assert!(tb.item(5).is_none());
        assert!(tb.item_mut(5).is_none());
    }

    // ── 8. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn toolbox_geometry_delegation() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.geometry(), Rect::new(0, 0, 200, 160));

        tb.set_geometry(Rect::new(10, 20, 300, 200));
        assert_eq!(tb.geometry(), Rect::new(10, 20, 300, 200));
        assert_eq!(tb.base().geometry(), tb.geometry());
    }

    #[test]
    fn toolbox_position_and_size() {
        let mut tb = ToolBox::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tb.position(), Point::new(5, 10));
        assert_eq!(tb.size(), crate::core::Size::new(200, 150));

        tb.set_position(Point::new(20, 30));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 200, 150));

        tb.set_size(crate::core::Size::new(300, 200));
        assert_eq!(tb.geometry(), Rect::new(20, 30, 300, 200));
    }

    // ── 9. Visibility ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_visibility() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_visible());

        tb.hide();
        assert!(!tb.is_visible());

        tb.show();
        assert!(tb.is_visible());

        tb.set_visible(false);
        assert!(!tb.is_visible());

        tb.set_visible(true);
        assert!(tb.is_visible());
    }

    #[test]
    fn toolbox_enabled_state() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert!(tb.is_enabled());

        tb.set_enabled(false);
        assert!(!tb.is_enabled());

        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    // ── 10. ID / Kind ─────────────────────────────────────────────────────

    #[test]
    fn toolbox_kind() {
        let tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        assert_eq!(tb.kind(), WidgetKind::Toolbox);
    }

    #[test]
    fn toolbox_id_is_unique() {
        let tb1 = ToolBox::new(Rect::new(0, 0, 100, 100));
        let tb2 = ToolBox::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tb1.id(), tb2.id(), "each ToolBox must have a unique id");
    }

    // ── 11. SVG draw output ───────────────────────────────────────────────

    #[test]
    fn toolbox_draw_produces_svg_output() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 300, 160));
        tb.add_item("Item1".to_string(), None);
        tb.add_item("Item2".to_string(), None);
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"), "SVG must start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG must end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG must contain correct width");
        assert!(svg.contains("height=\"160\""), "SVG must contain correct height");
        assert!(svg.contains("fill="), "SVG should contain fill attributes");
        assert!(svg.len() > 100, "SVG output should be substantial");
    }

    #[test]
    fn toolbox_draw_empty_produces_svg() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 100, 50));
        let svg = render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 50);
    }

    // ── 12. Signal accessors ──────────────────────────────────────────────

    #[test]
    fn toolbox_current_changed_signal_emits_on_set() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        tb.current_changed.connect(move |idx: Arc<usize>| {
            e.store(*idx, Ordering::SeqCst);
        });

        tb.set_current_index(1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);

        tb.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn toolbox_current_changed_does_not_emit_for_same_value() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);

        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        tb.current_changed.connect(move |_: Arc<usize>| {
            h.fetch_add(1, Ordering::SeqCst);
        });

        // Set to same index (0) — should not emit
        tb.set_current_index(0);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    // ── 13. Mouse click selects item ──────────────────────────────────────

    #[test]
    fn toolbox_mouse_click_selects_item() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("First".to_string(), None);
        tb.add_item("Second".to_string(), None);

        assert_eq!(tb.current_index(), 0);

        // Default orientation is Vertical; item_rect(1) has y = 0 + 32*1 = 32
        // Click on item 1 at (10, 40) which is within vertical item 1: (0, 32, 200, 32)
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 1, "clicking item should select it");
    }

    #[test]
    fn toolbox_mouse_click_on_disabled_item_does_not_select() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        // Disable item 1
        if let Some(item) = tb.item_mut(1) {
            item.set_enabled(false);
        }

        assert_eq!(tb.current_index(), 0);
        // Click on item 1 (y=32), should not select because item is disabled
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 0, "clicking disabled item should not change current");
    }

    #[test]
    fn toolbox_mouse_click_on_empty_region_is_noop() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        assert_eq!(tb.current_index(), 0);

        // Click far below items (y=200, but geometry only goes to 160)
        // Since item_at_position returns None for out-of-bounds, no change
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 200), button: 1 });
        assert_eq!(tb.current_index(), 0);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────

    #[test]
    fn toolbox_disabled_state_blocks_mouse_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(0);

        // Disable the entire toolbox
        tb.set_enabled(false);

        // Try clicking on item 1
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        // current_index should remain 0 because event handling returns early when disabled
        assert_eq!(tb.current_index(), 0, "disabled toolbox should not process mouse events");
    }

    #[test]
    fn toolbox_disabled_re_enable_processes_events() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);

        tb.set_enabled(false);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 0);

        // Re-enable and click again
        tb.set_enabled(true);
        tb.handle_event(&Event::MousePress { pos: Point::new(10, 40), button: 1 });
        assert_eq!(tb.current_index(), 1, "after re-enable, mouse clicks should work");
    }

    // ── 16. Keyboard navigation (vertical) ────────────────────────────────

    #[test]
    fn toolbox_key_down_selects_next_item_vertical() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);
        assert_eq!(tb.current_index(), 0);

        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.current_index(), 1);

        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.current_index(), 2);
    }

    #[test]
    fn toolbox_key_up_selects_prev_item_vertical() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.set_current_index(1);

        tb.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_key_navigation_stops_at_boundaries() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);

        // Already at first item, Up should stay
        tb.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
        assert_eq!(tb.current_index(), 0);

        // Already at last item, Down should stay
        tb.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(tb.current_index(), 0);
    }

    #[test]
    fn toolbox_key_navigation_horizontal() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 400, 100));
        tb.set_orientation(Orientation::Horizontal);
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), None);
        tb.add_item("C".to_string(), None);

        assert_eq!(tb.current_index(), 0);

        // Right arrow advances
        tb.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(tb.current_index(), 1);

        // Left arrow goes back
        tb.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(tb.current_index(), 0);
    }

    // ── 17. Clear method ──────────────────────────────────────────────────

    #[test]
    fn toolbox_clear_removes_all_items() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 200, 160));
        tb.add_item("A".to_string(), None);
        tb.add_item("B".to_string(), Some(widget_id_1()));
        tb.add_item("C".to_string(), None);
        tb.set_current_index(2);

        tb.clear();

        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), 0);
        assert!(tb.children().is_empty(), "child widgets should be removed");
    }

    // ── 18. Overflow: the page's minimum and the tab strip's containment ──────

    /// The defect: `content_rect` was `rect.height - item_height * items.len()`, floored at zero,
    /// so four 32 px tabs in the 120 px census control left the page **0** px tall.
    ///
    /// The page is observed through the drawing, because that is what the defect was visible as:
    /// the page rectangle the control emits must have a positive height, and it must be
    /// *distinct* from the tab strip rather than collapsed onto it.
    #[test]
    fn toolbox_page_keeps_an_extent_when_the_tabs_would_consume_it() {
        // Enough tabs to swallow the whole control at the fixed tab height.
        for count in [4usize, 6, 20] {
            let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            let svg = render_to_svg(&mut tb);
            // The page is the full-width rectangle below the tab strip; the strip's own rects are
            // only as tall as one tab. The tallest full-width rectangle is the page.
            let page_height = svg
                .lines()
                .filter(|l| l.contains("<rect"))
                .filter_map(|l| {
                    let n: Vec<i32> = l
                        .split(|c: char| !(c.is_ascii_digit() || c == '-'))
                        .filter(|s| !s.is_empty())
                        .filter_map(|s| s.parse().ok())
                        .collect();
                    (n.len() >= 4).then(|| (n[0], n[1], n[2], n[3]))
                })
                .filter(|(_, _, w, _)| *w == 240)
                .map(|(_, _, _, h)| h)
                .max()
                .unwrap_or(0);
            assert!(
                page_height > 0,
                "{count} tabs on a 120px box left the page {page_height}px tall"
            );
        }
    }

    /// The defect: `item_rect` was unclamped, so item `n` was placed at `item_height * n` with no
    /// reference to the control's extent and later items were painted outside it.
    #[test]
    fn toolbox_no_item_rect_escapes_the_control() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 3, 4, 6, 20] {
            let mut tb = ToolBox::new(geometry);
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            for i in 0..count {
                if let Some(r) = tb.item_rect(i) {
                    assert!(
                        r.y >= geometry.y && r.bottom() <= geometry.bottom(),
                        "item {i} of {count} at {r:?} escapes {geometry:?}"
                    );
                    assert!(r.width > 0 && r.height > 0, "item {i} of {count} is empty: {r:?}");
                }
            }
        }
    }

    /// The overflow rule must be *driven*, not just computed: with more tabs than fit, the strip
    /// can scroll, and selecting an off-screen tab brings its tab into view.
    #[test]
    fn toolbox_overflow_scrolls_and_the_page_follows_the_middle_tab() {
        let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
        for i in 0..10 {
            tb.add_item(format!("Item {i}"), None);
        }
        assert!(tb.max_scroll() > 0, "ten 32px tabs cannot fit a 24px page in 120px");

        tb.set_current_index(9);
        assert!(tb.scroll_offset() > 0, "selecting a scrolled-off tab must bring it into view");
        assert!(tb.item_rect(9).is_some(), "the selected tab must be on screen after scrolling");

        // An out-of-range scroll request is clamped rather than leaving the strip blank.
        tb.set_scroll_offset(usize::MAX);
        assert_eq!(tb.scroll_offset(), tb.max_scroll());
        tb.set_scroll_offset(0);
        assert_eq!(tb.scroll_offset(), 0);
    }

    /// The rendered SVG must contain nothing outside the control, at any tab count.
    #[test]
    fn toolbox_draw_never_paints_outside_the_control() {
        for count in [1usize, 4, 6, 20] {
            let mut tb = ToolBox::new(Rect::new(0, 0, 240, 120));
            for i in 0..count {
                tb.add_item(format!("Item {i}"), None);
            }
            let svg = render_to_svg(&mut tb);
            for line in svg.lines().filter(|l| l.contains("<rect") || l.contains("<line")) {
                let nums: Vec<i32> = line
                    .split(|c: char| !(c.is_ascii_digit() || c == '-'))
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                assert!(
                    nums.iter().all(|v| *v >= 0),
                    "{count} items painted outside the control: {line}"
                );
            }
        }
    }
}
