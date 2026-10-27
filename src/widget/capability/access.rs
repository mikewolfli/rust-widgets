// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Id-level property access and the value/name codecs the schema layer publishes.
//!
//! Backends hold widget **ids**, not `&dyn Widget`, so [`read_widget_property_by_id`]
//! and [`write_widget_property_by_id`] are the entry points they use. Resolving the
//! id through `crate::widget::runtime` means the answer always describes the live
//! control rather than a copy (BLUE15 §10.3).
//!
//! # History
//!
//! This module used to also host `read_widget_property_legacy` /
//! `write_widget_property_legacy`: two large match-on-`widget.kind()` functions that
//! downcast the trait object and call native getters and setters, as a fallback for
//! controls whose readers had not yet moved onto their own `WidgetProperties`
//! impl (BLUE15 Phase C-1). Every registered control now implements the contract and
//! a test asserts it, so those two functions — and the nine `access_read_*.in.rs` /
//! `access_write_*.in.rs` arms they dispatched over — duplicated the contract for
//! 39 controls and have been deleted rather than left as dead code.
//!
//! What remains are the shared codecs below: they convert the enum-backed property
//! values to and from the strings the schema publishes, and several controls call
//! them directly from their own `set` / `get` arms.

/// The reflection entry points, re-exported so the id-level accessors in this module
/// resolve them from one place.
pub use super::properties_trait::{read_widget_property_by_name, write_widget_property_by_name};

#[cfg(full_widgets)]
use chrono::Weekday;

use crate::core::ObjectId;

#[cfg(full_widgets)]
use crate::widget::advanced_widgets::date_edit::Date;
#[cfg(full_widgets)]
use crate::widget::advanced_widgets::time_edit::Time;
#[cfg(full_widgets)]
use crate::widget::capability::coercion::*;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::container_widgets::scrollarea::ScrollBarPolicy;
use crate::widget::input_widgets::listbox::SelectionMode as ListBoxSelectionMode;
#[cfg(full_widgets)]
use crate::widget::menu_toolbar::tool_bar::ToolBarOrientation;
#[cfg(full_widgets)]
use crate::widget::view_widgets::data_grid::{ColumnFilter, SortSpec};
#[cfg(full_widgets)]
use crate::widget::view_widgets::list_view::{SelectionMode, ViewMode};
#[cfg(full_widgets)]
use crate::widget::WidgetKind;

/// Reads a property from the widget registered under `widget_id`.
///
/// The id-level counterpart to [`read_widget_property_by_name`]. Backends hold
/// widget **ids**, not `&dyn Widget`, so they need this shape; resolving the id
/// through `crate::widget::runtime` also means the answer always describes the
/// live control rather than a copy (BLUE15 §10.3).
///
/// Returns [`CapabilityAccessError::UnknownWidget`] when the id addresses nothing.
pub fn read_widget_property_by_id(
    widget_id: ObjectId,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    crate::widget::runtime::with_widget(widget_id, |widget| {
        read_widget_property_by_name(widget, property_name)
    })
    .unwrap_or(Err(CapabilityAccessError::UnknownWidget))
}

/// Writes a property to the widget registered under `widget_id`.
///
/// See [`read_widget_property_by_id`]. Asks the platform to repaint on success,
/// because a property write that changes what the control looks like must not
/// leave a stale frame on screen.
pub fn write_widget_property_by_id(
    widget_id: ObjectId,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    let written = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        write_widget_property_by_name(widget, property_name, value)
    })
    .unwrap_or(Err(CapabilityAccessError::UnknownWidget));
    if written.is_ok() {
        crate::widget::runtime::request_repaint(widget_id);
    }
    written
}

// ---------------------------------------------------------------------------
// Helper to-str / to-string conversions
// ---------------------------------------------------------------------------

#[cfg(full_widgets)]
/// Encodes a data-grid sort specification as a single string for the property
/// layer.
///
/// Each entry becomes `"<column>:asc"` or `"<column>:desc"`, joined with `,`.
/// The column *name* is written verbatim, so a name containing `:` or `,`
/// produces an ambiguous encoding that a reader cannot reliably split.
/// Empty input yields an empty string.
pub fn sort_specs_to_string(sort_specs: &[SortSpec]) -> String {
    sort_specs
        .iter()
        .map(|spec| format!("{}:{}", spec.column, if spec.descending { "desc" } else { "asc" }))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
/// Encodes data-grid column filters as a single `"<column>=<query>"` string,
/// joined with `,`.
///
/// As with [`sort_specs_to_string`], `=` and `,` inside a column name or query
/// make the result ambiguous. Empty input yields an empty string.
pub fn column_filters_to_string(filters: &[ColumnFilter]) -> String {
    filters
        .iter()
        .map(|filter| format!("{}={}", filter.column, filter.query))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(full_widgets)]
/// Returns the published token for a list-view selection mode: one of
/// `"single"`, `"multi"`, `"extended"`, or `"none"`.
///
/// These strings are part of the property schema and are matched by consumers,
/// so they must not be reworded.
pub fn selection_mode_to_str(mode: SelectionMode) -> &'static str {
    match mode {
        SelectionMode::Single => "single",
        SelectionMode::Multi => "multi",
        SelectionMode::Extended => "extended",
        SelectionMode::None => "none",
    }
}

/// The published token for a list-box selection mode.
///
/// `ListBox`'s `SelectionMode` is a type alias of the list view's, so this is a
/// spelling of the same mapping, not a second one — it delegates rather than
/// repeating the match (principle #54). It is available wherever `ListBox` is,
/// which is why it is not gated with the list-view-only converters above.
pub fn list_box_selection_mode_to_str(mode: ListBoxSelectionMode) -> &'static str {
    match mode {
        ListBoxSelectionMode::Single => "single",
        ListBoxSelectionMode::Multi => "multi",
        ListBoxSelectionMode::Extended => "extended",
        ListBoxSelectionMode::None => "none",
    }
}

#[cfg(full_widgets)]
/// Returns the published token for a list-view display mode: one of `"list"`,
/// `"icon"`, `"details"`, or `"thumbnails"`.
///
/// `thumbnails` exists in the codec even though the icon view may not render a
/// distinct thumbnail layout; the token is still valid to round-trip.
pub fn view_mode_to_str(mode: ViewMode) -> &'static str {
    match mode {
        ViewMode::List => "list",
        ViewMode::Icon => "icon",
        ViewMode::Details => "details",
        ViewMode::Thumbnails => "thumbnails",
    }
}

#[cfg(full_widgets)]
/// Returns the published token for a toolbar orientation: `"horizontal"` or
/// `"vertical"`.
pub fn tool_bar_orientation_to_str(orientation: ToolBarOrientation) -> &'static str {
    match orientation {
        ToolBarOrientation::Horizontal => "horizontal",
        ToolBarOrientation::Vertical => "vertical",
    }
}

/// Returns the published token for a scroll bar policy: `"always_on"`,
/// `"always_off"`, or `"as_needed"`.
///
/// Note the underscore spelling, unlike the other codecs in this module which
/// use single words — these tokens are the schema's, not a convention.
pub fn scroll_bar_policy_to_str(policy: ScrollBarPolicy) -> &'static str {
    match policy {
        ScrollBarPolicy::AlwaysOn => "always_on",
        ScrollBarPolicy::AlwaysOff => "always_off",
        ScrollBarPolicy::AsNeeded => "as_needed",
    }
}

pub use super::coercion::{
    alignment_to_str, check_state_to_str, lcd_mode_to_str, orientation_to_str,
    segment_style_to_str, tick_position_to_str,
};

#[cfg(full_widgets)]
/// Returns the published three-letter token for a weekday: `"mon"` through
/// `"sun"`.
///
/// Lowercase and fixed-length; unlike locale-aware day names, these are stable
/// identifiers suitable for storing and matching.
pub fn weekday_to_str(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "mon",
        Weekday::Tue => "tue",
        Weekday::Wed => "wed",
        Weekday::Thu => "thu",
        Weekday::Fri => "fri",
        Weekday::Sat => "sat",
        Weekday::Sun => "sun",
    }
}

#[cfg(full_widgets)]
/// Formats a [`Date`] as its `Display` representation for the property layer.
/// The exact layout is the date type's, not fixed here.
pub fn date_to_string(date: Date) -> String {
    date.to_string()
}

#[cfg(full_widgets)]
/// Formats a [`Time`] as its `Display` representation for the property layer.
/// The exact layout is the time type's, not fixed here.
pub fn time_to_string(time: Time) -> String {
    time.to_string()
}

// ---------------------------------------------------------------------------
// Default property value lookup
// ---------------------------------------------------------------------------

/// A canonical-name override for a default that a shared `WidgetKind` cannot answer.
///
/// Defaults are normally keyed by kind, because a kind names one visual role. Two of the new
/// companions share a kind with a control whose record schema is different (`command_palette_host`
/// with `ListView`, `drop_down_popup_host` with `Panel`), so the record default is answered here
/// by canonical name first.
#[cfg(full_widgets)]
pub fn default_named_property_value(
    canonical_name: &str,
    property_name: &str,
) -> Option<CapabilityValue> {
    match (canonical_name, property_name) {
        ("drop_down_popup_host", "items") => {
            Some(CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::POPUP_ITEM_SCHEMA,
            )))
        }
        ("command_palette_host", "entries") => {
            Some(CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::PALETTE_ENTRY_SCHEMA,
            )))
        }
        _ => None,
    }
}

