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
use crate::compat::HashMap;

#[cfg(full_widgets)]
use crate::core::Rect;

#[cfg(full_widgets)]
use super::{Widget, WidgetKind};
#[cfg(full_widgets)]
use crate::widget::base_widgets::toggle_button::ToggleButton;
#[cfg(full_widgets)]
use crate::widget::dialog::popup_window::PopupWindow;
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
use crate::widget::special_widgets::command_palette::CommandPalette;
#[cfg(full_widgets)]
use crate::widget::special_widgets::diff_viewer::DiffViewer;
#[cfg(full_widgets)]
use crate::widget::special_widgets::gantt_widget::GanttWidget;
#[cfg(full_widgets)]
use crate::widget::special_widgets::map_view::MapView;
#[cfg(full_widgets)]
use crate::widget::special_widgets::markdown_editor::MarkdownEditor;
#[cfg(full_widgets)]
use crate::widget::special_widgets::media_player::MediaPlayer;
#[cfg(full_widgets)]
use crate::widget::special_widgets::notification_center::NotificationCenter;
#[cfg(full_widgets)]
use crate::widget::special_widgets::segmented_control::SegmentedControl;
#[cfg(full_widgets)]
use crate::widget::special_widgets::snackbar::Snackbar;
#[cfg(full_widgets)]
use crate::widget::special_widgets::split_button::SplitButton;
#[cfg(full_widgets)]
use crate::widget::special_widgets::terminal_view::TerminalView;
#[cfg(full_widgets)]
use crate::widget::special_widgets::timeline_widget::TimelineWidget;
#[cfg(full_widgets)]
use crate::widget::special_widgets::toast::ToastStack;
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::DataGrid;
#[cfg(full_widgets)]
use crate::widget::view_widgets::list_view::ListView;
#[cfg(full_widgets)]
use crate::widget::view_widgets::table_widget::TableWidget;
#[cfg(full_widgets)]
use crate::widget::view_widgets::tree_view::TreeView;
#[cfg(full_widgets)]
use crate::widget::view_widgets::virtual_list::VirtualList;
#[cfg(full_widgets)]
use crate::widget::view_widgets::virtual_table::VirtualTable;
#[cfg(full_widgets)]
use crate::widget::web_widgets::web_view::WebView;
// The container types the tie-break table below compares against. Each is named
// there because its kind is shared with another control, so the concrete-type check
// is the only way to tell which capability a mounted widget belongs to.
#[cfg(full_widgets)]
use crate::widget::container_widgets::groupbox::GroupBox;
#[cfg(full_widgets)]
use crate::widget::container_widgets::toolbox::ToolBox;

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
    append_widget_list_item, base_property_get, base_property_set, clear_widget_list_items,
    geometry_to_value, read_widget_property_by_name, widget_list_item, widget_list_item_count,
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
/// several controls share a kind. Derived from the variant's spelling, which follows
/// the factory's own naming convention (`WidgetKind::ToolButton` → `tool_button`).
///
/// Gated with its only caller: `capability_by_kind` lives in the `full_widgets`
/// `impl WidgetFactory` block, so a build with an OS backend but no device profile
/// (CI's `windows-cross-check` feature set) compiles the registry out and had this
/// function left over as dead code.
#[cfg(full_widgets)]
pub(crate) fn kind_canonical_name(kind: crate::widget::WidgetKind) -> alloc::string::String {
    let mut name = alloc::string::String::new();
    kind_canonical_name_into(kind, &mut name);
    name
}

