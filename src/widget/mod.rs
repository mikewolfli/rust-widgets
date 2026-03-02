//! Widget models and controls.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    Dialog,
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    TextEdit,
    ComboBox,
    SpinBox,
    ListBox,
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    Panel,
    GroupBox,
    TabWidget,
    StackWidget,
    MenuBar,
    Menu,
    ToolBar,
    StatusBar,
    Canvas,
    Table,
    Grid,
    /// Chart surface widget.
    Chart,
}

/// Common widget contract implemented by all widget models.
pub trait Widget: EventHandler {
    /// Get stable widget id.
    fn id(&self) -> ObjectId;
    /// Get widget runtime kind.
    fn kind(&self) -> WidgetKind;
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    /// Returns widget rectangle aliasing `geometry()`.
    fn rect(&self) -> Rect {
        self.geometry()
    }
    /// Sets widget rectangle aliasing `set_geometry()`.
    fn set_rect(&mut self, rect: Rect) {
        self.set_geometry(rect);
    }
    /// Returns widget position from its geometry origin.
    fn position(&self) -> Point {
        self.geometry().position()
    }
    /// Returns widget size from its geometry extent.
    fn size(&self) -> Size {
        self.geometry().size()
    }
    /// Updates widget position while preserving size.
    fn set_position(&mut self, position: Point) {
        self.set_geometry(Rect::from_position_size(position, self.size()));
    }
    /// Updates widget size while preserving position.
    fn set_size(&mut self, size: Size) {
        self.set_geometry(Rect::from_position_size(self.position(), size));
    }
    /// Returns minimum size constraint when configured.
    fn min_size(&self) -> Option<Size>;
    /// Returns maximum size constraint when configured.
    fn max_size(&self) -> Option<Size>;
    /// Sets minimum size constraint.
    fn set_min_size(&mut self, min_size: Option<Size>);
    /// Sets maximum size constraint.
    fn set_max_size(&mut self, max_size: Option<Size>);
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);
    fn children(&self) -> &[ObjectId];
    /// Show widget.
    fn show(&mut self);
    /// Hide widget.
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn is_enabled(&self) -> bool;
    fn set_tooltip(&mut self, tooltip: String);
    fn tooltip(&self) -> &str;
    fn style(&self) -> &WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
    /// Returns optional background color shorthand.
    fn background_color(&self) -> Option<Color> {
        self.style().background_color
    }
    /// Sets optional background color shorthand.
    fn set_background_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.background_color = color;
        self.set_style(style);
    }
    /// Returns optional foreground (text) color shorthand.
    fn foreground_color(&self) -> Option<Color> {
        self.style().text_color
    }
    /// Sets optional foreground (text) color shorthand.
    fn set_foreground_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.text_color = color;
        self.set_style(style);
    }
    /// Returns optional font shorthand.
    fn font(&self) -> Option<&Font> {
        self.style().font.as_ref()
    }
    /// Sets optional font shorthand.
    fn set_font(&mut self, font: Option<Font>) {
        let mut style = self.style().clone();
        style.font = font;
        self.set_style(style);
    }
    /// Returns optional border color shorthand.
    fn border_color(&self) -> Option<Color> {
        self.style().border_color
    }
    /// Returns border width shorthand.
    fn border_width(&self) -> u32 {
        self.style().border_width
    }
    /// Returns border radius shorthand.
    fn border_radius(&self) -> u32 {
        self.style().border_radius
    }
    /// Sets optional border color shorthand.
    fn set_border_color(&mut self, color: Option<Color>) {
        let mut style = self.style().clone();
        style.border_color = color;
        self.set_style(style);
    }
    /// Sets border width shorthand.
    fn set_border_width(&mut self, width: u32) {
        let mut style = self.style().clone();
        style.border_width = width;
        self.set_style(style);
    }
    /// Sets border radius shorthand.
    fn set_border_radius(&mut self, radius: u32) {
        let mut style = self.style().clone();
        style.border_radius = radius;
        self.set_style(style);
    }
    /// Sets border shorthand in one call.
    fn set_border(&mut self, color: Option<Color>, width: u32, radius: u32) {
        let mut style = self.style().clone();
        style.border_color = color;
        style.border_width = width;
        style.border_radius = radius;
        self.set_style(style);
    }
    /// Returns current per-side content padding.
    fn padding(&self) -> &Padding {
        &self.style().padding
    }
    /// Returns current per-side outer margin.
    fn margin(&self) -> &Margin {
        &self.style().margin
    }
    /// Updates widget content padding while preserving other style properties.
    fn set_padding(&mut self, padding: Padding) {
        let mut style = self.style().clone();
        style.padding = padding;
        self.set_style(style);
    }
    /// Updates widget margin while preserving other style properties.
    fn set_margin(&mut self, margin: Margin) {
        let mut style = self.style().clone();
        style.margin = margin;
        self.set_style(style);
    }
    /// Returns connection scope used to auto-disconnect slots when widget drops.
    fn connection_scope(&self) -> &ConnectionScope;
    /// Emits on hover/move interactions while pointer is over widget.
    fn hover_signal(&self) -> &Signal1<Point>;
    /// Emits on mouse/pointer press interactions.
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)>;
    /// Emits on mouse/pointer release interactions.
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)>;
    /// Emits on keyboard press interactions.
    fn key_down_signal(&self) -> &Signal1<(u32, u32)>;
    /// Emits on keyboard release interactions.
    fn key_up_signal(&self) -> &Signal1<(u32, u32)>;
    /// Emits when logical focus is gained.
    fn focus_gained_signal(&self) -> &GenericSignal;
    /// Emits when logical focus is lost.
    fn focus_lost_signal(&self) -> &GenericSignal;
    /// Emits when redraw is requested.
    fn redraw_requested_signal(&self) -> &GenericSignal;
    /// Emits when layout pass is requested.
    fn layout_requested_signal(&self) -> &GenericSignal;
    /// Requests redraw and emits redraw signal.
    fn request_redraw(&self) {
        self.redraw_requested_signal().emit();
    }
    /// Requests layout and emits layout signal.
    fn request_layout(&self) {
        self.layout_requested_signal().emit();
    }
}

/// Shared widget state and signals used by concrete controls.
pub struct BaseWidget {
    object: Object,
    kind: WidgetKind,
    geometry: Rect,
    min_size: Option<Size>,
    max_size: Option<Size>,
    parent: Option<ObjectId>,
    children: Vec<ObjectId>,
    visible: bool,
    enabled: bool,
    tooltip: String,
    style: WidgetStyle,
    connection_scope: ConnectionScope,
    /// Emitted when a click-like interaction is received.
    pub clicked: GenericSignal,
    /// Emitted when widget internal value/state changes.
    pub changed: GenericSignal,
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
            tooltip: String::new(),
            style: WidgetStyle::default(),
            connection_scope: ConnectionScope::new(),
            clicked: GenericSignal::new(),
            changed: GenericSignal::new(),
            hover: Signal1::new(),
            mouse_down: Signal1::new(),
            mouse_up: Signal1::new(),
            key_down: Signal1::new(),
            key_up: Signal1::new(),
            focus_gained: GenericSignal::new(),
            focus_lost: GenericSignal::new(),
            redraw_requested: GenericSignal::new(),
            layout_requested: GenericSignal::new(),
        }
    }
}

impl Widget for BaseWidget {
    fn id(&self) -> ObjectId { self.object.id() }
    fn kind(&self) -> WidgetKind { self.kind }
    fn geometry(&self) -> Rect { self.geometry }
    fn set_geometry(&mut self, geometry: Rect) {
        self.geometry = Rect::from_position_size(geometry.position(), self.constrained_size(geometry.size()));
    }
    fn min_size(&self) -> Option<Size> { self.min_size }
    fn max_size(&self) -> Option<Size> { self.max_size }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.min_size = min_size;
        self.geometry = Rect::from_position_size(self.geometry.position(), self.constrained_size(self.geometry.size()));
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.max_size = max_size;
        self.geometry = Rect::from_position_size(self.geometry.position(), self.constrained_size(self.geometry.size()));
    }
    fn parent(&self) -> Option<ObjectId> { self.parent }
    fn set_parent(&mut self, parent: Option<ObjectId>) { self.parent = parent; }
    fn add_child(&mut self, child: ObjectId) { self.children.push(child); }
    fn remove_child(&mut self, child: ObjectId) { self.children.retain(|id| *id != child); }
    fn children(&self) -> &[ObjectId] { &self.children }
    fn show(&mut self) { self.visible = true; }
    fn hide(&mut self) { self.visible = false; }
    fn is_visible(&self) -> bool { self.visible }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_tooltip(&mut self, tooltip: String) { self.tooltip = tooltip; }
    fn tooltip(&self) -> &str { &self.tooltip }
    fn style(&self) -> &WidgetStyle { &self.style }
    fn set_style(&mut self, style: WidgetStyle) { self.style = style; }
    fn connection_scope(&self) -> &ConnectionScope { &self.connection_scope }
    fn hover_signal(&self) -> &Signal1<Point> { &self.hover }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { &self.mouse_down }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { &self.mouse_up }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> { &self.key_down }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> { &self.key_up }
    fn focus_gained_signal(&self) -> &GenericSignal { &self.focus_gained }
    fn focus_lost_signal(&self) -> &GenericSignal { &self.focus_lost }
    fn redraw_requested_signal(&self) -> &GenericSignal { &self.redraw_requested }
    fn layout_requested_signal(&self) -> &GenericSignal { &self.layout_requested }
}

