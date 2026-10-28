//! `CalendarMarkLayer` — per-date marks drawn over a month's 7x6 day grid (COMP-A28).

use crate::compat::{BTreeMap, String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_i32_saturating, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

// ---------------------------------------------------------------------------
// COMP-A28 CalendarMarkLayer
// ---------------------------------------------------------------------------

/// Days in the grid: seven columns over six rows.
const CALENDAR_COLUMNS: u32 = 7;
/// Week rows in the grid.
const CALENDAR_ROWS: u32 = 6;
/// Space reserved at the top of a day cell for its number.
const DAY_LABEL_HEIGHT: u32 = 14;
/// Default marks a single date may carry.
const DEFAULT_MAX_PER_CELL: usize = 3;
/// The month a fresh layer shows.
const DEFAULT_YEAR: i32 = 2026;
/// The month a fresh layer shows.
const DEFAULT_MONTH: u32 = 1;

/// How a calendar mark is painted inside its date cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarMarkShape {
    /// A filled dot.
    #[default]
    Dot,
    /// A hollow ring.
    Ring,
    /// A line along the bottom of the mark lane.
    Underline,
    /// A rounded badge carrying [`CalendarMark::badge_text`].
    Badge,
}

impl CalendarMarkShape {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            CalendarMarkShape::Dot => "dot",
            CalendarMarkShape::Ring => "ring",
            CalendarMarkShape::Underline => "underline",
            CalendarMarkShape::Badge => "badge",
        }
    }

    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dot" => Some(CalendarMarkShape::Dot),
            "ring" => Some(CalendarMarkShape::Ring),
            "underline" => Some(CalendarMarkShape::Underline),
            "badge" => Some(CalendarMarkShape::Badge),
            _ => None,
        }
    }
}

/// One mark attached to a calendar date.
///
/// The colour is the caller's (a series colour, a category colour); the shape distinguishes
/// several marks on one date without needing a second legend. `badge_text` is only painted for
/// [`CalendarMarkShape::Badge`] and is ignored by the other shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarMark {
    /// Mark colour.
    pub color: Color,
    /// Mark shape.
    pub shape: CalendarMarkShape,
    /// Short text painted inside a badge; empty for the other shapes.
    pub badge_text: String,
}

impl CalendarMark {
    /// Creates a mark with an empty badge text.
    pub fn new(color: Color, shape: CalendarMarkShape) -> Self {
        Self { color, shape, badge_text: String::new() }
    }

    /// Creates a badge mark carrying `text`.
    pub fn badge(color: Color, text: impl Into<String>) -> Self {
        Self { color, shape: CalendarMarkShape::Badge, badge_text: text.into() }
    }
}

/// A month grid that paints per-date marks and reports a click on one of them.
///
/// # Model
///
/// Marks are keyed by the concrete `(year, month, day)` date, not by grid slot, so changing
/// [`Self::set_month`] does not lose or shuffle them. A date accepts at most
/// [`Self::set_max_per_cell`] marks; further additions are refused rather than replacing an
/// existing mark. Dates are validated against the real calendar, including leap years.
pub struct CalendarMarkLayer {
    base: BaseWidget,
    /// Marks per date, ordered by date so iteration and `mark_count` are deterministic.
    marks: BTreeMap<(i32, u32, u32), Vec<CalendarMark>>,
    /// The month currently drawn.
    year: i32,
    /// The month currently drawn (1-12).
    month: u32,
    /// How many marks one date may carry.
    max_per_cell: usize,
    /// Emitted with `(year, month, day, mark)` when a mark is clicked.
    pub mark_activated: Signal1<(i32, u32, u32, CalendarMark)>,
}

