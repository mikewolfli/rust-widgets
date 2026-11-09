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
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind,
    PropertyValueKind as K, RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use crate::project::gantt::gantt_widget::GanttWidget;
use crate::project::kanban_board::KanbanBoard;

pub const GANTT_WIDGET_PROPERTIES: &[PropertySchema] = &[
    // The whole schedule as records (B26.09). Dependencies travel as `<id>:<fts|sts|ftf>`
    // tokens because a record field cannot hold the nested edge structure; `set_tasks` is
    // the payload-free spelling of the write (B26.03).
    PropertySchema::records(
        "tasks",
        true,
        true,
        &crate::project::gantt::gantt_widget::GANTT_TASK_SCHEMA,
    ),
    PropertySchema::new("task_count", PropertyValueKind::UInt, true, false),
    // The selection is reported only. The control's selection entry point is the Rust-side
    // `select_index`; publishing `select_task` would need a writable id property as its
    // payload channel, which would be a second spelling of the same action (B26.03).
    PropertySchema::new("selected_id", PropertyValueKind::String, true, false),
    // `set_viewport` is never published: no property is named `viewport`, and the two
    // bounds travel through these writable scalars — the same reason `mini_chart.set_range`
    // stays unpublished.
    PropertySchema::new("viewport_start", PropertyValueKind::Int, true, true),
    PropertySchema::new("viewport_end", PropertyValueKind::Int, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
    PropertySchema::new("show_critical_path", PropertyValueKind::Bool, true, true),
    PropertySchema::new("show_today", PropertyValueKind::Bool, true, true),
];

/// Gantt task column (COMP-A4). The derived counts are read-only; the row metrics and the
/// header switch are the column's own settings.
pub const GANTT_TASK_GRID_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("row_height", PropertyValueKind::UInt, true, true),
    PropertySchema::new("indent_step", PropertyValueKind::UInt, true, true),
    PropertySchema::new("header_visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("columns", PropertyValueKind::UInt, true, false),
    PropertySchema::new("tasks", PropertyValueKind::UInt, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Date/value axis for a Gantt chart (`COMP-14`).
pub const TIMELINE_AXIS_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("tick_density", PropertyValueKind::UInt, true, true),
    PropertySchema::enumerated("date_format", true, true, &["numeric", "iso"]),
    PropertySchema::new("zoom_level", PropertyValueKind::Float, true, true),
    // The range pair is read as `[start, end]`; the two scalar names below write it.
    PropertySchema::new("range", PropertyValueKind::NumberList, true, false),
    PropertySchema::new("range_start", PropertyValueKind::Int, true, true),
    PropertySchema::new("range_end", PropertyValueKind::Int, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Editable dependency edges for a Gantt chart (`COMP-14`).
pub const DEPENDENCY_EDITOR_PROPERTIES: &[PropertySchema] = &[
    // The reader reports the list's length; the list itself is replaced through the
    // control's `set_dependencies` method, so the property is read-only.
    PropertySchema::new("dependencies", PropertyValueKind::UInt, true, false),
    // The edge list itself, round-tripped through the `Records` contract (SP-123).
    PropertySchema::records(
        "dependency_edges",
        true,
        true,
        &crate::project::gantt::gantt_widget::companions::DEPENDENCY_SCHEMA,
    ),
    PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

pub const KANBAN_BOARD_PROPERTIES: &[PropertySchema] = &[
    // The two levels of the model travel as two flat record lists (B26.09): `columns`
    // carries column metadata, `cards` carries every card with its `column` index and
    // optional `lane`. Their `set_*` commands are the payload-free spelling of the write
    // (B26.03); `add_column` / `add_card` / `move_card` stay Rust-side methods whose
    // payloads the records now transport, so they are not published.
    PropertySchema::records(
        "columns",
        true,
        true,
        &crate::project::kanban_board::KANBAN_COLUMN_SCHEMA,
    ),
    PropertySchema::records("cards", true, true, &crate::project::kanban_board::KANBAN_CARD_SCHEMA),
    PropertySchema::new("column_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("card_count", PropertyValueKind::UInt, true, false),
    PropertySchema::new("dragging_card_id", PropertyValueKind::String, true, false),
    PropertySchema::new("hovered_column", PropertyValueKind::UInt, true, false),
    PropertySchema::new("column_width", PropertyValueKind::UInt, true, true),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
    PropertySchema::new("horizontal_scroll_offset", PropertyValueKind::UInt, true, true),
    // Swimlanes: `lane_mode` is the explicit switch, `swimlanes` the boolean alias the
    // control accepts for it, and the count is read-only because a lane is an identity.
    PropertySchema::enumerated("lane_mode", true, true, &["none", "lanes"]),
    PropertySchema::new("swimlanes", PropertyValueKind::Bool, true, true),
    PropertySchema::new("lane_count", PropertyValueKind::UInt, true, false),
];

/// Real card control for a kanban column (`COMP-18`).
pub const KANBAN_CARD_VIEW_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("title", PropertyValueKind::String, true, true),
    PropertySchema::new("description", PropertyValueKind::String, true, true),
    PropertySchema::new("done", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tags", PropertyValueKind::StringList, true, true),
    PropertySchema::new("assignee", PropertyValueKind::String, true, true),
    PropertySchema::new("editing", PropertyValueKind::Bool, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// Real column header for a kanban board (`COMP-18`).
pub const KANBAN_COLUMN_HEADER_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("title", PropertyValueKind::String, true, true),
    PropertySchema::new("count", PropertyValueKind::UInt, true, true),
    PropertySchema::new("wip_limit", PropertyValueKind::UInt, true, true),
    PropertySchema::new("collapsed", PropertyValueKind::Bool, true, true),
    PropertySchema::new("editable", PropertyValueKind::Bool, true, true),
    PropertySchema::new("at_wip_limit", PropertyValueKind::Bool, true, false),
    PropertySchema::new("editing", PropertyValueKind::Bool, true, false),
    PropertySchema::new("enabled", PropertyValueKind::Bool, true, true),
    PropertySchema::new("visible", PropertyValueKind::Bool, true, true),
    PropertySchema::new("tooltip", PropertyValueKind::String, true, true),
    PropertySchema::new("geometry", PropertyValueKind::String, false, false),
];

/// `gantt_widget`'s published events.
const GANTT_WIDGET_EVENTS: &[EventSchema] = &[
    EventSchema { name: "dependency_added", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "dependency_removed", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "link_selected", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "row_height_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "scroll_changed", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "task_hovered", payload: Some(K::UInt), shape: Some(S::OptionalScalar) },
    EventSchema { name: "task_rescheduled", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "task_resize_finished", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "task_resize_started", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "task_selected", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "viewport_changed", payload: Some(K::Int), shape: Some(S::Tuple2) },
];

/// `gantt_task_grid`'s published events.
const GANTT_TASK_GRID_EVENTS: &[EventSchema] = &[
    EventSchema { name: "on_header_clicked", payload: Some(K::UInt), shape: Some(S::Scalar) },
    EventSchema { name: "on_task_selected", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// `timeline_axis`'s published events.
const TIMELINE_AXIS_EVENTS: &[EventSchema] =
    &[EventSchema { name: "scale_changed", payload: Some(K::Int), shape: Some(S::Tuple2) }];

/// `dependency_editor`'s published events.
const DEPENDENCY_EDITOR_EVENTS: &[EventSchema] = &[
    EventSchema { name: "dependency_added", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "dependency_removed", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "link_selected", payload: Some(K::String), shape: Some(S::Tuple2) },
];

/// `kanban_board`'s published events.
const KANBAN_BOARD_EVENTS: &[EventSchema] = &[
    EventSchema { name: "card_activated", payload: Some(K::String), shape: Some(S::Scalar) },
    EventSchema { name: "card_hovered", payload: Some(K::String), shape: Some(S::OptionalScalar) },
    EventSchema { name: "card_moved", payload: Some(K::String), shape: Some(S::Tuple2) },
    EventSchema { name: "cards_moved", payload: Some(K::String), shape: Some(S::ListScalar) },
    EventSchema { name: "selection_changed", payload: Some(K::String), shape: Some(S::ListScalar) },
];

/// `kanban_card_view`'s published events.
const KANBAN_CARD_VIEW_EVENTS: &[EventSchema] = &[
    EventSchema::unit("card_clicked"),
    EventSchema { name: "card_done_toggled", payload: Some(K::Bool), shape: Some(S::Scalar) },
    EventSchema { name: "card_edited", payload: Some(K::String), shape: Some(S::Scalar) },
];

/// `kanban_column_header`'s published events.
const KANBAN_COLUMN_HEADER_EVENTS: &[EventSchema] = &[
    EventSchema::unit("add_card_requested"),
    EventSchema {
        name: "column_collapsed_changed",
        payload: Some(K::Bool),
        shape: Some(S::Scalar),
    },
    EventSchema { name: "column_renamed", payload: Some(K::String), shape: Some(S::Scalar) },
];

pub fn gantt_widget_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_widget",
        aliases: &["gantt"],
        properties: GANTT_WIDGET_PROPERTIES,
        events: GANTT_WIDGET_EVENTS,
        // `set_viewport` stays unpublished — the viewport is the writable
        // `viewport_start`/`viewport_end` pair (principle #18). `set_tasks` names its payload
        // through the `tasks` records property (B26.03), whose `dependencies` field carries each
        // relationship kind as an `<id>:<fts|sts|ftf>` token, so a schedule write cannot lose a
        // link. `select_task` names a task while `selected_id` is read-only, so it stays
        // unpublished.
        commands: &["zoom", "set_tasks"],
    }
}

/// Gantt task column (COMP-A4).
///
/// It shares `WidgetKind::Chart` with the chart controls; the column is the row-aligned task list
/// the gantt chart draws beside its bars.
pub fn gantt_task_grid_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Chart,
        canonical_name: "gantt_task_grid",
        aliases: &[],
        properties: GANTT_TASK_GRID_PROPERTIES,
        events: GANTT_TASK_GRID_EVENTS,
        commands: &[],
    }
}

/// `timeline_axis` — the Gantt date/value axis (`COMP-14`).
pub fn timeline_axis_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::TimelineAxis,
        canonical_name: "timeline_axis",
        aliases: &["gantt_timeline_axis"],
        properties: TIMELINE_AXIS_PROPERTIES,
        events: TIMELINE_AXIS_EVENTS,
        commands: &[],
    }
}

/// `dependency_editor` — the Gantt dependency edge editor (`COMP-14`).
pub fn dependency_editor_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::DependencyEditor,
        canonical_name: "dependency_editor",
        aliases: &["gantt_dependency_editor"],
        properties: DEPENDENCY_EDITOR_PROPERTIES,
        events: DEPENDENCY_EDITOR_EVENTS,
        commands: &[],
    }
}

/// `KanbanBoard` — columns of draggable cards.
///
/// The first control built on `event::dnd`: its own `DropTarget` impl is the
/// acceptance path a dragged card resolves through.
pub fn kanban_board_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanBoard,
        canonical_name: "kanban_board",
        aliases: &["kanban", "board"],
        properties: KANBAN_BOARD_PROPERTIES,
        events: KANBAN_BOARD_EVENTS,
        // `cancel_drag` is payload-free; `set_columns` / `set_cards` name their payload through
        // the `columns` / `cards` records properties (B26.03).
        commands: &["cancel_drag", "set_columns", "set_cards"],
    }
}

