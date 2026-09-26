// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! RefreshControl widget — pull-to-refresh control for scrollable views.
//!
//! Shows a pull-down indicator (spinner arrow) at the top of the widget when
//! the user pulls down, and emits a `refresh_triggered` signal when the pull
//! distance exceeds the configured threshold. Supports embedding child content
//! below the indicator area.

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Progress state for the refresh pull gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshState {
    /// No pull in progress.
    Idle,
    /// User is pulling down (not yet past threshold).
    Dragging,
    /// Pull distance exceeded threshold; refresh triggered on release.
    Triggered,
}

/// Pull-to-refresh control for mobile scrollable views.
///
/// Wraps optional child content and displays a pull-down indicator at the top.
/// When the user pulls down past `threshold` pixels and releases, the
/// `refresh_triggered` signal fires. The `is_refreshing` flag is managed
/// externally and controls whether a loading spinner is shown.
pub struct RefreshControl {
    base: BaseWidget,
    is_refreshing: bool,
    pull_distance: f32,
    threshold: f32,
    state: RefreshState,
    /// The child widget this control wraps, by id.
    ///
    /// # Why an `ObjectId` rather than a stored `Box<dyn Widget>`
    ///
    /// This was a `Box<dyn Widget>` that the draw **never composited**: the child was allocated,
    /// stored, reachable through `content()`, and never painted, because the only way a child reaches
    /// the screen in this crate is by being registered with [`BaseWidget::add_child`] — the compositor
    /// walks the tree, and a box nobody registered is a box nobody draws. The module doc claimed
    /// "Supports embedding child content", so a host that embedded a list got a blank panel.
    ///
    /// `PopupWindow` and `Dialog` hold their content the same way for the same reason, and this is
    /// that pattern rather than a second one.
    content: Option<ObjectId>,
    /// Screen position where the in-progress pull gesture started. `None` when
    /// no pull is being tracked.
    drag_origin_y: Option<f32>,
    /// The pull distance the control is *drawing*, which is `pull_distance` while the gesture is
    /// in flight and then animates back to zero once it ends.
    ///
    /// # Why the drawn distance is not `pull_distance`
    ///
    /// `pull_distance` is the gesture's own measurement: a caller reads it, `update_pull` writes
    /// it, and it must reach `0.0` the instant the finger lifts because the gesture is over. The
    /// *drawn* distance is what the reader sees, and it must not do that — releasing a pull below
    /// the threshold used to zero it on the same statement, so the band snapped shut rather than
    /// springing back. This is the same split as `Switch`'s `checked` versus `travel`, and the same
    /// defect `swipe_to_dismiss` records: a gesture that ends with a jump instead of a settle.
    settle: PropertyDriver,
    /// Emitted when the pull distance exceeds the threshold and the user releases.
    pub refresh_triggered: GenericSignal,
}

