// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `PlaylistPanel` — the companion list for a media player's queue (COMP-I03).
//!
//! # Why a companion rather than part of the player
//!
//! `MediaPlayer` owns the playlist model — `set_playlist`, `playlist`, `playlist_index`,
//! `select_track`, `next_track`, `previous_track` — and reports `media_ended`, but it paints only
//! the current source. The queue is a *second* piece of chrome, placed wherever the layout puts
//! it, and the player cannot know that placement. This panel lists the queue and reports the two
//! things a user does with it — pick a track and reorder it — as signals, so the host can apply
//! them to its own list and feed the result back. The panel keeps no model of its own beyond the
//! rows it is given.
//!
//! # Why the indices in the signals are original indices
//!
//! A filter narrows what is *shown*, not what is *stored*. Every signal therefore carries the
//! track's position in the full list, so a host that selects a filtered row does not have to undo
//! the filter to find out which track was meant.

use crate::compat::{String, ToString, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Width of the track-number column, shown when numbering is on.
const PLAYLIST_NUMBER_WIDTH: i32 = 26;
/// Width of the trailing remove button.
const PLAYLIST_REMOVE_WIDTH: i32 = 16;
/// Padding inside the panel.
const PLAYLIST_PADDING: i32 = 4;

/// The queue a media player's playlist API is driven from.
pub struct PlaylistPanel {
    base: BaseWidget,
    tracks: Vec<String>,
    current_index: Option<usize>,
    show_track_numbers: bool,
    show_now_playing_badge: bool,
    row_height: u32,
    /// Whether a track's friendly name strips its path and extension.
    title_from_source: bool,
    /// The filter text; `None` or empty shows every track.
    filter: Option<String>,
    /// The selected row, as an index into `tracks`.
    selected: Option<usize>,
    /// The first visible row of the *filtered* view.
    scroll_offset: usize,
    /// The row the pointer is on, for the hover tint.
    hovered: Option<usize>,
    /// A reorder drag: the original index picked up at the press.
    drag_from: Option<usize>,
    /// Emitted with the original track index when a row is chosen.
    pub track_selected: Signal1<usize>,
    /// Emitted with `(from, to)` original indices after a row drag.
    pub track_move_requested: Signal1<(usize, usize)>,
    /// Emitted with the original track index when the row's remove button is pressed.
    pub track_remove_requested: Signal1<usize>,
}

impl PlaylistPanel {
    /// Creates an empty playlist panel.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "PlaylistPanel"),
            tracks: Vec::new(),
            current_index: None,
            show_track_numbers: true,
            show_now_playing_badge: true,
            row_height: 22,
            title_from_source: true,
            filter: None,
            selected: None,
            scroll_offset: 0,
            hovered: None,
            drag_from: None,
            track_selected: Signal1::new(),
            track_move_requested: Signal1::new(),
            track_remove_requested: Signal1::new(),
        }
    }

    /// Replaces the queue the panel mirrors.
    pub fn set_tracks(&mut self, tracks: Vec<String>) {
        self.tracks = tracks;
        if let Some(index) = self.current_index {
            if index >= self.tracks.len() {
                self.current_index = None;
            }
        }
        if let Some(index) = self.selected {
            if index >= self.tracks.len() {
                self.selected = None;
            }
        }
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The mirrored queue.
    pub fn tracks(&self) -> &[String] {
        &self.tracks
    }

    /// The index of the track the player is on, if any.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Sets the track the player is on, scrolling it into view.
    pub fn set_current_index(&mut self, index: Option<usize>) {
        let index = index.filter(|index| *index < self.tracks.len());
        if self.current_index == index {
            return;
        }
        self.current_index = index;
        if let Some(index) = index {
            self.scroll_to_index(index);
        }
        self.base.request_redraw();
    }

    /// The selected row's original index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// Selects a track by its original index, returning whether it exists.
    pub fn select_track(&mut self, index: usize) -> bool {
        if index >= self.tracks.len() {
            return false;
        }
        self.selected = Some(index);
        self.scroll_to_index(index);
        self.track_selected.emit(index);
        self.base.request_redraw();
        true
    }

    /// Scrolls the current track into view.
    pub fn scroll_to_current(&mut self) -> bool {
        let Some(index) = self.current_index else {
            return false;
        };
        self.scroll_to_index(index);
        true
    }

    /// Sets the filter text; an empty string clears it.
    pub fn set_filter(&mut self, filter: Option<String>) {
        let filter = filter.filter(|text| !text.is_empty());
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        self.scroll_offset = 0;
        self.base.request_redraw();
    }

    /// The current filter text.
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Whether rows show their queue number.
    pub fn show_track_numbers(&self) -> bool {
        self.show_track_numbers
    }

    /// Shows or hides the queue numbers.
    pub fn set_show_track_numbers(&mut self, show: bool) {
        if self.show_track_numbers == show {
            return;
        }
        self.show_track_numbers = show;
        self.base.request_redraw();
    }

    /// Whether the playing row shows its badge.
    pub fn show_now_playing_badge(&self) -> bool {
        self.show_now_playing_badge
    }

    /// Shows or hides the now-playing badge.
    pub fn set_show_now_playing_badge(&mut self, show: bool) {
        if self.show_now_playing_badge == show {
            return;
        }
        self.show_now_playing_badge = show;
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

    /// Whether a row's label is the friendly title rather than the raw source string.
    pub fn title_from_source(&self) -> bool {
        self.title_from_source
    }

    /// Sets whether labels are derived from the source path.
    pub fn set_title_from_source(&mut self, friendly: bool) {
        if self.title_from_source == friendly {
            return;
        }
        self.title_from_source = friendly;
        self.base.request_redraw();
    }

    /// The original indices shown, in order, after the filter.
    fn visible_tracks(&self) -> Vec<usize> {
        match self.filter.as_deref() {
            None => (0..self.tracks.len()).collect(),
            Some(filter) => {
                let folded = filter.to_lowercase();
                (0..self.tracks.len())
                    .filter(|index| {
                        self.tracks
                            .get(*index)
                            .is_some_and(|track| track.to_lowercase().contains(&folded))
                    })
                    .collect()
            }
        }
    }

    /// How many rows fit in the panel.
    fn visible_rows(&self) -> usize {
        (self.geometry().height.max(1) / self.row_height.max(1)) as usize
    }

    /// One row's rectangle, for a position in the filtered view.
    fn row_rect(&self, visible_row: usize) -> Option<Rect> {
        if visible_row < self.scroll_offset
            || visible_row >= self.scroll_offset + self.visible_rows()
        {
            return None;
        }
        let rect = self.geometry();
        let offset = (visible_row - self.scroll_offset) as i32 * self.row_height as i32;
        Some(Rect::new(rect.x, rect.y + offset, rect.width, self.row_height))
    }

    /// The filtered-view position under `pos`, as an index into [`Self::visible_tracks`].
    fn visible_row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(pos) {
            return None;
        }
        let row =
            self.scroll_offset + ((pos.y - rect.y) / self.row_height.max(1) as i32).max(0) as usize;
        (row < self.visible_tracks().len()).then_some(row)
    }

    /// Scrolls so the original track `index` is visible.
    fn scroll_to_index(&mut self, index: usize) {
        let visible = self.visible_tracks();
        let Some(position) = visible.iter().position(|track| *track == index) else {
            return;
        };
        let rows = self.visible_rows().max(1);
        if position < self.scroll_offset {
            self.scroll_offset = position;
        } else if position >= self.scroll_offset + rows {
            self.scroll_offset = position + 1 - rows;
        }
    }

    /// Keeps the scroll offset inside the filtered list.
    fn clamp_scroll(&mut self) {
        let max = self.visible_tracks().len().saturating_sub(self.visible_rows().max(1));
        self.scroll_offset = self.scroll_offset.min(max);
    }

    /// The remove button's rectangle inside `row`.
    fn remove_rect(&self, row: Rect) -> Rect {
        Rect::new(
            row.x + row.width as i32 - PLAYLIST_REMOVE_WIDTH - PLAYLIST_PADDING,
            row.y,
            PLAYLIST_REMOVE_WIDTH as u32,
            row.height,
        )
    }

    /// The label painted for a track.
    ///
    /// A friendly title is the file's own name, without its directory or extension, which is what
    /// a queue conventionally shows; the raw string stays the data the panel holds.
    fn track_label(&self, track: &str) -> String {
        if !self.title_from_source {
            return track.to_string();
        }
        let name = track.rsplit(['/', '\\']).next().unwrap_or(track);
        match name.rfind('.') {
            Some(dot) if dot > 0 => name[..dot].to_string(),
            _ => name.to_string(),
        }
    }
}

