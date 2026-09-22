// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CupertinoDatePicker widget — an iOS UIPickerView-style scrolling wheel date picker.
//!
//! The CupertinoDatePicker presents three scrolling columns (year, month, day)
//! side-by-side, similar to the iOS date picker wheel. Options are limited by
//! optional min/max date constraints. A `date_changed` signal is emitted
//! whenever the selected date changes.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::misc_widgets::date_utils::{days_in_month, DAY_STRINGS, MONTH_NAMES};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Parses an ISO `YYYY-MM-DD` date into a `(year, month, day)` tuple.
///
/// Returns `None` for anything that is not exactly that shape, so the property
/// contract can answer [`CapabilityAccessError::TypeMismatch`] instead of
/// silently clamping a malformed string into a valid-looking date.
///
/// The day is only range-checked to `1..=31`, not against the length of the
/// specific month: [`CupertinoDatePicker::set_selected_date`] normalises it
/// afterwards, and clamping there is what lets a caller step through months
/// without the day being rejected first.
fn parse_iso_date(text: &str) -> Option<(i32, u32, u32)> {
    let mut parts = text.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoDatePicker`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_date` reads the
/// picker's real date and publishes it as ISO `YYYY-MM-DD`, replacing the fixed
/// `2025-01-01` the centralised defaults table returned.
impl WidgetProperties for CupertinoDatePicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_date" => {
                let (year, month, day) = self.selected_date();
                Ok(CapabilityValue::String(format!("{year:04}-{month:02}-{day:02}")))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_date" => {
                let parsed = parse_iso_date(&expect_string(value)?)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_selected_date(parsed.0, parsed.1, parsed.2);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_date", BASE_PROPERTY_NAMES]
    }
}

impl Draw for CupertinoDatePicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be
        // a literal, so a light/dark switch left the wheel, its columns, its divider, its
        // selection band and its text unchanged — the rendering census reported the control
        // as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("cupertino_date_picker");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. The picker is not in the role table, so it
        // classifies as `Surface` and its resolved background is the window fill itself; the
        // wheel below therefore derives its own distinct surface rather than painting the
        // window's. The selected row is the wheel's value indicator, so it reads the theme's
        // accent. `muted` is the theme's own de-emphasised colour, which replaces the two
        // fixed greys the unselected rows and the arrows used.
        let (window_fill, foreground, accent, muted, disabled) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.accent,
                    active.colors.secondary,
                    active.colors.disabled,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(255, 152, 0),
                    Color::rgb(158, 158, 158),
                    Color::rgb(200, 200, 200),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The wheel interior: one step from the window fill toward the text colour, so it
        // reads as a recessed field on a light theme and on a dark one. The filter is on the
        // **resolved** value, not only on the theme's: the active theme is applied to every
        // control before it is drawn, so `style.background_color` already holds `Surface`'s
        // window fill and letting it through unfiltered is exactly the invisible-field defect
        // this guards against. A caller's own colour still wins.
        let wheel_from_theme = window_fill.blend(&ink, 0.06);
        let wheel = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => wheel_from_theme,
        };
        // A `Surface` role resolves no border colour, so the column rule is derived one
        // visible step from the wheel and a caller's explicit border still wins.
        let column_rule = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != wheel)
            .unwrap_or_else(|| wheel.blend(&muted, 0.45));
        // The selection band, the selected row's text and the arrows are all accents: opaque
        // variants for the text and arrows, a low-alpha one for the band. The band is painted
        // with `fill_rounded_rect`, which alpha-blends, so a translucent accent really does
        // tint the wheel behind it here (unlike `fill_rect`, which overwrites).
        //
        // The selected row's ink is therefore chosen against the surface it is really painted
        // on — the band composited over the wheel — and not against `accent`. Choosing it from
        // the accent (and then blending 80% of the way back to the accent) made the label and
        // its backdrop the same hue at the same lightness: `rgb(204,122,0)` on the light
        // theme's `rgb(243,222,192)` band measured 2.52:1. `Color::blend` is the same alpha
        // composite the renderer performs, so this predicts the painted colour exactly.
        let band_alpha = 50.0 / 255.0;
        let banded_surface = wheel.blend(&accent.with_alpha(255), band_alpha);
        let selected_text = banded_surface.contrast_color();
        let band = Color::rgba(accent.r, accent.g, accent.b, 50);
        // The up/down arrows say "there are more values this way", so they are held legible on
        // the wheel while keeping the accent's hue — the plain 35% blend measured 3.68:1 on the
        // light wheel, so the only cue that the list continues was the faintest mark in it.
        let arrow = accent.blend(&ink, 0.35).legible_on(wheel, 4.5);
        // The disabled treatment of each of the three, so the appearance agrees with the
        // control's refusal to accept a pick.
        let disabled_text = muted.blend(&wheel, 0.35);
        let disabled_arrow = muted.blend(&wheel, 0.20);
        let disabled_surface = disabled.blend(&wheel, 0.55);

        // Background
        let bg_color = if is_enabled { wheel } else { disabled_surface };
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
                context.draw_rect_stroke(Rect::new(col_x, rect.y, 1, rect.height), column_rule, 1);
            }

            // Highlight bar for the center (selected) row
            let highlight_y = rect.y + 2 * row_height as i32;
            let highlight_rect =
                Rect::new(col_x + 4, highlight_y, col_width.saturating_sub(8), row_height);
            context.fill_rounded_rect(highlight_rect, 6, band);

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
                // Centred inside the row's own band. The old origin added `ascent` on top of
                // an already-centred y, so the glyph box (which starts at the origin and runs
                // a full line down) began half a line low — the fifth row was pushed past the
                // control and only its top edge was ever painted.
                let text_y = item_y + (row_height as i32 - metrics.height as i32) / 2;

                let text_color = if !is_enabled {
                    disabled_text
                } else if is_selected {
                    selected_text
                } else {
                    // The neighbouring rows are *read*: a wheel's whole purpose is to show the
                    // values next to the selected one. They are secondary relative to the
                    // selection, not faint to the point of unreadable — the theme's `muted`
                    // token measured 2.07:1 on the light appearance's wheel, below even the 3:1
                    // large-text floor, so the rows a user is scrolling towards were the ones
                    // they could not read. Pushed clear while staying below the selection's
                    // contrast, which is what makes the selection still read as selected.
                    muted.legible_on(wheel, 4.5)
                };

                context.draw_text_fitted(
                    Rect::new(col_x, text_y, col_width, metrics.height),
                    text,
                    &font,
                    text_color,
                    HorizontalAlignment::Center,
                );
            }

            // Up arrow indicator (top of column)
            let arrow_color = if is_enabled { arrow } else { disabled_arrow };
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
