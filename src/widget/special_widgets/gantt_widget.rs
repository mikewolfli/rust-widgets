// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GanttWidget for timeline task planning.

use crate::compat::HashMap;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_i64;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How far past the predecessor's right edge a dependency elbow turns: a short stub, so a link is
/// visibly an elbow and not a straight diagonal when the two bars are close on the axis.
const LINK_STUB: i32 = 4;
/// The half-width of a dependency arrow head, in pixels.
const LINK_HEAD: i32 = 4;

/// The width of a bar's start/end resize handle, in pixels.
const RESIZE_HANDLE: i32 = 5;

/// The height of the strip along a bar's bottom edge that drags its progress.
const PROGRESS_STRIP: i32 = 7;

/// What a drag on a task row is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GanttDragKind {
    /// The whole bar (or milestone) moves.
    Move,
    /// The start edge moves; the end is pinned.
    ResizeStart,
    /// The end edge moves; the start is pinned.
    ResizeEnd,
    /// The completion percentage follows the pointer.
    Progress,
}

/// A drag in progress.
struct GanttDrag {
    /// The task's index at press time.
    task: usize,
    /// The pointer position the drag started from.
    origin: Point,
    /// The task's start at press time.
    start: i64,
    /// The task's end at press time.
    end: i64,
    /// Which part of the bar is being edited.
    kind: GanttDragKind,
}

/// One gantt task bar.
///
/// Describes a single bar on the timeline. The widget treats `start` and `end`
/// as opaque integers on a linear axis — no calendar or timezone interpretation
/// is applied, so the unit (days since epoch, milliseconds, sprint numbers, ...)
/// is whatever the caller decides. Only their ordering and difference matter for
/// drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GanttTask {
    /// Stable identifier. Not shown to the user; it is how a caller correlates a
    /// selected task with its own data.
    pub id: String,
    /// Caption drawn on or beside the bar.
    pub label: String,
    /// Start position on the task axis.
    pub start: i64,
    /// End position on the task axis. Must be at least `start`; a reversed range
    /// yields a zero- or negative-width bar rather than an error.
    pub end: i64,
    /// Completion percentage, `0 ..= 100`. Values above 100 are not clamped by
    /// the type, so a caller is responsible for the range; drawing a value over
    /// 100 would overflow the bar.
    pub progress: u8,
    /// The tasks this one waits on, by [`id`](Self::id).
    ///
    /// A dependency is a **relationship between two bars**, and a gantt without it is a bar chart
    /// with a time axis. The widget resolves these against the whole task list and draws an elbow
    /// from each predecessor's right edge to this task's left edge; an id that names no task is
    /// skipped rather than reported, because a caller editing a schedule will transiently hold
    /// dangling ids and a chart is not the place to complain about it. Empty means "no
    /// predecessors", which is the common case and what every task built through
    /// [`GanttTask::new`] gets.
    pub depends_on: Vec<String>,
    /// Typed dependencies, carrying the relationship kind.
    ///
    /// `depends_on` is the finish-to-start spelling kept for callers that predate the typed
    /// form; both are resolved when the elbows are drawn.
    pub dependencies: Vec<GanttDependency>,
    /// Whether this row is a **milestone** rather than a bar.
    ///
    /// A milestone is a single instant, so it is drawn as a diamond at [`start`](Self::start) rather
    /// than a bar spanning a range — the one shape in a gantt that carries meaning no bar can, and
    /// the reason a zero-length task has to be expressible at all. [`GanttTask::new`] widens a
    /// reversed range to `start + 1` so an ordinary bar is never zero-length; a milestone opts out
    /// of that by asking for it explicitly.
    pub milestone: bool,
}

impl GanttTask {
    /// Creates task.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        start: i64,
        end: i64,
        progress: u8,
    ) -> Self {
        let s = start;
        let e = end.max(s + 1);
        Self {
            id: id.into(),
            label: label.into(),
            start: s,
            end: e,
            progress: progress.min(100),
            depends_on: Vec::new(),
            dependencies: Vec::new(),
            milestone: false,
        }
    }

    /// Creates a **milestone**: a single instant drawn as a diamond, not a bar.
    ///
    /// `end` is set equal to `start` rather than widened, because a milestone *is* an instant —
    /// that is the whole distinction from a bar, and widening it would erase it.
    pub fn milestone(id: impl Into<String>, label: impl Into<String>, at: i64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            start: at,
            end: at,
            progress: 0,
            depends_on: Vec::new(),
            dependencies: Vec::new(),
            milestone: true,
        }
    }

    /// Declares that this task waits on `predecessors`, by id.
    ///
    /// A builder rather than a constructor argument: most tasks have no dependencies, and a
    /// five-argument constructor that made every caller write `vec![]` for the common case would
    /// have been the wrong default.
    pub fn depends_on(mut self, predecessors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.depends_on = predecessors.into_iter().map(Into::into).collect();
        self
    }

    /// Declares a typed dependency on `id`.
    pub fn with_dependency(mut self, id: impl Into<String>, kind: DependencyKind) -> Self {
        self.dependencies.push(GanttDependency { id: id.into(), kind });
        self
    }
}

/// How one task waits on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DependencyKind {
    /// The successor starts when the predecessor finishes; the default elbow.
    #[default]
    FinishToStart,
    /// Both start together, so the elbow leaves the predecessor's left edge.
    StartToStart,
    /// Both finish together, so the elbow enters the successor's right edge.
    FinishToFinish,
}

/// One typed dependency edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GanttDependency {
    /// The predecessor's task id.
    pub id: String,
    /// The relationship kind.
    pub kind: DependencyKind,
}

/// Basic gantt widget with selectable task rows and zoomable viewport.
pub struct GanttWidget {
    base: BaseWidget,
    tasks: Vec<GanttTask>,
    selected_index: Option<usize>,
    viewport_start: i64,
    viewport_end: i64,
    row_height: u32,
    /// The axis position to mark as "now", if any.
    ///
    /// A gantt is read against the present: the whole question it answers is "where are we against
    /// the plan", and the answer is invisible without this line. `Option` because the axis unit is
    /// the caller's (days since epoch, sprint numbers, …) — the widget cannot invent "now" on an
    /// axis it cannot interpret, so it draws the line only when told where now is.
    today: Option<i64>,
    /// Rows scrolled out of view at the top.
    scroll_offset: usize,
    /// Whether the time axis header is drawn.
    show_time_axis: bool,
    /// The number of ticks on that axis.
    time_axis_ticks: u32,
    /// Whether the critical path is highlighted.
    show_critical_path: bool,
    /// Whether the caller-supplied "now" line is drawn.
    show_today: bool,
    /// Task id to index, so a dependency is resolved in constant time.
    id_index: HashMap<String, usize>,
    /// A drag in progress.
    drag: Option<GanttDrag>,
    /// Emitted when task selected. Payload is task id.
    pub task_selected: Signal1<String>,
    /// Emitted when a task's dates change through a drag. Payload is task id.
    pub task_rescheduled: Signal1<String>,
    /// Emitted when a resize (or progress) drag starts. Payload is task id.
    pub task_resize_started: Signal1<String>,
    /// Emitted when a resize (or progress) drag ends. Payload is task id.
    pub task_resize_finished: Signal1<String>,
    /// Emitted when the visible axis range changes.
    pub viewport_changed: Signal1<(i64, i64)>,
}

