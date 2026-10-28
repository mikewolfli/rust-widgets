// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MapOverviewPanel` — the minimap/overview inset for a large map (COMP-A7).
//!
//! # Scope
//!
//! The panel draws a rectangle representing the map's full extent and a box representing the
//! current viewport. A press or a drag inside the rectangle is converted to normalised
//! `(0.0 ..= 1.0)` coordinates and published through
//! [`MapOverviewPanel::on_overview_drag`]; the host applies the position to its map.
//!
//! The control deliberately does **not** paint tiles. `MapView` exposes a camera and a tile
//! address space, but no world bounds — a tile pyramid has no finite extent at a given zoom, and
//! `MapView` does not expose the pixel window the camera covers. The panel therefore works from
//! an explicit extent and an explicit viewport ratio, both of which the host owns. That is
//! reported as a missing primitive rather than guessed at: a ratio derived from the zoom level
//! would be a number this control invented, not one the map reported.

use crate::compat::{lock, Arc, Mutex};
use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, EventSignalRef, Signal1};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::special_widgets::map_attribution::{map_overlay_palette, Corner};
use crate::widget::special_widgets::map_view::MapView;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The world span the overview shows before [`MapOverviewPanel::set_extent`] is called.
///
/// `(360, 180)` is one full turn of longitude and half of latitude, so a map centred at the
/// origin fills the whole overview by default.
const DEFAULT_EXTENT: (f32, f32) = (360.0, 180.0);
/// The narrowest viewport box, as a fraction of the overview.
const MIN_VIEWPORT_RATIO: f32 = 0.02;
/// Inset between the drawn overview and the widget's own rectangle.
const OVERVIEW_INSET: i32 = 1;
/// The collapsed tab's size, in logical pixels.
const COLLAPSED_TAB_WIDTH: u32 = 22;
const COLLAPSED_TAB_HEIGHT: u32 = 14;
/// Inset of the collapsed tab from its corner.
const COLLAPSED_INSET: i32 = 2;

/// How the viewport's centre is marked inside the overview.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CenterMarkerStyle {
    /// The marker's fill colour.
    pub color: Color,
    /// The marker's radius in logical pixels (floored at one).
    pub radius: u32,
    /// Whether the marker is painted at all.
    pub visible: bool,
}

impl Default for CenterMarkerStyle {
    fn default() -> Self {
        Self { color: Color::rgb(230, 88, 76), radius: 4, visible: true }
    }
}

impl CenterMarkerStyle {
    /// Creates a visible marker with `color` and `radius`.
    pub fn new(color: Color, radius: u32) -> Self {
        Self { color, radius: radius.max(1), visible: true }
    }
}

/// A feed of the attached map's centre into a shared cell.
///
/// A widget cannot hold a borrow of another, so [`MapView::center_changed`] parks the latest
/// centre here and [`MapOverviewPanel::poll_map_view`] applies it on the next paint or input.
struct MapCenterBinding {
    /// The latest reported world centre, or `None` when nothing is pending.
    center: Arc<Mutex<Option<(f32, f32)>>>,
    /// The subscription, dropped with the binding so a detached panel leaves no slot behind.
    _scope: ConnectionScope,
}

/// A minimap that reports the viewport position a host should apply to its map.
pub struct MapOverviewPanel {
    base: BaseWidget,
    viewport_ratio: f32,
    extent: (f32, f32),
    corner: Corner,
    collapsed: bool,
    /// The viewport centre in normalised overview coordinates (`0.0 ..= 1.0`).
    center: (f32, f32),
    marker_style: CenterMarkerStyle,
    /// Whether a drag is extending from a press inside the overview.
    dragging: bool,
    /// The live link to a map's centre, when one is attached.
    map_center: Option<MapCenterBinding>,
    /// Emitted with the normalised position of a press or drag inside the overview.
    pub on_overview_drag: Signal1<(f32, f32)>,
}

