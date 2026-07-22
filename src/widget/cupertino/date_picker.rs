//! CupertinoDatePicker widget — an iOS UIPickerView-style scrolling wheel date picker.
//!
//! The CupertinoDatePicker presents three scrolling columns (year, month, day)
//! side-by-side, similar to the iOS date picker wheel. Options are limited by
//! optional min/max date constraints. A `date_changed` signal is emitted
//! whenever the selected date changes.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

const MONTH_NAMES: &[&str] =
    &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Pre-formatted day strings ("01" through "31") reused across draw calls.
const DAY_STRINGS: &[&str] = &[
    "01", "02", "03", "04", "05", "06", "07", "08", "09", "10",
    "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
    "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
];

/// Returns the number of days in the given month, accounting for leap years.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// A valid date range for year generation.
#[derive(Clone, Debug)]
struct DateConstraint {
    min_year: i32,
    max_year: i32,
}

/// iOS-style scrolling wheel date picker with year/month/day columns.
///
/// Shows three columns (year, month, day) with a wheel-style scroll list.
/// The currently selected value in each column is visually highlighted.
/// A `date_changed` signal is emitted as a `(year, month, day)` tuple
/// whenever the selection changes.
pub struct CupertinoDatePicker {
    base: BaseWidget,
    selected_year: i32,
    selected_month: u32,
    selected_day: u32,
    min_date: Option<(i32, u32, u32)>,
    max_date: Option<(i32, u32, u32)>,
    /// Emitted with a `(year, month, day)` tuple when the date changes.
    pub date_changed: Signal1<(i32, u32, u32)>,
}

impl CupertinoDatePicker {
    /// Creates a new CupertinoDatePicker with the given geometry.
    ///
    /// Initial date defaults to 2025-01-01.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CupertinoDatePicker, geometry, "CupertinoDatePicker"),
            selected_year: 2025,
            selected_month: 1,
            selected_day: 1,
            min_date: None,
            max_date: None,
            date_changed: Signal1::new(),
        }
    }

    /// Returns the current selected date as `(year, month, day)`.
    pub fn selected_date(&self) -> (i32, u32, u32) {
        (self.selected_year, self.selected_month, self.selected_day)
    }

    /// Sets the selected date, clamping month/day to valid ranges.
    /// Emits `date_changed` if the value actually changes.
    pub fn set_selected_date(&mut self, year: i32, month: u32, day: u32) {
        let clamped_month = month.clamp(1, 12);
        let max_day = days_in_month(year, clamped_month);
        let clamped_day = day.clamp(1, max_day);

        // Apply min/max constraints
        let (year, month, day) = self.clamp_to_date_range(year, clamped_month, clamped_day);

        if self.selected_year != year || self.selected_month != month || self.selected_day != day {
            self.selected_year = year;
            self.selected_month = month;
            self.selected_day = day;
            self.date_changed.emit((year, month, day));
            self.base.request_redraw();
        }
    }

    /// Sets the allowable date range. Pass `None` for no constraint.
    pub fn set_date_range(&mut self, min: Option<(i32, u32, u32)>, max: Option<(i32, u32, u32)>) {
        self.min_date = min;
        self.max_date = max;
        // Re-clamp current date to new range
        let clamped =
            self.clamp_to_date_range(self.selected_year, self.selected_month, self.selected_day);
        self.set_selected_date(clamped.0, clamped.1, clamped.2);
    }

    /// Returns the current min date constraint.
    pub fn min_date(&self) -> Option<(i32, u32, u32)> {
        self.min_date
    }

    /// Returns the current max date constraint.
    pub fn max_date(&self) -> Option<(i32, u32, u32)> {
        self.max_date
    }

    /// Clamps a date to the configured min/max range.
    fn clamp_to_date_range(&self, year: i32, month: u32, day: u32) -> (i32, u32, u32) {
        let (y, m, d) = (year, month, day);
        if let Some((min_y, min_m, min_d)) = self.min_date {
            if y < min_y || (y == min_y && (m < min_m || (m == min_m && d < min_d))) {
                return (min_y, min_m, min_d);
            }
        }
        if let Some((max_y, max_m, max_d)) = self.max_date {
            if y > max_y || (y == max_y && (m > max_m || (m == max_m && d > max_d))) {
                return (max_y, max_m, max_d);
            }
        }
        (y, m, d)
    }

    /// Determines which column (0=year, 1=month, 2=day) contains the given x-coordinate.
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

    /// Returns the range of years available for selection.
    fn year_range(&self) -> std::ops::RangeInclusive<i32> {
        let min_y = self.min_date.map(|d| d.0).unwrap_or(1900);
        let max_y = self.max_date.map(|d| d.0).unwrap_or(2100);
        min_y..=max_y
    }

    /// Returns the constraint for the current selection context.
    fn date_constraint(&self) -> DateConstraint {
        let min_y = self.min_date.map(|d| d.0).unwrap_or(1900);
        let max_y = self.max_date.map(|d| d.0).unwrap_or(2100);
        DateConstraint { min_year: min_y, max_year: max_y }
    }
}