impl EventHandler for BaseWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.enabled || !self.visible {
            return;
        }
        match event {
            Event::MouseMove { pos } | Event::MouseEnter { pos } => self.hover.emit(*pos),
            Event::MousePress { pos, button } => self.mouse_down.emit((*pos, *button)),
            Event::MouseRelease { pos, button } => self.mouse_up.emit((*pos, *button)),
            Event::MouseLeave { .. } => self.focus_lost.emit(),
            Event::KeyPress { key, modifiers } => self.key_down.emit((*key, *modifiers)),
            Event::KeyRelease { key, modifiers } => self.key_up.emit((*key, *modifiers)),
            Event::Custom { name, .. } if name == "focus_gained" => self.focus_gained.emit(),
            Event::Custom { name, .. } if name == "focus_lost" => self.focus_lost.emit(),
            Event::Paint => self.redraw_requested.emit(),
            Event::Resize { .. } => self.layout_requested.emit(),
            _ => {}
        }
    }
}

impl BaseWidget {
    fn constrained_size(&self, size: Size) -> Size {
        let mut width = size.width;
        let mut height = size.height;

        if let Some(min) = self.min_size {
            width = width.max(min.width);
            height = height.max(min.height);
        }
        if let Some(max) = self.max_size {
            let effective_max_width = self.min_size.map(|min| max.width.max(min.width)).unwrap_or(max.width);
            let effective_max_height = self.min_size.map(|min| max.height.max(min.height)).unwrap_or(max.height);
            width = width.min(effective_max_width);
            height = height.min(effective_max_height);
        }

        Size::new(width, height)
    }
}

macro_rules! impl_widget_delegate {
    ($ty:ty, $field:ident) => {
        impl Widget for $ty {
            fn id(&self) -> ObjectId { self.$field.id() }
            fn kind(&self) -> WidgetKind { self.$field.kind() }
            fn geometry(&self) -> Rect { self.$field.geometry() }
            fn set_geometry(&mut self, geometry: Rect) { self.$field.set_geometry(geometry); }
            fn min_size(&self) -> Option<Size> { self.$field.min_size() }
            fn max_size(&self) -> Option<Size> { self.$field.max_size() }
            fn set_min_size(&mut self, min_size: Option<Size>) { self.$field.set_min_size(min_size); }
            fn set_max_size(&mut self, max_size: Option<Size>) { self.$field.set_max_size(max_size); }
            fn parent(&self) -> Option<ObjectId> { self.$field.parent() }
            fn set_parent(&mut self, parent: Option<ObjectId>) { self.$field.set_parent(parent); }
            fn add_child(&mut self, child: ObjectId) { self.$field.add_child(child); }
            fn remove_child(&mut self, child: ObjectId) { self.$field.remove_child(child); }
            fn children(&self) -> &[ObjectId] { self.$field.children() }
            fn show(&mut self) { self.$field.show(); }
            fn hide(&mut self) { self.$field.hide(); }
            fn is_visible(&self) -> bool { self.$field.is_visible() }
            fn set_enabled(&mut self, enabled: bool) { self.$field.set_enabled(enabled); }
            fn is_enabled(&self) -> bool { self.$field.is_enabled() }
            fn set_tooltip(&mut self, tooltip: String) { self.$field.set_tooltip(tooltip); }
            fn tooltip(&self) -> &str { self.$field.tooltip() }
            fn style(&self) -> &WidgetStyle { self.$field.style() }
            fn set_style(&mut self, style: WidgetStyle) { self.$field.set_style(style); }
            fn connection_scope(&self) -> &ConnectionScope { self.$field.connection_scope() }
            fn hover_signal(&self) -> &Signal1<Point> { self.$field.hover_signal() }
            fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { self.$field.mouse_down_signal() }
            fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { self.$field.mouse_up_signal() }
            fn key_down_signal(&self) -> &Signal1<(u32, u32)> { self.$field.key_down_signal() }
            fn key_up_signal(&self) -> &Signal1<(u32, u32)> { self.$field.key_up_signal() }
            fn focus_gained_signal(&self) -> &GenericSignal { self.$field.focus_gained_signal() }
            fn focus_lost_signal(&self) -> &GenericSignal { self.$field.focus_lost_signal() }
            fn redraw_requested_signal(&self) -> &GenericSignal { self.$field.redraw_requested_signal() }
            fn layout_requested_signal(&self) -> &GenericSignal { self.$field.layout_requested_signal() }
        }
        impl EventHandler for $ty {
            fn handle_event(&mut self, event: &Event) { self.$field.handle_event(event); }
        }
    };
}

/// Top-level window widget.
pub struct Window {
    base: BaseWidget,
    title: String,
    /// Emitted when the window is closed.
    pub closed: GenericSignal,
}

impl Window {
    /// Creates a new window with title and geometry.
    pub fn new(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "Window"),
            title,
            closed: GenericSignal::new(),
        }
    }
    /// Returns window title.
    pub fn title(&self) -> &str { &self.title }
    /// Updates window title.
    pub fn set_title(&mut self, title: String) { self.title = title; }

    /// Emits the window closed signal.
    pub fn close(&mut self) {
        self.closed.emit();
    }
}

impl Widget for Window {
    fn id(&self) -> ObjectId { self.base.id() }
    fn kind(&self) -> WidgetKind { self.base.kind() }
    fn geometry(&self) -> Rect { self.base.geometry() }
    fn set_geometry(&mut self, geometry: Rect) { self.base.set_geometry(geometry); }
    fn min_size(&self) -> Option<Size> { self.base.min_size() }
    fn max_size(&self) -> Option<Size> { self.base.max_size() }
    fn set_min_size(&mut self, min_size: Option<Size>) { self.base.set_min_size(min_size); }
    fn set_max_size(&mut self, max_size: Option<Size>) { self.base.set_max_size(max_size); }
    fn parent(&self) -> Option<ObjectId> { self.base.parent() }
    fn set_parent(&mut self, parent: Option<ObjectId>) { self.base.set_parent(parent); }
    fn add_child(&mut self, child: ObjectId) { self.base.add_child(child); }
    fn remove_child(&mut self, child: ObjectId) { self.base.remove_child(child); }
    fn children(&self) -> &[ObjectId] { self.base.children() }
    fn show(&mut self) { self.base.show(); }
    fn hide(&mut self) { self.base.hide(); }
    fn is_visible(&self) -> bool { self.base.is_visible() }
    fn set_enabled(&mut self, enabled: bool) { self.base.set_enabled(enabled); }
    fn is_enabled(&self) -> bool { self.base.is_enabled() }
    fn set_tooltip(&mut self, tooltip: String) { self.base.set_tooltip(tooltip); }
    fn tooltip(&self) -> &str { self.base.tooltip() }
    fn style(&self) -> &WidgetStyle { self.base.style() }
    fn set_style(&mut self, style: WidgetStyle) { self.base.set_style(style); }
    fn connection_scope(&self) -> &ConnectionScope { self.base.connection_scope() }
    fn hover_signal(&self) -> &Signal1<Point> { self.base.hover_signal() }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_down_signal() }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_up_signal() }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_down_signal() }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_up_signal() }
    fn focus_gained_signal(&self) -> &GenericSignal { self.base.focus_gained_signal() }
    fn focus_lost_signal(&self) -> &GenericSignal { self.base.focus_lost_signal() }
    fn redraw_requested_signal(&self) -> &GenericSignal { self.base.redraw_requested_signal() }
    fn layout_requested_signal(&self) -> &GenericSignal { self.base.layout_requested_signal() }
}

impl EventHandler for Window {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if matches!(event, Event::Quit) {
            self.closed.emit();
        }
    }
}

/// Dialog widget.
pub struct Dialog {
    base: BaseWidget,
}
/// Creates a dialog with geometry.
impl Dialog { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::Dialog, geometry, "Dialog") } } }
impl_widget_delegate!(Dialog, base);

/// Popup window widget.
pub struct PopupWindow { base: BaseWidget }
/// Creates a popup window with geometry.
impl PopupWindow { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow") } } }
impl_widget_delegate!(PopupWindow, base);

/// Push button widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Pressed,
    Disabled,
}

