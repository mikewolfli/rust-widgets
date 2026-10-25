// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DrumColumn` — one wheel column of a mobile-style picker.
//!
//! # Why this is its own control
//!
//! A drum (a fixed-row wheel with the selection held in the middle row) is an interaction, not a
//! date picker: a time picker, a quantity stepper and an enum chooser all want it. The row layout,
//! the highlight bar, the arrow hints and the wheel/click stepping were all inline inside
//! `mobile_date_picker`, so none of them could be reused and each new picker would have copied them
//! (COMP-K02).
//!
//! # What it owns
//!
//! The **items and the selected index**. It does not own the meaning of the selection: a picker that
//! composes three columns reads [`DrumColumn::selected_index`] and maps it back to a year/month/day,
//! which is where the date-specific clamping stays.

use crate::compat::Vec;
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The picker drum metrics the column defaults to.
const DEFAULT_VISIBLE_ROWS: u32 = dimensions::PICKER_VISIBLE_ROWS;
/// The default row height, from the shared picker metrics.
const DEFAULT_ROW_HEIGHT: u32 = dimensions::PICKER_ROW_HEIGHT;

/// One wheel column: a vertical list of values with the selected row held in the middle.
pub struct DrumColumn {
    base: BaseWidget,
    items: Vec<String>,
    /// The selected item index, always inside `items` while the list is non-empty.
    selected_index: usize,
    /// How many rows are visible, including the selected one.
    visible_rows: u32,
    /// The height of one row, in logical pixels.
    row_height: u32,
    /// Whether stepping past an end wraps to the other end.
    loop_wrap: bool,
    /// Explicit divider colour, or the theme's own derived step.
    divider_color: Option<Color>,
    /// Explicit highlight-bar colour, or the theme's primary.
    highlight_color: Option<Color>,
    /// Explicit row ink, or the theme's foreground.
    ink: Option<Color>,
    /// Emitted with the new index when the selection moves.
    pub selection_changed: Signal1<usize>,
}

