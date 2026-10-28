// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gantt companion controls (COMP-14): timeline axis and dependency editor.

use crate::compat::{lock, Arc, HashMap, Mutex};
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, Signal1};
use crate::widget::capability::coercion::{
    expect_bool, expect_f32, expect_i64, expect_string, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{DependencyKind, GanttTask, GanttWidget};

// `GanttWidget` draws bars against a viewport owned by the host. The two controls
// below add the parts the chart itself cannot express: an axis that converts between
// pixels and dates, and an editor for the dependency edges. Both are standalone so a
// host can place them beside a gantt (or over a shared axis) without the chart owning
// their state.
// ─────────────────────────────────────────────────────────────────────────────

/// Resolves the surface, border and ink a gantt companion paints with.
fn chart_chrome_palette(style: &crate::style::WidgetStyle, role: &str) -> (Color, Color, Color) {
    let theme = crate::style::resolved_theme_style_shared(role);
    let ink = style
        .text_color
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or(Color::rgb(45, 58, 74));
    let border = style
        .border_color
        .or_else(|| theme.as_ref().and_then(|t| t.border_color))
        .unwrap_or(Color::rgb(168, 184, 201));
    let surface = style
        .background_color
        .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        .unwrap_or(Color::rgb(235, 243, 250));
    (surface, border, ink)
}

/// Smallest magnification a [`TimelineAxis`] accepts.
pub const AXIS_MIN_ZOOM: f32 = 0.25;
/// Largest magnification a [`TimelineAxis`] accepts.
pub const AXIS_MAX_ZOOM: f32 = 64.0;

/// Horizontal padding between the control's edge and the axis track.
const AXIS_PADDING: i32 = 8;
/// Height of a tick mark.
const AXIS_TICK: i32 = 4;

/// How a [`TimelineAxis`] renders the label of a tick.
///
/// The axis unit is the caller's (see `GanttTask`), so only two interpretations
/// are offered: print the value, or read it as days since the Unix epoch. A format
/// string language would silently assume a unit the chart does not own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateFormat {
    /// The axis value as a plain integer (sprints, quarters, …).
    #[default]
    Numeric,
    /// Days since 1970-01-01 rendered as `YYYY-MM-DD`.
    IsoDate,
}

impl DateFormat {
    /// The stable property token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::IsoDate => "iso",
        }
    }

    /// Parses a property token.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "numeric" => Some(Self::Numeric),
            "iso" => Some(Self::IsoDate),
            _ => None,
        }
    }
}

