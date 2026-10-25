// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `RecentValuesPopup` — the recently committed values of an input field (COMP-B03).
//!
//! # Why this is a companion and not a field feature
//!
//! Repeated entry is what a command box, a path field or a find bar is used for, and the family had
//! no memory of it: `auto_complete_edit`'s suggestions are a list the host maintains through
//! `add_suggestion`/`remove_suggestion`, and nothing remembered what had actually been **committed**.
//! This popup is that memory — a de-duplicated, capacity-bounded history with a pick list — mounted
//! beside the field, exactly as `SuggestionPopup` is.
//!
//! It shares `SuggestionPopup`'s row rhythm and empty-state text through
//! `popup_support`, so the two popups in this family cannot drift into two
//! different row heights.

use crate::compat::{String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_string, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::popup_support::{
    expect_rect, expect_strings, rect_value, row_height_for, strings_value,
};
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// How many values a popup remembers by default.
const DEFAULT_CAPACITY: usize = 10;
/// Horizontal inset inside a row.
const ROW_INSET: i32 = 8;

/// A bounded, de-duplicated list of recently committed values.
pub struct RecentValuesPopup {
    base: BaseWidget,
    /// Newest first, each value appearing once.
    entries: Vec<String>,
    capacity: usize,
    anchor: Option<Rect>,
    selected_index: Option<usize>,
    /// A caller-chosen row height, or `None` to derive it from the font.
    row_height: Option<u32>,
    empty_text: String,
    shown: bool,
    /// Emitted with the value the user took from the list.
    pub entry_picked: Signal1<String>,
    /// Emitted when the history is emptied.
    pub history_cleared: GenericSignal,
}

impl RecentValuesPopup {
    /// Creates an empty history popup.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "RecentValuesPopup"),
            entries: Vec::new(),
            capacity: DEFAULT_CAPACITY,
            anchor: None,
            selected_index: None,
            row_height: None,
            empty_text: String::from("(No matches)"),
            shown: false,
            entry_picked: Signal1::new(),
            history_cleared: GenericSignal::new(),
        }
    }

    /// Remembers `value` as the newest entry.
    ///
    /// A repeat moves the existing entry to the front rather than duplicating it, and the list is
    /// truncated to [`Self::capacity`]. An empty value is not remembered: it is not something the
    /// user could pick again.
    pub fn remember(&mut self, value: &str) {
        if value.is_empty() {
            return;
        }
        if let Some(index) = self.entries.iter().position(|entry| entry == value) {
            self.entries.remove(index);
        }
        self.entries.insert(0, value.to_string());
        self.entries.truncate(self.capacity.max(1));
        self.clamp_highlight();
        self.base.request_redraw();
    }

    /// Forgets one value, returning whether it was there.
    pub fn forget(&mut self, value: &str) -> bool {
        let Some(index) = self.entries.iter().position(|entry| entry == value) else {
            return false;
        };
        self.entries.remove(index);
        self.clamp_highlight();
        self.base.request_redraw();
        true
    }

    /// Empties the history.
    pub fn clear(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.entries.clear();
        self.selected_index = None;
        self.shown = false;
        self.base.request_redraw();
        self.history_cleared.emit();
    }

    /// Caps the remembered count, dropping the oldest entries.
    pub fn set_capacity(&mut self, capacity: usize) {
        let capacity = capacity.max(1);
        if self.capacity == capacity {
            return;
        }
        self.capacity = capacity;
        self.entries.truncate(capacity);
        self.clamp_highlight();
        self.base.request_redraw();
    }

    /// The remembered count cap.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The remembered values, newest first.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Anchors the popup under the field it serves.
    pub fn set_anchor(&mut self, anchor: Rect) {
        if self.anchor == Some(anchor) {
            return;
        }
        self.anchor = Some(anchor);
        self.base.request_redraw();
    }

    /// The field band the popup hangs from.
    pub fn anchor(&self) -> Option<Rect> {
        self.anchor
    }

    /// Shows the popup, highlighting the newest entry when nothing is highlighted yet.
    pub fn show(&mut self) {
        if self.shown {
            return;
        }
        self.shown = true;
        if self.selected_index.is_none() && !self.entries.is_empty() {
            self.selected_index = Some(0);
        }
        self.base.request_redraw();
    }

    /// Hides the popup.
    pub fn hide(&mut self) {
        if !self.shown {
            return;
        }
        self.shown = false;
        self.base.request_redraw();
    }

    /// Whether the popup is showing.
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// Moves the highlight by `delta` rows, returning whether it moved.
    pub fn move_highlight(&mut self, delta: i32) -> bool {
        let rows = self.entries.len();
        if rows == 0 {
            return false;
        }
        let last = rows - 1;
        let next = match self.selected_index {
            Some(index) => (index as i32 + delta).clamp(0, last as i32) as usize,
            None => {
                if delta < 0 {
                    last
                } else {
                    0
                }
            }
        };
        if self.selected_index == Some(next) {
            return false;
        }
        self.selected_index = Some(next);
        self.base.request_redraw();
        true
    }

    /// Takes the highlighted entry: emits `entry_picked` and hides.
    pub fn commit_highlighted(&mut self) -> bool {
        // A hidden popup has nothing on screen to take.
        if !self.shown {
            return false;
        }
        let Some(index) = self.selected_index.filter(|index| *index < self.entries.len()) else {
            return false;
        };
        let value = self.entries[index].clone();
        self.entry_picked.emit(value);
        self.hide();
        true
    }

    /// Sets a fixed row height, or restores the font-derived one with `None`.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = Some(row_height.max(1));
        if self.row_height == next {
            return;
        }
        self.row_height = next;
        self.base.request_redraw();
    }

    /// Replaces the text painted when the history is empty.
    pub fn set_empty_text(&mut self, text: &str) {
        if self.empty_text == text {
            return;
        }
        self.empty_text = text.to_string();
        self.base.request_redraw();
    }

    /// The text painted when the history is empty.
    pub fn empty_text(&self) -> &str {
        &self.empty_text
    }

    /// The height of one painted row, shared with `SuggestionPopup`.
    pub fn row_height(&self) -> u32 {
        self.row_height.unwrap_or_else(|| row_height_for(effective_font(self.base.style())))
    }

    /// The row under `pos`, when the popup is showing and the point is on one.
    pub fn row_at(&self, pos: Point) -> Option<usize> {
        if !self.shown {
            return None;
        }
        (0..self.entries.len())
            .find(|row| self.row_rect(*row).is_some_and(|rect| rect.contains(pos)))
    }

    /// Keeps the highlight inside the list after a removal or a truncation.
    fn clamp_highlight(&mut self) {
        self.selected_index = match self.selected_index {
            Some(_) if self.entries.is_empty() => None,
            Some(index) => Some(index.min(self.entries.len() - 1)),
            None => None,
        };
    }

    /// The popup's own box: rows tall, hanging from the anchor when one is set.
    fn popup_rect(&self) -> Rect {
        let rows = self.entries.len().max(1) as u32;
        let height = rows.saturating_mul(self.row_height());
        match self.anchor {
            Some(anchor) => {
                Rect::new(anchor.x, anchor.y + anchor.height as i32, anchor.width, height)
            }
            None => {
                let geometry = self.geometry();
                Rect::new(geometry.x, geometry.y, geometry.width, height)
            }
        }
    }

    /// The box of one painted row.
    fn row_rect(&self, row: usize) -> Option<Rect> {
        if row >= self.entries.len() {
            return None;
        }
        let popup = self.popup_rect();
        Some(Rect::new(
            popup.x,
            popup.y + row as i32 * self.row_height() as i32,
            popup.width,
            self.row_height(),
        ))
    }
}