impl DrumColumn {
    /// Creates an empty column with the picker's default metrics.
    ///
    /// The control is registered under `WidgetKind::MobileDatePicker`: a drum column is the
    /// building block of exactly that control, and the kind table is outside this module's scope,
    /// so the closest existing kind is used rather than inventing a variant no dispatcher knows.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MobileDatePicker, geometry, "DrumColumn"),
            items: Vec::new(),
            selected_index: 0,
            visible_rows: DEFAULT_VISIBLE_ROWS,
            row_height: DEFAULT_ROW_HEIGHT,
            loop_wrap: false,
            divider_color: None,
            highlight_color: None,
            ink: None,
            selection_changed: Signal1::new(),
        }
    }

    /// The column's values, in wheel order.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Replaces the values and clamps the selection into the new list.
    ///
    /// Replacing the list does not emit `selection_changed` when the index survives unchanged; a
    /// caller that also changed the list is already repainting.
    pub fn set_items(&mut self, items: Vec<String>) {
        if self.items == items {
            return;
        }
        self.items = items;
        self.selected_index = self.selected_index.min(self.max_index());
        self.base.request_redraw();
    }

    /// The selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Selects `index`, applying the wrap rule and clamping into the list.
    ///
    /// Returns whether the selection moved. An out-of-range index is clamped (or wrapped when
    /// [`Self::loop_wrap`] is set) rather than refused, because the wheel's edge behaviour is the
    /// column's own decision and a caller that has just shrunk the list should not have to know it.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let count = self.items.len();
        let next = if self.loop_wrap { index % count } else { index.min(count - 1) };
        if next == self.selected_index {
            return false;
        }
        self.selected_index = next;
        self.selection_changed.emit(next);
        self.base.request_redraw();
        true
    }

    /// How many rows the wheel shows.
    pub fn visible_rows(&self) -> u32 {
        self.visible_rows
    }

    /// Sets how many rows the wheel shows (floored at 3, so a selection is centred).
    pub fn set_visible_rows(&mut self, rows: u32) {
        let rows = rows.max(3);
        if self.visible_rows == rows {
            return;
        }
        self.visible_rows = rows;
        self.base.request_redraw();
    }

    /// The height of one row.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the row height (floored at 1 px).
    pub fn set_row_height(&mut self, height: u32) {
        let height = height.max(1);
        if self.row_height == height {
            return;
        }
        self.row_height = height;
        self.base.request_redraw();
    }

    /// Whether stepping past an end wraps around.
    pub fn loop_wrap(&self) -> bool {
        self.loop_wrap
    }

    /// Sets the wrap rule.
    pub fn set_loop_wrap(&mut self, wrap: bool) {
        if self.loop_wrap == wrap {
            return;
        }
        self.loop_wrap = wrap;
        self.base.request_redraw();
    }

    /// The explicit divider colour, if one was set.
    pub fn divider_color(&self) -> Option<Color> {
        self.divider_color
    }

    /// Sets the divider colour; `None` returns it to the theme's derived step.
    pub fn set_divider_color(&mut self, color: Option<Color>) {
        if self.divider_color == color {
            return;
        }
        self.divider_color = color;
        self.base.request_redraw();
    }

    /// The explicit highlight-bar colour, if one was set.
    pub fn highlight_color(&self) -> Option<Color> {
        self.highlight_color
    }

    /// Sets the highlight-bar colour; `None` returns it to the theme's primary.
    pub fn set_highlight_color(&mut self, color: Option<Color>) {
        if self.highlight_color == color {
            return;
        }
        self.highlight_color = color;
        self.base.request_redraw();
    }

    /// The explicit row ink, if one was set.
    pub fn ink(&self) -> Option<Color> {
        self.ink
    }

    /// Sets the row ink; `None` returns it to the theme's foreground.
    pub fn set_ink(&mut self, color: Option<Color>) {
        if self.ink == color {
            return;
        }
        self.ink = color;
        self.base.request_redraw();
    }

    /// Shifts the selection by `delta` rows, honouring [`Self::loop_wrap`].
    ///
    /// This is the entry point the [`Event::Wheel`] arm and a composing picker's keyboard path both
    /// use, so every way of stepping a column behaves the same.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        if self.items.is_empty() || delta == 0 {
            return false;
        }
        let count = self.items.len() as i64;
        let current = self.selected_index as i64;
        let raw = current + i64::from(delta);
        let next = if self.loop_wrap { raw.rem_euclid(count) } else { raw.clamp(0, count - 1) };
        self.set_selected_index(next as usize)
    }

    /// The visible row index under `point`, or `None` when the point is outside the drum.
    ///
    /// A composing picker uses this to tell which row a press landed on; the row index is relative
    /// to the drum (row 0 is the top visible row), which is what the click stepping needs.
    pub fn row_at(&self, point: Point) -> Option<usize> {
        let drum = self.drum_rect();
        if !drum.contains_point(point) {
            return None;
        }
        let row_height = self.effective_row_height(drum);
        let row = (point.y - drum.y).max(0) as u32 / row_height;
        Some((row as usize).min(self.visible_rows as usize - 1))
    }

    /// The largest valid index, or `0` for an empty list.
    fn max_index(&self) -> usize {
        self.items.len().saturating_sub(1)
    }

    /// The band the drum occupies: a fixed number of rows, centred in the control.
    fn drum_rect(&self) -> Rect {
        let rect = self.geometry();
        ControlMetrics::center_in(rect, Size::new(rect.width, self.row_height * self.visible_rows))
    }

    /// The row height actually painted: the configured one, squeezed if the band is shorter.
    fn effective_row_height(&self, drum: Rect) -> u32 {
        (drum.height / self.visible_rows.max(1)).max(1)
    }

    /// The resolved chrome: `(drum fill, divider, highlight, selected ink, row ink, arrow ink)`.
    fn resolve_chrome(&self, style: &crate::style::WidgetStyle) -> DrumChrome {
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        let primary = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let secondary =
            palette.as_ref().map(|colors| colors.secondary).unwrap_or(Color::rgb(158, 158, 158));
        let foreground = palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK);
        let ink = style.text_color.or(self.ink).unwrap_or(foreground);
        // One visible step off the window, so the wheel is a distinct surface on either appearance.
        let drum = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let divider = self.divider_color.unwrap_or_else(|| drum.blend(&secondary, 0.40));
        let highlight = self.highlight_color.unwrap_or_else(|| primary.blend(&drum, 0.72));
        DrumChrome {
            drum,
            divider,
            highlight,
            selected_ink: highlight.contrast_color(),
            row_ink: ink.blend(&drum, 0.45),
            arrow_ink: ink.blend(&drum, 0.20),
        }
    }
}

