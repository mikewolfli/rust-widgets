//! Action widget — represents a command or toggle that can be placed in menus and toolbars.
use crate::core::{Color, Font, Point};
use crate::core::{ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Represents a user action (command, toggle, etc.) used in menus and toolbars.
pub struct Action {
    base: BaseWidget,
    text: String,
    icon_text: String,
    shortcut: String,
    checkable: bool,
    checked: bool,
    separator: bool,
    pub triggered: Signal1<bool>,
    pub toggled: Signal1<bool>,
    pub hovered: GenericSignal,
    pub changed: GenericSignal,
}
impl Action {
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        Self {
            base: BaseWidget::new(WidgetKind::Action, geometry, &text),
            text: text.clone(),
            icon_text: String::new(),
            shortcut: String::new(),
            checkable: false,
            checked: false,
            separator: false,
            triggered: Signal1::new(),
            toggled: Signal1::new(),
            hovered: GenericSignal::new(),
            changed: GenericSignal::new(),
        }
    }
    pub fn separator(geometry: Rect) -> Self {
        let mut a = Self::new("", geometry);
        a.separator = true;
        a
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn icon_text(&self) -> &str {
        &self.icon_text
    }
    pub fn shortcut(&self) -> &str {
        &self.shortcut
    }
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    pub fn is_separator(&self) -> bool {
        self.separator
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.changed.emit();
    }
    pub fn set_icon_text(&mut self, text: impl Into<String>) {
        self.icon_text = text.into();
        self.changed.emit();
    }
    pub fn set_shortcut(&mut self, shortcut: impl Into<String>) {
        self.shortcut = shortcut.into();
        self.changed.emit();
    }
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
        if !checkable {
            self.checked = false;
        }
        self.changed.emit();
    }
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable && self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
            self.changed.emit();
        }
    }
    pub fn trigger(&mut self) {
        if self.checkable {
            self.set_checked(!self.checked);
        }
        self.triggered.emit(self.checked);
    }
}
impl Widget for Action {
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
impl EventHandler for Action {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { button, .. } if *button == 1 => self.trigger(),
            _ => {}
        }
    }
}
impl Draw for Action {
    fn draw(&mut self, context: &mut RenderContext) {
        // Actions are drawn by their parent menu/toolbar, not directly.
    }
}
