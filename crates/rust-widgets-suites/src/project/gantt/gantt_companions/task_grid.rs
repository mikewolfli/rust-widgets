// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The task-grid table that sits beside a Gantt chart.

use rust_widgets::compat::{lock, Arc, HashMap, Mutex};
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, Signal1};
use rust_widgets::widget::capability::coercion::{expect_bool, expect_u32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::view_widgets::rtl_columns::{is_rtl, mirror_span};
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

use super::super::gantt_widget::{GanttTask, GanttWidget};
use super::common::chart_chrome_palette;
use rust_widgets::impl_draw_bridge;

/// Height of the task-grid header band; matches the gantt's time-axis header.
const GRID_HEADER_HEIGHT: u32 = 18;
/// Horizontal inset between a column edge and its text.
const GRID_PADDING: i32 = 6;

/// One column of a [`GanttTaskGrid`].
///
/// A column is declarative: it owns a caption, a preferred width and the alignment used for
/// both the header and the task labels in that band. The last column absorbs the remaining
/// width, so a caller can leave it at zero and still fill a wide pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskColumn {
    /// Caption painted in the header band.
    pub title: String,
    /// Preferred width in pixels. The final column takes whatever remains.
    pub width: u32,
    /// Alignment of the header and labels inside this column.
    pub alignment: HorizontalAlignment,
}

impl TaskColumn {
    /// A column titled `title` with a preferred `width` and `alignment`.
    pub fn new(title: impl Into<String>, width: u32, alignment: HorizontalAlignment) -> Self {
        Self { title: title.into(), width, alignment }
    }
}

/// One task row the grid paints: the fields copied out of [`GanttTask`].
#[derive(Debug, Clone, PartialEq, Eq)]
struct GridTask {
    id: String,
    label: String,
}

/// A live binding to a gantt's row height and vertical scroll.
///
/// A widget cannot hold a borrow of another, so the gantt publishes its changes through
/// [`GanttWidget::row_height_changed`] and [`GanttWidget::scroll_changed`], and this binding
/// carries the latest values into the grid's reads.
struct GridBinding {
    row_height: Arc<Mutex<u32>>,
    scroll_offset: Arc<Mutex<usize>>,
    _scope: ConnectionScope,
}

/// The left-hand task list of a gantt: a header band plus one labelled row per task.
///
/// [`GanttWidget`] paints task names *on* the bars, clipped to the plot's left edge, so a long
/// label under a deeply nested task is unreadable without a separate list. This control is that
/// list: it draws the same rows in the same order, indents labels by hierarchy depth, and
/// reports a press either as a selected task id or as a header-column press. Attach it to a
/// chart with [`Self::attach_gantt`] to follow that chart's row height and vertical scroll.
pub struct GanttTaskGrid {
    base: BaseWidget,
    columns: Vec<TaskColumn>,
    tasks: Vec<GridTask>,
    /// Child task id to parent task id, mirroring the chart's hierarchy.
    parents: HashMap<String, String>,
    row_height: u32,
    indent_step: u32,
    header_visible: bool,
    /// Deepest hierarchy level painted; [`usize::MAX`] means every row.
    max_depth: usize,
    scroll_offset: usize,
    selected: Option<usize>,
    binding: Option<GridBinding>,
    /// Emitted with the task id a press selected.
    pub task_selected: Signal1<String>,
    /// Emitted with the column index a header press hit.
    pub header_clicked: Signal1<usize>,
    /// Column bands reused by the paint path.
    column_bounds_scratch: Vec<(usize, i32, u32)>,
    /// Visible task indices reused by the paint path.
    visible_indices_scratch: Vec<usize>,
}

