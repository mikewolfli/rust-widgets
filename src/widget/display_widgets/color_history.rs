//! ColorHistory widget — a compact color history picker for design tools.
//!
//! Displays a grid of recently used color swatches. Supports adding colors,
//! removing them, clearing the history, and selecting a color via click.
//! Hovering over a swatch emits a preview signal.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

const SWATCHES_PER_ROW: u32 = 5;
const SWATCH_SIZE: u32 = 20;
const SWATCH_PADDING: u32 = 4;

/// A color history picker that displays recently used colors in a grid.
pub struct ColorHistory {
    base: BaseWidget,
    colors: Vec<Color>,
    max_colors: usize,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    /// Emitted when a color swatch is clicked/selected.
    pub color_selected: Signal1<Color>,
    /// Emitted when the pointer hovers over a color swatch.
    pub color_hovered: Signal1<Color>,
}

impl ColorHistory {
    /// Creates a new ColorHistory widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorHistory, geometry, "ColorHistory"),
            colors: Vec::new(),
            max_colors: 20,
            selected_index: None,
            hovered_index: None,
            color_selected: Signal1::new(),
            color_hovered: Signal1::new(),
        }
    }

    /// Adds a color to the history. If at capacity, removes the oldest color.
    /// Prevents duplicate consecutive entries.
    pub fn add_color(&mut self, color: Color) {
        // Avoid adding the same color as the most recent entry
        if self.colors.last() == Some(&color) {
            return;
        }
        if self.colors.len() >= self.max_colors {
            self.colors.remove(0);
        }
        self.colors.push(color);
        self.base.request_redraw();
    }

    /// Removes a color at the given index. Returns `true` if successful.
    pub fn remove_color(&mut self, index: usize) -> bool {
        if index < self.colors.len() {
            self.colors.remove(index);
            // Adjust selected index
            if let Some(sel) = self.selected_index {
                if sel == index {
                    self.selected_index = None;
                } else if sel > index {
                    self.selected_index = Some(sel - 1);
                }
            }
            self.base.request_redraw();
            return true;
        }
        false
    }

    /// Clears all colors in the history.
    pub fn clear_history(&mut self) {
        self.colors.clear();
        self.selected_index = None;
        self.hovered_index = None;
        self.base.request_redraw();
    }

    /// Returns a slice of all stored colors.
    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    /// Returns the currently selected index, if any.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sets the selected index. Pass `None` to clear selection.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        if self.selected_index != index {
            self.selected_index = index;
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected color, if any.
    pub fn selected_color(&self) -> Option<Color> {
        self.selected_index.and_then(|i| self.colors.get(i).copied())
    }

    /// Returns the maximum number of colors that can be stored.
    pub fn max_colors(&self) -> usize {
        self.max_colors
    }

    /// Sets the maximum number of colors. Truncates if current colors exceed the new limit.
    pub fn set_max_colors(&mut self, max: usize) {
        self.max_colors = max;
        while self.colors.len() > max {
            self.colors.remove(0);
        }
        if self.selected_index.is_some_and(|i| i >= self.colors.len()) {
            self.selected_index = None;
        }
        self.base.request_redraw();
    }

    /// Returns the index of the swatch at the given position, if any.
    fn swatch_at(&self, x: i32, y: i32) -> Option<usize> {
        let rect = self.geometry();
        let x = x - rect.x;
        let y = y - rect.y;
        if x < 0 || y < 0 {
            return None;
        }
        let col = x as u32 / (SWATCH_SIZE + SWATCH_PADDING);
        let row = y as u32 / (SWATCH_SIZE + SWATCH_PADDING);
        let index = (row * SWATCHES_PER_ROW + col) as usize;
        if index < self.colors.len() {
            Some(index)
        } else {
            None
        }
    }
}

impl Widget for ColorHistory {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ColorHistory {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Draw background
        context.fill_rect(rect, Color::rgba(240, 240, 240, 255));

