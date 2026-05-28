//! Widget capability metadata and runtime factory.
//!
//! This module provides:
//! - A lightweight schema describing widget properties/events/commands.
//! - A registry-based widget factory that can construct widgets by kind/name.

use std::collections::HashMap;

use crate::core::Rect;

use super::{
    advanced_widgets::ribbon_bar::RibbonBar,
    base_widgets::button::Button, input_widgets::lineedit::LineEdit, menu_toolbar::menu::Menu,
    menu_toolbar::menu_bar::MenuBar, menu_toolbar::tool_bar::ToolBar,
    view_widgets::list_view::ListView, view_widgets::table_widget::TableWidget,
    view_widgets::tree_view::TreeView, Widget, WidgetKind,
};

/// Runtime property value returned by capability-based reflection APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityValue {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
}

/// Errors emitted by capability-based reflection APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAccessError {
    UnknownWidget,
    UnknownProperty,
    ReadOnlyProperty,
    TypeMismatch,
    UnsupportedOnWidget,
}

/// Primitive property value kinds used by capability metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyValueKind {
    Bool,
    Int,
    UInt,
    Float,
    String,
    Enum,
}

/// Metadata for one readable/writable property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySchema {
    pub name: &'static str,
    pub value_kind: PropertyValueKind,
    pub readable: bool,
    pub writable: bool,
}

/// Capability metadata for a widget kind.
#[derive(Debug, Clone)]
pub struct WidgetCapability {
    pub kind: WidgetKind,
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    pub properties: &'static [PropertySchema],
    pub events: &'static [&'static str],
    pub commands: &'static [&'static str],
}

type WidgetCtor = fn(Rect, &str) -> Box<dyn Widget>;

/// Factory + metadata registry for dynamic widget instantiation.
pub struct WidgetFactory {
    capabilities: Vec<WidgetCapability>,
    key_to_index: HashMap<String, usize>,
    kind_to_index: Vec<(WidgetKind, usize)>,
    constructors: HashMap<String, WidgetCtor>,
}

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
        if let Some((_, stored_idx)) = self
            .kind_to_index
            .iter_mut()
            .find(|(kind, _)| *kind == capability.kind)
        {
            *stored_idx = idx;
        } else {
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
    pub fn create(&self, kind_or_name: &str, geometry: Rect, text: &str) -> Option<Box<dyn Widget>> {
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
        let capability = self
            .capability_by_kind(widget.kind())
            .ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) = capability
            .properties
            .iter()
            .find(|schema| normalize_key(schema.name) == normalized)
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
        let capability = self
            .capability_by_kind(widget.kind())
            .ok_or(CapabilityAccessError::UnknownWidget)?;

        let normalized = normalize_key(property_name);
        let Some(property) = capability
            .properties
            .iter()
            .find(|schema| normalize_key(schema.name) == normalized)
        else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        if !property.writable {
            return Err(CapabilityAccessError::ReadOnlyProperty);
        }

        write_widget_property_value(widget, property.name, value)
    }

    fn register_core_widgets(&mut self) {
        self.register(button_capability(), create_button);
        self.register(line_edit_capability(), create_line_edit);
        self.register(list_view_capability(), create_list_view);
        self.register(tree_view_capability(), create_tree_view);
        self.register(table_widget_capability(), create_table_widget);
        self.register(menu_capability(), create_menu);
        self.register(menu_bar_capability(), create_menu_bar);
        self.register(tool_bar_capability(), create_tool_bar);
        self.register(ribbon_bar_capability(), create_ribbon_bar);
    }
}

