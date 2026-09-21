// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! DateRangePicker widget — a calendar-based date range selection widget.
//!
//! The DateRangePicker displays a calendar month grid where users click to
//! select a start date and then click again to select an end date, forming
//! a date range. The range between the two dates is visually highlighted.
//! A `range_changed` signal is emitted whenever the selection changes.

use super::date_utils::{days_in_month, parse_iso_date, DAY_NAMES, MONTH_NAMES};
#[cfg(test)]
use crate::core::Point;
use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A selected date range: `(start_date, end_date)` each as `(year, month, day)`.
pub(crate) type DateRange = (Option<(i32, u32, u32)>, Option<(i32, u32, u32)>);

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
    pub range_changed: Signal1<DateRange>,
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

    /// The calendar grid's geometry for the current month and rectangle.
    ///
    /// Shared by the draw and the hit test, because a click must land in the cell that is
    /// visibly under it: the two used to derive the grid independently from the same
    /// literals, so any change to one silently desynchronised the other.
    ///
    /// The cell size is derived from the room the grid actually has rather than being a
    /// fixed 28 px. A month can need five or six week rows, so a fixed cell walked the last
    /// row — and the labels in it — past the control's bottom edge, where the raster
    /// backends clipped it and the SVG snapshot showed it leaving the picture. The nominal
    /// size is kept as the *maximum*, so a roomy control is unchanged.
    fn grid_layout(&self) -> GridLayout {
        let rect = self.geometry();
        let total_days = self.days_in_display_month() as usize;
        let first_dow = self.first_day_of_week() as usize;
        let rows = ((first_dow + total_days).div_ceil(7) as u32).max(1);
        let grid_left = rect.x + GRID_LEFT_INSET;
        let grid_top = rect.y + HEADER_HEIGHT as i32 + DAY_HEADER_HEIGHT as i32;
        let available_height = (rect.y + rect.height as i32 - grid_top).max(1) as u32;
        // `rows` rows of `cell_size` with `CELL_SPACING` between them must fit, so the gaps
        // are subtracted before the division. A floor of 1 keeps a degenerate box from
        // producing a zero-sized cell the label would then be fitted into nothing.
        let gaps = CELL_SPACING * (rows - 1);
        let cell_size = (available_height.saturating_sub(gaps) / rows).clamp(1, NOMINAL_CELL);
        GridLayout {
            grid_left,
            grid_top,
            cell_size,
            total_cell: cell_size + CELL_SPACING,
            total_cells: first_dow + total_days,
        }
    }
}

/// Reserved height of the month/year header band.
const HEADER_HEIGHT: u32 = 40;
/// Reserved height of the weekday row.
const DAY_HEADER_HEIGHT: u32 = 20;
/// Left inset of the grid from the control's edge.
const GRID_LEFT_INSET: i32 = 4;
/// Gap between calendar cells.
const CELL_SPACING: u32 = 2;
/// The largest a calendar cell is allowed to be.
const NOMINAL_CELL: u32 = 28;

/// The calendar grid's resolved geometry.
struct GridLayout {
    /// Absolute x of the first column.
    grid_left: i32,
    /// Absolute y of the first week row.
    grid_top: i32,
    /// Edge length of one day cell.
    cell_size: u32,
    /// Column pitch: the cell plus its gap.
    total_cell: u32,
    /// Cells to draw, including the leading blanks of the first week.
    total_cells: usize,
}

/// Converts a date to a single ordinal number for easy comparison.
fn date_to_ordinal(date: (i32, u32, u32)) -> i64 {
    date.0 as i64 * 10000 + date.1 as i64 * 100 + date.2 as i64
}

/// Formats a date as an ISO `YYYY-MM-DD` string.
///
/// Thin wrapper over [`date_utils::format_iso_date`] so this module can keep
/// passing its `(year, month, day)` tuples around as one value.
fn format_iso_date(date: (i32, u32, u32)) -> String {
    super::date_utils::format_iso_date(date.0, date.1, date.2)
}

