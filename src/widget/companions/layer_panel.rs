// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `LayerPanel` — the companion list for a canvas's layers (COMP-I01).
//!
//! # Why a companion rather than part of the canvas
//!
//! `Canvas` owns a complete layer model — `begin_layer`, `set_layer_visible`, `move_layer`,
//! `remove_layer`, `layer_count` and the `layer_changed` signal — but it paints none of it. A
//! drawing surface needs the list *beside* it, in whatever place and size the layout gives it,
//! and a canvas that painted its own layer list would have hard-coded one placement for every
//! host. This control is that list: it holds a read-only mirror the host feeds from
//! `layer_changed`, and reports every user intent as a signal so the host can translate it into
//! the canvas's own call. The panel never touches the canvas.
//!
//! # Identity, not position
//!
//! Entries are addressed by their stable `id`, which is the identity `Canvas` gives a layer. A
//! reorder therefore cannot make a selection point at the wrong layer, and every signal carries
//! the id rather than the row index.

use crate::compat::{String, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// One row of the panel: the mirror of one canvas layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerEntry {
    /// The layer's stable identity, as `Canvas` reports it.
    pub id: u64,
    /// The layer's display name.
    pub name: String,
    /// Whether the layer is painted.
    pub visible: bool,
}

impl LayerEntry {
    /// Creates a visible layer row.
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), visible: true }
    }

    /// Sets the initial visibility.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

/// Width of the visibility checkbox column.
const LAYER_CHECK_WIDTH: i32 = 18;
/// Width of the reorder handle column.
const LAYER_HANDLE_WIDTH: i32 = 12;
/// Width of the trailing remove button.
const LAYER_REMOVE_WIDTH: i32 = 16;
/// Padding inside the panel.
const LAYER_PADDING: i32 = 4;

/// The layer list a canvas's layer API is driven from.
pub struct LayerPanel {
    base: BaseWidget,
    entries: Vec<LayerEntry>,
    selected_layer: Option<u64>,
    show_visibility_checkboxes: bool,
    show_reorder_handles: bool,
    row_height: u32,
    /// The first row painted; keeps the selection reachable in a long layer stack.
    scroll_offset: usize,
    /// The row the pointer is on, for the hover tint.
    hovered: Option<u64>,
    /// A reorder drag: the id picked up at the press.
    reorder_from: Option<u64>,
    /// The rename in progress: the layer being renamed and the text typed so far.
    renaming: Option<(u64, String)>,
    /// Emitted with the layer id when the selection moves.
    pub layer_selected: Signal1<u64>,
    /// Emitted with `(layer id, visible)` after the checkbox is pressed.
    pub visibility_toggled: Signal1<(u64, bool)>,
    /// Emitted with `(from id, to id)` after a handle drag, for the host to translate to
    /// `Canvas::move_layer`.
    pub reorder_requested: Signal1<(u64, u64)>,
    /// Emitted with `(layer id, new name)` after an in-place rename is committed.
    pub rename_requested: Signal1<(u64, String)>,
    /// Emitted with the layer id when the row's trailing remove button is pressed.
    pub remove_requested: Signal1<u64>,
}

