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
//! Currently every capability-bearing kind is registered, covering all major
//! control families: base widgets, inputs, containers, dialogs, displays,
//! menu/toolbar, advanced widgets, special widgets (productivity + rich media),
//! the macOS and Cupertino component sets, and web widgets. The authoritative
//! count is the factory's canonical-name list ([`WidgetFactory::widget_names`]);
//! no number is repeated here, so this paragraph cannot drift from the registry.
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
use crate::widget::menu_toolbar::menu::Menu;
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
use crate::widget::advanced_widgets::key_sequence_edit::KeySequenceEdit;
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::time_edit::TimeEdit;
#[cfg(full_widgets)]
use crate::widget::container_widgets::groupbox::GroupBox;
#[cfg(full_widgets)]
use crate::widget::container_widgets::splitter::Splitter;
#[cfg(full_widgets)]
use crate::widget::container_widgets::toolbox::ToolBox;
// The one concrete type behind four capability names. `password_edit`, `phone_input` and
// `email_input` are `LineEdit` with a constructor-installed mode or grammar, not a second
// widget, so the tie-break row below answers all four with the same concrete check.
#[cfg(full_widgets)]
use crate::widget::input_widgets::lineedit::LineEdit;

// The macOS (desktop) component set, and the three controls whose shared kind it refines. Each
// name that a kind group gains needs a row in the tie-break table below, because the kind alone
// cannot say which concrete control a mounted widget is — the same reason the types above are
// named here.
#[cfg(full_widgets)]
use crate::widget::container_widgets::stepper::Stepper;
#[cfg(full_widgets)]
use crate::widget::macos::{
    MacSegmentedControl, MacSourceList, MacStepper, MacToolbar, MacWindowChrome,
};
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::tool_bar::ToolBar;
#[cfg(full_widgets)]
use crate::widget::window::Window;
// The companion overlays that declare the kind of the control they partner (`COMP-21..29`).
// `Panel`, `Table`, `Slider`, `SearchBar` and `EmptyState` each gain at least one capability from
// this set, so every name needs a row in the concrete-type tie-break below.
#[cfg(full_widgets)]
use crate::widget::companions::{
    AxisPicker, CalendarMarkLayer, ChartDataTable, DragGhostLayer, FindReplaceBar,
    LineNumberGutter, ListStateHost, SummaryRow, ZoomStrip,
};
#[cfg(full_widgets)]
use crate::widget::display_widgets::empty_state::EmptyState;
#[cfg(full_widgets)]
use crate::widget::display_widgets::slider::Slider;
#[cfg(full_widgets)]
use crate::widget::input_widgets::search_bar::SearchBar;

/// Shared capability value and error types (`CapabilityValue`,
/// `CapabilityAccessError`, …) exchanged through the property contract below.
pub mod types;
pub use types::*;

/// Typed records and record lists — the carrier for a control's collection property (BLUE26 B26.09).
///
/// Compiled in every profile for the same reason as `target_facts`: it is pure data and predicates
/// over `CapabilityValue` and `PropertyValueKind`, which exist everywhere, and it walks no registry.
/// A caller supplies the schema; the module answers whether a list conforms to it.
pub mod records;
pub use records::{
    as_record_list, describe_records, RecordField, RecordList, RecordOperation, RecordSchema,
    RecordValue, RecordViolation, EMPTY_RECORD_SCHEMA,
};

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

/// Events every control inherits from [`BaseWidget`](crate::widget::BaseWidget).
///
/// # Why one shared table instead of two rows in every capability
///
/// Focus is emitted by `BaseWidget::handle_event` for every control the runtime routes focus to,
/// so it is the same two unit events for all of them. Writing those names into 180 capability
/// tables would be 360 copies of one fact, and the event census would then have to be edited to
/// add a focus event to a new control — a step a later control would forget (W-302). The lookup
/// consults this table after the capability's own list, so a control that ever needs to spell a
/// focus event differently still wins with its own row.
static BASE_EVENT_SCHEMAS: &[EventSchema] =
    &[EventSchema::unit("focus_gained"), EventSchema::unit("focus_lost")];

/// Finds the event schema `name` refers to on `capability`, using the same normalisation as every
/// other name lookup (`"value-changed"` and `"value_changed"` are the same event).
///
/// One implementation instead of four: the JSON binding, the JSON loader's publication check, the
/// designer generator and `connect_event` each used to spell the same `normalize_key` comparison.
/// An exact match is tried first, so the common already-canonical spelling allocates nothing.
///
/// A capability's own list is consulted first; [`BASE_EVENT_SCHEMAS`] then supplies the focus
/// events every control inherits from [`BaseWidget`](crate::widget::BaseWidget).
pub fn find_event_schema<'a>(
    capability: &'a WidgetCapability,
    name: &str,
) -> Option<&'a EventSchema> {
    if let Some(schema) = capability.events.iter().find(|schema| schema.name == name) {
        return Some(schema);
    }
    if let Some(schema) = capability.events.iter().find(|schema| same_key(schema.name, name)) {
        return Some(schema);
    }
    // Inherited base-widget events: subscribed through the control's own `event_signal_dyn` arms
    // (the `Widget` trait's default answers both focus names from `BaseWidget`).
    if let Some(schema) = BASE_EVENT_SCHEMAS.iter().find(|schema| same_key(schema.name, name)) {
        return Some(schema);
    }
    // Compatibility aliases (W-19/W-20/W-21): a spelling that differs by more than separators
    // (`cancelled` vs `canceled`) reaches the same published row. The pair is checked in both
    // directions, so the finder does not need a second row saying the reverse.
    for row in EVENT_ALIASES {
        if row.alias != name && row.canonical != name {
            continue;
        }
        if let Some(schema) = capability
            .events
            .iter()
            .find(|schema| schema.name == row.alias || schema.name == row.canonical)
        {
            return Some(schema);
        }
    }
    None
}

/// A normalised-name index over one capability's property table.
///
/// The table is a `&'static [PropertySchema]`, so its address identifies it for the lifetime of
/// the process and the index can be cached per thread without a lock. `len` is checked before a
/// hit is trusted, so a capability built over a different slice that happens to reuse the address
/// rebuilds instead of answering from the old table.
#[cfg(full_widgets)]
struct PropertyIndex {
    len: usize,
    /// Normalised-name hash -> the first schema position with that name (the same entry a linear
    /// `find` would return).
    by_hash: HashMap<u64, usize>,
}

#[cfg(full_widgets)]
thread_local! {
    /// Property indexes, keyed by the address of the `&'static [PropertySchema]` slice.
    ///
    /// The table has at most a few dozen entries per control and there are ~190 controls, so the
    /// map is bounded in practice; the cap is a guard against a host that builds and drops many
    /// custom factories on one thread.
    static PROPERTY_INDICES: crate::compat::RefCell<HashMap<usize, PropertyIndex>> =
        crate::compat::RefCell::new(HashMap::new());
}

