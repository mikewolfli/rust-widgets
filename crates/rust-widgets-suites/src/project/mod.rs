// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `project` group: Gantt, timeline axis and kanban.
//!
//! # Why these are one group
//!
//! All three controls answer the same question — "what is planned, in what order, over what
//! span" — and a host that shows a schedule usually shows all of them. They share the `Chart`
//! kind with the plots that stay in the parent crate, which is why each one has its own
//! canonical name and capability row rather than a kind of its own.
//!
//! # Adding a group later
//!
//! A group is a module under `src/` with a `capability` submodule holding its property schemas,
//! event schemas and constructors, and an `install(factory)` function. A `mod.rs` only declares
//! and re-exports (principle #8).

mod capability;
/// The Gantt family: the chart control and the two companions it ships with.
pub mod gantt;
/// Kanban board and the card/column controls it composes.
pub mod kanban_board;

pub use capability::{
    create_dependency_editor, create_gantt_task_grid, create_gantt_widget, create_kanban_board,
    create_kanban_card_view, create_kanban_column_header, create_timeline_axis,
    default_property_value, dependency_editor_capability, gantt_task_grid_capability,
    gantt_widget_capability, install, kanban_board_capability, kanban_card_view_capability,
    kanban_column_header_capability, published_event_names, published_property_names,
    timeline_axis_capability, DEPENDENCY_EDITOR_PROPERTIES, GANTT_TASK_GRID_PROPERTIES,
    GANTT_WIDGET_PROPERTIES, KANBAN_BOARD_PROPERTIES, KANBAN_CARD_VIEW_PROPERTIES,
    KANBAN_COLUMN_HEADER_PROPERTIES, TIMELINE_AXIS_PROPERTIES,
};
pub use gantt::gantt_widget::{
    DateFormat, Dependency, DependencyEditor, GanttTask, GanttTaskGrid, GanttWidget, TimelineAxis,
};
pub use gantt::{gantt_companions, gantt_widget};
pub use kanban_board::{
    CardPosition, KanbanBoard, KanbanCard, KanbanCardView, KanbanColumn, KanbanColumnHeader,
};
