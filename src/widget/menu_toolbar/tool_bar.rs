// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tool bar widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::access::tool_bar_orientation_to_str;
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_toolbar_orientation};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Orientation of a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolBarOrientation {
    /// Items laid out left to right.
    Horizontal,
    /// Items laid out top to bottom.
    Vertical,
}
/// A button entry in the toolbar.
#[derive(Debug, Clone)]
pub struct ToolBarItem {
    id: String,
    text: String,
    tooltip: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
}
impl ToolBarItem {
    /// Creates an enabled, unchecked, non-separator item.
    ///
    /// `id` identifies the item in the `action_triggered` signal; `text` is what
    /// is painted on the button. The tooltip starts empty.
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
        }
    }
    /// Creates a separator: an item with an empty id and text and
    /// [`Self::is_separator`] true.
    ///
    /// Separators occupy a fixed gap, are never hit-testable as actions, and are
    /// skipped when a press is dispatched.
    pub fn separator() -> Self {
        let mut t = Self::new("", "");
        t.set_separator(true);
        t
    }

    // --- Accessors ---

    /// The item's identity, emitted through `action_triggered` when it is chosen.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Replaces the item's identity. Changing it changes what a later activation
    /// emits, so any handler matching on the old id stops firing.
    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }

    /// The label painted on the item.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the label.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// The item's hover tooltip, or `""` when none was set.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the hover tooltip. Note that this widget only stores it: nothing in
    /// `ToolBar`'s own drawing shows a tooltip.
    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = tooltip.into();
    }

    /// Whether the item toggles its checked state when activated.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Makes the item toggle on activation, or turns that off. Turning it off
    /// leaves `checked` as it is.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    /// Whether the item is currently checked. Only meaningful when
    /// [`Self::is_checkable`] is true — a non-checkable item keeps whatever
    /// value was set here but nothing will ever toggle it.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state directly, regardless of [`Self::is_checkable`].
    /// Use `ToolBar::set_item_checked`, which respects checkability, if the
    /// caller is not sure which kind of item it holds.
    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    /// Whether the item can be activated. A disabled item still draws (in grey)
    /// but ignores presses.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enables or disables the item.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether this item is a visual divider rather than an action.
    pub fn is_separator(&self) -> bool {
        self.separator
    }

    /// Makes this item a separator (`true`) or an ordinary action (`false`).
    ///
    /// A separator is drawn as a rule and occupies a fixed gap, but setting this
    /// does not clear the id: a press can never reach a separator through
    /// `hit_item`, so nothing is emitted either way.
    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
    }
}
/// Toolbar widget.
pub struct ToolBar {
    base: BaseWidget,
    orientation: ToolBarOrientation,
    icon_size: f32,
    movable: bool,
    floatable: bool,
    /// Whether the toolbar currently lives in its own top-level window. See
    /// [`ToolBar::is_top_level`] for why this is not a `BaseWidget` flag.
    top_level: bool,
    items: Vec<ToolBarItem>,
    hovered_index: Option<usize>,
    /// Emitted with the id of the item that was activated.
    ///
    /// Fires on primary-button press over an enabled, non-separator item. For a
    /// checkable item the checked flag has already been toggled by the time this
    /// is emitted.
    pub action_triggered: Signal1<String>,
    /// Emitted by [`ToolBar::set_orientation`] with `true` for horizontal and
    /// `false` for vertical. Not emitted when the orientation is unchanged.
    pub orientation_changed: Signal1<bool>,
    /// Emitted when the toolbar is docked or floated, by
    /// [`ToolBar::set_top_level`]. [`ToolBar::is_floatable`] separately records the
    /// permission to float.
    pub top_level_changed: Signal1<bool>,
    /// Emitted when the toolbar's visibility changes, by [`ToolBar::set_visible`].
    pub visibility_changed: Signal1<bool>,
}
impl ToolBar {
    /// Creates a horizontal toolbar with a default icon size of 24 pixels, and
    /// with both movable and floatable permitted.
    ///
    /// Starts with no items; add them with [`ToolBar::add_action`],
    /// [`ToolBar::add_separator`] or by pushing to [`ToolBar::items`].
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "ToolBar"),
            orientation: ToolBarOrientation::Horizontal,
            icon_size: 24.0,
            movable: true,
            floatable: true,
            top_level: false,
            items: Vec::new(),
            hovered_index: None,
            action_triggered: Signal1::new(),
            orientation_changed: Signal1::new(),
            top_level_changed: Signal1::new(),
            visibility_changed: Signal1::new(),
        }
    }
    /// The direction items are laid out in.
    pub fn orientation(&self) -> ToolBarOrientation {
        self.orientation
    }
    /// The icon size in **pixels**. It sets the button size, but the toolbar has
    /// no icons of its own — item labels are drawn at the default font size
    /// regardless — so in practice this controls the spacing between items.
    pub fn icon_size(&self) -> f32 {
        self.icon_size
    }
    /// Whether the toolbar is allowed to be dragged to another dock position.
    pub fn is_movable(&self) -> bool {
        self.movable
    }
    /// Whether the toolbar is allowed to be torn off into a floating window.
    ///
    /// This is the *permission*; the current docking state is
    /// [`ToolBar::is_top_level`], reached through [`ToolBar::set_top_level`].
    pub fn is_floatable(&self) -> bool {
        self.floatable
    }
    /// The current items, in draw order.
    pub fn items(&self) -> &[ToolBarItem] {
        &self.items
    }
    /// Sets the layout direction and repaints.
    ///
    /// Emits `orientation_changed` with `true` for horizontal, and only when the
    /// value actually changes.
    pub fn set_orientation(&mut self, o: ToolBarOrientation) {
        let changed = self.orientation != o;
        self.orientation = o;
        if changed {
            self.orientation_changed.emit(o == ToolBarOrientation::Horizontal);
        }
        self.base.request_redraw();
    }
    /// Sets the icon size in **pixels** and repaints.
    ///
    /// Values below 8 are raised to 8, which is the smallest size at which an
    /// item remains clickable rather than collapsing to a sliver.
    pub fn set_icon_size(&mut self, size: f32) {
        self.icon_size = size.max(8.0);
        self.base.request_redraw();
    }
    /// Allows or forbids dragging the toolbar; repaints either way.
    pub fn set_movable(&mut self, v: bool) {
        self.movable = v;
        self.base.request_redraw();
    }
    /// Allows or forbids tearing the toolbar off; repaints either way.
    ///
    /// Forbidding it also docks a currently-floating toolbar, so the permission and
    /// the state cannot disagree.
    pub fn set_floatable(&mut self, v: bool) {
        self.floatable = v;
        if !v {
            // Docking through the setter keeps `top_level_changed` truthful: the
            // state really did change, and a host tracking it must hear about it.
            self.set_top_level(false);
        }
        self.base.request_redraw();
    }
    /// Whether the toolbar is currently shown.
    pub fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    /// Shows or hides the toolbar and repaints.
    ///
    /// Emits `visibility_changed`, and only on a real change: a caller using the
    /// signal to relayout a surrounding window must not do that work for a write
    /// that changed nothing.
    ///
    /// This is the wrapper that makes the signal reachable. `BaseWidget` owns the
    /// flag and knows nothing about toolbar signals, so without this method
    /// `visibility_changed` could only be emitted from inside `BaseWidget` — where
    /// the signal does not exist.
    pub fn set_visible(&mut self, visible: bool) {
        if self.base.is_visible() == visible {
            return;
        }
        if visible {
            self.base.show();
        } else {
            self.base.hide();
        }
        self.base.request_redraw();
        self.visibility_changed.emit(visible);
    }
    /// Whether the toolbar is currently docked into a window (`false`) or hosted in
    /// its own top-level window (`true`).
    ///
    /// This is the toolbar's own state, not a `BaseWidget` flag: whether a control
    /// has been torn off into a separate window is a toolbar concept, and inventing
    /// a base-level flag for it would put a toolbar-only idea in every control.
    pub fn is_top_level(&self) -> bool {
        self.top_level
    }
    /// Moves the toolbar between docked and floating, and repaints.
    ///
    /// Emits `top_level_changed`, and only on a real change. As with
    /// [`Self::set_visible`], this exists so the signal has an emitter at all:
    /// `is_floatable` records the *permission* to float, which is a different
    /// question from whether the toolbar *is* floating, and only the latter is what
    /// the signal reports.
    ///
    /// Returns `false` without emitting when the toolbar is not floatable, so a
    /// caller cannot float a toolbar the host forbade. The refusal is silent in the
    /// signal stream but visible in the return value, which is where a caller can
    /// act on it.
    pub fn set_top_level(&mut self, top_level: bool) -> bool {
        if top_level && !self.floatable {
            return false;
        }
        if self.top_level == top_level {
            return false;
        }
        self.top_level = top_level;
        self.base.request_redraw();
        self.top_level_changed.emit(top_level);
        true
    }
    /// Appends an action item and returns its index in [`ToolBar::items`].
    ///
    /// `id` is what a later `action_triggered` carries and `text` is the visible
    /// label. The item starts enabled and unchecked. The returned index is the
    /// item's position, so removing items later invalidates it.
    pub fn add_action(&mut self, id: impl Into<String>, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(ToolBarItem::new(id, text));
        idx
    }
    /// Appends a visual divider.
    ///
    /// Unlike [`ToolBar::add_action`] this returns nothing, so a caller that
    /// needs the separator's index must read `items().len() - 1`.
    pub fn add_separator(&mut self) {
        self.items.push(ToolBarItem::separator());
    }
    /// Removes every item, including separators. Invalidates all previously
    /// returned indices.
    pub fn clear(&mut self) {
        self.items.clear();
    }
    /// Enables or disables the item at `index`, and repaints.
    ///
    /// An out-of-range index is ignored rather than treated as an error; read
    /// [`ToolBar::item_enabled`] afterwards to confirm the change took effect.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
        self.base.request_redraw();
    }
    /// Returns enabled state for item at index.
    ///
    /// `None` when `index` is out of range — which is how a caller notices that
    /// [`ToolBar::set_item_enabled`] silently ignored its index.
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_enabled())
    }
    /// Sets the checked state of the item at `index`, and repaints.
    ///
    /// Only takes effect on a checkable item; on any other item the call does
    /// nothing, silently. An out-of-range index is likewise ignored.
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
        self.base.request_redraw();
    }
    /// Returns checked state for item at index.
    ///
    /// `None` when `index` is out of range. For a non-checkable item this
    /// returns `Some(false)` (or whatever was set directly on the item) rather
    /// than `None`, so it does not tell the caller whether the item is togglable
    /// — check `items()[index].is_checkable()` for that.
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_checked())
    }
    fn button_size(&self) -> f32 {
        self.icon_size + 8.0
    }
    fn item_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let btn_sz = self.icon_size as u32 + 8;
        let sep_sz = 8u32;
        let mut offset = 2i32;
        for (i, item) in self.items.iter().enumerate() {
            let sz = if item.is_separator() { sep_sz } else { btn_sz };
            if i == index {
                return match self.orientation {
                    ToolBarOrientation::Horizontal => Rect {
                        x: rect.x + offset,
                        y: rect.y + 2,
                        width: sz,
                        height: rect.height.saturating_sub(4),
                    },
                    ToolBarOrientation::Vertical => Rect {
                        x: rect.x + 2,
                        y: rect.y + offset,
                        width: rect.width.saturating_sub(4),
                        height: sz,
                    },
                };
            }
            offset += sz as i32;
        }
        Rect { x: 0, y: 0, width: 0, height: 0 }
    }
    fn hit_item(&self, pos: Point) -> Option<usize> {
        for i in 0..self.items.len() {
            let r = self.item_rect(i);
            if pos.x >= r.x
                && pos.x <= r.x + r.width as i32
                && pos.y >= r.y
                && pos.y <= r.y + r.height as i32
            {
                return Some(i);
            }
        }
        None
    }
}
impl Widget for ToolBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ToolBar`'s property contract.
///
/// `orientation` is published as its token spelling and parsed back through
/// `expect_toolbar_orientation`; `icon_size` crosses the boundary as `Float`,
/// narrowed to `f32` by `expect_f32`.
impl WidgetProperties for ToolBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "orientation" => Ok(CapabilityValue::String(
                tool_bar_orientation_to_str(self.orientation()).to_string(),
            )),
            "icon_size" => Ok(CapabilityValue::Float(self.icon_size() as f64)),
            "floatable" => Ok(CapabilityValue::Bool(self.is_floatable())),
            "movable" => Ok(CapabilityValue::Bool(self.is_movable())),
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            "floatable" => {
                self.set_floatable(expect_bool(value)?);
                Ok(())
            }
            "icon_size" => {
                self.set_icon_size(expect_f32(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_toolbar_orientation(value)?);
                Ok(())
            }
            // Derived from the item list, which is mutated through `add_action` /
            // `add_separator` rather than by assigning a count.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "orientation",
            "icon_size",
            "movable",
            "floatable",
            "item_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for ToolBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.hit_item(*pos);
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_item(*pos) {
                    if let Some(item) = self.items.get_mut(idx) {
                        if item.is_enabled() && !item.is_separator() {
                            if item.is_checkable() {
                                item.set_checked(!item.is_checked());
                            }
                            let id = item.id().to_string();
                            self.action_triggered.emit(id);
                        }
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ToolBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let _btn_sz = self.button_size();
        // Background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        // Draw bottom border line
        let y = rect.y + rect.height as f32 as i32 - 1;
        context.draw_line(
            Point::new(rect.x, y),
            Point::new(rect.x + rect.width as i32, y),
            Color::rgb(200, 200, 200),
        );
        for i in 0..self.items.len() {
            let item_r = self.item_rect(i);
            let item = &self.items[i];
            if item.is_separator() {
                match self.orientation {
                    ToolBarOrientation::Horizontal => {
                        let mid_x = item_r.x + (item_r.width as i32) / 2;
                        context.draw_line(
                            Point::new(mid_x, rect.y + 4),
                            Point::new(mid_x, rect.y + rect.height as i32 - 4),
                            Color::rgb(200, 200, 200),
                        );
                    }
                    ToolBarOrientation::Vertical => {
                        let mid_y = item_r.y + item_r.height as i32 / 2;
                        context.draw_line(
                            Point::new(rect.x + 4, mid_y),
                            Point::new(rect.x + rect.width as i32 - 4, mid_y),
                            Color::rgb(200, 200, 200),
                        );
                    }
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            let bg = if item.is_checked() {
                Color::rgb(180, 210, 255)
            } else if is_hovered {
                Color::rgb(210, 230, 255)
            } else {
                Color::rgb(245, 245, 245)
            };
            context.fill_rect(Rect::new(item_r.x, item_r.y, item_r.width, item_r.height), bg);
            if is_hovered || item.is_checked() {
                context.draw_rect(
                    Rect::new(item_r.x, item_r.y, item_r.width, item_r.height),
                    Color::rgb(0, 120, 215),
                );
            }
            let fg =
                if !item.is_enabled() { Color::rgb(150, 150, 150) } else { Color::rgb(0, 0, 0) };
            context.draw_text(
                Point::new(item_r.x + item_r.width as i32 / 2, item_r.y + item_r.height as i32 / 2),
                item.text(),
                &Font::default(),
                fg,
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_item_state_accessors_handle_valid_and_oob_indices() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        let idx = tool_bar.add_action("save", "Save");

        assert_eq!(tool_bar.item_enabled(idx), Some(true));
        tool_bar.set_item_enabled(idx, false);
        assert_eq!(tool_bar.item_enabled(idx), Some(false));
        assert_eq!(tool_bar.item_enabled(99), None);

        tool_bar.items[idx].set_checkable(true);
        assert_eq!(tool_bar.item_checked(idx), Some(false));
        tool_bar.set_item_checked(idx, true);
        assert_eq!(tool_bar.item_checked(idx), Some(true));
        assert_eq!(tool_bar.item_checked(99), None);
    }
}