#[cfg(full_widgets)]
/// Looks up the value a property reports before anything has written it.
///
/// Returns `None` when the kind/property pair is unknown to the schema, so a
/// `None` here means "no default is declared", not "the default is null".
/// Values are produced fresh on each call; enum-backed properties are returned
/// in their published string form.
pub fn default_widget_property_default_value(
    kind: WidgetKind,
    property_name: &str,
) -> Option<CapabilityValue> {
    // The properties every control inherits from `BaseWidget` get their defaults
    // here, once, rather than being repeated in every kind's arm below. Every
    // schema declares them (they are part of each control's published contract),
    // so a per-kind arm could only ever be a copy of these values.
    match property_name {
        "enabled" => return Some(CapabilityValue::Bool(true)),
        "tooltip" => return Some(CapabilityValue::String(String::new())),
        "geometry" => {
            return Some(CapabilityValue::String("0,0,0,0".to_string()));
        }
        // `visible` is shared by every control and declared by every schema. It
        // always means the base widget's visibility, which defaults to shown —
        // including for the popup kinds. Those publish their own popup state under a
        // *different* name (`shown` for `Tooltip`/`Popover`, `active` for
        // `ModalBottomSheet`, `message` plus this flag for `StatusBar`), precisely so
        // that bare `visible` keeps one meaning across the whole surface. The
        // widgets' own property tables say so explicitly (see `tooltip.rs`: "bare
        // `visible` keeps meaning what it means for every other control"), and the
        // live controls agree — a freshly constructed `Tooltip` reports
        // `visible == true` while `shown == false`.
        //
        // Those four kinds were excluded here and given a `false` arm further down,
        // which made the schema *disagree with the control it describes*. The value
        // is load-bearing: `view::apply::resolve_null_reset` substitutes this default
        // on every `CapabilityValue::Null` patch, so dropping a `visible` binding in a
        // view manifest would have hidden a `Tooltip` that the manifest never touched.
        // (`ModalBottomSheet` was the one genuine exception and is handled below,
        // together with `Menu`, whose constructor calls `hide()`.)
        // `Menu` is the one kind whose *base* visibility genuinely starts hidden:
        // its constructor ends with `base.hide()`, because a menu is opened rather
        // than displayed. `MaterialSnackbar` does the same, for the same reason and
        // with the same doc (see `cupertino/core.rs`: "The snackbar starts hidden.").
        //
        // `ModalBottomSheet` is excluded for the opposite reason: it overrides
        // `visible` in its own `WidgetProperties` (see `modal_bottom_sheet.rs`:
        // `"visible" => self.is_sheet_visible()`), so its arm below is the authority
        // for that name and this shared entry must not shadow it.
        "visible"
            if matches!(
                kind,
                WidgetKind::Menu
                    | WidgetKind::MaterialSnackbar
                    | WidgetKind::ModalBottomSheet
                    | WidgetKind::FindBar
                    | WidgetKind::WebFindBar
            ) =>
        {
            return Some(CapabilityValue::Bool(false));
        }
        "visible" => return Some(CapabilityValue::Bool(true)),
        _ => {}
    }

    // Properties published by the 2.9.2 contract round (issue-20261025). Each names exactly one
    // control (or two sharing one schema, for `thumb_color`), so they live here as one auditable
    // table instead of being spread across the per-kind arms below.
    let contract_default = match (kind, property_name) {
        // `virtual_table` shares `WidgetKind::Table`; `selected_cells` is declared only by its
        // schema, so this entry serves exactly that row. The value is the flattened row-major
        // `(row, column)` pairs of an empty selection grid.
        (WidgetKind::Table, "selected_cells") => {
            Some(CapabilityValue::Tuple(crate::compat::Vec::new()))
        }
        (WidgetKind::ScrollBar, "idle_delay_ms") => Some(CapabilityValue::UInt(600)),
        (WidgetKind::ScrollBar, "idle_opacity") => Some(CapabilityValue::Float(0.35)),
        (WidgetKind::Spinner, "track_color" | "arc_color") => Some(CapabilityValue::Null),
        (WidgetKind::Switch | WidgetKind::CupertinoSwitch, "thumb_color") => {
            Some(CapabilityValue::Null)
        }
        (WidgetKind::Arc, "value_format") => {
            Some(CapabilityValue::String("{percent}%".to_string()))
        }
        (WidgetKind::EmojiPicker, "recent_capacity") => Some(CapabilityValue::UInt(20)),
        (WidgetKind::ImageView, "rotation") => Some(CapabilityValue::String("0".to_string())),
        (WidgetKind::ImageView, "flip_horizontal" | "flip_vertical") => {
            Some(CapabilityValue::Bool(false))
        }
        (WidgetKind::ImageView, "zoom_level") => Some(CapabilityValue::Float(1.0)),
        (WidgetKind::ImageView, "pan_offset") => Some(CapabilityValue::String("0,0".to_string())),
        (WidgetKind::SkeletonLoader, "text_line_rows") => Some(CapabilityValue::UInt(3)),
        (WidgetKind::Divider, "line_style") => Some(CapabilityValue::String("plain".to_string())),
        (WidgetKind::Rating, "allow_half_step") => Some(CapabilityValue::Bool(false)),
        (WidgetKind::Avatar, "square") => Some(CapabilityValue::Bool(false)),
        (WidgetKind::Avatar, "size") => Some(CapabilityValue::UInt(40)),
        (WidgetKind::EmptyState, "secondary_action_text") => {
            Some(CapabilityValue::String(String::new()))
        }
        // `Tag` reports the cleared state as the token `none`, so the schema default says the same.
        (WidgetKind::Tag, "icon") => Some(CapabilityValue::String("none".to_string())),
        // `key_sequence_editor` publishes `LineEdit`'s kind; its own schema adds these four names.
        (WidgetKind::LineEdit, "key_sequence") => Some(CapabilityValue::String(String::new())),
        (WidgetKind::LineEdit, "recording" | "can_undo" | "can_redo") => {
            Some(CapabilityValue::Bool(false))
        }
        (WidgetKind::BezierCurveEditor, "segments") => Some(CapabilityValue::UInt(1)),
        (WidgetKind::BezierCurveEditor, "curve_points") => Some(CapabilityValue::String(
            "0.0000,0.0000;0.2500,0.1000;0.7500,0.9000;1.0000,1.0000".to_string(),
        )),
        (WidgetKind::Dropdown, "placeholder") => {
            Some(CapabilityValue::String("(Select)".to_string()))
        }
        (WidgetKind::Cascader, "placeholder") => {
            Some(CapabilityValue::String("Select…".to_string()))
        }
        (WidgetKind::AutoCompleteEdit, "placeholder") => {
            Some(CapabilityValue::String(String::new()))
        }
        (WidgetKind::MultiSelectComboBox, "selected_items" | "placeholder") => {
            Some(CapabilityValue::String(String::new()))
        }
        (WidgetKind::RangeSlider, "step") => Some(CapabilityValue::Float(1.0)),
        (WidgetKind::RangeSlider, "min_range") => Some(CapabilityValue::Float(0.0)),
        (WidgetKind::RangeSlider, "tick_interval") => Some(CapabilityValue::Null),
        (WidgetKind::RangeSlider, "tick_labels" | "value_hint_enabled") => {
            Some(CapabilityValue::Bool(false))
        }
        (WidgetKind::OtpInput, "read_only") => Some(CapabilityValue::Bool(false)),
        (WidgetKind::OtpInput, "digits_only") => Some(CapabilityValue::Bool(true)),
        (WidgetKind::TagInput, "multiline") => Some(CapabilityValue::Bool(false)),
        (WidgetKind::NumberPicker, "prefix") => Some(CapabilityValue::String(String::new())),
        // `special_value_text` is an `Option<String>`: a fresh picker has no special value, so the
        // property route reports `Null` and writes an empty string back as `Null` too.
        (WidgetKind::NumberPicker, "special_value_text") => Some(CapabilityValue::Null),
        (WidgetKind::NumberPicker, "decimals") => Some(CapabilityValue::UInt(0)),
        _ => None,
    };
    if let Some(value) = contract_default {
        return Some(value);
    }

    let value = match kind {
        WidgetKind::Button => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "pressed" => CapabilityValue::Bool(false),
            "default" => CapabilityValue::Bool(false),
            "icon_source" => CapabilityValue::Null,
            // Mirrors `Button::new`'s field initialisers, so a fresh button and the
            // schema's defaults agree.
            "icon_size" => {
                CapabilityValue::UInt(crate::widget::metrics::dimensions::BUTTON_ICON_SIZE as u64)
            }
            "shortcut" => CapabilityValue::Null,
            "auto_default" => CapabilityValue::Bool(false),
            "auto_exclusive" => CapabilityValue::Bool(false),
            "flat" => CapabilityValue::Bool(false),
            "elide" => CapabilityValue::String("right".to_string()),
            "drag_enabled" => CapabilityValue::Bool(false),
            "accepts_drops" => CapabilityValue::Bool(false),
            "auto_repeat" => CapabilityValue::Bool(false),
            "auto_repeat_delay" => CapabilityValue::UInt(500),
            "auto_repeat_interval" => CapabilityValue::UInt(100),
            "menu_open" => CapabilityValue::Bool(false),
            "menu_item_count" => CapabilityValue::UInt(0),
            "enabled" => CapabilityValue::Bool(true),
            "tooltip" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::Label => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            "word_wrap" => CapabilityValue::Bool(false),
            "elide" => CapabilityValue::String("none".to_string()),
            "margin" => CapabilityValue::UInt(0),
            "scaled_contents" => CapabilityValue::Bool(false),
            "indent" => CapabilityValue::UInt(0),
            "text_format" => CapabilityValue::String("plain".to_string()),
            "open_external_links" => CapabilityValue::Bool(false),
            "link_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::CheckBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "state" => CapabilityValue::String("unchecked".to_string()),
            "checked" => CapabilityValue::Bool(false),
            "tristate_enabled" => CapabilityValue::Bool(false),
            "auto_exclusive" => CapabilityValue::Bool(false),
            "auto_repeat" => CapabilityValue::Bool(false),
            "auto_repeat_delay" => CapabilityValue::UInt(500),
            "auto_repeat_interval" => CapabilityValue::UInt(100),
            _ => return None,
        },
        WidgetKind::RadioButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "group_id" => CapabilityValue::Null,
            // A radio is exclusive by construction; see `RadioButton::new`.
            "auto_exclusive" => CapabilityValue::Bool(true),
            "auto_repeat" => CapabilityValue::Bool(false),
            "auto_repeat_delay" => CapabilityValue::UInt(500),
            "auto_repeat_interval" => CapabilityValue::UInt(100),
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
            "track_click" => CapabilityValue::String("page".to_string()),
            "slider_position" => CapabilityValue::Int(0),
            // `zoom_strip` shares this kind. Its constructor starts at 100% over `0.1..=8.0`,
            // stepping by 0.25 with the Fit segment enabled, so the schema's defaults mirror it.
            "zoom_level" => CapabilityValue::Float(1.0),
            "min" => CapabilityValue::Float(0.1),
            "max" => CapabilityValue::Float(8.0),
            "step" => CapabilityValue::Float(0.25),
            "show_percentage" => CapabilityValue::Bool(true),
            "fit_actions" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::ProgressBar => match property_name {
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "value" => CapabilityValue::Int(0),
            "text_visible" => CapabilityValue::Bool(true),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "inverted_appearance" => CapabilityValue::Bool(false),
            "indeterminate" => CapabilityValue::Bool(false),
            "direction" => CapabilityValue::String("ltr".to_string()),
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
            "direction" => CapabilityValue::String("ltr".to_string()),
            "slider_size" => CapabilityValue::Float(0.1),
            "slider_position" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ListBox => match property_name {
            // A fresh list box has no rows; the empty list is what a dropped binding resets to.
            "items" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "item_count" => CapabilityValue::UInt(0),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "current_row" => CapabilityValue::Null,
            "item_height" => CapabilityValue::Float(20.0),
            "selected_count" => CapabilityValue::UInt(0),
            "selected_indices" => CapabilityValue::String(String::new()),
            "hovered_row" => CapabilityValue::Null,
            "font_size" => CapabilityValue::Float(14.0),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::SpinBox => match property_name {
            // Integer-shaped by default, matching `SpinBox::new`: `decimals == 0` is the
            // default precision, so the default value of these four is an `Int` like it
            // always was. A decimal-mode spin box reports `Float` through `get` instead;
            // the declared kind is `Number`, which is exactly this two-carrier promise.
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(99),
            "value" => CapabilityValue::Int(0),
            "single_step" => CapabilityValue::Int(1),
            "decimals" => CapabilityValue::UInt(0),
            "prefix" => CapabilityValue::String(String::new()),
            "suffix" => CapabilityValue::String(String::new()),
            // A fresh box holds its minimum with no decimals, no unit marks and no special text, so the
            // two derived strings are both `"0"` — the same answer `formatted_value` gives.
            "display_text" | "value_text" => CapabilityValue::String("0".to_string()),
            "special_value_text" => CapabilityValue::Null,
            "wrapping" => CapabilityValue::Bool(false),
            "keyboard_tracking" => CapabilityValue::Bool(true),
            // A fresh spin box is left-to-right, the same default `SpinBox::new` sets. Declared here as
            // well as in the schema because `schema_defaults_are_readable_and_writable_when_declared`
            // requires every declared property to answer its own default — a name the designer offers
            // and the control cannot answer is the defect that gate exists to find.
            "direction" => CapabilityValue::String("ltr".to_string()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::ComboBox => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            // An empty list rather than `Null`: a dropped binding resolves to this default, and
            // `Null` is a value the `StringList` schema check refuses.
            "items" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "current_index" => CapabilityValue::Null,
            "current_text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "minimum_contents_length" => CapabilityValue::UInt(0),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::UInt(10),
            "insert_policy" => CapabilityValue::String("insert_at_bottom".to_string()),
            "size_adjust_policy" => CapabilityValue::String("adjust_to_contents".to_string()),
            "open" => CapabilityValue::Bool(false),
            // The direction the field runs in. `ComboBox::new` starts left-to-right, so the schema's
            // default has to say the same thing or a manifest reset would silently mirror a field
            // the caller had never asked to change.
            "direction" => CapabilityValue::String("ltr".to_string()),
            "alignment" => CapabilityValue::String("left".to_string()),
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
            // The macOS title bar's own names. `chrome_height` is the height the desktop bar
            // actually paints (28) rather than the generic window row's 32, because a default
            // that disagreed with the control would make a dropped binding silently resize it.
            "chrome_height" => CapabilityValue::UInt(28),
            "buttons_visible" => CapabilityValue::Bool(true),
            "draggable" => CapabilityValue::Bool(true),
            "fullscreen" => CapabilityValue::Bool(false),
            "document_edited" => CapabilityValue::Bool(false),
            "focused" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::GroupBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(true),
            _ => return None,
        },
        // COMP-21 / COMP-26 / COMP-28 / COMP-29: the four companion overlays that report the
        // plain container kind. `panel` itself is a `GroupBox` row, so this arm serves only the
        // overlays; each default mirrors its control's constructor.
        WidgetKind::Panel => match property_name {
            "first_line" => CapabilityValue::UInt(1),
            "line_count" => CapabilityValue::UInt(0),
            "current_line" => CapabilityValue::UInt(0),
            "align" => CapabilityValue::String("right".to_string()),
            "marker_lines" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "series_names" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "assignments" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            // A fresh picker describes all four edges, each labelled by its token with auto-range
            // on, which is exactly `AxisPicker::new`'s list.
            "edges" => {
                let mut edges = crate::compat::Vec::new();
                for edge in ["left", "right", "top", "bottom"] {
                    // One spec per edge, built in place: a vector created and then pushed into is
                    // the shape `clippy::vec_init_then_push` flags.
                    let spec = crate::compat::Vec::from([
                        CapabilityValue::String(String::from(edge)),
                        CapabilityValue::String(String::from(edge)),
                        CapabilityValue::Bool(true),
                    ]);
                    edges.push(CapabilityValue::Tuple(spec));
                }
                CapabilityValue::Tuple(edges)
            }
            "year" => CapabilityValue::Int(2026),
            "month" => CapabilityValue::UInt(1),
            "max_per_cell" => CapabilityValue::UInt(3),
            "mark_count" => CapabilityValue::UInt(0),
            "title" => CapabilityValue::String(String::new()),
            "active" => CapabilityValue::Bool(false),
            "snap_back" => CapabilityValue::Bool(true),
            "opacity" => CapabilityValue::Float(0.82),
            "drop_bounds" => CapabilityValue::Null,
            "ghost_rect" | "origin_rect" => {
                CapabilityValue::Rect(crate::core::Rect::new(0, 0, 0, 0))
            }
            // COMP-A21 popup host (shares `Panel`).
            "open" => CapabilityValue::Bool(false),
            "highlighted_index" => CapabilityValue::Null,
            "scroll_offset" => CapabilityValue::Float(0.0),
            "max_visible_rows" => CapabilityValue::UInt(8),
            "anchor_rect" => CapabilityValue::Rect(crate::core::Rect::new(0, 0, 0, 0)),
            "placement" => CapabilityValue::String("automatic".to_string()),
            _ => return None,
        },
        WidgetKind::Splitter => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "pane_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Frame => match property_name {
            // Mirrors `Frame::new`'s field initialisers, so a freshly constructed
            // frame and the schema's defaults agree.
            "frame_shape" => CapabilityValue::String("box".to_string()),
            "frame_shadow" => CapabilityValue::String("plain".to_string()),
            "line_width" => CapabilityValue::Float(1.0),
            "mid_line_width" => CapabilityValue::Float(0.0),
            "frame_width" => CapabilityValue::Float(1.0),
            "contents_margins" => CapabilityValue::String("0,0,0,0".to_string()),
            "theme_bevel" => CapabilityValue::Bool(false),
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
            "shield" => CapabilityValue::Bool(false),
            "enabled" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::FontComboBox => match property_name {
            "current_font_family" => CapabilityValue::String("Arial".to_string()),
            "item_count" => CapabilityValue::Int(0),
            "current_index" => CapabilityValue::Int(-1),
            "editable" => CapabilityValue::Bool(false),
            "max_visible_items" => CapabilityValue::Int(10),
            "alignment" => CapabilityValue::String("left".to_string()),
            "font_filters" => CapabilityValue::String(String::new()),
            "writing_system_filter" => CapabilityValue::Null,
            "sample_text" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Action => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "icon_text" => CapabilityValue::String(String::new()),
            "icon_source" => CapabilityValue::Null,
            "shortcut" => CapabilityValue::String(String::new()),
            "checkable" => CapabilityValue::Bool(false),
            "checked" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::Bool(false),
            "command_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Toolbox => match property_name {
            "mode" => CapabilityValue::String("exclusive".to_string()),
            "item_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            "orientation" => CapabilityValue::String("vertical".to_string()),
            _ => return None,
        },
        WidgetKind::TabBar => match property_name {
            // A fresh strip has no tabs; the empty list of the declared schema is what a dropped
            // binding resets to (`Null` would be a value the property's schema refuses).
            "tabs" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::advanced_widgets::tab_bar::TAB_BAR_TAB_SCHEMA,
            )),
            "tab_count" => CapabilityValue::UInt(0),
            // A freshly built strip has no tabs, so no tab is selected: `Null` is the honest
            // default, and writing it clears the index.
            "current_index" => CapabilityValue::Null,
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_min_width" => CapabilityValue::UInt(40),
            "tab_max_width" => CapabilityValue::UInt(200),
            "direction" => CapabilityValue::String("ltr".to_string()),
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
            "show_week_numbers" => CapabilityValue::Bool(false),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "selected_dates" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::DatePicker => match property_name {
            "date" => CapabilityValue::String("2024-01-01".to_string()),
            "minimum_date" => CapabilityValue::String("1752-09-14".to_string()),
            "maximum_date" => CapabilityValue::String("9999-12-31".to_string()),
            "display_format" => CapabilityValue::String("yyyy-MM-dd".to_string()),
            "calendar_popup" => CapabilityValue::Bool(false),
            "alignment" => CapabilityValue::String("left".to_string()),
            "calendar_first_day" => CapabilityValue::String("sun".to_string()),
            _ => return None,
        },
        WidgetKind::TimePicker => match property_name {
            "time" => CapabilityValue::String("00:00:00".to_string()),
            "minimum_time" => CapabilityValue::String("00:00:00".to_string()),
            "maximum_time" => CapabilityValue::String("23:59:59".to_string()),
            "display_format" => CapabilityValue::String("HH:mm:ss".to_string()),
            "clock_popup" => CapabilityValue::Bool(false),
            "alignment" => CapabilityValue::String("left".to_string()),
            "time_format" => CapabilityValue::String("24h".to_string()),
            _ => return None,
        },
        WidgetKind::LineEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            // A fresh field allows the selection to be dragged out; see `LineEdit::new`.
            "drag_enabled" => CapabilityValue::Bool(true),
            // Mirrors `LineEdit::new`, which starts in `EchoMode::Normal`.
            "echo_mode" => CapabilityValue::String("normal".to_string()),
            "cursor_position" => CapabilityValue::UInt(0),
            // A fresh field shows nothing but its value, so every decoration slot is empty and the
            // derived pair is "no counter" / "nothing refused".
            "prefix" | "suffix" | "helper" | "error" => CapabilityValue::String(String::new()),
            "counter" => CapabilityValue::Null,
            "over_limit" => CapabilityValue::Bool(false),
            // A field without a grammar (`line_edit`, `password_edit`) is valid and has no
            // message; `phone_input` / `email_input` start empty, which both grammars accept.
            "valid" => CapabilityValue::Bool(true),
            "validation_state" => CapabilityValue::String("acceptable".to_string()),
            "validation_message" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            // Mirrors `LineEdit::new`: clean, with neither the clear affordance nor the
            // reveal-on-edit echo enabled, and the value scrolled to its leading edge.
            "modified" => CapabilityValue::Bool(false),
            "clear_button_enabled" => CapabilityValue::Bool(false),
            "password_echo_on_edit" => CapabilityValue::Bool(false),
            "scroll_x" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::ListView => match property_name {
            // `notification_center` is served by this kind; a fresh center has no rows. The empty
            // list of the declared schema is what a dropped binding resets to. (A plain `ListView`
            // has no `items` property, so this answer is never reached for it.)
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::notification_center::NOTIFICATION_ITEM_SCHEMA,
            )),
            "icon_visible_in_list_mode" => CapabilityValue::Bool(false),
            "has_model" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "focused_row" => CapabilityValue::Null,
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "view_mode" => CapabilityValue::String("list".to_string()),
            // The names below belong to the capabilities that share this kind
            // (`command_palette`, `notification_center`). Defaults are keyed by
            // kind, so the arm must answer for every capability that reports it;
            // each control's own `get` answers its real value.
            // `command_palette` shares this kind; a fresh palette has no entries. The empty list
            // of the declared schema is what a dropped binding resets to.
            "entries" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::command_palette::COMMAND_ENTRY_SCHEMA,
            )),
            "query" => CapabilityValue::String(String::new()),
            "entry_count" => CapabilityValue::UInt(0),
            "filtered_count" => CapabilityValue::UInt(0),
            // COMP-A30 command palette host (shares `ListView`).
            "filter" => CapabilityValue::String(String::new()),
            "highlighted_index" => CapabilityValue::Null,
            "item_count" => CapabilityValue::UInt(0),
            "unread_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            // The macOS source list's own names (see `MAC_SOURCE_LIST_PROPERTIES`): a fresh
            // sidebar has no sections and nothing selected.
            "section_count" => CapabilityValue::UInt(0),
            "selected_row" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(24),
            "alignment" => CapabilityValue::String("left".to_string()),
            "group_by_category" => CapabilityValue::Bool(false),
            "level_filter" => CapabilityValue::String("all".to_string()),
            "max_items" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::TreeView => match property_name {
            "expansion_animated" => CapabilityValue::Bool(false),
            "has_model" => CapabilityValue::Bool(false),
            "node_count" => CapabilityValue::UInt(0),
            "focused_node" => CapabilityValue::Null,
            "selected_node" => CapabilityValue::Null,
            "has_delegate" => CapabilityValue::Bool(false),
            // `Null` rather than `false`: a fresh tree has no focused branch, and "no branch" is not
            // the same fact as "the branch is collapsed".
            "expanded_focused_node" => CapabilityValue::Null,
            "lazy_loading" => CapabilityValue::Bool(false),
            "loading_indicator_visible" => CapabilityValue::Bool(false),
            "fetch_timeout_ms" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::TreeTable => match property_name {
            "header_visible" => CapabilityValue::Bool(true),
            "expands_on_double_click" => CapabilityValue::Bool(true),
            "middle_click_expands" => CapabilityValue::Bool(true),
            "sort_role" => CapabilityValue::String("text".to_string()),
            "case_sensitive" => CapabilityValue::Bool(false),
            "projection_strategy" => CapabilityValue::String("windowed".to_string()),
            "has_model" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "selected_row" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(20),
            "column_width" => CapabilityValue::UInt(140),
            "projection_state" => CapabilityValue::String("rows=0,selected=None".to_string()),
            "lazy_loading" => CapabilityValue::Bool(false),
            "loading_indicator_visible" => CapabilityValue::Bool(false),
            "fetch_timeout_ms" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Table => match property_name {
            "header_visible" => CapabilityValue::Bool(true),
            "editable" => CapabilityValue::Bool(true),
            "edit_triggers" => CapabilityValue::String("none".to_string()),
            "section_resize_mode" => CapabilityValue::String("fixed".to_string()),
            "selection_behavior" => CapabilityValue::String("cells".to_string()),
            "sort_indicator_visible" => CapabilityValue::Bool(true),
            "current_cell" => CapabilityValue::Null,
            "wheel_scroll_mode" => CapabilityValue::String("per_row".to_string()),
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
            // `diff_viewer` shares this kind; see the `ListView` note above.
            "left_text" => CapabilityValue::String(String::new()),
            "right_text" => CapabilityValue::String(String::new()),
            "line_count" => CapabilityValue::UInt(0),
            "change_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "view_mode" => CapabilityValue::String("side_by_side".to_string()),
            "word_granularity" => CapabilityValue::Bool(false),
            "fold_unchanged" => CapabilityValue::Bool(false),
            "fold_threshold" => CapabilityValue::UInt(8),
            "fold_context_lines" => CapabilityValue::UInt(0),
            "unified_changed_pair" => CapabilityValue::Bool(true),
            "whitespace_mode" => CapabilityValue::String("show".to_string()),
            "lazy_loading" => CapabilityValue::Bool(false),
            "loading_indicator_visible" => CapabilityValue::Bool(false),
            "fetch_timeout_ms" => CapabilityValue::UInt(0),
            "summary_text" => CapabilityValue::String(String::new()),
            // COMP-24 / COMP-27: `summary_row` and `chart_data_table` share this kind. A fresh
            // summary row has no aggregates and is not sticky; a fresh data table has no headers
            // or rows, starts in preview mode and caps its preview at 100 rows.
            "aggregates" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "sticky" => CapabilityValue::Bool(false),
            "headers" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "rows" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "visible_series" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "mode" => CapabilityValue::String("preview".to_string()),
            "max_rows" => CapabilityValue::UInt(100),
            _ => return None,
        },
        WidgetKind::DataView => match property_name {
            "selected_rows" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "selection_mode" => CapabilityValue::String("single".to_string()),
            "has_data_source" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "scroll_row" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(20),
            "overscan" => CapabilityValue::UInt(2),
            "selected_row" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::GridTable => match property_name {
            "sorting_enabled" => CapabilityValue::Bool(true),
            "sort_role" => CapabilityValue::String("display".to_string()),
            "auto_refresh" => CapabilityValue::Bool(true),
            "has_data_source" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            "scroll_row" => CapabilityValue::UInt(0),
            "scroll_column" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(24),
            "selection_mode" => CapabilityValue::String("cell".to_string()),
            "sort_spec_count" => CapabilityValue::UInt(0),
            "selected_cell" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::NumberPicker => match property_name {
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "step" => CapabilityValue::Int(1),
            "wrap" => CapabilityValue::Bool(false),
            "suffix" => CapabilityValue::String(String::new()),
            "row_count" => CapabilityValue::UInt(101),
            "selected_row" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(28),
            _ => return None,
        },
        WidgetKind::OtpInput => match property_name {
            "value" => CapabilityValue::String(String::new()),
            "length" => CapabilityValue::UInt(6),
            "masked" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::String(String::new()),
            "focused_index" => CapabilityValue::UInt(0),
            "is_complete" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Banner => match property_name {
            // A fresh banner has no action buttons; the empty list is what a dropped binding
            // resets to.
            "actions" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "text" => CapabilityValue::String(String::new()),
            "severity" => CapabilityValue::String("info".to_string()),
            "dismissible" => CapabilityValue::Bool(true),
            "dismissed" => CapabilityValue::Bool(false),
            "action_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Toast => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "level" => CapabilityValue::String("info".to_string()),
            // Matches `Toast::new`, which is the value a caller gets without asking:
            // a default that disagreed with the constructor would make a schema read
            // describe a control that cannot be built.
            "ttl_ms" => CapabilityValue::UInt(3000),
            "dismissible" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::CandlestickChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "overlay_count" => CapabilityValue::UInt(0),
            "show_price_levels" => CapabilityValue::Bool(false),
            "show_overlay_legend" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::VolumeChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "color_mode" => CapabilityValue::String(alloc::string::String::from("direction")),
            "headroom" => CapabilityValue::Float(0.92),
            _ => return None,
        },
        WidgetKind::DepthChart => match property_name {
            "depth" => CapabilityValue::UInt(0),
            "bid_color" => CapabilityValue::Color(crate::core::Color::rgb(38, 166, 91)),
            "ask_color" => CapabilityValue::Color(crate::core::Color::rgb(220, 68, 70)),
            _ => return None,
        },
        WidgetKind::OrderBook => match property_name {
            "depth" => CapabilityValue::UInt(5),
            "decimals" => CapabilityValue::Null,
            "show_spread" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::QuoteBoard => match property_name {
            "sort" => CapabilityValue::String(alloc::string::String::from("none")),
            "selected_index" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(22),
            "scroll_offset" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::IndicatorChart => match property_name {
            "series" => CapabilityValue::String(alloc::string::String::new()),
            "mode" => CapabilityValue::String(alloc::string::String::from("macd")),
            "period" => CapabilityValue::UInt(14),
            "show_reference_levels" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::SplashScreen => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "subtitle" => CapabilityValue::String(String::new()),
            // `Null` is the indeterminate state, matching `SplashScreen::new`.
            "progress" => CapabilityValue::Null,
            "skippable" => CapabilityValue::Bool(false),
            // No logo is the default accent block `SplashScreen::new` paints.
            "logo" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Pagination => match property_name {
            "wheel_paging_enabled" => CapabilityValue::Bool(true),
            "total" => CapabilityValue::UInt(0),
            "page_size" => CapabilityValue::UInt(10),
            "page" => CapabilityValue::UInt(0),
            "page_count" => CapabilityValue::UInt(1),
            "last_page" => CapabilityValue::UInt(0),
            "sibling_count" => CapabilityValue::UInt(1),
            "show_nav_buttons" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Menu => match property_name {
            // A fresh menu has no rows; the empty list of the declared schema is what a dropped
            // binding resets to (`Null` would be a value the property's schema refuses).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::menu_toolbar::menu::MENU_ITEM_SCHEMA,
            )),
            "title" => CapabilityValue::String(String::new()),
            "item_count" => CapabilityValue::UInt(0),
            "hovered_index" => CapabilityValue::Null,
            "direction" => CapabilityValue::String("ltr".to_string()),
            // A freshly constructed menu is a standalone popup and paints its heading; the
            // menu-bar path clears the flag when it creates one (see `create_menu`).
            "show_heading" => CapabilityValue::Bool(true),
            "column_count" => CapabilityValue::UInt(1),
            // COMP-A32 header menu (shares `Menu`).
            "sortable" => CapabilityValue::Bool(true),
            "filterable" => CapabilityValue::Bool(true),
            "sort_state" => CapabilityValue::String("none".to_string()),
            "filter_text" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::MenuBar => match property_name {
            // A fresh bar has no entries; the empty list of the declared schema is what a dropped
            // binding resets to.
            "entries" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::menu_toolbar::menu_bar::MENU_BAR_ENTRY_SCHEMA,
            )),
            "entry_count" => CapabilityValue::UInt(0),
            "active_index" => CapabilityValue::Null,
            "hovered_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ToolBar => match property_name {
            // A fresh strip has no items; the empty list of the declared schema is what a dropped
            // binding resets to.
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::menu_toolbar::tool_bar::TOOL_BAR_ITEM_SCHEMA,
            )),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "icon_size" => CapabilityValue::Float(24.0),
            "movable" => CapabilityValue::Bool(true),
            "floatable" => CapabilityValue::Bool(true),
            "item_count" => CapabilityValue::UInt(0),
            // SP-022: `markdown_toolbar` shares this kind. Its defaults are the control's own
            // constructor defaults (the standard action set, icons on the left, labels hidden).
            "actions" => CapabilityValue::Tuple(
                crate::widget::special_widgets::markdown_editor::MARKDOWN_TOOLBAR_ACTIONS
                    .iter()
                    .map(|token| CapabilityValue::String((*token).to_string()))
                    .collect(),
            ),
            "icon_side" => CapabilityValue::String("left".to_string()),
            "show_labels" => CapabilityValue::Bool(false),
            // The macOS toolbar's own names: an action strip is not selectable and has no
            // selected slot until the host asks for one.
            "selectable" => CapabilityValue::Bool(false),
            "selected_index" => CapabilityValue::Null,
            "overflow_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RibbonBar => match property_name {
            // A fresh ribbon has no tabs; the empty list is what a dropped binding resets to.
            "tabs" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "tab_count" => CapabilityValue::UInt(0),
            "current_tab" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(true),
            "minimized" => CapabilityValue::Bool(false),
            "quick_access_items" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "quick_access_overflow_menu" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ColorDialog => match property_name {
            "current_color" => CapabilityValue::String("#FFFFFFFF".to_string()),
            "modal" => CapabilityValue::Bool(true),
            "options_alpha" => CapabilityValue::Bool(false),
            // Mirrors `ColorDialog::new`: white is the second preset, and every picker section
            // starts hidden with an empty custom palette.
            "hex_rgba" => CapabilityValue::String("#FFFFFFFF".to_string()),
            "preset_index" => CapabilityValue::UInt(1),
            "show_hex_input" | "show_presets" | "show_hue_rail" => CapabilityValue::Bool(false),
            "custom_color_count" => CapabilityValue::UInt(0),
            "show_screen_picker" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // `ColorPicker` declares its own kind since BLUE16 phase E-6. The defaults
        // match the dialog's picker because both describe the same picker geometry:
        // the difference between them is the host window, not the control.
        WidgetKind::ColorPicker => match property_name {
            "hex_rgba" => CapabilityValue::String("#FF0000FF".to_string()),
            "show_alpha" => CapabilityValue::Bool(true),
            "preset_count" => CapabilityValue::UInt(6),
            "presets" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::String("#F44336FF".to_string()),
                CapabilityValue::String("#2196F3FF".to_string()),
                CapabilityValue::String("#4CAF50FF".to_string()),
                CapabilityValue::String("#FFC107FF".to_string()),
                CapabilityValue::String("#9C27B0FF".to_string()),
                CapabilityValue::String("#607D8BFF".to_string()),
            ])),
            "hue_degrees" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RichEdit => match property_name {
            // `rich_edit` and `code_editor` share this kind; a fresh editor has no diagnostic
            // markers, and the empty list of the declared schema is what a dropped binding resets
            // to. (A plain `RichEdit` has no `markers` property, so this answer is never reached
            // for it.)
            "markers" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::code_editor::DIAGNOSTIC_MARKER_SCHEMA,
            )),
            "text" => CapabilityValue::String(String::new()),
            "line_count" => CapabilityValue::UInt(0),
            "cursor_line" => CapabilityValue::UInt(0),
            "cursor_column" => CapabilityValue::UInt(0),
            "marker_count" => CapabilityValue::UInt(0),
            // `rich_edit` and `code_editor` share this kind; see the `ListView` note.
            "read_only" => CapabilityValue::Bool(false),
            "line_wrap" => CapabilityValue::Bool(false),
            "placeholder_text" => CapabilityValue::String(String::new()),
            // `markdown_editor` shares this kind; see the `ListView` note above.
            "preview_mode" => CapabilityValue::Bool(false),
            "word_count" => CapabilityValue::UInt(0),
            "heading_count" => CapabilityValue::UInt(0),
            "undo_stack_limit" => CapabilityValue::UInt(100),
            "undo_depth" => CapabilityValue::UInt(0),
            "goto_bar_visible" => CapabilityValue::Bool(false),
            "breakpoint_gutter" => CapabilityValue::Bool(false),
            "show_scrollbar" => CapabilityValue::Bool(true),
            "gfm_extensions" => CapabilityValue::Bool(true),
            "code_highlight" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Chart => match property_name {
            // `gantt_widget` shares this kind; a fresh chart has no tasks, and the empty list of
            // the declared schema is what a dropped binding resets to (`Null` would be a value the
            // schema refuses).
            "tasks" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::gantt_widget::GANTT_TASK_SCHEMA,
            )),
            "task_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "selected_marker_id" => CapabilityValue::Null,
            "viewport_start" => CapabilityValue::Int(0),
            "viewport_end" => CapabilityValue::Int(100),
            // `timeline_widget` shares this kind; its item list is records of the declared schema
            // and the empty list is what a dropped binding resets to. (No other Chart-kind
            // capability declares an `items` property, so this answer is never reached for them.)
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::timeline_widget::TIMELINE_ITEM_SCHEMA,
            )),
            // `timeline_widget` shares this kind; see the `ListView` note above.
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(24),
            // `chart` shares this kind; see the `ListView` note above.
            "chart_type" => CapabilityValue::String("bar".to_string()),
            "point_count" => CapabilityValue::UInt(0),
            "label_count" => CapabilityValue::UInt(0),
            "series_count" => CapabilityValue::UInt(0),
            "show_critical_path" => CapabilityValue::Bool(false),
            "show_today" => CapabilityValue::Bool(true),
            "show_time_labels" => CapabilityValue::Bool(false),
            "lane_mode" => CapabilityValue::String("row".to_string()),
            _ => return None,
        },
        WidgetKind::TextEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            // SP-121: `chip_input` shares this kind. Its committed items are a chip record list,
            // and the four scalar settings are the control's documented defaults.
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::chip::CHIP_ITEM_SCHEMA,
            )),
            "item_count" => CapabilityValue::UInt(0),
            "separator" => CapabilityValue::String(",".to_string()),
            "max_count" => CapabilityValue::UInt(0),
            "allow_duplicates" => CapabilityValue::Bool(false),
            "placeholder" => CapabilityValue::String(String::new()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "max_length" => CapabilityValue::Null,
            "read_only" => CapabilityValue::Bool(false),
            "line_wrap" => CapabilityValue::Bool(true),
            "output_line_count" => CapabilityValue::UInt(0),
            "input_line" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            "scrollback_lines" => CapabilityValue::UInt(5000),
            "wrap_output" => CapabilityValue::Bool(true),
            "show_scrollbar" => CapabilityValue::Bool(true),
            "caret_style" => CapabilityValue::String("line".to_string()),
            "caret_blink_enabled" => CapabilityValue::Bool(true),
            "prompt" => CapabilityValue::String("> ".to_string()),
            "selection_mode" => CapabilityValue::String("lines".to_string()),
            "ansi_palette" => CapabilityValue::Null,
            _ => return None,
        },

        // `Canvas` and `MapView` share `WidgetKind::Canvas`; defaults are keyed by
        // kind, so this arm answers for both controls.
        WidgetKind::Canvas => match property_name {
            // `canvas`
            "command_count" => CapabilityValue::UInt(0),
            // `map_view` — a fresh map has no markers; the empty list is what a dropped binding
            // resets to. (A plain `Canvas` has no `markers` property, so this answer is never
            // reached for it.)
            "markers" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::map_view::MAP_MARKER_SCHEMA,
            )),
            // `map_view`
            "center_x" => CapabilityValue::Float(0.0),
            "center_y" => CapabilityValue::Float(0.0),
            "zoom" => CapabilityValue::Float(1.0),
            "marker_count" => CapabilityValue::UInt(0),
            "selected_marker_id" => CapabilityValue::Null,
            "projection" => CapabilityValue::String("linear".to_string()),
            "layer_count" => CapabilityValue::UInt(0),
            "anti_alias" => CapabilityValue::Bool(true),
            "show_grid" => CapabilityValue::Bool(false),
            "grid_step" => CapabilityValue::UInt(20),
            "snap_to_grid" => CapabilityValue::Bool(false),
            "zoom_range" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Float(0.125),
                CapabilityValue::Float(16.0),
            ])),
            "zoom_step" => CapabilityValue::Float(1.1),
            _ => return None,
        },
        WidgetKind::Carousel => match property_name {
            "current_index" => CapabilityValue::UInt(0),
            "item_count" => CapabilityValue::UInt(0),
            "current_page_title" => CapabilityValue::String(String::new()),
            // Mirrors `Carousel::new`: wrap and autoplay are off, the indicator is a
            // dot row at the bottom.
            "loop" => CapabilityValue::Bool(false),
            "autoplay_interval" => CapabilityValue::Null,
            "indicator_style" => CapabilityValue::String("dots".to_string()),
            "indicator_position" => CapabilityValue::String("bottom".to_string()),
            _ => return None,
        },
        WidgetKind::WebEngineView => match property_name {
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
            "playlist_index" => CapabilityValue::Null,
            "track_count" => CapabilityValue::UInt(0),
            "user_agent" => CapabilityValue::String(
                crate::widget::web_widgets::web_engine::DEFAULT_USER_AGENT.to_string(),
            ),
            "background_transparent" => CapabilityValue::Bool(false),
            "cookie_persistence_enabled" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // `Breadcrumb` previously reported `WidgetKind::Panel` (a shared kind), so
        // this arm served it. It now has its own `WidgetKind::Breadcrumb` kind, so
        // the `segment_count` / `selected_index` defaults are declared on the kind
        // that actually owns them.
        WidgetKind::Breadcrumb => match property_name {
            // A control starts with no path, so the default is an **empty list of the declared
            // schema** rather than `Null`: `view::apply::resolve_null_reset` substitutes this default
            // for a dropped binding, and `Null` would be a value the property's schema check refuses.
            "segments" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::breadcrumb::BREADCRUMB_SEGMENT_SCHEMA,
            )),
            "segment_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "collapsed_count" => CapabilityValue::UInt(0),
            "separator" => CapabilityValue::String("/".to_string()),
            _ => return None,
        },
        WidgetKind::SignaturePad => match property_name {
            "stroke_count" => CapabilityValue::UInt(0),
            "stroke_width" => CapabilityValue::UInt(2),
            "stroke_color" => CapabilityValue::Color(crate::core::Color::rgb(20, 20, 20)),
            "min_point_distance" => CapabilityValue::Float(1.5),
            "min_point_interval_ms" => CapabilityValue::UInt(10),
            // D09-POINTER-01: pressure-driven width is on by default, and the retained pen datum
            // starts neutral (full pressure, no tilt) until a pointer event reports otherwise.
            "pressure_affects_width" => CapabilityValue::Bool(true),
            "last_pressure" => CapabilityValue::Float(1.0),
            "last_tilt_x" | "last_tilt_y" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::DropZone => match property_name {
            "accepted_type" => CapabilityValue::String("text/plain".to_string()),
            "state" => CapabilityValue::String("idle".to_string()),
            "hovered" => CapabilityValue::Bool(false),
            "accepted_types" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "multi_file" => CapabilityValue::Bool(false),
            "hover_style" => CapabilityValue::String("highlight".to_string()),
            "show_count_badge" => CapabilityValue::Bool(false),
            "dropped_count" => CapabilityValue::UInt(0),
            _ => return None,
        },

        WidgetKind::ToggleButton => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "state" => CapabilityValue::String("normal".to_string()),
            "auto_exclusive" => CapabilityValue::Bool(false),
            "auto_raise" => CapabilityValue::Bool(false),
            "group_id" => CapabilityValue::Null,
            "auto_repeat" => CapabilityValue::Bool(false),
            "auto_repeat_delay" => CapabilityValue::UInt(500),
            "auto_repeat_interval" => CapabilityValue::UInt(100),
            "icon_source" => CapabilityValue::Null,
            "arrow_type" => CapabilityValue::String("none".to_string()),
            "popup_mode" => CapabilityValue::String("instant".to_string()),
            "menu_open" => CapabilityValue::Bool(false),
            "menu_item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        // `SegmentedControl` has its own kind and therefore its own arm. These two arms used to be
        // one, because the control reported `WidgetKind::ToggleButton`: a segmentation control
        // answering `selected_id` and a toggle answering `state` from the same match is how a kind
        // collision hides — neither control declares the other's properties, but the shared arm
        // made `capability_by_kind` unable to say which was which.
        WidgetKind::SegmentedControl => match property_name {
            // A fresh control has no segments, and the empty list of the declared schema is what a
            // dropped binding resets to (`Null` would be a value the property's schema refuses).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::segmented_control::SEGMENT_ITEM_SCHEMA,
            )),
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            "selected_id" => CapabilityValue::Null,
            // The macOS picker's own derived count (see `MAC_SEGMENTED_CONTROL_PROPERTIES`).
            "segment_count" => CapabilityValue::UInt(0),
            "size_to_fit" => CapabilityValue::Bool(false),
            "min_segment_width" => CapabilityValue::UInt(56),
            "multi_select" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // A `Chip` declares these; `CheckListBox` is a *type alias for `ListBox`* and
        // is served by the `ListBox` arm below. The two were conflated before, which
        // left `WidgetKind::Chip` with no capability and made the list-box lookup
        // ambiguous.
        WidgetKind::Chip => match property_name {
            // A control starts with no chips, so the default is an **empty list of the declared
            // schema** rather than `Null`: `view::apply::resolve_null_reset` substitutes this default
            // for a dropped binding, and `Null` would be a value the property's schema check refuses
            // (B26.09).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::chip::CHIP_ITEM_SCHEMA,
            )),
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
            // `selected_count` and `page` follow the current selection/scroll position, so their
            // static default is the empty first page.
            "selected_count" => CapabilityValue::UInt(0),
            "page" => CapabilityValue::UInt(0),
            "selected_cell" => CapabilityValue::Null,
            "page_count" => CapabilityValue::UInt(1),
            "headers" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::FreeformShape => match property_name {
            "path_kind" => CapabilityValue::String("rounded_rect".to_string()),
            "fill_rgba" => CapabilityValue::String("#C8DCFFFF".to_string()),
            "stroke_rgba" => CapabilityValue::String("#5078C8FF".to_string()),
            "stroke_width" => CapabilityValue::UInt(2),
            "movable" => CapabilityValue::Bool(true),
            _ => return None,
        },
        // ── Always-available widget defaults (not mini-gated) ─────
        WidgetKind::Arc => match property_name {
            "value" => CapabilityValue::UInt(0),
            "minimum" => CapabilityValue::UInt(0),
            "maximum" => CapabilityValue::UInt(100),
            "thickness" => CapabilityValue::UInt(10),
            "sweep_angle" => CapabilityValue::UInt(360),
            // Mirrors `Arc::new`'s 12-o'clock origin.
            "start_angle" => CapabilityValue::Float(0.0),
            "indeterminate" => CapabilityValue::Bool(false),
            "rounded" => CapabilityValue::Bool(false),
            "show_value" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Spinner => match property_name {
            "active" => CapabilityValue::Bool(true),
            "thickness" => CapabilityValue::UInt(4),
            "speed" => CapabilityValue::Float(1.0),
            "size_ratio" => CapabilityValue::Float(0.8),
            // Mirrors `SpinnerStyle::default()`.
            "style" => CapabilityValue::String("arc".to_string()),
            _ => return None,
        },
        WidgetKind::Roller => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            "visible_count" => CapabilityValue::UInt(5),
            "item_count" => CapabilityValue::UInt(0),
            "options" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::Dropdown => match property_name {
            // A fresh dropdown has no rows; the empty list is what a dropped binding resets to.
            "items" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "text" => CapabilityValue::String(String::new()),
            "selected_index" => CapabilityValue::UInt(0),
            "item_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            "max_visible" => CapabilityValue::Null,
            "scroll_offset" => CapabilityValue::UInt(0),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::TextArea => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String(String::new()),
            "read_only" => CapabilityValue::Bool(false),
            // `0` is unlimited, matching `TextArea::new`.
            "max_length" => CapabilityValue::UInt(0),
            // A fresh area starts unwrapped, flush with its inset, with no block budget and
            // no horizontal scroll.
            "soft_wrap" => CapabilityValue::Bool(false),
            "content_margin" => CapabilityValue::UInt(0),
            "max_block_count" => CapabilityValue::Null,
            "scroll_x" => CapabilityValue::Int(0),
            _ => return None,
        },
        WidgetKind::Keyboard => match property_name {
            "layout" => CapabilityValue::String("qwerty".to_string()),
            "lowercase" => CapabilityValue::Bool(true),
            "press_animation" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // `CupertinoSwitch` publishes the same schema as `Switch` (it is the
        // iOS-styled sibling of the same control), so it shares this arm rather
        // than repeating the defaults. Registering the kind without adding it
        // here left `default_property_value("cupertino_switch", "checked")`
        // answering `None` while the schema declared the property readable,
        // which `schema_defaults_are_readable_and_writable_when_declared`
        // rejects.
        WidgetKind::Switch | WidgetKind::CupertinoSwitch => match property_name {
            "checked" => CapabilityValue::Bool(false),
            // The ON track takes the theme's accent unless the caller names a colour, so "not
            // set" is `Null` — the same optional-colour default the other override properties
            // use.
            "on_track_color" => CapabilityValue::Null,
            // Mirrors `SwitchGeometry::default()` (`Material`).
            "switch_geometry" => CapabilityValue::String("material".to_string()),
            _ => return None,
        },
        WidgetKind::Line => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            // Mirrors `Line::new`'s default thickness and its literal fallback colour.
            "thickness" => CapabilityValue::UInt(2),
            "color" => CapabilityValue::Color(crate::core::Color::rgb(180, 180, 180)),
            _ => return None,
        },
        WidgetKind::Meter => match property_name {
            "value" => CapabilityValue::UInt(0),
            "minimum" => CapabilityValue::UInt(0),
            "maximum" => CapabilityValue::UInt(100),
            // Mirrors `Meter::new`.
            "tick_count" => CapabilityValue::UInt(5),
            "show_tick_labels" => CapabilityValue::Bool(false),
            "unit" => CapabilityValue::String(String::new()),
            "value_text" => CapabilityValue::String("0".to_string()),
            "threshold_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RadarChart => match property_name {
            "axis_count" => CapabilityValue::UInt(0),
            "series_count" => CapabilityValue::UInt(0),
            // Mirrors `RadarChart::new`.
            "show_grid" => CapabilityValue::Bool(true),
            "show_axis_labels" => CapabilityValue::Bool(true),
            "show_legend" => CapabilityValue::Bool(true),
            "hovered_axis" => CapabilityValue::Null,
            "series_names" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::KanbanBoard => match property_name {
            // A fresh board has no columns and no cards; the empty list of the declared schema is
            // what a dropped binding resets to (`Null` would be a value the schema refuses).
            "columns" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::kanban_board::KANBAN_COLUMN_SCHEMA,
            )),
            "cards" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::kanban_board::KANBAN_CARD_SCHEMA,
            )),
            "column_count" => CapabilityValue::UInt(0),
            "card_count" => CapabilityValue::UInt(0),
            "dragging_card_id" => CapabilityValue::Null,
            "hovered_column" => CapabilityValue::Null,
            // Mirrors the `COLUMN_WIDTH` layout constant in the control.
            "column_width" => CapabilityValue::UInt(220),
            "horizontal_scroll_offset" => CapabilityValue::UInt(0),
            // The flat board is the default; lanes must be turned on explicitly.
            "lane_mode" => CapabilityValue::String("none".to_string()),
            "swimlanes" => CapabilityValue::Bool(false),
            "lane_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Heatmap => match property_name {
            "row_count" => CapabilityValue::UInt(0),
            "column_count" => CapabilityValue::UInt(0),
            // `None` means "derive the scale from the data", so the default is a null
            // rather than a fabricated 0..1 range — a fabricated range would make an
            // empty control report a colour scale it is not using.
            "scale_minimum" => CapabilityValue::Null,
            "scale_maximum" => CapabilityValue::Null,
            "resolved_minimum" => CapabilityValue::Null,
            "resolved_maximum" => CapabilityValue::Null,
            // Mirrors `Heatmap::new`.
            "show_labels" => CapabilityValue::Bool(true),
            "show_legend" => CapabilityValue::Bool(true),
            "viewport_rows" => CapabilityValue::Null,
            "viewport_columns" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Cascader => match property_name {
            // Nothing is selected on a fresh control; the empty index sequence is what a dropped
            // binding resets to.
            "selected_path" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "value" => CapabilityValue::String(String::new()),
            "depth" => CapabilityValue::UInt(0),
            // Mirrors `Cascader::new`.
            "expanded" => CapabilityValue::Bool(false),
            "separator" => CapabilityValue::String("/".to_string()),
            "filterable" => CapabilityValue::Bool(false),
            "filter" => CapabilityValue::String(String::new()),
            "max_visible" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::QueryBuilder => match property_name {
            // A fresh builder has neither fields nor rows; the empty lists of the declared schemas
            // are what a dropped binding resets to.
            "fields" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::view_widgets::query_builder::FILTER_FIELD_SCHEMA,
            )),
            "rows" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::view_widgets::query_builder::QUERY_ROW_SCHEMA,
            )),
            "live_update" => CapabilityValue::Bool(true),
            "commit_delay_ms" => CapabilityValue::UInt(300),
            "has_pending_commit" => CapabilityValue::Bool(false),
            "row_count" => CapabilityValue::UInt(0),
            "incomplete_row_count" => CapabilityValue::UInt(0),
            "field_count" => CapabilityValue::UInt(0),
            // Mirrors `QueryBuilder::new`.
            "conjunction" => CapabilityValue::String("and".to_string()),
            "active_row" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::EmojiPicker => match property_name {
            // A fresh picker has no glyphs and no tabs; the empty values are what a dropped
            // binding resets to.
            "glyphs" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::display_widgets::emoji_picker::EMOJI_GLYPH_SCHEMA,
            )),
            "categories" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "glyph_count" => CapabilityValue::UInt(0),
            "visible_glyph_count" => CapabilityValue::UInt(0),
            "tab_count" => CapabilityValue::UInt(0),
            // Mirrors `EmojiPicker::new`.
            "active_tab" => CapabilityValue::UInt(0),
            "search" => CapabilityValue::String(String::new()),
            "recent_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Mention => match property_name {
            // A fresh control has no candidates; the empty list of the declared schema is what a
            // dropped binding resets to.
            "candidates" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::input_widgets::mention::MENTION_CANDIDATE_SCHEMA,
            )),
            "text" => CapabilityValue::String(String::new()),
            // Mirrors `Mention::new`.
            "trigger" => CapabilityValue::String("@".to_string()),
            "candidate_count" => CapabilityValue::UInt(0),
            "visible_candidate_count" => CapabilityValue::UInt(0),
            "mention_count" => CapabilityValue::UInt(0),
            "popup_open" => CapabilityValue::Bool(false),
            "caret" => CapabilityValue::UInt(0),
            // Mirrors `Mention::new` (`MAX_VISIBLE` = 6 rows of `ROW_HEIGHT` = 24).
            "max_visible" => CapabilityValue::UInt(6),
            "row_height" => CapabilityValue::UInt(24),
            _ => return None,
        },
        WidgetKind::MiniChart => match property_name {
            // A fresh chart has no samples; the empty list of the declared schema is what a dropped
            // binding resets to.
            "data" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::display_widgets::mini_chart::MINI_CHART_DATA_SCHEMA,
            )),
            // Mirrors `MiniChart::new`'s axis bounds.
            "min_value" => CapabilityValue::UInt(0),
            "max_value" => CapabilityValue::UInt(100),
            "chart_type" => CapabilityValue::String("line".to_string()),
            "hover_enabled" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::ImageView => match property_name {
            "scaled" => CapabilityValue::Bool(false),
            _ => return None,
        },
        // ── Dialog widgets ──────────────────────────────────
        WidgetKind::Dialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "modal" => CapabilityValue::Bool(true),
            "has_content" => CapabilityValue::Bool(false),
            // `Dialog::new` starts with no outcome recorded and no close affordance.
            "result" => CapabilityValue::Null,
            "accepted" => CapabilityValue::Bool(false),
            "rejected" => CapabilityValue::Bool(false),
            "closable" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::MessageBox => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "text" => CapabilityValue::String(String::new()),
            // Mirrors `MessageBox::new`, which starts at `MessageBoxIcon::NoIcon`.
            "icon" => CapabilityValue::String("none".to_string()),
            "modal" => CapabilityValue::Bool(true),
            // The supplementary text and the checkbox label start empty, the checkbox is
            // unchecked, Enter answers the OK button and Escape answers no button.
            "informative_text" | "detailed_text" | "check_box" => {
                CapabilityValue::String(String::new())
            }
            "check_box_checked" => CapabilityValue::Bool(false),
            "default_button" => CapabilityValue::String("ok".to_string()),
            "escape_button" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::FileDialog => match property_name {
            "title" => CapabilityValue::String("Open File".to_string()),
            "modal" => CapabilityValue::Bool(true),
            "directory" => CapabilityValue::String(String::new()),
            "selected_file" => CapabilityValue::Null,
            "mode" => CapabilityValue::String("open_file".to_string()),
            // No prompt or suffix is preset, the host has supplied no rows yet, and a fresh
            // dialog sits at the start of its history.
            "label_text" | "default_suffix" => CapabilityValue::String(String::new()),
            "file_count" => CapabilityValue::UInt(0),
            "can_go_back" | "can_go_forward" => CapabilityValue::Bool(false),
            "view_mode" => CapabilityValue::String("list".to_string()),
            "sidebar_urls" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::FontDialog => match property_name {
            "modal" => CapabilityValue::Bool(true),
            // Mirrors `FontDialog::new`: no face option is set and the preview is on.
            "underline" | "strikethrough" | "no_buttons" => CapabilityValue::Bool(false),
            "show_preview" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::InputDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "mode" => CapabilityValue::String("text".to_string()),
            "text_value" => CapabilityValue::String(String::new()),
            "int_value" => CapabilityValue::Int(0),
            "double_value" => CapabilityValue::Float(0.0),
            "int_step" => CapabilityValue::Int(1),
            "current_item" => CapabilityValue::UInt(0),
            "dropdown_open" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ProgressDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "label_text" => CapabilityValue::String(String::new()),
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            // `cancel()` returns the value to the minimum, the button and the bar are
            // painted, and the bar neither eases nor takes the accent colour until asked.
            "auto_reset" | "show_cancel_button" | "show_bar" => CapabilityValue::Bool(true),
            "minimum_duration" => CapabilityValue::UInt(0),
            "animated" | "accent_fill" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::PopupWindow => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "has_content" => CapabilityValue::Bool(false),
            // A fresh popup is transient (`modal == false`), has no owner, and paints the
            // square frame; `0` in `transient_parent` means "no parent".
            "modal" | "floating_chrome" => CapabilityValue::Bool(false),
            "transient_parent" => CapabilityValue::UInt(0),
            // `toast_stack` shares this kind; see the `ListView` note above.
            "toast_count" => CapabilityValue::UInt(0),
            "selected_id" => CapabilityValue::Null,
            "row_height" => CapabilityValue::UInt(30),
            "resizable" => CapabilityValue::Bool(false),
            "max_visible" => CapabilityValue::UInt(5),
            "anchor" => CapabilityValue::String("bottom_left".to_string()),
            "stacking_direction" => CapabilityValue::String("newest_at_bottom".to_string()),
            _ => return None,
        },
        // ── Container widgets ───────────────────────────────
        WidgetKind::ScrollArea => match property_name {
            "widget_resizable" => CapabilityValue::Bool(true),
            "horizontal_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "vertical_scroll_bar_policy" => CapabilityValue::String("as_needed".to_string()),
            "scroll_position_x" => CapabilityValue::Int(0),
            "scroll_position_y" => CapabilityValue::Int(0),
            // The content extent is unknown until a caller states it; `ScrollArea::new`
            // starts at zero, so the schema's default is the control's own answer.
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "sticky_region_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::TabWidget => match property_name {
            "document_mode" => CapabilityValue::Bool(false),
            "tab_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::UInt(0),
            // The first tab's title. A freshly constructed `TabWidget` has no tabs, so the
            // default is the empty string — the same answer `TabWidget::get` gives for that
            // state, which is what keeps the schema and the control in agreement.
            "text" | "title" => CapabilityValue::String(String::new()),
            "closable" => CapabilityValue::Bool(false),
            "movable" => CapabilityValue::Bool(false),
            "tab_position" => CapabilityValue::String("north".to_string()),
            // `TabShape::default()` is `Rounded`.
            "tab_shape" => CapabilityValue::String("rounded".to_string()),
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
            // An empty list of the declared schema, not `Null`: a dropped binding resets through the
            // default, and `Null` is a value the property's own schema check refuses (B26.09).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::advanced_widgets::pie_menu::PIE_MENU_ITEM_SCHEMA,
            )),
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
            "alignment" => CapabilityValue::String("left".to_string()),
            "calendar_first_day" => CapabilityValue::String("sun".to_string()),
            _ => return None,
        },
        // ── Group A widgets (non-mini) ─────────────────────
        WidgetKind::SearchBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Search...".to_string()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::Badge => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "count" => CapabilityValue::Int(0),
            "dot_mode" => CapabilityValue::Bool(false),
            // Mirrors `BadgeLevel::default()`, which is `Info`.
            "level" => CapabilityValue::String("info".to_string()),
            _ => return None,
        },
        WidgetKind::SkeletonLoader => match property_name {
            "active" => CapabilityValue::Bool(true),
            "shape" => CapabilityValue::String("rect".to_string()),
            _ => return None,
        },
        WidgetKind::FAB => match property_name {
            "icon" => CapabilityValue::String("+".to_string()),
            "icon_source" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::BottomSheet => match property_name {
            "expanded" => CapabilityValue::Bool(false),
            "peek_height" => CapabilityValue::Float(100.0),
            _ => return None,
        },
        WidgetKind::BottomNavigationBar => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::NavigationDrawer => match property_name {
            "section_separator_visible" => CapabilityValue::Bool(true),
            "open" => CapabilityValue::Bool(false),
            "width" => CapabilityValue::Float(250.0),
            _ => return None,
        },
        WidgetKind::AppBar => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "direction" => CapabilityValue::String("ltr".to_string()),
            _ => return None,
        },
        WidgetKind::MobileDatePicker => match property_name {
            // Matches `MobileDatePicker::new`, which starts at 2025-01-01. The table used
            // to claim the empty string, which the property route cannot write back
            // (`parse_iso_date("")` is malformed) — a default no instance can hold.
            "selected_date" => CapabilityValue::String("2025-01-01".to_string()),
            "year_span" => CapabilityValue::UInt(50),
            _ => return None,
        },
        WidgetKind::Divider => match property_name {
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "thickness" => CapabilityValue::Float(1.0),
            // The constructor's neutral grey, which is also the fallback the line uses when no
            // theme is installed.
            "color" => CapabilityValue::Color(crate::core::Color::rgba(180, 180, 180, 200)),
            _ => return None,
        },
        WidgetKind::Stepper => match property_name {
            "value" => CapabilityValue::Int(0),
            "minimum" => CapabilityValue::Int(0),
            "maximum" => CapabilityValue::Int(100),
            "step" => CapabilityValue::Int(1),
            // The macOS stepper's own flag (see `MAC_STEPPER_PROPERTIES`).
            "wraps" => CapabilityValue::Bool(false),
            "repeat_delay_ms" => CapabilityValue::UInt(400),
            "repeat_interval_ms" => CapabilityValue::UInt(80),
            _ => return None,
        },
        WidgetKind::Rating => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "max" => CapabilityValue::UInt(5),
            "star_size" => {
                CapabilityValue::UInt(crate::widget::metrics::dimensions::RATING_STAR_SIZE as u64)
            }
            // Unrated, so nothing is filled — and the text says so rather than claiming "0 of 5".
            "fill" => CapabilityValue::Float(0.0),
            "display_text" => CapabilityValue::String("no rating".to_string()),
            _ => return None,
        },
        WidgetKind::Avatar => match property_name {
            "initials" => CapabilityValue::String(String::new()),
            "image_source" => CapabilityValue::String(String::new()),
            "badge" => CapabilityValue::Null,
            "ring_color" => CapabilityValue::Null,
            "ring_width" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::EmptyState => match property_name {
            "message" => CapabilityValue::String("No data".to_string()),
            "description" => CapabilityValue::String(String::new()),
            // COMP-23: `list_state_host` shares this kind. A fresh host is idle, has no empty or
            // error content and no pull gesture in progress.
            "state" => CapabilityValue::String("idle".to_string()),
            "empty_icon" => CapabilityValue::Null,
            "empty_title" | "empty_hint" | "error_text" => CapabilityValue::String(String::new()),
            "loading_rows" => CapabilityValue::UInt(0),
            "pull_progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::ColorHistory => match property_name {
            "color_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ColorWell => match property_name {
            // A fresh well starts on red, and the declared kind is `Color` — so the default is
            // carried as one rather than as a hex string the schema never promised.
            "color" => CapabilityValue::Color(crate::core::Color::rgb(255, 0, 0)),
            "show_border" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::TagInput => match property_name {
            "tags" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Add tag...".to_string()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::ImePreedit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "cursor_position" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::InplaceEditor => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "editing" => CapabilityValue::Bool(false),
            // Mirrors `EditTriggers::default()` (`DOUBLE_CLICK`).
            "edit_triggers" => CapabilityValue::String("double_click".to_string()),
            _ => return None,
        },
        WidgetKind::QRCode => match property_name {
            "data" => CapabilityValue::String(String::new()),
            // The constructor's own size: `(MATRIX_SIZE + 2 * quiet_zone) * module_size`
            // = `(21 + 4) * 4` = 100. The table used to claim 256, which is not
            // representable at all (the control quantises size to whole modules of the
            // 25-module symbol) — a write of 256 therefore read back 250.
            "size" => CapabilityValue::UInt(100),
            // The constructor's own default.
            "quiet_zone" => CapabilityValue::UInt(2),
            "ec_level" => CapabilityValue::String("L".to_string()),
            "min_version" => CapabilityValue::UInt(1),
            "max_version" => CapabilityValue::UInt(40),
            "foreground_color" => CapabilityValue::Null,
            "background_color" => CapabilityValue::Null,
            "logo" => CapabilityValue::Null,
            "logo_size_modules" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MasonryLayout => match property_name {
            "item_spacing" => CapabilityValue::UInt(4),
            "column_count" => CapabilityValue::UInt(2),
            "item_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MaterialSnackbar => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "action_text" => CapabilityValue::String(String::new()),
            "duration_ms" => CapabilityValue::UInt(4000),
            "slide_progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::AdaptiveScaffold => match property_name {
            "badge_style" => CapabilityValue::String("text".to_string()),
            "drawer_animated" => CapabilityValue::Bool(true),
            "scrim_opacity" => CapabilityValue::Float(0.32),
            "page_transition" => CapabilityValue::Bool(true),
            "title" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::WizardDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "subtitle" => CapabilityValue::String(String::new()),
            "current_step" => CapabilityValue::UInt(0),
            "step_count" => CapabilityValue::UInt(0),
            "can_go_back" => CapabilityValue::Bool(false),
            "can_go_forward" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::SafeArea => match property_name {
            "top_inset" => CapabilityValue::Float(0.0),
            "bottom_inset" => CapabilityValue::Float(0.0),
            "left_inset" => CapabilityValue::Float(0.0),
            "right_inset" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::CupertinoAlertDialog => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "message" => CapabilityValue::String(String::new()),
            _ => return None,
        },
        WidgetKind::CupertinoSlider => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "min" => CapabilityValue::Float(0.0),
            "max" => CapabilityValue::Float(1.0),
            _ => return None,
        },
        WidgetKind::Tooltip => match property_name {
            "text" => CapabilityValue::String(String::new()),
            // The popup state, published under its own name so bare `visible` can keep
            // meaning the base widget's visibility (served by the shared entry above).
            "shown" => CapabilityValue::Bool(false),
            // A fresh tooltip is one eliding line; wrapping is the caller's opt-in.
            "wrap" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::SegmentedButton => match property_name {
            // A fresh button has no segments and no selection, so `Null` is the only
            // default it can report — `0` would name a segment that does not exist and
            // that the setter rejects on an empty control.
            "selected_index" => CapabilityValue::Null,
            "selected_indices" => CapabilityValue::String(String::new()),
            "allows_multiple" => CapabilityValue::Bool(false),
            "segment_count" => CapabilityValue::UInt(0),
            "group_id" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::NavigationStack => match property_name {
            "back_visible" => CapabilityValue::Bool(true),
            "page_count" => CapabilityValue::UInt(0),
            // A fresh stack is empty, so it has no current page (and page 0 is not
            // reachable until something is pushed).
            "current_page" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ProgressCircle => match property_name {
            "value" => CapabilityValue::Float(0.0),
            "thickness" => CapabilityValue::Float(4.0),
            // The constructor derives `diameter` from the initial bounds, so a fresh control's
            // answer depends on its geometry; this is the declared default diameter the size hint
            // uses, which is the value a document can rely on when it does not set one.
            "diameter" => CapabilityValue::UInt(u64::from(
                crate::widget::metrics::dimensions::PROGRESS_CIRCLE_DEFAULT_DIAMETER,
            )),
            "progress_color" => CapabilityValue::Color(
                crate::style::resolved_theme_style_shared("progress_bar")
                    .and_then(|resolved| resolved.background_color)
                    .unwrap_or(crate::core::Color::PRIMARY),
            ),
            "indeterminate" => CapabilityValue::Bool(false),
            // The centre reading is opt-in, and its template is the documented default.
            "show_center_text" => CapabilityValue::Bool(false),
            "center_text_format" => CapabilityValue::String("{percent}%".to_string()),
            _ => return None,
        },
        WidgetKind::Icon => match property_name {
            "icon_name" => CapabilityValue::String(String::new()),
            // A control starts with no typed source, so the default is `Null` — the same "no icon
            // set" answer the other `icon_source` owners publish.
            "icon_source" => CapabilityValue::Null,
            "size" => CapabilityValue::Float(24.0),
            // An icon with no explicit colour follows the theme, so the schema
            // default is `Null` rather than a colour literal: "unset" is the honest
            // default, `Icon::color` resolves what will be drawn, and a `Null` write
            // clears the explicit colour again. (The empty string this used to
            // declare was neither readable back nor writable: `Color::parse_hex("")`
            // is a type mismatch.)
            "color" => CapabilityValue::Null,
            "icon_rotation" => CapabilityValue::Float(0.0),
            "icon_flip" => CapabilityValue::String("none".to_string()),
            _ => return None,
        },
        WidgetKind::DropdownMenu => match property_name {
            // An empty list of the declared schema, not `Null`: a dropped binding resets through the
            // default, and `Null` is a value the property's own schema check refuses (B26.09).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::menu_toolbar::dropdown_menu::DROPDOWN_ITEM_SCHEMA,
            )),
            "item_count" => CapabilityValue::UInt(0),
            // A fresh menu has no items, so there is no item 0 to select; `Null` is the
            // state the control actually starts in (and the setter's clear value).
            "selected_index" => CapabilityValue::Null,
            "expanded" => CapabilityValue::Bool(false),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::MaskedEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "mask" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::MenuButton => match property_name {
            "text" => CapabilityValue::String("Menu".to_string()),
            "icon_source" => CapabilityValue::Null,
            // An empty list of the declared schema, not `Null`: a dropped binding resets through the
            // default, and `Null` is a value the property's schema check refuses (B26.09).
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::menu_toolbar::menu_button::MENU_BUTTON_ITEM_SCHEMA,
            )),
            "item_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Popover => match property_name {
            "shown" => CapabilityValue::Bool(false),
            "text" => CapabilityValue::String(String::new()),
            // A fresh popover is transient, auto-places its arrow, uses the built-in
            // placeholder and paints an opaque card.
            "behavior" => CapabilityValue::String("transient".to_string()),
            "arrow_direction" => CapabilityValue::String("auto".to_string()),
            "placeholder_text" => CapabilityValue::String(String::new()),
            "translucent" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::AutoCompleteEdit => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "suggestion_count" => CapabilityValue::UInt(0),
            // `Null` is the default for "nothing highlighted", matching what `get` publishes.
            "selected_index" => CapabilityValue::Null,
            "selected_suggestion" => CapabilityValue::Null,
            "dropdown_visible" => CapabilityValue::Bool(false),
            // `AutoCompleteEdit::new` seeds `max_visible` with 5, so that is the default a fresh
            // control reports and therefore the one the schema must publish.
            "max_visible" => CapabilityValue::UInt(5),
            "can_undo" => CapabilityValue::Bool(false),
            "can_redo" => CapabilityValue::Bool(false),
            "match_mode" => CapabilityValue::String("contains".to_string()),
            "case_sensitive" => CapabilityValue::Bool(false),
            "inline_completion" => CapabilityValue::Bool(false),
            "close_policy" => CapabilityValue::String("always".to_string()),
            "completion_prefix" => CapabilityValue::String(String::new()),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::MultiSelectComboBox => match property_name {
            "selected_count" => CapabilityValue::UInt(0),
            "expanded" => CapabilityValue::Bool(false),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::RangeSlider => match property_name {
            "min_value" => CapabilityValue::Float(0.0),
            "max_value" => CapabilityValue::Float(100.0),
            "lower" => CapabilityValue::Float(25.0),
            "upper" => CapabilityValue::Float(75.0),
            // The control's own default is `Horizontal`; publishing the matching token is what lets
            // the schema-driven property API round-trip it.
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "direction" => CapabilityValue::String("ltr".to_string()),
            _ => return None,
        },
        WidgetKind::FloatingLabel => match property_name {
            "text" => CapabilityValue::String(String::new()),
            // The caption the control floats. Its default is empty, the same as `text`:
            // a field with no caption has no label, and inventing one would put words the
            // caller never wrote into their UI.
            "label" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String(String::new()),
            "focused" => CapabilityValue::Bool(false),
            // Material's own default behaviour, and the one the `Default` impl picks.
            "floating_label_behavior" => CapabilityValue::String("auto".to_string()),
            _ => return None,
        },
        WidgetKind::FontPreview => match property_name {
            "font_family" => CapabilityValue::String("Arial".to_string()),
            "font_size" => CapabilityValue::Float(16.0),
            "preview_text" => CapabilityValue::String("The quick brown fox...".to_string()),
            _ => return None,
        },
        WidgetKind::CupertinoNavigationBar => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "large_title" => CapabilityValue::Bool(true),
            "prompt" => CapabilityValue::String("".to_string()),
            "collapse_offset" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::CupertinoSegmentedControl => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            "segment_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::RefreshControl => match property_name {
            "refreshing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ModalBottomSheet => match property_name {
            "visible" => CapabilityValue::Bool(false),
            // `0` is the documented "derive the height from the control" value, so it is
            // the round-trippable default; the accessor reports the effective height.
            "sheet_height" => CapabilityValue::UInt(0),
            "animated" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::FindReplaceDialog => match property_name {
            "find_text" => CapabilityValue::String(String::new()),
            "replace_text" => CapabilityValue::String(String::new()),
            "match_case" => CapabilityValue::Bool(false),
            "whole_word" => CapabilityValue::Bool(false),
            "use_regex" => CapabilityValue::Bool(false),
            "highlight_all" => CapabilityValue::Bool(false),
            "wrap_around" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::PropertiesPanel => match property_name {
            "description_mode" => CapabilityValue::String("tooltip".to_string()),
            "edit_trigger" => CapabilityValue::String("single_click".to_string()),
            "property_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::CupertinoDatePicker => match property_name {
            "selected_date" => CapabilityValue::String("2025-01-01".to_string()),
            "mode" => CapabilityValue::String("date".to_string()),
            "use_24_hour" => CapabilityValue::Bool(true),
            "minute_interval" => CapabilityValue::UInt(1),
            _ => return None,
        },
        WidgetKind::EditableComboBox => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "item_count" => CapabilityValue::UInt(0),
            "cursor_position" => CapabilityValue::UInt(0),
            "max_visible_items" => CapabilityValue::Null,
            "scroll_offset" => CapabilityValue::UInt(0),
            "alignment" => CapabilityValue::String("left".to_string()),
            _ => return None,
        },
        WidgetKind::DateRangePicker => match property_name {
            // A fresh range is unselected on both ends, which is `Null` — the getter's
            // answer and the setter's clear value. `""` is neither readable back nor
            // writable (`parse_iso_date("")` is malformed).
            "start_date" => CapabilityValue::Null,
            "end_date" => CapabilityValue::Null,
            "presets" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "double_month" => CapabilityValue::Bool(false),
            "min_nights" => CapabilityValue::UInt(0),
            "max_nights" => CapabilityValue::UInt(0),
            _ => return None,
        },

        // ── New widgets (menu/toolbar) ───────────────────────────
        WidgetKind::ToolButton => match property_name {
            // A fresh split button has no actions; the empty list of the declared schema is what a
            // dropped binding resets to.
            "actions" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::special_widgets::split_button::SPLIT_ACTION_SCHEMA,
            )),
            "text" => CapabilityValue::String(String::new()),
            "checked" => CapabilityValue::Bool(false),
            "icon_source" => CapabilityValue::Null,
            "menu_open" => CapabilityValue::Bool(false),
            "action_count" => CapabilityValue::UInt(0),
            "row_height" => CapabilityValue::UInt(24),
            "popup_direction" => CapabilityValue::String("down".to_string()),
            "max_menu_height" => CapabilityValue::Null,
            "menu_scroll" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::StatusBar => match property_name {
            "message" => CapabilityValue::String(String::new()),
            "action_label" => CapabilityValue::Null,
            "queued_count" => CapabilityValue::UInt(0),
            "max_queued" => CapabilityValue::UInt(4),
            "anchor" => CapabilityValue::String("center".to_string()),
            // COMP-A23: `grid_status_bar` shares this kind. A new strip has no rows, no
            // selection, no filter, and therefore an empty explicit text and summary.
            "row_count" | "selected_count" => CapabilityValue::UInt(0),
            "filter_active" => CapabilityValue::Bool(false),
            "text" => CapabilityValue::String(String::new()),
            "summary" => CapabilityValue::String(String::new()),
            _ => return None,
        },

        // ── New widgets (input) ────────────────────────────────────
        WidgetKind::SearchBar => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "placeholder" => CapabilityValue::String("Search...".to_string()),
            "alignment" => CapabilityValue::String("left".to_string()),
            // COMP-22: `find_replace_bar` shares this kind. A fresh strip is closed, with empty
            // query and replacement text, both flags off and no matches.
            "query" | "replacement" => CapabilityValue::String(String::new()),
            "match_case" | "use_regex" => CapabilityValue::Bool(false),
            "match_count" | "current_match" => CapabilityValue::UInt(0),
            "replace_visible" | "open" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ShortcutEditor => match property_name {
            "filter_text" => CapabilityValue::String(String::new()),
            // A freshly constructed editor has an empty table, so every derived count is zero and the
            // empty-state flag is true. The control's own accessors are the authority; these mirror them.
            "shortcut_count" | "visible_count" | "category_count" => CapabilityValue::UInt(0),
            "categories" => CapabilityValue::String(String::new()),
            "empty" => CapabilityValue::Bool(true),
            "can_undo" | "can_redo" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── New widgets (navigation) ───────────────────────────────
        WidgetKind::TabView => match property_name {
            "keyboard_navigation_enabled" => CapabilityValue::Bool(true),
            "tab_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MaterialNavigationRail => match property_name {
            "selected_index" => CapabilityValue::UInt(0),
            _ => return None,
        },

        // ── New widgets (overlay) ───────────────────────────────────
        WidgetKind::SwipeToDismiss => match property_name {
            "is_dismissed" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── New widgets (chart) ─────────────────────────────────────
        WidgetKind::LineChart => match property_name {
            "stroke_width" => CapabilityValue::Float(2.0),
            _ => return None,
        },
        WidgetKind::Sparkline => match property_name {
            "stroke_width" => CapabilityValue::Float(1.5),
            "data" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "show_end_value" => CapabilityValue::Bool(false),
            "fill" => CapabilityValue::Bool(false),
            "show_points" => CapabilityValue::Bool(false),
            "show_last_marker" => CapabilityValue::Bool(true),
            "capacity" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::BarChart => match property_name {
            "bar_spacing" => CapabilityValue::Float(0.2),
            _ => return None,
        },
        WidgetKind::PieChart => match property_name {
            "donut" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── New widgets (media/animation) ───────────────────────────
        WidgetKind::AnimatedImage => match property_name {
            "playing" => CapabilityValue::Bool(false),
            "direction" => CapabilityValue::String("forward".to_string()),
            "frame_cache_limit" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::HeroAnimation => match property_name {
            "animation_progress" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::LottieWidget => match property_name {
            "playing" => CapabilityValue::Bool(false),
            "loop_count" => CapabilityValue::Int(0),
            "loops_completed" => CapabilityValue::Int(0),
            "direction" => CapabilityValue::String("forward".to_string()),
            "speed" => CapabilityValue::Float(1.0),
            _ => return None,
        },
        WidgetKind::RiveWidget => match property_name {
            "is_playing" => CapabilityValue::Bool(false),
            "loop_count" => CapabilityValue::Int(0),
            "loops_completed" => CapabilityValue::Int(0),
            "state" => CapabilityValue::String("idle".to_string()),
            _ => return None,
        },
        WidgetKind::VideoPlayer => match property_name {
            "is_playing" => CapabilityValue::Bool(false),
            "volume" => CapabilityValue::Float(0.8),
            "muted" => CapabilityValue::Bool(false),
            "playback_rate" => CapabilityValue::Float(1.0),
            "controls_visible" => CapabilityValue::Bool(true),
            "looping" => CapabilityValue::Bool(false),
            // A freshly loaded player is not stalled; the host raises this with the `buffering`
            // property when it has no data (MW-003).
            "buffering" => CapabilityValue::Bool(false),
            "pipeline" => CapabilityValue::String("Idle".to_string()),
            _ => return None,
        },

        // ── New widgets (view) ──────────────────────────────────────
        WidgetKind::ImageGallery => match property_name {
            "strip_offset" => CapabilityValue::UInt(0),
            "strip_scroll_enabled" => CapabilityValue::Bool(true),
            "wheel_mode" => CapabilityValue::String("strip".to_string()),
            "arrow_key_navigation_enabled" => CapabilityValue::Bool(true),
            "current_index" => CapabilityValue::UInt(0),
            // An empty list of the declared schema rather than `Null`: a dropped binding resolves to
            // this default, and `Null` is a value the `Records` schema check refuses.
            "images" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::view_widgets::image_gallery::GALLERY_IMAGE_SCHEMA,
            )),
            "thumbnail_size" => CapabilityValue::UInt(64),
            "show_thumbnails" => CapabilityValue::Bool(true),
            _ => return None,
        },

        // ── New widgets (view / property) ───────────────────────────
        WidgetKind::PropertyGrid => match property_name {
            // A fresh grid has no rows; the empty list of the declared schema is what a dropped
            // binding resets to.
            "properties" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::view_widgets::property_grid::PROPERTY_ITEM_SCHEMA,
            )),
            "property_count" => CapabilityValue::UInt(0),
            "selected_index" => CapabilityValue::Null,
            _ => return None,
        },

        // ── New widgets (misc) ──────────────────────────────────────
        WidgetKind::AudioVisualizer => match property_name {
            "bar_count" => CapabilityValue::UInt(64),
            "peak_threshold" => CapabilityValue::Float(0.9),
            "mode" => CapabilityValue::String("waveform".to_string()),
            "level_report_interval" => CapabilityValue::UInt(50),
            _ => return None,
        },
        WidgetKind::CameraPreview => match property_name {
            "is_active" => CapabilityValue::Bool(false),
            "pixel_format" => CapabilityValue::String("Rgba8888".to_string()),
            "target_fps" => CapabilityValue::UInt(30),
            "camera_id" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::BarcodeScanner => match property_name {
            "is_scanning" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::BezierCurveEditor => match property_name {
            "snap_to_grid" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── Layout family ───────────────────────────────────────────
        //
        // These mirror the constructors' own initial state. A wrapper's content box starts empty
        // at zero size, so both content setters are writable from the schema default; the
        // distribution controls start with no items, which is why their counts are zero.
        WidgetKind::Spacer => match property_name {
            "widget" => CapabilityValue::Null,
            "flex" => CapabilityValue::UInt(1),
            "base_size" => CapabilityValue::UInt(0),
            "paint_hint" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Center => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "width_factor" | "height_factor" => CapabilityValue::Float(1.0),
            _ => return None,
        },
        WidgetKind::Align => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "alignment" => CapabilityValue::String("centre".to_string()),
            _ => return None,
        },
        WidgetKind::SizedBox | WidgetKind::IntrinsicWidth => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            // `SizedBox` also publishes the fixed box it asks its parent for. A freshly built box
            // starts at its own geometry, which the schema cannot know; the honest default is the
            // one value the constructor can produce for any geometry.
            "box_width" | "box_height" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::Padding => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "padding_top" | "padding_right" | "padding_bottom" | "padding_left" => {
                CapabilityValue::UInt(0)
            }
            _ => return None,
        },
        WidgetKind::Expanded => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "flex" => CapabilityValue::UInt(1),
            _ => return None,
        },
        WidgetKind::Flexible => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            "flex" => CapabilityValue::UInt(1),
            "fit" => CapabilityValue::String("loose".to_string()),
            _ => return None,
        },
        WidgetKind::AspectRatio => match property_name {
            "widget" => CapabilityValue::Null,
            "content" => CapabilityValue::String(String::new()),
            "content_width" | "content_height" => CapabilityValue::UInt(0),
            // The constructor's own default, so the schema and the control agree.
            "ratio" => CapabilityValue::Float(f64::from(16.0_f32 / 9.0)),
            "respect_parent" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Flex => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "direction" => CapabilityValue::String("row".to_string()),
            "wrap" => CapabilityValue::String("no_wrap".to_string()),
            "justify_content" => CapabilityValue::String("flex_start".to_string()),
            "align_items" => CapabilityValue::String("stretch".to_string()),
            "gap" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::FlowLayout => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "direction" => CapabilityValue::String("horizontal".to_string()),
            "alignment" => CapabilityValue::String("start".to_string()),
            "spacing" | "padding" => CapabilityValue::UInt(8),
            "wrap" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::WrapLayout => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "direction" => CapabilityValue::String("horizontal".to_string()),
            "alignment" => CapabilityValue::String("start".to_string()),
            "spacing" | "padding" => CapabilityValue::UInt(8),
            // `null` is the documented "follow the item spacing" state.
            "run_spacing" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Accordion => match property_name {
            "section_count" | "expanded_count" => CapabilityValue::UInt(0),
            "single_expand" => CapabilityValue::Bool(true),
            _ => return None,
        },

        // ── Small display controls ──────────────────────────────────
        WidgetKind::Tag => match property_name {
            "text" => CapabilityValue::String(String::new()),
            "tone" => CapabilityValue::String("neutral".to_string()),
            "selected" | "closable" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::Clock => match property_name {
            "hour" | "minute" | "second" => CapabilityValue::UInt(0),
            "show_seconds" => CapabilityValue::Bool(false),
            // Mirrors `ClockTimeSource::default()` (`Manual`).
            "time_source" => CapabilityValue::String("manual".to_string()),
            "display" => CapabilityValue::String("dial".to_string()),
            "utc_offset" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::Gauge => match property_name {
            "value" | "minimum" => CapabilityValue::Float(0.0),
            "maximum" => CapabilityValue::Float(100.0),
            "label" | "unit" => CapabilityValue::String(String::new()),
            "thickness" => CapabilityValue::UInt(10),
            "zone_count" => CapabilityValue::UInt(0),
            "animated" => CapabilityValue::Bool(false),
            "bands" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "tick_count" => CapabilityValue::UInt(5),
            "start_angle" => CapabilityValue::Float(2.3561945),
            "end_angle" => CapabilityValue::Float(7.0685835),
            "needle_style" => CapabilityValue::String("none".to_string()),
            "unit_label" => CapabilityValue::String("".to_string()),
            _ => return None,
        },

        // ── Menus and modal notices ────────────────────────────────
        WidgetKind::ContextMenu => match property_name {
            "item_count" => CapabilityValue::UInt(0),
            "open" => CapabilityValue::Bool(false),
            "highlighted_index" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::AlertDialog => match property_name {
            "title" | "message" => CapabilityValue::String(String::new()),
            "button_text" => CapabilityValue::String("OK".to_string()),
            // The shared notice card starts with empty supplementary text and checkbox label,
            // no icon, the first button as default, and modality on.
            "informative_text" | "detailed_text" | "check_box" | "icon" => {
                CapabilityValue::String(String::new())
            }
            "modal" => CapabilityValue::Bool(true),
            "default_button" => CapabilityValue::UInt(0),
            "check_box_checked" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::ConfirmDialog => match property_name {
            "title" | "message" => CapabilityValue::String(String::new()),
            "accept_text" => CapabilityValue::String("OK".to_string()),
            "reject_text" => CapabilityValue::String("Cancel".to_string()),
            "informative_text" | "detailed_text" | "check_box" | "icon" => {
                CapabilityValue::String(String::new())
            }
            "modal" => CapabilityValue::Bool(true),
            "default_button" => CapabilityValue::UInt(0),
            "check_box_checked" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::AboutDialog => match property_name {
            "title" => CapabilityValue::String("About".to_string()),
            "application_name" | "version" | "message" => CapabilityValue::String(String::new()),
            "informative_text" | "detailed_text" | "check_box" | "icon" => {
                CapabilityValue::String(String::new())
            }
            "modal" => CapabilityValue::Bool(true),
            "default_button" => CapabilityValue::UInt(0),
            "check_box_checked" => CapabilityValue::Bool(false),
            _ => return None,
        },

        // ── Companion controls (COMP-01/05/07/08/09/10/12/13/14/15/16/17/18/19/20) ──
        //
        // Each default mirrors the control's own `new`/
        // `Default` initialiser, so a designer's "untouched" value and a freshly
        // constructed control agree.
        WidgetKind::Swatch => match property_name {
            "colors" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Color(crate::core::Color::ERROR),
                CapabilityValue::Color(crate::core::Color::WARNING),
                CapabilityValue::Color(crate::core::Color::SUCCESS),
                CapabilityValue::Color(crate::core::Color::PRIMARY),
                CapabilityValue::Color(crate::core::Color::rgb(156, 39, 176)),
                CapabilityValue::Color(crate::core::Color::SECONDARY),
                CapabilityValue::Color(crate::core::Color::BLACK),
                CapabilityValue::Color(crate::core::Color::WHITE),
            ])),
            "selected_index" => CapabilityValue::UInt(0),
            "cell_size" => CapabilityValue::UInt(24),
            "columns" => CapabilityValue::UInt(6),
            _ => return None,
        },
        WidgetKind::FilterRow => match property_name {
            "columns" | "conditions" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "operator" => CapabilityValue::String("contains".to_string()),
            "live" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::Legend => match property_name {
            "entries" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "orientation" => CapabilityValue::String("vertical".to_string()),
            "checkable" => CapabilityValue::Bool(false),
            "position" => CapabilityValue::String("top".to_string()),
            _ => return None,
        },
        WidgetKind::ChartTooltip => match property_name {
            "anchor" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Int(0),
                CapabilityValue::Int(0),
            ])),
            "offset" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Int(12),
                CapabilityValue::Int(18),
            ])),
            "follow_pointer" => CapabilityValue::Bool(true),
            "content" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            _ => return None,
        },
        WidgetKind::TooltipManager => match property_name {
            "delay_ms" => CapabilityValue::UInt(500),
            "duration_ms" => CapabilityValue::UInt(3000),
            "enabled" => CapabilityValue::Bool(true),
            "follow_pointer" => CapabilityValue::Bool(true),
            "max_width" => CapabilityValue::UInt(260),
            _ => return None,
        },
        WidgetKind::ZoomPanController => match property_name {
            "zoom_level" => CapabilityValue::Float(1.0),
            "pan_offset" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Int(0),
                CapabilityValue::Int(0),
            ])),
            "zoom_mode" => CapabilityValue::String("free".to_string()),
            "min_zoom" => CapabilityValue::Float(0.05),
            "max_zoom" => CapabilityValue::Float(32.0),
            "roi" => CapabilityValue::Null,
            // Mirrors `ZoomPanController::new`'s content extent.
            "content_width" => CapabilityValue::UInt(1024),
            "content_height" => CapabilityValue::UInt(768),
            _ => return None,
        },
        WidgetKind::ToastHost => match property_name {
            "max_visible" => CapabilityValue::UInt(5),
            "max_queued" => CapabilityValue::UInt(10),
            "default_duration_ms" => CapabilityValue::UInt(3000),
            "anchor" => CapabilityValue::String("bottom_left".to_string()),
            "stacking_direction" => CapabilityValue::String("newest_at_top".to_string()),
            "overflow_policy" => CapabilityValue::String("drop_oldest".to_string()),
            _ => return None,
        },
        WidgetKind::TileLoader => match property_name {
            "url_template" => CapabilityValue::String("tiles/{z}/{x}/{y}.png".to_string()),
            "min_zoom" => CapabilityValue::UInt(0),
            "max_zoom" => CapabilityValue::UInt(19),
            "tile_size" => CapabilityValue::UInt(256),
            "cache_limit" => CapabilityValue::UInt(256),
            "cache_size" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::ZoomControl => match property_name {
            "zoom_level" => CapabilityValue::Float(1.0),
            "min" => CapabilityValue::Float(0.25),
            "max" => CapabilityValue::Float(8.0),
            _ => return None,
        },
        WidgetKind::ScaleBar => match property_name {
            "meters_per_pixel" => CapabilityValue::Float(1.0),
            "units" => CapabilityValue::String("metric".to_string()),
            "style" => CapabilityValue::String("plain".to_string()),
            _ => return None,
        },
        WidgetKind::Compass => match property_name {
            "bearing_deg" => CapabilityValue::Float(0.0),
            "clickable" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::TimelineAxis => match property_name {
            "tick_density" => CapabilityValue::UInt(6),
            "date_format" => CapabilityValue::String("numeric".to_string()),
            "zoom_level" => CapabilityValue::Float(1.0),
            "range" => CapabilityValue::Tuple(crate::compat::Vec::from([
                CapabilityValue::Int(0),
                CapabilityValue::Int(100),
            ])),
            "range_start" => CapabilityValue::Int(0),
            "range_end" => CapabilityValue::Int(100),
            _ => return None,
        },
        WidgetKind::DependencyEditor => match property_name {
            "dependencies" => CapabilityValue::UInt(0),
            // An editor starts with no edges: the empty list of the declared schema.
            "dependency_edges" => {
                CapabilityValue::Records(crate::widget::capability::RecordList::new(
                    &crate::widget::special_widgets::gantt_widget::companions::DEPENDENCY_SCHEMA,
                ))
            }
            "editable" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::DiffFileList => match property_name {
            "files" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "file_count" => CapabilityValue::UInt(0),
            "selected_path" => CapabilityValue::Null,
            "selection_mode" => CapabilityValue::String("single".to_string()),
            _ => return None,
        },
        WidgetKind::MarkdownOutlinePanel => match property_name {
            "entries" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "active_index" => CapabilityValue::Null,
            "width" => CapabilityValue::UInt(180),
            "entry_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::FindBar => match property_name {
            "query" | "replace_text" => CapabilityValue::String(String::new()),
            "case_sensitive" => CapabilityValue::Bool(false),
            "whole_word" => CapabilityValue::Bool(false),
            "visible" => CapabilityValue::Bool(false),
            "replace_visible" => CapabilityValue::Bool(false),
            "match_count" => CapabilityValue::UInt(0),
            "current_range" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::WebFindBar => match property_name {
            "query" => CapabilityValue::String(String::new()),
            "case_sensitive" => CapabilityValue::Bool(false),
            "visible" => CapabilityValue::Bool(false),
            "match_index" | "match_count" => CapabilityValue::UInt(0),
            _ => return None,
        },
        WidgetKind::MenuPanel => match property_name {
            // A fresh panel has no rows; the empty list is what a dropped binding resets to.
            "items" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::MENU_PANEL_ITEM_SCHEMA,
            )),
            "selected_index" | "highlighted_index" => CapabilityValue::Null,
            // `MENU_DEFAULT_MAX_ROWS` in `companions/menu_panel.rs`.
            "max_visible_rows" => CapabilityValue::UInt(8),
            "open" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::SpeedDial => match property_name {
            "actions" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::SPEED_DIAL_ACTION_SCHEMA,
            )),
            "expanded" => CapabilityValue::Bool(false),
            "direction" => CapabilityValue::String("up".to_string()),
            "spacing" => CapabilityValue::UInt(12),
            "collapse_on_activate" => CapabilityValue::Bool(true),
            _ => return None,
        },
        WidgetKind::SwipeActions => match property_name {
            "leading_actions" | "trailing_actions" => {
                CapabilityValue::Records(crate::widget::capability::RecordList::new(
                    &crate::widget::companions::SWIPE_ACTION_SCHEMA,
                ))
            }
            "max_reveal_width" => CapabilityValue::UInt(240),
            "icon_side" => CapabilityValue::UInt(18),
            "offset" => CapabilityValue::Float(0.0),
            _ => return None,
        },
        WidgetKind::DrawerSectionHeader => match property_name {
            "label" => CapabilityValue::String(String::new()),
            "inset" => CapabilityValue::UInt(16),
            "uppercase" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::LayerPanel => match property_name {
            // A fresh panel mirrors no layers; the empty list is what a dropped binding resets
            // to (`Null` would be a value the schema refuses).
            "entries" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::LAYER_ENTRY_SCHEMA,
            )),
            "entry_count" => CapabilityValue::UInt(0),
            "selected_layer" => CapabilityValue::Null,
            "show_visibility_checkboxes" => CapabilityValue::Bool(true),
            "show_reorder_handles" => CapabilityValue::Bool(true),
            "row_height" => CapabilityValue::UInt(22),
            _ => return None,
        },
        WidgetKind::PlaylistPanel => match property_name {
            // A fresh panel mirrors an empty queue; the empty list is what a dropped binding
            // resets to (`Null` would be a value the schema refuses).
            "tracks" => CapabilityValue::Records(crate::widget::capability::RecordList::new(
                &crate::widget::companions::PLAYLIST_TRACK_SCHEMA,
            )),
            "track_count" => CapabilityValue::UInt(0),
            "current_index" => CapabilityValue::Null,
            "show_track_numbers" => CapabilityValue::Bool(true),
            "show_now_playing_badge" => CapabilityValue::Bool(true),
            "row_height" => CapabilityValue::UInt(22),
            "title_from_source" => CapabilityValue::Bool(true),
            "filter" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::ShapeCanvas => match property_name {
            "shapes" | "selection" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "shape_count" => CapabilityValue::UInt(0),
            "editable" => CapabilityValue::Bool(false),
            "snap_to_grid" => CapabilityValue::Bool(false),
            "grid_step" => CapabilityValue::UInt(8),
            _ => return None,
        },
        WidgetKind::KanbanCardView => match property_name {
            "title" | "description" | "assignee" => CapabilityValue::String(String::new()),
            "done" => CapabilityValue::Bool(false),
            "tags" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "draggable" => CapabilityValue::Bool(true),
            "editing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::KanbanColumnHeader => match property_name {
            "title" => CapabilityValue::String(String::new()),
            "count" => CapabilityValue::UInt(0),
            "wip_limit" => CapabilityValue::Null,
            "collapsed" => CapabilityValue::Bool(false),
            "editable" => CapabilityValue::Bool(true),
            "at_wip_limit" => CapabilityValue::Bool(false),
            "editing" => CapabilityValue::Bool(false),
            _ => return None,
        },
        WidgetKind::RangePresetBar => match property_name {
            "presets" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "orientation" => CapabilityValue::String("horizontal".to_string()),
            "compact" => CapabilityValue::Bool(false),
            "selected_label" => CapabilityValue::Null,
            _ => return None,
        },
        WidgetKind::GridViewHeader => match property_name {
            "columns" => CapabilityValue::Tuple(crate::compat::Vec::new()),
            "column_count" => CapabilityValue::UInt(0),
            "sort_indicator_visible" => CapabilityValue::Bool(true),
            "resize_mode" => CapabilityValue::String("fixed".to_string()),
            "filter_icon_visible" => CapabilityValue::Bool(true),
            "sort_columns" => CapabilityValue::String(String::new()),
            _ => return None,
        },

        _ => return None,
    };

    Some(value)
}

#[cfg(stripped_widgets)]
/// Stripped-profile stub: always `None`.
///
/// With the concrete controls compiled out there is no schema to consult, so
/// no property has a declared default. Callers must treat `None` as "unknown"
/// rather than "unset", exactly as in the full profile.
pub fn default_widget_property_default_value(
    _kind: WidgetKind,
    _property_name: &str,
) -> Option<CapabilityValue> {
    None
}
