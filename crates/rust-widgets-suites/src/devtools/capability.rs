// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of this suite group.
//!
//! A capability is the designer's entire view of a control: the name it is created by, the
//! property contract with a type and an accessibility flag per row, and the published events
//! with their payload kinds. All of those are declared here, next to the constructors, so the
//! registration and the documentation cannot describe different controls.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::apply_control_label;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema,
    PropertyValueKind, PropertyValueKind as K, RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use crate::devtools::diff_viewer::{DiffFileList, DiffViewer};
use crate::devtools::terminal::terminal_find_bar::TerminalFindBar;
use crate::devtools::terminal::terminal_view::TerminalView;

pub const TERMINAL_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("output_line_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("input_line", PropertyValueKind::String, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("scrollback_lines", PropertyValueKind::UInt, true, true),
    PropertySchema::new("wrap_output", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_scrollbar", PropertyValueKind::Bool, true, true),
    PropertySchema::enumerated("caret_style", true, true, &["line", "block", "underline"]),
    PropertySchema::new("caret_blink_enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("prompt", PropertyValueKind::String, true, true),
    PropertySchema::enumerated("selection_mode", true, true, &["lines", "characters"]),
    PropertySchema::new("ansi_palette", PropertyValueKind::NumberList, true, true),
];


pub const DIFF_VIEWER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("left_text", PropertyValueKind::String, true, true),
    PropertySchema::new("right_text", PropertyValueKind::String, true, true),
    PropertySchema::new("line_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("change_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("selected_index", PropertyValueKind::UInt, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::enumerated("view_mode", true, true, &["side_by_side", "unified"]),
    PropertySchema::new("fold_unchanged", PropertyValueKind::Bool, true, true),
    PropertySchema::new("fold_threshold", PropertyValueKind::UInt, true, true),
    PropertySchema::new("fold_context_lines", PropertyValueKind::UInt, true, true),
    PropertySchema::new("unified_changed_pair", PropertyValueKind::Bool, true, true),
    PropertySchema::enumerated(
        "whitespace_mode",
        true,
        true,
        &["show", "ignore_eol", "ignore_all", "mark_blank_changes"],
    ),
    PropertySchema::new("word_granularity", PropertyValueKind::Bool, true, true),
];


/// Multi-file selector beside a diff viewer (`COMP-15`).
pub const DIFF_FILE_LIST_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("files", PropertyValueKind::StringList, true, true),
    PropertySchema::new("file_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("selected_path", PropertyValueKind::String, true, false),
    PropertySchema::enumerated(
        "selection_mode",
        true,
        true,
        &["single", "multi", "extended", "none"],
    ),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];


/// Terminal scrollback find bar (COMP-A2). The match count is produced by a scan of the
/// attached terminal, so it is read-only; `visible` is the bar's own open/close flag.
pub const TERMINAL_FIND_BAR_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("query", PropertyValueKind::String, true, true),
    PropertySchema::new("case_sensitive", PropertyValueKind::Bool, true, true),
    PropertySchema::new("use_regex", PropertyValueKind::Bool, true, true),
    PropertySchema::new("wrap_search", PropertyValueKind::Bool, true, true),
    PropertySchema::new("match_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];


/// `terminal_view`'s published events.
const TERMINAL_VIEW_EVENTS: &[EventSchema] = &[
    EventSchema { name: "command_submitted", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "completion_requested", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "interrupt_requested", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "output_received", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "resized", payload: Some(K::UInt), shape: Some(S::Tuple2) },
    EventSchema { name: "selection_changed", payload: Some(K::Bool), shape: Some(S::Scalar) },
];

/// `diff_viewer`'s published events.
const DIFF_VIEWER_EVENTS: &[EventSchema] = &[
    EventSchema { name: "compared", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "fold_changed", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema { name: "hunk_expanded", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "selection_changed", payload: Some(K::UInt), shape: Some(S::OptionalScalar) },
    EventSchema { name: "view_mode_changed", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// `diff_file_list`'s published events.
const DIFF_FILE_LIST_EVENTS: &[EventSchema] = &[
    EventSchema { name: "file_activated", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "file_selected", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// `terminal_find_bar`'s published events.
const TERMINAL_FIND_BAR_EVENTS: &[EventSchema] = &[
    EventSchema { name: "on_matches_changed", payload: Some(K::UInt), shape: Some(S::Tuple2) },
];

pub fn terminal_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TextEdit,
        canonical_name: "terminal_view",
        aliases: &["terminal"],
        properties: TERMINAL_VIEW_PROPERTIES,
        events: TERMINAL_VIEW_EVENTS,
        // Audit 20261026: `append_output` stays unpublished — output is an append-only stream and
        // the control publishes only its line count (`output_line_count`), with no property that
        // carries the buffer itself; the control's own `append_output` API is the write path
        // (principle #18).
        commands: &["submit"],
    }
}


pub fn diff_viewer_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Table,
        canonical_name: "diff_viewer",
        aliases: &["diff"],
        properties: DIFF_VIEWER_PROPERTIES,
        events: DIFF_VIEWER_EVENTS,
        // Audit 20261026: `set_texts` stays unpublished because its name resolves to no property —
        // the two writable properties `left_text` / `right_text` each write through `set_texts`
        // and recompute the diff, so the pair is the honest route (principle #18). `select_index`
        // names a line with no property route; `selected_index` is read-only.
        commands: &[],
    }
}


/// `diff_file_list` — the multi-file selector beside a diff (`COMP-15`).
pub fn diff_file_list_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DiffFileList,
        canonical_name: "diff_file_list",
        aliases: &["diff_files"],
        properties: DIFF_FILE_LIST_PROPERTIES,
        events: DIFF_FILE_LIST_EVENTS,
        commands: &[],
    }
}


/// Terminal scrollback find bar (COMP-A2).
///
/// It shares `WidgetKind::SearchBar` with the search field; the bar attaches to a `TerminalView`
/// and reports no factory event of its own.
pub fn terminal_find_bar_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::SearchBar,
        canonical_name: "terminal_find_bar",
        aliases: &[],
        properties: TERMINAL_FIND_BAR_PROPERTIES,
        events: TERMINAL_FIND_BAR_EVENTS,
        commands: &[],
    }
}


/// Creates a terminal view with no output. `text` becomes the initial contents of
/// the input line, not the transcript.
pub fn create_terminal_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut terminal = TerminalView::new(geometry);
    if !text.is_empty() {
        terminal.set_input_line(text.to_string());
    }
    Box::new(terminal)
}


