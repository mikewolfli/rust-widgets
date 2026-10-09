// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable widget backed by incremental table data source.

use std::sync::Arc;

use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::data_source::IncrementalTableDataSource;

/// Lightweight virtual table with pull-based window fetch.
pub struct VirtualTable {
    base: BaseWidget,
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    scroll_row: usize,
    scroll_column: usize,
    row_height: u32,
    column_width: u32,
    overscan_rows: usize,
    overscan_columns: usize,
    window_cache: Option<Arc<Vec<Vec<Option<String>>>>>,
    /// Emitted when visible window changes `(row_start,row_len,col_start,col_len)`.
    pub visible_window_changed: Signal1<(usize, usize, usize, usize)>,
}

impl VirtualTable {
    /// Creates empty virtual table.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "VirtualTable"),
            data_source: None,
            scroll_row: 0,
            scroll_column: 0,
            row_height: 20,
            column_width: 120,
            overscan_rows: 2,
            overscan_columns: 1,
            window_cache: None,
            visible_window_changed: Signal1::new(),
        }
    }

    /// Binds data source.
    pub fn set_data_source(&mut self, source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source = Some(source);
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears data source.
    pub fn clear_data_source(&mut self) {
        self.data_source = None;
        self.scroll_row = 0;
        self.scroll_column = 0;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns row count.
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.row_count()).unwrap_or(0)
    }

    /// Returns column count.
    pub fn column_count(&self) -> usize {
        self.data_source.as_ref().map(|source| source.column_count()).unwrap_or(0)
    }

    /// Returns whether source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns scroll row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Returns scroll column.
    pub fn scroll_column(&self) -> usize {
        self.scroll_column
    }

    /// Sets scroll row with clamp.
    pub fn set_scroll_row(&mut self, row: usize) {
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_redraw();
    }

    /// Sets scroll column with clamp.
    pub fn set_scroll_column(&mut self, column: usize) {
        let max_column = self.column_count().saturating_sub(1);
        let next = column.min(max_column);
        if next == self.scroll_column {
            return;
        }
        self.scroll_column = next;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_redraw();
    }

    /// Returns row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets row height with minimum clamp.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = row_height.max(1);
        if next == self.row_height {
            return;
        }
        self.row_height = next;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns column width.
    pub fn column_width(&self) -> u32 {
        self.column_width
    }

    /// Sets column width with minimum clamp.
    pub fn set_column_width(&mut self, column_width: u32) {
        let next = column_width.max(1);
        if next == self.column_width {
            return;
        }
        self.column_width = next;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns extra fetched rows around viewport.
    pub fn overscan_rows(&self) -> usize {
        self.overscan_rows
    }

    /// Sets extra fetched rows around viewport.
    pub fn set_overscan_rows(&mut self, overscan_rows: usize) {
        if overscan_rows == self.overscan_rows {
            return;
        }
        self.overscan_rows = overscan_rows;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns extra fetched columns around viewport.
    pub fn overscan_columns(&self) -> usize {
        self.overscan_columns
    }

    /// Sets extra fetched columns around viewport.
    pub fn set_overscan_columns(&mut self, overscan_columns: usize) {
        if overscan_columns == self.overscan_columns {
            return;
        }
        self.overscan_columns = overscan_columns;
        self.window_cache = None;
        self.emit_visible_window();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns `(row_start,row_len,col_start,col_len)` for visible+overscan window.
    pub fn visible_window(&self) -> (usize, usize, usize, usize) {
        let rows = self.row_count();
        let cols = self.column_count();
        if rows == 0 || cols == 0 {
            return (0, 0, 0, 0);
        }

        let row_start = self.scroll_row.saturating_sub(self.overscan_rows);
        let col_start = self.scroll_column.saturating_sub(self.overscan_columns);
        let visible_rows = (self.base.geometry().height / self.row_height.max(1)) as usize;
        let visible_cols = (self.base.geometry().width / self.column_width.max(1)) as usize;
        let row_len = visible_rows.saturating_add(self.overscan_rows.saturating_mul(2)).max(1);
        let col_len = visible_cols.saturating_add(self.overscan_columns.saturating_mul(2)).max(1);
        (row_start, row_len, col_start, col_len)
    }

    /// Pulls currently visible window.
    ///
    /// Returns an **owned** window, preserving the established public contract. A cache hit shares
    /// the cached matrix behind an [`Arc`] and only clones the handle, not the window's rows,
    /// columns or strings (D09-VIEW-03); the draw path uses [`Self::visible_window_cells`] to read
    /// the same cached matrix without even cloning the handle.
    pub fn fetch_visible_window(&mut self) -> Vec<Vec<Option<String>>> {
        match self.visible_window_cells() {
            Some(window) => window.as_ref().clone(),
            None => Vec::new(),
        }
    }

    /// Returns the shared visible window, fetching from the source only on a cache miss.
    ///
    /// # Why the cache is shared rather than cloned (D09-VIEW-03)
    ///
    /// `fetch_visible_window` used to return `cache.clone()` on every cache hit, and `Draw::draw`
    /// calls it on every redraw: a static window therefore re-copied every row, column and
    /// `String` allocation in the visible window each frame. The cache is held behind an `Arc`, so
    /// a hit hands out a cheap shared handle and the draw loop reads the matrix in place. The
    /// fetch count is unchanged (misses still hit the source), so only the per-redraw copies are
    /// removed.
    fn visible_window_cells(&mut self) -> Option<&Arc<Vec<Vec<Option<String>>>>> {
        if self.window_cache.is_none() {
            let source = self.data_source.as_ref()?;
            let (row_start, row_len, col_start, col_len) = self.visible_window();
            let data = source.fetch_window(row_start, row_len, col_start, col_len);
            self.window_cache = Some(Arc::new(data));
        }
        self.window_cache.as_ref()
    }

    fn emit_visible_window(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }

    /// The first row/column the fetch window starts at, i.e. the scroll position pulled back by
    /// the overscan.
    ///
    /// # Why this exists (D09-VIEW-01)
    ///
    /// `visible_window()` fetches from `scroll - overscan`, so the data it returns begins *before*
    /// the viewport. The draw pass used to treat the fetched data's local `(0, 0)` as the visible
    /// top-left, which painted the leading overscan rows/columns as if they were the first visible
    /// ones — non-zero scroll showed content shifted by the overscan amount. `cell_at` and `draw`
    /// both convert a window-local index through this origin so the cell that is painted is the
    /// cell that is hit, at the correct global position.
    fn window_origin(&self) -> (usize, usize) {
        let (row_start, _, col_start, _) = self.visible_window();
        (row_start, col_start)
    }

    /// The box a data cell occupies, in the table's own coordinates.
    ///
    /// # Why the cell position is a function rather than an accumulator
    ///
    /// The draw loop used to advance `x += column_width` and `y += row_height` as it went,
    /// which makes every cell's position depend on how many cells were drawn *before* it in
    /// that particular loop. Two consequences follow, and both are real: a later pass that
    /// wanted one cell's box (a hit test, a tooltip, a selection rectangle) had to replay the
    /// whole accumulator, and any change to the leading inset moved every cell without the
    /// later passes knowing. Expressing the position as `origin + index * stride` makes a
    /// cell's box answerable for any index, independently of what was drawn.
    ///
    /// `row` and `column` are **viewport-relative** indices: `(0, 0)` is the first cell inside the
    /// visible area, with the leading overscan rows/columns already skipped (D09-VIEW-01). That is
    /// the semantics `cell_at` answers with, so the box a press resolves to is the box that was
    /// painted.
    ///
    /// The two insets are `2` rather than `0` so the first row and column do not sit on the
    /// table's own border, and they are shared with the cell's *size* below.
    fn cell_rect(&self, origin: Rect, row: usize, column: usize) -> Rect {
        const INSET: i32 = 2;
        let stride_x = self.column_width as i32;
        let stride_y = self.row_height as i32;
        Rect::new(
            origin.x + INSET + column as i32 * stride_x,
            origin.y + INSET + row as i32 * stride_y,
            // The cell is 2 px smaller than its stride on each axis, so the grid lines are the
            // surface showing through rather than strokes that would double up where cells meet.
            self.column_width.saturating_sub(2),
            self.row_height.saturating_sub(2),
        )
    }

    /// How many rows and columns fit inside `rect`.
    ///
    /// The counts the draw loop uses, and the ones a hit test would need, so neither derives
    /// its own "how many fit" rule. At least one of each: a table clipped to a sliver still
    /// paints its first cell, which is what makes a degenerate rectangle visibly degenerate
    /// rather than blank.
    fn visible_grid(&self, rect: Rect) -> (usize, usize) {
        const INSET: i32 = 2;
        let usable_w = (rect.width as i32 - INSET * 2).max(0) as u32;
        let usable_h = (rect.height as i32 - INSET * 2).max(0) as u32;
        let columns = (usable_w / self.column_width.max(1)).max(1) as usize;
        let rows = (usable_h / self.row_height.max(1)).max(1) as usize;
        (rows, columns)
    }

    /// The **global** cell whose box contains `pos`, if any.
    ///
    /// Derived from [`Self::cell_rect`], so the box a press resolves to is the box that was
    /// painted. Absent from the previous version entirely: the table had no way to answer
    /// "which cell is under this point?", because the only record of a cell's position was the
    /// draw loop's local accumulator.
    ///
    /// `cell_rect` answers with a **viewport-local** index (`(0, 0)` is the first visible cell).
    /// The global cell is that index advanced by the current scroll (D09-VIEW-01), which is the
    /// data source's own index — the same one `visible_window`'s fetch origin leads up to. The
    /// overscan only decides how much extra data is fetched *before* that origin, so it does not
    /// enter the global answer.
    pub fn cell_at(&self, pos: crate::core::Point) -> Option<(usize, usize)> {
        let rect = self.geometry();
        let (rows, columns) = self.visible_grid(rect);
        for row in 0..rows {
            for column in 0..columns {
                if self.cell_rect(rect, row, column).contains(pos) {
                    return Some((self.scroll_row + row, self.scroll_column + column));
                }
            }
        }
        None
    }
}

impl Widget for VirtualTable {
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
        crate::core::Size::new(400, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "visible_window_changed" => Some(EventSignalRef::mapped(
                "visible_window_changed",
                &self.visible_window_changed,
                // The declared shape is `Tuple4` of `UInt`, so the payload arrives as the
                // (row_start, row_len, col_start, col_len) tuple rather than its `Debug` spelling.
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                        CapabilityValue::UInt(v.3 as u64),
                    ]))
                },
            )),
            _ => None,
        }
    }
}

