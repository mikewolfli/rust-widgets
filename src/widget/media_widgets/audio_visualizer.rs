// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AudioVisualizer widget — real-time audio waveform/spectrum visualization.
//!
//! Displays vertical bars representing audio frequency bands or waveform samples.
//! Supports mirror mode (bottom half mirrors top), peak hold indicators, and
//! configurable bar count and spacing.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Audio waveform/spectrum visualization widget.
///
/// Renders vertical bars representing audio samples (normalized -1.0 to 1.0).
/// Supports mirroring, peak hold, and configurable appearance.
pub struct AudioVisualizer {
    base: BaseWidget,
    /// Audio sample data normalized to -1.0 to 1.0.
    samples: Vec<f32>,
    /// Number of vertical bars to display.
    bar_count: usize,
    /// Spacing between bars in pixels.
    bar_spacing: f32,
    /// Color of the bars.
    bar_color: Color,
    /// Background color of the visualization area, when the caller has chosen one.
    ///
    /// `None` means "use the active theme's surface", which is what a visualizer drawn on a themed
    /// page wants. It used to be a **fixed** `rgb(20, 20, 30)` initialised in `new()`, so the
    /// control painted a near-black rectangle on the light appearance as well and the
    /// `style.background_color` that `apply_active_theme` wrote was read by nobody — a declared
    /// value with no consumer. Keeping it `Option` is what lets "the caller set one" and "the
    /// theme supplies one" be told apart, the same distinction `WidgetStyle::theme_derived`
    /// records for the base style.
    background_color: Option<Color>,
    /// Whether to mirror the visualization (bottom half mirrors top).
    mirror: bool,
    /// Whether to show peak hold markers.
    peak_hold: bool,
    /// Duration in ms to hold peak values.
    peak_hold_duration: u64,
    /// Current peak hold values for each bar.
    peak_values: Vec<f32>,
    /// Milliseconds each bar's peak has been held, indexed like `peak_values`.
    ///
    /// Per bar rather than one clock for the whole control: a peak is raised by *its own* bar, so
    /// one bar falling silent must not release the peak the bar next to it has just set. A single
    /// shared timer would do exactly that, and the symptom is every marker sliding down together as
    /// soon as the loudest band stopped.
    peak_ages: Vec<u64>,
}

/// The fall rate of a released peak, in units of full height per second.
///
/// A held peak does not *vanish* when its duration expires -- that reads as a flicker rather than
/// as a marker falling back. It descends at a constant speed, one full bar height per second, which
/// is slow enough to follow at 60 fps and fast enough that a stale marker is gone by the time the
/// next beat arrives.
const PEAK_FALL_PER_SECOND: f32 = 1.0;

