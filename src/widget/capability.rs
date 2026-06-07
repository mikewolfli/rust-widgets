//! Widget capability metadata, runtime factory, and generic property read/write layer.
//!
//! # Purpose
//!
//! This module implements BLUE9 R2 (Capability Metadata Layer). It serves as the
//! bridge between the concrete widget struct hierarchy and a uniform, introspectable
//! API for querying and manipulating widget state without direct type knowledge.
//! It enables design tools, scripting, serialization, and cross-widget automation
//! to interact with any registered widget through a single generic interface.
//!
//! # Key Concepts
//!
//! ## CapabilityValue
//! An enum over all property value types (Bool, Int, UInt, Float, String, Null)
//! that the generic read/write layer can transport. All property access through
//! `WidgetFactory` goes via this type — downcast on write, upcast on read.
//!
//! ## PropertySchema
//! Describes a single widget property: its name, value kind (Bool/Int/Enum/…),
//! and whether it supports generic read and/or write access. Each widget kind
//! declares a static `&[PropertySchema]` that the factory uses at runtime to
//! enumerate, validate, and discover properties.
//!
//! ## WidgetCapability
//! The full capability record for one widget kind, containing:
//! - `kind` — the `WidgetKind` enum variant.
//! - `canonical_name` / `aliases` — string keys for factory lookup (case/separator
//!   insensitive, so `"list_view"`, `"listview"`, `"ListView"` all resolve).
//! - `properties` — the array of `PropertySchema` entries.
//! - `events` — string names of signals the widget can emit (e.g. `"clicked"`,
//!   `"selection_changed"`, `"text_changed"`).
//! - `commands` — string names of imperative actions the widget supports
//!   (e.g. `"set_text"`, `"clear_selection"`, `"play"`).
//!
//! ## WidgetFactory
//! The central registry that:
//! 1. Maps canonical names and aliases → `WidgetCapability` + constructor closure.
//! 2. Constructs widgets via `create(name, geometry, text)` or `create_by_kind(kind, …)`.
//! 3. Provides generic property read/write through `read_property` / `write_property`,
//!    which downcast the trait object to the concrete widget type and call the
//!    corresponding getter/setter.
//! 4. Exports `capability(name)` / `capability_by_kind(kind)` for introspection.
//! 5. Generates `capability_manifest()` for serialization/export of the full schema.
//!
//! ## Generic Read/Write Dispatch
//!
//! `read_widget_property_value()` and `write_widget_property_value()` are large
//! match-on-`widget.kind()` functions that downcast the `&dyn Widget` to the
//! concrete type (via `widget_as!` / `widget_as_mut!`) and call the native getter
//! or setter. This avoids requiring every widget to implement a separate trait for
//! generic property access — the dispatch is centralized in this one module.
//!
//! # Capability Registration
//!
//! Each widget kind that should be constructible through the factory must:
//! 1. Define a `const XXX_PROPERTIES: &[PropertySchema]` array.
//! 2. Write a `fn xxx_capability() -> WidgetCapability` function referencing that array.
//! 3. Write a `fn create_xxx(geometry, text) -> Box<dyn Widget>` constructor.
//! 4. Register all three in `WidgetFactory::register_core_widgets()`.
//!
//! Currently **64 widget kinds** are registered, covering all major control
//! families: base widgets, inputs, containers, dialogs, displays, menu/toolbar,
//! advanced widgets, special widgets (productivity + rich media), and web widgets.
//!
//! # Relationship to BLUE9 Milestones
//!
//! - **R2 (Extensibility)**: This module IS the capability metadata layer.
//!   A third-party widget can register itself via `factory.register(...)` at runtime.
//! - **R1 (API Symmetry)**: The property schemas document the "can read / can write"
//!   contract for each widget, making gaps visible and enforceable by test.
//! - **R3-R5**: All modern data, productivity, and rich media widgets have
//!   capability entries alongside their concrete implementations.
//! - **R6 (Quality Gate)**: The manifest export and factory tests are part of the
//!   CI quality matrix.

use std::collections::HashMap;

use crate::core::Rect;

use super::{
    advanced_widgets::calendar::Calendar,
    advanced_widgets::date_edit::{Date, DateEdit},
    advanced_widgets::date_time_edit::DateTimeEdit,
    advanced_widgets::dial::Dial,
    advanced_widgets::pie_menu::PieMenu,
    advanced_widgets::ribbon_bar::RibbonBar,
    advanced_widgets::tab_bar::TabBar,
    advanced_widgets::time_edit::{Time, TimeEdit},
    base_widgets::button::Button,
    base_widgets::checkbox::{CheckBox, CheckState},
    base_widgets::label::Label,
    base_widgets::radiobutton::RadioButton,
    container_widgets::collapsible_pane::CollapsiblePane,
    container_widgets::dockwidget::DockWidget,
    container_widgets::groupbox::GroupBox,
    container_widgets::mdiarea::MdiArea,
    container_widgets::scrollarea::ScrollArea,
    container_widgets::splitter::Splitter,
    container_widgets::stackedwidget::StackedWidget,
    container_widgets::tabwidget::TabWidget,
    container_widgets::toolbox::ToolBox,
    dialog::file_dialog::FileDialog,
    dialog::font_dialog::FontDialog,
    dialog::input_dialog::InputDialog,
    dialog::message_box::MessageBox,
    dialog::popup_window::PopupWindow,
    dialog::progress_dialog::ProgressDialog,
    display_widgets::lcd_number::{LCDNumber, LCDNumberMode, SegmentStyle},
    display_widgets::progressbar::ProgressBar,
    display_widgets::scrollbar::ScrollBar,
    display_widgets::slider::{Slider, TickPosition},
    input_widgets::combobox::ComboBox,
    input_widgets::command_link::CommandLink,
    input_widgets::font_combo_box::FontComboBox,
    input_widgets::lineedit::LineEdit,
    input_widgets::listbox::{ListBox, SelectionMode as ListBoxSelectionMode},
    input_widgets::spinbox::SpinBox,
    input_widgets::textedit::TextEdit,
    menu_toolbar::action::Action,
    menu_toolbar::menu::Menu,
    menu_toolbar::menu_bar::MenuBar,
    menu_toolbar::tool_bar::ToolBar,
    special_widgets::breadcrumb::Breadcrumb,
    special_widgets::chip::Chip,
    special_widgets::code_editor::CodeEditor,
    special_widgets::color_picker::ColorPicker,
    special_widgets::freeform_shape::{FreeformShapeWidget, ShapePath},
    special_widgets::gantt_widget::GanttWidget,
    special_widgets::grid::GridWidget,
    special_widgets::map_view::MapView,
    special_widgets::media_player::MediaPlayer,
    special_widgets::segmented_control::SegmentedControl,
    special_widgets::snackbar::Snackbar,
    special_widgets::split_button::SplitButton,
    special_widgets::terminal_view::TerminalView,
    view_widgets::data_grid::{ColumnFilter, DataGrid, SortSpec},
    view_widgets::list_view::ListView,
    view_widgets::table_widget::TableWidget,
    view_widgets::tree_table::TreeTable,
    view_widgets::tree_view::TreeView,
    view_widgets::virtual_list::VirtualList,
    view_widgets::virtual_table::VirtualTable,
    web_widgets::web_view::WebView,
    window::Window,
    Widget, WidgetKind,
};

pub mod types;
pub use types::*;

impl Default for WidgetFactory {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

impl WidgetFactory {
    /// Creates an empty factory.
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            key_to_index: HashMap::new(),
            kind_to_index: Vec::new(),
            constructors: HashMap::new(),
        }
    }

    /// Creates a factory preloaded with core widget registrations.
    pub fn new_with_defaults() -> Self {
        let mut factory = Self::new();
        factory.register_core_widgets();
        factory
    }

    /// Registers one widget capability and constructor.
    pub fn register(&mut self, capability: WidgetCapability, ctor: WidgetCtor) {
        let idx = self.capabilities.len();
        if self.kind_to_index.iter().all(|(kind, _)| *kind != capability.kind) {
            self.kind_to_index.push((capability.kind, idx));
        }

        let canonical_key = normalize_key(capability.canonical_name);
        self.key_to_index.insert(canonical_key.clone(), idx);
        self.constructors.insert(canonical_key, ctor);

        for alias in capability.aliases {
            let key = normalize_key(alias);
            self.key_to_index.insert(key.clone(), idx);
            self.constructors.insert(key, ctor);
        }

        self.capabilities.push(capability);
    }

    /// Creates a widget by canonical name or alias.
    pub fn create(
        &self,
        kind_or_name: &str,
        geometry: Rect,
        text: &str,
    ) -> Option<Box<dyn Widget>> {
        let key = normalize_key(kind_or_name);
        self.constructors.get(&key).map(|ctor| ctor(geometry, text))
    }

    /// Creates a widget by `WidgetKind` using the registered canonical builder.
    pub fn create_by_kind(
        &self,
        kind: WidgetKind,
        geometry: Rect,
        text: &str,
    ) -> Option<Box<dyn Widget>> {
        let capability = self.capability_by_kind(kind)?;
        self.create(capability.canonical_name, geometry, text)
    }

    /// Returns capability metadata by canonical name or alias.
    pub fn capability(&self, kind_or_name: &str) -> Option<&WidgetCapability> {
        let key = normalize_key(kind_or_name);
        let idx = self.key_to_index.get(&key).copied()?;
        self.capabilities.get(idx)
    }

    /// Returns capability metadata by widget kind.
    pub fn capability_by_kind(&self, kind: WidgetKind) -> Option<&WidgetCapability> {
        let idx = self
            .kind_to_index
            .iter()
            .find(|(stored_kind, _)| *stored_kind == kind)
            .map(|(_, index)| *index)?;
        self.capabilities.get(idx)
    }

    /// Returns all registered capabilities.
    pub fn capabilities(&self) -> &[WidgetCapability] {
        &self.capabilities
    }

    /// Reads a known property from a widget instance by property name.
    ///
    /// This is a minimal read-only reflection layer intended for R2 integration.
    pub fn read_property(
        &self,
        widget: &dyn Widget,
        property_name: &str,
    ) -> Result<CapabilityValue, CapabilityAccessError> {
        let capability =
            self.capability_for_widget(widget).ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) =
            capability.properties.iter().find(|schema| normalize_key(schema.name) == normalized)
        else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        if !property.readable {
            return Err(CapabilityAccessError::UnsupportedOnWidget);
        }

        read_widget_property_value(widget, property.name)
    }

    /// Writes a known property on a widget instance by property name.
    ///
    /// This is a minimal write path for stable scalar properties.
    pub fn write_property(
        &self,
        widget: &mut dyn Widget,
        property_name: &str,
        value: CapabilityValue,
    ) -> Result<(), CapabilityAccessError> {
        let capability =
            self.capability_for_widget(widget).ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) =
            capability.properties.iter().find(|schema| normalize_key(schema.name) == normalized)
        else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        if !property.writable {
            return Err(CapabilityAccessError::ReadOnlyProperty);
        }

        write_widget_property_value(widget, property.name, value)
    }

    fn capability_for_widget(&self, widget: &dyn Widget) -> Option<&WidgetCapability> {
        if widget_as::<DataGrid>(widget).is_some() {
            return self.capability("data_grid");
        }
        if widget_as::<VirtualTable>(widget).is_some() {
            return self.capability("virtual_table");
        }
        if widget_as::<TreeTable>(widget).is_some() {
            return self.capability("tree_table");
        }
        self.capability_by_kind(widget.kind())
    }

    /// Returns a schema-level default value for a known property.
    pub fn default_property_value(
        &self,
        kind_or_name: &str,
        property_name: &str,
    ) -> Result<CapabilityValue, CapabilityAccessError> {
        let capability =
            self.capability(kind_or_name).ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) =
            capability.properties.iter().find(|schema| normalize_key(schema.name) == normalized)
        else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        default_widget_property_value(capability.kind, property.name)
            .ok_or(CapabilityAccessError::UnsupportedOnWidget)
    }

    /// Returns one property schema by canonical/alias widget name and property name.
    pub fn property_schema(
        &self,
        kind_or_name: &str,
        property_name: &str,
    ) -> Result<PropertySchema, CapabilityAccessError> {
        let capability =
            self.capability(kind_or_name).ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) =
            capability.properties.iter().find(|schema| normalize_key(schema.name) == normalized)
        else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        Ok(*property)
    }

    /// Exports complete manifest (schema + default values) for one widget capability.
    pub fn capability_manifest(
        &self,
        kind_or_name: &str,
    ) -> Result<WidgetCapabilityManifest, CapabilityAccessError> {
        let capability =
            self.capability(kind_or_name).ok_or(CapabilityAccessError::UnknownWidget)?;

        let mut properties = Vec::with_capacity(capability.properties.len());
        for property in capability.properties {
            let default_value = default_widget_property_value(capability.kind, property.name)
                .ok_or(CapabilityAccessError::UnsupportedOnWidget)?;
            properties.push(CapabilityPropertyManifest { schema: *property, default_value });
        }

        Ok(WidgetCapabilityManifest {
            kind: capability.kind,
            canonical_name: capability.canonical_name,
            aliases: capability.aliases.to_vec(),
            properties,
            events: capability.events.to_vec(),
            commands: capability.commands.to_vec(),
        })
    }

    fn register_core_widgets(&mut self) {
        self.register(button_capability(), create_button);
        self.register(label_capability(), create_label);
        self.register(check_box_capability(), create_check_box);
        self.register(radio_button_capability(), create_radio_button);
        self.register(slider_capability(), create_slider);
        self.register(progress_bar_capability(), create_progress_bar);
        self.register(scroll_bar_capability(), create_scroll_bar);
        self.register(list_box_capability(), create_list_box);
        self.register(spin_box_capability(), create_spin_box);
        self.register(combo_box_capability(), create_combo_box);
        self.register(dial_capability(), create_dial);
        self.register(window_capability(), create_window);
        self.register(group_box_capability(), create_group_box);
        self.register(splitter_capability(), create_splitter);
        self.register(lcd_number_capability(), create_lcd_number);
        self.register(command_link_capability(), create_command_link);
        self.register(font_combo_box_capability(), create_font_combo_box);
        self.register(action_capability(), create_action);
        self.register(tool_box_capability(), create_tool_box);
        self.register(tab_bar_capability(), create_tab_bar);
        self.register(calendar_capability(), create_calendar);
        self.register(date_edit_capability(), create_date_edit);
        self.register(time_edit_capability(), create_time_edit);
        self.register(line_edit_capability(), create_line_edit);
        self.register(list_view_capability(), create_list_view);
        self.register(tree_view_capability(), create_tree_view);
        self.register(table_widget_capability(), create_table_widget);
        self.register(data_grid_capability(), create_data_grid);
        self.register(tree_table_capability(), create_tree_table);
        self.register(virtual_table_capability(), create_virtual_table);
        self.register(virtual_list_capability(), create_virtual_list);
        self.register(menu_capability(), create_menu);
        self.register(menu_bar_capability(), create_menu_bar);
        self.register(tool_bar_capability(), create_tool_bar);
        self.register(ribbon_bar_capability(), create_ribbon_bar);
        self.register(color_picker_capability(), create_color_picker);
        self.register(code_editor_capability(), create_code_editor);
        self.register(gantt_widget_capability(), create_gantt_widget);
        self.register(terminal_view_capability(), create_terminal_view);
        self.register(snackbar_capability(), create_snackbar);
        self.register(map_view_capability(), create_map_view);
        self.register(media_player_capability(), create_media_player);
        self.register(breadcrumb_capability(), create_breadcrumb);
        self.register(split_button_capability(), create_split_button);
        self.register(segmented_control_capability(), create_segmented_control);
        self.register(chip_capability(), create_chip);
        self.register(grid_capability(), create_grid);
        self.register(freeform_shape_capability(), create_freeform_shape);

        // ── Dialog widgets ────────────────────────────────────────
        self.register(message_box_capability(), create_message_box);
        self.register(file_dialog_capability(), create_file_dialog);
        self.register(font_dialog_capability(), create_font_dialog);
        self.register(input_dialog_capability(), create_input_dialog);
        self.register(progress_dialog_capability(), create_progress_dialog);
        self.register(popup_window_capability(), create_popup_window);

        // ── Container widgets ─────────────────────────────────────
        self.register(scroll_area_capability(), create_scroll_area);
        self.register(tab_widget_capability(), create_tab_widget);
        self.register(stacked_widget_capability(), create_stacked_widget);
        self.register(collapsible_pane_capability(), create_collapsible_pane);
        self.register(dock_widget_capability(), create_dock_widget);
        self.register(mdi_area_capability(), create_mdi_area);

        // ── Text widget ───────────────────────────────────────────
        self.register(text_edit_capability(), create_text_edit);

        // ── Web widget ────────────────────────────────────────────
        self.register(web_view_capability(), create_web_view);

        // ── Advanced widgets ──────────────────────────────────────
        self.register(pie_menu_capability(), create_pie_menu);
        self.register(date_time_edit_capability(), create_date_time_edit);
    }
}

