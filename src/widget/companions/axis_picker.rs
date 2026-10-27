//! `AxisPicker` — assign chart series to plot edges (COMP-A27).
//!
//! # Why a companion
//!
//! Multi-axis charts (`LineChart`, `BarChart`, `ScatterChart`, `AreaChart`) accept an attached
//! axis on the left, right, top or bottom edge, but each host built its own series-to-edge UI.
//! This type owns that list once: it holds the series names, one [`AxisEdge`] per series and the
//! four [`AxisSpec`] descriptors, and emits [`AxisPicker::axis_assigned`] when a row changes.
//!
//! # Defaults
//!
//! Every series starts on the left edge, because `WidgetKind::Chart` already treats left as the
//! primary Y axis. A new series added by [`AxisPicker::set_series`] therefore starts on the left
//! rather than appearing unassigned, and clicking a row cycles through the four edges.
//!
//! # Relationship to the chart model
//!
//! The chart's own `chart_widgets::AxisEdge` names the same four edges. This module keeps a local
//! copy because the companion is usable without the chart's model types; a host maps one to the
//! other when it attaches the resulting spec.

use crate::compat::{String, ToString, Vec};
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_string_list};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    effective_font, estimate_line_height, estimate_text_width, ControlMetrics,
};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// The width of the edge-token column.
const TOKEN_WIDTH: u32 = 34;
/// The narrowest usable picker.
const PICKER_MIN_WIDTH: u32 = 220;
/// The minimum height of one series row.
const PICKER_ROW_MIN_HEIGHT: u32 = 24;
/// Extra air one row needs beyond one text line.
const PICKER_ROW_AIR: u32 = 6;
/// Vertical inset used by the metric derivation.
const PICKER_PADDING_V: u32 = 3;
/// Horizontal inset used by the metric derivation.
const PICKER_PADDING_H: u32 = 6;

/// One edge of a chart's plot area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AxisEdge {
    /// The left edge — the usual primary Y axis.
    #[default]
    Left,
    /// The right edge — the usual secondary Y axis.
    Right,
    /// The top edge.
    Top,
    /// The bottom edge — the usual X axis.
    Bottom,
}

impl AxisEdge {
    /// Every edge, in the order the picker cycles through them.
    pub const ALL: [AxisEdge; 4] =
        [AxisEdge::Left, AxisEdge::Right, AxisEdge::Top, AxisEdge::Bottom];

    /// The published token used by the property contract.
    pub fn as_str(self) -> &'static str {
        match self {
            AxisEdge::Left => "left",
            AxisEdge::Right => "right",
            AxisEdge::Top => "top",
            AxisEdge::Bottom => "bottom",
        }
    }

    /// Parses the published token.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(AxisEdge::Left),
            "right" => Some(AxisEdge::Right),
            "top" => Some(AxisEdge::Top),
            "bottom" => Some(AxisEdge::Bottom),
            _ => None,
        }
    }

    /// The one-letter token painted in the compact series list.
    pub fn token(self) -> &'static str {
        match self {
            AxisEdge::Left => "L",
            AxisEdge::Right => "R",
            AxisEdge::Top => "T",
            AxisEdge::Bottom => "B",
        }
    }

    /// The next edge in the click cycle.
    fn next(self) -> Self {
        match self {
            AxisEdge::Left => AxisEdge::Right,
            AxisEdge::Right => AxisEdge::Top,
            AxisEdge::Top => AxisEdge::Bottom,
            AxisEdge::Bottom => AxisEdge::Left,
        }
    }
}

/// The descriptor the chart keeps for one edge.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisSpec {
    /// Which plot edge this descriptor is for.
    pub edge: AxisEdge,
    /// The label the chart paints for the edge.
    pub label: String,
    /// Whether the chart chooses the axis range automatically.
    pub auto_range: bool,
}

impl AxisSpec {
    /// Creates one edge descriptor.
    pub fn new(edge: AxisEdge, label: impl Into<String>, auto_range: bool) -> Self {
        Self { edge, label: label.into(), auto_range }
    }
}

