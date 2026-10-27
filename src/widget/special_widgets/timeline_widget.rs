// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TimelineWidget for basic time-range visualization.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use core::cell::{Ref, RefCell};

/// One timeline entry with inclusive start/end time units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Start point.
    pub start: i64,
    /// End point.
    pub end: i64,
}

/// The declared shape of one timeline entry, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `items` property, and the control owns both ends of it: the field
/// names are [`TimelineItem`]'s own fields. Every field is required because every one of them is
/// what the bar is laid out and drawn from.
pub const TIMELINE_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "start",
                crate::widget::capability::PropertyValueKind::Int,
            ),
            crate::widget::capability::RecordField::required(
                "end",
                crate::widget::capability::PropertyValueKind::Int,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// One row of [`TIMELINE_ITEM_SCHEMA`].
fn timeline_item_record(item: &TimelineItem) -> crate::widget::capability::RecordValue {
    crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(item.id.clone()))
        .with("label", CapabilityValue::String(item.label.clone()))
        .with("start", CapabilityValue::Int(item.start))
        .with("end", CapabilityValue::Int(item.end))
}

/// Rebuilds one [`TimelineItem`] from a row of [`TIMELINE_ITEM_SCHEMA`].
fn timeline_item_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<TimelineItem, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let number = |name: &str| match record.get(name) {
        Some(CapabilityValue::Int(value)) => Ok(*value),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(TimelineItem {
        id: text("id")?,
        label: text("label")?,
        start: number("start")?,
        end: number("end")?,
    })
}

/// How items are assigned to rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineLaneMode {
    /// One row per item, in list order.
    #[default]
    Row,
    /// Overlapping items share a row: each item takes the first lane whose previous item has
    /// already ended, so a dense timeline stays as short as its overlap requires.
    LanePacked,
}

impl TimelineLaneMode {
    /// The token this mode is published under.
    pub fn token(self) -> &'static str {
        match self {
            Self::Row => "row",
            Self::LanePacked => "lane_packed",
        }
    }

    /// Parses a published token; `None` for an unknown spelling.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "row" => Some(Self::Row),
            "lane_packed" => Some(Self::LanePacked),
            _ => None,
        }
    }
}

/// Height of the time-label header, in pixels.
const TIME_LABEL_HEIGHT: u32 = 16;

/// How close to a bar's edge a press has to land to resize it rather than move it.
const TIMELINE_EDGE_GRAB: i32 = 4;

/// How far the pointer must travel before a press on empty track becomes a pan.
const TIMELINE_PAN_THRESHOLD: i32 = 3;

/// A pointer gesture in progress on the time track.
///
/// The control owns no item model, so a drag only **reports** the new span through
/// [`TimelineWidget::item_moved`] / [`TimelineWidget::item_resized`]; whether the host writes it
/// back is the host's decision.
enum TimelineDrag {
    /// Dragging the empty track pans the visible range.
    Pan {
        /// Pointer x at the press.
        origin_x: i32,
        /// Viewport start at the press.
        start: i64,
        /// Viewport end at the press.
        end: i64,
        /// Whether the movement threshold has been passed.
        moved: bool,
    },
    /// Dragging a bar's body moves it in time.
    Move {
        /// The dragged item.
        index: usize,
        /// Pointer x at the press.
        origin_x: i32,
        /// Item start at the press.
        start: i64,
        /// Item end at the press.
        end: i64,
    },
    /// Dragging a bar's leading edge changes its start.
    ResizeStart {
        /// The dragged item.
        index: usize,
        /// Pointer x at the press.
        origin_x: i32,
        /// Item start at the press.
        start: i64,
        /// Item end at the press.
        end: i64,
    },
    /// Dragging a bar's trailing edge changes its end.
    ResizeEnd {
        /// The dragged item.
        index: usize,
        /// Pointer x at the press.
        origin_x: i32,
        /// Item start at the press.
        start: i64,
        /// Item end at the press.
        end: i64,
    },
}

/// How many intervals the time-label header is divided into.
const TIME_LABEL_TICKS: u32 = 4;

impl TimelineItem {
    /// Creates a timeline item.
    pub fn new(id: impl Into<String>, label: impl Into<String>, start: i64, end: i64) -> Self {
        let normalized_end = end.max(start);
        Self { id: id.into(), label: label.into(), start, end: normalized_end }
    }
}

/// Basic vertical timeline widget with selectable rows and zoomable viewport.
pub struct TimelineWidget {
    base: BaseWidget,
    items: Vec<TimelineItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    viewport_start: i64,
    viewport_end: i64,
    row_height: u32,
    /// Rows scrolled out of view at the top.
    scroll_offset: usize,
    /// The "now" instant drawn as a marker, if the host supplied one.
    now: Option<i64>,
    /// Whether bars grow in on the first paint.
    animate_entry: bool,
    /// The entry animation's progress.
    reveal: PropertyDriver,
    /// Whether the time-label header is drawn above the rows.
    show_time_labels: bool,
    /// How items are assigned to rows.
    lane_mode: TimelineLaneMode,
    /// Bumped whenever the item list changes; keys the lane cache.
    items_revision: u64,
    /// The lane packing, recomputed only when the items or the mode change (SP-P69).
    lane_cache: RefCell<Option<(u64, TimelineLaneMode, Vec<Vec<usize>>)>>,
    /// The pointer gesture in progress, if any.
    drag: Option<TimelineDrag>,
    /// The span the dragged bar is previewed at, while a drag is in progress (SP-113).
    drag_preview: Option<(usize, i64, i64)>,
    /// Whether the time labels use a 1-2-5 step, time formatting and density markers (SP-112).
    auto_time_step: bool,
    /// Emitted when selected item changes. Payload is selected item id.
    pub item_selected: Signal1<String>,
    /// Emitted when the visible time range changes. Payload is `(start, end)`.
    pub viewport_changed: Signal1<(i64, i64)>,
    /// Emitted when the item under the pointer changes; `None` when the pointer leaves.
    pub item_hovered: Signal1<Option<String>>,
    /// Emitted with `(id, new start, new end)` while a bar is dragged to a new time span (SP-61).
    pub item_moved: Signal1<(String, i64, i64)>,
    /// Emitted with `(id, new start, new end)` while a bar's edge is dragged (SP-61).
    pub item_resized: Signal1<(String, i64, i64)>,
}