impl Widget for DateRangePicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `DateRangePicker`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. Each endpoint reports the
/// widget's real date as an ISO `YYYY-MM-DD` string, or `Null` while that endpoint
/// is unset, rather than the legacy hardcoded blank. A write parses the same format;
/// malformed text is reported as [`CapabilityAccessError::TypeMismatch`] instead of
/// quietly ignoring the write, and `Null` clears the endpoint.
impl WidgetProperties for DateRangePicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "start_date" => Ok(match self.start_date() {
                Some(date) => CapabilityValue::String(format_iso_date(date)),
                None => CapabilityValue::Null,
            }),
            "end_date" => Ok(match self.end_date() {
                Some(date) => CapabilityValue::String(format_iso_date(date)),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "start_date" => {
                self.set_start_date(expect_optional_iso_date(value)?);
                Ok(())
            }
            "end_date" => {
                self.set_end_date(expect_optional_iso_date(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["start_date", "end_date", BASE_PROPERTY_NAMES]
    }
}

/// Reads an optional ISO date: `Null` clears, a string parses, anything else is a
/// type mismatch.
fn expect_optional_iso_date(
    value: CapabilityValue,
) -> Result<Option<(i32, u32, u32)>, CapabilityAccessError> {
    match value {
        CapabilityValue::Null => Ok(None),
        other => {
            let text = expect_string(other)?;
            parse_iso_date(&text).map(Some).ok_or(CapabilityAccessError::TypeMismatch)
        }
    }
}

impl Draw for DateRangePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously the calendar surface, its
        // text, and the range highlight were all hardcoded, so light and dark
        // rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("date_range_picker");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // A disabled calendar is the same surface, faded: derived from the resolved
        // colour rather than a second literal so it still follows the appearance.
        let bg_color =
            if is_enabled { surface.with_alpha(255) } else { surface.blend(&text_color, 0.1) };
        // The selected range is a state, so it reads the theme's selection accent
        // rather than a fixed blue. `primary` is the theme's brand/action colour.
        let accent = theme
            .as_ref()
            .and_then(|_| crate::style::semantic_color(crate::style::SemanticColor::Info))
            .unwrap_or(Color::BLUE);

        context.fill_rect(rect, bg_color);

        // Layout parameters, resolved from the month and the rectangle. See
        // [`DateRangePicker::grid_layout`] for why the cell size is not a constant.
        let layout = self.grid_layout();
        let grid_left = layout.grid_left;
        let grid_top = layout.grid_top;
        let cell_size = layout.cell_size;
        let total_cell = layout.total_cell;

        // ── Month/Year header ──
        //
        // The label is fitted to the header band and centred **inside** it. The previous
        // origin added `ascent` on top of a `y` that was already past the band, so the glyph
        // box (which starts at the origin and extends down a full line) began near the band's
        // bottom: the header read as sitting on the grid, and the last day row was pushed off
        // the control entirely. `ascent` is *inside* the line box, not above it — the render
        // origin is the glyph's top edge.
        let header_band = Rect::new(rect.x + 20, rect.y + 6, rect.width.saturating_sub(40), 18);
        let header_font = Font::new("sans-serif", 14.0, true, false);
        let header_text = format!("{} {}", self.month_name(), self.display_year);
        context.draw_text_fitted(
            header_band,
            &header_text,
            &header_font,
            text_color,
            HorizontalAlignment::Center,
        );

        // Navigation arrows, each in its own strip at the band's ends so they cannot collide
        // with a long month name.
        //
        // The glyphs are the filled triangles the rest of the toolkit navigates with
        // (`calendar`, `image_gallery`) rather than `<`/`>`. Those two are the only
        // characters in the registry that a serializer must escape: the SVG backend emits
        // `&gt;`, and a bounds reader that counts the *source* text as the advance then
        // measures five glyphs where one was drawn — which is what reported this arrow at
        // `[224,8..253,20]`. A glyph that needs no escaping is the honest fix, since the
        // drawn extent and the emitted text then agree.
        //
        // Both strips are anchored to the control's edges and the glyph is inset inside its
        // strip, so the drawn extent is bounded by the rectangle rather than by the glyph's
        // advance.
        let nav_font = Font::new("sans-serif", 12.0, true, false);
        let nav_height = context.measure_text("◀", &nav_font).height;
        context.draw_text_fitted(
            Rect::new(rect.x + 6, rect.y + 8, 14, nav_height),
            "◀",
            &nav_font,
            text_color,
            HorizontalAlignment::Center,
        );
        context.draw_text_fitted(
            Rect::new(rect.x + rect.width as i32 - 20, rect.y + 8, 14, nav_height),
            "▶",
            &nav_font,
            text_color,
            HorizontalAlignment::Center,
        );

        // ── Day-of-week header ──
        let dow_font = Font::new("sans-serif", 9.0, false, false);
        // The weekday row is secondary chrome: derived from the resolved text colour
        // so it stays legible against either surface.
        let dow_color = surface.blend(&text_color, 0.55);
        // Each weekday label is fitted inside its own column. `cell_x + 6` with a centred
        // cell put the last column's label past the grid's right edge; deriving the column
        // from `cell_size` and centring inside it keeps every label within its own cell.
        let dow_height = context.measure_text("M", &dow_font).height;
        for (i, day_name) in DAY_NAMES.iter().enumerate() {
            let cell_x = grid_left + (i as u32 * total_cell) as i32;
            let cell_y = grid_top - DAY_HEADER_HEIGHT as i32;
            context.draw_text_fitted(
                Rect::new(cell_x, cell_y + 4, cell_size, dow_height),
                day_name,
                &dow_font,
                dow_color,
                HorizontalAlignment::Center,
            );
        }

        // ── Calendar grid ──
        let day_font = Font::new("sans-serif", 10.0, false, false);
        let total_cells = layout.total_cells;

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
                let range_color =
                    if is_start || is_end { accent.with_alpha(200) } else { accent.with_alpha(60) };
                context.fill_rounded_rect(cell_rect, 4, range_color);
            } else if is_hover && is_enabled {
                context.fill_rounded_rect(cell_rect, 4, surface.blend(&text_color, 0.16));
            } else if is_today && is_enabled {
                let today_color = if is_start || is_end {
                    accent.with_alpha(200)
                } else {
                    surface.blend(&text_color, 0.14)
                };
                context.fill_rounded_rect(cell_rect, 4, today_color);
            }

            // Draw day number, fitted inside its own cell and centred on it. The origin is
            // the glyph's top edge, so the vertical centre is half the *line box* — the old
            // `(cell - height)/2 + ascent` began the glyph box half a line below the cell's
            // middle, which walked the bottom row past the control's edge.
            let day_text = day.to_string();
            let day_metrics = context.measure_text(&day_text, &day_font);
            let day_y = cell_y + (cell_size as i32 - day_metrics.height as i32) / 2;

            let day_color = if !is_enabled {
                surface.blend(&text_color, 0.35)
            } else if is_start || is_end {
                // A date on the accent fill: the resolved surface colour is the
                // legible counterpart of the accent in either appearance.
                surface
            } else if in_range {
                accent
            } else {
                text_color
            };
            context.draw_text_fitted(
                Rect::new(cell_x, day_y, cell_size, day_metrics.height),
                &day_text,
                &day_font,
                day_color,
                HorizontalAlignment::Center,
            );
        }
    }
}

impl EventHandler for DateRangePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if let Event::MousePress { pos, button: 1 } = event {
            let rect = self.geometry();
            if !rect.contains_point(*pos) {
                return;
            }

            // The same grid the draw uses, so a click lands in the cell under the pointer.
            let layout = self.grid_layout();
            let (grid_left, grid_top) = (layout.grid_left, layout.grid_top);
            let total_cell = layout.total_cell;

            // Check header clicks for navigation
            let header_rect_left = Rect::new(rect.x + 4, rect.y + 4, 20, HEADER_HEIGHT);
            let header_rect_right =
                Rect::new(rect.x + rect.width as i32 - 24, rect.y + 4, 20, HEADER_HEIGHT);

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
            if cell_idx >= layout.total_cells {
                // A click in the blank tail of the last week belongs to no date; treating it
                // as a selection would invent one.
                return;
            }
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
