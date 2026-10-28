// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `WebHistoryPanel` — the address-bar history drop-down for a web view (COMP-A5).
//!
//! `WebEngineView` records visit history (`history_entries`, `history_index`, `go_back`,
//! `go_forward`) but shipped no panel to show it, so an address-bar drop-down had to be
//! assembled by the host from scratch. This companion is that panel: it takes the engine's own
//! [`HistoryEntry`] values, paints title/URL/timestamp rows, groups them by day when asked, and
//! emits the URL a row activates.
//!
//! # Times are supplied, never invented
//!
//! [`HistoryEntry`] carries no timestamp — the engine records *what* was visited, not *when*.
//! The panel therefore stores an optional visit time beside each entry. [`set_entries`] leaves
//! that time unknown (and the panel then paints no timestamp), while [`set_records`] attaches
//! the times a host already knows. Both grouping and the time column read only those supplied
//! values; nothing is stamped with an invented clock reading.
//!
//! [`set_entries`]: WebHistoryPanel::set_entries

use rust_widgets::compat::{format, String, ToString, Vec};
use rust_widgets::impl_draw_bridge;
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_widget_property_hooks;
use rust_widgets::property_names_of;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::coercion::{
    expect_bool, expect_string, expect_u32_saturating, expect_usize,
};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::web_engine::{HistoryEntry, WebEngineView};
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Height of one history row when the host does not choose one, in logical pixels.
const DEFAULT_ROW_HEIGHT: u32 = 44;

/// Height of a day-group header, in logical pixels.
const GROUP_HEADER_HEIGHT: u32 = 24;

/// Inset from a row's edge to its text.
const PANEL_INSET: u32 = 8;

/// Vertical inset from a row's top edge to its first text line.
const ROW_INSET: u32 = 4;

/// Width reserved at a row's trailing edge for the `HH:MM` timestamp.
const TIMESTAMP_WIDTH: u32 = 44;

/// How many entries the panel shows when the host does not choose a limit.
const DEFAULT_MAX_ENTRIES: usize = 8;

/// The header that groups entries whose visit time is unknown.
const UNDATED_LABEL: &str = "Undated";

/// One history row plus the time the page was visited, when the host knows it.
///
/// The engine's [`HistoryEntry`] has no timestamp, so the companion keeps one beside it rather
/// than fabricating a time. An entry with `visited_ms: None` paints no time and, when grouping is
/// on, joins the [`UNDATED_LABEL`] group.
#[derive(Debug, Clone, PartialEq)]
pub struct WebHistoryRecord {
    /// The engine history entry this row shows.
    pub entry: HistoryEntry,
    /// Visit time in Unix milliseconds (UTC), or `None` when the host did not report one.
    pub visited_ms: Option<i64>,
}

impl WebHistoryRecord {
    /// Wraps an entry without a visit time.
    pub fn new(entry: HistoryEntry) -> Self {
        Self { entry, visited_ms: None }
    }

    /// Wraps an entry with the Unix-millisecond time it was visited.
    pub fn visited_at(entry: HistoryEntry, visited_ms: i64) -> Self {
        Self { entry, visited_ms: Some(visited_ms) }
    }

    /// The page URL.
    pub fn url(&self) -> &str {
        &self.entry.url
    }
}

/// A drop-down list of visited pages, newest first.
///
/// The panel is a companion rather than part of the engine: it owns no navigation state and
/// calls none of the engine's mutating APIs. Activating a row emits
/// [`Self::on_entry_activated`] with the URL; the host decides whether that means
/// `set_url`, `go_back` or opening a new tab.
pub struct WebHistoryPanel {
    base: BaseWidget,
    /// Every known record, oldest first — the engine's own order.
    records: Vec<WebHistoryRecord>,
    /// The URL prefix currently filtering the list; empty means no filter.
    filter: String,
    /// The most rows the panel shows.
    max_entries: usize,
    /// Whether a row paints its visit time.
    show_timestamps: bool,
    /// Whether rows are preceded by a per-day header.
    group_by_day: bool,
    /// Height of one entry row.
    row_height: u32,
    /// Emitted with the entry's URL when a row is activated.
    pub on_entry_activated: Signal1<String>,
}

