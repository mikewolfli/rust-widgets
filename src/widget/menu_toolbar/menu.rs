//! Menu widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// A single item in a menu.
#[derive(Debug, Clone)]
pub struct MenuItem {
    text: String,
    shortcut: String,
    checkable: bool,
    checked: bool,
    enabled: bool,
    separator: bool,
    has_submenu: bool,
}
impl MenuItem {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            shortcut: String::new(),
            checkable: false,
            checked: false,
            enabled: true,
            separator: false,
            has_submenu: false,
        }
    }
    pub fn separator() -> Self {
        let mut m = Self::new("");
        m.set_separator(true);
        m
    }
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = shortcut.into();
        self
    }

    // --- Accessors ---

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }

    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
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

    pub fn has_submenu(&self) -> bool {
        self.has_submenu
    }

    pub fn set_has_submenu(&mut self, has: bool) {
        self.has_submenu = has;
    }
}
/// Menu widget.
pub struct Menu {
    base: BaseWidget,
    title: String,
    items: Vec<MenuItem>,
    hovered_index: Option<usize>,
    pub triggered: Signal1<String>,
    pub triggered_index: Signal1<usize>,
    pub about_to_show: GenericSignal,
    pub about_to_hide: GenericSignal,
}
impl Menu {
    pub fn new(title: impl Into<String>, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "Menu"),
            title: title.into(),
            items: Vec::new(),
            hovered_index: None,
            triggered: Signal1::new(),
            triggered_index: Signal1::new(),
            about_to_show: GenericSignal::new(),
            about_to_hide: GenericSignal::new(),
        }
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }
    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }
    pub fn add_separator(&mut self) {
        self.items.push(MenuItem::separator());
    }
    pub fn add_action(&mut self, text: impl Into<String>) -> usize {
        let idx = self.items.len();
        self.items.push(MenuItem::new(text));
        idx
    }
    pub fn add_action_with_shortcut(
        &mut self,
        text: impl Into<String>,
        shortcut: impl Into<String>,
    ) -> usize {
        let idx = self.items.len();
        self.items.push(MenuItem::new(text).with_shortcut(shortcut));
        idx
    }
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.set_enabled(enabled);
        }
    }
    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            if item.is_checkable() {
                item.set_checked(checked);
            }
        }
    }
    pub fn clear(&mut self) {
        self.items.clear();
    }
    fn item_height() -> f32 {
        22.0
    }
    fn separator_height() -> f32 {
        6.0
    }
    fn _item_rect(&self, index: usize, base_y: f32) -> Rect {
        let rect = self.geometry();
        let mut y = base_y;
        for (i, item) in self.items.iter().enumerate() {
            let h = if item.is_separator() {
                Self::separator_height()
            } else {
                Self::item_height()
            };
            if i == index {
                return Rect {
                    x: rect.x,
                    y: y as i32,
                    width: rect.width,
                    height: h as u32,
                };
            }
            y += h;
        }
        Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }
    fn popup_height(&self) -> f32 {
        self.items
            .iter()
            .map(|item| {
                if item.is_separator() {
                    Self::separator_height()
                } else {
                    Self::item_height()
                }
            })
            .sum::<f32>()
            + 4.0
    }
}
impl Widget for Menu {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn show(&mut self) {
        self.about_to_show.emit();
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
        self.about_to_hide.emit();
    }
}
impl EventHandler for Menu {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for (i, item) in self.items.iter().enumerate() {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator() && pos.y >= y as i32 && pos.y < (y + h) as i32 {
                        self.hovered_index = Some(i);
                        break;
                    }
                    y += h;
                }
            }
            Event::MousePress { pos, button: 1 } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for item in self.items.iter() {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator()
                        && item.is_enabled()
                        && pos.y >= y as i32
                        && pos.y < (y + h) as i32
                    {
                        let text = item.text().to_string();
                        self.triggered.emit(text);
                        self.hide();
                        break;
                    }
                    y += h;
                }
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                let rect = self.geometry();
                let mut y = rect.y as f32 + 2.0;
                for item in &self.items {
                    let h = if item.is_separator() {
                        Self::separator_height()
                    } else {
                        Self::item_height()
                    };
                    if !item.is_separator()
                        && item.is_enabled()
                        && pos.y >= y as i32
                        && pos.y < (y + h) as i32
                    {
                        let text = item.text().to_string();
                        self.triggered.emit(text);
                        self.hide();
                        break;
                    }
                    y += h;
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 27 {
                    self.hide();
                }
                // Escape
                else if *key == 13 {
                    // Enter — trigger hovered
                    if let Some(idx) = self.hovered_index {
                        if let Some(item) = self.items.get(idx) {
                            if !item.is_separator() && item.is_enabled() {
                                let text = item.text().to_string();
                                self.triggered.emit(text);
                                self.hide();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
impl Draw for Menu {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.is_visible() {
            return;
        }
        let rect = self.geometry();
        let popup_h = self.popup_height();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, popup_h as u32),
            Color::from_rgb(250, 250, 250),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, popup_h as u32),
            Color::from_rgb(160, 160, 160),
        );
        let mut y = rect.y as f32 + 2.0;
        for (i, item) in self.items.iter().enumerate() {
            if item.is_separator() {
                let sep_y = y + Self::separator_height() / 2.0;
                context.draw_line(
                    Point::new(rect.x + 4, sep_y as i32),
                    Point::new(rect.x + rect.width as i32 - 4, sep_y as i32),
                    Color::from_rgb(200, 200, 200),
                );
                y += Self::separator_height();
                continue;
            }
            let is_hovered = self.hovered_index == Some(i);
            if is_hovered {
                context.fill_rect(
                    Rect::new(
                        rect.x + 2,
                        y as i32,
                        rect.width - 4,
                        Self::item_height() as u32,
                    ),
                    Color::from_rgb(0, 120, 215),
                );
            }
            let fg = if !item.is_enabled() {
                Color::from_rgb(150, 150, 150)
            } else if is_hovered {
                Color::from_rgb(255, 255, 255)
            } else {
                Color::from_rgb(0, 0, 0)
            };
            if item.is_checkable() {
                let check_sym = if item.is_checked() { "✓" } else { " " };
                context.draw_text(
                    Point::from_f32(rect.x as f32 + 8.0, y + Self::item_height() / 2.0),
                    check_sym,
                    &Font::default(),
                    fg,
                );
            }
            context.draw_text(
                Point::from_f32(rect.x as f32 + 28.0, y + Self::item_height() / 2.0),
                item.text(),
                &Font::default(),
                fg,
            );
            if !item.shortcut().is_empty() {
                context.draw_text(
                    Point::new(
                        rect.x + rect.width as i32 - 8,
                        (y + Self::item_height() / 2.0) as i32,
                    ),
                    item.shortcut(),
                    &Font::default(),
                    fg,
                );
            }
            if item.has_submenu() {
                context.draw_text(
                    Point::new(
                        rect.x + rect.width as i32 - 4,
                        (y + Self::item_height() / 2.0) as i32,
                    ),
                    "▶",
                    &Font::default(),
                    fg,
                );
            }
            y += Self::item_height();
        }
    }
}
