// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TableWidget property contract.

use super::*;

/// `TableWidget`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_view.in.rs` / `access_write_view.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
///
/// The old `WidgetKind::Table` arms were shared with `DataGrid` and
/// `VirtualTable`, which carry most of the names (`scroll_row`, `row_height`,
/// `sort_specs`, `visible_window`, …). Those stay with the controls that own the
/// fields; this block publishes only what `TableWidget` itself answers.
impl WidgetProperties for TableWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_model" => Ok(CapabilityValue::Bool(self.has_model())),
            "has_delegate" => Ok(CapabilityValue::Bool(self.has_delegate())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "selection_mode" => Ok(CapabilityValue::String(
                selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "header_visible" => Ok(CapabilityValue::Bool(self.header_visible())),
            "editable" => Ok(CapabilityValue::Bool(self.editable())),
            "edit_triggers" => {
                Ok(CapabilityValue::String(edit_triggers_to_str(self.edit_triggers()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selection_mode" => {
                self.set_selection_mode(expect_selection_mode(value)?);
                Ok(())
            }
            "header_visible" => {
                self.set_header_visible(expect_bool(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "edit_triggers" => {
                self.set_edit_triggers(expect_edit_triggers(value)?);
                Ok(())
            }
            "column_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "has_delegate" => Err(CapabilityAccessError::ReadOnlyProperty),
            "has_model" => Err(CapabilityAccessError::ReadOnlyProperty),
            "row_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "has_model",
            "has_delegate",
            "row_count",
            "column_count",
            "selection_mode",
            "header_visible",
            "editable",
            "edit_triggers",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `table` publishes. Both are payload-free.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_selection" => {
                self.clear_selection();
                Ok(())
            }
            "clear_focused_row" => {
                self.clear_focused_row();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
