// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HeroAnimation widget — shared element transition (hero animation).
//!
//! The HeroAnimation widget interpolates between a source and target widget
//! over a configurable duration, providing a smooth visual transition similar
//! to Android Shared Element Transitions or iOS Hero animations. It supports
//! position, size, and opacity interpolation based on a progress value from
//! 0.0 (source) to 1.0 (target).

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A rendered endpoint, captured once so the transition never mutates child geometry.
///
/// The old draw path called `set_geometry(interp_rect)` on the endpoint, painted it, then restored
/// the old rectangle: a draw function with a side effect. The snapshot is the endpoint's own
/// picture at its own rectangle; the transition scales and fades that image instead.
struct EndpointSnapshot {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// HeroAnimation widget for shared element transitions.
pub struct HeroAnimation {
    base: BaseWidget,
    source_widget: Option<Box<dyn Widget>>,
    target_widget: Option<Box<dyn Widget>>,
    /// Animation progress from 0.0 (source) to 1.0 (target).
    animation_progress: f32,
    is_animating: bool,
    /// Duration of the animation in milliseconds.
    duration_ms: u64,
    /// Elapsed time in milliseconds.
    elapsed_ms: u64,
    /// An optional easing curve applied to the elapsed fraction.
    ///
    /// `None` keeps the historical linear advance, so the existing frame-by-frame expectations
    /// are unchanged. A curve only changes the intermediate frames; both endpoints are exact.
    easing: Option<crate::style::EasingFunction>,
    /// An explicit source rectangle for the transition, overriding the source widget's own
    /// geometry. This is how a navigation host drives a hero from the rect a control had on
    /// the previous page without owning that control.
    source_rect: Option<Rect>,
    /// An explicit target rectangle, the counterpart of [`Self::source_rect`].
    target_rect: Option<Rect>,
    /// Emitted when the animation completes.
    pub animation_completed: GenericSignal,
    /// Colour interpolated from at progress 0, when the caller wants an explicit pair.
    source_tint: Color,
    /// Colour interpolated to at progress 1.
    target_tint: Color,
    /// The progress caption, rebuilt when progress changes instead of every draw.
    progress_label: String,
    /// Cached picture of the source endpoint, at its own geometry.
    source_snapshot: Option<EndpointSnapshot>,
    /// Cached picture of the target endpoint, at its own geometry.
    target_snapshot: Option<EndpointSnapshot>,
}

impl HeroAnimation {
    /// Creates a new HeroAnimation with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::HeroAnimation, geometry, "HeroAnimation"),
            source_widget: None,
            target_widget: None,
            animation_progress: 0.0,
            is_animating: false,
            duration_ms: 300,
            elapsed_ms: 0,
            easing: None,
            source_rect: None,
            target_rect: None,
            animation_completed: GenericSignal::new(),
            source_tint: Color::rgb(33, 118, 210),
            target_tint: Color::rgb(76, 175, 80),
            progress_label: "Progress: 0%".to_string(),
            source_snapshot: None,
            target_snapshot: None,
        }
    }

    /// Starts the animation from progress 0.0 through 1.0 over the configured duration.
    pub fn start_animation(&mut self) {
        if self.source_widget.is_none() || self.target_widget.is_none() {
            return;
        }
        self.animation_progress = 0.0;
        self.elapsed_ms = 0;
        self.is_animating = true;
        self.progress_label = "Progress: 0%".to_string();
        self.refresh_snapshots();
        self.base.request_redraw();
    }

    /// Re-captures both endpoints.
    ///
    /// A snapshot is a picture of the endpoint at the moment it was taken, so a host that changes
    /// a child control's appearance during a paused transition calls this to refresh it.
    pub fn refresh_snapshots(&mut self) {
        self.source_snapshot = self.source_widget.as_mut().and_then(capture_endpoint);
        self.target_snapshot = self.target_widget.as_mut().and_then(capture_endpoint);
    }

    /// The colour interpolated at progress 0.
    pub fn source_tint(&self) -> Color {
        self.source_tint
    }

    /// The colour interpolated at progress 1.
    pub fn target_tint(&self) -> Color {
        self.target_tint
    }

    /// Sets the endpoint colours. Nothing is emitted; the next frame uses them.
    pub fn set_tints(&mut self, source: Color, target: Color) {
        if self.source_tint == source && self.target_tint == target {
            return;
        }
        self.source_tint = source;
        self.target_tint = target;
        self.base.request_redraw();
    }

    /// Stops the animation at its current progress.
    pub fn stop_animation(&mut self) {
        self.is_animating = false;
        self.base.request_redraw();
    }

    /// Sets the animation progress manually (clamped to 0.0–1.0).
    /// When progress reaches 1.0, the animation is marked as complete.
    pub fn set_progress(&mut self, progress: f32) {
        let clamped = progress.clamp(0.0, 1.0);
        if (self.animation_progress - clamped).abs() > 0.001 {
            self.animation_progress = clamped;
            self.progress_label = format!("Progress: {:.0}%", clamped * 100.0);
            if (clamped - 1.0).abs() < 0.001 {
                self.is_animating = false;
                self.animation_completed.emit();
            }
            self.base.request_redraw();
        }
    }

    /// Returns the current animation progress (0.0–1.0).
    pub fn progress(&self) -> f32 {
        self.animation_progress
    }

    /// Returns whether the animation is currently running.
    pub fn is_animating(&self) -> bool {
        self.is_animating
    }

    /// Sets the source widget (the "from" state).
    pub fn set_source(&mut self, widget: Box<dyn Widget>) {
        self.source_widget = Some(widget);
        self.source_snapshot = None;
        self.base.request_redraw();
    }

    /// Sets the target widget (the "to" state).
    pub fn set_target(&mut self, widget: Box<dyn Widget>) {
        self.target_widget = Some(widget);
        self.target_snapshot = None;
        self.base.request_redraw();
    }

    /// Returns a reference to the source widget, if set.
    pub fn source(&self) -> Option<&dyn Widget> {
        self.source_widget.as_deref()
    }

    /// Returns a reference to the target widget, if set.
    pub fn target(&self) -> Option<&dyn Widget> {
        self.target_widget.as_deref()
    }

    /// Sets the animation duration in milliseconds.
    pub fn set_duration(&mut self, ms: u64) {
        self.duration_ms = ms.max(1);
    }

    /// Sets the easing curve applied to the elapsed fraction, or `None` for linear motion.
    pub fn set_easing(&mut self, easing: Option<crate::style::EasingFunction>) {
        self.easing = easing;
    }

    /// Returns the easing curve, when one is set.
    pub fn easing(&self) -> Option<crate::style::EasingFunction> {
        self.easing
    }

    /// Sets explicit endpoint rectangles for the transition.
    ///
    /// A host that knows where the shared element was and where it will land (a navigation
    /// push, a list-to-detail transition) can state both facts here without moving the source
    /// and target widgets themselves. Passing `None` returns to reading each widget's own
    /// geometry.
    pub fn set_transition_rects(&mut self, source: Option<Rect>, target: Option<Rect>) {
        if self.source_rect == source && self.target_rect == target {
            return;
        }
        self.source_rect = source;
        self.target_rect = target;
        self.source_snapshot = None;
        self.target_snapshot = None;
        self.base.request_redraw();
    }

    /// The explicit source rectangle, when one is set.
    pub fn source_rect(&self) -> Option<Rect> {
        self.source_rect
    }

    /// The explicit target rectangle, when one is set.
    pub fn target_rect(&self) -> Option<Rect> {
        self.target_rect
    }

    /// Returns the animation duration in milliseconds.
    pub fn duration(&self) -> u64 {
        self.duration_ms
    }

    /// Advances the animation by the given number of milliseconds.
    /// Returns true if the animation is still running after the tick.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.is_animating {
            return false;
        }
        self.elapsed_ms += delta_ms;
        let fraction = (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0);
        let progress = match self.easing {
            Some(curve) => curve.apply(fraction),
            None => fraction,
        };
        self.set_progress(progress);
        self.is_animating
    }

    /// Returns the interpolated opacity (1.0 at endpoints, may dip in middle for cross-fade).
    fn interpolated_opacity(&self) -> f32 {
        let t = self.animation_progress;
        // Simple cross-fade: source fades out, target fades in.
        if t < 0.5 {
            1.0 - t * 2.0 // source visibility: 1.0 -> 0.0
        } else {
            (t - 0.5) * 2.0 // target visibility: 0.0 -> 1.0
        }
    }
}

