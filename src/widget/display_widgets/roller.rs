//! Roller widget — scroll-wheel style selector (BLUE13 R2.3).
//!
//! Displays a scroll-wheel list of options where one item is highlighted in the
//! center. Supports mouse wheel scrolling and click-to-select interaction.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Roller widget for selecting from a list via scroll-wheel interaction.
pub struct Roller {
    base: BaseWidget,
    /// List of option strings.
    options: Vec<String>,
    /// Currently selected index.
    selected_index: usize,
    /// Number of visible items (should be odd for symmetry).
    visible_count: u32,
    /// Font size for items.
    font_size: f32,
    /// Signal emitted when selection changes.
    pub changed_signal: GenericSignal,
}

impl Roller {
    /// Creates a new `Roller` with the given options and geometry.
    ///
    /// The initial selection is at index 0 and `visible_count` defaults to 5
    /// (clamped to an odd value so the selected item sits in the center).
    pub fn new(options: Vec<String>, rect: Rect) -> Self {
        let visible_count = 5u32 | 1; // ensure odd, at least 3
        Self {
            base: BaseWidget::new(WidgetKind::Roller, rect, "Roller"),
            options,
            selected_index: 0,
            visible_count,
            font_size: 16.0,
            changed_signal: GenericSignal::new(),
        }
    }

    /// Returns all option strings.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// Replaces the option list and clamps the selected index to the new range.
    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options;
        if !self.options.is_empty() {
            self.selected_index = self.selected_index.min(self.options.len() - 1);
        } else {
            self.selected_index = 0;
        }
        self.changed_signal.emit();
        self.base.request_redraw();
    }

    /// Returns the currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Sets the selected index, clamped to the valid range.
    ///
    /// Emits `changed_signal` only when the value actually changes.
    pub fn set_selected_index(&mut self, index: usize) {
        let clamped = if self.options.is_empty() { 0 } else { index.min(self.options.len() - 1) };
        if clamped != self.selected_index {
            self.selected_index = clamped;
            self.changed_signal.emit();
            self.base.request_redraw();
        }
    }

    /// Returns the text of the currently selected option, or `None` if the
    /// options list is empty.
    pub fn selected_text(&self) -> Option<&str> {
        if self.selected_index < self.options.len() {
            Some(self.options[self.selected_index].as_str())
        } else {
            None
        }
    }

    /// Returns the number of visible items in the wheel.
    pub fn visible_count(&self) -> u32 {
        self.visible_count
    }

    /// Sets the number of visible items. The value is rounded up to the next
    /// odd number (minimum 3) so that the selected item appears centered.
    pub fn set_visible_count(&mut self, count: u32) {
        let clamped = count.max(3);
        let odd = if clamped.is_multiple_of(2) { clamped + 1 } else { clamped };
        if odd != self.visible_count {
            self.visible_count = odd;
            self.base.request_redraw();
        }
    }

    /// Sets the font size used to render option text.
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.max(4.0);
        self.base.request_redraw();
    }

    /// Returns the font size used to render option text.
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Computes the height of a single item row based on the current font size.
    fn item_height(&self) -> u32 {
        (self.font_size * 1.5).ceil() as u32
    }

    /// Computes the total content height for all visible items.
    fn content_height(&self) -> u32 {
        self.item_height() * self.visible_count
    }
}

impl Widget for Roller {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // Estimate width from the longest option string.
        let char_width = self.font_size * 0.6;
        let max_len = self.options.iter().map(|s| s.len()).max().unwrap_or(10);
        let width = (max_len as f32 * char_width).ceil().max(80.0) as u32;
        Size::new(width, self.content_height())
    }
}

