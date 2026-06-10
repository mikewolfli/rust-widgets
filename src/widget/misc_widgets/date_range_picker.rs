//! DateRangePicker widget — a calendar-based date range selection widget.
//!
//! The DateRangePicker displays a calendar month grid where users click to
//! select a start date and then click again to select an end date, forming
//! a date range. The range between the two dates is visually highlighted.
//! A `range_changed` signal is emitted whenever the selection changes.

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

const MONTH_NAMES: &[&str] =
    &["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const DAY_NAMES: &[&str] = &["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

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

/// Returns the day of week (0=Sunday, 1=Monday, ..., 6=Saturday)
/// for the given date using Zeller-like / Tomohiko Sakamoto's algorithm.
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let (y, m) = if month < 3 { (year - 1, month + 12) } else { (year, month) };
    let c = y / 100;
    let y = y % 100;
    let d = day as i32;
    let w = (d + (13 * (m as i32 + 1)) / 5 + y + y / 4 + c / 4 - 2 * c) % 7;
    ((w + 7) % 7) as u32
}

/// DateRangePicker widget — select a date range by clicking start/end dates.
///
/// Displays a calendar for a given month/year. The user clicks once to set
/// the start date, and clicks again to set the end date. The range is
/// visually highlighted. Clicking outside the grid (or an already-selected
/// start) with no end date clears the selection.
pub struct DateRangePicker {
    base: BaseWidget,
    display_year: i32,
    display_month: u32,
    start_date: Option<(i32, u32, u32)>,
    end_date: Option<(i32, u32, u32)>,
    hover_date: Option<(i32, u32, u32)>,
    /// Emitted when the range changes, with `(start_date, end_date)`.
    pub range_changed: Signal1<(Option<(i32, u32, u32)>, Option<(i32, u32, u32)>)>,
}