impl EventHandler for RecentValuesPopup {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.shown {
            return;
        }
        match event {
            Event::KeyPress { key, .. } => match *key {
                38 => {
                    self.move_highlight(-1);
                }
                40 => {
                    self.move_highlight(1);
                }
                36 => self.selected_index = (!self.entries.is_empty()).then_some(0),
                35 => self.selected_index = self.entries.len().checked_sub(1),
                13 => {
                    self.commit_highlighted();
                }
                27 => self.hide(),
                _ => {}
            },
            Event::MousePress { pos, button: 1, .. } => match self.row_at(*pos) {
                Some(row) => {
                    self.selected_index = Some(row);
                    self.commit_highlighted();
                }
                None => self.hide(),
            },
            Event::FocusLost => self.hide(),
            _ => {}
        }
    }
}

impl Draw for RecentValuesPopup {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.shown {
            return;
        }
        let popup = self.popup_rect();
        if popup.width == 0 || popup.height == 0 {
            return;
        }
        let style = self.style();
        let font = effective_font(style);
        let surface = style.background_color.unwrap_or(Color::WHITE);
        let ink = style.text_color.unwrap_or_else(|| surface.contrast_color());
        let border = style.border_color.unwrap_or_else(|| surface.blend(&ink, 0.25));
        let accent = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|theme| theme.colors.primary).unwrap_or(Color::PRIMARY)
        };

        context.fill_rounded_rect(popup, 2, surface);
        context.draw_rounded_rect_stroke(popup, 2, border, 1);

        if self.entries.is_empty() {
            let row = Rect::new(popup.x, popup.y, popup.width, self.row_height());
            let line = context.text_line(row, font);
            context.draw_text_fitted_cow(
                Rect::new(
                    row.x + ROW_INSET,
                    line.y,
                    row.width.saturating_sub(ROW_INSET as u32 * 2),
                    line.height.max(1),
                ),
                &self.empty_text,
                font,
                ink.with_alpha(120),
                HorizontalAlignment::Left,
            );
            return;
        }

        for row in 0..self.entries.len() {
            let Some(rect) = self.row_rect(row) else { continue };
            let highlighted = self.selected_index == Some(row);
            if highlighted {
                context.fill_rect(rect, accent.with_alpha(40));
            }
            let Some(entry) = self.entries.get(row) else { continue };
            let line = context.text_line(rect, font);
            context.draw_text_fitted_cow(
                Rect::new(
                    rect.x + ROW_INSET,
                    line.y,
                    rect.width.saturating_sub(ROW_INSET as u32 * 2),
                    line.height.max(1),
                ),
                entry,
                font,
                if highlighted { ink } else { ink.with_alpha(230) },
                HorizontalAlignment::Left,
            );
        }
    }
}