/// The hash [`same_key`] implies: separators ignored and the rest lower-cased.
#[cfg(full_widgets)]
fn normalized_hash(name: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for ch in name.chars().filter(|ch| !matches!(*ch, '_' | '-' | ' ')).flat_map(char::to_lowercase)
    {
        for byte in (ch as u32).to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }
    hash
}

/// Finds a property schema by any spelling [`same_key`] accepts, in O(1) table probes (G-35).
///
/// # Why an index and not a linear `find`
///
/// Every `read_property` / `write_property` / `default_property_value` / `property_schema` call
/// used to scan the whole schema table, comparing names one at a time. The table is static and
/// immutable after construction, so a per-thread hash index turns the common case into one map
/// probe. A hash collision (or a different table at a reused address) falls back to the original
/// linear `find`, which keeps the first-match semantics exactly.
#[cfg(full_widgets)]
fn find_property<'a>(capability: &'a WidgetCapability, name: &str) -> Option<&'a PropertySchema> {
    let properties = capability.properties;
    if properties.is_empty() {
        return None;
    }
    let key = properties.as_ptr() as usize;
    let hash = normalized_hash(name);
    let index = PROPERTY_INDICES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 256 {
            cache.clear();
        }
        let entry = cache.entry(key).or_insert_with(|| {
            let mut by_hash = HashMap::new();
            for (index, schema) in properties.iter().enumerate() {
                by_hash.entry(normalized_hash(schema.name)).or_insert(index);
            }
            PropertyIndex { len: properties.len(), by_hash }
        });
        if entry.len != properties.len() {
            let mut by_hash = HashMap::new();
            for (index, schema) in properties.iter().enumerate() {
                by_hash.entry(normalized_hash(schema.name)).or_insert(index);
            }
            *entry = PropertyIndex { len: properties.len(), by_hash };
        }
        entry.by_hash.get(&hash).copied()
    });

    let Some(index) = index else {
        // No schema hashes to this value, so no spelling can compare equal to it.
        return None;
    };
    match properties.get(index) {
        Some(schema) if same_key(schema.name, name) => Some(schema),
        // A hash collision reached another property: fall back to the exact linear rule.
        _ => properties.iter().find(|schema| same_key(schema.name, name)),
    }
}

#[cfg(all(test, full_widgets))]
mod property_lookup_tests {
    use super::*;

    /// Every spelling `same_key` accepts must reach the same schema, and an unknown name must not.
    ///
    /// The index is built from the normalised form, so the separators and case are the parts that
    /// a hand-written lookup could easily disagree with (G-35).
    #[test]
    fn the_property_index_accepts_the_same_spellings_as_the_linear_rule() {
        let factory = WidgetFactory::shared();
        let canonical = factory.property_schema("combobox", "current_index").expect("schema");
        for spelling in ["current_index", "current-index", "current index", "CURRENT_INDEX"] {
            let found = factory.property_schema("combobox", spelling).expect(spelling);
            assert_eq!(
                found.name, canonical.name,
                "{spelling} must resolve to the canonical schema"
            );
        }
        assert_eq!(
            factory.property_schema("combobox", "no-such-property"),
            Err(CapabilityAccessError::UnknownProperty),
            "an unknown name is refused, not mapped to an arbitrary schema"
        );
    }

    /// `kind_canonical_name_into` derives snake_case in one pass and appends to the buffer.
    ///
    /// The derivation is the shared contract behind `factory_name_for_kind` and the alias-table
    /// gate (which re-implements the same transform lexically), so the capital-run rule and the
    /// append-not-truncate behaviour are pinned here (G-107).
    #[test]
    fn kind_canonical_name_is_snake_case_and_appends_in_place() {
        let mut buffer = alloc::string::String::from("prefix:");
        kind_canonical_name_into(crate::widget::WidgetKind::QRCode, &mut buffer);
        assert_eq!(buffer, "prefix:qrcode", "a capital run stays one word");

        kind_canonical_name_into(crate::widget::WidgetKind::ContextMenu, &mut buffer);
        assert_eq!(
            buffer, "prefix:qrcodecontext_menu",
            "the next name is appended without clearing the buffer"
        );

        for (kind, expected) in [
            (crate::widget::WidgetKind::Menu, "menu"),
            (crate::widget::WidgetKind::ProgressBar, "progress_bar"),
            (crate::widget::WidgetKind::ActivityIndicator, "activity_indicator"),
            (crate::widget::WidgetKind::MaterialNavigationRail, "material_navigation_rail"),
            (crate::widget::WidgetKind::CupertinoSegmentedControl, "cupertino_segmented_control"),
            (crate::widget::WidgetKind::FAB, "fab"),
            (crate::widget::WidgetKind::LCDNumber, "lcdnumber"),
        ] {
            let mut name = alloc::string::String::new();
            kind_canonical_name_into(kind, &mut name);
            assert_eq!(name, expected, "{kind:?} must derive {expected}");
        }
    }
}

/// Runs `f` with the canonical name of `kind`, reusing one scratch buffer per thread.
///
/// The allocating form above is fine for one-shot use, but
/// [`WidgetFactory::capability_by_kind`] is on the hover/property path and used it only to
/// compare against each entry's `canonical_name`. A per-thread scratch buffer removes that
/// allocation without interning the (bounded) set of kind names in a global map.
#[cfg(full_widgets)]
pub(crate) fn with_kind_canonical_name<R>(
    kind: crate::widget::WidgetKind,
    f: impl FnOnce(&str) -> R,
) -> R {
    thread_local! {
        static SCRATCH: crate::compat::RefCell<alloc::string::String> =
            const { crate::compat::RefCell::new(alloc::string::String::new()) };
    }
    SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        scratch.clear();
        kind_canonical_name_into(kind, &mut scratch);
        f(scratch.as_str())
    })
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
    // G-107: this function is documented as allocation-free, and now is. The previous
    // implementation formatted `{kind:?}` into `out`, then lowercased that segment through
    // two intermediate `String`s and indexed them with `chars().nth` — two allocations and
    // O(n²) indexing. `KindNameWriter` transforms each character as `Debug` writes it, so
    // the only storage touched is the caller's buffer and the scan is a single pass.
    let mut writer = KindNameWriter { out, started: false, previous_is_upper: false };
    let _ = write!(writer, "{kind:?}");
}

