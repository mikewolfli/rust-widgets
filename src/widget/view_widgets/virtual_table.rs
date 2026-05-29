//! VirtualTable widget backed by incremental table data source.

use std::sync::Arc;

use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
    window_cache: Option<Vec<Vec<Option<String>>>>,
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
        self.data_source
            .as_ref()
            .map(|source| source.row_count())
            .unwrap_or(0)
    }

    /// Returns column count.
    pub fn column_count(&self) -> usize {
        self.data_source
            .as_ref()
            .map(|source| source.column_count())
            .unwrap_or(0)
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
        let row_len = visible_rows.saturating_add(self.overscan_rows * 2).max(1);
        let col_len = visible_cols
            .saturating_add(self.overscan_columns * 2)
            .max(1);
        (row_start, row_len, col_start, col_len)
    }

    /// Pulls currently visible window.
    pub fn fetch_visible_window(&mut self) -> Vec<Vec<Option<String>>> {
        if let Some(cache) = &self.window_cache {
            return cache.clone();
        }
        let Some(source) = &self.data_source else {
            return Vec::new();
        };
        let (row_start, row_len, col_start, col_len) = self.visible_window();
        let data = source.fetch_window(row_start, row_len, col_start, col_len);
        self.window_cache = Some(data.clone());
        data
    }

    fn emit_visible_window(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }
}

impl Widget for VirtualTable {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
                _ => {}
            },
            _ => {}
        }
    }
}

impl Draw for VirtualTable {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(251, 252, 254));
        context.draw_rect(rect, Color::from_rgb(190, 198, 210));

        let data = self.fetch_visible_window();
        if data.is_empty() {
            return;
        }

        let mut y = rect.y + 4;
        for row in data.iter().take(10) {
            let mut x = rect.x + 4;
            for cell in row.iter().take(6) {
                let cell_rect = Rect::new(
                    x,
                    y,
                    self.column_width.saturating_sub(2),
                    self.row_height.saturating_sub(2),
                );
                context.draw_rect(cell_rect, Color::from_rgb(220, 225, 233));
                if let Some(value) = cell {
                    context.draw_text(
                        Point::new(x + 4, y + self.row_height as i32 / 2),
                        value,
                        &Font::default(),
                        Color::from_rgb(49, 60, 78),
                    );
                }
                x += self.column_width as i32;
            }
            y += self.row_height as i32;
            if y >= rect.y + rect.height as i32 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

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

        let got = emitted
            .lock()
            .ok()
            .map(|guard| guard.clone())
            .unwrap_or_default();
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
}