/// A compact series list with an edge token per row.
///
/// Clicking a row advances that series to the next edge and emits
/// [`Self::axis_assigned`]. A host that wants a menu instead can call [`Self::assign`] directly;
/// paint is identical either way.
pub struct AxisPicker {
    base: BaseWidget,
    series_names: Vec<String>,
    assignments: Vec<AxisEdge>,
    edges: Vec<AxisSpec>,
    /// Emitted with `(series_index, edge)` when a row's assignment actually changes.
    pub axis_assigned: Signal1<(usize, AxisEdge)>,
}

impl AxisPicker {
    /// Creates an empty picker with all four edge descriptors.
    pub fn new(geometry: Rect) -> Self {
        let edges: Vec<AxisSpec> =
            AxisEdge::ALL.iter().map(|edge| AxisSpec::new(*edge, edge.as_str(), true)).collect();
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "AxisPicker"),
            series_names: Vec::new(),
            assignments: Vec::new(),
            edges,
            axis_assigned: Signal1::new(),
        }
    }

    /// Returns the series names.
    pub fn series_names(&self) -> &[String] {
        &self.series_names
    }

    /// Replaces the series names.
    ///
    /// Assignments already stored at an index are preserved; new indices start on the left edge.
    /// This is bulk configuration, so it emits no [`Self::axis_assigned`].
    pub fn set_series(&mut self, series_names: Vec<String>) {
        self.assignments.resize(series_names.len(), AxisEdge::Left);
        self.series_names = series_names;
        self.base.request_redraw();
    }

    /// Returns the edge assigned to each series, aligned with [`Self::series_names`].
    pub fn assignments(&self) -> &[AxisEdge] {
        &self.assignments
    }

    /// Replaces the assignment list.
    ///
    /// Missing tail entries default to the left edge and extra entries are ignored, so the result
    /// is always aligned with the series list. This is bulk configuration, so it emits no
    /// [`Self::axis_assigned`]; use [`Self::assign`] for a single user-driven change.
    pub fn set_assignments(&mut self, assignments: Vec<AxisEdge>) {
        let count = self.series_names.len();
        self.assignments.clear();
        self.assignments.extend(assignments.into_iter().take(count));
        self.assignments.resize(count, AxisEdge::Left);
        self.base.request_redraw();
    }

    /// Assigns `series_index` to `edge`.
    ///
    /// Returns `false` when `series_index` is past the end of the series list. Re-assigning the
    /// edge a series already uses succeeds but emits nothing, because nothing changed.
    pub fn assign(&mut self, series_index: usize, edge: AxisEdge) -> bool {
        if series_index >= self.series_names.len() {
            return false;
        }
        if self.assignments.len() < self.series_names.len() {
            self.assignments.resize(self.series_names.len(), AxisEdge::Left);
        }
        if self.assignments[series_index] == edge {
            return true;
        }
        self.assignments[series_index] = edge;
        self.base.request_redraw();
        self.axis_assigned.emit((series_index, edge));
        true
    }

    /// Returns the edge assigned to `series_index`, or `None` for an unknown index.
    pub fn assignment(&self, series_index: usize) -> Option<AxisEdge> {
        self.assignments.get(series_index).copied()
    }

    /// Returns the edge descriptors, always one per [`AxisEdge`].
    pub fn edges(&self) -> &[AxisSpec] {
        &self.edges
    }

    /// Sets one edge's label.
    ///
    /// The four descriptors are created by [`Self::new`], so this updates rather than appends for
    /// every valid edge; the append branch keeps the method total if a caller ever builds a
    /// reduced descriptor list through the property route.
    pub fn set_edge_label(&mut self, edge: AxisEdge, label: impl Into<String>) {
        let label = label.into();
        if let Some(spec) = self.edges.iter_mut().find(|spec| spec.edge == edge) {
            if spec.label != label {
                spec.label = label;
                self.base.request_redraw();
            }
        } else {
            self.edges.push(AxisSpec::new(edge, label, true));
            self.base.request_redraw();
        }
    }

    /// Returns the label stored for `edge`, if the edge has a descriptor.
    pub fn label_for(&self, edge: AxisEdge) -> Option<&str> {
        self.edges.iter().find(|spec| spec.edge == edge).map(|spec| spec.label.as_str())
    }

    /// Sets whether `edge` chooses its range automatically.
    ///
    /// Returns `false` when the edge has no descriptor.
    pub fn set_auto_range(&mut self, edge: AxisEdge, auto_range: bool) -> bool {
        let changed = match self.edges.iter_mut().find(|spec| spec.edge == edge) {
            Some(spec) => {
                let changed = spec.auto_range != auto_range;
                spec.auto_range = auto_range;
                changed
            }
            None => return false,
        };
        if changed {
            self.base.request_redraw();
        }
        true
    }

    /// The series row under `pos`.
    fn series_at(&self, pos: Point) -> Option<usize> {
        if self.series_names.is_empty() || !self.geometry().contains_point(pos) {
            return None;
        }
        let rect = self.geometry();
        let row_height = (rect.height / self.series_names.len() as u32).max(1);
        let offset = (pos.y - rect.y).max(0) as u32;
        Some(((offset / row_height) as usize).min(self.series_names.len() - 1))
    }

    /// Replaces the descriptors, restoring a default descriptor for any missing edge.
    fn replace_edges(&mut self, specs: Vec<AxisSpec>) {
        let mut edges = Vec::with_capacity(AxisEdge::ALL.len());
        for edge in AxisEdge::ALL {
            let spec = specs
                .iter()
                .find(|spec| spec.edge == edge)
                .cloned()
                .unwrap_or_else(|| AxisSpec::new(edge, edge.as_str(), true));
            edges.push(spec);
        }
        self.edges = edges;
        self.base.request_redraw();
    }
}