/// The resolved colours one drum paint uses.
struct DrumChrome {
    /// The drum's own surface.
    drum: Color,
    /// The rule between the drum and the page.
    divider: Color,
    /// The selected row's bar.
    highlight: Color,
    /// The selected row's ink, chosen against the bar.
    selected_ink: Color,
    /// The other rows' ink.
    row_ink: Color,
    /// The arrow hints' ink.
    arrow_ink: Color,
}

impl Widget for DrumColumn {
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
        Size::new(80, self.row_height * self.visible_rows)
    }
    /// A wheel is a keyboard target: the arrows step it.
    fn is_focusable(&self) -> bool {
        true
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl EventHandler for DrumColumn {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // A notch is one row; the event's positive y means "towards the end of the list".
            Event::Wheel { delta, .. } => {
                if delta.y > 0 {
                    self.scroll_by(1);
                } else if delta.y < 0 {
                    self.scroll_by(-1);
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                // The upper half steps back and the lower half steps forward, which is the mobile
                // picker's own rule and the reason the selected row is the middle one.
                if let Some(row) = self.row_at(*pos) {
                    if row as u32 <= self.visible_rows / 2 {
                        self.scroll_by(-1);
                    } else {
                        self.scroll_by(1);
                    }
                }
            }
            Event::KeyPress { key, .. } => match *key {
                crate::event::key_codes::UP => {
                    self.scroll_by(-1);
                }
                crate::event::key_codes::DOWN => {
                    self.scroll_by(1);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for DrumColumn {
    fn draw(&mut self, context: &mut RenderContext) {
        let drum_rect = self.drum_rect();
        if drum_rect.width == 0 || drum_rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let chrome = self.resolve_chrome(style);
        context.fill_rect(drum_rect, chrome.drum);
        let row_height = self.effective_row_height(drum_rect);
        let font_size = (row_height as f32 * 0.38).clamp(9.0, 15.0);
        let font = role_font(style, font_size);
        let arrow_font = role_font(style, (font_size * 1.3).max(10.0)).with_bold(true);
        let text_height = context.measure_text("M", &font).height;

        // The selected row's bar.
        let highlight_y = drum_rect.y + (self.visible_rows as i32 / 2) * row_height as i32;
        let highlight_rect =
            Rect::new(drum_rect.x + 4, highlight_y, drum_rect.width.saturating_sub(8), row_height);
        context.fill_rounded_rect(highlight_rect, 6, chrome.highlight);

        // The visible window, centred on the selection.
        let centre = self.visible_rows / 2;
        for row in 0..self.visible_rows {
            let offset = row as i64 - centre as i64;
            let Some(index) = self.index_for_offset(offset) else {
                continue;
            };
            let Some(text) = self.items.get(index) else {
                continue;
            };
            let row_y = drum_rect.y + (row * row_height) as i32;
            let text_y = row_y + (row_height as i32 - text_height as i32) / 2;
            let color = if row == centre { chrome.selected_ink } else { chrome.row_ink };
            context.draw_text_fitted_cow(
                Rect::new(drum_rect.x, text_y, drum_rect.width, text_height),
                text,
                &font,
                color,
                HorizontalAlignment::Center,
            );
        }

        // Up/down hints, on the drum rather than on the control's own bottom edge.
        context.draw_text(
            Point::new(drum_rect.x + (drum_rect.width as i32 - 8) / 2, drum_rect.y + 2),
            "^",
            &arrow_font,
            chrome.arrow_ink,
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(
                drum_rect.x + (drum_rect.width as i32 - 8) / 2,
                drum_rect.y + drum_rect.height as i32 - row_height as i32 + 2,
            ),
            "v",
            &arrow_font,
            chrome.arrow_ink,
            HorizontalAlignment::Left,
        );

        context.draw_rect(drum_rect, chrome.divider);
    }
}

impl DrumColumn {
    /// The item index at `offset` rows from the selection, honouring the wrap rule.
    ///
    /// Returns `None` where the wheel has no row: without wrapping, the rows past an end are empty
    /// rather than repeating the last item, which is what makes an end visible.
    fn index_for_offset(&self, offset: i64) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let count = self.items.len() as i64;
        let raw = self.selected_index as i64 + offset;
        if self.loop_wrap {
            Some(raw.rem_euclid(count) as usize)
        } else if raw < 0 || raw >= count {
            None
        } else {
            Some(raw as usize)
        }
    }
}

impl WidgetProperties for DrumColumn {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.items.len() as u64)),
            "loop_wrap" => Ok(CapabilityValue::Bool(self.loop_wrap)),
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height))),
            "items" => Ok(CapabilityValue::Tuple(
                self.items.iter().map(|item| CapabilityValue::String(item.clone())).collect(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                let index = crate::widget::capability::coercion::expect_u32(value)? as usize;
                self.set_selected_index(index);
                Ok(())
            }
            "loop_wrap" => {
                self.set_loop_wrap(expect_bool(value)?);
                Ok(())
            }
            "row_height" => {
                let height = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_row_height(height);
                Ok(())
            }
            "items" => {
                let CapabilityValue::Tuple(values) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                let mut items = Vec::with_capacity(values.len());
                for value in values {
                    let CapabilityValue::String(text) = value else {
                        return Err(CapabilityAccessError::TypeMismatch);
                    };
                    items.push(text);
                }
                self.set_items(items);
                Ok(())
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "selected_index",
            "item_count",
            "loop_wrap",
            "row_height",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column() -> DrumColumn {
        let mut column = DrumColumn::new(Rect::new(0, 0, 80, 200));
        column.set_items((1..=5).map(|n| format!("{n:02}")).collect());
        column
    }

    #[test]
    fn stepping_clamps_without_wrap() {
        let mut column = column();
        assert!(column.scroll_by(1));
        assert_eq!(column.selected_index(), 1);
        column.set_selected_index(0);
        assert!(!column.scroll_by(-1), "clamped at the first item");
        assert_eq!(column.selected_index(), 0);
    }

    #[test]
    fn stepping_wraps_when_asked() {
        let mut column = column();
        column.set_loop_wrap(true);
        column.set_selected_index(4);
        assert!(column.scroll_by(1));
        assert_eq!(column.selected_index(), 0);
    }

    #[test]
    fn the_selection_signal_fires_only_on_a_change() {
        use std::sync::{Arc, Mutex};
        let mut column = column();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        column.selection_changed.connect(move |index| sink.lock().unwrap().push(*index));
        assert!(column.set_selected_index(2));
        assert!(!column.set_selected_index(2));
        assert_eq!(seen.lock().unwrap().as_slice(), &[2]);
    }

    #[test]
    fn a_row_resolves_under_the_pointer() {
        let column = column();
        // Five 32 px rows (160 px) centred in a 200 px box start at y = 20.
        assert_eq!(column.row_at(Point::new(40, 25)), Some(0));
        assert_eq!(column.row_at(Point::new(40, 90)), Some(2));
        assert_eq!(column.row_at(Point::new(40, 170)), Some(4));
        assert_eq!(column.row_at(Point::new(40, 400)), None);
    }

    #[test]
    fn shrinking_the_list_clamps_the_selection() {
        let mut column = column();
        column.set_selected_index(4);
        column.set_items(vec!["only".to_string()]);
        assert_eq!(column.selected_index(), 0);
    }
}