impl AudioVisualizer {
    /// Creates a new AudioVisualizer widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let bar_count = 64;
        Self {
            base: BaseWidget::new(WidgetKind::AudioVisualizer, geometry, "AudioVisualizer"),
            samples: Vec::new(),
            bar_count,
            bar_spacing: 2.0,
            bar_color: Color::rgba(0, 150, 255, 255),
            background_color: None,
            mirror: false,
            peak_hold: false,
            peak_hold_duration: 500,
            peak_values: vec![0.0; bar_count],
            peak_ages: vec![0; bar_count],
        }
    }

    /// Sets the audio sample data. Values should be normalized to -1.0 to 1.0.
    pub fn set_samples(&mut self, data: Vec<f32>) {
        self.samples = data;
        self.base.request_redraw();
    }

    /// Adds a single audio sample. Useful for streaming data.
    pub fn add_sample(&mut self, value: f32) {
        self.samples.push(value.clamp(-1.0, 1.0));
        self.base.request_redraw();
    }

    /// Clears all audio samples.
    pub fn clear_samples(&mut self) {
        self.samples.clear();
        self.base.request_redraw();
    }

    /// Returns a reference to the current samples.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Sets the number of vertical bars to display.
    pub fn set_bar_count(&mut self, n: usize) {
        self.bar_count = n.max(1);
        // Both per-bar vectors resize together. They are indexed alike, so resizing one alone left
        // the other short -- and the marker walk, which indexes both, would have panicked on the
        // next tick after any widening.
        self.peak_values.resize(self.bar_count, 0.0);
        self.peak_ages.resize(self.bar_count, 0);
        self.base.request_redraw();
    }

    /// Returns the current bar count.
    pub fn bar_count(&self) -> usize {
        self.bar_count
    }

    /// Sets the spacing between bars in pixels.
    pub fn set_bar_spacing(&mut self, spacing: f32) {
        self.bar_spacing = spacing.max(0.0);
        self.base.request_redraw();
    }

    /// Returns the current bar spacing.
    pub fn bar_spacing(&self) -> f32 {
        self.bar_spacing
    }

    /// Sets the color of the bars.
    pub fn set_bar_color(&mut self, color: Color) {
        self.bar_color = color;
        self.base.request_redraw();
    }

    /// Returns the current bar color.
    pub fn bar_color(&self) -> Color {
        self.bar_color
    }

    /// Sets the background color of the visualization area.
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the background color the visualizer will paint.
    ///
    /// An explicit [`Self::set_background_color`] wins; otherwise this is the active theme's
    /// surface, so the control sits on the page rather than on a fixed near-black. The final
    /// fallback covers a build with no theme at all.
    pub fn background_color(&self) -> Color {
        self.resolved_background()
    }

    /// The fill the draw path should use: the caller's colour, else the theme's surface.
    fn resolved_background(&self) -> Color {
        if let Some(chosen) = self.background_color {
            return chosen;
        }
        // A visualizer's panel is a **surface one step above the page**, the same role
        // `font_preview` and the popup panels use. The theme guard is released before returning.
        let themed = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.surface_container);
        themed.unwrap_or(Color::rgba(20, 20, 30, 255))
    }

    /// Enables or disables mirror mode. When enabled, the bottom half mirrors the top.
    pub fn set_mirror(&mut self, mirror: bool) {
        self.mirror = mirror;
        self.base.request_redraw();
    }

    /// Returns whether mirror mode is enabled.
    pub fn is_mirror_enabled(&self) -> bool {
        self.mirror
    }

    /// Enables or disables peak hold indicators.
    pub fn set_peak_hold(&mut self, enabled: bool) {
        self.peak_hold = enabled;
        self.base.request_redraw();
    }

    /// Returns whether peak hold is enabled.
    pub fn is_peak_hold_enabled(&self) -> bool {
        self.peak_hold
    }

    /// Sets the peak hold duration in milliseconds.
    ///
    /// The field is read by [`AudioVisualizer::advance_peak_hold`], which the animation bus drives:
    /// a peak is held for this long after the bar leaves it, then falls. Before that method existed
    /// the field was stored, published and read by nothing, and a marker sat at its highest ever
    /// value for the lifetime of the widget -- which is not "hold for 500 ms", it is "hold forever".
    pub fn set_peak_hold_duration(&mut self, ms: u64) {
        self.peak_hold_duration = ms;
        self.base.request_redraw();
    }

    /// Returns the peak hold duration in milliseconds.
    pub fn peak_hold_duration(&self) -> u64 {
        self.peak_hold_duration
    }

    /// Advances the peak-hold markers by `delta_ms`, and reports whether anything moved.
    ///
    /// # The two jobs, and why they are one method
    ///
    /// A marker is raised to its bar's current level and then held for [`Self::peak_hold_duration`]
    /// before falling. Raising is a *state* and falling is a *clock*, and they share the per-bar
    /// vectors: a caller that ran one without the other would get a marker that rises and never falls
    /// (the defect this replaces) or one whose age is reset by the bar it belongs to. Both are here
    /// so neither can be called alone.
    ///
    /// # Why the animation bus rather than `draw`
    ///
    /// A duration is only observable if time passes for the widget. `draw` runs when something asks
    /// for a repaint, which for a paused stream is never, so a marker's age must not be computed
    /// there. [`Widget::tick`] is the bus's clock, so the duration reaches it here and the control
    /// asks for the repaint its falling marker needs.
    ///
    /// Returns `true` when a marker moved, which is what tells the bus to keep ticking.
    fn advance_peak_hold(&mut self, delta_ms: u64) -> bool {
        if !self.peak_hold || delta_ms == 0 {
            return false;
        }
        let fall = PEAK_FALL_PER_SECOND * (delta_ms as f32 / 1000.0);
        let mut moved = false;
        for index in 0..self.peak_values.len() {
            let value = self.bar_value(index);
            if value >= self.peak_values[index] {
                // The bar has reached or passed its marker, so the marker is re-set and its clock
                // restarts. `>=` rather than `>` so a steady tone keeps its marker pinned instead of
                // letting it drift down a step at a time.
                moved |= self.peak_values[index] != value;
                self.peak_values[index] = value;
                self.peak_ages[index] = 0;
                continue;
            }
            self.peak_ages[index] = self.peak_ages[index].saturating_add(delta_ms);
            if self.peak_ages[index] <= self.peak_hold_duration {
                continue;
            }
            let next = (self.peak_values[index] - fall).max(self.peak_values[index].min(value));
            // A marker that has fallen to (or below) its bar has done its job and is released here
            // rather than being left a hair above it, which would paint a fraction of a pixel of
            // highlight on every quiet bar for the widget's whole life.
            if next <= 0.0 || next <= value {
                self.peak_values[index] = value;
                self.peak_ages[index] = 0;
            } else {
                self.peak_values[index] = next;
            }
            moved = true;
        }
        moved
    }

    /// The normalized level of bar `index`, or a synthetic level when no samples were supplied.
    ///
    /// # Why the paint and the marker share this
    ///
    /// The paint used to derive its own `bars` vector inline, and a peak-hold marker that measured
    /// the same bar *differently* would sit above or below the bar it belongs to. So the derivation
    /// is one function: index in, level out, and "no samples" is the same synthetic sweep the paint
    /// has always drawn.
    ///
    /// # Mean magnitude, not peak
    ///
    /// The level is the mean of the window's absolute samples, which is what this control's bars
    /// have always meant -- a *loudness* per band rather than a sample peak. A marker derived from a
    /// maximum would ride above the bar it marks on any non-constant window, which is the one thing
    /// a peak-hold marker must not do.
    fn bar_value(&self, index: usize) -> f32 {
        if self.bar_count == 0 || index >= self.bar_count {
            return 0.0;
        }
        if self.samples.is_empty() {
            // The paint's synthetic sweep, so a control with no data still has bars for its markers
            // to belong to rather than a row of markers over an empty panel.
            let t = index as f32 / self.bar_count as f32;
            return ((t * std::f32::consts::PI * 4.0).sin().abs() * 0.6 + 0.1).min(1.0);
        }
        let step = (self.samples.len() as f32 / self.bar_count as f32).max(1.0);
        let start = (index as f32 * step) as usize;
        let end = (((index as f32 + 1.0) * step) as usize).min(self.samples.len());
        if start >= end {
            return 0.0;
        }
        let chunk = &self.samples[start..end];
        let sum: f32 = chunk.iter().map(|sample| sample.abs()).sum();
        (sum / chunk.len() as f32).min(1.0)
    }
}

