//! Widget models and controls.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{Datelike, Timelike};

use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::{Margin, Padding, WidgetStyle};

/// Image structure for widget icons and favicons.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    // In a real implementation, this would contain image data
    // For now, we'll just use a placeholder
    pub data: Vec<u8>,
}

impl Image {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

/// Discrete widget categories supported by the widget model layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
    /// Top-level window.
    Window,
    Dialog,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    PopupWindow,
    Button,
    CheckBox,
    RadioButton,
    Label,
    LineEdit,
    TextEdit,
    RichEdit,
    ComboBox,
    SpinBox,
    ListBox,
    ListView,
    TreeView,
    ProgressBar,
    Slider,
    ScrollBar,
    ScrollArea,
    Panel,
    DockPanel,
    GroupBox,
    TabWidget,
    Splitter,
    StackWidget,
    MdiArea,
    MenuBar,
    Menu,
    ToolBar,
    StatusBar,
    Canvas,
    Table,
    Grid,
    /// Chart surface widget.
    Chart,
    ToggleButton,
    CheckListBox,
    DoubleSpinBox,
    Dial,
    Wizard,
    DatePicker,
    TimePicker,
    DateTimePicker,
    DirectoryPicker,
    DataView,
    PropertyGrid,
    Toolbox,
    StackedWidget,
    CollapsiblePane,
    DockWidget,
    WebView,
    ActivityIndicator,
    Calendar,
    ColumnView,
    UndoView,
    CommandLink,
    LCDNumber,
    FontComboBox,
    /// Web engine view widget for displaying web content.
    WebEngineView,
    /// Web engine page widget for managing web content.
    WebEnginePage,
    /// Web engine settings widget for configuring web engine behavior.
    WebEngineSettings,
    /// Web engine download item widget for managing downloads.
    WebEngineDownloadItem,
    /// Web engine cookie store widget for managing cookies.
    WebEngineCookieStore,
    /// Web engine web channel widget for JavaScript communication.
    WebEngineWebChannel,
    /// Web engine find text result widget for text search results.
    WebEngineFindTextResult,
    /// Web engine notification widget for web notifications.
    WebEngineNotification,
    /// Web engine script dialog widget for JavaScript dialogs.
    WebEngineScriptDialog,
    /// Web engine context menu request widget for context menu handling.
    WebEngineContextMenuRequest,
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
    fn id(&self) -> ObjectId {
        self.object.id()
    }
    fn kind(&self) -> WidgetKind {
        self.kind
    }
    fn geometry(&self) -> Rect {
        self.geometry
    }
    fn set_geometry(&mut self, geometry: Rect) {
        let new_geometry =
            Rect::from_position_size(geometry.position(), self.constrained_size(geometry.size()));
        if self.geometry != new_geometry {
            self.geometry = new_geometry;
            // Notify control backend
            use crate::control_backend::get_control_backend;
            let backend = get_control_backend();
            backend.set_widget_geometry(
                self.id(),
                self.geometry.x,
                self.geometry.y,
                self.geometry.width,
                self.geometry.height,
            );
        }
    }
    fn min_size(&self) -> Option<Size> {
        self.min_size
    }
    fn max_size(&self) -> Option<Size> {
        self.max_size
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.min_size = min_size;
        self.geometry = Rect::from_position_size(
            self.geometry.position(),
            self.constrained_size(self.geometry.size()),
        );
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.max_size = max_size;
        self.geometry = Rect::from_position_size(
            self.geometry.position(),
            self.constrained_size(self.geometry.size()),
        );
    }
    fn parent(&self) -> Option<ObjectId> {
        self.parent
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.parent = parent;
    }
    fn add_child(&mut self, child: ObjectId) {
        self.children.push(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.children.retain(|id| *id != child);
    }
    fn children(&self) -> &[ObjectId] {
        &self.children
    }
    fn show(&mut self) {
        self.visible = true;
        // Notify control backend
        use crate::control_backend::get_control_backend;
        get_control_backend().show_widget(self.id());
    }
    fn hide(&mut self) {
        self.visible = false;
        // Notify control backend
        use crate::control_backend::get_control_backend;
        get_control_backend().hide_widget(self.id());
    }
    fn is_visible(&self) -> bool {
        self.visible
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    fn tooltip(&self) -> &str {
        &self.tooltip
    }
    fn style(&self) -> &WidgetStyle {
        &self.style
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.style = style;
    }
    fn connection_scope(&self) -> &ConnectionScope {
        &self.connection_scope
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        &self.hover
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_down
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        &self.mouse_up
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_down
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        &self.key_up
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        &self.focus_gained
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        &self.focus_lost
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        &self.redraw_requested
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        &self.layout_requested
    }
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
            let effective_max_width = self
                .min_size
                .map(|min| max.width.max(min.width))
                .unwrap_or(max.width);
            let effective_max_height = self
                .min_size
                .map(|min| max.height.max(min.height))
                .unwrap_or(max.height);
            width = width.min(effective_max_width);
            height = height.min(effective_max_height);
        }

        Size::new(width, height)
    }
}

/// Macro to delegate Widget and EventHandler implementation to a base field.
///
/// This macro generates boilerplate code for widget structs that contain a base widget field,
/// automatically delegating all Widget trait methods and EventHandler trait method to the base field.
macro_rules! impl_widget_delegate {
    ($ty:ty, $field:ident) => {
        impl Widget for $ty {
            fn id(&self) -> ObjectId {
                self.$field.id()
            }
            fn kind(&self) -> WidgetKind {
                self.$field.kind()
            }
            fn geometry(&self) -> Rect {
                self.$field.geometry()
            }
            fn set_geometry(&mut self, geometry: Rect) {
                self.$field.set_geometry(geometry);
            }
            fn min_size(&self) -> Option<Size> {
                self.$field.min_size()
            }
            fn max_size(&self) -> Option<Size> {
                self.$field.max_size()
            }
            fn set_min_size(&mut self, min_size: Option<Size>) {
                self.$field.set_min_size(min_size);
            }
            fn set_max_size(&mut self, max_size: Option<Size>) {
                self.$field.set_max_size(max_size);
            }
            fn parent(&self) -> Option<ObjectId> {
                self.$field.parent()
            }
            fn set_parent(&mut self, parent: Option<ObjectId>) {
                self.$field.set_parent(parent);
            }
            fn add_child(&mut self, child: ObjectId) {
                self.$field.add_child(child);
            }
            fn remove_child(&mut self, child: ObjectId) {
                self.$field.remove_child(child);
            }
            fn children(&self) -> &[ObjectId] {
                self.$field.children()
            }
            fn show(&mut self) {
                self.$field.show();
            }
            fn hide(&mut self) {
                self.$field.hide();
            }
            fn is_visible(&self) -> bool {
                self.$field.is_visible()
            }
            fn set_enabled(&mut self, enabled: bool) {
                self.$field.set_enabled(enabled);
            }
            fn is_enabled(&self) -> bool {
                self.$field.is_enabled()
            }
            fn set_tooltip(&mut self, tooltip: String) {
                self.$field.set_tooltip(tooltip);
            }
            fn tooltip(&self) -> &str {
                self.$field.tooltip()
            }
            fn style(&self) -> &WidgetStyle {
                self.$field.style()
            }
            fn set_style(&mut self, style: WidgetStyle) {
                self.$field.set_style(style);
            }
            fn connection_scope(&self) -> &ConnectionScope {
                self.$field.connection_scope()
            }
            fn hover_signal(&self) -> &Signal1<Point> {
                self.$field.hover_signal()
            }
            fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
                self.$field.mouse_down_signal()
            }
            fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
                self.$field.mouse_up_signal()
            }
            fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
                self.$field.key_down_signal()
            }
            fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
                self.$field.key_up_signal()
            }
            fn focus_gained_signal(&self) -> &GenericSignal {
                self.$field.focus_gained_signal()
            }
            fn focus_lost_signal(&self) -> &GenericSignal {
                self.$field.focus_lost_signal()
            }
            fn redraw_requested_signal(&self) -> &GenericSignal {
                self.$field.redraw_requested_signal()
            }
            fn layout_requested_signal(&self) -> &GenericSignal {
                self.$field.layout_requested_signal()
            }
        }
        impl EventHandler for $ty {
            fn handle_event(&mut self, event: &Event) {
                self.$field.handle_event(event);
            }
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
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Updates window title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Emits the window closed signal.
    pub fn close(&mut self) {
        self.closed.emit();
    }
}

impl Widget for Window {
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

impl EventHandler for Window {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if matches!(event, Event::Quit) {
            self.closed.emit();
        }
    }
}

/// Dialog widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Accepted,
    Rejected,
    Canceled,
}

pub struct Dialog {
    base: BaseWidget,
    modal: bool,
    result: Option<DialogResult>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
    pub finished: Signal1<DialogResult>,
}

impl Dialog {
    /// Creates a dialog with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dialog, geometry, "Dialog"),
            modal: false,
            result: None,
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            finished: Signal1::new(),
        }
    }

    /// Returns whether this dialog is modal.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Sets modal flag.
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }

    /// Returns last result.
    pub fn result(&self) -> Option<DialogResult> {
        self.result
    }

    /// Completes dialog with provided result and emits signals.
    pub fn finish(&mut self, result: DialogResult) {
        self.result = Some(result);
        match result {
            DialogResult::Accepted => self.accepted.emit(),
            DialogResult::Rejected | DialogResult::Canceled => self.rejected.emit(),
        }
        self.finished.emit(result);
    }

    /// Accepts dialog.
    pub fn accept(&mut self) {
        self.finish(DialogResult::Accepted);
    }

    /// Rejects dialog.
    pub fn reject(&mut self) {
        self.finish(DialogResult::Rejected);
    }

    /// Cancels dialog.
    pub fn cancel(&mut self) {
        self.finish(DialogResult::Canceled);
    }
}
impl_widget_delegate!(Dialog, base);

/// Message box icon kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxIcon {
    Info,
    Warning,
    Error,
    Question,
}

/// Message box baseline dialog contract.
pub struct MessageBox {
    base: BaseWidget,
    title: String,
    text: String,
    icon: MessageBoxIcon,
    result: Option<DialogResult>,
    pub result_changed: Signal1<DialogResult>,
}

impl MessageBox {
    /// Creates a message box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MessageBox, geometry, "MessageBox"),
            title: String::new(),
            text: String::new(),
            icon: MessageBoxIcon::Info,
            result: None,
            result_changed: Signal1::new(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn icon(&self) -> MessageBoxIcon {
        self.icon
    }
    pub fn result(&self) -> Option<DialogResult> {
        self.result
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    pub fn set_icon(&mut self, icon: MessageBoxIcon) {
        self.icon = icon;
    }

    pub fn set_result(&mut self, result: DialogResult) {
        self.result = Some(result);
        self.result_changed.emit(result);
    }
}
impl_widget_delegate!(MessageBox, base);

/// File dialog baseline contract.
pub struct FileDialog {
    base: BaseWidget,
    current_dir: String,
    selected_file: Option<String>,
    pub file_selected: Signal1<Option<String>>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}

impl FileDialog {
    /// Creates a file dialog.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FileDialog, geometry, "FileDialog"),
            current_dir: String::new(),
            selected_file: None,
            file_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }

    pub fn current_dir(&self) -> &str {
        &self.current_dir
    }
    pub fn selected_file(&self) -> Option<&str> {
        self.selected_file.as_deref()
    }

    pub fn set_current_dir(&mut self, dir: String) {
        self.current_dir = dir;
    }

    pub fn set_selected_file(&mut self, file: Option<String>) {
        if self.selected_file == file {
            return;
        }
        self.selected_file = file.clone();
        self.file_selected.emit(file);
    }

    pub fn accept(&self) {
        self.accepted.emit();
    }
    pub fn reject(&self) {
        self.rejected.emit();
    }
}
impl_widget_delegate!(FileDialog, base);

/// Color dialog baseline contract.
pub struct ColorDialog {
    base: BaseWidget,
    color: Color,
    pub color_selected: Signal1<Color>,
}

impl ColorDialog {
    /// Creates a color dialog with opaque black default.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorDialog, geometry, "ColorDialog"),
            color: Color::rgba(0, 0, 0, 255),
            color_selected: Signal1::new(),
        }
    }

    pub fn color(&self) -> Color {
        self.color
    }

    pub fn set_color(&mut self, color: Color) {
        if self.color == color {
            return;
        }
        self.color = color;
        self.color_selected.emit(color);
    }
}
impl_widget_delegate!(ColorDialog, base);

/// Font dialog baseline contract.
pub struct FontDialog {
    base: BaseWidget,
    font: Font,
    pub font_selected: Signal1<Font>,
}

impl FontDialog {
    /// Creates a font dialog with default UI font.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FontDialog, geometry, "FontDialog"),
            font: Font::default_ui(),
            font_selected: Signal1::new(),
        }
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        if self.font == font {
            return;
        }
        self.font = font.clone();
        self.font_selected.emit(font);
    }
}
impl_widget_delegate!(FontDialog, base);

/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
}
/// Creates a popup window with geometry.
impl PopupWindow {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
        }
    }
}
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
    pub fn text(&self) -> &str {
        &self.text
    }

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
        self.set_enabled_state(enabled);
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
    pub fn state(&self) -> CheckState {
        self.state
    }

    /// Returns true when the checkbox is fully checked.
    pub fn is_checked(&self) -> bool {
        self.state == CheckState::Checked
    }

    /// Enables/disables tri-state semantics.
    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled && self.state == CheckState::PartiallyChecked {
            self.set_state(CheckState::Unchecked);
        }
    }

    /// Returns whether tri-state semantics are enabled.
    pub fn is_tristate_enabled(&self) -> bool {
        self.tristate_enabled
    }

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
        self.set_state(if checked {
            CheckState::Checked
        } else {
            CheckState::Unchecked
        });
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
    pub fn text(&self) -> &str {
        &self.text
    }
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
    pub fn text(&self) -> &str {
        &self.text
    }

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
    pub fn password_mode(&self) -> bool {
        self.password_mode
    }

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
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

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
                    Some((
                        clamped_start.min(clamped_end),
                        clamped_start.max(clamped_end),
                    ))
                };
                self.selection_changed.emit(self.selection);
            }
        }
    }
}
impl Widget for LineEdit {
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

impl EventHandler for LineEdit {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if matches!(event, Event::KeyPress { key: 13, .. }) {
            self.return_pressed.emit();
        }
    }
}

/// Multi-line text editor widget.
pub struct TextEdit {
    base: BaseWidget,
    text: String,
}
/// Creates an empty text editor.
impl TextEdit {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"),
            text: String::new(),
        }
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl_widget_delegate!(TextEdit, base);

/// Rich text/code editor baseline widget contract.
pub struct RichEdit {
    base: BaseWidget,
    text: String,
    selection: Option<(usize, usize)>,
    read_only: bool,
    pub text_changed: Signal1<String>,
    pub selection_changed: Signal1<Option<(usize, usize)>>,
    pub read_only_changed: Signal1<bool>,
    pub cursor_position_changed: Signal1<usize>,
}

impl RichEdit {
    /// Creates an empty rich editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "RichEdit"),
            text: String::new(),
            selection: None,
            read_only: false,
            text_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            read_only_changed: Signal1::new(),
            cursor_position_changed: Signal1::new(),
        }
    }

    /// Returns current editor text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces editor text and resets selection/cursor to end.
    pub fn set_text(&mut self, text: String) {
        if self.read_only || self.text == text {
            return;
        }
        self.text = text.clone();
        self.text_changed.emit(text);
        self.selection = None;
        self.selection_changed.emit(None);
        self.cursor_position_changed.emit(self.text.len());
    }

    /// Returns whether editor is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets read-only mode.
    pub fn set_read_only(&mut self, read_only: bool) {
        if self.read_only == read_only {
            return;
        }
        self.read_only = read_only;
        self.read_only_changed.emit(read_only);
    }

    /// Returns current selected byte range.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    /// Sets current selected byte range (clamped to text length).
    pub fn set_selection(&mut self, start: usize, end: usize) {
        let text_len = self.text.len();
        let normalized = {
            let start = start.min(text_len);
            let end = end.min(text_len);
            if start == end {
                None
            } else {
                Some((start.min(end), start.max(end)))
            }
        };
        if self.selection == normalized {
            return;
        }
        self.selection = normalized;
        self.selection_changed.emit(self.selection);
        self.cursor_position_changed.emit(self.cursor_position());
    }

    /// Clears selected range.
    pub fn clear_selection(&mut self) {
        self.set_selection(0, 0);
    }

    /// Returns current cursor byte offset.
    pub fn cursor_position(&self) -> usize {
        self.selection
            .map(|(_, end)| end)
            .unwrap_or(self.text.len())
    }

    /// Inserts text at selection/cursor and updates cursor.
    pub fn insert_text(&mut self, text: &str) {
        if self.read_only || text.is_empty() {
            return;
        }

        if let Some((start, end)) = self.selection {
            if self.text.get(start..end).is_some() {
                self.text.replace_range(start..end, text);
                self.text_changed.emit(self.text.clone());
                self.selection = None;
                self.selection_changed.emit(None);
                self.cursor_position_changed.emit(start + text.len());
                return;
            }
        }

        self.text.push_str(text);
        self.text_changed.emit(self.text.clone());
        self.cursor_position_changed.emit(self.text.len());
    }

    /// Appends text at end.
    pub fn append_text(&mut self, text: &str) {
        self.insert_text(text);
    }

    /// Deletes selected range, returning removed text.
    pub fn delete_selection(&mut self) -> Option<String> {
        if self.read_only {
            return None;
        }
        let (start, end) = self.selection?;
        let removed = self.text.get(start..end)?.to_string();
        self.text.replace_range(start..end, "");
        self.text_changed.emit(self.text.clone());
        self.selection = None;
        self.selection_changed.emit(None);
        self.cursor_position_changed.emit(start);
        Some(removed)
    }
}

impl_widget_delegate!(RichEdit, base);

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
    pub fn add_item(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
    }

    /// Returns selected index when available.
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// Returns selected text when available.
    pub fn current_text(&self) -> Option<&str> {
        self.current
            .and_then(|index| self.items.get(index).map(String::as_str))
    }

    /// Returns total item count.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns item text by index when available.
    pub fn item_text(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(String::as_str)
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
    pub fn min(&self) -> i32 {
        self.min
    }

    /// Returns maximum value.
    pub fn max(&self) -> i32 {
        self.max
    }

    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Returns configured step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }

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
pub struct ListBox {
    base: BaseWidget,
    items: Vec<String>,
}
/// Creates an empty list-box and appends items.
impl ListBox {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListBox, geometry, "ListBox"),
            items: Vec::new(),
        }
    }
    pub fn add_item(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
    }

    /// Returns total item count.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Returns item text by index when available.
    pub fn item_text(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(String::as_str)
    }

    /// Clears all items from the list box.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}
impl_widget_delegate!(ListBox, base);

/// List view widget with optional external model binding and row selection.
pub struct ListView {
    base: BaseWidget,
    /// Optional bound list model.
    model: Option<Arc<dyn ListModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
}

impl ListView {
    /// Creates an empty list view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ListView, geometry, "ListView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: SelectionModel::new(),
            focused_row: None,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }

    /// Binds an external list model.
    pub fn set_model(&mut self, model: Arc<dyn ListModel>) {
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns visible row count.
    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }

    /// Returns item text by row index.
    pub fn item(&self, row: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row))
    }

    /// Select one row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selection.select_row(row);
            self.selection_changed.emit(row);
            self.set_focused_row(row);
            true
        } else {
            false
        }
    }

    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Sets focused row in current projection.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        true
    }

    /// Clears focused row.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
    }

    /// Returns focused row when still visible in projection.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row.filter(|row| *row < self.row_count())
    }

    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection
            .current_row()
            .filter(|row| *row < self.row_count())
    }

    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection
            .rows()
            .into_iter()
            .filter(|row| *row < self.row_count())
            .collect()
    }

    /// Sets row selection mode.
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.selection.set_mode(mode);
    }

    /// Returns current selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.selection.mode()
    }

    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        self.selection.selected_rows.retain(|row| *row < row_count);
        self.selection.current_row = self.selection.current_row.filter(|row| *row < row_count);
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }
}

impl_widget_delegate!(ListView, base);

/// List model abstraction for list-like views.
pub trait ListModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Data for row index, if present.
    fn data(&self, row: usize) -> Option<String>;

    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}

/// In-memory list model backed by a vector of strings.
pub struct VecListModel {
    items: Vec<String>,
    data_changed: GenericSignal,
}

impl VecListModel {
    /// Creates a list model from item values.
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            data_changed: GenericSignal::new(),
        }
    }

    /// Appends one list item and notifies observers.
    pub fn add_item(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
        self.data_changed.emit();
    }

    /// Replaces one list item value, returning false for out-of-range index.
    pub fn set_item(&mut self, row: usize, value: impl Into<String>) -> bool {
        let Some(item) = self.items.get_mut(row) else {
            return false;
        };
        let next = value.into();
        if *item == next {
            return true;
        }
        *item = next;
        self.data_changed.emit();
        true
    }

    /// Removes one list item by index and notifies observers when removed.
    pub fn remove_item(&mut self, row: usize) -> bool {
        if row >= self.items.len() {
            return false;
        }
        self.items.remove(row);
        self.data_changed.emit();
        true
    }

    /// Emits a data-changed notification for external batch updates.
    pub fn notify_data_changed(&self) {
        self.data_changed.emit();
    }

    /// Returns model data-change signal.
    pub fn data_changed(&self) -> &GenericSignal {
        &self.data_changed
    }
}

impl ListModel for VecListModel {
    fn row_count(&self) -> usize {
        self.items.len()
    }

    fn data(&self, row: usize) -> Option<String> {
        self.items.get(row).cloned()
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
    }
}

/// Tree model abstraction for node/path-style views.
pub trait TreeModel: Send + Sync {
    fn node_count(&self) -> usize;
    fn node_path(&self, index: usize) -> Option<String>;

    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
    }
}

/// In-memory tree model backed by a vector of paths.
pub struct VecTreeModel {
    paths: Vec<String>,
    data_changed: GenericSignal,
}

impl VecTreeModel {
    /// Creates a tree model from path list.
    pub fn new(paths: Vec<String>) -> Self {
        Self {
            paths,
            data_changed: GenericSignal::new(),
        }
    }

    /// Appends one node path.
    pub fn add_node(&mut self, path: impl Into<String>) {
        self.paths.push(path.into());
        self.data_changed.emit();
    }

    /// Emits a data-changed notification for external batch updates.
    pub fn notify_data_changed(&self) {
        self.data_changed.emit();
    }
}

impl TreeModel for VecTreeModel {
    fn node_count(&self) -> usize {
        self.paths.len()
    }

    fn node_path(&self, index: usize) -> Option<String> {
        self.paths.get(index).cloned()
    }

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
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

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.source.data_changed_signal()
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
            DataRole::Tooltip
            | DataRole::Decoration
            | DataRole::Foreground
            | DataRole::Background => None,
            DataRole::User(_) => None,
        }
    }

    /// Whether a cell is editable by default model contract.
    fn is_editable(&self, _row: usize, _col: usize) -> bool {
        false
    }

    /// Optional signal emitted when model data projection changes.
    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        None
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
    data_changed: GenericSignal,
}