impl EventHandler for AxisPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        let Event::MouseRelease { pos, button: 1 } = event else {
            return;
        };
        let Some(index) = self.series_at(*pos) else {
            return;
        };
        let current = self.assignment(index).unwrap_or(AxisEdge::Left);
        self.assign(index, current.next());
    }
}

impl Draw for AxisPicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        if self.series_names.is_empty() {
            let empty_text_rect =
                Rect::new(rect.x + 8, rect.y, rect.width.saturating_sub(16), rect.height);
            context.draw_text_fitted_cow(
                empty_text_rect,
                "No series",
                font,
                surface.blend(&ink, 0.55),
                HorizontalAlignment::Left,
            );
            return;
        }

        let row_height = (rect.height / self.series_names.len() as u32).max(1);
        let token_width = TOKEN_WIDTH.min(rect.width / 2);
        for (index, name) in self.series_names.iter().enumerate() {
            let offset = (index as u32).saturating_mul(row_height);
            let y = rect.y.saturating_add(offset.min(i32::MAX as u32) as i32);
            let row = Rect::new(rect.x, y, rect.width, row_height);
            if index + 1 < self.series_names.len() {
                let separator_y =
                    y.saturating_add(row_height.min(i32::MAX as u32) as i32).saturating_sub(1);
                context.draw_line(
                    Point::new(row.x, separator_y),
                    Point::new(
                        row.x.saturating_add(row.width.min(i32::MAX as u32) as i32),
                        separator_y,
                    ),
                    surface.blend(&ink, 0.10),
                );
            }

            let name_rect = Rect::new(
                row.x.saturating_add(6),
                row.y,
                row.width.saturating_sub(token_width.saturating_add(12)),
                row.height,
            );
            context.draw_text_fitted_cow(name_rect, name, font, ink, HorizontalAlignment::Left);

            let edge = self.assignments.get(index).copied().unwrap_or(AxisEdge::Left);
            let token_x = row.x.saturating_add(
                row.width.saturating_sub(token_width.saturating_add(4)).min(i32::MAX as u32) as i32,
            );
            let token_rect = Rect::new(token_x, row.y, token_width, row.height);
            context.fill_rect(token_rect, accent.with_alpha(28));
            context.draw_rect(token_rect, accent);
            context.draw_text_fitted_cow(
                token_rect,
                edge.token(),
                font,
                accent,
                HorizontalAlignment::Center,
            );
        }
    }
}

