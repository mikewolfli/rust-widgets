//! Base widget state — shared struct used by all concrete controls.

use super::WidgetKind;
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
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
    /// Child widget IDs. Under mini, this is a fixed-capacity `MiniVec<ObjectId, 64>`
    /// (compile-time known size, no heap alloc). Under desktop, `alloc::vec::Vec`.
    pub(crate) children: crate::compat::MiniVec<ObjectId>,
    pub(crate) visible: bool,
    pub(crate) enabled: bool,
    pub(crate) mouse_pressed: bool,
    pub(crate) tooltip: crate::compat::MiniString,
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
            children: crate::compat::MiniVec::new(),
            visible: true,
            enabled: true,
            mouse_pressed: false,
            tooltip: crate::compat::MiniString::new(),
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
    pub fn set_tooltip(&mut self, tooltip: crate::compat::MiniString) {
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
        #[cfg(feature = "desktop")]
        {
            self.tooltip = crate::compat::mini_string_from(crate::i18n::translate(key));
        }
        #[cfg(not(feature = "desktop"))]
        {
            self.tooltip = crate::compat::into_mini(key);
        }
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
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_base() -> BaseWidget {
        BaseWidget::new(WidgetKind::Button, Rect::new(10, 20, 100, 30), "Button")
    }

    #[test]
    fn test_new_creates_widget_with_defaults() {
        let bw = make_base();
        assert_eq!(bw.kind(), WidgetKind::Button);
        assert_eq!(bw.geometry(), Rect::new(10, 20, 100, 30));
        assert!(bw.is_visible());
        assert!(bw.is_enabled());
        assert!(!bw.is_mouse_pressed());
        assert!(bw.tooltip().is_empty());
        assert!((bw.dpi_scale() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_id_unique_per_instance() {
        let a = make_base();
        let b = make_base();
        assert_ne!(a.id(), b.id());
    }

    #[test]
    fn test_set_geometry_updates() {
        let mut bw = make_base();
        bw.set_geometry(Rect::new(0, 0, 200, 50));
        assert_eq!(bw.geometry(), Rect::new(0, 0, 200, 50));
    }

    #[test]
    fn test_min_max_size() {
        let mut bw = make_base();
        assert!(bw.min_size().is_none());
        assert!(bw.max_size().is_none());

        bw.set_min_size(Some(Size::new(50, 20)));
        bw.set_max_size(Some(Size::new(500, 300)));
        assert_eq!(bw.min_size(), Some(Size::new(50, 20)));
        assert_eq!(bw.max_size(), Some(Size::new(500, 300)));
    }

    #[test]
    fn test_parent_and_children() {
        let mut bw = make_base();
        assert!(bw.parent().is_none());
        assert!(bw.children().is_empty());

        bw.set_parent(Some(100));
        assert_eq!(bw.parent(), Some(100));

        bw.add_child(200);
        bw.add_child(300);
        assert_eq!(bw.children(), &[200, 300]);

        bw.remove_child(200);
        assert_eq!(bw.children(), &[300]);
    }

    #[test]
    fn test_show_hide_visibility() {
        let mut bw = make_base();

        bw.hide();
        assert!(!bw.is_visible());

        bw.show();
        assert!(bw.is_visible());
    }

    #[test]
    fn test_set_enabled() {
        let mut bw = make_base();
        assert!(bw.is_enabled());

        bw.set_enabled(false);
        assert!(!bw.is_enabled());

        bw.set_enabled(true);
        assert!(bw.is_enabled());
    }

    #[test]
    fn test_tooltip() {
        let mut bw = make_base();
        assert!(bw.tooltip().is_empty());

        bw.set_tooltip("Help text".to_string());
        assert_eq!(bw.tooltip(), "Help text");
    }

    #[test]
    fn test_dpi_scale_clamps_to_minimum() {
        let mut bw = make_base();

        bw.set_dpi_scale(2.0);
        assert!((bw.dpi_scale() - 2.0).abs() < 0.01);

        bw.set_dpi_scale(0.0); // should clamp to 0.1
        assert!((bw.dpi_scale() - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_contains_point_without_touch_expansion() {
        let bw = make_base(); // Rect(10, 20, 100, 30)
        assert!(bw.contains_point_with_touch_expansion(Point::new(15, 25)));
        assert!(!bw.contains_point_with_touch_expansion(Point::new(0, 0)));
    }

    #[test]
    fn test_contains_point_with_touch_expansion() {
        let mut bw = make_base(); // Rect(10, 20, 100, 30)
        bw.style.touch_target = Some(Size::new(150, 50));

        // Inside expanded area but outside original rect
        assert!(bw.contains_point_with_touch_expansion(Point::new(5, 15)));
        assert!(bw.contains_point_with_touch_expansion(Point::new(115, 55)));

        // Far outside
        assert!(!bw.contains_point_with_touch_expansion(Point::new(-50, -50)));
    }

    #[test]
    fn test_mouse_pressed_state() {
        let mut bw = make_base();
        assert!(!bw.is_mouse_pressed());

        bw.set_mouse_pressed(true);
        assert!(bw.is_mouse_pressed());

        bw.set_mouse_pressed(false);
        assert!(!bw.is_mouse_pressed());
    }

    #[test]
    fn test_style_accessors() {
        let mut bw = make_base();
        let _style = bw.style();
        let _style_mut = bw.style_mut();
        // Just verify methods exist and don't panic
        bw.set_style(WidgetStyle::default());
    }

    #[test]
    fn test_signal_accessors_return_signals() {
        let bw = make_base();
        // All signal accessors should return Some (signal exists)
        let _ = bw.hover_signal();
        let _ = bw.mouse_down_signal();
        let _ = bw.mouse_up_signal();
        let _ = bw.key_down_signal();
        let _ = bw.key_up_signal();
        let _ = bw.focus_gained_signal();
        let _ = bw.focus_lost_signal();
        let _ = bw.redraw_requested_signal();
        let _ = bw.layout_requested_signal();
    }

    #[test]
    fn test_event_routing_mouse_move_emits_hover() {
        let mut bw = make_base();
        let hovered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let last_pos = std::sync::Arc::new(std::sync::Mutex::new(Point::new(0, 0)));
        let h = hovered.clone();
        let lp = last_pos.clone();
        bw.hover.connect(move |p| {
            h.store(true, std::sync::atomic::Ordering::SeqCst);
            *lp.lock().unwrap() = *p;
        });

        bw.handle_event(&Event::MouseMove { pos: Point::new(50, 25) });
        assert!(hovered.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(*last_pos.lock().unwrap(), Point::new(50, 25));
    }

    #[test]
    fn test_event_routing_key_down_emits_signal() {
        let mut bw = make_base();
        let emitted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let emitted_clone = emitted.clone();
        bw.key_down.connect(move |args| {
            let (key, _mods) = *args;
            if key == 65 {
                emitted_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });

        bw.handle_event(&Event::KeyDown((65, 0)));
        assert!(emitted.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_request_redraw_and_layout_emit_signals() {
        let bw = make_base();
        let redrawn = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let laid_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = redrawn.clone();
        let l = laid_out.clone();
        bw.redraw_requested.connect(move || r.store(true, std::sync::atomic::Ordering::SeqCst));
        bw.layout_requested.connect(move || l.store(true, std::sync::atomic::Ordering::SeqCst));

        bw.request_redraw();
        bw.request_layout();

        assert!(redrawn.load(std::sync::atomic::Ordering::SeqCst));
        assert!(laid_out.load(std::sync::atomic::Ordering::SeqCst));
    }
}
