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
    pub on_task_selected: Signal1<String>,
    /// Emitted with the column index a header press hit.
    pub on_header_clicked: Signal1<usize>,
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
            on_task_selected: Signal1::new(),
            on_header_clicked: Signal1::new(),
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
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(8);
        if self.current_row_height() == row_height && self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        if let Some(binding) = &self.binding {
            *lock(&binding.row_height) = row_height;
        }
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
        self.tasks = tasks
            .iter()
            .map(|task| GridTask { id: task.id.clone(), label: task.label.clone() })
            .collect();
        let ids: Vec<&str> = self.tasks.iter().map(|task| task.id.as_str()).collect();
        self.parents.retain(|child, parent| {
            ids.contains(&child.as_str()) && ids.contains(&parent.as_str())
        });
        if self.selected.is_some_and(|index| index >= self.tasks.len()) {
            self.selected = None;
        }
        self.clamp_scroll();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Binds this grid to `gantt`, adopting its tasks, hierarchy, row height and scroll.
    ///
    /// The binding is live: a later row-height change or scroll on the chart updates this grid
    /// on the next paint, so the two panes stay row-aligned without the host copying state each
    /// frame. Calling this again replaces the previous subscriptions.
    pub fn attach_gantt(&mut self, gantt: &mut GanttWidget) {
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
        if depth > self.max_depth {
            self.max_depth = depth;
        }
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
        let visible = (local_y / self.current_row_height() as i32) as usize;
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
        self.visible_indices().len().saturating_sub(self.visible_row_count())
    }

    /// The task indices that pass the depth filter, in display order.
    fn visible_indices(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(index, _)| self.depth_of(*index) <= self.max_depth)
            .map(|(index, _)| index)
            .collect()
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
        let rect = self.geometry();
        let right = rect.x + rect.width as i32;
        let mut x = rect.x;
        let mut bounds = Vec::with_capacity(self.columns.len());
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
            bounds.push((index, x, width));
            x += width as i32;
        }
        bounds
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
            "task_selected" | "on_task_selected" => {
                Some(EventSignalRef::mapped("task_selected", &self.on_task_selected, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            "header_clicked" | "on_header_clicked" => {
                Some(EventSignalRef::mapped("header_clicked", &self.on_header_clicked, |column| {
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
                    self.on_header_clicked.emit(column);
                    return;
                }
                if let Some(index) = self.hit_task(*pos) {
                    self.selected = Some(index);
                    if let Some(task) = self.tasks.get(index) {
                        self.on_task_selected.emit(task.id.clone());
                    }
                    self.base.request_redraw();
                }
            }
            Event::Wheel { delta, .. } if delta.y != 0 => {
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
        let bounds = self.column_bounds();

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
                context.draw_text_line_cow(band, &column.title, font, ink, column.alignment);
            }
            context.draw_line(
                Point::new(rect.x, header.bottom()),
                Point::new(rect.x + rect.width as i32, header.bottom()),
                border,
            );
        }

        // Column separators span the whole control, so the header and the rows share one
        // partition; the last column has no trailing separator.
        for (_, x, _) in bounds.iter().skip(1) {
            context.draw_line(
                Point::new(*x, rect.y),
                Point::new(*x, rect.y + rect.height as i32),
                border,
            );
        }

        let indices = self.visible_indices();
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
            let band = Rect::new(label_x, row.y, available, row.height);
            context.draw_text_line_cow(band, &task.label, font, ink, column.alignment);
        }
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
        grid.on_task_selected.connect(move |id| {
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
        grid.on_header_clicked.connect(move |column| {
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
}
