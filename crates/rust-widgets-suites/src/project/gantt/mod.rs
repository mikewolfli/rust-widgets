// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The Gantt family: the chart control and its two companion controls.
//!
//! This folder groups the whole family so its members are identifiable at a glance:
//!
//! * [`gantt_widget`] — the main `GanttWidget` (task model, bar rows, dependencies,
//!   critical path, and the interaction that edits them).
//! * [`gantt_companions`] — the companion controls it ships with: the
//!   `TimelineAxis` and the `DependencyEditor`.
//!
//! Each control stays in its own file; this module only declares them. The previous
//! flat paths (`chart_widgets::gantt_widget` and `chart_widgets::gantt_companions`)
//! keep resolving through the re-exports in [`super`].

pub mod gantt_companions;
pub mod gantt_widget;