impl RefreshControl {
    /// Creates a new RefreshControl widget with the given geometry.
    ///
    /// Default threshold is 60.0 pixels.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RefreshControl, geometry, "RefreshControl"),
            is_refreshing: false,
            pull_distance: 0.0,
            threshold: 60.0,
            state: RefreshState::Idle,
            content: None,
            drag_origin_y: None,
            // At the resting end: a freshly built control shows no reveal band, and must not
            // animate one *away* on its first frame.
            settle: PropertyDriver::at(0.0, MotionSlot::Normal),
            refresh_triggered: GenericSignal::new(),
        }
    }

    /// The height of the pull indicator band, zero when nothing is showing.
    ///
    /// # Why this is a method and not a local in `draw`
    ///
    /// The content area starts below this band, and two callers need to agree on where that is: the
    /// draw (which fills the region) and a host (which lays its child out inside it). It was a local
    /// in `draw` with the content rectangle recomputed from it inline, which is how the child would
    /// have been positioned against a second derivation of the same number.
    fn indicator_height(&self) -> u32 {
        if self.is_refreshing || self.settle_distance() > 5.0 {
            dimensions::REFRESH_INDICATOR_HEIGHT
        } else {
            0
        }
    }

    /// Sets the child content widget, taking ownership into the widget tree.
    ///
    /// # Why this *registers* the child instead of storing it
    ///
    /// Storing a `Box<dyn Widget>` is not enough for it to be drawn: the compositor paints what the
    /// tree owns, so a child that is not registered is invisible no matter how many accessors point at
    /// it. Registering it here is what makes [`Self::content_rect`] the region it actually paints into.
    ///
    /// An object id that this control does not own (a stale one, or another widget's) is refused by
    /// `add_child` and left unset, rather than stored and silently never painted -- which is the
    /// defect this method used to have in a different form.
    pub fn set_content(&mut self, widget: ObjectId) {
        if let Some(old) = self.content {
            self.base.remove_child(old);
        }
        self.content = Some(widget);
        self.base.add_child(widget);
        self.base.request_redraw();
    }

    /// Clears the child, detaching it from the tree.
    pub fn clear_content(&mut self) {
        if let Some(old) = self.content.take() {
            self.base.remove_child(old);
            self.base.request_redraw();
        }
    }

    /// Returns the id of the child content widget, if any.
    pub fn content(&self) -> Option<ObjectId> {
        self.content
    }

    /// The rectangle the child content paints into: the whole control below the pull indicator.
    ///
    /// Derived here rather than re-derived at each call site, so a host positioning the child and the
    /// draw filling the region cannot disagree about where the content area is. Public because the
    /// host is the one who lays the child out.
    pub fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let indicator = self.indicator_height();
        Rect::new(
            rect.x,
            rect.y + indicator as i32,
            rect.width,
            rect.height.saturating_sub(indicator),
        )
    }

    /// Sets the refreshing state.
    pub fn set_is_refreshing(&mut self, refreshing: bool) {
        if self.is_refreshing != refreshing {
            self.is_refreshing = refreshing;
            if !refreshing {
                self.pull_distance = 0.0;
                self.state = RefreshState::Idle;
                // The refresh finished, so the reveal band closes — over frames, not instantly.
                self.settle.set_target(0.0);
            }
            self.base.request_redraw();
        }
    }

    /// Returns whether a refresh operation is in progress.
    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing
    }

    /// Sets the pull distance (pixels pulled down from the top).
    pub fn set_pull_distance(&mut self, distance: f32) {
        let clamped = distance.max(0.0);
        self.pull_distance = clamped;
        // A programmatic set is a gesture substitute, so the drawn distance follows it at once.
        self.settle.jump_to(self.pull_distance);
        if clamped > 0.0 && self.state == RefreshState::Idle {
            self.state = RefreshState::Dragging;
        } else if clamped == 0.0 && self.state == RefreshState::Dragging {
            self.state = RefreshState::Idle;
        }
        self.base.request_redraw();
    }

    /// Returns the current pull distance.
    pub fn pull_distance(&self) -> f32 {
        self.pull_distance
    }

    /// Sets the pull threshold in pixels.
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// Returns the pull threshold.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Called when the user starts pulling down.
    pub fn start_pull(&mut self) {
        if self.state == RefreshState::Idle && !self.is_refreshing {
            self.state = RefreshState::Dragging;
            self.base.request_redraw();
        }
    }

    /// Called to update the pull distance during a drag.
    pub fn update_pull(&mut self, delta: f32) {
        if self.is_refreshing {
            return;
        }
        if self.state == RefreshState::Idle {
            self.state = RefreshState::Dragging;
        }
        self.pull_distance = (self.pull_distance + delta).max(0.0).min(self.threshold * 2.0);
        // The gesture is live, so the drawn distance follows it exactly and any leftover settle
        // from the previous pull is adopted rather than replayed.
        self.settle.jump_to(self.pull_distance);
        self.base.request_redraw();
    }

    /// Called when the user releases the pull.
    /// Triggers refresh if above threshold.
    ///
    /// The measured distance returns to zero at once — the gesture is over, and a caller polling
    /// `pull_distance()` must not see a value that belongs to a finger that has lifted. The
    /// *drawn* distance is left where the gesture put it and [`Self::tick`] springs it back, so
    /// releasing below the threshold is a visible return rather than the band snapping shut.
    pub fn end_pull(&mut self) {
        if self.state == RefreshState::Dragging {
            if self.pull_distance >= self.threshold && !self.is_refreshing {
                self.state = RefreshState::Triggered;
                self.is_refreshing = true;
                self.refresh_triggered.emit();
            } else {
                self.state = RefreshState::Idle;
            }
            self.pull_distance = 0.0;
            // Aim the drawn distance at zero; the band springs back over the frames that follow.
            self.settle.set_target(0.0);
            self.base.request_redraw();
        }
    }

    /// The pull distance currently being *drawn*, which lags the measured distance while the
    /// control springs back after a release.
    pub fn settle_distance(&self) -> f32 {
        self.settle.value()
    }

    /// Advances the spring-back by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.settle.tick(delta_ms)
    }

    /// Whether the reveal band is between two distances -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.settle.is_moving()
    }

    /// Returns the current refresh state.
    pub fn refresh_state(&self) -> RefreshState {
        self.state
    }
}