pub struct Button {
    base: BaseWidget,
    text: String,
    pressed: bool,
    pub activated: GenericSignal,
    pub pressed_signal: GenericSignal,
    pub released_signal: GenericSignal,
    pub state_changed: Signal1<ButtonState>,
}
impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            pressed: false,
            activated: GenericSignal::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }
    /// Returns button text.
    pub fn text(&self) -> &str { &self.text }

    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }

    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Sets pressed state and emits transition signals when changed.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
        self.state_changed.emit(self.state());
    }

    pub fn press(&mut self) {
        self.set_pressed(true);
    }

    pub fn release(&mut self) {
        self.set_pressed(false);
    }

    /// Enables/disables button while preserving deterministic state transitions.
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        self.base.set_enabled(enabled);
        if !enabled {
            self.pressed = false;
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
        }
    }
}

impl Widget for Button {
    fn id(&self) -> ObjectId { self.base.id() }
    fn kind(&self) -> WidgetKind { self.base.kind() }
    fn geometry(&self) -> Rect { self.base.geometry() }
    fn set_geometry(&mut self, geometry: Rect) { self.base.set_geometry(geometry); }
    fn min_size(&self) -> Option<Size> { self.base.min_size() }
    fn max_size(&self) -> Option<Size> { self.base.max_size() }
    fn set_min_size(&mut self, min_size: Option<Size>) { self.base.set_min_size(min_size); }
    fn set_max_size(&mut self, max_size: Option<Size>) { self.base.set_max_size(max_size); }
    fn parent(&self) -> Option<ObjectId> { self.base.parent() }
    fn set_parent(&mut self, parent: Option<ObjectId>) { self.base.set_parent(parent); }
    fn add_child(&mut self, child: ObjectId) { self.base.add_child(child); }
    fn remove_child(&mut self, child: ObjectId) { self.base.remove_child(child); }
    fn children(&self) -> &[ObjectId] { self.base.children() }
    fn show(&mut self) { self.base.show(); }
    fn hide(&mut self) { self.base.hide(); }
    fn is_visible(&self) -> bool { self.base.is_visible() }
    fn set_enabled(&mut self, enabled: bool) { self.set_enabled_state(enabled); }
    fn is_enabled(&self) -> bool { self.base.is_enabled() }
    fn set_tooltip(&mut self, tooltip: String) { self.base.set_tooltip(tooltip); }
    fn tooltip(&self) -> &str { self.base.tooltip() }
    fn style(&self) -> &WidgetStyle { self.base.style() }
    fn set_style(&mut self, style: WidgetStyle) { self.base.set_style(style); }
    fn connection_scope(&self) -> &ConnectionScope { self.base.connection_scope() }
    fn hover_signal(&self) -> &Signal1<Point> { self.base.hover_signal() }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_down_signal() }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_up_signal() }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_down_signal() }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_up_signal() }
    fn focus_gained_signal(&self) -> &GenericSignal { self.base.focus_gained_signal() }
    fn focus_lost_signal(&self) -> &GenericSignal { self.base.focus_lost_signal() }
    fn redraw_requested_signal(&self) -> &GenericSignal { self.base.redraw_requested_signal() }
    fn layout_requested_signal(&self) -> &GenericSignal { self.base.layout_requested_signal() }
}

impl EventHandler for Button {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { .. } => self.press(),
            Event::MouseRelease { .. } => {
                let was_pressed = self.is_pressed();
                self.release();
                if was_pressed && self.is_enabled() {
                    self.activated.emit();
                }
            }
            _ => {}
        }
    }
}

/// Checkbox widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    PartiallyChecked,
    Checked,
}

pub struct CheckBox {
    base: BaseWidget,
    state: CheckState,
    tristate_enabled: bool,
    pub toggled: Signal1<bool>,
    pub state_changed: Signal1<CheckState>,
}
impl CheckBox {
    /// Creates an unchecked checkbox with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckBox, geometry, "CheckBox"),
            state: CheckState::Unchecked,
            tristate_enabled: false,
            toggled: Signal1::new(),
            state_changed: Signal1::new(),
        }
    }

    /// Returns current check state.
    pub fn state(&self) -> CheckState { self.state }

    /// Returns true when the checkbox is fully checked.
    pub fn is_checked(&self) -> bool { self.state == CheckState::Checked }

    /// Enables/disables tri-state semantics.
    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled && self.state == CheckState::PartiallyChecked {
            self.set_state(CheckState::Unchecked);
        }
    }

    /// Returns whether tri-state semantics are enabled.
    pub fn is_tristate_enabled(&self) -> bool { self.tristate_enabled }

    /// Sets state and emits deterministic state/toggle signals when changed.
    pub fn set_state(&mut self, state: CheckState) {
        let normalized = if !self.tristate_enabled && state == CheckState::PartiallyChecked {
            CheckState::Unchecked
        } else {
            state
        };
        if self.state == normalized {
            return;
        }

        let previous_checked = self.is_checked();
        self.state = normalized;
        let checked = self.is_checked();
        if previous_checked != checked {
            self.toggled.emit(checked);
        }
        self.state_changed.emit(self.state);
    }

    /// Sets checked/unchecked state.
    pub fn set_checked(&mut self, checked: bool) {
        self.set_state(if checked { CheckState::Checked } else { CheckState::Unchecked });
    }

    /// Toggles checkbox according to tri-state configuration.
    pub fn toggle(&mut self) {
        let next = if self.tristate_enabled {
            match self.state {
                CheckState::Unchecked => CheckState::PartiallyChecked,
                CheckState::PartiallyChecked => CheckState::Checked,
                CheckState::Checked => CheckState::Unchecked,
            }
        } else if self.is_checked() {
            CheckState::Unchecked
        } else {
            CheckState::Checked
        };
        self.set_state(next);
    }
}
impl_widget_delegate!(CheckBox, base);

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
    pub fn is_checked(&self) -> bool { self.checked }

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
impl_widget_delegate!(RadioButton, base);

/// Text label widget.
pub struct Label {
    base: BaseWidget,
    text: String,
    image_source: Option<String>,
    alignment: Alignment,
    word_wrap: bool,
    pub text_changed: Signal1<String>,
    pub alignment_changed: Signal1<Alignment>,
    pub image_changed: Signal1<Option<String>>,
    pub word_wrap_changed: Signal1<bool>,
}
impl Label {
    /// Creates a label with text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "Label"),
            text,
            image_source: None,
            alignment: Alignment::Left,
            word_wrap: false,
            text_changed: Signal1::new(),
            alignment_changed: Signal1::new(),
            image_changed: Signal1::new(),
            word_wrap_changed: Signal1::new(),
        }
    }

    /// Sets label text and emits `text_changed` when value changes.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text.clone();
        self.text_changed.emit(text);
    }

    /// Returns optional image source path/identifier.
    pub fn image_source(&self) -> Option<&str> {
        self.image_source.as_deref()
    }

    /// Sets optional image source and emits `image_changed` when value changes.
    pub fn set_image_source(&mut self, image_source: Option<String>) {
        if self.image_source == image_source {
            return;
        }
        self.image_source = image_source.clone();
        self.image_changed.emit(image_source);
    }

    /// Returns current label alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }

    /// Sets label text alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.alignment_changed.emit(alignment);
    }

    /// Enables/disables word wrap behavior.
    pub fn set_word_wrap(&mut self, word_wrap: bool) {
        if self.word_wrap == word_wrap {
            return;
        }
        self.word_wrap = word_wrap;
        self.word_wrap_changed.emit(word_wrap);
    }

    /// Returns whether word wrap is enabled.
    pub fn word_wrap(&self) -> bool {
        self.word_wrap
    }

    /// Returns label text.
    pub fn text(&self) -> &str { &self.text }
}
impl_widget_delegate!(Label, base);