impl DateRangePicker {
    /// Creates a new DateRangePicker widget with the given geometry.
    ///
    /// The displayed month defaults to the current date's month (or 2025-01).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DateRangePicker, geometry, "DateRangePicker"),
            display_year: 2025,
            display_month: 1,
            start_date: None,
            end_date: None,
            hover_date: None,
            range_changed: Signal1::new(),
        }
    }

    /// Returns the current start date, if set.
    pub fn start_date(&self) -> Option<(i32, u32, u32)> {
        self.start_date
    }

    /// Sets the start date and emits `range_changed` if it changes.
    pub fn set_start_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.start_date != date {
            self.start_date = date;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Returns the current end date, if set.
    pub fn end_date(&self) -> Option<(i32, u32, u32)> {
        self.end_date
    }

    /// Sets the end date and emits `range_changed` if it changes.
    pub fn set_end_date(&mut self, date: Option<(i32, u32, u32)>) {
        if self.end_date != date {
            self.end_date = date;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Clears both start and end dates.
    pub fn clear_selection(&mut self) {
        if self.start_date.is_some() || self.end_date.is_some() {
            self.start_date = None;
            self.end_date = None;
            self.emit_range_changed();
            self.base.request_redraw();
        }
    }

    /// Navigates to the previous month.
    pub fn previous_month(&mut self) {
        if self.display_month == 1 {
            self.display_year -= 1;
            self.display_month = 12;
        } else {
            self.display_month -= 1;
        }
        self.base.request_redraw();
    }

    /// Navigates to the next month.
    pub fn next_month(&mut self) {
        if self.display_month == 12 {
            self.display_year += 1;
            self.display_month = 1;
        } else {
            self.display_month += 1;
        }
        self.base.request_redraw();
    }

    /// Returns the currently displayed year.
    pub fn display_year(&self) -> i32 {
        self.display_year
    }

    /// Returns the currently displayed month.
    pub fn display_month(&self) -> u32 {
        self.display_month
    }

    /// Emits the `range_changed` signal with current start/end.
    fn emit_range_changed(&self) {
        self.range_changed.emit((self.start_date, self.end_date));
    }

    /// Returns the display month name.
    fn month_name(&self) -> &str {
        let idx = (self.display_month as usize).saturating_sub(1);
        MONTH_NAMES[idx.min(11)]
    }

    /// Returns the number of days in the displayed month.
    fn days_in_display_month(&self) -> u32 {
        days_in_month(self.display_year, self.display_month)
    }

    /// Returns the day of week for the 1st of the displayed month.
    fn first_day_of_week(&self) -> u32 {
        day_of_week(self.display_year, self.display_month, 1)
    }

    /// Returns the day, month, year for a given cell position in the grid.
    fn date_at_cell(&self, cell_index: usize) -> Option<(i32, u32, u32)> {
        let first_dow = self.first_day_of_week() as usize;
        let total_days = self.days_in_display_month() as usize;

        if cell_index < first_dow || cell_index >= first_dow + total_days {
            return None;
        }

        let day = (cell_index - first_dow + 1) as u32;
        Some((self.display_year, self.display_month, day))
    }

    /// Checks if a date falls within the current selection range.
    fn is_in_range(&self, date: (i32, u32, u32)) -> bool {
        let Some(start) = self.start_date else {
            return false;
        };
        if date_to_ordinal(date) < date_to_ordinal(start) {
            return false;
        }
        if let Some(end) = self.end_date {
            date_to_ordinal(date) <= date_to_ordinal(end)
        } else {
            // If no end date, highlight just the start
            date == start
        }
    }
}

/// Converts a date to a single ordinal number for easy comparison.
fn date_to_ordinal(date: (i32, u32, u32)) -> i64 {
    date.0 as i64 * 10000 + date.1 as i64 * 100 + date.2 as i64
}

impl Widget for DateRangePicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for DateRangePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let is_enabled = self.base.is_enabled();
        let bg_color = if is_enabled {
            Color::rgba(255, 255, 255, 255)
        } else {
            Color::rgba(240, 240, 240, 255)
        };
        context.fill_rect(rect, bg_color);

        // Layout parameters
        let header_height = 40u32;
        let day_header_height = 20u32;
        let cell_size = 28u32;
        let cell_spacing = 2u32;
        let total_cell = cell_size + cell_spacing;
        let grid_left = rect.x + 4;
        let grid_top = rect.y + header_height as i32 + day_header_height as i32;

        // ── Month/Year header ──
        let header_font = Font::new("sans-serif", 14.0, true, false);
        let header_text = format!("{} {}", self.month_name(), self.display_year);
        let header_metrics = context.measure_text(&header_text, &header_font);
        let header_x = rect.x + (rect.width as i32 - header_metrics.width as i32) / 2;
        let header_y = rect.y + 14 + header_metrics.ascent as i32;
        context.draw_text(
            Point::new(header_x, header_y),
            &header_text,
            &header_font,
            Color::DARK_GRAY,
        );

        // Navigation arrows
        let nav_font = Font::new("sans-serif", 12.0, true, false);
        context.draw_text(Point::new(rect.x + 8, rect.y + 16), "<", &nav_font, Color::DARK_GRAY);
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 16, rect.y + 16),
            ">",
            &nav_font,
            Color::DARK_GRAY,
        );

        // ── Day-of-week header ──
        let dow_font = Font::new("sans-serif", 9.0, false, false);
        for (i, day_name) in DAY_NAMES.iter().enumerate() {
            let cell_x = grid_left + (i as u32 * total_cell) as i32;
            let cell_y = grid_top - day_header_height as i32;
            context.draw_text(
                Point::new(cell_x + 6, cell_y + 14),
                day_name,
                &dow_font,
                Color::rgba(120, 120, 120, 255),
            );
        }

        // ── Calendar grid ──
        let day_font = Font::new("sans-serif", 10.0, false, false);
        let total_days = self.days_in_display_month() as usize;
        let first_dow = self.first_day_of_week() as usize;
        let total_cells = first_dow + total_days;

        for cell_idx in 0..total_cells {
            let Some(date) = self.date_at_cell(cell_idx) else {
                continue;
            };
            let day = date.2;

            let row = cell_idx / 7;
            let col = cell_idx % 7;
            let cell_x = grid_left + (col as u32 * total_cell) as i32;
            let cell_y = grid_top + (row as u32 * total_cell) as i32;

            let cell_rect = Rect::new(cell_x, cell_y, cell_size, cell_size);

            // Determine cell styling
            let is_start = self.start_date == Some(date);
            let is_end = self.end_date == Some(date);
            let is_today = is_start || is_end;
            let in_range = self.is_in_range(date);
            let is_hover = self.hover_date == Some(date);

            // Background
            if in_range && is_enabled {
                let range_color = if is_start || is_end {
                    Color::rgba(52, 120, 246, 200)
                } else {
                    Color::rgba(52, 120, 246, 60)
                };
                context.fill_rounded_rect(cell_rect, 4, range_color);
            } else if is_hover && is_enabled {
                context.fill_rounded_rect(cell_rect, 4, Color::rgba(200, 200, 200, 100));
            } else if is_today && is_enabled {
                let today_color = if is_start || is_end {
                    Color::rgba(52, 120, 246, 200)
                } else {
                    Color::rgba(220, 220, 220, 180)
                };
                context.fill_rounded_rect(cell_rect, 4, today_color);
            }

            // Draw day number
            let day_text = day.to_string();
            let day_metrics = context.measure_text(&day_text, &day_font);
            let day_x = cell_x + (cell_size as i32 - day_metrics.width as i32) / 2;
            let day_y = cell_y
                + (cell_size as i32 - day_metrics.height as i32) / 2
                + day_metrics.ascent as i32;

            let day_color = if !is_enabled {
                Color::rgba(180, 180, 180, 255)
            } else if is_start || is_end {
                Color::WHITE
            } else if in_range {
                Color::rgba(30, 80, 200, 255)
            } else {
                Color::DARK_GRAY
            };
            context.draw_text(Point::new(day_x, day_y), &day_text, &day_font, day_color);
        }
    }
}