/// `fmt::Write` adapter that appends a variant name to the caller's buffer as `snake_case`.
///
/// `Debug` for a fieldless enum writes the variant identifier through `write_str`; this
/// adapter performs the snake_case derivation in the same pass (G-107). A run of capitals
/// (`QRCode`) stays one word, so only a capital preceded by a non-capital starts a new
/// word. The caller's buffer is the only storage written.
#[cfg(all(not(alloc_frugal), not(embedded_surface)))]
struct KindNameWriter<'a> {
    out: &'a mut alloc::string::String,
    /// Whether any character has been written: the first one never gets a leading `_`.
    started: bool,
    /// Whether the previous character was an ASCII capital.
    previous_is_upper: bool,
}

#[cfg(all(not(alloc_frugal), not(embedded_surface)))]
impl core::fmt::Write for KindNameWriter<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for ch in text.chars() {
            let is_upper = ch.is_ascii_uppercase();
            if is_upper && self.started && !self.previous_is_upper {
                self.out.push('_');
            }
            // `to_ascii_lowercase` leaves non-ASCII unchanged, matching the previous
            // segment-wide `to_ascii_lowercase`.
            self.out.push(ch.to_ascii_lowercase());
            self.started = true;
            self.previous_is_upper = is_upper;
        }
        Ok(())
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
/// `alias_factory_name` resolves those variants. The result is an `Option` because a
/// kind in the `embedded` widget set has no constructor at all, and the one place
/// that difference matters — `theme::apply_active_theme` — must be able to tell
/// "this kind has no name" from "this kind is named", rather than acting on the
/// empty string a `&str` would force it to receive.
///
/// No caller treats "no name" as an empty spelling any more: every one matches on
/// the `Option`, so a kind that cannot be named is reported where it is used
/// instead of being styled as nothing.
#[cfg(widgets_unstripped)]
pub fn factory_name_for_kind(kind: crate::widget::WidgetKind) -> Option<&'static str> {
    let name = {
        #[cfg(not(full_widgets))]
        {
            factory_name_for_kind_without_registry(kind)
        }
        #[cfg(full_widgets)]
        {
            if let Some(capability) = WidgetFactory::shared().capability_by_kind(kind) {
                return Some(capability.canonical_name);
            }
            alias_factory_name(kind)
        }
    };
    // `""` is not a name: the alias tables below answer it for a kind their profile
    // does not ship. Converting here keeps that single sentinel from leaking into
    // every caller as a special case.
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The alias table, shared by the registry-backed lookup above and the
/// registry-free lookup below.
///
/// # Why there are two spellings of one table, and how they are kept in step
///
/// This `match` is on **variants** and needs the full widget set (it names `ActivityIndicator`,
/// `ColumnView`, … which `embedded` compiles out). [`alias_for_name`] does the same lookup keyed
/// on the **canonical spelling**, and is the one that compiles where the variants are gated out.
/// A `cfg` boundary separates them, so no single build can see both — which is exactly how such a
/// pair drifts. The guard is therefore a **gate**, not a test: `check_alias_tables_agree.py`
/// parses both tables from this file and asserts every `(spelling, target)` pair in one is in the
/// other. Adding a row to one table alone fails that gate.
///
/// # The one anchor for a kind-level alias (G-305)
///
/// This table — not the kind declaration and not the aliased type's docs — is the single
/// fact source for every "kind X is kind Y under another name" relation. `ContextMenu` is
/// the deliberate example: `WidgetKind::ContextMenu`'s variant comment in `kind.rs` points
/// here, `Menu`'s type comment in `menu_toolbar/menu.rs` points here, and the
/// `context_menu` -> `menu` row in [`ALIAS_OF`] records the same pair for the name-keyed
/// consumers. Those three references describe one mechanism; a second place that decides
/// a kind alias is what principle #101 rules out.
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
/// # Why `Panel` and `MenuItem` are here
///
/// Both variants are legitimate `WidgetKind`s with a real constructor, but neither
/// has a capability row of its own, so `capability_by_kind` misses for both and
/// they used to reach the `other` arm and return `""`:
///
/// * `Panel` — `panel_capability()` declares `kind: WidgetKind::GroupBox` (the two
///   are the same control under two names, as `DockPanel` is to `DockWidget`), so
///   the `panel` name lives on a `GroupBox`-kinded row.
/// * `MenuItem` — a child created by its owning `Menu`, never by a factory call on
///   its own name, so it publishes no capability row; the `menu_item` name is what
///   the registry-free path derives from the variant.
///
/// An empty return is not harmless: `theme::apply_active_theme` bails out on it, so
/// a `Panel`- or `MenuItem`-kinded widget silently received no theme role (the
/// diagnostic was only `debug!`). Both rows were missing, which is the same class of
/// bug the `DockPanel` note above records.
///
/// Returns `None` for a kind with neither a capability nor an alias row, which the
/// caller surfaces as "this kind has no name" rather than as an empty spelling.
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
        // G-305: this arm is the single fact source for "`ContextMenu` is a `Menu` alias
        // kind" (`pub type ContextMenu = Menu;`). `WidgetKind::ContextMenu`'s variant
        // comment in `kind.rs` and `Menu`'s type comment in `menu_toolbar/menu.rs` point
        // here, and the `ALIAS_OF` row below records the same pair for the name-keyed
        // consumers. Change a kind-level alias here, not in those docs.
        crate::widget::WidgetKind::ContextMenu => "menu",
        crate::widget::WidgetKind::Wizard => "wizard_dialog",
        crate::widget::WidgetKind::DockPanel => "dock_widget",
        // `Panel` is `GroupBox` under another name; the capability row is keyed on
        // `GroupBox`, so this is an alias row, exactly like `DockPanel` above.
        crate::widget::WidgetKind::Panel => "panel",
        // A menu row: constructible, but only as a child of a `Menu`, so it has no
        // capability row of its own and must be named here.
        crate::widget::WidgetKind::MenuItem => "menu_item",
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
    // Build the name in a scratch buffer and look it up immediately. The name is not
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
///
/// See [`alias_factory_name`] for why the same eleven pairs appear twice and how
/// `check_alias_tables_agree.py` keeps the two spellings in step across the `cfg` boundary that
/// stops a single build from seeing both.
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
        // A menu row is constructible but created only as a child of a `Menu`, so it publishes no
        // capability row. It must still be nameable for the theme layer to classify it; the
        // spelling is its own, so it belongs here rather than in the alias table above.
        crate::widget::WidgetKind::MenuItem => "menu_item",
        _ => "",
    }
}