impl CalendarMarkLayer {
    /// Creates a layer showing January 2026 with up to three marks per date.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "CalendarMarkLayer"),
            marks: BTreeMap::new(),
            year: DEFAULT_YEAR,
            month: DEFAULT_MONTH,
            max_per_cell: DEFAULT_MAX_PER_CELL,
            mark_activated: Signal1::new(),
        }
    }

    /// Selects the month to draw.
    ///
    /// Returns `false` and leaves the layer unchanged for a month outside `1..=12`.
    pub fn set_month(&mut self, year: i32, month: u32) -> bool {
        if !(1..=12).contains(&month) {
            return false;
        }
        if self.year != year || self.month != month {
            self.year = year;
            self.month = month;
            self.base.request_redraw();
        }
        true
    }

    /// Returns the drawn year.
    pub fn year(&self) -> i32 {
        self.year
    }

    /// Returns the drawn month (`1..=12`).
    pub fn month(&self) -> u32 {
        self.month
    }

    /// Sets how many marks each date may carry, trimming any date that already exceeds it.
    ///
    /// A cap of zero is promoted to one: a layer that can hold no marks has no useful state, and
    /// silently refusing every [`Self::set_mark`] would hide the caller's mistake.
    pub fn set_max_per_cell(&mut self, max_per_cell: usize) {
        let max_per_cell = max_per_cell.max(1);
        if self.max_per_cell == max_per_cell {
            return;
        }
        self.max_per_cell = max_per_cell;
        for marks in self.marks.values_mut() {
            marks.truncate(max_per_cell);
        }
        self.base.request_redraw();
    }

    /// Returns the per-date cap.
    pub fn max_per_cell(&self) -> usize {
        self.max_per_cell
    }

    /// Adds one mark to a date.
    ///
    /// Returns `false` for an impossible date or when the date already holds
    /// [`Self::max_per_cell()`] marks; no existing mark is displaced.
    pub fn set_mark(&mut self, year: i32, month: u32, day: u32, mark: CalendarMark) -> bool {
        if day == 0 || day > days_in_month(year, month) {
            return false;
        }
        if self.max_per_cell == 0 {
            return false;
        }
        let marks = self.marks.entry((year, month, day)).or_default();
        if marks.len() >= self.max_per_cell {
            return false;
        }
        marks.push(mark);
        self.base.request_redraw();
        true
    }

    /// Removes every mark from one date, reporting whether there was anything to remove.
    pub fn clear_date(&mut self, year: i32, month: u32, day: u32) -> bool {
        let removed = self.marks.remove(&(year, month, day)).is_some();
        if removed {
            self.base.request_redraw();
        }
        removed
    }

    /// Removes every mark from every date.
    pub fn clear_all(&mut self) {
        if self.marks.is_empty() {
            return;
        }
        self.marks.clear();
        self.base.request_redraw();
    }

    /// Returns a copy of one date's marks, in insertion order.
    ///
    /// An impossible date, or one with no marks, returns an empty vector.
    pub fn marks_for(&self, year: i32, month: u32, day: u32) -> Vec<CalendarMark> {
        self.marks.get(&(year, month, day)).cloned().unwrap_or_default()
    }

    /// Returns how many marks every date holds in total.
    pub fn mark_count(&self) -> usize {
        self.marks.values().map(|marks| marks.len()).sum()
    }

    /// Returns a date's cell rectangle when that date belongs to the drawn month.
    pub fn day_rect(&self, day: u32) -> Option<Rect> {
        let days = days_in_month(self.year, self.month);
        if day == 0 || day > days {
            return None;
        }
        let rect = self.geometry();
        let cell_width = (rect.width / CALENDAR_COLUMNS).max(1);
        let cell_height = (rect.height / CALENDAR_ROWS).max(1);
        let slot = weekday_index(self.year, self.month, 1) + day - 1;
        if slot >= CALENDAR_COLUMNS * CALENDAR_ROWS {
            return None;
        }
        let column = slot % CALENDAR_COLUMNS;
        let row = slot / CALENDAR_COLUMNS;
        Some(Rect::new(
            rect.x + (column * cell_width) as i32,
            rect.y + (row * cell_height) as i32,
            cell_width,
            cell_height,
        ))
    }

    /// The date under `pos`, if the point is on a day of the drawn month.
    fn date_at(&self, pos: Point) -> Option<(i32, u32, u32)> {
        let rect = self.geometry();
        if !rect.contains(pos) || rect.width == 0 || rect.height == 0 {
            return None;
        }
        let cell_width = (rect.width / CALENDAR_COLUMNS).max(1);
        let cell_height = (rect.height / CALENDAR_ROWS).max(1);
        let column = ((pos.x - rect.x).max(0) as u32 / cell_width).min(CALENDAR_COLUMNS - 1);
        let row = ((pos.y - rect.y).max(0) as u32 / cell_height).min(CALENDAR_ROWS - 1);
        let slot = row * CALENDAR_COLUMNS + column;
        let offset = weekday_index(self.year, self.month, 1);
        if slot < offset {
            return None;
        }
        let day = slot - offset + 1;
        (day <= days_in_month(self.year, self.month)).then_some((self.year, self.month, day))
    }

    /// The date and mark index under `pos`.
    ///
    /// A click in the day-number band counts as the first mark, so a short cell still has a
    /// usable activation target; a click on a mark lane selects that lane.
    fn mark_at(&self, pos: Point) -> Option<(i32, u32, u32, usize)> {
        let (year, month, day) = self.date_at(pos)?;
        let marks = self.marks.get(&(year, month, day))?;
        let count = marks.len().min(self.max_per_cell);
        if count == 0 {
            return None;
        }
        let cell = self.day_rect(day)?;
        let index = (0..count)
            .find(|&candidate| mark_lane_rect(cell, candidate, count).contains(pos))
            .unwrap_or(0);
        Some((year, month, day, index))
    }
}

