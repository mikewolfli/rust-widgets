// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//!
//! The `ZoomControl` strip and its button vocabulary.

use super::common::companion_palette;
use rust_widgets::compat::{lock, Arc, Mutex};
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, Signal1};
use rust_widgets::widget::capability::coercion::expect_f32;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::companions::ZoomPanController;
use rust_widgets::widget::metrics::effective_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// The multiplier one click of a [`ZoomControl`] applies.
const ZOOM_CONTROL_STEP: f32 = 2.0;

/// One button of a [`ZoomControl`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomButton {
    /// Halves the zoom level.
    Out,
    /// Returns to 1:1.
    Reset,
    /// Doubles the zoom level.
    In,
}

/// The live link between a [`ZoomControl`] and a [`ZoomPanController`] (chapter 3, gap 12).
///
/// A widget cannot hold a borrow of another, so the controller's `zoom_changed` slot parks the
/// latest level in a shared cell and [`ZoomControl::poll_zoom_pan`] applies it on the next paint
/// or interaction. A strip has no pan, so only the level is mirrored.
///
/// # Why the target range is shared
///
/// The controller's range is captured when the strip is attached, but the strip's own range can
/// still move through [`ZoomControl::set_min`]/[`ZoomControl::set_max`]. The conversion therefore
/// reads the strip's bounds from this cell on every report instead of freezing them at attach
/// time, so a resized range does not turn later conversions into a clamp against a stale end.
struct ZoomPanBinding {
    /// The latest level the controller reported, already mapped, or `None` when nothing is
    /// pending.
    zoom: Arc<Mutex<Option<f32>>>,
    /// The strip range the controller's levels are converted into.
    target: Arc<Mutex<(f32, f32)>>,
    /// The subscription, dropped with the binding so a detached strip leaves no slot behind.
    _scope: ConnectionScope,
}

/// The measured boxes of the three zoom labels, keyed by the font they were measured with.
///
/// `clippy::type_complexity` flags the tuple inline; naming it keeps the field and the cache
/// update readable without changing layout.
type MapMetricsCache = (rust_widgets::core::Font, [(u32, u32); 3]);

/// A three-button zoom strip: `-`, `1:1`, `+`.
///
/// The control owns a zoom level and reports every committed change through
/// [`Self::zoom_requested`]; it does not zoom a map itself, so a host can apply
/// the value to a `MapView`, a tile loader or another viewport.
pub struct ZoomControl {
    base: BaseWidget,
    zoom_level: f32,
    min: f32,
    max: f32,
    /// The zoom/pan controller this strip follows, when one is attached (chapter 3, gap 12).
    zoom_pan: Option<ZoomPanBinding>,
    /// The measured label boxes, keyed by the resolved font (SP-120).
    ///
    /// The three labels are static strings, so their sizes change only when the font does; the
    /// draw used to measure all three on every frame.
    pub(super) metrics_cache: core::cell::RefCell<Option<MapMetricsCache>>,
    /// Emitted with the new level after a change.
    pub zoom_requested: Signal1<f32>,
}