/// One capability name that is a second spelling of another control.
///
/// # Why this table is the single source of alias identity
///
/// A few capability rows name the same struct twice, because the second name adds a mode
/// rather than a control: `password_edit`, `phone_input` and `email_input` are `LineEdit` with
/// an echo mode or a validator installed, `split_layout` is the two-pane spelling of
/// `Splitter`, `time_picker` is `TimeEdit` under the kind's own name, and `context_menu` is
/// `Menu` in its role spelling. Each name keeps its own capability — its own published events
/// and defaults — so the rows must stay distinct; the *identity* underneath them must not be
/// restated.
///
/// Before this table that identity was written down in every consumer that needed it: the
/// payload deriver, the emit gate, the dynamic-wiring gate and the concrete-type tie-break each
/// carried their own copy. Copies of a fact drift, and a drifted copy in a gate produces the
/// worst failure a gate can have — a comparison between one control's published names and
/// another control's `event_signal_dyn` arms.
///
/// # Who reads it
///
/// `tools/check_event_signal_dyn.py` parses this table by its `AliasRow` shape and holds its
/// `CONVERTED` list to it: an alias must be converted, its row must name the canonical
/// control's struct and file, and its published names are compared against that struct's arms
/// rather than against its own narrower row. [`canonical_of`] and [`aliases_of`] are the
/// in-crate face of the same data.
///
/// `implementation` and `file` are stored, not derived from the alias: a row that names a
/// struct its file does not implement is a claim a reader can check, while a derived value
/// would restate the mistake it exists to catch.
pub struct AliasRow {
    /// The second spelling, as the census and `factory.create(..)` spell it.
    pub alias: &'static str,
    /// The control the alias spells — the row whose capability the same struct serves.
    ///
    /// Named `alias_of` rather than `canonical` because every consumer of this table (the census
    /// generator, the converted-control validator and the designer manifest) writes the relation
    /// into its own artifact under that key. One field name is what lets those three artifacts be
    /// compared without a translation table.
    pub alias_of: &'static str,
    /// The Rust type both names construct.
    pub implementation: &'static str,
    /// The file holding that type's `impl Widget`, relative to the crate root.
    pub file: &'static str,
}

/// Every capability name that is a second spelling of another capability's control.
///
/// Adding a row is what makes the alias addressable as one identity with its canonical name;
/// removing one is what makes the wiring gate treat it as a control of its own. Rows are
/// ordered by `alias` so the table reads as a lookup.
pub static ALIAS_OF: &[AliasRow] = &[
    AliasRow {
        alias: "context_menu",
        alias_of: "menu",
        implementation: "Menu",
        file: "src/widget/menu_toolbar/menu.rs",
    },
    AliasRow {
        alias: "email_input",
        alias_of: "line_edit",
        implementation: "LineEdit",
        file: "src/widget/input_widgets/lineedit.rs",
    },
    AliasRow {
        alias: "page_find_bar",
        alias_of: "web_find_bar",
        implementation: "WebFindBar",
        file: "src/widget/web_widgets/find_bar.rs",
    },
    AliasRow {
        alias: "password_edit",
        alias_of: "line_edit",
        implementation: "LineEdit",
        file: "src/widget/input_widgets/lineedit.rs",
    },
    AliasRow {
        alias: "phone_input",
        alias_of: "line_edit",
        implementation: "LineEdit",
        file: "src/widget/input_widgets/lineedit.rs",
    },
    AliasRow {
        alias: "split_layout",
        alias_of: "splitter",
        implementation: "Splitter",
        file: "src/widget/container_widgets/splitter.rs",
    },
    AliasRow {
        alias: "time_picker",
        alias_of: "time_edit",
        implementation: "TimeEdit",
        file: "src/widget/advanced_widgets/time_edit.rs",
    },
];

/// The control `name` is a second spelling of, or `None` when `name` is its own control.
pub fn resolve(name: &str) -> Option<&'static str> {
    ALIAS_OF.iter().find(|row| row.alias == name).map(|row| row.alias_of)
}

/// Compatibility spelling of [`resolve`].
///
/// Kept as a separate name rather than replaced: existing readers ask the question in the
/// canonical direction (`canonical_of(alias)`), while the designer manifest writes the answer
/// under the `alias_of` key. Both read the same table, so the two names cannot disagree.
pub fn canonical_of(name: &str) -> Option<&'static str> {
    resolve(name)
}

/// Every alias spelling of `canonical`, in table order. Empty when the name has no alias.
///
/// This is not [`WidgetFactory::aliases_of`]: that answers which *doc names* one row also
/// accepts (`btn` for `button`), while this answers which other **capability rows** the same
/// struct serves. A `context_menu` row cannot be an entry in `menu`'s `aliases` list — a name may
/// not be both an alias and a canonical row — which is why the relationship needs its own table.
pub fn aliases_of(canonical: &str) -> impl Iterator<Item = &'static str> + '_ {
    ALIAS_OF.iter().filter(move |row| row.alias_of == canonical).map(|row| row.alias)
}

/// One compatibility spelling of a published event name.
///
/// # Why aliases are data rather than extra schema rows
///
/// The published-name table is generated from signal declarations, and two spellings of one
/// signal cannot both be declared on it. A row here is the resolution rule instead: either side
/// of the pair reaches the control's one published schema, so an old document keeps working while
/// the canonical spelling is what the designer lists first. `note` records which side is preferred
/// and why, so a reader can tell a spelling fix from a semantic rename.
pub struct EventAlias {
    /// The spelling kept for compatibility.
    pub alias: &'static str,
    /// The spelling new documents should use.
    pub canonical: &'static str,
    /// Why the pair exists and which side is authoritative.
    pub note: &'static str,
}

/// Every compatibility spelling the event layer accepts.
///
/// The table is intentionally small and explicit: an alias is a published promise, so it is not
/// derived from string similarity (that would make `selected_changed` and `selection_changed`
/// collide even though their payloads differ).
///
/// # The backing invariant (W-19/W-20/W-21)
///
/// A row is only kept when **both** spellings resolve to a schema in a control's published
/// `events` table, and every published row is backed by an `.emit(..)` in production code (the
/// `check_capability_events_are_emitted` gate). `connect_event(alias, ..)` subscribes under the
/// resolved schema's own name, so it can only return `Ok` for a signal that fires. A spelling a
/// control has no concept for is not in this table at all.
pub static EVENT_ALIASES: &[EventAlias] = &[
    EventAlias {
        alias: "cancelled",
        canonical: "canceled",
        note: "W-19: `canceled` is the emitted field; the double-l spelling resolves to it",
    },
    EventAlias {
        alias: "edit_cancelled",
        canonical: "edit_canceled",
        note: "W-19: spelling variant that resolves to the same emitted edit-canceled signal",
    },
    EventAlias {
        alias: "current_index_changed",
        canonical: "current_changed",
        note: "W-20: single-current controls emit `current_changed`; the index spelling resolves",
    },
    EventAlias {
        alias: "selected_changed",
        canonical: "current_changed",
        note: "W-20: single-selection current item; multi-selection keeps `selection_changed`",
    },
    EventAlias {
        alias: "hovered_point_changed",
        canonical: "point_hover_changed",
        note: "W-21: subject-first hover name; the old subject-last spelling resolves",
    },
    EventAlias {
        alias: "hovered_glyph_changed",
        canonical: "glyph_hover_changed",
        note: "W-21: subject-first hover spelling",
    },
    EventAlias {
        alias: "hovered_entry",
        canonical: "entry_hovered",
        note: "W-21: subject-first hover spelling; the old name was subject-last",
    },
    EventAlias {
        alias: "hovered_changed",
        canonical: "hover_changed",
        note: "W-21: the subject is the control itself, so the template is `hover_changed`",
    },
];