impl VecTableModel {
    /// Creates a table model from headers and row data.
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self {
            headers,
            rows,
            data_changed: GenericSignal::new(),
        }
    }

    /// Updates one cell value, returning false for out-of-range indices.
    pub fn set_cell(&mut self, row: usize, col: usize, value: impl Into<String>) -> bool {
        let Some(row_data) = self.rows.get_mut(row) else {
            return false;
        };
        let Some(cell) = row_data.get_mut(col) else {
            return false;
        };
        let next = value.into();
        if *cell == next {
            return true;
        }
        *cell = next;
        self.data_changed.emit();
        true
    }

    /// Emits a data-changed notification for external batch updates.
    pub fn notify_data_changed(&self) {
        self.data_changed.emit();
    }

    /// Returns model data-change signal.
    pub fn data_changed(&self) -> &GenericSignal {
        &self.data_changed
    }

    /// Appends one row and notifies observers.
    pub fn push_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
        self.data_changed.emit();
    }

    /// Removes one row by index and notifies observers when removed.
    pub fn remove_row(&mut self, index: usize) -> bool {
        if index >= self.rows.len() {
            return false;
        }
        self.rows.remove(index);
        self.data_changed.emit();
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

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        Some(&self.data_changed)
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
                let left_value = self.source.data(*left, sort_column).unwrap_or_default();
                let right_value = self.source.data(*right, sort_column).unwrap_or_default();
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

    fn data_changed_signal(&self) -> Option<&GenericSignal> {
        self.source.data_changed_signal()
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
    pub fn min(&self) -> u32 {
        self.min
    }

    /// Returns current maximum.
    pub fn max(&self) -> u32 {
        self.max
    }

    /// Returns current value.
    pub fn value(&self) -> u32 {
        self.value
    }

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
    pub fn min(&self) -> i32 {
        self.min
    }

    /// Returns current maximum.
    pub fn max(&self) -> i32 {
        self.max
    }

    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }

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
pub struct ScrollBar {
    base: BaseWidget,
    min: i32,
    max: i32,
    value: i32,
    page_step: i32,
    single_step: i32,
    pub value_changed: Signal1<i32>,
}

impl ScrollBar {
    /// Creates a scroll bar with default range/value and step contract.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollBar, geometry, "ScrollBar"),
            min: 0,
            max: 100,
            value: 0,
            page_step: 10,
            single_step: 1,
            value_changed: Signal1::new(),
        }
    }

    /// Returns minimum value.
    pub fn min(&self) -> i32 {
        self.min
    }

    /// Returns maximum value.
    pub fn max(&self) -> i32 {
        self.max
    }

    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Returns page step.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }

    /// Returns single step.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }

    /// Sets range and clamps current value.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    /// Sets page step for page-wise scrolling.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
    }

    /// Sets single step for line-wise scrolling.
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

    /// Moves by one single-step toward minimum.
    pub fn line_decrement(&mut self) {
        self.set_value(self.value.saturating_sub(self.single_step));
    }

    /// Moves by one single-step toward maximum.
    pub fn line_increment(&mut self) {
        self.set_value(self.value.saturating_add(self.single_step));
    }

    /// Moves by one page-step toward minimum.
    pub fn page_decrement(&mut self) {
        self.set_value(self.value.saturating_sub(self.page_step));
    }

    /// Moves by one page-step toward maximum.
    pub fn page_increment(&mut self) {
        self.set_value(self.value.saturating_add(self.page_step));
    }
}
impl_widget_delegate!(ScrollBar, base);

/// Scroll area widget with deterministic viewport/content/offset contracts.
pub struct ScrollArea {
    base: BaseWidget,
    content_size: Size,
    viewport_size: Size,
    scroll_offset: Point,
    pub content_size_changed: Signal1<Size>,
    pub viewport_size_changed: Signal1<Size>,
    pub scroll_offset_changed: Signal1<Point>,
}

impl ScrollArea {
    /// Creates a scroll area with default content/viewport matching geometry size.
    pub fn new(geometry: Rect) -> Self {
        let initial_size = Size::new(geometry.width, geometry.height);
        Self {
            base: BaseWidget::new(WidgetKind::ScrollArea, geometry, "ScrollArea"),
            content_size: initial_size,
            viewport_size: initial_size,
            scroll_offset: Point::new(0, 0),
            content_size_changed: Signal1::new(),
            viewport_size_changed: Signal1::new(),
            scroll_offset_changed: Signal1::new(),
        }
    }

    /// Returns content size.
    pub fn content_size(&self) -> Size {
        self.content_size
    }

    /// Returns viewport size.
    pub fn viewport_size(&self) -> Size {
        self.viewport_size
    }

    /// Returns current scroll offset.
    pub fn scroll_offset(&self) -> Point {
        self.scroll_offset
    }

    /// Sets content size and normalizes scroll offset.
    pub fn set_content_size(&mut self, size: Size) {
        if self.content_size == size {
            return;
        }
        self.content_size = size;
        self.content_size_changed.emit(size);
        self.normalize_offset();
    }

    /// Sets viewport size and normalizes scroll offset.
    pub fn set_viewport_size(&mut self, size: Size) {
        if self.viewport_size == size {
            return;
        }
        self.viewport_size = size;
        self.viewport_size_changed.emit(size);
        self.normalize_offset();
    }

    /// Sets scroll offset with deterministic clamp to valid range.
    pub fn set_scroll_offset(&mut self, offset: Point) {
        let clamped = self.clamp_offset(offset);
        if self.scroll_offset == clamped {
            return;
        }
        self.scroll_offset = clamped;
        self.scroll_offset_changed.emit(clamped);
    }

    fn normalize_offset(&mut self) {
        self.set_scroll_offset(self.scroll_offset);
    }

    fn clamp_offset(&self, offset: Point) -> Point {
        let max_x = self
            .content_size
            .width
            .saturating_sub(self.viewport_size.width) as i32;
        let max_y = self
            .content_size
            .height
            .saturating_sub(self.viewport_size.height) as i32;
        Point::new(offset.x.clamp(0, max_x), offset.y.clamp(0, max_y))
    }
}
impl_widget_delegate!(ScrollArea, base);

/// Macro to create simple widget controls that wrap around `BaseWidget`.
///
/// This macro generates a basic widget struct with only a base widget field,
/// along with a constructor and delegated Widget/EventHandler implementations.
macro_rules! simple_control {
    ($name:ident, $kind:expr) => {
        /// Simple widget control wrapper around `BaseWidget`.
        pub struct $name {
            base: BaseWidget,
        }
        impl $name {
            /// Creates a new instance of the widget with the given geometry.
            pub fn new(geometry: Rect) -> Self {
                Self {
                    base: BaseWidget::new($kind, geometry, stringify!($name)),
                }
            }
        }
        impl_widget_delegate!($name, base);
    };
}

simple_control!(Panel, WidgetKind::Panel);

/// Group box widget with optional checkable state.
pub struct GroupBox {
    base: BaseWidget,
    title: String,
    checkable: bool,
    checked: bool,
    pub title_changed: Signal1<String>,
    pub checkable_changed: Signal1<bool>,
    pub checked_changed: Signal1<bool>,
}

impl GroupBox {
    /// Creates a group box with empty title.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GroupBox, geometry, "GroupBox"),
            title: String::new(),
            checkable: false,
            checked: false,
            title_changed: Signal1::new(),
            checkable_changed: Signal1::new(),
            checked_changed: Signal1::new(),
        }
    }

    /// Returns current title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets title and emits when changed.
    pub fn set_title(&mut self, title: String) {
        if self.title == title {
            return;
        }
        self.title = title.clone();
        self.title_changed.emit(title);
    }

    /// Returns whether group box is checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }

    /// Enables/disables checkable behavior.
    pub fn set_checkable(&mut self, checkable: bool) {
        if self.checkable == checkable {
            return;
        }
        self.checkable = checkable;
        self.checkable_changed.emit(checkable);
        if !checkable {
            self.set_checked(false);
        }
    }

    /// Returns checked state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets checked state (only effective when checkable).
    pub fn set_checked(&mut self, checked: bool) {
        let normalized = if self.checkable { checked } else { false };
        if self.checked == normalized {
            return;
        }
        self.checked = normalized;
        self.checked_changed.emit(normalized);
    }

    /// Toggles checked state when checkable.
    pub fn toggle_checked(&mut self) {
        if self.checkable {
            self.set_checked(!self.checked);
        }
    }
}
impl_widget_delegate!(GroupBox, base);

/// Tab widget with deterministic selected-index contract.
pub struct TabWidget {
    base: BaseWidget,
    tabs: Vec<ObjectId>,
    current_index: Option<usize>,
    pub current_index_changed: Signal1<usize>,
}

impl TabWidget {
    /// Creates an empty tab widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabWidget, geometry, "TabWidget"),
            tabs: Vec::new(),
            current_index: None,
            current_index_changed: Signal1::new(),
        }
    }

    /// Returns number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns current selected tab index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Returns current selected tab object id.
    pub fn current_tab(&self) -> Option<ObjectId> {
        self.current_index
            .and_then(|index| self.tabs.get(index).copied())
    }

    /// Adds a tab by page object id and returns assigned index.
    pub fn add_tab(&mut self, page_id: ObjectId) -> usize {
        self.tabs.push(page_id);
        let index = self.tabs.len() - 1;
        if self.current_index.is_none() {
            self.current_index = Some(0);
            self.current_index_changed.emit(0);
        }
        index
    }

    /// Removes a tab by page object id.
    pub fn remove_tab(&mut self, page_id: ObjectId) -> bool {
        let Some(removed_index) = self.tabs.iter().position(|id| *id == page_id) else {
            return false;
        };
        self.tabs.remove(removed_index);

        let next_index = match self.current_index {
            None => None,
            Some(_) if self.tabs.is_empty() => None,
            Some(current) if current == removed_index => {
                Some(removed_index.min(self.tabs.len().saturating_sub(1)))
            }
            Some(current) if current > removed_index => Some(current - 1),
            Some(current) => Some(current),
        };

        if self.current_index != next_index {
            self.current_index = next_index;
            if let Some(index) = self.current_index {
                self.current_index_changed.emit(index);
            }
        }

        true
    }

    /// Selects current tab index and emits changed signal when state transitions.
    pub fn set_current_index(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        if self.current_index == Some(index) {
            return true;
        }
        self.current_index = Some(index);
        self.current_index_changed.emit(index);
        true
    }
}
impl_widget_delegate!(TabWidget, base);

/// Splitter orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterOrientation {
    Horizontal,
    Vertical,
}

/// Splitter widget with deterministic pane-ratio distribution contract.
pub struct Splitter {
    base: BaseWidget,
    orientation: SplitterOrientation,
    panes: Vec<ObjectId>,
    ratios: Vec<f32>,
    pub pane_layout_changed: Signal1<Vec<f32>>,
    pub orientation_changed: Signal1<SplitterOrientation>,
}

impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            orientation: SplitterOrientation::Horizontal,
            panes: Vec::new(),
            ratios: Vec::new(),
            pane_layout_changed: Signal1::new(),
            orientation_changed: Signal1::new(),
        }
    }

    /// Returns splitter orientation.
    pub fn orientation(&self) -> SplitterOrientation {
        self.orientation
    }

    /// Sets splitter orientation and emits change signal on transition.
    pub fn set_orientation(&mut self, orientation: SplitterOrientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.orientation_changed.emit(orientation);
    }

    /// Returns pane count.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Returns pane ids in stable order.
    pub fn pane_ids(&self) -> &[ObjectId] {
        &self.panes
    }

    /// Returns ratio for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.ratios.get(index).copied()
    }

    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        self.panes.push(pane_id);
        self.ratios.push((stretch.max(1)) as f32);
        self.pane_layout_changed.emit(self.ratios.clone());
        self.panes.len() - 1
    }

    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let Some(index) = self.panes.iter().position(|id| *id == pane_id) else {
            return false;
        };
        self.panes.remove(index);
        self.ratios.remove(index);
        self.pane_layout_changed.emit(self.ratios.clone());
        true
    }

    /// Sets pane ratio and emits layout change signal.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        let Some(slot) = self.ratios.get_mut(index) else {
            return false;
        };
        let normalized = ratio.max(0.01);
        if (*slot - normalized).abs() <= f32::EPSILON {
            return true;
        }
        *slot = normalized;
        self.pane_layout_changed.emit(self.ratios.clone());
        true
    }

    /// Returns deterministic pane sizes for a primary axis length and splitter spacing.
    pub fn distribute_sizes(&self, primary_extent: u32, spacing: u32) -> Vec<u32> {
        if self.panes.is_empty() {
            return Vec::new();
        }
        let gaps = (self.panes.len().saturating_sub(1)) as u32;
        let available = primary_extent.saturating_sub(gaps.saturating_mul(spacing));
        let total_ratio = self.ratios.iter().copied().sum::<f32>().max(0.01);

        let mut sizes = self
            .ratios
            .iter()
            .map(|ratio| (((available as f32) * (*ratio / total_ratio)).max(1.0)) as u32)
            .collect::<Vec<_>>();

        let mut assigned: u32 = sizes.iter().sum();
        while assigned < available {
            for size in &mut sizes {
                if assigned >= available {
                    break;
                }
                *size = size.saturating_add(1);
                assigned = assigned.saturating_add(1);
            }
        }
        while assigned > available {
            for size in sizes.iter_mut().rev() {
                if assigned <= available {
                    break;
                }
                if *size > 1 {
                    *size = size.saturating_sub(1);
                    assigned = assigned.saturating_sub(1);
                }
            }
            if sizes.iter().all(|size| *size <= 1) {
                break;
            }
        }

        sizes
    }
}
impl_widget_delegate!(Splitter, base);

simple_control!(StackWidget, WidgetKind::StackWidget);

/// Docking area used by `DockPanel` pane placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockArea {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

/// Dock panel container with deterministic pane placement contract.
pub struct DockPanel {
    base: BaseWidget,
    panes: Vec<(ObjectId, DockArea)>,
    pub pane_added: Signal1<ObjectId>,
    pub pane_removed: Signal1<ObjectId>,
    pub layout_changed: Signal1<Vec<(ObjectId, DockArea)>>,
}

impl DockPanel {
    /// Creates an empty dock panel.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DockPanel, geometry, "DockPanel"),
            panes: Vec::new(),
            pane_added: Signal1::new(),
            pane_removed: Signal1::new(),
            layout_changed: Signal1::new(),
        }
    }

    /// Returns ordered pane list.
    pub fn panes(&self) -> &[(ObjectId, DockArea)] {
        &self.panes
    }

    /// Adds pane to one dock area.
    pub fn add_pane(&mut self, pane_id: ObjectId, area: DockArea) -> bool {
        if self.panes.iter().any(|(id, _)| *id == pane_id) {
            return false;
        }
        self.panes.push((pane_id, area));
        self.pane_added.emit(pane_id);
        self.layout_changed.emit(self.panes.clone());
        true
    }

    /// Removes pane from layout.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let Some(index) = self.panes.iter().position(|(id, _)| *id == pane_id) else {
            return false;
        };
        self.panes.remove(index);
        self.pane_removed.emit(pane_id);
        self.layout_changed.emit(self.panes.clone());
        true
    }

    /// Returns pane area when present.
    pub fn pane_area(&self, pane_id: ObjectId) -> Option<DockArea> {
        self.panes
            .iter()
            .find(|(id, _)| *id == pane_id)
            .map(|(_, area)| *area)
    }

    /// Moves one pane to target area.
    pub fn move_pane(&mut self, pane_id: ObjectId, area: DockArea) -> bool {
        let Some((_, current_area)) = self.panes.iter_mut().find(|(id, _)| *id == pane_id) else {
            return false;
        };
        if *current_area == area {
            return true;
        }
        *current_area = area;
        self.layout_changed.emit(self.panes.clone());
        true
    }
}

impl_widget_delegate!(DockPanel, base);

/// Multiple-document area with deterministic active-document contract.
pub struct MdiArea {
    base: BaseWidget,
    documents: Vec<ObjectId>,
    active_document: Option<ObjectId>,
    pub document_added: Signal1<ObjectId>,
    pub document_removed: Signal1<ObjectId>,
    pub active_document_changed: Signal1<Option<ObjectId>>,
}

impl MdiArea {
    /// Creates an empty MDI area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MdiArea, geometry, "MdiArea"),
            documents: Vec::new(),
            active_document: None,
            document_added: Signal1::new(),
            document_removed: Signal1::new(),
            active_document_changed: Signal1::new(),
        }
    }

    /// Returns ordered document ids.
    pub fn documents(&self) -> &[ObjectId] {
        &self.documents
    }

    /// Returns active document id when present.
    pub fn active_document(&self) -> Option<ObjectId> {
        self.active_document
    }

    /// Adds one document to MDI area.
    pub fn add_document(&mut self, document_id: ObjectId) -> bool {
        if self.documents.contains(&document_id) {
            return false;
        }
        self.documents.push(document_id);
        self.document_added.emit(document_id);
        if self.active_document.is_none() {
            self.active_document = Some(document_id);
            self.active_document_changed.emit(self.active_document);
        }
        true
    }

    /// Removes one document from MDI area.
    pub fn remove_document(&mut self, document_id: ObjectId) -> bool {
        let Some(index) = self.documents.iter().position(|id| *id == document_id) else {
            return false;
        };
        self.documents.remove(index);
        self.document_removed.emit(document_id);

        if self.active_document == Some(document_id) {
            self.active_document = self.documents.first().copied();
            self.active_document_changed.emit(self.active_document);
        }
        true
    }

    /// Activates one existing document.
    pub fn set_active_document(&mut self, document_id: ObjectId) -> bool {
        if !self.documents.contains(&document_id) {
            return false;
        }
        if self.active_document == Some(document_id) {
            return true;
        }
        self.active_document = Some(document_id);
        self.active_document_changed.emit(self.active_document);
        true
    }
}

impl_widget_delegate!(MdiArea, base);

/// Menu bar widget managing menu host ordering.
pub struct MenuBar {
    base: BaseWidget,
    menus: Vec<ObjectId>,
    current_menu: Option<ObjectId>,
    pub menu_added: Signal1<ObjectId>,
    pub menu_removed: Signal1<ObjectId>,
    pub current_menu_changed: Signal1<Option<ObjectId>>,
}

impl MenuBar {
    /// Creates an empty menu bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MenuBar, geometry, "MenuBar"),
            menus: Vec::new(),
            current_menu: None,
            menu_added: Signal1::new(),
            menu_removed: Signal1::new(),
            current_menu_changed: Signal1::new(),
        }
    }

    /// Returns ordered menu ids.
    pub fn menus(&self) -> &[ObjectId] {
        &self.menus
    }

    /// Returns current menu id when selected.
    pub fn current_menu(&self) -> Option<ObjectId> {
        self.current_menu
    }

    /// Adds one menu id, returns false when already present.
    pub fn add_menu(&mut self, menu_id: ObjectId) -> bool {
        if self.menus.contains(&menu_id) {
            return false;
        }
        self.menus.push(menu_id);
        self.menu_added.emit(menu_id);
        if self.current_menu.is_none() {
            self.current_menu = Some(menu_id);
            self.current_menu_changed.emit(self.current_menu);
        }
        true
    }

    /// Removes one menu id.
    pub fn remove_menu(&mut self, menu_id: ObjectId) -> bool {
        let Some(index) = self.menus.iter().position(|id| *id == menu_id) else {
            return false;
        };
        self.menus.remove(index);
        self.menu_removed.emit(menu_id);

        if self.current_menu == Some(menu_id) {
            self.current_menu = self.menus.first().copied();
            self.current_menu_changed.emit(self.current_menu);
        }
        true
    }

    /// Selects current menu by id.
    pub fn set_current_menu(&mut self, menu_id: ObjectId) -> bool {
        if !self.menus.contains(&menu_id) {
            return false;
        }
        if self.current_menu == Some(menu_id) {
            return true;
        }
        self.current_menu = Some(menu_id);
        self.current_menu_changed.emit(self.current_menu);
        true
    }
}
impl_widget_delegate!(MenuBar, base);

/// Menu widget with action-host contract.
pub struct Menu {
    base: BaseWidget,
    title: String,
    action_ids: Vec<String>,
    pub action_added: Signal1<String>,
    pub action_removed: Signal1<String>,
    pub action_triggered: Signal1<String>,
}

impl Menu {
    /// Creates an empty menu host.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Menu, geometry, "Menu"),
            title: String::new(),
            action_ids: Vec::new(),
            action_added: Signal1::new(),
            action_removed: Signal1::new(),
            action_triggered: Signal1::new(),
        }
    }

    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns action ids bound to menu.
    pub fn actions(&self) -> &[String] {
        &self.action_ids
    }

    /// Adds one action id.
    pub fn add_action(&mut self, action_id: impl Into<String>) -> bool {
        let action_id = action_id.into();
        if self.action_ids.iter().any(|id| id == &action_id) {
            return false;
        }
        self.action_ids.push(action_id.clone());
        self.action_added.emit(action_id);
        true
    }

    /// Removes one action id.
    pub fn remove_action(&mut self, action_id: &str) -> bool {
        let Some(index) = self.action_ids.iter().position(|id| id == action_id) else {
            return false;
        };
        let removed = self.action_ids.remove(index);
        self.action_removed.emit(removed);
        true
    }

    /// Emits action-triggered route when action id exists.
    pub fn trigger_action(&self, action_id: &str) -> bool {
        if !self.action_ids.iter().any(|id| id == action_id) {
            return false;
        }
        self.action_triggered.emit(action_id.to_string());
        true
    }
}
impl_widget_delegate!(Menu, base);

/// Toolbar widget with action-host contract.
pub struct ToolBar {
    base: BaseWidget,
    action_ids: Vec<String>,
    pub action_added: Signal1<String>,
    pub action_removed: Signal1<String>,
    pub action_triggered: Signal1<String>,
}

impl ToolBar {
    /// Creates an empty toolbar host.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToolBar, geometry, "ToolBar"),
            action_ids: Vec::new(),
            action_added: Signal1::new(),
            action_removed: Signal1::new(),
            action_triggered: Signal1::new(),
        }
    }

    /// Returns action ids bound to toolbar.
    pub fn actions(&self) -> &[String] {
        &self.action_ids
    }

    /// Adds one action id.
    pub fn add_action(&mut self, action_id: impl Into<String>) -> bool {
        let action_id = action_id.into();
        if self.action_ids.iter().any(|id| id == &action_id) {
            return false;
        }
        self.action_ids.push(action_id.clone());
        self.action_added.emit(action_id);
        true
    }

    /// Removes one action id.
    pub fn remove_action(&mut self, action_id: &str) -> bool {
        let Some(index) = self.action_ids.iter().position(|id| id == action_id) else {
            return false;
        };
        let removed = self.action_ids.remove(index);
        self.action_removed.emit(removed);
        true
    }

    /// Emits action-triggered route when action id exists.
    pub fn trigger_action(&self, action_id: &str) -> bool {
        if !self.action_ids.iter().any(|id| id == action_id) {
            return false;
        }
        self.action_triggered.emit(action_id.to_string());
        true
    }
}
impl_widget_delegate!(ToolBar, base);

/// Status bar widget with deterministic message contract.
pub struct StatusBar {
    base: BaseWidget,
    message: String,
    pub message_changed: Signal1<String>,
}

