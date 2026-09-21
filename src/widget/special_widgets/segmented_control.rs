// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SegmentedControl widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Single segment entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SegmentItem {
    /// Creates one segment item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Single-selection segmented control.
pub struct SegmentedControl {
    base: BaseWidget,
    items: Vec<SegmentItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    /// Emitted when selected segment changes. Payload is selected id.
    pub selection_changed: Signal1<String>,
}

impl SegmentedControl {
    /// Creates an empty segmented control.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "SegmentedControl"),
            items: Vec::new(),
            selected_index: None,
            hovered_index: None,
            selection_changed: Signal1::new(),
        }
    }

    /// Replaces all segment items.
    pub fn set_items(&mut self, items: Vec<SegmentItem>) {
        self.items = items;
        self.selected_index = if self.items.is_empty() { None } else { Some(0) };
        self.hovered_index = self.selected_index;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all segment items.
    pub fn items(&self) -> &[SegmentItem] {
        &self.items
    }

    /// Returns selected segment index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.items.len())
    }

    /// Returns selected segment id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index()?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Sets selected segment index.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.selection_changed.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Moves selection by signed delta.
    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let max = self.items.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        let _ = self.set_selected_index(next);
    }

    fn segment_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() {
            return None;
        }
        let rect = self.geometry();
        if self.items.is_empty() {
            return None;
        }
        let width = (rect.width as usize / self.items.len()).max(1) as u32;
        let x = rect.x + index as i32 * width as i32;
        let mut actual_width = width;
        if index + 1 == self.items.len() {
            let consumed = width.saturating_mul(index as u32);
            actual_width = rect.width.saturating_sub(consumed);
        }
        Some(Rect::new(x, rect.y, actual_width, rect.height))
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };
            if pos.x >= seg.x && pos.x < seg.x + seg.width as i32 {
                return Some(index);
            }
        }
        None
    }
}

impl Widget for SegmentedControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SegmentedControl`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` dispatch. All three properties describe the item set
/// and the current selection and have no writers in the legacy path, so `set`
/// refuses them with [`CapabilityAccessError::ReadOnlyProperty`] rather than
/// pretending the name does not exist. `SegmentedControl` reports
/// `WidgetKind::ToggleButton`, shared with `ToggleButton`; dispatching on the
/// concrete type here is what keeps the two contracts separate.
impl WidgetProperties for SegmentedControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_id" => match self.selected_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "item_count" | "selected_index" | "selected_id" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["item_count", "selected_index", "selected_id", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `segmented_control` publishes.
    ///
    /// `move_selection` takes a signed delta, and the affordance the control itself
    /// offers for a bare move is "advance one": its right-arrow handler calls
    /// `move_selection(1)`. There is no way to express a direction without an
    /// argument, so rather than guess one — which would silently do something the
    /// caller did not ask for — the name is refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing. `set_items` carries the item list and is refused
    /// for the same reason.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "move_selection" | "set_items" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SegmentedControl {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.hit_index(*pos);
            }
            Event::MouseLeave { .. } => {
                self.hovered_index = None;
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SegmentedControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the bar, its dividers, its selection and its labels
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // `segmented_control` reports `WidgetKind::ToggleButton`, whose role is `Primary`; the
        // control itself is not a filled call to action, so the role's primary fill is not used.
        // Its accent serves the selection instead, and the bar derives its own surface below.
        let theme = crate::style::resolved_theme_style("segmented_control");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, primary, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The bar: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. The filter is on the **resolved** value, not
        // only on the theme's: the active theme is applied to every control before it is drawn, so
        // a control classified as `Surface` already carries the window fill and letting it through
        // unfiltered would make the bar invisible. A caller's own colour still wins.
        let bar_from_theme = window_fill.blend(&ink, 0.08);
        let bar = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => bar_from_theme,
        };
        // The dividers are a fixed step out of the bar so they stay visible whatever the bar is.
        let divider = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| bar.blend(&muted, 0.45));
        // A selected segment is the control's value indicator, so it carries the theme's primary
        // rather than a fixed blue; a hovered one is a lighter step of the same.
        let selected_bg = primary.blend(&bar, 0.55);
        let hovered_bg = primary.blend(&bar, 0.25);

        context.fill_rect(rect, bar);
        context.draw_rect(rect, divider);

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };

            let bg = if self.selected_index == Some(index) {
                selected_bg
            } else if self.hovered_index == Some(index) {
                hovered_bg
            } else {
                bar
            };
            context.fill_rect(seg, bg);

            if index > 0 {
                context.draw_line(
                    Point::new(seg.x, seg.y),
                    Point::new(seg.x, seg.y + seg.height as i32),
                    divider,
                );
            }

            if let Some(item) = self.items.get(index) {
                // The label contrasts with the fill it is painted on — the primary for the
                // selected segment, the bar for the rest — so it stays legible on either
                // appearance instead of being a fixed dark slate on a dark bar.
                let label_color = if self.selected_index == Some(index) {
                    selected_bg.contrast_color()
                } else {
                    ink
                };
                context.draw_text(
                    Point::new(seg.x + 8, seg.y + seg.height as i32 / 2),
                    &item.label,
                    &Font::default(),
                    label_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<SegmentItem> {
        vec![
            SegmentItem::new("overview", "Overview"),
            SegmentItem::new("details", "Details"),
            SegmentItem::new("history", "History"),
        ]
    }

    #[test]
    fn set_items_selects_first_item() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("overview"));
    }

    #[test]
    fn keyboard_navigation_updates_selection() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("details"));

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("history"));

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_id(), Some("details"));
    }

    #[test]
    fn selection_changed_emits_selected_id() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        control.selection_changed.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        let _ = control.set_selected_index(2);
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["history".to_string()]);
    }

    #[test]
    fn default_state() {
        let control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        assert!(control.items().is_empty());
        assert_eq!(control.selected_index(), None);
        assert_eq!(control.selected_id(), None);
    }

    #[test]
    fn set_selected_index_get_set() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("tab1", "Tab 1"),
            SegmentItem::new("tab2", "Tab 2"),
            SegmentItem::new("tab3", "Tab 3"),
        ]);

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));

        assert!(control.set_selected_index(2));
        assert_eq!(control.selected_index(), Some(2));
        assert_eq!(control.selected_id(), Some("tab3"));

        assert!(control.set_selected_index(0));
        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));
    }

    #[test]
    fn invalid_index_handling() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![SegmentItem::new("a", "A")]);

        // Out of bounds returns false
        assert!(!control.set_selected_index(10));
        assert_eq!(control.selected_index(), Some(0));

        // Valid index returns true
        assert!(control.set_selected_index(0));

        // Move out of bounds clamped
        control.move_selection(10);
        assert_eq!(control.selected_index(), Some(0));

        control.move_selection(-10);
        assert_eq!(control.selected_index(), Some(0));
    }

    #[test]
    fn empty_segments() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        // Navigation on empty should not panic
        control.move_selection(1);
        assert_eq!(control.selected_index(), None);

        // set_selected_index on empty returns false
        assert!(!control.set_selected_index(0));

        // handle event on empty should not panic
        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_index(), None);

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_index(), None);
    }

    #[test]
    fn move_selection_previous_next() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("x", "X"),
            SegmentItem::new("y", "Y"),
            SegmentItem::new("z", "Z"),
        ]);

        // Start at 0, move forward
        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("z"));

        // Move backward
        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("x"));
    }
}