impl LayerPanel {
    /// Creates an empty layer panel.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "LayerPanel"),
            entries: Vec::new(),
            selected_layer: None,
            show_visibility_checkboxes: true,
            show_reorder_handles: true,
            row_height: 22,
            scroll_offset: 0,
            hovered: None,
            reorder_from: None,
            renaming: None,
            layer_selected: Signal1::new(),
            visibility_toggled: Signal1::new(),
            reorder_requested: Signal1::new(),
            rename_requested: Signal1::new(),
            remove_requested: Signal1::new(),
        }
    }

    /// Replaces the mirrored entry list.
    ///
    /// The host calls this from the canvas's `layer_changed` signal. A selection that names a layer
    /// which is no longer present is dropped rather than left pointing at nothing.
    pub fn set_entries(&mut self, entries: Vec<LayerEntry>) {
        self.entries = entries;
        if let Some(selected) = self.selected_layer {
            if !self.entries.iter().any(|entry| entry.id == selected) {
                self.selected_layer = None;
            }
        }
        let max = self.entries.len().saturating_sub(self.visible_rows());
        self.scroll_offset = self.scroll_offset.min(max);
        self.base.request_redraw();
    }

    /// The mirrored entries, top-most first.
    pub fn entries(&self) -> &[LayerEntry] {
        &self.entries
    }

    /// The selected layer's id.
    pub fn selected_layer(&self) -> Option<u64> {
        self.selected_layer
    }

    /// Selects the layer with `id`, returning whether it exists.
    pub fn select_layer(&mut self, id: u64) -> bool {
        if !self.entries.iter().any(|entry| entry.id == id) {
            return false;
        }
        if self.selected_layer == Some(id) {
            return true;
        }
        self.selected_layer = Some(id);
        self.scroll_to_layer(id);
        self.layer_selected.emit(id);
        self.base.request_redraw();
        true
    }

    /// Updates the panel's own visibility mirror and publishes the change.
    ///
    /// The host applies it to the canvas; the panel updates its mirror so the checkbox answers
    /// immediately rather than one `layer_changed` round-trip later.
    pub fn set_entry_visible(&mut self, id: u64, visible: bool) -> bool {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) else {
            return false;
        };
        if entry.visible == visible {
            return true;
        }
        entry.visible = visible;
        self.visibility_toggled.emit((id, visible));
        self.base.request_redraw();
        true
    }

    /// Scrolls the list so the layer with `id` is visible, returning whether it exists.
    pub fn scroll_to_layer(&mut self, id: u64) -> bool {
        let Some(row) = self.entries.iter().position(|entry| entry.id == id) else {
            return false;
        };
        let visible = self.visible_rows().max(1);
        if row < self.scroll_offset {
            self.scroll_offset = row;
        } else if row >= self.scroll_offset + visible {
            self.scroll_offset = row + 1 - visible;
        }
        self.base.request_redraw();
        true
    }

    /// Whether each row paints a visibility checkbox.
    pub fn visibility_checkboxes(&self) -> bool {
        self.show_visibility_checkboxes
    }

    /// Shows or hides the visibility checkboxes.
    pub fn set_visibility_checkboxes(&mut self, show: bool) {
        if self.show_visibility_checkboxes == show {
            return;
        }
        self.show_visibility_checkboxes = show;
        self.base.request_redraw();
    }

    /// Whether each row paints a reorder handle.
    pub fn reorder_handles(&self) -> bool {
        self.show_reorder_handles
    }

    /// Shows or hides the reorder handles.
    pub fn set_reorder_handles(&mut self, show: bool) {
        if self.show_reorder_handles == show {
            return;
        }
        self.show_reorder_handles = show;
        self.base.request_redraw();
    }

    /// The row height in pixels (floored at 12).
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the row height.
    pub fn set_row_height(&mut self, height: u32) {
        let height = height.max(12);
        if self.row_height == height {
            return;
        }
        self.row_height = height;
        self.base.request_redraw();
    }

    /// How many rows fit in the panel.
    fn visible_rows(&self) -> usize {
        let height = self.geometry().height.max(1);
        (height / self.row_height.max(1)) as usize
    }

    /// One row's rectangle, in window coordinates.
    fn row_rect(&self, row: usize) -> Option<Rect> {
        if row < self.scroll_offset || row >= self.scroll_offset + self.visible_rows() {
            return None;
        }
        let rect = self.geometry();
        let offset = (row - self.scroll_offset) as i32 * self.row_height as i32;
        Some(Rect::new(rect.x, rect.y + offset, rect.width, self.row_height))
    }

    /// The row under `pos`, as an index into [`Self::entries`].
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row =
            self.scroll_offset + ((pos.y - rect.y) / self.row_height.max(1) as i32).max(0) as usize;
        (row < self.entries.len()).then_some(row)
    }

    /// The checkbox's rectangle inside `row`.
    fn check_rect(&self, row: Rect) -> Option<Rect> {
        if !self.show_visibility_checkboxes {
            return None;
        }
        let side = 12u32.min(row.height.saturating_sub(4)).max(1);
        Some(Rect::new(
            row.x + LAYER_PADDING,
            row.y + (row.height.saturating_sub(side) / 2) as i32,
            side,
            side,
        ))
    }

    /// The reorder handle's rectangle inside `row`.
    fn handle_rect(&self, row: Rect) -> Option<Rect> {
        if !self.show_reorder_handles {
            return None;
        }
        Some(Rect::new(
            row.x + LAYER_PADDING + LAYER_CHECK_WIDTH,
            row.y,
            LAYER_HANDLE_WIDTH as u32,
            row.height,
        ))
    }

    /// The trailing remove button's rectangle inside `row`.
    fn remove_rect(&self, row: Rect) -> Rect {
        Rect::new(
            row.x + row.width as i32 - LAYER_REMOVE_WIDTH - LAYER_PADDING,
            row.y,
            LAYER_REMOVE_WIDTH as u32,
            row.height,
        )
    }
}

