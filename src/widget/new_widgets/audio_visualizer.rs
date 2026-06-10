//! AudioVisualizer widget — real-time audio waveform/spectrum visualization.
//!
//! Displays vertical bars representing audio frequency bands or waveform samples.
//! Supports mirror mode (bottom half mirrors top), peak hold indicators, and
//! configurable bar count and spacing.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
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
    /// Background color of the visualization area.
    background_color: Color,
    /// Whether to mirror the visualization (bottom half mirrors top).
    mirror: bool,
    /// Whether to show peak hold markers.
    peak_hold: bool,
    /// Duration in ms to hold peak values.
    peak_hold_duration: u64,
    /// Current peak hold values for each bar.
    peak_values: Vec<f32>,
}

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
            background_color: Color::rgba(20, 20, 30, 255),
            mirror: false,
            peak_hold: false,
            peak_hold_duration: 500,
            peak_values: vec![0.0; bar_count],
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
        self.peak_values.resize(self.bar_count, 0.0);
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
        self.background_color = color;
        self.base.request_redraw();
    }

    /// Returns the current background color.
    pub fn background_color(&self) -> Color {
        self.background_color
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
}

impl Widget for AudioVisualizer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
        context.fill_rect(rect, self.background_color);

        // Calculate bar layout
        let total_spacing = self.bar_spacing * (self.bar_count as f32 + 1.0);
        let bar_width = ((w - total_spacing) / self.bar_count as f32).max(1.0);
        let center_y = rect.y as f32 + h / 2.0;
        let half_height = h / 2.0 - 2.0;

        // Prepare sample data: downsample to bar_count
        let bars: Vec<f32> = if self.samples.is_empty() {
            // Generate some test data when no samples are provided
            (0..self.bar_count)
                .map(|i| {
                    let t = i as f32 / self.bar_count as f32;
                    (t * std::f32::consts::PI * 4.0).sin().abs() * 0.6 + 0.1
                })
                .collect()
        } else {
            let step = (self.samples.len() as f32 / self.bar_count as f32).max(1.0);
            (0..self.bar_count)
                .map(|i| {
                    let start = (i as f32 * step) as usize;
                    let end = ((i as f32 + 1.0) * step) as usize;
                    let end = end.min(self.samples.len());
                    if start < end {
                        let chunk = &self.samples[start..end];
                        let sum: f32 = chunk.iter().map(|v| v.abs()).sum();
                        (sum / chunk.len() as f32).min(1.0)
                    } else {
                        0.0
                    }
                })
                .collect()
        };

        for i in 0..self.bar_count {
            let value = bars[i].min(1.0);
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

            // Peak hold indicator
            if self.peak_hold {
                let peak_value = self.peak_values[i];
                if value > peak_value {
                    self.peak_values[i] = value;
                }
                if self.peak_values[i] > 0.0 {
                    let peak_y = center_y - self.peak_values[i] * half_height;
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
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button: _ } => {
                // Basic interaction: can be used to toggle pause/resume
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

    #[test]
    fn audio_visualizer_bar_spacing() {
        let mut av = AudioVisualizer::new(Rect::new(0, 0, 300, 150));
        assert!((av.bar_spacing() - 2.0).abs() < f32::EPSILON);
        av.set_bar_spacing(5.0);
        assert!((av.bar_spacing() - 5.0).abs() < f32::EPSILON);
    }
}