impl Widget for AxisPicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        let font = effective_font(self.style());
        let line_height = estimate_line_height(font, 1.0);
        let row_height = line_height.saturating_add(PICKER_ROW_AIR).max(PICKER_ROW_MIN_HEIGHT);
        let name_width = self
            .series_names
            .iter()
            .fold(0u32, |width, name| width.max(estimate_text_width(name, font, 1.0)));
        let content = Size::new(
            name_width.saturating_add(TOKEN_WIDTH).saturating_add(PICKER_PADDING_H * 2),
            row_height.saturating_mul(self.series_names.len() as u32),
        );
        ControlMetrics::implicit_size(
            content,
            EdgeOffsets::symmetric(PICKER_PADDING_V, PICKER_PADDING_H),
            Size::new(PICKER_MIN_WIDTH, PICKER_ROW_MIN_HEIGHT),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "axis_assigned" => {
                Some(EventSignalRef::mapped("axis_assigned", &self.axis_assigned, |value| {
                    let (index, edge) = *value;
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::UInt(index as u64),
                        CapabilityValue::String(edge.as_str().to_string()),
                    ])
                }))
            }
            _ => None,
        }
    }
}

/// Encodes one edge descriptor as `[edge, label, auto_range]`.
fn edge_value(spec: &AxisSpec) -> CapabilityValue {
    CapabilityValue::Tuple(vec![
        CapabilityValue::String(spec.edge.as_str().to_string()),
        CapabilityValue::String(spec.label.clone()),
        CapabilityValue::Bool(spec.auto_range),
    ])
}

/// Decodes the assignment-list property.
fn expect_assignments(value: CapabilityValue) -> Result<Vec<AxisEdge>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut assignments = Vec::with_capacity(items.len());
    for item in items {
        let token = expect_string(item)?;
        let edge = AxisEdge::from_name(&token.to_ascii_lowercase())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        assignments.push(edge);
    }
    Ok(assignments)
}