impl Widget for AudioVisualizer {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::AUDIO_VISUALIZER_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::AUDIO_VISUALIZER_DEFAULT_HEIGHT,
        )
    }

    /// Drives the peak-hold markers' clock.
    ///
    /// The trait's `tick` is the animation bus's entry point, which is the only place a *duration*
    /// can be observed: `draw` runs when something asks for a repaint, and a paused stream asks for
    /// none. Returning `advance_peak_hold`'s answer keeps the bus ticking while markers fall and lets
    /// it stop once they have settled, rather than asking a still visualizer to run at frame rate
    /// forever.
    fn tick(&mut self, delta_ms: u32) -> bool {
        let moved = self.advance_peak_hold(delta_ms as u64);
        if moved {
            self.base.request_redraw();
        }
        moved
    }

    /// Whether the peak-hold markers are still falling, i.e. whether a frame is owed.
    ///
    /// # Why this is not the trait's default
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host uses the
    /// answer to decide whether a frame is needed **before** paying for a sweep. A visualizer whose
    /// markers were still falling answered `false`, so such a host would stop scheduling frames
    /// mid-decline and the markers would freeze part-way — the exact "held forever" shape the
    /// peak-hold clock was written to end, reintroduced one layer up. The condition mirrors
    /// [`Self::advance_peak_hold`]: a marker is in flight while peak-hold is on and it sits above the
    /// bar it tracks.
    fn is_animating(&self) -> bool {
        self.peak_hold
            && (0..self.peak_values.len())
                .any(|index| self.peak_values[index] > self.bar_value(index))
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `AudioVisualizer` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
}