impl EventHandler for PlaylistPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let visible = self.visible_tracks();
                let hovered = self.visible_row_at(*pos).and_then(|row| visible.get(row).copied());
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                self.hovered = None;
            }
            Event::MousePress { pos, button: 1, .. } => {
                let visible = self.visible_tracks();
                let Some(row) = self.visible_row_at(*pos) else { return };
                let Some(&track) = visible.get(row) else { return };
                let rect = self.row_rect(row).unwrap_or_else(|| self.geometry());
                if self.remove_rect(rect).contains_point(*pos) {
                    self.track_remove_requested.emit(track);
                    return;
                }
                self.drag_from = Some(track);
                self.selected = Some(track);
                self.track_selected.emit(track);
                self.base.request_redraw();
            }
            Event::MouseRelease { pos, button: 1 } => {
                if let Some(from) = self.drag_from.take() {
                    let visible = self.visible_tracks();
                    let to = self.visible_row_at(*pos).and_then(|row| visible.get(row).copied());
                    if let Some(to) = to {
                        if to != from {
                            self.track_move_requested.emit((from, to));
                        }
                    }
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                // A double click is the confirmation of the row's selection: the host starts
                // playback on the second press and the first has already reported the pick.
                let visible = self.visible_tracks();
                if let Some(track) = self.visible_row_at(*pos).and_then(|row| visible.get(row)) {
                    self.track_selected.emit(*track);
                }
            }
            Event::KeyPress { key, .. } => match *key {
                38 => self.move_selection(-1),
                40 => self.move_selection(1),
                13 => {
                    if let Some(index) = self.selected {
                        self.track_selected.emit(index);
                    }
                }
                46 => {
                    if let Some(index) = self.selected {
                        self.track_remove_requested.emit(index);
                    }
                }
                _ => {}
            },
            Event::Wheel { delta, .. } if delta.y != 0 => {
                let max = self.visible_tracks().len().saturating_sub(self.visible_rows().max(1));
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

impl PlaylistPanel {
    /// Moves the selection by `delta` positions in the filtered view.
    fn move_selection(&mut self, delta: isize) {
        let visible = self.visible_tracks();
        if visible.is_empty() {
            return;
        }
        let current = self
            .selected
            .and_then(|index| visible.iter().position(|track| *track == index))
            .unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, visible.len() as isize - 1) as usize;
        let index = visible[next];
        self.selected = Some(index);
        let rows = self.visible_rows().max(1);
        if next < self.scroll_offset {
            self.scroll_offset = next;
        } else if next >= self.scroll_offset + rows {
            self.scroll_offset = next + 1 - rows;
        }
        self.base.request_redraw();
    }
}

impl Draw for PlaylistPanel {
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

        let visible = self.visible_tracks();
        let rows = self.visible_rows();
        for (position, &track_index) in
            visible.iter().enumerate().skip(self.scroll_offset).take(rows)
        {
            let Some(row_rect) = self.row_rect(position) else { continue };
            let Some(track) = self.tracks.get(track_index) else { continue };
            let is_current = self.current_index == Some(track_index);
            if self.selected == Some(track_index) {
                context.fill_rect(row_rect, selected_fill);
            } else if self.hovered == Some(track_index) {
                context.fill_rect(row_rect, hover_fill);
            }

            let mut label_x = row_rect.x + PLAYLIST_PADDING;
            if self.show_track_numbers {
                // A queue number is a position within the *list*, so it counts the original
                // index, not the filtered row: the number a reader sees is the one the player
                // reports.
                let number = (track_index + 1).to_string();
                context.draw_text_fitted_cow(
                    Rect::new(label_x, row_rect.y, PLAYLIST_NUMBER_WIDTH as u32, row_rect.height),
                    &number,
                    font,
                    ink.with_alpha(120),
                    HorizontalAlignment::Right,
                );
                label_x += PLAYLIST_NUMBER_WIDTH;
            }
            if is_current && self.show_now_playing_badge {
                let side = 8u32.min(row_rect.height.saturating_sub(6)).max(1);
                context.fill_rect(
                    Rect::new(
                        label_x,
                        row_rect.y + (row_rect.height.saturating_sub(side) / 2) as i32,
                        side,
                        side,
                    ),
                    accent,
                );
            }
            label_x += if is_current && self.show_now_playing_badge { 12 } else { 0 };

            let remove = self.remove_rect(row_rect);
            let label_width = (remove.x - 4 - label_x).max(0) as u32;
            let label = self.track_label(track);
            let line = context
                .text_line(Rect::new(label_x, row_rect.y, label_width, row_rect.height), font);
            context.draw_text_fitted_cow(
                Rect::new(label_x, line.y, label_width, line.height),
                &label,
                font,
                if is_current { ink } else { ink.blend(&surface, 0.2) },
                HorizontalAlignment::Left,
            );

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

impl Widget for PlaylistPanel {
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
        Size::new(220, (self.tracks.len() as u32 * self.row_height).max(self.row_height))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "track_selected" => {
                Some(EventSignalRef::mapped("track_selected", &self.track_selected, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "track_move_requested" => Some(EventSignalRef::mapped(
                "track_move_requested",
                &self.track_move_requested,
                |value| {
                    CapabilityValue::Tuple(Vec::from([
                        CapabilityValue::UInt(value.0 as u64),
                        CapabilityValue::UInt(value.1 as u64),
                    ]))
                },
            )),
            "track_remove_requested" => Some(EventSignalRef::mapped(
                "track_remove_requested",
                &self.track_remove_requested,
                |value| CapabilityValue::UInt(*value as u64),
            )),
            _ => None,
        }
    }
}

impl WidgetProperties for PlaylistPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "track_count" => Ok(CapabilityValue::UInt(self.tracks.len() as u64)),
            "current_index" => Ok(match self.current_index {
                Some(index) => CapabilityValue::UInt(index as u64),
                None => CapabilityValue::Null,
            }),
            "show_track_numbers" => Ok(CapabilityValue::Bool(self.show_track_numbers)),
            "show_now_playing_badge" => Ok(CapabilityValue::Bool(self.show_now_playing_badge)),
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height))),
            "title_from_source" => Ok(CapabilityValue::Bool(self.title_from_source)),
            "filter" => Ok(match self.filter() {
                Some(text) => CapabilityValue::String(text.to_string()),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The queue is the player's model, mirrored through `set_tracks`.
            "track_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "current_index" => {
                match value {
                    CapabilityValue::Null => self.set_current_index(None),
                    CapabilityValue::UInt(index) => {
                        self.set_current_index(Some(index as usize));
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "show_track_numbers" => {
                self.set_show_track_numbers(expect_bool(value)?);
                Ok(())
            }
            "show_now_playing_badge" => {
                self.set_show_now_playing_badge(expect_bool(value)?);
                Ok(())
            }
            "row_height" => {
                let CapabilityValue::UInt(height) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_row_height(u32::try_from(height).unwrap_or(u32::MAX));
                Ok(())
            }
            "title_from_source" => {
                self.set_title_from_source(expect_bool(value)?);
                Ok(())
            }
            "filter" => {
                match value {
                    CapabilityValue::Null => self.set_filter(None),
                    capability => {
                        self.set_filter(Some(expect_string(capability)?));
                    }
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "track_count",
            "current_index",
            "show_track_numbers",
            "show_now_playing_badge",
            "row_height",
            "title_from_source",
            "filter",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            // Every command this control publishes needs a track index or text.
            "track_selected" | "set_tracks" | "set_filter" => {
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

    fn panel() -> PlaylistPanel {
        let mut panel = PlaylistPanel::new(Rect::new(0, 0, 240, 100));
        panel.set_tracks(vec![
            "/music/one.mp3".to_string(),
            "/music/two.mp3".to_string(),
            "/music/three.mp3".to_string(),
        ]);
        panel
    }

    #[test]
    fn the_label_is_the_friendly_title_unless_asked_otherwise() {
        let mut panel = panel();
        assert_eq!(panel.track_label(&panel.tracks()[0]), "one");
        panel.set_title_from_source(false);
        assert_eq!(panel.track_label(&panel.tracks()[0]), "/music/one.mp3");
    }

    #[test]
    fn a_filter_narrows_the_list_but_signals_carry_original_indices() {
        let mut panel = panel();
        panel.set_filter(Some("three".to_string()));
        let picked = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&picked);
        panel.track_selected.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        // The only visible row is the third track, whose queue number is 3.
        panel.handle_event(&Event::mouse_press(100, 11, 1));
        assert_eq!(*picked.lock().expect("picked lock"), vec![2]);
        assert_eq!(panel.selected_index(), Some(2));
    }

    #[test]
    fn a_row_drag_requests_a_move_using_original_indices() {
        let mut panel = panel();
        let moved = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = Arc::clone(&moved);
        panel.track_move_requested.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });
        panel.handle_event(&Event::mouse_press(100, 11, 1));
        panel.handle_event(&Event::mouse_release(100, 55, 1));
        assert_eq!(*moved.lock().expect("moved lock"), vec![(0, 2)]);
    }

    #[test]
    fn the_remove_button_reports_the_track() {
        let mut panel = panel();
        let removed = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&removed);
        panel.track_remove_requested.connect(move |index| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*index);
            }
        });
        panel.handle_event(&Event::mouse_press(230, 33, 1));
        assert_eq!(*removed.lock().expect("removed lock"), vec![1]);
    }

    #[test]
    fn the_current_track_is_scrolled_into_view() {
        let mut panel = PlaylistPanel::new(Rect::new(0, 0, 240, 30));
        panel.set_tracks((0..10).map(|index| format!("/music/{index}.mp3")).collect());
        panel.set_current_index(Some(9));
        assert!(panel.scroll_offset > 0, "the playing row must be reachable");
        assert_eq!(panel.current_index(), Some(9));
        panel.set_current_index(Some(42));
        assert_eq!(panel.current_index(), None, "an index past the queue is dropped");
    }
}