impl Widget for CupertinoDatePicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }
}

impl Draw for CupertinoDatePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Background
        let bg_color = if is_enabled {
            Color::rgba(240, 240, 245, 255)
        } else {
            Color::rgba(225, 225, 230, 200)
        };
        context.fill_rect(rect, bg_color);

        // Column layout
        let col_width = rect.width / 3;
        let row_height = (rect.height / 5).max(1);
        let font_size = (row_height as f32 * 0.38).clamp(10.0, 15.0);
        let font = Font::new("sans-serif", font_size, false, false);
        let arrow_font = Font::new("sans-serif", (font_size * 1.3).max(12.0), true, false);

        // Build visible column items (reuse static arrays — no per-draw Vec<String>)
        let constraint = self.date_constraint();
        let year_sel_idx = self.selected_year - constraint.min_year;

        // Months: reference the static array directly (no allocation)
        let month_items: &[&str] = MONTH_NAMES;
        let month_sel_idx = (self.selected_month as usize - 1) as i32;

        // Days: reference the static DAY_STRINGS array directly (no allocation)
        let max_days = days_in_month(self.selected_year, self.selected_month);
        let day_items: &[&str] = &DAY_STRINGS[..max_days as usize];
        let day_sel_idx = (self.selected_day as usize - 1) as i32;

        // Columns as (&[&str], ...) — borrow from static data
        // Year values are dynamic (min_y..=max_y), so still need per-draw strings
        let year_items: Vec<String> = self.year_range().map(|y| y.to_string()).collect();
        let year_refs: Vec<&str> = year_items.iter().map(|s| s.as_str()).collect();
        let columns: [(i32, &[&str], &str); 3] = [
            (year_sel_idx, &year_refs, "Year"),
            (month_sel_idx, month_items, "Month"),
            (day_sel_idx, day_items, "Day"),
        ];

        for (col_idx, (sel_offset, items, _label)) in columns.iter().enumerate() {
            let col_x = rect.x + (col_idx as u32 * col_width) as i32;

            // Vertical divider between columns
            if col_idx > 0 {
                context.draw_rect_stroke(
                    Rect::new(col_x, rect.y, 1, rect.height),
                    Color::rgba(200, 200, 210, 255),
                    1,
                );
            }

            // Highlight bar for the center (selected) row
            let highlight_y = rect.y + 2 * row_height as i32;
            let highlight_rect = Rect::new(col_x + 4, highlight_y, col_width - 8, row_height);
            context.fill_rounded_rect(highlight_rect, 6, Color::rgba(60, 120, 240, 50));

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

                let text_color = if !is_enabled {
                    Color::rgba(160, 160, 170, 200)
                } else if is_selected {
                    Color::rgba(40, 70, 190, 255)
                } else {
                    Color::rgba(130, 130, 150, 210)
                };

                context.draw_text(Point::new(text_x, text_y), text, &font, text_color, HorizontalAlignment::Left);
            }

            // Up arrow indicator (top of column)
            let arrow_color = if is_enabled {
                Color::rgba(80, 80, 100, 220)
            } else {
                Color::rgba(160, 160, 170, 150)
            };
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

impl EventHandler for CupertinoDatePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::MousePress { pos, button: 1 } = event {
            let col = self.column_at(pos.x);
            let rect = self.geometry();
            let row_height = rect.height / 5;
            let rel_y = pos.y - rect.y;
            let row = rel_y / row_height as i32;
            // Upper half (row 0-2) = increment, lower half (row 3-4) = decrement
            let increment = row <= 2;

            match col {
                0 => {
                    // Year column: cycle through available years
                    let constraint = self.date_constraint();
                    let new_year = if increment {
                        (self.selected_year + 1).min(constraint.max_year)
                    } else {
                        (self.selected_year - 1).max(constraint.min_year)
                    };
                    let max_day = days_in_month(new_year, self.selected_month);
                    let clamped_day = self.selected_day.min(max_day);
                    self.set_selected_date(new_year, self.selected_month, clamped_day);
                }
                1 => {
                    // Month column
                    let new_month = if increment {
                        (self.selected_month as i32 + 1).clamp(1, 12) as u32
                    } else {
                        (self.selected_month as i32 - 1).clamp(1, 12) as u32
                    };
                    let max_day = days_in_month(self.selected_year, new_month);
                    let clamped_day = self.selected_day.min(max_day);
                    self.set_selected_date(self.selected_year, new_month, clamped_day);
                }
                2 => {
                    // Day column
                    let max_days = days_in_month(self.selected_year, self.selected_month) as i32;
                    let new_day = if increment {
                        (self.selected_day as i32 + 1).clamp(1, max_days) as u32
                    } else {
                        (self.selected_day as i32 - 1).clamp(1, max_days) as u32
                    };
                    self.set_selected_date(self.selected_year, self.selected_month, new_day);
                }
                _ => {}
            }
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

