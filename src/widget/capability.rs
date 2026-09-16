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
#[cfg(full_widgets)]
use std::collections::HashMap;

#[cfg(full_widgets)]
use crate::core::Rect;

#[cfg(full_widgets)]
use super::{Widget, WidgetKind};
#[cfg(full_widgets)]
use crate::widget::base_widgets::toggle_button::ToggleButton;
#[cfg(full_widgets)]
use crate::widget::input_widgets::rich_edit::RichEdit;
#[cfg(full_widgets)]
use crate::widget::input_widgets::textedit::TextEdit;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::status_bar::StatusBar;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::tool_button::ToolButton;
#[cfg(full_widgets)]
use crate::widget::special_widgets::canvas::Canvas;
#[cfg(full_widgets)]
use crate::widget::special_widgets::chart::ChartWidget;
#[cfg(full_widgets)]
use crate::widget::special_widgets::code_editor::CodeEditor;
#[cfg(full_widgets)]
use crate::widget::special_widgets::gantt_widget::GanttWidget;
#[cfg(full_widgets)]
use crate::widget::special_widgets::map_view::MapView;
#[cfg(full_widgets)]
use crate::widget::special_widgets::media_player::MediaPlayer;
#[cfg(full_widgets)]
use crate::widget::special_widgets::segmented_control::SegmentedControl;
#[cfg(full_widgets)]
use crate::widget::special_widgets::snackbar::Snackbar;
#[cfg(full_widgets)]
use crate::widget::special_widgets::split_button::SplitButton;
#[cfg(full_widgets)]
use crate::widget::special_widgets::terminal_view::TerminalView;
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
#[cfg(full_widgets)]
use crate::widget::web_widgets::web_view::WebView;

/// Shared capability value and error types (`CapabilityValue`,
/// `CapabilityAccessError`, …) exchanged through the property contract below.
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
    base_property_get, base_property_set, geometry_to_value, read_widget_property_by_name,
    widget_property_get, widget_property_names, widget_property_set, write_widget_property_by_name,
    WidgetProperties, BASE_PROPERTY_NAMES,
};

/// The id-level property accessors, re-exported so a backend can read and write a
/// mounted control's properties without depending on the module layout.
///
/// Gated with `access` itself: the alloc-frugal profile compiles the whole
/// capability layer out, so these cannot exist there. `embedded` keeps them — its
/// widget set is smaller, not absent.
#[cfg(widgets_unstripped)]
pub use access::{read_widget_property_by_id, write_widget_property_by_id};

/// The canonical `snake_case` name of a kind.
///
/// Used by [`WidgetFactory::capability_by_kind`] to pick the *canonical* entry when
/// several controls share a kind, and by the registry-free name lookup that serves
/// builds without the capability registry. Derived from the variant's spelling,
/// which follows the factory's own naming convention (`WidgetKind::ToolButton` →
/// `tool_button`).
#[cfg(widgets_unstripped)]
pub(crate) fn kind_canonical_name(kind: crate::widget::WidgetKind) -> alloc::string::String {
    let mut name = alloc::string::String::new();
    kind_canonical_name_into(kind, &mut name);
    name
}

