//! Unified incremental data source protocol for modern data views.
//!
//! BLUE9 R3 groundwork: provides a common pull-based window API that can back
//! virtualized list/table/tree widgets without duplicating model traversal logic.

use std::sync::Arc;

use crate::signal::GenericSignal;
use crate::widget::view_widgets::list_view::ListModel;
use crate::widget::view_widgets::table_widget::TableModel;
use crate::widget::view_widgets::tree_view::TreeModel;

/// Pull-based data source protocol used by modern data controls.
pub trait IncrementalTableDataSource: Send + Sync {
    /// Total rows available in source.
    fn row_count(&self) -> usize;
    /// Total columns available in source.
    fn column_count(&self) -> usize;
    /// Returns cell text for a row/column if available.
    fn data(&self, row: usize, column: usize) -> Option<String>;
    /// Optional monotonic revision for cache invalidation.
    fn revision(&self) -> u64 {
        0
    }
    /// Optional signal emitted when source projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
    /// Fetches a bounded window from the source.
    fn fetch_window(
        &self,
        row_start: usize,
        row_len: usize,
        column_start: usize,
        column_len: usize,
    ) -> Vec<Vec<Option<String>>> {
        let total_rows = self.row_count();
        let total_columns = self.column_count();

        if row_start >= total_rows || column_start >= total_columns {
            return Vec::new();
        }

        let row_end = row_start.saturating_add(row_len).min(total_rows);
        let col_end = column_start.saturating_add(column_len).min(total_columns);
        let mut window = Vec::with_capacity(row_end.saturating_sub(row_start));

        for row in row_start..row_end {
            let mut line = Vec::with_capacity(col_end.saturating_sub(column_start));
            for column in column_start..col_end {
                line.push(self.data(row, column));
            }
            window.push(line);
        }

        window
    }
}

/// Adapter that projects `ListModel` into the unified tabular data source protocol.
pub struct ListModelDataSource {
    model: Arc<dyn ListModel>,
}

impl ListModelDataSource {
    /// Creates a new list model adapter.
    pub fn new(model: Arc<dyn ListModel>) -> Self {
        Self { model }
    }

    /// Returns underlying list model.
    pub fn model_ref(&self) -> &Arc<dyn ListModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for ListModelDataSource {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn column_count(&self) -> usize {
        1
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        if column == 0 {
            self.model.data(row)
        } else {
            None
        }
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

/// Adapter that projects `TreeModel` into the unified tabular data source protocol.
pub struct TreeModelDataSource {
    model: Arc<dyn TreeModel>,
}

impl TreeModelDataSource {
    /// Creates a new tree model adapter.
    pub fn new(model: Arc<dyn TreeModel>) -> Self {
        Self { model }
    }

    /// Returns underlying tree model.
    pub fn model_ref(&self) -> &Arc<dyn TreeModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for TreeModelDataSource {
    fn row_count(&self) -> usize {
        self.model.node_count()
    }

    fn column_count(&self) -> usize {
        1
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        if column == 0 {
            self.model.node_path(row)
        } else {
            None
        }
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

/// Adapter that projects `TableModel` into the unified tabular data source protocol.
pub struct TableModelDataSource {
    model: Arc<dyn TableModel>,
}

impl TableModelDataSource {
    /// Creates a new table model adapter.
    pub fn new(model: Arc<dyn TableModel>) -> Self {
        Self { model }
    }

    /// Returns underlying table model.
    pub fn model_ref(&self) -> &Arc<dyn TableModel> {
        &self.model
    }
}

impl IncrementalTableDataSource for TableModelDataSource {
    fn row_count(&self) -> usize {
        self.model.row_count()
    }

    fn column_count(&self) -> usize {
        self.model.column_count()
    }

    fn data(&self, row: usize, column: usize) -> Option<String> {
        self.model.data(row, column)
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.model.data_changed_signal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticList;

    impl ListModel for StaticList {
        fn row_count(&self) -> usize {
            3
        }

        fn data(&self, row: usize) -> Option<String> {
            match row {
                0 => Some("A".to_string()),
                1 => Some("B".to_string()),
                2 => Some("C".to_string()),
                _ => None,
            }
        }
    }

    struct StaticTree;

    impl TreeModel for StaticTree {
        fn node_count(&self) -> usize {
            2
        }

        fn node_path(&self, index: usize) -> Option<String> {
            match index {
                0 => Some("root".to_string()),
                1 => Some("root/child".to_string()),
                _ => None,
            }
        }
    }

    struct StaticTable;

    impl TableModel for StaticTable {
        fn row_count(&self) -> usize {
            2
        }

        fn column_count(&self) -> usize {
            3
        }

        fn data(&self, row: usize, column: usize) -> Option<String> {
            Some(format!("{}:{}", row, column))
        }
    }

    #[test]
    fn list_adapter_projects_single_column_window() {
        let source = ListModelDataSource::new(Arc::new(StaticList));

        assert_eq!(source.row_count(), 3);
        assert_eq!(source.column_count(), 1);
        assert_eq!(source.data(1, 0), Some("B".to_string()));
        assert_eq!(source.data(1, 1), None);

        let window = source.fetch_window(1, 4, 0, 1);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0][0], Some("B".to_string()));
        assert_eq!(window[1][0], Some("C".to_string()));
    }

    #[test]
    fn tree_adapter_projects_single_column_window() {
        let source = TreeModelDataSource::new(Arc::new(StaticTree));

        assert_eq!(source.row_count(), 2);
        assert_eq!(source.column_count(), 1);
        assert_eq!(source.data(0, 0), Some("root".to_string()));

        let window = source.fetch_window(0, 2, 0, 1);
        assert_eq!(window.len(), 2);
        assert_eq!(window[1][0], Some("root/child".to_string()));
    }

    #[test]
    fn table_adapter_fetches_bounded_window() {
        let source = TableModelDataSource::new(Arc::new(StaticTable));

        assert_eq!(source.row_count(), 2);
        assert_eq!(source.column_count(), 3);
        assert_eq!(source.data(1, 2), Some("1:2".to_string()));

        let window = source.fetch_window(0, 2, 1, 5);
        assert_eq!(window.len(), 2);
        assert_eq!(window[0].len(), 2);
        assert_eq!(window[0][0], Some("0:1".to_string()));
        assert_eq!(window[1][1], Some("1:2".to_string()));

        let empty = source.fetch_window(10, 2, 0, 1);
        assert!(empty.is_empty());
    }
}