impl EventHandler for Roller {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || self.options.is_empty() {
            return;
        }
        match event {
            // Wheel / scroll: positive delta.y scrolls forward (next item),
            // negative delta.y scrolls backward (previous item).
            Event::Wheel { delta, modifiers: _ } => {
                if delta.y > 0 && self.selected_index + 1 < self.options.len() {
                    self.set_selected_index(self.selected_index + 1);
                } else if delta.y < 0 && self.selected_index > 0 {
                    self.set_selected_index(self.selected_index - 1);
                }
            }
            // Mouse press: determine which item was clicked relative to the
            // center of the visible wheel.
            Event::MousePress { pos, button: _ } => {
                let rect = self.geometry();
                let item_h = self.item_height() as i32;
                let center_y = rect.y + (rect.height as i32) / 2;
                let clicked_offset = pos.y - center_y;
                let half_visible = (self.visible_count / 2) as i32;
                // Clamp the row offset to the visible range.
                let row_offset = (clicked_offset / item_h).clamp(-half_visible, half_visible);
                if row_offset == 0 {
                    // Clicked the center item — fire clicked signal.
                    self.base.clicked.emit();
                } else {
                    let target = self.selected_index as i32 + row_offset;
                    let target =
                        target.clamp(0, self.options.len().saturating_sub(1) as i32) as usize;
                    self.set_selected_index(target);
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos: _ } => {
                self.base.clicked.emit();
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Roller {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 || self.options.is_empty() {
            return;
        }

        // Resolve style colors with sensible fallbacks.
        let bg_color = self.style().background_color.unwrap_or(Color::rgb(240, 240, 240));
        let selected_bg = self
            .style()
            .background_color
            .map(|c| Color::rgba(c.r, c.g, c.b, 200))
            .unwrap_or(Color::rgb(0, 120, 215));
        let text_color = self.style().text_color.unwrap_or(Color::rgb(50, 50, 50));
        let selected_text_color = Color::WHITE;
        let muted_color = Color::rgba(text_color.r, text_color.g, text_color.b, 120);

        let font =
            self.style().font.clone().unwrap_or_else(|| Font::simple("sans-serif", self.font_size));

        let item_h = self.item_height() as i32;
        let half_visible = (self.visible_count / 2) as usize;
        let center_x = rect.x + (rect.width as i32) / 2;
        let center_y = rect.y + (rect.height as i32) / 2;

        // Draw a background fill for the entire widget area.
        context.fill_rect(rect, bg_color);

        // Clipping region to ensure items don't spill outside the widget.
        context.push_clip(rect.x, rect.y, rect.width, rect.height);

        // Draw each visible item around the center.
        for offset in 0..=half_visible {
            for &sign in &[1i32, -1i32] {
                if offset == 0 && sign == -1 {
                    continue; // center item only once
                }
                let idx = self.selected_index as i32 + sign * offset as i32;
                if idx < 0 || idx >= self.options.len() as i32 {
                    continue;
                }
                let idx = idx as usize;
                let y = center_y + sign * offset as i32 * item_h;

                let item_rect = Rect::new(rect.x, y - item_h / 2, rect.width, item_h as u32);

                let is_selected = offset == 0;
                if is_selected {
                    // Highlight the selected item in the center.
                    context.fill_rect(item_rect, selected_bg);
                }

                // Draw the option text, measuring to center horizontally.
                let metrics = context.measure_text(&self.options[idx], &font);
                let text_x = center_x - (metrics.width as i32) / 2;
                // Vertically center text within the item row.
                let text_y = y - (metrics.height as i32) / 2;

                let color = if is_selected {
                    selected_text_color
                } else {
                    // Items farther from center are more muted.
                    let fade = 1.0 - (offset as f32 / (half_visible.max(1)) as f32) * 0.5;
                    Color::rgba(
                        (muted_color.r as f32 * fade) as u8,
                        (muted_color.g as f32 * fade) as u8,
                        (muted_color.b as f32 * fade) as u8,
                        muted_color.a,
                    )
                };

                context.draw_text(
                    Point::new(text_x, text_y),
                    &self.options[idx],
                    &font,
                    color,
                    HorizontalAlignment::Left,
                );
            }
        }

        context.pop_clip();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn make_roller() -> Roller {
        let options = vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Cherry".to_string(),
            "Dragonfruit".to_string(),
            "Elderberry".to_string(),
        ];
        Roller::new(options, Rect::new(0, 0, 160, 120))
    }

    #[test]
    fn roller_creation_defaults() {
        let roller = make_roller();
        assert_eq!(roller.options.len(), 5);
        assert_eq!(roller.selected_index(), 0);
        assert_eq!(roller.selected_text(), Some("Apple"));
        assert!(roller.visible_count() >= 3);
        assert!(roller.visible_count() % 2 == 1);
        assert_eq!(roller.kind(), WidgetKind::Roller);
    }

    #[test]
    fn roller_set_selected_index() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_index(), 0);

        roller.set_selected_index(2);
        assert_eq!(roller.selected_index(), 2);
        assert_eq!(roller.selected_text(), Some("Cherry"));

        // Clamp above max.
        roller.set_selected_index(100);
        assert_eq!(roller.selected_index(), 4);
        assert_eq!(roller.selected_text(), Some("Elderberry"));
    }

    #[test]
    fn roller_selected_text() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_text(), Some("Apple"));