/// Creates a diff viewer with both snapshots empty. `text` is applied as the
/// control's label.
pub fn create_diff_viewer(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(DiffViewer::new(geometry)), text)
}


/// Creates an empty diff file list (`COMP-15`).
pub fn create_diff_file_list(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(DiffFileList::new(geometry)), _text)
}


/// Creates a closed terminal find bar with no query (COMP-A2). `text` is applied as the control's
/// label, not as the query.
pub fn create_terminal_find_bar(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(
        Box::new(TerminalFindBar::new(geometry)),
        text,
    )
}


/// A registered control: its capability row, its constructor, and its defaults provider.
type DevtoolsRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the devtools suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [DevtoolsRow; 4] = [
        (
            terminal_view_capability(),
            create_terminal_view,
            |p| default_property_value("terminal_view", p),
        ),
        (
            terminal_find_bar_capability(),
            create_terminal_find_bar,
            |p| default_property_value("terminal_find_bar", p),
        ),
        (
            diff_viewer_capability(),
            create_diff_viewer,
            |p| default_property_value("diff_viewer", p),
        ),
        (
            diff_file_list_capability(),
            create_diff_file_list,
            |p| default_property_value("diff_file_list", p),
        ),
    ];
    for (capability, ctor, defaults) in rows {
        let name = capability.canonical_name;
        factory.try_register(capability, ctor)?;
        let _ = factory.register_property_defaults(name, defaults);
    }
    Ok(())
}

/// A property's declared default, for the capability manifest.
///
/// Mirrors what the controls' constructors set, so a dropped binding resets to the value a fresh
/// control would have had.
pub fn default_property_value(kind: &str, name: &str) -> Option<CapabilityValue> {
    match (kind, name) {
        ("terminal_view", "output_line_count") => Some(CapabilityValue::UInt(0)),
        ("terminal_view", "input_line") => Some(CapabilityValue::String(String::new())),
        ("terminal_view", "scrollback_lines") => Some(CapabilityValue::UInt(5000)),
        ("terminal_view", "wrap_output" | "show_scrollbar" | "caret_blink_enabled") => {
            Some(CapabilityValue::Bool(true))
        }
        ("terminal_view", "prompt") => Some(CapabilityValue::String("> ".to_string())),
        ("terminal_view", "ansi_palette") => Some(CapabilityValue::Null),
        ("terminal_find_bar", "query") => Some(CapabilityValue::String(String::new())),
        ("terminal_find_bar", "case_sensitive" | "use_regex") => Some(CapabilityValue::Bool(false)),
        ("terminal_find_bar", "match_count") => Some(CapabilityValue::UInt(0)),
        ("diff_viewer", "left_text" | "right_text") => Some(CapabilityValue::String(String::new())),
        ("diff_viewer", "line_count" | "change_count" | "selected_index") => {
            Some(CapabilityValue::UInt(0))
        }
        ("diff_viewer", "fold_threshold") => Some(CapabilityValue::UInt(8)),
        ("diff_viewer", "fold_context_lines") => Some(CapabilityValue::UInt(0)),
        ("diff_viewer", "fold_unchanged") => Some(CapabilityValue::Bool(false)),
        ("diff_file_list", "files") => Some(CapabilityValue::Tuple(rust_widgets::compat::Vec::new())),
        ("diff_file_list", "file_count") => Some(CapabilityValue::UInt(0)),
        ("diff_file_list", "selected_path") => Some(CapabilityValue::Null),
        ("diff_file_list", "selection_mode") => {
            Some(CapabilityValue::String("single".to_string()))
        }
        _ => None,
    }
}

/// Every property name the devtools suite publishes, for a quick completeness check.
pub fn published_property_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "terminal_view",
            TERMINAL_VIEW_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        (
            "terminal_find_bar",
            TERMINAL_FIND_BAR_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        (
            "diff_viewer",
            DIFF_VIEWER_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        (
            "diff_file_list",
            DIFF_FILE_LIST_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
    ]
}

/// Every event name the devtools suite publishes, for a quick completeness check.
pub fn published_event_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "terminal_view",
            TERMINAL_VIEW_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        (
            "terminal_find_bar",
            TERMINAL_FIND_BAR_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        (
            "diff_viewer",
            DIFF_VIEWER_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        (
            "diff_file_list",
            DIFF_FILE_LIST_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
    ]
}