fn read_widget_property_value(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => match property_name {
            "text" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::String(button.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "pressed" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::Bool(button.is_pressed()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "default" => {
                if let Some(button) = widget_as::<Button>(widget) {
                    Ok(CapabilityValue::Bool(button.is_default()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "enabled" => Ok(CapabilityValue::Bool(widget.is_enabled())),
            "tooltip" => Ok(CapabilityValue::String(widget.tooltip().to_string())),
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Label => match property_name {
            "text" => {
                if let Some(label) = widget_as::<Label>(widget) {
                    Ok(CapabilityValue::String(label.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "alignment" => {
                if let Some(label) = widget_as::<Label>(widget) {
                    Ok(CapabilityValue::String(alignment_to_str(label.alignment()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckBox => match property_name {
            "text" => {
                if let Some(check_box) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::String(check_box.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "state" => {
                if let Some(check_box) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::String(check_state_to_str(check_box.state()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(check_box) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::Bool(check_box.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tristate_enabled" => {
                if let Some(check_box) = widget_as::<CheckBox>(widget) {
                    Ok(CapabilityValue::Bool(check_box.is_tristate_enabled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RadioButton => match property_name {
            "text" => {
                if let Some(radio_button) = widget_as::<RadioButton>(widget) {
                    Ok(CapabilityValue::String(radio_button.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(radio_button) = widget_as::<RadioButton>(widget) {
                    Ok(CapabilityValue::Bool(radio_button.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "group_id" => {
                if let Some(radio_button) = widget_as::<RadioButton>(widget) {
                    Ok(match radio_button.group_id() {
                        Some(group_id) => CapabilityValue::String(group_id.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Slider => match property_name {
            "minimum" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(slider.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tick_position" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::String(
                        tick_position_to_str(slider.tick_position()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tick_interval" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.tick_interval() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tracking" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Bool(slider.tracking()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_position" => {
                if let Some(slider) = widget_as::<Slider>(widget) {
                    Ok(CapabilityValue::Int(slider.slider_position() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ProgressBar => match property_name {
            "minimum" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(progress_bar.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(progress_bar.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Int(progress_bar.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "text_visible" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Bool(progress_bar.is_text_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(progress_bar.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "inverted_appearance" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Bool(progress_bar.is_inverted_appearance()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "progress" => {
                if let Some(progress_bar) = widget_as::<ProgressBar>(widget) {
                    Ok(CapabilityValue::Float(progress_bar.progress() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ScrollBar => match property_name {
            "minimum" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(scroll_bar.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(scroll_bar.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(scroll_bar.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(scroll_bar.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Int(scroll_bar.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(scroll_bar.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_size" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Float(scroll_bar.slider_size() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "slider_position" => {
                if let Some(scroll_bar) = widget_as::<ScrollBar>(widget) {
                    Ok(CapabilityValue::Float(scroll_bar.slider_position() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ListBox => match property_name {
            "item_count" => {
                if let Some(list_box) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::UInt(list_box.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selection_mode" => {
                if let Some(list_box) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::String(
                        list_box_selection_mode_to_str(list_box.selection_mode()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_row" => {
                if let Some(list_box) = widget_as::<ListBox>(widget) {
                    Ok(match list_box.current_row() {
                        Some(row) => CapabilityValue::UInt(row as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_height" => {
                if let Some(list_box) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::Float(list_box.item_height() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_count" => {
                if let Some(list_box) = widget_as::<ListBox>(widget) {
                    Ok(CapabilityValue::UInt(list_box.selected_indices().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::SpinBox => match property_name {
            "minimum" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(spin_box.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(spin_box.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(spin_box.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Int(spin_box.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "prefix" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::String(spin_box.prefix().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "suffix" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::String(spin_box.suffix().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "special_value_text" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(match spin_box.special_value_text() {
                        Some(value) => CapabilityValue::String(value.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "wrapping" => {
                if let Some(spin_box) = widget_as::<SpinBox>(widget) {
                    Ok(CapabilityValue::Bool(spin_box.wrapping()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ComboBox => match property_name {
            "item_count" => {
                if let Some(combo_box) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::UInt(combo_box.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(combo_box) = widget_as::<ComboBox>(widget) {
                    Ok(match combo_box.current_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_text" => {
                if let Some(combo_box) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::String(combo_box.current_text()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "editable" => {
                if let Some(combo_box) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::Bool(combo_box.is_editable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_visible_items" => {
                if let Some(combo_box) = widget_as::<ComboBox>(widget) {
                    Ok(CapabilityValue::UInt(combo_box.max_visible_items() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Dial => match property_name {
            "minimum" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.minimum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.maximum() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "value" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.value() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "single_step" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.single_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "page_step" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Int(dial.page_step() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "notches_visible" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Bool(dial.notches_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "notch_target" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Float(dial.notch_target()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "wrapping" => {
                if let Some(dial) = widget_as::<Dial>(widget) {
                    Ok(CapabilityValue::Bool(dial.wrapping()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Window => match property_name {
            "title" => {
                if let Some(window) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::String(window.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "title_bar_height" => {
                if let Some(window) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(window.title_bar_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "close_button_size" => {
                if let Some(window) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(window.close_button_size() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "button_spacing" => {
                if let Some(window) = widget_as::<Window>(widget) {
                    Ok(CapabilityValue::UInt(window.button_spacing() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::GroupBox => match property_name {
            "title" => {
                if let Some(group_box) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::String(group_box.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "alignment" => {
                if let Some(group_box) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::String(alignment_to_str(group_box.alignment()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checkable" => {
                if let Some(group_box) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::Bool(group_box.is_checkable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(group_box) = widget_as::<GroupBox>(widget) {
                    Ok(CapabilityValue::Bool(group_box.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Splitter => match property_name {
            "orientation" => {
                if let Some(splitter) = widget_as::<Splitter>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(splitter.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "pane_count" => {
                if let Some(splitter) = widget_as::<Splitter>(widget) {
                    Ok(CapabilityValue::UInt(splitter.pane_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LCDNumber => match property_name {
            "value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "min_value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.min_value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_value" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Float(lcd.max_value()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "num_digits" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Int(lcd.num_digits() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "small_decimal_point" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::Bool(lcd.is_small_decimal_point()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "mode" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::String(lcd_mode_to_str(lcd.mode()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "segment_style" => {
                if let Some(lcd) = widget_as::<LCDNumber>(widget) {
                    Ok(CapabilityValue::String(
                        segment_style_to_str(lcd.segment_style()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CommandLink => match property_name {
            "text" => {
                if let Some(command_link) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::String(command_link.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "description" => {
                if let Some(command_link) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::String(command_link.description().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "enabled" => {
                if let Some(command_link) = widget_as::<CommandLink>(widget) {
                    Ok(CapabilityValue::Bool(command_link.is_enabled()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FontComboBox => match property_name {
            "current_font_family" => {
                if let Some(font_combo) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::String(font_combo.current_font().family.clone()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(font_combo) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(font_combo.count() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(font_combo) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(font_combo.current_index() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "editable" => {
                if let Some(font_combo) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Bool(font_combo.is_editable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_visible_items" => {
                if let Some(font_combo) = widget_as::<FontComboBox>(widget) {
                    Ok(CapabilityValue::Int(font_combo.max_visible_items() as i64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Action => match property_name {
            "text" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "icon_text" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.icon_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "shortcut" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::String(action.shortcut().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checkable" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_checkable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "checked" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_checked()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "separator" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(CapabilityValue::Bool(action.is_separator()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "command_id" => {
                if let Some(action) = widget_as::<Action>(widget) {
                    Ok(match action.command_id() {
                        Some(command_id) => CapabilityValue::String(command_id.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolBox => match property_name {
            "item_count" => {
                if let Some(tool_box) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::UInt(tool_box.count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(tool_box) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::UInt(tool_box.current_index() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(tool_box) = widget_as::<ToolBox>(widget) {
                    Ok(CapabilityValue::String(
                        orientation_to_str(tool_box.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TabBar => match property_name {
            "tab_count" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tab_bar.tab_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_index" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(match tab_bar.current_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "closable" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::Bool(tab_bar.closable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "movable" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::Bool(tab_bar.movable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tab_min_width" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tab_bar.tab_min_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "tab_max_width" => {
                if let Some(tab_bar) = widget_as::<TabBar>(widget) {
                    Ok(CapabilityValue::UInt(tab_bar.tab_max_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Calendar => match property_name {
            "selected_date" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(calendar.selected_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_date" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(calendar.minimum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_date" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(naive_date_to_string(calendar.maximum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "first_day_of_week" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(
                        weekday_to_str(calendar.first_day_of_week()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "grid_visible" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(calendar.is_grid_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "navigation_bar_visible" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(calendar.is_navigation_bar_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "horizontal_header_visible" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(calendar.is_horizontal_header_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "vertical_header_visible" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::Bool(calendar.is_vertical_header_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "date_format" => {
                if let Some(calendar) = widget_as::<Calendar>(widget) {
                    Ok(CapabilityValue::String(calendar.date_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DatePicker => match property_name {
            "date" => {
                if let Some(date_edit) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(date_edit.date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_date" => {
                if let Some(date_edit) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(date_edit.minimum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_date" => {
                if let Some(date_edit) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_to_string(date_edit.maximum_date())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "display_format" => {
                if let Some(date_edit) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::String(date_edit.display_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "calendar_popup" => {
                if let Some(date_edit) = widget_as::<DateEdit>(widget) {
                    Ok(CapabilityValue::Bool(date_edit.calendar_popup()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TimePicker => match property_name {
            "time" => {
                if let Some(time_edit) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(time_edit.time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimum_time" => {
                if let Some(time_edit) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(time_edit.minimum_time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "maximum_time" => {
                if let Some(time_edit) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_to_string(time_edit.maximum_time())))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "display_format" => {
                if let Some(time_edit) = widget_as::<TimeEdit>(widget) {
                    Ok(CapabilityValue::String(time_edit.display_format().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LineEdit => match property_name {
            "text" => {
                if let Some(line_edit) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::String(line_edit.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "placeholder_text" => {
                if let Some(line_edit) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::String(line_edit.placeholder_text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "max_length" => {
                if let Some(line_edit) = widget_as::<LineEdit>(widget) {
                    Ok(match line_edit.max_length() {
                        Some(max) => CapabilityValue::UInt(max as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "read_only" => {
                if let Some(line_edit) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::Bool(line_edit.is_read_only()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_position" => {
                if let Some(line_edit) = widget_as::<LineEdit>(widget) {
                    Ok(CapabilityValue::UInt(line_edit.cursor_position() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ListView => match property_name {
            "has_model" => {
                if let Some(list_view) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::Bool(list_view.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(list_view) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::UInt(list_view.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "focused_row" => {
                if let Some(list_view) = widget_as::<ListView>(widget) {
                    Ok(match list_view.focused_row() {
                        Some(row) => CapabilityValue::UInt(row as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selection_mode" => {
                if let Some(list_view) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::String(
                        selection_mode_to_str(list_view.selection_mode()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "view_mode" => {
                if let Some(list_view) = widget_as::<ListView>(widget) {
                    Ok(CapabilityValue::String(view_mode_to_str(list_view.view_mode()).to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TreeView => {
            if let Some(tree_table) = widget_as::<TreeTable>(widget) {
                match property_name {
                    "has_model" => Ok(CapabilityValue::Bool(tree_table.has_model())),
                    "row_count" => Ok(CapabilityValue::UInt(tree_table.row_count() as u64)),
                    "column_count" => Ok(CapabilityValue::UInt(tree_table.column_count() as u64)),
                    "selected_row" => Ok(match tree_table.selected_row() {
                        Some(row) => CapabilityValue::UInt(row as u64),
                        None => CapabilityValue::Null,
                    }),
                    "row_height" => Ok(CapabilityValue::UInt(tree_table.row_height() as u64)),
                    "column_width" => Ok(CapabilityValue::UInt(tree_table.column_width() as u64)),
                    "projection_state" => Ok(CapabilityValue::String(format!(
                        "rows={},selected={:?}",
                        tree_table.row_count(),
                        tree_table.selected_row()
                    ))),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(tree_view) = widget_as::<TreeView>(widget) {
                match property_name {
                    "has_model" => Ok(CapabilityValue::Bool(tree_view.has_model())),
                    "node_count" => Ok(CapabilityValue::UInt(tree_view.node_count() as u64)),
                    "focused_node" => Ok(match tree_view.focused_node() {
                        Some(node) => CapabilityValue::UInt(node as u64),
                        None => CapabilityValue::Null,
                    }),
                    "selected_node" => Ok(match tree_view.selected_node() {
                        Some(node) => CapabilityValue::UInt(node as u64),
                        None => CapabilityValue::Null,
                    }),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Table => {
            if let Some(data_grid) = widget_as::<DataGrid>(widget) {
                match property_name {
                    "has_data_source" => Ok(CapabilityValue::Bool(data_grid.has_data_source())),
                    "row_count" => Ok(CapabilityValue::UInt(data_grid.row_count() as u64)),
                    "column_count" => Ok(CapabilityValue::UInt(data_grid.column_count() as u64)),
                    "scroll_row" => Ok(CapabilityValue::UInt(data_grid.scroll_row() as u64)),
                    "scroll_column" => Ok(CapabilityValue::UInt(data_grid.scroll_column() as u64)),
                    "row_height" => Ok(CapabilityValue::UInt(data_grid.row_height() as u64)),
                    "column_width" => Ok(CapabilityValue::UInt(data_grid.column_width() as u64)),
                    "frozen_columns" => {
                        Ok(CapabilityValue::UInt(data_grid.frozen_columns() as u64))
                    }
                    "sort_spec_count" => {
                        Ok(CapabilityValue::UInt(data_grid.sort_specs().len() as u64))
                    }
                    "filter_count" => Ok(CapabilityValue::UInt(data_grid.filters().len() as u64)),
                    "sort_specs" => {
                        Ok(CapabilityValue::String(sort_specs_to_string(data_grid.sort_specs())))
                    }
                    "filters" => {
                        Ok(CapabilityValue::String(column_filters_to_string(data_grid.filters())))
                    }
                    "visible_window" => {
                        let (row_start, row_len, col_start, col_len) = data_grid.visible_window();
                        Ok(CapabilityValue::String(format!(
                            "{row_start}:{row_len}:{col_start}:{col_len}"
                        )))
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(virtual_table) = widget_as::<VirtualTable>(widget) {
                match property_name {
                    "has_data_source" => Ok(CapabilityValue::Bool(virtual_table.has_data_source())),
                    "row_count" => Ok(CapabilityValue::UInt(virtual_table.row_count() as u64)),
                    "column_count" => {
                        Ok(CapabilityValue::UInt(virtual_table.column_count() as u64))
                    }
                    "scroll_row" => Ok(CapabilityValue::UInt(virtual_table.scroll_row() as u64)),
                    "scroll_column" => {
                        Ok(CapabilityValue::UInt(virtual_table.scroll_column() as u64))
                    }
                    "row_height" => Ok(CapabilityValue::UInt(virtual_table.row_height() as u64)),
                    "column_width" => {
                        Ok(CapabilityValue::UInt(virtual_table.column_width() as u64))
                    }
                    "overscan_rows" => {
                        Ok(CapabilityValue::UInt(virtual_table.overscan_rows() as u64))
                    }
                    "overscan_columns" => {
                        Ok(CapabilityValue::UInt(virtual_table.overscan_columns() as u64))
                    }
                    "visible_window" => {
                        let (row_start, row_len, col_start, col_len) =
                            virtual_table.visible_window();
                        Ok(CapabilityValue::String(format!(
                            "{row_start}:{row_len}:{col_start}:{col_len}"
                        )))
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(table_widget) = widget_as::<TableWidget>(widget) {
                match property_name {
                    "has_model" => Ok(CapabilityValue::Bool(table_widget.has_model())),
                    "has_delegate" => Ok(CapabilityValue::Bool(table_widget.has_delegate())),
                    "row_count" => Ok(CapabilityValue::UInt(table_widget.row_count() as u64)),
                    "column_count" => Ok(CapabilityValue::UInt(table_widget.column_count() as u64)),
                    "selection_mode" => Ok(CapabilityValue::String(
                        selection_mode_to_str(table_widget.selection_mode()).to_string(),
                    )),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Menu => match property_name {
            "title" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    Ok(CapabilityValue::String(menu.title().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "item_count" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    Ok(CapabilityValue::UInt(menu.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "hovered_index" => {
                if let Some(menu) = widget_as::<Menu>(widget) {
                    Ok(match menu.hovered_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::DataView => match property_name {
            "has_data_source" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::Bool(virtual_list.has_data_source()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(virtual_list.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "scroll_row" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(virtual_list.scroll_row() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_height" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(virtual_list.row_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "overscan" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(CapabilityValue::UInt(virtual_list.overscan() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_row" => {
                if let Some(virtual_list) = widget_as::<VirtualList>(widget) {
                    Ok(match virtual_list.selected_row() {
                        Some(row) => CapabilityValue::UInt(row as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::MenuBar => match property_name {
            "entry_count" => {
                if let Some(menu_bar) = widget_as::<MenuBar>(widget) {
                    Ok(CapabilityValue::UInt(menu_bar.entries().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "active_index" => {
                if let Some(menu_bar) = widget_as::<MenuBar>(widget) {
                    Ok(match menu_bar.active_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "hovered_index" => {
                if let Some(menu_bar) = widget_as::<MenuBar>(widget) {
                    Ok(match menu_bar.hovered_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolBar => match property_name {
            "item_count" => {
                if let Some(tool_bar) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::UInt(tool_bar.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "movable" => {
                if let Some(tool_bar) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Bool(tool_bar.is_movable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "floatable" => {
                if let Some(tool_bar) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Bool(tool_bar.is_floatable()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "orientation" => {
                if let Some(tool_bar) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::String(
                        tool_bar_orientation_to_str(tool_bar.orientation()).to_string(),
                    ))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "icon_size" => {
                if let Some(tool_bar) = widget_as::<ToolBar>(widget) {
                    Ok(CapabilityValue::Float(tool_bar.icon_size() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RibbonBar => match property_name {
            "tab_count" => {
                if let Some(ribbon_bar) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::UInt(ribbon_bar.tab_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "current_tab" => {
                if let Some(ribbon_bar) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::UInt(ribbon_bar.current_tab() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "expanded" => {
                if let Some(ribbon_bar) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::Bool(ribbon_bar.is_expanded()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "minimized" => {
                if let Some(ribbon_bar) = widget_as::<RibbonBar>(widget) {
                    Ok(CapabilityValue::Bool(ribbon_bar.is_minimized()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ColorDialog => match property_name {
            "hex_rgba" => {
                if let Some(color_picker) = widget_as::<ColorPicker>(widget) {
                    Ok(CapabilityValue::String(color_picker.hex_rgba()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "show_alpha" => {
                if let Some(color_picker) = widget_as::<ColorPicker>(widget) {
                    Ok(CapabilityValue::Bool(color_picker.show_alpha()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "preset_count" => {
                if let Some(color_picker) = widget_as::<ColorPicker>(widget) {
                    Ok(CapabilityValue::UInt(color_picker.preset_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RichEdit => match property_name {
            "text" => {
                if let Some(code_editor) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::String(code_editor.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "line_count" => {
                if let Some(code_editor) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::UInt(code_editor.line_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_line" => {
                if let Some(code_editor) = widget_as::<CodeEditor>(widget) {
                    let (line, _) = code_editor.cursor();
                    Ok(CapabilityValue::UInt(line as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cursor_column" => {
                if let Some(code_editor) = widget_as::<CodeEditor>(widget) {
                    let (_, column) = code_editor.cursor();
                    Ok(CapabilityValue::UInt(column as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "marker_count" => {
                if let Some(code_editor) = widget_as::<CodeEditor>(widget) {
                    Ok(CapabilityValue::UInt(code_editor.markers().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Chart => match property_name {
            "task_count" => {
                if let Some(gantt_widget) = widget_as::<GanttWidget>(widget) {
                    Ok(CapabilityValue::UInt(gantt_widget.tasks().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_id" => {
                if let Some(gantt_widget) = widget_as::<GanttWidget>(widget) {
                    Ok(match gantt_widget.selected_id() {
                        Some(id) => CapabilityValue::String(id.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "viewport_start" => {
                if let Some(gantt_widget) = widget_as::<GanttWidget>(widget) {
                    let (start, _) = gantt_widget.viewport();
                    Ok(CapabilityValue::Int(start))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "viewport_end" => {
                if let Some(gantt_widget) = widget_as::<GanttWidget>(widget) {
                    let (_, end) = gantt_widget.viewport();
                    Ok(CapabilityValue::Int(end))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TextEdit => match property_name {
            "output_line_count" => {
                if let Some(terminal_view) = widget_as::<TerminalView>(widget) {
                    Ok(CapabilityValue::UInt(terminal_view.lines().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "input_line" => {
                if let Some(terminal_view) = widget_as::<TerminalView>(widget) {
                    Ok(CapabilityValue::String(terminal_view.input_line().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::StatusBar => match property_name {
            "message" => {
                if let Some(snackbar) = widget_as::<Snackbar>(widget) {
                    Ok(CapabilityValue::String(snackbar.message().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "visible" => {
                if let Some(snackbar) = widget_as::<Snackbar>(widget) {
                    Ok(CapabilityValue::Bool(snackbar.is_visible()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "action_label" => {
                if let Some(snackbar) = widget_as::<Snackbar>(widget) {
                    Ok(match snackbar.action_label() {
                        Some(label) => CapabilityValue::String(label.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Canvas => match property_name {
            "center_x" => {
                if let Some(map_view) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(map_view.center().0 as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "center_y" => {
                if let Some(map_view) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(map_view.center().1 as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "zoom" => {
                if let Some(map_view) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::Float(map_view.zoom() as f64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "marker_count" => {
                if let Some(map_view) = widget_as::<MapView>(widget) {
                    Ok(CapabilityValue::UInt(map_view.markers().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_marker_id" => {
                if let Some(map_view) = widget_as::<MapView>(widget) {
                    Ok(match map_view.selected_marker_id() {
                        Some(id) => CapabilityValue::String(id.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::WebView => match property_name {
            "source" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(match media_player.source() {
                        Some(source) => CapabilityValue::String(source.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "playing" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::Bool(media_player.is_playing()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "duration_ms" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::UInt(media_player.duration_ms()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "position_ms" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::UInt(media_player.position_ms()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "volume" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::UInt(media_player.volume() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "muted" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::Bool(media_player.muted()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "fullscreen" => {
                if let Some(media_player) = widget_as::<MediaPlayer>(widget) {
                    Ok(CapabilityValue::Bool(media_player.fullscreen()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Panel => match property_name {
            "segment_count" => {
                if let Some(breadcrumb) = widget_as::<Breadcrumb>(widget) {
                    Ok(CapabilityValue::UInt(breadcrumb.segments().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_index" => {
                if let Some(breadcrumb) = widget_as::<Breadcrumb>(widget) {
                    Ok(match breadcrumb.selected_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToolButton => match property_name {
            "text" => {
                if let Some(split_button) = widget_as::<SplitButton>(widget) {
                    Ok(CapabilityValue::String(split_button.text().to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "action_count" => {
                if let Some(split_button) = widget_as::<SplitButton>(widget) {
                    Ok(CapabilityValue::UInt(split_button.actions().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "menu_open" => {
                if let Some(split_button) = widget_as::<SplitButton>(widget) {
                    Ok(CapabilityValue::Bool(split_button.menu_open()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_height" => {
                if let Some(split_button) = widget_as::<SplitButton>(widget) {
                    Ok(CapabilityValue::UInt(split_button.row_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::ToggleButton => match property_name {
            "item_count" => {
                if let Some(segmented_control) = widget_as::<SegmentedControl>(widget) {
                    Ok(CapabilityValue::UInt(segmented_control.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_index" => {
                if let Some(segmented_control) = widget_as::<SegmentedControl>(widget) {
                    Ok(match segmented_control.selected_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_id" => {
                if let Some(segmented_control) = widget_as::<SegmentedControl>(widget) {
                    Ok(match segmented_control.selected_id() {
                        Some(id) => CapabilityValue::String(id.to_string()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CheckListBox => match property_name {
            "item_count" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(CapabilityValue::UInt(chip.items().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "multi_select" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(CapabilityValue::Bool(chip.multi_select()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "focused_index" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(match chip.focused_index() {
                        Some(index) => CapabilityValue::UInt(index as u64),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "selected_count" => {
                if let Some(chip) = widget_as::<Chip>(widget) {
                    Ok(CapabilityValue::UInt(chip.selected_ids().len() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Grid => match property_name {
            "rows" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.rows() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "columns" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.columns() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "spacing" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.spacing() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "line_color" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(match grid.line_color() {
                        Some(color) => CapabilityValue::String(color.to_hex_rgba()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cell_width" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.cell_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "cell_height" => {
                if let Some(grid) = widget_as::<GridWidget>(widget) {
                    Ok(CapabilityValue::UInt(grid.cell_height() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::FreeformShape => match property_name {
            "path_kind" => {
                if let Some(shape) = widget_as::<FreeformShapeWidget>(widget) {
                    let token = match shape.path() {
                        ShapePath::Heart => "heart",
                        ShapePath::Star { .. } => "star",
                        ShapePath::Polygon(_) => "polygon",
                        ShapePath::RoundedRect { .. } => "rounded_rect",
                        ShapePath::Bubble { .. } => "bubble",
                        ShapePath::Custom(_) => "custom",
                    };
                    Ok(CapabilityValue::String(token.to_string()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "fill_rgba" => {
                if let Some(shape) = widget_as::<FreeformShapeWidget>(widget) {
                    Ok(CapabilityValue::String(shape.fill_color().to_hex_rgba()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "stroke_rgba" => {
                if let Some(shape) = widget_as::<FreeformShapeWidget>(widget) {
                    Ok(match shape.stroke_color() {
                        Some(color) => CapabilityValue::String(color.to_hex_rgba()),
                        None => CapabilityValue::Null,
                    })
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "stroke_width" => {
                if let Some(shape) = widget_as::<FreeformShapeWidget>(widget) {
                    Ok(CapabilityValue::UInt(shape.stroke_width() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}

fn write_widget_property_value(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => {
            if let Some(button) = widget_as_mut::<Button>(widget) {
                match property_name {
                    "text" => {
                        button.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "pressed" => {
                        button.set_pressed(expect_bool(value)?);
                        Ok(())
                    }
                    "default" => {
                        button.set_default(expect_bool(value)?);
                        Ok(())
                    }
                    "enabled" => {
                        button.set_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    "tooltip" => {
                        button.set_tooltip(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Label => {
            if let Some(label) = widget_as_mut::<Label>(widget) {
                match property_name {
                    "text" => {
                        label.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "alignment" => {
                        label.set_alignment(expect_alignment(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CheckBox => {
            if let Some(check_box) = widget_as_mut::<CheckBox>(widget) {
                match property_name {
                    "text" => {
                        check_box.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "state" => {
                        check_box.set_state(expect_check_state(value)?);
                        Ok(())
                    }
                    "checked" => {
                        check_box.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "tristate_enabled" => {
                        check_box.set_tristate_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RadioButton => {
            if let Some(radio_button) = widget_as_mut::<RadioButton>(widget) {
                match property_name {
                    "text" => {
                        radio_button.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "checked" => {
                        radio_button.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "group_id" => {
                        match value {
                            CapabilityValue::Null => radio_button.set_group_id(None),
                            other => radio_button.set_group_id(Some(expect_string(other)?)),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Slider => {
            if let Some(slider) = widget_as_mut::<Slider>(widget) {
                match property_name {
                    "minimum" => {
                        slider.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        slider.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        slider.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        slider.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        slider.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "orientation" => {
                        slider.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    "tick_position" => {
                        slider.set_tick_position(expect_tick_position(value)?);
                        Ok(())
                    }
                    "tick_interval" => {
                        slider.set_tick_interval(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "tracking" => {
                        slider.set_tracking(expect_bool(value)?);
                        Ok(())
                    }
                    "slider_position" => {
                        slider.set_slider_position(expect_i64(value)? as i32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ProgressBar => {
            if let Some(progress_bar) = widget_as_mut::<ProgressBar>(widget) {
                match property_name {
                    "minimum" => {
                        progress_bar.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        progress_bar.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        progress_bar.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "text_visible" => {
                        progress_bar.set_text_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        progress_bar.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    "inverted_appearance" => {
                        progress_bar.set_inverted_appearance(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ScrollBar => {
            if let Some(scroll_bar) = widget_as_mut::<ScrollBar>(widget) {
                match property_name {
                    "minimum" => {
                        scroll_bar.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        scroll_bar.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        scroll_bar.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        scroll_bar.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        scroll_bar.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "orientation" => {
                        scroll_bar.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ListBox => {
            if let Some(list_box) = widget_as_mut::<ListBox>(widget) {
                match property_name {
                    "selection_mode" => {
                        list_box.set_selection_mode(expect_list_box_selection_mode(value)?);
                        Ok(())
                    }
                    "current_row" => {
                        match value {
                            CapabilityValue::Null => list_box.set_current_row(None),
                            other => list_box.set_current_row(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "item_height" => {
                        list_box.set_item_height(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::SpinBox => {
            if let Some(spin_box) = widget_as_mut::<SpinBox>(widget) {
                match property_name {
                    "minimum" => {
                        spin_box.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        spin_box.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        spin_box.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        spin_box.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "prefix" => {
                        spin_box.set_prefix(expect_string(value)?);
                        Ok(())
                    }
                    "suffix" => {
                        spin_box.set_suffix(expect_string(value)?);
                        Ok(())
                    }
                    "special_value_text" => {
                        match value {
                            CapabilityValue::Null => spin_box.set_special_value_text(None),
                            other => {
                                spin_box.set_special_value_text(Some(expect_string(other)?));
                            }
                        }
                        Ok(())
                    }
                    "wrapping" => {
                        spin_box.set_wrapping(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ComboBox => {
            if let Some(combo_box) = widget_as_mut::<ComboBox>(widget) {
                match property_name {
                    "current_index" => {
                        match value {
                            CapabilityValue::Null => combo_box.set_current_index(None),
                            other => combo_box.set_current_index(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "current_text" => {
                        combo_box.set_current_text(expect_string(value)?);
                        Ok(())
                    }
                    "editable" => {
                        combo_box.set_editable(expect_bool(value)?);
                        Ok(())
                    }
                    "max_visible_items" => {
                        combo_box.set_max_visible_items(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Dial => {
            if let Some(dial) = widget_as_mut::<Dial>(widget) {
                match property_name {
                    "minimum" => {
                        dial.set_minimum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "maximum" => {
                        dial.set_maximum(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "value" => {
                        dial.set_value(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "single_step" => {
                        dial.set_single_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "page_step" => {
                        dial.set_page_step(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "notches_visible" => {
                        dial.set_notches_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "notch_target" => {
                        dial.set_notch_target(expect_f64(value)?);
                        Ok(())
                    }
                    "wrapping" => {
                        dial.set_wrapping(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Window => {
            if let Some(window) = widget_as_mut::<Window>(widget) {
                match property_name {
                    "title" => {
                        window.set_title(expect_string(value)?);
                        Ok(())
                    }
                    "title_bar_height" => {
                        window.set_title_bar_height(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "close_button_size" => {
                        window.set_close_button_size(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "button_spacing" => {
                        window.set_button_spacing(expect_usize(value)? as u32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::GroupBox => {
            if let Some(group_box) = widget_as_mut::<GroupBox>(widget) {
                match property_name {
                    "title" => {
                        group_box.set_title(expect_string(value)?);
                        Ok(())
                    }
                    "alignment" => {
                        group_box.set_alignment(expect_alignment(value)?);
                        Ok(())
                    }
                    "checkable" => {
                        group_box.set_checkable(expect_bool(value)?);
                        Ok(())
                    }
                    "checked" => {
                        group_box.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Splitter => {
            if let Some(splitter) = widget_as_mut::<Splitter>(widget) {
                match property_name {
                    "orientation" => {
                        splitter.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::LCDNumber => {
            if let Some(lcd) = widget_as_mut::<LCDNumber>(widget) {
                match property_name {
                    "value" => {
                        lcd.set_value(expect_f64(value)?);
                        Ok(())
                    }
                    "min_value" => {
                        lcd.set_min_value(expect_f64(value)?);
                        Ok(())
                    }
                    "max_value" => {
                        lcd.set_max_value(expect_f64(value)?);
                        Ok(())
                    }
                    "num_digits" => {
                        lcd.set_num_digits(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "small_decimal_point" => {
                        lcd.set_small_decimal_point(expect_bool(value)?);
                        Ok(())
                    }
                    "mode" => {
                        lcd.set_mode(expect_lcd_mode(value)?);
                        Ok(())
                    }
                    "segment_style" => {
                        lcd.set_segment_style(expect_segment_style(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CommandLink => {
            if let Some(command_link) = widget_as_mut::<CommandLink>(widget) {
                match property_name {
                    "text" => {
                        command_link.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "description" => {
                        command_link.set_description(expect_string(value)?);
                        Ok(())
                    }
                    "enabled" => {
                        command_link.set_enabled(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::FontComboBox => {
            if let Some(font_combo) = widget_as_mut::<FontComboBox>(widget) {
                match property_name {
                    "current_index" => {
                        font_combo.set_current_index(expect_i64(value)? as i32);
                        Ok(())
                    }
                    "editable" => {
                        font_combo.set_editable(expect_bool(value)?);
                        Ok(())
                    }
                    "max_visible_items" => {
                        font_combo.set_max_visible_items(expect_i64(value)? as i32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Action => {
            if let Some(action) = widget_as_mut::<Action>(widget) {
                match property_name {
                    "text" => {
                        action.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "icon_text" => {
                        action.set_icon_text(expect_string(value)?);
                        Ok(())
                    }
                    "shortcut" => {
                        action.set_shortcut(expect_string(value)?);
                        Ok(())
                    }
                    "checkable" => {
                        action.set_checkable(expect_bool(value)?);
                        Ok(())
                    }
                    "checked" => {
                        action.set_checked(expect_bool(value)?);
                        Ok(())
                    }
                    "command_id" => {
                        match value {
                            CapabilityValue::Null => action.clear_command_id(),
                            other => action.set_command_id(expect_string(other)?),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ToolBox => {
            if let Some(tool_box) = widget_as_mut::<ToolBox>(widget) {
                match property_name {
                    "current_index" => {
                        tool_box.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        tool_box.set_orientation(expect_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TabBar => {
            if let Some(tab_bar) = widget_as_mut::<TabBar>(widget) {
                match property_name {
                    "current_index" => {
                        tab_bar.set_current_index(expect_usize(value)?);
                        Ok(())
                    }
                    "closable" => {
                        tab_bar.set_closable(expect_bool(value)?);
                        Ok(())
                    }
                    "movable" => {
                        tab_bar.set_movable(expect_bool(value)?);
                        Ok(())
                    }
                    "tab_min_width" => {
                        tab_bar.set_tab_min_width(expect_usize(value)? as u32);
                        Ok(())
                    }
                    "tab_max_width" => {
                        tab_bar.set_tab_max_width(expect_usize(value)? as u32);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Calendar => {
            if let Some(calendar) = widget_as_mut::<Calendar>(widget) {
                match property_name {
                    "selected_date" => {
                        calendar.set_selected_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "minimum_date" => {
                        calendar.set_minimum_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "maximum_date" => {
                        calendar.set_maximum_date(expect_naive_date(value)?);
                        Ok(())
                    }
                    "first_day_of_week" => {
                        calendar.set_first_day_of_week(expect_weekday(value)?);
                        Ok(())
                    }
                    "grid_visible" => {
                        calendar.set_grid_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "navigation_bar_visible" => {
                        calendar.set_navigation_bar_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "horizontal_header_visible" => {
                        calendar.set_horizontal_header_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "vertical_header_visible" => {
                        calendar.set_vertical_header_visible(expect_bool(value)?);
                        Ok(())
                    }
                    "date_format" => {
                        calendar.set_date_format(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::DatePicker => {
            if let Some(date_edit) = widget_as_mut::<DateEdit>(widget) {
                match property_name {
                    "date" => {
                        date_edit.set_date(expect_date(value)?);
                        Ok(())
                    }
                    "minimum_date" => {
                        date_edit.set_minimum_date(expect_date(value)?);
                        Ok(())
                    }
                    "maximum_date" => {
                        date_edit.set_maximum_date(expect_date(value)?);
                        Ok(())
                    }
                    "display_format" => {
                        date_edit.set_display_format(expect_string(value)?);
                        Ok(())
                    }
                    "calendar_popup" => {
                        date_edit.set_calendar_popup(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TimePicker => {
            if let Some(time_edit) = widget_as_mut::<TimeEdit>(widget) {
                match property_name {
                    "time" => {
                        time_edit.set_time(expect_time(value)?);
                        Ok(())
                    }
                    "minimum_time" => {
                        time_edit.set_minimum_time(expect_time(value)?);
                        Ok(())
                    }
                    "maximum_time" => {
                        time_edit.set_maximum_time(expect_time(value)?);
                        Ok(())
                    }
                    "display_format" => {
                        time_edit.set_display_format(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::LineEdit => {
            if let Some(line_edit) = widget_as_mut::<LineEdit>(widget) {
                match property_name {
                    "text" => {
                        line_edit.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "placeholder_text" => {
                        line_edit.set_placeholder_text(expect_string(value)?);
                        Ok(())
                    }
                    "max_length" => {
                        match value {
                            CapabilityValue::Null => line_edit.set_max_length(None),
                            other => line_edit.set_max_length(Some(expect_usize(other)?)),
                        }
                        Ok(())
                    }
                    "read_only" => {
                        line_edit.set_read_only(expect_bool(value)?);
                        Ok(())
                    }
                    "cursor_position" => {
                        line_edit.set_cursor_position(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ListView => {
            if let Some(list_view) = widget_as_mut::<ListView>(widget) {
                match property_name {
                    "focused_row" => match value {
                        CapabilityValue::Null => {
                            list_view.clear_focused_row();
                            Ok(())
                        }
                        other => {
                            let row = expect_usize(other)?;
                            if list_view.set_focused_row(row) {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    "selection_mode" => {
                        list_view.set_selection_mode(expect_selection_mode(value)?);
                        Ok(())
                    }
                    "view_mode" => {
                        list_view.set_view_mode(expect_view_mode(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TreeView => {
            if let Some(tree_table) = widget_as_mut::<TreeTable>(widget) {
                match property_name {
                    "selected_row" => match value {
                        CapabilityValue::Null => {
                            if let Some(selected) = tree_table.selected_row() {
                                let _ = tree_table.select_row(selected);
                            }
                            Ok(())
                        }
                        other => {
                            let row = expect_usize(other)?;
                            if tree_table.select_row(row) || tree_table.row_count() == 0 {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    "row_height" => {
                        tree_table.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        tree_table.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(tree_view) = widget_as_mut::<TreeView>(widget) {
                match property_name {
                    "focused_node" => match value {
                        CapabilityValue::Null => {
                            tree_view.clear_focused_node();
                            Ok(())
                        }
                        other => {
                            let node = expect_usize(other)?;
                            if tree_view.set_focused_node(node) {
                                Ok(())
                            } else {
                                Err(CapabilityAccessError::UnsupportedOnWidget)
                            }
                        }
                    },
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Menu => {
            if let Some(menu) = widget_as_mut::<Menu>(widget) {
                match property_name {
                    "title" => {
                        menu.set_title(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::DataView => {
            if let Some(virtual_list) = widget_as_mut::<VirtualList>(widget) {
                match property_name {
                    "scroll_row" => {
                        virtual_list.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        let row_height = expect_u32(value)?;
                        virtual_list.set_row_height(row_height);
                        Ok(())
                    }
                    "overscan" => {
                        virtual_list.set_overscan(expect_usize(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Table => {
            if let Some(data_grid) = widget_as_mut::<DataGrid>(widget) {
                match property_name {
                    "scroll_row" => {
                        data_grid.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "scroll_column" => {
                        data_grid.set_scroll_column(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        data_grid.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        data_grid.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    "frozen_columns" => {
                        data_grid.set_frozen_columns(expect_usize(value)?);
                        Ok(())
                    }
                    "sort_specs" => {
                        data_grid.set_sort_specs(expect_sort_specs(value)?);
                        Ok(())
                    }
                    "filters" => {
                        data_grid.set_filters(expect_column_filters(value)?);
                        Ok(())
                    }
                    "sort_spec_count" | "filter_count" | "visible_window" => {
                        Err(CapabilityAccessError::ReadOnlyProperty)
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(virtual_table) = widget_as_mut::<VirtualTable>(widget) {
                match property_name {
                    "scroll_row" => {
                        virtual_table.set_scroll_row(expect_usize(value)?);
                        Ok(())
                    }
                    "scroll_column" => {
                        virtual_table.set_scroll_column(expect_usize(value)?);
                        Ok(())
                    }
                    "row_height" => {
                        virtual_table.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    "column_width" => {
                        virtual_table.set_column_width(expect_u32(value)?);
                        Ok(())
                    }
                    "overscan_rows" => {
                        virtual_table.set_overscan_rows(expect_usize(value)?);
                        Ok(())
                    }
                    "overscan_columns" => {
                        virtual_table.set_overscan_columns(expect_usize(value)?);
                        Ok(())
                    }
                    "visible_window" => Err(CapabilityAccessError::ReadOnlyProperty),
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else if let Some(table_widget) = widget_as_mut::<TableWidget>(widget) {
                match property_name {
                    "selection_mode" => {
                        table_widget.set_selection_mode(expect_selection_mode(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ToolBar => {
            if let Some(tool_bar) = widget_as_mut::<ToolBar>(widget) {
                match property_name {
                    "movable" => {
                        tool_bar.set_movable(expect_bool(value)?);
                        Ok(())
                    }
                    "floatable" => {
                        tool_bar.set_floatable(expect_bool(value)?);
                        Ok(())
                    }
                    "icon_size" => {
                        tool_bar.set_icon_size(expect_f32(value)?);
                        Ok(())
                    }
                    "orientation" => {
                        tool_bar.set_orientation(expect_toolbar_orientation(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RibbonBar => {
            if let Some(ribbon_bar) = widget_as_mut::<RibbonBar>(widget) {
                match property_name {
                    "current_tab" => {
                        ribbon_bar.set_current_tab(expect_usize(value)?);
                        Ok(())
                    }
                    "expanded" => {
                        ribbon_bar.set_expanded(expect_bool(value)?);
                        Ok(())
                    }
                    "minimized" => {
                        ribbon_bar.set_minimized(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ColorDialog => {
            if let Some(color_picker) = widget_as_mut::<ColorPicker>(widget) {
                match property_name {
                    "hex_rgba" => {
                        let hex = expect_string(value)?;
                        if color_picker.set_hex(&hex) {
                            Ok(())
                        } else {
                            Err(CapabilityAccessError::TypeMismatch)
                        }
                    }
                    "show_alpha" => {
                        color_picker.set_show_alpha(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::RichEdit => {
            if let Some(code_editor) = widget_as_mut::<CodeEditor>(widget) {
                match property_name {
                    "text" => {
                        code_editor.set_text(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Chart => {
            if let Some(gantt_widget) = widget_as_mut::<GanttWidget>(widget) {
                match property_name {
                    "viewport_start" => {
                        let start = expect_i64(value)?;
                        let (_, end) = gantt_widget.viewport();
                        gantt_widget.set_viewport(start, end);
                        Ok(())
                    }
                    "viewport_end" => {
                        let end = expect_i64(value)?;
                        let (start, _) = gantt_widget.viewport();
                        gantt_widget.set_viewport(start, end);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TextEdit => {
            if let Some(terminal_view) = widget_as_mut::<TerminalView>(widget) {
                match property_name {
                    "input_line" => {
                        terminal_view.set_input_line(expect_string(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::StatusBar => {
            if let Some(snackbar) = widget_as_mut::<Snackbar>(widget) {
                match property_name {
                    "message" => {
                        let message = expect_string(value)?;
                        snackbar.show(message);
                        Ok(())
                    }
                    "visible" => {
                        if expect_bool(value)? {
                            snackbar.show(snackbar.message().to_string());
                        } else {
                            snackbar.dismiss();
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Canvas => {
            if let Some(map_view) = widget_as_mut::<MapView>(widget) {
                match property_name {
                    "center_x" => {
                        let x = expect_f32(value)?;
                        let (_, y) = map_view.center();
                        map_view.set_center(x, y);
                        Ok(())
                    }
                    "center_y" => {
                        let y = expect_f32(value)?;
                        let (x, _) = map_view.center();
                        map_view.set_center(x, y);
                        Ok(())
                    }
                    "zoom" => {
                        map_view.set_zoom(expect_f32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::WebView => {
            if let Some(media_player) = widget_as_mut::<MediaPlayer>(widget) {
                match property_name {
                    "source" => match value {
                        CapabilityValue::Null => {
                            media_player.clear_source();
                            Ok(())
                        }
                        other => {
                            let source = expect_string(other)?;
                            let duration = media_player.duration_ms();
                            media_player.set_source(source, duration);
                            Ok(())
                        }
                    },
                    "playing" => {
                        if expect_bool(value)? {
                            let _ = media_player.play();
                        } else {
                            media_player.pause();
                        }
                        Ok(())
                    }
                    "position_ms" => {
                        media_player.seek_to(expect_usize(value)? as u64);
                        Ok(())
                    }
                    "volume" => {
                        media_player.set_volume(expect_u32(value)? as u8);
                        Ok(())
                    }
                    "muted" => {
                        media_player.set_muted(expect_bool(value)?);
                        Ok(())
                    }
                    "fullscreen" => {
                        media_player.set_fullscreen(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::ToolButton => {
            if let Some(split_button) = widget_as_mut::<SplitButton>(widget) {
                match property_name {
                    "text" => {
                        split_button.set_text(expect_string(value)?);
                        Ok(())
                    }
                    "menu_open" => {
                        if expect_bool(value)? {
                            split_button.open_menu();
                        } else {
                            split_button.close_menu();
                        }
                        Ok(())
                    }
                    "row_height" => {
                        split_button.set_row_height(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::CheckListBox => {
            if let Some(chip) = widget_as_mut::<Chip>(widget) {
                match property_name {
                    "multi_select" => {
                        chip.set_multi_select(expect_bool(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::Grid => {
            if let Some(grid) = widget_as_mut::<GridWidget>(widget) {
                match property_name {
                    "rows" => {
                        grid.set_rows(expect_u32(value)?);
                        Ok(())
                    }
                    "columns" => {
                        grid.set_columns(expect_u32(value)?);
                        Ok(())
                    }
                    "spacing" => {
                        grid.set_spacing(expect_u32(value)?);
                        Ok(())
                    }
                    "line_color" => {
                        match value {
                            CapabilityValue::Null => grid.set_line_color(None),
                            CapabilityValue::String(raw) => {
                                let Some(color) = crate::core::Color::parse_hex(&raw) else {
                                    return Err(CapabilityAccessError::TypeMismatch);
                                };
                                grid.set_line_color(Some(color));
                            }
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        }
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::FreeformShape => {
            if let Some(shape) = widget_as_mut::<FreeformShapeWidget>(widget) {
                match property_name {
                    "fill_rgba" => {
                        let raw = expect_string(value)?;
                        let Some(color) = crate::core::Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        shape.set_fill_color(color);
                        Ok(())
                    }
                    "stroke_rgba" => {
                        match value {
                            CapabilityValue::Null => shape.set_stroke_color(None),
                            CapabilityValue::String(raw) => {
                                let Some(color) = crate::core::Color::parse_hex(&raw) else {
                                    return Err(CapabilityAccessError::TypeMismatch);
                                };
                                shape.set_stroke_color(Some(color));
                            }
                            _ => return Err(CapabilityAccessError::TypeMismatch),
                        }
                        Ok(())
                    }
                    "stroke_width" => {
                        shape.set_stroke_width(expect_u32(value)?);
                        Ok(())
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}

fn widget_as<T: Widget + 'static>(widget: &dyn Widget) -> Option<&T> {
    (widget as &dyn std::any::Any).downcast_ref::<T>()
}

fn widget_as_mut<T: Widget + 'static>(widget: &mut dyn Widget) -> Option<&mut T> {
    (widget as &mut dyn std::any::Any).downcast_mut::<T>()
}

fn expect_bool(value: CapabilityValue) -> Result<bool, CapabilityAccessError> {
    match value {
        CapabilityValue::Bool(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_string(value: CapabilityValue) -> Result<String, CapabilityAccessError> {
    match value {
        CapabilityValue::String(v) => Ok(v),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_usize(value: CapabilityValue) -> Result<usize, CapabilityAccessError> {
    match value {
        CapabilityValue::UInt(v) => {
            usize::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        CapabilityValue::Int(v) if v >= 0 => {
            usize::try_from(v as u64).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_f32(value: CapabilityValue) -> Result<f32, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v as f32),
        CapabilityValue::UInt(v) => Ok(v as f32),
        CapabilityValue::Int(v) => Ok(v as f32),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_f64(value: CapabilityValue) -> Result<f64, CapabilityAccessError> {
    match value {
        CapabilityValue::Float(v) => Ok(v),
        CapabilityValue::Int(v) => Ok(v as f64),
        CapabilityValue::UInt(v) => Ok(v as f64),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_i64(value: CapabilityValue) -> Result<i64, CapabilityAccessError> {
    match value {
        CapabilityValue::Int(v) => Ok(v),
        CapabilityValue::UInt(v) => {
            i64::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch)
        }
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_u32(value: CapabilityValue) -> Result<u32, CapabilityAccessError> {
    let raw = expect_usize(value)?;
    u32::try_from(raw).map_err(|_| CapabilityAccessError::TypeMismatch)
}

fn expect_naive_date(value: CapabilityValue) -> Result<chrono::NaiveDate, CapabilityAccessError> {
    let text = expect_string(value)?;
    chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d")
        .map_err(|_| CapabilityAccessError::TypeMismatch)
}

fn expect_date(value: CapabilityValue) -> Result<Date, CapabilityAccessError> {
    let text = expect_string(value)?;
    let mut parts = text.split('-');
    let year = parts
        .next()
        .and_then(|v| v.parse::<i32>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let month = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let day = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    if parts.next().is_some() {
        return Err(CapabilityAccessError::TypeMismatch);
    }
    let date = Date::new(year, month, day);
    if date.is_valid() {
        Ok(date)
    } else {
        Err(CapabilityAccessError::TypeMismatch)
    }
}

fn expect_time(value: CapabilityValue) -> Result<Time, CapabilityAccessError> {
    let text = expect_string(value)?;
    let mut parts = text.split(':');
    let hour = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let minute = parts
        .next()
        .and_then(|v| v.parse::<u8>().ok())
        .ok_or(CapabilityAccessError::TypeMismatch)?;
    let second_part = parts.next().ok_or(CapabilityAccessError::TypeMismatch)?;
    if parts.next().is_some() {
        return Err(CapabilityAccessError::TypeMismatch);
    }

    let (second, msec) = if let Some((sec, frac)) = second_part.split_once('.') {
        let second = sec.parse::<u8>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        let frac_trimmed = frac.chars().take(3).collect::<String>();
        let scale = 10u16.pow((3usize.saturating_sub(frac_trimmed.len())) as u32);
        let raw = frac_trimmed.parse::<u16>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        (second, raw * scale)
    } else {
        let second = second_part.parse::<u8>().map_err(|_| CapabilityAccessError::TypeMismatch)?;
        (second, 0)
    };

    let time = Time::new(hour, minute, second, msec);
    if time.is_valid() {
        Ok(time)
    } else {
        Err(CapabilityAccessError::TypeMismatch)
    }
}

fn expect_weekday(value: CapabilityValue) -> Result<chrono::Weekday, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    match token.as_str() {
        "mon" | "monday" => Ok(chrono::Weekday::Mon),
        "tue" | "tues" | "tuesday" => Ok(chrono::Weekday::Tue),
        "wed" | "wednesday" => Ok(chrono::Weekday::Wed),
        "thu" | "thur" | "thurs" | "thursday" => Ok(chrono::Weekday::Thu),
        "fri" | "friday" => Ok(chrono::Weekday::Fri),
        "sat" | "saturday" => Ok(chrono::Weekday::Sat),
        "sun" | "sunday" => Ok(chrono::Weekday::Sun),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn sort_specs_to_string(sort_specs: &[SortSpec]) -> String {
    sort_specs
        .iter()
        .map(|spec| format!("{}:{}", spec.column, if spec.descending { "desc" } else { "asc" }))
        .collect::<Vec<_>>()
        .join(",")
}

fn expect_sort_specs(value: CapabilityValue) -> Result<Vec<SortSpec>, CapabilityAccessError> {
    let text = expect_string(value)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut specs = Vec::new();
    for token in text.split(',') {
        let mut parts = token.split(':');
        let column = parts
            .next()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let order = parts
            .next()
            .map(|v| normalize_key(v.trim()))
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        if parts.next().is_some() {
            return Err(CapabilityAccessError::TypeMismatch);
        }

        let descending = match order.as_str() {
            "asc" => false,
            "desc" => true,
            _ => return Err(CapabilityAccessError::TypeMismatch),
        };
        specs.push(SortSpec { column, descending });
    }
    Ok(specs)
}

fn column_filters_to_string(filters: &[ColumnFilter]) -> String {
    filters
        .iter()
        .map(|filter| format!("{}={}", filter.column, filter.query))
        .collect::<Vec<_>>()
        .join(",")
}

fn expect_column_filters(
    value: CapabilityValue,
) -> Result<Vec<ColumnFilter>, CapabilityAccessError> {
    let text = expect_string(value)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut filters = Vec::new();
    for token in text.split(',') {
        let mut parts = token.splitn(2, '=');
        let column = parts
            .next()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let query =
            parts.next().map(|v| v.to_string()).ok_or(CapabilityAccessError::TypeMismatch)?;
        filters.push(ColumnFilter { column, query });
    }
    Ok(filters)
}

fn expect_selection_mode(
    value: CapabilityValue,
) -> Result<crate::widget::view_widgets::list_view::SelectionMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "single" => Ok(crate::widget::view_widgets::list_view::SelectionMode::Single),
        "multi" => Ok(crate::widget::view_widgets::list_view::SelectionMode::Multi),
        "extended" => Ok(crate::widget::view_widgets::list_view::SelectionMode::Extended),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_list_box_selection_mode(
    value: CapabilityValue,
) -> Result<ListBoxSelectionMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "noselection" => Ok(ListBoxSelectionMode::NoSelection),
        "single" | "singleselection" => Ok(ListBoxSelectionMode::SingleSelection),
        "multi" | "multiselection" => Ok(ListBoxSelectionMode::MultiSelection),
        "extended" | "extendedselection" => Ok(ListBoxSelectionMode::ExtendedSelection),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_view_mode(
    value: CapabilityValue,
) -> Result<crate::widget::view_widgets::list_view::ViewMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "list" => Ok(crate::widget::view_widgets::list_view::ViewMode::List),
        "icon" => Ok(crate::widget::view_widgets::list_view::ViewMode::Icon),
        "details" => Ok(crate::widget::view_widgets::list_view::ViewMode::Details),
        "thumbnails" => Ok(crate::widget::view_widgets::list_view::ViewMode::Thumbnails),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_toolbar_orientation(
    value: CapabilityValue,
) -> Result<crate::widget::menu_toolbar::tool_bar::ToolBarOrientation, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "horizontal" => Ok(crate::widget::menu_toolbar::tool_bar::ToolBarOrientation::Horizontal),
        "vertical" => Ok(crate::widget::menu_toolbar::tool_bar::ToolBarOrientation::Vertical),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_alignment(
    value: CapabilityValue,
) -> Result<crate::core::Alignment, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "left" => Ok(crate::core::Alignment::Left),
        "center" | "centre" => Ok(crate::core::Alignment::Center),
        "right" => Ok(crate::core::Alignment::Right),
        "top" => Ok(crate::core::Alignment::Top),
        "bottom" => Ok(crate::core::Alignment::Bottom),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_check_state(value: CapabilityValue) -> Result<CheckState, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "unchecked" | "off" => Ok(CheckState::Unchecked),
        "partiallychecked" | "partial" | "indeterminate" => Ok(CheckState::PartiallyChecked),
        "checked" | "on" => Ok(CheckState::Checked),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_orientation(
    value: CapabilityValue,
) -> Result<crate::core::Orientation, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "horizontal" => Ok(crate::core::Orientation::Horizontal),
        "vertical" => Ok(crate::core::Orientation::Vertical),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_tick_position(value: CapabilityValue) -> Result<TickPosition, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "none" | "noticks" => Ok(TickPosition::NoTicks),
        "above" | "ticksabove" | "left" => Ok(TickPosition::TicksAbove),
        "below" | "ticksbelow" | "right" => Ok(TickPosition::TicksBelow),
        "both" | "ticksbothsides" => Ok(TickPosition::TicksBothSides),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_lcd_mode(value: CapabilityValue) -> Result<LCDNumberMode, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "hex" => Ok(LCDNumberMode::Hex),
        "dec" | "decimal" => Ok(LCDNumberMode::Dec),
        "oct" | "octal" => Ok(LCDNumberMode::Oct),
        "bin" | "binary" => Ok(LCDNumberMode::Bin),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn expect_segment_style(value: CapabilityValue) -> Result<SegmentStyle, CapabilityAccessError> {
    let token = match value {
        CapabilityValue::String(v) => normalize_key(&v),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };

    match token.as_str() {
        "outline" => Ok(SegmentStyle::Outline),
        "filled" => Ok(SegmentStyle::Filled),
        "flat" => Ok(SegmentStyle::Flat),
        _ => Err(CapabilityAccessError::TypeMismatch),
    }
}

fn selection_mode_to_str(
    mode: crate::widget::view_widgets::list_view::SelectionMode,
) -> &'static str {
    match mode {
        crate::widget::view_widgets::list_view::SelectionMode::Single => "single",
        crate::widget::view_widgets::list_view::SelectionMode::Multi => "multi",
        crate::widget::view_widgets::list_view::SelectionMode::Extended => "extended",
    }
}

fn list_box_selection_mode_to_str(mode: ListBoxSelectionMode) -> &'static str {
    match mode {
        ListBoxSelectionMode::NoSelection => "none",
        ListBoxSelectionMode::SingleSelection => "single",
        ListBoxSelectionMode::MultiSelection => "multi",
        ListBoxSelectionMode::ExtendedSelection => "extended",
    }
}

fn view_mode_to_str(mode: crate::widget::view_widgets::list_view::ViewMode) -> &'static str {
    match mode {
        crate::widget::view_widgets::list_view::ViewMode::List => "list",
        crate::widget::view_widgets::list_view::ViewMode::Icon => "icon",
        crate::widget::view_widgets::list_view::ViewMode::Details => "details",
        crate::widget::view_widgets::list_view::ViewMode::Thumbnails => "thumbnails",
    }
}

fn tool_bar_orientation_to_str(
    orientation: crate::widget::menu_toolbar::tool_bar::ToolBarOrientation,
) -> &'static str {
    match orientation {
        crate::widget::menu_toolbar::tool_bar::ToolBarOrientation::Horizontal => "horizontal",
        crate::widget::menu_toolbar::tool_bar::ToolBarOrientation::Vertical => "vertical",
    }
}

fn alignment_to_str(alignment: crate::core::Alignment) -> &'static str {
    match alignment {
        crate::core::Alignment::Left => "left",
        crate::core::Alignment::Center => "center",
        crate::core::Alignment::Right => "right",
        crate::core::Alignment::Top => "top",
        crate::core::Alignment::Bottom => "bottom",
    }
}

fn check_state_to_str(state: CheckState) -> &'static str {
    match state {
        CheckState::Unchecked => "unchecked",
        CheckState::PartiallyChecked => "partially_checked",
        CheckState::Checked => "checked",
    }
}

fn orientation_to_str(orientation: crate::core::Orientation) -> &'static str {
    match orientation {
        crate::core::Orientation::Horizontal => "horizontal",
        crate::core::Orientation::Vertical => "vertical",
    }
}

fn tick_position_to_str(tick_position: TickPosition) -> &'static str {
    match tick_position {
        TickPosition::NoTicks => "none",
        TickPosition::TicksAbove => "above",
        TickPosition::TicksBelow => "below",
        TickPosition::TicksBothSides => "both",
    }
}

fn lcd_mode_to_str(mode: LCDNumberMode) -> &'static str {
    match mode {
        LCDNumberMode::Hex => "hex",
        LCDNumberMode::Dec => "dec",
        LCDNumberMode::Oct => "oct",
        LCDNumberMode::Bin => "bin",
    }
}

fn segment_style_to_str(style: SegmentStyle) -> &'static str {
    match style {
        SegmentStyle::Outline => "outline",
        SegmentStyle::Filled => "filled",
        SegmentStyle::Flat => "flat",
    }
}

fn weekday_to_str(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "mon",
        chrono::Weekday::Tue => "tue",
        chrono::Weekday::Wed => "wed",
        chrono::Weekday::Thu => "thu",
        chrono::Weekday::Fri => "fri",
        chrono::Weekday::Sat => "sat",
        chrono::Weekday::Sun => "sun",
    }
}

fn naive_date_to_string(date: chrono::NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn date_to_string(date: Date) -> String {
    date.to_string()
}

fn time_to_string(time: Time) -> String {
    time.to_string()
}

fn default_widget_property_value(kind: WidgetKind, property_name: &str) -> Option<CapabilityValue> {
    let value = match kind {
        WidgetKind::Button => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "pressed" => CapabilityValue::Bool(false),
            "default" => CapabilityValue::Bool(false),
            "enabled" => CapabilityValue::Bool(true),
            "tooltip" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::Label => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::CheckBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "state" => CapabilityValue::String("unchecked".to_string()),
            "checked" => CapabilityValue::Bool(false),
            "tristate_enabled" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::RadioButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "group_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Slider => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "tick_position" => CapabilityValue::String("none".to_string()),
            "tick_interval" => CapabilityValue::Int(0),
            "tracking" => CapabilityValue::Bool(true),
            "slider_position" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::ProgressBar => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "text_visible" => CapabilityValue::Bool(true),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "inverted_appearance" => CapabilityValue::Bool(false),
            "progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ScrollBar => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "slider_size" => CapabilityValue::Float(0.1),
            "slider_position" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ListBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "current_row" => CapabilityValue::Null,
            "item_height" => CapabilityValue::Float(20.0),
            "selected_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::SpinBox => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(99),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "prefix" => CapabilityValue::String(String::new()),
            "suffix" => CapabilityValue::String(String::new()),
            "special_value_text" => CapabilityValue::Null,
            "wrapping" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ComboBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::Null,
            "current_text" => CapabilityValue::String(String::new()),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::UInt(10),
            _ => return None,
        },
        WidgetKind::Dial => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(99),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "page_step" => CapabilityValue::Int(10),
            "notches_visible" => CapabilityValue::Bool(false),
            "notch_target" => CapabilityValue::Float(3.7),
            "wrapping" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Window => match property_name {
            "title" => CapabilityValue::String("Window".to_string()),
            "title_bar_height" => CapabilityValue::UInt(32),
            "close_button_size" => CapabilityValue::UInt(14),
            "button_spacing" => CapabilityValue::UInt(40),
            _ => return None,
        },
        WidgetKind::GroupBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Splitter => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "pane_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::LCDNumber => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "min_value" => CapabilityValue::Float(-999999.0),
            "max_value" => CapabilityValue::Float(999999.0),
            "num_digits" => CapabilityValue::Int(6),
            "small_decimal_point" => CapabilityValue::Bool(false),
            "mode" => CapabilityValue::String("dec".to_string()),
            "segment_style" => CapabilityValue::String("filled".to_string()),
            _ => return None,
        },
        WidgetKind::CommandLink => match property_name {
            "text" => CapabilityValue::String("Command".to_string()),
            "description" => CapabilityValue::String(String::new()),
            "enabled" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FontComboBox => match property_name {
            "current_font_family" => CapabilityValue::String("Arial".to_string()),
            "item_count" => CapabilityValue::Int(0),
            "current_index" => CapabilityValue::Int(-1),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::Int(10),
            _ => return None,
        },
        WidgetKind::Action => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "icon_text" => CapabilityValue::String(String::new()),
            "shortcut" => CapabilityValue::String(String::new()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::Bool(false),
            "command_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ToolBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "orientation" => CapabilityValue::String("vertical".to_string()),
            _ => return None,
        },
        WidgetKind::TabBar => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_min_width" => CapabilityValue::UInt(40),
            "tab_max_width" => CapabilityValue::UInt(200),
            _ => return None,
        },
        WidgetKind::Calendar => match property_name {
            "selected_date" => {
                CapabilityValue::String(naive_date_to_string(chrono::Local::now().date_naive()))
            }
            "minimum_date" => CapabilityValue::String("1900-01-01".to_string()),
            "maximum_date" => CapabilityValue::String("3000-12-31".to_string()),
            "first_day_of_week" => CapabilityValue::String("mon".to_string()),
            "grid_visible" => CapabilityValue::Bool(true),
            "navigation_bar_visible" => CapabilityValue::Bool(true),
            "horizontal_header_visible" => CapabilityValue::Bool(true),
            "vertical_header_visible" => CapabilityValue::Bool(false),
            "date_format" => CapabilityValue::String("%Y-%m-%d".to_string()),
            _ => return None,
        },
        WidgetKind::DatePicker => match property_name {
            "date" => CapabilityValue::String("2024-01-01".to_string()),
            "minimum_date" => CapabilityValue::String("1752-09-14".to_string()),
            "maximum_date" => CapabilityValue::String("9999-12-31".to_string()),
            "display_format" => CapabilityValue::String("yyyy-MM-dd".to_string()),
            "calendar_popup" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::TimePicker => match property_name {
            "time" => CapabilityValue::String("00:00:00".to_string()),
            "minimum_time" => CapabilityValue::String("00:00:00".to_string()),
            "maximum_time" => CapabilityValue::String("23:59:59".to_string()),
            "display_format" => CapabilityValue::String("HH:mm:ss".to_string()),
            _ => return None,
        },
        WidgetKind::LineEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            "cursor_position" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ListView => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "focused_row" => CapabilityValue::Null,
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "view_mode" => CapabilityValue::String("list".to_string()),
            _ => return None,
        },
        WidgetKind::TreeView => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "node_count" => CapabilityValue::UInt(0),
            "focused_node" => CapabilityValue::Null,
            "selected_node" => CapabilityValue::Null,
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "selected_row" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(20),
            "column_width" => CapabilityValue::UInt(140),
            "projection_state" => CapabilityValue::String("rows=0,selected=None".to_string()),
            _ => return None,
        },
        WidgetKind::Table => match property_name {
            "has_model" => CapabilityValue::Bool(false),
            "has_delegate" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "has_data_source" => CapabilityValue::Bool(false),
            "scroll_row" => CapabilityValue::UInt(0),
            "scroll_column" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(20),
            "column_width" => CapabilityValue::UInt(120),
            "overscan_rows" => CapabilityValue::UInt(2),
            "overscan_columns" => CapabilityValue::UInt(1),
            "frozen_columns" => CapabilityValue::UInt(0),
            "sort_spec_count" => CapabilityValue::UInt(0),
            "filter_count" => CapabilityValue::UInt(0),
            "sort_specs" => CapabilityValue::String(String::new()),
            "filters" => CapabilityValue::String(String::new()),
            "visible_window" => CapabilityValue::String("0:0:0:0".to_string()),
            _ => return None,
        },
        WidgetKind::DataView => match property_name {
            "has_data_source" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "scroll_row" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(20),
            "overscan" => CapabilityValue::UInt(2),
            "selected_row" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Menu => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "item_count" => CapabilityValue::UInt(0),
            "hovered_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::MenuBar => match property_name {
            "entry_count" => CapabilityValue::UInt(0),
            "active_index" => CapabilityValue::Null,
            "hovered_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ToolBar => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "icon_size" => CapabilityValue::Float(24.0),
            "movable" => CapabilityValue::Bool(true),
            "floatable" => CapabilityValue::Bool(true),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RibbonBar => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_tab" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(true),
            "minimized" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ColorDialog => match property_name {
            "hex_rgba" => CapabilityValue::String("#FF0000FF".to_string()),
            "show_alpha" => CapabilityValue::Bool(true),
            "preset_count" => CapabilityValue::UInt(6),
            _ => return None,
        },
        WidgetKind::RichEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "line_count" => CapabilityValue::UInt(0),
            "cursor_line" => CapabilityValue::UInt(0),
            "cursor_column" => CapabilityValue::UInt(0),
            "marker_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Chart => match property_name {
            "task_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "viewport_start" => CapabilityValue::Int(0),
            "viewport_end" => CapabilityValue::Int(100),
            _ => return None,
        },
        WidgetKind::TextEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            "line_wrap" => CapabilityValue::Bool(true),
            "output_line_count" => CapabilityValue::UInt(0),
            "input_line" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::StatusBar => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "visible" => CapabilityValue::Bool(false),
            "action_label" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Canvas => match property_name {
            "center_x" => CapabilityValue::Float(0.0),
            "center_y" => CapabilityValue::Float(0.0),
            "zoom" => CapabilityValue::Float(1.0),
            "marker_count" => CapabilityValue::UInt(0),
            "selected_marker_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::WebView => match property_name {
            "url" => CapabilityValue::String("about:blank".to_string()),
            "loading" => CapabilityValue::Bool(false),
            "title" => CapabilityValue::String(String::new()),
            "can_go_back" => CapabilityValue::Bool(false),
            "can_go_forward" => CapabilityValue::Bool(false),
            "source" => CapabilityValue::Null,
            "playing" => CapabilityValue::Bool(false),
            "duration_ms" => CapabilityValue::UInt(0),
            "position_ms" => CapabilityValue::UInt(0),
            "volume" => CapabilityValue::UInt(80),
            "muted" => CapabilityValue::Bool(false),
            "fullscreen" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Panel => match property_name {
            "segment_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ToolButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "action_count" => CapabilityValue::UInt(0),
            "menu_open" => CapabilityValue::Bool(false),
            "row_height" => CapabilityValue::UInt(22),
            _ => return None,
        },
        WidgetKind::ToggleButton => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "selected_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::CheckListBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "multi_select" => CapabilityValue::Bool(false),
            "focused_index" => CapabilityValue::Null,
            "selected_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Grid => match property_name {
            "rows" => CapabilityValue::UInt(1),
            "columns" => CapabilityValue::UInt(1),
            "spacing" => CapabilityValue::UInt(0),
            "line_color" => CapabilityValue::String("#DCDCDCFF".to_string()),
            "cell_width" => CapabilityValue::UInt(0),
            "cell_height" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::FreeformShape => match property_name {
            "path_kind" => CapabilityValue::String("rounded_rect".to_string()),
            "fill_rgba" => CapabilityValue::String("#C8DCFFFF".to_string()),
            "stroke_rgba" => CapabilityValue::String("#5078C8FF".to_string()),
            "stroke_width" => CapabilityValue::UInt(2),
            _ => return None,
        },
        // ── Dialog widgets ──────────────────────────────────
        WidgetKind::MessageBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "text" => CapabilityValue::String(String::new()),
            "modal" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FileDialog => match property_name {
            "title" => CapabilityValue::String("Open File".to_string()),
            "modal" => CapabilityValue::Bool(true),
            "directory" => CapabilityValue::String(String::new()),
            "selected_file" => CapabilityValue::Null,
            "mode" => CapabilityValue::String("open_file".to_string()),
            _ => return None,
        },
        WidgetKind::FontDialog => match property_name {
            "modal" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::InputDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "mode" => CapabilityValue::String("text".to_string()),
            "text_value" => CapabilityValue::String(String::new()),
            "int_value" => CapabilityValue::Int(0),
            "double_value" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ProgressDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            _ => return None,
        },
        WidgetKind::PopupWindow => match property_name {
            "has_content" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // ── Container widgets ───────────────────────────────
        WidgetKind::ScrollArea => match property_name {
            "widget_resizable" => CapabilityValue::Bool(true),
            "horizontal_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "vertical_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "scroll_position_x" => CapabilityValue::Int(0),
            "scroll_position_y" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::TabWidget => match property_name {
            "tab_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_position" => CapabilityValue::String("north".to_string()),
            _ => return None,
        },
        WidgetKind::StackedWidget => match property_name {
            "widget_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::CollapsiblePane => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "collapsed" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::DockWidget => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "floating" => CapabilityValue::Bool(false),
            "docked" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::MdiArea => match property_name {
            "subwindow_count" => CapabilityValue::UInt(0),
            "active_subwindow" => CapabilityValue::Null,
            "view_mode" => CapabilityValue::String("sub_window_view".to_string()),
            _ => return None,
        },

        // ── Advanced widgets ────────────────────────────────
        WidgetKind::PieMenu => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "radius" => CapabilityValue::Float(100.0),
            "inner_radius" => CapabilityValue::Float(35.0),
            "current_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::DateTimePicker => match property_name {
            "datetime" => CapabilityValue::Null,
            "display_format" => CapabilityValue::String("yyyy-MM-dd HH:mm:ss".to_string()),
            "calendar_popup" => CapabilityValue::Bool(false),
            "minimum" => CapabilityValue::Null,
            "maximum" => CapabilityValue::Null,
            _ => return None,
        },
        _ => return None,
    };

    Some(value)
}

fn normalize_key(input: &str) -> String {
    input
        .chars()
        .filter(|ch| !matches!(*ch, '_' | '-' | ' '))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn create_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Button::new(text.to_string(), geometry))
}

fn create_label(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Label::new(text.to_string(), geometry))
}

fn create_check_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut check_box = CheckBox::new(geometry);
    if !text.is_empty() {
        check_box.set_text(text.to_string());
    }
    Box::new(check_box)
}

fn create_radio_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut radio_button = RadioButton::new(geometry);
    if !text.is_empty() {
        radio_button.set_text(text.to_string());
    }
    Box::new(radio_button)
}

fn create_slider(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Slider::new(geometry))
}

fn create_progress_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ProgressBar::new(geometry))
}

fn create_scroll_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ScrollBar::new(geometry))
}

fn create_list_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ListBox::new(geometry))
}

fn create_spin_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SpinBox::new(geometry))
}

fn create_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ComboBox::new(geometry))
}

fn create_dial(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Dial::new(geometry))
}

fn create_window(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let title = if text.is_empty() { "Window".to_string() } else { text.to_string() };
    Box::new(Window::new(title, geometry))
}

fn create_group_box(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut group_box = GroupBox::new(geometry);
    if !text.is_empty() {
        group_box.set_title(text.to_string());
    }
    Box::new(group_box)
}

fn create_splitter(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Splitter::new(geometry))
}

fn create_lcd_number(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(LCDNumber::new(geometry))
}

fn create_command_link(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut command_link = CommandLink::new(geometry);
    if !text.is_empty() {
        command_link.set_text(text.to_string());
    }
    Box::new(command_link)
}

fn create_font_combo_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FontComboBox::new(geometry))
}

fn create_action(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Action::new(text.to_string(), geometry))
}

fn create_tool_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ToolBox::new(geometry))
}

fn create_tab_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabBar::new(geometry))
}

fn create_calendar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Calendar::new(geometry))
}

fn create_date_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateEdit::new(geometry))
}

fn create_time_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TimeEdit::new(geometry))
}

fn create_line_edit(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut line_edit = LineEdit::new(geometry);
    if !text.is_empty() {
        line_edit.set_text(text.to_string());
    }
    Box::new(line_edit)
}

fn create_list_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ListView::new(geometry))
}

fn create_tree_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TreeView::new(geometry))
}

fn create_table_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TableWidget::new(geometry))
}

fn create_data_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DataGrid::new(geometry))
}

fn create_tree_table(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TreeTable::new(geometry))
}

fn create_virtual_table(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(VirtualTable::new(geometry))
}

fn create_virtual_list(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(VirtualList::new(geometry))
}

fn create_menu(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(Menu::new(text, geometry))
}

fn create_menu_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MenuBar::new(geometry))
}

fn create_tool_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ToolBar::new(geometry))
}

fn create_ribbon_bar(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(RibbonBar::new(geometry))
}

fn create_color_picker(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ColorPicker::new(geometry))
}

fn create_code_editor(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut editor = CodeEditor::new(geometry);
    if !text.is_empty() {
        editor.set_text(text.to_string());
    }
    Box::new(editor)
}

fn create_gantt_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GanttWidget::new(geometry))
}

fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}

fn create_snackbar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut snackbar = Snackbar::new(geometry);
    if !text.is_empty() {
        snackbar.show(text.to_string());
    }
    Box::new(snackbar)
}

fn create_map_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MapView::new(geometry))
}

fn create_media_player(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MediaPlayer::new(geometry))
}

fn create_breadcrumb(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Breadcrumb::new(geometry))
}

fn create_split_button(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(SplitButton::new(text.to_string(), geometry))
}

fn create_segmented_control(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(SegmentedControl::new(geometry))
}

fn create_chip(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(Chip::new(geometry))
}

fn create_grid(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(GridWidget::new(geometry))
}

fn create_freeform_shape(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FreeformShapeWidget::new(geometry, ShapePath::RoundedRect { radius: 8 }))
}

// ── Dialog widget constructors ────────────────────────────────

fn create_message_box(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MessageBox::new(geometry))
}

fn create_file_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FileDialog::new(geometry))
}

fn create_font_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FontDialog::new(geometry))
}

fn create_input_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(InputDialog::new(geometry))
}

fn create_progress_dialog(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ProgressDialog::new(geometry))
}

fn create_popup_window(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PopupWindow::new(geometry))
}

// ── Container widget constructors ─────────────────────────────

fn create_scroll_area(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(ScrollArea::new(geometry))
}

fn create_tab_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TabWidget::new(geometry))
}

fn create_stacked_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(StackedWidget::new(geometry))
}

fn create_collapsible_pane(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(CollapsiblePane::new(geometry, String::new()))
}

fn create_dock_widget(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DockWidget::new(geometry))
}

fn create_mdi_area(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(MdiArea::new(geometry))
}

// ── Text widget constructors ──────────────────────────────────

fn create_text_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(TextEdit::new(geometry))
}

// ── Web widget constructors ───────────────────────────────────

fn create_web_view(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(WebView::new(geometry))
}

// ── Advanced widget constructors ──────────────────────────────

fn create_pie_menu(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(PieMenu::new(
        crate::core::Point::new(
            geometry.x + (geometry.width / 2) as i32,
            geometry.y + (geometry.height / 2) as i32,
        ),
        geometry.width.min(geometry.height) as f32 / 2.0,
    ))
}

fn create_date_time_edit(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(DateTimeEdit::new(geometry))
}

const BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "pressed",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "default",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tooltip",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const LABEL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "alignment",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

const CHECK_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "state",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tristate_enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const RADIO_BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "group_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const SLIDER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tick_position",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tick_interval",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tracking",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "slider_position",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

const PROGRESS_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "text_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "inverted_appearance",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "progress",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
];

const SCROLL_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "slider_size",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "slider_position",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: false,
    },
];

const LIST_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "current_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_height",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const SPIN_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "prefix",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "suffix",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "special_value_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "wrapping",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "current_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "editable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_visible_items",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const DIAL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "single_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "page_step",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "notches_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "notch_target",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "wrapping",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const WINDOW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "title_bar_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "close_button_size",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "button_spacing",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const GROUP_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "alignment",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checkable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const SPLITTER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "pane_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const LCD_NUMBER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "min_value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_value",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "num_digits",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "small_decimal_point",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "segment_style",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

const COMMAND_LINK_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "description",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const FONT_COMBO_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "current_font_family",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "editable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_visible_items",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

const ACTION_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "icon_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "shortcut",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checkable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "separator",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "command_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const TOOL_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

const TAB_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "closable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tab_min_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "tab_max_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const CALENDAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "selected_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "first_day_of_week",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "grid_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "navigation_bar_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "horizontal_header_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "vertical_header_visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "date_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const DATE_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_date",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "calendar_popup",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimum_time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "maximum_time",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const LINE_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "placeholder_text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "max_length",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "read_only",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "cursor_position",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const LIST_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "focused_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "view_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

const TREE_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "node_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "focused_node",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_node",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const TABLE_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "has_delegate",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selection_mode",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
];

const DATA_GRID_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "scroll_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "frozen_columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "sort_spec_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "filter_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "sort_specs",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "filters",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const TREE_TABLE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_model",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const VIRTUAL_TABLE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "column_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "scroll_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "column_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan_rows",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan_columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "visible_window",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

const VIRTUAL_LIST_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "has_data_source",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "row_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "scroll_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "overscan",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "selected_row",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const MENU_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "hovered_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const MENU_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "entry_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "active_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "hovered_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const TOOL_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "orientation",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "icon_size",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "floatable",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const RIBBON_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "current_tab",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "expanded",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "minimized",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const COLOR_PICKER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "hex_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "show_alpha",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "preset_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const CODE_EDITOR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "line_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cursor_line",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cursor_column",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "marker_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "task_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "viewport_start",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "viewport_end",
        value_kind: PropertyValueKind::Int,
        readable: true,
        writable: true,
    },
];

const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "output_line_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "input_line",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
];

const SNACKBAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "message",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "visible",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "action_label",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

const MAP_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "center_x",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "center_y",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "zoom",
        value_kind: PropertyValueKind::Float,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "marker_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_marker_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

const MEDIA_PLAYER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "source",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "playing",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "duration_ms",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "position_ms",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "volume",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "muted",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "fullscreen",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
];

const BREADCRUMB_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "segment_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const SPLIT_BUTTON_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "action_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "menu_open",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "row_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

const SEGMENTED_CONTROL_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_id",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: false,
    },
];

const CHIP_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "multi_select",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "focused_index",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "selected_count",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const GRID_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "rows",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "columns",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "spacing",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "line_color",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "cell_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "cell_height",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: false,
    },
];

const FREEFORM_SHAPE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "path_kind",
        value_kind: PropertyValueKind::Enum,
        readable: true,
        writable: false,
    },
    PropertySchema {
        name: "fill_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "stroke_rgba",
        value_kind: PropertyValueKind::String,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "stroke_width",
        value_kind: PropertyValueKind::UInt,
        readable: true,
        writable: true,
    },
];

// ── Dialog widgets ──────────────────────────────────────────

const MESSAGE_BOX_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "modal",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

const FILE_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "modal",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "directory",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "selected_file",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

const FONT_DIALOG_PROPERTIES: &[PropertySchema] = &[PropertySchema {
    name: "modal",
    value_kind: PropertyValueKind::Bool,
    readable: false,
    writable: false,
}];

const INPUT_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "label_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "text_value",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "int_value",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "double_value",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
];

const PROGRESS_DIALOG_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "label_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "value",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
];

const POPUP_WINDOW_PROPERTIES: &[PropertySchema] = &[PropertySchema {
    name: "has_content",
    value_kind: PropertyValueKind::Bool,
    readable: false,
    writable: false,
}];

// ── Container widgets ───────────────────────────────────────

const SCROLL_AREA_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "widget_resizable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "horizontal_scroll_bar_policy",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "vertical_scroll_bar_policy",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "scroll_position_x",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "scroll_position_y",
        value_kind: PropertyValueKind::Int,
        readable: false,
        writable: false,
    },
];

const TAB_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "tab_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "closable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "movable",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "tab_position",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

const STACKED_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "widget_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
];

const COLLAPSIBLE_PANE_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "collapsed",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

const DOCK_WIDGET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "floating",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "docked",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

const MDI_AREA_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "subwindow_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "active_subwindow",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "view_mode",
        value_kind: PropertyValueKind::Enum,
        readable: false,
        writable: false,
    },
];

// ── Input / text widgets ────────────────────────────────────

const TEXT_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "placeholder_text",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "max_length",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "read_only",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "line_wrap",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

// ── Web widgets ─────────────────────────────────────────────

const WEB_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "url",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "loading",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "title",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "can_go_back",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "can_go_forward",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
];

// ── Advanced widgets ────────────────────────────────────────

const PIE_MENU_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "item_count",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "radius",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "inner_radius",
        value_kind: PropertyValueKind::Float,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "current_index",
        value_kind: PropertyValueKind::UInt,
        readable: false,
        writable: false,
    },
];

const DATE_TIME_EDIT_PROPERTIES: &[PropertySchema] = &[
    PropertySchema {
        name: "datetime",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "display_format",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "calendar_popup",
        value_kind: PropertyValueKind::Bool,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "minimum",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
    PropertySchema {
        name: "maximum",
        value_kind: PropertyValueKind::String,
        readable: false,
        writable: false,
    },
];

fn button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Button,
        canonical_name: "button",
        aliases: &["pushbutton", "btn"],
        properties: BUTTON_PROPERTIES,
        events: &["clicked", "pressed", "released", "state_changed"],
        commands: &["click"],
    }
}

fn label_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "label",
        aliases: &["text_label"],
        properties: LABEL_PROPERTIES,
        events: &[],
        commands: &["set_text", "set_alignment"],
    }
}

fn check_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CheckBox,
        canonical_name: "check_box",
        aliases: &["checkbox"],
        properties: CHECK_BOX_PROPERTIES,
        events: &["toggled", "state_changed"],
        commands: &["set_checked", "toggle", "set_state"],
    }
}

fn radio_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RadioButton,
        canonical_name: "radio_button",
        aliases: &["radiobutton"],
        properties: RADIO_BUTTON_PROPERTIES,
        events: &["selected", "checked_changed"],
        commands: &["set_checked", "set_group_id"],
    }
}

fn slider_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Slider,
        canonical_name: "slider",
        aliases: &["range_slider"],
        properties: SLIDER_PROPERTIES,
        events: &["value_changed", "slider_moved"],
        commands: &["set_range", "set_value", "set_slider_position"],
    }
}

fn progress_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressBar,
        canonical_name: "progress_bar",
        aliases: &["progressbar"],
        properties: PROGRESS_BAR_PROPERTIES,
        events: &["value_changed", "range_changed"],
        commands: &["set_range", "set_value", "set_orientation"],
    }
}

fn scroll_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollBar,
        canonical_name: "scroll_bar",
        aliases: &["scrollbar"],
        properties: SCROLL_BAR_PROPERTIES,
        events: &["value_changed", "range_changed", "slider_moved"],
        commands: &["set_range", "set_value", "set_steps", "set_orientation"],
    }
}

fn list_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListBox,
        canonical_name: "list_box",
        aliases: &["listbox"],
        properties: LIST_BOX_PROPERTIES,
        events: &["item_selected", "item_activated", "selection_changed"],
        commands: &["add_item", "remove_item", "clear", "clear_selection", "set_selection_mode"],
    }
}

fn spin_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SpinBox,
        canonical_name: "spin_box",
        aliases: &["spinbox"],
        properties: SPIN_BOX_PROPERTIES,
        events: &["value_changed", "editing_finished"],
        commands: &["set_range", "set_value", "step_up", "step_down"],
    }
}

fn combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ComboBox,
        canonical_name: "combo_box",
        aliases: &["combobox"],
        properties: COMBO_BOX_PROPERTIES,
        events: &["current_index_changed", "current_text_changed", "activated"],
        commands: &["set_items", "set_current_index", "clear"],
    }
}

fn dial_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Dial,
        canonical_name: "dial",
        aliases: &["knob"],
        properties: DIAL_PROPERTIES,
        events: &["value_changed", "slider_moved", "slider_pressed", "slider_released"],
        commands: &["set_range", "set_value", "set_wrapping"],
    }
}

fn window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Window,
        canonical_name: "window",
        aliases: &["main_window"],
        properties: WINDOW_PROPERTIES,
        events: &["closed"],
        commands: &["set_title", "close"],
    }
}

fn group_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::GroupBox,
        canonical_name: "group_box",
        aliases: &["groupbox"],
        properties: GROUP_BOX_PROPERTIES,
        events: &["toggled"],
        commands: &["set_title", "set_checkable", "set_checked", "toggle"],
    }
}

fn splitter_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Splitter,
        canonical_name: "splitter",
        aliases: &["pane_splitter"],
        properties: SPLITTER_PROPERTIES,
        events: &["pane_layout_changed", "orientation_changed"],
        commands: &["set_orientation", "set_ratio", "set_ratios"],
    }
}

fn lcd_number_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LCDNumber,
        canonical_name: "lcd_number",
        aliases: &["lcdnumber"],
        properties: LCD_NUMBER_PROPERTIES,
        events: &["value_changed", "overflow"],
        commands: &["set_value", "set_mode", "set_segment_style"],
    }
}

fn command_link_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CommandLink,
        canonical_name: "command_link",
        aliases: &["commandlink"],
        properties: COMMAND_LINK_PROPERTIES,
        events: &["clicked", "hovered"],
        commands: &["set_text", "set_description", "set_enabled", "click"],
    }
}

fn font_combo_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontComboBox,
        canonical_name: "font_combo_box",
        aliases: &["fontcombobox"],
        properties: FONT_COMBO_BOX_PROPERTIES,
        events: &[
            "current_font_changed",
            "current_index_changed",
            "activated",
            "text_edited",
            "popup_shown",
            "popup_hidden",
        ],
        commands: &[
            "set_current_index",
            "set_editable",
            "set_max_visible_items",
            "show_popup",
            "hide_popup",
        ],
    }
}

fn action_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Action,
        canonical_name: "action",
        aliases: &["command_action"],
        properties: ACTION_PROPERTIES,
        events: &["triggered", "toggled", "hovered", "changed"],
        commands: &["set_text", "set_checkable", "set_checked", "trigger"],
    }
}

fn tool_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBox,
        canonical_name: "tool_box",
        aliases: &["toolbox"],
        properties: TOOL_BOX_PROPERTIES,
        events: &["current_changed"],
        commands: &["add_item", "remove_item", "set_current_index", "set_orientation"],
    }
}

fn tab_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabBar,
        canonical_name: "tab_bar",
        aliases: &["tabbar"],
        properties: TAB_BAR_PROPERTIES,
        events: &["current_changed", "tab_close_requested", "tab_moved"],
        commands: &["add_tab", "remove_tab", "set_current_index", "set_closable", "set_movable"],
    }
}

fn calendar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Calendar,
        canonical_name: "calendar",
        aliases: &["date_calendar"],
        properties: CALENDAR_PROPERTIES,
        events: &["selection_changed"],
        commands: &["set_selected_date", "set_date_range", "set_first_day_of_week", "show_today"],
    }
}

fn date_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DatePicker,
        canonical_name: "date_edit",
        aliases: &["dateedit", "date_picker"],
        properties: DATE_EDIT_PROPERTIES,
        events: &["date_changed"],
        commands: &["set_date", "set_date_range", "set_display_format"],
    }
}

fn time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimePicker,
        canonical_name: "time_edit",
        aliases: &["timeedit", "time_picker"],
        properties: TIME_EDIT_PROPERTIES,
        events: &["time_changed"],
        commands: &["set_time", "set_time_range", "set_display_format"],
    }
}

fn line_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::LineEdit,
        canonical_name: "line_edit",
        aliases: &["lineedit", "text_input", "input"],
        properties: LINE_EDIT_PROPERTIES,
        events: &["text_changed", "editing_finished", "return_pressed"],
        commands: &["clear", "select_all"],
    }
}

fn list_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ListView,
        canonical_name: "list_view",
        aliases: &["listview"],
        properties: LIST_VIEW_PROPERTIES,
        events: &["selection_changed", "focused_row_changed"],
        commands: &["clear_selection", "clear_focused_row"],
    }
}

fn tree_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_view",
        aliases: &["treeview"],
        properties: TREE_VIEW_PROPERTIES,
        events: &["selection_changed", "focused_node_changed"],
        commands: &["clear_selection", "clear_focused_node"],
    }
}

fn table_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "table_widget",
        aliases: &["tablewidget", "table"],
        properties: TABLE_WIDGET_PROPERTIES,
        events: &["selection_changed", "focused_row_changed"],
        commands: &["clear_selection", "clear_focused_row"],
    }
}

fn data_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "data_grid",
        aliases: &["datagrid"],
        properties: DATA_GRID_PROPERTIES,
        events: &["visible_window_changed"],
        commands: &[
            "set_data_source",
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_frozen_columns",
        ],
    }
}

fn tree_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TreeView,
        canonical_name: "tree_table",
        aliases: &["treetable"],
        properties: TREE_TABLE_PROPERTIES,
        events: &["projection_changed", "selection_changed"],
        commands: &["set_model", "clear_model", "expand_row", "collapse_row", "select_row"],
    }
}

fn virtual_table_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "virtual_table",
        aliases: &["virtualtable"],
        properties: VIRTUAL_TABLE_PROPERTIES,
        events: &["visible_window_changed"],
        commands: &[
            "set_data_source",
            "clear_data_source",
            "set_scroll_row",
            "set_scroll_column",
            "set_row_height",
            "set_column_width",
            "set_overscan_rows",
            "set_overscan_columns",
            "fetch_visible_window",
        ],
    }
}

fn virtual_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DataView,
        canonical_name: "virtual_list",
        aliases: &["virtuallist", "data_view", "dataview"],
        properties: VIRTUAL_LIST_PROPERTIES,
        events: &["selection_changed", "visible_window_changed"],
        commands: &["clear_data_source", "fetch_visible_rows"],
    }
}

fn menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Menu,
        canonical_name: "menu",
        aliases: &["context_menu"],
        properties: MENU_PROPERTIES,
        events: &["triggered", "triggered_index", "about_to_show", "about_to_hide"],
        commands: &["clear", "add_action", "add_separator"],
    }
}

fn menu_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MenuBar,
        canonical_name: "menu_bar",
        aliases: &["menubar"],
        properties: MENU_BAR_PROPERTIES,
        events: &["triggered", "hovered_entry"],
        commands: &["clear", "add_menu", "remove_menu"],
    }
}

fn tool_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolBar,
        canonical_name: "tool_bar",
        aliases: &["toolbar"],
        properties: TOOL_BAR_PROPERTIES,
        events: &[
            "action_triggered",
            "orientation_changed",
            "top_level_changed",
            "visibility_changed",
        ],
        commands: &["clear", "add_action", "add_separator"],
    }
}

fn ribbon_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RibbonBar,
        canonical_name: "ribbon_bar",
        aliases: &["ribbonbar", "ribbon"],
        properties: RIBBON_BAR_PROPERTIES,
        events: &["current_tab_changed", "item_triggered"],
        commands: &["add_tab", "add_group", "add_item", "add_large_item", "clear"],
    }
}

fn color_picker_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ColorDialog,
        canonical_name: "color_picker",
        aliases: &["colorpicker"],
        properties: COLOR_PICKER_PROPERTIES,
        events: &["color_changed", "hex_changed"],
        commands: &["set_hex", "apply_preset"],
    }
}

fn code_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::RichEdit,
        canonical_name: "code_editor",
        aliases: &["codeeditor"],
        properties: CODE_EDITOR_PROPERTIES,
        events: &["text_changed", "cursor_moved", "selection_changed"],
        commands: &["set_text", "append_line", "set_markers", "set_cursor"],
    }
}

fn gantt_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_widget",
        aliases: &["gantt", "ganttwidget"],
        properties: GANTT_WIDGET_PROPERTIES,
        events: &["task_selected", "viewport_changed"],
        commands: &["set_tasks", "zoom", "set_viewport", "select_task"],
    }
}

fn terminal_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "terminal_view",
        aliases: &["terminal", "terminalview"],
        properties: TERMINAL_VIEW_PROPERTIES,
        events: &["command_submitted"],
        commands: &["append_output", "submit"],
    }
}

fn snackbar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StatusBar,
        canonical_name: "snackbar",
        aliases: &["toast_bar"],
        properties: SNACKBAR_PROPERTIES,
        events: &["action_triggered", "dismissed"],
        commands: &["show", "show_with_action", "dismiss"],
    }
}

fn map_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Canvas,
        canonical_name: "map_view",
        aliases: &["mapview"],
        properties: MAP_VIEW_PROPERTIES,
        events: &["center_changed", "zoom_changed", "marker_selected"],
        commands: &["set_markers", "set_center", "set_zoom"],
    }
}

fn media_player_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebView,
        canonical_name: "media_player",
        aliases: &["mediaplayer"],
        properties: MEDIA_PLAYER_PROPERTIES,
        events: &["playback_changed", "position_changed", "volume_changed", "source_changed"],
        commands: &["set_source", "clear_source", "play", "pause", "seek_to", "set_volume"],
    }
}

fn breadcrumb_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Panel,
        canonical_name: "breadcrumb",
        aliases: &["nav_breadcrumb"],
        properties: BREADCRUMB_PROPERTIES,
        events: &["segment_activated"],
        commands: &["set_segments", "push_segment", "clear_segments"],
    }
}

fn split_button_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToolButton,
        canonical_name: "split_button",
        aliases: &["splitbutton"],
        properties: SPLIT_BUTTON_PROPERTIES,
        events: &["triggered", "action_selected", "menu_toggled"],
        commands: &["add_action", "open_menu", "close_menu", "trigger_primary"],
    }
}

fn segmented_control_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ToggleButton,
        canonical_name: "segmented_control",
        aliases: &["segmentedcontrol"],
        properties: SEGMENTED_CONTROL_PROPERTIES,
        events: &["selection_changed"],
        commands: &["set_items", "move_selection"],
    }
}

fn chip_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CheckListBox,
        canonical_name: "chip",
        aliases: &["chips"],
        properties: CHIP_PROPERTIES,
        events: &["chip_toggled"],
        commands: &["set_items", "toggle_index", "move_focus"],
    }
}

fn grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Grid,
        canonical_name: "grid",
        aliases: &["grid_widget", "gridwidget"],
        properties: GRID_PROPERTIES,
        events: &["cell_clicked", "cell_double_clicked", "selection_changed"],
        commands: &[
            "set_rows",
            "set_columns",
            "set_spacing",
            "select_cell",
            "clear_selection",
            "set_line_color",
        ],
    }
}

fn freeform_shape_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FreeformShape,
        canonical_name: "freeform_shape",
        commands: &["set_fill_color", "set_stroke_color", "set_stroke_width"],
        aliases: &["freeformshape"],
        properties: FREEFORM_SHAPE_PROPERTIES,
        events: &["clicked", "hovered_changed", "pressed_changed"],
    }
}

// ── Dialog widget capabilities ────────────────────────────────

fn message_box_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MessageBox,
        canonical_name: "message_box",
        aliases: &["messagebox", "msgbox"],
        properties: MESSAGE_BOX_PROPERTIES,
        events: &["button_clicked", "accepted", "rejected"],
        commands: &["set_text", "set_title", "set_icon"],
    }
}

fn file_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FileDialog,
        canonical_name: "file_dialog",
        aliases: &["filedialog"],
        properties: FILE_DIALOG_PROPERTIES,
        events: &["file_selected", "files_selected", "accepted", "rejected"],
        commands: &["set_mode", "set_directory", "open"],
    }
}

fn font_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FontDialog,
        canonical_name: "font_dialog",
        aliases: &["fontdialog"],
        properties: FONT_DIALOG_PROPERTIES,
        events: &["font_selected", "accepted", "rejected"],
        commands: &["set_current_font", "accept", "reject"],
    }
}

fn input_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::InputDialog,
        canonical_name: "input_dialog",
        aliases: &["inputdialog"],
        properties: INPUT_DIALOG_PROPERTIES,
        events: &[
            "text_value_changed",
            "int_value_changed",
            "double_value_changed",
            "accepted",
            "rejected",
        ],
        commands: &["set_mode", "set_text_value", "set_int_value", "set_double_value"],
    }
}

fn progress_dialog_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ProgressDialog,
        canonical_name: "progress_dialog",
        aliases: &["progressdialog"],
        properties: PROGRESS_DIALOG_PROPERTIES,
        events: &["canceled"],
        commands: &["set_value", "set_range", "set_title", "set_label_text"],
    }
}

fn popup_window_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PopupWindow,
        canonical_name: "popup_window",
        aliases: &["popupwindow", "popup"],
        properties: POPUP_WINDOW_PROPERTIES,
        events: &[],
        commands: &["set_content_widget"],
    }
}

// ── Container widget capabilities ─────────────────────────────

fn scroll_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::ScrollArea,
        canonical_name: "scroll_area",
        aliases: &["scrollarea"],
        properties: SCROLL_AREA_PROPERTIES,
        events: &["scroll_position_changed"],
        commands: &["set_widget_resizable", "set_horizontal_policy", "set_vertical_policy"],
    }
}

fn tab_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TabWidget,
        canonical_name: "tab_widget",
        aliases: &["tabwidget"],
        properties: TAB_WIDGET_PROPERTIES,
        events: &["current_changed", "tab_close_requested"],
        commands: &[
            "add_tab",
            "remove_tab",
            "set_current_index",
            "set_closable",
            "set_movable",
            "set_tab_position",
        ],
    }
}

fn stacked_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::StackedWidget,
        canonical_name: "stacked_widget",
        aliases: &["stackedwidget", "stacked"],
        properties: STACKED_WIDGET_PROPERTIES,
        events: &["current_changed"],
        commands: &["add_widget", "remove_widget", "set_current_index"],
    }
}

fn collapsible_pane_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::CollapsiblePane,
        canonical_name: "collapsible_pane",
        aliases: &["collapsiblepane", "collapsible"],
        properties: COLLAPSIBLE_PANE_PROPERTIES,
        events: &["toggled"],
        commands: &["set_title", "set_collapsed", "toggle"],
    }
}

fn dock_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DockWidget,
        canonical_name: "dock_widget",
        aliases: &["dockwidget", "dock"],
        properties: DOCK_WIDGET_PROPERTIES,
        events: &["dock_location_changed", "features_changed", "top_level_changed"],
        commands: &["set_title", "set_floating", "set_features", "set_allowed_areas"],
    }
}

fn mdi_area_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::MdiArea,
        canonical_name: "mdi_area",
        aliases: &["mdiarea", "mdi"],
        properties: MDI_AREA_PROPERTIES,
        events: &["subwindow_activated"],
        commands: &["add_subwindow", "remove_subwindow", "set_view_mode", "activate_subwindow"],
    }
}

// ── Text/input widget capabilities ───────────────────────────

fn text_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "text_edit",
        aliases: &["textedit"],
        properties: TEXT_EDIT_PROPERTIES,
        events: &["text_changed", "cursor_position_changed"],
        commands: &[
            "set_text",
            "set_placeholder_text",
            "set_max_length",
            "set_read_only",
            "set_line_wrap",
        ],
    }
}

// ── Web widget capabilities ──────────────────────────────────

fn web_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::WebView,
        canonical_name: "web_view",
        aliases: &["webview"],
        properties: WEB_VIEW_PROPERTIES,
        events: &[
            "loading_started",
            "loading_finished",
            "title_changed",
            "url_changed",
            "error_occurred",
            "navigation_state_changed",
        ],
        commands: &["set_url", "load_url", "reload", "go_back", "go_forward", "stop"],
    }
}

// ── Advanced widget capabilities ─────────────────────────────

fn pie_menu_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::PieMenu,
        canonical_name: "pie_menu",
        aliases: &["piemenu", "radial_menu"],
        properties: PIE_MENU_PROPERTIES,
        events: &["triggered", "triggered_text", "about_to_show", "about_to_hide"],
        commands: &["add_item", "remove_item", "set_radius", "set_current_index"],
    }
}

fn date_time_edit_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DateTimePicker,
        canonical_name: "date_time_edit",
        aliases: &["datetimeedit", "datetimepicker", "date_time_picker"],
        properties: DATE_TIME_EDIT_PROPERTIES,
        events: &["datetime_changed"],
        commands: &["set_datetime", "set_display_format", "set_calendar_popup"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_factory_registers_core_capabilities() {
        let factory = WidgetFactory::new_with_defaults();

        assert!(factory.capability("label").is_some());
        assert!(factory.capability("checkbox").is_some());
        assert!(factory.capability("radiobutton").is_some());
        assert!(factory.capability("slider").is_some());
        assert!(factory.capability("lineedit").is_some());
        assert!(factory.capability("list_view").is_some());
        assert!(factory.capability("treeview").is_some());
        assert!(factory.capability("table").is_some());
        assert!(factory.capability("dataview").is_some());
        assert!(factory.capability("menu").is_some());
        assert!(factory.capability("menubar").is_some());
        assert!(factory.capability("toolbar").is_some());
        assert!(factory.capability("ribbon").is_some());
        assert!(factory.capability("colorpicker").is_some());
        assert!(factory.capability("code_editor").is_some());
        assert!(factory.capability("gantt").is_some());
        assert!(factory.capability("terminalview").is_some());
        assert!(factory.capability("snackbar").is_some());
        assert!(factory.capability("mapview").is_some());
        assert!(factory.capability("mediaplayer").is_some());
        assert!(factory.capability("breadcrumb").is_some());
        assert!(factory.capability("splitbutton").is_some());
        assert!(factory.capability("segmentedcontrol").is_some());
        assert!(factory.capability("chips").is_some());
        assert!(factory.capability("gridwidget").is_some());
        assert!(factory.capability("freeformshape").is_some());
        assert!(factory.capability("progressbar").is_some());
        assert!(factory.capability("scrollbar").is_some());
        assert!(factory.capability("listbox").is_some());
        assert!(factory.capability("spinbox").is_some());
        assert!(factory.capability("combobox").is_some());
        assert!(factory.capability("dial").is_some());
        assert!(factory.capability("window").is_some());
        assert!(factory.capability("groupbox").is_some());
        assert!(factory.capability("splitter").is_some());
        assert!(factory.capability("lcdnumber").is_some());
        assert!(factory.capability("commandlink").is_some());
        assert!(factory.capability("fontcombobox").is_some());
        assert!(factory.capability("action").is_some());
        assert!(factory.capability("toolbox").is_some());
        assert!(factory.capability("tabbar").is_some());
        assert!(factory.capability("calendar").is_some());
        assert!(factory.capability("dateedit").is_some());
        assert!(factory.capability("timeedit").is_some());
        assert!(factory.capability("datagrid").is_some());
        assert!(factory.capability("treetable").is_some());
        assert!(factory.capability("virtualtable").is_some());

        // ── Newly registered capabilities (R3/R4/R5) ───────
        assert!(factory.capability("messagebox").is_some());
        assert!(factory.capability("filedialog").is_some());
        assert!(factory.capability("fontdialog").is_some());
        assert!(factory.capability("inputdialog").is_some());
        assert!(factory.capability("progressdialog").is_some());
        assert!(factory.capability("popupwindow").is_some());
        assert!(factory.capability("scrollarea").is_some());
        assert!(factory.capability("tabwidget").is_some());
        assert!(factory.capability("stackedwidget").is_some());
        assert!(factory.capability("collapsiblepane").is_some());
        assert!(factory.capability("dockwidget").is_some());
        assert!(factory.capability("mdiarea").is_some());
        assert!(factory.capability("textedit").is_some());
        assert!(factory.capability("webview").is_some());
        assert!(factory.capability("piemenu").is_some());
        assert!(factory.capability("datetimepicker").is_some());
    }

    #[test]
    fn factory_creates_registered_widgets_by_alias() {
        let factory = WidgetFactory::new_with_defaults();
        let rect = Rect::new(1, 2, 120, 40);

        let button = factory.create("btn", rect, "Run").expect("button must be created via alias");
        assert_eq!(button.kind(), WidgetKind::Button);
        assert_eq!(button.geometry(), rect);

        let label =
            factory.create("label", rect, "Name").expect("label must be created by canonical name");
        assert_eq!(label.kind(), WidgetKind::Label);

        let check_box =
            factory.create("checkbox", rect, "Accept").expect("checkbox must be created via alias");
        assert_eq!(check_box.kind(), WidgetKind::CheckBox);

        let radio_button = factory
            .create("radiobutton", rect, "Option")
            .expect("radio button must be created via alias");
        assert_eq!(radio_button.kind(), WidgetKind::RadioButton);

        let slider =
            factory.create("slider", rect, "").expect("slider must be created by canonical name");
        assert_eq!(slider.kind(), WidgetKind::Slider);

        let line_edit =
            factory.create("input", rect, "hello").expect("line edit must be created via alias");
        assert_eq!(line_edit.kind(), WidgetKind::LineEdit);

        let table =
            factory.create("table", rect, "").expect("table widget must be created via alias");
        assert_eq!(table.kind(), WidgetKind::Table);

        let data_view =
            factory.create("dataview", rect, "").expect("data view must be created via alias");
        assert_eq!(data_view.kind(), WidgetKind::DataView);

        let tree =
            factory.create("treeview", rect, "").expect("tree view must be created via alias");
        assert_eq!(tree.kind(), WidgetKind::TreeView);

        let ribbon =
            factory.create("ribbon", rect, "").expect("ribbon bar must be created via alias");
        assert_eq!(ribbon.kind(), WidgetKind::RibbonBar);

        let color_picker = factory
            .create("colorpicker", rect, "")
            .expect("color picker must be created via alias");
        assert_eq!(color_picker.kind(), WidgetKind::ColorDialog);

        let code_editor = factory
            .create("codeeditor", rect, "let x = 1;")
            .expect("code editor must be created via alias");
        assert_eq!(code_editor.kind(), WidgetKind::RichEdit);

        let gantt = factory.create("gantt", rect, "").expect("gantt must be created via alias");
        assert_eq!(gantt.kind(), WidgetKind::Chart);

        let terminal = factory
            .create("terminalview", rect, "echo hi")
            .expect("terminal view must be created via alias");
        assert_eq!(terminal.kind(), WidgetKind::TextEdit);

        let snackbar =
            factory.create("snackbar", rect, "Saved").expect("snackbar must be created via alias");
        assert_eq!(snackbar.kind(), WidgetKind::StatusBar);

        let map = factory.create("mapview", rect, "").expect("map view must be created via alias");
        assert_eq!(map.kind(), WidgetKind::Canvas);

        let media = factory
            .create("mediaplayer", rect, "")
            .expect("media player must be created via alias");
        assert_eq!(media.kind(), WidgetKind::WebView);

        let breadcrumb =
            factory.create("breadcrumb", rect, "").expect("breadcrumb must be created via alias");
        assert_eq!(breadcrumb.kind(), WidgetKind::Panel);

        let split_button = factory
            .create("splitbutton", rect, "Build")
            .expect("split button must be created via alias");
        assert_eq!(split_button.kind(), WidgetKind::ToolButton);

        let segmented_control = factory
            .create("segmentedcontrol", rect, "")
            .expect("segmented control must be created via alias");
        assert_eq!(segmented_control.kind(), WidgetKind::ToggleButton);

        let chip = factory.create("chips", rect, "").expect("chip must be created via alias");
        assert_eq!(chip.kind(), WidgetKind::CheckListBox);

        let grid = factory.create("gridwidget", rect, "").expect("grid must be created via alias");
        assert_eq!(grid.kind(), WidgetKind::Grid);

        let shape = factory
            .create("freeformshape", rect, "")
            .expect("freeform shape must be created via alias");
        assert_eq!(shape.kind(), WidgetKind::FreeformShape);

        let progress = factory
            .create("progressbar", rect, "")
            .expect("progress bar must be created via alias");
        assert_eq!(progress.kind(), WidgetKind::ProgressBar);

        let scroll =
            factory.create("scrollbar", rect, "").expect("scroll bar must be created via alias");
        assert_eq!(scroll.kind(), WidgetKind::ScrollBar);

        let list_box =
            factory.create("listbox", rect, "").expect("list box must be created via alias");
        assert_eq!(list_box.kind(), WidgetKind::ListBox);

        let spin_box =
            factory.create("spinbox", rect, "").expect("spin box must be created via alias");
        assert_eq!(spin_box.kind(), WidgetKind::SpinBox);

        let combo_box =
            factory.create("combobox", rect, "").expect("combo box must be created via alias");
        assert_eq!(combo_box.kind(), WidgetKind::ComboBox);

        let dial =
            factory.create("dial", rect, "").expect("dial must be created by canonical name");
        assert_eq!(dial.kind(), WidgetKind::Dial);

        let window = factory
            .create("window", rect, "Main")
            .expect("window must be created by canonical name");
        assert_eq!(window.kind(), WidgetKind::Window);

        let group_box = factory
            .create("groupbox", rect, "Options")
            .expect("group box must be created via alias");
        assert_eq!(group_box.kind(), WidgetKind::GroupBox);

        let splitter = factory
            .create("splitter", rect, "")
            .expect("splitter must be created by canonical name");
        assert_eq!(splitter.kind(), WidgetKind::Splitter);

        let lcd_number =
            factory.create("lcdnumber", rect, "").expect("lcd number must be created via alias");
        assert_eq!(lcd_number.kind(), WidgetKind::LCDNumber);

        let command_link = factory
            .create("commandlink", rect, "Open")
            .expect("command link must be created via alias");
        assert_eq!(command_link.kind(), WidgetKind::CommandLink);

        let font_combo = factory
            .create("fontcombobox", rect, "")
            .expect("font combo box must be created via alias");
        assert_eq!(font_combo.kind(), WidgetKind::FontComboBox);

        let action = factory
            .create("action", rect, "Run")
            .expect("action must be created by canonical name");
        assert_eq!(action.kind(), WidgetKind::Action);

        let tool_box =
            factory.create("toolbox", rect, "").expect("tool box must be created via alias");
        assert_eq!(tool_box.kind(), WidgetKind::ToolBox);

        let tab_bar =
            factory.create("tabbar", rect, "").expect("tab bar must be created via alias");
        assert_eq!(tab_bar.kind(), WidgetKind::TabBar);

        let calendar = factory
            .create("calendar", rect, "")
            .expect("calendar must be created by canonical name");
        assert_eq!(calendar.kind(), WidgetKind::Calendar);

        let date_edit =
            factory.create("dateedit", rect, "").expect("date edit must be created via alias");
        assert_eq!(date_edit.kind(), WidgetKind::DatePicker);

        let time_edit =
            factory.create("timeedit", rect, "").expect("time edit must be created via alias");
        assert_eq!(time_edit.kind(), WidgetKind::TimePicker);

        let data_grid =
            factory.create("datagrid", rect, "").expect("data grid must be created via alias");
        assert_eq!(data_grid.kind(), WidgetKind::Table);

        let tree_table =
            factory.create("treetable", rect, "").expect("tree table must be created via alias");
        assert_eq!(tree_table.kind(), WidgetKind::TreeView);

        let virtual_table = factory
            .create("virtualtable", rect, "")
            .expect("virtual table must be created via alias");
        assert_eq!(virtual_table.kind(), WidgetKind::Table);
    }

    #[test]
    fn capability_by_kind_returns_expected_schema() {
        let factory = WidgetFactory::new_with_defaults();
        let table_cap =
            factory.capability_by_kind(WidgetKind::Table).expect("table capability must exist");

        assert_eq!(table_cap.canonical_name, "table_widget");
        assert!(table_cap.properties.iter().any(|p| p.name == "has_model"));
        assert!(table_cap.properties.iter().any(|p| p.name == "has_delegate"));
        assert!(table_cap.events.contains(&"selection_changed"));
    }

    #[test]
    fn create_unknown_widget_returns_none() {
        let factory = WidgetFactory::new_with_defaults();
        assert!(factory.create("not_registered", Rect::new(0, 0, 1, 1), "").is_none());
        assert!(factory.capability("not_registered").is_none());
    }

    #[test]
    fn create_by_kind_uses_registered_constructor() {
        let factory = WidgetFactory::new_with_defaults();
        let rect = Rect::new(10, 20, 180, 30);
        let widget = factory
            .create_by_kind(WidgetKind::LineEdit, rect, "abc")
            .expect("line edit must be created by kind");

        assert_eq!(widget.kind(), WidgetKind::LineEdit);
        assert_eq!(widget.geometry(), rect);

        let label = factory
            .create_by_kind(WidgetKind::Label, rect, "caption")
            .expect("label must be created by kind");
        assert_eq!(label.kind(), WidgetKind::Label);

        let check_box = factory
            .create_by_kind(WidgetKind::CheckBox, rect, "accept")
            .expect("checkbox must be created by kind");
        assert_eq!(check_box.kind(), WidgetKind::CheckBox);

        let radio_button = factory
            .create_by_kind(WidgetKind::RadioButton, rect, "a")
            .expect("radio button must be created by kind");
        assert_eq!(radio_button.kind(), WidgetKind::RadioButton);

        let slider = factory
            .create_by_kind(WidgetKind::Slider, rect, "")
            .expect("slider must be created by kind");
        assert_eq!(slider.kind(), WidgetKind::Slider);

        let tree = factory
            .create_by_kind(WidgetKind::TreeView, rect, "")
            .expect("tree view must be created by kind");
        assert_eq!(tree.kind(), WidgetKind::TreeView);

        let menubar = factory
            .create_by_kind(WidgetKind::MenuBar, rect, "")
            .expect("menu bar must be created by kind");
        assert_eq!(menubar.kind(), WidgetKind::MenuBar);

        let ribbon = factory
            .create_by_kind(WidgetKind::RibbonBar, rect, "")
            .expect("ribbon bar must be created by kind");
        assert_eq!(ribbon.kind(), WidgetKind::RibbonBar);

        let color_picker = factory
            .create_by_kind(WidgetKind::ColorDialog, rect, "")
            .expect("color picker must be created by kind");
        assert_eq!(color_picker.kind(), WidgetKind::ColorDialog);

        let code_editor = factory
            .create_by_kind(WidgetKind::RichEdit, rect, "fn main() {}")
            .expect("code editor must be created by kind");
        assert_eq!(code_editor.kind(), WidgetKind::RichEdit);

        let gantt = factory
            .create_by_kind(WidgetKind::Chart, rect, "")
            .expect("gantt widget must be created by kind");
        assert_eq!(gantt.kind(), WidgetKind::Chart);

        let terminal = factory
            .create_by_kind(WidgetKind::TextEdit, rect, "cmd")
            .expect("terminal view must be created by kind");
        assert_eq!(terminal.kind(), WidgetKind::TextEdit);

        let snackbar = factory
            .create_by_kind(WidgetKind::StatusBar, rect, "saved")
            .expect("snackbar must be created by kind");
        assert_eq!(snackbar.kind(), WidgetKind::StatusBar);

        let map = factory
            .create_by_kind(WidgetKind::Canvas, rect, "")
            .expect("map view must be created by kind");
        assert_eq!(map.kind(), WidgetKind::Canvas);

        let media = factory
            .create_by_kind(WidgetKind::WebView, rect, "")
            .expect("media player must be created by kind");
        assert_eq!(media.kind(), WidgetKind::WebView);

        let breadcrumb = factory
            .create_by_kind(WidgetKind::Panel, rect, "")
            .expect("breadcrumb must be created by kind");
        assert_eq!(breadcrumb.kind(), WidgetKind::Panel);

        let split_button = factory
            .create_by_kind(WidgetKind::ToolButton, rect, "Action")
            .expect("split button must be created by kind");
        assert_eq!(split_button.kind(), WidgetKind::ToolButton);

        let segmented_control = factory
            .create_by_kind(WidgetKind::ToggleButton, rect, "")
            .expect("segmented control must be created by kind");
        assert_eq!(segmented_control.kind(), WidgetKind::ToggleButton);

        let chip = factory
            .create_by_kind(WidgetKind::CheckListBox, rect, "")
            .expect("chip must be created by kind");
        assert_eq!(chip.kind(), WidgetKind::CheckListBox);

        let grid = factory
            .create_by_kind(WidgetKind::Grid, rect, "")
            .expect("grid must be created by kind");
        assert_eq!(grid.kind(), WidgetKind::Grid);

        let shape = factory
            .create_by_kind(WidgetKind::FreeformShape, rect, "")
            .expect("freeform shape must be created by kind");
        assert_eq!(shape.kind(), WidgetKind::FreeformShape);

        let progress = factory
            .create_by_kind(WidgetKind::ProgressBar, rect, "")
            .expect("progress bar must be created by kind");
        assert_eq!(progress.kind(), WidgetKind::ProgressBar);

        let scroll = factory
            .create_by_kind(WidgetKind::ScrollBar, rect, "")
            .expect("scroll bar must be created by kind");
        assert_eq!(scroll.kind(), WidgetKind::ScrollBar);

        let list_box = factory
            .create_by_kind(WidgetKind::ListBox, rect, "")
            .expect("list box must be created by kind");
        assert_eq!(list_box.kind(), WidgetKind::ListBox);

        let spin_box = factory
            .create_by_kind(WidgetKind::SpinBox, rect, "")
            .expect("spin box must be created by kind");
        assert_eq!(spin_box.kind(), WidgetKind::SpinBox);

        let combo_box = factory
            .create_by_kind(WidgetKind::ComboBox, rect, "")
            .expect("combo box must be created by kind");
        assert_eq!(combo_box.kind(), WidgetKind::ComboBox);

        let dial = factory
            .create_by_kind(WidgetKind::Dial, rect, "")
            .expect("dial must be created by kind");
        assert_eq!(dial.kind(), WidgetKind::Dial);

        let window = factory
            .create_by_kind(WidgetKind::Window, rect, "Main")
            .expect("window must be created by kind");
        assert_eq!(window.kind(), WidgetKind::Window);

        let group_box = factory
            .create_by_kind(WidgetKind::GroupBox, rect, "Options")
            .expect("group box must be created by kind");
        assert_eq!(group_box.kind(), WidgetKind::GroupBox);

        let splitter = factory
            .create_by_kind(WidgetKind::Splitter, rect, "")
            .expect("splitter must be created by kind");
        assert_eq!(splitter.kind(), WidgetKind::Splitter);

        let lcd_number = factory
            .create_by_kind(WidgetKind::LCDNumber, rect, "")
            .expect("lcd number must be created by kind");
        assert_eq!(lcd_number.kind(), WidgetKind::LCDNumber);

        let command_link = factory
            .create_by_kind(WidgetKind::CommandLink, rect, "Open")
            .expect("command link must be created by kind");
        assert_eq!(command_link.kind(), WidgetKind::CommandLink);

        let font_combo = factory
            .create_by_kind(WidgetKind::FontComboBox, rect, "")
            .expect("font combo box must be created by kind");
        assert_eq!(font_combo.kind(), WidgetKind::FontComboBox);

        let action = factory
            .create_by_kind(WidgetKind::Action, rect, "Run")
            .expect("action must be created by kind");
        assert_eq!(action.kind(), WidgetKind::Action);

        let tool_box = factory
            .create_by_kind(WidgetKind::ToolBox, rect, "")
            .expect("tool box must be created by kind");
        assert_eq!(tool_box.kind(), WidgetKind::ToolBox);

        let tab_bar = factory
            .create_by_kind(WidgetKind::TabBar, rect, "")
            .expect("tab bar must be created by kind");
        assert_eq!(tab_bar.kind(), WidgetKind::TabBar);

        let calendar = factory
            .create_by_kind(WidgetKind::Calendar, rect, "")
            .expect("calendar must be created by kind");
        assert_eq!(calendar.kind(), WidgetKind::Calendar);

        let date_edit = factory
            .create_by_kind(WidgetKind::DatePicker, rect, "")
            .expect("date edit must be created by kind");
        assert_eq!(date_edit.kind(), WidgetKind::DatePicker);

        let time_edit = factory
            .create_by_kind(WidgetKind::TimePicker, rect, "")
            .expect("time edit must be created by kind");
        assert_eq!(time_edit.kind(), WidgetKind::TimePicker);
    }

    #[test]
    fn read_property_returns_value_for_registered_widget() {
        let factory = WidgetFactory::new_with_defaults();
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 80));
        menu.add_action("Open");

        let title = factory.read_property(&menu, "title").expect("title should be readable");
        let item_count =
            factory.read_property(&menu, "item_count").expect("item_count should be readable");

        assert_eq!(title, CapabilityValue::String("File".to_string()));
        assert_eq!(item_count, CapabilityValue::UInt(1));
    }

    #[test]
    fn read_property_returns_unknown_property_for_missing_schema_item() {
        let factory = WidgetFactory::new_with_defaults();
        let menu = Menu::new("File", Rect::new(0, 0, 200, 80));
        let result = factory.read_property(&menu, "non_existing_property");
        assert_eq!(result, Err(CapabilityAccessError::UnknownProperty));
    }

    #[test]
    fn read_property_is_case_and_separator_insensitive() {
        let factory = WidgetFactory::new_with_defaults();
        let table = TableWidget::new(Rect::new(0, 0, 200, 120));
        let value = factory
            .read_property(&table, "HAS-MODEL")
            .expect("normalized property lookup should work");
        assert_eq!(value, CapabilityValue::Bool(false));

        let data_grid = DataGrid::new(Rect::new(0, 0, 200, 120));
        let data_grid_value = factory
            .read_property(&data_grid, "ROW-HEIGHT")
            .expect("data grid should resolve to dedicated capability profile");
        assert_eq!(data_grid_value, CapabilityValue::UInt(20));
    }

    #[test]
    fn write_property_updates_mutable_scalar_fields() {
        let factory = WidgetFactory::new_with_defaults();

        let mut button = Button::new("Run".to_string(), Rect::new(0, 0, 120, 30));
        factory
            .write_property(&mut button, "text", CapabilityValue::String("Stop".to_string()))
            .expect("button text should be writable");
        factory
            .write_property(&mut button, "enabled", CapabilityValue::Bool(false))
            .expect("button enabled should be writable");
        assert_eq!(button.text(), "Stop");
        assert!(!button.is_enabled());

        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 80));
        factory
            .write_property(&mut menu, "title", CapabilityValue::String("Tools".to_string()))
            .expect("menu title should be writable");
        assert_eq!(menu.title(), "Tools");

        let mut toolbar = ToolBar::new(Rect::new(0, 0, 200, 40));
        factory
            .write_property(&mut toolbar, "movable", CapabilityValue::Bool(false))
            .expect("toolbar movable should be writable");
        assert!(!toolbar.is_movable());
    }

    #[test]
    fn write_property_reports_readonly_property() {
        let factory = WidgetFactory::new_with_defaults();
        let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));

        let result = factory.write_property(&mut table, "has_model", CapabilityValue::Bool(true));
        assert_eq!(result, Err(CapabilityAccessError::ReadOnlyProperty));
    }

    #[test]
    fn write_property_reports_type_mismatch() {
        let factory = WidgetFactory::new_with_defaults();
        let mut button = Button::new("Run".to_string(), Rect::new(0, 0, 120, 30));

        let result = factory.write_property(
            &mut button,
            "enabled",
            CapabilityValue::String("true".to_string()),
        );
        assert_eq!(result, Err(CapabilityAccessError::TypeMismatch));
    }

    #[test]
    fn read_property_covers_declared_scalar_fields() {
        let factory = WidgetFactory::new_with_defaults();

        let mut button = Button::new("Run".to_string(), Rect::new(0, 0, 120, 30));
        button.set_default(true);
        assert_eq!(
            factory.read_property(&button, "text"),
            Ok(CapabilityValue::String("Run".to_string()))
        );
        assert_eq!(factory.read_property(&button, "default"), Ok(CapabilityValue::Bool(true)));

        let mut line_edit = LineEdit::new(Rect::new(0, 0, 120, 30));
        line_edit.set_text("abc".to_string());
        line_edit.set_max_length(Some(32));
        line_edit.set_read_only(true);
        assert_eq!(
            factory.read_property(&line_edit, "text"),
            Ok(CapabilityValue::String("abc".to_string()))
        );
        assert_eq!(factory.read_property(&line_edit, "max_length"), Ok(CapabilityValue::UInt(32)));
        assert_eq!(factory.read_property(&line_edit, "read_only"), Ok(CapabilityValue::Bool(true)));
    }

    #[test]
    fn read_property_returns_null_for_optional_projection_fields() {
        let factory = WidgetFactory::new_with_defaults();
        let list = ListView::new(Rect::new(0, 0, 160, 100));
        let tree = TreeView::new(Rect::new(0, 0, 160, 100));
        let menu = Menu::new("File", Rect::new(0, 0, 160, 100));
        let menubar = MenuBar::new(Rect::new(0, 0, 240, 24));

        assert_eq!(factory.read_property(&list, "focused_row"), Ok(CapabilityValue::Null));
        assert_eq!(factory.read_property(&tree, "focused_node"), Ok(CapabilityValue::Null));
        assert_eq!(factory.read_property(&tree, "selected_node"), Ok(CapabilityValue::Null));
        assert_eq!(factory.read_property(&menu, "hovered_index"), Ok(CapabilityValue::Null));
        assert_eq!(factory.read_property(&menubar, "active_index"), Ok(CapabilityValue::Null));
    }

    #[test]
    fn write_property_supports_enum_backed_fields() {
        let factory = WidgetFactory::new_with_defaults();

        let mut list = ListView::new(Rect::new(0, 0, 160, 100));
        factory
            .write_property(
                &mut list,
                "selection_mode",
                CapabilityValue::String("multi".to_string()),
            )
            .expect("list selection mode should be writable");
        factory
            .write_property(&mut list, "view_mode", CapabilityValue::String("details".to_string()))
            .expect("list view mode should be writable");
        assert_eq!(
            factory.read_property(&list, "selection_mode"),
            Ok(CapabilityValue::String("multi".to_string()))
        );
        assert_eq!(
            factory.read_property(&list, "view_mode"),
            Ok(CapabilityValue::String("details".to_string()))
        );

        let mut table = TableWidget::new(Rect::new(0, 0, 200, 120));
        factory
            .write_property(
                &mut table,
                "selection_mode",
                CapabilityValue::String("extended".to_string()),
            )
            .expect("table selection mode should be writable");
        assert_eq!(
            factory.read_property(&table, "selection_mode"),
            Ok(CapabilityValue::String("extended".to_string()))
        );

        let mut toolbar = ToolBar::new(Rect::new(0, 0, 220, 30));
        factory
            .write_property(
                &mut toolbar,
                "orientation",
                CapabilityValue::String("vertical".to_string()),
            )
            .expect("toolbar orientation should be writable");
        assert_eq!(
            factory.read_property(&toolbar, "orientation"),
            Ok(CapabilityValue::String("vertical".to_string()))
        );
    }

    #[test]
    fn write_property_supports_r3_data_controls() {
        let factory = WidgetFactory::new_with_defaults();

        let mut data_grid = DataGrid::new(Rect::new(0, 0, 260, 120));
        factory
            .write_property(&mut data_grid, "row_height", CapabilityValue::UInt(28))
            .expect("data grid row_height should be writable");
        factory
            .write_property(&mut data_grid, "frozen_columns", CapabilityValue::UInt(2))
            .expect("data grid frozen_columns should be writable");
        factory
            .write_property(
                &mut data_grid,
                "sort_specs",
                CapabilityValue::String("1:desc,3:asc".to_string()),
            )
            .expect("data grid sort_specs should be writable");
        factory
            .write_property(
                &mut data_grid,
                "filters",
                CapabilityValue::String("0=alpha,2=beta".to_string()),
            )
            .expect("data grid filters should be writable");
        assert_eq!(factory.read_property(&data_grid, "row_height"), Ok(CapabilityValue::UInt(28)));
        assert_eq!(
            factory.read_property(&data_grid, "frozen_columns"),
            Ok(CapabilityValue::UInt(0))
        );
        assert_eq!(
            factory.read_property(&data_grid, "sort_spec_count"),
            Ok(CapabilityValue::UInt(2))
        );
        assert_eq!(factory.read_property(&data_grid, "filter_count"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(
            factory.read_property(&data_grid, "sort_specs"),
            Ok(CapabilityValue::String("1:desc,3:asc".to_string()))
        );
        assert_eq!(
            factory.read_property(&data_grid, "filters"),
            Ok(CapabilityValue::String("0=alpha,2=beta".to_string()))
        );

        let mut tree_table = TreeTable::new(Rect::new(0, 0, 260, 120));
        factory
            .write_property(&mut tree_table, "column_width", CapabilityValue::UInt(180))
            .expect("tree table column_width should be writable");
        assert_eq!(
            factory.read_property(&tree_table, "column_width"),
            Ok(CapabilityValue::UInt(180))
        );

        factory
            .write_property(&mut tree_table, "selected_row", CapabilityValue::UInt(0))
            .expect("tree table selected_row should be writable");
        assert_eq!(factory.read_property(&tree_table, "selected_row"), Ok(CapabilityValue::Null));

        let mut virtual_table = VirtualTable::new(Rect::new(0, 0, 260, 120));
        factory
            .write_property(&mut virtual_table, "scroll_row", CapabilityValue::UInt(10))
            .expect("virtual table scroll_row should be writable");
        factory
            .write_property(&mut virtual_table, "row_height", CapabilityValue::UInt(26))
            .expect("virtual table row_height should be writable");
        factory
            .write_property(&mut virtual_table, "column_width", CapabilityValue::UInt(140))
            .expect("virtual table column_width should be writable");
        factory
            .write_property(&mut virtual_table, "overscan_rows", CapabilityValue::UInt(4))
            .expect("virtual table overscan_rows should be writable");
        factory
            .write_property(&mut virtual_table, "overscan_columns", CapabilityValue::UInt(3))
            .expect("virtual table overscan_columns should be writable");
        assert_eq!(
            factory.read_property(&virtual_table, "scroll_row"),
            Ok(CapabilityValue::UInt(0))
        );
        assert_eq!(
            factory.read_property(&virtual_table, "row_height"),
            Ok(CapabilityValue::UInt(26))
        );
        assert_eq!(
            factory.read_property(&virtual_table, "column_width"),
            Ok(CapabilityValue::UInt(140))
        );
        assert_eq!(
            factory.read_property(&virtual_table, "overscan_rows"),
            Ok(CapabilityValue::UInt(4))
        );
        assert_eq!(
            factory.read_property(&virtual_table, "overscan_columns"),
            Ok(CapabilityValue::UInt(3))
        );
    }

    #[test]
    fn write_property_accepts_null_for_optional_focus_fields() {
        let factory = WidgetFactory::new_with_defaults();

        let mut list = ListView::new(Rect::new(0, 0, 160, 100));
        factory
            .write_property(&mut list, "focused_row", CapabilityValue::Null)
            .expect("list focused_row should accept Null");
        assert_eq!(factory.read_property(&list, "focused_row"), Ok(CapabilityValue::Null));

        let mut tree = TreeView::new(Rect::new(0, 0, 160, 100));
        factory
            .write_property(&mut tree, "focused_node", CapabilityValue::Null)
            .expect("tree focused_node should accept Null");
        assert_eq!(factory.read_property(&tree, "focused_node"), Ok(CapabilityValue::Null));
    }

    #[test]
    fn schema_defaults_are_readable_and_writable_when_declared() {
        let factory = WidgetFactory::new_with_defaults();

        for capability in factory.capabilities() {
            let mut widget = factory
                .create(capability.canonical_name, Rect::new(0, 0, 200, 120), "")
                .expect("registered capability should be constructible");

            for property in capability.properties {
                let default = factory
                    .default_property_value(capability.canonical_name, property.name)
                    .expect("declared property should have default value");

                if property.readable {
                    let read_result = factory.read_property(widget.as_ref(), property.name);
                    assert!(
                        read_result.is_ok(),
                        "{}:{} should be readable",
                        capability.canonical_name,
                        property.name
                    );
                }

                if property.writable {
                    let write_result =
                        factory.write_property(widget.as_mut(), property.name, default.clone());
                    assert!(
                        write_result.is_ok(),
                        "{}:{} should accept schema default write",
                        capability.canonical_name,
                        property.name
                    );
                }
            }
        }
    }

    #[test]
    fn default_property_value_returns_schema_defaults() {
        let factory = WidgetFactory::new_with_defaults();

        assert_eq!(
            factory.default_property_value("button", "enabled"),
            Ok(CapabilityValue::Bool(true))
        );
        assert_eq!(
            factory.default_property_value("label", "alignment"),
            Ok(CapabilityValue::String("left".to_string()))
        );
        assert_eq!(
            factory.default_property_value("checkbox", "state"),
            Ok(CapabilityValue::String("unchecked".to_string()))
        );
        assert_eq!(
            factory.default_property_value("radiobutton", "checked"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(
            factory.default_property_value("slider", "maximum"),
            Ok(CapabilityValue::Int(100))
        );
        assert_eq!(
            factory.default_property_value("line_edit", "max_length"),
            Ok(CapabilityValue::Null)
        );
        assert_eq!(
            factory.default_property_value("listview", "selection_mode"),
            Ok(CapabilityValue::String("single".to_string()))
        );
        assert_eq!(
            factory.default_property_value("toolbar", "orientation"),
            Ok(CapabilityValue::String("horizontal".to_string()))
        );
        assert_eq!(
            factory.default_property_value("color_picker", "show_alpha"),
            Ok(CapabilityValue::Bool(true))
        );
        assert_eq!(
            factory.default_property_value("gantt", "viewport_end"),
            Ok(CapabilityValue::Int(100))
        );
        assert_eq!(
            factory.default_property_value("terminalview", "output_line_count"),
            Ok(CapabilityValue::UInt(0))
        );
        assert_eq!(
            factory.default_property_value("snackbar", "visible"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(
            factory.default_property_value("mapview", "zoom"),
            Ok(CapabilityValue::Float(1.0))
        );
        assert_eq!(
            factory.default_property_value("mediaplayer", "volume"),
            Ok(CapabilityValue::UInt(80))
        );
        assert_eq!(
            factory.default_property_value("breadcrumb", "segment_count"),
            Ok(CapabilityValue::UInt(0))
        );
        assert_eq!(
            factory.default_property_value("splitbutton", "menu_open"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(
            factory.default_property_value("segmentedcontrol", "item_count"),
            Ok(CapabilityValue::UInt(0))
        );
        assert_eq!(
            factory.default_property_value("chip", "multi_select"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(factory.default_property_value("grid", "rows"), Ok(CapabilityValue::UInt(1)));
        assert_eq!(
            factory.default_property_value("freeformshape", "stroke_width"),
            Ok(CapabilityValue::UInt(2))
        );
        assert_eq!(
            factory.default_property_value("progressbar", "orientation"),
            Ok(CapabilityValue::String("horizontal".to_string()))
        );
        assert_eq!(
            factory.default_property_value("scrollbar", "single_step"),
            Ok(CapabilityValue::Int(1))
        );
        assert_eq!(
            factory.default_property_value("listbox", "selection_mode"),
            Ok(CapabilityValue::String("single".to_string()))
        );
        assert_eq!(
            factory.default_property_value("spinbox", "maximum"),
            Ok(CapabilityValue::Int(99))
        );
        assert_eq!(
            factory.default_property_value("combobox", "max_visible_items"),
            Ok(CapabilityValue::UInt(10))
        );
        assert_eq!(
            factory.default_property_value("dial", "notch_target"),
            Ok(CapabilityValue::Float(3.7))
        );
        assert_eq!(
            factory.default_property_value("window", "title_bar_height"),
            Ok(CapabilityValue::UInt(32))
        );
        assert_eq!(
            factory.default_property_value("groupbox", "checked"),
            Ok(CapabilityValue::Bool(true))
        );
        assert_eq!(
            factory.default_property_value("splitter", "orientation"),
            Ok(CapabilityValue::String("horizontal".to_string()))
        );
        assert_eq!(
            factory.default_property_value("lcdnumber", "mode"),
            Ok(CapabilityValue::String("dec".to_string()))
        );
        assert_eq!(
            factory.default_property_value("commandlink", "enabled"),
            Ok(CapabilityValue::Bool(true))
        );
        assert_eq!(
            factory.default_property_value("fontcombobox", "current_index"),
            Ok(CapabilityValue::Int(-1))
        );
        assert_eq!(
            factory.default_property_value("action", "checkable"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(
            factory.default_property_value("toolbox", "orientation"),
            Ok(CapabilityValue::String("vertical".to_string()))
        );
        assert_eq!(
            factory.default_property_value("tabbar", "tab_min_width"),
            Ok(CapabilityValue::UInt(40))
        );
        assert_eq!(
            factory.default_property_value("calendar", "first_day_of_week"),
            Ok(CapabilityValue::String("mon".to_string()))
        );
        assert_eq!(
            factory.default_property_value("dateedit", "display_format"),
            Ok(CapabilityValue::String("yyyy-MM-dd".to_string()))
        );
        assert_eq!(
            factory.default_property_value("timeedit", "display_format"),
            Ok(CapabilityValue::String("HH:mm:ss".to_string()))
        );
    }

    #[test]
    fn property_schema_lookup_is_normalized() {
        let factory = WidgetFactory::new_with_defaults();

        let schema = factory
            .property_schema("line-edit", "max length")
            .expect("normalized schema lookup should succeed");
        assert_eq!(schema.name, "max_length");
        assert_eq!(schema.value_kind, PropertyValueKind::UInt);
        assert!(schema.readable);
        assert!(schema.writable);
    }

    #[test]
    fn capability_manifest_exports_defaults_and_metadata() {
        let factory = WidgetFactory::new_with_defaults();

        let manifest =
            factory.capability_manifest("table").expect("table manifest should be exportable");

        assert_eq!(manifest.kind, WidgetKind::Table);
        assert_eq!(manifest.canonical_name, "table_widget");
        assert!(manifest.aliases.contains(&"table"));
        assert!(manifest.events.contains(&"selection_changed"));
        assert!(manifest.commands.contains(&"clear_selection"));

        let has_model = manifest
            .properties
            .iter()
            .find(|entry| entry.schema.name == "has_model")
            .expect("has_model schema should exist");
        assert_eq!(has_model.default_value, CapabilityValue::Bool(false));

        let selection_mode = manifest
            .properties
            .iter()
            .find(|entry| entry.schema.name == "selection_mode")
            .expect("selection_mode schema should exist");
        assert_eq!(selection_mode.default_value, CapabilityValue::String("single".to_string()));
    }

    #[test]
    fn virtual_list_capability_read_write_roundtrip() {
        let factory = WidgetFactory::new_with_defaults();
        let mut list = VirtualList::new(Rect::new(0, 0, 120, 60));

        assert_eq!(
            factory.read_property(&list, "has_data_source"),
            Ok(CapabilityValue::Bool(false))
        );
        assert_eq!(factory.read_property(&list, "selected_row"), Ok(CapabilityValue::Null));

        factory
            .write_property(&mut list, "row_height", CapabilityValue::UInt(32))
            .expect("row_height should be writable");
        factory
            .write_property(&mut list, "overscan", CapabilityValue::UInt(4))
            .expect("overscan should be writable");
        factory
            .write_property(&mut list, "scroll_row", CapabilityValue::UInt(7))
            .expect("scroll_row should be writable");

        assert_eq!(factory.read_property(&list, "row_height"), Ok(CapabilityValue::UInt(32)));
        assert_eq!(factory.read_property(&list, "overscan"), Ok(CapabilityValue::UInt(4)));
        // Without a data source, scroll_row is normalized back to 0.
        assert_eq!(factory.read_property(&list, "scroll_row"), Ok(CapabilityValue::UInt(0)));
    }
}
