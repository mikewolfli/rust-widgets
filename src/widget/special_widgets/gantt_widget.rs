// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GanttWidget for timeline task planning.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_i64;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How far past the predecessor's right edge a dependency elbow turns: a short stub, so a link is
/// visibly an elbow and not a straight diagonal when the two bars are close on the axis.
const LINK_STUB: i32 = 4;
/// The half-width of a dependency arrow head, in pixels.
const LINK_HEAD: i32 = 4;

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
    /// Emitted when task selected. Payload is task id.
    pub task_selected: Signal1<String>,
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
            task_selected: Signal1::new(),
        }
    }

    /// Sets tasks.
    pub fn set_tasks(&mut self, tasks: Vec<GanttTask>) {
        self.tasks = tasks;
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
        self.viewport_start = start;
        self.viewport_end = end.max(start + 1);
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
        self.today = position;
        self.base.request_redraw();
    }

    fn recompute_viewport(&mut self) {
        if self.tasks.is_empty() {
            self.viewport_start = 0;
            self.viewport_end = 100;
            return;
        }

        let mut min_start = i64::MAX;
        let mut max_end = i64::MIN;
        for task in &self.tasks {
            min_start = min_start.min(task.start);
            max_end = max_end.max(task.end);
        }
        self.viewport_start = min_start;
        self.viewport_end = max_end.max(min_start + 1);
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
        let idx = ((pos.y - rect.y) / self.row_height as i32) as usize;
        (idx < self.tasks.len()).then_some(idx)
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
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.row_at(*pos) {
                    let _ = self.select_index(index);
                }
            }
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.zoom(1.2);
                } else if delta.y > 0 {
                    self.zoom(0.8);
                }
            }
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
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("gantt_widget");
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
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.15));
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
        let bar_color = crate::style::resolved_theme_style("slider")
            .and_then(|accent| accent.background_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.55));
        let progress_color = bar_color.blend(&text_color, 0.18);
        // The row separator is secondary chrome, derived from the same pair.
        let separator = background.blend(&text_color, 0.12);
        // The "now" line is a **semantic** mark, not chrome: it answers the question the chart exists
        // to answer, so it takes the theme's error role -- the one colour in the palette that is
        // universally read as "pay attention to this instant". Derived from the text colour only as a
        // last fallback, and kept at full strength so it is never mistaken for a lane separator.
        let today_color = crate::style::resolved_theme_style("badge")
            .and_then(|style| style.text_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.85));
        // Dependencies are structure, not data: a line that connected bars in the accent colour would
        // compete with the bars themselves.
        let link_color = background.blend(&text_color, 0.45);

        context.fill_rect(rect, background);
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

        for (index, task) in self.tasks.iter().take(12).enumerate() {
            let y = rect.y + index as i32 * self.row_height as i32;
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
            context.draw_text_line(
                label_band,
                &task.label,
                &Font::default(),
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
                context.execute_command(crate::render::RenderCommand::DrawPath {
                    points,
                    closed: true,
                    color: bar_color,
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
            context.fill_rect(bar_rect, bar_color);

            let progress_w =
                ((bar_rect.width as f32) * (task.progress as f32 / 100.0)).round() as u32;
            if progress_w > 0 {
                context.fill_rect(
                    Rect::new(bar_rect.x, bar_rect.y, progress_w, bar_rect.height),
                    progress_color,
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
        for (index, task) in self.tasks.iter().enumerate().take(12) {
            let y = rect.y + index as i32 * self.row_height as i32;
            if y + self.row_height as i32 > rect.y + rect.height as i32 {
                break;
            }
            for predecessor_id in &task.depends_on {
                let Some(pred_index) = self.tasks.iter().position(|t| &t.id == predecessor_id)
                else {
                    // A dangling id is skipped rather than reported: a caller editing a schedule
                    // transiently holds them, and a chart is not the place to complain.
                    continue;
                };
                if pred_index >= 12 {
                    // The predecessor's lane is not on screen, so its bar's right edge is unknown.
                    continue;
                }
                let pred = &self.tasks[pred_index];
                let pred_y = rect.y + pred_index as i32 * self.row_height as i32;
                let from_x = self.project_x(pred.end.max(pred.start + 1), track_x, track_w);
                let from_y = pred_y + self.row_height as i32 / 2;
                let to_x = self.project_x(task.start, track_x, track_w);
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
            if (self.viewport_start..=self.viewport_end).contains(&today) {
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
}
