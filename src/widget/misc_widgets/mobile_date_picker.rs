// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MobileDatePicker widget — a mobile-style date picker with year/month/day
//! column spinners.
//!
//! The MobileDatePicker presents three scrollable columns (year, month, day)
//! arranged side-by-side, similar to a mobile OS date picker. Each column
//! shows five vertically stacked values with the current selection highlighted
//! in the center and up/down arrow indicators. Users can scroll through values
//! using the mouse wheel, or click on column values to increment/decrement.
//! A `date_changed` signal is emitted with a "YYYY-MM-DD" formatted string
//! whenever the date changes.

use super::date_utils::{days_in_month, parse_iso_date, MONTH_NAMES};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Mobile-style date picker with year/month/day column spinners.
///
/// Presents three side-by-side columns for selecting year, month, and day.
/// The currently selected value in each column is visually highlighted.
pub struct MobileDatePicker {
    base: BaseWidget,
    year: i32,
    month: u32,
    day: u32,
    /// Emitted with a "YYYY-MM-DD" string when the date changes.
    pub date_changed: Signal1<String>,
}

impl MobileDatePicker {
    /// Creates a new MobileDatePicker with the given geometry.
    ///
    /// Initial date defaults to 2025-01-01.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MobileDatePicker, geometry, "MobileDatePicker"),
            year: 2025,
            month: 1,
            day: 1,
            date_changed: Signal1::new(),
        }
    }

    /// Sets the date, clamping month to 1..=12 and day to the valid range
    /// for the given month/year. Emits `date_changed` if the value actually
    /// changes.
    pub fn set_date(&mut self, year: i32, month: u32, day: u32) {
        let clamped_month = month.clamp(1, 12);
        let max_day = days_in_month(year, clamped_month);
        let clamped_day = day.clamp(1, max_day);
        if self.year != year || self.month != clamped_month || self.day != clamped_day {
            self.year = year;
            self.month = clamped_month;
            self.day = clamped_day;
            self.date_changed.emit(self.date_string());
            self.base.request_redraw();
        }
    }

    /// Returns the current year.
    pub fn year(&self) -> i32 {
        self.year
    }

    /// Returns the current month (1..=12).
    pub fn month(&self) -> u32 {
        self.month
    }

    /// Returns the current day (1..=31, depending on month/year).
    pub fn day(&self) -> u32 {
        self.day
    }

    /// Returns the current date as a "YYYY-MM-DD" formatted string.
    pub fn date_string(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Determines which column (0=year, 1=month, 2=day) contains the given
    /// x-coordinate.
    fn column_at(&self, x: i32) -> usize {
        let rect = self.geometry();
        let col_width = rect.width / 3;
        if x < rect.x {
            return 0;
        }
        let rel_x = (x - rect.x) as u32;
        let col = rel_x / col_width;
        (col as usize).min(2)
    }
}