impl StatusBar {
    /// Creates an empty status bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "StatusBar"),
            message: String::new(),
            message_changed: Signal1::new(),
        }
    }

    /// Returns current status message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets status message and emits on change.
    pub fn set_message(&mut self, message: String) {
        if self.message == message {
            return;
        }
        self.message = message.clone();
        self.message_changed.emit(message);
    }
}
impl_widget_delegate!(StatusBar, base);

simple_control!(Canvas, WidgetKind::Canvas);

/// Tree view widget with optional external model binding.
pub struct TreeView {
    base: BaseWidget,
    /// Optional bound tree model.
    model: Option<Arc<dyn TreeModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    // ...existing code...
    /// View-side selected node index.
    selected_node: Option<usize>,
    /// View-side focused node index.
    focused_node: Option<usize>,
    /// Emitted when selected node changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused node changes.
    pub focused_node_changed: Signal1<Option<usize>>,
}

impl TreeView {
    /// Creates an empty tree view.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selected_node: None,
            focused_node: None,
            selection_changed: Signal1::new(),
            focused_node_changed: Signal1::new(),
        }
    }

    /// Binds an external tree model.
    pub fn set_model(&mut self, model: Arc<dyn TreeModel>) {
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Deprecated: add_node is no longer supported. TreeView requires a bound model.
    #[deprecated(note = "Imperative add_node is removed. Use set_model with a TreeModel.")]
    pub fn add_node(&mut self, _node: impl Into<String>) {
        panic!("TreeView::add_node is deprecated. Use set_model with a TreeModel.");
    }

    /// Returns current visible node count.
    pub fn node_count(&self) -> usize {
        self.model
            .as_ref()
            .map(|model| model.node_count())
            .unwrap_or(0)
    }

    /// Returns node path by visible index.
    pub fn node_path(&self, index: usize) -> Option<String> {
        self.model.as_ref().and_then(|model| model.node_path(index))
    }

    /// Selects a node by visible index.
    pub fn select_node(&mut self, index: usize) -> bool {
        if index < self.node_count() {
            self.selected_node = Some(index);
            self.selection_changed.emit(index);
            self.set_focused_node(index);
            true
        } else {
            false
        }
    }

    /// Clears node selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
    }

    /// Sets focused node by visible index.
    pub fn set_focused_node(&mut self, index: usize) -> bool {
        if index >= self.node_count() {
            return false;
        }
        if self.focused_node == Some(index) {
            return true;
        }
        self.focused_node = Some(index);
        self.focused_node_changed.emit(self.focused_node);
        true
    }

    /// Clears node focus.
    pub fn clear_focused_node(&mut self) {
        if self.focused_node.is_none() {
            return;
        }
        self.focused_node = None;
        self.focused_node_changed.emit(None);
    }

    /// Returns focused node index when present.
    pub fn focused_node(&self) -> Option<usize> {
        self.focused_node.filter(|index| *index < self.node_count())
    }

    /// Returns selected node index if present.
    pub fn selected_node(&self) -> Option<usize> {
        self.selected_node
            .filter(|index| *index < self.node_count())
    }

    fn normalize_projection_state(&mut self) {
        let node_count = self.node_count();
        self.selected_node = self.selected_node.filter(|index| *index < node_count);
        self.focused_node = self.focused_node.filter(|index| *index < node_count);
    }
}

impl_widget_delegate!(TreeView, base);

/// Table widget with model/view helpers and selection state.
pub struct TableWidget {
    base: BaseWidget,
    /// Optional bound data model.
    model: Option<Arc<dyn TableModel>>,
    /// Scoped model-to-view signal subscriptions.
    model_connection_scope: ConnectionScope,
    /// View-side selection state.
    selection: SelectionModel,
    /// View-side focused row.
    focused_row: Option<usize>,
    /// Explicit column width overrides in logical pixels.
    column_widths: HashMap<usize, u32>,
    /// Explicit row height overrides in logical pixels.
    row_heights: HashMap<usize, u32>,
    /// Optional display/editor delegate.
    delegate: Option<Arc<dyn ItemDelegate>>,
    /// Emitted when selected row changes.
    pub selection_changed: Signal1<usize>,
    /// Emitted when focused row changes.
    pub focused_row_changed: Signal1<Option<usize>>,
}

impl TableWidget {
    /// Creates an empty table widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
            model_connection_scope: ConnectionScope::new(),
            selection: SelectionModel::new(),
            focused_row: None,
            column_widths: HashMap::new(),
            row_heights: HashMap::new(),
            delegate: None,
            selection_changed: Signal1::new(),
            focused_row_changed: Signal1::new(),
        }
    }

    /// Binds an external table model.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.model_connection_scope = ConnectionScope::new();
        if let Some(data_changed) = model.data_changed_signal() {
            let redraw = self.base.redraw_requested_signal().clone();
            let layout = self.base.layout_requested_signal().clone();
            data_changed.connect_scoped(&self.model_connection_scope, move || {
                redraw.emit();
                layout.emit();
            });
        }
        self.model = Some(model);
        self.normalize_projection_state();
        self.base.request_layout();
        self.base.request_redraw();
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
        self.model
            .as_ref()
            .and_then(|m| m.data_with_role(row, col, role))
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
            self.set_focused_row(row);
            true
        } else {
            false
        }
    }

    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Sets focused row in current projection.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        if row >= self.row_count() {
            return false;
        }
        if self.focused_row == Some(row) {
            return true;
        }
        self.focused_row = Some(row);
        self.focused_row_changed.emit(self.focused_row);
        true
    }

    /// Clears focused row.
    pub fn clear_focused_row(&mut self) {
        if self.focused_row.is_none() {
            return;
        }
        self.focused_row = None;
        self.focused_row_changed.emit(None);
    }

    /// Returns focused row when still visible in projection.
    pub fn focused_row(&self) -> Option<usize> {
        self.focused_row.filter(|row| *row < self.row_count())
    }

    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selection
            .current_row()
            .filter(|row| *row < self.row_count())
    }

    /// All selected rows in stable order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.selection
            .rows()
            .into_iter()
            .filter(|row| *row < self.row_count())
            .collect()
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

    fn normalize_projection_state(&mut self) {
        let row_count = self.row_count();
        self.selection.selected_rows.retain(|row| *row < row_count);
        self.selection.current_row = self.selection.current_row.filter(|row| *row < row_count);
        self.focused_row = self.focused_row.filter(|row| *row < row_count);
    }
}

impl_widget_delegate!(TableWidget, base);

/// Dedicated table-view widget contract with table model projection parity.
///
/// TableView provides a simplified interface to TableWidget, focusing on model-view functionality
/// with a clean API for common table operations.
pub struct TableView {
    table: TableWidget,
}

impl TableView {
    /// Creates an empty table view with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            table: TableWidget::new(geometry),
        }
    }

    /// Binds an external table model to the view.
    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.table.set_model(model);
    }

    /// Returns the number of visible rows in the table.
    pub fn row_count(&self) -> usize {
        self.table.row_count()
    }

    /// Returns the number of visible columns in the table.
    pub fn column_count(&self) -> usize {
        self.table.column_count()
    }

    /// Reads the table header text for the specified column.
    pub fn header(&self, col: usize) -> Option<String> {
        self.table.header(col)
    }

    /// Reads the table cell value at the specified row and column.
    pub fn cell(&self, row: usize, col: usize) -> Option<String> {
        self.table.cell(row, col)
    }

    /// Reads the formatted display value for the specified cell, taking into account any item delegate.
    pub fn display_cell(&self, row: usize, col: usize) -> Option<String> {
        self.table.display_cell(row, col)
    }

    /// Sets an item delegate for display and editor conversion.
    pub fn set_delegate(&mut self, delegate: Arc<dyn ItemDelegate>) {
        self.table.set_delegate(delegate);
    }

    /// Clears any custom item delegate, reverting to default behavior.
    pub fn clear_delegate(&mut self) {
        self.table.clear_delegate();
    }

    /// Selects a single row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        self.table.select_row(row)
    }

    /// Clears the current row selection.
    pub fn clear_selection(&mut self) {
        self.table.clear_selection();
    }

    /// Returns the currently selected row index, if any.
    pub fn selected_row(&self) -> Option<usize> {
        self.table.selected_row()
    }

    /// Returns all selected rows in stable (sorted) order.
    pub fn selected_rows(&self) -> Vec<usize> {
        self.table.selected_rows()
    }

    /// Sets the row selection mode (single or multi-select).
    pub fn set_selection_mode(&mut self, mode: SelectionMode) {
        self.table.set_selection_mode(mode);
    }

    /// Returns the current selection mode.
    pub fn selection_mode(&self) -> SelectionMode {
        self.table.selection_mode()
    }

    /// Sets the focused row in the current projection.
    pub fn set_focused_row(&mut self, row: usize) -> bool {
        self.table.set_focused_row(row)
    }

    /// Clears the focused row.
    pub fn clear_focused_row(&mut self) {
        self.table.clear_focused_row();
    }

    /// Returns the focused row index, if still visible in the projection.
    pub fn focused_row(&self) -> Option<usize> {
        self.table.focused_row()
    }

    /// Returns the signal emitted when the selection changes.
    pub fn selection_changed_signal(&self) -> &Signal1<usize> {
        &self.table.selection_changed
    }

    /// Returns the signal emitted when the focused row changes.
    pub fn focused_row_changed_signal(&self) -> &Signal1<Option<usize>> {
        &self.table.focused_row_changed
    }
}

impl_widget_delegate!(TableView, table);

simple_control!(GridWidget, WidgetKind::Grid);

simple_control!(ChartWidget, WidgetKind::Chart);

/// Toggle button state enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleButtonState {
    Normal,
    Checked,
    Disabled,
}

pub struct ToggleButton {
    base: BaseWidget,
    text: String,
    checked: bool,
    auto_exclusive: bool,
    group_id: Option<String>,
    pressed: bool,
    pub toggled: Signal1<bool>,
    pub checked_changed: Signal1<bool>,
    pub pressed_signal: GenericSignal,
    pub released_signal: GenericSignal,
    pub state_changed: Signal1<ToggleButtonState>,
}

impl ToggleButton {
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "ToggleButton"),
            text,
            checked: false,
            auto_exclusive: false,
            group_id: None,
            pressed: false,
            toggled: Signal1::new(),
            checked_changed: Signal1::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, text: String) {
        if self.text != text {
            self.text = text;
        }
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.checked_changed.emit(checked);
        self.toggled.emit(checked);
        self.state_changed.emit(self.state());
    }

    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }

    pub fn set_auto_exclusive(&mut self, exclusive: bool) {
        self.auto_exclusive = exclusive;
    }

    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }

    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    fn set_pressed(&mut self, pressed: bool) {
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

    pub fn state(&self) -> ToggleButtonState {
        if !self.base.is_enabled() {
            ToggleButtonState::Disabled
        } else if self.checked {
            ToggleButtonState::Checked
        } else {
            ToggleButtonState::Normal
        }
    }

    pub fn select_in_group(peers: &mut [&mut ToggleButton], selected_index: usize) -> bool {
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

impl Widget for ToggleButton {
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
        self.state_changed.emit(self.state());
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

impl EventHandler for ToggleButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { .. } => {
                self.set_pressed(true);
            }
            Event::MouseRelease { .. } => {
                let was_pressed = self.is_pressed();
                self.set_pressed(false);
                if was_pressed {
                    self.toggle();
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckListBoxItemState {
    Unchecked,
    Checked,
    PartiallyChecked,
}

#[derive(Debug, Clone)]
pub struct CheckListBoxItem {
    text: String,
    state: CheckListBoxItemState,
    enabled: bool,
}

impl CheckListBoxItem {
    pub fn new(text: String) -> Self {
        Self {
            text,
            state: CheckListBoxItemState::Unchecked,
            enabled: true,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }
    pub fn state(&self) -> CheckListBoxItemState {
        self.state
    }
    pub fn set_state(&mut self, state: CheckListBoxItemState) {
        self.state = state;
    }
    pub fn is_checked(&self) -> bool {
        matches!(self.state, CheckListBoxItemState::Checked)
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

pub struct CheckListBox {
    base: BaseWidget,
    items: Vec<CheckListBoxItem>,
    selected_index: Option<usize>,
    tristate_enabled: bool,
    pub item_state_changed: Signal1<(usize, CheckListBoxItemState)>,
    pub selection_changed: Signal1<usize>,
    pub item_checked: Signal1<usize>,
    pub item_unchecked: Signal1<usize>,
}

impl CheckListBox {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckListBox, geometry, "CheckListBox"),
            items: Vec::new(),
            selected_index: None,
            tristate_enabled: false,
            item_state_changed: Signal1::new(),
            selection_changed: Signal1::new(),
            item_checked: Signal1::new(),
            item_unchecked: Signal1::new(),
        }
    }

    pub fn add_item(&mut self, text: impl Into<String>) -> usize {
        let item = CheckListBoxItem::new(text.into());
        self.items.push(item);
        self.items.len() - 1
    }

    pub fn insert_item(&mut self, index: usize, text: impl Into<String>) -> bool {
        if index > self.items.len() {
            return false;
        }
        let item = CheckListBoxItem::new(text.into());
        self.items.insert(index, item);
        true
    }

    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        if self.selected_index == Some(index) {
            self.selected_index = None;
        } else if self.selected_index.map(|i| i > index).unwrap_or(false) {
            self.selected_index = self.selected_index.map(|i| i - 1);
        }
        true
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }
    pub fn item(&self, index: usize) -> Option<&CheckListBoxItem> {
        self.items.get(index)
    }
    pub fn item_mut(&mut self, index: usize) -> Option<&mut CheckListBoxItem> {
        self.items.get_mut(index)
    }
    pub fn item_text(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(|item| item.text.as_str())
    }

    pub fn set_item_text(&mut self, index: usize, text: String) -> bool {
        if let Some(item) = self.items.get_mut(index) {
            item.set_text(text);
            true
        } else {
            false
        }
    }

    pub fn item_state(&self, index: usize) -> Option<CheckListBoxItemState> {
        self.items.get(index).map(|item| item.state())
    }

    pub fn set_item_state(&mut self, index: usize, state: CheckListBoxItemState) -> bool {
        if index >= self.items.len() {
            return false;
        }

        let normalized_state =
            if !self.tristate_enabled && state == CheckListBoxItemState::PartiallyChecked {
                CheckListBoxItemState::Unchecked
            } else {
                state
            };

        let item = &mut self.items[index];
        if item.state() == normalized_state {
            return true;
        }

        let was_checked = item.is_checked();
        item.set_state(normalized_state);
        let is_checked = item.is_checked();

        self.item_state_changed.emit((index, normalized_state));

        if was_checked && !is_checked {
            self.item_unchecked.emit(index);
        } else if !was_checked && is_checked {
            self.item_checked.emit(index);
        }

        true
    }

    pub fn is_item_checked(&self, index: usize) -> bool {
        self.items
            .get(index)
            .map(|item| item.is_checked())
            .unwrap_or(false)
    }

    pub fn set_item_checked(&mut self, index: usize, checked: bool) -> bool {
        let state = if checked {
            CheckListBoxItemState::Checked
        } else {
            CheckListBoxItemState::Unchecked
        };
        self.set_item_state(index, state)
    }

    pub fn toggle_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }

        let item = &self.items[index];
        let next_state = if self.tristate_enabled {
            match item.state() {
                CheckListBoxItemState::Unchecked => CheckListBoxItemState::PartiallyChecked,
                CheckListBoxItemState::PartiallyChecked => CheckListBoxItemState::Checked,
                CheckListBoxItemState::Checked => CheckListBoxItemState::Unchecked,
            }
        } else if item.is_checked() {
            CheckListBoxItemState::Unchecked
        } else {
            CheckListBoxItemState::Checked
        };

        self.set_item_state(index, next_state)
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        self.selection_changed.emit(index);
        true
    }

    pub fn clear_selection(&mut self) {
        if self.selected_index.is_some() {
            self.selected_index = None;
        }
    }

    pub fn is_tristate_enabled(&self) -> bool {
        self.tristate_enabled
    }

    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled {
            for (index, item) in self.items.iter_mut().enumerate() {
                if item.state() == CheckListBoxItemState::PartiallyChecked {
                    item.set_state(CheckListBoxItemState::Unchecked);
                    self.item_state_changed
                        .emit((index, CheckListBoxItemState::Unchecked));
                }
            }
        }
    }

    pub fn checked_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_checked())
            .map(|(index, _)| index)
            .collect()
    }

    pub fn check_all(&mut self) {
        for index in 0..self.items.len() {
            self.set_item_checked(index, true);
        }
    }

    pub fn uncheck_all(&mut self) {
        for index in 0..self.items.len() {
            self.set_item_checked(index, false);
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.selected_index = None;
    }
}

impl Widget for CheckListBox {
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

impl EventHandler for CheckListBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

pub struct DoubleSpinBox {
    base: BaseWidget,
    min: f64,
    max: f64,
    value: f64,
    single_step: f64,
    decimals: u32,
    prefix: String,
    suffix: String,
    pub value_changed: Signal1<f64>,
    pub decimals_changed: Signal1<u32>,
}

impl DoubleSpinBox {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DoubleSpinBox, geometry, "DoubleSpinBox"),
            min: 0.0,
            max: 100.0,
            value: 0.0,
            single_step: 1.0,
            decimals: 2,
            prefix: String::new(),
            suffix: String::new(),
            value_changed: Signal1::new(),
            decimals_changed: Signal1::new(),
        }
    }

    pub fn min(&self) -> f64 {
        self.min
    }
    pub fn max(&self) -> f64 {
        self.max
    }
    pub fn value(&self) -> f64 {
        self.value
    }
    pub fn single_step(&self) -> f64 {
        self.single_step
    }
    pub fn decimals(&self) -> u32 {
        self.decimals
    }
    pub fn prefix(&self) -> &str {
        &self.prefix
    }
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    pub fn set_range(&mut self, min: f64, max: f64) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    pub fn set_single_step(&mut self, step: f64) {
        self.single_step = step.max(0.0);
    }

    pub fn set_decimals(&mut self, decimals: u32) {
        if self.decimals != decimals {
            self.decimals = decimals;
            self.decimals_changed.emit(decimals);
        }
    }

    pub fn set_prefix(&mut self, prefix: String) {
        self.prefix = prefix;
    }
    pub fn set_suffix(&mut self, suffix: String) {
        self.suffix = suffix;
    }

    pub fn set_value(&mut self, value: f64) {
        let factor = 10f64.powi(self.decimals as i32);
        let rounded = (value * factor).round() / factor;
        let clamped = rounded.clamp(self.min, self.max);

        if (self.value - clamped).abs() > f64::EPSILON {
            self.value = clamped;
            self.value_changed.emit(clamped);
        }
    }

    pub fn step_up(&mut self) {
        self.set_value(self.value + self.single_step);
    }
    pub fn step_down(&mut self) {
        self.set_value(self.value - self.single_step);
    }

    pub fn text(&self) -> String {
        format!(
            "{}{:.decimals$}{}",
            self.prefix,
            self.value,
            self.suffix,
            decimals = self.decimals as usize
        )
    }

    pub fn set_text(&mut self, text: &str) {
        let text_without_prefix_suffix = text
            .strip_prefix(&self.prefix)
            .unwrap_or(text)
            .strip_suffix(&self.suffix)
            .unwrap_or(text);

        if let Ok(value) = text_without_prefix_suffix.trim().parse::<f64>() {
            self.set_value(value);
        }
    }
}

impl Widget for DoubleSpinBox {
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

impl EventHandler for DoubleSpinBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

pub struct Dial {
    base: BaseWidget,
    min: i32,
    max: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    wrapping: bool,
    notch_target: i32,
    pub value_changed: Signal1<i32>,
    pub pressed: GenericSignal,
    pub released: GenericSignal,
}

impl Dial {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Dial, geometry, "Dial"),
            min: 0,
            max: 100,
            value: 0,
            single_step: 1,
            page_step: 10,
            wrapping: false,
            notch_target: 0,
            value_changed: Signal1::new(),
            pressed: GenericSignal::new(),
            released: GenericSignal::new(),
        }
    }

    pub fn min(&self) -> i32 {
        self.min
    }
    pub fn max(&self) -> i32 {
        self.max
    }
    pub fn value(&self) -> i32 {
        self.value
    }
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    pub fn is_wrapping(&self) -> bool {
        self.wrapping
    }
    pub fn notch_target(&self) -> i32 {
        self.notch_target
    }

    pub fn set_range(&mut self, min: i32, max: i32) {
        self.min = min;
        self.max = max.max(min);
        self.set_value(self.value);
    }

    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
    }
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
    }
    pub fn set_wrapping(&mut self, wrapping: bool) {
        self.wrapping = wrapping;
    }
    pub fn set_notch_target(&mut self, target: i32) {
        self.notch_target = target;
    }

    pub fn set_value(&mut self, value: i32) {
        let normalized = if self.wrapping {
            let range = self.max - self.min + 1;
            if range > 0 {
                let offset = (value - self.min).rem_euclid(range);
                self.min + offset
            } else {
                self.min
            }
        } else {
            value.clamp(self.min, self.max)
        };

        if self.value != normalized {
            self.value = normalized;
            self.value_changed.emit(normalized);
        }
    }

    pub fn step_up(&mut self) {
        self.set_value(self.value.saturating_add(self.single_step));
    }
    pub fn step_down(&mut self) {
        self.set_value(self.value.saturating_sub(self.single_step));
    }
    pub fn page_up(&mut self) {
        self.set_value(self.value.saturating_add(self.page_step));
    }
    pub fn page_down(&mut self) {
        self.set_value(self.value.saturating_sub(self.page_step));
    }

    pub fn angle(&self) -> f64 {
        if self.max == self.min {
            return 0.0;
        }
        let ratio = (self.value - self.min) as f64 / (self.max - self.min) as f64;
        ratio * 360.0
    }

    pub fn set_angle(&mut self, angle: f64) {
        if self.max == self.min {
            return;
        }
        let normalized_angle = angle.rem_euclid(360.0);
        let ratio = normalized_angle / 360.0;
        let value = self.min + (ratio * (self.max - self.min) as f64).round() as i32;
        self.set_value(value);
    }
}

impl Widget for Dial {
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

impl EventHandler for Dial {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        match event {
            Event::MousePress { .. } => {
                self.pressed.emit();
            }
            Event::MouseRelease { .. } => {
                self.released.emit();
            }
            _ => {}
        }
    }
}

/// Wizard page structure.
pub struct WizardPage {
    id: ObjectId,
    title: String,
    subtitle: String,
    enabled: bool,
    complete: bool,
}

