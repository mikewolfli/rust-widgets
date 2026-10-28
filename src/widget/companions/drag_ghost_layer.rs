//! `DragGhostLayer` — the translucent drag proxy that reports a drop (COMP-A29).
//!
//! # Why a layer
//!
//! A drag is not a widget the user can hit; it is a short-lived visual proxy owned by the control
//! being dragged from. The layer therefore keeps no children and paints nothing when it is
//! inactive. The host starts a gesture with [`DragGhostLayer::begin`], forwards pointer motion to
//! [`DragGhostLayer::follow`] (or lets the layer's own event handler do it), and ends the gesture
//! with [`DragGhostLayer::finish`] or [`DragGhostLayer::cancel`].
//!
//! # What the drop signal carries
//!
//! [`DragGhostLayer::ghost_dropped`] always reports the *final pointer point* and one boolean:
//! whether the pointer finished outside the caller-supplied drop bounds. `snap_back` only decides
//! where the proxy is left on screen — it never rewrites the point the host must act on. That
//! separation is what lets a host reuse one signal for both "move the item" and "reject the drop
//! and animate home".

use crate::compat::String;
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{
    accent_color, expect_optional_rect, optional_rect_value, point_value, surface_ink,
};

/// Default translucency of the ghost.
const DEFAULT_OPACITY: f32 = 0.82;
/// Corner radius of the ghost card.
const GHOST_RADIUS: u32 = 8;
/// Inset between the ghost edge and its title.
const GHOST_INSET: u32 = 8;

/// A translucent card that follows the pointer during a drag.
///
/// The card is centred on the pointer rather than anchored by its top-left corner, because a
/// centred proxy reads as "this is the thing you picked up" and does not jump when the pointer
/// crosses the origin rectangle's edge. [`Self::finish`] reports the pointer, not the card's
/// origin, so the visual offset never leaks into the host's drop maths.
pub struct DragGhostLayer {
    base: BaseWidget,
    title: String,
    /// The card currently painted.
    ghost_rect: Rect,
    /// The rectangle the gesture started from; also the snap-back target.
    origin_rect: Rect,
    /// Whether a completed drag returns the proxy to `origin_rect`.
    snap_back: bool,
    /// Card fill opacity in `0.0..=1.0`.
    opacity: f32,
    /// The region a drop must finish inside; `None` accepts every point.
    drop_bounds: Option<Rect>,
    /// The final pointer position, kept independently of the card's painted rectangle.
    pointer: Point,
    active: bool,
    /// Emitted with `(final point, floated)`; `floated` is `true` for a drop outside the bounds.
    pub ghost_dropped: Signal1<(Point, bool)>,
    /// Emitted when [`Self::cancel`] ends an active drag.
    pub ghost_cancelled: GenericSignal,
}