impl EventHandler for LayerPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => {
                if let Some((_, buffer)) = self.renaming.as_mut() {
                    match *key {
                        8 => {
                            buffer.pop();
                            self.base.request_redraw();
                        }
                        13 => {
                            let finished = self.renaming.take();
                            if let Some((id, name)) = finished {
                                if let Some(entry) =
                                    self.entries.iter_mut().find(|entry| entry.id == id)
                                {
                                    if !name.is_empty() {
                                        entry.name = name.clone();
                                        self.rename_requested.emit((id, name));
                                    }
                                }
                            }
                            self.base.request_redraw();
                        }
                        27 => {
                            self.renaming = None;
                            self.base.request_redraw();
                        }
                        _ => {}
                    }
                    return;
                }
                match *key {
                    38 => self.move_selection(-1),
                    40 => self.move_selection(1),
                    46 => {
                        if let Some(id) = self.selected_layer {
                            self.remove_requested.emit(id);
                        }
                    }
                    _ => {}
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if let Some((_, buffer)) = self.renaming.as_mut() {
                    buffer.push_str(text);
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } => {
                let hovered = self.row_at(*pos).map(|row| self.entries[row].id);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered = None;
            }
            Event::MousePress { pos, button: 1, .. } => {
                let Some(row) = self.row_at(*pos) else { return };
                let entry = self.entries[row].clone();
                let rect = self.row_rect(row).unwrap_or_else(|| self.geometry());
                // The checkbox toggles visibility without moving the selection, so a reader can
                // hide a layer and keep working where they were.
                if self.check_rect(rect).is_some_and(|check| check.contains_point(*pos)) {
                    self.set_entry_visible(entry.id, !entry.visible);
                    return;
                }
                if self.handle_rect(rect).is_some_and(|handle| handle.contains_point(*pos)) {
                    self.reorder_from = Some(entry.id);
                    return;
                }
                if self.remove_rect(rect).contains_point(*pos) {
                    self.remove_requested.emit(entry.id);
                    return;
                }
                let _ = self.select_layer(entry.id);
            }
            Event::MouseRelease { pos, button: 1 } => {
                if let Some(from) = self.reorder_from.take() {
                    let to = self.row_at(*pos).map(|row| self.entries[row].id);
                    if let Some(to) = to {
                        if to != from {
                            self.reorder_requested.emit((from, to));
                        }
                    }
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(row) = self.row_at(*pos) {
                    let entry = &self.entries[row];
                    self.renaming = Some((entry.id, entry.name.clone()));
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, .. }
                if delta.y != 0 => {
                    let max = self.entries.len().saturating_sub(self.visible_rows().max(1));
                    let next = if delta.y > 0 {
                        (self.scroll_offset + 1).min(max)
                    } else {
                        self.scroll_offset.saturating_sub(1)
                    };
                    if next != self.scroll_offset {
                        self.scroll_offset = next;
                        self.base.request_redraw();
                    }
                }
            _ => {}
        }
    }
}

impl LayerPanel {
    /// Moves the selection by `delta` rows, skipping nothing.
    fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let current = self
            .selected_layer
            .and_then(|id| self.entries.iter().position(|entry| entry.id == id))
            .unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, self.entries.len() as isize - 1) as usize;
        let id = self.entries[next].id;
        let _ = self.select_layer(id);
    }
}

