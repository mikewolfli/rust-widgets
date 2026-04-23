//! Radio button widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{BaseWidget, Draw, Image, Widget, WidgetKind};
/// Radio button widget.
pub struct RadioButton {
    base: BaseWidget,
    checked: bool,
    group_id: Option<String>,
    pub selected: GenericSignal,
    pub checked_changed: Signal1<bool>,
}
impl RadioButton {
    /// Creates an unchecked radio button with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RadioButton, geometry, "RadioButton"),
            checked: false,
            group_id: None,
            selected: GenericSignal::new(),
            checked_changed: Signal1::new(),
        }
    }
    /// Returns current checked state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Sets optional group identifier.
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
    }
    /// Returns optional group identifier.
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    /// Sets checked state and emits deterministic signals.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.checked_changed.emit(checked);
        if checked {
            self.selected.emit();
        }
    }
    /// Selects one radio button within a peer group.
    pub fn select_in_group(peers: &mut [&mut RadioButton], selected_index: usize) -> bool {
        if selected_index >= peers.len() {
            return false;
        }
        let selected_group = peers[selected_index].group_id.clone();
        for (index, peer) in peers.iter_mut().enumerate() {
            if selected_group.is_some() && peer.group_id != selected_group {
                continue;
            }
            peer.set_checked(index == selected_index);
        }
        true
    }
}
// Implement Widget trait
impl Widget for RadioButton {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
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
    fn set_enabled(&mut self, enabled: bool) {
        self.base.set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
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
impl EventHandler for RadioButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    self.set_checked(true);
                    self.base.clicked.emit();
                }
            }
            Event::KeyPress { key, modifiers: _ } => {
                match *key {
                    32 | 13 => {
                        // Space or Enter
                        self.set_checked(true);
                        self.base.clicked.emit();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
impl Draw for RadioButton {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw radio button
        let rect = self.geometry();
        let center = Point::new(
            rect.x + rect.width as i32 / 2,
            rect.y + rect.height as i32 / 2,
        );
        let radius = (rect.height.min(rect.width) / 4) as u32;
        // Draw outer circle
        context.draw_circle(
            center,
            radius,
            Color::from_rgb(100 as u32, 100, 100),
        );
        // Draw inner circle if checked
        if self.checked {
            let inner_radius = radius / 2;
            context.fill_circle(
                center,
                inner_radius,
                Color::from_rgb(0 as u32, 120, 215),
            );
        }
    }
}