impl GanttTaskGrid {
    /// Creates an empty grid with one `Task` column and a visible header.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "GanttTaskGrid"),
            columns: alloc::vec![TaskColumn::new("Task", 200, HorizontalAlignment::Left)],
            tasks: Vec::new(),
            parents: HashMap::new(),
            row_height: 24,
            indent_step: 12,
            header_visible: true,
            max_depth: usize::MAX,
            scroll_offset: 0,
            selected: None,
            binding: None,
            task_selected: Signal1::new(),
            header_clicked: Signal1::new(),
            column_bounds_scratch: Vec::new(),
            visible_indices_scratch: Vec::new(),
        }
    }

    /// The columns in paint and hit-test order.
    pub fn columns(&self) -> &[TaskColumn] {
        &self.columns
    }

    /// Replaces the columns. An empty list paints no header captions and reserves no bands.
    pub fn set_columns(&mut self, columns: Vec<TaskColumn>) {
        self.columns = columns;
        self.base.request_redraw();
    }

    /// The row height in pixels, as currently painted.
    pub fn row_height(&self) -> u32 {
        self.current_row_height()
    }

    /// Sets the row height, floored at 8 pixels.
    ///
    /// While bound the chart owns the row height: `binding.row_height` is the chart's published
    /// mirror, so writing it here painted rows the chart does not have and the chart never heard
    /// about it. The write is ignored rather than desynchronising the pair (the same rule the
    /// wheel follows while bound).
    pub fn set_row_height(&mut self, row_height: u32) {
        if self.binding.is_some() {
            return;
        }
        let row_height = row_height.max(8);
        if self.current_row_height() == row_height && self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The indent applied per hierarchy level, in pixels.
    pub fn indent_step(&self) -> u32 {
        self.indent_step
    }

    /// Sets the indent applied per hierarchy level.
    pub fn set_indent_step(&mut self, indent_step: u32) {
        if self.indent_step == indent_step {
            return;
        }
        self.indent_step = indent_step;
        self.base.request_redraw();
    }

    /// Whether the header band is painted.
    pub fn header_visible(&self) -> bool {
        self.header_visible
    }

    /// Shows or hides the header band.
    pub fn set_header_visible(&mut self, header_visible: bool) {
        if self.header_visible == header_visible {
            return;
        }
        self.header_visible = header_visible;
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Replaces the task rows. Only the id and label are copied; hierarchy comes from
    /// [`Self::attach_gantt`] or from the existing parent map when both ids survive.
    pub fn set_tasks(&mut self, tasks: &[GanttTask]) {
        // The selection is an index, and replacing the rows can put a different task there. The id
        // is captured first so a surviving task keeps its highlight; a task that left is dropped.
        let selected_id =
            self.selected.and_then(|index| self.tasks.get(index)).map(|task| task.id.clone());
        self.tasks = tasks
            .iter()
            .map(|task| GridTask { id: task.id.clone(), label: task.label.clone() })
            .collect();
        let ids: Vec<&str> = self.tasks.iter().map(|task| task.id.as_str()).collect();
        self.parents.retain(|child, parent| {
            ids.contains(&child.as_str()) && ids.contains(&parent.as_str())
        });
        self.selected = selected_id.and_then(|id| self.tasks.iter().position(|task| task.id == id));
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Binds this grid to `gantt`, adopting its tasks, hierarchy, row height and scroll.
    ///
    /// The binding is live: a later row-height change or scroll on the chart updates this grid
    /// on the next paint, so the two panes stay row-aligned without the host copying state each
    /// frame. Calling this again replaces the previous subscriptions.
    ///
    /// The mirror is one way: while bound, the chart owns the row offset and this grid refuses
    /// its own wheel so a local scroll cannot leave the chart's lanes behind.
    pub fn attach_gantt(&mut self, gantt: &mut GanttWidget) {
        // Drop the previous binding before adopting the new chart's state: a stale binding would
        // keep answering `current_scroll_offset`/`current_row_height` with the old chart's values
        // while `clamp_scroll` runs, so re-attaching could import neither chart's real offset.
        self.binding = None;
        self.max_depth = usize::MAX;
        self.set_tasks(gantt.tasks());
        self.parents = gantt
            .tasks()
            .iter()
            .filter_map(|task| {
                gantt.task_parent(&task.id).map(|parent| (task.id.clone(), parent.to_string()))
            })
            .collect();
        self.row_height = gantt.row_height();
        self.scroll_offset = gantt.scroll_offset();
        self.clamp_scroll();

        let row_height = Arc::new(Mutex::new(self.row_height));
        let scroll_offset = Arc::new(Mutex::new(self.scroll_offset));
        let scope = ConnectionScope::new();
        let redraw = self.base.redraw_requested_signal().clone();

        let height_sink = Arc::clone(&row_height);
        let height_redraw = redraw.clone();
        gantt.row_height_changed.connect_ref_scoped(&scope, move |height| {
            *lock(&height_sink) = *height;
            height_redraw.emit();
        });
        let scroll_sink = Arc::clone(&scroll_offset);
        gantt.scroll_changed.connect_ref_scoped(&scope, move |offset| {
            *lock(&scroll_sink) = *offset;
            redraw.emit();
        });

        self.binding = Some(GridBinding { row_height, scroll_offset, _scope: scope });
        self.base.request_redraw();
    }

    /// Scrolls so the task named `id` is on screen, expanding its depth first if needed.
    ///
    /// Returns `false` when no task carries that id.
    pub fn scroll_to_task(&mut self, id: &str) -> bool {
        let Some(index) = self.tasks.iter().position(|task| task.id == id) else {
            return false;
        };
        let depth = self.depth_of(index);
        // Through the public entry point: it clamps the scroll and requests the frame the reveal
        // needs. The direct write changed what paints without asking for a repaint, so when the
        // scroll offset happened to be unchanged the row stayed invisible.
        self.expand_to_depth(depth);
        let indices = self.visible_indices();
        let Some(position) = indices.iter().position(|candidate| *candidate == index) else {
            return false;
        };
        let rows = self.visible_row_count().max(1);
        let current = self.current_scroll_offset();
        let next = if position < current {
            position
        } else if position >= current + rows {
            position + 1 - rows
        } else {
            current
        };
        self.set_scroll_offset(next);
        true
    }

    /// Shows rows down to `depth` (inclusive) and collapses everything deeper.
    ///
    /// `0` leaves only roots visible; [`usize::MAX`] shows the whole hierarchy, which is the
    /// default and the state [`Self::attach_gantt`] adopts.
    pub fn expand_to_depth(&mut self, depth: usize) {
        if self.max_depth == depth {
            return;
        }
        self.max_depth = depth;
        self.clamp_scroll();
        self.base.request_redraw();
    }

    /// The task index a press selected, when one is selected.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    /// The task index under `point`, using the geometry [`Draw`] paints.
    pub fn hit_task(&self, point: Point) -> Option<usize> {
        let rect = self.geometry();
        if !rect.contains_point(point) {
            return None;
        }
        let local_y = point.y - rect.y - self.header_height();
        if local_y < 0 {
            return None;
        }
        let row_height = self.current_row_height().max(1) as i32;
        let visible = (local_y / row_height) as usize;
        // Paint only walks `visible_row_count` rows; a partially fitting row below that window is
        // never drawn, so it must not answer a hit either.
        if visible >= self.visible_row_count() {
            return None;
        }
        self.row_rect(visible)?;
        let indices = self.visible_indices();
        indices.get(self.current_scroll_offset() + visible).copied()
    }

    /// The header column under `point`, or `None` outside the header band.
    pub fn hit_header(&self, point: Point) -> Option<usize> {
        if !self.header_visible {
            return None;
        }
        let rect = self.geometry();
        let bottom = rect.y + GRID_HEADER_HEIGHT.min(rect.height) as i32;
        if point.y < rect.y || point.y >= bottom {
            return None;
        }
        self.column_at(point.x)
    }

    /// The first on-screen row, in rows scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.current_scroll_offset()
    }

    /// Scrolls the list by `delta` rows, clamped to the visible task range.
    pub fn scroll_by(&mut self, delta: isize) {
        let max = self.max_scroll();
        let current = self.current_scroll_offset();
        let next = (current as isize + delta).clamp(0, max as isize) as usize;
        self.set_scroll_offset(next);
    }

    /// The row height currently painted, following an attached gantt when bound.
    fn current_row_height(&self) -> u32 {
        self.binding
            .as_ref()
            .map(|binding| *lock(&binding.row_height))
            .unwrap_or(self.row_height)
            .max(8)
    }

    /// The scroll offset currently painted, following an attached gantt when bound.
    fn current_scroll_offset(&self) -> usize {
        let offset = self
            .binding
            .as_ref()
            .map(|binding| *lock(&binding.scroll_offset))
            .unwrap_or(self.scroll_offset);
        offset.min(self.max_scroll())
    }

    /// The header height in pixels.
    fn header_height(&self) -> i32 {
        if self.header_visible {
            GRID_HEADER_HEIGHT as i32
        } else {
            0
        }
    }

    /// How many task rows fit below the header band.
    fn visible_row_count(&self) -> usize {
        let rect = self.geometry();
        let content = rect.height.saturating_sub(self.header_height().max(0) as u32);
        (content / self.current_row_height()) as usize
    }

    /// The largest valid scroll offset for the currently visible rows.
    fn max_scroll(&self) -> usize {
        self.visible_index_count().saturating_sub(self.visible_row_count())
    }

    /// How many rows pass the depth filter, without materialising the index list.
    ///
    /// The paint path asks for the scroll clamp every frame, so counting in place avoids the
    /// `Vec` allocation `visible_indices()` would make for a value that is only a length.
    fn visible_index_count(&self) -> usize {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(index, _)| self.depth_of(*index) <= self.max_depth)
            .count()
    }

    /// The task indices that pass the depth filter, in display order.
    fn visible_indices(&self) -> Vec<usize> {
        let mut out = Vec::new();
        self.fill_visible_indices(&mut out);
        out
    }

    /// [`Self::visible_indices`] into a caller-held buffer, for the paint path.
    fn fill_visible_indices(&self, out: &mut Vec<usize>) {
        out.clear();
        out.extend(
            self.tasks
                .iter()
                .enumerate()
                .filter(|(index, _)| self.depth_of(*index) <= self.max_depth)
                .map(|(index, _)| index),
        );
    }

    /// How many ancestors task `index` has.
    fn depth_of(&self, index: usize) -> usize {
        let Some(task) = self.tasks.get(index) else {
            return 0;
        };
        let mut depth = 0usize;
        let mut cursor = self.parents.get(&task.id);
        while let Some(parent) = cursor {
            depth += 1;
            if depth > self.tasks.len() {
                break;
            }
            cursor = self.parents.get(parent);
        }
        depth
    }

    /// The geometry of one on-screen row, shared by painting and hit-testing.
    fn row_rect(&self, visible: usize) -> Option<Rect> {
        let rect = self.geometry();
        let row_height = self.current_row_height();
        let top = rect.y + self.header_height() + visible as i32 * row_height as i32;
        let bottom = rect.y + rect.height as i32;
        if top >= bottom {
            return None;
        }
        let height = (row_height as i32).min(bottom - top) as u32;
        Some(Rect::new(rect.x, top, rect.width, height))
    }

    /// The x origin of a task label, the one place indentation is applied.
    ///
    /// Test-only: the paint path derives the same origin from the column band it already has,
    /// so a frame does not allocate for every row.
    #[cfg(test)]
    fn label_x(&self, index: usize) -> Option<i32> {
        let bounds = self.column_bounds();
        let &(_, x, _) = bounds.first()?;
        Some(x + GRID_PADDING + self.depth_of(index) as i32 * self.indent_step as i32)
    }

    /// Column bands in paint order, with the final column absorbing the remainder.
    fn column_bounds(&self) -> Vec<(usize, i32, u32)> {
        let mut out = Vec::new();
        self.fill_column_bounds(&mut out);
        out
    }

    /// [`Self::column_bounds`] into a caller-held buffer, for the paint path.
    ///
    /// In a right-to-left page every band is reflected inside the control's rectangle, so the
    /// first column — the one a right-to-left reader starts at — is the rightmost. Widths are
    /// unchanged and the bands still tile the rectangle, so the header, the row labels, the
    /// separators and [`Self::column_at`] all read the same partition: the hit test moves with
    /// the paint because neither derives its own geometry.
    fn fill_column_bounds(&self, out: &mut Vec<(usize, i32, u32)>) {
        out.clear();
        let rect = self.geometry();
        let right = rect.x + rect.width as i32;
        let mut x = rect.x;
        for (index, column) in self.columns.iter().enumerate() {
            if x >= right {
                break;
            }
            let remaining = right - x;
            let width = if index + 1 == self.columns.len() {
                remaining as u32
            } else {
                (column.width.max(1) as i32).min(remaining) as u32
            };
            out.push((index, x, width));
            x += width as i32;
        }
        if is_rtl() {
            for (_, start, width) in out.iter_mut() {
                *start = mirror_span(*start, *width, rect);
            }
        }
    }

    /// The column containing `x`, using the same bands [`Self::column_bounds`] returns.
    fn column_at(&self, x: i32) -> Option<usize> {
        self.column_bounds()
            .into_iter()
            .find(|(_, left, width)| x >= *left && x < *left + *width as i32)
            .map(|(index, _, _)| index)
    }

    /// Scrolls to an absolute row offset, clamped and reported through the binding.
    fn set_scroll_offset(&mut self, offset: usize) {
        let next = offset.min(self.max_scroll());
        if self.current_scroll_offset() == next && self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        if let Some(binding) = &self.binding {
            *lock(&binding.scroll_offset) = next;
        }
        self.base.request_redraw();
    }

    /// Re-clamps the stored offsets after the row set or geometry changed.
    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        let next = self.current_scroll_offset().min(max);
        self.scroll_offset = next;
        if let Some(binding) = &self.binding {
            *lock(&binding.scroll_offset) = next;
        }
    }
}

