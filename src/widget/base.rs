//! Base widget types and traits.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use chrono::{Datelike, Timelike};
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
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
    MdiArea,
    MenuBar,
    Menu,
    ContextMenu,
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
    DirectoryDialog,
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
/// Custom drawing trait for widgets that want to render their own content.
/// Widgets implementing this trait can provide custom drawing logic instead of
/// relying solely on native platform rendering.
pub trait Draw {
    /// Draw the widget's content using the provided render context.
    /// This method is called when the widget needs to be repainted.
    fn draw(&mut self, context: &mut RenderContext);
    /// Returns true if this widget uses custom drawing, false for native rendering.
    /// This allows the rendering system to choose between native and custom paths.
    fn uses_custom_drawing(&self) -> bool {
        true
    }
    /// Optional: Request a redraw of the widget.
    /// Default implementation calls request_redraw() on the widget.
    fn request_custom_redraw(&self)
    where
        Self: Widget,
    {
        self.request_redraw();
    }
}
/// Common widget contract implemented by all widget models.
pub trait Widget: EventHandler {
    /// Returns shared base widget state for default trait delegation.
    fn base(&self) -> &BaseWidget {
        panic!("Widget::base() not implemented")
    }
    /// Returns mutable base widget state for default trait delegation.
    fn base_mut(&mut self) -> &mut BaseWidget {
        panic!("Widget::base_mut() not implemented")
    }
    /// Get stable widget id.
    fn id(&self) -> ObjectId {
        self.base().id()
    }
    /// Get widget runtime kind.
    fn kind(&self) -> WidgetKind {
        self.base().kind()
    }
    fn geometry(&self) -> Rect {
        self.base().geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base_mut().set_geometry(geometry);
    }
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
    fn min_size(&self) -> Option<Size> {
        self.base().min_size()
    }
    /// Returns maximum size constraint when configured.
    fn max_size(&self) -> Option<Size> {
        self.base().max_size()
    }
    /// Sets minimum size constraint.
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base_mut().set_min_size(min_size);
    }
    /// Sets maximum size constraint.
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base_mut().set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base().parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base_mut().set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base_mut().add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base_mut().remove_child(child);
    }
    fn children(&self) -> &[ObjectId] {
        self.base().children()
    }
    /// Show widget.
    fn show(&mut self) {
        self.base_mut().show();
    }
    /// Hide widget.
    fn hide(&mut self) {
        self.base_mut().hide();
    }
    fn is_visible(&self) -> bool {
        self.base().is_visible()
    }
    fn set_visible(&mut self, visible: bool) {
        if visible {
            self.show();
        } else {
            self.hide();
        }
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base_mut().set_enabled(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base().is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base_mut().set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base().tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base().style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base_mut().set_style(style);
    }
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
    fn connection_scope(&self) -> &ConnectionScope {
        self.base().connection_scope()
    }
    /// Optional clicked signal (legacy API compatibility).
    fn clicked_signal(&self) -> &GenericSignal {
        &self.base().clicked
    }
    /// Optional changed signal (legacy API compatibility).
    fn changed_signal(&self) -> &GenericSignal {
        &self.base().changed
    }
    /// Emits on hover/move interactions while pointer is over widget.
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base().hover_signal()
    }
    /// Emits on mouse/pointer press interactions.
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_down_signal()
    }
    /// Emits on mouse/pointer release interactions.
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base().mouse_up_signal()
    }
    /// Emits on keyboard press interactions.
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_down_signal()
    }
    /// Emits on keyboard release interactions.
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base().key_up_signal()
    }
    /// Emits when logical focus is gained.
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base().focus_gained_signal()
    }
    /// Emits when logical focus is lost.
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base().focus_lost_signal()
    }
    /// Emits when redraw is requested.
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base().redraw_requested_signal()
    }
    /// Emits when layout pass is requested.
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base().layout_requested_signal()
    }
    /// Requests redraw and emits redraw signal.
    fn request_redraw(&self) {
        self.redraw_requested_signal().emit();
    }
    /// Requests layout and emits layout signal.
    fn request_layout(&self) {
        self.layout_requested_signal().emit();
    }
    /// Returns the preferred size hint for layout calculations.
    fn size_hint(&self) -> Size {
        self.size()
    }
}
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
    pub(crate) tooltip: String,
    pub(crate) style: WidgetStyle,
    pub(crate) connection_scope: ConnectionScope,
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
    // 基础方法实现
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
    pub fn style(&self) -> &WidgetStyle {
        &self.style
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
    pub fn request_redraw(&self) {
        self.redraw_requested.emit();
    }
    pub fn request_layout(&self) {
        self.layout_requested.emit();
    }
}
impl EventHandler for BaseWidget {
    fn handle_event(&mut self, event: &Event) {
        // 基础事件处理逻辑
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