impl GanttWidget {
    /// Creates empty gantt.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "GanttWidget"),
            tasks: Vec::new(),
            selected_index: None,
            viewport_start: 0,
            viewport_end: 100,
            row_height: 24,
            // No "now" until the caller says where it is: the axis unit is theirs.
            today: None,
            scroll_offset: 0,
            show_time_axis: false,
            time_axis_ticks: 4,
            show_critical_path: false,
            show_today: true,
            id_index: HashMap::new(),
            drag: None,
            task_selected: Signal1::new(),
            task_rescheduled: Signal1::new(),
            task_resize_started: Signal1::new(),
            task_resize_finished: Signal1::new(),
            viewport_changed: Signal1::new(),
        }
    }

    /// Shows or hides the time axis header.
    ///
    /// The header is opt-in because it shifts the lanes down by its own height; a schedule that
    /// sits in a fixed-height card keeps its previous pixel layout until it asks for the axis.
    pub fn set_show_time_axis(&mut self, show: bool) {
        if self.show_time_axis == show {
            return;
        }
        self.show_time_axis = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether the time axis header is shown.
    pub fn show_time_axis(&self) -> bool {
        self.show_time_axis
    }

    /// Sets the number of time-axis ticks.
    pub fn set_time_axis_ticks(&mut self, ticks: u32) {
        let ticks = ticks.clamp(2, 12);
        if self.time_axis_ticks == ticks {
            return;
        }
        self.time_axis_ticks = ticks;
        self.base.request_redraw();
    }

    /// Returns the number of time-axis ticks.
    pub fn time_axis_ticks(&self) -> u32 {
        self.time_axis_ticks
    }

    /// Sets the row height in pixels (floored at 8).
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(8);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns the row height in pixels.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Whether the critical path is highlighted.
    pub fn show_critical_path(&self) -> bool {
        self.show_critical_path
    }

    /// Enables highlighting of the longest dependency chain.
    pub fn set_show_critical_path(&mut self, show: bool) {
        if self.show_critical_path == show {
            return;
        }
        self.show_critical_path = show;
        self.base.request_redraw();
    }

    /// Whether the "now" line is drawn when a position is set.
    pub fn show_today(&self) -> bool {
        self.show_today
    }

    /// Shows or hides the "now" line without discarding its position.
    pub fn set_show_today(&mut self, show: bool) {
        if self.show_today == show {
            return;
        }
        self.show_today = show;
        self.base.request_redraw();
    }

    /// The task ids on the longest dependency chain, in schedule order.
    ///
    /// A schedule's critical path is the chain of dependent tasks with the largest total duration:
    /// every day it slips, the whole project slips by one. The chain is computed over the typed and
    /// untyped dependency lists together, so both spellings contribute. Cyclic dependencies have no
    /// longest chain; the nodes reachable from a cycle are omitted rather than reported as critical,
    /// because a cycle is a caller error and inventing a path through it would hide that.
    pub fn critical_path(&self) -> Vec<String> {
        self.critical_path_indices()
            .into_iter()
            .filter_map(|index| self.tasks.get(index).map(|task| task.id.clone()))
            .collect()
    }

    /// The task indices on the critical path.
    fn critical_path_indices(&self) -> Vec<usize> {
        let count = self.tasks.len();
        if count == 0 {
            return Vec::new();
        }
        let duration = |index: usize| -> i64 {
            self.tasks.get(index).map_or(0, |task| (task.end - task.start).max(0))
        };
        // Kahn's topological order. `edges[pred]` lists the successors that wait on it.
        let mut edges: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut indegree = vec![0usize; count];
        for successor in 0..count {
            for predecessor in self.predecessor_indices(successor) {
                edges[predecessor].push(successor);
                indegree[successor] += 1;
            }
        }
        let mut queue: Vec<usize> = (0..count).filter(|index| indegree[*index] == 0).collect();
        let mut order = Vec::with_capacity(count);
        let mut cursor = 0usize;
        while cursor < queue.len() {
            let node = queue[cursor];
            cursor += 1;
            order.push(node);
            for successor in &edges[node] {
                indegree[*successor] -= 1;
                if indegree[*successor] == 0 {
                    queue.push(*successor);
                }
            }
        }
        // `finish[i]` is the longest chain that **ends** at `i`; `previous[i]` is the predecessor
        // that chain came through, which is what makes the answer a path rather than a number.
        let mut finish = vec![0i64; count];
        let mut previous: Vec<Option<usize>> = vec![None; count];
        let mut ordered = vec![false; count];
        for node in order {
            ordered[node] = true;
            let candidate = finish[node] + duration(node);
            for successor in &edges[node] {
                if candidate > finish[*successor] {
                    finish[*successor] = candidate;
                    previous[*successor] = Some(node);
                }
            }
        }
        // Only nodes that reached a topological position can be on a chain. A node inside a cycle
        // never did, and reporting it as "critical" would invent a path through a schedule error.
        let Some(end) = (0..count)
            .filter(|index| ordered[*index])
            .max_by_key(|index| finish[*index] + duration(*index))
        else {
            return Vec::new();
        };
        if finish[end] == 0 && previous[end].is_none() && duration(end) == 0 {
            // Every task is a zero-length isolated node: there is no chain to mark, and naming an
            // arbitrary one would be a claim the data does not support.
            return Vec::new();
        }
        let mut path = Vec::new();
        let mut node = Some(end);
        while let Some(index) = node {
            path.push(index);
            node = previous[index];
        }
        path.reverse();
        path
    }

    /// The indices of `index`'s predecessors, from both dependency spellings.
    fn predecessor_indices(&self, index: usize) -> Vec<usize> {
        let Some(task) = self.tasks.get(index) else {
            return Vec::new();
        };
        let mut out: Vec<usize> = Vec::new();
        let ids =
            task.depends_on.iter().chain(task.dependencies.iter().map(|dependency| &dependency.id));
        for id in ids {
            if let Some(found) = self.id_index.get(id).copied() {
                if found != index && !out.contains(&found) {
                    out.push(found);
                }
            }
        }
        out
    }

    /// The header height the time axis reserves.
    fn axis_height(&self) -> i32 {
        if self.show_time_axis {
            18
        } else {
            0
        }
    }

    /// The lane rectangle for a task row.
    fn lane_rect(&self, visible_row: usize) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x,
            rect.y + self.axis_height() + visible_row as i32 * self.row_height as i32,
            rect.width,
            self.row_height,
        )
    }

    /// How many task lanes fit under the axis.
    fn visible_row_count(&self) -> usize {
        let height = self.geometry().height.saturating_sub(self.axis_height().max(0) as u32);
        (height / self.row_height.max(1)) as usize
    }

    /// Returns how many rows are scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the task lanes by `delta`.
    pub fn scroll_by(&mut self, delta: isize) {
        let max = self.tasks.len().saturating_sub(self.visible_row_count());
        let next = (self.scroll_offset as isize + delta).clamp(0, max as isize) as usize;
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    /// Maps a pointer x to an axis position.
    fn axis_value_at(&self, x: i32) -> i64 {
        let rect = self.geometry();
        let track_x = rect.x + dimensions::CHART_LABEL_GUTTER;
        let track_w = rect
            .width
            .saturating_sub(
                dimensions::CHART_LABEL_GUTTER as u32 + dimensions::CHART_TRACK_MARGIN as u32,
            )
            .max(1);
        let ratio = ((x - track_x) as f64 / track_w as f64).clamp(0.0, 1.0);
        let span = (self.viewport_end - self.viewport_start).max(1);
        self.viewport_start + (ratio * span as f64).round() as i64
    }

    /// The time track's left edge and width.
    ///
    /// One derivation shared by the press hit test and the progress drag: the two used to respell
    /// `CHART_LABEL_GUTTER`/`CHART_TRACK_MARGIN` and could drift apart.
    fn track_geometry(&self) -> (i32, u32) {
        let rect = self.geometry();
        (
            rect.x + dimensions::CHART_LABEL_GUTTER,
            rect.width
                .saturating_sub(
                    dimensions::CHART_LABEL_GUTTER as u32 + dimensions::CHART_TRACK_MARGIN as u32,
                )
                .max(1),
        )
    }

    /// Sets `index`'s progress from the pointer's x over the bar `start..end`.
    ///
    /// Shared by the press and the move so that a press inside the progress strip immediately
    /// applies the value the user pointed at, instead of waiting for a move that may never come (a
    /// click alone used to leave the old percentage in place).
    fn set_progress_from_pointer(&mut self, index: usize, start: i64, end: i64, x: i32) {
        let (track_x, track_w) = self.track_geometry();
        let x0 = self.project_x(start, track_x, track_w);
        let x1 = self.project_x(end, track_x, track_w).max(x0 + 2);
        let ratio = ((x - x0) as f32 / (x1 - x0) as f32).clamp(0.0, 1.0);
        if let Some(task) = self.tasks.get_mut(index) {
            let progress = (ratio * 100.0).round() as u8;
            if task.progress != progress {
                task.progress = progress;
                self.base.request_redraw();
            }
        }
    }

    /// Commits the drag in progress and reports what it changed.
    fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let Some(task) = self.tasks.get(drag.task) else {
            self.base.request_redraw();
            return;
        };
        let id = task.id.clone();
        if task.start != drag.start || task.end != drag.end {
            self.task_rescheduled.emit(id.clone());
        }
        if drag.kind != GanttDragKind::Move {
            self.task_resize_finished.emit(id);
        }
        self.base.request_redraw();
    }

    /// Sets tasks.
    pub fn set_tasks(&mut self, tasks: Vec<GanttTask>) {
        self.tasks = tasks;
        // The map is rebuilt with the list rather than maintained per lookup: a dependency arrow
        // resolves its predecessor through it, and `tasks.iter().position` per arrow made one paint
        // O(tasks × links). A duplicate id keeps the first row, matching the first-match rule the
        // arrow drawing had before the map existed.
        self.id_index.clear();
        for (index, task) in self.tasks.iter().enumerate() {
            self.id_index.entry(task.id.clone()).or_insert(index);
        }
        self.selected_index = None;
        self.recompute_viewport();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns tasks.
    pub fn tasks(&self) -> &[GanttTask] {
        &self.tasks
    }

    /// Returns viewport range.
    pub fn viewport(&self) -> (i64, i64) {
        (self.viewport_start, self.viewport_end)
    }

    /// Sets viewport range.
    pub fn set_viewport(&mut self, start: i64, end: i64) {
        let end = end.max(start + 1);
        if self.viewport_start == start && self.viewport_end == end {
            return;
        }
        self.viewport_start = start;
        self.viewport_end = end;
        // The range is what the axis header and every bar are projected through, so a zoom or a
        // scroll changes the picture. It used to change silently, which left a host unable to keep a
        // linked chart (the finance panes, a second gantt) in step.
        self.viewport_changed.emit((start, end));
        self.base.request_redraw();
    }

    /// Zooms around center.
    pub fn zoom(&mut self, factor: f32) {
        if factor <= 0.0 {
            return;
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f32;
        let center = (self.viewport_start + self.viewport_end) as f32 / 2.0;
        let half = (span / factor / 2.0).max(1.0);
        self.set_viewport((center - half).floor() as i64, (center + half).ceil() as i64);
    }

    /// Select task by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.tasks.len() {
            return false;
        }
        self.selected_index = Some(index);
        if let Some(task) = self.tasks.get(index) {
            self.task_selected.emit(task.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// Returns selected task id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index?;
        self.tasks.get(index).map(|task| task.id.as_str())
    }

    /// Returns the axis position marked as "now", if the caller set one.
    pub fn today(&self) -> Option<i64> {
        self.today
    }

    /// Marks `position` on the axis as "now".
    ///
    /// Pass `None` to remove the marker. The line is drawn over the bars at full plot height, which
    /// is what makes it readable against a busy schedule; before this it did not exist and the only
    /// way a reader could locate "now" was to compare today's date against the axis by eye.
    pub fn set_today(&mut self, position: Option<i64>) {
        if self.today == position {
            return;
        }
        self.today = position;
        self.base.request_redraw();
    }

    fn recompute_viewport(&mut self) {
        let (start, end) = if self.tasks.is_empty() {
            (0, 100)
        } else {
            let mut min_start = i64::MAX;
            let mut max_end = i64::MIN;
            for task in &self.tasks {
                min_start = min_start.min(task.start);
                max_end = max_end.max(task.end);
            }
            (min_start, max_end.max(min_start + 1))
        };
        self.viewport_start = start;
        self.viewport_end = end;
        self.viewport_changed.emit((start, end));
    }

    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }
        let idx = ((pos.y - rect.y - self.axis_height()) / self.row_height as i32) as usize
            + self.scroll_offset;
        (idx < self.tasks.len()
            && pos.y >= rect.y + self.axis_height()
            && ((pos.y - rect.y - self.axis_height()) / self.row_height as i32)
                < self.visible_row_count() as i32)
            .then_some(idx)
    }

    fn project_x(&self, value: i64, track_x: i32, track_w: u32) -> i32 {
        let start = self.viewport_start;
        let end = self.viewport_end.max(start + 1);
        let span = (end - start) as f32;
        let ratio = ((value - start) as f32 / span).clamp(0.0, 1.0);
        track_x + (ratio * track_w as f32) as i32
    }
}

