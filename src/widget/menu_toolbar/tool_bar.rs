//! Tool bar widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
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
    id: String,
    text: String,
    tooltip: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
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
        t.set_separator(true);
        t
    }

    // --- Accessors ---

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn set_id(&mut self, id: impl Into<String>) {
        self.id = id.into();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    pub fn set_tooltip(&mut self, tooltip: impl Into<String>) {
        self.tooltip = tooltip.into();
    }

    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_separator(&self) -> bool {
        self.separator
    }

    pub fn set_separator(&mut self, separator: bool) {
        self.separator = separator;
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
            icon_size: 24.0,
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
        self.icon_size = size.max(8.0);
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
            item.set_enabled(enabled);
        }
    }
    /// Returns enabled state for item at index.
    pub fn item_enabled(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_enabled())
    }
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
    }
    /// Returns checked state for item at index.
    pub fn item_checked(&self, index: usize) -> Option<bool> {
        self.items.get(index).map(|item| item.is_checked())
    }
    fn button_size(&self) -> f32 {
        self.icon_size + 8.0
    }
    fn item_rect(&self, index: usize) -> Rect {
        let rect = self.geometry();
        let btn_sz = self.icon_size as u32 + 8;
        let sep_sz = 8u32;
        let mut offset = 2i32;
        for (i, item) in self.items.iter().enumerate() {
            let sz = if item.is_separator() { sep_sz } else { btn_sz };
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
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
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
                        if item.is_enabled() && !item.is_separator() {
                            if item.is_checkable() {
                                item.set_checked(!item.is_checked());
                            }
                            let id = item.id().to_string();
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
        let _btn_sz = self.button_size();
        // Background
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(245, 245, 245),
        );
        // Draw bottom border line
        let y = rect.y + rect.height as f32 as i32 - 1;
        context.draw_line(
            Point::new(rect.x, y),
            Point::new(rect.x + rect.width as i32, y),
            Color::from_rgb(200, 200, 200),
        );
        for i in 0..self.items.len() {
            let item_r = self.item_rect(i);
            let item = &self.items[i];
            if item.is_separator() {
                match self.orientation {
                    ToolBarOrientation::Horizontal => {
                        let mid_x = item_r.x + (item_r.width as i32) / 2;
                        context.draw_line(
                            Point::new(mid_x, rect.y + 4),
                            Point::new(mid_x, rect.y + rect.height as i32 - 4),
                            Color::from_rgb(200, 200, 200),
                        );
                    }
                    ToolBarOrientation::Vertical => {
                        let mid_y = item_r.y + item_r.height as i32 / 2;
                        context.draw_line(
                            Point::new(rect.x + 4, mid_y),
                            Point::new(rect.x + rect.width as i32 - 4, mid_y),
                            Color::from_rgb(200, 200, 200),
                        );
                    }
                }
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            let bg = if item.is_checked() {
                Color::from_rgb(180, 210, 255)
            } else if is_hovered {
                Color::from_rgb(210, 230, 255)
            } else {
                Color::from_rgb(245, 245, 245)
            };
            context.fill_rect(
                Rect::new(item_r.x, item_r.y, item_r.width, item_r.height),
                bg,
            );
            if is_hovered || item.is_checked() {
                context.draw_rect(
                    Rect::new(item_r.x, item_r.y, item_r.width, item_r.height),
                    Color::from_rgb(0, 120, 215),
                );
            }
            let fg = if !item.is_enabled() {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            context.draw_text(
                Point::new(
                    item_r.x + item_r.width as i32 / 2,
                    item_r.y + item_r.height as i32 / 2,
                ),
                item.text(),
                &Font::default(),
                fg,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_item_state_accessors_handle_valid_and_oob_indices() {
        let mut tool_bar = ToolBar::new(Rect::new(0, 0, 240, 36));
        let idx = tool_bar.add_action("save", "Save");

        assert_eq!(tool_bar.item_enabled(idx), Some(true));
        tool_bar.set_item_enabled(idx, false);
        assert_eq!(tool_bar.item_enabled(idx), Some(false));
        assert_eq!(tool_bar.item_enabled(99), None);

        tool_bar.items[idx].set_checkable(true);
        assert_eq!(tool_bar.item_checked(idx), Some(false));
        tool_bar.set_item_checked(idx, true);
        assert_eq!(tool_bar.item_checked(idx), Some(true));
        assert_eq!(tool_bar.item_checked(99), None);
    }
}
