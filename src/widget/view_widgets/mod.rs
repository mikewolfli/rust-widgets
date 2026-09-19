// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! View widgets module.
//!
//! Contains list, tree, table, and property view widgets.
pub mod data_grid;
pub mod data_source;
/// Recursive filter trees shared by `DataGrid` and any query-builder UI.
pub mod filter_expr;
/// GridTable — feature-rich virtualized table with grid lines, headers, sorting, and selection.
pub mod grid_table;
pub mod image_gallery;
pub mod list_view;
pub mod properties_panel;
pub mod property_grid;
/// Renders a recursive filter as editable condition rows.
pub mod query_builder;
pub mod table_widget;
pub mod tree_table;
pub mod tree_view;
pub mod virtual_list;
pub mod virtual_table;
// Re-export view widgets
pub use data_grid::{ColumnFilter, DataGrid, SortSpec};
pub use data_source::{
    IncrementalTableDataSource, ListModelDataSource, TableModelDataSource, TreeModelDataSource,
};
pub use filter_expr::{FilterCondition, FilterExpr, FilterOperator};
pub use grid_table::{GridTableSelectionMode, GridTableSortSpec, GridTableWidget};
pub use image_gallery::{GalleryImage, ImageGallery};
pub use list_view::ListView;
pub use properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue};
pub use property_grid::{PropertyGrid, PropertyItem};
pub use query_builder::{FilterConjunction, FilterField, QueryBuilder, QueryBuilderRow};
pub use table_widget::TableWidget;
pub use tree_table::{TreeTable, TreeTableModel};
pub use tree_view::TreeView;
pub use virtual_list::VirtualList;
pub use virtual_table::VirtualTable;
