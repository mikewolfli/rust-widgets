// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The `Compass` bearing dial.

use super::common::companion_palette;
use rust_widgets::core::{HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::GenericSignal;
use rust_widgets::widget::capability::coercion::{expect_bool, expect_f32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// A north needle that reports a reset request when clicked.
///
/// The compass displays a bearing but does not own one: `bearing_deg` is written
/// by the host, and a click emits [`Self::reset_north_requested`] without changing
/// it — the component that owns the rotation is the only one that can undo it.
pub struct Compass {
    base: BaseWidget,
    bearing_deg: f32,
    clickable: bool,
    /// Emitted when a clickable compass is pressed.
    pub reset_north_requested: GenericSignal,
}

impl Compass {
    /// Creates a clickable compass pointing north.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Compass, geometry, "Compass"),
            bearing_deg: 0.0,
            clickable: true,
            reset_north_requested: GenericSignal::new(),
        }
    }

    /// Returns the displayed bearing in `0 .. 360` degrees, clockwise from north.
    pub fn bearing_deg(&self) -> f32 {
        self.bearing_deg
    }

    /// Sets the displayed bearing, normalised into `0 .. 360`.
    ///
    /// Non-finite values are ignored; `-90` becomes `270` and `450` becomes `90`.
    pub fn set_bearing(&mut self, bearing_deg: f32) {
        if !bearing_deg.is_finite() {
            return;
        }
        let normalized = bearing_deg.rem_euclid(360.0);
        if (self.bearing_deg - normalized).abs() < f32::EPSILON {
            return;
        }
        self.bearing_deg = normalized;
        self.base.request_redraw();
    }

    /// Whether a press reports a reset request.
    pub fn clickable(&self) -> bool {
        self.clickable
    }

    /// Enables or disables the click target.
    pub fn set_clickable(&mut self, clickable: bool) {
        if self.clickable == clickable {
            return;
        }
        self.clickable = clickable;
        self.base.request_redraw();
    }

    /// Whether `point` falls inside the dial.
    pub fn hit_dial(&self, point: Point) -> bool {
        let rect = self.geometry();
        let center = Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
        let radius = i64::from(self.radius());
        let dx = i64::from(point.x) - i64::from(center.x);
        let dy = i64::from(point.y) - i64::from(center.y);
        dx * dx + dy * dy <= radius * radius
    }

    /// The dial's radius in pixels.
    fn radius(&self) -> u32 {
        let rect = self.geometry();
        (rect.width.min(rect.height) as i32 / 2 - 2).max(4) as u32
    }
}