/// Appends the canonical `snake_case` name of `kind` to `out`, without allocating.
///
/// The allocation-free half of [`kind_canonical_name`], for callers that already own a
/// buffer (which is all of them — the name is used transiently). Reusing one buffer
/// across lookups removed the mutex that used to guard an intern map here; see
/// [`canonical_name_for_kind`] for the history.
///
/// Compiled wherever a caller exists. `mini` compiles the capability layer out
/// entirely, and `embedded` names its kinds from the static table in
/// [`canonical_name_for_kind`] — it registers no aliases, so it never needs to derive a
/// spelling at runtime.
#[cfg(all(not(alloc_frugal), not(embedded_surface)))]
pub(crate) fn kind_canonical_name_into(
    kind: crate::widget::WidgetKind,
    out: &mut alloc::string::String,
) {
    use core::fmt::Write as _;
    // `Debug` for a fieldless enum writes the variant name with no allocation; the
    // snake_case conversion is then done in place, one character at a time.
    let start = out.len();
    let _ = write!(out, "{kind:?}");

    // Lowercase the segment just written, inserting `_` before an inner capital.
    // A run of capitals (`QRCode`) stays one word, so only a capital preceded by a
    // lowercase letter or digit starts a new word.
    let segment = out[start..].to_ascii_lowercase();
    let original: alloc::string::String = out[start..].into();
    out.truncate(start);
    for (index, ch) in original.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            let previous_is_upper = index > 0
                && original.chars().nth(index - 1).is_some_and(|p| p.is_ascii_uppercase());
            if index > 0 && !previous_is_upper {
                out.push('_');
            }
        }
        // Take the already-lowercased character from the parallel string.
        if let Some(lower) = segment.chars().nth(index) {
            out.push(lower);
        }
    }
}

/// The `WidgetFactory` name under which `kind` is registered.
///
/// Derived from the capability registry — the same table [`WidgetFactory::create`]
/// dispatches on — rather than from `Debug` output, so the two cannot disagree.
///
/// # Aliases
///
/// Several `WidgetKind` variants name a *type alias* rather than a distinct type:
/// `ActivityIndicator` is `ProgressBar`, `DoubleSpinBox` is `SpinBox`,
/// `ColumnView` is `TreeView`, `UndoView` is `ListView`, `DirectoryDialog` is
/// `FileDialog`, `ContextMenu` is `Menu`, `Dialog` is `PopupWindow`. The factory
/// registers the target type once, so a lookup by the alias finds nothing and the
/// control would refuse to be created.
///
/// `alias_factory_name` resolves those variants, which is why this returns a `&str`
/// rather than `Option`: every kind has a constructor, either directly or through
/// the variant's own spelling, with the alias table applied, so a build without the
/// capability registry can still name every kind's constructor.
#[cfg(widgets_unstripped)]
pub fn factory_name_for_kind(kind: crate::widget::WidgetKind) -> &'static str {
    #[cfg(not(full_widgets))]
    {
        factory_name_for_kind_without_registry(kind)
    }
    #[cfg(full_widgets)]
    {
        let factory = WidgetFactory::new_with_defaults();
        if let Some(capability) = factory.capability_by_kind(kind) {
            return capability.canonical_name;
        }
        alias_factory_name(kind)
    }
}

/// The alias table, shared by the registry-backed lookup above and the
/// registry-free lookup below so the two cannot disagree about the fallback.
///
/// Gated with the full widget set because it names variants `embedded` compiles
/// out (`ActivityIndicator`, `ColumnView`, …). The registry-free path resolves the
/// same names from the variant's spelling, so nothing is lost there.
#[cfg(full_widgets)]
fn alias_factory_name(kind: crate::widget::WidgetKind) -> &'static str {
    // Alias variants resolve to the name their target type is registered under.
    match kind {
        crate::widget::WidgetKind::ActivityIndicator => "progress_bar",
        crate::widget::WidgetKind::DoubleSpinBox => "spin_box",
        crate::widget::WidgetKind::ColumnView => "tree_view",
        crate::widget::WidgetKind::UndoView => "list_view",
        crate::widget::WidgetKind::CheckListBox => "list_box",
        crate::widget::WidgetKind::DirectoryDialog => "file_dialog",
        crate::widget::WidgetKind::ContextMenu => "menu",
        crate::widget::WidgetKind::Dialog => "popup_window",
        crate::widget::WidgetKind::Wizard => "wizard_dialog",
        other => {
            // Reached only if a kind is added to `WidgetKind` with neither a
            // capability nor an alias. Reported loudly rather than returning a
            // plausible-looking name the factory would reject.
            log::error!(
                "widget capability registry has no entry and the alias table no mapping for \
                 {other:?}; its constructor cannot be resolved"
            );
            ""
        }
    }
}