impl EventHandler for CalendarMarkLayer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            let Some((year, month, day, index)) = self.mark_at(*pos) else {
                return;
            };
            let Some(mark) =
                self.marks.get(&(year, month, day)).and_then(|marks| marks.get(index)).cloned()
            else {
                return;
            };
            self.mark_activated.emit((year, month, day, mark));
        }
    }
}

impl Draw for CalendarMarkLayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        let grid = surface.blend(&ink, 0.10);
        // The empty slots are part of the month's shape, so they are painted too.
        let cell_width = (rect.width / CALENDAR_COLUMNS).max(1);
        let cell_height = (rect.height / CALENDAR_ROWS).max(1);
        for row in 0..CALENDAR_ROWS {
            for column in 0..CALENDAR_COLUMNS {
                let cell = Rect::new(
                    rect.x + (column * cell_width) as i32,
                    rect.y + (row * cell_height) as i32,
                    cell_width,
                    cell_height,
                );
                context.draw_rect(cell, grid);
            }
        }

        let days = days_in_month(self.year, self.month);
        // One reused buffer for the day numbers: `format!` built a fresh `String` per cell.
        let mut day_text = String::new();
        for day in 1..=days {
            let Some(cell) = self.day_rect(day) else {
                continue;
            };
            day_text.clear();
            let _ = core::fmt::Write::write_fmt(&mut day_text, format_args!("{day}"));
            let label_rect = Rect::new(
                cell.x + 3,
                cell.y + 1,
                cell.width.saturating_sub(6),
                DAY_LABEL_HEIGHT.min(cell.height),
            );
            context.draw_text_fitted_cow(
                label_rect,
                &day_text,
                font,
                surface.blend(&ink, 0.65),
                HorizontalAlignment::Left,
            );
            let Some(marks) = self.marks.get(&(self.year, self.month, day)) else {
                continue;
            };
            let count = marks.len().min(self.max_per_cell);
            for (index, mark) in marks.iter().take(count).enumerate() {
                draw_mark(context, mark_lane_rect(cell, index, count), mark, font, accent);
            }
        }
    }
}

impl Widget for CalendarMarkLayer {
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
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(280, 240),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "mark_activated" => {
                Some(EventSignalRef::mapped("mark_activated", &self.mark_activated, |value| {
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::Int(i64::from(value.0)),
                        CapabilityValue::UInt(u64::from(value.1)),
                        CapabilityValue::UInt(u64::from(value.2)),
                        mark_value(&value.3),
                    ])
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for CalendarMarkLayer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "year" => Ok(CapabilityValue::Int(i64::from(self.year))),
            "month" => Ok(CapabilityValue::UInt(u64::from(self.month))),
            "max_per_cell" => Ok(CapabilityValue::UInt(self.max_per_cell as u64)),
            "mark_count" => Ok(CapabilityValue::UInt(self.mark_count() as u64)),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "year" => {
                let year = expect_i32_saturating(value)?;
                self.set_month(year, self.month);
                Ok(())
            }
            "month" => {
                let month = expect_u32(value)?;
                if self.set_month(self.year, month) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::ConstraintViolation {
                        property: String::from("month"),
                        reason: String::from("month must be in 1..=12"),
                    })
                }
            }
            "max_per_cell" => {
                self.set_max_per_cell(expect_usize(value)?);
                Ok(())
            }
            "mark_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["year", "month", "max_per_cell", "mark_count", BASE_PROPERTY_NAMES]
    }
}