impl Widget for Compass {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("compass")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(48, 48)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "reset_north_requested" => {
                Some(EventSignalRef::unit("reset_north_requested", &self.reset_north_requested))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for Compass {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "bearing_deg" => Ok(CapabilityValue::Float(f64::from(self.bearing_deg))),
            "clickable" => Ok(CapabilityValue::Bool(self.clickable)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bearing_deg" => {
                self.set_bearing(expect_f32(value)?);
                Ok(())
            }
            "clickable" => {
                self.set_clickable(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["bearing_deg", "clickable", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for Compass {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.clickable && self.hit_dial(*pos) {
                self.reset_north_requested.emit();
                self.base.request_redraw();
            }
        }
    }
}

impl Draw for Compass {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "compass");
        let center = rect.center();
        let radius = self.radius();
        context.fill_circle(center, radius, surface);
        context.draw_circle(center, radius, border);
        context.draw_text(
            Point::new(center.x - 3, rect.y + 2),
            "N",
            effective_font(style),
            ink,
            HorizontalAlignment::Left,
        );
        // `bearing_deg` is clockwise from north, so the needle tip is `sin` across
        // and `-cos` up — the same convention a map rotation uses.
        let angle = self.bearing_deg.to_radians();
        let length = radius as f32 - 2.0;
        let tip = Point::new(
            center.x + (angle.sin() * length) as i32,
            center.y - (angle.cos() * length) as i32,
        );
        context.draw_line(center, tip, ink);
        let head = (radius as f32 * 0.35).max(4.0);
        for offset in [0.45f32, -0.45] {
            let wing = angle + offset;
            context.draw_line(
                tip,
                Point::new(tip.x - (wing.sin() * head) as i32, tip.y + (wing.cos() * head) as i32),
                ink,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ScaleBar, TileLoader, ZoomControl};
    use super::*;

    /// The tile status, the scale label and the zoom strip's label metrics are rebuilt only when
    /// their inputs move, not on every frame (SP-120).
    #[test]
    fn derived_labels_are_cached_until_their_inputs_move() {
        use rust_widgets::widget::svg::render_to_svg;

        let mut loader = TileLoader::new(Rect::new(0, 0, 180, 24));
        let _ = render_to_svg(&mut loader);
        let status = loader.status_cache.as_ref().expect("the first draw builds the label");
        assert_eq!(status.0, 0, "the revision starts at zero");
        let status_text = status.3.clone();

        // No model change: the same cached string and the same revision.
        let _ = render_to_svg(&mut loader);
        let reused = loader.status_cache.as_ref().expect("the cache survives");
        assert_eq!(reused.3, status_text);
        assert_eq!(reused.0, 0);

        // A cache mutation moves the revision and the counts, so the next draw rebuilds.
        let _ = loader.load_tile(2, 1, 1).expect("a tile in the world");
        let _ = render_to_svg(&mut loader);
        let rebuilt = loader.status_cache.as_ref().expect("a draw rebuilds");
        assert_ne!(rebuilt.0, 0);
        assert_ne!(rebuilt.3, status_text);

        let mut bar = ScaleBar::new(Rect::new(0, 0, 120, 24));
        let _ = render_to_svg(&mut bar);
        let first = bar.label_cache.as_ref().expect("the scale label").3.clone();
        let _ = render_to_svg(&mut bar);
        assert_eq!(bar.label_cache.as_ref().expect("cached").3, first);
        bar.set_scale(10.0);
        let _ = render_to_svg(&mut bar);
        assert_ne!(bar.label_cache.as_ref().expect("rebuilt").3, first);

        let mut zoom = ZoomControl::new(Rect::new(0, 0, 96, 28));
        let _ = render_to_svg(&mut zoom);
        let first = zoom.metrics_cache.borrow().as_ref().expect("measured labels").1;
        let _ = render_to_svg(&mut zoom);
        assert_eq!(
            zoom.metrics_cache.borrow().as_ref().expect("cached labels").1,
            first,
            "a still zoom strip re-measured its labels"
        );
    }

    /// Chapter 3, gap 12: a controller level and a fit reach the strip through the documented
    /// range conversion, and detaching stops the wiring.
    #[test]
    fn a_controller_zoom_and_fit_reach_the_zoom_strip() {
        use rust_widgets::core::Size;
        use rust_widgets::widget::companions::ZoomPanController;

        let mut strip = ZoomControl::new(Rect::new(0, 0, 96, 28));
        let mut controller = ZoomPanController::new(Rect::new(0, 0, 200, 100));
        // A 50 px wide picture in a 200 px viewport fits at 4.0.
        controller.set_content_size(Size::new(50, 25));

        strip.attach_zoom_pan(&controller);
        assert!(strip.is_zoom_pan_attached());

        let source = controller.zoom_range();
        let target = (strip.min(), strip.max());
        assert!(
            (ZoomPanController::map_zoom_to_range(1.0, source, target) - 1.0).abs() < 1e-6,
            "the shared 1:1 anchor is preserved"
        );

        controller.set_zoom_level(2.0);
        assert!(strip.poll_zoom_pan(), "the reported level is drained");
        let expected = ZoomPanController::map_zoom_to_range(2.0, source, target);
        assert!((strip.zoom_level() - expected).abs() < 1e-5, "the controller level is mapped");
        assert!(!strip.poll_zoom_pan(), "a drained slot reports nothing on the next poll");

        controller.fit_to_width();
        assert!(strip.poll_zoom_pan(), "a fit is reported like any other change");
        let fitted = ZoomPanController::map_zoom_to_range(4.0, source, target);
        assert!((strip.zoom_level() - fitted).abs() < 1e-5, "fit width drives the strip");

        strip.detach_zoom_pan();
        assert!(!strip.is_zoom_pan_attached());
        controller.set_zoom_level(8.0);
        assert!(!strip.poll_zoom_pan(), "a detached controller reports nothing");
        assert!((strip.zoom_level() - fitted).abs() < 1e-5, "the last applied level stays");
    }
}