impl Widget for MobileDatePicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MobileDatePicker`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_date` reports the
/// widget's real date as an ISO `YYYY-MM-DD` string rather than the legacy
/// hardcoded blank. A write parses the same format; malformed text is reported as
/// [`CapabilityAccessError::TypeMismatch`] instead of quietly ignoring the write.
impl WidgetProperties for MobileDatePicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_date" => Ok(CapabilityValue::String(self.date_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_date" => {
                let text = expect_string(value)?;
                let date = parse_iso_date(&text).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_date(date.0, date.1, date.2);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_date", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MobileDatePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the drum, its dividers, its highlight bar and its
        // numbers unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // `mobile_date_picker` is not in the role table, so it classifies as `Surface` and its
        // resolved background is the window fill itself; the drum below therefore derives its own
        // distinct surface rather than painting the window's.
        let theme = crate::theme::resolved_theme_style("mobile_date_picker");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, primary, secondary, disabled) = {
            let manager = crate::theme::global_theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                    active.colors.disabled,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                    Color::rgb(200, 200, 200),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The drum: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. The filter is on the **resolved** value, not
        // only on the theme's: a control classified as `Surface` already carries the window fill,
        // so letting it through unfiltered is exactly the invisible-drum defect this guards
        // against. A caller's own colour still wins.
        let drum_from_theme = window_fill.blend(&ink, 0.06);
        let drum_surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => drum_from_theme,
        };
        let drum = if is_enabled { drum_surface } else { drum_surface.blend(&disabled, 0.35) };
        // The column dividers are a fixed step out of the drum, and the highlight bar behind the
        // selected row is the theme's primary so the selection is distinguishable rather than a
        // translucent fixed blue.
        let divider = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| drum.blend(&secondary, 0.40));
        let highlight = primary.blend(&drum, 0.72);
        // The numbers: the selected row carries the theme's primary ink, the other rows are
        // de-emphasised steps of the separator colour, and a disabled picker recedes from the drum
        // — none of them a fixed grey a dark theme would swallow.
        let selected_ink = if is_enabled { primary } else { drum.blend(&disabled, 0.55) };
        let row_ink = if is_enabled { ink.blend(&drum, 0.45) } else { drum.blend(&disabled, 0.50) };
        let arrow_color = if is_enabled { ink.blend(&drum, 0.20) } else { row_ink };

        // Background
        context.fill_rect(rect, drum);

        // Column layout
        let col_width = rect.width / 3;
        let row_height = rect.height / 5;
        let font_size = (row_height as f32 * 0.38).clamp(10.0, 15.0);
        let font = Font::new("sans-serif", font_size, false, false);
        let arrow_font = Font::new("sans-serif", (font_size * 1.3).max(12.0), true, false);

        // Prepare column data: (offset_into_items, visible_items_list)
        let year_items: Vec<String> =
            (self.year - 2..=self.year + 2).map(|y| y.to_string()).collect();
        let year_offset: i32 = 2; // current year is always at index 2

        let month_items: Vec<String> = MONTH_NAMES.iter().map(|&n| n.to_string()).collect();
        let month_offset = (self.month as usize - 1) as i32;

        let max_day = days_in_month(self.year, self.month);
        let day_items: Vec<String> = (1..=max_day).map(|d| format!("{d:02}")).collect();
        let day_offset = (self.day as usize - 1) as i32;

        let columns: [(i32, Vec<String>, &str); 3] = [
            (year_offset, year_items, "Year"),
            (month_offset, month_items, "Month"),
            (day_offset, day_items, "Day"),
        ];

        for (col_idx, (sel_offset, items, _label)) in columns.iter().enumerate() {
            let col_x = rect.x + (col_idx as u32 * col_width) as i32;

            // Vertical divider between columns
            if col_idx > 0 {
                context.draw_rect_stroke(Rect::new(col_x, rect.y, 1, rect.height), divider, 1);
            }

            // Highlight bar for the center (selected) row
            let highlight_y = rect.y + 2 * row_height as i32;
            let highlight_rect =
                Rect::new(col_x + 4, highlight_y, col_width.saturating_sub(8), row_height);
            context.fill_rounded_rect(highlight_rect, 6, highlight);

            // Draw the five visible rows
            for row in 0..5 {
                let item_idx = row + (sel_offset - 2);
                if item_idx < 0 || item_idx >= items.len() as i32 {
                    continue;
                }

                let text = &items[item_idx as usize];
                let is_selected = row == 2;
                let item_y = rect.y + (row as u32 * row_height) as i32;

                let metrics = context.measure_text(text, &font);
                let text_x = col_x + (col_width as i32 - metrics.width as i32) / 2;
                let text_y = item_y
                    + (row_height as i32 - metrics.height as i32) / 2
                    + metrics.ascent as i32;

                let text_color = if is_selected { selected_ink } else { row_ink };

                context.draw_text(
                    Point::new(text_x, text_y),
                    text,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }

            // Up arrow indicator (top of column)
            let up_y = rect.y + 2;
            context.draw_text(
                Point::new(col_x + (col_width as i32 - 8) / 2, up_y),
                "^",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );

            // Down arrow indicator (bottom of column)
            let down_y = rect.y + rect.height as i32 - row_height as i32 + 2;
            context.draw_text(
                Point::new(col_x + (col_width as i32 - 8) / 2, down_y),
                "v",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for MobileDatePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::Wheel { delta, .. } => {
                // Scroll the year column when vertical wheel is used.
                // MousePress handles per-column interaction.
                if delta.y > 0 {
                    self.set_date(
                        self.year + 1,
                        self.month,
                        self.day.min(days_in_month(self.year + 1, self.month)),
                    );
                } else if delta.y < 0 {
                    self.set_date(
                        self.year - 1,
                        self.month,
                        self.day.min(days_in_month(self.year - 1, self.month)),
                    );
                }
            }
            Event::MousePress { pos, button: 1 } => {
                let col = self.column_at(pos.x);
                let rect = self.geometry();
                let row_height = rect.height / 5;
                let rel_y = pos.y - rect.y;
                let row = rel_y / row_height as i32;
                // Upper half (row 0-2) = increment, lower half (row 3-4) = decrement
                let increment = row <= 2;

                match col {
                    0 => {
                        // Year column
                        let new_year = if increment { self.year + 1 } else { self.year - 1 };
                        let max_day = days_in_month(new_year, self.month);
                        let clamped_day = self.day.min(max_day);
                        self.set_date(new_year, self.month, clamped_day);
                    }
                    1 => {
                        // Month column
                        let new_month = if increment {
                            (self.month as i32 + 1).clamp(1, 12) as u32
                        } else {
                            (self.month as i32 - 1).clamp(1, 12) as u32
                        };
                        let max_day = days_in_month(self.year, new_month);
                        let clamped_day = self.day.min(max_day);
                        self.set_date(self.year, new_month, clamped_day);
                    }
                    2 => {
                        // Day column
                        let max_days = days_in_month(self.year, self.month) as i32;
                        let new_day = if increment {
                            (self.day as i32 + 1).clamp(1, max_days) as u32
                        } else {
                            (self.day as i32 - 1).clamp(1, max_days) as u32
                        };
                        self.set_date(self.year, self.month, new_day);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn make_picker() -> MobileDatePicker {
        MobileDatePicker::new(Rect::new(0, 0, 240, 200))
    }

    #[test]
    fn picker_default_creation() {
        let picker = make_picker();
        assert_eq!(picker.kind(), WidgetKind::MobileDatePicker);
        assert_eq!(picker.year(), 2025);
        assert_eq!(picker.month(), 1);
        assert_eq!(picker.day(), 1);
        assert_eq!(picker.date_string(), "2025-01-01");
        assert!(picker.is_visible());
        assert!(picker.is_enabled());
        assert_eq!(picker.geometry(), Rect::new(0, 0, 240, 200));
    }

    #[test]
    fn picker_set_date_changes_values() {
        let mut picker = make_picker();
        picker.set_date(2026, 12, 25);
        assert_eq!(picker.year(), 2026);
        assert_eq!(picker.month(), 12);
        assert_eq!(picker.day(), 25);
        assert_eq!(picker.date_string(), "2026-12-25");
    }

    #[test]
    fn picker_set_date_clamps_month_out_of_range() {
        let mut picker = make_picker();
        picker.set_date(2025, 0, 1);
        assert_eq!(picker.month(), 1);

        picker.set_date(2025, 13, 1);
        assert_eq!(picker.month(), 12);
    }

    #[test]
    fn picker_set_date_clamps_day_out_of_range() {
        let mut picker = make_picker();
        // January has 31 days, so day 35 should clamp to 31
        picker.set_date(2025, 1, 35);
        assert_eq!(picker.day(), 31);
        assert_eq!(picker.date_string(), "2025-01-31");

        // February in non-leap year has 28 days
        picker.set_date(2025, 2, 30);
        assert_eq!(picker.day(), 28);
        assert_eq!(picker.date_string(), "2025-02-28");
    }

    #[test]
    fn picker_set_date_handles_leap_year() {
        let mut picker = make_picker();
        // 2024 is a leap year
        picker.set_date(2024, 2, 29);
        assert_eq!(picker.day(), 29);
        assert_eq!(picker.date_string(), "2024-02-29");

        // 2025 is not a leap year - should clamp
        picker.set_date(2025, 2, 29);
        assert_eq!(picker.day(), 28);
        assert_eq!(picker.date_string(), "2025-02-28");
    }

    #[test]
    fn picker_set_date_same_value_no_emit() {
        let mut picker = make_picker();
        let emitted = Arc::new(AtomicBool::new(false));
        let e = emitted.clone();
        picker.date_changed.connect(move |_val: Arc<String>| {
            e.store(true, Ordering::SeqCst);
        });

        // Set same date - should NOT emit
        picker.set_date(2025, 1, 1);
        assert!(!emitted.load(Ordering::SeqCst));

        // Set different date - SHOULD emit
        picker.set_date(2025, 3, 15);
        assert!(emitted.load(Ordering::SeqCst));
    }

    #[test]
    fn picker_set_date_emits_signal() {
        let mut picker = make_picker();
        let captured = Arc::new(std::sync::Mutex::new(None));
        let c = captured.clone();
        picker.date_changed.connect(move |val: Arc<String>| {
            *c.lock().unwrap() = Some(val.to_string());
        });

        picker.set_date(2026, 7, 4);
        let result = captured.lock().unwrap().clone();
        assert_eq!(result, Some("2026-07-04".to_string()));
    }

    #[test]
    fn picker_individual_accessors() {
        let picker = make_picker();
        assert_eq!(picker.year(), 2025);
        assert_eq!(picker.month(), 1);
        assert_eq!(picker.day(), 1);
    }

    #[test]
    fn picker_date_string_format() {
        let mut picker = make_picker();
        assert_eq!(picker.date_string(), "2025-01-01");

        picker.set_date(1999, 12, 31);
        assert_eq!(picker.date_string(), "1999-12-31");

        picker.set_date(2024, 2, 9);
        assert_eq!(picker.date_string(), "2024-02-09");
    }

    #[test]
    fn picker_column_at_returns_correct_column() {
        let picker = make_picker();
        // Column width = 240 / 3 = 80
        assert_eq!(picker.column_at(10), 0); // Year column
        assert_eq!(picker.column_at(85), 1); // Month column
        assert_eq!(picker.column_at(170), 2); // Day column
        assert_eq!(picker.column_at(250), 2); // Clamped
        assert_eq!(picker.column_at(-5), 0); // Before start -> column 0
    }

    #[test]
    fn picker_mouse_press_year_column() {
        let mut picker = make_picker();
        // Click upper half of year column -> increment year
        picker.handle_event(&Event::MousePress { pos: Point::new(20, 30), button: 1 });
        assert_eq!(picker.year(), 2026);

        // Click lower half of year column -> decrement year
        picker.handle_event(&Event::MousePress { pos: Point::new(20, 150), button: 1 });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_mouse_press_month_column() {
        let mut picker = make_picker();
        // Click upper half of month column -> increment month
        picker.handle_event(&Event::MousePress { pos: Point::new(100, 30), button: 1 });
        assert_eq!(picker.month(), 2);

        // Click lower half -> decrement month
        picker.handle_event(&Event::MousePress { pos: Point::new(100, 150), button: 1 });
        assert_eq!(picker.month(), 1);
    }

    #[test]
    fn picker_mouse_press_day_column() {
        let mut picker = make_picker();
        // Click upper half of day column -> increment day
        picker.handle_event(&Event::MousePress { pos: Point::new(180, 30), button: 1 });
        assert_eq!(picker.day(), 2);

        // Click lower half -> decrement day
        picker.handle_event(&Event::MousePress { pos: Point::new(180, 150), button: 1 });
        assert_eq!(picker.day(), 1);
    }

    #[test]
    fn picker_wheel_scrolls_year() {
        let mut picker = make_picker();
        // Scroll up (delta.y > 0) -> increment year
        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.year(), 2026);

        // Scroll down (delta.y < 0) -> decrement year
        picker.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_disabled_blocks_events() {
        let mut picker = make_picker();
        picker.set_enabled(false);

        picker.handle_event(&Event::MousePress { pos: Point::new(20, 30), button: 1 });
        assert_eq!(picker.year(), 2025);

        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_other_button_noop() {
        let mut picker = make_picker();
        picker.handle_event(&Event::MousePress { pos: Point::new(20, 30), button: 2 });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_day_clamp_on_year_change() {
        // When changing year from leap to non-leap with Feb 29
        let mut picker = make_picker();
        picker.set_date(2024, 2, 29);
        assert_eq!(picker.day(), 29);

        // Scroll to 2025 - day should clamp to 28
        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.year(), 2025);
        assert_eq!(picker.day(), 28);
    }

    #[test]
    fn picker_day_wraps_correctly() {
        let mut picker = make_picker();
        picker.set_date(2025, 1, 31);

        // Change to February - day should clamp to 28
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 30), // upper half of month column -> increment
            button: 1,
        });
        assert_eq!(picker.month(), 2);
        assert_eq!(picker.day(), 28);
    }

    #[test]
    fn picker_svg_output() {
        let mut picker = make_picker();
        picker.set_date(2026, 7, 4);
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"240\""));
        assert!(svg.contains("height=\"200\""));
    }

    #[test]
    fn picker_svg_output_disabled() {
        let mut picker = make_picker();
        picker.set_enabled(false);
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn picker_day_edge_cases() {
        let mut picker = make_picker();

        // Test day = 1, decrement should stay at 1
        picker.set_date(2025, 1, 1);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 150), // lower half of day column -> decrement
            button: 1,
        });
        assert_eq!(picker.day(), 1);

        // Test max day of a 31-day month, increment should stay at 31
        picker.set_date(2025, 1, 31);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 30), // upper half of day column -> increment
            button: 1,
        });
        assert_eq!(picker.day(), 31);
    }

    #[test]
    fn picker_month_edge_cases() {
        let mut picker = make_picker();

        // January, decrement month -> should stay at 1
        picker.set_date(2025, 1, 15);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 150), // lower half of month column -> decrement
            button: 1,
        });
        assert_eq!(picker.month(), 1);

        // December, increment month -> should stay at 12
        picker.set_date(2025, 12, 15);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 30), // upper half of month column -> increment
            button: 1,
        });
        assert_eq!(picker.month(), 12);
    }
}