impl MapOverviewPanel {
    /// Creates an expanded panel showing a quarter of the default extent, anchored bottom-right.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "MapOverviewPanel"),
            viewport_ratio: 0.25,
            extent: DEFAULT_EXTENT,
            corner: Corner::BottomRight,
            collapsed: false,
            center: (0.5, 0.5),
            marker_style: CenterMarkerStyle::default(),
            dragging: false,
            map_center: None,
            on_overview_drag: Signal1::new(),
        }
    }

    /// Sets how much of the extent the viewport covers, clamped to `0.02 ..= 1.0`.
    ///
    /// A non-finite value is ignored. The ratio is host-supplied: see the module documentation
    /// for why the control cannot derive it from the map's zoom.
    pub fn set_viewport_ratio(&mut self, ratio: f32) {
        if !ratio.is_finite() {
            return;
        }
        let ratio = ratio.clamp(MIN_VIEWPORT_RATIO, 1.0);
        if (self.viewport_ratio - ratio).abs() < f32::EPSILON {
            return;
        }
        self.viewport_ratio = ratio;
        self.base.request_redraw();
    }

    /// Returns the viewport box's fraction of the overview.
    pub fn viewport_ratio(&self) -> f32 {
        self.viewport_ratio
    }

    /// Sets the world span the overview depicts, anchored at the map origin.
    ///
    /// `(width, height)` is the size of the world rectangle `-width/2 ..= width/2` by
    /// `-height/2 ..= height/2`. A non-positive or non-finite pair is ignored, so a degenerate
    /// extent cannot divide the normalisation by zero.
    pub fn set_extent(&mut self, extent: (f32, f32)) {
        let (width, height) = extent;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return;
        }
        if (self.extent.0 - width).abs() < f32::EPSILON
            && (self.extent.1 - height).abs() < f32::EPSILON
        {
            return;
        }
        self.extent = (width, height);
        self.base.request_redraw();
    }

    /// Returns the world span the overview depicts.
    pub fn extent(&self) -> (f32, f32) {
        self.extent
    }

    /// Moves the collapsed tab and the pixel inset to `corner`.
    ///
    /// An expanded overview fills its whole rectangle, so the corner only affects the collapsed
    /// tab; storing it while expanded keeps the tab where the host asked when it collapses.
    pub fn set_corner(&mut self, corner: Corner) {
        if self.corner == corner {
            return;
        }
        self.corner = corner;
        self.base.request_redraw();
    }

    /// Returns the corner the collapsed tab is anchored to.
    pub fn corner(&self) -> Corner {
        self.corner
    }

    /// Collapses the panel to a small corner tab, or expands it again.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.dragging = false;
        self.base.request_redraw();
    }

    /// Whether the panel is collapsed to its corner tab.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Sets the viewport centre from world coordinates, returning whether it moved.
    ///
    /// The pair is normalised through [`Self::extent`] and clamped into the overview, so a centre
    /// outside the depicted world parks the box against the edge rather than drawing it there.
    pub fn set_viewport_center(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let normalized =
            ((x / self.extent.0 + 0.5).clamp(0.0, 1.0), (y / self.extent.1 + 0.5).clamp(0.0, 1.0));
        self.set_normalized_center(normalized.0, normalized.1)
    }

    /// Returns the viewport centre in normalised overview coordinates.
    pub fn normalized_viewport_center(&self) -> (f32, f32) {
        self.center
    }

    /// Sets the centre-marker style.
    pub fn set_center_marker_style(&mut self, style: CenterMarkerStyle) {
        let style = CenterMarkerStyle { radius: style.radius.max(1), ..style };
        if self.marker_style == style {
            return;
        }
        self.marker_style = style;
        self.base.request_redraw();
    }

    /// Returns the centre-marker style.
    pub fn center_marker_style(&self) -> CenterMarkerStyle {
        self.marker_style
    }

    /// Follows `map`'s centre.
    ///
    /// The binding is one-way, map → panel: [`MapView`] is the authority for the camera, and the
    /// panel mirrors it through [`Self::poll_map_view`], which the paint and input paths call.
    /// Attaching replaces any previous subscription rather than doubling it. The panel's extent is
    /// not changed: a map does not report a world span, so the host sets it with
    /// [`Self::set_extent`].
    pub fn attach_map_view(&mut self, map: &mut MapView) {
        self.detach_map_view();
        let center = Arc::new(Mutex::new(None));
        let scope = ConnectionScope::new();
        let sink = Arc::clone(&center);
        let redraw = self.base.redraw_requested_signal().clone();
        map.center_changed.connect_scoped(&scope, move |value| {
            *lock(&sink) = Some(*value);
            // A camera move redraws the overview, so the poll that applies it happens next frame
            // rather than on whatever event arrives afterwards.
            redraw.emit();
        });
        self.map_center = Some(MapCenterBinding { center, _scope: scope });
        // Adopt the map's current centre now, so attaching does not wait for the next pan.
        let (x, y) = map.center();
        let _ = self.set_viewport_center(x, y);
    }

    /// Stops following the attached map; the last applied centre stays.
    pub fn detach_map_view(&mut self) {
        if self.map_center.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a map is attached.
    pub fn is_map_view_attached(&self) -> bool {
        self.map_center.is_some()
    }

    /// Applies the centre the attached map reported since the last poll.
    ///
    /// Returns whether a centre was pending. Called from `draw` and `handle_event`, so a pan is
    /// reflected by the next frame.
    pub fn poll_map_view(&mut self) -> bool {
        let Some(binding) = self.map_center.as_ref() else {
            return false;
        };
        let Some((x, y)) = lock(&binding.center).take() else {
            return false;
        };
        let _ = self.set_viewport_center(x, y);
        true
    }

    /// Sets the normalised centre directly, returning whether it moved.
    fn set_normalized_center(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let next = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        if (self.center.0 - next.0).abs() < f32::EPSILON
            && (self.center.1 - next.1).abs() < f32::EPSILON
        {
            return false;
        }
        self.center = next;
        self.base.request_redraw();
        true
    }

    /// The rectangle the overview is drawn in, or `None` when collapsed or degenerate.
    pub fn overview_rect(&self) -> Option<Rect> {
        if self.collapsed {
            return None;
        }
        let rect = self.geometry();
        let width = rect.width.checked_sub((OVERVIEW_INSET * 2) as u32)?;
        let height = rect.height.checked_sub((OVERVIEW_INSET * 2) as u32)?;
        if width == 0 || height == 0 {
            return None;
        }
        Some(Rect::new(rect.x + OVERVIEW_INSET, rect.y + OVERVIEW_INSET, width, height))
    }

    /// The viewport box, in the widget's own coordinates.
    pub fn viewport_box_rect(&self) -> Option<Rect> {
        let area = self.overview_rect()?;
        let width = ((area.width as f32 * self.viewport_ratio).round() as u32).clamp(1, area.width);
        let height =
            ((area.height as f32 * self.viewport_ratio).round() as u32).clamp(1, area.height);
        let max_x = area.width.saturating_sub(width);
        let max_y = area.height.saturating_sub(height);
        let x = ((self.center.0 * area.width as f32 - width as f32 / 2.0).round() as i32)
            .clamp(0, max_x as i32);
        let y = ((self.center.1 * area.height as f32 - height as f32 / 2.0).round() as i32)
            .clamp(0, max_y as i32);
        Some(Rect::new(area.x + x, area.y + y, width, height))
    }

    /// Converts a point in the widget to a normalised overview position.
    ///
    /// `None` when the panel is collapsed, degenerate, or the point is outside the overview.
    pub fn normalized_at(&self, point: Point) -> Option<(f32, f32)> {
        let area = self.overview_rect()?;
        if !area.contains_point(point) || area.width == 0 || area.height == 0 {
            return None;
        }
        Some((
            ((point.x - area.x) as f32 / area.width as f32).clamp(0.0, 1.0),
            ((point.y - area.y) as f32 / area.height as f32).clamp(0.0, 1.0),
        ))
    }

    /// The collapsed tab's rectangle, or `None` when the panel is expanded.
    pub fn collapsed_tab_rect(&self) -> Option<Rect> {
        if !self.collapsed {
            return None;
        }
        let area = self.geometry();
        if area.width == 0 || area.height == 0 {
            return None;
        }
        let width = COLLAPSED_TAB_WIDTH.min(area.width);
        let height = COLLAPSED_TAB_HEIGHT.min(area.height);
        let x = if self.corner.is_right() {
            area.right() - COLLAPSED_INSET - width as i32
        } else {
            area.x + COLLAPSED_INSET
        };
        let y = if self.corner.is_bottom() {
            area.bottom() - COLLAPSED_INSET - height as i32
        } else {
            area.y + COLLAPSED_INSET
        };
        Some(Rect::new(x.max(area.x), y.max(area.y), width, height))
    }
}