impl TimelineWidget {
    /// Creates empty timeline widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "TimelineWidget"),
            items: Vec::new(),
            selected_index: None,
            hovered_index: None,
            viewport_start: 0,
            viewport_end: 100,
            row_height: 24,
            scroll_offset: 0,
            now: None,
            animate_entry: false,
            reveal: PropertyDriver::at(1.0, MotionSlot::Normal),
            show_time_labels: false,
            lane_mode: TimelineLaneMode::Row,
            items_revision: 1,
            lane_cache: RefCell::new(None),
            drag: None,
            drag_preview: None,
            auto_time_step: false,
            item_selected: Signal1::new(),
            viewport_changed: Signal1::new(),
            item_hovered: Signal1::new(),
            item_moved: Signal1::new(),
            item_resized: Signal1::new(),
        }
    }

    /// Sets the instant drawn as a "now" marker, or `None` to hide it.
    ///
    /// The control does not read the clock: a widget that called `SystemTime::now` would
    /// repaint on its own schedule and make every snapshot depend on the moment it ran.
    pub fn set_now(&mut self, now: Option<i64>) {
        if self.now == now {
            return;
        }
        self.now = now;
        self.base.request_redraw();
    }

    /// Returns the "now" marker instant, if any.
    pub fn now(&self) -> Option<i64> {
        self.now
    }

    /// Returns how many rows are scrolled out of view.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scrolls the visible rows by `delta`.
    pub fn scroll_by(&mut self, delta: isize) {
        let max = self.lane_count().saturating_sub(self.visible_count());
        let next = (self.scroll_offset as isize + delta).clamp(0, max as isize) as usize;
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    /// Whether the time-label header is drawn.
    pub fn show_time_labels(&self) -> bool {
        self.show_time_labels
    }

    /// Shows or hides the time-label header.
    ///
    /// The header is opt-in because it shifts every lane down by its own height: a timeline that
    /// sits in a tight card keeps its pixel layout until it asks for the scale.
    pub fn set_show_time_labels(&mut self, show: bool) {
        if self.show_time_labels == show {
            return;
        }
        self.show_time_labels = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the time labels use the automatic step and formatted labels.
    pub fn auto_time_step(&self) -> bool {
        self.auto_time_step
    }

    /// Enables the automatic 1-2-5 time step, `m:ss` labels and density markers.
    ///
    /// Off by default: the historical header is four equal divisions with bare numbers, and a
    /// caller opts into the scale it wants (SP-112).
    pub fn set_auto_time_step(&mut self, auto: bool) {
        if self.auto_time_step == auto {
            return;
        }
        self.auto_time_step = auto;
        self.base.request_redraw();
    }

    /// The tick values the header draws.
    ///
    /// The historical layout divides the viewport into [`TIME_LABEL_TICKS`] equal parts. The
    /// automatic layout picks a 1-2-5 step with the chart family's [`nice_tick_step`] and lands
    /// the ticks on round multiples of that step (SP-112).
    ///
    /// [`nice_tick_step`]: crate::widget::chart_widgets::charts::nice_tick_step
    fn time_ticks(&self) -> Vec<i64> {
        if !self.auto_time_step {
            let span = self.viewport_end - self.viewport_start;
            return (0..=TIME_LABEL_TICKS)
                .map(|tick| {
                    let ratio = tick as f64 / f64::from(TIME_LABEL_TICKS);
                    self.viewport_start + (span as f64 * ratio).round() as i64
                })
                .collect();
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f64;
        let step =
            crate::widget::chart_widgets::charts::nice_tick_step(span, TIME_LABEL_TICKS as usize);
        // Timeline coordinates are integer instants, so a fractional step is rounded up to one
        // unit rather than producing duplicate labels.
        let step = if step.is_finite() { (step.round() as i64).max(1) } else { 1 };
        let first = (self.viewport_start as f64 / step as f64).ceil() as i64 * step;
        let mut ticks = Vec::new();
        let mut value = first;
        while value <= self.viewport_end && ticks.len() < 32 {
            ticks.push(value);
            value = match value.checked_add(step) {
                Some(next) => next,
                None => break,
            };
        }
        if ticks.is_empty() {
            ticks.push(self.viewport_start);
        }
        ticks
    }

    /// Formats one automatic tick as `m:ss`, or `h:mm:ss` past an hour.
    fn format_time_label(value: i64) -> String {
        let sign = if value < 0 { "-" } else { "" };
        let total = value.unsigned_abs();
        let hours = total / 3600;
        let minutes = (total / 60) % 60;
        let seconds = total % 60;
        if hours > 0 {
            crate::compat::format!("{sign}{hours}:{minutes:02}:{seconds:02}")
        } else {
            crate::compat::format!("{sign}{minutes}:{seconds:02}")
        }
    }

    /// Start times where at least three items begin close together, with their counts.
    ///
    /// "Close" is a twelfth of the visible span, so the marker follows the zoom instead of being
    /// tied to a fixed unit (SP-112).
    fn dense_clusters(&self) -> Vec<(i64, usize)> {
        let span = (self.viewport_end - self.viewport_start).max(1) as f64;
        let threshold = ((span / 12.0).ceil() as i64).max(1);
        let mut starts: Vec<i64> = self.items.iter().map(|item| item.start).collect();
        starts.sort_unstable();
        let mut clusters = Vec::new();
        let mut current: Option<(i64, usize)> = None;
        for start in starts {
            match current {
                Some((first, count)) if start - first <= threshold => {
                    current = Some((first, count + 1));
                }
                Some((first, count)) => {
                    if count >= 3 {
                        clusters.push((first, count));
                    }
                    current = Some((start, 1));
                }
                None => current = Some((start, 1)),
            }
        }
        if let Some((first, count)) = current {
            if count >= 3 {
                clusters.push((first, count));
            }
        }
        clusters
    }

    /// Returns the row assignment mode.
    pub fn lane_mode(&self) -> TimelineLaneMode {
        self.lane_mode
    }

    /// Sets the row assignment mode.
    pub fn set_lane_mode(&mut self, mode: TimelineLaneMode) {
        if self.lane_mode == mode {
            return;
        }
        self.lane_mode = mode;
        // A different packing changes how many rows there are, so the scroll offset and the grid
        // the pointer maps through must both be recomputed.
        self.scroll_offset = self.scroll_offset.min(self.lane_count().saturating_sub(1));
        self.hovered_index = None;
        self.drag = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rows of the current mode: each row is the item indices it holds.
    ///
    /// # Why the packing is cached rather than rebuilt
    ///
    /// The packing is a pure function of the item list and the mode, and it used to be recomputed
    /// on **both** the paint and the hit-test path — a pointer move and the frame that followed it
    /// each ran the full `O(items × lanes)` pass (SP-P69). The cache is keyed by the item revision
    /// and the mode, and both of those are bumped at every writer, so a stale packing is not
    /// reachable.
    fn lanes(&self) -> Ref<'_, Vec<Vec<usize>>> {
        let fresh = self.lane_cache.borrow().as_ref().is_some_and(|(revision, mode, _)| {
            *revision == self.items_revision && *mode == self.lane_mode
        });
        if !fresh {
            let lanes = self.compute_lanes();
            *self.lane_cache.borrow_mut() = Some((self.items_revision, self.lane_mode, lanes));
        }
        Ref::map(self.lane_cache.borrow(), |cache| {
            &cache.as_ref().expect("the lane packing was just computed").2
        })
    }

    /// Runs the packing for the current mode.
    fn compute_lanes(&self) -> Vec<Vec<usize>> {
        match self.lane_mode {
            TimelineLaneMode::Row => (0..self.items.len()).map(|index| vec![index]).collect(),
            TimelineLaneMode::LanePacked => {
                let mut lanes: Vec<Vec<usize>> = Vec::new();
                // The end of the last item placed in each lane: a new item overlaps the lane when
                // it starts no later than that end.
                let mut ends: Vec<i64> = Vec::new();
                for (index, item) in self.items.iter().enumerate() {
                    let mut placed = false;
                    for (lane, end) in ends.iter_mut().enumerate() {
                        if *end < item.start {
                            *end = item.end;
                            lanes[lane].push(index);
                            placed = true;
                            break;
                        }
                    }
                    if !placed {
                        ends.push(item.end);
                        lanes.push(vec![index]);
                    }
                }
                lanes
            }
        }
    }

    /// The number of rows the current mode lays out.
    fn lane_count(&self) -> usize {
        match self.lane_mode {
            TimelineLaneMode::Row => self.items.len(),
            TimelineLaneMode::LanePacked => self.lanes().len(),
        }
    }

    /// The header height the time labels reserve.
    fn axis_height(&self) -> u32 {
        if self.show_time_labels {
            TIME_LABEL_HEIGHT
        } else {
            0
        }
    }

    /// Enables the bar entry animation.
    pub fn set_animate_entry(&mut self, animate: bool) {
        if self.animate_entry == animate {
            return;
        }
        self.animate_entry = animate;
        if animate {
            self.reveal.jump_to(0.0);
            self.reveal.set_target(1.0);
        } else {
            self.reveal.jump_to(1.0);
        }
        self.base.request_redraw();
    }

    /// Returns whether bars animate in.
    pub fn animate_entry(&self) -> bool {
        self.animate_entry
    }

    /// How far the bars have grown, in `0.0..=1.0`.
    pub fn reveal_progress(&self) -> f32 {
        if self.animate_entry {
            self.reveal.value()
        } else {
            1.0
        }
    }

    /// Replaces timeline items.
    pub fn set_items(&mut self, items: Vec<TimelineItem>) {
        self.items = items;
        self.selected_index = None;
        self.items_revision = self.items_revision.wrapping_add(1);
        self.drag = None;
        self.drag_preview = None;
        self.recompute_viewport();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns timeline items.
    pub fn items(&self) -> &[TimelineItem] {
        &self.items
    }

    /// Returns selected item index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.items.len())
    }

    /// Returns selected item id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index()?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Returns viewport tuple `(start, end)`.
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
        // The range is what every bar is projected through, so a zoom changes the picture. It used
        // to change silently, which left a host with no way to keep a linked axis in step.
        self.viewport_changed.emit((start, end));
        self.base.request_redraw();
    }

    /// Zooms around current viewport center.
    pub fn zoom(&mut self, factor: f32) {
        if factor <= 0.0 {
            return;
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f32;
        let center = (self.viewport_start + self.viewport_end) as f32 / 2.0;
        let next_half = (span / factor / 2.0).max(1.0);
        let start = (center - next_half).floor() as i64;
        let end = (center + next_half).ceil() as i64;
        self.set_viewport(start, end);
    }

    /// Sets item row height.
    pub fn set_row_height(&mut self, row_height: u32) {
        let row_height = row_height.max(1);
        if self.row_height == row_height {
            return;
        }
        self.row_height = row_height;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns item row height.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Selects item by index.
    pub fn select_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        if let Some(item) = self.items.get(index) {
            self.item_selected.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    fn recompute_viewport(&mut self) {
        let (start, end) = if self.items.is_empty() {
            (0, 100)
        } else {
            let mut min_start = i64::MAX;
            let mut max_end = i64::MIN;
            for item in &self.items {
                min_start = min_start.min(item.start);
                max_end = max_end.max(item.end);
            }
            if min_start >= max_end {
                max_end = min_start + 1;
            }
            (min_start, max_end)
        };
        self.viewport_start = start;
        self.viewport_end = end;
        self.viewport_changed.emit((start, end));
    }

    fn visible_count(&self) -> usize {
        let height = self.base.geometry().height.saturating_sub(self.axis_height()) as usize;
        let row_h = self.row_height as usize;
        if row_h == 0 {
            return 0;
        }
        (height / row_h).max(1)
    }

    /// The visual row under `pos`.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.x < rect.x
            || pos.x >= rect.x + rect.width as i32
            || pos.y < rect.y
            || pos.y >= rect.y + rect.height as i32
        {
            return None;
        }
        // The header is above the rows, so a point in it addresses none of them.
        let body_top = rect.y + self.axis_height() as i32;
        if pos.y < body_top {
            return None;
        }
        let row = ((pos.y - body_top) / self.row_height as i32) as usize;
        let index = row + self.scroll_offset;
        (index < self.lane_count().min(self.visible_count() + self.scroll_offset)).then_some(index)
    }

    /// The **item** under `pos`.
    ///
    /// In row mode that is the row's item. In lane mode a row holds several items, so the one whose
    /// bar covers the pointer wins; a press in the lane's empty space addresses the first item of
    /// the lane rather than nothing, which keeps a packed timeline selectable by row.
    fn item_at(&self, pos: Point) -> Option<usize> {
        let lane = self.row_at(pos)?;
        let lanes = self.lanes();
        let members = lanes.get(lane)?;
        let (track_x, track_w) = self.track_geometry();
        for index in members {
            let item = self.items.get(*index)?;
            let x0 = self.project_x(item.start, track_x, track_w);
            let x1 = self.project_x(item.end, track_x, track_w).max(x0 + 2);
            if pos.x >= x0 - 2 && pos.x <= x1 + 2 {
                return Some(*index);
            }
        }
        members.first().copied()
    }

    /// The time track's left edge and width.
    fn track_geometry(&self) -> (i32, u32) {
        let rect = self.geometry();
        (
            rect.x + dimensions::CHART_LABEL_GUTTER,
            rect.width.saturating_sub(
                dimensions::CHART_LABEL_GUTTER as u32 + dimensions::CHART_TRACK_MARGIN as u32,
            ),
        )
    }

    /// Converts a horizontal pointer delta into time units at the current scale.
    fn pixels_to_time(&self, dx: i32, track_w: u32) -> i64 {
        if track_w == 0 {
            return 0;
        }
        let span = (self.viewport_end - self.viewport_start).max(1) as f64;
        ((dx as f64 / track_w as f64) * span).round() as i64
    }

    /// Continues the drag in progress with the pointer at `pos` and puts the gesture back.
    fn continue_drag(&mut self, drag: TimelineDrag, pos: Point) {
        let (_, track_w) = self.track_geometry();
        match drag {
            TimelineDrag::Pan { origin_x, start, end, moved } => {
                let dx = pos.x - origin_x;
                if !moved && dx.abs() < TIMELINE_PAN_THRESHOLD {
                    self.drag = Some(TimelineDrag::Pan { origin_x, start, end, moved });
                    return;
                }
                let delta = self.pixels_to_time(dx, track_w);
                self.set_viewport(start - delta, end - delta);
                self.drag = Some(TimelineDrag::Pan { origin_x, start, end, moved: true });
            }
            TimelineDrag::Move { index, origin_x, start, end } => {
                let delta = self.pixels_to_time(pos.x - origin_x, track_w);
                let next = (index, start + delta, end + delta);
                if let Some(item) = self.items.get(index) {
                    self.item_moved.emit((item.id.clone(), next.1, next.2));
                }
                self.drag_preview = Some(next);
                self.drag = Some(TimelineDrag::Move { index, origin_x, start, end });
                self.base.request_redraw();
            }
            TimelineDrag::ResizeStart { index, origin_x, start, end } => {
                let delta = self.pixels_to_time(pos.x - origin_x, track_w);
                // An edge cannot cross its counterpart, so the span never inverts.
                let next_start = (start + delta).min(end - 1);
                if let Some(item) = self.items.get(index) {
                    self.item_resized.emit((item.id.clone(), next_start, end));
                }
                self.drag_preview = Some((index, next_start, end));
                self.drag = Some(TimelineDrag::ResizeStart { index, origin_x, start, end });
                self.base.request_redraw();
            }
            TimelineDrag::ResizeEnd { index, origin_x, start, end } => {
                let delta = self.pixels_to_time(pos.x - origin_x, track_w);
                let next_end = (end + delta).max(start + 1);
                if let Some(item) = self.items.get(index) {
                    self.item_resized.emit((item.id.clone(), start, next_end));
                }
                self.drag_preview = Some((index, start, next_end));
                self.drag = Some(TimelineDrag::ResizeEnd { index, origin_x, start, end });
                self.base.request_redraw();
            }
        }
    }

    fn project_x(&self, value: i64, track_x: i32, track_w: u32) -> i32 {
        let start = self.viewport_start;
        let end = self.viewport_end.max(start + 1);
        let span = (end - start) as f32;
        let ratio = ((value - start) as f32 / span).clamp(0.0, 1.0);
        track_x + (ratio * track_w as f32) as i32
    }
}

impl Widget for TimelineWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(600, 200)
    }

    /// Advances the entry animation.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate_entry {
            return false;
        }
        let moving = self.reveal.tick(delta_ms);
        if moving {
            self.base.request_redraw();
        }
        moving
    }

    /// Whether the entry animation still owes frames.
    fn is_animating(&self) -> bool {
        self.animate_entry && self.reveal.is_moving()
    }
    impl_draw_bridge!();
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
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "viewport_changed" => {
                Some(EventSignalRef::mapped("viewport_changed", &self.viewport_changed, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Int(v.0),
                        CapabilityValue::Int(v.1),
                    ])
                }))
            }
            "item_hovered" => {
                Some(EventSignalRef::mapped("item_hovered", &self.item_hovered, |v| match v {
                    Some(id) => CapabilityValue::String(id.clone()),
                    None => CapabilityValue::Null,
                }))
            }
            "item_moved" => Some(EventSignalRef::mapped("item_moved", &self.item_moved, |v| {
                CapabilityValue::Tuple(alloc::vec![
                    CapabilityValue::String(v.0.clone()),
                    CapabilityValue::Int(v.1),
                    CapabilityValue::Int(v.2),
                ])
            })),
            "item_resized" => {
                Some(EventSignalRef::mapped("item_resized", &self.item_resized, |v| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::String(v.0.clone()),
                        CapabilityValue::Int(v.1),
                        CapabilityValue::Int(v.2),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// `TimelineWidget`'s property contract.
///
/// `selected_index` has no setter on purpose: selection is a command-shaped
/// operation (`select_index` reports whether the index was in range and emits
/// `item_selected`), so assigning a number here would silently diverge from the
/// signal. Callers that mean to select invoke the command.
impl WidgetProperties for TimelineWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole item list as records (BLUE26 B26.09): `set_items` takes a list the bare
            // command channel cannot carry, so the list is the property and `set_items` is its
            // published command form.
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &TIMELINE_ITEM_SCHEMA,
                    self.items().iter().map(timeline_item_record).collect::<Vec<_>>(),
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "viewport_start" => Ok(CapabilityValue::Int(self.viewport().0)),
            "viewport_end" => Ok(CapabilityValue::Int(self.viewport().1)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height() as u64)),
            "show_time_labels" => Ok(CapabilityValue::Bool(self.show_time_labels())),
            "lane_mode" => Ok(CapabilityValue::String(self.lane_mode().token().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Writing one viewport bound must not invert the range, so the value
            // is combined with the *other* live bound rather than stored blind.
            "viewport_start" => match value {
                CapabilityValue::Int(start) => {
                    let end = self.viewport().1;
                    self.set_viewport(start, end);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "viewport_end" => match value {
                CapabilityValue::Int(end) => {
                    let start = self.viewport().0;
                    self.set_viewport(start, end);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "row_height" => match value {
                CapabilityValue::UInt(height) => {
                    let height =
                        u32::try_from(height).map_err(|_| CapabilityAccessError::TypeMismatch)?;
                    self.set_row_height(height);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "show_time_labels" => {
                let show = match value {
                    CapabilityValue::Bool(show) => show,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_time_labels(show);
                Ok(())
            }
            "lane_mode" => {
                let token = match value {
                    CapabilityValue::String(token) => token,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                let mode = TimelineLaneMode::from_token(&token)
                    .ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_lane_mode(mode);
                Ok(())
            }
            "item_count" | "selected_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            // A whole-list replacement through the control's own `set_items`, which clears the
            // selection and bumps the layout revision.
            "items" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&TIMELINE_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("timeline_widget.items refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("items"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let items = list.items().iter().map(timeline_item_from_record).collect::<Result<
                    Vec<_>,
                    CapabilityAccessError,
                >>(
                )?;
                self.set_items(items);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "selected_index",
            "viewport_start",
            "viewport_end",
            "row_height",
            "show_time_labels",
            "lane_mode",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `timeline_widget` publishes.
    ///
    /// `zoom` takes a factor, and the factor the control's own wheel handler uses for
    /// a zoom-in is the one a bare command should mean — this mirrors `gantt_widget`,
    /// which answers the same-named command with the same fixed factor, so the two
    /// timelines cannot answer a single published name two different ways. `select_index`
    /// names which item to select and `set_items` / `set_viewport` carry their own
    /// payloads, so all three are [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom" => {
                self.zoom(1.2);
                Ok(())
            }
            "select_index" | "set_items" | "set_viewport" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TimelineWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                if let Some(drag) = self.drag.take() {
                    self.continue_drag(drag, *pos);
                    return;
                }
                // Hover is painted (the hovered row takes `hovered_background`), so it must
                // repaint — and only when the hovered item actually changes. Without this the
                // highlight lagged until an unrelated event repainted.
                let hovered = self.item_at(*pos);
                if hovered != self.hovered_index {
                    self.hovered_index = hovered;
                    self.item_hovered.emit(
                        hovered.and_then(|index| self.items.get(index)).map(|item| item.id.clone()),
                    );
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                self.drag = None;
                if self.drag_preview.take().is_some() {
                    self.base.request_redraw();
                }
                if self.hovered_index.take().is_some() {
                    self.item_hovered.emit(None);
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                let (track_x, track_w) = self.track_geometry();
                if track_w == 0 {
                    return;
                }
                match self.item_at(*pos) {
                    Some(index) => {
                        let _ = self.select_index(index);
                        // Three zones: the two edge bands resize, the body moves. Both report
                        // through signals only — the host owns the item model (SP-61).
                        let Some(item) = self.items.get(index) else { return };
                        let x0 = self.project_x(item.start, track_x, track_w);
                        let x1 = self.project_x(item.end, track_x, track_w).max(x0 + 2);
                        let base = (index, pos.x, item.start, item.end);
                        self.drag = Some(if (pos.x - x0).abs() <= TIMELINE_EDGE_GRAB {
                            TimelineDrag::ResizeStart {
                                index: base.0,
                                origin_x: base.1,
                                start: base.2,
                                end: base.3,
                            }
                        } else if (pos.x - x1).abs() <= TIMELINE_EDGE_GRAB {
                            TimelineDrag::ResizeEnd {
                                index: base.0,
                                origin_x: base.1,
                                start: base.2,
                                end: base.3,
                            }
                        } else {
                            TimelineDrag::Move {
                                index: base.0,
                                origin_x: base.1,
                                start: base.2,
                                end: base.3,
                            }
                        });
                    }
                    None => {
                        // Empty track: a drag pans the visible range, which is the primary
                        // interaction a timeline is expected to offer (SP-60). The threshold
                        // keeps a click on the row grid from shifting the range.
                        let rect = self.geometry();
                        let body_top = rect.y + self.axis_height() as i32;
                        if pos.y >= body_top {
                            self.drag = Some(TimelineDrag::Pan {
                                origin_x: pos.x,
                                start: self.viewport_start,
                                end: self.viewport_end,
                                moved: false,
                            });
                        }
                    }
                }
            }
            Event::MouseRelease { .. } => {
                self.drag = None;
                if self.drag_preview.take().is_some() {
                    self.base.request_redraw();
                }
            }
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
            Event::KeyPress { key, modifiers: _ } => match *key {
                38 => {
                    let next = self.selected_index.map_or(0, |index| index.saturating_sub(1));
                    let _ = self.select_index(next);
                }
                40 => {
                    if !self.items.is_empty() {
                        let next = self
                            .selected_index
                            .map_or(0, |index| (index + 1).min(self.items.len() - 1));
                        let _ = self.select_index(next);
                    }
                }
                33 => self.scroll_by(-(self.visible_count() as isize)),
                34 => self.scroll_by(self.visible_count() as isize),
                36 => self.scroll_by(-1),
                35 => self.scroll_by(1),
                _ => {}
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for TimelineWidget {
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
        let theme = crate::style::resolved_theme_style_shared("timeline_widget");
        // `timeline_widget` is not a control kind in the role table, so it classifies
        // as `Surface`, whose background is `theme.colors.background` — the window's
        // own colour. The panel is therefore a step toward the foreground, so it is
        // distinguishable from what is behind it.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(resolved));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);
        // Selection and hover are chrome states, derived from the resolved colours
        // so they follow the appearance.
        let selected_background = background.blend(&text_color, 0.14);
        let hovered_background = background.blend(&text_color, 0.07);
        let separator = background.blend(&text_color, 0.12);
        // A bar is the control's *value* indicator, so it reads the theme's accent
        // slot through the resolved border colour rather than a literal blue.
        let bar = border.blend(&text_color, 0.25);

        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        if self.items.is_empty() {
            return;
        }

        // The label column and the track share one derivation: `CHART_LABEL_GUTTER` is the room
        // reserved for item labels, and the track begins at its trailing edge and ends a
        // `CHART_TRACK_MARGIN` short of the control's right edge — the same pair `gantt_widget`
        // uses. The literals this replaces (`rect.x + 120` and `rect.width - 130`) spelled the
        // same fact twice and left the two charts disagreeing about how wide a label column is.
        let track_x = rect.x + dimensions::CHART_LABEL_GUTTER;
        let track_w = rect.width.saturating_sub(
            dimensions::CHART_LABEL_GUTTER as u32 + dimensions::CHART_TRACK_MARGIN as u32,
        );
        let growth = f64::from(self.reveal_progress());

        // ── Time labels ──
        //
        // Opt-in because the header shifts the rows down. The bars had no scale at all before this,
        // so a reader could compare their lengths but could not read a point in time off the axis.
        if self.show_time_labels {
            let header = Rect::new(rect.x, rect.y, rect.width, TIME_LABEL_HEIGHT);
            context.draw_line(
                Point::new(header.x, header.y + header.height as i32 - 1),
                Point::new(header.x + header.width as i32, header.y + header.height as i32 - 1),
                separator,
            );
            let auto = self.auto_time_step;
            let ink = text_color.blend(&background, 0.3);
            if auto {
                for value in self.time_ticks() {
                    let x = self.project_x(value, track_x, track_w);
                    let band = Rect::new((x - 24).max(rect.x), header.y, 48, header.height);
                    context.draw_text_line_cow(
                        band,
                        &Self::format_time_label(value),
                        effective_font(style),
                        ink,
                        HorizontalAlignment::Center,
                    );
                }
            } else {
                // The historical four equal divisions, kept at their original arithmetic so the
                // default header is unchanged.
                let span = self.viewport_end - self.viewport_start;
                for tick in 0..=TIME_LABEL_TICKS {
                    let ratio = tick as f64 / f64::from(TIME_LABEL_TICKS);
                    let value = self.viewport_start + (span as f64 * ratio).round() as i64;
                    let x = track_x + (track_w as f64 * ratio) as i32;
                    let band = Rect::new((x - 24).max(rect.x), header.y, 48, header.height);
                    context.draw_text_line_cow(
                        band,
                        &value.to_string(),
                        effective_font(style),
                        ink,
                        HorizontalAlignment::Center,
                    );
                }
            }
            // Dense start times get one marker instead of a wall of overprinted bars; the count
            // says how many arrivals the marker stands for (SP-112).
            if auto {
                for (start, count) in self.dense_clusters() {
                    let x = self.project_x(start, track_x, track_w);
                    context.fill_circle(
                        Point::new(x, header.y + header.height as i32 - 3),
                        2,
                        text_color,
                    );
                    context.draw_text(
                        Point::new(x + 4, header.y + 1),
                        &crate::compat::format!("+{count}"),
                        effective_font(style),
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        let lanes = self.lanes();
        let max_rows = self.visible_count().min(lanes.len().saturating_sub(self.scroll_offset));
        let body_top = rect.y + self.axis_height() as i32;

        for visible in 0..max_rows {
            let lane_index = self.scroll_offset + visible;
            let Some(members) = lanes.get(lane_index) else {
                break;
            };
            let y = body_top + visible as i32 * self.row_height as i32;
            let row_rect = Rect::new(rect.x, y, rect.width, self.row_height);

            let highlighted =
                |index: Option<usize>| index.is_some_and(|index| members.contains(&index));
            if highlighted(self.selected_index) {
                context.fill_rect(row_rect, selected_background);
            } else if highlighted(self.hovered_index) {
                context.fill_rect(row_rect, hovered_background);
            }

            for index in members {
                let Some(item) = self.items.get(*index) else {
                    continue;
                };
                // Row mode has one item per row, so its label takes the gutter. A packed lane has
                // several, whose gutter labels would overprint each other, so those labels move
                // beside their own bars.
                if self.lane_mode == TimelineLaneMode::Row {
                    // The label is fitted to the label column rather than spilling into the track:
                    // the old origin was the raw row midpoint, which put the glyph box's top edge
                    // on the middle line and let a long label run under its own bar. Fitting is
                    // only the horizontal half — `draw_text_fitted` leaves `y` at the band's top
                    // — so the row's own line box is taken first; otherwise replacing the midpoint
                    // with the raw `row_height` band would move the same error from "half a line
                    // low" to "pinned to the row's top edge". `draw_text_line` does both.
                    let label_band = Rect::new(
                        rect.x,
                        y,
                        dimensions::CHART_LABEL_GUTTER as u32,
                        self.row_height,
                    );
                    context.draw_text_line_cow(
                        label_band,
                        &item.label,
                        effective_font(style),
                        text_color,
                        HorizontalAlignment::Left,
                    );
                }

                let x0 = self.project_x(item.start, track_x, track_w);
                let x1 = self.project_x(item.end, track_x, track_w).max(x0 + 2);
                // Growth runs from the bar's start, so an animating bar keeps its left edge.
                let x1 = if growth >= 1.0 {
                    x1
                } else {
                    x0 + ((x1 - x0) as f64 * growth).round() as i32
                };
                let bar_w = (x1 - x0).max(2) as u32;
                let bar_rect = Rect::new(x0, y + 6, bar_w, self.row_height.saturating_sub(12));
                context.fill_rect(bar_rect, bar);
                // While a drag is in progress the pending span is painted over the bar at the
                // place it will occupy, so the pointer's effect is visible before release
                // (SP-113). A plain alpha fill reuses the bar's own colour.
                if let Some((preview_index, preview_start, preview_end)) = self.drag_preview {
                    if preview_index == *index {
                        let px0 = self.project_x(preview_start, track_x, track_w);
                        let px1 = self.project_x(preview_end, track_x, track_w).max(px0 + 2);
                        let preview = Rect::new(
                            px0,
                            y + 6,
                            (px1 - px0).max(2) as u32,
                            self.row_height.saturating_sub(12),
                        );
                        context.fill_rect(preview, bar.with_alpha(110));
                        context.draw_rect(preview, bar.with_alpha(230));
                    }
                }

                if self.lane_mode == TimelineLaneMode::LanePacked {
                    let (label_x, label_w, label_color) = if bar_w >= 28 {
                        let inset = 4i32;
                        (x0 + inset, bar_w.saturating_sub(inset as u32 * 2), bar.contrast_color())
                    } else {
                        let right = x1 + 4;
                        let width = (track_x + track_w as i32 - right).max(0) as u32;
                        (right, width, text_color)
                    };
                    if label_w > 0 {
                        let line_height = 14u32.min(self.row_height);
                        context.draw_text_fitted_cow(
                            Rect::new(
                                label_x,
                                y + ((self.row_height.saturating_sub(line_height)) / 2) as i32,
                                label_w,
                                line_height,
                            ),
                            &item.label,
                            effective_font(style),
                            label_color,
                            HorizontalAlignment::Left,
                        );
                    }
                }
            }

            context.draw_line(
                Point::new(rect.x, y + self.row_height as i32),
                Point::new(rect.x + rect.width as i32, y + self.row_height as i32),
                separator,
            );
        }

        // ── Now marker ──
        if let Some(now) = self.now {
            let x = self.project_x(now, track_x, track_w);
            let marker = crate::style::semantic_color(crate::style::SemanticColor::Error)
                .unwrap_or(Color::ERROR);
            context.draw_line_stroke(
                Point::new(x, rect.y),
                Point::new(x, rect.y + rect.height as i32),
                marker,
                1,
            );
            let label_band = Rect::new((x - 18).max(rect.x), rect.y, 36, 12);
            context.draw_text_line_cow(
                label_band,
                "now",
                effective_font(style),
                marker,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<TimelineItem> {
        vec![
            TimelineItem::new("a", "Design", 0, 10),
            TimelineItem::new("b", "Build", 8, 22),
            TimelineItem::new("c", "Verify", 20, 30),
        ]
    }

    #[test]
    fn set_items_recomputes_viewport() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        assert_eq!(timeline.viewport(), (0, 30));
        assert_eq!(timeline.items().len(), 3);
    }

    #[test]
    fn selection_emits_selected_item_id() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        timeline.item_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(timeline.select_index(1));
        assert_eq!(timeline.selected_id(), Some("b"));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["b".to_string()]);
    }

    #[test]
    fn wheel_zoom_updates_viewport_span() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        let before = timeline.viewport();
        timeline.handle_event(&Event::wheel(0, -120, 0));
        let after_in = timeline.viewport();
        assert!((after_in.1 - after_in.0) < (before.1 - before.0));

        timeline.handle_event(&Event::wheel(0, 120, 0));
        let after_out = timeline.viewport();
        assert!((after_out.1 - after_out.0) >= (after_in.1 - after_in.0));
    }

    #[test]
    fn new_creates_default_state() {
        let timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        assert!(timeline.items().is_empty());
        assert_eq!(timeline.selected_index(), None);
        assert_eq!(timeline.selected_id(), None);
        assert_eq!(timeline.viewport(), (0, 100));
        assert_eq!(timeline.row_height(), 24);
    }

    #[test]
    fn items_returns_items() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        let items = sample_items();
        timeline.set_items(items.clone());
        assert_eq!(timeline.items(), items.as_slice());
    }

    #[test]
    fn select_index_out_of_bounds_returns_false() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        assert!(!timeline.select_index(10));
        assert_eq!(timeline.selected_index(), None);
    }

    #[test]
    fn select_index_duplicate_guard_returns_true() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        assert!(timeline.select_index(0));
        assert_eq!(timeline.selected_index(), Some(0));
        // Selecting same index again returns true but doesn't emit again
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        timeline.item_selected.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });
        assert!(timeline.select_index(0));
        assert_eq!(emitted.lock().ok().map(|g| g.len()).unwrap_or_default(), 0);
    }

    #[test]
    fn set_viewport_updates_range() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_viewport(50, 150);
        assert_eq!(timeline.viewport(), (50, 150));
    }

    #[test]
    fn set_viewport_maintains_min_span() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_viewport(10, 10);
        assert_eq!(timeline.viewport(), (10, 11));
    }

    #[test]
    fn empty_items_returns_default_viewport() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(vec![]);
        assert!(timeline.items().is_empty());
        assert_eq!(timeline.selected_index(), None);
        assert_eq!(timeline.viewport(), (0, 100));
    }

    #[test]
    fn overlapping_items_viewport_spans_union() {
        let items = vec![
            TimelineItem::new("a", "Design", 0, 10),
            TimelineItem::new("b", "Build", 8, 22),
            TimelineItem::new("c", "Verify", 20, 30),
        ];
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(items);
        assert_eq!(timeline.viewport(), (0, 30));
    }

    #[test]
    fn zoom_with_zero_factor_does_nothing() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_items(sample_items());
        let before = timeline.viewport();
        timeline.zoom(0.0);
        assert_eq!(timeline.viewport(), before);
    }

    #[test]
    fn row_height_setter_validates() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 800, 600));
        timeline.set_row_height(0);
        assert_eq!(timeline.row_height(), 1);
    }

    /// Two overlapping items need two lanes; two disjoint ones share a lane.
    #[test]
    fn lane_packing_compresses_non_overlapping_items() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 200));
        timeline.set_items(vec![
            TimelineItem::new("a", "Design", 0, 10),
            TimelineItem::new("b", "Build", 5, 15),
            TimelineItem::new("c", "Verify", 20, 30),
        ]);
        // Row mode gives every item its own row.
        assert_eq!(timeline.lanes().len(), 3);

        timeline.set_lane_mode(TimelineLaneMode::LanePacked);
        {
            // Scoped: `lanes` borrows the packing cache, which must be released before the
            // capability writes below take `&mut timeline`.
            let lanes = timeline.lanes();
            assert_eq!(lanes.len(), 2, "two overlaps need two lanes, not three rows");
            // `a` and `b` overlap, so they differ; `c` starts after `a` ended, so it shares that
            // lane.
            assert!(lanes.iter().any(|lane| lane.contains(&0) && lane.contains(&2)));
            assert!(!lanes.iter().any(|lane| lane.contains(&0) && lane.contains(&1)));
        }

        // The mode is published as a token, and an unknown token is refused.
        assert_eq!(
            timeline.get("lane_mode").unwrap(),
            CapabilityValue::String("lane_packed".into())
        );
        assert!(timeline.set("lane_mode", CapabilityValue::String("diagonal".into())).is_err());
    }

    #[test]
    fn time_labels_reserve_a_header_that_is_not_a_row() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 200));
        timeline.set_items(sample_items());
        timeline.set_show_time_labels(true);
        assert!(timeline.show_time_labels());

        // A point in the header addresses no item; the first row starts below it.
        assert_eq!(timeline.row_at(Point::new(200, 2)), None);
        assert_eq!(timeline.row_at(Point::new(200, TIME_LABEL_HEIGHT as i32 + 2)), Some(0));
    }

    #[test]
    fn viewport_changed_fires_on_zoom() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());
        let ranges = Arc::new(Mutex::new(Vec::<(i64, i64)>::new()));
        let sink = ranges.clone();
        timeline.viewport_changed.connect(move |range| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*range);
            }
        });
        timeline.handle_event(&Event::wheel(0, -120, 0));
        let got = ranges.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got.len(), 1);
        assert!(got[0].1 - got[0].0 < 30, "the zoom must shrink the range: {:?}", got[0]);
    }

    #[test]
    fn hovering_an_item_reports_its_id_and_clearing_reports_none() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());
        let hovered = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let sink = hovered.clone();
        timeline.item_hovered.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        timeline.handle_event(&Event::mouse_move(200, 2));
        timeline.handle_event(&Event::MouseLeave { pos: Point::new(0, 0) });

        let got = hovered.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![Some("a".to_string()), None]);
    }

    /// SP-112: the automatic header uses round 1-2-5 ticks with time labels.
    #[test]
    fn auto_time_step_uses_round_ticks_and_time_labels() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 200));
        timeline.set_items(sample_items());
        assert!(!timeline.auto_time_step(), "the historical header is the default");

        timeline.set_auto_time_step(true);
        assert_eq!(timeline.time_ticks(), vec![0, 10, 20, 30]);
        assert_eq!(TimelineWidget::format_time_label(0), "0:00");
        assert_eq!(TimelineWidget::format_time_label(65), "1:05");
        assert_eq!(TimelineWidget::format_time_label(3_661), "1:01:01");
        assert_eq!(timeline.dense_clusters(), Vec::new(), "sample starts are far apart");
    }

    /// SP-112: three nearly simultaneous starts collapse into one counted marker.
    #[test]
    fn dense_starts_are_grouped_into_one_marker() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 200));
        timeline.set_items(vec![
            TimelineItem::new("a", "A", 0, 5),
            TimelineItem::new("b", "B", 1, 6),
            TimelineItem::new("c", "C", 2, 7),
            TimelineItem::new("d", "D", 20, 25),
        ]);
        assert_eq!(timeline.dense_clusters(), vec![(0, 3)]);
    }

    /// SP-113: a drag paints its pending span before the release commits it.
    #[test]
    fn dragging_a_bar_updates_the_preview_span() {
        let mut timeline = TimelineWidget::new(Rect::new(0, 0, 500, 120));
        timeline.set_items(sample_items());

        timeline.handle_event(&Event::mouse_press(180, 12, 1));
        assert!(matches!(timeline.drag, Some(TimelineDrag::Move { index: 0, .. })));
        timeline.handle_event(&Event::mouse_move(200, 12));
        assert!(
            matches!(timeline.drag_preview, Some((0, start, end)) if start > 0 && end > start),
            "the moved bar must preview its new span: {:?}",
            timeline.drag_preview
        );

        timeline.handle_event(&Event::mouse_release(200, 12, 1));
        assert_eq!(timeline.drag_preview, None, "release clears the preview");
    }
}
