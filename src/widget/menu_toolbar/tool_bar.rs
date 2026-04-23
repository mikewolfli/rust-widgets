//! Tool bar widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Orientation of a toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolBarOrientation {
    Horizontal,
    Vertical,
}
/// A button entry in the toolbar.
#[derive(Debug, Clone)]
pub struct ToolBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: String,
    pub checkable: bool,
    pub checked: bool,
    pub enabled: bool,
    pub separator: bool,
}
impl ToolBarItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
        }
    }
    pub fn separator() -> Self {
        let mut t = Self::new("", "");
        t.separator = true;
        t
    }
}
/// Toolbar widget.
pub struct ToolBar {
    base: BaseWidget,
    orientation: ToolBarOrientation,
    icon_size: f32,
    movable: bool,
    floatable: bool,
    items: Vec<ToolBarItem>,
    hovered_index: Option<usize>,
    pub action_triggered: Signal1<String>,
    pub orientation_changed: Signal1<bool>,
    pub top_level_changed: Signal1<bool>,
    pub visibility_changed: Signal1<bool>,
}
impl ToolBar {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "ToolBar"),
            orientation: ToolBarOrientation::Horizontal,
            icon_size: 24,
            movable: true,
            floatable: true,
            items: Vec::new(),
            hovered_index: None,
            action_triggered: Signal1::new(),
            orientation_changed: Signal1::new(),
            top_level_changed: Signal1::new(),
            visibility_changed: Signal1::new(),
        }
    }
    pub fn orientation(&self) -> ToolBarOrientation {
        self.orientation
    }
    pub fn icon_size(&self) -> f32 {
        self.icon_size
    }
    pub fn is_movable(&self) -> bool {
        self.movable
    }
    pub fn is_floatable(&self) -> bool {
        self.floatable
    }
    pub fn items(&self) -> &[ToolBarItem] {
        &self.items
    }
    pub fn set_orientation(&mut self, o: ToolBarOrientation) {
        let changed = self.orientation != o;
        self.orientation = o;
        if changed {
            self.orientation_changed
                .emit(o == ToolBarOrientation::Horizontal);
        }
    }
    pub fn set_icon_size(&mut self, size: f32) {
        self.icon_size = size.max(8);
    }
    pub fn set_movable(&mut self, v: bool) {
        self.movable = v;
    }
    pub fn set_floatable(&mut self, v: bool) {
        self.floatable = v;
    }
    pub fn add_action(&mut self, id: impl Into<String>, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(ToolBarItem::new(id, text));
        idx
    }
    pub fn add_separator(&mut self) {
        self.items.push(ToolBarItem::separator());
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.enabled = enabled;
        }
    }
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.checkable {
                item.checked = checked;
            }
        }
    }
    fn button_size(&self) -> f32 {
        self.icon_size as u32 + 8
    }
    fn item_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let btn_sz = self.icon_size as u32 + 8;
        let sep_sz = 8u32;
        let mut offset = 2i32;
        for (i, item) in self.items.iter().enumerate() {
            let sz = if item.separator { sep_sz } else { btn_sz };
            if i == index {
                return match self.orientation {
                    ToolBarOrientation::Horizontal => Rect {
                        x: rect.x + offset,
                        y: rect.y + 2,
                        width: sz,
                        height: rect.height.saturating_sub(4),
                    },
                    ToolBarOrientation::Vertical => Rect {
                        x: rect.x + 2,
                        y: rect.y + offset,
                        width: rect.width.saturating_sub(4),
                        height: sz,
                    },
                };
            }
            offset += sz as i32;
        }
        Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
    fn hit_item(&self, pos: Point) -> Option<usize> {
        for i in 0..self.items.len() {
            let r = self.item_rect(i);
            if pos.x >= r.x
                && pos.x <= r.x + r.width as i32
                && pos.y >= r.y
                && pos.y <= r.y + r.height as i32
            {
                return Some(i);
            }
        }
        None
    }
}
impl Widget for ToolBar {
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
impl EventHandler for ToolBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                self.hovered_index = self.hit_item(*pos);
            }
            Event::MousePress { pos, button: 1 } => {
                if let Some(idx) = self.hit_item(*pos) {
                    if let Some(item) = self.items.get_mut(idx) {
                        if item.enabled && !item.separator {
                            if item.checkable {
                                item.checked = !item.checked;
                            }
                            let id = item.id.clone();
                            self.action_triggered.emit(id);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
impl Draw for ToolBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let btn_sz = self.button_size();
        // Background
        context.fill_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            Color::from_rgb(245, 245, 245),
        );
        // Draw bottom border line
        let y = rect.y + rect.height as i32 as i32 - 1;
        context.draw_line(Point::new(Point::new(rect.x, y)), Point::new(Point::new(rect.x + rect.width as i32 as i32, y)), Color::from_rgb(200, 200, 200),
        );
        for i in 0..self.items.len() {
            let item_r = self.item_rect(i);
            let item = &self.items[i];
            if item.separator {
                match self.orientation {
                    ToolBarOrientation::Horizontal => {
                        let mid_x = item_r.x + (item_r.width as i32) / 2;
                        context.draw_line(Point::new(Point::new(mid_x, rect.y + 4)), Point::new(Point::new(mid_x, rect.y + rect.height as i32 as i32 - 4)), Color::from_rgb(200, 200, 200),
                        );
                    }
                    ToolBarOrientation::Vertical => {
                        let mid_y = item_r.y + item_r.height as i32 / 2;
                        context.draw_line(Point::new(rect.x + 4, mid_y), Point::new(rect.x + rect.width as i32 - 4, mid_y), Color::from_rgb(200, 200, 200),
                        );
                    }
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            let bg = if item.checked {
                Color::from_rgb(180, 210, 255)
            } else if is_hovered {
                Color::from_rgb(210, 230, 255)
            } else {
                Color::from_rgb(245, 245, 245)
            };
            context.fill_rect(Rect::new(item_r.x, item_r.y, item_r.width, item_r.height), bg);
            if is_hovered || item.checked {
                context.draw_rect(
                    item_r.x,
                    item_r.y,
                    item_r.width,
                    item_r.height,
                    Color::from_rgb(0, 120, 215),
                );
            }
            let fg = if !item.enabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_text(
                item_r.x + item_r.width as i32 / 2,
                item_r.y + item_r.height as i32 / 2,
                &item.text,
                &Font::default(),
                fg,
                Alignment::Center,
            );
        }
    }
}