/// A row the panel paints, with the rectangle it occupies.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RowKind {
    /// A day header carrying its label.
    Header(String),
    /// An entry row, naming its index in [`WebHistoryPanel::records`].
    Entry(usize),
}

/// One laid-out row. The same layout feeds the paint and the hit test, so a click cannot land on
/// a row other than the one drawn under the pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryRow {
    rect: Rect,
    kind: RowKind,
}

impl WebHistoryPanel {
    /// Creates an empty panel occupying `geometry`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "WebHistoryPanel"),
            records: Vec::new(),
            filter: String::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
            show_timestamps: false,
            group_by_day: false,
            row_height: DEFAULT_ROW_HEIGHT,
            on_entry_activated: Signal1::new(),
        }
    }

    /// The most rows the panel shows at once. Never less than one.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Caps the number of rows shown at once. A limit of zero is raised to one: a drop-down that
    /// can never show a row is indistinguishable from a missing panel.
    pub fn set_max_entries(&mut self, max_entries: usize) {
        let max_entries = max_entries.max(1);
        if self.max_entries == max_entries {
            return;
        }
        self.max_entries = max_entries;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether rows paint their visit time.
    pub fn show_timestamps(&self) -> bool {
        self.show_timestamps
    }

    /// Turns the `HH:MM` (UTC) time column on or off. Rows without a supplied time stay blank.
    pub fn set_show_timestamps(&mut self, show: bool) {
        if self.show_timestamps == show {
            return;
        }
        self.show_timestamps = show;
        self.base.request_redraw();
    }

    /// Whether rows are preceded by per-day headers.
    pub fn group_by_day(&self) -> bool {
        self.group_by_day
    }

    /// Turns per-day grouping on or off.
    pub fn set_group_by_day(&mut self, group: bool) {
        if self.group_by_day == group {
            return;
        }
        self.group_by_day = group;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The height of one entry row, in logical pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the height of one entry row, clamped to at least one pixel.
    pub fn set_row_height(&mut self, height: u32) {
        let height = height.max(1);
        if self.row_height == height {
            return;
        }
        self.row_height = height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Replaces the list with the engine's entries, all with an unknown visit time.
    ///
    /// The engine stores history oldest first; the panel displays it newest first.
    pub fn set_entries(&mut self, entries: &[HistoryEntry]) {
        self.records = entries.iter().cloned().map(WebHistoryRecord::new).collect();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Replaces the list with records that carry their visit times.
    pub fn set_records(&mut self, records: &[WebHistoryRecord]) {
        self.records = records.to_vec();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns every known record, in the engine's oldest-first order.
    pub fn records(&self) -> &[WebHistoryRecord] {
        &self.records
    }

    /// The URL prefix in force, or an empty string when nothing is filtered.
    pub fn filter_prefix(&self) -> &str {
        &self.filter
    }

    /// Shows only entries whose URL starts with `prefix`, ignoring ASCII case.
    ///
    /// An empty or whitespace-only prefix removes the filter. The filter applies to the URL
    /// alone: a title match is not a prefix match.
    pub fn filter_by_prefix(&mut self, prefix: &str) {
        let prefix = prefix.trim();
        if self.filter == prefix {
            return;
        }
        self.filter = prefix.to_string();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every entry and clears the filter.
    pub fn clear(&mut self) {
        if self.records.is_empty() && self.filter.is_empty() {
            return;
        }
        self.records.clear();
        self.filter.clear();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The number of rows the panel currently shows, after filtering and the entry cap.
    pub fn visible_count(&self) -> usize {
        self.visible_indices().len()
    }

    /// The URL of visible entry `index` (`0` is the newest), when the index names one.
    pub fn entry_url(&self, index: usize) -> Option<&str> {
        let record = self.visible_indices().get(index).copied()?;
        Some(self.records[record].url())
    }

    /// The visible entry index under `pos`, when a row is there.
    ///
    /// Day headers and empty space answer `None`; the index is the same one
    /// [`Self::entry_url`] and [`Self::activate_entry`] take.
    pub fn entry_at(&self, pos: Point) -> Option<usize> {
        if !self.geometry().contains_point(pos) {
            return None;
        }
        let mut entry_index = 0;
        for row in self.layout_rows() {
            match row.kind {
                RowKind::Entry(_) => {
                    if row.rect.contains_point(pos) {
                        return Some(entry_index);
                    }
                    entry_index += 1;
                }
                RowKind::Header(_) => {}
            }
        }
        None
    }

    /// Activates visible entry `index`, emitting its URL. Returns whether the index named one.
    pub fn activate_entry(&mut self, index: usize) -> bool {
        let Some(url) = self.entry_url(index).map(String::from) else {
            return false;
        };
        self.on_entry_activated.emit(url);
        true
    }

    /// Anchors the panel under `view`'s address bar and adopts the view's history.
    ///
    /// The bar's painted rectangle is authoritative once the view has drawn; before that the
    /// view has not recorded one, so the same `top_band` derivation the view itself uses is
    /// applied to its geometry. The panel keeps its own height and takes the bar's x, width and
    /// bottom edge, which is where a drop-down belongs.
    pub fn attach_web_view(&mut self, view: &mut WebEngineView) {
        let bar = view.url_bar_rect().unwrap_or_else(|| {
            ControlMetrics::top_band(view.geometry(), dimensions::WEB_URL_BAR_HEIGHT)
        });
        let current = self.geometry();
        self.base.set_geometry(Rect::new(bar.x, bar.bottom(), bar.width, current.height));
        self.set_entries(view.history_entries());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Record indices the panel shows, newest first, filtered and capped.
    fn visible_indices(&self) -> Vec<usize> {
        self.records
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, record)| self.matches_filter(record))
            .take(self.max_entries)
            .map(|(index, _)| index)
            .collect()
    }

    /// Whether `record` survives the URL prefix filter.
    fn matches_filter(&self, record: &WebHistoryRecord) -> bool {
        self.filter.is_empty() || starts_with_ignore_ascii_case(&record.entry.url, &self.filter)
    }

    /// Lays the visible records out top to bottom, inserting day headers when grouping is on.
    fn layout_rows(&self) -> Vec<HistoryRow> {
        let rect = self.geometry();
        let mut rows = Vec::new();
        let mut y = rect.y;
        let mut last_day: Option<Option<i64>> = None;
        for index in self.visible_indices() {
            let record = &self.records[index];
            if self.group_by_day {
                let day = record.visited_ms.map(day_index);
                if last_day != Some(day) {
                    let label = match record.visited_ms {
                        Some(ms) => day_label(ms),
                        None => UNDATED_LABEL.to_string(),
                    };
                    rows.push(HistoryRow {
                        rect: Rect::new(rect.x, y, rect.width, GROUP_HEADER_HEIGHT),
                        kind: RowKind::Header(label),
                    });
                    y += GROUP_HEADER_HEIGHT as i32;
                    last_day = Some(day);
                }
            }
            rows.push(HistoryRow {
                rect: Rect::new(rect.x, y, rect.width, self.row_height),
                kind: RowKind::Entry(index),
            });
            y += self.row_height as i32;
        }
        rows
    }
}

impl Widget for WebHistoryPanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        let row_height = self.row_height.max(1);
        Size::new(320, (self.visible_count() as u32).saturating_mul(row_height).max(row_height))
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "entry_activated" | "on_entry_activated" => {
                Some(EventSignalRef::mapped("entry_activated", &self.on_entry_activated, |url| {
                    CapabilityValue::String(url.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for WebHistoryPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "max_entries" => Ok(CapabilityValue::UInt(self.max_entries() as u64)),
            "show_timestamps" => Ok(CapabilityValue::Bool(self.show_timestamps())),
            "group_by_day" => Ok(CapabilityValue::Bool(self.group_by_day())),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "filter_prefix" => Ok(CapabilityValue::String(self.filter_prefix().to_string())),
            "visible_count" => Ok(CapabilityValue::UInt(self.visible_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "max_entries" => {
                self.set_max_entries(expect_usize(value)?);
                Ok(())
            }
            "show_timestamps" => {
                self.set_show_timestamps(expect_bool(value)?);
                Ok(())
            }
            "group_by_day" => {
                self.set_group_by_day(expect_bool(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32_saturating(value)?);
                Ok(())
            }
            "filter_prefix" => {
                let prefix = expect_string(value)?;
                self.filter_by_prefix(&prefix);
                Ok(())
            }
            "visible_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "max_entries",
            "show_timestamps",
            "group_by_day",
            "row_height",
            "filter_prefix",
            "visible_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for WebHistoryPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(250, 250, 250, 255));

        context.fill_rect(rect, surface);
        context.draw_rect(rect, ink.with_alpha(60));
        context.push_clip(rect.x, rect.y, rect.width, rect.height);

        let title_font = role_font(style, 12.5);
        let secondary_font = role_font(style, 11.0);
        for row in self.layout_rows() {
            match row.kind {
                RowKind::Header(label) => {
                    context.fill_rect(row.rect, ink.with_alpha(12));
                    let metrics = context.measure_text("M", &secondary_font);
                    let top =
                        row.rect.y + ((row.rect.height as i32 - metrics.height as i32) / 2).max(0);
                    let width = row.rect.width.saturating_sub(PANEL_INSET * 2);
                    if width > 0 {
                        context.draw_text_fitted_cow(
                            Rect::new(
                                row.rect.x + PANEL_INSET as i32,
                                top,
                                width,
                                metrics.height.max(1),
                            ),
                            &label,
                            &secondary_font,
                            ink.with_alpha(150),
                            HorizontalAlignment::Left,
                        );
                    }
                }
                RowKind::Entry(index) => {
                    let record = &self.records[index];
                    let title = if record.entry.title.is_empty() {
                        record.entry.url.as_str()
                    } else {
                        record.entry.title.as_str()
                    };
                    let title_metrics = context.measure_text("M", &title_font);
                    let secondary_metrics = context.measure_text("M", &secondary_font);
                    let title_h = title_metrics.height.max(1);
                    let timestamp_room = if self.show_timestamps { TIMESTAMP_WIDTH } else { 0 };
                    let text_width =
                        row.rect.width.saturating_sub(PANEL_INSET * 2 + timestamp_room);
                    let top = row.rect.y + ROW_INSET as i32;
                    if text_width > 0 {
                        context.draw_text_fitted_cow(
                            Rect::new(row.rect.x + PANEL_INSET as i32, top, text_width, title_h),
                            title,
                            &title_font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                        // The second line carries the URL. Its band is whatever the row has
                        // left below the title, so a short row clips the URL instead of
                        // painting it over the next row.
                        let url_top = top + title_h as i32;
                        let url_h = secondary_metrics.height.max(1);
                        let url_room = (row.rect.y + row.rect.height as i32 - ROW_INSET as i32)
                            .saturating_sub(url_top)
                            .max(0) as u32;
                        if url_room > 0 {
                            context.draw_text_fitted_cow(
                                Rect::new(
                                    row.rect.x + PANEL_INSET as i32,
                                    url_top,
                                    text_width,
                                    url_h.min(url_room),
                                ),
                                &record.entry.url,
                                &secondary_font,
                                ink.with_alpha(150),
                                HorizontalAlignment::Left,
                            );
                        }
                    }
                    if timestamp_room > 0 {
                        if let Some(visited_ms) = record.visited_ms {
                            let time = time_label(visited_ms);
                            context.draw_text_fitted_cow(
                                Rect::new(
                                    row.rect.x + row.rect.width as i32
                                        - TIMESTAMP_WIDTH as i32
                                        - PANEL_INSET as i32,
                                    top,
                                    TIMESTAMP_WIDTH,
                                    title_h,
                                ),
                                &time,
                                &secondary_font,
                                ink.with_alpha(120),
                                HorizontalAlignment::Right,
                            );
                        }
                    }
                }
            }
        }
        context.pop_clip();
    }
}

impl EventHandler for WebHistoryPanel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.entry_at(*pos) {
                self.activate_entry(index);
            }
        }
    }
}

/// Whether `text` begins with `prefix`, ignoring ASCII case.
///
/// `str::get` keeps the slice on a character boundary, so a multi-byte prefix cannot panic the
/// byte comparison.
fn starts_with_ignore_ascii_case(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len()).is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

/// Whole days since the Unix epoch for a Unix-millisecond timestamp.
fn day_index(ms: i64) -> i64 {
    ms.div_euclid(86_400_000)
}

/// `YYYY-MM-DD` (UTC) for a Unix-millisecond timestamp.
fn day_label(ms: i64) -> String {
    let (year, month, day) = civil_date(day_index(ms));
    format!("{year:04}-{month:02}-{day:02}")
}

/// `HH:MM` (UTC) for a Unix-millisecond timestamp.
fn time_label(ms: i64) -> String {
    let seconds = ms.div_euclid(1_000).rem_euclid(86_400);
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    format!("{hours:02}:{minutes:02}")
}

/// Converts a day count since 1970-01-01 into a proleptic-Gregorian `(year, month, day)`.
///
/// This is Howard Hinnant's `civil_from_days` algorithm. The crate has no calendar dependency,
/// and a history header only needs a stable day label, so the arithmetic lives here rather than
/// pulling one in.
fn civil_date(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_prime + 2) / 5 + 1) as u32;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 } as u32;
    (year + i64::from(month <= 2), month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// One engine history entry with the fields a row paints.
    fn entry(url: &str, title: &str) -> HistoryEntry {
        HistoryEntry { url: url.to_string(), title: title.to_string(), scroll_pos: 0.0 }
    }

    /// A panel with three entries; the display order is newest first (c, b, a).
    fn three_entries() -> WebHistoryPanel {
        let mut panel = WebHistoryPanel::new(Rect::new(10, 40, 300, 200));
        panel.set_entries(&[
            entry("https://a.example/", "A"),
            entry("https://b.example/", "B"),
            entry("https://c.example/", "C"),
        ]);
        panel
    }

    /// A press on a painted row emits that row's URL and nothing else.
    #[test]
    fn a_history_row_press_emits_its_url() {
        let mut panel = three_entries();
        let activated = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&activated);
        panel.on_entry_activated.connect(move |url: Arc<String>| {
            sink.lock().unwrap().push((*url).clone());
        });

        // Newest first, so the second row is `b`.
        assert_eq!(panel.entry_url(0), Some("https://c.example/"));
        assert_eq!(panel.entry_at(Point::new(20, 40 + 20)), Some(0));
        assert_eq!(panel.entry_at(Point::new(20, 40 + DEFAULT_ROW_HEIGHT as i32 + 20)), Some(1));

        let pos = Point::new(20, 40 + DEFAULT_ROW_HEIGHT as i32 + 20);
        panel.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert_eq!(
            activated.lock().unwrap().as_slice(),
            [String::from("https://b.example/")],
            "the row under the pointer, not the first row, is the one activated"
        );

        // A press below the last row activates nothing.
        panel.handle_event(&Event::MousePress {
            pos: Point::new(20, 40 + (DEFAULT_ROW_HEIGHT * 5) as i32),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(activated.lock().unwrap().len(), 1);
    }

    /// The prefix filter narrows the list, ignoring ASCII case, and clears with an empty prefix.
    #[test]
    fn the_prefix_filter_narrows_the_visible_entries() {
        let mut panel = WebHistoryPanel::new(Rect::new(0, 0, 300, 200));
        panel.set_entries(&[
            entry("https://example.com/docs/a", "A"),
            entry("https://example.com/blog/b", "B"),
            entry("https://other.example/docs/c", "C"),
        ]);
        assert_eq!(panel.visible_count(), 3);

        panel.filter_by_prefix("HTTPS://EXAMPLE.COM/DOCS/");
        assert_eq!(panel.filter_prefix(), "HTTPS://EXAMPLE.COM/DOCS/");
        assert_eq!(panel.visible_count(), 1);
        assert_eq!(panel.entry_url(0), Some("https://example.com/docs/a"));

        // Filtering by title is not a prefix match.
        panel.filter_by_prefix("A");
        assert_eq!(panel.visible_count(), 0);

        panel.filter_by_prefix("   ");
        assert_eq!(panel.filter_prefix(), "");
        assert_eq!(panel.visible_count(), 3);
    }

    /// The entry cap keeps the newest rows and can never be zero.
    #[test]
    fn the_entry_cap_keeps_the_newest_rows() {
        let mut panel = three_entries();
        assert_eq!(panel.visible_count(), 3);

        panel.set_max_entries(2);
        assert_eq!(panel.max_entries(), 2);
        assert_eq!(panel.visible_count(), 2);
        assert_eq!(panel.entry_url(0), Some("https://c.example/"));
        assert_eq!(panel.entry_url(1), Some("https://b.example/"));
        assert_eq!(panel.entry_url(2), None);

        panel.set_max_entries(0);
        assert_eq!(panel.max_entries(), 1, "a zero-row drop-down is meaningless");
        assert_eq!(panel.visible_count(), 1);
        assert_eq!(panel.entry_url(0), Some("https://c.example/"));
    }

    /// Grouping inserts one header per day, newest day first, with supplied times only.
    #[test]
    fn grouping_inserts_one_header_per_day() {
        const DAY_MS: i64 = 86_400_000;
        let mut panel = WebHistoryPanel::new(Rect::new(0, 0, 320, 240));
        panel.set_records(&[
            WebHistoryRecord::visited_at(entry("https://a.example/", "A"), 0),
            WebHistoryRecord::visited_at(entry("https://b.example/", "B"), 60_000),
            WebHistoryRecord::visited_at(entry("https://c.example/", "C"), DAY_MS + 1_000),
        ]);
        panel.set_group_by_day(true);

        let rows = panel.layout_rows();
        let headers: Vec<&str> = rows
            .iter()
            .filter_map(|row| match &row.kind {
                RowKind::Header(label) => Some(label.as_str()),
                RowKind::Entry(_) => None,
            })
            .collect();
        assert_eq!(headers, ["1970-01-02", "1970-01-01"]);
        assert_eq!(rows.len(), 5, "two headers plus three entry rows");

        // Glyphs are painted as outlines, so the paint is checked as text runs: two headers plus
        // a title and a URL per entry.
        let without_times = rust_widgets::widget::svg::render_to_svg(&mut panel);
        let runs_without_times = rust_widgets::widget::svg::text_ink_boxes(&without_times).len();
        assert_eq!(runs_without_times, 8, "two headers + three titles + three URLs");

        // The time column adds exactly one run per entry that supplied a time.
        panel.set_show_timestamps(true);
        let with_times = rust_widgets::widget::svg::render_to_svg(&mut panel);
        assert_eq!(
            rust_widgets::widget::svg::text_ink_boxes(&with_times).len(),
            runs_without_times + 3,
            "each dated row paints its time"
        );
    }

    /// Entries without a supplied time group under a single honest header.
    #[test]
    fn entries_without_times_group_under_one_undated_header() {
        let mut panel = three_entries();
        panel.set_group_by_day(true);
        let headers: Vec<String> = panel
            .layout_rows()
            .into_iter()
            .filter_map(|row| match row.kind {
                RowKind::Header(label) => Some(label),
                RowKind::Entry(_) => None,
            })
            .collect();
        assert_eq!(headers, [String::from(UNDATED_LABEL)]);
        assert!(!panel.show_timestamps(), "no time is invented for an undated entry");
    }

    /// Attaching anchors the panel under the address bar and adopts the engine's history.
    #[test]
    fn attaching_a_view_positions_the_panel_and_loads_its_history() {
        let mut view = WebEngineView::new(Rect::new(5, 6, 320, 240));
        view.set_url("https://example.com/a".to_string());
        view.set_url("https://example.com/b".to_string());

        let mut panel = WebHistoryPanel::new(Rect::new(0, 0, 200, 120));
        panel.attach_web_view(&mut view);

        let bar = ControlMetrics::top_band(view.geometry(), dimensions::WEB_URL_BAR_HEIGHT);
        assert_eq!(panel.geometry(), Rect::new(bar.x, bar.bottom(), bar.width, 120));
        assert_eq!(panel.visible_count(), 2);
        assert_eq!(panel.entry_url(0), Some("https://example.com/b"));
        assert_eq!(panel.entry_url(1), Some("https://example.com/a"));

        panel.clear();
        assert_eq!(panel.visible_count(), 0);
        assert!(panel.records().is_empty());
    }
}