impl Widget for GanttTaskGrid {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("gantt_task_grid")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::widget::metrics::ControlMetrics::implicit_size(
            rust_widgets::core::Size::new(0, 0),
            rust_widgets::style::EdgeOffsets::all(0),
            rust_widgets::core::Size::new(220, 200),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "task_selected" => {
                Some(EventSignalRef::mapped("task_selected", &self.task_selected, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            // Legacy spelling (DX-10): the name published before the `on_` prefix was removed.
            "on_task_selected" => {
                Some(EventSignalRef::mapped("on_task_selected", &self.task_selected, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            "header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.header_clicked, |column| {
                    CapabilityValue::UInt(*column as u64)
                }))
            }
            // Legacy spelling (DX-10).
            "on_header_clicked" => {
                Some(EventSignalRef::mapped("on_header_clicked", &self.header_clicked, |column| {
                    CapabilityValue::UInt(*column as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for GanttTaskGrid {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "row_height" => Ok(CapabilityValue::UInt(u64::from(self.row_height()))),
            "indent_step" => Ok(CapabilityValue::UInt(u64::from(self.indent_step))),
            "header_visible" => Ok(CapabilityValue::Bool(self.header_visible)),
            "columns" => Ok(CapabilityValue::UInt(self.columns.len() as u64)),
            "tasks" => Ok(CapabilityValue::UInt(self.tasks.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "indent_step" => {
                self.set_indent_step(expect_u32(value)?);
                Ok(())
            }
            "header_visible" => {
                self.set_header_visible(expect_bool(value)?);
                Ok(())
            }
            "columns" | "tasks" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "row_height",
            "indent_step",
            "header_visible",
            "columns",
            "tasks",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for GanttTaskGrid {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(column) = self.hit_header(*pos) {
                    self.header_clicked.emit(column);
                    return;
                }
                if let Some(index) = self.hit_task(*pos) {
                    self.selected = Some(index);
                    if let Some(task) = self.tasks.get(index) {
                        self.task_selected.emit(task.id.clone());
                    }
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, .. } if delta.y != 0 => {
                // The chart owns the offset while bound: the binding only mirrors chart → grid, so
                // scrolling the replica here moved the rows and left the chart's lanes where they
                // were, silently breaking the row alignment the binding exists to keep.
                if self.binding.is_some() {
                    return;
                }
                self.scroll_by(if delta.y > 0 { -1 } else { 1 });
            }
            _ => {}
        }
    }
}

impl Draw for GanttTaskGrid {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let (surface, border, ink) = chart_chrome_palette(style, "gantt_task_grid");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let font = effective_font(style);
        // Read once for the frame; `fill_column_bounds` is the single place that reflects the
        // column bands, so `hit_header`/`column_at` agree with the paint without a second
        // derivation.
        let rtl = is_rtl();
        let mut bounds = core::mem::take(&mut self.column_bounds_scratch);
        self.fill_column_bounds(&mut bounds);

        if self.header_visible {
            let header = Rect::new(rect.x, rect.y, rect.width, GRID_HEADER_HEIGHT.min(rect.height));
            context.fill_rect(header, Color::rgba(0, 0, 0, 12));
            for (index, x, width) in &bounds {
                let Some(column) = self.columns.get(*index) else {
                    continue;
                };
                let band = Rect::new(
                    *x + GRID_PADDING,
                    header.y,
                    width.saturating_sub((2 * GRID_PADDING) as u32),
                    header.height,
                );
                context.draw_text_line_ellipsis(band, &column.title, font, ink, column.alignment);
            }
            context.draw_line(
                Point::new(rect.x, header.bottom()),
                Point::new(rect.x + rect.width as i32, header.bottom()),
                border,
            );
        }

        // Column separators span the whole control, so the header and the rows share one
        // partition; the outer column has no separator on the pane's own edge. That column is
        // the first entry in LTR and the last one once the bands are reflected.
        for (position, (_, x, _)) in bounds.iter().enumerate() {
            if (rtl && position + 1 == bounds.len()) || (!rtl && position == 0) {
                continue;
            }
            context.draw_line(
                Point::new(*x, rect.y),
                Point::new(*x, rect.y + rect.height as i32),
                border,
            );
        }

        let mut indices = core::mem::take(&mut self.visible_indices_scratch);
        self.fill_visible_indices(&mut indices);
        let scroll = self.current_scroll_offset();
        let first_column = bounds.first().copied();
        let indent_step = self.indent_step as i32;
        for visible in 0..self.visible_row_count() {
            let Some(&task_index) = indices.get(scroll + visible) else {
                break;
            };
            let Some(row) = self.row_rect(visible) else {
                break;
            };
            let Some(task) = self.tasks.get(task_index) else {
                continue;
            };
            if self.selected == Some(task_index) {
                context.fill_rect(row, Color::rgba(0, 0, 0, 22));
            }
            context.draw_line(
                Point::new(row.x, row.bottom()),
                Point::new(row.x + row.width as i32, row.bottom()),
                border,
            );
            let Some((first_index, first_x, first_width)) = first_column else {
                continue;
            };
            let Some(column) = self.columns.get(first_index) else {
                continue;
            };
            let indent = self.depth_of(task_index) as i32 * indent_step;
            let label_x = first_x + GRID_PADDING + indent;
            let available =
                (first_width as i32 - GRID_PADDING - indent - GRID_PADDING).max(0) as u32;
            // The label band is reflected inside the column band that owns it: the indent then
            // comes off the column's leading (right) edge instead of its left one.
            let label_x = if rtl {
                mirror_span(label_x, available, Rect::new(first_x, row.y, first_width.max(1), 0))
            } else {
                label_x
            };
            let band = Rect::new(label_x, row.y, available, row.height);
            context.draw_text_line_ellipsis(band, &task.label, font, ink, column.alignment);
        }
        self.column_bounds_scratch = bounds;
        self.visible_indices_scratch = indices;
        self.base.paint_hover_wash(context, rect);
        crate::veil_if_disabled(&self.base, context);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::gantt_widget::DependencyKind;
    use super::super::{Dependency, DependencyEditor, DEPENDENCY_SCHEMA};
    use super::*;

    /// The edge list has a records read/write face, while `dependencies` stays the count it was
    /// (SP-123).
    #[test]
    fn the_dependency_edges_property_round_trips_records() {
        let mut editor = DependencyEditor::new(Rect::new(0, 0, 260, 160));
        editor.set_dependencies(vec![Dependency::with_kind(
            "a",
            "b",
            DependencyKind::StartToStart,
        )]);

        let records = match editor.get("dependency_edges").expect("the list is readable") {
            CapabilityValue::Records(records) => records,
            other => panic!("dependency_edges must read as Records, got {other:?}"),
        };
        assert_eq!(records.len(), 1);
        let row = &records.items()[0];
        assert_eq!(row.get("predecessor"), Some(&CapabilityValue::String(String::from("a"))));
        assert_eq!(row.get("successor"), Some(&CapabilityValue::String(String::from("b"))));
        assert_eq!(row.get("kind"), Some(&CapabilityValue::String(String::from("sts"))));

        // A whole-list write goes through `set_dependencies`, so duplicates and self-edges keep
        // their existing rule.
        let list = rust_widgets::widget::capability::RecordList::from_items(
            &DEPENDENCY_SCHEMA,
            vec![
                rust_widgets::widget::capability::RecordValue::new()
                    .with("predecessor", CapabilityValue::String(String::from("x")))
                    .with("successor", CapabilityValue::String(String::from("y")))
                    .with("kind", CapabilityValue::String(String::from("ftf"))),
                rust_widgets::widget::capability::RecordValue::new()
                    .with("predecessor", CapabilityValue::String(String::from("x")))
                    .with("successor", CapabilityValue::String(String::from("y"))),
            ],
        );
        editor
            .set("dependency_edges", CapabilityValue::Records(list))
            .expect("the records are writable");
        assert_eq!(editor.dependencies().len(), 1, "the duplicate is refused as before");
        assert_eq!(editor.dependencies()[0].kind, DependencyKind::FinishToFinish);
        assert_eq!(
            editor.get("dependencies").expect("the count still reads"),
            CapabilityValue::UInt(1)
        );

        // An unknown kind token is a constraint violation, not a silent default.
        let bad = rust_widgets::widget::capability::RecordList::from_items(
            &DEPENDENCY_SCHEMA,
            vec![rust_widgets::widget::capability::RecordValue::new()
                .with("predecessor", CapabilityValue::String(String::from("p")))
                .with("successor", CapabilityValue::String(String::from("q")))
                .with("kind", CapabilityValue::String(String::from("wat")))],
        );
        assert!(editor.set("dependency_edges", CapabilityValue::Records(bad)).is_err());
        assert_eq!(editor.dependencies().len(), 1, "a refused write keeps the live list");
    }

    /// A bound grid refuses its own wheel: the chart owns the row offset.
    ///
    /// The binding mirrors chart → grid only, so scrolling the replica moved the task rows while
    /// the chart's lanes stayed put — the two panes stopped being row-aligned.
    #[test]
    fn a_bound_grid_does_not_scroll_itself() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 400, 160));
        for index in 0..10 {
            gantt.add_task(GanttTask::new(format!("t{index}"), "Task", 0, 1, index));
        }
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 120));
        grid.attach_gantt(&mut gantt);
        let before = grid.scroll_offset();
        let chart_before = gantt.scroll_offset();

        grid.handle_event(&Event::wheel(0, 1, 0));

        assert_eq!(grid.scroll_offset(), before, "the grid mirror did not move");
        assert_eq!(gantt.scroll_offset(), chart_before, "the chart is the owner");
    }

    /// A press on a task row selects it and reports its id.
    #[test]
    fn a_press_selects_the_task_under_the_pointer() {
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 160));
        grid.set_columns(alloc::vec![
            TaskColumn::new("Task", 120, HorizontalAlignment::Left),
            TaskColumn::new("Owner", 80, HorizontalAlignment::Left),
        ]);
        grid.set_row_height(20);
        grid.set_tasks(&[
            GanttTask::new("a", "Alpha", 0, 1, 0),
            GanttTask::new("b", "Beta", 0, 1, 0),
        ]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        grid.task_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        // The header occupies the first 18 px, so row 1 starts at 18 + 20.
        grid.handle_event(&Event::MousePress {
            pos: Point::new(10, 18 + 20 + 5),
            button: 1,
            modifiers: 0,
        });

        assert_eq!(grid.selected_index(), Some(1));
        let seen = seen.lock().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(seen, alloc::vec![String::from("b")]);
    }

    /// A press in the header reports the column index under the pointer.
    #[test]
    fn a_header_press_reports_the_column_index() {
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 160));
        grid.set_columns(alloc::vec![
            TaskColumn::new("Task", 80, HorizontalAlignment::Left),
            TaskColumn::new("Owner", 120, HorizontalAlignment::Left),
        ]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        grid.header_clicked.connect(move |column| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*column);
            }
        });

        grid.handle_event(&Event::MousePress { pos: Point::new(100, 5), button: 1, modifiers: 0 });

        assert_eq!(seen.lock().map(|guard| guard.clone()).unwrap_or_default(), alloc::vec![1usize]);
    }

