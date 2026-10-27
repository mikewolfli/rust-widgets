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
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Width reserved at the leading edge of a column for its up/down hints, so a hint never shares
/// pixels with the row's value.
const ARROW_GUTTER: u32 = 10;

/// The in-progress pointer drag over one drum column.
///
/// A drag scrolls the column under the press; the last sample is kept so each `MouseMove` only
/// contributes whole rows instead of re-deriving the gesture from the control's origin.
#[derive(Clone, Copy)]
struct DragScroll {
    /// The column the press started in: `0` year, `1` month, `2` day.
    column: usize,
    /// The y of the last sample that was consumed as whole rows.
    last_y: i32,
}

/// Mobile-style date picker with year/month/day column spinners.
///
/// Presents three side-by-side columns for selecting year, month, and day.
/// The currently selected value in each column is visually highlighted.
pub struct MobileDatePicker {
    base: BaseWidget,
    year: i32,
    month: u32,
    day: u32,
    /// The three drum columns, rebuilt only when the date changes. They used to be built
    /// (roughly 48 `String` allocations) inside `Draw::draw`, i.e. every frame.
    year_items: Vec<String>,
    month_items: Vec<String>,
    day_items: Vec<String>,
    /// Half-width of the year drum around the selected year.
    year_span: u32,
    /// First year in the drum.
    year_start: i32,
    /// Last year in the drum.
    year_end: i32,
    /// The day count `day_items` was built for, so a month change that does not move it can skip
    /// the rebuild (K-P13).
    day_items_count: u32,
    /// The measured line height of the row text, keyed by the font it was measured with (K-P13).
    row_text_metrics: crate::compat::RefCell<Option<((u32, u16, u64), u32)>>,
    /// The column the keyboard is on: `0` year, `1` month, `2` day (MISC-12).
    focus_column: usize,
    /// The last pointer position seen over the control, so a wheel notch can address the column
    /// under the pointer (the wheel event itself carries no position).
    pointer: Option<Point>,
    /// The active drag scroll, if any.
    drag: Option<DragScroll>,
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
            year_items: Vec::new(),
            month_items: Vec::new(),
            day_items: Vec::new(),
            year_span: 50,
            year_start: 2025 - 50,
            year_end: 2025 + 50,
            day_items_count: 31,
            row_text_metrics: crate::compat::RefCell::new(None),
            focus_column: 0,
            pointer: None,
            drag: None,
            date_changed: Signal1::new(),
        }
        .with_rebuilt_items()
    }

    /// Rebuilds the cached drum columns for the current date.
    ///
    /// Each column is rebuilt only when its own inputs moved (K-P13): the year list depends on the
    /// range, the month list is constant, and the day list only on how many days the month has. A
    /// day step inside the same month used to rebuild all three lists — the year column alone can
    /// be over a hundred strings.
    fn rebuild_drum_items(&mut self) {
        self.year = self.year.clamp(self.year_start, self.year_end);
        // The year list is a contiguous run, so a matching length means matching contents: a date
        // step (which is what calls this in practice) cannot change either end of the range.
        let expected_years =
            (self.year_end.saturating_sub(self.year_start).saturating_add(1)).max(0) as usize;
        if self.year_items.len() != expected_years {
            self.rebuild_year_items();
        }
        if self.month_items.is_empty() {
            self.month_items = MONTH_NAMES.iter().map(|&n| n.to_string()).collect();
        }
        self.rebuild_day_items();
    }

    /// Rebuilds only the year column.
    ///
    /// The range setters call this directly rather than going through
    /// [`Self::rebuild_drum_items`]: a range can move while keeping its length, so the length check
    /// there would keep a stale list.
    fn rebuild_year_items(&mut self) {
        // A range change can drop the cached list entirely; rebuilding it is the whole point.
        self.year_items = (self.year_start..=self.year_end).map(|y| y.to_string()).collect();
    }

    /// Rebuilds only the day column, and only when the month's length changed.
    fn rebuild_day_items(&mut self) {
        let max_day = days_in_month(self.year, self.month);
        if max_day == self.day_items_count && !self.day_items.is_empty() {
            return;
        }
        self.day_items = (1..=max_day).map(|d| format!("{d:02}")).collect();
        self.day_items_count = max_day;
    }

    /// The number of years offered on either side of the selected year.
    pub fn year_span(&self) -> u32 {
        self.year_span
    }

    /// Sets how many years the drum offers on either side of the selected year.
    pub fn set_year_span(&mut self, span: u32) {
        if self.year_span == span {
            return;
        }
        self.year_span = span;
        self.year_start = self.year.saturating_sub(span as i32);
        self.year_end = self.year.saturating_add(span as i32);
        // The range changed even if its length did not, so the year list is rebuilt explicitly.
        self.year = self.year.clamp(self.year_start, self.year_end);
        self.rebuild_year_items();
        self.rebuild_day_items();
        self.base.request_redraw();
    }

    /// Sets an explicit inclusive year range for the drum.
    ///
    /// A reversed range is normalised. The selected year is clamped into the range, so the
    /// wheel always has a row for the current value.
    pub fn set_year_range(&mut self, start: i32, end: i32) {
        let (start, end) = if start <= end { (start, end) } else { (end, start) };
        if self.year_start == start && self.year_end == end {
            return;
        }
        self.year_start = start;
        self.year_end = end;
        // A shifted range can keep its length, so rebuild the list rather than infer it.
        self.year = self.year.clamp(self.year_start, self.year_end);
        self.rebuild_year_items();
        self.rebuild_day_items();
        self.base.request_redraw();
    }

    /// The inclusive year range shown by the drum.
    pub fn year_range(&self) -> (i32, i32) {
        (self.year_start, self.year_end)
    }

    fn with_rebuilt_items(mut self) -> Self {
        self.rebuild_drum_items();
        self
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
            self.rebuild_drum_items();
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "date_changed" => {
                Some(EventSignalRef::mapped("date_changed", &self.date_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            _ => None,
        }
    }
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
            "year_span" => Ok(CapabilityValue::UInt(u64::from(self.year_span))),
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
            "year_span" => {
                let span = match value {
                    CapabilityValue::UInt(value) => value,
                    CapabilityValue::Int(value) if value >= 0 => value as u64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_year_span(u32::try_from(span).unwrap_or(u32::MAX));
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_date", "year_span", BASE_PROPERTY_NAMES]
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
        // The style lookups are the cached, revision-validated forms, so the whole frame reads the
        // palette once and neither lookup takes the manager lock (K-P17).
        let style = self.base.style();
        // `mobile_date_picker` is not in the role table, so it classifies as `Surface` and its
        // resolved background is the window fill itself; the drum below therefore derives its own
        // distinct surface rather than painting the window's.
        let theme = crate::style::resolved_theme_style_shared("mobile_date_picker");
        let (window_fill, foreground, primary, secondary, disabled) =
            match crate::style::palette_shared() {
                Some(colors) => (
                    colors.background,
                    colors.foreground,
                    colors.primary,
                    colors.secondary,
                    colors.disabled,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                    Color::rgb(200, 200, 200),
                ),
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
        // The numbers: the selected row sits **on the highlight bar**, so its ink has to be
        // chosen against that bar rather than being the token the bar was built from. Deriving
        // both from `primary` made the pair nearly the same colour — on the light theme the
        // selected `rgb(33,150,243)` on the bar's `rgb(172,205,231)` measured 1.88:1, so the
        // selected value was the least readable row in a picker whose whole job is to show
        // which value is selected. The other rows are de-emphasised steps of the separator
        // colour, and a disabled picker recedes from the drum — none of them a fixed grey a
        // dark theme would swallow.
        let selected_ink =
            if is_enabled { highlight.contrast_color() } else { drum.blend(&disabled, 0.55) };
        let row_ink = if is_enabled { ink.blend(&drum, 0.45) } else { drum.blend(&disabled, 0.50) };
        let arrow_color = if is_enabled { ink.blend(&drum, 0.20) } else { row_ink };

        // Background
        //
        // `rect` is the area the picker was *given*; the drum is a fixed-row wheel, so its
        // height is [`dimensions::PICKER_VISIBLE_ROWS`] rows of
        // [`dimensions::PICKER_ROW_HEIGHT`] rather than a fifth of whatever height the caller
        // supplied. That is what stops a 120 px cell from compressing five rows to 24 px each
        // and drawing them at a 10 px font. The band is centred and clamped to the rectangle,
        // so a picker shorter than five rows still fits inside it.
        let drum_rect = ControlMetrics::center_in(
            rect,
            Size::new(rect.width, dimensions::PICKER_ROW_HEIGHT * dimensions::PICKER_VISIBLE_ROWS),
        );
        context.fill_rect(drum_rect, drum);

        // Column layout
        //
        // The column width still divides the available width (a wheel fills its panel
        // horizontally), but the row height is the picker's own fact.
        let col_width = drum_rect.width / 3;
        let row_height = (drum_rect.height / dimensions::PICKER_VISIBLE_ROWS).max(1);
        // The font follows the row **as painted**, so a band clamped shorter than the nominal
        // five rows shrinks its type with it instead of overflowing the rows it sits in.
        let font_size = (row_height as f32 * 0.38).clamp(9.0, 15.0);
        let font = role_font(style, font_size);
        let arrow_font = role_font(style, (font_size * 1.3).max(10.0)).with_bold(true);

        // Prepare column data: (offset_into_items, visible_items_list). The lists are cached
        // fields rebuilt by `set_date`, not rebuilt here.
        let year_offset: i32 = (self.year - self.year_start).max(0);
        let month_offset = (self.month as usize - 1) as i32;
        let day_offset = (self.day as usize - 1) as i32;

        let columns: [(i32, &[String], &str); 3] = [
            (year_offset, self.year_items.as_slice(), "Year"),
            (month_offset, self.month_items.as_slice(), "Month"),
            (day_offset, self.day_items.as_slice(), "Day"),
        ];

        // The today marker is read once for the whole frame rather than once per column: the three
        // columns ask the same clock the same question, and a date rollover between two columns
        // could otherwise mark two different days in one picture (MISC-01).
        let today = chrono::Local::now().date_naive();

        for (col_idx, (sel_offset, items, _label)) in columns.iter().enumerate() {
            let col_x = drum_rect.x + (col_idx as u32 * col_width) as i32;
            // The row's line height is one number for the whole font, so it is measured once per
            // font rather than once per visible row per column (K-P13).
            let row_text_height = self.row_text_height(&font, context);
            let is_today_column = |item_index: i32| -> bool {
                use chrono::Datelike;
                match col_idx {
                    0 => self.year_start + item_index == today.year(),
                    1 => item_index + 1 == today.month() as i32,
                    _ => item_index + 1 == today.day() as i32,
                }
            };

            // Vertical rule between columns. Drawn as a *line* rather than a 1 px wide
            // rectangle: `draw_rect_stroke` on a 1-wide rect emits a degenerate element
            // (`width="1"` in the SVG) whose own two outline strokes overdraw it, so the
            // division read as a module rather than as a rule.
            if col_idx > 0 {
                context.draw_line(
                    Point::new(col_x, drum_rect.y),
                    Point::new(col_x, drum_rect.y + drum_rect.height as i32),
                    divider,
                );
            }

            // Highlight bar for the center (selected) row
            let highlight_y = drum_rect.y + 2 * row_height as i32;
            let highlight_rect =
                Rect::new(col_x + 4, highlight_y, col_width.saturating_sub(8), row_height);
            context.fill_rounded_rect(highlight_rect, 6, highlight);
            // The keyboard's column is outlined while the control has focus, so the arrows have a
            // visible target (MISC-12's focus, the ring every other control uses).
            if self.base.draws_focus_ring() && col_idx == self.focus_column {
                context.draw_rounded_rect_stroke(highlight_rect, 6, selected_ink, 1);
            }

            // Draw the five visible rows
            for row in 0..dimensions::PICKER_VISIBLE_ROWS {
                let item_idx = row as i32 + (sel_offset - 2);
                if item_idx < 0 || item_idx >= items.len() as i32 {
                    continue;
                }

                let text = &items[item_idx as usize];
                let is_selected = row == 2;
                let item_y = drum_rect.y + (row * row_height) as i32;

                // Centred inside the row's own band. The old origin added `ascent` on top of
                // an already-centred y, so the glyph box (which starts at the origin and runs
                // a full line down) began half a line low — the fifth row was pushed past the
                // control and only its top edge was ever painted.
                let text_y = item_y + (row_height as i32 - row_text_height as i32) / 2;

                let text_color = if is_selected { selected_ink } else { row_ink };

                // The five visible rows are painted in the columns' full width *minus* the arrow
                // gutter. The hints at the top and bottom edge used to be drawn over the first and
                // last row's value ("2023" had a caret through it), because a five-row drum fills
                // its band completely and there is no spare row to put them in. Reserving a gutter
                // on the leading side is what keeps the two from sharing pixels at any drum height.
                let text_rect = Rect::new(
                    col_x + ARROW_GUTTER as i32,
                    text_y,
                    col_width.saturating_sub(ARROW_GUTTER * 2),
                    row_text_height,
                );

                context.draw_text_fitted_cow(
                    text_rect,
                    text,
                    &font,
                    text_color,
                    HorizontalAlignment::Center,
                );

                // Today is a reference point, not a selection: a small accent dot inside the row's
                // trailing edge, so it cannot be mistaken for the highlight bar (MISC-11). It sits
                // in the same gutter the rows leave free, so it never touches the value.
                if is_today_column(item_idx) {
                    context.fill_circle(
                        Point::new(
                            col_x + col_width as i32 - ARROW_GUTTER as i32 / 2,
                            item_y + row_height as i32 / 2,
                        ),
                        3,
                        primary,
                    );
                }
            }

            // Up arrow indicator (top of column), in the gutter the rows leave free rather than
            // over the first row's value.
            let up_y = drum_rect.y + 2;
            context.draw_text(
                Point::new(col_x + 2, up_y),
                "^",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );

            // Down arrow indicator (bottom of column), on the drum rather than on the control's
            // own bottom edge, which sat half a row below the last visible row.
            let down_y = drum_rect.y + drum_rect.height as i32 - row_height as i32 + 2;
            context.draw_text(
                Point::new(col_x + 2, down_y),
                "v",
                &arrow_font,
                arrow_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl MobileDatePicker {
    /// Applies `delta` steps to one drum column, through the same clamps every path uses.
    /// `delta` may be larger than one when a drag covers several rows.
    fn apply_column_delta(&mut self, column: usize, delta: i32) {
        if delta == 0 {
            return;
        }
        match column {
            0 => {
                let new_year = self.year.saturating_add(delta);
                let max_day = days_in_month(new_year, self.month);
                self.set_date(new_year, self.month, self.day.min(max_day));
            }
            1 => {
                let new_month = (self.month as i32 + delta).clamp(1, 12) as u32;
                let max_day = days_in_month(self.year, new_month);
                self.set_date(self.year, new_month, self.day.min(max_day));
            }
            _ => {
                let max_days = days_in_month(self.year, self.month) as i32;
                let new_day = (self.day as i32 + delta).clamp(1, max_days) as u32;
                self.set_date(self.year, self.month, new_day);
            }
        }
    }

    /// Steps the focused column by `delta`, with the same clamping every other path uses.
    fn step_column(&mut self, delta: i32) {
        self.apply_column_delta(self.focus_column, delta);
    }

    /// The height of one painted drum row, shared by the press, wheel and drag paths.
    fn drum_row_height(&self) -> i32 {
        let rect = self.geometry();
        let drum_rect = ControlMetrics::center_in(
            rect,
            Size::new(rect.width, dimensions::PICKER_ROW_HEIGHT * dimensions::PICKER_VISIBLE_ROWS),
        );
        (drum_rect.height / dimensions::PICKER_VISIBLE_ROWS).max(1) as i32
    }

    /// The line height of the drum's row text, measured once per font (K-P13).
    ///
    /// The draw loop asked `measure_text` once per visible row per column — fifteen measurements
    /// per frame — for a height that is a property of the font alone.
    fn row_text_height(&self, font: &crate::core::Font, context: &RenderContext) -> u32 {
        let mut family_hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in font.family().as_bytes() {
            family_hash ^= u64::from(*byte);
            family_hash = family_hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let key = (font.size().to_bits(), font.weight(), family_hash);
        if let Some((cached_key, height)) = *self.row_text_metrics.borrow() {
            if cached_key == key {
                return height;
            }
        }
        let height = context.measure_text("M", font).height;
        *self.row_text_metrics.borrow_mut() = Some((key, height));
        height
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
                // The wheel addresses the column under the last pointer position, so one gesture
                // scrolls whichever drum the user is aiming at (MISC-02). With no recorded pointer
                // it falls back to the keyboard's column, which keeps the historical year scroll.
                let column = self.pointer.map(|p| self.column_at(p.x)).unwrap_or(self.focus_column);
                if delta.y > 0 {
                    self.apply_column_delta(column, 1);
                } else if delta.y < 0 {
                    self.apply_column_delta(column, -1);
                }
            }
            // A pointer move over a pressed column drags that drum: every whole row of travel
            // contributes one step, so a drag and a wheel reach the same values through the same
            // clamp. The wheel event carries no position, so the move also records the pointer.
            Event::MouseMove { pos } => {
                self.pointer = Some(*pos);
                let Some(drag) = self.drag else {
                    return;
                };
                let row_height = self.drum_row_height();
                let travel = drag.last_y - pos.y;
                if travel.abs() >= row_height {
                    let steps = travel / row_height;
                    if let Some(active) = self.drag.as_mut() {
                        active.last_y -= steps * row_height;
                    }
                    self.apply_column_delta(drag.column, steps);
                }
            }
            Event::MouseRelease { .. } => {
                self.drag = None;
            }
            Event::MouseEnter { pos } => {
                self.pointer = Some(*pos);
            }
            Event::MousePress { pos, button: 1, .. } => {
                self.pointer = Some(*pos);
                let col = self.column_at(pos.x);
                // Pressing a column focuses it, so the keyboard continues from the column the user
                // touched and the drag below scrolls the same column.
                self.focus_column = col;
                self.drag = Some(DragScroll { column: col, last_y: pos.y });
                let rect = self.geometry();
                // The row is resolved against the same fixed-height drum the rows are painted
                // in, so a click maps to the value under the pointer rather than to a fraction of
                // the control's height — which picked the wrong row on every non-200 px panel.
                let row_height = self.drum_row_height();
                let drum_rect = ControlMetrics::center_in(
                    rect,
                    Size::new(
                        rect.width,
                        dimensions::PICKER_ROW_HEIGHT * dimensions::PICKER_VISIBLE_ROWS,
                    ),
                );
                let rel_y = pos.y - drum_rect.y;
                let row = rel_y / row_height;
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
            // The keyboard path (MISC-12): left/right move the focused column, up/down change that
            // column by one through the same `set_date` clamp and signal the wheel and click paths
            // use, and Enter re-announces the current date so a host can confirm it.
            Event::KeyPress { key, .. } => match *key {
                crate::event::key_codes::LEFT => {
                    self.focus_column = self.focus_column.saturating_sub(1);
                    self.base.request_redraw();
                }
                crate::event::key_codes::RIGHT => {
                    self.focus_column = (self.focus_column + 1).min(2);
                    self.base.request_redraw();
                }
                crate::event::key_codes::UP => {
                    self.step_column(1);
                }
                crate::event::key_codes::DOWN => {
                    self.step_column(-1);
                }
                crate::event::key_codes::ENTER => {
                    let date = self.date_string();
                    self.date_changed.emit(date);
                }
                _ => {}
            },
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
        Arc, Mutex,
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
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.year(), 2026);

        // Click lower half of year column -> decrement year
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 150),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_mouse_press_month_column() {
        let mut picker = make_picker();
        // Click upper half of month column -> increment month
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.month(), 2);

        // Click lower half -> decrement month
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 150),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.month(), 1);
    }

    #[test]
    fn picker_mouse_press_day_column() {
        let mut picker = make_picker();
        // Click upper half of day column -> increment day
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.day(), 2);

        // Click lower half -> decrement day
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 150),
            button: 1,
            modifiers: 0,
        });
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

        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 30),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.year(), 2025);

        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.year(), 2025);
    }

    #[test]
    fn picker_other_button_noop() {
        let mut picker = make_picker();
        picker.handle_event(&Event::MousePress {
            pos: Point::new(20, 30),
            button: 2,
            modifiers: 0,
        });
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
            modifiers: 0,
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
            modifiers: 0,
        });
        assert_eq!(picker.day(), 1);

        // Test max day of a 31-day month, increment should stay at 31
        picker.set_date(2025, 1, 31);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 30), // upper half of day column -> increment
            button: 1,
            modifiers: 0,
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
            modifiers: 0,
        });
        assert_eq!(picker.month(), 1);

        // December, increment month -> should stay at 12
        picker.set_date(2025, 12, 15);
        picker.handle_event(&Event::MousePress {
            pos: Point::new(100, 30), // upper half of month column -> increment
            button: 1,
            modifiers: 0,
        });
        assert_eq!(picker.month(), 12);
    }

    /// The drum's rows are a fixed height, not a fifth of whatever the caller supplied.
    ///
    /// The defect this pins: `row_height` was `rect.height / 5` and the font was derived from
    /// the *nominal* 32 px row, so a 120 px control drew 24 px rows carrying 12 px type and a
    /// 15.8 px arrow — text overflowing the row it sat in. Both the row and the font now follow
    /// the drum as painted.
    #[test]
    fn the_drum_rows_are_a_fixed_height() {
        let mut picker = MobileDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_date(2025, 6, 15);
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        // 5 rows of PICKER_ROW_HEIGHT tile the drum exactly.
        assert!(
            svg.contains("height=\"32\""),
            "a row must be PICKER_ROW_HEIGHT tall, not a fraction of the panel"
        );
    }

    #[test]
    fn the_year_drum_span_is_configurable() {
        let mut picker = MobileDatePicker::new(Rect::new(0, 0, 300, 200));
        assert_eq!(picker.year_span(), 50);
        assert_eq!(picker.year_range().0, picker.year() - 50);
        picker.set_year_range(1990, 2030);
        assert_eq!(picker.year_range(), (1990, 2030));
        picker.set_year_span(5);
        assert_eq!(picker.year_range(), (picker.year() - 5, picker.year() + 5));
        assert_eq!(picker.get("year_span"), Ok(CapabilityValue::UInt(5)));
    }

    /// The arrow keys move a focus column and change that column only (MISC-12).
    #[test]
    fn the_keyboard_steps_the_focused_column() {
        let mut picker = make_picker();
        picker.set_date(2026, 6, 15);

        // The year column is focused initially.
        picker.handle_event(&Event::key_press(crate::event::key_codes::UP, 0));
        assert_eq!(picker.year(), 2027);
        assert_eq!(picker.month(), 6);

        picker.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        picker.handle_event(&Event::key_press(crate::event::key_codes::UP, 0));
        assert_eq!(picker.month(), 7, "the focus moved to the month column");
        assert_eq!(picker.year(), 2027);

        picker.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        picker.handle_event(&Event::key_press(crate::event::key_codes::DOWN, 0));
        assert_eq!(picker.day(), 14, "and then to the day column");
    }

    /// Enter re-announces the current date, which is the keyboard's confirm path.
    #[test]
    fn enter_re_emits_the_date() {
        let confirmed = Arc::new(Mutex::new(Vec::new()));
        let mut picker = make_picker();
        let sink = Arc::clone(&confirmed);
        picker.date_changed.connect(move |value| sink.lock().unwrap().push(value.to_string()));
        picker.set_date(2026, 3, 9);
        picker.handle_event(&Event::key_press(crate::event::key_codes::ENTER, 0));
        assert_eq!(confirmed.lock().unwrap().last().map(String::as_str), Some("2026-03-09"));
    }

    /// A wheel notch addresses the column under the pointer, not always the year (MISC-02).
    #[test]
    fn the_wheel_scrolls_the_column_under_the_pointer() {
        let mut picker = make_picker();
        // Column width is 80 px: x = 180 is the day column.
        picker.handle_event(&Event::MouseMove { pos: Point::new(180, 100) });
        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.day(), 2);
        assert_eq!(picker.month(), 1);
        assert_eq!(picker.year(), 2025);

        // Moving over the month column retargets the same gesture.
        picker.handle_event(&Event::MouseMove { pos: Point::new(100, 100) });
        picker.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(picker.month(), 2);
        assert_eq!(picker.year(), 2025);
    }

    /// Dragging a column scrolls it by whole rows, and the drag ends on release (MISC-02).
    #[test]
    fn dragging_a_column_scrolls_it_by_rows() {
        let mut picker = make_picker();
        let row_height = picker.drum_row_height();
        // Press the day column's upper half: one step, and the drag starts here.
        picker.handle_event(&Event::MousePress {
            pos: Point::new(180, 70),
            button: 1,
            modifiers: 0,
        });
        let after_press = picker.day();
        assert_eq!(after_press, 2);

        // Dragging up by three rows is three more steps.
        picker.handle_event(&Event::MouseMove { pos: Point::new(180, 70 - 3 * row_height) });
        assert_eq!(picker.day(), after_press + 3);
        assert_eq!(picker.year(), 2025);

        // A release ends the gesture: later motion does not keep stepping.
        picker.handle_event(&Event::MouseRelease { pos: Point::new(180, 10), button: 1 });
        picker.handle_event(&Event::MouseMove { pos: Point::new(180, 70 - 6 * row_height) });
        assert_eq!(picker.day(), after_press + 3);
    }

    /// The clock is read once per frame, so the today dot still marks the wall-clock day.
    #[test]
    fn the_today_marker_is_painted_for_the_wall_clock_day() {
        use chrono::Datelike;
        let today = chrono::Local::now().date_naive();
        let mut picker = MobileDatePicker::new(Rect::new(0, 0, 300, 200));
        picker.set_date(today.year(), today.month(), today.day());
        let svg = crate::widget::svg::render_to_svg(&mut picker);
        assert!(svg.contains("<circle"), "the today dot must be painted: {svg}");
    }
}