impl DragGhostLayer {
    /// Creates an inactive layer.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "DragGhostLayer"),
            title: String::new(),
            ghost_rect: Rect::new(0, 0, 0, 0),
            origin_rect: Rect::new(0, 0, 0, 0),
            snap_back: true,
            opacity: DEFAULT_OPACITY,
            drop_bounds: None,
            pointer: Point::new(0, 0),
            active: false,
            ghost_dropped: Signal1::new(),
            ghost_cancelled: GenericSignal::new(),
        }
    }

    /// Starts a drag from `origin`, labelling the proxy with `title`.
    ///
    /// The proxy starts exactly over `origin` and is centred on its centre. Starting again while
    /// a drag is active replaces that drag without emitting a cancellation; the host that calls
    /// `begin` owns the gesture and should `cancel` first if it wants the signal.
    pub fn begin(&mut self, origin: Rect, title: impl Into<String>) {
        self.origin_rect = origin;
        self.ghost_rect = origin;
        self.pointer = rect_center(origin);
        self.title = title.into();
        self.active = true;
        self.base.request_redraw();
    }

    /// Moves the proxy so it is centred on `point`.
    ///
    /// Does nothing when no drag is active, so a stray move after `finish` cannot resurrect the
    /// ghost.
    pub fn follow(&mut self, point: Point) {
        if !self.active {
            return;
        }
        self.pointer = point;
        let width = self.origin_rect.width.max(1);
        let height = self.origin_rect.height.max(1);
        self.ghost_rect =
            Rect::new(point.x - (width / 2) as i32, point.y - (height / 2) as i32, width, height);
        self.base.request_redraw();
    }

    /// Ends an active drag and reports it.
    ///
    /// Returns whether the final pointer point was inside [`Self::drop_bounds()`] (`true` when no
    /// bounds were supplied). The signal is emitted even when `snap_back` restores the proxy to
    /// its origin; it always carries the final pointer and the "outside bounds" flag. An inactive
    /// call returns `false` and emits nothing.
    pub fn finish(&mut self) -> bool {
        if !self.active {
            return false;
        }
        let point = self.pointer;
        let inside = match self.drop_bounds {
            Some(bounds) => bounds.contains(point),
            None => true,
        };
        if self.snap_back {
            self.ghost_rect = self.origin_rect;
            self.pointer = rect_center(self.origin_rect);
        }
        self.active = false;
        self.base.request_redraw();
        self.ghost_dropped.emit((point, !inside));
        inside
    }

    /// Abandons an active drag, returning the proxy to its origin.
    ///
    /// Cancellation always snaps home, regardless of `snap_back`, because an abandoned
    /// drag has no drop point to stay at. Returns whether a drag was active; an inactive call
    /// emits nothing.
    pub fn cancel(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.ghost_rect = self.origin_rect;
        self.pointer = rect_center(self.origin_rect);
        self.active = false;
        self.base.request_redraw();
        self.ghost_cancelled.emit();
        true
    }

    /// Whether a drag is in progress.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the proxy's current rectangle.
    pub fn ghost_rect(&self) -> Rect {
        self.ghost_rect
    }

    /// Returns the rectangle the drag started from.
    pub fn origin_rect(&self) -> Rect {
        self.origin_rect
    }

    /// Returns the last pointer position the layer recorded.
    pub fn pointer(&self) -> Point {
        self.pointer
    }

    /// Returns the proxy title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the proxy title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title != title {
            self.title = title;
            self.base.request_redraw();
        }
    }

    /// Sets the region a drop must finish inside.
    ///
    /// The rectangle uses the usual inclusive-origin, exclusive-max-edge rule, so a zero-width or
    /// zero-height rectangle accepts nothing. That is the honest reading of an explicit bound
    /// rather than a silent fallback to "no bounds".
    pub fn set_drop_bounds(&mut self, bounds: Rect) {
        self.drop_bounds = Some(bounds);
    }

    /// Removes the drop bounds, making every point an accepted drop.
    pub fn clear_drop_bounds(&mut self) {
        self.drop_bounds = None;
    }

    /// Returns the drop bounds, if any.
    pub fn drop_bounds(&self) -> Option<Rect> {
        self.drop_bounds
    }

    /// Chooses whether a finished drag leaves the proxy at its origin.
    pub fn set_snap_back(&mut self, snap_back: bool) {
        self.snap_back = snap_back;
    }

    /// Returns whether a finished drag snaps back.
    pub fn snap_back(&self) -> bool {
        self.snap_back
    }

    /// Sets the card fill opacity, clamped to `0.0..=1.0`.
    ///
    /// A non-finite value restores the default opacity rather than poisoning the paint path with
    /// `NaN`.
    pub fn set_opacity(&mut self, opacity: f32) {
        let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { DEFAULT_OPACITY };
        if self.opacity != opacity {
            self.opacity = opacity;
            self.base.request_redraw();
        }
    }

    /// Returns the card fill opacity.
    pub fn opacity(&self) -> f32 {
        self.opacity
    }
}

impl EventHandler for DragGhostLayer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.active {
            return;
        }
        match event {
            Event::MouseMove { pos } => self.follow(*pos),
            Event::MouseRelease { pos, button: 1 } => {
                self.follow(*pos);
                self.finish();
            }
            Event::KeyPress { key, .. } if *key == crate::event::key_codes::ESCAPE => {
                self.cancel();
            }
            _ => {}
        }
    }
}