        roller.set_selected_index(3);
        assert_eq!(roller.selected_text(), Some("Dragonfruit"));

        // Empty options → no selected text.
        let empty: Roller = Roller::new(Vec::new(), Rect::new(0, 0, 100, 100));
        assert!(empty.selected_text().is_none());
    }

    #[test]
    fn roller_mouse_wheel_changes_selection() {
        let mut roller = make_roller();
        assert_eq!(roller.selected_index(), 0);

        // Scroll down (positive delta) should move to next item.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 1);

        // Scroll down again.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 2);

        // Scroll up (negative delta) should move to previous item.
        roller.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 1);

        // Scroll up past start clamps to 0.
        roller.set_selected_index(0);
        roller.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(roller.selected_index(), 0);

        // Scroll down past end clamps to last.
        roller.set_selected_index(roller.options.len() - 1);
        roller.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(roller.selected_index(), roller.options.len() - 1);
    }

    #[test]
    fn roller_draw_does_not_panic() {
        let mut roller = make_roller();
        // Create a minimal render context for testing.
        let mut backend = SoftwarePaintBackend::new(Size::new(160, 120), 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        roller.draw(&mut ctx);
        backend.end_frame();
        // Also test with empty options.
        let mut empty: Roller = Roller::new(Vec::new(), Rect::new(0, 0, 100, 100));
        let mut backend2 = SoftwarePaintBackend::new(Size::new(100, 100), 1.0);
        backend2.begin_frame(Color::WHITE);
        let mut ctx2 = RenderContext::new(&mut backend2);
        empty.draw(&mut ctx2);
        backend2.end_frame();
    }

    #[test]
    fn roller_set_options_replaces_and_clamps() {
        let mut roller = make_roller();
        roller.set_selected_index(3);
        assert_eq!(roller.selected_text(), Some("Dragonfruit"));

        // Replace with fewer options; index should clamp.
        roller.set_options(vec!["X".to_string(), "Y".to_string()]);
        assert_eq!(roller.options.len(), 2);
        assert_eq!(roller.selected_index(), 1);
        assert_eq!(roller.selected_text(), Some("Y"));

        // Replace with empty list.
        roller.set_options(Vec::new());
        assert!(roller.options.is_empty());
        assert_eq!(roller.selected_index(), 0);
        assert!(roller.selected_text().is_none());
    }

    #[test]
    fn roller_visible_count_is_always_odd() {
        let roller = make_roller();
        assert!(roller.visible_count() % 2 == 1);

        let mut roller = roller;
        roller.set_visible_count(4);
        assert!(roller.visible_count() % 2 == 1);
        assert!(roller.visible_count() >= 3);

        roller.set_visible_count(7);
        assert_eq!(roller.visible_count(), 7);
    }

    #[test]
    fn roller_font_size_accessors() {
        let mut roller = make_roller();
        assert!((roller.font_size() - 16.0).abs() < 0.01);

        roller.set_font_size(20.0);
        assert!((roller.font_size() - 20.0).abs() < 0.01);

        // Clamp to minimum.
        roller.set_font_size(0.0);
        assert!((roller.font_size() - 4.0).abs() < 0.01);
    }

    #[test]
    fn roller_mouse_press_selects_item() {
        let mut roller = make_roller();
        let rect = roller.geometry();
        let item_h = roller.item_height() as i32;
        let center_y = rect.y + (rect.height as i32) / 2;

        // Click one item above center.
        roller.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 10, center_y - item_h),
            button: 1,
        });
        // Should have moved one index up (if available).
        assert_eq!(roller.selected_index(), 0); // already at 0, can't go up

        roller.set_selected_index(2);
        // Click one item below center → index 3.
        roller.handle_event(&Event::MousePress {
            pos: Point::new(rect.x + 10, center_y + item_h),
            button: 1,
        });
        assert_eq!(roller.selected_index(), 3);
    }
}
