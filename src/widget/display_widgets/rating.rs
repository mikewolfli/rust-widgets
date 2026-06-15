//! Rating widget — a star rating control (like 1-5 stars).
//!
//! The Rating widget displays a horizontal row of stars that the user can click
//! to set a rating value. Filled stars (★) are drawn in gold for the rated
//! portion, while unrated stars (☆) are drawn in gray outline.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Star rating widget for selecting a rating from 1 to N stars.
pub struct Rating {
    base: BaseWidget,
    rating: u32,
    max_rating: u32,
    star_size: u32,
    /// Emitted when the rating value changes.
    pub rating_changed: Signal1<u32>,
}

impl Rating {
    /// Creates a new Rating widget with the given geometry.
    /// Defaults to 5 stars with a rating of 0 (no stars selected).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Rating, geometry, "Rating"),
            rating: 0,
            max_rating: 5,
            star_size: 24,
            rating_changed: Signal1::new(),
        }
    }

    /// Returns the current rating value.
    pub fn rating(&self) -> u32 {
        self.rating
    }

    /// Sets the rating value, clamped to `[0, max_rating]`.
    /// Emits `rating_changed` if the value actually changes.
    pub fn set_rating(&mut self, rating: u32) {
        let clamped = rating.min(self.max_rating);
        if self.rating != clamped {
            self.rating = clamped;
            self.rating_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the maximum rating value.
    pub fn max_rating(&self) -> u32 {
        self.max_rating
    }

    /// Sets the maximum rating value.
    /// The current rating is clamped to the new maximum.
    pub fn set_max_rating(&mut self, max_rating: u32) {
        let max = max_rating.max(1);
        self.max_rating = max;
        if self.rating > max {
            self.rating = max;
            self.rating_changed.emit(max);
        }
        self.base.request_redraw();
    }

    /// Returns the size of each star in pixels.
    pub fn star_size(&self) -> u32 {
        self.star_size
    }

    /// Sets the size of each star in pixels.
    pub fn set_star_size(&mut self, size: u32) {
        let size = size.max(8);
        self.star_size = size;
        self.base.request_redraw();
    }
}

impl Widget for Rating {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Rating {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // Clear background
        context.fill_rect(rect, Color::rgba(245, 245, 245, 255));

        let gap = 4;
        let total_width = self.max_rating * self.star_size + (self.max_rating - 1) * gap;
        let start_x = rect.x + (rect.width as i32 - total_width as i32).max(0) / 2;
        let center_y = rect.y + rect.height as i32 / 2;

        let filled_color = Color::GOLD;
        let empty_color = if is_enabled {
            Color::rgba(180, 180, 180, 200)
        } else {
            Color::rgba(200, 200, 200, 100)
        };

        let font = Font::default();

        for i in 0..self.max_rating {
            let star_x = start_x + (i * (self.star_size + gap)) as i32;
            let is_filled = i < self.rating;

            let ch = if is_filled { "★" } else { "☆" };
            let color = if is_filled { filled_color } else { empty_color };

            // Center the character vertically and horizontally within its star cell
            let text_point = Point::new(star_x + self.star_size as i32 / 2, center_y);

            context.draw_text(text_point, ch, &font, color, HorizontalAlignment::Left);
        }
    }
}

impl EventHandler for Rating {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                let rect = self.geometry();
                let gap = 4;
                let total_width = self.max_rating * self.star_size + (self.max_rating - 1) * gap;
                let start_x = rect.x + (rect.width as i32 - total_width as i32).max(0) / 2;

                if pos.y < rect.y || pos.y >= rect.y + rect.height as i32 {
                    return;
                }

                let rel_x = pos.x - start_x;
                if rel_x < 0 {
                    return;
                }

                let step = (self.star_size + gap) as i32;
                let index = (rel_x / step) as u32;
                if index < self.max_rating {
                    // Clicking the same star as current rating toggles between
                    // that star and clearing, depending on whether we click
                    // the same or a different star.
                    let new_rating = if self.rating == index + 1 { index } else { index + 1 };
                    self.set_rating(new_rating);
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
    fn rating_default_values() {
        let r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.rating(), 0);
        assert_eq!(r.max_rating(), 5);
        assert_eq!(r.star_size(), 24);
        assert_eq!(r.kind(), WidgetKind::Rating);
    }

    #[test]
    fn rating_set_rating_emits_signal() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let captured = Arc::new(Mutex::new(None));
        r.rating_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<u32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        r.set_rating(3);
        assert_eq!(r.rating(), 3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn rating_set_rating_clamped() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(10);
        assert_eq!(r.rating(), 5);
    }

    #[test]
    fn rating_set_rating_zero() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(4);
        assert_eq!(r.rating(), 4);
        r.set_rating(0);
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_set_max_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(5);
        r.set_max_rating(3);
        assert_eq!(r.max_rating(), 3);
        assert_eq!(r.rating(), 3);
    }

    #[test]
    fn rating_max_rating_min_one() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_max_rating(0);
        assert_eq!(r.max_rating(), 1);
    }

    #[test]
    fn rating_star_size_get_set() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        assert_eq!(r.star_size(), 24);
        r.set_star_size(32);
        assert_eq!(r.star_size(), 32);
    }

    #[test]
    fn rating_star_size_min_eight() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_star_size(2);
        assert_eq!(r.star_size(), 8);
    }

    #[test]
    fn rating_mouse_press_sets_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        // With star_size=24, gap=4, max=5: total_width=136, start_x=(200-136)/2=32
        // Star 0: [32..56), Star 1: [60..84), Star 2: [88..112), Star 3: [116..140)
        // Press the 3rd star (index 2 => rating 3)
        r.handle_event(&Event::MouseRelease { pos: Point::new(100, 20), button: 1 });
        assert_eq!(r.rating(), 3);
    }

    #[test]
    fn rating_mouse_press_toggles_current_star() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(3);
        // Click on the 3rd star again (index 2 => rating 3) should set rating back to 2
        r.handle_event(&Event::MouseRelease { pos: Point::new(100, 20), button: 1 });
        assert_eq!(r.rating(), 2);
    }

    #[test]
    fn rating_mouse_press_out_of_bounds() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.handle_event(&Event::MousePress { pos: Point::new(300, 20), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_mouse_press_before_first_star() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.handle_event(&Event::MousePress { pos: Point::new(-10, 20), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_mouse_press_outside_vertically() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.handle_event(&Event::MousePress { pos: Point::new(30, 100), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_disabled_blocks_events() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_enabled(false);
        r.handle_event(&Event::MousePress { pos: Point::new(40, 20), button: 1 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_right_click_ignored() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.handle_event(&Event::MousePress { pos: Point::new(40, 20), button: 2 });
        assert_eq!(r.rating(), 0);
    }

    #[test]
    fn rating_same_value_no_emit() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        let count = Arc::new(Mutex::new(0usize));
        r.rating_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<u32>| {
                *count.lock().unwrap() += 1;
            }
        });

        r.set_rating(3);
        r.set_rating(3); // should not emit again
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn rating_mouse_release_also_sets_rating() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.handle_event(&Event::MouseRelease { pos: Point::new(40, 20), button: 1 });
        assert_eq!(r.rating(), 1);
    }

    #[test]
    fn rating_svg_output() {
        let mut r = Rating::new(Rect::new(0, 0, 200, 40));
        r.set_rating(3);
        let svg = crate::widget::svg::render_to_svg(&mut r);
        assert!(svg.starts_with("<svg"));
    }
}
