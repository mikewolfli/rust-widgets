use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Widget, WidgetKind};

/// Command link widget for command link buttons.
pub struct CommandLink {
    base: BaseWidget,
    text: String,
    description: String,
    enabled: bool,
    /// Emitted when command link is clicked.
    pub clicked: GenericSignal,
    /// Emitted when command link is hovered.
    pub hovered: Signal1<bool>,
}

impl CommandLink {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CommandLink, geometry, "CommandLink"),
            text: "Command".to_string(),
            description: "".to_string(),
            enabled: true,
            clicked: GenericSignal::new(),
            hovered: Signal1::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
    pub fn set_description(&mut self, description: String) {
        self.description = description;
        self.base.request_redraw();
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.base.request_redraw();
    }

    pub fn click(&self) {
        if self.enabled {
            self.clicked.emit();
        }
    }
}

impl Widget for CommandLink {
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
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
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
        self.enabled && self.base.is_enabled()
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

impl EventHandler for CommandLink {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { button: 1, .. } => {
                if self.enabled {
                    self.clicked.emit();
                }
            }
            Event::MouseEnter { .. } => {
                self.hovered.emit(true);
            }
            Event::MouseLeave { .. } => {
                self.hovered.emit(false);
            }
            _ => {}
        }
    }
}