/// The name lookup for a build that compiles the capability registry out.
///
/// `embedded` (and any other build without device profiles) still needs to name a
/// kind's constructor — its widget set is simply smaller. Deriving the name from
/// the variant's own spelling keeps that working without compiling the registry,
/// and the alias table above is applied on top so the two paths name the same
/// constructor.
#[cfg(all(widgets_unstripped, not(full_widgets)))]
fn factory_name_for_kind_without_registry(kind: crate::widget::WidgetKind) -> &'static str {
    use core::fmt::Write as _;

    // Build the name in a stack buffer and look it up immediately. The name is not
    // retained, so nothing is interned and no lock is taken — see
    // `intern_kind_name` below for why that matters.
    let mut buffer = alloc::string::String::new();
    kind_canonical_name_into(kind, &mut buffer);

    // The alias table is a `match`, so it resolves to a `&'static str` with no
    // allocation and no synchronisation.
    if let Some(alias) = alias_for_name(&buffer) {
        return alias;
    }
    // No alias: the canonical name must itself be a known factory name. Reaching
    // here means the caller asked for a kind whose variant name differs from its
    // registered name without an alias entry — a missing alias, not a reason to
    // borrow one from another kind. The name is returned from a small static table so
    // the caller still gets a `&'static str` without leaking.
    canonical_name_for_kind(kind)
}

/// The alias table keyed by canonical name, for the registry-free path.
#[cfg(all(widgets_unstripped, not(full_widgets)))]
fn alias_for_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "activity_indicator" => "progress_bar",
        "double_spin_box" => "spin_box",
        "column_view" => "tree_view",
        "undo_view" => "list_view",
        "check_list_box" => "list_box",
        "directory_dialog" => "file_dialog",
        "context_menu" => "menu",
        "dialog" => "popup_window",
        "wizard" => "wizard_dialog",
        _ => return None,
    })
}

/// The canonical factory name of a kind whose variant spelling already matches it.
///
/// The fallback for the registry-free path when [`alias_for_name`] has no entry. Its
/// matched set is exactly the kinds whose `Debug` spelling and factory name agree;
/// everything else is covered by the alias table above.
///
/// Returns `""` for an unmatched kind, which the constructor lookup reads as "not
/// available in this profile" — the same answer it gives for a kind the `embedded`
/// widget set does not ship. Inventing a name here would let the factory build a
/// control the profile does not have.
#[cfg(all(widgets_unstripped, not(full_widgets)))]
fn canonical_name_for_kind(kind: crate::widget::WidgetKind) -> &'static str {
    match kind {
        crate::widget::WidgetKind::Button => "button",
        crate::widget::WidgetKind::Label => "label",
        crate::widget::WidgetKind::CheckBox => "check_box",
        crate::widget::WidgetKind::RadioButton => "radio_button",
        crate::widget::WidgetKind::LineEdit => "line_edit",
        crate::widget::WidgetKind::Slider => "slider",
        crate::widget::WidgetKind::ProgressBar => "progress_bar",
        crate::widget::WidgetKind::ComboBox => "combo_box",
        crate::widget::WidgetKind::ListBox => "list_box",
        crate::widget::WidgetKind::Panel => "panel",
        crate::widget::WidgetKind::GroupBox => "group_box",
        crate::widget::WidgetKind::ScrollArea => "scroll_area",
        crate::widget::WidgetKind::ScrollBar => "scroll_bar",
        crate::widget::WidgetKind::Splitter => "splitter",
        crate::widget::WidgetKind::TabWidget => "tab_widget",
        crate::widget::WidgetKind::StatusBar => "status_bar",
        crate::widget::WidgetKind::ToolBar => "tool_bar",
        crate::widget::WidgetKind::MenuBar => "menu_bar",
        _ => "",
    }
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

