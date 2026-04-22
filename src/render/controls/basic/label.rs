//! Label rendering implementation.

use crate::core::{Alignment, Color, Rect};
use crate::quality::QualityLevel;
use crate::render::RenderContext;
use crate::widget::Label;

/// Label renderer with quality-aware rendering.
pub struct LabelRenderer;

impl LabelRenderer {
    /// Renders a label with adaptive quality.
    pub fn draw(context: &mut RenderContext, label: &Label) {
        let rect = label.geometry();
        let text = label.text();
        let alignment = label.alignment();

        // Select rendering strategy based on quality level
        match context.quality_level() {
            QualityLevel::High => Self::draw_high_quality(context, rect, text, alignment),
            QualityLevel::Medium => Self::draw_medium_quality(context, rect, text, alignment),
            QualityLevel::Low => Self::draw_low_quality(context, rect, text, alignment),
        }
    }

    /// High quality rendering with text shaping and anti-aliasing.
    fn draw_high_quality(
        context: &mut RenderContext,
        rect: Rect,
        text: &str,
        alignment: Alignment,
    ) {
        if text.is_empty() {
            return;
        }

        // Draw background if needed
        if let Some(bg_color) = context.current_background_color() {
            context.fill_rect(rect, bg_color);
        }

        // Draw text with advanced features
        let text_color = context
            .current_text_color()
            .unwrap_or(Color::from_rgb(0, 0, 0));

        // Use shaped text for better typography
        let shaped_text = context.shape_text(text);

        // Calculate text position based on alignment
        let text_rect = Self::calculate_text_rect(rect, &shaped_text, alignment);

        // Draw text with anti-aliasing
        context.draw_shaped_text_antialiased(text_rect, &shaped_text, text_color);

        // Draw subtle text shadow for depth (only for certain alignments)
        if alignment == Alignment::Center || alignment == Alignment::Right {
            let shadow_rect = Rect::new(
                text_rect.x + 1,
                text_rect.y + 1,
                text_rect.width,
                text_rect.height,
            );
            context.draw_shaped_text(shadow_rect, &shaped_text, Color::from_rgba(0, 0, 0, 30));
        }
    }

    /// Medium quality rendering with basic text.
    fn draw_medium_quality(
        context: &mut RenderContext,
        rect: Rect,
        text: &str,
        alignment: Alignment,
    ) {
        if text.is_empty() {
            return;
        }

        // Draw background if needed
        if let Some(bg_color) = context.current_background_color() {
            context.fill_rect(rect, bg_color);
        }

        // Draw text
        let text_color = context
            .current_text_color()
            .unwrap_or(Color::from_rgb(0, 0, 0));
        let text_rect = Self::calculate_simple_text_rect(rect, text, alignment);

        context.draw_text(text_rect, text, text_color);
    }

    /// Low quality rendering - minimal text rendering.
    fn draw_low_quality(context: &mut RenderContext, rect: Rect, text: &str, alignment: Alignment) {
        if text.is_empty() {
            return;
        }

        // Only draw essential text
        let text_color = context
            .current_text_color()
            .unwrap_or(Color::from_rgb(0, 0, 0));

        // For low quality, use simple left alignment for performance
        let text_rect = Rect::new(
            rect.x + 2,
            rect.y + (rect.height as i32 / 2) - 6,
            rect.width,
            rect.height,
        );

        // Use simple text rendering (no shaping, no anti-aliasing)
        context.draw_text_simple(text_rect, text, text_color);
    }

    /// Calculate text rectangle based on alignment for shaped text.
    fn calculate_text_rect(
        rect: Rect,
        shaped_text: &crate::render::ShapedText,
        alignment: Alignment,
    ) -> Rect {
        let text_width = shaped_text.advance() as i32;
        let text_height = 12; // Standard text height

        match alignment {
            Alignment::Left => Rect::new(
                rect.x + 2,
                rect.y + (rect.height as i32 - text_height) / 2,
                rect.width,
                rect.height,
            ),
            Alignment::Center => {
                let x = rect.x + (rect.width as i32 - text_width) / 2;
                Rect::new(
                    x,
                    rect.y + (rect.height as i32 - text_height) / 2,
                    rect.width,
                    rect.height,
                )
            }
            Alignment::Right => {
                let x = rect.x + rect.width as i32 - text_width - 2;
                Rect::new(
                    x,
                    rect.y + (rect.height as i32 - text_height) / 2,
                    rect.width,
                    rect.height,
                )
            }
        }
    }

    /// Calculate text rectangle for simple text rendering.
    fn calculate_simple_text_rect(rect: Rect, text: &str, alignment: Alignment) -> Rect {
        let text_width = text.len() as i32 * 6; // Approximate width
        let text_height = 12;

        match alignment {
            Alignment::Left => Rect::new(
                rect.x + 2,
                rect.y + (rect.height as i32 - text_height) / 2,
                rect.width,
                rect.height,
            ),
            Alignment::Center => {
                let x = rect.x + (rect.width as i32 - text_width) / 2;
                Rect::new(
                    x,
                    rect.y + (rect.height as i32 - text_height) / 2,
                    rect.width,
                    rect.height,
                )
            }
            Alignment::Right => {
                let x = rect.x + rect.width as i32 - text_width - 2;
                Rect::new(
                    x,
                    rect.y + (rect.height as i32 - text_height) / 2,
                    rect.width,
                    rect.height,
                )
            }
        }
    }

    /// Batch render multiple labels for performance.
    pub fn batch_draw(context: &mut RenderContext, labels: &[(Rect, &str, Alignment)]) {
        // Group by alignment for efficient rendering
        let mut left_labels = Vec::new();
        let mut center_labels = Vec::new();
        let mut right_labels = Vec::new();

        for (rect, text, alignment) in labels {
            match alignment {
                Alignment::Left => left_labels.push((*rect, *text)),
                Alignment::Center => center_labels.push((*rect, *text)),
                Alignment::Right => right_labels.push((*rect, *text)),
            }
        }

        // Batch render each group
        if !left_labels.is_empty() {
            Self::batch_draw_group(context, &left_labels, Alignment::Left);
        }
        if !center_labels.is_empty() {
            Self::batch_draw_group(context, &center_labels, Alignment::Center);
        }
        if !right_labels.is_empty() {
            Self::batch_draw_group(context, &right_labels, Alignment::Right);
        }
    }

    fn batch_draw_group(
        context: &mut RenderContext,
        labels: &[(Rect, &str)],
        alignment: Alignment,
    ) {
        // In a real implementation, this would batch text rendering
        // For now, just draw each label individually
        for (rect, text) in labels {
            // Create a temporary label for rendering
            let temp_label = Label::new(text.to_string(), *rect);
            // We need to set the alignment
            // For now, we'll use the individual draw method
            Self::draw(context, &temp_label);
        }
    }
}
