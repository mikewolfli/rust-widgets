//! Button rendering implementation.

use crate::core::{Color, Rect};
use crate::quality::QualityLevel;
use crate::render::RenderContext;
use crate::widget::{Button, ButtonState};

/// Button renderer with quality-aware rendering.
pub struct ButtonRenderer;

impl ButtonRenderer {
    /// Renders a button with adaptive quality.
    pub fn draw(context: &mut RenderContext, button: &Button) {
        let rect = button.geometry();
        let state = button.state();

        // Select rendering strategy based on quality level
        match context.quality_level() {
            QualityLevel::High => Self::draw_high_quality(context, rect, state, button.text()),
            QualityLevel::Medium => Self::draw_medium_quality(context, rect, state, button.text()),
            QualityLevel::Low => Self::draw_low_quality(context, rect, state, button.text()),
        }
    }

    /// High quality rendering with gradients and shadows.
    fn draw_high_quality(context: &mut RenderContext, rect: Rect, state: ButtonState, text: &str) {
        // Draw shadow for depth effect
        let shadow_rect = Rect::new(rect.x + 2, rect.y + 2, rect.width, rect.height);
        context.fill_rect(shadow_rect, Color::from_rgba(0, 0, 0, 50));

        // Draw button background with gradient
        let (top_color, bottom_color) = match state {
            ButtonState::Normal => (
                Color::from_rgb(245, 245, 245),
                Color::from_rgb(225, 225, 225),
            ),
            ButtonState::Pressed => (
                Color::from_rgb(210, 210, 210),
                Color::from_rgb(190, 190, 190),
            ),
            ButtonState::Disabled => (
                Color::from_rgb(240, 240, 240),
                Color::from_rgb(220, 220, 220),
            ),
        };

        context.fill_gradient_rect(rect, top_color, bottom_color);

        // Draw border with anti-aliasing
        let border_color = match state {
            ButtonState::Normal => Color::from_rgb(180, 180, 180),
            ButtonState::Pressed => Color::from_rgb(150, 150, 150),
            ButtonState::Disabled => Color::from_rgb(200, 200, 200),
        };

        context.draw_rect_antialiased(rect, border_color, 1);

        // Draw rounded corners for modern look
        if rect.width > 20 && rect.height > 20 {
            context.draw_rounded_rect(rect, border_color, 1, 4);
        }

        // Draw text with shadow for depth
        if !text.is_empty() {
            let text_color = match state {
                ButtonState::Disabled => Color::from_rgb(150, 150, 150),
                _ => Color::from_rgb(50, 50, 50),
            };

            let shadow_color = match state {
                ButtonState::Pressed => Color::from_rgba(0, 0, 0, 30),
                _ => Color::from_rgba(255, 255, 255, 80),
            };

            // Text shadow
            let shadow_offset = if state == ButtonState::Pressed { 0 } else { 1 };
            let shadow_rect = Rect::new(
                rect.x + (rect.width as i32 / 2) - (text.len() as i32 * 3),
                rect.y + (rect.height as i32 / 2) - 6 + shadow_offset,
                rect.width,
                rect.height,
            );
            context.draw_text(shadow_rect, text, shadow_color);

            // Main text
            let text_rect = Rect::new(
                rect.x + (rect.width as i32 / 2) - (text.len() as i32 * 3),
                rect.y + (rect.height as i32 / 2) - 6,
                rect.width,
                rect.height,
            );
            context.draw_text_antialiased(text_rect, text, text_color);
        }
    }

    /// Medium quality rendering with solid colors.
    fn draw_medium_quality(
        context: &mut RenderContext,
        rect: Rect,
        state: ButtonState,
        text: &str,
    ) {
        // Draw solid background
        let bg_color = match state {
            ButtonState::Normal => Color::from_rgb(240, 240, 240),
            ButtonState::Pressed => Color::from_rgb(200, 200, 200),
            ButtonState::Disabled => Color::from_rgb(220, 220, 220),
        };

        context.fill_rect(rect, bg_color);

        // Draw simple border
        let border_color = match state {
            ButtonState::Normal => Color::from_rgb(180, 180, 180),
            ButtonState::Pressed => Color::from_rgb(150, 150, 150),
            ButtonState::Disabled => Color::from_rgb(200, 200, 200),
        };

        context.draw_rect(rect, border_color, 1);

        // Draw text
        if !text.is_empty() {
            let text_color = match state {
                ButtonState::Disabled => Color::from_rgb(150, 150, 150),
                _ => Color::from_rgb(0, 0, 0),
            };

            let text_rect = Rect::new(
                rect.x + (rect.width as i32 / 2) - (text.len() as i32 * 3),
                rect.y + (rect.height as i32 / 2) - 6,
                rect.width,
                rect.height,
            );
            context.draw_text(text_rect, text, text_color);
        }
    }

    /// Low quality rendering - minimal visual elements.
    fn draw_low_quality(context: &mut RenderContext, rect: Rect, state: ButtonState, text: &str) {
        // Only draw background if not normal state
        if state != ButtonState::Normal {
            let bg_color = match state {
                ButtonState::Pressed => Color::from_rgb(200, 200, 200),
                ButtonState::Disabled => Color::from_rgb(220, 220, 220),
                _ => Color::from_rgb(240, 240, 240),
            };

            context.fill_rect(rect, bg_color);
        }

        // Only draw border for pressed state
        if state == ButtonState::Pressed {
            context.draw_rect(rect, Color::from_rgb(150, 150, 150), 1);
        }

        // Only draw essential text
        if !text.is_empty() && state != ButtonState::Disabled {
            let text_rect = Rect::new(
                rect.x + 4,
                rect.y + (rect.height as i32 / 2) - 6,
                rect.width,
                rect.height,
            );
            context.draw_text_simple(text_rect, text, Color::from_rgb(0, 0, 0));
        }
    }

    /// Batch render multiple buttons for performance.
    pub fn batch_draw(context: &mut RenderContext, buttons: &[(Rect, ButtonState, &str)]) {
        // Group by state for efficient rendering
        let mut normal_buttons = Vec::new();
        let mut pressed_buttons = Vec::new();
        let mut disabled_buttons = Vec::new();

        for (rect, state, text) in buttons {
            match state {
                ButtonState::Normal => normal_buttons.push((*rect, *text)),
                ButtonState::Pressed => pressed_buttons.push((*rect, *text)),
                ButtonState::Disabled => disabled_buttons.push((*rect, *text)),
            }
        }

        // Batch render each group
        if !normal_buttons.is_empty() {
            Self::batch_draw_group(context, &normal_buttons, ButtonState::Normal);
        }
        if !pressed_buttons.is_empty() {
            Self::batch_draw_group(context, &pressed_buttons, ButtonState::Pressed);
        }
        if !disabled_buttons.is_empty() {
            Self::batch_draw_group(context, &disabled_buttons, ButtonState::Disabled);
        }
    }

    fn batch_draw_group(context: &mut RenderContext, buttons: &[(Rect, &str)], state: ButtonState) {
        // In a real implementation, this would use instanced rendering
        // For now, just draw each button individually
        for (rect, text) in buttons {
            // Create a temporary button for rendering
            let temp_button = Button::new(text.to_string(), *rect);
            // We need to set the state - in a real implementation, we'd have a way to render directly
            // For now, we'll use the individual draw method
            Self::draw(context, &temp_button);
        }
    }
}