/// By-id widget construction entry points used by the crate-root `create_*`
/// wrappers. Requires the full widget set.
#[cfg(full_widgets)]
pub mod constructors;
#[cfg(full_widgets)]
pub use constructors::*;

/// The centralised per-kind property table and the capability descriptors built
/// from it. Requires the full widget set.
#[cfg(full_widgets)]
pub mod properties;
#[cfg(full_widgets)]
pub(crate) use properties::*;

#[cfg(widgets_unstripped)]
pub mod access;

/// By-id control-state access for the crate-root wrappers (crate-internal).
pub(crate) mod widget_access;
#[cfg(widgets_unstripped)]
pub use access::*;

/// The schema-default lookup, re-exported so the factory can reach it from the
/// module that owns the property tables.
#[cfg(full_widgets)]
pub use access::default_widget_property_default_value;

#[cfg(full_widgets)]
pub mod registration;

/// Factory and registration tests.
///
/// Gated on `full_widgets`, matching the factory's own gate rather than the wider
/// `widgets_unstripped`. The tests below call `WidgetFactory::new_with_defaults`
/// and inspect the registry it populates, so they describe machinery that only
/// exists where the core registrations are installed. Under a build with no device
/// profile (such as `--features android`) the factory exists but is empty, and
/// these assertions would be describing an absent table — the mismatch showed up as
/// `cannot find type WidgetKind in this scope` because a doc/test `use` resolved into
/// a module the wider gate let through.
#[cfg(all(test, full_widgets))]
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
#[cfg(full_widgets)]
impl Default for WidgetFactory {
    fn default() -> Self {
        Self::new_with_defaults()
    }
}

