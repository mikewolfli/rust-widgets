// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `DataGrid`'s `Widget` and `WidgetProperties` contracts.

use super::*;

impl Widget for DataGrid {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::DATA_GRID_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::DATA_GRID_DEFAULT_HEIGHT,
        )
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `DataGrid` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

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
                |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                        CapabilityValue::UInt(v.2 as u64),
                        CapabilityValue::UInt(v.3 as u64),
                    ])
                },
            )),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "edit_requested" => {
                Some(EventSignalRef::mapped("edit_requested", &self.edit_requested, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::Int(v.0 as i64),
                        crate::widget::capability::CapabilityValue::Int(v.1 as i64),
                    ]))
                }))
            }
            "filter_changed" => {
                Some(EventSignalRef::mapped("filter_changed", &self.filter_changed, |v| {
                    crate::widget::capability::CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.header_clicked, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            "page_applied" => {
                Some(EventSignalRef::mapped("page_applied", &self.page_applied, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "sort_changed" => {
                Some(EventSignalRef::mapped("sort_changed", &self.sort_changed, |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::Bool(v.1),
                    ]))
                }))
            }
            // The five interaction signals the control has always emitted natively; without
            // these arms the capability surface could observe none of them (the schema gate
            // answers `UnknownProperty` for any name absent here).
            "selection_changed" => Some(EventSignalRef::mapped(
                "selection_changed",
                self.selection_changed_signal(),
                |_v| CapabilityValue::Null,
            )),
            "current_changed" => Some(EventSignalRef::mapped(
                "current_changed",
                self.current_changed_signal(),
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            "cell_clicked" => {
                Some(EventSignalRef::mapped("cell_clicked", self.cell_clicked_signal(), |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "cell_double_clicked" => Some(EventSignalRef::mapped(
                "cell_double_clicked",
                self.cell_double_clicked_signal(),
                |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                },
            )),
            "column_resized" => {
                Some(EventSignalRef::mapped("column_resized", self.column_resized_signal(), |v| {
                    crate::widget::capability::CapabilityValue::Tuple(crate::compat::Vec::from([
                        crate::widget::capability::CapabilityValue::UInt(v.0 as u64),
                        crate::widget::capability::CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

/// `DataGrid`'s property contract.
///
/// This control is one of the fourteen the old centralised dispatch could not
/// reach. It shares `WidgetKind::Table` with `TableWidget`, so the
/// `WidgetKind::Table` arms downcast to `TableWidget` and a `DataGrid` answered
/// `UnsupportedOnWidget` for **every** property — despite
/// `DATA_GRID_PROPERTIES` describing a full contract for it. Declaring the
/// contract here makes it reachable, which is the whole point of moving the
/// contract onto the control (BLUE15 rule #67).
///
/// The property set follows `DATA_GRID_PROPERTIES`: the two counts and the two
/// item collections are derived state and therefore read-only, while the viewport
/// and sizing knobs are writable.
impl WidgetProperties for DataGrid {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "has_data_source" => Ok(CapabilityValue::Bool(self.has_data_source())),
            "row_count" => Ok(CapabilityValue::UInt(self.row_count() as u64)),
            "column_count" => Ok(CapabilityValue::UInt(self.column_count() as u64)),
            "scroll_row" => Ok(CapabilityValue::UInt(self.scroll_row() as u64)),
            "scroll_column" => Ok(CapabilityValue::UInt(self.scroll_column() as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "column_width" => Ok(CapabilityValue::UInt(self.default_column_width() as u64)),
            "frozen_columns" => Ok(CapabilityValue::UInt(self.frozen_columns() as u64)),
            "sort_spec_count" => Ok(CapabilityValue::UInt(self.sort_specs().len() as u64)),
            // The count is of *conditions*, not of `ColumnFilter`s: a tree with a
            // nested `And` holds more conditions than the flat list would, and the
            // number a query-builder UI shows is the condition count.
            "filter_count" => {
                Ok(CapabilityValue::UInt(self.filter_expr().condition_count() as u64))
            }
            "sort_specs" => Ok(CapabilityValue::String(sort_specs_to_string(self.sort_specs()))),
            "filters" => Ok(CapabilityValue::String(column_filters_to_string(&self.filters()))),
            "selection_mode" => Ok(CapabilityValue::String(
                data_grid_selection_mode_to_str(self.selection_mode()).to_string(),
            )),
            "selection_behavior" => Ok(CapabilityValue::String(
                selection_behavior_to_str(self.selection_behavior()).to_string(),
            )),
            "sort_indicator_visible" => Ok(CapabilityValue::Bool(self.sort_indicator_visible())),
            "section_resize_mode" => Ok(CapabilityValue::String(
                column_width_mode_to_str(self.section_resize_mode()).to_string(),
            )),
            "current_cell" => match self.current_cell() {
                Some((row, column)) => Ok(CapabilityValue::String(format!("{row},{column}"))),
                None => Ok(CapabilityValue::Null),
            },
            "summary_text" => Ok(CapabilityValue::String(self.summary_text())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "summary_text" => Err(CapabilityAccessError::ReadOnlyProperty),
            "scroll_row" => {
                self.set_scroll_row(expect_usize(value)?);
                Ok(())
            }
            "scroll_column" => {
                self.set_scroll_column(expect_usize(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32_saturating(value)?);
                Ok(())
            }
            "column_width" => {
                self.set_default_column_width(expect_u32_saturating(value)?);
                Ok(())
            }
            "frozen_columns" => {
                self.set_frozen_columns(expect_usize(value)?);
                Ok(())
            }
            "sort_specs" => {
                self.set_sort_specs(expect_sort_specs(value)?);
                Ok(())
            }
            "filters" => {
                self.set_filters(expect_column_filters(value)?);
                Ok(())
            }
            "selection_mode" => {
                self.set_selection_mode(expect_data_grid_selection_mode(value)?);
                Ok(())
            }
            "selection_behavior" => {
                self.set_selection_behavior(expect_selection_behavior(value)?);
                Ok(())
            }
            "sort_indicator_visible" => {
                self.set_sort_indicator_visible(crate::widget::capability::coercion::expect_bool(
                    value,
                )?);
                Ok(())
            }
            "section_resize_mode" => {
                self.set_section_resize_mode(expect_column_width_mode(value)?);
                Ok(())
            }
            "current_cell" => Err(CapabilityAccessError::ReadOnlyProperty),
            // Derived and structural state: presented but not settable, matching
            // `DATA_GRID_PROPERTIES`' `writable: false` entries.
            "has_data_source" | "row_count" | "column_count" | "sort_spec_count"
            | "filter_count" => Err(CapabilityAccessError::ReadOnlyProperty),
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
            "frozen_columns",
            "sort_spec_count",
            "filter_count",
            "sort_specs",
            "filters",
            "selection_mode",
            "selection_behavior",
            "sort_indicator_visible",
            "section_resize_mode",
            "current_cell",
            "summary_text",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `data_grid` publishes.
    ///
    /// `clear_data_source` is payload-free and executes here. The remaining names
    /// need a payload (a source, an index, a size) and are answered through the
    /// property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_data_source" => {
                self.clear_data_source();
                Ok(())
            }
            // `select_all` takes every cell of the current behaviour, so it is one of the
            // payload-free commands the arm can honour outright.
            "select_all" => {
                self.select_all();
                Ok(())
            }
            "set_data_source" | "set_scroll_row" | "set_scroll_column" | "set_row_height"
            | "set_column_width" | "set_frozen_columns" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}
