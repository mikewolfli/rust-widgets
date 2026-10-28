//! `ChartDataTable` — a chart's series values as a previewable, exportable table (COMP-A26).

use crate::compat::{format, lock, Arc, Mutex, String, ToString, Vec};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_string_list, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

// ---------------------------------------------------------------------------
// COMP-A26 ChartDataTable
// ---------------------------------------------------------------------------

/// Inset around the table's cell area.
const TABLE_INSET: u32 = 4;
/// Height of the header row.
const HEADER_HEIGHT: u32 = 22;
/// Height of one data row.
const ROW_HEIGHT: u32 = 20;
/// Default cap on the rows a preview renders and exports.
const DEFAULT_MAX_ROWS: usize = 100;

/// A chart-side snapshot: the `(name, points)` series and the indices to show.
///
/// The visible list travels with the data because [`ChartDataTable::set_from_series`] seeds every
/// series visible; a chart that hides a series must not see its row reappear on the next data
/// change.
pub(crate) type ChartSeriesSnapshot = (Vec<(String, Vec<(f64, f64)>)>, Vec<usize>);

/// How much of the table a host is looking at.
///
/// [`ChartDataMode::Preview`] follows the on-screen preview: drawing and CSV both stop at
/// [`ChartDataTable::max_rows()`]. [`ChartDataMode::Export`] keeps the drawing contract but lets
/// [`ChartDataTable::to_csv`] emit every visible row, because an export is the point at which the
/// host wants the full data set rather than the clipped view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartDataMode {
    /// The clipped on-screen preview.
    #[default]
    Preview,
    /// The complete visible data set, as used by an export.
    Export,
}

impl ChartDataMode {
    /// The published token.
    pub fn as_str(self) -> &'static str {
        match self {
            ChartDataMode::Preview => "preview",
            ChartDataMode::Export => "export",
        }
    }

    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "preview" => Some(ChartDataMode::Preview),
            "export" => Some(ChartDataMode::Export),
            _ => None,
        }
    }
}

/// A chart's series values flattened into rows, with series visibility and CSV export.
///
/// # Model
///
/// [`Self::set_from_series`] accepts a slice of `(series name, points)` pairs and materialises one
/// row per point, under the fixed headers `Series`, `X` and `Y`. `visible_series` holds *series
/// indices*, not row indices: every data row remembers the series it came from, and hiding a
/// series removes all of that series' rows from the count, the drawing and the CSV in one step.
/// An empty `visible_series` therefore shows no rows; [`Self::set_from_series`] seeds it with all
/// series indices.
pub struct ChartDataTable {
    base: BaseWidget,
    headers: Vec<String>,
    /// Materialised data rows, aligned with `row_series`.
    rows: Vec<Vec<String>>,
    /// The series index that produced each row in `rows`.
    row_series: Vec<usize>,
    /// Series indices currently included.
    visible_series: Vec<usize>,
    /// How many series the last [`Self::set_from_series`] call supplied.
    series_count: usize,
    mode: ChartDataMode,
    max_rows: usize,
    /// Writer end of a chart-side snapshot queue, when a chart is attached (COMP-A8).
    pending_series: Option<Arc<Mutex<Option<ChartSeriesSnapshot>>>>,
    /// Emitted with the complete CSV document when [`Self::request_export`] is called.
    pub export_requested: Signal1<String>,
    /// Emitted with the series index after a data row toggled that series.
    pub series_toggled: Signal1<usize>,
}