/// Single-line text editor widget.
pub struct LineEdit {
    base: BaseWidget,
    text: String,
    password_mode: bool,
    selection: Option<(usize, usize)>,
    pub text_changed: Signal1<String>,
    pub return_pressed: GenericSignal,
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    pub password_mode_changed: Signal1<bool>,
}
impl LineEdit {
    /// Creates an empty line editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"),
            text: String::new(),
            password_mode: false,
            selection: None,
            text_changed: Signal1::new(),
            return_pressed: GenericSignal::new(),
            selection_changed: Signal1::new(),
            password_mode_changed: Signal1::new(),
        }
    }

    /// Returns current editor text.
    pub fn text(&self) -> &str { &self.text }

    /// Sets text and emits `text_changed` when content changes.
    pub fn set_text(&mut self, text: String) {
        if self.text == text {
            return;
        }
        self.text = text.clone();
        self.text_changed.emit(text);
        self.normalize_selection();
    }

    /// Returns whether password mode is enabled.
    pub fn password_mode(&self) -> bool { self.password_mode }

    /// Enables/disables password mode.
    pub fn set_password_mode(&mut self, password_mode: bool) {
        if self.password_mode == password_mode {
            return;
        }
        self.password_mode = password_mode;
        self.password_mode_changed.emit(password_mode);
    }

    /// Returns display text (masked in password mode).
    pub fn display_text(&self) -> String {
        if self.password_mode {
            "•".repeat(self.text.chars().count())
        } else {
            self.text.clone()
        }
    }

    /// Returns current selected byte range.
    pub fn selection(&self) -> Option<(usize, usize)> { self.selection }

    /// Updates selected byte range, clamped to text bounds.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        let text_len = self.text.len();
        let start = start.min(text_len);
        let end = end.min(text_len);
        let normalized = if start == end {
            None
        } else {
            Some((start.min(end), start.max(end)))
        };
        if self.selection == normalized {
            return;
        }
        self.selection = normalized;
        self.selection_changed.emit(self.selection);
    }

    /// Clears selected text range.
    pub fn clear_selection(&mut self) {
        self.set_selection(0, 0);
    }

    /// Copies selected text when selection is valid.
    pub fn copy_selection(&self) -> Option<String> {
        let (start, end) = self.selection?;
        self.text.get(start..end).map(ToString::to_string)
    }

    /// Cuts selected text and returns removed text.
    pub fn cut_selection(&mut self) -> Option<String> {
        let (start, end) = self.selection?;
        let cut = self.text.get(start..end)?.to_string();
        self.text.replace_range(start..end, "");
        self.text_changed.emit(self.text.clone());
        self.selection = None;
        self.selection_changed.emit(None);
        Some(cut)
    }

    /// Pastes text into selection or appends when no selection exists.
    pub fn paste_text(&mut self, text: &str) {
        if let Some((start, end)) = self.selection {
            if self.text.get(start..end).is_some() {
                self.text.replace_range(start..end, text);
                self.text_changed.emit(self.text.clone());
                self.selection = None;
                self.selection_changed.emit(None);
                return;
            }
        }

        self.text.push_str(text);
        self.text_changed.emit(self.text.clone());
    }

    fn normalize_selection(&mut self) {
        if let Some((start, end)) = self.selection {
            let text_len = self.text.len();
            if start > text_len || end > text_len {
                let clamped_start = start.min(text_len);
                let clamped_end = end.min(text_len);
                self.selection = if clamped_start == clamped_end {
                    None
                } else {
                    Some((clamped_start.min(clamped_end), clamped_start.max(clamped_end)))
                };
                self.selection_changed.emit(self.selection);
            }
        }
    }
}
impl Widget for LineEdit {
    fn id(&self) -> ObjectId { self.base.id() }
    fn kind(&self) -> WidgetKind { self.base.kind() }
    fn geometry(&self) -> Rect { self.base.geometry() }
    fn set_geometry(&mut self, geometry: Rect) { self.base.set_geometry(geometry); }
    fn min_size(&self) -> Option<Size> { self.base.min_size() }
    fn max_size(&self) -> Option<Size> { self.base.max_size() }
    fn set_min_size(&mut self, min_size: Option<Size>) { self.base.set_min_size(min_size); }
    fn set_max_size(&mut self, max_size: Option<Size>) { self.base.set_max_size(max_size); }
    fn parent(&self) -> Option<ObjectId> { self.base.parent() }
    fn set_parent(&mut self, parent: Option<ObjectId>) { self.base.set_parent(parent); }
    fn add_child(&mut self, child: ObjectId) { self.base.add_child(child); }
    fn remove_child(&mut self, child: ObjectId) { self.base.remove_child(child); }
    fn children(&self) -> &[ObjectId] { self.base.children() }
    fn show(&mut self) { self.base.show(); }
    fn hide(&mut self) { self.base.hide(); }
    fn is_visible(&self) -> bool { self.base.is_visible() }
    fn set_enabled(&mut self, enabled: bool) { self.base.set_enabled(enabled); }
    fn is_enabled(&self) -> bool { self.base.is_enabled() }
    fn set_tooltip(&mut self, tooltip: String) { self.base.set_tooltip(tooltip); }
    fn tooltip(&self) -> &str { self.base.tooltip() }
    fn style(&self) -> &WidgetStyle { self.base.style() }
    fn set_style(&mut self, style: WidgetStyle) { self.base.set_style(style); }
    fn connection_scope(&self) -> &ConnectionScope { self.base.connection_scope() }
    fn hover_signal(&self) -> &Signal1<Point> { self.base.hover_signal() }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_down_signal() }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> { self.base.mouse_up_signal() }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_down_signal() }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> { self.base.key_up_signal() }
    fn focus_gained_signal(&self) -> &GenericSignal { self.base.focus_gained_signal() }
    fn focus_lost_signal(&self) -> &GenericSignal { self.base.focus_lost_signal() }
    fn redraw_requested_signal(&self) -> &GenericSignal { self.base.redraw_requested_signal() }
    fn layout_requested_signal(&self) -> &GenericSignal { self.base.layout_requested_signal() }
}

impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if matches!(event, Event::KeyPress { key: 13, .. }) {
            self.return_pressed.emit();
        }
    }
}

/// Multi-line text editor widget.
pub struct TextEdit { base: BaseWidget, text: String }
/// Creates an empty text editor.
impl TextEdit { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"), text: String::new() } } pub fn set_text(&mut self, text: String) { self.text = text; } }
impl_widget_delegate!(TextEdit, base);

/// Combo-box widget with simple string item storage.
pub struct ComboBox {
    base: BaseWidget,
    items: Vec<String>,
    current: Option<usize>,
    dropdown_open: bool,
    /// Emitted when selected index changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when selected index changes.
    pub index_changed: Signal1<usize>,
    /// Emitted when selected value changes.
    pub value_changed: Signal1<String>,
    /// Emitted when dropdown visibility changes.
    pub dropdown_visibility_changed: Signal1<bool>,
    /// Emitted when dropdown opens.
    pub dropdown_opened: GenericSignal,
    /// Emitted when dropdown closes.
    pub dropdown_closed: GenericSignal,
}
impl ComboBox {
    /// Creates an empty combo-box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"),
            items: Vec::new(),
            current: None,
            dropdown_open: false,
            selection_changed: Signal1::new(),
            index_changed: Signal1::new(),
            value_changed: Signal1::new(),
            dropdown_visibility_changed: Signal1::new(),
            dropdown_opened: GenericSignal::new(),
            dropdown_closed: GenericSignal::new(),
        }
    }
    /// Appends one item.
    pub fn add_item(&mut self, item: impl Into<String>) { self.items.push(item.into()); }

    /// Returns selected index when available.
    pub fn current_index(&self) -> Option<usize> { self.current }

    /// Returns selected text when available.
    pub fn current_text(&self) -> Option<&str> {
        self.current.and_then(|index| self.items.get(index).map(String::as_str))
    }

    /// Returns whether dropdown list is currently open.
    pub fn is_dropdown_open(&self) -> bool {
        self.dropdown_open
    }

    /// Opens dropdown list and emits visibility signals when state changes.
    pub fn open_dropdown(&mut self) {
        if self.dropdown_open {
            return;
        }
        self.dropdown_open = true;
        self.dropdown_visibility_changed.emit(true);
        self.dropdown_opened.emit();
    }

    /// Closes dropdown list and emits visibility signals when state changes.
    pub fn close_dropdown(&mut self) {
        if !self.dropdown_open {
            return;
        }
        self.dropdown_open = false;
        self.dropdown_visibility_changed.emit(false);
        self.dropdown_closed.emit();
    }

    /// Toggles dropdown list visibility.
    pub fn toggle_dropdown(&mut self) {
        if self.dropdown_open {
            self.close_dropdown();
        } else {
            self.open_dropdown();
        }
    }

    /// Updates current item index when in range.
    pub fn set_current_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.current == Some(index) {
            return true;
        }
        self.current = Some(index);
        self.selection_changed.emit(index);
        self.index_changed.emit(index);
        if let Some(value) = self.items.get(index) {
            self.value_changed.emit(value.clone());
        }
        true
    }

    /// Clears all items and selection.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
        self.close_dropdown();
    }
}
impl_widget_delegate!(ComboBox, base);

/// Spin-box widget.
pub struct SpinBox {
    base: BaseWidget,
    min: i32,
    max: i32,
    value: i32,
    single_step: i32,
    pub value_changed: Signal1<i32>,
}