impl Draw for LayerPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let font = effective_font(style);
        let accent = accent_color();
        let selected_fill = ink.blend(&surface, 0.85);
        let hover_fill = ink.blend(&surface, 0.93);
        let rect = self.geometry();
        context.fill_rect(rect, surface);
        context.draw_rect(rect, ink.blend(&surface, 0.7));

        let rows = self.visible_rows();
        for row in self.scroll_offset..(self.scroll_offset + rows).min(self.entries.len()) {
            let Some(row_rect) = self.row_rect(row) else { continue };
            let entry = &self.entries[row];
            if self.selected_layer == Some(entry.id) {
                context.fill_rect(row_rect, selected_fill);
            } else if self.hovered == Some(entry.id) {
                context.fill_rect(row_rect, hover_fill);
            }

            let mut label_x = row_rect.x + LAYER_PADDING;
            if let Some(check) = self.check_rect(row_rect) {
                context.draw_rect(check, ink.blend(&surface, 0.5));
                if entry.visible {
                    // A filled inset is the check mark: it reads at 12 px where a glyph would not.
                    context.fill_rect(
                        Rect::new(
                            check.x + 2,
                            check.y + 2,
                            check.width.saturating_sub(4),
                            check.height.saturating_sub(4),
                        ),
                        accent,
                    );
                }
                label_x += LAYER_CHECK_WIDTH;
            }
            if let Some(handle) = self.handle_rect(row_rect) {
                // Two short rules stand in for a grab handle; no glyph is needed for six pixels.
                let mid = handle.y + handle.height as i32 / 2;
                for offset in [-2i32, 2] {
                    context.draw_line(
                        Point::new(handle.x + 2, mid + offset),
                        Point::new(handle.x + handle.width as i32 - 2, mid + offset),
                        ink.with_alpha(90),
                    );
                }
                label_x += LAYER_HANDLE_WIDTH;
            }

            let remove = self.remove_rect(row_rect);
            let label_width = (remove.x - 4 - label_x).max(0) as u32;
            let label = match &self.renaming {
                Some((id, buffer)) if *id == entry.id => buffer.as_str(),
                _ => entry.name.as_str(),
            };
            let line = context
                .text_line(Rect::new(label_x, row_rect.y, label_width, row_rect.height), font);
            context.draw_text_fitted_cow(
                Rect::new(label_x, line.y, label_width, line.height),
                label,
                font,
                if entry.visible { ink } else { ink.with_alpha(110) },
                HorizontalAlignment::Left,
            );
            // The remove affordance is a small cross in the row's trailing band.
            let cross_ink = ink.with_alpha(150);
            let cx = remove.x + remove.width as i32 / 2;
            let cy = remove.y + remove.height as i32 / 2;
            context.draw_line_stroke(
                Point::new(cx - 3, cy - 3),
                Point::new(cx + 3, cy + 3),
                cross_ink,
                1,
            );
            context.draw_line_stroke(
                Point::new(cx + 3, cy - 3),
                Point::new(cx - 3, cy + 3),
                cross_ink,
                1,
            );
        }
    }
}