/// `kanban_card_view` — a real card inside a kanban column (`COMP-18`).
pub fn kanban_card_view_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanCardView,
        canonical_name: "kanban_card_view",
        aliases: &["kanban_card_control"],
        properties: KANBAN_CARD_VIEW_PROPERTIES,
        events: KANBAN_CARD_VIEW_EVENTS,
        commands: &[],
    }
}

/// `kanban_column_header` — a real column header for a kanban board (`COMP-18`).
pub fn kanban_column_header_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::KanbanColumnHeader,
        canonical_name: "kanban_column_header",
        aliases: &["kanban_column_bar"],
        properties: KANBAN_COLUMN_HEADER_PROPERTIES,
        events: KANBAN_COLUMN_HEADER_EVENTS,
        commands: &[],
    }
}

/// Creates a Gantt chart with no tasks. `text` is applied as the control's
/// label.
pub fn create_gantt_widget(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(GanttWidget::new(geometry)), text)
}

/// Creates a gantt task column with no tasks (COMP-A4). `text` is applied as the control's label.
pub fn create_gantt_task_grid(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(
        Box::new(crate::project::gantt::gantt_widget::GanttTaskGrid::new(geometry)),
        text,
    )
}

/// Creates a timeline axis over `0 ..= 100` (`COMP-14`).
pub fn create_timeline_axis(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::project::gantt::gantt_widget::TimelineAxis::new(geometry))
}