impl Widget for MapOverviewPanel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> Size {
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(160, 120),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "overview_drag" | "on_overview_drag" => {
                Some(EventSignalRef::mapped("overview_drag", &self.on_overview_drag, |value| {
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::Float(f64::from(value.0)),
                        CapabilityValue::Float(f64::from(value.1)),
                    ])
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MapOverviewPanel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "viewport_ratio" => Ok(CapabilityValue::Float(self.viewport_ratio() as f64)),
            "collapsed" => Ok(CapabilityValue::Bool(self.is_collapsed())),
            "map_view_attached" => Ok(CapabilityValue::Bool(self.is_map_view_attached())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "viewport_ratio" => {
                let CapabilityValue::Float(ratio) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if !ratio.is_finite() {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                self.set_viewport_ratio(ratio as f32);
                Ok(())
            }
            "collapsed" => {
                self.set_collapsed(expect_bool(value)?);
                Ok(())
            }
            "map_view_attached" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["viewport_ratio", "collapsed", "map_view_attached", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for MapOverviewPanel {
    fn handle_event(&mut self, event: &Event) {
        // A camera report is a shared-cell write, so it is drained before routing: the frame that
        // follows already shows the centre the map reported.
        self.poll_map_view();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        if self.collapsed {
            if let Event::MousePress { pos, button: 1, .. } = event {
                if self.collapsed_tab_rect().is_some_and(|tab| tab.contains_point(*pos)) {
                    self.set_collapsed(false);
                }
            }
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some((x, y)) = self.normalized_at(*pos) {
                    let _ = self.set_normalized_center(x, y);
                    self.dragging = true;
                    self.on_overview_drag.emit((x, y));
                }
            }
            Event::MouseMove { pos } => {
                if !self.dragging {
                    return;
                }
                if let Some((x, y)) = self.normalized_at(*pos) {
                    let _ = self.set_normalized_center(x, y);
                    self.on_overview_drag.emit((x, y));
                }
            }
            Event::MouseRelease { button: 1, .. } => self.dragging = false,
            _ => { /* Other events are not relevant. */ }
        }
    }
}

impl Draw for MapOverviewPanel {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a centre the attached map reported, so the paint shows the camera the map is in
        // rather than the one from before its pan.
        self.poll_map_view();
        let style = self.base.style();
        let (surface, border, _ink) = map_overlay_palette(style, "map_overview");
        let accent = crate::style::palette_shared()
            .as_ref()
            .map(|colors| colors.primary)
            .unwrap_or(Color::PRIMARY);

        if self.collapsed {
            if let Some(tab) = self.collapsed_tab_rect() {
                context.fill_rounded_rect(tab, 3, surface);
                crate::widget::draw::stroke_surface_border(context, tab, [3, 3, 3, 3], accent, 1);
                // A chevron made of two bars, so the tab reads as "expand" without a glyph face.
                let bar = Rect::new(tab.x + tab.width as i32 / 2 - 4, tab.y + 5, 9, 1);
                context.fill_rect(bar, accent);
                context.fill_rect(Rect::new(bar.x + 2, bar.y + 2, 5, 1), accent);
            }
            return;
        }

        let Some(area) = self.overview_rect() else {
            return;
        };
        context.fill_rounded_rect(area, 3, surface);
        crate::widget::draw::stroke_surface_border(context, area, [3, 3, 3, 3], border, 1);

        let Some(viewport) = self.viewport_box_rect() else {
            return;
        };
        context.fill_rect(viewport, accent.with_alpha(56));
        context.draw_rect(viewport, accent);

        let marker = self.marker_style;
        if marker.visible && viewport.width > 0 && viewport.height > 0 {
            let radius = marker.radius.min(viewport.width.min(viewport.height) / 2).max(1);
            let center_x = viewport.x + viewport.width as i32 / 2;
            let center_y = viewport.y + viewport.height as i32 / 2;
            let size = radius * 2;
            context.fill_rounded_rect(
                Rect::new(center_x - radius as i32, center_y - radius as i32, size, size),
                radius,
                marker.color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn drag_events(panel: &mut MapOverviewPanel, events: &[Event]) -> Vec<(f32, f32)> {
        let emitted = Arc::new(Mutex::new(Vec::<(f32, f32)>::new()));
        let sink = emitted.clone();
        panel.on_overview_drag.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((value.0, value.1));
            }
        });
        for event in events {
            panel.handle_event(event);
        }
        emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default()
    }

    /// Press and drag map a point inside the overview to normalised coordinates and emit each one.
    #[test]
    fn dragging_maps_to_normalized_coordinates_and_emits() {
        // The 1 px inset leaves a 200x100 overview at (1, 1), so the arithmetic is exact.
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        let emitted = drag_events(
            &mut panel,
            &[
                Event::mouse_press(101, 51, 1),
                Event::MouseMove { pos: Point::new(51, 26) },
                Event::mouse_release(51, 26, 1),
            ],
        );

        assert_eq!(emitted.len(), 2, "the press and the move both report a position");
        assert!((emitted[0].0 - 0.5).abs() < 1e-5);
        assert!((emitted[0].1 - 0.5).abs() < 1e-5);
        assert!((emitted[1].0 - 0.25).abs() < 1e-5);
        assert!((emitted[1].1 - 0.25).abs() < 1e-5);
        let center = panel.normalized_viewport_center();
        assert!((center.0 - 0.25).abs() < 1e-5);
        assert!((center.1 - 0.25).abs() < 1e-5);
    }

    /// The box's size follows the ratio and its position follows the centre.
    #[test]
    fn the_viewport_box_follows_the_ratio_and_the_center() {
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        panel.set_viewport_ratio(0.5);
        let box_rect = panel.viewport_box_rect().expect("an expanded panel has a box");
        assert_eq!(box_rect, Rect::new(51, 26, 100, 50));

        assert!(panel.set_viewport_center(90.0, 45.0));
        let center = panel.normalized_viewport_center();
        assert!((center.0 - 0.75).abs() < 1e-5);
        assert!((center.1 - 0.75).abs() < 1e-5);
        let moved = panel.viewport_box_rect().expect("an expanded panel has a box");
        assert!(moved.x > box_rect.x && moved.y > box_rect.y);
        assert!(moved.right() <= panel.overview_rect().expect("overview").right());
        assert!(moved.bottom() <= panel.overview_rect().expect("overview").bottom());
    }

    /// A zero-sized panel has neither a viewport nor a hit target, so it must stay silent.
    #[test]
    fn a_zero_sized_panel_ignores_pointer_input() {
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 0, 0));
        assert!(panel.overview_rect().is_none());
        assert!(panel.viewport_box_rect().is_none());
        assert!(panel.normalized_at(Point::new(0, 0)).is_none());
        assert!(drag_events(&mut panel, &[Event::mouse_press(0, 0, 1)]).is_empty());
    }

    /// Collapsing removes the viewport and the drag target; pressing the tab expands it again.
    #[test]
    fn a_collapsed_panel_has_no_viewport_and_expands_from_its_tab() {
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        panel.set_collapsed(true);
        assert!(panel.is_collapsed());
        assert!(panel.viewport_box_rect().is_none());
        assert!(panel.normalized_at(Point::new(100, 50)).is_none());

        let tab = panel.collapsed_tab_rect().expect("a collapsed panel has a tab");
        assert!(drag_events(&mut panel, &[Event::mouse_press(tab.x + 1, tab.y + 1, 1)]).is_empty());
        assert!(!panel.is_collapsed(), "pressing the tab expands the panel");
        assert!(panel.viewport_box_rect().is_some());
    }

    /// Degenerate extents and ratios are ignored rather than producing a division by zero.
    #[test]
    fn degenerate_extent_and_ratio_are_ignored() {
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        let extent = panel.extent();
        panel.set_extent((0.0, 180.0));
        panel.set_extent((f32::NAN, 180.0));
        panel.set_extent((-10.0, 180.0));
        assert_eq!(panel.extent(), extent);

        panel.set_viewport_ratio(f32::NAN);
        assert_eq!(panel.viewport_ratio(), 0.25);
        panel.set_viewport_ratio(0.0);
        assert_eq!(panel.viewport_ratio(), MIN_VIEWPORT_RATIO);
        panel.set_viewport_ratio(4.0);
        assert_eq!(panel.viewport_ratio(), 1.0);
    }

    /// Attaching mirrors the map's centre and detaching stops the feed.
    #[test]
    fn attachment_follows_the_map_center() {
        let mut map = MapView::new(Rect::new(0, 0, 300, 200));
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        panel.attach_map_view(&mut map);
        assert!(panel.is_map_view_attached());
        assert_eq!(panel.normalized_viewport_center(), (0.5, 0.5));

        map.set_center(90.0, 45.0);
        assert!(panel.poll_map_view(), "the pending centre is drained");
        assert!((panel.normalized_viewport_center().0 - 0.75).abs() < 1e-5);
        assert!((panel.normalized_viewport_center().1 - 0.75).abs() < 1e-5);
        assert!(!panel.poll_map_view(), "a drained slot reports nothing on the next poll");

        panel.detach_map_view();
        assert!(!panel.is_map_view_attached());
        map.set_center(-90.0, -45.0);
        assert!(!panel.poll_map_view(), "a detached map reports nothing");
        assert!((panel.normalized_viewport_center().0 - 0.75).abs() < 1e-5);
    }

    /// The marker style is validated and round trips.
    #[test]
    fn the_center_marker_style_round_trips() {
        let mut panel = MapOverviewPanel::new(Rect::new(0, 0, 202, 102));
        assert!(panel.center_marker_style().visible);
        panel.set_center_marker_style(CenterMarkerStyle {
            color: Color::PRIMARY,
            radius: 0,
            visible: false,
        });
        let style = panel.center_marker_style();
        assert_eq!(style.radius, 1, "a radius of zero is floored");
        assert_eq!(style.color, Color::PRIMARY);
        assert!(!style.visible);
    }
}
