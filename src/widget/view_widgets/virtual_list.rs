//! Virtualized list widget backed by incremental data source protocol.

use std::sync::Arc;

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::Event;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::data_source::IncrementalTableDataSource;

#[derive(Clone, Debug, PartialEq, Eq)]
struct WindowCache {
    start: usize,
    len: usize,
    revision: u64,
    rows: Vec<(usize, String)>,
}

/// Virtualized list view that renders only a visible row window.
pub struct VirtualList {
    base: BaseWidget,
    data_source: Option<Arc<dyn IncrementalTableDataSource>>,
    data_source_connection_scope: ConnectionScope,
    scroll_row: usize,
    row_height: u32,
    overscan: usize,
    selected_row: Option<usize>,
    window_cache: Option<WindowCache>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<Option<usize>>,
    /// Emitted when visible row window changes `(start, len)`.
    pub visible_window_changed: Signal1<(usize, usize)>,
}

impl VirtualList {
    /// Creates an empty virtual list.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DataView, geometry, "VirtualList"),
            data_source: None,
            data_source_connection_scope: ConnectionScope::new(),
            scroll_row: 0,
            row_height: 20,
            overscan: 2,
            selected_row: None,
            window_cache: None,
            selection_changed: Signal1::new(),
            visible_window_changed: Signal1::new(),
        }
    }

    /// Binds incremental data source.
    pub fn set_data_source(&mut self, data_source: Arc<dyn IncrementalTableDataSource>) {
        self.data_source_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = data_source.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            data_changed.connect_scoped(&self.data_source_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.data_source = Some(data_source);
        self.scroll_row = 0;
        self.selected_row = None;
        self.clear_window_cache();
        self.normalize_projection_state();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears bound data source.
    pub fn clear_data_source(&mut self) {
        self.data_source_connection_scope = ConnectionScope::new();
        self.data_source = None;
        self.scroll_row = 0;
        self.selected_row = None;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether a data source is bound.
    pub fn has_data_source(&self) -> bool {
        self.data_source.is_some()
    }

    /// Returns bound data source when present.
    pub fn data_source_ref(&self) -> Option<&Arc<dyn IncrementalTableDataSource>> {
        self.data_source.as_ref()
    }

    /// Returns total row count.
    pub fn row_count(&self) -> usize {
        self.data_source.as_ref().map(|s| s.row_count()).unwrap_or(0)
    }

    /// Returns current top visible row.
    pub fn scroll_row(&self) -> usize {
        self.scroll_row
    }

    /// Sets top visible row with clamping.
    pub fn set_scroll_row(&mut self, row: usize) {
        self.normalize_projection_state();
        let max_row = self.row_count().saturating_sub(1);
        let next = row.min(max_row);
        if next == self.scroll_row {
            return;
        }
        self.scroll_row = next;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Scrolls by signed row delta.
    pub fn scroll_by_rows(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let current = self.scroll_row as isize;
        let next = (current + delta).max(0) as usize;
        self.set_scroll_row(next);
    }

    /// Returns row height in pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets row height in pixels.
    pub fn set_row_height(&mut self, row_height: u32) {
        let next = row_height.max(1);
        if self.row_height == next {
            return;
        }
        self.row_height = next;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns overscan row count.
    pub fn overscan(&self) -> usize {
        self.overscan
    }

    /// Sets overscan row count.
    pub fn set_overscan(&mut self, overscan: usize) {
        if self.overscan == overscan {
            return;
        }
        self.overscan = overscan;
        self.clear_window_cache();
        self.emit_visible_window_changed();
        self.base.request_redraw();
    }

    /// Returns selected row when present.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row.filter(|row| *row < self.row_count())
    }

    /// Selects a row.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.selected_row == Some(row) {
            return true;
        }
        self.selected_row = Some(row);
        self.selection_changed.emit(self.selected_row);
        self.base.request_redraw();
        true
    }

    /// Returns `(start, len)` for visible + overscan row window.
    pub fn visible_window(&self) -> (usize, usize) {
        let row_count = self.row_count();
        if row_count == 0 {
            return (0, 0);
        }

        let start = self.scroll_row.saturating_sub(self.overscan);
        let visible_capacity = self.visible_capacity();
        let fetch_len = visible_capacity.saturating_add(self.overscan.saturating_mul(2));
        let end = start.saturating_add(fetch_len).min(row_count);
        (start, end.saturating_sub(start))
    }

    /// Fetches visible rows as `(row_index, text)` pairs.
    pub fn fetch_visible_rows(&mut self) -> Vec<(usize, String)> {
        self.normalize_projection_state();

        let Some(source) = self.data_source.as_ref() else {
            return Vec::new();
        };

        let (start, len) = self.visible_window();
        if len == 0 {
            return Vec::new();
        }

        let revision = source.revision();
        if revision > 0 {
            if let Some(cache) = self.window_cache.as_ref() {
                if cache.start == start && cache.len == len && cache.revision == revision {
                    return cache.rows.clone();
                }
            }
        }

        let rows = self.fetch_window_rows(start, len);
        if revision > 0 {
            self.window_cache = Some(WindowCache { start, len, revision, rows: rows.clone() });
        } else {
            self.window_cache = None;
        }

        rows
    }

    fn fetch_window_rows(&self, start: usize, len: usize) -> Vec<(usize, String)> {
        let Some(source) = self.data_source.as_ref() else {
            return Vec::new();
        };

        source
            .fetch_window(start, len, 0, 1)
            .into_iter()
            .enumerate()
            .map(|(offset, row)| {
                let text = row.into_iter().next().flatten().unwrap_or_default();
                (start + offset, text)
            })
            .collect()
    }

    fn clear_window_cache(&mut self) {
        self.window_cache = None;
    }

    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        if row_count == 0 {
            self.scroll_row = 0;
            self.selected_row = None;
            return;
        }

        let max_row = row_count.saturating_sub(1);
        if self.scroll_row > max_row {
            self.scroll_row = max_row;
        }
        self.selected_row = self.selected_row.filter(|row| *row <= max_row);
    }

    fn visible_capacity(&self) -> usize {
        let h = self.base.geometry().height;
        if h == 0 {
            return 0;
        }
        let rh = self.row_height.max(1);
        h.div_ceil(rh) as usize
    }

    fn emit_visible_window_changed(&self) {
        self.visible_window_changed.emit(self.visible_window());
    }
}