impl ChartDataTable {
    /// Creates an empty table in [`ChartDataMode::Preview`].
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "ChartDataTable"),
            headers: Vec::new(),
            rows: Vec::new(),
            row_series: Vec::new(),
            visible_series: Vec::new(),
            series_count: 0,
            mode: ChartDataMode::Preview,
            max_rows: DEFAULT_MAX_ROWS,
            pending_series: None,
            export_requested: Signal1::new(),
            series_toggled: Signal1::new(),
        }
    }

    /// Rebuilds the table from a series snapshot.
    ///
    /// Each pair is `(series name, points)`, where every point is `(x, y)`. One row is emitted per
    /// point with the columns `Series`, `X`, `Y`; floats use their shortest `Display` spelling.
    /// Both axes are rendered as text, so a non-finite value is preserved rather than silently
    /// dropped. All series start visible.
    pub fn set_from_series(&mut self, series: &[(String, Vec<(f64, f64)>)]) {
        // Built in place: a vector created and then pushed into is `clippy::vec_init_then_push`.
        self.headers = Vec::from([String::from("Series"), String::from("X"), String::from("Y")]);

        let mut rows = Vec::new();
        let mut row_series = Vec::new();
        for (series_index, (name, points)) in series.iter().enumerate() {
            for (x, y) in points {
                let mut row = Vec::with_capacity(3);
                row.push(name.clone());
                row.push(format!("{x}"));
                row.push(format!("{y}"));
                rows.push(row);
                row_series.push(series_index);
            }
        }
        self.rows = rows;
        self.row_series = row_series;
        self.series_count = series.len();
        self.visible_series = (0..series.len()).collect();
        self.base.request_redraw();
    }

    /// Accepts a chart-side snapshot queue (COMP-A8).
    ///
    /// The chart keeps the write end and this table drains it through
    /// [`Self::sync_series_updates`], which the paint and event paths call, so a host only has to
    /// attach the pair once. A new queue replaces the previous pairing; the current rows stay
    /// until the next snapshot arrives.
    pub(crate) fn accept_series_updates(
        &mut self,
        source: Arc<Mutex<Option<ChartSeriesSnapshot>>>,
    ) {
        self.pending_series = Some(source);
        self.base.request_redraw();
    }

    /// Absorbs the snapshot a paired chart pushed since the last call.
    ///
    /// Returns whether a snapshot was applied. The data goes through [`Self::set_from_series`] and
    /// the visibility list through [`Self::set_visible_series`], so row building and redraw rules
    /// stay in one place.
    pub fn sync_series_updates(&mut self) -> bool {
        let Some(source) = self.pending_series.as_ref() else {
            return false;
        };
        let Some((series, visible)) = lock(source).take() else {
            return false;
        };
        self.set_from_series(&series);
        self.set_visible_series(visible);
        true
    }

    /// Replaces the header labels without touching the data rows.
    pub fn set_headers(&mut self, headers: Vec<String>) {
        if self.headers != headers {
            self.headers = headers;
            self.base.request_redraw();
        }
    }

    /// Returns the header labels.
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    /// Returns the materialised data rows, before series filtering.
    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    /// Returns how many series the last [`Self::set_from_series`] call supplied.
    pub fn series_count(&self) -> usize {
        self.series_count
    }

    /// Replaces the visible series set.
    ///
    /// Out-of-range indices are ignored and duplicates are collapsed; the set is kept sorted.
    /// This is a programmatic update and does not emit [`Self::series_toggled`] — that signal is
    /// the report of a user action, mirroring the way a checkable legend reports toggles.
    pub fn set_visible_series(&mut self, series: Vec<usize>) {
        let mut next: Vec<usize> =
            series.into_iter().filter(|&index| index < self.series_count).collect();
        next.sort_unstable();
        next.dedup();
        if self.visible_series != next {
            self.visible_series = next;
            self.base.request_redraw();
        }
    }

    /// Returns the visible series indices.
    pub fn visible_series(&self) -> &[usize] {
        &self.visible_series
    }

    /// Whether one series is currently included.
    pub fn series_visible(&self, series: usize) -> bool {
        self.visible_series.contains(&series)
    }

    /// Flips one series' visibility and reports it.
    ///
    /// Returns `false` for an index the current snapshot does not contain; the signal is only
    /// emitted for a series that actually exists.
    pub fn toggle_series(&mut self, series: usize) -> bool {
        if series >= self.series_count {
            return false;
        }
        if let Some(position) = self.visible_series.iter().position(|&index| index == series) {
            self.visible_series.remove(position);
        } else {
            self.visible_series.push(series);
            self.visible_series.sort_unstable();
        }
        self.base.request_redraw();
        self.series_toggled.emit(series);
        true
    }

    /// Sets how many rows the preview may render and export.
    pub fn set_max_rows(&mut self, max_rows: usize) {
        if self.max_rows != max_rows {
            self.max_rows = max_rows;
            self.base.request_redraw();
        }
    }

    /// Returns the preview row cap.
    pub fn max_rows(&self) -> usize {
        self.max_rows
    }

    /// Sets the mode that selects how much CSV [`Self::to_csv`] emits.
    pub fn set_mode(&mut self, mode: ChartDataMode) {
        if self.mode != mode {
            self.mode = mode;
            self.base.request_redraw();
        }
    }

    /// Returns the export mode.
    pub fn mode(&self) -> ChartDataMode {
        self.mode
    }

    /// Returns the number of rows that pass the series filter, before the row cap.
    pub fn row_count(&self) -> usize {
        self.rows
            .iter()
            .zip(&self.row_series)
            .filter(|(_, &series)| self.series_visible(series))
            .count()
    }

    /// Returns the number of rows the table actually draws: [`Self::row_count()`] capped by
    /// [`Self::max_rows()`]. Drawing always honours the cap, in both modes.
    pub fn displayed_row_count(&self) -> usize {
        self.row_count().min(self.max_rows)
    }

    /// Serialises the table as RFC 4180-style CSV.
    ///
    /// The header row comes first when headers exist. Every field containing a comma, a quote or
    /// a line break is quoted, and embedded quotes are doubled. In [`ChartDataMode::Preview`] the
    /// document stops at `max_rows`; in [`ChartDataMode::Export`] it carries every visible row.
    /// Every emitted line ends with `\n`, so a non-empty table always ends with a newline.
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        if !self.headers.is_empty() {
            push_csv_row(&mut out, &self.headers);
        }
        let limit = if self.mode == ChartDataMode::Preview { self.max_rows } else { usize::MAX };
        let mut written = 0usize;
        for (row, &series) in self.rows.iter().zip(&self.row_series) {
            if !self.series_visible(series) {
                continue;
            }
            if written >= limit {
                break;
            }
            push_csv_row(&mut out, row);
            written += 1;
        }
        out
    }

    /// Publishes the current CSV document through [`Self::export_requested`].
    ///
    /// A host calls this from its own export command (menu item, shortcut); the table does not
    /// guess a key binding, because an editor and a dashboard disagree about what "export" is.
    pub fn request_export(&mut self) {
        let csv = self.to_csv();
        self.export_requested.emit(csv);
    }

    /// The row index under `pos`, skipping rows hidden by the series filter or the row cap.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains(pos) {
            return None;
        }
        let inner = Rect::new(
            rect.x + TABLE_INSET as i32,
            rect.y + TABLE_INSET as i32,
            rect.width.saturating_sub(TABLE_INSET * 2),
            rect.height.saturating_sub(TABLE_INSET * 2),
        );
        let header_height = HEADER_HEIGHT.min(inner.height);
        let rows_top = inner.y + header_height as i32;
        let rows_bottom = inner.y + inner.height as i32;
        if pos.y < rows_top || pos.y >= rows_bottom {
            return None;
        }
        let offset = ((pos.y - rows_top) as u32 / ROW_HEIGHT.max(1)) as usize;
        if offset >= self.max_rows {
            return None;
        }
        let mut visible_offset = 0usize;
        for (index, &series) in self.row_series.iter().enumerate() {
            if !self.series_visible(series) {
                continue;
            }
            if visible_offset == offset {
                return Some(index);
            }
            visible_offset += 1;
        }
        None
    }
}

