//! Radio button renderer with quality levels.

use crate::core::{Color, Rect};
use crate::quality::QualityLevel;
use crate::render::RenderContext;
use crate::widget::RadioButton;

/// Radio button renderer with quality-aware rendering.
pub struct RadioButtonRenderer {
    quality: QualityLevel,
}

impl RadioButtonRenderer {
    /// Creates a new radio button renderer with specified quality.
    pub fn new(quality: QualityLevel) -> Self {
        Self { quality }
    }

    /// Renders a radio button.
    pub fn render(&self, context: &mut RenderContext, radio_button: &RadioButton, rect: Rect) {
        match self.quality {
            QualityLevel::High => self.render_high_quality(context, radio_button, rect),
            QualityLevel::Medium => self.render_medium_quality(context, radio_button, rect),
            QualityLevel::Low => self.render_low_quality(context, radio_button, rect),
        }
    }

    /// High quality rendering with anti-aliasing and smooth edges.
    fn render_high_quality(
        &self,
        context: &mut RenderContext,
        radio_button: &RadioButton,
        rect: Rect,
    ) {
        let center_x = rect.x + rect.width / 2.0;
        let center_y = rect.y + rect.height / 2.0;
        let radius = rect.height.min(rect.width) / 4.0;

        // Draw outer circle with anti-aliasing
        context.draw_circle_aa(
            center_x,
            center_y,
            radius,
            Color::from_rgb(100, 100, 100),
            2.0,
        );

        // Draw inner circle if checked
        if radio_button.is_checked() {
            let inner_radius = radius / 2.0;
            context.fill_circle_aa(
                center_x,
                center_y,
                inner_radius,
                Color::from_rgb(0, 120, 215),
            );
        }

        // Draw focus indicator if needed
        // (would need focus state tracking)
    }

    /// Medium quality rendering with basic shapes.
    fn render_medium_quality(
        &self,
        context: &mut RenderContext,
        radio_button: &RadioButton,
        rect: Rect,
    ) {
        let center_x = rect.x + rect.width / 2.0;
        let center_y = rect.y + rect.height / 2.0;
        let radius = rect.height.min(rect.width) / 4.0;

        // Draw outer circle
        context.draw_circle(center_x, center_y, radius, Color::from_rgb(100, 100, 100));

        // Draw inner circle if checked
        if radio_button.is_checked() {
            let inner_radius = radius / 2.0;
            context.fill_circle(
                center_x,
                center_y,
                inner_radius,
                Color::from_rgb(0, 120, 215),
            );
        }
    }

    /// Low quality rendering with simple rectangles.
    fn render_low_quality(
        &self,
        context: &mut RenderContext,
        radio_button: &RadioButton,
        rect: Rect,
    ) {
        let size = rect.height.min(rect.width) / 2.0;
        let x = rect.x + (rect.width - size) / 2.0;
        let y = rect.y + (rect.height - size) / 2.0;

        // Draw outer square
        context.draw_rect(x, y, size, size, Color::from_rgb(100, 100, 100));

        // Draw inner square if checked
        if radio_button.is_checked() {
            let inner_size = size / 2.0;
            let inner_x = x + (size - inner_size) / 2.0;
            let inner_y = y + (size - inner_size) / 2.0;
            context.fill_rect(
                inner_x,
                inner_y,
                inner_size,
                inner_size,
                Color::from_rgb(0, 120, 215),
            );
        }
    }

    /// Updates quality level.
    pub fn set_quality(&mut self, quality: QualityLevel) {
        self.quality = quality;
    }

    /// Returns current quality level.
    pub fn quality(&self) -> QualityLevel {
        self.quality
    }

    /// Batch renders multiple radio buttons.
    pub fn render_batch(&self, context: &mut RenderContext, items: &[(&RadioButton, Rect)]) {
        for (radio_button, rect) in items {
            self.render(context, radio_button, *rect);
        }
    }
}