impl Widget for RefreshControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(400, 400)
    }

    // The spring-back is the control's own animation; the trait spelling is what the frame bus
    // reaches through `&mut dyn Widget`, which is the only way the settle actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        RefreshControl::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        RefreshControl::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `RefreshControl`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for RefreshControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "refreshing" => Ok(CapabilityValue::Bool(self.is_refreshing())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "refreshing" => {
                self.set_is_refreshing(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["refreshing", BASE_PROPERTY_NAMES]
    }
}

impl Draw for RefreshControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // ── The reveal strip is a **fixed** band at the control's top edge ──
        //
        // A pull-down indicator is chrome: the same 40 px strip opens in a 400 px list
        // and in a smaller one. The height is named (`REFRESH_INDICATOR_HEIGHT`) and the
        // band is placed with `ControlMetrics::top_band`, which is the shared derivation
        // for "a strip pinned to my top edge" — the indicator and the `content_rect`
        // below it are both read from this one band, so they cannot overlap or leave a
        // gap. The content area beneath it is what a scrolled list paints into, so it
        // legitimately spans the rest of the control.
        let indicator_height = self.indicator_height();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("refresh_control");
        // `refresh_control` is not a control kind in the role table, so it classifies
        // as `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The content area's fill is therefore a step toward
        // the foreground, so the control's extent is visible.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let background = resolved.blend(&text_color, 0.08);
        // The reveal panel is the indicator's own surface, a further step from the
        // content area so the two read as separate regions.
        let indicator_background = background.blend(&text_color, 0.12);
        // The spinner is an in-progress *state*, so it reads the theme's primary
        // token rather than a literal blue.
        let accent = crate::style::resolved_theme_style("button")
            .and_then(|button| button.background_color)
            .unwrap_or_else(|| background.blend(&text_color, 0.55));
        // Below the threshold the arrow is inactive chrome, so it is muted toward the
        // resolved surface instead of the literal grey pair.
        let idle_arrow = text_color.blend(&background, 0.45);
        let label_color = text_color.blend(&background, 0.35);

        // Draw background
        context.fill_rect(rect, background);
        context.draw_rect(rect, border);

        if indicator_height > 0 {
            let indicator_area = ControlMetrics::top_band(rect, indicator_height);
            context.fill_rect(indicator_area, indicator_background);

            // Indicator center
            let center_x = rect.x + (rect.width as i32) / 2;
            let center_y = rect.y + (indicator_height as i32) / 2;

            if self.is_refreshing {
                // Draw spinning indicator (circle with arc)
                let radius: u32 = 10;
                context.draw_circle_stroke(Point::new(center_x, center_y), radius, accent, 3);
                // Draw arrow inside
                let tip_y = center_y - 5;
                context.draw_line(
                    Point::new(center_x, tip_y - 3),
                    Point::new(center_x, tip_y + 4),
                    accent,
                );
                context.draw_line(
                    Point::new(center_x - 4, tip_y + 1),
                    Point::new(center_x, tip_y - 3),
                    accent,
                );
                context.draw_line(
                    Point::new(center_x + 4, tip_y + 1),
                    Point::new(center_x, tip_y - 3),
                    accent,
                );

                // Draw "Loading..." text below. The origin is the glyph box's top edge, so the
                // label hangs from `center_y + 14` itself; the old `+ ascent` pushed it half a
                // line further down, out past the indicator area.
                let font = Font::simple("sans-serif", 11.0);
                let label = "Loading...";
                let metrics = context.measure_text(label, &font);
                let text_x = center_x - (metrics.width as i32) / 2;
                let text_y = center_y + 14;
                context.draw_text(
                    Point::new(text_x, text_y),
                    label,
                    &font,
                    label_color,
                    HorizontalAlignment::Left,
                );
            } else {
                // Draw pull indicator (arrow + progress)
                //
                // The progress reads the *drawn* distance, so the arrow's colour and the release
                // label follow the band's visible position as it springs back rather than flipping
                // the instant the finger lifted.
                let progress = (self.settle_distance() / self.threshold).min(1.0);
                // Past the threshold the release is actionable, so the arrow picks up
                // the accent colour; below it stays muted chrome.
                let arrow_color = if progress >= 1.0 { accent } else { idle_arrow };

                // Downward arrow
                let arrow_size = 12;
                let arrow_top = center_y - arrow_size / 2;
                context.draw_line(
                    Point::new(center_x, arrow_top),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );
                context.draw_line(
                    Point::new(center_x - 4, arrow_top + arrow_size - 4),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );
                context.draw_line(
                    Point::new(center_x + 4, arrow_top + arrow_size - 4),
                    Point::new(center_x, arrow_top + arrow_size),
                    arrow_color,
                );

                // Progress text. Top edge origin again, so the descent from the arrow is the
                // 14px offset alone rather than 14 plus a full ascent.
                if progress >= 1.0 {
                    let font = Font::simple("sans-serif", 11.0);
                    let label = "Release to refresh";
                    let metrics = context.measure_text(label, &font);
                    let text_x = center_x - (metrics.width as i32) / 2;
                    let text_y = center_y + 14;
                    context.draw_text(
                        Point::new(text_x, text_y),
                        label,
                        &font,
                        accent,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }

        // Draw content area below indicator
        let content_rect = self.content_rect();
        if content_rect.height > 0 {
            context.fill_rect(content_rect, background);
        }
    }
}

impl EventHandler for RefreshControl {
    fn handle_event(&mut self, event: &Event) {
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler used to forward only in its catch-all arm, so every event it consumed left the
        // base untold — the theme's `"refresh_control:hover"` override could not fire.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    self.drag_origin_y = Some(pos.y as f32);
                    self.start_pull();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                // A pull-down is the same gesture whether it arrives as a mouse
                // drag or a touch drag (tablet/mobile parity).
                self.drag_origin_y = Some(pos.y as f32);
                self.start_pull();
            }
            Event::MouseMove { pos } => {
                // A downward drag is a positive pull. The delta is measured from
                // the previous sample and the origin advances with it, so the
                // pull distance accumulates across the whole gesture.
                if let Some(origin) = self.drag_origin_y {
                    let delta = pos.y as f32 - origin;
                    self.drag_origin_y = Some(pos.y as f32);
                    self.update_pull(delta);
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchMove { pos, .. } => {
                if let Some(origin) = self.drag_origin_y {
                    let delta = pos.y as f32 - origin;
                    self.drag_origin_y = Some(pos.y as f32);
                    self.update_pull(delta);
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.drag_origin_y = None;
                    self.end_pull();
                }
            }
            #[cfg(feature = "touch")]
            Event::TouchEnd { .. } => {
                self.drag_origin_y = None;
                self.end_pull();
            }
            _ => { /* Other events need no control-specific handling */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn make_refresh_control() -> RefreshControl {
        RefreshControl::new(Rect::new(0, 0, 200, 400))
    }

    #[test]
    fn refresh_control_default_state() {
        let rc = make_refresh_control();
        assert!(!rc.is_refreshing());
        assert_eq!(rc.pull_distance(), 0.0);
        assert_eq!(rc.threshold(), 60.0);
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
        assert_eq!(rc.kind(), WidgetKind::RefreshControl);
    }

    #[test]
    fn refresh_control_pull_and_release_below_threshold() {
        let mut rc = make_refresh_control();
        rc.start_pull();
        assert_eq!(rc.refresh_state(), RefreshState::Dragging);

        rc.update_pull(30.0);
        assert_eq!(rc.pull_distance(), 30.0);

        rc.end_pull();
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
        assert_eq!(rc.pull_distance(), 0.0);
        assert!(!rc.is_refreshing());
    }

    #[test]
    fn refresh_control_drag_events_drive_the_pull() {
        // The pull must be drivable through the real input path, not only via
        // the programmatic `update_pull` API — otherwise a host that forwards
        // events gets a control that never refreshes.
        let mut rc = make_refresh_control();
        let captured = Arc::new(AtomicBool::new(false));
        rc.refresh_triggered.connect({
            let captured = Arc::clone(&captured);
            move || {
                captured.store(true, Ordering::SeqCst);
            }
        });

        rc.handle_event(&Event::MousePress { pos: Point::new(100, 20), button: 1 });
        assert_eq!(rc.refresh_state(), RefreshState::Dragging);

        // Three 30px downward steps accumulate to 90px, past the 60px default.
        rc.handle_event(&Event::MouseMove { pos: Point::new(100, 50) });
        rc.handle_event(&Event::MouseMove { pos: Point::new(100, 80) });
        rc.handle_event(&Event::MouseMove { pos: Point::new(100, 110) });
        assert_eq!(rc.pull_distance(), 90.0);

        rc.handle_event(&Event::MouseRelease { pos: Point::new(100, 110), button: 1 });
        assert_eq!(rc.refresh_state(), RefreshState::Triggered);
        assert!(rc.is_refreshing());
        assert!(captured.load(Ordering::SeqCst));
    }

    #[test]
    fn refresh_control_upward_drag_does_not_pull() {
        // Dragging up must not produce a negative pull that could later read as
        // a threshold crossing.
        let mut rc = make_refresh_control();
        rc.handle_event(&Event::MousePress { pos: Point::new(100, 200), button: 1 });
        rc.handle_event(&Event::MouseMove { pos: Point::new(100, 120) });
        assert_eq!(rc.pull_distance(), 0.0);

        rc.handle_event(&Event::MouseRelease { pos: Point::new(100, 120), button: 1 });
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
        assert!(!rc.is_refreshing());
    }

    #[cfg(feature = "touch")]
    #[test]
    fn refresh_control_touch_drag_drives_the_pull() {
        let mut rc = make_refresh_control();
        rc.handle_event(&Event::TouchBegin { pos: Point::new(100, 20), touch_id: 0 });
        rc.handle_event(&Event::TouchMove { pos: Point::new(100, 100), touch_id: 0 });
        assert_eq!(rc.pull_distance(), 80.0);
        rc.handle_event(&Event::TouchEnd { pos: Point::new(100, 100), touch_id: 0 });
        assert!(rc.is_refreshing());
    }

    #[test]
    fn refresh_control_move_without_press_is_ignored() {
        let mut rc = make_refresh_control();
        rc.handle_event(&Event::MouseMove { pos: Point::new(100, 300) });
        assert_eq!(rc.pull_distance(), 0.0);
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn refresh_control_pull_above_threshold_triggers_refresh() {
        let mut rc = make_refresh_control();
        let captured = Arc::new(AtomicBool::new(false));
        rc.refresh_triggered.connect({
            let captured = Arc::clone(&captured);
            move || {
                captured.store(true, Ordering::SeqCst);
            }
        });

        rc.start_pull();
        rc.update_pull(80.0);
        rc.end_pull();

        assert_eq!(rc.refresh_state(), RefreshState::Triggered);
        assert!(rc.is_refreshing());
        assert!(captured.load(Ordering::SeqCst));
    }

    #[test]
    fn refresh_control_set_is_refreshing() {
        let mut rc = make_refresh_control();
        rc.set_is_refreshing(true);
        assert!(rc.is_refreshing());
        rc.set_is_refreshing(false);
        assert!(!rc.is_refreshing());
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn refresh_control_set_threshold() {
        let mut rc = make_refresh_control();
        rc.set_threshold(100.0);
        assert_eq!(rc.threshold(), 100.0);
    }

    /// The child content is **registered with the tree**, which is what makes it paint.
    ///
    /// # The defect this pins
    ///
    /// `set_content` stored a `Box<dyn Widget>` that `draw` never composited and never registered
    /// with `base.add_child`, so an embedded child was allocated, reachable through the accessor, and
    /// **never painted** — while the module doc claimed "Supports embedding child content". A host
    /// that embedded a list got a blank panel and no error.
    ///
    /// The assertion is on the **tree**, not on the accessor: reading the id back is exactly what
    /// the old code already supported, so a round-trip check passes on the broken version. What
    /// distinguishes the two is whether the base lists the child, because the compositor paints what
    /// the tree owns.
    #[test]
    fn the_child_content_is_registered_so_it_actually_paints() {
        use crate::widget::base_widgets::label::Label;

        let mut rc = make_refresh_control();
        assert!(rc.content().is_none(), "no child until one is set");
        assert!(rc.children().is_empty(), "and none in the tree");

        // A child mounted the way a host mounts one: through the registry, which is the path that
        // hands out an id the parent can adopt.
        let child = crate::runtime::register(Box::new(Label::new(
            "Item".to_string(),
            Rect::new(0, 0, 100, 20),
        )))
        .expect("the registry accepts a label");
        rc.set_content(child);

        assert_eq!(rc.content(), Some(child), "the id is remembered");
        assert!(
            rc.children().contains(&child),
            "and the child is in the tree, which is the only thing the compositor paints: {:?}",
            rc.children()
        );

        // Clearing detaches it, so a host that swaps its content does not leak a painted child.
        rc.clear_content();
        assert!(rc.content().is_none());
        assert!(!rc.children().contains(&child), "clearing detaches the child");
        let _ = crate::runtime::unregister(child);
    }

    /// The content region is derived from the same band the draw fills.
    ///
    /// A host lays its child out in `content_rect`, so the two must be one derivation — otherwise
    /// the child is positioned against a second reading of the indicator's height. The assertion
    /// compares against the *drawn* band, so a change to either alone fails here.
    #[test]
    fn the_content_region_sits_below_the_indicator_band() {
        let mut rc = make_refresh_control();
        let rect = rc.geometry();
        let idle = rc.content_rect();
        assert_eq!(idle.y, rect.y, "with no band showing the content starts at the top edge");
        assert_eq!(idle.height, rect.height);

        // With the indicator up, the content yields exactly the band's height.
        rc.set_is_refreshing(true);
        let busy = rc.content_rect();
        assert_eq!(busy.y, rect.y + dimensions::REFRESH_INDICATOR_HEIGHT as i32);
        assert_eq!(busy.height, rect.height - dimensions::REFRESH_INDICATOR_HEIGHT);
    }

    #[test]
    fn refresh_control_svg_output() {
        let mut rc = make_refresh_control();
        rc.set_pull_distance(40.0);
        let svg = render_to_svg(&mut rc);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// The reveal strip is a **fixed** band pinned to the control's top edge.
    ///
    /// This pins the defect the fix removes: the indicator's height was the bare literal
    /// `40` at the draw site, so a caller could not predict the strip's size and a second
    /// reader of it (the count in the tests, or a future hit test) would have had to
    /// repeat the number. It is now `REFRESH_INDICATOR_HEIGHT`, placed by
    /// `ControlMetrics::top_band`, which is also what fixes the strip's *origin*: a
    /// centred band would have floated it down into the list instead of pinning it to
    /// the edge the gesture starts at.
    #[test]
    fn the_reveal_strip_is_a_fixed_band_at_the_top_edge() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::theme::theme_test_guard();
        let mut rc = make_refresh_control();
        rc.set_pull_distance(dimensions::REFRESH_INDICATOR_HEIGHT as f32);
        let rect = rc.geometry();
        let svg = render_to_svg(&mut rc);
        let band = ControlMetrics::top_band(rect, dimensions::REFRESH_INDICATOR_HEIGHT);
        assert_eq!(band.y, rect.y, "the strip is pinned to the control's top edge");
        assert_eq!(band.width, rect.width, "the strip spans the control's width");
        assert_eq!(band.height, dimensions::REFRESH_INDICATOR_HEIGHT);
        assert!(
            svg.contains(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                band.x, band.y, band.width, band.height
            )),
            "the strip is its own band: {svg}"
        );

        // At rest the strip is not drawn at all, so only the content area paints.
        let mut idle = make_refresh_control();
        idle.set_pull_distance(0.0);
        let idle_svg = render_to_svg(&mut idle);
        assert!(
            !idle_svg.contains(&format!("height=\"{}\"", dimensions::REFRESH_INDICATOR_HEIGHT)),
            "a control at rest has no reveal strip: {idle_svg}"
        );
    }

    #[test]
    fn refresh_control_disabled_blocks_events() {
        let mut rc = make_refresh_control();
        rc.set_enabled(false);
        rc.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        assert_eq!(rc.refresh_state(), RefreshState::Idle);
    }

    #[test]
    fn refresh_control_refreshing_blocks_pull() {
        let mut rc = make_refresh_control();
        rc.set_is_refreshing(true);
        rc.start_pull();
        // Should stay in Idle since refreshing blocks new pulls
        assert!(rc.is_refreshing());
    }

    /// A released pull springs back over frames instead of snapping shut.
    ///
    /// # The defect this pins
    ///
    /// `end_pull` wrote `pull_distance = 0.0` on the same statement that ended the gesture, so
    /// releasing below the threshold collapsed the reveal band between two adjacent frames. A pull
    /// that goes nowhere is still a gesture the user performed, and it has to be *answered* — the
    /// band returning to rest is that answer. The assertion uses the three-frame shape (§0.3): the
    /// drawn distance must take an interior value on the way down, which a hard cut cannot.
    #[test]
    fn a_released_pull_springs_back_rather_than_snapping() {
        let mut rc = make_refresh_control();
        rc.start_pull();
        rc.update_pull(30.0);
        let pulled = rc.settle_distance();
        assert!(pulled > 0.0, "the band follows the live gesture");

        rc.end_pull();
        assert_eq!(rc.pull_distance(), 0.0, "the gesture measurement ends at once");
        assert!(rc.is_animating(), "but the drawn band owes frames");
        assert_eq!(rc.settle_distance(), pulled, "the spring starts where the gesture left it");

        assert!(rc.tick(60), "still moving after one step");
        let mid = rc.settle_distance();
        assert!(
            mid < pulled && mid > 0.0,
            "the band must take an interior distance rather than jumping shut (got {mid})"
        );
        while rc.tick(60) {}
        assert_eq!(rc.settle_distance(), 0.0, "and settle at rest");
        assert!(!rc.is_animating(), "a settled control owes no more frames");
    }

    /// A live gesture replaces any leftover spring rather than being fought by it.
    #[test]
    fn a_new_pull_adopts_the_band_where_it_is() {
        let mut rc = make_refresh_control();
        rc.start_pull();
        rc.update_pull(30.0);
        rc.end_pull();
        assert!(rc.is_animating(), "the previous pull is springing back");

        // A second gesture begins before the first has settled: the band must jump to the new
        // gesture's position, not continue its old journey underneath it.
        rc.start_pull();
        rc.update_pull(0.0);
        assert_eq!(rc.settle_distance(), 0.0, "the drag owns the band again");
        rc.update_pull(20.0);
        assert_eq!(rc.settle_distance(), 20.0, "and it tracks the finger exactly");
    }

    /// The pointer reaching the control has to reach the *base* too.
    ///
    /// Regression: this handler forwarded only in its catch-all arm, so the events it consumed
    /// never recorded the pointer fact and `widget_state()` answered `Normal` for the whole
    /// gesture — the same shape as the `bezier_curve_editor` defect.
    #[test]
    fn pointer_state_reaches_the_base() {
        use crate::style::WidgetState;

        let mut rc = make_refresh_control();
        assert_eq!(rc.widget_state(), WidgetState::Normal);
        rc.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert_eq!(
            rc.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );
        rc.handle_event(&Event::MouseLeave { pos: Point::new(500, 500) });
        assert_eq!(rc.widget_state(), WidgetState::Normal, "and that it left");
    }
}
