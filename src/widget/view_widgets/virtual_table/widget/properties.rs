// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable property contract.

use super::*;

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
            "overscan_rows" => Ok(CapabilityValue::UInt(self.overscan_rows as u64)),
            "overscan_columns" => Ok(CapabilityValue::UInt(self.overscan_columns as u64)),
            "wheel_scroll_mode" => Ok(CapabilityValue::String(
                wheel_scroll_mode_to_str(self.wheel_scroll_mode()).to_string(),
            )),
            "header_visible" => Ok(CapabilityValue::Bool(self.show_header())),
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            // Row-major `(row, column)` pairs of the live selection, flattened
            // into one number list — the same carrier the timeline axis `range`
            // pair uses. Empty when nothing is selected.
            "selected_cells" => Ok(CapabilityValue::Tuple(
                self.selected_cells()
                    .iter()
                    .map(|(row, column)| {
                        CapabilityValue::Tuple(crate::compat::Vec::from([
                            CapabilityValue::UInt(*row as u64),
                            CapabilityValue::UInt(*column as u64),
                        ]))
                    })
                    .collect(),
            )),
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
            "wheel_scroll_mode" => {
                self.set_wheel_scroll_mode(expect_wheel_scroll_mode(value)?);
                Ok(())
            }
            "header_visible" => {
                self.set_show_header(expect_bool(value)?);
                Ok(())
            }
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            // Derived counts and the visible-window summary are computed from the
            // data source and layout, so they are refused as read-only rather
            // than reported as names this control does not know. `selected_cells`
            // is the live selection: it is changed by selecting, not by writing.
            "has_data_source" | "row_count" | "column_count" | "selected_cells"
            | "visible_window" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "wheel_scroll_mode",
            "header_visible",
            // `get` answers this name; leaving it out of the list made the schema
            // declare a readable property no contract published.
            "visible_window",
            "selection_mode",
            "selected_cells",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `virtual_table` publishes.
    ///
    /// `clear_data_source` and `clear_selection` are payload-free and execute
    /// here; `fetch_visible_window` is a *query*, so it belongs to the read path
    /// (the `visible_window` property) rather than an action. The remaining
    /// names need a payload and are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "fetch_visible_window" => {
                // This is a query with no result channel. Warming the cache is the only
                // useful effect here; the owned fetch copied the whole window and then
                // discarded it.
                let _ = self.visible_window_cells();
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
