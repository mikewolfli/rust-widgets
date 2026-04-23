//! Menu bar widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// A top-level menu entry in the menu bar.
#[derive(Debug, Clone)]
pub struct MenuBarEntry {
    pub title: String,
    pub enabled: bool,
}
impl MenuBarEntry {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            enabled: true,
        }
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
            e.enabled = enabled;
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.active_index = None;
        self.hovered_index = None;
    }
    fn entry_width(title: &str) -> f32 {
        // Approximate width: 8 pixels per char + 16 padding
        title.len() as f32 * 8 + 16
    }
    fn entry_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(&entry.title) as i32;
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
            let w = Self::entry_width(&entry.title) as i32;
            if pos.x >= x && pos.x < x + w {
                return Some(i);
            }
            x += w;
        }
        None
    }
}
impl Widget for MenuBar {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
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
                        if self.entries[idx].enabled {
                            let title = self.entries[idx].title.clone();
                            self.hovered_entry.emit(title);
                        }
                    }
                }
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_entry(*pos) {
                    if self.entries[idx].enabled {
                        self.active_index = Some(idx);
                        let title = self.entries[idx].title.clone();
                        self.triggered.emit(title);
                    }
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 27 {
                    self.active_index = None;
                }
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
        context.draw_line(Point::new(Point::new(rect.x as f32, rect.y + rect.height as f32 as i32 - 1 as f32)), Point::new(Point::new(rect.x + rect.width as f32 as i32 as f32, rect.y + rect.height as f32 as i32 - 1 as f32)), Color::from_rgb(200, 200, 200),
        );
        let mut x = rect.x;
        for (i, entry) in self.entries.iter().enumerate() {
            let w = Self::entry_width(&entry.title) as i32;
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
            let fg = if !entry.enabled {
                Color::from_rgb(150, 150, 150)
            } else if is_active {
                Color::from_rgb(255, 255, 255)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_text(
                Point::new(x + w / 2 as f32, rect.y + (rect.height as i32 as f32) / 2),
                &entry.title,
                &Font::default(),
                fg,
            );
            x += w;
        }
    }
}