/// Encodes one mark as `[colour, shape, badge text]`.
fn mark_value(mark: &CalendarMark) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::Color(mark.color),
        CapabilityValue::String(mark.shape.as_str().to_string()),
        CapabilityValue::String(mark.badge_text.clone()),
    ])
}

/// The horizontal strip a mark occupies inside its date cell.
fn mark_lane_rect(cell: Rect, index: usize, count: usize) -> Rect {
    let label = DAY_LABEL_HEIGHT.min(cell.height);
    let available = cell.height.saturating_sub(label);
    let lane_height = (available / count.max(1) as u32).max(1);
    Rect::new(
        cell.x + 2,
        cell.y + label as i32 + (index as u32 * lane_height) as i32,
        cell.width.saturating_sub(4),
        lane_height,
    )
}

/// Paints one mark in its lane.
fn draw_mark(
    context: &mut RenderContext,
    lane: Rect,
    mark: &CalendarMark,
    font: &crate::core::Font,
    fallback: Color,
) {
    let color = if mark.color.a == 0 { fallback } else { mark.color };
    match mark.shape {
        CalendarMarkShape::Dot => {
            let center =
                Point::new(lane.x + lane.width as i32 / 2, lane.y + lane.height as i32 / 2);
            let radius = (lane.height.saturating_sub(2) / 2).clamp(1, 4);
            context.fill_circle(center, radius, color);
        }
        CalendarMarkShape::Ring => {
            let center =
                Point::new(lane.x + lane.width as i32 / 2, lane.y + lane.height as i32 / 2);
            let radius = (lane.height.saturating_sub(2) / 2).clamp(1, 4);
            context.draw_circle(center, radius, color);
        }
        CalendarMarkShape::Underline => {
            let bottom = lane.y + lane.height as i32 - 1;
            context.draw_line_stroke(
                Point::new(lane.x, bottom),
                Point::new(lane.x + lane.width.saturating_sub(1) as i32, bottom),
                color,
                2,
            );
        }
        CalendarMarkShape::Badge => {
            context.fill_rounded_rect(lane, 2, color);
            if !mark.badge_text.is_empty() {
                let text_rect =
                    Rect::new(lane.x + 2, lane.y, lane.width.saturating_sub(4), lane.height);
                context.draw_text_fitted_cow(
                    text_rect,
                    mark.badge_text.as_str(),
                    font,
                    color.contrast_color(),
                    HorizontalAlignment::Center,
                );
            }
        }
    }
}

