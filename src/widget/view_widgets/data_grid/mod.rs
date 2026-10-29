// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Data grid widget backed by incremental data source protocol.

// Shared prelude: child modules pull these in with `use super::*;`.
use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::compat::Rc;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, WheelScroll};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{ConnectionHandle, ConnectionScope, GenericSignal, Signal1};
use crate::widget::capability::access::{column_filters_to_string, sort_specs_to_string};
use crate::widget::capability::coercion::{
    expect_bool, expect_column_filters, expect_sort_specs, expect_string_list,
    expect_u32_saturating, expect_usize,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::{
    ColumnAggregate, ColumnAggregateKind, FilterRow, GridStatusBar, HeaderMenu, SortState,
    SummaryRow,
};
use crate::widget::metrics::{effective_font, ControlMetrics};
use crate::widget::nav_widgets::pagination::Pagination;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use self::column_chooser::ColumnChooser;
use super::data_source::{DataRole, IncrementalTableDataSource};

use super::filter_expr::{FilterCondition, FilterExpr, FilterOperator};

mod bindings;
/// COMP-F03: show/hide and reorder table columns.
pub mod column_chooser;
mod columns;
mod events;
mod grid;
/// COMP-F02: incremental find bar for the table family.
pub mod grid_find_bar;
mod header;
mod paint;
mod projection;
mod types;
mod widget_impl;

#[cfg(test)]
mod tests;

// Crate-internal surface: impl blocks spread over the children share one state.
pub(crate) use bindings::*;
pub(crate) use columns::*;
pub(crate) use projection::*;
pub(crate) use types::*;

// Public API: unchanged paths under `data_grid::`.
pub use bindings::HeaderMenuPoll;
pub use grid::DataGrid;
pub use header::GridViewHeader;
pub use types::{
    CellSpan, ColumnFilter, ColumnSpec, ColumnWidthMode, DataGridCellDelegate, DataGridCellEditor,
    DataGridSelectionBehavior, EditTriggers, RowDetailProvider, SortOrder, SortSpec,
};