impl EventHandler for ChartDataTable {
    fn handle_event(&mut self, event: &Event) {
        // A chart-side write is drained before routing, so the click already sees the new rows.
        self.sync_series_updates();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            let Some(row_index) = self.row_at(*pos) else {
                return;
            };
            if let Some(&series) = self.row_series.get(row_index) {
                self.toggle_series(series);
            }
        }
    }
}

impl Draw for ChartDataTable {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a snapshot the paired chart pushed, so this frame paints it.
        self.sync_series_updates();
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

        let inner = Rect::new(
            rect.x + TABLE_INSET as i32,
            rect.y + TABLE_INSET as i32,
            rect.width.saturating_sub(TABLE_INSET * 2),
            rect.height.saturating_sub(TABLE_INSET * 2),
        );
        if self.headers.is_empty() {
            context.draw_text_fitted_cow(
                inner,
                "No series data",
                font,
                surface.blend(&ink, 0.55),
                HorizontalAlignment::Left,
            );
            return;
        }

        let header_height = HEADER_HEIGHT.min(inner.height);
        let header_rect = Rect::new(inner.x, inner.y, inner.width, header_height);
        let grid = surface.blend(&ink, 0.14);
        context.fill_rect(header_rect, surface.blend(&accent, 0.12));
        draw_table_row(
            context,
            header_rect,
            &self.headers,
            self.headers.len(),
            font,
            surface.blend(&ink, 0.80),
            grid,
        );

