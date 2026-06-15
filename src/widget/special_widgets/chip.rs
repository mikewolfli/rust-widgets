//! Chip widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// One chip item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether the chip is selected.
    pub selected: bool,
}

impl ChipItem {
    /// Creates a new chip item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), selected: false }
    }
}

/// Horizontal chip list with single or multi select mode.
pub struct Chip {
    base: BaseWidget,
    items: Vec<ChipItem>,
    multi_select: bool,
    focused_index: Option<usize>,
    chip_padding: i32,
    chip_spacing: i32,
    /// Emitted when chip selection changes. Payload is chip id.
    pub chip_toggled: Signal1<String>,
}

impl Chip {
    /// Creates empty chip widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckListBox, geometry, "Chip"),
            items: Vec::new(),
            multi_select: false,
            focused_index: None,
            chip_padding: 8,
            chip_spacing: 6,
            chip_toggled: Signal1::new(),
        }
    }

    /// Replaces all chip items.
    pub fn set_items(&mut self, items: Vec<ChipItem>) {
        self.items = items;
        self.focused_index = if self.items.is_empty() { None } else { Some(0) };
        if !self.multi_select {
            // Keep only first selected chip in single-select mode.
            let mut selected_seen = false;
            for item in &mut self.items {
                if item.selected {
                    if selected_seen {
                        item.selected = false;
                    }
                    selected_seen = true;
                }
            }
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all chip items.
    pub fn items(&self) -> &[ChipItem] {
        &self.items
    }

    /// Enables/disables multi-select mode.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        if !self.multi_select {
            let mut selected_seen = false;
            for item in &mut self.items {
                if item.selected {
                    if selected_seen {
                        item.selected = false;
                    }
                    selected_seen = true;
                }
            }
        }
        self.base.request_redraw();
    }

    /// Returns whether multi-select is enabled.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Returns focused chip index.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index.filter(|index| *index < self.items.len())
    }

    /// Returns ids of selected chips.
    pub fn selected_ids(&self) -> Vec<&str> {
        self.items.iter().filter(|item| item.selected).map(|item| item.id.as_str()).collect()
    }

    /// Toggles chip selection.
    pub fn toggle_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }

        if self.multi_select {
            self.items[index].selected = !self.items[index].selected;
        } else {
            let next = !self.items[index].selected;
            for item in &mut self.items {
                item.selected = false;
            }
            self.items[index].selected = next;
        }

        let id = self.items[index].id.clone();
        self.chip_toggled.emit(id);
        self.base.request_redraw();
        true
    }

    /// Moves focus by signed delta.
    pub fn move_focus(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.focused_index = None;
            return;
        }
        let current = self.focused_index.unwrap_or(0) as isize;
        let max = self.items.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.focused_index = Some(next);
        self.base.request_redraw();
    }

    fn chip_width(item: &ChipItem, padding: i32) -> i32 {
        (item.label.chars().count() as i32) * 8 + padding * 2
    }

    fn chip_rect(&self, index: usize) -> Option<Rect> {
        let rect = self.geometry();
        let mut x = rect.x + 4;
        for (i, item) in self.items.iter().enumerate() {
            let width = Self::chip_width(item, self.chip_padding).max(10);
            if i == index {
                return Some(Rect::new(x, rect.y + 4, width as u32, rect.height.saturating_sub(8)));
            }
            x += width + self.chip_spacing;
        }
        None
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        for index in 0..self.items.len() {
            let Some(chip) = self.chip_rect(index) else {
                continue;
            };
            if pos.x >= chip.x
                && pos.x < chip.x + chip.width as i32
                && pos.y >= chip.y
                && pos.y < chip.y + chip.height as i32
            {
                return Some(index);
            }
        }
        None
    }
}