impl Widget for LayerPanel {
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
        Size::new(180, (self.entries.len() as u32 * self.row_height).max(self.row_height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "layer_selected" => {
                Some(EventSignalRef::mapped("layer_selected", &self.layer_selected, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            "visibility_toggled" => Some(EventSignalRef::mapped(
                "visibility_toggled",
                &self.visibility_toggled,
                |value| {
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::UInt(value.0),
                        CapabilityValue::Bool(value.1),
                    ]))
                },
            )),
            "reorder_requested" => Some(EventSignalRef::mapped(
                "reorder_requested",
                &self.reorder_requested,
                |value| {
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::UInt(value.0),
                        CapabilityValue::UInt(value.1),
                    ]))
                },
            )),
            "rename_requested" => {
                Some(EventSignalRef::mapped("rename_requested", &self.rename_requested, |value| {
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::UInt(value.0),
                        CapabilityValue::String(value.1.clone()),
                    ]))
                }))
            }
            "remove_requested" => {
                Some(EventSignalRef::mapped("remove_requested", &self.remove_requested, |value| {
                    CapabilityValue::UInt(*value)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for LayerPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entry_count" => Ok(CapabilityValue::UInt(self.entries.len() as u64)),
            "selected_layer" => Ok(match self.selected_layer {
                Some(id) => CapabilityValue::UInt(id),
                None => CapabilityValue::Null,
            }),
            "show_visibility_checkboxes" => {
                Ok(CapabilityValue::Bool(self.show_visibility_checkboxes))
            }
            "show_reorder_handles" => Ok(CapabilityValue::Bool(self.show_reorder_handles)),
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The entry list is the canvas's model, mirrored through `set_entries`.
            "entry_count" | "selected_layer" => Err(CapabilityAccessError::ReadOnlyProperty),
            "show_visibility_checkboxes" => {
                self.set_visibility_checkboxes(expect_bool(value)?);
                Ok(())
            }
            "show_reorder_handles" => {
                self.set_reorder_handles(expect_bool(value)?);
                Ok(())
            }
            "row_height" => {
                let CapabilityValue::UInt(height) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_row_height(u32::try_from(height).unwrap_or(u32::MAX));
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "entry_count",
            "selected_layer",
            "show_visibility_checkboxes",
            "show_reorder_handles",
            "row_height",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            // Every command this control publishes needs a layer id, which a bare invocation does
            // not carry.
            "select_layer" | "set_entry_visible" | "scroll_to_layer" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn panel() -> LayerPanel {
        let mut panel = LayerPanel::new(Rect::new(0, 0, 200, 100));
        panel.set_entries(vec![
            LayerEntry::new(1, "background"),
            LayerEntry::new(2, "ink"),
            LayerEntry::new(3, "notes").with_visible(false),
        ]);
        panel
    }

    #[test]
    fn replacing_the_entries_drops_a_selection_that_no_longer_exists() {
        let mut panel = panel();
        assert!(panel.select_layer(2));
        assert_eq!(panel.selected_layer(), Some(2));
        panel.set_entries(vec![LayerEntry::new(1, "background")]);
        assert_eq!(panel.selected_layer(), None, "a stale id must not stay selected");
    }

    #[test]
    fn the_checkbox_toggles_visibility_without_moving_the_selection() {
        let mut panel = panel();
        assert!(panel.select_layer(1));
        let toggled = Arc::new(Mutex::new(Vec::<(u64, bool)>::new()));
        let sink = Arc::clone(&toggled);
        panel.visibility_toggled.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });
        // Row 2 sits at y = 22..44; its checkbox is the 12 px square at the row's leading edge.
        panel.handle_event(&Event::mouse_press(10, 33, 1));
        assert_eq!(*toggled.lock().expect("toggled lock"), vec![(2, false)]);
        assert_eq!(panel.selected_layer(), Some(1), "toggling is not selecting");
        assert!(!panel.entries()[1].visible);
    }

    #[test]
    fn a_handle_drag_requests_a_reorder() {
        let mut panel = panel();
        let reordered = Arc::new(Mutex::new(Vec::<(u64, u64)>::new()));
        let sink = Arc::clone(&reordered);
        panel.reorder_requested.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });
        // The handle column starts after the checkbox column.
        panel.handle_event(&Event::mouse_press(24, 11, 1));
        panel.handle_event(&Event::mouse_release(24, 33, 1));
        assert_eq!(*reordered.lock().expect("reordered lock"), vec![(1, 2)]);
    }

    #[test]
    fn a_double_click_renames_through_the_signal() {
        let mut panel = panel();
        let renamed = Arc::new(Mutex::new(Vec::<(u64, String)>::new()));
        let sink = Arc::clone(&renamed);
        panel.rename_requested.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*value).clone());
            }
        });
        panel.handle_event(&Event::mouse_double_click(100, 11, 1));
        // Clearing the prefilled name and typing a new one.
        for _ in 0.."background".len() {
            panel.handle_event(&Event::key_press(8, 0));
        }
        panel.handle_event(&Event::text_input("base"));
        panel.handle_event(&Event::key_press(13, 0));
        let expected = vec![(1u64, "base".to_string())];
        assert_eq!(*renamed.lock().expect("renamed lock"), expected);
        assert_eq!(panel.entries()[0].name, "base");
    }
}