impl Widget for GanttWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(dimensions::GANTT_DEFAULT_WIDTH, dimensions::GANTT_DEFAULT_HEIGHT)
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "task_selected" => {
                Some(EventSignalRef::mapped("task_selected", &self.task_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "task_rescheduled" => {
                Some(EventSignalRef::mapped("task_rescheduled", &self.task_rescheduled, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "task_resize_started" => Some(EventSignalRef::mapped(
                "task_resize_started",
                &self.task_resize_started,
                |v| CapabilityValue::String(v.clone()),
            )),
            "task_resize_finished" => Some(EventSignalRef::mapped(
                "task_resize_finished",
                &self.task_resize_finished,
                |v| CapabilityValue::String(v.clone()),
            )),
            "viewport_changed" => {
                Some(EventSignalRef::mapped("viewport_changed", &self.viewport_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Int(v.0),
                        CapabilityValue::Int(v.1),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// `GanttWidget`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `GanttWidget` reports
/// `WidgetKind::Chart`, shared with `ChartWidget` and `TimelineWidget`;
/// dispatching on the concrete type here is what keeps the three contracts
/// separate.
impl WidgetProperties for GanttWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "task_count" => Ok(CapabilityValue::UInt(self.tasks().len() as u64)),
            "selected_id" => match self.selected_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "viewport_start" => Ok(CapabilityValue::Int(self.viewport().0)),
            "viewport_end" => Ok(CapabilityValue::Int(self.viewport().1)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "show_critical_path" => Ok(CapabilityValue::Bool(self.show_critical_path())),
            "show_today" => Ok(CapabilityValue::Bool(self.show_today())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "viewport_start" => {
                let start = expect_i64(value)?;
                let (_, end) = self.viewport();
                self.set_viewport(start, end);
                Ok(())
            }
            "viewport_end" => {
                let end = expect_i64(value)?;
                let (start, _) = self.viewport();
                self.set_viewport(start, end);
                Ok(())
            }
            "row_height" => {
                let height = match value {
                    CapabilityValue::UInt(height) => {
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_row_height(height);
                Ok(())
            }
            "show_critical_path" => {
                self.set_show_critical_path(crate::widget::capability::coercion::expect_bool(
                    value,
                )?);
                Ok(())
            }
            "show_today" => {
                self.set_show_today(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            // Derived counts and the selection identity are computed from the task
            // list, so they are refused as read-only rather than reported as names
            // this control does not know.
            "task_count" | "selected_id" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "task_count",
            "selected_id",
            "viewport_start",
            "viewport_end",
            "row_height",
            "show_critical_path",
            "show_today",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `gantt_widget` publishes.
    ///
    /// `zoom` is payload-free here: a bare invocation zooms in around the
    /// centre, which is the direction the affordance always offers, so the
    /// widget's real `zoom` receives a fixed factor. `set_tasks`, `set_viewport`
    /// and `select_task` need the tasks, the bounds and the index respectively.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom" => {
                self.zoom(1.25);
                Ok(())
            }
            "set_tasks" | "set_viewport" | "select_task" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for GanttWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                let Some(index) = self.row_at(*pos) else {
                    return;
                };
                let _ = self.select_index(index);
                let Some(task) = self.tasks.get(index) else {
                    return;
                };
                let (start, end, milestone, id) =
                    (task.start, task.end, task.milestone, task.id.clone());
                let (track_x, track_w) = self.track_geometry();
                let x0 = self.project_x(start, track_x, track_w);
                let x1 = self.project_x(end, track_x, track_w).max(x0 + 2);
                let lane_y = self.lane_rect(index - self.scroll_offset).y;
                let bar_top = lane_y + 5;
                let bar_bottom = lane_y + self.row_height as i32 - 5;
                let kind = if milestone {
                    // A milestone is a diamond, not a bar, so only the diamond itself is grabbable.
                    // It used to be undraggable at all, which left the one point-shaped mark on the
                    // chart as the only thing a user could not move.
                    let half = (self.row_height.saturating_sub(10) / 2) as i32;
                    if (pos.x - x0).abs() > half + 2 {
                        return;
                    }
                    GanttDragKind::Move
                } else {
                    if pos.x < x0 || pos.x > x1 || pos.y < bar_top || pos.y > bar_bottom {
                        return;
                    }
                    // The edges resize, the bottom strip sets progress, and the rest moves the bar.
                    // Before this the whole bar moved regardless of where it was grabbed, so a
                    // schedule could not be adjusted without editing the task list.
                    if pos.x <= x0 + RESIZE_HANDLE {
                        GanttDragKind::ResizeStart
                    } else if pos.x >= x1 - RESIZE_HANDLE {
                        GanttDragKind::ResizeEnd
                    } else if pos.y >= bar_bottom - PROGRESS_STRIP {
                        GanttDragKind::Progress
                    } else {
                        GanttDragKind::Move
                    }
                };
                self.drag = Some(GanttDrag { task: index, origin: *pos, start, end, kind });
                if kind == GanttDragKind::Progress {
                    self.set_progress_from_pointer(index, start, end, pos.x);
                }
                if kind != GanttDragKind::Move {
                    self.task_resize_started.emit(id);
                }
            }
            Event::MouseMove { pos } => {
                let Some(drag) = self.drag.as_ref() else {
                    return;
                };
                let (index, origin, start, end, kind) =
                    (drag.task, drag.origin, drag.start, drag.end, drag.kind);
                if index >= self.tasks.len() {
                    return;
                }
                // The delta is converted through the axis rather than a pixels-per-unit
                // constant, so the bar tracks the pointer at any zoom level. Both endpoints are
                // recomputed from the **press** value, so a drag that reverses direction across the
                // origin lands where the pointer is rather than drifting by the distance travelled.
                let shift = self.axis_value_at(pos.x) - self.axis_value_at(origin.x);
                if kind == GanttDragKind::Progress {
                    self.set_progress_from_pointer(index, start, end, pos.x);
                    return;
                }
                if shift == 0 {
                    return;
                }
                let Some(task) = self.tasks.get_mut(index) else {
                    return;
                };
                match kind {
                    GanttDragKind::Move => {
                        if start + shift >= 0 {
                            task.start = start + shift;
                            task.end = end + shift;
                        }
                    }
                    // A resize keeps the opposite edge pinned and never crosses it: a reversed
                    // range would paint a negative-width bar, which reads as a rendering fault.
                    GanttDragKind::ResizeStart => {
                        task.start = (start + shift).clamp(0, end - 1);
                        task.end = end;
                    }
                    GanttDragKind::ResizeEnd => {
                        task.start = start;
                        task.end = (end + shift).max(start + 1);
                    }
                    GanttDragKind::Progress => unreachable!("progress returned above"),
                }
                self.base.request_redraw();
            }
            Event::MouseRelease { button: 1, .. } => self.finish_drag(),
            // A drag that loses the pointer must not leave a bar following a pointer that is no
            // longer there; the gesture is committed at its last position.
            Event::MouseLeave { .. } => self.finish_drag(),
            Event::Wheel { delta, modifiers } => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
                    if delta.y != 0 {
                        self.scroll_by(if delta.y > 0 { 1 } else { -1 });
                    }
                } else if delta.y < 0 {
                    self.zoom(1.2);
                } else if delta.y > 0 {
                    self.zoom(0.8);
                }
            }
            Event::KeyPress { key, .. } => match *key {
                38 => {
                    let index = self.selected_index.map_or(0, |index| index.saturating_sub(1));
                    let _ = self.select_index(index);
                }
                40 => {
                    if !self.tasks.is_empty() {
                        let index = self
                            .selected_index
                            .map_or(0, |index| (index + 1).min(self.tasks.len() - 1));
                        let _ = self.select_index(index);
                    }
                }
                33 => self.scroll_by(-(self.visible_row_count().max(1) as isize)),
                34 => self.scroll_by(self.visible_row_count().max(1) as isize),
                _ => {}
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for GanttWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("gantt_widget");
        // `gantt_widget` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The chart's own fill is therefore a step toward
        // the foreground, so it reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(251, 252, 254));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // A selected lane is a chrome state of the chart, so it is derived from the
        // resolved pair rather than a literal tint.
        let selected_lane = background.blend(&text_color, 0.14);
        // Task bars are data marks drawn in the accent colour, which is what makes
        // them a chart rather than chrome.
        let bar_color = crate::style::resolved_theme_style_shared("slider")
            .and_then(|accent| accent.background_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.55));
        let progress_color = bar_color.blend(&text_color, 0.18);
        // The row separator is secondary chrome, derived from the same pair.
        let separator = background.blend(&text_color, 0.12);
        // The "now" line is a **semantic** mark, not chrome: it answers the question the chart exists
        // to answer, so it takes the theme's error role -- the one colour in the palette that is
        // universally read as "pay attention to this instant". It read the `badge` style's text
        // colour instead, and `badge` is not a role, so the mark was painted in ordinary ink.
        let today_color = crate::style::semantic_color(crate::style::SemanticColor::Error)
            .unwrap_or_else(|| background.blend(&text_color, 0.85));
        // Dependencies are structure, not data: a line that connected bars in the accent colour would
        // compete with the bars themselves.
        let link_color = background.blend(&text_color, 0.45);
        // The critical path is a *state* of the schedule ("this chain decides the finish date"), so
        // it takes a semantic role rather than a fourth literal.
        let critical_color = crate::style::semantic_color(crate::style::SemanticColor::Warning)
            .map(|token| token.blend(&background, 0.25))
            .unwrap_or_else(|| bar_color.blend(&progress_color, 0.5));
        let critical_flags: Vec<bool> = {
            let mut flags = vec![false; self.tasks.len()];
            if self.show_critical_path {
                for index in self.critical_path_indices() {
                    if let Some(flag) = flags.get_mut(index) {
                        *flag = true;
                    }
                }
            }
            flags
        };

        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        // The label column and the track share one derivation: `CHART_LABEL_GUTTER` is the room
        // reserved for task names, and the track begins at its trailing edge and ends a
        // `CHART_TRACK_MARGIN` short of the control's right edge. The pair this replaces —
        // `rect.x + 150` for the track and `rect.width - 160` for its length — encoded the same
        // fact twice with two different numbers, so the track began 10 px past the gutter its
        // labels were measured against.
        let track_x = rect.x + dimensions::CHART_LABEL_GUTTER;
        let track_w = rect.width.saturating_sub(
            dimensions::CHART_LABEL_GUTTER as u32 + dimensions::CHART_TRACK_MARGIN as u32,
        );

        // ── Time axis header ──
        //
        // Opt-in because it shifts the lanes down. Ticks are evenly spaced across the track and
        // labelled with their axis position, so the bars can be read against a scale instead of
        // only against each other.
        if self.show_time_axis {
            let header = Rect::new(rect.x, rect.y, rect.width, self.axis_height().max(1) as u32);
            context.draw_line(
                Point::new(header.x, header.y + header.height as i32 - 1),
                Point::new(header.x + header.width as i32, header.y + header.height as i32 - 1),
                separator,
            );
            let ticks = self.time_axis_ticks.max(2);
            let span = self.viewport_end - self.viewport_start;
            for tick in 0..=ticks {
                let ratio = tick as f64 / f64::from(ticks);
                let value = self.viewport_start + (span as f64 * ratio).round() as i64;
                let x = track_x + (track_w as f64 * ratio) as i32;
                let band = Rect::new((x - 24).max(rect.x), header.y, 48, header.height);
                context.draw_text_line_cow(
                    band,
                    &value.to_string(),
                    effective_font(style),
                    text_color.blend(&background, 0.3),
                    HorizontalAlignment::Center,
                );
            }
        }

        let first_visible = self.scroll_offset;
        let visible_rows = self.visible_row_count();
        for visible_row in 0..visible_rows {
            let index = first_visible + visible_row;
            let Some(task) = self.tasks.get(index) else {
                break;
            };
            let y = self.lane_rect(visible_row).y;
            if y + self.row_height as i32 > rect.y + rect.height as i32 {
                break;
            }
            if self.selected_index == Some(index) {
                context.fill_rect(Rect::new(rect.x, y, rect.width, self.row_height), selected_lane);
            }

            // The lane label is centred in its lane. The raw midpoint put the glyph box's
            // *top* edge on the lane's middle line, half a line low; the fix is the shared
            // *line-box* primitive, not a wider band — `draw_text_fitted` centres only the
            // horizontal axis and leaves `y` at the band's top edge, so handing it a
            // `row_height`-tall band pinned every task name to the top of its lane again.
            // `draw_text_line` is the entry point that centres on both axes.
            let label_band =
                Rect::new(rect.x, y, track_x.saturating_sub(rect.x) as u32, self.row_height);
            context.draw_text_line_cow(
                label_band,
                &task.label,
                effective_font(style),
                text_color,
                HorizontalAlignment::Left,
            );

            let x0 = self.project_x(task.start, track_x, track_w);

            // ── A milestone is a diamond, not a bar ──
            //
            // It marks an instant, and an instant has no width, so drawing it as a bar would state
            // something false about the schedule. The half-size is half the lane's bar height, which
            // is what makes it read as the same weight as its neighbours' bars.
            if task.milestone {
                let bar_h = self.row_height.saturating_sub(10);
                let half = (bar_h / 2) as i32;
                let cy = y + 5 + half;
                let points = vec![
                    Point::new(x0, cy - half),
                    Point::new(x0 + half, cy),
                    Point::new(x0, cy + half),
                    Point::new(x0 - half, cy),
                ];
                let fill = if critical_flags.get(index).copied().unwrap_or(false) {
                    critical_color
                } else {
                    bar_color
                };
                context.execute_command(crate::render::RenderCommand::DrawPath {
                    points,
                    closed: true,
                    color: fill,
                    filled: true,
                    width: 1,
                });
                context.draw_line(
                    Point::new(rect.x, y + self.row_height as i32),
                    Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                    separator,
                );
                continue;
            }

            let x1 = self.project_x(task.end, track_x, track_w).max(x0 + 2);
            let bar_h = self.row_height.saturating_sub(10);
            let bar_rect = Rect::new(x0, y + 5, (x1 - x0) as u32, bar_h);
            let fill = if critical_flags.get(index).copied().unwrap_or(false) {
                critical_color
            } else {
                bar_color
            };
            let progress_fill = fill.blend(&text_color, 0.18);
            context.fill_rect(bar_rect, fill);

            let progress_w =
                ((bar_rect.width as f32) * (task.progress as f32 / 100.0)).round() as u32;
            if progress_w > 0 {
                context.fill_rect(
                    Rect::new(bar_rect.x, bar_rect.y, progress_w, bar_rect.height),
                    progress_fill,
                );
            }

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                separator,
            );
        }

        // ── Dependency arrows ──
        //
        // Drawn **after** every bar, so a successor's bar cannot cover the elbow that explains it.
        // The shape is the conventional one: out of the predecessor's right edge, a half-gap to the
        // right or left, then into the successor's left edge -- two segments and an arrow head, which
        // is the minimum that still says "this one waits for that one" rather than "these two happen
        // to be near each other".
        for visible_row in 0..visible_rows {
            let index = first_visible + visible_row;
            let Some(task) = self.tasks.get(index) else {
                break;
            };
            let y = self.lane_rect(visible_row).y;
            if y + self.row_height as i32 > rect.y + rect.height as i32 {
                break;
            }
            let predecessors = task
                .depends_on
                .iter()
                .map(|id| (id.as_str(), DependencyKind::FinishToStart))
                .chain(
                    task.dependencies
                        .iter()
                        .map(|dependency| (dependency.id.as_str(), dependency.kind)),
                );
            for (predecessor_id, kind) in predecessors {
                // Constant-time id resolution. The per-arrow `tasks.iter().position` made one paint
                // quadratic in the schedule's size; the map is built and maintained by `set_tasks`.
                let Some(pred_index) = self.id_index.get(predecessor_id).copied() else {
                    // A dangling id is skipped rather than reported: a caller editing a schedule
                    // transiently holds them, and a chart is not the place to complain.
                    continue;
                };
                let Some(pred) = self.tasks.get(pred_index) else {
                    continue;
                };
                // A predecessor scrolled out of view still has a known **axis** position — only its
                // lane is off screen. The elbow is clipped to the visible band instead of dropped,
                // which is what keeps a dependency visible when the task it comes from is above or
                // below the viewport. Dropping it made the link silently disappear on scroll.
                let pred_visible_row = pred_index as i64 - first_visible as i64;
                let pred_offset = pred_visible_row
                    .clamp(-(self.tasks.len() as i64), self.tasks.len() as i64)
                    as i32;
                let pred_y = rect.y
                    + self.axis_height()
                    + pred_offset.saturating_mul(self.row_height as i32);
                // The three relationship kinds differ in which edges the elbow joins: a
                // finish-to-start link leaves the predecessor's end and enters the successor's
                // start, start-to-start joins the two left edges, finish-to-finish the two right
                // edges. Routing each with its own pair of edges is what makes the arrow state the
                // relationship instead of only the dependency.
                let (from_x, to_x) = match kind {
                    DependencyKind::FinishToStart => (
                        self.project_x(pred.end.max(pred.start + 1), track_x, track_w),
                        self.project_x(task.start, track_x, track_w),
                    ),
                    DependencyKind::StartToStart => (
                        self.project_x(pred.start, track_x, track_w),
                        self.project_x(task.start, track_x, track_w),
                    ),
                    DependencyKind::FinishToFinish => (
                        self.project_x(pred.end.max(pred.start + 1), track_x, track_w),
                        self.project_x(task.end.max(task.start + 1), track_x, track_w),
                    ),
                };
                let from_y = (pred_y + self.row_height as i32 / 2)
                    .clamp(rect.y, rect.y + rect.height as i32 - 1);
                let to_y = y + self.row_height as i32 / 2;
                // The elbow's vertical segment sits a short step past the predecessor's right edge,
                // so a forward link and a backward one both read as an elbow rather than as a
                // straight diagonal. One expression, not a branch on the direction: the step is the
                // same either way, and the branch would have been two spellings of one value.
                let mid_x = from_x + LINK_STUB;
                context.draw_line(
                    Point::new(from_x, from_y),
                    Point::new(mid_x, from_y),
                    link_color,
                );
                context.draw_line(Point::new(mid_x, from_y), Point::new(mid_x, to_y), link_color);
                context.draw_line(Point::new(mid_x, to_y), Point::new(to_x, to_y), link_color);
                // The head: a small solid triangle at the successor's left edge, pointing right.
                let head = LINK_HEAD;
                context.execute_command(crate::render::RenderCommand::DrawPath {
                    points: vec![
                        Point::new(to_x, to_y),
                        Point::new(to_x - head, to_y - head / 2),
                        Point::new(to_x - head, to_y + head / 2),
                    ],
                    closed: true,
                    color: link_color,
                    filled: true,
                    width: 1,
                });
            }
        }

        // ── The "now" line ──
        //
        // Last, so nothing hides it: it is the one mark on the chart that is *about* the reader.
        // Painted only when it falls inside the viewport, because `project_x` extrapolates linearly
        // and would otherwise draw a confident line at a position off the plot.
        if let Some(today) = self.today {
            if self.show_today && (self.viewport_start..=self.viewport_end).contains(&today) {
                let x = self.project_x(today, track_x, track_w);
                let top = rect.y;
                let bottom = rect.y + rect.height as i32;
                context.draw_line(Point::new(x, top), Point::new(x, bottom), today_color);
                // A one-pixel line at the bottom edge is how a "now" mark is told apart from a
                // separator at a glance when the two coincide.
                context.draw_line(
                    Point::new(x.saturating_sub(2), bottom),
                    Point::new(x + 2, bottom),
                    today_color,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_tasks() -> Vec<GanttTask> {
        vec![
            GanttTask::new("t1", "Design", 0, 10, 100),
            GanttTask::new("t2", "Build", 8, 22, 60),
            GanttTask::new("t3", "Verify", 20, 30, 20),
        ]
    }

    #[test]
    fn set_tasks_recomputes_viewport() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        assert_eq!(gantt.viewport(), (0, 30));
        assert_eq!(gantt.tasks().len(), 3);
    }

    #[test]
    fn selecting_task_emits_signal() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        let selected = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = selected.clone();
        gantt.task_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(gantt.select_index(1));
        assert_eq!(gantt.selected_id(), Some("t2"));

        let got = selected.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["t2".to_string()]);
    }

    #[test]
    fn wheel_zoom_changes_viewport_span() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());

        let before = gantt.viewport();
        gantt.handle_event(&Event::wheel(0, -120, 0));
        let after_in = gantt.viewport();
        assert!((after_in.1 - after_in.0) < (before.1 - before.0));

        gantt.handle_event(&Event::wheel(0, 120, 0));
        let after_out = gantt.viewport();
        assert!((after_out.1 - after_out.0) >= (after_in.1 - after_in.0));
    }

    #[test]
    fn new_creates_default_state() {
        let gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        assert!(gantt.tasks().is_empty());
        assert_eq!(gantt.selected_id(), None);
        assert_eq!(gantt.viewport(), (0, 100));
    }

    #[test]
    fn task_creation_validates_end_gt_start() {
        let task = GanttTask::new("t1", "Task", 10, 5, 50);
        assert!(task.end > task.start);
        assert_eq!(task.start, 10);
        assert_eq!(task.end, 11); // end clamped to start + 1
    }

    #[test]
    fn progress_clamp_upper_bound() {
        let task = GanttTask::new("t1", "Task", 0, 10, 200);
        assert_eq!(task.progress, 100);
    }

    #[test]
    fn progress_clamp_lower_bound() {
        // Progress is u8, so 0 is the minimum. Just check behavior.
        let task = GanttTask::new("t1", "Task", 0, 10, 0);
        assert_eq!(task.progress, 0);
    }

    #[test]
    fn select_index_out_of_bounds_returns_false() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        assert!(!gantt.select_index(10));
        assert_eq!(gantt.selected_id(), None);
    }

    #[test]
    fn select_index_duplicate_guard() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        assert!(gantt.select_index(0));
        assert_eq!(gantt.selected_id(), Some("t1"));

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        gantt.task_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });
        gantt.select_index(0);
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1); // note: select_index always emits even if same
                                  // This is the existing behavior - select_index does not guard
    }

    #[test]
    fn set_viewport_updates_range() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_viewport(50, 150);
        assert_eq!(gantt.viewport(), (50, 150));
    }

    #[test]
    fn set_viewport_maintains_min_span() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_viewport(10, 10);
        assert_eq!(gantt.viewport(), (10, 11));
    }

    #[test]
    fn empty_tasks_returns_default_viewport() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(vec![]);
        assert!(gantt.tasks().is_empty());
        assert_eq!(gantt.selected_id(), None);
        assert_eq!(gantt.viewport(), (0, 100));
    }

    #[test]
    fn zoom_with_zero_factor_does_nothing() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 800, 600));
        gantt.set_tasks(sample_tasks());
        let before = gantt.viewport();
        gantt.zoom(0.0);
        assert_eq!(gantt.viewport(), before);
    }

    /// A milestone is an instant, so it is a diamond and not a bar.
    ///
    /// # The defect this pins
    ///
    /// The widget could only express a **range**: every `GanttTask` was a bar and
    /// [`GanttTask::new`] widened a reversed range to `start + 1`. A schedule's milestones -- the
    /// sign-off, the release, the review -- are single instants, so the only way to draw one was a
    /// one-unit-wide bar, which states that the milestone took a unit of time. The assertion is on
    /// the **pixels**: a diamond and a bar of the same centre must not produce the same document,
    /// and the milestone's ink must be *narrower* than a bar spanning the same row would be.
    #[test]
    fn a_milestone_is_a_diamond_rather_than_a_bar() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        let milestone = GanttTask::milestone("m1", "Sign-off", 10);
        assert!(milestone.milestone, "the constructor marks it");
        assert_eq!(
            milestone.start, milestone.end,
            "a milestone is an instant, so its range has no width"
        );

        // The same instant drawn as an ordinary bar, for comparison.
        let as_bar = GanttTask::new("b1", "Sign-off", 10, 10, 0);
        assert!(as_bar.end > as_bar.start, "a bar always spans something");

        let mut diamond = GanttWidget::new(Rect::new(0, 0, 400, 60));
        diamond.set_tasks(vec![milestone]);
        let diamond_svg = render_to_svg(&mut diamond);

        let mut bar = GanttWidget::new(Rect::new(0, 0, 400, 60));
        bar.set_tasks(vec![as_bar]);
        let bar_svg = render_to_svg(&mut bar);

        assert_ne!(diamond_svg, bar_svg, "a milestone must not be drawn as a bar");
        // The diamond is a `<path>` and a bar is a `<rect>`. That difference is asserted directly,
        // because `assert_ne!` alone is satisfied by *anything* differing -- including a milestone
        // that paints no shape at all. It was checked: knocking out the `DrawPath` above left this
        // test green until these two lines existed.
        assert!(
            diamond_svg.contains("<path"),
            "the milestone is drawn as a closed diamond: {diamond_svg}"
        );
        assert!(
            crate::widget::svg::first_shape_bounds(&bar_svg).is_none(),
            "an ordinary bar is a rectangle, not a path: {bar_svg}"
        );
        // And the diamond is centred on the milestone's instant rather than on the bar's left edge.
        // Both shapes are compared through the **same** projection, so the assertion is about the
        // shape's placement and not about two independently-scraped coordinates: a bar and a diamond
        // for the same instant must share a centre.
        let (diamond_left, _, diamond_right, _) =
            crate::widget::svg::first_shape_bounds(&diamond_svg).expect("the diamond's bounds");
        let diamond_centre = (diamond_left + diamond_right) / 2;
        let (bar_left, bar_right) = bar_horizontal_span(&bar_svg);
        // The diamond is centred on the instant, and the bar *starts* at it: `project_x` maps the
        // instant to one x, the diamond straddles it and the bar begins there and runs right. So the
        // two are compared as centre-vs-left-edge -- asserting centre-vs-centre would have been
        // asserting that a bar is symmetric about its start, which it is not.
        assert!(
            (diamond_centre - bar_left).abs() <= 2,
            "the diamond is centred on the instant and the bar starts at it: \
             diamond={diamond_left}..{diamond_right} centre={diamond_centre} bar starts {bar_left}"
        );
        assert!(
            bar_right > bar_left,
            "the bar spans forward from the instant: bar={bar_left}..{bar_right}"
        );
    }

    /// The horizontal span of the task **bar**: the shortest plain rectangle in the document.
    ///
    /// The control fills its whole box with a background first, so "the first plain rect" is the
    /// control, not the bar -- which is why this picks the narrowest one instead. That is a property
    /// of a gantt with one row and one short task, and the assertion above reports both spans so a
    /// future geometry change is diagnosable rather than mysterious.
    fn bar_horizontal_span(svg: &str) -> (i32, i32) {
        svg.split("<rect ")
            .filter(|chunk| !chunk.contains("rx=\"") && chunk.contains("width=\""))
            .filter_map(|chunk| {
                let x = chunk.split("x=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
                let w = chunk.split("width=\"").nth(1)?.split('"').next()?.parse::<i32>().ok()?;
                Some((x, x + w, w))
            })
            .min_by_key(|(_, _, w)| *w)
            .map(|(x, x2, _)| (x, x2))
            .unwrap_or((0, 0))
    }

    /// The "now" line appears only when the caller says where now is, and only inside the viewport.
    ///
    /// # The defect this pins
    ///
    /// There was no "now" mark at all. A gantt is read against the present -- the question it
    /// answers is where the plan stands against today -- so without this the reader has to compare
    /// the axis against the wall clock by eye. The assertion is on the painted document, and it
    /// checks all three states, because "drawn" and "not drawn" are the whole feature: off the
    /// viewport must be silent, on the viewport must lay ink, and the ink must differ from the
    /// no-marker case.
    #[test]
    fn the_now_line_is_drawn_only_where_it_belongs() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        let tasks = || vec![GanttTask::new("t1", "Design", 0, 10, 100)];

        let mut unset = GanttWidget::new(Rect::new(0, 0, 400, 60));
        unset.set_tasks(tasks());
        assert_eq!(unset.today(), None, "nothing is marked until the caller says so");
        let unset_svg = render_to_svg(&mut unset);

        let mut marked = GanttWidget::new(Rect::new(0, 0, 400, 60));
        marked.set_tasks(tasks());
        marked.set_today(Some(5));
        assert_eq!(marked.today(), Some(5), "and it reports what was set");
        let marked_svg = render_to_svg(&mut marked);
        assert_ne!(
            marked_svg, unset_svg,
            "a marked \"now\" must lay ink, or the marker is not drawn at all"
        );

        // Outside the viewport it must be silent: `project_x` extrapolates, so an unguarded line
        // would be drawn confidently at a position the axis does not span.
        let mut outside = GanttWidget::new(Rect::new(0, 0, 400, 60));
        outside.set_tasks(tasks());
        outside.set_today(Some(9_999));
        let outside_svg = render_to_svg(&mut outside);
        assert_eq!(
            outside_svg, unset_svg,
            "a \"now\" outside the viewport must paint nothing extra"
        );
    }

    /// A dependency is a link between two bars, resolved by id and drawn as an elbow.
    ///
    /// # The defect this pins
    ///
    /// `GanttTask` had four fields and none of them related two tasks, so the widget drew a set of
    /// bars with no structure between them -- a bar chart with a time axis. The assertion is that the
    /// elbowing ink is *added* by the declaration (a dependency-free schedule paints less) and that a
    /// **dangling id is skipped rather than drawn**: a caller editing a schedule transiently holds
    /// ids that name nothing, and a link to nowhere would be a line with no bar at either end.
    #[test]
    fn a_dependency_draws_an_elbow_and_a_dangling_one_is_skipped() {
        use crate::widget::svg::render_to_svg;
        let _theme_guard = crate::style::theme_test_guard();

        let linked = || {
            vec![
                GanttTask::new("a", "Design", 0, 10, 100),
                GanttTask::new("b", "Build", 14, 24, 0).depends_on(["a"]),
            ]
        };
        let unlinked = || {
            vec![GanttTask::new("a", "Design", 0, 10, 100), GanttTask::new("b", "Build", 14, 24, 0)]
        };

        let mut with = GanttWidget::new(Rect::new(0, 0, 400, 80));
        with.set_tasks(linked());
        let with_svg = render_to_svg(&mut with);

        let mut without = GanttWidget::new(Rect::new(0, 0, 400, 80));
        without.set_tasks(unlinked());
        let without_svg = render_to_svg(&mut without);
        assert_ne!(
            with_svg, without_svg,
            "declaring a dependency must change the picture, or it is stored and never read"
        );

        // A dependency naming a task that does not exist is skipped: a transient edit state must not
        // paint a line to nowhere, and it must not panic.
        let mut dangling = GanttWidget::new(Rect::new(0, 0, 400, 80));
        dangling.set_tasks(vec![GanttTask::new("b", "Build", 14, 24, 0).depends_on(["missing"])]);
        let dangling_svg = render_to_svg(&mut dangling);
        let mut bare = GanttWidget::new(Rect::new(0, 0, 400, 80));
        bare.set_tasks(vec![GanttTask::new("b", "Build", 14, 24, 0)]);
        let bare_svg = render_to_svg(&mut bare);
        assert_eq!(
            dangling_svg, bare_svg,
            "a dependency that names no task must paint nothing rather than a line to nowhere"
        );
    }

    #[test]
    fn critical_path_is_the_longest_duration_chain() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(vec![
            // 0..10 (10 units) → 10..40 (30) → 40..45 (5): total 45.
            GanttTask::new("a", "Design", 0, 10, 100),
            GanttTask::new("b", "Build", 10, 40, 0).depends_on(["a"]),
            GanttTask::new("c", "Ship", 40, 45, 0).depends_on(["b"]),
            // A short side branch: 2 units, not critical.
            GanttTask::new("x", "Side", 0, 2, 0),
        ]);

        assert_eq!(gantt.critical_path(), vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        // The property is what turns the highlight on; reading it is the switch's own state.
        gantt.set("show_critical_path", CapabilityValue::Bool(true)).expect("bool accepted");
        assert!(gantt.show_critical_path());
    }

    #[test]
    fn critical_path_names_nothing_when_every_task_is_zero_length_and_isolated() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(vec![GanttTask::milestone("m", "Start", 5)]);
        assert!(
            gantt.critical_path().is_empty(),
            "one instant is not a chain, and naming it would be an invented claim"
        );
    }

    #[test]
    fn a_dependency_cycle_has_no_critical_path() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(vec![
            GanttTask::new("a", "A", 0, 10, 0).depends_on(["b"]),
            GanttTask::new("b", "B", 10, 20, 0).depends_on(["a"]),
        ]);
        assert!(
            gantt.critical_path().is_empty(),
            "a cycle has no longest chain; reporting one would hide the caller's error"
        );
    }

    #[test]
    fn viewport_changed_fires_on_zoom_and_on_task_replacement() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());
        let ranges = Arc::new(Mutex::new(Vec::<(i64, i64)>::new()));
        let sink = ranges.clone();
        gantt.viewport_changed.connect(move |range| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*range);
            }
        });

        gantt.handle_event(&Event::wheel(0, -120, 0));
        gantt.set_tasks(sample_tasks());

        let got = ranges.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(got.len() >= 2, "both changes must be reported, got {got:?}");
        assert!(got[0].1 - got[0].0 < 30, "the zoom must shrink the range: {:?}", got[0]);
    }

    #[test]
    fn dragging_a_bars_right_edge_resizes_and_reports_start_and_finish() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());
        let started = Arc::new(Mutex::new(Vec::<String>::new()));
        let finished = Arc::new(Mutex::new(Vec::<String>::new()));
        let start_sink = started.clone();
        let finish_sink = finished.clone();
        gantt.task_resize_started.connect(move |id| {
            if let Ok(mut guard) = start_sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });
        gantt.task_resize_finished.connect(move |id| {
            if let Ok(mut guard) = finish_sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        let (track_x, track_w) = gantt.track_geometry();
        let task = &gantt.tasks()[0];
        let x1 = gantt.project_x(task.end, track_x, track_w);
        let y = gantt.lane_rect(0).y + 10;
        gantt.handle_event(&Event::mouse_press(x1 - 1, y, 1));
        gantt.handle_event(&Event::mouse_move(x1 + 20, y));
        gantt.handle_event(&Event::mouse_release(x1 + 20, y, 1));

        assert!(
            gantt.tasks()[0].end > 10,
            "the end edge must move: end is {}",
            gantt.tasks()[0].end
        );
        assert_eq!(gantt.tasks()[0].start, 0, "resizing the end pins the start");
        assert_eq!(
            started.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec!["t1".to_string()]
        );
        assert_eq!(
            finished.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec!["t1".to_string()]
        );
    }

    #[test]
    fn dragging_the_progress_strip_changes_progress_without_moving_the_bar() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());
        let (track_x, track_w) = gantt.track_geometry();
        let (start, end) = {
            let task = &gantt.tasks()[0];
            (task.start, task.end)
        };
        let x0 = gantt.project_x(start, track_x, track_w);
        let x1 = gantt.project_x(end, track_x, track_w);
        let lane_y = gantt.lane_rect(0).y;
        let bar_bottom = lane_y + gantt.row_height() as i32 - 6;
        // Halfway across the bottom strip: the bar's 100 % becomes 50 %.
        let mid = x0 + (x1 - x0) / 2;
        gantt.handle_event(&Event::mouse_press(mid, bar_bottom, 1));
        gantt.handle_event(&Event::mouse_release(mid, bar_bottom, 1));

        let task = &gantt.tasks()[0];
        assert_eq!((task.start, task.end), (start, end), "progress must not move the bar");
        assert!((task.progress as i32 - 50).abs() <= 1, "progress was {}", task.progress);
    }

    #[test]
    fn dependency_lookup_is_indexed_and_survives_task_replacement() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 700, 180));
        gantt.set_tasks(sample_tasks());
        assert_eq!(gantt.id_index.get("t3").copied(), Some(2));
        gantt.set_tasks(vec![GanttTask::new("only", "Only", 0, 5, 0)]);
        assert_eq!(gantt.id_index.get("t3"), None);
        assert_eq!(gantt.id_index.get("only").copied(), Some(0));
    }
}