/// A date axis that converts between pixels and axis values.
///
/// The chart's own axis labels are bare integers (`GanttWidget` formats the tick
/// value with `to_string`). This control owns the other half of that contract: a
/// caller can ask which date a pixel stands for, and where a date falls, without
/// re-deriving the chart's margins. `range` is authoritative — `zoom_level` is the
/// magnification the host requested, kept for display and for the next zoom step.
pub struct TimelineAxis {
    base: BaseWidget,
    tick_density: u32,
    date_format: DateFormat,
    /// Visible axis values, `start ..= end`.
    range: (i64, i64),
    /// Magnification relative to `base_span`.
    zoom_level: f32,
    /// The span `zoom_level == 1.0` refers to.
    base_span: f64,
    /// Tick positions, labels and label widths, with the key they were built for.
    ///
    /// Every tick used to be formatted and measured on every frame, which made a scrub or a zoom
    /// pay for the whole axis per animation step.
    tick_entries: Vec<(i32, String, i32)>,
    tick_key: Option<(i64, i64, u32, &'static str, u32, u32)>,
    /// Emitted with the new visible range after a range or zoom change.
    pub scale_changed: Signal1<(i64, i64)>,
}

impl TimelineAxis {
    /// Creates an axis over `0 ..= 100` with six ticks.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimelineAxis, geometry, "TimelineAxis"),
            tick_density: 6,
            date_format: DateFormat::Numeric,
            range: (0, 100),
            zoom_level: 1.0,
            base_span: 100.0,
            tick_entries: Vec::new(),
            tick_key: None,
            scale_changed: Signal1::new(),
        }
    }

    /// Returns the visible range.
    pub fn range(&self) -> (i64, i64) {
        self.range
    }

    /// Sets the visible range and returns the magnification to 1:1.
    ///
    /// A reversed range is widened to a single unit, mirroring `GanttWidget`'s
    /// viewport rule; an unchanged range at 1:1 emits nothing.
    pub fn set_range(&mut self, start: i64, end: i64) {
        let end = end.max(start.saturating_add(1));
        let unchanged = self.range == (start, end) && (self.zoom_level - 1.0).abs() < f32::EPSILON;
        if unchanged {
            return;
        }
        self.range = (start, end);
        self.base_span = (end - start).max(1) as f64;
        self.zoom_level = 1.0;
        self.scale_changed.emit(self.range);
        self.base.request_redraw();
    }

    /// Returns the magnification.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Magnifies the axis around its centre, clamped to
    /// [`AXIS_MIN_ZOOM`]` ..= `[`AXIS_MAX_ZOOM`].
    ///
    /// Returns `false` when the value is non-finite or unchanged. The span is
    /// `base_span / zoom_level`, so zooming is symmetric: 2.0 shows half the range
    /// that 1.0 showed, regardless of how many steps it took to get there.
    pub fn set_zoom_level(&mut self, zoom_level: f32) -> bool {
        if !zoom_level.is_finite() {
            return false;
        }
        let level = zoom_level.clamp(AXIS_MIN_ZOOM, AXIS_MAX_ZOOM);
        if (self.zoom_level - level).abs() < f32::EPSILON {
            return false;
        }
        let center = (self.range.0 as f64 + self.range.1 as f64) / 2.0;
        let span = (self.base_span / f64::from(level)).max(1.0);
        let start = (center - span / 2.0).round() as i64;
        let end = (center + span / 2.0).round() as i64;
        self.zoom_level = level;
        self.range = (start, end.max(start.saturating_add(1)));
        self.scale_changed.emit(self.range);
        self.base.request_redraw();
        true
    }

    /// Returns the requested tick count.
    pub fn tick_density(&self) -> u32 {
        self.tick_density
    }

    /// Sets the tick count, clamped to `1 ..= 32`.
    pub fn set_tick_density(&mut self, tick_density: u32) {
        let density = tick_density.clamp(1, 32);
        if self.tick_density == density {
            return;
        }
        self.tick_density = density;
        self.base.request_redraw();
    }

    /// Returns how tick labels are formatted.
    pub fn date_format(&self) -> DateFormat {
        self.date_format
    }

    /// Sets how tick labels are formatted.
    pub fn set_date_format(&mut self, date_format: DateFormat) {
        if self.date_format == date_format {
            return;
        }
        self.date_format = date_format;
        self.base.request_redraw();
    }

    /// Formats one axis value for a tick label.
    pub fn format_tick(&self, value: i64) -> String {
        match self.date_format {
            DateFormat::Numeric => value.to_string(),
            DateFormat::IsoDate => iso_date_from_days(value),
        }
    }

    /// The ticks the axis paints, as `(value, x)` pairs.
    pub fn ticks(&self) -> Vec<(i64, i32)> {
        let density = i64::from(self.tick_density.max(1));
        let (start, end) = self.range;
        let span = (end - start).max(1);
        let mut ticks: Vec<(i64, i32)> = Vec::new();
        for index in 0..=density {
            let value = if index == density { end } else { start + span * index / density };
            if ticks.last().is_some_and(|(previous, _)| *previous == value) {
                continue;
            }
            ticks.push((value, self.date_to_pixel(value)));
        }
        ticks
    }

    /// Converts an x coordinate to the axis value it stands for.
    ///
    /// `x` outside the track is clamped to the range's ends; the mapping is the
    /// inverse of [`Self::date_to_pixel`] up to rounding.
    pub fn pixel_to_date(&self, x: i32) -> i64 {
        let (left, width) = self.track_geometry();
        let span = (self.range.1 - self.range.0).max(1) as f64;
        let ratio = (f64::from(x - left) / f64::from(width)).clamp(0.0, 1.0);
        self.range.0 + (ratio * span).round() as i64
    }

    /// Converts an axis value to an x coordinate.
    ///
    /// Values outside the range project outside the track rather than clamping, so
    /// a caller can tell "off the axis" from "at the edge".
    pub fn date_to_pixel(&self, date: i64) -> i32 {
        let (left, width) = self.track_geometry();
        let span = (self.range.1 - self.range.0).max(1) as f64;
        let ratio = (date - self.range.0) as f64 / span;
        left + (ratio * f64::from(width)).round() as i32
    }

    /// The track's left edge and width.
    fn track_geometry(&self) -> (i32, u32) {
        let rect = self.geometry();
        (rect.x + AXIS_PADDING, rect.width.saturating_sub(2 * AXIS_PADDING as u32).max(1))
    }
}