impl WizardPage {
    pub fn new(id: ObjectId, title: String, subtitle: String) -> Self {
        Self {
            id,
            title,
            subtitle,
            enabled: true,
            complete: false,
        }
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn subtitle(&self) -> &str {
        &self.subtitle
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    pub fn set_subtitle(&mut self, subtitle: String) {
        self.subtitle = subtitle;
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn set_complete(&mut self, complete: bool) {
        self.complete = complete;
    }
}

/// Wizard widget for multi-step dialogs.
pub struct Wizard {
    base: BaseWidget,
    pages: Vec<WizardPage>,
    current_page: Option<usize>,
    title: String,
    /// Emitted when the current page changes.
    pub current_page_changed: Signal1<usize>,
    /// Emitted when the wizard is finished.
    pub finished: Signal1<WizardResult>,
    /// Emitted when the wizard is canceled.
    pub canceled: GenericSignal,
    /// Emitted when a page is added.
    pub page_added: Signal1<usize>,
    /// Emitted when a page is removed.
    pub page_removed: Signal1<usize>,
}

/// Wizard result enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardResult {
    /// Wizard was accepted.
    Accepted,
    /// Wizard was rejected.
    Rejected,
}

impl Wizard {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Wizard, geometry, "Wizard"),
            pages: Vec::new(),
            current_page: None,
            title: String::new(),
            current_page_changed: Signal1::new(),
            finished: Signal1::new(),
            canceled: GenericSignal::new(),
            page_added: Signal1::new(),
            page_removed: Signal1::new(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn add_page(&mut self, page: WizardPage) -> usize {
        let index = self.pages.len();
        self.pages.push(page);
        self.page_added.emit(index);
        if self.current_page.is_none() && !self.pages.is_empty() {
            self.set_current_page(0);
        }
        index
    }

    pub fn insert_page(&mut self, index: usize, page: WizardPage) -> bool {
        if index > self.pages.len() {
            return false;
        }
        self.pages.insert(index, page);
        self.page_added.emit(index);
        if let Some(current) = self.current_page {
            if current >= index {
                self.current_page = Some(current + 1);
            }
        }
        true
    }

    pub fn remove_page(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        self.pages.remove(index);
        self.page_removed.emit(index);
        if let Some(current) = self.current_page {
            if current == index {
                if self.pages.is_empty() {
                    self.current_page = None;
                } else {
                    self.current_page = Some(current.min(self.pages.len() - 1));
                    self.current_page_changed.emit(self.current_page.unwrap());
                }
            } else if current > index {
                self.current_page = Some(current - 1);
            }
        }
        true
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
    pub fn page(&self, index: usize) -> Option<&WizardPage> {
        self.pages.get(index)
    }
    pub fn page_mut(&mut self, index: usize) -> Option<&mut WizardPage> {
        self.pages.get_mut(index)
    }

    pub fn current_page(&self) -> Option<usize> {
        self.current_page
    }

    pub fn set_current_page(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        if self.current_page == Some(index) {
            return true;
        }
        self.current_page = Some(index);
        self.current_page_changed.emit(index);
        true
    }

    pub fn next_page(&mut self) -> bool {
        if let Some(current) = self.current_page {
            if current < self.pages.len() - 1 {
                self.set_current_page(current + 1);
                return true;
            }
        }
        false
    }

    pub fn previous_page(&mut self) -> bool {
        if let Some(current) = self.current_page {
            if current > 0 {
                self.set_current_page(current - 1);
                return true;
            }
        }
        false
    }

    pub fn accept(&mut self) {
        self.finished.emit(WizardResult::Accepted);
    }

    pub fn reject(&mut self) {
        self.finished.emit(WizardResult::Rejected);
    }

    pub fn cancel(&mut self) {
        self.canceled.emit();
    }

    pub fn is_last_page(&self) -> bool {
        if let Some(current) = self.current_page {
            current == self.pages.len() - 1
        } else {
            false
        }
    }

    pub fn is_first_page(&self) -> bool {
        self.current_page == Some(0)
    }

    pub fn can_next(&self) -> bool {
        if let Some(current) = self.current_page {
            current < self.pages.len() - 1
                && self
                    .pages
                    .get(current)
                    .map(|p| p.is_complete())
                    .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn can_previous(&self) -> bool {
        self.current_page.map(|c| c > 0).unwrap_or(false)
    }
}

impl Widget for Wizard {
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

impl EventHandler for Wizard {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Date picker widget for date selection.
pub struct DatePicker {
    base: BaseWidget,
    date: chrono::NaiveDate,
    minimum_date: Option<chrono::NaiveDate>,
    maximum_date: Option<chrono::NaiveDate>,
    calendar_popup: bool,
    /// Emitted when the date changes.
    pub date_changed: Signal1<chrono::NaiveDate>,
    /// Emitted when the calendar popup is opened.
    pub calendar_opened: GenericSignal,
    /// Emitted when the calendar popup is closed.
    pub calendar_closed: GenericSignal,
}

impl DatePicker {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DatePicker, geometry, "DatePicker"),
            date: chrono::Local::now().date_naive(),
            minimum_date: None,
            maximum_date: None,
            calendar_popup: true,
            date_changed: Signal1::new(),
            calendar_opened: GenericSignal::new(),
            calendar_closed: GenericSignal::new(),
        }
    }

    pub fn date(&self) -> chrono::NaiveDate {
        self.date
    }
    pub fn minimum_date(&self) -> Option<chrono::NaiveDate> {
        self.minimum_date
    }
    pub fn maximum_date(&self) -> Option<chrono::NaiveDate> {
        self.maximum_date
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }

    pub fn set_date(&mut self, date: chrono::NaiveDate) {
        let clamped = self.clamp_date(date);
        if self.date != clamped {
            self.date = clamped;
            self.date_changed.emit(clamped);
        }
    }

    pub fn set_minimum_date(&mut self, date: Option<chrono::NaiveDate>) {
        self.minimum_date = date;
        self.set_date(self.date);
    }

    pub fn set_maximum_date(&mut self, date: Option<chrono::NaiveDate>) {
        self.maximum_date = date;
        self.set_date(self.date);
    }

    pub fn set_calendar_popup(&mut self, enabled: bool) {
        self.calendar_popup = enabled;
    }

    fn clamp_date(&self, date: chrono::NaiveDate) -> chrono::NaiveDate {
        let mut result = date;
        if let Some(min) = self.minimum_date {
            if result < min {
                result = min;
            }
        }
        if let Some(max) = self.maximum_date {
            if result > max {
                result = max;
            }
        }
        result
    }

    pub fn add_days(&mut self, days: i64) {
        self.set_date(self.date + chrono::Duration::days(days));
    }

    pub fn add_months(&mut self, months: i32) {
        let months_u32 = months.try_into().unwrap_or(0);
        let new_date = self
            .date
            .checked_add_months(chrono::Months::new(months_u32));
        if let Some(date) = new_date {
            self.set_date(date);
        }
    }

    pub fn add_years(&mut self, years: i32) {
        let months_u32 = (years * 12).try_into().unwrap_or(0);
        let new_date = self
            .date
            .checked_add_months(chrono::Months::new(months_u32));
        if let Some(date) = new_date {
            self.set_date(date);
        }
    }

    pub fn set_date_from_ymd(&mut self, year: i32, month: u32, day: u32) {
        if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
            self.set_date(date);
        }
    }

    pub fn year(&self) -> i32 {
        self.date.year()
    }
    pub fn month(&self) -> u32 {
        self.date.month()
    }
    pub fn day(&self) -> u32 {
        self.date.day()
    }

    pub fn open_calendar(&mut self) {
        self.calendar_opened.emit();
    }

    pub fn close_calendar(&mut self) {
        self.calendar_closed.emit();
    }
}

impl Widget for DatePicker {
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

impl EventHandler for DatePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Time picker widget for time selection.
pub struct TimePicker {
    base: BaseWidget,
    time: chrono::NaiveTime,
    minimum_time: Option<chrono::NaiveTime>,
    maximum_time: Option<chrono::NaiveTime>,
    is_24_hour: bool,
    /// Emitted when the time changes.
    pub time_changed: Signal1<chrono::NaiveTime>,
    /// Emitted when the time is edited.
    pub time_edited: Signal1<String>,
}

impl TimePicker {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TimePicker, geometry, "TimePicker"),
            time: chrono::Local::now().time(),
            minimum_time: None,
            maximum_time: None,
            is_24_hour: true,
            time_changed: Signal1::new(),
            time_edited: Signal1::new(),
        }
    }

    pub fn time(&self) -> chrono::NaiveTime {
        self.time
    }
    pub fn minimum_time(&self) -> Option<chrono::NaiveTime> {
        self.minimum_time
    }
    pub fn maximum_time(&self) -> Option<chrono::NaiveTime> {
        self.maximum_time
    }
    pub fn is_24_hour(&self) -> bool {
        self.is_24_hour
    }

    pub fn set_time(&mut self, time: chrono::NaiveTime) {
        let clamped = self.clamp_time(time);
        if self.time != clamped {
            self.time = clamped;
            self.time_changed.emit(clamped);
        }
    }

    pub fn set_minimum_time(&mut self, time: Option<chrono::NaiveTime>) {
        self.minimum_time = time;
        self.set_time(self.time);
    }

    pub fn set_maximum_time(&mut self, time: Option<chrono::NaiveTime>) {
        self.maximum_time = time;
        self.set_time(self.time);
    }

    pub fn set_24_hour(&mut self, enabled: bool) {
        self.is_24_hour = enabled;
    }

    fn clamp_time(&self, time: chrono::NaiveTime) -> chrono::NaiveTime {
        let mut result = time;
        if let Some(min) = self.minimum_time {
            if result < min {
                result = min;
            }
        }
        if let Some(max) = self.maximum_time {
            if result > max {
                result = max;
            }
        }
        result
    }

    pub fn set_time_from_hms(&mut self, hour: u32, minute: u32, second: u32) {
        if let Some(time) = chrono::NaiveTime::from_hms_opt(hour, minute, second) {
            self.set_time(time);
        }
    }

    pub fn hour(&self) -> u32 {
        self.time.hour()
    }
    pub fn minute(&self) -> u32 {
        self.time.minute()
    }
    pub fn second(&self) -> u32 {
        self.time.second()
    }

    pub fn add_hours(&mut self, hours: i64) {
        self.set_time(self.time + chrono::Duration::hours(hours));
    }

    pub fn add_minutes(&mut self, minutes: i64) {
        self.set_time(self.time + chrono::Duration::minutes(minutes));
    }

    pub fn add_seconds(&mut self, seconds: i64) {
        self.set_time(self.time + chrono::Duration::seconds(seconds));
    }

    pub fn set_time_from_string(&mut self, time_str: &str) {
        if let Ok(time) = chrono::NaiveTime::parse_from_str(
            time_str,
            if self.is_24_hour {
                "%H:%M:%S"
            } else {
                "%I:%M:%S %p"
            },
        ) {
            self.set_time(time);
            self.time_edited.emit(time_str.to_string());
        }
    }

    pub fn format_time(&self) -> String {
        self.time
            .format(if self.is_24_hour {
                "%H:%M:%S"
            } else {
                "%I:%M:%S %p"
            })
            .to_string()
    }
}

impl Widget for TimePicker {
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

impl EventHandler for TimePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Date and time picker widget for date and time selection.
pub struct DateTimePicker {
    base: BaseWidget,
    date_time: chrono::NaiveDateTime,
    minimum_date_time: Option<chrono::NaiveDateTime>,
    maximum_date_time: Option<chrono::NaiveDateTime>,
    calendar_popup: bool,
    is_24_hour: bool,
    /// Emitted when the date and time changes.
    pub date_time_changed: Signal1<chrono::NaiveDateTime>,
    /// Emitted when the date changes.
    pub date_changed: Signal1<chrono::NaiveDate>,
    /// Emitted when the time changes.
    pub time_changed: Signal1<chrono::NaiveTime>,
    /// Emitted when the calendar popup is opened.
    pub calendar_opened: GenericSignal,
    /// Emitted when the calendar popup is closed.
    pub calendar_closed: GenericSignal,
}

impl DateTimePicker {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DateTimePicker, geometry, "DateTimePicker"),
            date_time: chrono::Local::now().naive_local(),
            minimum_date_time: None,
            maximum_date_time: None,
            calendar_popup: true,
            is_24_hour: true,
            date_time_changed: Signal1::new(),
            date_changed: Signal1::new(),
            time_changed: Signal1::new(),
            calendar_opened: GenericSignal::new(),
            calendar_closed: GenericSignal::new(),
        }
    }

    pub fn date_time(&self) -> chrono::NaiveDateTime {
        self.date_time
    }
    pub fn minimum_date_time(&self) -> Option<chrono::NaiveDateTime> {
        self.minimum_date_time
    }
    pub fn maximum_date_time(&self) -> Option<chrono::NaiveDateTime> {
        self.maximum_date_time
    }
    pub fn calendar_popup(&self) -> bool {
        self.calendar_popup
    }
    pub fn is_24_hour(&self) -> bool {
        self.is_24_hour
    }

    pub fn set_date_time(&mut self, date_time: chrono::NaiveDateTime) {
        let clamped = self.clamp_date_time(date_time);
        if self.date_time != clamped {
            let old_date = self.date_time.date();
            let old_time = self.date_time.time();
            self.date_time = clamped;
            self.date_time_changed.emit(clamped);
            if old_date != clamped.date() {
                self.date_changed.emit(clamped.date());
            }
            if old_time != clamped.time() {
                self.time_changed.emit(clamped.time());
            }
        }
    }

    pub fn set_minimum_date_time(&mut self, date_time: Option<chrono::NaiveDateTime>) {
        self.minimum_date_time = date_time;
        self.set_date_time(self.date_time);
    }

    pub fn set_maximum_date_time(&mut self, date_time: Option<chrono::NaiveDateTime>) {
        self.maximum_date_time = date_time;
        self.set_date_time(self.date_time);
    }

    pub fn set_calendar_popup(&mut self, enabled: bool) {
        self.calendar_popup = enabled;
    }

    pub fn set_24_hour(&mut self, enabled: bool) {
        self.is_24_hour = enabled;
    }

    fn clamp_date_time(&self, date_time: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
        let mut result = date_time;
        if let Some(min) = self.minimum_date_time {
            if result < min {
                result = min;
            }
        }
        if let Some(max) = self.maximum_date_time {
            if result > max {
                result = max;
            }
        }
        result
    }

    pub fn set_date(&mut self, date: chrono::NaiveDate) {
        let new_date_time = chrono::NaiveDateTime::new(date, self.date_time.time());
        self.set_date_time(new_date_time);
    }

    pub fn set_time(&mut self, time: chrono::NaiveTime) {
        let new_date_time = chrono::NaiveDateTime::new(self.date_time.date(), time);
        self.set_date_time(new_date_time);
    }

    pub fn date(&self) -> chrono::NaiveDate {
        self.date_time.date()
    }
    pub fn time(&self) -> chrono::NaiveTime {
        self.date_time.time()
    }

    pub fn add_days(&mut self, days: i64) {
        self.set_date_time(self.date_time + chrono::Duration::days(days));
    }

    pub fn add_hours(&mut self, hours: i64) {
        self.set_date_time(self.date_time + chrono::Duration::hours(hours));
    }

    pub fn add_minutes(&mut self, minutes: i64) {
        self.set_date_time(self.date_time + chrono::Duration::minutes(minutes));
    }

    pub fn add_seconds(&mut self, seconds: i64) {
        self.set_date_time(self.date_time + chrono::Duration::seconds(seconds));
    }

    pub fn set_date_time_from_ymd_hms(
        &mut self,
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) {
        if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
            if let Some(time) = chrono::NaiveTime::from_hms_opt(hour, minute, second) {
                let date_time = chrono::NaiveDateTime::new(date, time);
                self.set_date_time(date_time);
            }
        }
    }

    pub fn open_calendar(&mut self) {
        self.calendar_opened.emit();
    }

    pub fn close_calendar(&mut self) {
        self.calendar_closed.emit();
    }

    pub fn format_date_time(&self) -> String {
        self.date_time
            .format(if self.is_24_hour {
                "%Y-%m-%d %H:%M:%S"
            } else {
                "%Y-%m-%d %I:%M:%S %p"
            })
            .to_string()
    }
}

impl Widget for DateTimePicker {
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

impl EventHandler for DateTimePicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Directory picker widget for directory selection.
pub struct DirectoryPicker {
    base: BaseWidget,
    directory: String,
    show_hidden: bool,
    /// Emitted when the directory is selected.
    pub directory_selected: Signal1<String>,
    /// Emitted when the directory is changed.
    pub directory_changed: Signal1<String>,
    /// Emitted when the dialog is accepted.
    pub accepted: GenericSignal,
    /// Emitted when the dialog is rejected.
    pub rejected: GenericSignal,
}

impl DirectoryPicker {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DirectoryPicker, geometry, "DirectoryPicker"),
            directory: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("/"))
                .to_string_lossy()
                .to_string(),
            show_hidden: false,
            directory_selected: Signal1::new(),
            directory_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }

    pub fn directory(&self) -> &str {
        &self.directory
    }
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn set_directory(&mut self, directory: String) {
        if self.directory != directory {
            self.directory = directory.clone();
            self.directory_changed.emit(directory);
        }
    }

    pub fn set_show_hidden(&mut self, show: bool) {
        self.show_hidden = show;
    }

    pub fn select_directory(&mut self, directory: String) {
        self.set_directory(directory.clone());
        self.directory_selected.emit(directory);
    }

    pub fn accept(&mut self) {
        self.accepted.emit();
    }

    pub fn reject(&mut self) {
        self.rejected.emit();
    }

    pub fn browse(&mut self) {
        // In a real implementation, this would open a file dialog
        // For now, we'll just emit the signals
    }

    pub fn current_directory(&self) -> String {
        self.directory.clone()
    }

    pub fn set_current_directory(&mut self, directory: String) {
        self.set_directory(directory);
    }
}

impl Widget for DirectoryPicker {
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

impl EventHandler for DirectoryPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Data view widget for data visualization and manipulation.
pub struct DataView {
    base: BaseWidget,
    model: Option<ObjectId>,
    selection_mode: DataViewSelectionMode,
    show_headers: bool,
    alternating_row_colors: bool,
    /// Emitted when the selection changes.
    pub selection_changed: Signal1<Vec<usize>>,
    /// Emitted when an item is activated (double-clicked).
    pub item_activated: Signal1<usize>,
    /// Emitted when a context menu is requested.
    pub context_menu_requested: Signal1<(Point, Option<usize>)>,
    /// Emitted when columns are reordered.
    pub columns_reordered: Signal1<Vec<usize>>,
    /// Emitted when columns are resized.
    pub columns_resized: Signal1<Vec<(usize, u32)>>,
}

/// Data view selection modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataViewSelectionMode {
    /// No selection allowed.
    NoSelection,
    /// Single item selection.
    SingleSelection,
    /// Multiple item selection with keyboard modifiers.
    MultiSelection,
    /// Extended selection with shift key support.
    ExtendedSelection,
}

impl DataView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::DataView, geometry, "DataView"),
            model: None,
            selection_mode: DataViewSelectionMode::SingleSelection,
            show_headers: true,
            alternating_row_colors: false,
            selection_changed: Signal1::new(),
            item_activated: Signal1::new(),
            context_menu_requested: Signal1::new(),
            columns_reordered: Signal1::new(),
            columns_resized: Signal1::new(),
        }
    }

    pub fn model(&self) -> Option<ObjectId> {
        self.model
    }
    pub fn selection_mode(&self) -> DataViewSelectionMode {
        self.selection_mode
    }
    pub fn show_headers(&self) -> bool {
        self.show_headers
    }
    pub fn alternating_row_colors(&self) -> bool {
        self.alternating_row_colors
    }

    pub fn set_model(&mut self, model: Option<ObjectId>) {
        self.model = model;
    }

    pub fn set_selection_mode(&mut self, mode: DataViewSelectionMode) {
        self.selection_mode = mode;
    }

    pub fn set_show_headers(&mut self, show: bool) {
        self.show_headers = show;
    }

    pub fn set_alternating_row_colors(&mut self, enabled: bool) {
        self.alternating_row_colors = enabled;
    }

    pub fn select_item(&mut self, index: usize) {
        // In a real implementation, this would select the item
        // For now, we'll just emit the signal
        self.selection_changed.emit(vec![index]);
    }

    pub fn select_items(&mut self, indices: Vec<usize>) {
        // In a real implementation, this would select multiple items
        // For now, we'll just emit the signal
        self.selection_changed.emit(indices);
    }

    pub fn clear_selection(&mut self) {
        // In a real implementation, this would clear the selection
        // For now, we'll just emit the signal
        self.selection_changed.emit(vec![]);
    }

    pub fn activate_item(&mut self, index: usize) {
        // In a real implementation, this would activate the item
        // For now, we'll just emit the signal
        self.item_activated.emit(index);
    }

    pub fn request_context_menu(&mut self, position: Point, item_index: Option<usize>) {
        // In a real implementation, this would show a context menu
        // For now, we'll just emit the signal
        self.context_menu_requested.emit((position, item_index));
    }

    pub fn reorder_columns(&mut self, new_order: Vec<usize>) {
        // In a real implementation, this would reorder the columns
        // For now, we'll just emit the signal
        self.columns_reordered.emit(new_order);
    }

    pub fn resize_column(&mut self, column_index: usize, new_width: u32) {
        // In a real implementation, this would resize the column
        // For now, we'll just emit the signal
        self.columns_resized.emit(vec![(column_index, new_width)]);
    }
}

impl Widget for DataView {
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

impl EventHandler for DataView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Property grid widget for property editing interface.
pub struct PropertyGrid {
    base: BaseWidget,
    object: Option<ObjectId>,
    show_category_headers: bool,
    auto_expand_all: bool,
    sort_properties: bool,
    /// Emitted when a property value changes.
    pub property_changed: Signal1<(String, String)>,
    /// Emitted when a property is selected.
    pub property_selected: Signal1<String>,
    /// Emitted when the object being edited changes.
    pub object_changed: Signal1<Option<ObjectId>>,
    /// Emitted when a property is double-clicked.
    pub property_activated: Signal1<String>,
}

impl PropertyGrid {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PropertyGrid, geometry, "PropertyGrid"),
            object: None,
            show_category_headers: true,
            auto_expand_all: false,
            sort_properties: true,
            property_changed: Signal1::new(),
            property_selected: Signal1::new(),
            object_changed: Signal1::new(),
            property_activated: Signal1::new(),
        }
    }

    pub fn object(&self) -> Option<ObjectId> {
        self.object
    }
    pub fn show_category_headers(&self) -> bool {
        self.show_category_headers
    }
    pub fn auto_expand_all(&self) -> bool {
        self.auto_expand_all
    }
    pub fn sort_properties(&self) -> bool {
        self.sort_properties
    }

    pub fn set_object(&mut self, object: Option<ObjectId>) {
        if self.object != object {
            self.object = object;
            self.object_changed.emit(object);
        }
    }

    pub fn set_show_category_headers(&mut self, show: bool) {
        self.show_category_headers = show;
    }

    pub fn set_auto_expand_all(&mut self, auto_expand: bool) {
        self.auto_expand_all = auto_expand;
    }

    pub fn set_sort_properties(&mut self, sort: bool) {
        self.sort_properties = sort;
    }

    pub fn set_property_value(&mut self, property_name: &str, value: &str) {
        // In a real implementation, this would set the property value
        // For now, we'll just emit the signal
        self.property_changed
            .emit((property_name.to_string(), value.to_string()));
    }

    pub fn select_property(&mut self, property_name: &str) {
        // In a real implementation, this would select the property
        // For now, we'll just emit the signal
        self.property_selected.emit(property_name.to_string());
    }

    pub fn activate_property(&mut self, property_name: &str) {
        // In a real implementation, this would activate the property
        // For now, we'll just emit the signal
        self.property_activated.emit(property_name.to_string());
    }

    pub fn refresh(&mut self) {
        // In a real implementation, this would refresh the property grid
        // For now, we'll just do nothing
    }

    pub fn clear(&mut self) {
        // In a real implementation, this would clear the property grid
        // For now, we'll just do nothing
    }
}

