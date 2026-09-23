// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! HeroAnimation widget — shared element transition (hero animation).
//!
//! The HeroAnimation widget interpolates between a source and target widget
//! over a configurable duration, providing a smooth visual transition similar
//! to Android Shared Element Transitions or iOS Hero animations. It supports
//! position, size, and opacity interpolation based on a progress value from
//! 0.0 (source) to 1.0 (target).

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_f32;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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
    /// Emitted when the animation completes.
    pub animation_completed: GenericSignal,
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
            animation_completed: GenericSignal::new(),
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
        self.base.request_redraw();
    }

    /// Sets the target widget (the "to" state).
    pub fn set_target(&mut self, widget: Box<dyn Widget>) {
        self.target_widget = Some(widget);
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
        let progress = (self.elapsed_ms as f32 / self.duration_ms as f32).min(1.0);
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
        let themed = crate::style::resolved_theme_style("hero_animation");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let themed_text = themed.as_ref().and_then(|resolved| resolved.text_color);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(240, 240, 240, 255));
        let shell_border = style.border_color.or(themed_border).unwrap_or(Color::rgba(0, 0, 0, 40));
        // The ink is the theme's foreground, with `.contrast_color()` as the second choice so
        // it stays readable on whichever surface the theme painted; only then the literal.
        let ink = style
            .text_color
            .or(themed_text)
            .or_else(|| style.background_color.or(themed_bg).map(|bg| bg.contrast_color()))
            .unwrap_or(Color::rgba(160, 160, 160, 220));

        // The outer shell is this widget's chrome. The former literals are the fallbacks, and
        // the disabled state is now *derived* from the resolved colour rather than being a
        // second fixed grey, so the two states stay distinguishable in any theme.
        let bg = if !is_enabled { base_bg.blend(&Color::WHITE, 0.35) } else { base_bg };
        context.fill_rect(rect, bg);

        let src = self.source_widget.as_ref();
        let tgt = self.target_widget.as_ref();

        if src.is_none() && tgt.is_none() {
            // No widgets configured: draw placeholder. The message is this widget's own
            // chrome — a hint about missing configuration — so it follows the theme.
            let font = crate::core::Font::default();
            // Two lines, each fitted to the control's width, and each centred on the vertical
            // midline as a pair. The previous single call passed a two-line string with one
            // origin: the renderer draws **one** row per call, so the origin was computed
            // from the *combined* width of both lines and the `\n` was painted as a missing
            // glyph. That is why the label started at a negative x and the second line never
            // appeared.
            let lines = ["HeroAnimation", "Set source & target"];
            let metrics = context.measure_text("Hg", &font);
            let first_y = rect.y + (rect.height as i32 - metrics.height as i32 * 2) / 2;
            for (index, line) in lines.iter().enumerate() {
                context.draw_text_fitted(
                    Rect::new(
                        rect.x,
                        first_y + index as i32 * metrics.height as i32,
                        rect.width,
                        metrics.height,
                    ),
                    line,
                    &font,
                    ink,
                    HorizontalAlignment::Center,
                );
            }
            return;
        }

        // Draw the interpolated transition effect.
        let t = self.animation_progress;
        let src_rect = src.map(|w| w.geometry()).unwrap_or(rect);
        let tgt_rect = tgt.map(|w| w.geometry()).unwrap_or(rect);

        // Interpolated rectangle.
        let ix = src_rect.x as f32 + (tgt_rect.x as f32 - src_rect.x as f32) * t;
        let iy = src_rect.y as f32 + (tgt_rect.y as f32 - src_rect.y as f32) * t;
        let iw = src_rect.width as f32 + (tgt_rect.width as f32 - src_rect.width as f32) * t;
        let ih = src_rect.height as f32 + (tgt_rect.height as f32 - src_rect.height as f32) * t;

        let interp_rect = Rect::new(ix as i32, iy as i32, iw as u32, ih as u32);

        // Color interpolates from a "source" blue to a "target" green.
        //
        // Deliberately not themed: these are *data*, not chrome. The widget's whole subject
        // is "animate from this colour to that colour", so recovering them from a palette
        // would erase the demonstration — and a theme whose two tokens coincided would make
        // the interpolation invisible.
        let src_color = Color::rgb(33, 118, 210); // Material blue
        let tgt_color = Color::rgb(76, 175, 80); // Material green
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

        // Draw the progress indicator label. The label is chrome, so it uses the resolved
        // foreground instead of the fixed mid-grey that made it unreadable on a dark shell.
        let progress_text = format!("Progress: {:.0}%", t * 100.0);
        let font = crate::core::Font::default();
        let metrics = context.measure_text(&progress_text, &font);
        let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
        let text_y = rect.y + rect.height as i32 - 10;
        context.draw_text(
            Point::new(text_x, text_y),
            &progress_text,
            &font,
            ink,
            HorizontalAlignment::Left,
        );

        // Draw source/target labels. These are the endpoint colours of the transition shown
        // above — "source blue" and "target green" — so like those they are data, and the
        // label must match the colour it names rather than the theme's. The label strip is
        // bounded by the control, so a narrow hero truncates it rather than overhanging.
        let label_band = Rect::new(rect.x + 4, rect.y + 4, rect.width.saturating_sub(8), 14);
        let label = match (src.is_some(), tgt.is_some()) {
            (true, false) => Some(("Source", Color::rgba(33, 118, 210, 200))),
            (false, true) => Some(("Target", Color::rgba(76, 175, 80, 200))),
            (true, true) if t < 0.5 => {
                Some(("Source \u{2192} Target", Color::rgba(33, 118, 210, 200)))
            }
            (true, true) => Some(("Source \u{2192} Target", Color::rgba(76, 175, 80, 200))),
            (false, false) => None,
        };
        if let Some((text, color)) = label {
            context.draw_text_fitted(label_band, text, &font, color, HorizontalAlignment::Left);
        }
    }
}

impl EventHandler for HeroAnimation {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } | Event::MouseRelease { pos, button } => {
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
        ha.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
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
}