impl SpinBox {
    /// Creates a spin-box with default range/value and step.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SpinBox, geometry, "SpinBox"),
            min: 0,
            max: 100,
            value: 0,
            single_step: 1,
            value_changed: Signal1::new(),
        }
    }

    /// Returns minimum value.
    pub fn min(&self) -> i32 { self.min }

    /// Returns maximum value.
    pub fn max(&self) -> i32 { self.max }

    /// Returns current value.
    pub fn value(&self) -> i32 { self.value }

    /// Returns configured step value.
    pub fn single_step(&self) -> i32 { self.single_step }

    /// Sets minimum/maximum range and clamps current value.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    /// Sets step used by increment/decrement operations.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }

    /// Sets value with deterministic clamping and change signal behavior.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(clamped);
    }

    /// Increments value by one step.
    pub fn step_up(&mut self) {
        self.set_value(self.value.saturating_add(self.single_step));
    }

    /// Decrements value by one step.
    pub fn step_down(&mut self) {
        self.set_value(self.value.saturating_sub(self.single_step));
    }
}
impl_widget_delegate!(SpinBox, base);

/// List-box widget with simple string item storage.
pub struct ListBox { base: BaseWidget, items: Vec<String> }
/// Creates an empty list-box and appends items.
impl ListBox { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ListBox, geometry, "ListBox"), items: Vec::new() } } pub fn add_item(&mut self, item: impl Into<String>) { self.items.push(item.into()); } }
impl_widget_delegate!(ListBox, base);

/// List model abstraction for list-like views.
pub trait ListModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Data for row index, if present.
    fn data(&self, row: usize) -> Option<String>;
}

/// Tree model abstraction for node/path-style views.
pub trait TreeModel: Send + Sync {
    fn node_count(&self) -> usize;
    fn node_path(&self, index: usize) -> Option<String>;
}

/// In-memory tree model backed by a vector of paths.
pub struct VecTreeModel {
    paths: Vec<String>,
}

impl VecTreeModel {
    /// Creates a tree model from path list.
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }

    /// Appends one node path.
    pub fn add_node(&mut self, path: impl Into<String>) {
        self.paths.push(path.into());
    }
}

impl TreeModel for VecTreeModel {
    fn node_count(&self) -> usize {
        self.paths.len()
    }

    fn node_path(&self, index: usize) -> Option<String> {
        self.paths.get(index).cloned()
    }
}

/// Filter/sort proxy model for tree views.
pub struct SortFilterTreeModel {
    /// Underlying source tree model.
    source: Arc<dyn TreeModel>,
    /// Optional case-insensitive substring filter.
    filter_text: Option<String>,
    /// Ascending path sort flag.
    sort_ascending: bool,
}

impl SortFilterTreeModel {
    /// Creates a tree proxy model over a source model.
    pub fn new(source: Arc<dyn TreeModel>) -> Self {
        Self {
            source,
            filter_text: None,
            sort_ascending: true,
        }
    }

    /// Sets optional filter text.
    pub fn set_filter_text(&mut self, text: Option<String>) {
        self.filter_text = text;
    }

    /// Sets sort direction for visible nodes.
    pub fn set_sort_ascending(&mut self, ascending: bool) {
        self.sort_ascending = ascending;
    }

    fn visible_nodes(&self) -> Vec<usize> {
        let mut nodes = if let Some(filter) = self.filter_text.as_ref() {
            (0..self.source.node_count())
                .filter(|index| {
                    self.source
                        .node_path(*index)
                        .map(|path| path.to_lowercase().contains(&filter.to_lowercase()))
                        .unwrap_or(false)
                })
                .collect::<Vec<_>>()
        } else {
            (0..self.source.node_count()).collect::<Vec<_>>()
        };

        nodes.sort_by(|left, right| {
            let left_value = self.source.node_path(*left).unwrap_or_default();
            let right_value = self.source.node_path(*right).unwrap_or_default();
            if self.sort_ascending {
                left_value.cmp(&right_value)
            } else {
                right_value.cmp(&left_value)
            }
        });

        nodes
    }

    /// Map view node index to source node index.
    pub fn source_index(&self, view_index: usize) -> Option<usize> {
        self.visible_nodes().get(view_index).copied()
    }
}

impl TreeModel for SortFilterTreeModel {
    fn node_count(&self) -> usize {
        self.visible_nodes().len()
    }

    fn node_path(&self, index: usize) -> Option<String> {
        self.visible_nodes()
            .get(index)
            .and_then(|source_index| self.source.node_path(*source_index))
    }
}

/// Table model abstraction for tabular views.
pub trait TableModel: Send + Sync {
    /// Number of rows.
    fn row_count(&self) -> usize;
    /// Number of columns.
    fn column_count(&self) -> usize;
    /// Cell value at row/column.
    fn data(&self, row: usize, col: usize) -> Option<String>;
    /// Header label for a column.
    fn header(&self, col: usize) -> Option<String>;

    /// Data payload by semantic role.
    fn data_with_role(&self, row: usize, col: usize, role: DataRole) -> Option<String> {
        match role {
            DataRole::Display | DataRole::Edit => self.data(row, col),
            DataRole::Tooltip | DataRole::Decoration | DataRole::Foreground | DataRole::Background => None,
            DataRole::User(_) => None,
        }
    }

    /// Whether a cell is editable by default model contract.
    fn is_editable(&self, _row: usize, _col: usize) -> bool {
        false
    }
}

/// Semantic model data role similar to common model/view frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRole {
    Display,
    Edit,
    Tooltip,
    Decoration,
    Foreground,
    Background,
    User(u32),
}

/// Editable model contract for in-place editor workflows.
pub trait EditableTableModel: TableModel {
    /// Set cell value in model storage.
    fn set_data(&mut self, row: usize, col: usize, value: String) -> bool;
}

/// Sort order for table view projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// In-memory table model backed by headers and string rows.
pub struct VecTableModel {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl VecTableModel {
    /// Creates a table model from headers and row data.
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
    }

    /// Updates one cell value, returning false for out-of-range indices.
    pub fn set_cell(&mut self, row: usize, col: usize, value: impl Into<String>) -> bool {
        let Some(row_data) = self.rows.get_mut(row) else {
            return false;
        };
        let Some(cell) = row_data.get_mut(col) else {
            return false;
        };
        *cell = value.into();
        true
    }
}

impl TableModel for VecTableModel {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn column_count(&self) -> usize {
        self.headers.len()
    }

    fn data(&self, row: usize, col: usize) -> Option<String> {
        self.rows.get(row).and_then(|r| r.get(col)).cloned()
    }

    fn header(&self, col: usize) -> Option<String> {
        self.headers.get(col).cloned()
    }

    fn is_editable(&self, row: usize, col: usize) -> bool {
        row < self.rows.len() && col < self.headers.len()
    }
}

impl EditableTableModel for VecTableModel {
    fn set_data(&mut self, row: usize, col: usize, value: String) -> bool {
        self.set_cell(row, col, value)
    }
}

/// Row selection mode for item/table views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multi,
}

/// Selection state container for row-oriented views.
#[derive(Debug, Clone)]
pub struct SelectionModel {
    mode: SelectionMode,
    current_row: Option<usize>,
    selected_rows: HashSet<usize>,
}

impl Default for SelectionModel {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionModel {
    /// Creates an empty single-selection model.
    pub fn new() -> Self {
        Self {
            mode: SelectionMode::Single,
            current_row: None,
            selected_rows: HashSet::new(),
        }
    }

    /// Returns active selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Updates selection mode and normalizes selected rows.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        if mode == SelectionMode::Single {
            if let Some(current) = self.current_row {
                self.selected_rows.clear();
                self.selected_rows.insert(current);
            } else {
                self.selected_rows.clear();
            }
        }
    }

    /// Selects a row according to active mode.
    pub fn select_row(&mut self, row: usize) {
        self.current_row = Some(row);
        match self.mode {
            SelectionMode::Single => {
                self.selected_rows.clear();
                self.selected_rows.insert(row);
            }
            SelectionMode::Multi => {
                self.selected_rows.insert(row);
            }
        }
    }

    /// Clears selection state.
    pub fn clear(&mut self) {
        self.current_row = None;
        self.selected_rows.clear();
    }

    /// Returns current row if present.
    pub fn current_row(&self) -> Option<usize> {
        self.current_row
    }

    /// Returns selected rows in ascending order.
    pub fn rows(&self) -> Vec<usize> {
        let mut rows = self.selected_rows.iter().copied().collect::<Vec<_>>();
        rows.sort_unstable();
        rows
    }
}

/// Delegate abstraction for view display/editor conversion.
pub trait ItemDelegate: Send + Sync {
    /// Convert model value to display text.
    fn format_display(&self, value: &str) -> String;
    /// Convert editor text back into model value.
    fn parse_editor(&self, edited: &str) -> String;
}

/// Default pass-through item delegate.
pub struct PlainTextItemDelegate;

impl ItemDelegate for PlainTextItemDelegate {
    fn format_display(&self, value: &str) -> String {
        value.to_string()
    }