impl Draw for DragGhostLayer {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.active {
            return;
        }
        let rect = self.ghost_rect;
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);

        let shadow = Rect::new(rect.x + 2, rect.y + 3, rect.width, rect.height);
        context.fill_rounded_rect_aa(
            shadow,
            GHOST_RADIUS,
            surface.blend(&ink, 0.28 * self.opacity),
        );
        context.fill_rounded_rect_aa(rect, GHOST_RADIUS, accent.with_alpha_f32(self.opacity));
        let border = accent.with_alpha_f32((self.opacity + 0.15).min(1.0));
        context.draw_rounded_rect_stroke_aa(rect, GHOST_RADIUS, border, 1);

        if self.title.is_empty() {
            return;
        }
        let title_rect = Rect::new(
            rect.x + GHOST_INSET as i32,
            rect.y,
            rect.width.saturating_sub(GHOST_INSET * 2),
            rect.height,
        );
        context.draw_text_fitted_cow(
            title_rect,
            self.title.as_str(),
            font,
            accent.contrast_color(),
            HorizontalAlignment::Center,
        );
    }
}

impl Widget for DragGhostLayer {
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
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(200, 32),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "ghost_dropped" => {
                Some(EventSignalRef::mapped("ghost_dropped", &self.ghost_dropped, |value| {
                    CapabilityValue::Tuple(vec![
                        point_value(value.0),
                        CapabilityValue::Bool(value.1),
                    ])
                }))
            }
            "ghost_cancelled" => {
                Some(EventSignalRef::unit("ghost_cancelled", &self.ghost_cancelled))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for DragGhostLayer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "active" => Ok(CapabilityValue::Bool(self.active)),
            "snap_back" => Ok(CapabilityValue::Bool(self.snap_back)),
            "opacity" => Ok(CapabilityValue::Float(f64::from(self.opacity))),
            "drop_bounds" => Ok(optional_rect_value(self.drop_bounds)),
            "ghost_rect" => Ok(CapabilityValue::Rect(self.ghost_rect)),
            "origin_rect" => Ok(CapabilityValue::Rect(self.origin_rect)),
            _ => base_property_get(self, name),
        }
    }
    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "snap_back" => {
                self.set_snap_back(expect_bool(value)?);
                Ok(())
            }
            "opacity" => {
                self.set_opacity(expect_f32(value)?);
                Ok(())
            }
            "drop_bounds" => {
                match expect_optional_rect(value)? {
                    Some(bounds) => self.set_drop_bounds(bounds),
                    None => self.clear_drop_bounds(),
                }
                Ok(())
            }
            // The gesture's geometry is owned by begin/follow/finish, not by a property write.
            "active" | "ghost_rect" | "origin_rect" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }
    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "active",
            "snap_back",
            "opacity",
            "drop_bounds",
            "ghost_rect",
            "origin_rect",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// The centre of a rectangle, tolerating zero-width and zero-height extents.