impl Widget for PropertyGrid {
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

impl EventHandler for PropertyGrid {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Toolbox widget for tool palette.
pub struct Toolbox {
    base: BaseWidget,
    items: Vec<ToolboxItem>,
    current_selection: Option<usize>,
    icon_size: Size,
    show_labels: bool,
    /// Emitted when a tool is selected.
    pub tool_selected: Signal1<usize>,
    /// Emitted when a tool is activated (clicked).
    pub tool_activated: Signal1<usize>,
    /// Emitted when a tool is added.
    pub tool_added: Signal1<usize>,
    /// Emitted when a tool is removed.
    pub tool_removed: Signal1<usize>,
}

/// Toolbox item structure.
pub struct ToolboxItem {
    pub id: String,
    pub name: String,
    pub icon: Option<ObjectId>,
    pub tooltip: String,
    pub enabled: bool,
    pub checked: bool,
}

impl Toolbox {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Toolbox, geometry, "Toolbox"),
            items: Vec::new(),
            current_selection: None,
            icon_size: Size::new(32, 32),
            show_labels: true,
            tool_selected: Signal1::new(),
            tool_activated: Signal1::new(),
            tool_added: Signal1::new(),
            tool_removed: Signal1::new(),
        }
    }

    pub fn items(&self) -> &[ToolboxItem] {
        &self.items
    }
    pub fn current_selection(&self) -> Option<usize> {
        self.current_selection
    }
    pub fn icon_size(&self) -> Size {
        self.icon_size
    }
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    pub fn set_icon_size(&mut self, size: Size) {
        self.icon_size = size;
    }

    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
    }

    pub fn add_item(&mut self, item: ToolboxItem) {
        let index = self.items.len();
        self.items.push(item);
        self.tool_added.emit(index);
    }

    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            if let Some(selection) = self.current_selection {
                if selection >= index {
                    self.current_selection = selection.checked_sub(1);
                }
            }
            self.tool_removed.emit(index);
        }
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
        self.current_selection = None;
    }

    pub fn select_item(&mut self, index: Option<usize>) {
        if self.current_selection != index {
            self.current_selection = index;
            if let Some(idx) = index {
                self.tool_selected.emit(idx);
            }
        }
    }

    pub fn activate_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.tool_activated.emit(index);
        }
    }

    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.enabled = enabled;
        }
    }

    pub fn set_item_checked(&mut self, index: usize, checked: bool) {
        if let Some(item) = self.items.get_mut(index) {
            item.checked = checked;
        }
    }

    pub fn item_at(&self, _point: Point) -> Option<usize> {
        // In a real implementation, this would find the item at the given point
        // For now, we'll just return None
        None
    }
}

impl Widget for Toolbox {
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

impl EventHandler for Toolbox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Stacked widget for stacked notebook interface.
pub struct StackedWidget {
    base: BaseWidget,
    pages: Vec<StackedPage>,
    current_index: Option<usize>,
    /// Emitted when the current page changes.
    pub current_changed: Signal1<Option<usize>>,
    /// Emitted when a page is added.
    pub page_added: Signal1<usize>,
    /// Emitted when a page is removed.
    pub page_removed: Signal1<usize>,
    /// Emitted when a page is shown.
    pub page_shown: Signal1<usize>,
    /// Emitted when a page is hidden.
    pub page_hidden: Signal1<usize>,
}

/// Stacked page structure.
pub struct StackedPage {
    pub widget: ObjectId,
    pub title: String,
    pub tooltip: String,
    pub enabled: bool,
    pub visible: bool,
}

impl StackedWidget {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StackedWidget, geometry, "StackedWidget"),
            pages: Vec::new(),
            current_index: None,
            current_changed: Signal1::new(),
            page_added: Signal1::new(),
            page_removed: Signal1::new(),
            page_shown: Signal1::new(),
            page_hidden: Signal1::new(),
        }
    }

    pub fn pages(&self) -> &[StackedPage] {
        &self.pages
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    pub fn current_page(&self) -> Option<&StackedPage> {
        self.current_index.and_then(|i| self.pages.get(i))
    }

    pub fn add_page(&mut self, page: StackedPage) {
        let index = self.pages.len();
        self.pages.push(page);
        self.page_added.emit(index);
        if self.current_index.is_none() {
            self.set_current_index(Some(index));
        }
    }

    pub fn remove_page(&mut self, index: usize) {
        if index < self.pages.len() {
            self.pages.remove(index);
            if let Some(selection) = self.current_index {
                if selection == index {
                    self.set_current_index(if self.pages.is_empty() {
                        None
                    } else if selection >= self.pages.len() {
                        Some(self.pages.len() - 1)
                    } else {
                        Some(selection)
                    });
                } else if selection > index {
                    self.current_index = Some(selection - 1);
                }
            }
            self.page_removed.emit(index);
        }
    }

    pub fn clear_pages(&mut self) {
        self.pages.clear();
        self.set_current_index(None);
    }

    pub fn set_current_index(&mut self, index: Option<usize>) {
        if self.current_index != index {
            let old_index = self.current_index;
            self.current_index = index;
            self.current_changed.emit(index);
            if let Some(idx) = old_index {
                self.page_hidden.emit(idx);
            }
            if let Some(idx) = index {
                self.page_shown.emit(idx);
            }
        }
    }

    pub fn set_current_page(&mut self, widget: ObjectId) {
        if let Some(index) = self.pages.iter().position(|p| p.widget == widget) {
            self.set_current_index(Some(index));
        }
    }

    pub fn set_page_title(&mut self, index: usize, title: String) {
        if let Some(page) = self.pages.get_mut(index) {
            page.title = title;
        }
    }

    pub fn set_page_tooltip(&mut self, index: usize, tooltip: String) {
        if let Some(page) = self.pages.get_mut(index) {
            page.tooltip = tooltip;
        }
    }

    pub fn set_page_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(page) = self.pages.get_mut(index) {
            page.enabled = enabled;
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn index_of(&self, widget: ObjectId) -> Option<usize> {
        self.pages.iter().position(|p| p.widget == widget)
    }
}

impl Widget for StackedWidget {
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

impl EventHandler for StackedWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Collapsible pane widget for collapsible containers.
pub struct CollapsiblePane {
    base: BaseWidget,
    title: String,
    collapsed: bool,
    animating: bool,
    content: Option<ObjectId>,
    /// Emitted when the pane is collapsed.
    pub collapsed_signal: GenericSignal,
    /// Emitted when the pane is expanded.
    pub expanded: GenericSignal,
    /// Emitted when the collapse/expand animation starts.
    pub animation_started: GenericSignal,
    /// Emitted when the collapse/expand animation finishes.
    pub animation_finished: GenericSignal,
}

impl CollapsiblePane {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CollapsiblePane, geometry, "CollapsiblePane"),
            title: "Collapsible Pane".to_string(),
            collapsed: false,
            animating: false,
            content: None,
            collapsed_signal: GenericSignal::new(),
            expanded: GenericSignal::new(),
            animation_started: GenericSignal::new(),
            animation_finished: GenericSignal::new(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }
    pub fn is_animating(&self) -> bool {
        self.animating
    }
    pub fn content(&self) -> Option<ObjectId> {
        self.content
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn set_content(&mut self, content: Option<ObjectId>) {
        self.content = content;
    }

    pub fn toggle(&mut self) {
        if self.collapsed {
            self.expand();
        } else {
            self.collapse();
        }
    }

    pub fn collapse(&mut self) {
        if !self.collapsed && !self.animating {
            self.animating = true;
            self.animation_started.emit();
            // In a real implementation, this would start the collapse animation
            // For now, we'll just simulate it
            self.collapsed = true;
            self.animating = false;
            self.animation_finished.emit();
            self.collapsed_signal.emit();
        }
    }

    pub fn expand(&mut self) {
        if self.collapsed && !self.animating {
            self.animating = true;
            self.animation_started.emit();
            // In a real implementation, this would start the expand animation
            // For now, we'll just simulate it
            self.collapsed = false;
            self.animating = false;
            self.animation_finished.emit();
            self.expanded.emit();
        }
    }

    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed != collapsed {
            if collapsed {
                self.collapse();
            } else {
                self.expand();
            }
        }
    }
}

impl Widget for CollapsiblePane {
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

impl EventHandler for CollapsiblePane {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web view widget for web browser integration.
pub struct WebView {
    base: BaseWidget,
    url: String,
    loading: bool,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    /// Emitted when the page starts loading.
    pub loading_started: Signal1<String>,
    /// Emitted when the page finishes loading.
    pub loading_finished: Signal1<String>,
    /// Emitted when the title changes.
    pub title_changed: Signal1<String>,
    /// Emitted when the URL changes.
    pub url_changed: Signal1<String>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
    /// Emitted when the navigation state changes.
    pub navigation_state_changed: Signal1<(bool, bool)>,
}

impl WebView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebView, geometry, "WebView"),
            url: "about:blank".to_string(),
            loading: false,
            title: "".to_string(),
            can_go_back: false,
            can_go_forward: false,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }

    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url;
            self.url_changed.emit(self.url.clone());
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            // In a real implementation, this would start loading the URL
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
        }
    }

    pub fn load_url(&mut self, url: &str) {
        self.set_url(url.to_string());
    }

    pub fn load_html(&mut self, _html: &str) {
        // In a real implementation, this would load the HTML
        // For now, we'll just simulate it
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
    }

    pub fn go_back(&mut self) {
        if self.can_go_back {
            // In a real implementation, this would navigate back
            // For now, we'll just simulate it
            self.can_go_back = false;
            self.can_go_forward = true;
            self.navigation_state_changed
                .emit((self.can_go_back, self.can_go_forward));
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // In a real implementation, this would navigate forward
            // For now, we'll just simulate it
            self.can_go_back = true;
            self.can_go_forward = false;
            self.navigation_state_changed
                .emit((self.can_go_back, self.can_go_forward));
        }
    }

    pub fn reload(&mut self) {
        // In a real implementation, this would reload the current page
        // For now, we'll just simulate it
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
    }

    pub fn stop(&mut self) {
        // In a real implementation, this would stop loading
        // For now, we'll just simulate it
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
    }

    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title;
            self.title_changed.emit(self.title.clone());
        }
    }

    pub fn evaluate_javascript(&mut self, _script: &str) -> Option<String> {
        // In a real implementation, this would evaluate the JavaScript
        // For now, we'll just return None
        None
    }
}

impl Widget for WebView {
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

impl EventHandler for WebView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Activity indicator widget for showing busy status.
pub struct ActivityIndicator {
    base: BaseWidget,
    animating: bool,
    minimum_delay: u32,
    color: Color,
    size: u32,
    /// Emitted when the animation starts.
    pub animation_started: GenericSignal,
    /// Emitted when the animation stops.
    pub animation_stopped: GenericSignal,
}

impl ActivityIndicator {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ActivityIndicator, geometry, "ActivityIndicator"),
            animating: false,
            minimum_delay: 0,
            color: Color::rgb(0, 0, 0),
            size: 24,
            animation_started: GenericSignal::new(),
            animation_stopped: GenericSignal::new(),
        }
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }
    pub fn minimum_delay(&self) -> u32 {
        self.minimum_delay
    }
    pub fn color(&self) -> Color {
        self.color
    }
    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn set_animating(&mut self, animating: bool) {
        if self.animating != animating {
            self.animating = animating;
            if animating {
                self.animation_started.emit();
            } else {
                self.animation_stopped.emit();
            }
        }
    }

    pub fn start(&mut self) {
        self.set_animating(true);
    }

    pub fn stop(&mut self) {
        self.set_animating(false);
    }

    pub fn set_minimum_delay(&mut self, delay: u32) {
        self.minimum_delay = delay;
    }

    pub fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }
}

impl Widget for ActivityIndicator {
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

impl EventHandler for ActivityIndicator {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Calendar widget for date selection and display.
pub struct Calendar {
    base: BaseWidget,
    selected_date: chrono::NaiveDate,
    minimum_date: Option<chrono::NaiveDate>,
    maximum_date: Option<chrono::NaiveDate>,
    first_day_of_week: chrono::Weekday,
    grid_visible: bool,
    navigation_visible: bool,
    /// Emitted when the selected date changes.
    pub selection_changed: Signal1<chrono::NaiveDate>,
    /// Emitted when the current month changes.
    pub current_page_changed: Signal1<(i32, u32)>,
    /// Emitted when a date is double-clicked.
    pub date_double_clicked: Signal1<chrono::NaiveDate>,
    /// Emitted when the calendar is activated.
    pub activated: Signal1<chrono::NaiveDate>,
}

impl Calendar {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Calendar, geometry, "Calendar"),
            selected_date: chrono::Local::now().date_naive(),
            minimum_date: None,
            maximum_date: None,
            first_day_of_week: chrono::Weekday::Mon,
            grid_visible: true,
            navigation_visible: true,
            selection_changed: Signal1::new(),
            current_page_changed: Signal1::new(),
            date_double_clicked: Signal1::new(),
            activated: Signal1::new(),
        }
    }

    pub fn selected_date(&self) -> chrono::NaiveDate {
        self.selected_date
    }
    pub fn minimum_date(&self) -> Option<chrono::NaiveDate> {
        self.minimum_date
    }
    pub fn maximum_date(&self) -> Option<chrono::NaiveDate> {
        self.maximum_date
    }
    pub fn first_day_of_week(&self) -> chrono::Weekday {
        self.first_day_of_week
    }
    pub fn is_grid_visible(&self) -> bool {
        self.grid_visible
    }
    pub fn is_navigation_visible(&self) -> bool {
        self.navigation_visible
    }

    pub fn set_selected_date(&mut self, date: chrono::NaiveDate) {
        let clamped = self.clamp_date(date);
        if self.selected_date != clamped {
            self.selected_date = clamped;
            self.selection_changed.emit(clamped);
        }
    }

    pub fn set_minimum_date(&mut self, date: Option<chrono::NaiveDate>) {
        self.minimum_date = date;
        self.set_selected_date(self.selected_date);
    }

    pub fn set_maximum_date(&mut self, date: Option<chrono::NaiveDate>) {
        self.maximum_date = date;
        self.set_selected_date(self.selected_date);
    }

    pub fn set_first_day_of_week(&mut self, day: chrono::Weekday) {
        self.first_day_of_week = day;
    }

    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }

    pub fn set_navigation_visible(&mut self, visible: bool) {
        self.navigation_visible = visible;
    }

    fn clamp_date(&self, date: chrono::NaiveDate) -> chrono::NaiveDate {
        let mut result = date;
        if let Some(min) = self.minimum_date {
            if result < min {
                result = min;
            }
        }
        if let Some(max) = self.maximum_date {
            if result > max {
                result = max;
            }
        }
        result
    }

    pub fn select_today(&mut self) {
        self.set_selected_date(chrono::Local::now().date_naive());
    }

    pub fn show_month(&mut self, year: i32, month: u32) {
        // In a real implementation, this would show the specified month
        // For now, we'll just emit the signal
        self.current_page_changed.emit((year, month));
    }

    pub fn show_prev_month(&mut self) {
        // In a real implementation, this would show the previous month
        // For now, we'll just emit the signal
        let year = self.selected_date.year();
        let month = self.selected_date.month();
        let prev_month = if month == 1 {
            (year - 1, 12)
        } else {
            (year, month - 1)
        };
        self.current_page_changed.emit(prev_month);
    }

    pub fn show_next_month(&mut self) {
        // In a real implementation, this would show the next month
        // For now, we'll just emit the signal
        let year = self.selected_date.year();
        let month = self.selected_date.month();
        let next_month = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };
        self.current_page_changed.emit(next_month);
    }

    pub fn show_prev_year(&mut self) {
        // In a real implementation, this would show the previous year
        // For now, we'll just emit the signal
        let year = self.selected_date.year();
        let month = self.selected_date.month();
        self.current_page_changed.emit((year - 1, month));
    }

    pub fn show_next_year(&mut self) {
        // In a real implementation, this would show the next year
        // For now, we'll just emit the signal
        let year = self.selected_date.year();
        let month = self.selected_date.month();
        self.current_page_changed.emit((year + 1, month));
    }

    pub fn double_click_date(&mut self, date: chrono::NaiveDate) {
        // In a real implementation, this would handle double-click on a date
        // For now, we'll just emit the signal
        self.date_double_clicked.emit(date);
        self.activated.emit(date);
    }
}

impl Widget for Calendar {
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

impl EventHandler for Calendar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Column view widget for hierarchical data display.
pub struct ColumnView {
    base: BaseWidget,
    columns: Vec<ColumnViewColumn>,
    root_item: Option<ObjectId>,
    selection_mode: ColumnViewSelectionMode,
    /// Emitted when the selection changes.
    pub selection_changed: Signal1<Vec<ObjectId>>,
    /// Emitted when an item is activated (double-clicked).
    pub item_activated: Signal1<ObjectId>,
    /// Emitted when a column is added.
    pub column_added: Signal1<usize>,
    /// Emitted when a column is removed.
    pub column_removed: Signal1<usize>,
    /// Emitted when a column is resized.
    pub column_resized: Signal1<(usize, u32)>,
    /// Emitted when a column is reordered.
    pub column_reordered: Signal1<(usize, usize)>,
}

/// Column view selection modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnViewSelectionMode {
    /// No selection allowed.
    NoSelection,
    /// Single item selection.
    SingleSelection,
    /// Multiple item selection with keyboard modifiers.
    MultiSelection,
    /// Extended selection with shift key support.
    ExtendedSelection,
}

/// Column view column structure.
pub struct ColumnViewColumn {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub resizable: bool,
    pub visible: bool,
}

impl ColumnView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColumnView, geometry, "ColumnView"),
            columns: Vec::new(),
            root_item: None,
            selection_mode: ColumnViewSelectionMode::SingleSelection,
            selection_changed: Signal1::new(),
            item_activated: Signal1::new(),
            column_added: Signal1::new(),
            column_removed: Signal1::new(),
            column_resized: Signal1::new(),
            column_reordered: Signal1::new(),
        }
    }

    pub fn columns(&self) -> &[ColumnViewColumn] {
        &self.columns
    }
    pub fn root_item(&self) -> Option<ObjectId> {
        self.root_item
    }
    pub fn selection_mode(&self) -> ColumnViewSelectionMode {
        self.selection_mode
    }

    pub fn set_root_item(&mut self, item: Option<ObjectId>) {
        self.root_item = item;
    }

    pub fn set_selection_mode(&mut self, mode: ColumnViewSelectionMode) {
        self.selection_mode = mode;
    }

    pub fn add_column(&mut self, column: ColumnViewColumn) {
        let index = self.columns.len();
        self.columns.push(column);
        self.column_added.emit(index);
    }

    pub fn remove_column(&mut self, index: usize) {
        if index < self.columns.len() {
            self.columns.remove(index);
            self.column_removed.emit(index);
        }
    }

    pub fn clear_columns(&mut self) {
        self.columns.clear();
    }

    pub fn resize_column(&mut self, index: usize, width: u32) {
        if let Some(column) = self.columns.get_mut(index) {
            column.width = width;
            self.column_resized.emit((index, width));
        }
    }

    pub fn reorder_column(&mut self, from_index: usize, to_index: usize) {
        if from_index < self.columns.len() && to_index < self.columns.len() {
            let column = self.columns.remove(from_index);
            self.columns.insert(to_index, column);
            self.column_reordered.emit((from_index, to_index));
        }
    }

    pub fn select_item(&mut self, item: ObjectId) {
        // In a real implementation, this would select the item
        // For now, we'll just emit the signal
        self.selection_changed.emit(vec![item]);
    }

    pub fn select_items(&mut self, items: Vec<ObjectId>) {
        // In a real implementation, this would select multiple items
        // For now, we'll just emit the signal
        self.selection_changed.emit(items);
    }

    pub fn clear_selection(&mut self) {
        // In a real implementation, this would clear the selection
        // For now, we'll just emit the signal
        self.selection_changed.emit(vec![]);
    }

    pub fn activate_item(&mut self, item: ObjectId) {
        // In a real implementation, this would activate the item
        // For now, we'll just emit the signal
        self.item_activated.emit(item);
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    pub fn column_at(&self, index: usize) -> Option<&ColumnViewColumn> {
        self.columns.get(index)
    }
}

impl Widget for ColumnView {
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

impl EventHandler for ColumnView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Undo view widget for undo/redo history display.
pub struct UndoView {
    base: BaseWidget,
    stack: Vec<UndoCommand>,
    current_index: usize,
    clean_index: usize,
    max_stack_size: usize,
    /// Emitted when the undo stack changes.
    pub stack_changed: GenericSignal,
    /// Emitted when an item is activated (double-clicked).
    pub item_activated: Signal1<usize>,
    /// Emitted when the clean state changes.
    pub clean_changed: Signal1<bool>,
}

/// Undo command structure.
pub struct UndoCommand {
    pub text: String,
    pub timestamp: chrono::NaiveDateTime,
    pub merged: bool,
}

impl UndoView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::UndoView, geometry, "UndoView"),
            stack: Vec::new(),
            current_index: 0,
            clean_index: 0,
            max_stack_size: 100,
            stack_changed: GenericSignal::new(),
            item_activated: Signal1::new(),
            clean_changed: Signal1::new(),
        }
    }

    pub fn stack(&self) -> &[UndoCommand] {
        &self.stack
    }
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    pub fn clean_index(&self) -> usize {
        self.clean_index
    }
    pub fn max_stack_size(&self) -> usize {
        self.max_stack_size
    }
    pub fn can_undo(&self) -> bool {
        self.current_index > 0
    }
    pub fn can_redo(&self) -> bool {
        self.current_index < self.stack.len()
    }
    pub fn is_clean(&self) -> bool {
        self.current_index == self.clean_index
    }

    pub fn set_max_stack_size(&mut self, size: usize) {
        self.max_stack_size = size;
        self.trim_stack();
    }

    pub fn add_command(&mut self, command: UndoCommand) {
        // Remove any commands after the current index
        self.stack.truncate(self.current_index);

        // Add the new command
        self.stack.push(command);
        self.current_index = self.stack.len();

        // Trim the stack if it's too big
        self.trim_stack();

        // Emit signals
        self.stack_changed.emit();
        self.clean_changed.emit(self.is_clean());
    }

    pub fn undo(&mut self) -> Option<&UndoCommand> {
        if self.can_undo() {
            self.current_index -= 1;
            self.stack_changed.emit();
            self.clean_changed.emit(self.is_clean());
            self.stack.get(self.current_index)
        } else {
            None
        }
    }

    pub fn redo(&mut self) -> Option<&UndoCommand> {
        if self.can_redo() {
            self.current_index += 1;
            self.stack_changed.emit();
            self.clean_changed.emit(self.is_clean());
            self.stack.get(self.current_index - 1)
        } else {
            None
        }
    }

    pub fn set_clean(&mut self) {
        self.clean_index = self.current_index;
        self.clean_changed.emit(true);
    }

    pub fn clear(&mut self) {
        self.stack.clear();
        self.current_index = 0;
        self.clean_index = 0;
        self.stack_changed.emit();
        self.clean_changed.emit(true);
    }

    pub fn activate_item(&mut self, index: usize) {
        if index < self.stack.len() {
            self.item_activated.emit(index);
            // In a real implementation, this would navigate to the specified index
            // For now, we'll just emit the signal
        }
    }

    fn trim_stack(&mut self) {
        if self.stack.len() > self.max_stack_size {
            let remove_count = self.stack.len() - self.max_stack_size;
            self.stack.drain(0..remove_count);
            self.current_index = self.current_index.saturating_sub(remove_count);
            self.clean_index = self.clean_index.saturating_sub(remove_count);
            self.stack_changed.emit();
        }
    }
}