fn read_widget_property_value(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::Button => match property_name {
            "enabled" => Ok(CapabilityValue::Bool(widget.is_enabled())),
            "tooltip" => Ok(CapabilityValue::String(widget.tooltip().to_string())),
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LineEdit => match property_name {
            "enabled" => Ok(CapabilityValue::Bool(widget.is_enabled())),
            "tooltip" => Ok(CapabilityValue::String(widget.tooltip().to_string())),
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
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::TreeView => match property_name {
            "has_model" => {
                if let Some(tree_view) = widget_as::<TreeView>(widget) {
                    Ok(CapabilityValue::Bool(tree_view.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "node_count" => {
                if let Some(tree_view) = widget_as::<TreeView>(widget) {
                    Ok(CapabilityValue::UInt(tree_view.node_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::Table => match property_name {
            "has_model" => {
                if let Some(table_widget) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::Bool(table_widget.has_model()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "has_delegate" => {
                if let Some(table_widget) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::Bool(table_widget.has_delegate()))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "row_count" => {
                if let Some(table_widget) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::UInt(table_widget.row_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            "column_count" => {
                if let Some(table_widget) = widget_as::<TableWidget>(widget) {
                    Ok(CapabilityValue::UInt(table_widget.column_count() as u64))
                } else {
                    Err(CapabilityAccessError::UnsupportedOnWidget)
                }
            }
            _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
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
                        line_edit.set_max_length(Some(expect_usize(value)?));
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
                    "focused_row" => {
                        let row = expect_usize(value)?;
                        if list_view.set_focused_row(row) {
                            Ok(())
                        } else {
                            Err(CapabilityAccessError::UnsupportedOnWidget)
                        }
                    }
                    _ => Err(CapabilityAccessError::UnsupportedOnWidget),
                }
            } else {
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
        }
        WidgetKind::TreeView => {
            if let Some(tree_view) = widget_as_mut::<TreeView>(widget) {
                match property_name {
                    "focused_node" => {
                        let node = expect_usize(value)?;
                        if tree_view.set_focused_node(node) {
                            Ok(())
                        } else {
                            Err(CapabilityAccessError::UnsupportedOnWidget)
                        }
                    }
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
        CapabilityValue::UInt(v) => usize::try_from(v).map_err(|_| CapabilityAccessError::TypeMismatch),
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
    PropertySchema {
        name: "menu_enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
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
    PropertySchema {
        name: "item_enabled",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
    },
    PropertySchema {
        name: "item_checked",
        value_kind: PropertyValueKind::Bool,
        readable: true,
        writable: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_factory_registers_core_capabilities() {
        let factory = WidgetFactory::new_with_defaults();
        assert_eq!(factory.capabilities().len(), 9);
        assert!(factory.capability("button").is_some());
        assert!(factory.capability("lineedit").is_some());
        assert!(factory.capability("list_view").is_some());
        assert!(factory.capability("treeview").is_some());
        assert!(factory.capability("table").is_some());
        assert!(factory.capability("menu").is_some());
        assert!(factory.capability("menubar").is_some());
        assert!(factory.capability("toolbar").is_some());
        assert!(factory.capability("ribbon").is_some());
    }

    #[test]
    fn factory_creates_registered_widgets_by_alias() {
        let factory = WidgetFactory::new_with_defaults();
        let rect = Rect::new(1, 2, 120, 40);

        let button = factory
            .create("btn", rect, "Run")
            .expect("button must be created via alias");
        assert_eq!(button.kind(), WidgetKind::Button);
        assert_eq!(button.geometry(), rect);

        let line_edit = factory
            .create("input", rect, "hello")
            .expect("line edit must be created via alias");
        assert_eq!(line_edit.kind(), WidgetKind::LineEdit);

        let table = factory
            .create("table", rect, "")
            .expect("table widget must be created via alias");
        assert_eq!(table.kind(), WidgetKind::Table);

        let tree = factory
            .create("treeview", rect, "")
            .expect("tree view must be created via alias");
        assert_eq!(tree.kind(), WidgetKind::TreeView);

        let ribbon = factory
            .create("ribbon", rect, "")
            .expect("ribbon bar must be created via alias");
        assert_eq!(ribbon.kind(), WidgetKind::RibbonBar);
    }

    #[test]
    fn capability_by_kind_returns_expected_schema() {
        let factory = WidgetFactory::new_with_defaults();
        let table_cap = factory
            .capability_by_kind(WidgetKind::Table)
            .expect("table capability must exist");

        assert_eq!(table_cap.canonical_name, "table_widget");
        assert!(table_cap.properties.iter().any(|p| p.name == "has_model"));
        assert!(table_cap.properties.iter().any(|p| p.name == "has_delegate"));
        assert!(table_cap.events.contains(&"selection_changed"));
    }

    #[test]
    fn create_unknown_widget_returns_none() {
        let factory = WidgetFactory::new_with_defaults();
        assert!(factory
            .create("not_registered", Rect::new(0, 0, 1, 1), "")
            .is_none());
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
    }

    #[test]
    fn read_property_returns_value_for_registered_widget() {
        let factory = WidgetFactory::new_with_defaults();
        let mut menu = Menu::new("File", Rect::new(0, 0, 200, 80));
        menu.add_action("Open");

        let title = factory
            .read_property(&menu, "title")
            .expect("title should be readable");
        let item_count = factory
            .read_property(&menu, "item_count")
            .expect("item_count should be readable");

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
            .write_property(
                &mut menu,
                "title",
                CapabilityValue::String("Tools".to_string()),
            )
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
}