    fn parse_editor(&self, edited: &str) -> String {
        edited.to_string()
    }
}

/// Filter/sort proxy model for table views.
pub struct SortFilterTableModel {
    /// Underlying source table model.
    source: Arc<dyn TableModel>,
    /// Optional case-insensitive substring filter.
    filter_text: Option<String>,
    /// Optional sort key column.
    sort_column: Option<usize>,
    /// Sort order for `sort_column`.
    sort_order: SortOrder,
}

impl SortFilterTableModel {
    /// Creates a table proxy model over a source model.
    pub fn new(source: Arc<dyn TableModel>) -> Self {
        Self {
            source,
            filter_text: None,
            sort_column: None,
            sort_order: SortOrder::Asc,
        }
    }

    /// Sets optional filter text.
    pub fn set_filter_text(&mut self, text: Option<String>) {
        self.filter_text = text;
    }

    /// Configure sort projection by source column and order.
    pub fn set_sort(&mut self, column: usize, order: SortOrder) {
        self.sort_column = Some(column);
        self.sort_order = order;
    }

    /// Clear configured sort projection.
    pub fn clear_sort(&mut self) {
        self.sort_column = None;
    }

    /// Return current sort configuration.
    pub fn sort(&self) -> Option<(usize, SortOrder)> {
        self.sort_column.map(|column| (column, self.sort_order))
    }

    fn visible_rows(&self) -> Vec<usize> {
        let mut rows = if let Some(filter) = self.filter_text.as_ref() {
            (0..self.source.row_count())
                .filter(|row| {
                    (0..self.source.column_count()).any(|col| {
                        self.source
                            .data(*row, col)
                            .map(|cell| cell.to_lowercase().contains(&filter.to_lowercase()))
                            .unwrap_or(false)
                    })
                })
                .collect::<Vec<_>>()
        } else {
            (0..self.source.row_count()).collect::<Vec<_>>()
        };

        if let Some(sort_column) = self.sort_column {
            rows.sort_by(|left, right| {
                let left_value = self
                    .source
                    .data(*left, sort_column)
                    .unwrap_or_default();
                let right_value = self
                    .source
                    .data(*right, sort_column)
                    .unwrap_or_default();
                match self.sort_order {
                    SortOrder::Asc => left_value.cmp(&right_value),
                    SortOrder::Desc => right_value.cmp(&left_value),
                }
            });
        }

        rows
    }

    /// Map view row index to source row index.
    pub fn source_row(&self, view_row: usize) -> Option<usize> {
        self.visible_rows().get(view_row).copied()
    }
}

impl TableModel for SortFilterTableModel {
    fn row_count(&self) -> usize {
        self.visible_rows().len()
    }

    fn column_count(&self) -> usize {
        self.source.column_count()
    }

    fn data(&self, row: usize, col: usize) -> Option<String> {
        self.visible_rows()
            .get(row)
            .and_then(|source_row| self.source.data(*source_row, col))
    }

    fn header(&self, col: usize) -> Option<String> {
        self.source.header(col)
    }
}

/// Progress bar widget.
pub struct ProgressBar {
    base: BaseWidget,
    min: u32,
    max: u32,
    value: u32,
    pub value_changed: Signal1<u32>,
}

impl ProgressBar {
    /// Creates a progress bar and initializes default range/value.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressBar, geometry, "ProgressBar"),
            min: 0,
            max: 100,
            value: 0,
            value_changed: Signal1::new(),
        }
    }

    /// Returns current minimum.
    pub fn min(&self) -> u32 { self.min }

    /// Returns current maximum.
    pub fn max(&self) -> u32 { self.max }

    /// Returns current value.
    pub fn value(&self) -> u32 { self.value }

    /// Sets range and clamps current value.
    pub fn set_range(&mut self, min: u32, max: u32) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    /// Sets progress value with deterministic clamping and change emission.
    pub fn set_value(&mut self, value: u32) {
        let clamped = value.clamp(self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(clamped);
    }
}
impl_widget_delegate!(ProgressBar, base);

/// Slider widget.
pub struct Slider {
    base: BaseWidget,
    min: i32,
    max: i32,
    value: i32,
    pub value_changed: Signal1<i32>,
}

impl Slider {
    /// Creates a slider and initializes default range/value.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Slider, geometry, "Slider"),
            min: 0,
            max: 100,
            value: 0,
            value_changed: Signal1::new(),
        }
    }

    /// Returns current minimum.
    pub fn min(&self) -> i32 { self.min }

    /// Returns current maximum.
    pub fn max(&self) -> i32 { self.max }

    /// Returns current value.
    pub fn value(&self) -> i32 { self.value }

    /// Sets slider range and clamps current value.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    /// Sets slider value with deterministic clamping and change emission.
    pub fn set_value(&mut self, value: i32) {
        let clamped = value.clamp(self.min, self.max);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(clamped);
    }
}
impl_widget_delegate!(Slider, base);

/// Scroll bar widget.
pub struct ScrollBar { base: BaseWidget, value: i32 }
/// Creates a scroll bar and updates current value.
impl ScrollBar { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ScrollBar, geometry, "ScrollBar"), value: 0 } } pub fn set_value(&mut self, value: i32) { self.value = value; } }
impl_widget_delegate!(ScrollBar, base);

macro_rules! simple_control {
    ($name:ident, $kind:expr) => {
        /// Simple widget control wrapper around `BaseWidget`.
        pub struct $name { base: BaseWidget }
        impl $name { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new($kind, geometry, stringify!($name)) } } }
        impl_widget_delegate!($name, base);
    };
}

simple_control!(Panel, WidgetKind::Panel);
simple_control!(GroupBox, WidgetKind::GroupBox);
simple_control!(TabWidget, WidgetKind::TabWidget);
simple_control!(StackWidget, WidgetKind::StackWidget);
simple_control!(MenuBar, WidgetKind::MenuBar);
simple_control!(Menu, WidgetKind::Menu);
simple_control!(ToolBar, WidgetKind::ToolBar);
simple_control!(StatusBar, WidgetKind::StatusBar);
simple_control!(Canvas, WidgetKind::Canvas);

/// Tree view widget with optional external model binding.
pub struct TreeView {
    base: BaseWidget,
    /// Optional bound tree model.
    model: Option<Arc<dyn TreeModel>>,
    /// Fallback path storage used when no external model is bound.
    fallback_nodes: Vec<String>,
    /// View-side selected node index.
    selected_node: Option<usize>,
    /// Emitted when selected node changes.
    pub selection_changed: Signal1<usize>,
}

impl TreeView {
    /// Creates an empty tree view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"),
            model: None,
            fallback_nodes: Vec::new(),
            selected_node: None,
            selection_changed: Signal1::new(),
        }
    }

    /// Binds an external tree model.
    pub fn set_model(&mut self, model: Arc<dyn TreeModel>) {
        self.model = Some(model);
    }

    /// Backward-compatible imperative insertion when no external model is used.
    pub fn add_node(&mut self, node: impl Into<String>) {
        self.fallback_nodes.push(node.into());
    }

    /// Returns current visible node count.
    pub fn node_count(&self) -> usize {
        self.model
            .as_ref()
            .map(|model| model.node_count())
            .unwrap_or(self.fallback_nodes.len())
    }

            /// Returns node path by visible index.
    pub fn node_path(&self, index: usize) -> Option<String> {
        self.model
            .as_ref()
            .and_then(|model| model.node_path(index))
            .or_else(|| self.fallback_nodes.get(index).cloned())
    }

            /// Selects a node by visible index.
    pub fn select_node(&mut self, index: usize) -> bool {
        if index < self.node_count() {
            self.selected_node = Some(index);
            self.selection_changed.emit(index);
            true
        } else {
            false
        }
    }

    /// Clears node selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
    }

    /// Returns selected node index if present.
    pub fn selected_node(&self) -> Option<usize> {
        self.selected_node
    }
}

impl_widget_delegate!(TreeView, base);

/// Table widget with model/view helpers and selection state.
pub struct TableWidget {
    base: BaseWidget,
    /// Optional bound data model.
    model: Option<Arc<dyn TableModel>>,
    /// View-side selection state.
    selection: SelectionModel,
    /// Explicit column width overrides in logical pixels.
    column_widths: HashMap<usize, u32>,
    /// Explicit row height overrides in logical pixels.
    row_heights: HashMap<usize, u32>,
    /// Optional display/editor delegate.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
}