impl Widget for RecentValuesPopup {
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
        Size::new(200, self.row_height().saturating_mul(self.entries.len().max(1) as u32))
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "entry_picked" => {
                Some(EventSignalRef::mapped("entry_picked", &self.entry_picked, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "history_cleared" => {
                Some(EventSignalRef::unit("history_cleared", &self.history_cleared))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for RecentValuesPopup {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "entries" => Ok(strings_value(&self.entries)),
            "capacity" => Ok(CapabilityValue::UInt(self.capacity as u64)),
            "anchor" => match self.anchor {
                Some(anchor) => Ok(rect_value(anchor)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_index" => match self.selected_index {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height()))),
            "empty_text" => Ok(CapabilityValue::String(self.empty_text.clone())),
            "shown" => Ok(CapabilityValue::Bool(self.shown)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "entries" => {
                let mut entries = expect_strings(value)?;
                entries.dedup();
                entries.truncate(self.capacity);
                self.entries = entries;
                self.clamp_highlight();
                self.base.request_redraw();
                Ok(())
            }
            "capacity" => {
                self.set_capacity(expect_usize(value)?);
                Ok(())
            }
            "anchor" => match value {
                CapabilityValue::Null => {
                    self.anchor = None;
                    self.base.request_redraw();
                    Ok(())
                }
                other => {
                    self.set_anchor(expect_rect(other)?);
                    Ok(())
                }
            },
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.selected_index = None;
                    self.base.request_redraw();
                    Ok(())
                }
                other => {
                    let index = expect_usize(other)?;
                    self.selected_index = (index < self.entries.len()).then_some(index);
                    self.base.request_redraw();
                    Ok(())
                }
            },
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "empty_text" => {
                let text = expect_string(value)?;
                self.set_empty_text(&text);
                Ok(())
            }
            "shown" => {
                if matches!(value, CapabilityValue::Bool(true)) {
                    self.show();
                } else {
                    self.hide();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "entries",
            "capacity",
            "anchor",
            "selected_index",
            "row_height",
            "empty_text",
            "shown",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popup() -> RecentValuesPopup {
        RecentValuesPopup::new(Rect::new(0, 0, 200, 1))
    }

    #[test]
    fn remembering_deduplicates_and_puts_the_newest_first() {
        let mut popup = popup();
        popup.remember("alpha");
        popup.remember("beta");
        popup.remember("alpha");
        assert_eq!(popup.entries(), ["alpha".to_string(), "beta".to_string()]);
        assert!(!popup.forget("gamma"));
        assert!(popup.forget("beta"));
        assert_eq!(popup.entries(), ["alpha".to_string()]);
    }

    #[test]
    fn the_capacity_drops_the_oldest_entries() {
        let mut popup = popup();
        popup.set_capacity(2);
        popup.remember("one");
        popup.remember("two");
        popup.remember("three");
        assert_eq!(popup.entries(), ["three".to_string(), "two".to_string()]);
    }

    #[test]
    fn committing_the_highlight_emits_the_value() {
        let mut popup = popup();
        popup.remember("alpha");
        popup.show();
        assert!(popup.commit_highlighted());
        assert!(!popup.is_shown());
    }

    #[test]
    fn clearing_empties_the_history() {
        let mut popup = popup();
        popup.remember("alpha");
        popup.clear();
        assert!(popup.entries().is_empty());
        assert!(!popup.is_shown());
    }

    #[test]
    fn an_empty_value_is_not_remembered() {
        let mut popup = popup();
        popup.remember("");
        assert!(popup.entries().is_empty());
    }
}