/// The canonical spelling for an alias, or `None` when `name` is already canonical.
pub fn canonical_event_alias(name: &str) -> Option<&'static str> {
    EVENT_ALIASES.iter().find(|row| row.alias == name).map(|row| row.canonical)
}

/// Every compatibility spelling of `canonical`.
pub fn event_aliases_of(canonical: &str) -> impl Iterator<Item = &'static str> + '_ {
    EVENT_ALIASES.iter().filter(move |row| row.canonical == canonical).map(|row| row.alias)
}

/// The struct and file that implement `name`, whether `name` is a canonical spelling or an alias.
///
/// This is the third column pair of an [`AliasRow`]: the identity a caller needs when the
/// question is "which code answers for this name" rather than "what is this name called".
pub fn implementation_of(name: &str) -> Option<(&'static str, &'static str)> {
    ALIAS_OF
        .iter()
        .find(|row| row.alias == name || row.alias_of == name)
        .map(|row| (row.implementation, row.file))
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

/// The designer's JSON view of a capability: export, re-load, and the type vocabulary the two
/// sides share.
///
/// The module is compiled in every profile because the vocabulary (`value_kind_token`,
/// `shape_token` and their inverses) belongs to the property contract, which exists everywhere.
/// The three entry points that walk registered controls are `full_widgets`-gated individually —
/// there is no capability table to walk in a stripped profile — so the re-export here is split the
/// same way rather than gating the module and losing the vocabulary with it.
pub mod designer_manifest;
#[cfg(full_widgets)]
pub use designer_manifest::{
    all_capability_manifests_json, capability_manifest_json, designer_manifest,
};
pub use designer_manifest::{
    events_of_manifest, manifest_to_json, shape_from_token, shape_token, value_kind_from_token,
    value_kind_token, DesignerEvent, DesignerManifest, DesignerProperty, ManifestExportError,
    ManifestParseError,
};

/// What a capability statement is *about*: the build profile, the target triple, and how
/// well the fact was verified (BLUE26 B26.01 / B26.06).
///
/// Compiled in every profile for the same reason as `designer_manifest`: it is pure data and
/// predicates over compile-time facts, and its whole point is that a statement is never read
/// without knowing which target it describes. It walks no registry, so it needs no capability
/// table.
pub mod target_facts;
pub use target_facts::{
    probe_capability, Availability, CurrentBuildVerdict, TargetProbe, TargetSpec, VerificationLevel,
};

/// The typed command table a designer validates an event-to-command binding against
/// (BLUE26 B26.03).
///
/// Compiled in every profile: `PropertySchema` and `PropertyValueKind` exist everywhere, and the
/// module walks no registry — it answers for a command given a control's property list, which the
/// caller supplies.
pub mod command_schema;
pub use command_schema::{
    command_schema as command_schema_for, describe_command, described_verbs, effect_tokens,
    is_described_command, CommandEffect, CommandSchema, CommandSignature,
};

/// The wire-type compatibility table a designer and the code generator share.
///
/// Compiled in every profile for the same reason as `designer_manifest`: it is pure data and
/// predicates over `PropertyValueKind`, which exists everywhere. It walks no registry, so it does
/// not need the capability table.
pub mod wire_rules;
pub use wire_rules::{compatibility, WireCompatibility, WireRule, WireTarget, WIRE_RULES};

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

    /// The process-wide default factory, built once and then borrowed.
    ///
    /// # Why this exists
    ///
    /// `new_with_defaults` registers ~190 controls, each doing a `normalize_key` `String`
    /// allocation and two `HashMap` inserts. That cost was paid on every property read/write
    /// through a handle, every hover/press theme re-resolution, every view diff node, and every
    /// JSON load — all read-only uses of a table that never changes after construction.
    ///
    /// The factory is immutable once built (`register` needs `&mut self`), so a shared `OnceLock`
    /// is exact rather than approximate: a caller that needs to extend the table must build its
    /// own with [`Self::new`] + [`Self::register`].
    pub fn shared() -> &'static Self {
        static SHARED: crate::compat::OnceLock<WidgetFactory> = crate::compat::OnceLock::new();
        SHARED.get_or_init(Self::new_with_defaults)
    }

    /// Registers one widget capability and constructor.
    ///
    /// # Overwriting is deliberate here, and why the checked form exists
    ///
    /// This is the form the core registration table uses, where a control and its aliases are one
    /// coherent set and re-registering is how a table is built. For a **third party** — a plugin, a
    /// host adding its own control — silently replacing an existing name is the wrong default: the
    /// name a plugin picked may already belong to a core control, and the result would be a control
    /// that changed its appearance depending on what was loaded. See
    /// [`WidgetFactory::try_register`], which refuses a collision instead.
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

    /// Registers a control, refusing a name that is already taken.
    ///
    /// # Why this is the form an extension should use (BLUE24 §12 U-7)
    ///
    /// A plugin brings its own name. `register` would let it **replace a core control** — and because
    /// the replacement is silent, the symptom is not a failed load but a control that looks wrong, in
    /// a build whose only difference is that a plugin was enabled. Refusing the collision turns that
    /// into one error at load time.
    ///
    /// The check covers the canonical name **and every alias**, because an alias is a name the
    /// factory resolves: a plugin claiming `"btn"` would shadow whatever `"btn"` already meant, which
    /// is the same defect reached by a different spelling.
    ///
    /// Nothing is registered when the check fails. A partial registration (canonical taken, alias
    /// free) would leave the factory in a state where the same control is reachable under some of its
    /// names and not others.
    pub fn try_register(
        &mut self,
        capability: WidgetCapability,
        ctor: WidgetCtor,
    ) -> Result<(), RegistrationError> {
        if let Some(existing) = self.name_taken_by(capability.canonical_name) {
            return Err(RegistrationError::NameTaken { name: capability.canonical_name, existing });
        }
        for alias in capability.aliases {
            if let Some(existing) = self.name_taken_by(alias) {
                return Err(RegistrationError::NameTaken { name: alias, existing });
            }
        }
        self.register(capability, ctor);
        Ok(())
    }

    /// The canonical name of the control that already answers to `name`, if any.
    ///
    /// Answers with the **canonical** name rather than the spelling that matched, so an error can say
    /// "`btn` is already `button`" instead of "`btn` is already `btn`" — the first tells the caller
    /// which control they collided with, which is what makes the message actionable.
    pub fn name_taken_by(&self, name: &str) -> Option<&'static str> {
        with_normalized_key(name, |key| {
            let index = self.key_to_index.get(key).copied()?;
            self.capabilities.get(index).map(|capability| capability.canonical_name)
        })
    }

    /// Every name an extension may not use, for a caller that wants to check before building.
    ///
    /// Sorted, and including aliases, because the question "is this name free?" is asked about one
    /// name and answered against the whole space. A caller with a generated name (a namespaced plugin
    /// id) can assert its own reservation rather than discovering a collision at load time.
    pub fn reserved_names(&self) -> Vec<crate::compat::String> {
        let mut names: Vec<crate::compat::String> = self.key_to_index.keys().cloned().collect();
        names.sort_unstable();
        names
    }

    /// Creates a widget by canonical name or alias.
    pub fn create(
        &self,
        kind_or_name: &str,
        geometry: Rect,
        text: &str,
    ) -> Option<Box<dyn Widget>> {
        // The normalised spelling is only a map key, so it never needs an owned `String`.
        with_normalized_key(kind_or_name, |key| {
            self.constructors.get(key).map(|ctor| ctor(geometry, text))
        })
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
        with_normalized_key(kind_or_name, |key| {
            let idx = self.key_to_index.get(key).copied()?;
            self.capabilities.get(idx)
        })
    }

    /// The **canonical** name of whichever control answers to `kind_or_name`.
    ///
    /// # Why a caller needs this rather than the spelling it passed in
    ///
    /// A registry resolves aliases, so `"btn"`, `"pushbutton"` and `"button"` all create
    /// the same control. Any consumer that has to make a decision *per control* — rather
    /// than per spelling — must fold those spellings onto one name first. The designer's
    /// code generator is the motivating case: it probed availability with the factory
    /// (which resolves `"btn"`) but then emitted a constructor arm by matching the raw
    /// document string against its own table (which did not), so an alias resolved as
    /// available and then produced **no arm at all** — a generated program that built
    /// nothing, with a report that said it was clean.
    ///
    /// Returning the canonical name makes the two halves answer the same question about
    /// the same control, because there is only one canonical name per control.
    ///
    /// Answers with the name that was passed in — normalised — when nothing is
    /// registered, so a caller can still use the return value as a map key for an
    /// unknown control without a second existence check.
    pub fn canonical_name(&self, kind_or_name: &str) -> crate::compat::String {
        // One owned string for the answer either way; the normalised spelling only borrows the
        // stack buffer while the lookup runs.
        with_normalized_key(kind_or_name, |key| match self.key_to_index.get(key) {
            Some(index) => self
                .capabilities
                .get(*index)
                .map(|capability| crate::compat::String::from(capability.canonical_name))
                .unwrap_or_else(|| crate::compat::String::from(key)),
            None => crate::compat::String::from(key),
        })
    }

    /// Every canonical name the factory can construct, sorted and deduplicated.
    ///
    /// # Why this is derived rather than listed
    ///
    /// This is the set a *generator* must be able to name. A hand-maintained copy of it
    /// (the shape `constructor_path`'s match arms used to be) drifts the moment a control
    /// or an alias is added, and the drift is silent because the consumer of the stale
    /// copy reports success for a name it then cannot construct. Deriving it from the one
    /// registry makes "what the factory can build" and "what the generator can emit" the
    /// same fact by construction.
    pub fn constructible_names(&self) -> crate::compat::Vec<crate::compat::String> {
        let mut names: crate::compat::Vec<crate::compat::String> = self
            .capabilities
            .iter()
            .filter(|capability| {
                with_normalized_key(capability.canonical_name, |key| {
                    self.constructors.contains_key(key)
                })
            })
            .map(|capability| crate::compat::String::from(capability.canonical_name))
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// The concrete constructor registered for `kind_or_name`, for a caller that needs
    /// the **function** rather than an instance of what it builds.
    ///
    /// The generator uses this to reach a control's own `new` arity through the same
    /// registration the runtime uses, instead of keeping a parallel table of which
    /// constructors take text.
    pub fn constructor_for(&self, kind_or_name: &str) -> Option<WidgetCtor> {
        with_normalized_key(kind_or_name, |key| self.constructors.get(key).copied())
    }

    /// Whether any registered control declares `alias` among its aliases.
    ///
    /// Answers the reverse question to [`Self::capability`]: not "what does this name
    /// mean" but "is this name an alias of something". A generator that emits an arm per
    /// canonical name still has to accept the alias spellings a document may use, and
    /// this is how it knows which those are without keeping a second list.
    pub fn aliases_of(&self, canonical: &str) -> crate::compat::Vec<crate::compat::String> {
        self.capabilities
            .iter()
            .find(|capability| same_key(capability.canonical_name, canonical))
            .map(|capability| {
                capability.aliases.iter().map(|alias| crate::compat::String::from(*alias)).collect()
            })
            .unwrap_or_default()
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
    /// preserves the previous behaviour for the unambiguous majority. The one exception is
    /// `WidgetKind::Panel`: its only rows are companion overlays, its representative `panel`
    /// belongs to `WidgetKind::GroupBox`, and a first-registration answer would name one overlay
    /// for all four, so the answer there is `None` and callers use the kind's alias name.
    pub fn capability_by_kind(&self, kind: WidgetKind) -> Option<&WidgetCapability> {
        let indices = self.kind_to_index.get(&kind)?;
        // Compare through a per-thread scratch buffer: this used to allocate a `String` on every
        // call, and the callers (hover state, theme application, JSON loading) are hot.
        with_kind_canonical_name(kind, |expected| {
            indices
                .iter()
                .filter_map(|index| self.capabilities.get(*index))
                .find(|capability| capability.canonical_name == expected)
                // A kind whose own name has no capability falls back to its first entry. That is
                // wrong for `WidgetKind::Panel`: four companion overlays now declare the kind, so
                // "first" would name one overlay as the representative of all four — and make
                // `create_by_kind(Panel)` build a line-number rail instead of a plain panel.
                // `Panel` is the one kind whose representative belongs to another kind (`panel`
                // is a `GroupBox` capability); `factory_name_for_kind` already reaches it through
                // the alias table, and `create_by_kind` falls back to that name path. Answering
                // `None` keeps both halves on the plain panel row rather than a sibling.
                .or_else(|| {
                    if kind == WidgetKind::Panel {
                        None
                    } else {
                        self.capabilities.get(indices[0])
                    }
                })
        })
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

        let Some(property) = find_property(capability, property_name) else {
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

        let Some(property) = find_property(capability, property_name) else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        if !property.writable {
            return Err(CapabilityAccessError::ReadOnlyProperty);
        }

        // The schema's constraints are enforced here, before the write reaches the control, so a
        // value outside a declared range is reported with the property name and the refused reason
        // rather than being clamped silently by the control (BLUE26 B26.02). This is the one place
        // the capability layer can check without a live control's internals — the control remains
        // the authority for its own final state.
        if let Err(violation) = crate::widget::capability::types::validate_property_value(
            property.name,
            property,
            &value,
        ) {
            return Err(CapabilityAccessError::ConstraintViolation {
                property: property.name.into(),
                reason: crate::compat::format!("{violation}"),
            });
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
        // Resolve the caller's spelling to the **published** one and dispatch that. Passing the
        // raw argument through (as this did) meant `"clear-selection"` passed validation and then
        // reached the control as an unknown name, so a legitimate spelling produced the
        // misleading `UnsupportedOnWidget` ("the control was expected to have it") instead of
        // succeeding. The published spelling is what the control's own `command` matches on.
        let Some(canonical) =
            capability.commands.iter().copied().find(|name| same_key(name, command_name))
        else {
            return Err(CapabilityAccessError::UnknownCommand);
        };

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

        match properties.command(canonical) {
            Ok(()) => Ok(()),
            // The capability published the name but the control refused it. Reporting
            // the caller's name as unknown would send them to look for a different
            // control; `UnsupportedOnWidget` says the control was expected to have it.
            Err(CapabilityAccessError::UnknownCommand) => {
                log::warn!(
                    "widget {canonical:?} is published by capability {:?} but the control \
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
        // Normalised, so `"value-changed"` and `"value_changed"` reach the same event
        // — the same tolerance `invoke_command` and `read_property` provide.
        let canonical = self.canonical_event_name(control_name, event_name)?;

        // Subscribe under the event's **canonical** published spelling, not the caller's
        // raw argument. The validation above already normalised the argument to find a
        // match, but the hub is a plain string→signal map with no normalisation of its
        // own: connecting `"value-changed"` while the control emits under `"value_changed"`
        // (which is what `event_signal_dyn` / the dynamic wiring resolves to) created two
        // different hub entries, so the slot was accepted and then never fired. Using the
        // canonical name here is what makes a hyphenated, differently-cased or
        // space-separated spelling bind to the one signal the control actually emits.
        Ok(hub.connect(canonical, slot))
    }

    /// Resolves a caller's event spelling to the name the capability publishes.
    ///
    /// One lookup shared by [`Self::connect_event`] and [`Self::event_is_subscribable`],
    /// so a connect and its matching disconnect cannot use different spellings. That
    /// asymmetry was the subscription leak P0-17: the probe connected under
    /// `value_changed` and disconnected under the caller's `value-changed`, which the
    /// hub's exact lookup did not find.
    fn canonical_event_name(
        &self,
        control_name: &str,
        event_name: &str,
    ) -> Result<&'static str, CapabilityAccessError> {
        let capability =
            self.capability(control_name).ok_or(CapabilityAccessError::UnknownWidget)?;
        find_event_schema(capability, event_name)
            .map(|schema| schema.name)
            .ok_or(CapabilityAccessError::UnknownCommand)
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
        // Resolve the canonical spelling once, and use **that** name for both halves
        // of the probe. The connection and the disconnect must name the same hub key;
        // the previous code connected under the canonical name and disconnected under
        // the caller's raw spelling, so a tolerated spelling such as `value-changed`
        // left its slot behind on every probe (P0-17).
        let canonical = self.canonical_event_name(control_name, event_name)?;
        let handle = hub.connect(canonical, || {});
        let _ = hub.disconnect(canonical, handle);
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
        // The row key is the canonical name, so an alias is folded into it first. That identity is
        // stated once, in `ALIAS_OF`, and both spellings of one control therefore reach the same
        // row instead of each needing its own alternative in the patterns below.
        let canonical_name = canonical_of(canonical_name).unwrap_or(canonical_name);
        match canonical_name {
            // `WidgetKind::GroupBox`, which `panel` shares because `Panel` is a
            // `pub type` for `GroupBox`.
            "group_box" | "panel" => self::coercion::widget_as::<GroupBox>(widget).is_some(),
            // `WidgetKind::LineEdit`. Three validated variants share the kind with the plain
            // field, and all four are the same concrete control: the variant is a constructor
            // that installs an echo mode or a grammar, so `canonical_of` folds every one of them
            // onto this row. `KeySequenceEdit` also reports this kind (it is a text-shaped chord
            // recorder) but is a different concrete control, so it needs its own row.
            "line_edit" => self::coercion::widget_as::<LineEdit>(widget).is_some(),
            "key_sequence_editor" => self::coercion::widget_as::<KeySequenceEdit>(widget).is_some(),
            // `WidgetKind::Splitter`. `split_layout` is the two-pane spelling of the same
            // control, folded into this row by `canonical_of`.
            "splitter" => self::coercion::widget_as::<Splitter>(widget).is_some(),
            // `WidgetKind::TimePicker`. `time_edit` is the field and `time_picker` the kind's own
            // spelling of it, so one concrete check answers both rows.
            "time_edit" => self::coercion::widget_as::<TimeEdit>(widget).is_some(),
            // `WidgetKind::Menu`. `context_menu` is the role spelling of the same control.
            "menu" => self::coercion::widget_as::<Menu>(widget).is_some(),
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
            "toggle_button" => self::coercion::widget_as::<ToggleButton>(widget).is_some(),
            // `WidgetKind::SegmentedControl`. The desktop picker is the same concept drawn
            // with the desktop metrics, so each name answers **its own** concrete type: a
            // shared arm would make the first-registered capability claim the other control
            // and hand it a sibling's schema.
            "segmented_control" => self::coercion::widget_as::<SegmentedControl>(widget).is_some(),
            "mac_segmented_control" => {
                self::coercion::widget_as::<MacSegmentedControl>(widget).is_some()
            }
            // `WidgetKind::ListView`. `list_view` shares the kind with
            // `command_palette`, `notification_center` and `mac_source_list`, so all four need
            // a row: without one the lookup falls through to an empty schema and the
            // control reports `UnknownWidget` for its own properties.
            "list_view" => self::coercion::widget_as::<ListView>(widget).is_some(),
            "command_palette" => self::coercion::widget_as::<CommandPalette>(widget).is_some(),
            "notification_center" => {
                self::coercion::widget_as::<NotificationCenter>(widget).is_some()
            }
            "mac_source_list" => self::coercion::widget_as::<MacSourceList>(widget).is_some(),
            // `WidgetKind::TextEdit`
            "text_edit" => self::coercion::widget_as::<TextEdit>(widget).is_some(),
            // `WidgetKind::DockWidget`. `dock_area` is the host-area spelling of the same
            // concrete dock control, so one row answers both names.
            "dock_widget" | "dock_area" => self::coercion::widget_as::<
                crate::widget::container_widgets::dockwidget::DockWidget,
            >(widget)
            .is_some(),
            // SP-121: the chip input shares `WidgetKind::TextEdit`; without its own row it
            // would inherit the plain editor's schema and its four published events could not
            // be addressed from a live instance.
            "chip_input" => {
                self::coercion::widget_as::<crate::widget::special_widgets::chip::ChipInput>(widget)
                    .is_some()
            }
            "terminal_view" => self::coercion::widget_as::<TerminalView>(widget).is_some(),
            // `WidgetKind::RichEdit`
            "rich_edit" => self::coercion::widget_as::<RichEdit>(widget).is_some(),
            "code_editor" => self::coercion::widget_as::<CodeEditor>(widget).is_some(),
            "markdown_editor" => self::coercion::widget_as::<MarkdownEditor>(widget).is_some(),
            // `WidgetKind::StatusBar`
            "status_bar" => self::coercion::widget_as::<StatusBar>(widget).is_some(),
            // COMP-A23: the grid status strip shares `WidgetKind::StatusBar` with the message
            // bar; its own row keeps each control on its own schema.
            "grid_status_bar" | "table_status_bar" => {
                self::coercion::widget_as::<crate::widget::companions::GridStatusBar>(widget)
                    .is_some()
            }
            "group_avatar" => self::coercion::widget_as::<
                crate::widget::misc_widgets::avatar::GroupAvatar,
            >(widget)
            .is_some(),
            // MISC-010 added a second Avatar-kind capability, so the single avatar now needs its
            // own concrete row as well.
            "avatar" => {
                self::coercion::widget_as::<crate::widget::misc_widgets::avatar::Avatar>(widget)
                    .is_some()
            }
            "drop_down_popup_host" => {
                self::coercion::widget_as::<crate::widget::companions::DropDownPopupHost>(widget)
                    .is_some()
            }
            "command_palette_host" => {
                self::coercion::widget_as::<crate::widget::companions::CommandPaletteHost>(widget)
                    .is_some()
            }
            "header_menu" => {
                self::coercion::widget_as::<crate::widget::companions::HeaderMenu>(widget).is_some()
            }
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
            // `WidgetKind::ToolBar`. The macOS toolbar strip is the desktop rendition of the
            // same kind; each name is keyed to its own type so neither can claim the other.
            "tool_bar" => self::coercion::widget_as::<ToolBar>(widget).is_some(),
            "mac_toolbar" => self::coercion::widget_as::<MacToolbar>(widget).is_some(),
            // SP-022: the markdown toolbar shares `WidgetKind::ToolBar` with the plain toolbar
            // and the macOS strip. Without this row it inherits another control's schema (or an
            // empty one), so its own `action_triggered` capability cannot be addressed from a
            // live instance.
            "markdown_toolbar" | "md_toolbar" => self::coercion::widget_as::<
                crate::widget::special_widgets::markdown_editor::MarkdownToolbar,
            >(widget)
            .is_some(),
            // `WidgetKind::Stepper`. Same arrangement for the desktop stepper.
            "stepper" => self::coercion::widget_as::<Stepper>(widget).is_some(),
            "mac_stepper" => self::coercion::widget_as::<MacStepper>(widget).is_some(),
            // `WidgetKind::Window`. The title bar reports the window kind so the theme and the
            // accessibility role classify it as window chrome.
            "window" => self::coercion::widget_as::<Window>(widget).is_some(),
            "mac_window_chrome" => self::coercion::widget_as::<MacWindowChrome>(widget).is_some(),
            // `WidgetKind::PopupWindow`
            "popup_window" => self::coercion::widget_as::<PopupWindow>(widget).is_some(),
            "toast_stack" => self::coercion::widget_as::<ToastStack>(widget).is_some(),
            // ── Companion overlays (COMP-21..29) ──
            //
            // Each of these reports the kind of the control it partners, so the kind alone can
            // never say which control a mounted widget is. `Panel` gains four rows
            // (`line_number_gutter`, `axis_picker`, `calendar_mark_layer`, `drag_ghost_layer`),
            // `Table` two (`summary_row`, `chart_data_table`), and `Slider`, `SearchBar` and
            // `EmptyState` one each — which also gives the base control of those three kinds a
            // row it needs now that its kind group is no longer a single entry.
            "line_number_gutter" => self::coercion::widget_as::<LineNumberGutter>(widget).is_some(),
            "axis_picker" => self::coercion::widget_as::<AxisPicker>(widget).is_some(),
            "calendar_mark_layer" => {
                self::coercion::widget_as::<CalendarMarkLayer>(widget).is_some()
            }
            "drag_ghost_layer" => self::coercion::widget_as::<DragGhostLayer>(widget).is_some(),
            "summary_row" => self::coercion::widget_as::<SummaryRow>(widget).is_some(),
            "chart_data_table" => self::coercion::widget_as::<ChartDataTable>(widget).is_some(),
            "slider" => self::coercion::widget_as::<Slider>(widget).is_some(),
            "zoom_strip" => self::coercion::widget_as::<ZoomStrip>(widget).is_some(),
            "search_bar" => self::coercion::widget_as::<SearchBar>(widget).is_some(),
            "find_replace_bar" => self::coercion::widget_as::<FindReplaceBar>(widget).is_some(),
            "empty_state" => self::coercion::widget_as::<EmptyState>(widget).is_some(),
            "list_state_host" => self::coercion::widget_as::<ListStateHost>(widget).is_some(),
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

        let Some(property) = find_property(capability, property_name) else {
            return Err(CapabilityAccessError::UnknownProperty);
        };

        // A shared kind cannot name two record schemas at once; these two companions are
        // answered by canonical name before the kind-keyed table is read.
        #[cfg(all(not(alloc_frugal), full_widgets))]
        if let Some(value) =
            access::default_named_property_value(capability.canonical_name, property.name)
        {
            return Ok(value);
        }

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

        let Some(property) = find_property(capability, property_name) else {
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

        // Owned copies, because a manifest outlives the `&'static` capability table it came from.
        // The name is copied as well as the payload: a designer holds a manifest, and a borrowed
        // name would tie its lifetime to a table it may outlive.
        let events = capability
            .events
            .iter()
            .map(|schema| EventManifest {
                name: schema.name.into(),
                payload: schema.payload,
                shape: schema.shape,
            })
            .collect();

        Ok(WidgetCapabilityManifest {
            kind: capability.kind,
            canonical_name: capability.canonical_name,
            aliases: capability.aliases.to_vec(),
            properties,
            events,
            commands: capability.commands.to_vec(),
        })
    }
}