        // Draw each color swatch in a grid
        for (i, color) in self.colors.iter().enumerate() {
            let row = i as u32 / SWATCHES_PER_ROW;
            let col = i as u32 % SWATCHES_PER_ROW;
            let x =
                rect.x + (col * (SWATCH_SIZE + SWATCH_PADDING)) as i32 + SWATCH_PADDING as i32 / 2;
            let y =
                rect.y + (row * (SWATCH_SIZE + SWATCH_PADDING)) as i32 + SWATCH_PADDING as i32 / 2;

            let swatch_rect = Rect::new(x, y, SWATCH_SIZE, SWATCH_SIZE);

            // Checkerboard for transparent colors
            if color.a < 255 {
                let checker_size: u32 = 3;
                let even = Color::rgba(200, 200, 200, 255);
                let odd = Color::rgba(255, 255, 255, 255);
                for cy in (y..y + SWATCH_SIZE as i32).step_by(checker_size as usize) {
                    for cx in (x..x + SWATCH_SIZE as i32).step_by(checker_size as usize) {
                        let tile_x = (cx - x) / checker_size as i32;
                        let tile_y = (cy - y) / checker_size as i32;
                        let tile_color = if (tile_x + tile_y) % 2 == 0 { even } else { odd };
                        let tw = checker_size.min((x + SWATCH_SIZE as i32 - cx) as u32);
                        let th = checker_size.min((y + SWATCH_SIZE as i32 - cy) as u32);
                        context.fill_rect(Rect::new(cx, cy, tw, th), tile_color);
                    }
                }
            }

            context.fill_rect(swatch_rect, *color);

            // Selected highlight border
            if Some(i) == self.selected_index {
                context.draw_rect_stroke(swatch_rect, Color::rgba(0, 0, 0, 200), 2);
            } else {
                context.draw_rect_stroke(swatch_rect, Color::rgba(0, 0, 0, 40), 1);
            }

            // Hovered highlight
            if Some(i) == self.hovered_index && Some(i) != self.selected_index {
                context.draw_rect_stroke(swatch_rect, Color::rgba(0, 120, 255, 180), 2);
            }
        }
    }
}

impl EventHandler for ColorHistory {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(index) = self.swatch_at(pos.x, pos.y) {
                    self.selected_index = Some(index);
                    self.color_selected.emit(self.colors[index]);
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } => {
                let new_hover = self.swatch_at(pos.x, pos.y);
                if new_hover != self.hovered_index {
                    self.hovered_index = new_hover;
                    if let Some(index) = new_hover {
                        self.color_hovered.emit(self.colors[index]);
                    }
                    self.base.request_redraw();
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
    use std::sync::Arc;

    #[test]
    fn color_history_initial_state() {
        let ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        assert!(ch.colors().is_empty());
        assert_eq!(ch.selected_index(), None);
        assert_eq!(ch.selected_color(), None);
        assert_eq!(ch.max_colors(), 20);
        assert_eq!(ch.kind(), WidgetKind::ColorHistory);
    }

    #[test]
    fn color_history_add_and_retrieve() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);

        assert_eq!(ch.colors().len(), 3);
        assert_eq!(ch.colors()[0], Color::RED);
        assert_eq!(ch.colors()[1], Color::GREEN);
        assert_eq!(ch.colors()[2], Color::BLUE);
    }

    #[test]
    fn color_history_remove_color() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);

        assert!(ch.remove_color(1));
        assert_eq!(ch.colors().len(), 2);
        assert_eq!(ch.colors()[0], Color::RED);
        assert_eq!(ch.colors()[1], Color::BLUE);

        assert!(!ch.remove_color(5));
        assert_eq!(ch.colors().len(), 2);
    }

    #[test]
    fn color_history_clear() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.clear_history();
        assert!(ch.colors().is_empty());
        assert_eq!(ch.selected_index(), None);
    }

    #[test]
    fn color_history_selection() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);

        ch.set_selected_index(Some(1));
        assert_eq!(ch.selected_index(), Some(1));
        assert_eq!(ch.selected_color(), Some(Color::GREEN));

        ch.set_selected_index(None);
        assert_eq!(ch.selected_index(), None);
    }

    #[test]
    fn color_history_signal_on_click() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);

        let selected = Arc::new(std::sync::Mutex::new(Color::BLACK));
        let sel_clone = selected.clone();
        ch.color_selected.connect(move |color| {
            *sel_clone.lock().unwrap() = *color;
        });

        // Click on the second swatch
        let swatch_x = (SWATCH_SIZE + SWATCH_PADDING) as i32 + SWATCH_PADDING as i32 / 2;
        ch.handle_event(&Event::MousePress {
            pos: Point::new(swatch_x, SWATCH_PADDING as i32 / 2),
            button: 1,
        });

        assert_eq!(*selected.lock().unwrap(), Color::GREEN);
    }

    #[test]
    fn color_history_prevents_duplicate_consecutive() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.add_color(Color::RED);
        ch.add_color(Color::RED); // Should be ignored (same as last)
        assert_eq!(ch.colors().len(), 1);
    }

    #[test]
    fn color_history_max_colors_respected() {
        let mut ch = ColorHistory::new(Rect::new(0, 0, 200, 100));
        ch.set_max_colors(3);
        ch.add_color(Color::RED);
        ch.add_color(Color::GREEN);
        ch.add_color(Color::BLUE);
        ch.add_color(Color::WHITE); // Should evict RED

        assert_eq!(ch.colors().len(), 3);
        assert_eq!(ch.colors()[0], Color::GREEN);
        assert_eq!(ch.colors()[2], Color::WHITE);
    }
}
