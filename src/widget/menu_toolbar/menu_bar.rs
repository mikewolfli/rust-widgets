//! Menu bar widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// A top-level menu entry in the menu bar.
#[derive(Debug, Clone)]
pub struct MenuBarEntry {
    title: String,
    enabled: bool,
}
impl MenuBarEntry {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            enabled: true,
        }
    }

    // --- Accessors ---

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
/// Menu bar widget.
pub struct MenuBar {
    base: BaseWidget,
    entries: Vec<MenuBarEntry>,
    active_index: Option<usize>,
    hovered_index: Option<usize>,
    pub triggered: Signal1<String>,
    pub hovered_entry: Signal1<String>,
}
impl MenuBar {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuBar, geometry, "MenuBar"),
            entries: Vec::new(),
            active_index: None,
            hovered_index: None,
            triggered: Signal1::new(),
            hovered_entry: Signal1::new(),
        }
    }
    pub fn entries(&self) -> &[MenuBarEntry] {
        &self.entries
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active_index
    }
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }
    pub fn add_menu(&mut self, title: impl Into<String>) -> usize {
        let idx = self.entries.len();
        self.entries.push(MenuBarEntry::new(title));
        idx
    }
    pub fn remove_menu(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }
    pub fn set_menu_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(e) = self.entries.get_mut(index) {
            e.set_enabled(enabled);
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.active_index = None;
        self.hovered_index = None;
    }
    fn entry_width(title: &str) -> f32 {
        // Approximate width: 8 pixels per char + 16 padding
        title.len() as f32 * 8.0 + 16.0
    }
    fn _entry_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            if i == index {
                return Rect {
                    x,
                    y: rect.y,
                    width: w as u32,
                    height: rect.height,
                };
            }
            x += w;
        }
        Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
    fn hit_entry(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        if pos.y < rect.y || pos.y > rect.y + rect.height as f32 as i32 {
            return None;
        }
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            if pos.x >= x && pos.x < x + w {
                return Some(i);
            }
            x += w;
        }
        None
    }
}
impl Widget for MenuBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for MenuBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let prev = self.hovered_index;
                self.hovered_index = self.hit_entry(*pos);
                if self.hovered_index != prev {
                    if let Some(idx) = self.hovered_index {
                        if self.entries[idx].is_enabled() {
                            let title = self.entries[idx].title().to_string();
                            self.hovered_entry.emit(title);
                        }
                    }
                }
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_entry(*pos) {
                    if self.entries[idx].is_enabled() {
                        self.active_index = Some(idx);
                        let title = self.entries[idx].title().to_string();
                        self.triggered.emit(title);
                    }
                }
            }
            Event::KeyPress { key, .. } if *key == 27 => {
                self.active_index = None;
            }
            _ => {}
        }
    }
}
impl Draw for MenuBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Menu bar background
        context.fill_rect(rect, Color::from_rgb(240, 240, 240));
        context.draw_line(
            Point::new(rect.x, rect.y + rect.height as i32 - 1),
            Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32 - 1),
            Color::from_rgb(200, 200, 200),
        );
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(entry.title()) as i32;
            let is_hovered = self.hovered_index == Some(i);
            let is_active = self.active_index == Some(i);
            let entry_rect = Rect {
                x,
                y: rect.y,
                width: w as u32,
                height: rect.height,
            };
            if is_active {
                context.fill_rect(entry_rect, Color::from_rgb(0, 120, 215));
            } else if is_hovered {
                context.fill_rect(entry_rect, Color::from_rgb(210, 230, 255));
            }
            let fg = if !entry.is_enabled() {
                Color::from_rgb(150, 150, 150)
            } else if is_active {
                Color::from_rgb(255, 255, 255)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_text(
                Point::new(x + w / 2, rect.y + rect.height as i32 / 2),
                entry.title(),
                &Font::default(),
                fg,
            );
            x += w;
        }
    }
}