impl Widget for HeroAnimation {
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
        crate::core::Size::new(200, 200)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `HeroAnimation` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // Trait contract: `u32` ms. This control's own `tick` takes `u64`; widen at the boundary.
    // `HeroAnimation` already answers `is_animating` itself, so only the tick needs bridging.
    fn tick(&mut self, delta_ms: u32) -> bool {
        HeroAnimation::tick(self, u64::from(delta_ms))
    }

    fn is_animating(&self) -> bool {
        HeroAnimation::is_animating(self)
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "animation_completed" => {
                Some(EventSignalRef::unit("animation_completed", &self.animation_completed))
            }
            _ => None,
        }
    }
}

/// `HeroAnimation`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch.
impl WidgetProperties for HeroAnimation {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "animation_progress" => Ok(CapabilityValue::Float(self.progress() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "animation_progress" => {
                self.set_progress(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["animation_progress", BASE_PROPERTY_NAMES]
    }
}

impl Draw for HeroAnimation {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to the original literal. The literal step is
        // kept deliberately: an inactive theme must still give the widget a defined
        // appearance, and the value is the one this widget painted before, so an existing
        // pixel baseline cannot move. Resolved once per draw, because the shell, the label,
        // the badge and the outline all read from it and re-resolving would take the theme
        // lock several times inside one draw.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style_shared("hero_animation");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let themed_text = themed.as_ref().and_then(|resolved| resolved.text_color);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(240, 240, 240, 255));
        // The page this shell sits in, read once. A disabled **fill** recedes toward the page it
        // sits on; it cannot recede toward its own contrast colour the way ink does, because that
        // would make it stand out *more* against the page -- the opposite of disabled.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let shell_border = style.border_color.or(themed_border).unwrap_or(Color::rgba(0, 0, 0, 40));
        // The ink is the theme's foreground, with `.contrast_color()` as the second choice so
        // it stays readable on whichever surface the theme painted; only then the literal.
        let ink = style
            .text_color
            .or(themed_text)
            .or_else(|| style.background_color.or(themed_bg).map(|bg| bg.contrast_color()))
            .unwrap_or(Color::rgba(160, 160, 160, 220));

        // The outer shell is this widget's chrome. The former literals are the fallbacks. The
        // disabled state steps the resolved fill toward its own contrast colour rather than
        // toward a fixed white, which on a dark surface made the disabled state *more* prominent
        // than the enabled one.
        let bg = if !is_enabled {
            self.base.disabled_surface_near(base_bg, window_fill)
        } else {
            base_bg
        };
        context.face_with_gradient(
            rect,
            bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        let src = self.source_widget.as_ref();
        let tgt = self.target_widget.as_ref();
        let has_src = src.is_some();
        let has_tgt = tgt.is_some();

        if !has_src && !has_tgt {
            // No widgets configured: draw placeholder. The message is this widget's own
            // chrome — a hint about missing configuration — so it follows the theme.
            let font = effective_font(&style);
            // Two lines, each fitted to the control's width, and each centred on the vertical
            // midline as a pair. The previous single call passed a two-line string with one
            // origin: the renderer draws **one** row per call, so the origin was computed
            // from the *combined* width of both lines and the `\n` was painted as a missing
            // glyph. That is why the label started at a negative x and the second line never
            // appeared.
            let lines = ["HeroAnimation", "Set source & target"];
            let metrics = context.measure_text("Hg", font);
            let first_y = rect.y + (rect.height as i32 - metrics.height as i32 * 2) / 2;
            for (index, line) in lines.iter().enumerate() {
                context.draw_text_fitted_cow(
                    Rect::new(
                        rect.x,
                        first_y + index as i32 * metrics.height as i32,
                        rect.width,
                        metrics.height,
                    ),
                    line,
                    font,
                    ink,
                    HorizontalAlignment::Center,
                );
            }
            return;
        }

        // Draw the interpolated transition effect.
        let t = self.animation_progress;
        let src_rect = self.source_rect.or_else(|| src.map(|w| w.geometry())).unwrap_or(rect);
        let tgt_rect = self.target_rect.or_else(|| tgt.map(|w| w.geometry())).unwrap_or(rect);

        // Interpolated rectangle.
        let ix = src_rect.x as f32 + (tgt_rect.x as f32 - src_rect.x as f32) * t;
        let iy = src_rect.y as f32 + (tgt_rect.y as f32 - src_rect.y as f32) * t;
        let iw = src_rect.width as f32 + (tgt_rect.width as f32 - src_rect.width as f32) * t;
        let ih = src_rect.height as f32 + (tgt_rect.height as f32 - src_rect.height as f32) * t;

        let interp_rect = Rect::new(ix as i32, iy as i32, iw as u32, ih as u32);

        // Colour interpolates between the caller's endpoint tints. The defaults preserve the
        // Material blue/green this control always used; a host can state its own pair so the
        // transition names the colours of the content it moves.
        let src_color = self.source_tint;
        let tgt_color = self.target_tint;
        let r = src_color.r as f32 + (tgt_color.r as f32 - src_color.r as f32) * t;
        let g = src_color.g as f32 + (tgt_color.g as f32 - src_color.g as f32) * t;
        let b = src_color.b as f32 + (tgt_color.b as f32 - src_color.b as f32) * t;
        let interp_color = Color::rgb(r as u8, g as u8, b as u8);

        // Draw the interpolated element with opacity.
        let opacity = self.interpolated_opacity();
        let fade_color = Color::rgba(
            interp_color.r,
            interp_color.g,
            interp_color.b,
            (interp_color.a as f32 * opacity) as u8,
        );

        context.fill_rounded_rect(interp_rect, 8, fade_color);
        // The outline around the interpolated element is chrome, so it takes the resolved
        // theme border; the fill above keeps its interpolated data colour.
        context.draw_rounded_rect_stroke(interp_rect, 8, shell_border, 1);

        // The endpoint that owns this half of the transition is painted into the interpolated
        // box. This is what makes the control a shared-element transition rather than a
        // coloured rectangle: the source's or target's own appearance travels with the rect.
        //
        // The endpoint is **not** resized here. Its picture is captured once into an offscreen
        // buffer and that image is scaled into `interp_rect`, with the two endpoints cross-fading
        // through a layer group instead of the old hard switch at `t < 0.5`.
        if self.source_snapshot.is_none() && has_src {
            self.source_snapshot = self.source_widget.as_mut().and_then(capture_endpoint);
        }
        if self.target_snapshot.is_none() && has_tgt {
            self.target_snapshot = self.target_widget.as_mut().and_then(capture_endpoint);
        }
        if let Some(snapshot) = self.source_snapshot.as_ref() {
            draw_snapshot(context, snapshot, interp_rect, 1.0 - t);
        }
        if let Some(snapshot) = self.target_snapshot.as_ref() {
            draw_snapshot(context, snapshot, interp_rect, t);
        }

        // Draw the progress indicator label. The label is chrome, so it uses the resolved
        // foreground instead of the fixed mid-grey that made it unreadable on a dark shell. The
        // text is rebuilt when progress changes, not in this paint path.
        let progress_text = &self.progress_label;
        let font = effective_font(&style);
        let metrics = context.measure_text(progress_text, font);
        let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
        let text_y = rect.y + rect.height as i32 - 10;
        context.draw_text(
            Point::new(text_x, text_y),
            progress_text,
            font,
            ink,
            HorizontalAlignment::Left,
        );

        // Draw source/target labels. These are the endpoint colours of the transition shown
        // above — "source blue" and "target green" — so like those they are data, and the
        // label must match the colour it names rather than the theme's. The label strip is
        // bounded by the control, so a narrow hero truncates it rather than overhanging.
        let label_band = Rect::new(rect.x + 4, rect.y + 4, rect.width.saturating_sub(8), 14);
        let label = match (has_src, has_tgt) {
            (true, false) => Some(("Source", self.source_tint.with_alpha(200))),
            (false, true) => Some(("Target", self.target_tint.with_alpha(200))),
            (true, true) if t < 0.5 => {
                Some(("Source \u{2192} Target", self.source_tint.with_alpha(200)))
            }
            (true, true) => Some(("Source \u{2192} Target", self.target_tint.with_alpha(200))),
            (false, false) => None,
        };
        if let Some((text, color)) = label {
            context.draw_text_fitted_cow(label_band, text, font, color, HorizontalAlignment::Left);
        }
    }
}

/// Renders `endpoint` at its own geometry into an offscreen RGBA buffer.
///
/// The endpoint is drawn where it actually is: the context is translated so its top-left lands at
/// the buffer's origin, and `set_geometry` is never called. A widget that cannot draw (an endpoint
/// with no drawable) yields `None`, and the transition falls back to its interpolated fill.
fn capture_endpoint(endpoint: &mut Box<dyn Widget>) -> Option<EndpointSnapshot> {
    let rect = endpoint.geometry();
    if rect.width == 0 || rect.height == 0 {
        return None;
    }
    let size = Size::new(rect.width, rect.height);
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    backend.begin_frame(Color::rgba(0, 0, 0, 0));
    {
        let mut context = RenderContext::new(&mut backend);
        context.push_offset(-rect.x, -rect.y);
        if let Some(drawable) = crate::widget::draw_bridge::draw_of(endpoint.as_mut()) {
            drawable.draw(&mut context);
        }
        context.pop_offset();
    }
    backend.end_frame();
    Some(EndpointSnapshot {
        width: rect.width,
        height: rect.height,
        data: backend.frame_rgba().to_vec(),
    })
}

/// Paints a captured endpoint, scaled to `dest`, through a composited layer at `opacity`.
fn draw_snapshot(
    context: &mut RenderContext,
    snapshot: &EndpointSnapshot,
    dest: Rect,
    opacity: f32,
) {
    if dest.width == 0 || dest.height == 0 || opacity <= 0.0 {
        return;
    }
    let scaled;
    let data: &[u8] = if dest.width == snapshot.width && dest.height == snapshot.height {
        &snapshot.data
    } else {
        scaled =
            scale_rgba(&snapshot.data, snapshot.width, snapshot.height, dest.width, dest.height);
        &scaled
    };
    context.push_layer_group(dest, None, opacity.clamp(0.0, 1.0), None);
    context.draw_image(dest.x, dest.y, dest.width, dest.height, data);
    context.pop_layer();
}

/// Nearest-neighbour resample of an RGBA buffer into `width` x `height`.
fn scale_rgba(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut out = vec![0u8; (width as usize) * (height as usize) * 4];
    if source_width == 0 || source_height == 0 || width == 0 || height == 0 {
        return out;
    }
    for y in 0..height {
        let source_y = (y as u64 * source_height as u64 / height as u64) as u32;
        for x in 0..width {
            let source_x = (x as u64 * source_width as u64 / width as u64) as u32;
            let source = ((source_y as usize * source_width as usize + source_x as usize) * 4)
                .min(data.len().saturating_sub(4));
            let destination = (y as usize * width as usize + x as usize) * 4;
            out[destination..destination + 4].copy_from_slice(&data[source..source + 4]);
        }
    }
    out
}

impl EventHandler for HeroAnimation {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } | Event::MouseRelease { pos, button } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.is_animating {
                        self.stop_animation();
                    } else {
                        self.start_animation();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    #[test]
    fn hero_animation_creation_defaults() {
        let ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        assert_eq!(ha.progress(), 0.0);
        assert!(!ha.is_animating());
        assert_eq!(ha.duration(), 300);
        assert!(ha.source().is_none());
        assert!(ha.target().is_none());
        assert_eq!(ha.kind(), WidgetKind::HeroAnimation);
    }

    #[test]
    fn hero_animation_set_source_and_target() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        let src_rect = Rect::new(0, 0, 50, 50);
        let tgt_rect = Rect::new(200, 100, 100, 80);
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            src_rect,
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            tgt_rect,
        )));
        assert!(ha.source().is_some());
        assert!(ha.target().is_some());
        assert_eq!(ha.source().unwrap().geometry(), src_rect);
        assert_eq!(ha.target().unwrap().geometry(), tgt_rect);
    }

    #[test]
    fn hero_animation_start_stop() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));

        assert!(!ha.is_animating());
        ha.start_animation();
        assert!(ha.is_animating());
        ha.stop_animation();
        assert!(!ha.is_animating());
    }

    #[test]
    fn hero_animation_start_without_widgets_does_nothing() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.start_animation();
        assert!(!ha.is_animating()); // no source+target, won't start
    }

    #[test]
    fn hero_animation_set_progress_clamps() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_progress(0.5);
        assert!((ha.progress() - 0.5).abs() < 0.001);

        ha.set_progress(1.5);
        assert!((ha.progress() - 1.0).abs() < 0.001);

        ha.set_progress(-0.5);
        assert!((ha.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn hero_animation_progress_completes_animation() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));

        let completed = Arc::new(Mutex::new(false));
        ha.animation_completed.connect({
            let completed = Arc::clone(&completed);
            move || {
                *completed.lock().unwrap() = true;
            }
        });

        ha.set_progress(1.0);
        assert!(!ha.is_animating());
        assert!(*completed.lock().unwrap());
    }

    #[test]
    fn hero_animation_tick() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        ha.start_animation();

        // Advance by 150ms (half of 300ms).
        let still_running = ha.tick(150);
        assert!(still_running);
        assert!((ha.progress() - 0.5).abs() < 0.01);

        // Advance by another 150ms to complete.
        let still_running = ha.tick(150);
        assert!(!still_running);
        assert!((ha.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn hero_animation_tick_not_started_does_nothing() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        let still_running = ha.tick(100);
        assert!(!still_running);
        assert!((ha.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn hero_animation_set_duration() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        assert_eq!(ha.duration(), 300);
        ha.set_duration(500);
        assert_eq!(ha.duration(), 500);
        ha.set_duration(0); // clamps to 1
        assert_eq!(ha.duration(), 1);
    }

    #[test]
    fn hero_animation_interpolated_opacity() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        assert!((ha.interpolated_opacity() - 1.0).abs() < 0.001); // at 0.0

        ha.set_progress(0.25);
        assert!((ha.interpolated_opacity() - 0.5).abs() < 0.01);

        ha.set_progress(0.5);
        assert!((ha.interpolated_opacity() - 0.0).abs() < 0.01);

        ha.set_progress(0.75);
        assert!((ha.interpolated_opacity() - 0.5).abs() < 0.01);

        ha.set_progress(1.0);
        assert!((ha.interpolated_opacity() - 1.0).abs() < 0.001);
    }

    #[test]
    fn hero_animation_disabled_blocks_events() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        ha.set_enabled(false);
        ha.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 });
        assert!(!ha.is_animating());
    }

    #[test]
    fn hero_animation_svg_output() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 100, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 200, 100),
        )));
        ha.set_progress(0.3);
        let svg = crate::widget::svg::render_to_svg(&mut ha);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn an_optional_curve_eases_the_movement_without_moving_the_endpoints() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        ha.set_easing(Some(crate::style::EasingFunction::EaseIn));
        ha.set_duration(100);
        ha.start_animation();
        ha.tick(50);
        assert!(ha.progress() < 0.5, "ease-in lags the linear fraction");
        ha.tick(50);
        assert!((ha.progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn explicit_endpoint_rects_drive_the_transition() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_transition_rects(Some(Rect::new(10, 10, 20, 20)), Some(Rect::new(100, 100, 80, 80)));
        assert_eq!(ha.source_rect(), Some(Rect::new(10, 10, 20, 20)));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        let svg = crate::widget::svg::render_to_svg(&mut ha);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn drawing_a_transition_does_not_move_the_endpoints() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        ha.set_source(Box::new(crate::widget::base_widgets::button::Button::new(
            "Src".to_string(),
            Rect::new(0, 0, 50, 50),
        )));
        ha.set_target(Box::new(crate::widget::base_widgets::button::Button::new(
            "Tgt".to_string(),
            Rect::new(200, 100, 100, 80),
        )));
        ha.set_progress(0.3);
        let source_before = ha.source().expect("source").geometry();
        let target_before = ha.target().expect("target").geometry();
        let _ = crate::widget::svg::render_to_svg(&mut ha);
        assert_eq!(ha.source().expect("source").geometry(), source_before);
        assert_eq!(ha.target().expect("target").geometry(), target_before);
    }

    #[test]
    fn endpoint_tints_and_cached_label_are_settings_not_paint_work() {
        let mut ha = HeroAnimation::new(Rect::new(0, 0, 300, 200));
        let blue = Color::rgb(10, 20, 30);
        let red = Color::rgb(200, 10, 10);
        ha.set_tints(blue, red);
        assert_eq!(ha.source_tint(), blue);
        assert_eq!(ha.target_tint(), red);
        ha.set_progress(0.5);
        assert_eq!(ha.progress_label, "Progress: 50%");
    }
}