    fn make_picker() -> CupertinoDatePicker {
        CupertinoDatePicker::new(Rect::new(0, 0, 240, 200))
    }

    #[test]
    fn picker_default_creation() {
        let picker = make_picker();
        assert_eq!(picker.kind(), WidgetKind::CupertinoDatePicker);
        assert_eq!(picker.selected_date(), (2025, 1, 1));
        assert!(picker.is_visible());
        assert!(picker.is_enabled());
        assert_eq!(picker.geometry(), Rect::new(0, 0, 240, 200));
    }

    #[test]
    fn picker_set_selected_date() {
        let mut picker = make_picker();
        picker.set_selected_date(2026, 12, 25);
        assert_eq!(picker.selected_date(), (2026, 12, 25));
    }

    #[test]
    fn picker_set_date_clamps_day_out_of_range() {
        let mut picker = make_picker();
        // January has 31 days, so day 35 should clamp to 31
        picker.set_selected_date(2025, 1, 35);
        assert_eq!(picker.selected_date(), (2025, 1, 31));

        // February in non-leap year has 28 days
        picker.set_selected_date(2025, 2, 30);
        assert_eq!(picker.selected_date(), (2025, 2, 28));
    }

    #[test]
    fn picker_set_date_handles_leap_year() {
        let mut picker = make_picker();
        // 2024 is a leap year
        picker.set_selected_date(2024, 2, 29);
        assert_eq!(picker.selected_date(), (2024, 2, 29));

        // 2025 is not a leap year - should clamp
        picker.set_selected_date(2025, 2, 29);
        assert_eq!(picker.selected_date(), (2025, 2, 28));
    }

    #[test]
    fn picker_set_date_range() {
        let mut picker = make_picker();
        picker.set_date_range(Some((2020, 1, 1)), Some((2030, 12, 31)));
        assert_eq!(picker.min_date(), Some((2020, 1, 1)));
        assert_eq!(picker.max_date(), Some((2030, 12, 31)));

        // Setting date outside range should clamp
        picker.set_selected_date(2019, 6, 15);
        assert_eq!(picker.selected_date(), (2020, 1, 1));

        picker.set_selected_date(2035, 6, 15);
        assert_eq!(picker.selected_date(), (2030, 12, 31));
    }

    #[test]
    fn picker_same_value_no_emit() {
        let mut picker = make_picker();
        let emitted = Arc::new(AtomicBool::new(false));
        let e = emitted.clone();
        picker.date_changed.connect(move |_val: Arc<(i32, u32, u32)>| {
            e.store(true, Ordering::SeqCst);
        });

        // Set same date - should NOT emit
        picker.set_selected_date(2025, 1, 1);
        assert!(!emitted.load(Ordering::SeqCst));

        // Set different date - SHOULD emit
        picker.set_selected_date(2025, 3, 15);
        assert!(emitted.load(Ordering::SeqCst));
    }

    #[test]
    fn picker_mouse_press_changes_date() {
        let mut picker = make_picker();
        // Click upper half of year column -> increment year
        picker.handle_event(&Event::MousePress { pos: Point::new(20, 30), button: 1 });
        assert_eq!(picker.selected_date().0, 2026);

        // Click lower half of year column -> decrement year
        picker.handle_event(&Event::MousePress { pos: Point::new(20, 150), button: 1 });
        assert_eq!(picker.selected_date().0, 2025);

        // Click upper half of month column -> increment month
        picker.handle_event(&Event::MousePress { pos: Point::new(100, 30), button: 1 });
        assert_eq!(picker.selected_date().1, 2);

        // Click upper half of day column -> increment day
        picker.handle_event(&Event::MousePress { pos: Point::new(180, 30), button: 1 });
        assert_eq!(picker.selected_date().2, 2);
    }

    #[test]
    fn picker_svg_output() {
        let mut picker = make_picker();
        picker.set_selected_date(2026, 7, 4);
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"240\""));
        assert!(svg.contains("height=\"200\""));
    }

    #[test]
    fn picker_disabled_blocks_events() {
        let mut picker = make_picker();
        picker.set_enabled(false);

        picker.handle_event(&Event::MousePress { pos: Point::new(20, 30), button: 1 });
        assert_eq!(picker.selected_date().0, 2025);
    }
}