/// `AudioVisualizer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `bar_count`
/// is published as an unsigned integer and clamped to at least one bar on write.
impl WidgetProperties for AudioVisualizer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "bar_count" => Ok(CapabilityValue::UInt(self.bar_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "bar_count" => {
                self.set_bar_count(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["bar_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for AudioVisualizer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let w = rect.width as f32;
        let h = rect.height as f32;

        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // Draw background
        context.face_with_gradient(
            rect,
            self.resolved_background(),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );

        // Calculate bar layout
        let total_spacing = self.bar_spacing * (self.bar_count as f32 + 1.0);
        let bar_width = ((w - total_spacing) / self.bar_count as f32).max(1.0);
        let center_y = rect.y as f32 + h / 2.0;
        let half_height = h / 2.0 - 2.0;

        // Prepare sample data: one shared derivation, so the marker and its bar cannot measure
        // different samples. Empty samples yield the synthetic sweep, which is what makes a
        // data-less control still a usable preview.
        let bars: Vec<f32> = (0..self.bar_count).map(|i| self.bar_value(i)).collect();

        for (i, &value) in bars.iter().enumerate() {
            let value = value.min(1.0);
            let bar_height = (value * half_height).max(1.0);
            let x = rect.x as f32 + self.bar_spacing + i as f32 * (bar_width + self.bar_spacing);
            let bar_rect = Rect::new(
                x as i32,
                (center_y - bar_height) as i32,
                bar_width as u32,
                (bar_height * 2.0) as u32,
            );

            // Color gradient based on amplitude
            let intensity = (value * 255.0) as u8;
            let bar_color = if value > 0.7 {
                Color::rgba(255, intensity, intensity, 255)
            } else if value > 0.4 {
                Color::rgba(intensity, 200, 255, 255)
            } else {
                Color::rgba(intensity / 2, intensity / 2, 200, 255)
            };

            if self.mirror {
                // Draw only top half-bar and mirror it
                let top_bar = Rect::new(
                    x as i32,
                    (center_y - bar_height) as i32,
                    bar_width as u32,
                    bar_height as u32,
                );
                let bottom_bar =
                    Rect::new(x as i32, center_y as i32, bar_width as u32, bar_height as u32);
                context.fill_rect(top_bar, bar_color);
                context.fill_rect(bottom_bar, bar_color);
            } else {
                context.fill_rect(bar_rect, bar_color);
            }

            // Peak hold indicator. The marker's own value is raised by `advance_peak_hold` on the
            // animation bus, not here: a duration is only observable if time passes for the widget,
            // and `draw` runs only when something asks for a repaint. This arm reads the value and
            // paints it, so the two callers cannot disagree about what the marker is.
            if self.peak_hold {
                let peak_value = self.peak_values[i];
                if peak_value > 0.0 {
                    let peak_y = center_y - peak_value * half_height;
                    let peak_rect = Rect::new(
                        x as i32,
                        peak_y as i32,
                        bar_width as u32,
                        std::cmp::max(1, (bar_width * 0.5) as u32),
                    );
                    context.fill_rect(peak_rect, Color::rgba(255, 255, 100, 255));
                }
            }
        }
    }
}

impl EventHandler for AudioVisualizer {
    /// Toggles peak-hold on a press that lands on the visualizer.
    ///
    /// The position used to be discarded, so a press on any other control in the same
    /// window flipped this one's peak-hold. The duplicate `MousePress` arm below it —
    /// an empty body for any non-left button — was dead weight and is gone with it.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. }
                if *button == 1 && self.geometry().contains_point(*pos) =>
            {
                self.peak_hold = !self.peak_hold;
                self.base.request_redraw();
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
    use crate::widget::svg::render_to_svg;

    #[test]
    fn audio_visualizer_default_state() {
        let av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert_eq!(av.bar_count(), 64);
        assert!(av.samples().is_empty());
        assert!(!av.is_mirror_enabled());
        assert!(!av.is_peak_hold_enabled());
        assert_eq!(av.kind(), WidgetKind::AudioVisualizer);
    }

    #[test]
    fn audio_visualizer_set_samples() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        let data = vec![0.0, 0.5, 1.0, -0.5, 0.0];
        av.set_samples(data.clone());
        assert_eq!(av.samples(), &data);
    }

    #[test]
    fn audio_visualizer_add_and_clear_samples() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.add_sample(0.5);
        av.add_sample(-0.3);
        av.add_sample(0.8);
        assert_eq!(av.samples().len(), 3);
        av.clear_samples();
        assert!(av.samples().is_empty());
    }

    #[test]
    fn audio_visualizer_set_bar_count() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(32);
        assert_eq!(av.bar_count(), 32);
        av.set_bar_count(0); // Should clamp to 1
        assert_eq!(av.bar_count(), 1);
    }

    #[test]
    fn audio_visualizer_toggle_mirror_and_peak_hold() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!(!av.is_mirror_enabled());
        av.set_mirror(true);
        assert!(av.is_mirror_enabled());
        assert!(!av.is_peak_hold_enabled());
        av.set_peak_hold(true);
        assert!(av.is_peak_hold_enabled());
    }

    /// `is_animating` answers for the peak-hold clock, and stops once the markers settle.
    ///
    /// # The defect this pins
    ///
    /// `Widget::is_animating` answers `false` unless a control overrides it, and a host consults the
    /// answer to decide whether a frame is needed **before** paying for a sweep. A visualizer whose
    /// markers were still falling answered `false`, so such a host would stop scheduling frames
    /// mid-decline and the markers would freeze part-way — the "held forever" shape the peak-hold
    /// clock was written to end, reintroduced one layer up.
    ///
    /// # Why the assertion is a pair, not a single check
    ///
    /// "It reports `true` while markers fall" alone would pass for a control that answered `true`
    /// forever, which would pin a host at frame rate for the life of the widget. The second half — it
    /// goes back to `false` once the markers have settled — is what makes the answer an animation
    /// rather than a traffic loop.
    #[test]
    fn is_animating_follows_the_peak_hold_clock() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!(!av.is_animating(), "a rest visualizer owes no frames");

        av.set_peak_hold(true);
        av.set_bar_count(4);
        av.set_peak_hold_duration(0);
        // A loud window raises the markers to its own level.
        av.set_samples(vec![1.0, 1.0, 1.0, 1.0]);
        assert!(av.tick(16), "raising the markers is a frame-worthy change");

        // A quiet window then leaves the markers *above* their bars, which is the state the clock
        // exists to animate and therefore the state in which a frame is owed.
        av.set_samples(vec![0.0, 0.0, 0.0, 0.0]);
        assert!(av.is_animating(), "markers above their bars are in flight");

        // Let the clock run: the markers fall to their bars and then stop.
        for _ in 0..400 {
            if !av.tick(16) {
                break;
            }
        }
        assert!(!av.is_animating(), "a settled visualizer must stop asking for frames");
    }

    #[test]
    fn audio_visualizer_bar_spacing() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!((av.bar_spacing() - 2.0).abs() < f32::EPSILON);
        av.set_bar_spacing(5.0);
        assert!((av.bar_spacing() - 5.0).abs() < f32::EPSILON);
    }

    /// The panel colour is the theme's surface unless the caller chose one.
    ///
    /// # The defect this pins
    ///
    /// `new()` initialised `background_color` to a fixed `rgb(20, 20, 30)` and `draw` read that
    /// field directly, so the visualizer painted a near-black rectangle on the light appearance too,
    /// and the `style.background_color` that `apply_active_theme` wrote was read by **nobody** — a
    /// declared value with no consumer. It now resolves to `surface_container` unless
    /// `set_background_color` was called, which is what the `Option` field distinguishes.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_background_defaults_to_the_theme_and_still_honours_an_override() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        let themed = |appearance| -> Color {
            crate::theme::global_theme_manager().set_appearance(appearance);
            AudioVisualizer::new(Rect::new(0, 0, 300, 150)).background_color()
        };

        let dark = themed(crate::theme::AppearanceMode::Dark);
        let light = themed(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "an un-configured visualizer must take the theme's surface, not a fixed near-black; \
             both were {dark:?}"
        );

        // An explicit choice still wins over the theme, which is what keeps the setter meaningful.
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        let chosen = Color::rgb(1, 2, 3);
        av.set_background_color(chosen);
        assert_eq!(av.background_color(), chosen, "a caller's colour must survive the theme");
    }

    // ── `peak_hold_duration` drives the markers' clock ──

    /// A marker is held for `peak_hold_duration` and then falls; the duration is what decides when.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `peak_hold_duration` was stored, published (getter, setter, schema row, round-trip test) and
    /// read by nothing. Worse, the peak values themselves were raised in `draw` and **never fell**,
    /// so a marker sat at its highest ever value for the widget's whole life. "Hold for 500 ms" was
    /// implemented as "hold forever", and no duration could change that because no clock existed.
    #[test]
    fn peak_hold_holds_for_its_duration_then_falls() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(1);
        av.set_peak_hold(true);
        av.set_peak_hold_duration(500);
        // A loud moment, then silence: the marker is raised by the loud one and has to fall on its
        // own once the bar left it.
        av.set_samples(vec![1.0]);
        assert!(av.advance_peak_hold(16), "the marker rises to meet the bar");
        assert!((av.peak_values[0] - 1.0).abs() < 0.001, "it rose to the bar's level");

        av.set_samples(vec![0.0]);
        // Well inside the hold window: the marker does not move at all, which is the "hold" half.
        assert!(!av.advance_peak_hold(400), "a held marker owes no frame");
        assert!((av.peak_values[0] - 1.0).abs() < 0.001, "still at its peak, not falling yet");

        // Past the window: now it descends rather than vanishing.
        assert!(av.advance_peak_hold(200), "past the hold, the marker moves");
        let falling = av.peak_values[0];
        assert!(
            falling < 1.0 && falling > 0.0,
            "a released marker descends instead of jumping to zero: {falling}"
        );

        // The duration is the knob: a longer hold keeps it up for longer from the same moment.
        let mut patient = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        patient.set_bar_count(1);
        patient.set_peak_hold(true);
        patient.set_peak_hold_duration(5_000);
        patient.set_samples(vec![1.0]);
        patient.advance_peak_hold(16);
        patient.set_samples(vec![0.0]);
        assert!(!patient.advance_peak_hold(200), "a 5 s hold is still holding at 200 ms");
        assert!((patient.peak_values[0] - 1.0).abs() < 0.001);

        // Once it has fallen back to the bar it marks, it is released and stops owing frames.
        for _ in 0..40 {
            av.advance_peak_hold(100);
        }
        assert!(
            (av.peak_values[0] - 0.0).abs() < 0.001,
            "a fully released marker sits on its bar: {}",
            av.peak_values[0]
        );
        assert!(!av.advance_peak_hold(100), "a settled marker owes no frame");
    }

    /// The duration is genuinely *observable*: two controls differing only in it render differently
    /// at the same instant.
    ///
    /// The state assertions above say when a marker moves; this says the marker is painted where the
    /// state says it is -- a `peak_values` nothing drew would satisfy every number above.
    #[test]
    fn the_hold_duration_reaches_the_pixels() {
        let build = |duration| -> String {
            let mut av = AudioVisualizer::new(Rect::new(0, 0, 200, 60));
            av.set_bar_count(1);
            av.set_peak_hold(true);
            av.set_peak_hold_duration(duration);
            av.set_samples(vec![1.0]);
            av.advance_peak_hold(16);
            av.set_samples(vec![0.0]);
            // One step past the shorter window and inside the longer one.
            av.advance_peak_hold(600);
            render_to_svg(&mut av)
        };
        assert_ne!(
            build(500),
            build(5_000),
            "the same instant must look different under two hold durations"
        );
    }

    /// Every bar carries its own clock, so one bar going quiet does not release its neighbour's peak.
    #[test]
    fn each_bar_holds_its_own_peak() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        av.set_bar_count(2);
        av.set_peak_hold(true);
        av.set_peak_hold_duration(0);
        // The first bar is loud, the second silent: both markers are raised where they stand.
        av.set_samples(vec![1.0, 0.0]);
        av.advance_peak_hold(16);
        assert!((av.peak_values[0] - 1.0).abs() < 0.001);
        assert!(av.peak_values[1].abs() < 0.001);

        // Bar 0 goes quiet while bar 1 becomes loud. Bar 1's marker must rise, and bar 0's must fall
        // without taking bar 1's with it.
        av.set_samples(vec![0.0, 1.0]);
        av.advance_peak_hold(1_000);
        assert!((av.peak_values[1] - 1.0).abs() < 0.001, "bar 1's marker rose");
        assert!(
            av.peak_values[0] < 1.0,
            "bar 0's marker fell on its own clock: {}",
            av.peak_values[0]
        );
    }
}