/// Days in `month`, or `0` when the month is outside `1..=12`.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Gregorian leap-year rule.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The Monday-based weekday index (`0` = Monday) of a date.
///
/// Uses Sakamoto's congruence, which needs no lookup table and handles leap years through the
/// same month offsets a calendar would use.
fn weekday_index(year: i32, month: u32, day: u32) -> u32 {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month = month.clamp(1, 12);
    let adjusted_year = if month < 3 { year - 1 } else { year };
    let sunday_based = (adjusted_year + adjusted_year / 4 - adjusted_year / 100
        + adjusted_year / 400
        + OFFSETS[(month - 1) as usize]
        + day as i32)
        .rem_euclid(7);
    ((sunday_based + 6) % 7) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use std::sync::{Arc, Mutex};

    fn layer() -> CalendarMarkLayer {
        CalendarMarkLayer::new(Rect::new(0, 0, 280, 240))
    }

    fn red() -> CalendarMark {
        CalendarMark::new(Color::rgb(220, 60, 60), CalendarMarkShape::Dot)
    }

    fn blue() -> CalendarMark {
        CalendarMark::new(Color::rgb(60, 90, 220), CalendarMarkShape::Ring)
    }

    #[test]
    fn marks_accumulate_per_date_without_touching_neighbours() {
        let mut layer = layer();
        assert!(layer.set_mark(2026, 3, 14, red()));
        assert!(layer.set_mark(2026, 3, 14, blue()));
        assert!(layer.set_mark(2026, 3, 15, red()));

        assert_eq!(layer.marks_for(2026, 3, 14), Vec::from([red(), blue()]));
        assert_eq!(layer.marks_for(2026, 3, 15), Vec::from([red()]));
        assert!(layer.marks_for(2026, 3, 16).is_empty());
        assert_eq!(layer.mark_count(), 3);
    }

    #[test]
    fn max_per_cell_clamps_further_marks() {
        let mut layer = layer();
        layer.set_max_per_cell(2);
        assert!(layer.set_mark(2026, 3, 14, red()));
        assert!(layer.set_mark(2026, 3, 14, blue()));
        assert!(!layer.set_mark(2026, 3, 14, red()), "the third mark is refused");
        assert_eq!(layer.marks_for(2026, 3, 14).len(), 2);

        // Lowering the cap trims dates that are already over it.
        layer.set_max_per_cell(1);
        assert_eq!(layer.marks_for(2026, 3, 14), Vec::from([red()]));
        assert_eq!(layer.max_per_cell(), 1);
    }

    #[test]
    fn clear_removes_one_date_or_the_whole_layer() {
        let mut layer = layer();
        layer.set_mark(2026, 3, 14, red());
        layer.set_mark(2026, 3, 15, blue());
        assert!(layer.clear_date(2026, 3, 14));
        assert!(layer.marks_for(2026, 3, 14).is_empty());
        assert_eq!(layer.marks_for(2026, 3, 15).len(), 1);
        assert!(!layer.clear_date(2026, 3, 14), "clearing twice is a no-op");

        layer.clear_all();
        assert_eq!(layer.mark_count(), 0);
        assert!(layer.marks_for(2026, 3, 15).is_empty());
    }

    #[test]
    fn a_click_reports_the_date_and_the_mark() {
        let mut layer = layer();
        assert!(layer.set_month(2026, 3));
        layer.set_mark(2026, 3, 14, blue());
        let seen = Arc::new(Mutex::new(Vec::<(i32, u32, u32, CalendarMark)>::new()));
        let sink = Arc::clone(&seen);
        layer.mark_activated.connect(move |value| sink.lock().unwrap().push((*value).clone()));

        let cell = layer.day_rect(14).expect("the 14th is in March 2026");
        let pos = Point::new(cell.x + cell.width as i32 / 2, cell.y + cell.height as i32 / 2);
        layer.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });

        let fired = seen.lock().unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].0, 2026);
        assert_eq!(fired[0].1, 3);
        assert_eq!(fired[0].2, 14);
        assert_eq!(fired[0].3, blue());
    }

    #[test]
    fn a_click_on_an_unmarked_date_reports_nothing() {
        let mut layer = layer();
        let seen = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&seen);
        layer.mark_activated.connect(move |_| *sink.lock().unwrap() += 1);

        let cell = layer.day_rect(1).expect("the first is in January 2026");
        layer.handle_event(&Event::MousePress {
            pos: Point::new(cell.x + 4, cell.y + 4),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*seen.lock().unwrap(), 0);
    }

    #[test]
    fn impossible_dates_are_refused() {
        let mut layer = layer();
        assert!(!layer.set_mark(2026, 2, 30, red()));
        assert!(!layer.set_mark(2026, 13, 1, red()));
        assert!(!layer.set_mark(2026, 1, 0, red()));
        assert!(layer.set_mark(2024, 2, 29, red()), "2024 is a leap year");
        assert!(!layer.set_mark(2026, 2, 29, red()));
        assert_eq!(layer.mark_count(), 1);
    }

    #[test]
    fn the_month_selects_both_drawing_and_hit_testing() {
        let mut layer = layer();
        assert!(layer.set_month(2026, 3));
        assert_eq!(layer.year(), 2026);
        assert_eq!(layer.month(), 3);
        // March 1 2026 is a Sunday, so Monday-based indexing puts it in the seventh slot.
        let first = layer.day_rect(1).expect("the first is in March 2026");
        assert_eq!(first.x, 6 * (280 / 7));
        assert!(!layer.set_month(2026, 0), "month zero is refused");
        assert!(!layer.set_month(2026, 13), "month thirteen is refused");
        assert_eq!(layer.month(), 3, "a refused month leaves the selection alone");
    }
}