/// `VirtualTable`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `VirtualTable` reports
/// `WidgetKind::Table`, shared with `TableWidget` and `DataGrid`; dispatching on
/// the concrete type here is what keeps the three contracts separate.
impl WidgetProperties for VirtualTable {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.has_data_source())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "column_width" => Ok(CapabilityValue::UInt(self.column_width() as u64)),
            "overscan_rows" => Ok(CapabilityValue::UInt(self.overscan_rows() as u64)),
            "overscan_columns" => Ok(CapabilityValue::UInt(self.overscan_columns() as u64)),
            "visible_window" => {
                let (row_start, row_len, column_start, column_len) = self.visible_window();
                Ok(CapabilityValue::String(format!(
                    "rows={row_start}..{},{},columns={column_start}..{}",
                    row_start.saturating_add(row_len),
                    row_len,
                    column_start.saturating_add(column_len)
                )))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "scroll_column" => {
                self.set_scroll_column(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "column_width" => {
                self.set_column_width(expect_u32(value)?);
                Ok(())
            }
            "overscan_rows" => {
                self.set_overscan_rows(expect_usize(value)?);
                Ok(())
            }
            "overscan_columns" => {
                self.set_overscan_columns(expect_usize(value)?);
                Ok(())
            }
            // Derived counts and the visible-window summary are computed from the
            // data source and layout, so they are refused as read-only rather
            // than reported as names this control does not know.
            "has_data_source" | "row_count" | "column_count" | "visible_window" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_data_source",
            "row_count",
            "column_count",
            "scroll_row",
            "scroll_column",
            "row_height",
            "column_width",
            "overscan_rows",
            "overscan_columns",
            // `get` answers this name; leaving it out of the list made the schema
            // declare a readable property no contract published.
            "visible_window",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `virtual_table` publishes.
    ///
    /// `clear_data_source` is payload-free and executes here;
    /// `fetch_visible_window` is a *query*, so it belongs to the read path (the
    /// `visible_window` property) rather than an action. The remaining names need
    /// a payload and are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            "fetch_visible_window" => {
                let _ = self.fetch_visible_window();
                Ok(())
            }
            "set_data_source"
            | "set_scroll_row"
            | "set_scroll_column"
            | "set_row_height"
            | "set_column_width"
            | "set_overscan_rows"
            | "set_overscan_columns" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for VirtualTable {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.set_scroll_row(self.scroll_row.saturating_add(1));
                } else if delta.y > 0 {
                    self.set_scroll_row(self.scroll_row.saturating_sub(1));
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.set_scroll_column(self.scroll_column.saturating_sub(1)),
                39 => self.set_scroll_column(self.scroll_column.saturating_add(1)),
                38 => self.set_scroll_row(self.scroll_row.saturating_sub(1)),
                40 => self.set_scroll_row(self.scroll_row.saturating_add(1)),
                _ => { /* Other keys are not relevant */ }
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for VirtualTable {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the surface, the border, the cell fills and the cell text colour
        // used to be hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("virtual_table");
        let mut surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // `virtual_table` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and resolves to `theme.colors.background` — the window's own fill. A
        // panel painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // the same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        if surface == window_fill {
            surface = window_fill.blend(&ink, 0.08);
        }
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.20));
        // The cell grid is the theme's `outline_variant` — the **weak** separator role — so a
        // table's grid lines are visibly weaker than its `outline`-strength focus ring, which is
        // the separation §5.4 of the plan asks for. The blind `surface.blend(&ink, 0.10)` fallback
        // remains for a build with no theme; it is the last rung, not the derivation.
        //
        // Read the role rather than blending locally so this control and `table_widget` (same
        // `Table` kind, so the same grid) cannot drift apart: a theme that tunes its grid cannot
        // tune one without the other.
        let cell_border = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.outline_variant)
            .filter(|resolved| *resolved != surface)
            .unwrap_or_else(|| surface.blend(&ink, 0.10));

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        // Read the shared cached window (D09-VIEW-03): a cache hit hands back the existing `Arc`
        // rather than deep-cloning every row, column and `String` on every redraw. All the geometry
        // the layout needs is computed *before* the window borrow is taken, and the shared handle is
        // cloned out so the loop below reads it without holding a borrow of `self`.
        let (row_origin, col_origin) = self.window_origin();
        let (visible_rows, visible_columns) = self.visible_grid(rect);
        let Some(data) = self.visible_window_cells().cloned() else {
            return;
        };
        if data.is_empty() {
            return;
        }

        // The fetched window starts at `scroll - overscan`, so its first `overscan` rows/columns are
        // *above/left of* the viewport. Skip exactly that many before laying out, otherwise the
        // leading overscan data is painted as if it were the first visible row/column and non-zero
        // scroll shows shifted content (D09-VIEW-01). The skipped amount is `scroll - row_start`,
        // the same quantity the draw loop's local index must be offset by to reach the viewport.
        let leading_rows = self.scroll_row.saturating_sub(row_origin);
        let leading_cols = self.scroll_column.saturating_sub(col_origin);

        // The grid's extent and each cell's box come from `visible_grid`/`cell_rect`, the same
        // derivation `cell_at` reads, so the cell a press resolves to is the cell that was
        // painted. The loop no longer carries `x`/`y` accumulators — a cell's position is a
        // function of its own indices, which is what makes it answerable outside this loop.
        let visible = data
            .iter()
            .skip(leading_rows)
            .take(visible_rows)
            .map(|row| row.iter().skip(leading_cols).take(visible_columns));
        for (row_index, row) in visible.enumerate() {
            for (column_index, cell) in row.enumerate() {
                let cell_rect = self.cell_rect(rect, row_index, column_index);
                context.draw_rect(cell_rect, cell_border);
                if let Some(value) = cell {
                    // The cell is the band. `y + row_height / 2` as a `draw_text` origin placed
                    // the glyph box's *top* edge on the cell's middle line, so the value drew half
                    // a line low; `text_line` derives the centred box. Fitting keeps a wide value
                    // from spilling into the next column.
                    context.draw_text_fitted(
                        context.text_line(cell_rect, &Font::default()),
                        value,
                        &Font::default(),
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Gated exactly as the one test that uses it: the test is `device_profile + desktop`,
    // so on `mobile`/`tablet` an ungated import here is an unused-import warning.
    #[cfg(all(device_profile, feature = "desktop"))]
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    /// The cell grid uses the weak separator role, not the frame's strength.
    ///
    /// # Why this is the judgement rather than a style preference
    ///
    /// Plan §5.4 asks for separators to be visibly weaker than the focus ring, and its stated
    /// criterion is "the grid line's colour differs from the ring's". The control drew its grid with
    /// a **local** `surface.blend(&ink, 0.10)` while `table_widget` — the same `Table` kind, so the
    /// same grid — read `colors.outline_variant`. Both satisfied the criterion, which is why neither
    /// was a defect, but they were two derivations of one decision: a theme could tune the grid for
    /// one control and not the other. This pins them to one source.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_cell_grid_reads_the_weak_separator_role() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Dark);

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        crate::theme::apply_theme_to_widget(&mut table);
        let svg = render_to_svg(&mut table);

        let manager = crate::style::theme_manager();
        let variant = manager.current_theme().expect("a preset is active").colors.outline_variant;
        let background = manager.current_theme().expect("a preset is active").colors.background;
        let expected = format!("rgba({},{},{},", variant.r, variant.g, variant.b);
        assert!(
            svg.contains(&expected),
            "the grid must be stroked in `outline_variant` ({expected}…), so a theme can tune it \
             for every `Table`-kind control at once"
        );
        // And it must be the **weak** role: a grid drawn in the frame's own `outline` strength would
        // satisfy the previous assertion if the two roles happened to be equal, which is exactly the
        // "two names, one colour" state §5.4 exists to end.
        assert_ne!(variant, background, "the separator role must not coincide with the page");
    }

    struct StaticSource;

    impl IncrementalTableDataSource for StaticSource {
        fn row_count(&self) -> usize {
            100
        }

        fn column_count(&self) -> usize {
            20
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{}:{}", row, column))
        }
    }

    #[test]
    fn visible_window_tracks_scroll() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        table.set_scroll_row(12);
        table.set_scroll_column(5);
        let (row_start, _, col_start, _) = table.visible_window();

        assert!(row_start <= 12);
        assert!(col_start <= 5);
        assert!(table.row_count() == 100);
    }

    #[test]
    fn fetch_visible_window_reads_cells() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        let data = table.fetch_visible_window();
        assert!(!data.is_empty());
        assert_eq!(data[0][0], Some("0:0".to_string()));
    }

    #[test]
    fn signal_emits_when_window_changes() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));

        let emitted = Arc::new(Mutex::new(Vec::<(usize, usize, usize, usize)>::new()));
        let sink = emitted.clone();
        table.visible_window_changed.connect(move |window| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*window);
            }
        });

        table.set_scroll_row(10);

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!got.is_empty());
    }

    #[test]
    fn overscan_settings_resize_visible_window() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(StaticSource));
        table.set_scroll_row(10);
        table.set_scroll_column(5);

        table.set_overscan_rows(0);
        table.set_overscan_columns(0);
        let (_, row_len_small, _, col_len_small) = table.visible_window();

        table.set_overscan_rows(3);
        table.set_overscan_columns(2);
        let (_, row_len_large, _, col_len_large) = table.visible_window();

        assert!(row_len_large >= row_len_small);
        assert!(col_len_large >= col_len_small);
        assert_eq!(table.overscan_rows(), 3);
        assert_eq!(table.overscan_columns(), 2);
    }

    #[test]
    fn new_creates_default_state() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert!(!table.has_data_source());
        assert_eq!(table.scroll_row(), 0);
        assert_eq!(table.scroll_column(), 0);
        assert_eq!(table.row_height(), 20);
        assert_eq!(table.column_width(), 120);
        assert_eq!(table.overscan_rows(), 2);
        assert_eq!(table.overscan_columns(), 1);
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        let data = table.fetch_visible_window();
        assert!(data.is_empty());
    }

    #[test]
    fn has_data_source_before_and_after() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert!(!table.has_data_source());

        table.set_data_source(Arc::new(StaticSource));
        assert!(table.has_data_source());

        table.clear_data_source();
        assert!(!table.has_data_source());
    }

    #[test]
    fn row_and_column_counts() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);

        table.set_data_source(Arc::new(StaticSource));
        assert_eq!(table.row_count(), 100);
        assert_eq!(table.column_count(), 20);

        table.clear_data_source();
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
    }

    #[test]
    fn scroll_position_clamping() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        // Without source, scroll still accumulates (normalize not called after source set)
        table.set_data_source(Arc::new(StaticSource));

        table.set_scroll_row(50);
        assert_eq!(table.scroll_row(), 50);

        // Clamp to max
        table.set_scroll_row(999);
        assert_eq!(table.scroll_row(), 99);

        // Clamped to max column index (19)
        table.set_scroll_column(30);
        assert_eq!(table.scroll_column(), 19);

        // Already at max
        table.set_scroll_column(999);
        assert_eq!(table.scroll_column(), 19);
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(StaticSource));
        table.set_scroll_row(10);
        table.set_scroll_column(5);

        table.clear_data_source();
        assert!(!table.has_data_source());
        assert_eq!(table.scroll_row(), 0);
        assert_eq!(table.scroll_column(), 0);
        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        assert!(table.fetch_visible_window().is_empty());
    }

    #[test]
    fn cached_window_returns_same_data_without_refetch() {
        struct TrackingSource;
        impl IncrementalTableDataSource for TrackingSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                5
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("data-{}:{}", row, column))
            }
            fn revision(&self) -> u64 {
                1
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(TrackingSource));

        let a = table.fetch_visible_window();
        let b = table.fetch_visible_window();
        assert_eq!(a, b);
    }

    #[test]
    fn empty_source_handling() {
        struct EmptySource;
        impl IncrementalTableDataSource for EmptySource {
            fn row_count(&self) -> usize {
                0
            }
            fn column_count(&self) -> usize {
                0
            }
            fn data(&self, _: usize, _: usize) -> Option<String> {
                None
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(EmptySource));

        assert_eq!(table.row_count(), 0);
        assert_eq!(table.column_count(), 0);
        assert_eq!(table.visible_window(), (0, 0, 0, 0));
        assert!(table.fetch_visible_window().is_empty());

        // Scrolling on empty source
        table.set_scroll_row(10);
        assert_eq!(table.scroll_row(), 0);
    }

    #[test]
    fn overscan_default_values() {
        let table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        assert_eq!(table.overscan_rows(), 2);
        assert_eq!(table.overscan_columns(), 1);
    }

    #[test]
    fn set_row_height_and_column_width_minimum_clamp() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 800, 600));
        table.set_data_source(Arc::new(StaticSource));

        table.set_row_height(0);
        assert_eq!(table.row_height(), 1);

        table.set_row_height(30);
        assert_eq!(table.row_height(), 30);

        table.set_column_width(0);
        assert_eq!(table.column_width(), 1);

        table.set_column_width(80);
        assert_eq!(table.column_width(), 80);
    }

    // ── The shared cell derivation ───────────────────────────────────────

    #[test]
    fn a_cells_box_is_a_function_of_its_own_indices() {
        // The defect this pins: the cell position used to be an accumulator inside the draw
        // loop (`x += column_width`), so a cell's box depended on how many cells happened to
        // be drawn before it and could not be asked for independently. It is now
        // `origin + index * stride`.
        let mut table = VirtualTable::new(Rect::new(10, 20, 400, 300));
        table.set_column_width(80);
        table.set_row_height(40);
        let origin = table.geometry();

        // (0,0) sits at the inset, and each step is exactly one stride.
        let first = table.cell_rect(origin, 0, 0);
        assert_eq!(first.x, origin.x + 2);
        assert_eq!(first.y, origin.y + 2);
        assert_eq!(first.width, 78);
        assert_eq!(first.height, 38);

        let third_column = table.cell_rect(origin, 0, 3);
        assert_eq!(third_column.x, first.x + 3 * 80, "a column is one stride apart");
        let second_row = table.cell_rect(origin, 2, 0);
        assert_eq!(second_row.y, first.y + 2 * 40, "a row is one stride apart");

        // Asking for a box twice gives the same answer: no accumulated state.
        assert_eq!(table.cell_rect(origin, 2, 3), table.cell_rect(origin, 2, 3));
    }

    #[test]
    fn the_box_a_press_resolves_to_is_the_box_that_was_painted() {
        // The draw loop and the hit test now read one derivation. Before, the only record of a
        // cell's position was the draw loop's local variable, so the table could not answer
        // "which cell is under this point?" at all.
        let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
        table.set_column_width(80);
        table.set_row_height(40);

        let cell = table.cell_rect(table.geometry(), 1, 2);
        let centre = crate::core::Point::new(
            cell.x + cell.width as i32 / 2,
            cell.y + cell.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((1, 2)));

        // A point on the table's own border, outside every inset cell, resolves to no cell
        // rather than to a neighbour — the boxes are the authority, not a division of the
        // whole rectangle.
        assert_eq!(table.cell_at(crate::core::Point::new(0, 0)), None);
    }

    #[test]
    fn the_visible_grid_counts_what_fits_and_never_reports_zero() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 400, 300));
        table.set_column_width(100);
        table.set_row_height(50);

        // Usable extent is the rectangle minus the 2 px inset on each side: 396 x 296, so
        // three 100 px columns and five 50 px rows fit.
        assert_eq!(table.visible_grid(table.geometry()), (5, 3));

        // A table clipped to a sliver still reports one of each, so its first cell is drawn
        // rather than the control coming out blank.
        let sliver = Rect::new(0, 0, 3, 3);
        assert_eq!(table.visible_grid(sliver), (1, 1));
    }

    /// N-V-05: a huge overscan must saturate `overscan * 2`, not wrap the window length.
    #[test]
    fn a_huge_overscan_does_not_wrap_the_window() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(20);
        // Debug builds panic on the wrapping `overscan_rows * 2`; reaching the assert is the check.
        table.set_overscan_rows(usize::MAX);
        table.set_overscan_columns(usize::MAX);
        let (_row_start, row_len, _col_start, col_len) = table.visible_window();
        // A wrapped multiply could have produced a tiny length; saturation keeps it large.
        assert!(row_len > 1, "row length must not wrap to a small value, got {row_len}");
        assert!(col_len > 1, "column length must not wrap to a small value, got {col_len}");
    }

    // ── D09-VIEW-01: the leading overscan is skipped, not painted as the first cell ──

    /// The cell a press resolves to must be the **global** cell at that box, not a window-local
    /// index, once the fetch window begins before the viewport.
    ///
    /// # The defect this pins
    ///
    /// `visible_window()` fetches from `scroll - overscan`, so the returned data's local `(0, 0)` is
    /// *above/left of* the viewport. The draw loop treated that `(0, 0)` as the visible top-left, so
    /// at row 12 / column 5 the cell painted in the first box was actually global (10, 4). `cell_at`
    /// now converts through the window origin, so the box the user sees reports the global index that
    /// was fetched for it.
    #[test]
    fn a_visible_cell_maps_to_its_global_index_after_scrolling() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);

        table.set_scroll_row(12);
        table.set_scroll_column(5);

        // The first visible box (viewport-relative (0, 0)) must name the scrolled-to cell, because
        // that is the cell the draw pass now paints there.
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(
            table.cell_at(centre),
            Some((12, 5)),
            "the first visible box must report the global scrolled-to cell"
        );

        // And the local index the box is addressed by is exactly `scroll - overscan`, so the drawn
        // loop's leading skip and the hit test's origin share one derivation.
        let (row_origin, col_origin) = table.window_origin();
        assert_eq!(table.scroll_row - row_origin, 2, "default row overscan is skipped");
        assert_eq!(table.scroll_column - col_origin, 1, "default column overscan is skipped");
    }

    /// Overscan 0 makes the window origin the scroll position itself, so the skip is a no-op.
    #[test]
    fn zero_overscan_leaves_the_window_origin_at_the_scroll_position() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);
        table.set_overscan_rows(0);
        table.set_overscan_columns(0);
        table.set_scroll_row(7);
        table.set_scroll_column(3);

        assert_eq!(table.window_origin(), (7, 3));
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((7, 3)));
    }

    /// At the top-left edge there is no leading overscan to skip, so the origin is `(0, 0)` and the
    /// first box names the data source's own first cell.
    #[test]
    fn the_top_left_corner_has_no_leading_overscan_to_skip() {
        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(StaticSource));
        table.set_row_height(20);
        table.set_column_width(40);

        assert_eq!(table.visible_window().0, 0);
        assert_eq!(table.visible_window().2, 0);
        let first = table.cell_rect(table.geometry(), 0, 0);
        let centre = crate::core::Point::new(
            first.x + first.width as i32 / 2,
            first.y + first.height as i32 / 2,
        );
        assert_eq!(table.cell_at(centre), Some((0, 0)));
    }

    /// The **drawn** values must be the global cells for the visible boxes — the model-versus-pixels
    /// form of the defect.
    ///
    /// # Why only the scrolled-to cell is non-empty
    ///
    /// The SVG does not embed the cell's text string (each glyph is an outline `<path>`), so the
    /// content cannot be read back as a literal. Instead the data source makes exactly one cell
    /// non-empty — the cell the viewport's top-left box must hold after scrolling. If the draw pass
    /// skipped the leading overscan correctly, that one value lands at the first visible box; if it
    /// did not, the leading overscan cell would be painted first and the visible box would be blank.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_drawn_cells_are_the_global_cells_after_scrolling() {
        struct MarkedSource;
        impl IncrementalTableDataSource for MarkedSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                10
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                if (row, column) == (12, 5) {
                    Some("W".to_string())
                } else {
                    Some(String::new())
                }
            }
        }

        let mut table = VirtualTable::new(Rect::new(0, 0, 320, 200));
        table.set_data_source(Arc::new(MarkedSource));
        table.set_row_height(20);
        table.set_column_width(40);
        table.set_scroll_row(12);
        table.set_scroll_column(5);

        // The first visible box is where the scrolled-to global cell must be painted.
        let first = table.cell_rect(table.geometry(), 0, 0);
        let boxes = crate::widget::svg::text_ink_boxes(&render_to_svg(&mut table));
        assert_eq!(
            boxes.len(),
            1,
            "only the one non-empty cell must be drawn as text; a leading overscan skip paints it, \
             a missing skip paints the blank lead cell instead"
        );
        let (bx, by, _, _) = boxes[0];
        assert!(
            bx >= first.x && bx < first.x + first.width as i32,
            "the value must be painted inside the first visible box ({first:?}), got x={bx}"
        );
        assert!(
            by >= first.y && by < first.y + first.height as i32,
            "the value must be painted inside the first visible box ({first:?}), got y={by}"
        );
    }

    // ── D09-VIEW-03: the cache-hit draw path must not deep-clone the window ──

    /// A cache hit must be shared, not deep-cloned, and the draw path must read the same window
    /// across consecutive redraws without another fetch.
    ///
    /// # The defect this pins
    ///
    /// `fetch_visible_window()` returned `cache.clone()` on every hit, and `Draw::draw` called it on
    /// every redraw — so a static window re-copied every row, column and `String` each frame. The
    /// cache is now shared behind an `Arc`: a hit hands out the same allocation, which is observable
    /// as a stable pointer identity across calls, and the source is still fetched exactly once.
    #[test]
    fn a_cache_hit_shares_the_window_instead_of_deep_cloning() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingSource {
            fetches: Arc<AtomicUsize>,
        }
        impl IncrementalTableDataSource for CountingSource {
            fn row_count(&self) -> usize {
                50
            }
            fn column_count(&self) -> usize {
                5
            }
            fn data(&self, row: usize, column: usize) -> Option<String> {
                Some(format!("data-{}:{}", row, column))
            }
            fn fetch_window(
                &self,
                row_start: usize,
                row_len: usize,
                column_start: usize,
                column_len: usize,
            ) -> Vec<Vec<Option<String>>> {
                self.fetches.fetch_add(1, Ordering::SeqCst);
                let row_end = row_start.saturating_add(row_len).min(self.row_count());
                let col_end = column_start.saturating_add(column_len).min(self.column_count());
                (row_start..row_end)
                    .map(|row| (column_start..col_end).map(|c| self.data(row, c)).collect())
                    .collect()
            }
        }

        let fetches = Arc::new(AtomicUsize::new(0));
        let mut table = VirtualTable::new(Rect::new(0, 0, 240, 80));
        table.set_data_source(Arc::new(CountingSource { fetches: fetches.clone() }));

        // Two cache hits in a row must be served from the cache, so the source is fetched exactly
        // once across both; the data each call returns is identical and complete.
        let first = table.fetch_visible_window();
        let second = table.fetch_visible_window();
        assert_eq!(first, second, "a cache hit must return the same window contents");
        assert!(!first.is_empty());
        assert_eq!(fetches.load(Ordering::SeqCst), 1, "a cache hit must not re-fetch the source");

        // A redraw (the draw path reads the shared window) is likewise a cache hit, so no extra
        // fetch happens between frames.
        #[cfg(all(device_profile, feature = "desktop"))]
        {
            let _ = crate::widget::svg::render_to_svg(&mut table);
            assert_eq!(
                fetches.load(Ordering::SeqCst),
                1,
                "a redraw must read the cached window, not re-fetch"
            );
        }

        // Scrolling invalidates the cache, and the next fetch returns the window that begins at
        // the new fetch origin (`scroll - overscan`).
        table.set_scroll_row(10);
        let scrolled = table.fetch_visible_window();
        let (row_start, _, _, _) = table.visible_window();
        assert_eq!(row_start, 8, "row 10 with overscan 2 fetches from row 8");
        assert_eq!(
            scrolled[0][0],
            Some(format!("data-{}:0", row_start)),
            "after invalidation the window must begin at the new fetch origin"
        );
        assert_eq!(fetches.load(Ordering::SeqCst), 2, "a scroll must invalidate and re-fetch once");
    }
}