/// Runtime widget factory: builds a control by name or kind.
///
/// See the module docs; gated with the profile-specific control set, because every
/// constructor it registers names a concrete control type.
#[cfg(full_widgets)]
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
    ///
    /// Gated with the registration table it installs: a build without the full
    /// widget set has no constructors to register, and a factory that reported
    /// itself as populated while holding nothing would be worse than its absence.
    /// Callers in such a build get [`Self::new`] instead and see an empty table,
    /// which is the truth.
    #[cfg(full_widgets)]
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

    /// Returns the capability that canonically represents `kind`.
    ///
    /// # Why "first registered" was wrong
    ///
    /// Ten `WidgetKind` values are shared by two or more controls, because a
    /// specialised control reuses its base kind: `split_button` and `tool_button`
    /// both declare `ToolButton`; `code_editor` and `rich_edit` both declare
    /// `RichEdit`; `snackbar` and `status_bar` both declare `StatusBar`; and so on
    /// for `Canvas`, `Chart`, `Table`, `TextEdit`, `ToggleButton`, `TreeView` and
    /// `WebEngineView`.
    ///
    /// Returning `indices[0]` therefore resolved a `ToolButton` lookup to
    /// `split_button` — a different control — whenever the specialised entry
    /// happened to be registered first. That made every kind→name lookup depend on
    /// registration order, and silently built the wrong widget.
    ///
    /// The canonical entry for a kind is the one whose `canonical_name` matches the
    /// kind's own name; a specialised control is reachable by its own name through
    /// [`Self::capability`] instead. When no name matches (a kind whose entry is
    /// named differently on purpose), the first registration is the fallback, which
    /// preserves the previous behaviour for the unambiguous majority.
    pub fn capability_by_kind(&self, kind: WidgetKind) -> Option<&WidgetCapability> {
        let indices = self.kind_to_index.get(&kind)?;
        let expected = kind_canonical_name(kind);
        indices
            .iter()
            .filter_map(|index| self.capabilities.get(*index))
            .find(|capability| capability.canonical_name == expected)
            .or_else(|| self.capabilities.get(indices[0]))
    }

    /// Returns every capability registered for `kind`.
    ///
    /// The specialisations of a base kind are reachable through this rather than
    /// through [`Self::capability_by_kind`], so a caller that wants "all controls of
    /// this kind" is not silently given only the canonical one.
    pub fn capabilities_for_kind(
        &self,
        kind: WidgetKind,
    ) -> impl Iterator<Item = &WidgetCapability> {
        self.kind_to_index
            .get(&kind)
            .into_iter()
            .flatten()
            .filter_map(|index| self.capabilities.get(*index))
    }

    /// Returns all registered capabilities.
    pub fn capabilities(&self) -> &[WidgetCapability] {
        &self.capabilities
    }

    /// Resolves the capability that describes `widget`, using its concrete type.
    ///
    /// Public because it is the only way to ask the question the registry answers
    /// internally, and answering it from outside is what lets a test prove that
    /// controls sharing a `WidgetKind` are still distinguishable. A caller walking
    /// `property_schema` has an equivalent need: given a live control, which schema
    /// describes it?
    pub fn capability_for_kind_instance(&self, widget: &dyn Widget) -> Option<&WidgetCapability> {
        self.capability_for_widget(widget)
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

        read_widget_property_by_name(widget, property.name)
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

        write_widget_property_by_name(widget, property.name, value)
    }

    /// Looks up capability by widget kind using the kind-based index.
    ///
    /// When several capabilities share a `WidgetKind` (`DataGrid`, `VirtualTable`
    /// and `TableWidget` all report `WidgetKind::Table`), the right one is chosen by
    /// comparing the *concrete* widget type against the capability's canonical name.
    fn capability_for_widget(&self, widget: &dyn Widget) -> Option<&WidgetCapability> {
        let kind = widget.kind();
        let indices = self.kind_to_index.get(&kind)?;

        // Fast path: only one capability for this kind.
        if indices.len() == 1 {
            return self.capabilities.get(indices[0]);
        }

        // Resolve by concrete type first. This is the only reliable tie-break: the
        // kind alone cannot distinguish two types that report the same one.
        for &idx in indices.iter() {
            let cap = &self.capabilities[idx];
            if self.widget_matches_capability(widget, cap.canonical_name) {
                return Some(cap);
            }
        }

        // No type-based answer. Returning the first registered capability (the old
        // behaviour) is what made `segmented_control` report `UnknownProperty` for
        // its own `item_count`: the lookup landed on `toggle_button` — whichever
        // registered first — and searched that schema instead. A wrong schema is
        // worse than no schema, because the caller gets "this name does not exist"
        // for a name the widget really has.
        //
        // An empty capability is the honest answer: it declares no properties rather
        // than another control's. The caller's own `properties_dyn` contract still
        // serves the real properties, so nothing is lost — the schema lookup simply
        // stops lying.
        log::warn!(
            "widget capability registry has no type-based tie-break for {kind:?}; \
             falling back to an empty schema rather than another control's"
        );
        self.capabilities.iter().find(|cap| cap.kind == kind && cap.properties.is_empty())
    }

    /// Check whether a widget instance matches a given capability's concrete type.
    ///
    /// Only capabilities that share a `WidgetKind` need this question answered
    /// (`DataGrid` / `VirtualTable` / `TableWidget` all report `WidgetKind::Table`).
    ///
    /// `false` is the default rather than `true`: a name this table does not know is
    /// not evidence that the widget matches it. Answering `true` made the caller
    /// accept the *first* candidate for an unknown name, which is how a widget ended
    /// up reading another control's schema.
    ///
    /// A profile that compiles the concrete types out cannot answer at all; there the
    /// kind-based index has already narrowed the field to one entry, so the question
    /// is only reached for kinds that cannot be ambiguous.
    #[cfg(all(not(full_widgets), not(embedded_surface)))]
    fn widget_matches_capability(&self, _widget: &dyn Widget, _canonical_name: &str) -> bool {
        true
    }

    /// Concrete-type tie-break for capabilities sharing a kind.
    ///
    /// Every capability whose canonical name can collide with another *must* have a
    /// row here. `every_shared_kind_has_a_tie_break` in the tests enforces that, so a
    /// newly registered control cannot quietly inherit another's schema the way
    /// `segmented_control` inherited `toggle_button`'s.
    #[cfg(full_widgets)]
    fn widget_matches_capability(&self, widget: &dyn Widget, canonical_name: &str) -> bool {
        match canonical_name {
            // `WidgetKind::Table`
            "data_grid" => self::coercion::widget_as::<DataGrid>(widget).is_some(),
            "virtual_table" => self::coercion::widget_as::<VirtualTable>(widget).is_some(),
            "table_widget" => self::coercion::widget_as::<TableWidget>(widget).is_some(),
            // `WidgetKind::TreeView`
            "tree_table" => self::coercion::widget_as::<TreeTable>(widget).is_some(),
            "tree_view" => self::coercion::widget_as::<TreeView>(widget).is_some(),
            // `WidgetKind::ToggleButton`
            "segmented_control" => self::coercion::widget_as::<SegmentedControl>(widget).is_some(),
            "toggle_button" => self::coercion::widget_as::<ToggleButton>(widget).is_some(),
            // `WidgetKind::TextEdit`
            "text_edit" => self::coercion::widget_as::<TextEdit>(widget).is_some(),
            "terminal_view" => self::coercion::widget_as::<TerminalView>(widget).is_some(),
            // `WidgetKind::RichEdit`
            "rich_edit" => self::coercion::widget_as::<RichEdit>(widget).is_some(),
            "code_editor" => self::coercion::widget_as::<CodeEditor>(widget).is_some(),
            // `WidgetKind::StatusBar`
            "status_bar" => self::coercion::widget_as::<StatusBar>(widget).is_some(),
            "snackbar" => self::coercion::widget_as::<Snackbar>(widget).is_some(),
            // `WidgetKind::Canvas`
            "canvas" => self::coercion::widget_as::<Canvas>(widget).is_some(),
            "map_view" => self::coercion::widget_as::<MapView>(widget).is_some(),
            // `WidgetKind::Chart`
            "chart" => self::coercion::widget_as::<ChartWidget>(widget).is_some(),
            "gantt_widget" => self::coercion::widget_as::<GanttWidget>(widget).is_some(),
            // `WidgetKind::WebEngineView`
            "web_view" => self::coercion::widget_as::<WebView>(widget).is_some(),
            "media_player" => self::coercion::widget_as::<MediaPlayer>(widget).is_some(),
            // `WidgetKind::ToolButton`
            "tool_button" => self::coercion::widget_as::<ToolButton>(widget).is_some(),
            "split_button" => self::coercion::widget_as::<SplitButton>(widget).is_some(),
            // A name this table does not know is not evidence of a match.
            _ => false,
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

        self.schema_default_value(capability.kind, property.name)
            .ok_or(CapabilityAccessError::UnsupportedOnWidget)
    }

    /// The schema default for `kind`'s `property_name`, from the property tables.
    ///
    /// Gated with the tables themselves: a stripped profile declares no schema
    /// properties, so there is no default to report. The `None` there is the same
    /// answer the covered arm gives for an unknown property, which is why the
    /// callers above already treat `None` as "unsupported" rather than "missing".
    #[cfg(full_widgets)]
    fn schema_default_value(
        &self,
        kind: crate::widget::WidgetKind,
        property_name: &str,
    ) -> Option<CapabilityValue> {
        access::default_widget_property_default_value(kind, property_name)
    }

    /// See the `full_widgets` definition: a stripped profile has no property tables.
    #[cfg(not(full_widgets))]
    fn schema_default_value(
        &self,
        _kind: crate::widget::WidgetKind,
        _property_name: &str,
    ) -> Option<CapabilityValue> {
        None
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
            let default_value = self
                .schema_default_value(capability.kind, property.name)
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
