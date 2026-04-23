//! Checkbox rendering implementation.
use crate::core::{Color, Rect};
use crate::quality::QualityLevel;
use crate::render::RenderContext;
use crate::widget::{CheckBox, CheckState};
/// Checkbox renderer with quality-aware rendering.
pub struct CheckBoxRenderer;
impl CheckBoxRenderer {
    /// Renders a checkbox with adaptive quality.
    pub fn draw(context: &mut RenderContext, checkbox: &CheckBox) {
        let rect = checkbox.geometry();
        let state = checkbox.state();
        let enabled = checkbox.is_enabled();
        // Select rendering strategy based on quality level
        match context.quality_level() {
            QualityLevel::High => Self::draw_high_quality(context, rect, state, enabled),
            QualityLevel::Medium => Self::draw_medium_quality(context, rect, state, enabled),
            QualityLevel::Low => Self::draw_low_quality(context, rect, state, enabled),
        }
    }
    /// High quality rendering with smooth gradients and anti-aliasing.
    fn draw_high_quality(
        context: &mut RenderContext,
        rect: Rect,
        state: CheckState,
        enabled: bool,
    ) {
        let checkbox_size = 16;
        let checkbox_rect = Self::calculate_checkbox_rect(rect, checkbox_size);
        // Draw shadow for depth
        let shadow_rect = Rect::new(
            checkbox_rect.x + 1,
            checkbox_rect.y + 1,
            checkbox_rect.width,
            checkbox_rect.height,
        );
        context.fill_rect(Rect::new(shadow_rect, Color::from_rgba(0, 0, 0), 30));
        // Draw checkbox background with gradient
        let (top_color, bottom_color) = if !enabled {
            (
                Color::from_rgb(245, 245, 245),
                Color::from_rgb(230, 230, 230),
            )
        } else {
            (
                Color::from_rgb(255, 255, 255),
                Color::from_rgb(245, 245, 245),
            )
        };
        context.fill_gradient_rect(checkbox_rect, top_color, bottom_color);
        // Draw border with anti-aliasing
        let border_color = if !enabled {
            Color::from_rgb(200, 200, 200)
        } else {
            Color::from_rgb(150, 150, 150)
        };
        context.draw_rect_antialiased(checkbox_rect, border_color, 1);
        // Draw rounded corners
        context.draw_rounded_rect(checkbox_rect, border_color, 1, 2);
        // Draw checkmark or partial check
        if state != CheckState::Unchecked {
            let check_color = if !enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 120, 215) // Modern blue
            };
            match state {
                CheckState::Checked => {
                    // Draw smooth checkmark
                    Self::draw_smooth_checkmark(context, checkbox_rect, check_color);
                }
                CheckState::PartiallyChecked => {
                    // Draw smooth partial check (minus sign)
                    Self::draw_smooth_partial_check(context, checkbox_rect, check_color);
                }
                _ => {}
            }
        }
        // Draw focus indicator if focused
        if enabled && context.has_focus() {
            let focus_rect = Rect::new(
                checkbox_rect.x - 2,
                checkbox_rect.y - 2,
                checkbox_rect.width + 4,
                checkbox_rect.height + 4,
            );
            context.draw_dotted_rect(focus_rect, Color::from_rgb(0, 120, 215), 1);
        }
    }
    /// Medium quality rendering with solid colors.
    fn draw_medium_quality(
        context: &mut RenderContext,
        rect: Rect,
        state: CheckState,
        enabled: bool,
    ) {
        let checkbox_size = 16;
        let checkbox_rect = Self::calculate_checkbox_rect(rect, checkbox_size);
        // Draw solid background
        let bg_color = if !enabled {
            Color::from_rgb(240, 240, 240)
        } else {
            Color::from_rgb(255, 255, 255)
        };
        context.fill_rect(checkbox_rect, bg_color);
        // Draw simple border
        let border_color = if !enabled {
            Color::from_rgb(180, 180, 180)
        } else {
            Color::from_rgb(100, 100, 100)
        };
        context.draw_rect(checkbox_rect, border_color, 1);
        // Draw checkmark or partial check
        if state != CheckState::Unchecked {
            let check_color = if !enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            match state {
                CheckState::Checked => {
                    // Draw simple checkmark
                    Self::draw_simple_checkmark(context, checkbox_rect, check_color);
                }
                CheckState::PartiallyChecked => {
                    // Draw simple partial check
                    Self::draw_simple_partial_check(context, checkbox_rect, check_color);
                }
                _ => {}
            }
        }
    }
    /// Low quality rendering - minimal visual elements.
    fn draw_low_quality(context: &mut RenderContext, rect: Rect, state: CheckState, enabled: bool) {
        let checkbox_size = 14; // Smaller for low quality
        let checkbox_rect = Self::calculate_checkbox_rect(rect, checkbox_size);
        // Only draw background for checked or disabled states
        if state != CheckState::Unchecked || !enabled {
            let bg_color = if !enabled {
                Color::from_rgb(220, 220, 220)
            } else {
                Color::from_rgb(240, 240, 240)
            };
            context.fill_rect(checkbox_rect, bg_color);
        }
        // Only draw border for enabled checkboxes
        if enabled {
            context.draw_rect(Rect::new(checkbox_rect, Color::from_rgb(100, 100, 100)), 1);
        }
        // Draw checkmark (simplified)
        if state == CheckState::Checked {
            let check_color = if !enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            // Simple X mark for low quality
            let center_x = checkbox_rect.x + checkbox_rect.width as i32 / 2;
            let center_y = checkbox_rect.y + checkbox_rect.height as i32 / 2;
            let size = 4;
            context.draw_line(Point::new(center_x - size as f32, center_y - size as f32), Point::new(center_x + size as f32, center_y + size as f32), check_color,
                1,);
            context.draw_line(Point::new(center_x + size as f32, center_y - size as f32), Point::new(center_x - size as f32, center_y + size as f32), check_color,
                1,);
        } else if state == CheckState::PartiallyChecked {
            // Simple horizontal line for partial check
            let line_y = checkbox_rect.y + checkbox_rect.height as i32 / 2;
            let line_color = if !enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_line(Point::new(checkbox_rect.x + 3 as f32, line_y as f32), Point::new(checkbox_rect.right() - 3 as f32, line_y as f32), line_color,
                2,);
        }
    }
    /// Calculate checkbox rectangle within label bounds.
    fn calculate_checkbox_rect(rect: Rect, size: i32) -> Rect {
        Rect::new(
            rect.x,
            rect.y + (rect.height as i32 - size) / 2,
            size as u32,
            size as u32,
        )
    }
    /// Draw smooth anti-aliased checkmark.
    fn draw_smooth_checkmark(context: &mut RenderContext, rect: Rect, color: Color) {
        // Draw checkmark using Bezier curves for smooth appearance
        let points = [
            (rect.x + 4, rect.y + 8),
            (rect.x + 7, rect.y + 11),
            (rect.x + 12, rect.y + 6),
        ];
        context.draw_smooth_polyline(&points, color, 2);
        // Additional subtle highlight for 3D effect
        let highlight_color = Color::from_rgba(255, 255, 255, 100);
        let highlight_points = [
            (rect.x + 5, rect.y + 9),
            (rect.x + 8, rect.y + 12),
            (rect.x + 13, rect.y + 7),
        ];
        context.draw_smooth_polyline(&highlight_points, highlight_color, 1);
    }
    /// Draw simple checkmark.
    fn draw_simple_checkmark(context: &mut RenderContext, rect: Rect, color: Color) {
        // Draw checkmark using straight lines
        context.draw_line(Point::new(Point::new(Point::new(rect.x + 4 as f32, rect.y + 8 as f32))), Point::new(Point::new(Point::new(rect.x + 7 as f32, rect.y + 11 as f32))), color, 2);
        context.draw_line(Point::new(Point::new(Point::new(rect.x + 7 as f32, rect.y + 11 as f32))), Point::new(Point::new(Point::new(rect.x + 12 as f32, rect.y + 6 as f32))), color, 2);
    }
    /// Draw smooth partial check (minus sign).
    fn draw_smooth_partial_check(context: &mut RenderContext, rect: Rect, color: Color) {
        let y = rect.y + rect.height as f32 as i32 / 2;
        let x1 = rect.x + 4;
        let x2 = rect.right() - 4;
        // Draw anti-aliased line
        context.draw_line_antialiased(x1, y, x2, y, color, 2);
        // Add subtle shadow for depth
        context.draw_line_antialiased(x1, y + 1, x2, y + 1, Color::from_rgba(0, 0, 0, 30), 1);
    }
    /// Draw simple partial check.
    fn draw_simple_partial_check(context: &mut RenderContext, rect: Rect, color: Color) {
        let y = rect.y + rect.height as f32 as i32 / 2;
        context.draw_line(Point::new(Point::new(Point::new(rect.x + 4 as f32, y as f32))), Point::new(Point::new(Point::new(rect.right() - 4 as f32, y as f32))), color, 2);
    }
    /// Batch render multiple checkboxes for performance.
    pub fn batch_draw(context: &mut RenderContext, checkboxes: &[(Rect, CheckState, bool)]) {
        // Group by state for efficient rendering
        let mut unchecked = Vec::new();
        let mut checked = Vec::new();
        let mut partial = Vec::new();
        for (rect, state, enabled) in checkboxes {
            match state {
                CheckState::Unchecked => unchecked.push((*rect, *enabled)),
                CheckState::Checked => checked.push((*rect, *enabled)),
                CheckState::PartiallyChecked => partial.push((*rect, *enabled)),
            }
        }
        // Batch render each group
        if !unchecked.is_empty() {
            Self::batch_draw_group(context, &unchecked, CheckState::Unchecked);
        }
        if !checked.is_empty() {
            Self::batch_draw_group(context, &checked, CheckState::Checked);
        }
        if !partial.is_empty() {
            Self::batch_draw_group(context, &partial, CheckState::PartiallyChecked);
        }
    }
    fn batch_draw_group(
        context: &mut RenderContext,
        checkboxes: &[(Rect, bool)],
        state: CheckState,
    ) {
        // In a real implementation, this would use instanced rendering
        // For now, just draw each checkbox individually
        for (rect, enabled) in checkboxes {
            // We need to create a temporary checkbox for rendering
            // For now, we'll simulate the rendering
            match context.quality_level() {
                QualityLevel::High => Self::draw_high_quality(context, *rect, state, *enabled),
                QualityLevel::Medium => Self::draw_medium_quality(context, *rect, state, *enabled),
                QualityLevel::Low => Self::draw_low_quality(context, *rect, state, *enabled),
            }
        }
    }
}
