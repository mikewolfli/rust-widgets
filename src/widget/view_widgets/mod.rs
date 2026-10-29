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
/// Inspector-style property grid plus the categorized panel built on it.
pub mod properties;
/// Renders a recursive filter as editable condition rows.
pub mod query_builder;
pub mod table_widget;
/// COMP-F04: bounded LRU cache of down-sampled bitmaps.
pub mod thumbnail_cache;
/// Outline tree plus the tree-with-columns control sharing its model.
pub mod tree;
pub mod virtual_list;
pub mod virtual_table;
// Re-export view widgets
pub use data_grid::column_chooser::ColumnChooser;
pub use data_grid::grid_find_bar::GridFindBar;
// Table companions live in the `data_grid` module folder; re-exported so the existing
// `view_widgets::column_chooser` / `view_widgets::grid_find_bar` paths keep resolving.
pub use data_grid::{column_chooser, grid_find_bar};
pub use data_grid::{ColumnFilter, ColumnWidthMode, DataGrid, EditTriggers, SortSpec};
pub use data_source::{
    IncrementalTableDataSource, ListModelDataSource, TableModelDataSource, TreeModelDataSource,
};
pub use filter_expr::{FilterCondition, FilterExpr, FilterOperator};
pub use grid_table::{
    GridSectionResizeMode, GridTableSelectionMode, GridTableSortSpec, GridTableWidget,
};
pub use image_gallery::{GalleryImage, ImageGallery};
pub use list_view::ListView;
// The property family lives in the `properties` module folder; re-exported so the existing
// `view_widgets::properties_panel` / `view_widgets::property_grid` paths keep resolving.
pub use properties::properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue};
pub use properties::property_grid::{PropertyGrid, PropertyItem};
pub use properties::{properties_panel, property_grid};
pub use query_builder::{
    FilterConjunction, FilterField, QueryBuilder, QueryBuilderGroup, QueryBuilderRow,
};
pub use table_widget::{TableEditTriggers, TableWidget};
pub use thumbnail_cache::ThumbnailCache;
// Same for the tree companions: `view_widgets::tree_view` / `view_widgets::tree_table`
// remain the canonical module paths.
pub use tree::tree_table::{TreeTable, TreeTableModel};
pub use tree::tree_view::TreeView;
pub use tree::{tree_table, tree_view};
pub use virtual_list::VirtualList;
pub use virtual_table::{VirtualCellDelegate, VirtualTable};