impl EventHandler for DateRangePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                let rect = self.geometry();
                if !rect.contains_point(*pos) {
                    return;
                }

                let header_height = 40u32;
                let day_header_height = 20u32;
                let cell_size = 28u32;
                let cell_spacing = 2u32;
                let total_cell = cell_size + cell_spacing;
                let grid_left = rect.x + 4;
                let grid_top = rect.y + header_height as i32 + day_header_height as i32;

                // Check header clicks for navigation
                let header_rect_left = Rect::new(rect.x + 4, rect.y + 4, 20, header_height);
                let header_rect_right =
                    Rect::new(rect.x + rect.width as i32 - 24, rect.y + 4, 20, header_height);

                if header_rect_left.contains_point(*pos) {
                    self.previous_month();
                    return;
                }
                if header_rect_right.contains_point(*pos) {
                    self.next_month();
                    return;
                }

                // Check if click is on a cell in the grid
                let rel_x = pos.x - grid_left;
                let rel_y = pos.y - grid_top;
                if rel_x < 0 || rel_y < 0 {
                    return;
                }

                let col = (rel_x as u32) / total_cell;
                let row = (rel_y as u32) / total_cell;
                if col >= 7 || row >= 6 {
                    // Click outside the grid — clear if only start is selected
                    if self.start_date.is_some() && self.end_date.is_none() {
                        self.clear_selection();
                    }
                    return;
                }

                let cell_idx = (row as usize) * 7 + (col as usize);
                let Some(clicked_date) = self.date_at_cell(cell_idx) else {
                    return;
                };

                match (self.start_date, self.end_date) {
                    (None, _) => {
                        // No selection — set start
                        self.start_date = Some(clicked_date);
                        self.emit_range_changed();
                        self.base.request_redraw();
                    }
                    (Some(start), None) => {
                        // Start is set, no end yet — set end
                        let start_ord = date_to_ordinal(start);
                        let click_ord = date_to_ordinal(clicked_date);

                        if click_ord > start_ord {
                            self.end_date = Some(clicked_date);
                        } else if click_ord < start_ord {
                            // Clicked before start: swap: new start = clicked, end = old start
                            self.start_date = Some(clicked_date);
                            self.end_date = Some(start);
                        } else {
                            // Clicked same as start — clear
                            self.clear_selection();
                            return;
                        }
                        self.emit_range_changed();
                        self.base.request_redraw();
                    }
                    (Some(_), Some(_)) => {
                        // Both set — start new selection
                        self.start_date = Some(clicked_date);
                        self.end_date = None;
                        self.emit_range_changed();
                        self.base.request_redraw();
                    }
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

    fn make_picker() -> DateRangePicker {
        DateRangePicker::new(Rect::new(0, 0, 240, 260))
    }

    #[test]
    fn date_range_picker_default_creation() {
        let picker = make_picker();
        assert_eq!(picker.kind(), WidgetKind::DateRangePicker);
        assert!(picker.start_date().is_none());
        assert!(picker.end_date().is_none());
        assert_eq!(picker.display_year(), 2025);
        assert_eq!(picker.display_month(), 1);
    }

    #[test]
    fn date_range_picker_set_start_end() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));
        assert_eq!(picker.start_date(), Some((2025, 1, 10)));

        picker.set_end_date(Some((2025, 1, 20)));
        assert_eq!(picker.end_date(), Some((2025, 1, 20)));
    }

    #[test]
    fn date_range_picker_clear_selection() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));
        picker.set_end_date(Some((2025, 1, 20)));
        assert!(picker.start_date().is_some());
        assert!(picker.end_date().is_some());

        picker.clear_selection();
        assert!(picker.start_date().is_none());
        assert!(picker.end_date().is_none());
    }

    #[test]
    fn date_range_picker_navigation() {
        let mut picker = make_picker();
        assert_eq!(picker.display_month(), 1);

        picker.previous_month();
        assert_eq!(picker.display_month(), 12);
        assert_eq!(picker.display_year(), 2024);

        picker.next_month();
        assert_eq!(picker.display_month(), 1);
        assert_eq!(picker.display_year(), 2025);
    }

    #[test]
    fn date_range_picker_svg_output() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 5)));
        picker.set_end_date(Some((2025, 1, 15)));
        let svg = render_to_svg(&mut picker);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn date_range_picker_mouse_press_sets_start() {
        let mut picker = make_picker();
        // Jan 1, 2025 is Wednesday -> day_of_week returns 4 (Zeller: 0=Sat, 4=Wed)
        // Day 1 is at cell index 4
        // Cell size = 28, spacing = 2, total_cell = 30
        // Grid starts at x=4, y=40+20=60
        // Day 15 = cell index 4 + 14 = 18 => row=2 (18/7=2), col=4 (18%7=4)
        // Position: x=4 + 4*30 = 124, y=60 + 2*30 = 120
        picker.handle_event(&Event::MousePress { pos: Point::new(124, 120), button: 1 });
        assert_eq!(picker.start_date(), Some((2025, 1, 15)));
        assert!(picker.end_date().is_none());
    }

    #[test]
    fn date_range_picker_mouse_press_clears_existing() {
        let mut picker = make_picker();
        picker.set_start_date(Some((2025, 1, 10)));

        // Click outside the grid area (row >= 6)
        picker.handle_event(&Event::MousePress {
            pos: Point::new(4, 250), // bottom of widget
            button: 1,
        });
        assert!(picker.start_date().is_none());
    }
}