/// Renders `days` since 1970-01-01 as `YYYY-MM-DD`.
///
/// Uses Howard Hinnant's `civil_from_days` decomposition, which is exact for the
/// whole `i64` range and needs no calendar table.
fn iso_date_from_days(days: i64) -> String {
    let shifted = days.saturating_add(719_468);
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097; // 0 ..= 146096
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153; // 0 ..= 11
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 { month_prime + 3 } else { month_prime - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{:04}-{:02}-{:02}", year, month, day)
}

impl Widget for TimelineAxis {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 24)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "scale_changed" => {
                Some(EventSignalRef::mapped("scale_changed", &self.scale_changed, |range| {
                    CapabilityValue::Tuple(alloc::vec![
                        CapabilityValue::Int(range.0),
                        CapabilityValue::Int(range.1),
                    ])
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for TimelineAxis {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tick_density" => Ok(CapabilityValue::UInt(u64::from(self.tick_density))),
            "date_format" => Ok(CapabilityValue::String(self.date_format.token().to_string())),
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "range" => Ok(CapabilityValue::Tuple(alloc::vec![
                CapabilityValue::Int(self.range.0),
                CapabilityValue::Int(self.range.1),
            ])),
            "range_start" => Ok(CapabilityValue::Int(self.range.0)),
            "range_end" => Ok(CapabilityValue::Int(self.range.1)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tick_density" => {
                self.set_tick_density(expect_u32(value)?);
                Ok(())
            }
            "date_format" => {
                let token = expect_string(value)?;
                let format =
                    DateFormat::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_date_format(format);
                Ok(())
            }
            "zoom_level" => {
                self.set_zoom_level(expect_f32(value)?);
                Ok(())
            }
            "range_start" => {
                self.set_range(expect_i64(value)?, self.range.1);
                Ok(())
            }
            "range_end" => {
                self.set_range(self.range.0, expect_i64(value)?);
                Ok(())
            }
            "range" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tick_density",
            "date_format",
            "zoom_level",
            "range",
            "range_start",
            "range_end",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for TimelineAxis {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::Wheel { delta, .. } = event {
            if delta.y < 0 {
                self.set_zoom_level(self.zoom_level * 1.25);
            } else if delta.y > 0 {
                self.set_zoom_level(self.zoom_level / 1.25);
            }
        }
    }
}

impl Draw for TimelineAxis {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = chart_chrome_palette(style, "timeline_axis");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let (left, width) = self.track_geometry();
        let baseline = rect.y + rect.height as i32 - 1;
        context.draw_line(
            Point::new(left, baseline),
            Point::new(left + width as i32, baseline),
            border,
        );
        let font = effective_font(style);
        // The labels and their advances are a pure function of the range, the density, the format,
        // the face and the track width; those form the cache key and nothing else is measured
        // while they hold.
        let key = (
            self.range.0,
            self.range.1,
            self.tick_density,
            self.date_format.token(),
            font.size().to_bits(),
            width,
        );
        if self.tick_key != Some(key) {
            let entries = self
                .ticks()
                .into_iter()
                .map(|(value, x)| {
                    let label = self.format_tick(value);
                    let label_width = context.measure_text(&label, font).width as i32;
                    (x, label, label_width)
                })
                .collect();
            self.tick_entries = entries;
            self.tick_key = Some(key);
        }
        for (x, label, label_width) in &self.tick_entries {
            let x = *x;
            context.draw_line(Point::new(x, baseline - AXIS_TICK), Point::new(x, baseline), border);
            if !label.is_empty() {
                let text_x =
                    (x + 2).clamp(rect.x, (rect.x + rect.width as i32 - *label_width).max(rect.x));
                context.draw_text(
                    Point::new(text_x, rect.y + 2),
                    label,
                    font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

/// One dependency edge: `predecessor` must come before `successor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// Id of the task that comes first.
    pub predecessor: String,
    /// Id of the task that waits.
    pub successor: String,
    /// The relationship kind. [`Dependency::new`] leaves it finish-to-start, which is what the
    /// untyped `GanttTask::depends_on` spelling means.
    pub kind: DependencyKind,
}

impl Dependency {
    /// Creates a finish-to-start edge from `predecessor` to `successor`.
    pub fn new(predecessor: impl Into<String>, successor: impl Into<String>) -> Self {
        Self {
            predecessor: predecessor.into(),
            successor: successor.into(),
            kind: DependencyKind::default(),
        }
    }

    /// Creates an edge with an explicit relationship kind.
    pub fn with_kind(
        predecessor: impl Into<String>,
        successor: impl Into<String>,
        kind: DependencyKind,
    ) -> Self {
        Self { predecessor: predecessor.into(), successor: successor.into(), kind }
    }
}

/// Height of one dependency row.
const DEPENDENCY_ROW_HEIGHT: i32 = 24;
/// Padding inside the editor.
const DEPENDENCY_PADDING: i32 = 8;
/// Height reserved for the editor's caption.
const DEPENDENCY_HEADER: i32 = 18;

/// The row shape of the `dependency_edges` property.
///
/// An edge is one flat record. `kind` carries the same `fts`/`sts`/`ftf` token a gantt task's
/// `dependencies` field uses, so the two list-shaped contracts in this family describe an edge
/// identically. The identity field is the successor: it is present on every edge and gives a
/// designer a stable handle to list against, while duplicate edges (which `set_dependencies`
/// already refuses) are left to that method rather than refused twice with two different errors.
pub const DEPENDENCY_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "predecessor",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "successor",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "kind",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "successor",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`DEPENDENCY_SCHEMA`].
fn dependency_record(dependency: &Dependency) -> crate::widget::capability::RecordValue {
    crate::widget::capability::RecordValue::new()
        .with("predecessor", CapabilityValue::String(dependency.predecessor.clone()))
        .with("successor", CapabilityValue::String(dependency.successor.clone()))
        .with("kind", CapabilityValue::String(dependency.kind.token().to_string()))
}

/// Rebuilds one [`Dependency`] from a row of [`DEPENDENCY_SCHEMA`].
fn dependency_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<Dependency, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    let kind = match record.get("kind") {
        None | Some(CapabilityValue::Null) => DependencyKind::default(),
        Some(CapabilityValue::String(token)) => {
            DependencyKind::from_token(token).ok_or_else(|| {
                CapabilityAccessError::ConstraintViolation {
                    property: crate::compat::String::from("dependency_edges"),
                    reason: crate::compat::format!("unknown dependency kind: {token}"),
                }
            })?
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(Dependency { predecessor: text("predecessor")?, successor: text("successor")?, kind })
}

/// A list of dependency edges that can be selected, added and removed.
///
/// Each edge occupies one row. Pressing a row reports it through
/// [`Self::link_selected`] when the editor is `editable`; the whole row is the hit
/// area, because a dependency drawn only as a one-pixel elbow is untappable on a
/// touch screen. A self-edge (`a -> a`) is refused: it can never be scheduled.
pub struct DependencyEditor {
    base: BaseWidget,
    dependencies: Vec<Dependency>,
    /// The row captions, rebuilt with the list.
    ///
    /// A caption is a formatting of an edge that cannot change while the edge cannot change, so
    /// it is built once instead of once per row per frame.
    labels: Vec<String>,
    editable: bool,
    selected: Option<usize>,
    /// Emitted when an edge is added.
    pub dependency_added: Signal1<Dependency>,
    /// Emitted when an edge is removed.
    pub dependency_removed: Signal1<Dependency>,
    /// Emitted with the edge a press selected.
    pub link_selected: Signal1<Dependency>,
}

impl DependencyEditor {
    /// Creates an empty, editable editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DependencyEditor, geometry, "DependencyEditor"),
            dependencies: Vec::new(),
            labels: Vec::new(),
            editable: true,
            selected: None,
            dependency_added: Signal1::new(),
            dependency_removed: Signal1::new(),
            link_selected: Signal1::new(),
        }
    }

    /// Returns the edges in display order.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    /// Replaces the whole list, dropping self-edges and duplicates.
    ///
    /// A bulk assignment is not an edit, so no `dependency_added` fires; the
    /// selection is cleared because its index may now name a different edge.
    pub fn set_dependencies(&mut self, dependencies: Vec<Dependency>) {
        let mut unique: Vec<Dependency> = Vec::new();
        for dependency in dependencies {
            if dependency.predecessor == dependency.successor {
                continue;
            }
            if unique.iter().any(|existing| {
                existing.predecessor == dependency.predecessor
                    && existing.successor == dependency.successor
            }) {
                continue;
            }
            unique.push(dependency);
        }
        self.dependencies = unique;
        self.selected = None;
        self.rebuild_labels();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Adds an edge. Returns `false` for a self-edge or a duplicate.
    pub fn add_dependency(&mut self, dependency: Dependency) -> bool {
        if dependency.predecessor == dependency.successor {
            return false;
        }
        if self.dependencies.iter().any(|edge| {
            edge.predecessor == dependency.predecessor && edge.successor == dependency.successor
        }) {
            return false;
        }
        self.dependencies.push(dependency.clone());
        self.rebuild_labels();
        self.dependency_added.emit(dependency);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Removes the first occurrence of `dependency`. Returns whether it existed.
    pub fn remove_dependency(&mut self, dependency: &Dependency) -> bool {
        let Some(index) = self.dependencies.iter().position(|edge| {
            edge.predecessor == dependency.predecessor && edge.successor == dependency.successor
        }) else {
            return false;
        };
        let removed = self.dependencies.remove(index);
        self.rebuild_labels();
        self.selected = match self.selected {
            Some(selected) if selected == index => None,
            Some(selected) if selected > index => Some(selected - 1),
            other => other,
        };
        self.dependency_removed.emit(removed);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Rebuilds the row captions from the current edge list.
    fn rebuild_labels(&mut self) {
        self.labels = self
            .dependencies
            .iter()
            .map(|dependency| {
                format!("{}  \u{2192}  {}", dependency.predecessor, dependency.successor)
            })
            .collect();
    }

    /// Whether presses select an edge.
    pub fn editable(&self) -> bool {
        self.editable
    }

    /// Enables or disables selection by press.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        self.base.request_redraw();
    }

    /// The row index under `point`, if any.
    pub fn hit_link(&self, point: Point) -> Option<usize> {
        (0..self.dependencies.len()).find(|index| {
            self.row_rect(*index).is_some_and(|row| {
                point.x >= row.x
                    && point.x < row.x + row.width as i32
                    && point.y >= row.y
                    && point.y < row.y + row.height as i32
            })
        })
    }

    /// The line an edge's arrow glyph is drawn along.
    ///
    /// Exposed so a host (and the hit test's tests) can place an overlay on the
    /// same geometry the control paints; `hit_link` deliberately uses the whole row.
    pub fn link_geometry(&self, index: usize) -> Option<(Point, Point)> {
        let row = self.row_rect(index)?;
        let y = row.y + row.height as i32 / 2;
        Some((
            Point::new(row.x + row.width as i32 - 44, y),
            Point::new(row.x + row.width as i32 - 10, y),
        ))
    }

    /// The rectangle one row occupies, or `None` when it does not fit.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.dependencies.len() {
            return None;
        }
        let rect = self.geometry();
        let top =
            rect.y + DEPENDENCY_PADDING + DEPENDENCY_HEADER + index as i32 * DEPENDENCY_ROW_HEIGHT;
        if top + DEPENDENCY_ROW_HEIGHT > rect.y + rect.height as i32 {
            return None;
        }
        Some(Rect::new(
            rect.x + DEPENDENCY_PADDING,
            top,
            rect.width.saturating_sub(2 * DEPENDENCY_PADDING as u32),
            DEPENDENCY_ROW_HEIGHT as u32,
        ))
    }
}

impl Widget for DependencyEditor {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(240, 160)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "dependency_added" => Some(EventSignalRef::mapped(
                "dependency_added",
                &self.dependency_added,
                dependency_value,
            )),
            "dependency_removed" => Some(EventSignalRef::mapped(
                "dependency_removed",
                &self.dependency_removed,
                dependency_value,
            )),
            "link_selected" => {
                Some(EventSignalRef::mapped("link_selected", &self.link_selected, dependency_value))
            }
            _ => None,
        }
    }
}

/// The published payload shape of a dependency event: `(predecessor, successor)`.
fn dependency_value(dependency: &Dependency) -> CapabilityValue {
    CapabilityValue::Tuple(alloc::vec![
        CapabilityValue::String(dependency.predecessor.clone()),
        CapabilityValue::String(dependency.successor.clone()),
    ])
}

impl WidgetProperties for DependencyEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "dependencies" => Ok(CapabilityValue::UInt(self.dependencies.len() as u64)),
            // The whole edge list as records (SP-123). `dependencies` stays the count it has always
            // been; this is the read/write face that can say *which* edges exist.
            "dependency_edges" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &DEPENDENCY_SCHEMA,
                    self.dependencies.iter().map(dependency_record).collect(),
                )))
            }
            "editable" => Ok(CapabilityValue::Bool(self.editable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            // A whole-list replacement through `set_dependencies`, so the record face and the
            // concrete API cannot disagree about duplicates or self-edges.
            "dependency_edges" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&DEPENDENCY_SCHEMA).map_err(|violation| {
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("dependency_edges"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let dependencies = list
                    .items()
                    .iter()
                    .map(dependency_from_record)
                    .collect::<Result<Vec<_>, CapabilityAccessError>>()?;
                self.set_dependencies(dependencies);
                Ok(())
            }
            // A count cannot say *which* edges to keep; the list is replaced through
            // `set_dependencies`, which carries the identities.
            "dependencies" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["dependencies", "dependency_edges", "editable", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for DependencyEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if !self.editable {
                return;
            }
            if let Some(index) = self.hit_link(*pos) {
                self.selected = Some(index);
                if let Some(dependency) = self.dependencies.get(index) {
                    self.link_selected.emit(dependency.clone());
                }
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for DependencyEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = chart_chrome_palette(style, "dependency_editor");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        let font = effective_font(style);
        context.draw_text_fitted_cow(
            Rect::new(rect.x + DEPENDENCY_PADDING, rect.y + 2, rect.width.saturating_sub(16), 14),
            "Dependencies",
            font,
            ink,
            HorizontalAlignment::Left,
        );
        if self.dependencies.is_empty() {
            context.draw_text_fitted_cow(
                Rect::new(
                    rect.x + DEPENDENCY_PADDING,
                    rect.y + DEPENDENCY_PADDING + DEPENDENCY_HEADER,
                    rect.width.saturating_sub(16),
                    16,
                ),
                "No dependencies",
                font,
                ink,
                HorizontalAlignment::Left,
            );
            return;
        }
        for index in 0..self.dependencies.len() {
            let Some(row) = self.row_rect(index) else {
                break;
            };
            if self.selected == Some(index) {
                context.draw_rect(row, ink);
            }
            let label = self.labels.get(index).map(String::as_str).unwrap_or("");
            context.draw_text_fitted_cow(
                Rect::new(row.x + 4, row.y + 4, row.width.saturating_sub(52), 14),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            if let Some((start, end)) = self.link_geometry(index) {
                context.draw_line(start, end, border);
                // Arrow head: two short strokes back from the tip.
                context.draw_line(end, Point::new(end.x - 4, end.y - 3), border);
                context.draw_line(end, Point::new(end.x - 4, end.y + 3), border);
            }
        }
    }
}

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

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(220, 200)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
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
        let list = crate::widget::capability::RecordList::from_items(
            &DEPENDENCY_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("predecessor", CapabilityValue::String(String::from("x")))
                    .with("successor", CapabilityValue::String(String::from("y")))
                    .with("kind", CapabilityValue::String(String::from("ftf"))),
                crate::widget::capability::RecordValue::new()
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
        let bad = crate::widget::capability::RecordList::from_items(
            &DEPENDENCY_SCHEMA,
            vec![crate::widget::capability::RecordValue::new()
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