impl Widget for UndoView {
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

impl EventHandler for UndoView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine view widget for web content rendering.
pub struct WebEngineView {
    base: BaseWidget,
    url: String,
    loading: bool,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
    javascript_enabled: bool,
    plugins_enabled: bool,
    private_browsing: bool,
    /// Emitted when the page starts loading.
    pub loading_started: Signal1<String>,
    /// Emitted when the page finishes loading.
    pub loading_finished: Signal1<String>,
    /// Emitted when the title changes.
    pub title_changed: Signal1<String>,
    /// Emitted when the URL changes.
    pub url_changed: Signal1<String>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
    /// Emitted when the navigation state changes.
    pub navigation_state_changed: Signal1<(bool, bool)>,
    /// Emitted when a certificate error occurs.
    pub certificate_error: Signal1<String>,
    /// Emitted when a JavaScript console message is received.
    pub console_message: Signal1<(String, u32, String)>,
    /// Emitted when a download is requested.
    pub download_requested: Signal1<String>,
    /// Emitted when the page is created.
    pub page_created: Signal1<ObjectId>,
    /// Emitted when the page is destroyed.
    pub page_destroyed: Signal1<ObjectId>,
}

impl WebEngineView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "WebEngineView"),
            url: "".to_string(),
            loading: false,
            title: "".to_string(),
            can_go_back: false,
            can_go_forward: false,
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            certificate_error: Signal1::new(),
            console_message: Signal1::new(),
            download_requested: Signal1::new(),
            page_created: Signal1::new(),
            page_destroyed: Signal1::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }

    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url;
            self.url_changed.emit(self.url.clone());
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            // In a real implementation, this would start loading the URL
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
        }
    }

    pub fn load_html(&mut self, _html: &str) {
        // In a real implementation, this would load the HTML
        // For now, we'll just simulate it
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
    }

    pub fn load_data(&mut self, _data: &[u8], _mime_type: &str, _encoding: &str, base_url: &str) {
        // In a real implementation, this would load the data
        // For now, we'll just simulate it
        self.url = base_url.to_string();
        self.title = "Data Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
    }

    pub fn go_back(&mut self) {
        if self.can_go_back {
            // In a real implementation, this would navigate back
            // For now, we'll just simulate it
            self.can_go_back = false;
            self.can_go_forward = true;
            self.update_navigation_state();
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward {
            // In a real implementation, this would navigate forward
            // For now, we'll just simulate it
            self.can_go_forward = false;
            self.can_go_back = true;
            self.update_navigation_state();
        }
    }

    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            // In a real implementation, this would reload the page
            // For now, we'll just simulate it
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
        }
    }

    pub fn stop(&mut self) {
        if self.loading {
            // In a real implementation, this would stop loading
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
        }
    }

    pub fn evaluate_javascript(&mut self, _script: &str) -> Result<String, String> {
        // In a real implementation, this would evaluate the JavaScript
        // For now, we'll just return a placeholder
        Ok("Result".to_string())
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.javascript_enabled = enabled;
    }

    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.plugins_enabled = enabled;
    }

    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.private_browsing = enabled;
    }

    pub fn clear_history(&mut self) {
        // In a real implementation, this would clear the history
        // For now, we'll just update the navigation state
        self.can_go_back = false;
        self.can_go_forward = false;
        self.update_navigation_state();
    }

    pub fn clear_cache(&mut self) {
        // In a real implementation, this would clear the cache
        // For now, we'll just do nothing
    }

    pub fn clear_cookies(&mut self) {
        // In a real implementation, this would clear the cookies
        // For now, we'll just do nothing
    }

    pub fn zoom_in(&mut self) {
        // In a real implementation, this would zoom in
        // For now, we'll just do nothing
    }

    pub fn zoom_out(&mut self) {
        // In a real implementation, this would zoom out
        // For now, we'll just do nothing
    }

    pub fn reset_zoom(&mut self) {
        // In a real implementation, this would reset the zoom
        // For now, we'll just do nothing
    }

    pub fn print(&mut self) {
        // In a real implementation, this would print the page
        // For now, we'll just do nothing
    }

    pub fn save_page(&mut self, _path: &str, _format: SaveFormat) -> Result<(), String> {
        // In a real implementation, this would save the page
        // For now, we'll just return a placeholder
        Ok(())
    }

    fn update_navigation_state(&mut self) {
        self.navigation_state_changed
            .emit((self.can_go_back, self.can_go_forward));
    }
}

/// Save format for web pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// Save as HTML only.
    HtmlOnly,
    /// Save as complete HTML with resources.
    CompleteHtml,
    /// Save as MHTML.
    MHtml,
}

impl Widget for WebEngineView {
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

impl EventHandler for WebEngineView {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine page widget for web content management.
pub struct WebEnginePage {
    base: BaseWidget,
    url: String,
    title: String,
    favicon: Option<Image>,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    javascript_enabled: bool,
    plugins_enabled: bool,
    private_browsing: bool,
    /// Emitted when the page starts loading.
    pub loading_started: Signal1<String>,
    /// Emitted when the page finishes loading.
    pub loading_finished: Signal1<String>,
    /// Emitted when the title changes.
    pub title_changed: Signal1<String>,
    /// Emitted when the URL changes.
    pub url_changed: Signal1<String>,
    /// Emitted when the favicon changes.
    pub favicon_changed: Signal1<Option<Image>>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
    /// Emitted when the navigation state changes.
    pub navigation_state_changed: Signal1<(bool, bool)>,
    /// Emitted when a certificate error occurs.
    pub certificate_error: Signal1<String>,
    /// Emitted when a JavaScript console message is received.
    pub console_message: Signal1<(String, u32, String)>,
    /// Emitted when a download is requested.
    pub download_requested: Signal1<String>,
    /// Emitted when a form is submitted.
    pub form_submitted: Signal1<String>,
    /// Emitted when a link is clicked.
    pub link_clicked: Signal1<String>,
    /// Emitted when a new window is requested.
    pub new_window_requested: Signal1<String>,
    /// Emitted when a popup window is requested.
    pub popup_window_requested: Signal1<String>,
}

impl WebEnginePage {
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEnginePage,
                Rect::new(0, 0, 0, 0),
                "WebEnginePage",
            ),
            url: "".to_string(),
            title: "".to_string(),
            favicon: None,
            loading: false,
            can_go_back: false,
            can_go_forward: false,
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            loading_started: Signal1::new(),
            loading_finished: Signal1::new(),
            title_changed: Signal1::new(),
            url_changed: Signal1::new(),
            favicon_changed: Signal1::new(),
            error_occurred: Signal1::new(),
            navigation_state_changed: Signal1::new(),
            certificate_error: Signal1::new(),
            console_message: Signal1::new(),
            download_requested: Signal1::new(),
            form_submitted: Signal1::new(),
            link_clicked: Signal1::new(),
            new_window_requested: Signal1::new(),
            popup_window_requested: Signal1::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn favicon(&self) -> Option<&Image> {
        self.favicon.as_ref()
    }
    pub fn is_loading(&self) -> bool {
        self.loading
    }
    pub fn can_go_back(&self) -> bool {
        self.can_go_back
    }
    pub fn can_go_forward(&self) -> bool {
        self.can_go_forward
    }
    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }

    pub fn set_url(&mut self, url: String) {
        if self.url != url {
            self.url = url;
            self.url_changed.emit(self.url.clone());
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            // In a real implementation, this would start loading the URL
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
            self.update_navigation_state();
        }
    }

    pub fn load_html(&mut self, _html: &str) {
        // In a real implementation, this would load the HTML
        // For now, we'll just simulate it
        self.url = "data:text/html".to_string();
        self.title = "HTML Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
    }

    pub fn load_data(&mut self, _data: &[u8], _mime_type: &str, _encoding: &str, base_url: &str) {
        // In a real implementation, this would load the data
        // For now, we'll just simulate it
        self.url = base_url.to_string();
        self.title = "Data Content".to_string();
        self.loading = true;
        self.loading_started.emit(self.url.clone());
        self.loading = false;
        self.loading_finished.emit(self.url.clone());
        self.title_changed.emit(self.title.clone());
        self.url_changed.emit(self.url.clone());
        self.update_navigation_state();
    }

    pub fn go_back(&mut self) -> bool {
        if self.can_go_back {
            // In a real implementation, this would navigate back
            // For now, we'll just simulate it
            self.can_go_back = false;
            self.can_go_forward = true;
            self.update_navigation_state();
            true
        } else {
            false
        }
    }

    pub fn go_forward(&mut self) -> bool {
        if self.can_go_forward {
            // In a real implementation, this would navigate forward
            // For now, we'll just simulate it
            self.can_go_forward = false;
            self.can_go_back = true;
            self.update_navigation_state();
            true
        } else {
            false
        }
    }

    pub fn reload(&mut self) {
        if !self.url.is_empty() {
            // In a real implementation, this would reload the page
            // For now, we'll just simulate it
            self.loading = true;
            self.loading_started.emit(self.url.clone());
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
        }
    }

    pub fn stop(&mut self) {
        if self.loading {
            // In a real implementation, this would stop loading
            // For now, we'll just simulate it
            self.loading = false;
            self.loading_finished.emit(self.url.clone());
        }
    }

    pub fn evaluate_javascript(&mut self, _script: &str) -> Result<String, String> {
        // In a real implementation, this would evaluate the JavaScript
        // For now, we'll just return a placeholder
        Ok("Result".to_string())
    }

    pub fn run_javascript(&mut self, _script: &str) {
        // In a real implementation, this would run the JavaScript
        // For now, we'll just do nothing
    }

    pub fn run_javascript_with_callback(
        &mut self,
        _script: &str,
        callback: impl FnOnce(Result<String, String>),
    ) {
        // In a real implementation, this would run the JavaScript with a callback
        // For now, we'll just call the callback with a placeholder
        callback(Ok("Result".to_string()));
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        self.javascript_enabled = enabled;
    }

    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        self.plugins_enabled = enabled;
    }

    pub fn set_private_browsing(&mut self, enabled: bool) {
        self.private_browsing = enabled;
    }

    pub fn set_favicon(&mut self, favicon: Option<Image>) {
        if self.favicon != favicon {
            self.favicon = favicon;
            self.favicon_changed.emit(self.favicon.clone());
        }
    }

    pub fn set_title(&mut self, title: String) {
        if self.title != title {
            self.title = title;
            self.title_changed.emit(self.title.clone());
        }
    }

    pub fn trigger_form_submitted(&mut self, url: &str) {
        self.form_submitted.emit(url.to_string());
    }

    pub fn trigger_link_clicked(&mut self, url: &str) {
        self.link_clicked.emit(url.to_string());
    }

    pub fn trigger_new_window_requested(&mut self, url: &str) {
        self.new_window_requested.emit(url.to_string());
    }

    pub fn trigger_popup_window_requested(&mut self, url: &str) {
        self.popup_window_requested.emit(url.to_string());
    }

    pub fn trigger_certificate_error(&mut self, error: &str) {
        self.certificate_error.emit(error.to_string());
    }

    pub fn trigger_console_message(&mut self, message: &str, line: u32, source: &str) {
        self.console_message
            .emit((message.to_string(), line, source.to_string()));
    }

    pub fn trigger_download_requested(&mut self, url: &str) {
        self.download_requested.emit(url.to_string());
    }

    fn update_navigation_state(&mut self) {
        self.navigation_state_changed
            .emit((self.can_go_back, self.can_go_forward));
    }
}

impl Default for WebEnginePage {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for WebEnginePage {
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

impl EventHandler for WebEnginePage {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine settings widget for web engine configuration.
pub struct WebEngineSettings {
    base: BaseWidget,
    javascript_enabled: bool,
    plugins_enabled: bool,
    private_browsing: bool,
    local_storage_enabled: bool,
    session_storage_enabled: bool,
    cookies_enabled: bool,
    images_enabled: bool,
    javascript_can_open_windows_automatically: bool,
    javascript_can_access_clipboard: bool,
    webgl_enabled: bool,
    webrtc_enabled: bool,
    pdf_viewer_enabled: bool,
    auto_load_images: bool,
    auto_play_media: bool,
    user_agent: String,
    default_font_family: String,
    default_font_size: u32,
    minimum_font_size: u32,
    /// Emitted when a setting changes.
    pub setting_changed: Signal1<(String, String)>,
}

impl WebEngineSettings {
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineSettings, Rect::new(0, 0, 0, 0), "WebEngineSettings"),
            javascript_enabled: true,
            plugins_enabled: false,
            private_browsing: false,
            local_storage_enabled: true,
            session_storage_enabled: true,
            cookies_enabled: true,
            images_enabled: true,
            javascript_can_open_windows_automatically: false,
            javascript_can_access_clipboard: false,
            webgl_enabled: true,
            webrtc_enabled: true,
            pdf_viewer_enabled: true,
            auto_load_images: true,
            auto_play_media: false,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string(),
            default_font_family: "Arial".to_string(),
            default_font_size: 16,
            minimum_font_size: 8,
            setting_changed: Signal1::new(),
        }
    }