impl ZoomControl {
    /// Creates a control at 1:1 with a `0.25 ..= 8.0` range.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ZoomControl, geometry, "ZoomControl"),
            zoom_level: 1.0,
            min: 0.25,
            max: 8.0,
            zoom_pan: None,
            metrics_cache: core::cell::RefCell::new(None),
            zoom_requested: Signal1::new(),
        }
    }

    /// Follows `controller`'s zoom level (chapter 3, gap 12).
    ///
    /// The binding is one-way, controller → strip: the controller is the authority for the level
    /// and this strip mirrors it through [`Self::poll_zoom_pan`], which the paint and event paths
    /// call. Standalone behaviour is unchanged — with no controller attached the strip is driven
    /// only by its own buttons and wheel, exactly as before. Attaching replaces any previous
    /// controller, so calling this twice leaves one subscription rather than two.
    ///
    /// # Range
    ///
    /// The two controls disagree about the range: the controller accepts `0.05 ..= 32.0` around
    /// its `1.0`, this strip `0.25 ..= 8.0` around the same anchor. Every controller level is
    /// therefore converted with [`ZoomPanController::map_zoom_to_range`], which keeps `1:1` at
    /// `1:1` and rescales each side of it logarithmically, so the controller's ends reach the
    /// strip's ends instead of being flattened by a clamp. The controller's range is captured at
    /// attach time — a widget cannot hold a borrow of another and so is never told that the
    /// controller's range moved — while the strip's own range stays live.
    pub fn attach_zoom_pan(&mut self, controller: &ZoomPanController) {
        let zoom = Arc::new(Mutex::new(None));
        let target = Arc::new(Mutex::new((self.min, self.max)));
        let scope = ConnectionScope::new();
        let source = controller.zoom_range();
        let redraw = self.base.redraw_requested_signal().clone();

        let sink = Arc::clone(&zoom);
        let target_sink = Arc::clone(&target);
        controller.zoom_changed.connect_scoped(&scope, move |level| {
            let range = *lock(&target_sink);
            *lock(&sink) = Some(ZoomPanController::map_zoom_to_range(*level, source, range));
            // A controller gesture redraws the strip, so the poll that applies it happens next
            // frame rather than on whatever event arrives afterwards.
            redraw.emit();
        });

        self.zoom_pan = Some(ZoomPanBinding { zoom, target, _scope: scope });
        // Adopt the controller's current level now, so attaching does not wait for the next
        // controller gesture before the two agree. This is a binding, not a user gesture: mirroring
        // silently keeps a host that forwards `zoom_requested` back from overwriting the
        // controller's own level with the strip's clamped/mapped copy.
        let range = (self.min, self.max);
        let mapped = ZoomPanController::map_zoom_to_range(controller.zoom_level(), source, range);
        self.mirror_zoom(mapped);
    }

    /// Stops following the attached controller; the last applied level stays.
    pub fn detach_zoom_pan(&mut self) {
        if self.zoom_pan.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Whether a [`ZoomPanController`] is attached.
    pub fn is_zoom_pan_attached(&self) -> bool {
        self.zoom_pan.is_some()
    }

    /// Applies the level the attached controller reported since the last poll.
    ///
    /// Called from `handle_event`; the paint path uses [`Self::sync_zoom_pan`] instead, because a
    /// signal emitted during `draw` cannot be handled by a host that owns the map. Returns whether
    /// a level was pending. The value goes through [`Self::set_zoom`], so `zoom_requested` fires
    /// for a controller-driven change too: a host that refetches tiles on that signal needs no
    /// second subscription. A host that forwards `zoom_requested` back into the controller cannot
    /// loop, because `set_zoom` refuses an unchanged level.
    pub fn poll_zoom_pan(&mut self) -> bool {
        match self.take_pending_pan() {
            Some(level) => {
                let _ = self.set_zoom(level);
                true
            }
            None => false,
        }
    }

    /// Applies the pending controller level without emitting `zoom_requested`.
    ///
    /// The paint path owns this: `draw` may run inside the map's own paint pass, where a host
    /// callback cannot legally touch the map. A controller gesture still reaches such a host
    /// through the controller's own `zoom_changed`; the strip is the view, not the authority.
    pub fn sync_zoom_pan(&mut self) -> bool {
        match self.take_pending_pan() {
            Some(level) => self.mirror_zoom(level),
            None => false,
        }
    }

    /// Takes the level the bound controller queued, if any.
    fn take_pending_pan(&mut self) -> Option<f32> {
        let binding = self.zoom_pan.as_ref()?;
        lock(&binding.zoom).take()
    }

    /// Returns the current zoom level.
    pub fn zoom_level(&self) -> f32 {
        self.zoom_level
    }

    /// Returns the lower bound.
    pub fn min(&self) -> f32 {
        self.min
    }

    /// Returns the upper bound.
    pub fn max(&self) -> f32 {
        self.max
    }

    /// Sets the level directly, clamped to the range.
    ///
    /// Returns `false` when the value is non-finite or unchanged; a no-op emits
    /// nothing.
    pub fn set_zoom(&mut self, zoom_level: f32) -> bool {
        if !self.mirror_zoom(zoom_level) {
            return false;
        }
        self.zoom_requested.emit(self.zoom_level);
        true
    }

    /// Applies `zoom_level` without emitting [`Self::zoom_requested`].
    ///
    /// A map's overlay strip is a *view* of the camera rather than a second source of truth: when
    /// the camera changed from the outside (a wheel, a key, a host's `set_zoom`), the strip is
    /// brought in step silently. Going through [`Self::set_zoom`] there would report the host's
    /// own change back to it as if the user had pressed the strip, and that report happens during
    /// the map's paint pass, where a host callback cannot legally touch the map.
    ///
    /// Returns `false` when the value is non-finite or unchanged.
    pub fn mirror_zoom(&mut self, zoom_level: f32) -> bool {
        if !zoom_level.is_finite() {
            return false;
        }
        let level = zoom_level.clamp(self.min, self.max);
        if (self.zoom_level - level).abs() < f32::EPSILON {
            return false;
        }
        self.zoom_level = level;
        self.base.request_redraw();
        true
    }

    /// Doubles the level, clamped to the range.
    pub fn zoom_in(&mut self) -> bool {
        self.set_zoom(self.zoom_level * ZOOM_CONTROL_STEP)
    }

    /// Halves the level, clamped to the range.
    pub fn zoom_out(&mut self) -> bool {
        self.set_zoom(self.zoom_level / ZOOM_CONTROL_STEP)
    }

    /// Returns to 1:1 (clamped to the range).
    pub fn reset(&mut self) -> bool {
        self.set_zoom(1.0)
    }

    /// Sets the lower bound, kept at or below the upper bound, and re-clamps the level.
    pub fn set_min(&mut self, min: f32) {
        if !min.is_finite() {
            return;
        }
        let min = min.min(self.max);
        if (self.min - min).abs() < f32::EPSILON {
            return;
        }
        self.min = min;
        let before = self.zoom_level;
        self.zoom_level = self.zoom_level.clamp(self.min, self.max);
        self.refresh_zoom_pan_target();
        if (self.zoom_level - before).abs() >= f32::EPSILON {
            // A range change that moves the level is a committed zoom change; without this a host
            // that mirrors `zoom_requested` kept the level the old range allowed.
            self.zoom_requested.emit(self.zoom_level);
        }
        self.base.request_redraw();
    }

    /// Sets the upper bound, kept at or above the lower bound, and re-clamps the level.
    pub fn set_max(&mut self, max: f32) {
        if !max.is_finite() {
            return;
        }
        let max = max.max(self.min);
        if (self.max - max).abs() < f32::EPSILON {
            return;
        }
        self.max = max;
        let before = self.zoom_level;
        self.zoom_level = self.zoom_level.clamp(self.min, self.max);
        self.refresh_zoom_pan_target();
        if (self.zoom_level - before).abs() >= f32::EPSILON {
            // See `set_min`: the clamp is a change the strip's listeners have to see.
            self.zoom_requested.emit(self.zoom_level);
        }
        self.base.request_redraw();
    }

    /// Publishes the strip's current bounds to an attached controller link, so a later report is
    /// converted into the range the strip actually shows.
    fn refresh_zoom_pan_target(&self) {
        if let Some(binding) = self.zoom_pan.as_ref() {
            *lock(&binding.target) = (self.min, self.max);
        }
    }

    /// The rectangle one button occupies.
    pub fn button_rect(&self, button: ZoomButton) -> Rect {
        let rect = self.geometry();
        const GAP: i32 = 2;
        const COUNT: i32 = 3;
        let available = (rect.width as i32 - GAP * (COUNT - 1)).max(1);
        let width = (available / COUNT).max(1);
        let index = match button {
            ZoomButton::Out => 0,
            ZoomButton::Reset => 1,
            ZoomButton::In => 2,
        };
        Rect::new(rect.x + index * (width + GAP), rect.y, width as u32, rect.height)
    }

    /// The button under `point`, if any.
    pub fn hit_button(&self, point: Point) -> Option<ZoomButton> {
        [ZoomButton::Out, ZoomButton::Reset, ZoomButton::In].into_iter().find(|button| {
            let rect = self.button_rect(*button);
            point.x >= rect.x
                && point.x < rect.x + rect.width as i32
                && point.y >= rect.y
                && point.y < rect.y + rect.height as i32
        })
    }
}