    /// Attaching adopts the chart's hierarchy, and each level moves the label one indent step.
    #[test]
    fn an_attached_hierarchy_indents_labels_by_depth() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 400, 200));
        gantt.set_tasks(alloc::vec![
            GanttTask::new("root", "Root", 0, 1, 0),
            GanttTask::new("child", "Child", 0, 1, 0),
            GanttTask::new("grand", "Grand", 0, 1, 0),
        ]);
        assert!(gantt.set_task_parent("child", "root"));
        assert!(gantt.set_task_parent("grand", "child"));
        gantt.set_row_height(22);

        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 200));
        grid.set_indent_step(10);
        grid.attach_gantt(&mut gantt);

        assert_eq!(grid.row_height(), 22, "the chart's row height is adopted");
        assert_eq!(grid.depth_of(0), 0);
        assert_eq!(grid.depth_of(1), 1);
        assert_eq!(grid.depth_of(2), 2);
        let root = grid.label_x(0).expect("the default column exists");
        let child = grid.label_x(1).expect("the default column exists");
        let grand = grid.label_x(2).expect("the default column exists");
        assert_eq!(child - root, 10);
        assert_eq!(grand - child, 10);

        grid.expand_to_depth(1);
        assert_eq!(grid.visible_indices().len(), 2, "the grandchild is collapsed");
    }

    /// An attached grid follows the chart's vertical scroll.
    #[test]
    fn an_attached_grid_follows_the_chart_scroll() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 400, 60));
        gantt.set_tasks(alloc::vec![
            GanttTask::new("t0", "Zero", 0, 1, 0),
            GanttTask::new("t1", "One", 0, 1, 0),
            GanttTask::new("t2", "Two", 0, 1, 0),
            GanttTask::new("t3", "Three", 0, 1, 0),
            GanttTask::new("t4", "Four", 0, 1, 0),
            GanttTask::new("t5", "Five", 0, 1, 0),
        ]);
        gantt.set_row_height(20);

        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 60));
        grid.attach_gantt(&mut gantt);
        gantt.scroll_by(1);

        assert_eq!(grid.scroll_offset(), 1, "the chart's scroll reaches the list");
        assert!(grid.scroll_to_task("t5"));
        assert!(grid.scroll_offset() > 1, "the grid can jump to a task on its own");
    }

    /// Paint walks whole rows only, so a partially visible row below the window is not a target.
    #[test]
    fn hit_task_ignores_a_row_the_paint_loop_does_not_draw() {
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 60));
        grid.set_row_height(20);
        grid.set_tasks(&[
            GanttTask::new("a", "Alpha", 0, 1, 0),
            GanttTask::new("b", "Beta", 0, 1, 0),
            GanttTask::new("c", "Gamma", 0, 1, 0),
        ]);
        // 18 px header + two 20 px rows leave 2 px for a third row that is never painted.
        assert_eq!(grid.visible_row_count(), 2);
        assert_eq!(grid.hit_task(Point::new(10, 40)), Some(1), "a painted row still hits");
        assert_eq!(
            grid.hit_task(Point::new(10, 59)),
            None,
            "the unpainted partial row must not select its task"
        );
    }

    /// Replacing the rows keeps the highlight on the same task, not on the same slot.
    #[test]
    fn set_tasks_keeps_the_selection_on_the_same_task() {
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 160));
        grid.set_row_height(20);
        grid.set_tasks(&[
            GanttTask::new("a", "Alpha", 0, 1, 0),
            GanttTask::new("b", "Beta", 0, 1, 0),
        ]);
        grid.handle_event(&Event::MousePress {
            pos: Point::new(10, 18 + 20 + 5),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(grid.selected_index(), Some(1));

        grid.set_tasks(&[
            GanttTask::new("b", "Beta", 0, 1, 0),
            GanttTask::new("a", "Alpha", 0, 1, 0),
        ]);
        let selected = grid.selected_index().expect("b is still on the board");
        assert_eq!(grid.tasks[selected].id, "b", "the highlight follows the task, not the index");

        grid.set_tasks(&[GanttTask::new("a", "Alpha", 0, 1, 0)]);
        assert_eq!(grid.selected_index(), None, "a removed task clears the selection");
    }

    /// Re-attaching adopts the new chart's scroll instead of keeping the old binding's value.
    #[test]
    fn reattaching_adopts_the_new_charts_scroll() {
        let mut first = GanttWidget::new(Rect::new(0, 0, 200, 60));
        first.set_tasks(
            (0..6)
                .map(|index| GanttTask::new(format!("f{index}"), "First", 0, 1, 0))
                .collect::<Vec<_>>(),
        );
        first.set_row_height(20);
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 60));
        grid.attach_gantt(&mut first);
        first.scroll_by(2);
        assert_eq!(grid.scroll_offset(), 2);

        let mut second = GanttWidget::new(Rect::new(0, 0, 200, 60));
        second.set_tasks(
            (0..6)
                .map(|index| GanttTask::new(format!("s{index}"), "Second", 0, 1, 0))
                .collect::<Vec<_>>(),
        );
        second.set_row_height(20);
        grid.attach_gantt(&mut second);
        assert_eq!(grid.scroll_offset(), 0, "the new chart starts at its own offset");
    }

    /// The allocation-free visible count the scroll clamp uses equals the materialised list.
    #[test]
    fn max_scroll_counts_only_rows_that_pass_the_depth_filter() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 200, 60));
        gantt.set_tasks(
            (0..6)
                .map(|index| GanttTask::new(format!("t{index}"), "Row", 0, 1, 0))
                .collect::<Vec<_>>(),
        );
        for index in 1..6 {
            assert!(gantt.set_task_parent(&format!("t{index}"), "t0"));
        }
        gantt.set_row_height(20);

        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 60));
        grid.attach_gantt(&mut gantt);
        grid.expand_to_depth(0);
        assert_eq!(grid.max_scroll(), 0, "only the root is visible");
        grid.expand_to_depth(1);
        assert_eq!(
            grid.max_scroll(),
            grid.visible_indices().len().saturating_sub(grid.visible_row_count())
        );
        assert_eq!(grid.max_scroll(), 4);
    }

    /// A bound grid refuses a row-height write: the chart owns the height it mirrors.
    ///
    /// Writing `binding.row_height` made the grid paint rows the chart did not have (the chart's
    /// `row_height_changed` is the only thing that may move that cell).
    #[test]
    fn a_bound_grid_ignores_a_row_height_write() {
        let mut gantt = GanttWidget::new(Rect::new(0, 0, 400, 200));
        gantt.set_tasks(alloc::vec![GanttTask::new("t", "Task", 0, 1, 0)]);
        gantt.set_row_height(24);
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 200));
        grid.attach_gantt(&mut gantt);
        assert_eq!(grid.row_height(), 24, "the chart's height is adopted");

        grid.set_row_height(40);
        assert_eq!(grid.row_height(), 24, "the chart still owns the bound height");
    }

    /// An RTL page mirrors the column run, and the header hit test moves with it (W5-15).
    ///
    /// The paint, `column_at` and `hit_header` all read `column_bounds`, so the reflection lives
    /// in `fill_column_bounds` and the clicked header is the painted one without a second
    /// derivation.
    #[test]
    fn rtl_mirrors_the_column_run_and_the_header_hit_test() {
        let mut grid = GanttTaskGrid::new(Rect::new(0, 0, 200, 160));
        grid.set_columns(alloc::vec![
            TaskColumn::new("Task", 80, HorizontalAlignment::Left),
            TaskColumn::new("Owner", 120, HorizontalAlignment::Left),
        ]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        grid.header_clicked.connect(move |column| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*column);
            }
        });

        assert_eq!(grid.column_bounds(), alloc::vec![(0usize, 0, 80u32), (1usize, 80, 120u32)]);

        let previous = rust_widgets::core::default_text_direction();
        rust_widgets::core::set_default_text_direction(
            rust_widgets::core::TextDirection::RightToLeft,
        );

        // The bands are reflected and keep their widths: column 0 starts at the right edge and
        // the last column ends at the left one.
        assert_eq!(grid.column_bounds(), alloc::vec![(0usize, 120, 80u32), (1usize, 0, 120u32)]);
        // A press where column 0 now is reports column 0, not the last column.
        grid.handle_event(&Event::MousePress { pos: Point::new(180, 5), button: 1, modifiers: 0 });
        assert_eq!(seen.lock().map(|guard| guard.clone()).unwrap_or_default(), alloc::vec![0usize]);

        rust_widgets::core::set_default_text_direction(previous);
    }
}