        let rows_top = header_rect.y + header_height as i32;
        let rows_bottom = inner.y + inner.height as i32;
        let mut drawn = 0usize;
        for (row_index, row) in self.rows.iter().enumerate() {
            if drawn >= self.max_rows {
                break;
            }
            let Some(&series) = self.row_series.get(row_index) else {
                continue;
            };
            if !self.series_visible(series) {
                continue;
            }
            let y = rows_top + (drawn as u32 * ROW_HEIGHT) as i32;
            let remaining = rows_bottom - y;
            if remaining <= 0 {
                break;
            }
            let height = ROW_HEIGHT.min(remaining as u32);
            let row_rect = Rect::new(inner.x, y, inner.width, height);
            if drawn % 2 == 1 {
                context.fill_rect(row_rect, surface.blend(&ink, 0.03));
            }
            draw_table_row(context, row_rect, row, self.headers.len(), font, ink, grid);
            drawn += 1;
        }
    }
}

impl Widget for ChartDataTable {
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
        let rows = self.max_rows.min(8) as u32;
        Size::new(320, HEADER_HEIGHT + rows * ROW_HEIGHT + TABLE_INSET * 2)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "export_requested" => {
                Some(EventSignalRef::mapped("export_requested", &self.export_requested, |csv| {
                    CapabilityValue::String(csv.clone())
                }))
            }
            "series_toggled" => {
                Some(EventSignalRef::mapped("series_toggled", &self.series_toggled, |series| {
                    CapabilityValue::UInt(*series as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ChartDataTable {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "headers" => Ok(CapabilityValue::Tuple(
                self.headers.iter().map(|header| CapabilityValue::String(header.clone())).collect(),
            )),
            "rows" => Ok(CapabilityValue::Tuple(
                self.rows
                    .iter()
                    .map(|row| {
                        CapabilityValue::Tuple(
                            row.iter().map(|cell| CapabilityValue::String(cell.clone())).collect(),
                        )
                    })
                    .collect(),
            )),
            "visible_series" => Ok(CapabilityValue::Tuple(
                self.visible_series
                    .iter()
                    .map(|&index| CapabilityValue::UInt(index as u64))
                    .collect(),
            )),
            "mode" => Ok(CapabilityValue::String(self.mode.as_str().to_string())),
            "max_rows" => Ok(CapabilityValue::UInt(self.max_rows as u64)),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "headers" => {
                self.set_headers(expect_string_list(value)?);
                Ok(())
            }
            "visible_series" => {
                self.set_visible_series(expect_index_list(value)?);
                Ok(())
            }
            "mode" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                let mode = ChartDataMode::from_name(&token.to_ascii_lowercase())
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_mode(mode);
                Ok(())
            }
            "max_rows" => {
                self.set_max_rows(expect_usize(value)?);
                Ok(())
            }
            // Rows are derived from the series snapshot; rewriting them alone would leave the
            // series index of every row dangling, so the honest answer is "read only".
            "rows" | "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "headers",
            "rows",
            "visible_series",
            "mode",
            "max_rows",
            "row_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// Draws one row of cells, splitting the width evenly across `columns`.
fn draw_table_row(
    context: &mut RenderContext,
    rect: Rect,
    cells: &[String],
    columns: usize,
    font: &crate::core::Font,
    color: Color,
    grid: Color,
) {
    if columns == 0 {
        return;
    }
    let width = (rect.width / columns as u32).max(1);
    for index in 0..columns {
        let x = rect.x + (index as u32 * width) as i32;
        let cell_width = if index + 1 == columns {
            rect.width.saturating_sub(index as u32 * width)
        } else {
            width
        };
        if let Some(text) = cells.get(index) {
            let text_rect = Rect::new(x + 6, rect.y, cell_width.saturating_sub(10), rect.height);
            context.draw_text_fitted_cow(
                text_rect,
                text.as_str(),
                font,
                color,
                HorizontalAlignment::Left,
            );
        }
        if index > 0 {
            context.draw_line(
                Point::new(x, rect.y),
                Point::new(x, rect.y + rect.height as i32),
                grid,
            );
        }
    }
}

/// Appends one CSV record, including its terminating newline.
fn push_csv_row(out: &mut String, cells: &[String]) {
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        push_csv_field(out, cell);
    }
    out.push('\n');
}

/// Appends one escaped CSV field.
fn push_csv_field(out: &mut String, field: &str) {
    let needs_quotes = field.chars().any(|character| matches!(character, ',' | '"' | '\n' | '\r'));
    if !needs_quotes {
        out.push_str(field);
        return;
    }
    out.push('"');
    for character in field.chars() {
        if character == '"' {
            out.push('"');
        }
        out.push(character);
    }
    out.push('"');
}

/// Decodes the visible-series property as a list of indices.
fn expect_index_list(value: CapabilityValue) -> Result<Vec<usize>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    items.into_iter().map(expect_usize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    fn series(name: &str, points: &[(f64, f64)]) -> (String, Vec<(f64, f64)>) {
        (String::from(name), Vec::from(points))
    }

    #[test]
    fn csv_escapes_commas_quotes_and_newlines() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[series("a,b\"c\nd", &[(1.0, 2.0)])]);
        table.set_mode(ChartDataMode::Export);
        assert_eq!(table.to_csv(), "Series,X,Y\n\"a,b\"\"c\nd\",1,2\n");
    }

    #[test]
    fn visible_series_filters_rows() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[
            series("Alpha", &[(0.0, 1.0), (1.0, 2.0)]),
            series("Beta", &[(2.0, 3.0)]),
        ]);
        assert_eq!(table.row_count(), 3);

        table.set_visible_series(Vec::from([1]));
        assert_eq!(table.row_count(), 1);
        let csv = table.to_csv();
        assert!(csv.contains("Beta"));
        assert!(!csv.contains("Alpha"), "the hidden series' rows must be gone");

        table.set_visible_series(Vec::from([0]));
        assert_eq!(table.row_count(), 2);
        assert!(table.to_csv().contains("Alpha"));
        assert!(!table.to_csv().contains("Beta"));
    }

    #[test]
    fn max_rows_truncates_the_preview_but_not_the_export() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[series("Only", &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)])]);
        table.set_max_rows(2);
        assert_eq!(table.row_count(), 3, "row_count is the model count");
        assert_eq!(table.displayed_row_count(), 2, "drawing honours the cap");

        table.set_mode(ChartDataMode::Preview);
        assert_eq!(table.to_csv().lines().count(), 3, "header plus two preview rows");

        table.set_mode(ChartDataMode::Export);
        assert_eq!(table.to_csv().lines().count(), 4, "header plus every visible row");
    }

    #[test]
    fn toggling_a_series_reports_it_and_changes_the_rows() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[series("Alpha", &[(0.0, 1.0)])]);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = std::sync::Arc::clone(&seen);
        table.series_toggled.connect(move |index| sink.lock().unwrap().push(*index));

        assert!(table.toggle_series(0));
        assert_eq!(*seen.lock().unwrap(), vec![0]);
        assert_eq!(table.row_count(), 0);
        assert!(!table.toggle_series(7), "a missing series is refused");
    }

    #[test]
    fn export_request_publishes_the_csv_document() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[series("Only", &[(3.0, 4.0)])]);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = std::sync::Arc::clone(&seen);
        table.export_requested.connect(move |csv| *sink.lock().unwrap() = (*csv).clone());

        table.request_export();
        assert_eq!(*seen.lock().unwrap(), "Series,X,Y\nOnly,3,4\n");
    }

    #[test]
    fn a_series_name_that_needs_no_escaping_is_written_verbatim() {
        // The escaping path is conditional; a plain field must not gain stray quotes.
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[(String::from("Plain"), Vec::from([(1.5, -2.25), (0.0, 8.0)]))]);
        assert_eq!(table.to_csv(), "Series,X,Y\nPlain,1.5,-2.25\nPlain,0,8\n");
    }

    #[test]
    fn an_empty_table_exports_an_empty_string() {
        let table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        assert_eq!(table.to_csv(), "");
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.mode(), ChartDataMode::Preview);
    }

    #[test]
    fn the_header_row_uses_the_a6_columns() {
        let mut table = ChartDataTable::new(Rect::new(0, 0, 320, 160));
        table.set_from_series(&[series("Only", &[])]);
        assert_eq!(
            table.headers(),
            &[String::from("Series"), String::from("X"), String::from("Y")]
        );
        assert_eq!(table.headers().len(), 3);
        assert_eq!(table.headers()[0], "Series");
        assert_eq!(table.headers()[1], "X");
        assert_eq!(table.headers()[2], "Y");
        // No points means no data rows, but the header shape is still published.
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.to_csv(), "Series,X,Y\n");
    }
}