fn rect_center(rect: Rect) -> Point {
    Point::new(rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
    use std::sync::{Arc, Mutex};

    fn layer() -> DragGhostLayer {
        DragGhostLayer::new(Rect::new(0, 0, 320, 240))
    }

    fn origin() -> Rect {
        Rect::new(10, 10, 80, 24)
    }

    #[test]
    fn a_drop_inside_the_bounds_reports_the_final_point() {
        let mut layer = layer();
        layer.set_drop_bounds(Rect::new(0, 0, 100, 100));
        let seen = Arc::new(Mutex::new(Vec::<(Point, bool)>::new()));
        let sink = Arc::clone(&seen);
        layer.ghost_dropped.connect(move |value| sink.lock().unwrap().push(*value));

        layer.begin(origin(), "Move");
        assert!(layer.is_active());
        layer.follow(Point::new(50, 60));
        assert!(layer.finish(), "the point is inside the bounds");
        assert!(!layer.is_active());

        let fired = seen.lock().unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0], (Point::new(50, 60), false));
        assert_eq!(layer.ghost_rect(), origin(), "the default snap_back restored the proxy");
    }

    #[test]
    fn a_drop_outside_the_bounds_is_reported_as_floated() {
        let mut layer = layer();
        layer.set_drop_bounds(Rect::new(0, 0, 100, 100));
        let seen = Arc::new(Mutex::new(Vec::<(Point, bool)>::new()));
        let sink = Arc::clone(&seen);
        layer.ghost_dropped.connect(move |value| sink.lock().unwrap().push(*value));

        layer.begin(origin(), "Move");
        layer.follow(Point::new(240, 180));
        assert!(!layer.finish(), "the point is outside the bounds");

        let fired = seen.lock().unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0], (Point::new(240, 180), true));
    }

    #[test]
    fn cancelling_restores_the_origin_and_reports_once() {
        let mut layer = layer();
        let dropped = Arc::new(Mutex::new(0usize));
        let cancelled = Arc::new(Mutex::new(0usize));
        let drop_sink = Arc::clone(&dropped);
        let cancel_sink = Arc::clone(&cancelled);
        layer.ghost_dropped.connect(move |_| *drop_sink.lock().unwrap() += 1);
        layer.ghost_cancelled.connect(move || *cancel_sink.lock().unwrap() += 1);

        layer.begin(origin(), "Move");
        layer.follow(Point::new(120, 140));
        assert!(layer.cancel());
        assert!(!layer.is_active());
        assert_eq!(layer.ghost_rect(), origin());
        assert_eq!(*cancelled.lock().unwrap(), 1);
        assert_eq!(*dropped.lock().unwrap(), 0, "a cancelled drag is not a drop");
    }

    #[test]
    fn finishing_while_inactive_is_a_silent_no_op() {
        let mut layer = layer();
        let dropped = Arc::new(Mutex::new(0usize));
        let cancelled = Arc::new(Mutex::new(0usize));
        let drop_sink = Arc::clone(&dropped);
        let cancel_sink = Arc::clone(&cancelled);
        layer.ghost_dropped.connect(move |_| *drop_sink.lock().unwrap() += 1);
        layer.ghost_cancelled.connect(move || *cancel_sink.lock().unwrap() += 1);

        assert!(!layer.finish());
        assert!(!layer.cancel());
        // follow must not resurrect a ghost either.
        layer.follow(Point::new(30, 30));
        assert!(!layer.is_active());
        assert_eq!(*dropped.lock().unwrap(), 0);
        assert_eq!(*cancelled.lock().unwrap(), 0);
    }

    #[test]
    fn snap_back_off_leaves_the_proxy_at_the_drop_point() {
        let mut layer = layer();
        layer.set_snap_back(false);
        assert!(!layer.snap_back());
        layer.begin(Rect::new(10, 10, 40, 20), "Move");
        layer.follow(Point::new(70, 80));
        assert!(layer.finish(), "no bounds means every drop is accepted");
        assert_eq!(layer.ghost_rect(), Rect::new(50, 70, 40, 20));
    }

    #[test]
    fn no_bounds_accepts_every_point() {
        let mut layer = layer();
        assert!(layer.drop_bounds().is_none());
        layer.begin(origin(), "Move");
        layer.follow(Point::new(-500, 900));
        assert!(layer.finish());
    }

    #[test]
    fn opacity_is_clamped_and_non_finite_values_fall_back() {
        let mut layer = layer();
        layer.set_opacity(2.0);
        assert_eq!(layer.opacity(), 1.0);
        layer.set_opacity(-1.0);
        assert_eq!(layer.opacity(), 0.0);
        layer.set_opacity(f32::NAN);
        assert_eq!(layer.opacity(), DEFAULT_OPACITY);
    }

    #[test]
    fn pointer_motion_and_release_drive_the_gesture() {
        let mut layer = layer();
        let seen = Arc::new(Mutex::new(Vec::<(Point, bool)>::new()));
        let sink = Arc::clone(&seen);
        layer.ghost_dropped.connect(move |value| sink.lock().unwrap().push(*value));

        layer.begin(origin(), "Move");
        layer.handle_event(&Event::MouseMove { pos: Point::new(44, 55) });
        assert_eq!(layer.pointer(), Point::new(44, 55));
        layer.handle_event(&Event::MouseRelease { pos: Point::new(60, 70), button: 1 });

        assert!(!layer.is_active());
        assert_eq!(*seen.lock().unwrap(), Vec::from([(Point::new(60, 70), false)]));
    }
}