impl Widget for Chip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl EventHandler for Chip {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    self.focused_index = Some(index);
                    let _ = self.toggle_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_focus(-1),
                39 => self.move_focus(1),
                13 | 32 => {
                    if let Some(index) = self.focused_index() {
                        let _ = self.toggle_index(index);
                    }
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Chip {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::from_rgb(248, 250, 253));
        context.draw_rect(rect, Color::from_rgb(202, 208, 218));

        for index in 0..self.items.len() {
            let Some(chip_rect) = self.chip_rect(index) else {
                continue;
            };

            let Some(item) = self.items.get(index) else {
                continue;
            };

            let bg = if item.selected {
                Color::from_rgb(196, 220, 248)
            } else if self.focused_index == Some(index) {
                Color::from_rgb(226, 237, 252)
            } else {
                Color::from_rgb(237, 241, 247)
            };
            context.fill_rect(chip_rect, bg);
            context.draw_rect(chip_rect, Color::from_rgb(176, 186, 200));
            context.draw_text(
                Point::new(
                    chip_rect.x + self.chip_padding,
                    chip_rect.y + chip_rect.height as i32 / 2,
                ),
                &item.label,
                &Font::default(),
                Color::from_rgb(32, 44, 61),
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<ChipItem> {
        vec![
            ChipItem::new("bug", "Bug"),
            ChipItem::new("feature", "Feature"),
            ChipItem::new("urgent", "Urgent"),
        ]
    }

    #[test]
    fn single_select_keeps_only_one_selected() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());

        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["bug"]);

        assert!(chip.toggle_index(1));
        assert_eq!(chip.selected_ids(), vec!["feature"]);
    }

    #[test]
    fn multi_select_allows_multiple_selected() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_multi_select(true);

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(2));
        assert_eq!(chip.selected_ids(), vec!["bug", "urgent"]);
    }

    #[test]
    fn chip_toggled_emits_toggled_id() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        chip.chip_toggled.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(chip.toggle_index(2));
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["urgent".to_string()]);
    }

    #[test]
    fn default_state() {
        let chip = Chip::new(Rect::new(0, 0, 800, 600));
        assert!(chip.items().is_empty());
        assert_eq!(chip.focused_index(), None);
        assert!(!chip.multi_select());
        assert!(chip.selected_ids().is_empty());
    }

    #[test]
    fn set_items_adds_chips() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![ChipItem::new("a", "Alpha"), ChipItem::new("b", "Beta")]);

        assert_eq!(chip.items().len(), 2);
        assert_eq!(chip.items()[0].id, "a");
        assert_eq!(chip.items()[1].label, "Beta");
        assert_eq!(chip.focused_index(), Some(0));
    }

    #[test]
    fn empty_items_state() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(Vec::new());

        assert_eq!(chip.focused_index(), None);
        assert!(chip.selected_ids().is_empty());

        // Toggle out of bounds returns false
        assert!(!chip.toggle_index(0));
        assert!(!chip.toggle_index(100));

        // Move focus on empty should not panic
        chip.move_focus(1);
        assert_eq!(chip.focused_index(), None);

        chip.move_focus(-1);
        assert_eq!(chip.focused_index(), None);
    }

    #[test]
    fn invalid_toggle_index() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![ChipItem::new("c1", "Chip 1")]);

        // Out of bounds returns false
        assert!(!chip.toggle_index(5));

        // Valid toggle works
        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["c1"]);
    }

    #[test]
    fn multi_select_toggle() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![
            ChipItem::new("a", "A"),
            ChipItem::new("b", "B"),
            ChipItem::new("c", "C"),
        ]);

        // Enable multi-select after set_items - should preserve no selection
        chip.set_multi_select(true);
        assert!(chip.multi_select());

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(2));
        assert_eq!(chip.selected_ids(), vec!["a", "c"]);

        // Toggle one off
        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["c"]);
    }

    #[test]
    fn set_multi_select_downgrade_preserves_one() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_multi_select(true);
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(1));
        assert_eq!(chip.selected_ids().len(), 2);

        // Switch to single-select
        chip.set_multi_select(false);
        assert!(!chip.multi_select());
        // Should keep only the last selected
        let ids = chip.selected_ids();
        assert_eq!(ids.len(), 1, "single-select must keep at most one selected");
    }

    #[test]
    fn keyboard_focus_and_toggle() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![
            ChipItem::new("a", "A"),
            ChipItem::new("b", "B"),
            ChipItem::new("c", "C"),
        ]);

        // Move right
        chip.handle_event(&Event::key_press(39, 0));
        assert_eq!(chip.focused_index(), Some(1));

        // Toggle with Enter
        chip.handle_event(&Event::key_press(13, 0));
        assert_eq!(chip.selected_ids(), vec!["b"]);

        // Move left
        chip.handle_event(&Event::key_press(37, 0));
        assert_eq!(chip.focused_index(), Some(0));

        // Toggle with Space
        chip.handle_event(&Event::key_press(32, 0));
        assert_eq!(chip.selected_ids(), vec!["a"]);
    }
}