/// Appends the canonical `snake_case` name of `kind` to `out`, without allocating.
///
/// The allocation-free half of the allocating wrapper above, for callers that already
/// own a buffer (which is all of them — the name is used transiently). Reusing one
/// buffer across lookups removed the mutex that used to guard an intern map here; see
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
/// `FileDialog`, `ContextMenu` is `Menu`. The factory
/// registers the target type once, so a lookup by the alias finds nothing and the
/// control would refuse to be created.
///
/// `Dialog` used to be listed here as an alias of `PopupWindow`. It is no longer
/// one: 2.4.0 gave `Dialog` its own module and constructor, so both are
/// independent controls and neither needs the other's entry.
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
/// # Why `DockPanel` is here
///
/// `src/widget/mod.rs` declares `pub type DockPanel = DockWidget;`, so the kind
/// and the type are the same control under two names — but the factory registers
/// the *canonical* `DockWidget` capability, whose canonical name is `dock_widget`.
/// `capability_by_kind(DockPanel)` therefore misses (no entry declares that kind)
/// and the lookup falls through to here. Omitting this row made
/// `factory_name_for_kind(DockPanel)` return `""`, so `create_dock_panel(..)`
/// silently produced id `0`.
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
        crate::widget::WidgetKind::Wizard => "wizard_dialog",
        crate::widget::WidgetKind::DockPanel => "dock_widget",
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
        "wizard" => "wizard_dialog",
        "dock_panel" => "dock_widget",
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

    /// Reports every `WidgetKind` that carries **more than one** capability.
    ///
    /// Several kinds are deliberately shared by distinct controls — `Table` is
    /// served by `table_widget`, `data_grid`, `tree_table` and `virtual_table`; the
    /// nine chart kinds all report `WidgetKind::Chart`. That sharing is intended, and
    /// [`Self::capability_by_kind`] documents how the right one is chosen.
    ///
    /// What is *not* intended is a kind whose only capability is named after a
    /// **different** kind.
    ///
    /// # The defect this answers
    ///
    /// `WidgetKind::Table` used to resolve to `tree_table`. The cause was structural:
    /// there was no `table` capability at all, so many-to-one resolution returned
    /// whichever chart/table entry happened to be registered first. Nothing caught it,
    /// because every gate that asks "is this kind reachable" is satisfied by *any*
    /// capability reporting the kind, and by that measure `Table` was fine — only the
    /// *control a caller gets back* was wrong.
    ///
    /// Returning these pairs lets a test turn the substitution into a statement about
    /// every kind at once instead of one hand-picked example. The same list shows an
    /// auditor where the kind→name table is load-bearing.
    ///
    /// Returns an empty vector when every kind's canonical control has a capability
    /// named after it, which is the healthy state for kinds with a single capability.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_widgets::widget::capability::WidgetFactory;
    /// let factory = WidgetFactory::new_with_defaults();
    /// for (kind, resolved, candidates) in factory.shared_kinds_resolving_to_other_names() {
    ///     // `resolved` is a real, constructible control — the kind just is not its
    ///     // own control's name because several controls share the kind.
    ///     assert!(!factory.capability(resolved).is_none(), "{resolved} must be constructible");
    ///     assert!(candidates >= 1, "{kind:?} must list at least the resolved control");
    /// }
    /// ```
    pub fn shared_kinds_resolving_to_other_names(&self) -> Vec<(WidgetKind, &'static str, usize)> {
        let mut result: Vec<(WidgetKind, &'static str, usize)> = Vec::new();
        for (kind, indices) in &self.kind_to_index {
            if indices.is_empty() {
                continue;
            }
            let expected = kind_canonical_name(*kind);
            let matches_own_name = indices.iter().any(|index| {
                self.capabilities
                    .get(*index)
                    .is_some_and(|capability| capability.canonical_name == expected)
            });
            if matches_own_name {
                continue;
            }
            if let Some(resolved) = self.capability_by_kind(*kind) {
                result.push((*kind, resolved.canonical_name, indices.len()));
            }
        }
        result.sort_by_key(|(kind, _, _)| *kind);
        result
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

    /// Runs a published command on a widget instance, by command name.
    ///
    /// The imperative counterpart of [`Self::write_property`]: where that assigns
    /// state, this performs an action. Both validate the name against the capability
    /// first, so a caller that read the name from
    /// [`WidgetCapability::commands`](types::WidgetCapability::commands) cannot be
    /// told "no such command" for a name the registry itself published — the two
    /// sources are compared rather than trusted, and a divergence is reported as
    /// [`CapabilityAccessError::UnsupportedOnWidget`] meaning "the registry is
    /// wrong", not "you are".
    ///
    /// # Why the capability check comes first
    ///
    /// The control's own `command` is the final authority on what it can do, but
    /// asking the capability first makes the *published list* load-bearing: if a
    /// control implements a command its capability forgot to list, this reports it
    /// instead of quietly succeeding, which is what keeps
    /// `capability::properties_tests::every_published_command_is_dispatched` able to
    /// fail.
    ///
    /// # Errors
    ///
    /// * [`CapabilityAccessError::UnknownWidget`] — the widget resolves to no
    ///   capability.
    /// * [`CapabilityAccessError::UnknownCommand`] — the capability does not publish
    ///   the name, so it is not a command of this control at all.
    /// * [`CapabilityAccessError::UnsupportedOnWidget`] — the capability publishes it
    ///   but the control does not implement it, which is a registry/implementation
    ///   disagreement rather than a caller mistake.
    pub fn invoke_command(
        &self,
        widget: &mut dyn Widget,
        command_name: &str,
    ) -> Result<(), CapabilityAccessError> {
        let capability =
            self.capability_for_widget(widget).ok_or(CapabilityAccessError::UnknownWidget)?;

        // `commands` are plain lower-case names by construction, but normalising is
        // what makes the lookup agree with `capability()` / `read_property`, which
        // both normalise. A caller using `"clear-selection"` must reach the same
        // command as one using `"clear_selection"`.
        let normalized = normalize_key(command_name);
        let published = capability.commands.iter().any(|name| normalize_key(name) == normalized);
        if !published {
            return Err(CapabilityAccessError::UnknownCommand);
        }

        // Route through the control's declared `WidgetProperties` contract, the same
        // way `read_property` / `write_property` do: `command` lives there so a
        // control implements it beside `get` / `set` / `property_names`, and so its
        // default ("no such command") cannot be bypassed by a type that merely happens
        // to have a same-named inherent method.
        //
        // A control with no contract declared at all answers `UnsupportedOnWidget`,
        // matching how the property path reports the same situation.
        let Some(properties) = widget.properties_dyn_mut() else {
            return Err(CapabilityAccessError::UnsupportedOnWidget);
        };

        match properties.command(command_name) {
            Ok(()) => Ok(()),
            // The capability published the name but the control refused it. Reporting
            // the caller's name as unknown would send them to look for a different
            // control; `UnsupportedOnWidget` says the control was expected to have it.
            Err(CapabilityAccessError::UnknownCommand) => {
                log::warn!(
                    "widget {command_name:?} is published by capability {:?} but the control \
                     does not implement it",
                    capability.canonical_name
                );
                Err(CapabilityAccessError::UnsupportedOnWidget)
            }
            Err(other) => Err(other),
        }
    }

    /// Reports whether a widget's control answers `command_name`.
    ///
    /// The read-only companion to [`Self::invoke_command`], for a caller building a
    /// menu or a palette that must show only the actions a control can actually
    /// perform. It runs the command, because there is no side-effect-free way to ask
    /// and a command is expected to be idempotent enough to probe — the alternative
    /// (a second "supports" table next to the dispatch) would be the drift this
    /// module already removed for properties.
    ///
    /// # Why this is not called `supports_command`
    ///
    /// It reports whether the command is *addressable*: published by the capability
    /// **and** implemented by the control. `supports` would suggest a capability
    /// question, and the capability's answer alone is the half that can be wrong.
    ///
    /// # Errors
    ///
    /// Exactly the errors [`Self::invoke_command`] returns. A caller that only needs
    /// a yes/no reads `is_ok()`.
    pub fn command_is_known(
        &self,
        widget: &mut dyn Widget,
        command_name: &str,
    ) -> Result<(), CapabilityAccessError> {
        self.invoke_command(widget, command_name)
    }

    /// Validates an event name a caller read from
    /// [`WidgetCapability::events`](types::WidgetCapability::events), and connects a
    /// slot to it on `hub`.
    ///
    /// # The bridge this provides, and why it is needed
    ///
    /// A capability publishes the names of the events its control can emit
    /// (`"clicked"`, `"selection_changed"`, …). A consumer that discovered the control
    /// through the registry therefore knows the names but has no way to *act* on them:
    /// unlike a command, an event cannot be invoked on demand, so there is no dispatch
    /// to add. What was missing is that the published names reached nothing — the name
    /// `"clicked"` in a capability and the `clicked` signal a control actually emits
    /// were two unrelated facts, connected only by spelling.
    ///
    /// This method is that connection. `signal::CustomSignalHub` is the library's
    /// name-addressed signal registry; routing the published name through validation
    /// here means a subscriber can only attach to a name its control publishes, which
    /// is what makes the published list load-bearing rather than decorative.
    ///
    /// # Why `hub` is a parameter rather than a global
    ///
    /// The hub is owned by whoever dispatches events — an application, a test, a
    /// platform backend — so this method must not invent a global one. Taking it as an
    /// argument also makes the function usable from a test without process-wide state.
    ///
    /// # Errors
    ///
    /// * [`CapabilityAccessError::UnknownWidget`] — no control is registered under
    ///   `control_name`, so it has no event list to validate against.
    /// * [`CapabilityAccessError::UnknownCommand`] — the control exists but does not
    ///   publish that event name. The variant names "command", but it is the
    ///   capability layer's single "this control does not have that action" answer and
    ///   is reused rather than forked: an event is an action the control performs, just
    ///   one the caller cannot trigger. Adding a fourth sibling with identical semantics
    ///   would be the duplication rule #54 forbids.
    ///
    /// # Examples
    ///
    /// ```
    /// use rust_widgets::signal::CustomSignalHub;
    /// use rust_widgets::widget::capability::WidgetFactory;
    ///
    /// let factory = WidgetFactory::new_with_defaults();
    /// let hub = CustomSignalHub::new();
    /// factory
    ///     .connect_event("button", "clicked", &hub, || {})
    ///     .expect("button publishes `clicked`");
    /// ```
    pub fn connect_event<F>(
        &self,
        control_name: &str,
        event_name: &str,
        hub: &crate::signal::CustomSignalHub,
        slot: F,
    ) -> Result<crate::signal::ConnectionHandle, CapabilityAccessError>
    where
        F: FnMut() + Send + Sync + 'static,
    {
        let capability =
            self.capability(control_name).ok_or(CapabilityAccessError::UnknownWidget)?;

        // Normalised, so `"value-changed"` and `"value_changed"` reach the same event
        // — the same tolerance `invoke_command` and `read_property` provide.
        let normalized = normalize_key(event_name);
        let published = capability.events.iter().any(|name| normalize_key(name) == normalized);
        if !published {
            return Err(CapabilityAccessError::UnknownCommand);
        }

        Ok(hub.connect(event_name, slot))
    }

    /// Reports whether `control_name` publishes `event_name`, without subscribing.
    ///
    /// The probing form of [`Self::connect_event`], for a consumer that builds a menu
    /// of available events and must not register a slot per entry. It answers exactly
    /// the same question — the validation is shared, not re-stated — so a name this
    /// accepts is a name `connect_event` accepts.
    ///
    /// # Why a `hub` is still needed
    ///
    /// The check itself is a lookup on the capability, but sharing one implementation
    /// with `connect_event` is what keeps the two from diverging, and the cheapest way
    /// to share it is to run the real path. The hub is therefore passed in and the
    /// connection is dropped immediately.
    ///
    /// # Errors
    ///
    /// Exactly the errors [`Self::connect_event`] returns.
    pub fn event_is_subscribable(
        &self,
        control_name: &str,
        event_name: &str,
        hub: &crate::signal::CustomSignalHub,
    ) -> Result<(), CapabilityAccessError> {
        // A real connection, not a flag: the slot is dropped with the handle, so this
        // has no lasting effect on the hub while exercising the identical validation.
        let handle = self.connect_event(control_name, event_name, hub, || {})?;
        let _ = hub.disconnect(event_name, handle);
        Ok(())
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
        //
        // Iteration order matters when two capabilities in this group are **the same
        // control under two names** (`table_widget` / `table`, `group_box` / `panel`,
        // `tool_box` / `toolbox`, `virtual_list` / `data_view`). Their tie-break rows
        // both accept the widget — correctly, because it is one widget — so the first
        // iteration wins. Choosing it by *registration order* rather than by "whichever
        // name this iteration happened to produce" is what makes the answer stable:
        // `indices` is a list of positions in a `Vec` built by the same deterministic
        // registration sequence every time, so the same name comes back on every call.
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
    /// # Why the row key is the *canonical* name, and what a missing row costs
    ///
    /// This table is reached only when a kind has more than one capability, so the
    /// names it must cover are the ones that actually collide. A missing row makes the
    /// control **unaddressable through the capability layer**: `capability_for_widget`
    /// finds no match and returns nothing, so `read_property`
    /// `write_property` answer `UnknownWidget` for a control that is mounted and
    /// constructible — and `capability_for_kind_instance` reports `None`, which is the
    /// signal `tests/control_backend_named_creation_test.rs` uses to catch substitutions.
    ///
    /// The rows below were each added because that signal fired, not speculatively:
    /// `web_engine_view` (and therefore the `web_view` / `webview` aliases and the
    /// `WebView` type), `tool_box` / `toolbox`, `panel` and `data_view` all share their
    /// kind with another control and had no row.
    #[cfg(full_widgets)]
    fn widget_matches_capability(&self, widget: &dyn Widget, canonical_name: &str) -> bool {
        match canonical_name {
            // `WidgetKind::GroupBox`, which `panel` shares because `Panel` is a
            // `pub type` for `GroupBox`.
            "group_box" | "panel" => self::coercion::widget_as::<GroupBox>(widget).is_some(),
            // `WidgetKind::Table`
            "data_grid" => self::coercion::widget_as::<DataGrid>(widget).is_some(),
            "virtual_table" => self::coercion::widget_as::<VirtualTable>(widget).is_some(),
            // `table_widget` and `table` are two names for one control, so both rows
            // have the same answer. The same holds for `tool_box` / `toolbox`,
            // `panel` / `group_box`, and `virtual_list` / `data_view` below: an alias
            // and its canonical name are one widget, and a tie-break row keyed on only
            // one of the two spellings makes the other address nothing.
            "table_widget" | "table" => self::coercion::widget_as::<TableWidget>(widget).is_some(),
            "diff_viewer" => self::coercion::widget_as::<DiffViewer>(widget).is_some(),
            // `WidgetKind::TreeView`
            "tree_view" => self::coercion::widget_as::<TreeView>(widget).is_some(),
            // `WidgetKind::ToggleButton`
            "segmented_control" => self::coercion::widget_as::<SegmentedControl>(widget).is_some(),
            "toggle_button" => self::coercion::widget_as::<ToggleButton>(widget).is_some(),
            // `WidgetKind::ListView`. `list_view` shares the kind with
            // `command_palette` and `notification_center`, so all three need a row:
            // without one the lookup falls through to an empty schema and the
            // control reports `UnknownWidget` for its own properties.
            "list_view" => self::coercion::widget_as::<ListView>(widget).is_some(),
            "command_palette" => self::coercion::widget_as::<CommandPalette>(widget).is_some(),
            "notification_center" => {
                self::coercion::widget_as::<NotificationCenter>(widget).is_some()
            }
            // `WidgetKind::TextEdit`
            "text_edit" => self::coercion::widget_as::<TextEdit>(widget).is_some(),
            "terminal_view" => self::coercion::widget_as::<TerminalView>(widget).is_some(),
            // `WidgetKind::RichEdit`
            "rich_edit" => self::coercion::widget_as::<RichEdit>(widget).is_some(),
            "code_editor" => self::coercion::widget_as::<CodeEditor>(widget).is_some(),
            "markdown_editor" => self::coercion::widget_as::<MarkdownEditor>(widget).is_some(),
            // `WidgetKind::StatusBar`
            "status_bar" => self::coercion::widget_as::<StatusBar>(widget).is_some(),
            "snackbar" => self::coercion::widget_as::<Snackbar>(widget).is_some(),
            // `WidgetKind::Canvas`
            "canvas" => self::coercion::widget_as::<Canvas>(widget).is_some(),
            "map_view" => self::coercion::widget_as::<MapView>(widget).is_some(),
            // `WidgetKind::Chart`
            "chart" => self::coercion::widget_as::<ChartWidget>(widget).is_some(),
            "gantt_widget" => self::coercion::widget_as::<GanttWidget>(widget).is_some(),
            "timeline_widget" => self::coercion::widget_as::<TimelineWidget>(widget).is_some(),
            // `WidgetKind::WebEngineView`. The canonical name is `web_engine_view`;
            // `web_view` and `webview` are aliases of it, so all three spellings must
            // resolve — an alias that resolves to a name absent from this table is an
            // alias that cannot address its own control.
            "web_engine_view" | "web_view" | "webview" => {
                self::coercion::widget_as::<WebView>(widget).is_some()
            }
            "media_player" => self::coercion::widget_as::<MediaPlayer>(widget).is_some(),
            // `WidgetKind::Toolbox`. Two registered names for one control.
            "tool_box" | "toolbox" => self::coercion::widget_as::<ToolBox>(widget).is_some(),
            // `WidgetKind::DataView`. `DataView` is `VirtualList` under a second name,
            // so one concrete check answers both rows.
            "virtual_list" | "data_view" => {
                self::coercion::widget_as::<VirtualList>(widget).is_some()
            }
            // `WidgetKind::ToolButton`
            "tool_button" => self::coercion::widget_as::<ToolButton>(widget).is_some(),
            "split_button" => self::coercion::widget_as::<SplitButton>(widget).is_some(),
            // `WidgetKind::PopupWindow`
            "popup_window" => self::coercion::widget_as::<PopupWindow>(widget).is_some(),
            "toast_stack" => self::coercion::widget_as::<ToastStack>(widget).is_some(),
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