impl TableWidget {
    /// Creates an empty table widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
            selection: SelectionModel::new(),
            column_widths: HashMap::new(),
            row_heights: HashMap::new(),
            delegate: None,
            selection_changed: Signal1::new(),
        }
    }

    /// Binds an external table model.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.model = Some(model);
    }

    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }

    /// Returns visible column count.
    pub fn column_count(&self) -> usize {
        self.model.as_ref().map(|m| m.column_count()).unwrap_or(0)
    }

    /// Read table header text by view column.
    pub fn header(&self, col: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.header(col))
    }

    /// Read table cell value by view row/column.
    pub fn cell(&self, row: usize, col: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row, col))
    }

    /// Read table cell value by role.
    pub fn cell_with_role(&self, row: usize, col: usize, role: DataRole) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data_with_role(row, col, role))
    }

    /// Read formatted display cell (delegate-aware).
    pub fn display_cell(&self, row: usize, col: usize) -> Option<String> {
        let value = self.cell_with_role(row, col, DataRole::Display)?;
        if let Some(delegate) = &self.delegate {
            Some(delegate.format_display(&value))
        } else {
            Some(value)
        }
    }

    /// Sets item delegate for display/editor conversion.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.delegate = Some(delegate);
    }

    /// Clears custom item delegate.
    pub fn clear_delegate(&mut self) {
        self.delegate = None;
    }

    /// Select one row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            true
        } else {
            false
        }
    }

    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection.current_row()
    }

    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection.rows()
    }

    /// Sets row selection mode.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection.set_mode(mode);
    }

    /// Returns current selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection.mode()
    }

    /// Sets explicit width override for a column.
    pub fn set_column_width(&mut self, col: usize, width: u32) {
        self.column_widths.insert(col, width.max(1));
    }

    /// Returns explicit width override for a column.
    pub fn column_width(&self, col: usize) -> Option<u32> {
        self.column_widths.get(&col).copied()
    }

    /// Sets explicit height override for a row.
    pub fn set_row_height(&mut self, row: usize, height: u32) {
        self.row_heights.insert(row, height.max(1));
    }

    /// Returns explicit height override for a row.
    pub fn row_height(&self, row: usize) -> Option<u32> {
        self.row_heights.get(&row).copied()
    }
}

impl_widget_delegate!(TableWidget, base);