/// Creates an empty dependency editor (`COMP-14`).
pub fn create_dependency_editor(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(crate::project::gantt::gantt_widget::DependencyEditor::new(geometry))
}

/// Creates an empty kanban board. `text` is applied as the control's label.
///
/// No sample columns: a column title is the caller's workflow vocabulary, and a
/// default like `"Column 1"` would have to be deleted before the board was usable.
pub fn create_kanban_board(geometry: Rect, text: &str) -> Box<dyn Widget> {
    apply_control_label(Box::new(KanbanBoard::new(geometry)), text)
}

/// Creates a kanban card titled by `text` (`COMP-18`).
pub fn create_kanban_card_view(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(crate::project::kanban_board::KanbanCardView::new(geometry, text))
}

/// Creates a kanban column header titled by `text` (`COMP-18`).
pub fn create_kanban_column_header(geometry: Rect, text: &str) -> Box<dyn Widget> {
    Box::new(crate::project::kanban_board::KanbanColumnHeader::new(geometry, text))
}

/// A registered control: its capability row, its constructor, and its defaults provider.
type ProjectRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the project suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [ProjectRow; 7] = [
        (gantt_widget_capability(), create_gantt_widget, |p| {
            default_property_value("gantt_widget", p)
        }),
        (gantt_task_grid_capability(), create_gantt_task_grid, |p| {
            default_property_value("gantt_task_grid", p)
        }),
        (timeline_axis_capability(), create_timeline_axis, |p| {
            default_property_value("timeline_axis", p)
        }),
        (dependency_editor_capability(), create_dependency_editor, |p| {
            default_property_value("dependency_editor", p)
        }),
        (kanban_board_capability(), create_kanban_board, |p| {
            default_property_value("kanban_board", p)
        }),
        (kanban_card_view_capability(), create_kanban_card_view, |p| {
            default_property_value("kanban_card_view", p)
        }),
        (kanban_column_header_capability(), create_kanban_column_header, |p| {
            default_property_value("kanban_column_header", p)
        }),
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
    match kind {
        "gantt_widget" => match name {
            // A fresh schedule has no tasks; the empty list of the declared schema is what a
            // dropped binding resets to (`Null` would be a value the schema refuses).
            "tasks" => {
                Some(CapabilityValue::Records(rust_widgets::widget::capability::RecordList::new(
                    &crate::project::gantt::gantt_widget::GANTT_TASK_SCHEMA,
                )))
            }
            "task_count" => Some(CapabilityValue::UInt(0)),
            "selected_id" => Some(CapabilityValue::Null),
            "viewport_start" => Some(CapabilityValue::Int(0)),
            "viewport_end" => Some(CapabilityValue::Int(100)),
            "row_height" => Some(CapabilityValue::UInt(24)),
            "show_critical_path" => Some(CapabilityValue::Bool(false)),
            "show_today" => Some(CapabilityValue::Bool(true)),
            _ => None,
        },
        "gantt_task_grid" => match name {
            // The constructor's initial values (`GanttTaskGrid::new`): 24-pixel rows, a
            // 12-pixel indent step and a visible header (RW-105). The constructor also creates
            // its one `Task` column, so `columns` starts at one; only `tasks` is empty.
            "row_height" => Some(CapabilityValue::UInt(24)),
            "indent_step" => Some(CapabilityValue::UInt(12)),
            "header_visible" => Some(CapabilityValue::Bool(true)),
            "columns" => Some(CapabilityValue::UInt(1)),
            "tasks" => Some(CapabilityValue::UInt(0)),
            _ => None,
        },
        "timeline_axis" => match name {
            "tick_density" => Some(CapabilityValue::UInt(6)),
            "date_format" => Some(CapabilityValue::String("numeric".to_string())),
            "zoom_level" => Some(CapabilityValue::Float(1.0)),
            "range" => Some(CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                CapabilityValue::Int(0),
                CapabilityValue::Int(100),
            ]))),
            "range_start" => Some(CapabilityValue::Int(0)),
            "range_end" => Some(CapabilityValue::Int(100)),
            _ => None,
        },
        "dependency_editor" => match name {
            "dependencies" => Some(CapabilityValue::UInt(0)),
            // An editor starts with no edges: the empty list of the declared schema.
            "dependency_edges" => {
                Some(CapabilityValue::Records(rust_widgets::widget::capability::RecordList::new(
                    &crate::project::gantt::gantt_widget::companions::DEPENDENCY_SCHEMA,
                )))
            }
            "editable" => Some(CapabilityValue::Bool(true)),
            _ => None,
        },
        "kanban_board" => match name {
            "columns" => {
                Some(CapabilityValue::Records(rust_widgets::widget::capability::RecordList::new(
                    &crate::project::kanban_board::KANBAN_COLUMN_SCHEMA,
                )))
            }
            "cards" => {
                Some(CapabilityValue::Records(rust_widgets::widget::capability::RecordList::new(
                    &crate::project::kanban_board::KANBAN_CARD_SCHEMA,
                )))
            }
            "column_count" | "card_count" | "horizontal_scroll_offset" | "lane_count" => {
                Some(CapabilityValue::UInt(0))
            }
            "dragging_card_id" | "hovered_column" => Some(CapabilityValue::Null),
            "column_width" => Some(CapabilityValue::UInt(220)),
            "lane_mode" => Some(CapabilityValue::String("none".to_string())),
            "swimlanes" => Some(CapabilityValue::Bool(false)),
            _ => None,
        },
        "kanban_card_view" => match name {
            "title" | "description" | "assignee" => Some(CapabilityValue::String(String::new())),
            "done" | "editing" => Some(CapabilityValue::Bool(false)),
            "tags" => Some(CapabilityValue::Tuple(rust_widgets::compat::Vec::new())),
            _ => None,
        },
        "kanban_column_header" => match name {
            "title" => Some(CapabilityValue::String(String::new())),
            "count" => Some(CapabilityValue::UInt(0)),
            "wip_limit" => Some(CapabilityValue::Null),
            "collapsed" | "at_wip_limit" | "editing" => Some(CapabilityValue::Bool(false)),
            "editable" => Some(CapabilityValue::Bool(true)),
            _ => None,
        },
        _ => None,
    }
}

