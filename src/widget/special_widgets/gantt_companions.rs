// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gantt companion controls (COMP-14): timeline axis and dependency editor.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    expect_bool, expect_f32, expect_i64, expect_string, expect_u32,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::DependencyKind;

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
            // A count cannot say *which* edges to keep; the list is replaced through
            // `set_dependencies`, which carries the identities.
            "dependencies" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["dependencies", "editable", BASE_PROPERTY_NAMES]
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