impl Widget for VirtualList {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 200)
    }
}

impl Draw for VirtualList {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        context.fill_rect(rect, Color::rgb(255, 255, 255));
        context.draw_rect(rect, Color::rgb(200, 200, 200));

        let rows = self.fetch_visible_rows();
        if rows.is_empty() {
            return;
        }

        let rh = self.row_height as i32;
        for (row_index, text) in rows {
            let y = rect.y + ((row_index as i32 - self.scroll_row as i32) * rh);
            if y + rh <= rect.y || y >= rect.y + rect.height as i32 {
                continue;
            }

            if self.selected_row == Some(row_index) {
                context.fill_rect(
                    Rect::new(rect.x, y, rect.width, self.row_height),
                    Color::rgb(210, 230, 255),
                );
            }

            context.draw_text(
                Point::new(rect.x + 4, y + rh / 2),
                &text,
                &Font::default(),
                Color::rgb(0, 0, 0),
                HorizontalAlignment::Left,
            );
        }
    }
}

impl crate::event::EventHandler for VirtualList {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        self.normalize_projection_state();

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.base.geometry();
                if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
                    return;
                }
                let local_row = ((pos.y - rect.y) / self.row_height as i32).max(0) as usize;
                let row = self.scroll_row.saturating_add(local_row);
                let _ = self.select_row(row);
            }
            Event::Wheel { delta, .. } => {
                let lines = ((delta.y.abs() / 120).max(1)) as isize;
                if delta.y < 0 {
                    self.scroll_by_rows(lines);
                } else if delta.y > 0 {
                    self.scroll_by_rows(-lines);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventHandler;
    use crate::signal::GenericSignal;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    struct StaticSource {
        rows: usize,
    }

    impl IncrementalTableDataSource for StaticSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            if column == 0 && row < self.rows {
                Some(format!("row-{}", row))
            } else {
                None
            }
        }
    }

    struct CountingSource {
        rows: usize,
        calls: AtomicUsize,
        revision: AtomicU64,
        changed: GenericSignal,
    }

    impl CountingSource {
        fn new(rows: usize, revision: u64) -> Self {
            Self {
                rows,
                calls: AtomicUsize::new(0),
                revision: AtomicU64::new(revision),
                changed: GenericSignal::new(),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }

        fn bump_revision(&self) {
            self.revision.fetch_add(1, Ordering::Relaxed);
            self.changed.emit();
        }
    }

    impl IncrementalTableDataSource for CountingSource {
        fn row_count(&self) -> usize {
            self.rows
        }

        fn column_count(&self) -> usize {
            1
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if column == 0 && row < self.rows {
                Some(format!("cached-{}", row))
            } else {
                None
            }
        }

        fn revision(&self) -> u64 {
            self.revision.load(Ordering::Relaxed)
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    #[test]
    fn visible_window_includes_overscan_and_clamps() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 45));
        list.set_row_height(20);
        list.set_overscan(2);
        list.set_data_source(Arc::new(StaticSource { rows: 10 }));

        assert_eq!(list.visible_window(), (0, 7));

        list.set_scroll_row(4);
        assert_eq!(list.visible_window(), (2, 7));

        list.set_scroll_row(999);
        assert_eq!(list.scroll_row(), 9);
        assert_eq!(list.visible_window(), (7, 3));
    }

    #[test]
    fn fetch_visible_rows_projects_window() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_data_source(Arc::new(StaticSource { rows: 5 }));
        list.set_overscan(0);

        let rows = list.fetch_visible_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (0, "row-0".to_string()));
        assert_eq!(rows[1], (1, "row-1".to_string()));

        list.set_scroll_row(3);
        let rows2 = list.fetch_visible_rows();
        assert_eq!(rows2.len(), 2);
        assert_eq!(rows2[0], (3, "row-3".to_string()));
        assert_eq!(rows2[1], (4, "row-4".to_string()));
    }

    #[test]
    fn visible_rows_cache_invalidation_uses_revision() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_overscan(0);

        let source = Arc::new(CountingSource::new(6, 1));
        list.set_data_source(source.clone());

        let a = list.fetch_visible_rows();
        let calls_after_a = source.calls();
        assert_eq!(a.len(), 2);
        assert!(calls_after_a > 0);

        let b = list.fetch_visible_rows();
        let calls_after_b = source.calls();
        assert_eq!(a, b);
        assert_eq!(calls_after_b, calls_after_a);

        source.bump_revision();
        let _ = list.fetch_visible_rows();
        assert!(source.calls() > calls_after_b);
    }

    #[test]
    fn wheel_scroll_and_selection_behave() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 60));
        list.set_data_source(Arc::new(StaticSource { rows: 20 }));

        list.handle_event(&Event::wheel(0, -120, 0));
        assert_eq!(list.scroll_row(), 1);

        list.handle_event(&Event::wheel(0, 120, 0));
        assert_eq!(list.scroll_row(), 0);

        list.handle_event(&Event::mouse_press(2, 42, 1));
        assert_eq!(list.selected_row(), Some(2));

        assert!(!list.select_row(999));
        assert_eq!(list.selected_row(), Some(2));
    }

    #[test]
    fn new_creates_default_state() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());
        assert_eq!(list.scroll_row(), 0);
        assert_eq!(list.selected_row(), None);
        assert_eq!(list.row_height(), 20);
        assert_eq!(list.overscan(), 2);
        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
    }

    #[test]
    fn has_data_source_and_data_source_ref_work() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());

        let source = Arc::new(StaticSource { rows: 5 });
        list.set_data_source(source.clone());
        assert!(list.has_data_source());
        assert!(list.data_source_ref().is_some());
        assert_eq!(list.data_source_ref().unwrap().row_count(), 5);

        list.clear_data_source();
        assert!(!list.has_data_source());
        assert!(list.data_source_ref().is_none());
    }

    #[test]
    fn clear_data_source_resets_state() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 10 }));
        list.set_scroll_row(5);
        list.select_row(3);

        list.clear_data_source();
        assert!(!list.has_data_source());
        assert_eq!(list.scroll_row(), 0);
        assert_eq!(list.selected_row(), None);
        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
    }

    #[test]
    fn row_count_reflects_source() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        assert_eq!(list.row_count(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 0 }));
        assert_eq!(list.row_count(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 7 }));
        assert_eq!(list.row_count(), 7);
    }

    #[test]
    fn scroll_row_clamps_and_invalid_positions_are_safe() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        // No data source - scroll_row stays 0
        list.set_scroll_row(100);
        assert_eq!(list.scroll_row(), 0);

        list.set_data_source(Arc::new(StaticSource { rows: 5 }));
        list.set_scroll_row(3);
        assert_eq!(list.scroll_row(), 3);

        // Clamp to max index
        list.set_scroll_row(999);
        assert_eq!(list.scroll_row(), 4);

        // scroll_by_rows edge cases
        list.scroll_by_rows(-10);
        assert_eq!(list.scroll_row(), 0);

        list.scroll_by_rows(100);
        assert_eq!(list.scroll_row(), 4);
    }

    #[test]
    fn overscan_affects_visible_window() {
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 40));
        list.set_data_source(Arc::new(StaticSource { rows: 20 }));

        // With overscan=2 (default), visible window extends beyond viewport
        let (start, len) = list.visible_window();
        assert_eq!(start, 0);
        assert!(len > 2);

        // Reduce overscan to 0
        list.set_overscan(0);
        let (start2, len2) = list.visible_window();
        assert_eq!(start2, 0);
        assert_eq!(len2, 2);

        assert_eq!(list.overscan(), 0);
    }

    #[test]
    fn signal_emission_on_selection_and_window_change() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 5 }));

        let last_selection = Arc::new(std::sync::Mutex::new(None::<Option<usize>>));
        let sink = last_selection.clone();
        list.selection_changed.connect(move |sel| {
            *sink.lock().unwrap() = Some(*sel);
        });

        list.select_row(2);
        assert_eq!(*last_selection.lock().unwrap(), Some(Some(2)));
    }

    #[test]
    fn empty_source_handling() {
        let mut list = VirtualList::new(Rect::new(0, 0, 800, 600));
        list.set_data_source(Arc::new(StaticSource { rows: 0 }));

        assert_eq!(list.row_count(), 0);
        assert_eq!(list.visible_window(), (0, 0));
        assert!(list.fetch_visible_rows().is_empty());
        assert!(!list.select_row(0));
        assert_eq!(list.selected_row(), None);

        // Scrolling does nothing on empty source
        list.set_scroll_row(10);
        assert_eq!(list.scroll_row(), 0);
    }
}