/// Every property name the project suite publishes, for a quick completeness check.
pub fn published_property_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("gantt_widget", GANTT_WIDGET_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("gantt_task_grid", GANTT_TASK_GRID_PROPERTIES.iter().map(|schema| schema.name).collect()),
        ("timeline_axis", TIMELINE_AXIS_PROPERTIES.iter().map(|schema| schema.name).collect()),
        (
            "dependency_editor",
            DEPENDENCY_EDITOR_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        ("kanban_board", KANBAN_BOARD_PROPERTIES.iter().map(|schema| schema.name).collect()),
        (
            "kanban_card_view",
            KANBAN_CARD_VIEW_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
        (
            "kanban_column_header",
            KANBAN_COLUMN_HEADER_PROPERTIES.iter().map(|schema| schema.name).collect(),
        ),
    ]
}

/// Every event name the project suite publishes, for a quick completeness check.
pub fn published_event_names() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("gantt_widget", GANTT_WIDGET_EVENTS.iter().map(|schema| schema.name).collect()),
        ("gantt_task_grid", GANTT_TASK_GRID_EVENTS.iter().map(|schema| schema.name).collect()),
        ("timeline_axis", TIMELINE_AXIS_EVENTS.iter().map(|schema| schema.name).collect()),
        ("kanban_board", KANBAN_BOARD_EVENTS.iter().map(|schema| schema.name).collect()),
        ("kanban_card_view", KANBAN_CARD_VIEW_EVENTS.iter().map(|schema| schema.name).collect()),
        (
            "kanban_column_header",
            KANBAN_COLUMN_HEADER_EVENTS.iter().map(|schema| schema.name).collect(),
        ),
        ("dependency_editor", DEPENDENCY_EDITOR_EVENTS.iter().map(|schema| schema.name).collect()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_widgets::widget::capability::WidgetProperties;

    /// A declared default has to be what a fresh control reports, or a dropped binding resets to a
    /// state the constructor can never produce.
    #[test]
    fn a_task_grids_declared_defaults_match_its_constructor() {
        let grid =
            crate::project::gantt::gantt_widget::GanttTaskGrid::new(Rect::new(0, 0, 220, 200));
        for name in ["row_height", "indent_step", "header_visible", "columns", "tasks"] {
            assert_eq!(
                default_property_value("gantt_task_grid", name),
                Some(grid.get(name).expect("the property is published")),
                "{name} must mirror a fresh grid"
            );
        }
        assert_eq!(grid.get("columns"), Ok(CapabilityValue::UInt(1)));
    }
}