    pub fn is_javascript_enabled(&self) -> bool {
        self.javascript_enabled
    }
    pub fn is_plugins_enabled(&self) -> bool {
        self.plugins_enabled
    }
    pub fn is_private_browsing(&self) -> bool {
        self.private_browsing
    }
    pub fn is_local_storage_enabled(&self) -> bool {
        self.local_storage_enabled
    }
    pub fn is_session_storage_enabled(&self) -> bool {
        self.session_storage_enabled
    }
    pub fn is_cookies_enabled(&self) -> bool {
        self.cookies_enabled
    }
    pub fn is_images_enabled(&self) -> bool {
        self.images_enabled
    }
    pub fn is_javascript_can_open_windows_automatically(&self) -> bool {
        self.javascript_can_open_windows_automatically
    }
    pub fn is_javascript_can_access_clipboard(&self) -> bool {
        self.javascript_can_access_clipboard
    }
    pub fn is_webgl_enabled(&self) -> bool {
        self.webgl_enabled
    }
    pub fn is_webrtc_enabled(&self) -> bool {
        self.webrtc_enabled
    }
    pub fn is_pdf_viewer_enabled(&self) -> bool {
        self.pdf_viewer_enabled
    }
    pub fn is_auto_load_images(&self) -> bool {
        self.auto_load_images
    }
    pub fn is_auto_play_media(&self) -> bool {
        self.auto_play_media
    }
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }
    pub fn default_font_family(&self) -> &str {
        &self.default_font_family
    }
    pub fn default_font_size(&self) -> u32 {
        self.default_font_size
    }
    pub fn minimum_font_size(&self) -> u32 {
        self.minimum_font_size
    }

    pub fn set_javascript_enabled(&mut self, enabled: bool) {
        if self.javascript_enabled != enabled {
            self.javascript_enabled = enabled;
            self.setting_changed
                .emit(("javascript_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_plugins_enabled(&mut self, enabled: bool) {
        if self.plugins_enabled != enabled {
            self.plugins_enabled = enabled;
            self.setting_changed
                .emit(("plugins_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_private_browsing(&mut self, enabled: bool) {
        if self.private_browsing != enabled {
            self.private_browsing = enabled;
            self.setting_changed
                .emit(("private_browsing".to_string(), enabled.to_string()));
        }
    }

    pub fn set_local_storage_enabled(&mut self, enabled: bool) {
        if self.local_storage_enabled != enabled {
            self.local_storage_enabled = enabled;
            self.setting_changed
                .emit(("local_storage_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_session_storage_enabled(&mut self, enabled: bool) {
        if self.session_storage_enabled != enabled {
            self.session_storage_enabled = enabled;
            self.setting_changed
                .emit(("session_storage_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_cookies_enabled(&mut self, enabled: bool) {
        if self.cookies_enabled != enabled {
            self.cookies_enabled = enabled;
            self.setting_changed
                .emit(("cookies_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_images_enabled(&mut self, enabled: bool) {
        if self.images_enabled != enabled {
            self.images_enabled = enabled;
            self.setting_changed
                .emit(("images_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_javascript_can_open_windows_automatically(&mut self, enabled: bool) {
        if self.javascript_can_open_windows_automatically != enabled {
            self.javascript_can_open_windows_automatically = enabled;
            self.setting_changed.emit((
                "javascript_can_open_windows_automatically".to_string(),
                enabled.to_string(),
            ));
        }
    }

    pub fn set_javascript_can_access_clipboard(&mut self, enabled: bool) {
        if self.javascript_can_access_clipboard != enabled {
            self.javascript_can_access_clipboard = enabled;
            self.setting_changed.emit((
                "javascript_can_access_clipboard".to_string(),
                enabled.to_string(),
            ));
        }
    }

    pub fn set_webgl_enabled(&mut self, enabled: bool) {
        if self.webgl_enabled != enabled {
            self.webgl_enabled = enabled;
            self.setting_changed
                .emit(("webgl_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_webrtc_enabled(&mut self, enabled: bool) {
        if self.webrtc_enabled != enabled {
            self.webrtc_enabled = enabled;
            self.setting_changed
                .emit(("webrtc_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_pdf_viewer_enabled(&mut self, enabled: bool) {
        if self.pdf_viewer_enabled != enabled {
            self.pdf_viewer_enabled = enabled;
            self.setting_changed
                .emit(("pdf_viewer_enabled".to_string(), enabled.to_string()));
        }
    }

    pub fn set_auto_load_images(&mut self, enabled: bool) {
        if self.auto_load_images != enabled {
            self.auto_load_images = enabled;
            self.setting_changed
                .emit(("auto_load_images".to_string(), enabled.to_string()));
        }
    }

    pub fn set_auto_play_media(&mut self, enabled: bool) {
        if self.auto_play_media != enabled {
            self.auto_play_media = enabled;
            self.setting_changed
                .emit(("auto_play_media".to_string(), enabled.to_string()));
        }
    }

    pub fn set_user_agent(&mut self, user_agent: String) {
        if self.user_agent != user_agent {
            self.user_agent = user_agent;
            self.setting_changed
                .emit(("user_agent".to_string(), self.user_agent.clone()));
        }
    }

    pub fn set_default_font_family(&mut self, font_family: String) {
        if self.default_font_family != font_family {
            self.default_font_family = font_family;
            self.setting_changed.emit((
                "default_font_family".to_string(),
                self.default_font_family.clone(),
            ));
        }
    }

    pub fn set_default_font_size(&mut self, font_size: u32) {
        if self.default_font_size != font_size {
            self.default_font_size = font_size;
            self.setting_changed
                .emit(("default_font_size".to_string(), font_size.to_string()));
        }
    }

    pub fn set_minimum_font_size(&mut self, font_size: u32) {
        if self.minimum_font_size != font_size {
            self.minimum_font_size = font_size;
            self.setting_changed
                .emit(("minimum_font_size".to_string(), font_size.to_string()));
        }
    }

    pub fn reset(&mut self) {
        self.javascript_enabled = true;
        self.plugins_enabled = false;
        self.private_browsing = false;
        self.local_storage_enabled = true;
        self.session_storage_enabled = true;
        self.cookies_enabled = true;
        self.images_enabled = true;
        self.javascript_can_open_windows_automatically = false;
        self.javascript_can_access_clipboard = false;
        self.webgl_enabled = true;
        self.webrtc_enabled = true;
        self.pdf_viewer_enabled = true;
        self.auto_load_images = true;
        self.auto_play_media = false;
        self.user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string();
        self.default_font_family = "Arial".to_string();
        self.default_font_size = 16;
        self.minimum_font_size = 8;
        self.setting_changed
            .emit(("reset".to_string(), "all".to_string()));
    }

    pub fn get_setting(&self, name: &str) -> Option<String> {
        match name {
            "javascript_enabled" => Some(self.javascript_enabled.to_string()),
            "plugins_enabled" => Some(self.plugins_enabled.to_string()),
            "private_browsing" => Some(self.private_browsing.to_string()),
            "local_storage_enabled" => Some(self.local_storage_enabled.to_string()),
            "session_storage_enabled" => Some(self.session_storage_enabled.to_string()),
            "cookies_enabled" => Some(self.cookies_enabled.to_string()),
            "images_enabled" => Some(self.images_enabled.to_string()),
            "javascript_can_open_windows_automatically" => {
                Some(self.javascript_can_open_windows_automatically.to_string())
            }
            "javascript_can_access_clipboard" => {
                Some(self.javascript_can_access_clipboard.to_string())
            }
            "webgl_enabled" => Some(self.webgl_enabled.to_string()),
            "webrtc_enabled" => Some(self.webrtc_enabled.to_string()),
            "pdf_viewer_enabled" => Some(self.pdf_viewer_enabled.to_string()),
            "auto_load_images" => Some(self.auto_load_images.to_string()),
            "auto_play_media" => Some(self.auto_play_media.to_string()),
            "user_agent" => Some(self.user_agent.clone()),
            "default_font_family" => Some(self.default_font_family.clone()),
            "default_font_size" => Some(self.default_font_size.to_string()),
            "minimum_font_size" => Some(self.minimum_font_size.to_string()),
            _ => None,
        }
    }

    pub fn set_setting(&mut self, name: &str, value: &str) -> bool {
        match name {
            "javascript_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_javascript_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "plugins_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_plugins_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "private_browsing" => {
                if let Ok(enabled) = value.parse() {
                    self.set_private_browsing(enabled);
                    true
                } else {
                    false
                }
            }
            "local_storage_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_local_storage_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "session_storage_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_session_storage_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "cookies_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_cookies_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "images_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_images_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "javascript_can_open_windows_automatically" => {
                if let Ok(enabled) = value.parse() {
                    self.set_javascript_can_open_windows_automatically(enabled);
                    true
                } else {
                    false
                }
            }
            "javascript_can_access_clipboard" => {
                if let Ok(enabled) = value.parse() {
                    self.set_javascript_can_access_clipboard(enabled);
                    true
                } else {
                    false
                }
            }
            "webgl_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_webgl_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "webrtc_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_webrtc_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "pdf_viewer_enabled" => {
                if let Ok(enabled) = value.parse() {
                    self.set_pdf_viewer_enabled(enabled);
                    true
                } else {
                    false
                }
            }
            "auto_load_images" => {
                if let Ok(enabled) = value.parse() {
                    self.set_auto_load_images(enabled);
                    true
                } else {
                    false
                }
            }
            "auto_play_media" => {
                if let Ok(enabled) = value.parse() {
                    self.set_auto_play_media(enabled);
                    true
                } else {
                    false
                }
            }
            "user_agent" => {
                self.set_user_agent(value.to_string());
                true
            }
            "default_font_family" => {
                self.set_default_font_family(value.to_string());
                true
            }
            "default_font_size" => {
                if let Ok(font_size) = value.parse() {
                    self.set_default_font_size(font_size);
                    true
                } else {
                    false
                }
            }
            "minimum_font_size" => {
                if let Ok(font_size) = value.parse() {
                    self.set_minimum_font_size(font_size);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

impl Default for WebEngineSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for WebEngineSettings {
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

impl EventHandler for WebEngineSettings {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine download item widget for download management.
pub struct WebEngineDownloadItem {
    base: BaseWidget,
    url: String,
    filename: String,
    total_bytes: u64,
    received_bytes: u64,
    state: DownloadState,
    speed: u64, // bytes per second
    error: Option<String>,
    /// Emitted when the download state changes.
    pub state_changed: Signal1<DownloadState>,
    /// Emitted when the download progress changes.
    pub progress_changed: Signal1<(u64, u64)>,
    /// Emitted when the download speed changes.
    pub speed_changed: Signal1<u64>,
    /// Emitted when the download filename changes.
    pub filename_changed: Signal1<String>,
    /// Emitted when the download is finished.
    pub finished: Signal1<()>,
    /// Emitted when the download is canceled.
    pub canceled: Signal1<()>,
    /// Emitted when an error occurs.
    pub error_occurred: Signal1<String>,
}

/// Download state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// The download has not started yet.
    Idle,
    /// The download is in progress.
    Downloading,
    /// The download is paused.
    Paused,
    /// The download is completed.
    Completed,
    /// The download is canceled.
    Canceled,
    /// The download failed with an error.
    Failed,
}

impl WebEngineDownloadItem {
    pub fn new(url: String, filename: String) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineDownloadItem,
                Rect::new(0, 0, 0, 0),
                "WebEngineDownloadItem",
            ),
            url,
            filename,
            total_bytes: 0,
            received_bytes: 0,
            state: DownloadState::Idle,
            speed: 0,
            error: None,
            state_changed: Signal1::new(),
            progress_changed: Signal1::new(),
            speed_changed: Signal1::new(),
            filename_changed: Signal1::new(),
            finished: Signal1::new(),
            canceled: Signal1::new(),
            error_occurred: Signal1::new(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn filename(&self) -> &str {
        &self.filename
    }
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    pub fn received_bytes(&self) -> u64 {
        self.received_bytes
    }
    pub fn state(&self) -> DownloadState {
        self.state
    }
    pub fn speed(&self) -> u64 {
        self.speed
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn progress(&self) -> f64 {
        if self.total_bytes > 0 {
            self.received_bytes as f64 / self.total_bytes as f64
        } else {
            0.0
        }
    }

    pub fn start(&mut self) {
        if self.state == DownloadState::Idle || self.state == DownloadState::Paused {
            self.state = DownloadState::Downloading;
            self.state_changed.emit(self.state);
        }
    }

    pub fn pause(&mut self) {
        if self.state == DownloadState::Downloading {
            self.state = DownloadState::Paused;
            self.state_changed.emit(self.state);
        }
    }

    pub fn resume(&mut self) {
        if self.state == DownloadState::Paused {
            self.state = DownloadState::Downloading;
            self.state_changed.emit(self.state);
        }
    }

    pub fn cancel(&mut self) {
        if self.state == DownloadState::Downloading || self.state == DownloadState::Paused {
            self.state = DownloadState::Canceled;
            self.state_changed.emit(self.state);
            self.canceled.emit(());
        }
    }

    pub fn set_filename(&mut self, filename: String) {
        if self.filename != filename {
            self.filename = filename;
            self.filename_changed.emit(self.filename.clone());
        }
    }

    pub fn set_total_bytes(&mut self, total_bytes: u64) {
        self.total_bytes = total_bytes;
        self.progress_changed
            .emit((self.received_bytes, self.total_bytes));
    }

    pub fn set_received_bytes(&mut self, received_bytes: u64) {
        self.received_bytes = received_bytes;
        self.progress_changed
            .emit((self.received_bytes, self.total_bytes));

        if self.received_bytes >= self.total_bytes && self.total_bytes > 0 {
            self.state = DownloadState::Completed;
            self.state_changed.emit(self.state);
            self.finished.emit(());
        }
    }

    pub fn set_speed(&mut self, speed: u64) {
        if self.speed != speed {
            self.speed = speed;
            self.speed_changed.emit(self.speed);
        }
    }

    pub fn set_error(&mut self, error: Option<String>) {
        if self.error != error {
            let error_msg = error.clone();
            self.error = error;
            if let Some(ref msg) = error_msg {
                self.state = DownloadState::Failed;
                self.state_changed.emit(self.state);
                self.error_occurred.emit(msg.clone());
            }
        }
    }

    pub fn open(&self) {
        // In a real implementation, this would open the downloaded file
        // For now, we'll just do nothing
    }

    pub fn open_folder(&self) {
        // In a real implementation, this would open the folder containing the downloaded file
        // For now, we'll just do nothing
    }

    pub fn remove(&mut self) {
        // In a real implementation, this would remove the downloaded file
        // For now, we'll just do nothing
    }

    pub fn is_finished(&self) -> bool {
        self.state == DownloadState::Completed
    }

    pub fn is_canceled(&self) -> bool {
        self.state == DownloadState::Canceled
    }

    pub fn is_failed(&self) -> bool {
        self.state == DownloadState::Failed
    }

    pub fn is_active(&self) -> bool {
        self.state == DownloadState::Downloading || self.state == DownloadState::Paused
    }
}

impl Widget for WebEngineDownloadItem {
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

impl EventHandler for WebEngineDownloadItem {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine cookie store widget for cookie management.
pub struct WebEngineCookieStore {
    base: BaseWidget,
    cookies: Vec<Cookie>,
    /// Emitted when a cookie is added.
    pub cookie_added: Signal1<Cookie>,
    /// Emitted when a cookie is removed.
    pub cookie_removed: Signal1<String>, // cookie name
    /// Emitted when a cookie is changed.
    pub cookie_changed: Signal1<Cookie>,
    /// Emitted when all cookies are removed.
    pub cookies_cleared: GenericSignal,
}

/// Cookie structure.
#[derive(Debug, Clone)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<chrono::NaiveDateTime>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSitePolicy,
    pub session: bool,
}

/// Same-site policy for cookies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSitePolicy {
    /// No same-site policy.
    None,
    /// Lax same-site policy.
    Lax,
    /// Strict same-site policy.
    Strict,
}

impl WebEngineCookieStore {
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineCookieStore,
                Rect::new(0, 0, 0, 0),
                "WebEngineCookieStore",
            ),
            cookies: Vec::new(),
            cookie_added: Signal1::new(),
            cookie_removed: Signal1::new(),
            cookie_changed: Signal1::new(),
            cookies_cleared: GenericSignal::new(),
        }
    }

    pub fn cookies(&self) -> &[Cookie] {
        &self.cookies
    }
    pub fn cookie_count(&self) -> usize {
        self.cookies.len()
    }

    pub fn add_cookie(&mut self, cookie: Cookie) {
        // Check if the cookie already exists
        if let Some(index) = self.cookies.iter().position(|c| {
            c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path
        }) {
            // Update existing cookie
            self.cookies[index] = cookie.clone();
            self.cookie_changed.emit(cookie);
        } else {
            // Add new cookie
            self.cookies.push(cookie.clone());
            self.cookie_added.emit(cookie);
        }
    }

    pub fn remove_cookie(&mut self, name: &str, domain: &str, path: &str) -> bool {
        if let Some(index) = self
            .cookies
            .iter()
            .position(|c| c.name == name && c.domain == domain && c.path == path)
        {
            self.cookies.remove(index);
            self.cookie_removed.emit(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn remove_all_cookies(&mut self) {
        self.cookies.clear();
        self.cookies_cleared.emit();
    }

    pub fn remove_cookies_for_domain(&mut self, domain: &str) -> usize {
        let initial_len = self.cookies.len();
        self.cookies.retain(|c| c.domain != domain);
        let removed = initial_len - self.cookies.len();
        if removed > 0 {
            // Emit cookie_removed for each removed cookie
            for _ in 0..removed {
                self.cookie_removed.emit("domain_cookies".to_string());
            }
        }
        removed
    }

    pub fn remove_session_cookies(&mut self) -> usize {
        let initial_len = self.cookies.len();
        self.cookies.retain(|c| !c.session);
        let removed = initial_len - self.cookies.len();
        if removed > 0 {
            // Emit cookie_removed for each removed cookie
            for _ in 0..removed {
                self.cookie_removed.emit("session_cookie".to_string());
            }
        }
        removed
    }

    pub fn get_cookie(&self, name: &str, domain: &str, path: &str) -> Option<&Cookie> {
        self.cookies
            .iter()
            .find(|c| c.name == name && c.domain == domain && c.path == path)
    }

    pub fn get_cookies_for_domain(&self, domain: &str) -> Vec<&Cookie> {
        self.cookies.iter().filter(|c| c.domain == domain).collect()
    }

    pub fn get_all_cookies(&self) -> Vec<Cookie> {
        self.cookies.clone()
    }

    pub fn contains_cookie(&self, name: &str, domain: &str, path: &str) -> bool {
        self.cookies
            .iter()
            .any(|c| c.name == name && c.domain == domain && c.path == path)
    }

    pub fn clear_expired_cookies(&mut self) -> usize {
        let now = chrono::Local::now().naive_local();
        let initial_len = self.cookies.len();
        self.cookies.retain(|c| {
            if let Some(expires) = c.expires {
                expires > now
            } else {
                true
            }
        });
        let removed = initial_len - self.cookies.len();
        if removed > 0 {
            // Emit cookie_removed for each removed cookie
            for _ in 0..removed {
                self.cookie_removed.emit("expired_cookie".to_string());
            }
        }
        removed
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_cookie(
        &mut self,
        name: &str,
        value: &str,
        domain: &str,
        path: &str,
        expires: Option<chrono::NaiveDateTime>,
        secure: bool,
        http_only: bool,
        same_site: SameSitePolicy,
    ) {
        let cookie = Cookie {
            name: name.to_string(),
            value: value.to_string(),
            domain: domain.to_string(),
            path: path.to_string(),
            expires,
            secure,
            http_only,
            same_site,
            session: expires.is_none(),
        };
        self.add_cookie(cookie);
    }

    pub fn load_cookies(&mut self, cookies: Vec<Cookie>) {
        for cookie in cookies {
            self.add_cookie(cookie);
        }
    }

    pub fn save_cookies(&self) -> Vec<Cookie> {
        self.get_all_cookies()
    }
}

impl Default for WebEngineCookieStore {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for WebEngineCookieStore {
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

impl EventHandler for WebEngineCookieStore {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine web channel widget for JavaScript communication.
pub struct WebEngineWebChannel {
    base: BaseWidget,
    objects: Vec<WebChannelObject>,
    /// Emitted when a message is received from JavaScript.
    pub message_received: Signal1<(String, serde_json::Value)>,
    /// Emitted when an object is registered.
    pub object_registered: Signal1<String>, // object name
    /// Emitted when an object is unregistered.
    pub object_unregistered: Signal1<String>, // object name
}

/// Web channel object structure.
pub struct WebChannelObject {
    pub name: String,
    pub data: serde_json::Value,
    pub methods: Vec<String>,
}

impl WebEngineWebChannel {
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineWebChannel,
                Rect::new(0, 0, 0, 0),
                "WebEngineWebChannel",
            ),
            objects: Vec::new(),
            message_received: Signal1::new(),
            object_registered: Signal1::new(),
            object_unregistered: Signal1::new(),
        }
    }

    pub fn objects(&self) -> &[WebChannelObject] {
        &self.objects
    }
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn register_object(&mut self, name: String, data: serde_json::Value, methods: Vec<String>) {
        // Check if the object already exists
        if let Some(index) = self.objects.iter().position(|o| o.name == name) {
            // Update existing object
            self.objects[index] = WebChannelObject {
                name: name.clone(),
                data,
                methods,
            };
        } else {
            // Add new object
            self.objects.push(WebChannelObject {
                name: name.clone(),
                data,
                methods,
            });
            self.object_registered.emit(name);
        }
    }

    pub fn unregister_object(&mut self, name: &str) -> bool {
        if let Some(index) = self.objects.iter().position(|o| o.name == name) {
            self.objects.remove(index);
            self.object_unregistered.emit(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn unregister_all_objects(&mut self) {
        for object in &self.objects {
            self.object_unregistered.emit(object.name.clone());
        }
        self.objects.clear();
    }

    pub fn get_object(&self, name: &str) -> Option<&WebChannelObject> {
        self.objects.iter().find(|o| o.name == name)
    }

    pub fn send_message(&mut self, _name: &str, _message: serde_json::Value) {
        // In a real implementation, this would send the message to JavaScript
        // For now, we'll just do nothing
    }

    pub fn send_message_to_all(&mut self, _message: serde_json::Value) {
        // In a real implementation, this would send the message to all registered objects
        // For now, we'll just do nothing
    }

    pub fn receive_message(&mut self, name: &str, message: serde_json::Value) {
        // This method would be called when a message is received from JavaScript
        self.message_received.emit((name.to_string(), message));
    }

    pub fn has_object(&self, name: &str) -> bool {
        self.objects.iter().any(|o| o.name == name)
    }

    pub fn clear(&mut self) {
        self.unregister_all_objects();
    }
}

impl Default for WebEngineWebChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for WebEngineWebChannel {
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

impl EventHandler for WebEngineWebChannel {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine find text result widget for text search results.
pub struct WebEngineFindTextResult {
    base: BaseWidget,
    active_match_ordinal: u32,
    number_of_matches: u32,
    finished: bool,
    /// Emitted when the find text result changes.
    pub result_changed: Signal1<(u32, u32, bool)>,
}

impl WebEngineFindTextResult {
    pub fn new() -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineFindTextResult,
                Rect::new(0, 0, 0, 0),
                "WebEngineFindTextResult",
            ),
            active_match_ordinal: 0,
            number_of_matches: 0,
            finished: false,
            result_changed: Signal1::new(),
        }
    }

    pub fn active_match_ordinal(&self) -> u32 {
        self.active_match_ordinal
    }
    pub fn number_of_matches(&self) -> u32 {
        self.number_of_matches
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn set_active_match_ordinal(&mut self, ordinal: u32) {
        if self.active_match_ordinal != ordinal {
            self.active_match_ordinal = ordinal;
            self.result_changed.emit((
                self.active_match_ordinal,
                self.number_of_matches,
                self.finished,
            ));
        }
    }

    pub fn set_number_of_matches(&mut self, count: u32) {
        if self.number_of_matches != count {
            self.number_of_matches = count;
            self.result_changed.emit((
                self.active_match_ordinal,
                self.number_of_matches,
                self.finished,
            ));
        }
    }

    pub fn set_finished(&mut self, finished: bool) {
        if self.finished != finished {
            self.finished = finished;
            self.result_changed.emit((
                self.active_match_ordinal,
                self.number_of_matches,
                self.finished,
            ));
        }
    }

    pub fn reset(&mut self) {
        self.active_match_ordinal = 0;
        self.number_of_matches = 0;
        self.finished = false;
        self.result_changed.emit((
            self.active_match_ordinal,
            self.number_of_matches,
            self.finished,
        ));
    }

    pub fn update(&mut self, active_match: u32, total_matches: u32, is_finished: bool) {
        self.active_match_ordinal = active_match;
        self.number_of_matches = total_matches;
        self.finished = is_finished;
        self.result_changed.emit((
            self.active_match_ordinal,
            self.number_of_matches,
            self.finished,
        ));
    }
}

impl Default for WebEngineFindTextResult {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for WebEngineFindTextResult {
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

impl EventHandler for WebEngineFindTextResult {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine notification widget for web notifications.
pub struct WebEngineNotification {
    base: BaseWidget,
    title: String,
    body: String,
    icon: Option<Image>,
    tag: String,
    lang: String,
    require_interaction: bool,
    silent: bool,
    data: serde_json::Value,
    /// Emitted when the notification is shown.
    pub shown: GenericSignal,
    /// Emitted when the notification is closed.
    pub closed: GenericSignal,
    /// Emitted when the notification is clicked.
    pub clicked: GenericSignal,
    /// Emitted when the notification's action is activated.
    pub action_activated: Signal1<String>, // action name
}

impl WebEngineNotification {
    pub fn new(title: String, body: String) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineNotification,
                Rect::new(0, 0, 0, 0),
                "WebEngineNotification",
            ),
            title,
            body,
            icon: None,
            tag: "".to_string(),
            lang: "".to_string(),
            require_interaction: false,
            silent: false,
            data: serde_json::Value::Null,
            shown: GenericSignal::new(),
            closed: GenericSignal::new(),
            clicked: GenericSignal::new(),
            action_activated: Signal1::new(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    pub fn tag(&self) -> &str {
        &self.tag
    }
    pub fn lang(&self) -> &str {
        &self.lang
    }
    pub fn require_interaction(&self) -> bool {
        self.require_interaction
    }
    pub fn is_silent(&self) -> bool {
        self.silent
    }
    pub fn data(&self) -> &serde_json::Value {
        &self.data
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn set_body(&mut self, body: String) {
        self.body = body;
    }

    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }

    pub fn set_tag(&mut self, tag: String) {
        self.tag = tag;
    }

    pub fn set_lang(&mut self, lang: String) {
        self.lang = lang;
    }

    pub fn set_require_interaction(&mut self, require: bool) {
        self.require_interaction = require;
    }

    pub fn set_silent(&mut self, silent: bool) {
        self.silent = silent;
    }

    pub fn set_data(&mut self, data: serde_json::Value) {
        self.data = data;
    }

    pub fn show(&mut self) {
        // In a real implementation, this would show the notification
        // For now, we'll just emit the signal
        self.shown.emit();
    }

    pub fn close(&mut self) {
        // In a real implementation, this would close the notification
        // For now, we'll just emit the signal
        self.closed.emit();
    }

    pub fn click(&mut self) {
        // In a real implementation, this would handle a click on the notification
        // For now, we'll just emit the signal
        self.clicked.emit();
    }

    pub fn activate_action(&mut self, action: &str) {
        // In a real implementation, this would handle an action activation
        // For now, we'll just emit the signal
        self.action_activated.emit(action.to_string());
    }
}

impl Widget for WebEngineNotification {
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

impl EventHandler for WebEngineNotification {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine script dialog widget for JavaScript dialogs.
pub struct WebEngineScriptDialog {
    base: BaseWidget,
    dialog_type: ScriptDialogType,
    message: String,
    default_value: String,
    prompt_text: String,
    accept_text: String,
    reject_text: String,
    /// Emitted when the dialog is accepted.
    pub accepted: Signal1<String>, // input text for prompt dialogs
    /// Emitted when the dialog is rejected.
    pub rejected: GenericSignal,
    /// Emitted when the dialog is closed.
    pub closed: GenericSignal,
}

/// Script dialog type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptDialogType {
    /// Alert dialog.
    Alert,
    /// Confirm dialog.
    Confirm,
    /// Prompt dialog.
    Prompt,
    /// Before unload dialog.
    BeforeUnload,
}

impl WebEngineScriptDialog {
    pub fn new(dialog_type: ScriptDialogType, message: String) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineScriptDialog,
                Rect::new(0, 0, 0, 0),
                "WebEngineScriptDialog",
            ),
            dialog_type,
            message,
            default_value: "".to_string(),
            prompt_text: "".to_string(),
            accept_text: "OK".to_string(),
            reject_text: "Cancel".to_string(),
            accepted: Signal1::new(),
            rejected: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    pub fn dialog_type(&self) -> ScriptDialogType {
        self.dialog_type
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn default_value(&self) -> &str {
        &self.default_value
    }
    pub fn prompt_text(&self) -> &str {
        &self.prompt_text
    }
    pub fn accept_text(&self) -> &str {
        &self.accept_text
    }
    pub fn reject_text(&self) -> &str {
        &self.reject_text
    }

    pub fn set_default_value(&mut self, value: String) {
        self.default_value = value;
    }

    pub fn set_prompt_text(&mut self, text: String) {
        self.prompt_text = text;
    }

    pub fn set_accept_text(&mut self, text: String) {
        self.accept_text = text;
    }

    pub fn set_reject_text(&mut self, text: String) {
        self.reject_text = text;
    }

    pub fn accept(&mut self, input: String) {
        // In a real implementation, this would handle the dialog acceptance
        // For now, we'll just emit the signal
        self.accepted.emit(input);
        self.closed.emit();
    }

    pub fn reject(&mut self) {
        // In a real implementation, this would handle the dialog rejection
        // For now, we'll just emit the signal
        self.rejected.emit();
        self.closed.emit();
    }

    pub fn close(&mut self) {
        // In a real implementation, this would close the dialog
        // For now, we'll just emit the signal
        self.closed.emit();
    }
}

impl Widget for WebEngineScriptDialog {
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

impl EventHandler for WebEngineScriptDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

/// Web engine context menu request widget for context menu handling.
pub struct WebEngineContextMenuRequest {
    base: BaseWidget,
    position: Point,
    link_url: String,
    image_url: String,
    selected_text: String,
    media_url: String,
    media_type: MediaType,
    is_editable: bool,
    is_selected: bool,
    menu_items: Vec<ContextMenuItem>,
    /// Emitted when the context menu is accepted.
    pub accepted: Signal1<usize>, // menu item index
    /// Emitted when the context menu is rejected.
    pub rejected: GenericSignal,
    /// Emitted when the context menu is closed.
    pub closed: GenericSignal,
}

/// Media type for context menu requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// No media.
    None,
    /// Image media.
    Image,
    /// Video media.
    Video,
    /// Audio media.
    Audio,
}

/// Context menu item structure.
pub struct ContextMenuItem {
    pub id: String,
    pub text: String,
    pub enabled: bool,
    pub checked: bool,
    pub action: ContextMenuAction,
}

/// Context menu action type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuAction {
    /// No action.
    None,
    /// Custom action.
    Custom,
    /// Copy action.
    Copy,
    /// Cut action.
    Cut,
    /// Paste action.
    Paste,
    /// Delete action.
    Delete,
    /// Select all action.
    SelectAll,
    /// Open link action.
    OpenLink,
    /// Save link action.
    SaveLink,
    /// Copy link action.
    CopyLink,
    /// Open link in new tab action.
    OpenLinkInNewTab,
    /// Open link in new window action.
    OpenLinkInNewWindow,
    /// Save image action.
    SaveImage,
    /// Copy image action.
    CopyImage,
    /// Open image in new tab action.
    OpenImageInNewTab,
    /// Reload action.
    Reload,
    /// Stop action.
    Stop,
    /// Back action.
    Back,
    /// Forward action.
    Forward,
    /// Print action.
    Print,
    /// View source action.
    ViewSource,
    /// Inspect element action.
    InspectElement,
}

impl WebEngineContextMenuRequest {
    pub fn new(position: Point) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::WebEngineContextMenuRequest,
                Rect::new(0, 0, 0, 0),
                "WebEngineContextMenuRequest",
            ),
            position,
            link_url: "".to_string(),
            image_url: "".to_string(),
            selected_text: "".to_string(),
            media_url: "".to_string(),
            media_type: MediaType::None,
            is_editable: false,
            is_selected: false,
            menu_items: Vec::new(),
            accepted: Signal1::new(),
            rejected: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    pub fn position(&self) -> Point {
        self.position
    }
    pub fn link_url(&self) -> &str {
        &self.link_url
    }
    pub fn image_url(&self) -> &str {
        &self.image_url
    }
    pub fn selected_text(&self) -> &str {
        &self.selected_text
    }
    pub fn media_url(&self) -> &str {
        &self.media_url
    }
    pub fn media_type(&self) -> MediaType {
        self.media_type
    }
    pub fn is_editable(&self) -> bool {
        self.is_editable
    }
    pub fn is_selected(&self) -> bool {
        self.is_selected
    }
    pub fn menu_items(&self) -> &[ContextMenuItem] {
        &self.menu_items
    }
    pub fn menu_item_count(&self) -> usize {
        self.menu_items.len()
    }

    pub fn set_link_url(&mut self, url: String) {
        self.link_url = url;
    }

    pub fn set_image_url(&mut self, url: String) {
        self.image_url = url;
    }

    pub fn set_selected_text(&mut self, text: String) {
        self.selected_text = text;
    }

    pub fn set_media_url(&mut self, url: String) {
        self.media_url = url;
    }

    pub fn set_media_type(&mut self, media_type: MediaType) {
        self.media_type = media_type;
    }

    pub fn set_editable(&mut self, editable: bool) {
        self.is_editable = editable;
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.is_selected = selected;
    }

    pub fn add_menu_item(&mut self, item: ContextMenuItem) {
        self.menu_items.push(item);
    }

    pub fn remove_menu_item(&mut self, index: usize) -> bool {
        if index < self.menu_items.len() {
            self.menu_items.remove(index);
            true
        } else {
            false
        }
    }

    pub fn clear_menu_items(&mut self) {
        self.menu_items.clear();
    }

    pub fn accept(&mut self, index: usize) {
        // In a real implementation, this would handle the menu item selection
        // For now, we'll just emit the signal
        self.accepted.emit(index);
        self.closed.emit();
    }

    pub fn reject(&mut self) {
        // In a real implementation, this would handle the menu rejection
        // For now, we'll just emit the signal
        self.rejected.emit();
        self.closed.emit();
    }

    pub fn close(&mut self) {
        // In a real implementation, this would close the context menu
        // For now, we'll just emit the signal
        self.closed.emit();
    }

    pub fn has_link(&self) -> bool {
        !self.link_url.is_empty()
    }
    pub fn has_image(&self) -> bool {
        !self.image_url.is_empty()
    }
    pub fn has_selected_text(&self) -> bool {
        !self.selected_text.is_empty()
    }
    pub fn has_media(&self) -> bool {
        self.media_type != MediaType::None && !self.media_url.is_empty()
    }
}

impl Widget for WebEngineContextMenuRequest {
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

impl EventHandler for WebEngineContextMenuRequest {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct TestObservableTreeModel {
        nodes: Mutex<Vec<String>>,
        changed: GenericSignal,
    }

    struct TestObservableListModel {
        rows: Mutex<Vec<String>>,
        changed: GenericSignal,
    }

    impl TestObservableListModel {
        fn new(rows: Vec<String>) -> Self {
            Self {
                rows: Mutex::new(rows),
                changed: GenericSignal::new(),
            }
        }

        fn push_row(&self, value: impl Into<String>) {
            self.rows
                .lock()
                .expect("list model lock poisoned")
                .push(value.into());
            self.changed.emit();
        }
    }

    impl ListModel for TestObservableListModel {
        fn row_count(&self) -> usize {
            self.rows.lock().expect("list model lock poisoned").len()
        }

        fn data(&self, row: usize) -> Option<String> {
            self.rows
                .lock()
                .expect("list model lock poisoned")
                .get(row)
                .cloned()
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    impl TestObservableTreeModel {
        fn new(nodes: Vec<String>) -> Self {
            Self {
                nodes: Mutex::new(nodes),
                changed: GenericSignal::new(),
            }
        }

        fn push_node(&self, node: impl Into<String>) {
            self.nodes
                .lock()
                .expect("tree model lock poisoned")
                .push(node.into());
            self.changed.emit();
        }
    }

    impl TreeModel for TestObservableTreeModel {
        fn node_count(&self) -> usize {
            self.nodes.lock().expect("tree model lock poisoned").len()
        }

        fn node_path(&self, index: usize) -> Option<String> {
            self.nodes
                .lock()
                .expect("tree model lock poisoned")
                .get(index)
                .cloned()
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    struct TestObservableTableModel {
        headers: Vec<String>,
        rows: Mutex<Vec<Vec<String>>>,
        changed: GenericSignal,
    }

    impl TestObservableTableModel {
        fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
            Self {
                headers,
                rows: Mutex::new(rows),
                changed: GenericSignal::new(),
            }
        }

        fn push_row(&self, row: Vec<String>) {
            self.rows
                .lock()
                .expect("table model lock poisoned")
                .push(row);
            self.changed.emit();
        }
    }

    impl TableModel for TestObservableTableModel {
        fn row_count(&self) -> usize {
            self.rows.lock().expect("table model lock poisoned").len()
        }

        fn column_count(&self) -> usize {
            self.headers.len()
        }

        fn data(&self, row: usize, col: usize) -> Option<String> {
            self.rows
                .lock()
                .expect("table model lock poisoned")
                .get(row)
                .and_then(|row_data| row_data.get(col))
                .cloned()
        }

        fn header(&self, col: usize) -> Option<String> {
            self.headers.get(col).cloned()
        }

        fn data_changed_signal(&self) -> Option<&GenericSignal> {
            Some(&self.changed)
        }
    }

    #[test]
    fn vec_table_model_edit_contract() {
        let mut model = VecTableModel::new(
            vec!["name".to_string(), "value".to_string()],
            vec![vec!["a".to_string(), "1".to_string()]],
        );
        assert!(model.is_editable(0, 1));
        assert!(EditableTableModel::set_data(
            &mut model,
            0,
            1,
            "2".to_string()
        ));
        assert_eq!(model.data(0, 1).as_deref(), Some("2"));
    }

    #[test]
    fn vec_list_model_emits_data_changed_on_mutation() {
        let mut model = VecListModel::new(vec!["a".to_string()]);
        let hits = Arc::new(AtomicUsize::new(0));
        {
            let hits_ref = Arc::clone(&hits);
            model.data_changed().connect(move || {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }

        model.add_item("b");
        assert!(model.set_item(0, "x"));
        assert!(model.remove_item(1));

        assert_eq!(model.row_count(), 1);
        assert_eq!(model.data(0).as_deref(), Some("x"));
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn vec_table_model_emits_data_changed_on_mutation() {
        let mut model = VecTableModel::new(
            vec!["name".to_string(), "value".to_string()],
            vec![vec!["a".to_string(), "1".to_string()]],
        );

        let hits = Arc::new(AtomicUsize::new(0));
        {
            let hits_ref = Arc::clone(&hits);
            model.data_changed().connect(move || {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(model.set_cell(0, 1, "2"));
        model.push_row(vec!["b".to_string(), "3".to_string()]);
        assert!(model.remove_row(0));

        assert_eq!(hits.load(Ordering::SeqCst), 3);
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
        let tree_model = Arc::new(VecTreeModel::new(vec!["root".to_string()]));
        tree.set_model(tree_model);
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
    fn tree_view_auto_refreshes_on_observable_model_change() {
        let model = Arc::new(TestObservableTreeModel::new(vec!["root".to_string()]));
        let mut tree = TreeView::new(Rect::new(0, 0, 120, 80));

        let redraw_hits = Arc::new(AtomicUsize::new(0));
        let layout_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&redraw_hits);
            tree.redraw_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&layout_hits);
            tree.layout_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        tree.set_model(model.clone());
        model.push_node("root/child");

        assert_eq!(tree.node_count(), 2);
        assert!(redraw_hits.load(Ordering::SeqCst) >= 2);
        assert!(layout_hits.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn table_widget_auto_refreshes_on_observable_model_change() {
        let model = Arc::new(TestObservableTableModel::new(
            vec!["name".to_string()],
            vec![vec!["a".to_string()]],
        ));
        let mut table = TableWidget::new(Rect::new(0, 0, 120, 80));

        let redraw_hits = Arc::new(AtomicUsize::new(0));
        let layout_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&redraw_hits);
            table.redraw_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&layout_hits);
            table.layout_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        table.set_model(model.clone());
        model.push_row(vec!["b".to_string()]);

        assert_eq!(table.row_count(), 2);
        assert!(redraw_hits.load(Ordering::SeqCst) >= 2);
        assert!(layout_hits.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn list_view_auto_refreshes_on_observable_model_change() {
        let model = Arc::new(TestObservableListModel::new(vec!["a".to_string()]));
        let mut list = ListView::new(Rect::new(0, 0, 120, 80));

        let redraw_hits = Arc::new(AtomicUsize::new(0));
        let layout_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&redraw_hits);
            list.redraw_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&layout_hits);
            list.layout_requested_signal().connect(move || {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        list.set_model(model.clone());
        model.push_row("b");

        assert_eq!(list.row_count(), 2);
        assert_eq!(list.item(1).as_deref(), Some("b"));
        assert!(redraw_hits.load(Ordering::SeqCst) >= 2);
        assert!(layout_hits.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn table_view_forwards_table_contract_and_selection_signal() {
        let model = Arc::new(VecTableModel::new(
            vec!["name".to_string()],
            vec![vec!["a".to_string()]],
        ));
        let mut table = TableView::new(Rect::new(0, 0, 120, 80));
        table.set_model(model);

        let hits = Arc::new(AtomicUsize::new(0));
        {
            let hits_ref = Arc::clone(&hits);
            table.selection_changed_signal().connect(move |_| {
                hits_ref.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert_eq!(table.column_count(), 1);
        assert_eq!(table.header(0).as_deref(), Some("name"));
        assert_eq!(table.cell(0, 0).as_deref(), Some("a"));
        assert!(table.select_row(0));
        assert_eq!(table.selected_row(), Some(0));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn list_view_selection_focus_projection_sync_contract() {
        let model_a = Arc::new(VecListModel::new(vec!["a".to_string(), "b".to_string()]));
        let model_b = Arc::new(VecListModel::new(vec!["a".to_string()]));
        let mut list = ListView::new(Rect::new(0, 0, 120, 80));

        let focus_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&focus_hits);
            list.focused_row_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        list.set_model(model_a);
        assert!(list.select_row(1));
        assert_eq!(list.selected_row(), Some(1));
        assert_eq!(list.focused_row(), Some(1));

        list.set_model(model_b);
        assert_eq!(list.selected_row(), None);
        assert_eq!(list.focused_row(), None);
        assert!(focus_hits.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn tree_view_selection_focus_projection_sync_contract() {
        let model_a = Arc::new(VecTreeModel::new(vec![
            "root".to_string(),
            "root/child".to_string(),
        ]));
        let model_b = Arc::new(VecTreeModel::new(vec!["root".to_string()]));
        let mut tree = TreeView::new(Rect::new(0, 0, 120, 80));

        let focus_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&focus_hits);
            tree.focused_node_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        tree.set_model(model_a);
        assert!(tree.select_node(1));
        assert_eq!(tree.selected_node(), Some(1));
        assert_eq!(tree.focused_node(), Some(1));

        tree.set_model(model_b);
        assert_eq!(tree.selected_node(), None);
        assert_eq!(tree.focused_node(), None);
        assert!(focus_hits.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn table_widget_selection_focus_projection_sync_contract() {
        let model_a = Arc::new(VecTableModel::new(
            vec!["name".to_string()],
            vec![vec!["a".to_string()], vec!["b".to_string()]],
        ));
        let model_b = Arc::new(VecTableModel::new(
            vec!["name".to_string()],
            vec![vec!["a".to_string()]],
        ));
        let mut table = TableWidget::new(Rect::new(0, 0, 120, 80));

        let focus_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&focus_hits);
            table.focused_row_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        table.set_model(model_a);
        assert!(table.select_row(1));
        assert_eq!(table.selected_row(), Some(1));
        assert_eq!(table.focused_row(), Some(1));

        table.set_model(model_b);
        assert_eq!(table.selected_row(), None);
        assert_eq!(table.focused_row(), None);
        assert!(focus_hits.load(Ordering::SeqCst) >= 1);
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

        base.handle_event(&Event::MouseEnter {
            pos: Point::new(1, 2),
        });
        base.handle_event(&Event::MouseMove {
            pos: Point::new(2, 3),
        });
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
        base.handle_event(&Event::MouseLeave {
            pos: Point::new(9, 9),
        });

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
    fn rich_edit_baseline_contract_covers_text_selection_read_only_and_signals() {
        let mut editor = RichEdit::new(Rect::new(0, 0, 200, 120));

        let text_hits = Arc::new(AtomicUsize::new(0));
        let selection_hits = Arc::new(AtomicUsize::new(0));
        let read_only_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&text_hits);
            editor.text_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&selection_hits);
            editor.selection_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&read_only_hits);
            editor.read_only_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        editor.append_text("abc");
        assert_eq!(editor.text(), "abc");

        editor.set_selection(1, 3);
        assert_eq!(editor.selection(), Some((1, 3)));
        assert_eq!(editor.delete_selection().as_deref(), Some("bc"));
        assert_eq!(editor.text(), "a");

        editor.set_read_only(true);
        editor.append_text("x");
        assert_eq!(editor.text(), "a");
        assert_eq!(editor.delete_selection(), None);

        editor.set_read_only(false);
        editor.insert_text("z");
        assert_eq!(editor.text(), "az");

        assert_eq!(text_hits.load(Ordering::SeqCst), 3);
        assert_eq!(selection_hits.load(Ordering::SeqCst), 2);
        assert_eq!(read_only_hits.load(Ordering::SeqCst), 2);
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

    #[test]
    fn scrollbar_and_scrollarea_contracts_are_deterministic() {
        let mut scrollbar = ScrollBar::new(Rect::new(0, 0, 16, 100));
        let scrollbar_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&scrollbar_hits);
            scrollbar.value_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        scrollbar.set_range(0, 50);
        scrollbar.set_page_step(7);
        scrollbar.set_single_step(3);
        scrollbar.set_value(45);
        scrollbar.page_increment();
        scrollbar.line_increment();
        scrollbar.page_decrement();

        assert_eq!(scrollbar.value(), 43);
        assert_eq!(scrollbar_hits.load(Ordering::SeqCst), 3);

        let mut area = ScrollArea::new(Rect::new(0, 0, 40, 30));
        let offset_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&offset_hits);
            area.scroll_offset_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        area.set_content_size(Size::new(200, 120));
        area.set_viewport_size(Size::new(40, 30));
        area.set_scroll_offset(Point::new(500, 500));
        assert_eq!(area.scroll_offset(), Point::new(160, 90));

        area.set_viewport_size(Size::new(190, 100));
        assert_eq!(area.scroll_offset(), Point::new(10, 20));
        assert_eq!(offset_hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn groupbox_and_tabwidget_contracts_are_deterministic() {
        let mut group = GroupBox::new(Rect::new(0, 0, 120, 60));
        let title_hits = Arc::new(AtomicUsize::new(0));
        let check_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&title_hits);
            group.title_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&check_hits);
            group.checked_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        group.set_title("Options".to_string());
        group.set_title("Options".to_string());
        group.set_checkable(true);
        group.toggle_checked();
        group.toggle_checked();

        assert_eq!(group.title(), "Options");
        assert!(group.is_checkable());
        assert!(!group.is_checked());
        assert_eq!(title_hits.load(Ordering::SeqCst), 1);
        assert_eq!(check_hits.load(Ordering::SeqCst), 2);

        let mut tab = TabWidget::new(Rect::new(0, 0, 200, 120));
        let index_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&index_hits);
            tab.current_index_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        let first = tab.add_tab(10);
        let second = tab.add_tab(11);
        assert_eq!(first, 0);
        assert_eq!(second, 1);
        assert_eq!(tab.current_index(), Some(0));
        assert_eq!(tab.current_tab(), Some(10));

        assert!(tab.set_current_index(1));
        assert_eq!(tab.current_tab(), Some(11));
        assert!(tab.remove_tab(11));
        assert_eq!(tab.current_index(), Some(0));
        assert_eq!(tab.current_tab(), Some(10));

        assert_eq!(index_hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn splitter_contract_distributes_sizes_and_emits_signals() {
        let mut splitter = Splitter::new(Rect::new(0, 0, 200, 100));

        let layout_hits = Arc::new(AtomicUsize::new(0));
        let orientation_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&layout_hits);
            splitter.pane_layout_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let hits = Arc::clone(&orientation_hits);
            splitter.orientation_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        splitter.add_pane(1, 1);
        splitter.add_pane(2, 3);
        let sizes = splitter.distribute_sizes(400, 0);
        assert_eq!(sizes.len(), 2);
        assert_eq!(sizes.iter().sum::<u32>(), 400);
        assert!(sizes[1] > sizes[0]);

        assert!(splitter.set_ratio(0, 2.0));
        let sizes_after = splitter.distribute_sizes(400, 0);
        assert_eq!(sizes_after.iter().sum::<u32>(), 400);
        assert!(sizes_after[0] > sizes[0]);

        splitter.set_orientation(SplitterOrientation::Vertical);
        assert_eq!(splitter.orientation(), SplitterOrientation::Vertical);
        assert!(splitter.remove_pane(2));
        assert_eq!(splitter.pane_count(), 1);

        assert_eq!(layout_hits.load(Ordering::SeqCst), 4);
        assert_eq!(orientation_hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dock_panel_and_mdi_area_contracts_are_deterministic() {
        let mut dock = DockPanel::new(Rect::new(0, 0, 300, 200));
        let dock_layout_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&dock_layout_hits);
            dock.layout_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(dock.add_pane(10, DockArea::Left));
        assert!(dock.add_pane(11, DockArea::Center));
        assert_eq!(dock.pane_area(10), Some(DockArea::Left));
        assert!(dock.move_pane(10, DockArea::Right));
        assert_eq!(dock.pane_area(10), Some(DockArea::Right));
        assert!(dock.remove_pane(11));
        assert_eq!(dock.panes(), &[(10, DockArea::Right)]);
        assert_eq!(dock_layout_hits.load(Ordering::SeqCst), 4);

        let mut mdi = MdiArea::new(Rect::new(0, 0, 400, 240));
        let active_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&active_hits);
            mdi.active_document_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(mdi.add_document(100));
        assert!(mdi.add_document(101));
        assert_eq!(mdi.active_document(), Some(100));
        assert!(mdi.set_active_document(101));
        assert_eq!(mdi.active_document(), Some(101));
        assert!(mdi.remove_document(101));
        assert_eq!(mdi.active_document(), Some(100));
        assert_eq!(mdi.documents(), &[100]);
        assert_eq!(active_hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn advanced_widgets_use_distinct_widget_kinds() {
        let rich = RichEdit::new(Rect::new(0, 0, 100, 80));
        let list_view = ListView::new(Rect::new(0, 0, 100, 80));
        let dock = DockPanel::new(Rect::new(0, 0, 100, 80));
        let mdi = MdiArea::new(Rect::new(0, 0, 100, 80));

        assert_eq!(rich.kind(), WidgetKind::RichEdit);
        assert_eq!(list_view.kind(), WidgetKind::ListView);
        assert_eq!(dock.kind(), WidgetKind::DockPanel);
        assert_eq!(mdi.kind(), WidgetKind::MdiArea);
    }

    #[test]
    fn menu_toolbar_statusbar_contracts_are_deterministic() {
        let mut menu_bar = MenuBar::new(Rect::new(0, 0, 200, 24));
        let mut menu = Menu::new(Rect::new(0, 0, 100, 24));
        let mut toolbar = ToolBar::new(Rect::new(0, 0, 200, 24));
        let mut status = StatusBar::new(Rect::new(0, 0, 200, 20));

        let menu_change_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&menu_change_hits);
            menu_bar.current_menu_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(menu_bar.add_menu(10));
        assert!(menu_bar.add_menu(11));
        assert!(menu_bar.set_current_menu(11));
        assert!(menu_bar.remove_menu(11));
        assert_eq!(menu_bar.current_menu(), Some(10));
        assert_eq!(menu_change_hits.load(Ordering::SeqCst), 3);

        let menu_trigger_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&menu_trigger_hits);
            menu.action_triggered.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(menu.add_action("open"));
        assert!(!menu.add_action("open"));
        assert!(menu.trigger_action("open"));
        assert!(!menu.trigger_action("save"));
        assert_eq!(menu_trigger_hits.load(Ordering::SeqCst), 1);

        let toolbar_trigger_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&toolbar_trigger_hits);
            toolbar.action_triggered.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(toolbar.add_action("build"));
        assert!(toolbar.trigger_action("build"));
        assert!(toolbar.remove_action("build"));
        assert_eq!(toolbar_trigger_hits.load(Ordering::SeqCst), 1);

        let status_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&status_hits);
            status.message_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        status.set_message("ready".to_string());
        status.set_message("ready".to_string());
        status.set_message("running".to_string());
        assert_eq!(status.message(), "running");
        assert_eq!(status_hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn dialog_family_contracts_emit_deterministic_result_signals() {
        let mut dialog = Dialog::new(Rect::new(0, 0, 200, 120));
        let dialog_finished = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&dialog_finished);
            dialog.finished.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        dialog.set_modal(true);
        dialog.accept();
        assert!(dialog.is_modal());
        assert_eq!(dialog.result(), Some(DialogResult::Accepted));
        assert_eq!(dialog_finished.load(Ordering::SeqCst), 1);

        let mut msg = MessageBox::new(Rect::new(0, 0, 180, 100));
        let msg_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&msg_hits);
            msg.result_changed.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        msg.set_title("Warn".to_string());
        msg.set_text("Disk low".to_string());
        msg.set_icon(MessageBoxIcon::Warning);
        msg.set_result(DialogResult::Rejected);
        assert_eq!(msg.title(), "Warn");
        assert_eq!(msg.text(), "Disk low");
        assert_eq!(msg.icon(), MessageBoxIcon::Warning);
        assert_eq!(msg.result(), Some(DialogResult::Rejected));
        assert_eq!(msg_hits.load(Ordering::SeqCst), 1);

        let mut file = FileDialog::new(Rect::new(0, 0, 220, 140));
        let file_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&file_hits);
            file.file_selected.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        file.set_current_dir("/tmp".to_string());
        file.set_selected_file(Some("a.txt".to_string()));
        assert_eq!(file.current_dir(), "/tmp");
        assert_eq!(file.selected_file(), Some("a.txt"));
        assert_eq!(file_hits.load(Ordering::SeqCst), 1);

        let mut color_dialog = ColorDialog::new(Rect::new(0, 0, 140, 120));
        let color_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&color_hits);
            color_dialog.color_selected.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        color_dialog.set_color(Color::rgba(12, 34, 56, 255));
        assert_eq!(color_dialog.color(), Color::rgba(12, 34, 56, 255));
        assert_eq!(color_hits.load(Ordering::SeqCst), 1);

        let mut font_dialog = FontDialog::new(Rect::new(0, 0, 140, 120));
        let font_hits = Arc::new(AtomicUsize::new(0));
        {
            let hits = Arc::clone(&font_hits);
            font_dialog.font_selected.connect(move |_| {
                hits.fetch_add(1, Ordering::SeqCst);
            });
        }
        font_dialog.set_font(Font::with_weight("Sans", 12.0, 700, false));
        assert_eq!(font_dialog.font().family, "Sans");
        assert_eq!(font_dialog.font().weight, 700);
        assert_eq!(font_hits.load(Ordering::SeqCst), 1);
    }
}