impl Widget for ZoomControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("zoom_control")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    fn size_hint(&self) -> rust_widgets::core::Size {
        rust_widgets::core::Size::new(96, 28)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "zoom_requested" => {
                Some(EventSignalRef::mapped("zoom_requested", &self.zoom_requested, |level| {
                    CapabilityValue::Float(f64::from(*level))
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ZoomControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "zoom_level" => Ok(CapabilityValue::Float(f64::from(self.zoom_level))),
            "min" => Ok(CapabilityValue::Float(f64::from(self.min))),
            "max" => Ok(CapabilityValue::Float(f64::from(self.max))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "zoom_level" => {
                self.set_zoom(expect_f32(value)?);
                Ok(())
            }
            "min" => {
                self.set_min(expect_f32(value)?);
                Ok(())
            }
            "max" => {
                self.set_max(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["zoom_level", "min", "max", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for ZoomControl {
    fn handle_event(&mut self, event: &Event) {
        // A controller report is a shared-cell write, so it is drained before routing: the frame
        // that follows already shows the level the controller reported (chapter 3, gap 12).
        self.poll_zoom_pan();
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => match self.hit_button(*pos) {
                Some(ZoomButton::Out) => {
                    self.zoom_out();
                }
                Some(ZoomButton::Reset) => {
                    self.reset();
                }
                Some(ZoomButton::In) => {
                    self.zoom_in();
                }
                None => { /* A press outside the three buttons changes nothing. */ }
            },
            Event::Wheel { delta, .. } => {
                if delta.y < 0 {
                    self.zoom_in();
                } else if delta.y > 0 {
                    self.zoom_out();
                }
            }
            _ => { /* Other events are not relevant. */ }
        }
    }
}

impl Draw for ZoomControl {
    fn draw(&mut self, context: &mut RenderContext) {
        // Drain a level the bound controller reported, so the paint shows the state the controller
        // is in rather than the one from before its gesture (chapter 3, gap 12). Silent: this runs
        // during the map's paint pass, and `zoom_requested` is a user gesture.
        self.sync_zoom_pan();
        let rect = self.geometry();
        let style = self.base.style();
        let (surface, border, ink) = companion_palette(style, "zoom_control");
        context.face_with_gradient(rect, surface, None, Default::default(), 0, Color::BLACK);
        rust_widgets::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        for button in [ZoomButton::Out, ZoomButton::Reset, ZoomButton::In] {
            let button_rect = self.button_rect(button);
            context.face_with_gradient(
                button_rect,
                surface,
                None,
                Default::default(),
                2,
                Color::BLACK,
            );
            // Rounded to the button's own face radius: a square outline left the corners bare.
            rust_widgets::widget::draw::stroke_surface_border(
                context,
                button_rect,
                [2, 2, 2, 2],
                border,
                1,
            );
            let font = effective_font(style);
            // The measured label boxes are cached with the font they were measured with, so a
            // still strip costs no shaping pass (SP-120).
            let stale =
                self.metrics_cache.borrow().as_ref().is_none_or(|(cached, _)| cached != font);
            if stale {
                let labels = ["-", "1:1", "+"];
                let mut measured = [(0u32, 0u32); 3];
                for (slot, label) in measured.iter_mut().zip(labels) {
                    let metrics = context.measure_text(label, font);
                    *slot = (metrics.width, metrics.height);
                }
                *self.metrics_cache.borrow_mut() = Some((font.clone(), measured));
            }
            let label = match button {
                ZoomButton::Out => "-",
                ZoomButton::Reset => "1:1",
                ZoomButton::In => "+",
            };
            let index = match button {
                ZoomButton::Out => 0,
                ZoomButton::Reset => 1,
                ZoomButton::In => 2,
            };
            let (width, height) = self
                .metrics_cache
                .borrow()
                .as_ref()
                .map(|(_, sizes)| sizes[index])
                .unwrap_or((0, 0));
            context.draw_text(
                Point::new(
                    button_rect.x + (button_rect.width as i32 - width as i32) / 2,
                    button_rect.y + (button_rect.height as i32 - height as i32) / 2,
                ),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        crate::veil_if_disabled(&self.base, context);
    }
}
