// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

// The factory and its supporting imports exist only where the full control set
// does; see the module's `cfg` section below for why the property *contract* is
// separate.
#[cfg(widgets_unstripped)]
use std::collections::HashMap;

#[cfg(widgets_unstripped)]
use crate::core::Rect;

#[cfg(widgets_unstripped)]
use super::{Widget, WidgetKind};
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::DataGrid;
#[cfg(full_widgets)]
use crate::widget::view_widgets::table_widget::TableWidget;
#[cfg(full_widgets)]
use crate::widget::view_widgets::tree_table::TreeTable;
#[cfg(full_widgets)]
use crate::widget::view_widgets::tree_view::TreeView;
#[cfg(full_widgets)]
use crate::widget::view_widgets::virtual_table::VirtualTable;

pub mod types;
pub use types::*;

/// The per-control property contract (`WidgetProperties`).
///
/// Compiled in every profile: `Widget::properties_dyn` returns this trait, and
/// `Widget` exists even where the factory does not. Replaces the centralised
/// `match widget.kind()` dispatch — a control implements its own `get` / `set` /
/// `property_names` in its own file, and forwards the properties every control
/// shares to `base_property_get` / `base_property_set`.
pub mod properties_trait;
pub use properties_trait::{
    base_property_get, base_property_set, geometry_to_value, widget_property_get,
    widget_property_names, widget_property_set, WidgetProperties, BASE_PROPERTY_NAMES,
};

/// The `WidgetFactory` name under which `kind` is registered.
///
/// Derived from the capability registry — the same table [`WidgetFactory::create`]
/// dispatches on — rather than from `Debug` output, so the two cannot disagree.
///
/// Returns `None` for a kind with no registered capability. That is the honest
/// answer: such a kind has no constructor, so a caller must not be handed a
/// plausible-looking name that the factory would reject anyway.
#[cfg(widgets_unstripped)]
pub fn factory_name_for_kind(kind: crate::widget::WidgetKind) -> Option<&'static str> {
    WidgetFactory::new_with_defaults()
        .capability_by_kind(kind)
        .map(|capability| capability.canonical_name)
}

// ── Profile-specific parts ──────────────────────────────────────────────────
//
// Everything below describes the *factory* and the legacy centralised access
// layer. Both enumerate concrete controls (`Button`, `Calendar`, …), which only
// exist when the full widget set is compiled, so they stay gated on the device
// profiles. The property *contract* above is deliberately outside this gate.
/// Type-coercion helpers used by every control's property writers.
///
/// Compiled in every profile: the `expect_*` helpers are primitives (`expect_bool`,
/// `expect_string`, …) that a control needs wherever it exists. The helpers for
/// profile-specific types (`Date`, `Time`, `SortSpec`, …) are individually gated
/// on `full_widgets` inside the file, so nothing profile-specific leaks out here.
pub mod coercion;
pub use coercion::*;

#[cfg(widgets_unstripped)]
pub mod constructors;
#[cfg(widgets_unstripped)]
pub use constructors::*;

#[cfg(widgets_unstripped)]
pub mod properties;
#[cfg(widgets_unstripped)]
pub(crate) use properties::*;

#[cfg(widgets_unstripped)]
pub mod access;
#[cfg(widgets_unstripped)]
pub use access::*;

#[cfg(widgets_unstripped)]
pub mod registration;

#[cfg(all(test, widgets_unstripped))]
pub mod tests;

/// Contract tests for the per-control property layer (BLUE15 Phase C-1).
///
/// Exercises every widget category so a category-wide mistake cannot hide behind
/// one well-behaved control.
#[cfg(all(test, full_widgets))]
mod properties_tests;

/// Default construction for [`WidgetFactory`].
///
/// Gated with the factory itself: the factory enumerates concrete controls, which
/// only exist when a device profile is compiled. The property contract above is
/// independent of this and available in every profile.
#[cfg(widgets_unstripped)]
impl Default for WidgetFactory {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

/// Runtime widget factory: builds a control by name or kind.
///
/// See the module docs; gated with the profile-specific control set, because every
/// constructor it registers names a concrete control type.
#[cfg(widgets_unstripped)]
impl WidgetFactory {
    /// Creates an empty factory.
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
            key_to_index: HashMap::new(),
            kind_to_index: HashMap::new(),
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
        self.kind_to_index.entry(capability.kind).or_default().push(idx);

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
    /// Returns the first registered capability for this kind.
    pub fn capability_by_kind(&self, kind: WidgetKind) -> Option<&WidgetCapability> {
        let indices = self.kind_to_index.get(&kind)?;
        self.capabilities.get(indices[0])
    }

    /// Returns all registered capabilities.
    pub fn capabilities(&self) -> &[WidgetCapability] {
        &self.capabilities
    }

    /// Returns the canonical name of every registered widget.
    ///
    /// Derived from [`Self::capabilities`] rather than kept as a second list, so
    /// a widget cannot be constructible yet absent from this enumeration — which
    /// is what lets coverage tests walk **every** widget instead of a hand-typed
    /// sample. (The sample is exactly how the painting bridge drifted to 6 of
    /// 168: see `crate::widget::draw_bridge`.)
    pub fn widget_names(&self) -> Vec<&'static str> {
        self.capabilities.iter().map(|capability| capability.canonical_name).collect()
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

    /// Looks up capability by widget kind using the kind-based index.
    /// When multiple capabilities share the same WidgetKind (e.g. DataGrid,
    /// VirtualTable, TableWidget all use WidgetKind::Table), tries to match
    /// by concrete widget type.
    fn capability_for_widget(&self, widget: &dyn Widget) -> Option<&WidgetCapability> {
        let kind = widget.kind();
        let indices = self.kind_to_index.get(&kind)?;

        // Fast path: only one capability for this kind
        if indices.len() == 1 {
            return self.capabilities.get(indices[0]);
        }

        // Multiple capabilities share this kind — find the right one by
        // matching the concrete widget type against the canonical name.
        for &idx in indices.iter() {
            let cap = &self.capabilities[idx];
            if self.widget_matches_capability(widget, cap.canonical_name) {
                return Some(cap);
            }
        }

        // Fallback: return the first registered capability
        self.capabilities.get(indices[0])
    }

    /// Check whether a widget instance matches a given capability's concrete type.
    fn widget_matches_capability(&self, widget: &dyn Widget, canonical_name: &str) -> bool {
        #[cfg(alloc_frugal)]
        {
            let _ = widget;
            let _ = canonical_name;
            true
        }
        #[cfg(full_widgets)]
        match canonical_name {
            #[cfg(full_widgets)]
            "data_grid" => self::coercion::widget_as::<DataGrid>(widget).is_some(),
            #[cfg(full_widgets)]
            "virtual_table" => self::coercion::widget_as::<VirtualTable>(widget).is_some(),
            #[cfg(full_widgets)]
            "table_widget" => self::coercion::widget_as::<TableWidget>(widget).is_some(),
            #[cfg(full_widgets)]
            "tree_table" => self::coercion::widget_as::<TreeTable>(widget).is_some(),
            #[cfg(full_widgets)]
            "tree_view" => self::coercion::widget_as::<TreeView>(widget).is_some(),
            _ => true,
        }
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
}
