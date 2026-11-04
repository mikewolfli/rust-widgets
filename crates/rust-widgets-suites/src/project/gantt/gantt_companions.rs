// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gantt companion controls (COMP-14): timeline axis, dependency editor and task grid.
//!
//! Each control has its own file under this module's folder; this file only declares and
//! re-exports them, so the historical `gantt_companions::TimelineAxis`-style paths keep
//! resolving while the family is one folder.

mod common;
mod dependency_editor;
mod task_grid;
mod timeline_axis;

pub use dependency_editor::{Dependency, DependencyEditor, DEPENDENCY_SCHEMA};
pub use task_grid::{GanttTaskGrid, TaskColumn};
pub use timeline_axis::{DateFormat, TimelineAxis, AXIS_MAX_ZOOM, AXIS_MIN_ZOOM};