simple_control!(GridWidget, WidgetKind::Grid);
simple_control!(ChartWidget, WidgetKind::Chart);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn vec_table_model_edit_contract() {
        let mut model = VecTableModel::new(
            vec!["name".to_string(), "value".to_string()],
            vec![vec!["a".to_string(), "1".to_string()]],
        );
        assert!(model.is_editable(0, 1));
        assert!(EditableTableModel::set_data(&mut model, 0, 1, "2".to_string()));
        assert_eq!(model.data(0, 1).as_deref(), Some("2"));
    }

    #[test]
    fn selection_model_multi_select() {
        let mut sel = SelectionModel::new();
        sel.set_mode(SelectionMode::Multi);
        sel.select_row(2);
        sel.select_row(5);
        assert_eq!(sel.current_row(), Some(5));
        assert_eq!(sel.rows(), vec![2, 5]);
    }

    #[test]
    fn window_closed_signal_emits_on_quit_event() {
        let mut window = Window::new(
            "Main".to_string(),
            Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 80,
            },
        );
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_ref = Arc::clone(&hits);
        window.closed.connect(move || {
            hits_ref.fetch_add(1, Ordering::SeqCst);
        });

        window.handle_event(&Event::Quit);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn selection_changed_signals_emit_for_combo_tree_table() {
        let hits = Arc::new(AtomicUsize::new(0));

        let mut combo = ComboBox::new(Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        });
        combo.add_item("A");
        combo.add_item("B");
        {
            let hits_ref = Arc::clone(&hits);
            combo.selection_changed.connect(move |_| {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }
        combo.set_current_index(1);

        let mut tree = TreeView::new(Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        });
        tree.add_node("root");
        {
            let hits_ref = Arc::clone(&hits);
            tree.selection_changed.connect(move |_| {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert!(tree.select_node(0));

        let rows = vec![vec!["a".to_string()]];
        let model = Arc::new(VecTableModel::new(vec!["c".to_string()], rows));
        let mut table = TableWidget::new(Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 80,
        });
        table.set_model(model);
        {
            let hits_ref = Arc::clone(&hits);
            table.selection_changed.connect(move |_| {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }
        assert!(table.select_row(0));

        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn base_widget_mouse_press_does_not_emit_clicked_directly() {
        let mut base = BaseWidget::new(
            WidgetKind::Button,
            Rect {
                x: 0,
                y: 0,
                width: 32,
                height: 24,
            },
            "Button",
        );
        let hits = Arc::new(AtomicUsize::new(0));
        let hits_ref = Arc::clone(&hits);
        base.clicked.connect(move || {
            hits_ref.fetch_add(1, Ordering::SeqCst);
        });

        let mouse_down_hits = Arc::new(AtomicUsize::new(0));
        let mouse_down_ref = Arc::clone(&mouse_down_hits);
        base.mouse_down_signal().connect(move |_| {
            mouse_down_ref.fetch_add(1, Ordering::SeqCst);
        });

        base.handle_event(&Event::MousePress {
            pos: crate::core::Point { x: 1, y: 1 },
            button: 1,
        });

        assert_eq!(hits.load(Ordering::SeqCst), 0);
        assert_eq!(mouse_down_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn widget_geometry_helpers_roundtrip_position_and_size() {
        let mut button = Button::new("ok".to_string(), Rect::new(10, 20, 80, 30));
        assert_eq!(button.position(), Point::new(10, 20));
        assert_eq!(button.size(), Size::new(80, 30));
        assert_eq!(button.rect(), Rect::new(10, 20, 80, 30));

        button.set_position(Point::new(3, 4));
        assert_eq!(button.geometry(), Rect::new(3, 4, 80, 30));

        button.set_size(Size::new(12, 8));
        assert_eq!(button.geometry(), Rect::new(3, 4, 12, 8));

        button.set_rect(Rect::new(7, 9, 21, 22));
        assert_eq!(button.geometry(), Rect::new(7, 9, 21, 22));
    }

    #[test]
    fn widget_style_helpers_update_padding_and_margin() {
        let mut panel = Panel::new(Rect::new(0, 0, 40, 20));
        panel.set_padding(Padding::normalized(-1, 4, 2, -8));
        panel.set_margin(Margin::symmetric(3, 7));

        assert_eq!(panel.padding(), &Padding::new(0, 4, 2, 0));
        assert_eq!(panel.margin(), &Margin::new(3, 7, 3, 7));
    }

    #[test]
    fn widget_min_max_size_constraints_clamp_geometry() {
        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 10, 10));
        button.set_min_size(Some(Size::new(20, 12)));
        button.set_geometry(Rect::new(1, 2, 5, 6));
        assert_eq!(button.geometry(), Rect::new(1, 2, 20, 12));

        button.set_max_size(Some(Size::new(24, 16)));
        button.set_geometry(Rect::new(3, 4, 100, 100));
        assert_eq!(button.geometry(), Rect::new(3, 4, 24, 16));
    }

    #[test]
    fn widget_conflicting_min_max_constraints_use_safe_effective_max() {
        let mut panel = Panel::new(Rect::new(0, 0, 10, 10));
        panel.set_min_size(Some(Size::new(30, 40)));
        panel.set_max_size(Some(Size::new(5, 6)));
        panel.set_size(Size::new(8, 9));
        assert_eq!(panel.size(), Size::new(30, 40));
    }

    #[test]
    fn widget_style_shorthands_cover_background_foreground_border_font() {
        let mut panel = Panel::new(Rect::new(0, 0, 10, 10));
        let font = Font::with_weight("Sans", 13.0, 600, true);

        panel.set_background_color(Some(Color::rgba(1, 2, 3, 255)));
        panel.set_foreground_color(Some(Color::rgba(9, 8, 7, 255)));
        panel.set_border(Some(Color::rgba(20, 21, 22, 255)), 3, 4);
        panel.set_font(Some(font.clone()));

        assert_eq!(panel.background_color(), Some(Color::rgba(1, 2, 3, 255)));
        assert_eq!(panel.foreground_color(), Some(Color::rgba(9, 8, 7, 255)));
        assert_eq!(panel.border_color(), Some(Color::rgba(20, 21, 22, 255)));
        assert_eq!(panel.border_width(), 3);
        assert_eq!(panel.border_radius(), 4);
        assert_eq!(panel.font(), Some(&font));
    }

    #[test]
    fn base_widget_emits_mouse_keyboard_and_focus_signals() {
        let mut base = BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 20, 20), "Panel");

        let hover_hits = Arc::new(AtomicUsize::new(0));
        let mouse_down_hits = Arc::new(AtomicUsize::new(0));
        let mouse_up_hits = Arc::new(AtomicUsize::new(0));
        let key_down_hits = Arc::new(AtomicUsize::new(0));
        let key_up_hits = Arc::new(AtomicUsize::new(0));
        let focus_gained_hits = Arc::new(AtomicUsize::new(0));
        let focus_lost_hits = Arc::new(AtomicUsize::new(0));

        {
            let hits = Arc::clone(&hover_hits);
            base.hover_signal().connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&mouse_down_hits);
            base.mouse_down_signal().connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&mouse_up_hits);
            base.mouse_up_signal().connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&key_down_hits);
            base.key_down_signal().connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&key_up_hits);
            base.key_up_signal().connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&focus_gained_hits);
            base.focus_gained_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&focus_lost_hits);
            base.focus_lost_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        base.handle_event(&Event::MouseEnter { pos: Point::new(1, 2) });
        base.handle_event(&Event::MouseMove { pos: Point::new(2, 3) });
        base.handle_event(&Event::MousePress {
            pos: Point::new(4, 5),
            button: 1,
        });
        base.handle_event(&Event::MouseRelease {
            pos: Point::new(4, 5),
            button: 1,
        });
        base.handle_event(&Event::KeyPress {
            key: 13,
            modifiers: 0,
        });
        base.handle_event(&Event::KeyRelease {
            key: 13,
            modifiers: 0,
        });
        base.handle_event(&Event::Custom {
            name: "focus_gained".to_string(),
            payload: Vec::new(),
        });
        base.handle_event(&Event::MouseLeave { pos: Point::new(9, 9) });

        assert_eq!(hover_hits.load(Ordering::SeqCst), 2);
        assert_eq!(mouse_down_hits.load(Ordering::SeqCst), 1);
        assert_eq!(mouse_up_hits.load(Ordering::SeqCst), 1);
        assert_eq!(key_down_hits.load(Ordering::SeqCst), 1);
        assert_eq!(key_up_hits.load(Ordering::SeqCst), 1);
        assert_eq!(focus_gained_hits.load(Ordering::SeqCst), 1);
        assert_eq!(focus_lost_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn base_widget_emits_redraw_and_layout_request_signals() {
        let mut base = BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 20, 20), "Panel");

        let redraw_hits = Arc::new(AtomicUsize::new(0));
        let layout_hits = Arc::new(AtomicUsize::new(0));

        {
            let hits = Arc::clone(&redraw_hits);
            base.redraw_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&layout_hits);
            base.layout_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        base.handle_event(&Event::Paint);
        base.handle_event(&Event::Resize {
            size: Size::new(100, 80),
        });
        base.request_redraw();
        base.request_layout();

        assert_eq!(redraw_hits.load(Ordering::SeqCst), 2);
        assert_eq!(layout_hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn button_state_machine_emits_press_release_and_activation() {
        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 40, 20));

        let pressed_hits = Arc::new(AtomicUsize::new(0));
        let released_hits = Arc::new(AtomicUsize::new(0));
        let activated_hits = Arc::new(AtomicUsize::new(0));
        let state_hits = Arc::new(AtomicUsize::new(0));

        {
            let hits = Arc::clone(&pressed_hits);
            button.pressed_signal.connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&released_hits);
            button.released_signal.connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&activated_hits);
            button.activated.connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&state_hits);
            button.state_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert_eq!(button.state(), ButtonState::Normal);

        button.handle_event(&Event::MousePress {
            pos: Point::new(1, 1),
            button: 1,
        });
        assert_eq!(button.state(), ButtonState::Pressed);

        button.handle_event(&Event::MouseRelease {
            pos: Point::new(1, 1),
            button: 1,
        });
        assert_eq!(button.state(), ButtonState::Normal);

        button.set_enabled(false);
        assert_eq!(button.state(), ButtonState::Disabled);

        button.handle_event(&Event::MousePress {
            pos: Point::new(1, 1),
            button: 1,
        });
        assert_eq!(button.state(), ButtonState::Disabled);

        assert_eq!(pressed_hits.load(Ordering::SeqCst), 1);
        assert_eq!(released_hits.load(Ordering::SeqCst), 1);
        assert_eq!(activated_hits.load(Ordering::SeqCst), 1);
        assert_eq!(state_hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn label_full_contract_emits_text_alignment_image_and_wrap_signals() {
        let mut label = Label::new("hello".to_string(), Rect::new(0, 0, 80, 20));

        let text_hits = Arc::new(AtomicUsize::new(0));
        let alignment_hits = Arc::new(AtomicUsize::new(0));
        let image_hits = Arc::new(AtomicUsize::new(0));
        let wrap_hits = Arc::new(AtomicUsize::new(0));

        {
            let hits = Arc::clone(&text_hits);
            label.text_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&alignment_hits);
            label.alignment_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&image_hits);
            label.image_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&wrap_hits);
            label.word_wrap_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert_eq!(label.alignment(), Alignment::Left);
        assert!(!label.word_wrap());
        assert_eq!(label.image_source(), None);

        label.set_text("hello".to_string());
        label.set_text("world".to_string());
        label.set_alignment(Alignment::Left);
        label.set_alignment(Alignment::Center);
        label.set_image_source(Some("icon.png".to_string()));
        label.set_word_wrap(true);

        assert_eq!(text_hits.load(Ordering::SeqCst), 1);
        assert_eq!(alignment_hits.load(Ordering::SeqCst), 1);
        assert_eq!(image_hits.load(Ordering::SeqCst), 1);
        assert_eq!(wrap_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn line_edit_full_contract_covers_return_password_and_edit_ops() {
        let mut edit = LineEdit::new(Rect::new(0, 0, 120, 24));
        let return_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&return_hits);
            edit.return_pressed.connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        edit.set_text("hello".to_string());
        edit.set_selection(1, 4);
        assert_eq!(edit.copy_selection().as_deref(), Some("ell"));
        assert_eq!(edit.cut_selection().as_deref(), Some("ell"));
        assert_eq!(edit.text(), "ho");

        edit.paste_text("abc");
        assert_eq!(edit.text(), "hoabc");

        edit.set_password_mode(true);
        assert_eq!(edit.display_text(), "•••••");

        edit.handle_event(&Event::KeyPress {
            key: 13,
            modifiers: 0,
        });
        edit.handle_event(&Event::KeyPress {
            key: 9,
            modifiers: 0,
        });

        assert_eq!(return_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn checkbox_and_radio_full_contracts_cover_tristate_and_group_selection() {
        let mut checkbox = CheckBox::new(Rect::new(0, 0, 20, 20));
        checkbox.toggle();
        assert_eq!(checkbox.state(), CheckState::Checked);

        checkbox.set_tristate_enabled(true);
        checkbox.toggle();
        assert_eq!(checkbox.state(), CheckState::Unchecked);
        checkbox.toggle();
        assert_eq!(checkbox.state(), CheckState::PartiallyChecked);

        let mut radio_a = RadioButton::new(Rect::new(0, 0, 20, 20));
        let mut radio_b = RadioButton::new(Rect::new(0, 0, 20, 20));
        let mut radio_c = RadioButton::new(Rect::new(0, 0, 20, 20));
        radio_a.set_group_id(Some("g".to_string()));
        radio_b.set_group_id(Some("g".to_string()));
        radio_c.set_group_id(Some("h".to_string()));

        let mut peers = vec![&mut radio_a, &mut radio_b, &mut radio_c];
        assert!(RadioButton::select_in_group(&mut peers, 1));

        assert!(!peers[0].is_checked());
        assert!(peers[1].is_checked());
        assert!(!peers[2].is_checked());
    }

    #[test]
    fn combo_slider_progress_and_spinbox_full_contracts_emit_deterministic_value_signals() {
        let mut combo = ComboBox::new(Rect::new(0, 0, 80, 24));
        let dropdown_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&dropdown_hits);
            combo.dropdown_visibility_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        combo.add_item("A");
        combo.add_item("B");
        assert!(combo.set_current_index(1));
        assert!(!combo.set_current_index(9));
        assert_eq!(combo.current_index(), Some(1));
        assert_eq!(combo.current_text(), Some("B"));
        assert!(!combo.is_dropdown_open());
        combo.open_dropdown();
        assert!(combo.is_dropdown_open());
        combo.close_dropdown();
        assert!(!combo.is_dropdown_open());
        assert_eq!(dropdown_hits.load(Ordering::SeqCst), 2);

        let mut slider = Slider::new(Rect::new(0, 0, 80, 24));
        slider.set_range(-10, 10);
        slider.set_value(25);
        assert_eq!(slider.value(), 10);

        let mut spin = SpinBox::new(Rect::new(0, 0, 80, 24));
        spin.set_range(-5, 5);
        spin.set_single_step(3);
        spin.set_value(9);
        assert_eq!(spin.value(), 5);
        spin.step_down();
        assert_eq!(spin.value(), 2);
        spin.step_up();
        assert_eq!(spin.value(), 5);

        let mut progress = ProgressBar::new(Rect::new(0, 0, 80, 24));
        progress.set_range(20, 60);
        progress.set_value(5);
        assert_eq!(progress.value(), 20);
    }
}
