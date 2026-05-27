//! Base widget state — shared struct used by all concrete controls.

use super::WidgetKind;
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;

/// Shared widget state and signals used by concrete controls.
pub struct BaseWidget {
    pub(crate) object: Object,
    pub(crate) kind: WidgetKind,
    pub(crate) geometry: Rect,
    pub(crate) min_size: Option<Size>,
    pub(crate) max_size: Option<Size>,
    pub(crate) parent: Option<ObjectId>,
    pub(crate) children: Vec<ObjectId>,
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    pub(crate) mouse_pressed: bool,
    pub(crate) tooltip: String,
    pub(crate) dpi_scale: f32,
    pub(crate) style: WidgetStyle,
    pub(crate) connection_scope: ConnectionScope,
    /// Emitted when a click-like interaction is received.
    pub clicked: GenericSignal,
    /// Emitted when hover/move interaction is observed.
    pub hover: Signal1<Point>,
    /// Emitted when mouse/pointer button is pressed.
    pub mouse_down: Signal1<(Point, u32)>,
    /// Emitted when mouse/pointer button is released.
    pub mouse_up: Signal1<(Point, u32)>,
    /// Emitted when keyboard key is pressed.
    pub key_down: Signal1<(u32, u32)>,
    /// Emitted when keyboard key is released.
    pub key_up: Signal1<(u32, u32)>,
    /// Emitted when focus-like state is gained.
    pub focus_gained: GenericSignal,
    /// Emitted when focus-like state is lost.
    pub focus_lost: GenericSignal,
    /// Emitted when redraw is requested.
    pub redraw_requested: GenericSignal,
    /// Emitted when layout is requested.
    pub layout_requested: GenericSignal,
    /// Emitted when a stateful value changes (e.g., slider value, checkbox state).
    pub changed: GenericSignal,
}
impl BaseWidget {
    /// Create base widget state and core signals.
    pub fn new(kind: WidgetKind, geometry: Rect, class_name: &'static str) -> Self {
        Self {
            object: Object::new(class_name),
            kind,
            geometry,
            min_size: None,
            max_size: None,
            parent: None,
            children: Vec::new(),
            visible: true,
            enabled: true,
            mouse_pressed: false,
            tooltip: String::new(),
            dpi_scale: 1.0,
            style: WidgetStyle::default(),
            connection_scope: ConnectionScope::new(),
            clicked: GenericSignal::new(),
            hover: Signal1::new(),
            mouse_down: Signal1::new(),
            mouse_up: Signal1::new(),
            key_down: Signal1::new(),
            key_up: Signal1::new(),
            focus_gained: GenericSignal::new(),
            focus_lost: GenericSignal::new(),
            redraw_requested: GenericSignal::new(),
            layout_requested: GenericSignal::new(),
            changed: GenericSignal::new(),
        }
    }
    // -- Base accessors --
    pub fn id(&self) -> ObjectId {
        self.object.id()
    }
    pub fn kind(&self) -> WidgetKind {
        self.kind
    }
    pub fn geometry(&self) -> Rect {
        self.geometry
    }
    pub fn set_geometry(&mut self, geometry: Rect) {
        self.geometry = geometry;
    }
    pub fn min_size(&self) -> Option<Size> {
        self.min_size
    }
    pub fn max_size(&self) -> Option<Size> {
        self.max_size
    }
    pub fn set_min_size(&mut self, min_size: Option<Size>) {
        self.min_size = min_size;
    }
    pub fn set_max_size(&mut self, max_size: Option<Size>) {
        self.max_size = max_size;
    }
    pub fn parent(&self) -> Option<ObjectId> {
        self.parent
    }
    pub fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.parent = parent;
    }
    pub fn children(&self) -> &[ObjectId] {
        &self.children
    }
    pub fn add_child(&mut self, child: ObjectId) {
        self.children.push(child);
    }
    pub fn remove_child(&mut self, child: ObjectId) {
        self.children.retain(|&id| id != child);
    }
    pub fn show(&mut self) {
        self.visible = true;
    }
    pub fn hide(&mut self) {
        self.visible = false;
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    pub fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }
    pub fn set_dpi_scale(&mut self, scale: f32) {
        self.dpi_scale = scale.max(0.1);
    }
    pub fn set_translated_tooltip(&mut self, key: &str) {
        self.tooltip = crate::i18n::translate(key);
    }
    pub fn style(&self) -> &WidgetStyle {
        &self.style
    }
    pub fn style_mut(&mut self) -> &mut WidgetStyle {
        &mut self.style
    }
    /// Check if a point is within this widget's geometry, optionally expanded
    /// to meet the minimum touch target size set in `WidgetStyle.touch_target`.
    ///
    /// When `touch_target` is set and the widget's visual geometry is smaller
    /// than that target, the effective hit-test area is expanded outward while
    /// keeping the visual center unchanged.
    pub fn contains_point_with_touch_expansion(&self, point: Point) -> bool {
        let rect = match self.style.touch_target {
            Some(min_size) => self.geometry.expand_to_touch_target(min_size),
            None => self.geometry,
        };
        rect.contains_point(point)
    }
    pub fn set_style(&mut self, style: WidgetStyle) {
        self.style = style;
    }
    pub fn connection_scope(&self) -> &ConnectionScope {
        &self.connection_scope
    }
    pub fn hover_signal(&self) -> &Signal1<Point> {
        &self.hover
    }
    pub fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_down
    }
    pub fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_up
    }
    pub fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_down
    }
    pub fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_up
    }
    pub fn focus_gained_signal(&self) -> &GenericSignal {
        &self.focus_gained
    }
    pub fn focus_lost_signal(&self) -> &GenericSignal {
        &self.focus_lost
    }
    pub fn redraw_requested_signal(&self) -> &GenericSignal {
        &self.redraw_requested
    }
    pub fn layout_requested_signal(&self) -> &GenericSignal {
        &self.layout_requested
    }
    pub fn is_mouse_pressed(&self) -> bool {
        self.mouse_pressed
    }
    pub fn set_mouse_pressed(&mut self, pressed: bool) {
        self.mouse_pressed = pressed;
    }
    pub fn paint(&mut self, context: &mut RenderContext) {
        // Default paint implementation - subclasses should override
        let _ = context;
    }
    pub fn request_redraw(&self) {
        self.redraw_requested.emit();
    }
    pub fn request_layout(&self) {
        self.layout_requested.emit();
    }
}
impl EventHandler for BaseWidget {
    fn handle_event(&mut self, event: &Event) {
        // Default event routing: delegate to typed signals
        match event {
            Event::MouseMove { pos } => {
                self.hover.emit(*pos);
            }
            Event::MouseDown((pos, button)) => {
                self.mouse_down.emit((*pos, *button));
            }
            Event::MouseUp((pos, button)) => {
                self.mouse_up.emit((*pos, *button));
            }
            Event::KeyDown((key, modifiers)) => {
                self.key_down.emit((*key, *modifiers));
            }
            Event::KeyUp((key, modifiers)) => {
                self.key_up.emit((*key, *modifiers));
            }
            _ => {}
        }
    }
}