/// Decodes the edge-descriptor property.
fn expect_edges(value: CapabilityValue) -> Result<Vec<AxisSpec>, CapabilityAccessError> {
    let CapabilityValue::Tuple(items) = value else {
        return Err(CapabilityAccessError::TypeMismatch);
    };
    let mut specs = Vec::with_capacity(items.len());
    for item in items {
        let CapabilityValue::Tuple(parts) = item else {
            return Err(CapabilityAccessError::TypeMismatch);
        };
        if parts.len() != 3 {
            return Err(CapabilityAccessError::TypeMismatch);
        }
        let mut parts = parts.into_iter();
        let edge_token = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let edge = AxisEdge::from_name(&edge_token.to_ascii_lowercase())
            .ok_or(CapabilityAccessError::TypeMismatch)?;
        let label = expect_string(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        let auto_range = expect_bool(parts.next().ok_or(CapabilityAccessError::TypeMismatch)?)?;
        specs.push(AxisSpec { edge, label, auto_range });
    }
    Ok(specs)
}

impl WidgetProperties for AxisPicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "series_names" => Ok(CapabilityValue::Tuple(
                self.series_names
                    .iter()
                    .map(|series| CapabilityValue::String(series.clone()))
                    .collect(),
            )),
            "assignments" => Ok(CapabilityValue::Tuple(
                self.assignments
                    .iter()
                    .map(|edge| CapabilityValue::String(edge.as_str().to_string()))
                    .collect(),
            )),
            "edges" => Ok(CapabilityValue::Tuple(self.edges.iter().map(edge_value).collect())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "series_names" => {
                self.set_series(expect_string_list(value)?);
                Ok(())
            }
            "assignments" => {
                self.set_assignments(expect_assignments(value)?);
                Ok(())
            }
            "edges" => {
                self.replace_edges(expect_edges(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["series_names", "assignments", "edges", BASE_PROPERTY_NAMES]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{lock, Arc, Mutex};

    /// Records a payload signal into a shared vector.
    fn record<T: Clone + Send + 'static>(signal: &Signal1<T>) -> Arc<Mutex<Vec<T>>> {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let handle = Arc::clone(&sink);
        signal.connect(move |value: Arc<T>| {
            lock(&handle).push((*value).clone());
        });
        sink
    }

    /// Reads the recorded values.
    fn values<T: Clone>(sink: &Arc<Mutex<Vec<T>>>) -> Vec<T> {
        lock(sink).clone()
    }

    fn picker() -> AxisPicker {
        let mut picker = AxisPicker::new(Rect::new(0, 0, 240, 96));
        picker.set_series(vec!["Revenue".to_string(), "Cost".to_string()]);
        picker
    }

    #[test]
    fn assign_replaces_the_previous_edge_for_a_series() {
        let mut picker = picker();
        assert_eq!(picker.assignment(0), Some(AxisEdge::Left));
        assert!(picker.assign(0, AxisEdge::Right));
        assert_eq!(picker.assignment(0), Some(AxisEdge::Right));
        assert!(picker.assign(0, AxisEdge::Top));
        assert_eq!(picker.assignment(0), Some(AxisEdge::Top));

        // One slot per series, not one slot per assignment.
        assert_eq!(picker.assignments().len(), 2);
        assert_eq!(picker.assignment(1), Some(AxisEdge::Left));
    }

    #[test]
    fn an_unknown_series_index_is_refused() {
        let mut picker = picker();
        assert!(!picker.assign(9, AxisEdge::Right));
        assert_eq!(picker.assignment(9), None);
        assert_eq!(picker.assignments().len(), 2);
    }

    #[test]
    fn an_edge_label_round_trips() {
        let mut picker = picker();
        picker.set_edge_label(AxisEdge::Right, "Secondary");
        assert_eq!(picker.label_for(AxisEdge::Right), Some("Secondary"));

        let spec = picker
            .edges()
            .iter()
            .find(|spec| spec.edge == AxisEdge::Right)
            .expect("the right edge exists");
        assert_eq!(spec.label, "Secondary");
        assert!(spec.auto_range);
        assert_eq!(picker.edges().len(), 4);
    }

    #[test]
    fn clicking_a_row_cycles_its_edge() {
        let mut picker = picker();
        assert_eq!(picker.assignment(0), Some(AxisEdge::Left));

        // Two 48-pixel rows; the first row covers y 0..47.
        picker.handle_event(&Event::MouseRelease { pos: Point::new(10, 10), button: 1 });
        assert_eq!(picker.assignment(0), Some(AxisEdge::Right));
        assert_eq!(picker.assignment(1), Some(AxisEdge::Left));
    }

    #[test]
    fn a_successful_assign_emits_one_signal() {
        let mut picker = picker();
        let assigned = record(&picker.axis_assigned);

        assert!(picker.assign(1, AxisEdge::Bottom));
        assert_eq!(values(&assigned), vec![(1, AxisEdge::Bottom)]);

        // Re-assigning the same edge is a valid no-op and emits nothing.
        assert!(picker.assign(1, AxisEdge::Bottom));
        assert_eq!(values(&assigned).len(), 1);

        // An unknown index changes nothing and emits nothing.
        assert!(!picker.assign(9, AxisEdge::Right));
        assert_eq!(values(&assigned).len(), 1);
    }

    #[test]
    fn the_property_contract_round_trips() {
        let mut picker = AxisPicker::new(Rect::new(0, 0, 240, 96));
        let names = CapabilityValue::Tuple(vec![
            CapabilityValue::String("Revenue".to_string()),
            CapabilityValue::String("Cost".to_string()),
        ]);
        picker.set("series_names", names).expect("series names are writable");
        assert_eq!(picker.series_names().len(), 2);

        let assignments =
            CapabilityValue::Tuple(vec![CapabilityValue::String("right".to_string())]);
        picker.set("assignments", assignments).expect("assignments are writable");
        assert_eq!(picker.assignment(0), Some(AxisEdge::Right));
        assert_eq!(
            picker.assignment(1),
            Some(AxisEdge::Left),
            "the missing tail defaults to the left edge"
        );

        let edges = CapabilityValue::Tuple(vec![CapabilityValue::Tuple(vec![
            CapabilityValue::String("bottom".to_string()),
            CapabilityValue::String("Volume".to_string()),
            CapabilityValue::Bool(false),
        ])]);
        picker.set("edges", edges).expect("edges are writable");
        assert_eq!(picker.label_for(AxisEdge::Bottom), Some("Volume"));
        assert_eq!(
            picker
                .edges()
                .iter()
                .find(|spec| spec.edge == AxisEdge::Bottom)
                .map(|spec| spec.auto_range),
            Some(false)
        );
        assert_eq!(picker.edges().len(), 4, "missing edges are restored");
    }
}
