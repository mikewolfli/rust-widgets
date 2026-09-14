// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Imports for the factory, which only exists where the full control set does.
#[cfg(widgets_unstripped)]
use std::collections::HashMap;

#[cfg(widgets_unstripped)]
use crate::core::Rect;
#[cfg(widgets_unstripped)]
use crate::widget::Widget;
use crate::widget::WidgetKind;

/// Runtime property value returned by capability-based reflection APIs.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    String(String),
}

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

/// One property entry in exported capability manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityPropertyManifest {
    pub schema: PropertySchema,
    pub default_value: CapabilityValue,
}

/// Exportable snapshot for one widget capability.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetCapabilityManifest {
    pub kind: WidgetKind,
    pub canonical_name: &'static str,
    pub aliases: Vec<&'static str>,
    pub properties: Vec<CapabilityPropertyManifest>,
    pub events: Vec<&'static str>,
    pub commands: Vec<&'static str>,
}

/// Constructor signature the factory registers for each control.
#[cfg(widgets_unstripped)]
pub(crate) type WidgetCtor = fn(Rect, &str) -> Box<dyn Widget>;

/// Factory + metadata registry for dynamic widget instantiation.
///
/// Gated with the full control set: the factory registers a constructor per
/// concrete control type, so it only exists where those types do. The
/// [property contract](crate::widget::capability::properties_trait::WidgetProperties)
/// is independent of it and available in every profile.
#[cfg(widgets_unstripped)]
pub struct WidgetFactory {
    pub(crate) capabilities: Vec<WidgetCapability>,
    pub(crate) key_to_index: HashMap<String, usize>,
    pub(crate) kind_to_index: HashMap<WidgetKind, Vec<usize>>,
    pub(crate) constructors: HashMap<String, WidgetCtor>,
}
