//! Widget models and controls.

use std::sync::Arc;

use crate::core::{Alignment, ObjectId, Rect};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::signal::{GenericSignal, Signal1};
use crate::style::WidgetStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetKind {
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
    Chart,
}

pub trait Widget: EventHandler {
    fn id(&self) -> ObjectId;
    fn kind(&self) -> WidgetKind;
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
    fn parent(&self) -> Option<ObjectId>;
    fn set_parent(&mut self, parent: Option<ObjectId>);
    fn add_child(&mut self, child: ObjectId);
    fn remove_child(&mut self, child: ObjectId);
    fn children(&self) -> &[ObjectId];
    fn show(&mut self);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);
    fn is_enabled(&self) -> bool;
    fn set_tooltip(&mut self, tooltip: String);
    fn tooltip(&self) -> &str;
    fn style(&self) -> &WidgetStyle;
    fn set_style(&mut self, style: WidgetStyle);
}

pub struct BaseWidget {
    object: Object,
    kind: WidgetKind,
    geometry: Rect,
    parent: Option<ObjectId>,
    children: Vec<ObjectId>,
    visible: bool,
    enabled: bool,
    tooltip: String,
    style: WidgetStyle,
    pub clicked: GenericSignal,
    pub changed: GenericSignal,
}

impl BaseWidget {
    pub fn new(kind: WidgetKind, geometry: Rect, class_name: &'static str) -> Self {
        Self {
            object: Object::new(class_name),
            kind,
            geometry,
            parent: None,
            children: Vec::new(),
            visible: true,
            enabled: true,
            tooltip: String::new(),
            style: WidgetStyle::default(),
            clicked: GenericSignal::new(),
            changed: GenericSignal::new(),
        }
    }
}

impl Widget for BaseWidget {
    fn id(&self) -> ObjectId { self.object.id() }
    fn kind(&self) -> WidgetKind { self.kind }
    fn geometry(&self) -> Rect { self.geometry }
    fn set_geometry(&mut self, geometry: Rect) { self.geometry = geometry; }
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
}

impl EventHandler for BaseWidget {
    fn handle_event(&mut self, event: &Event) {
        if !self.enabled || !self.visible {
            return;
        }
        if let Event::MousePress { .. } = event {
            self.clicked.emit();
        }
    }
}

macro_rules! impl_widget_delegate {
    ($ty:ty, $field:ident) => {
        impl Widget for $ty {
            fn id(&self) -> ObjectId { self.$field.id() }
            fn kind(&self) -> WidgetKind { self.$field.kind() }
            fn geometry(&self) -> Rect { self.$field.geometry() }
            fn set_geometry(&mut self, geometry: Rect) { self.$field.set_geometry(geometry); }
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
        }
        impl EventHandler for $ty {
            fn handle_event(&mut self, event: &Event) { self.$field.handle_event(event); }
        }
    };
}

pub struct Window {
    base: BaseWidget,
    title: String,
    pub closed: GenericSignal,
}

impl Window {
    pub fn new(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "Window"),
            title,
            closed: GenericSignal::new(),
        }
    }
    pub fn title(&self) -> &str { &self.title }
    pub fn set_title(&mut self, title: String) { self.title = title; }
}
impl_widget_delegate!(Window, base);

pub struct Dialog {
    base: BaseWidget,
}
impl Dialog { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::Dialog, geometry, "Dialog") } } }
impl_widget_delegate!(Dialog, base);

pub struct PopupWindow { base: BaseWidget }
impl PopupWindow { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow") } } }
impl_widget_delegate!(PopupWindow, base);

pub struct Button { base: BaseWidget, text: String, pub activated: GenericSignal }
impl Button {
    pub fn new(text: String, geometry: Rect) -> Self {
        Self { base: BaseWidget::new(WidgetKind::Button, geometry, "Button"), text, activated: GenericSignal::new() }
    }
    pub fn text(&self) -> &str { &self.text }
}
impl_widget_delegate!(Button, base);

pub struct CheckBox { base: BaseWidget, checked: bool, pub toggled: Signal1<bool> }
impl CheckBox {
    pub fn new(geometry: Rect) -> Self {
        Self { base: BaseWidget::new(WidgetKind::CheckBox, geometry, "CheckBox"), checked: false, toggled: Signal1::new() }
    }
    pub fn set_checked(&mut self, checked: bool) { self.checked = checked; self.toggled.emit(checked); }
}
impl_widget_delegate!(CheckBox, base);

pub struct RadioButton { base: BaseWidget, checked: bool }
impl RadioButton { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::RadioButton, geometry, "RadioButton"), checked: false } } pub fn set_checked(&mut self, checked: bool) { self.checked = checked; } }
impl_widget_delegate!(RadioButton, base);

pub struct Label { base: BaseWidget, text: String, alignment: Alignment }
impl Label {
    pub fn new(text: String, geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::Label, geometry, "Label"), text, alignment: Alignment::Left } }
    pub fn set_alignment(&mut self, alignment: Alignment) { self.alignment = alignment; }
    pub fn text(&self) -> &str { &self.text }
}
impl_widget_delegate!(Label, base);

pub struct LineEdit { base: BaseWidget, text: String, pub text_changed: Signal1<String> }
impl LineEdit {
    pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"), text: String::new(), text_changed: Signal1::new() } }
    pub fn set_text(&mut self, text: String) { self.text = text.clone(); self.text_changed.emit(text); }
}
impl_widget_delegate!(LineEdit, base);

pub struct TextEdit { base: BaseWidget, text: String }
impl TextEdit { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::TextEdit, geometry, "TextEdit"), text: String::new() } } pub fn set_text(&mut self, text: String) { self.text = text; } }
impl_widget_delegate!(TextEdit, base);

pub struct ComboBox { base: BaseWidget, items: Vec<String>, current: usize }
impl ComboBox {
    pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ComboBox, geometry, "ComboBox"), items: Vec::new(), current: 0 } }
    pub fn add_item(&mut self, item: impl Into<String>) { self.items.push(item.into()); }
    pub fn set_current_index(&mut self, index: usize) { if index < self.items.len() { self.current = index; } }
}
impl_widget_delegate!(ComboBox, base);

pub struct ListBox { base: BaseWidget, items: Vec<String> }
impl ListBox { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ListBox, geometry, "ListBox"), items: Vec::new() } } pub fn add_item(&mut self, item: impl Into<String>) { self.items.push(item.into()); } }
impl_widget_delegate!(ListBox, base);

pub struct TreeView { base: BaseWidget, nodes: Vec<String> }
impl TreeView { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"), nodes: Vec::new() } } pub fn add_node(&mut self, node: impl Into<String>) { self.nodes.push(node.into()); } }
impl_widget_delegate!(TreeView, base);

pub trait ListModel: Send + Sync {
    fn row_count(&self) -> usize;
    fn data(&self, row: usize) -> Option<String>;
}

pub trait TreeModel: Send + Sync {
    fn node_count(&self) -> usize;
    fn node_path(&self, index: usize) -> Option<String>;
}

pub trait TableModel: Send + Sync {
    fn row_count(&self) -> usize;
    fn column_count(&self) -> usize;
    fn data(&self, row: usize, col: usize) -> Option<String>;
    fn header(&self, col: usize) -> Option<String>;
}

pub struct VecTableModel {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl VecTableModel {
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self { headers, rows }
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
}

pub struct SortFilterTableModel {
    source: Arc<dyn TableModel>,
    filter_text: Option<String>,
}

impl SortFilterTableModel {
    pub fn new(source: Arc<dyn TableModel>) -> Self {
        Self {
            source,
            filter_text: None,
        }
    }

    pub fn set_filter_text(&mut self, text: Option<String>) {
        self.filter_text = text;
    }

    fn visible_rows(&self) -> Vec<usize> {
        let Some(filter) = self.filter_text.as_ref() else {
            return (0..self.source.row_count()).collect();
        };

        (0..self.source.row_count())
            .filter(|row| {
                (0..self.source.column_count()).any(|col| {
                    self.source
                        .data(*row, col)
                        .map(|cell| cell.to_lowercase().contains(&filter.to_lowercase()))
                        .unwrap_or(false)
                })
            })
            .collect()
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

pub struct ProgressBar { base: BaseWidget, value: u32 }
impl ProgressBar { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ProgressBar, geometry, "ProgressBar"), value: 0 } } pub fn set_value(&mut self, value: u32) { self.value = value.min(100); } }
impl_widget_delegate!(ProgressBar, base);

pub struct Slider { base: BaseWidget, value: i32 }
impl Slider { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::Slider, geometry, "Slider"), value: 0 } } pub fn set_value(&mut self, value: i32) { self.value = value; } }
impl_widget_delegate!(Slider, base);

pub struct ScrollBar { base: BaseWidget, value: i32 }
impl ScrollBar { pub fn new(geometry: Rect) -> Self { Self { base: BaseWidget::new(WidgetKind::ScrollBar, geometry, "ScrollBar"), value: 0 } } pub fn set_value(&mut self, value: i32) { self.value = value; } }
impl_widget_delegate!(ScrollBar, base);

macro_rules! simple_control {
    ($name:ident, $kind:expr) => {
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

pub struct TableWidget {
    base: BaseWidget,
    model: Option<Arc<dyn TableModel>>,
}

impl TableWidget {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
        }
    }

    pub fn set_model(&mut self, model: Arc<dyn TableModel>) {
        self.model = Some(model);
    }

    pub fn row_count(&self) -> usize {
        self.model.as_ref().map(|m| m.row_count()).unwrap_or(0)
    }

    pub fn column_count(&self) -> usize {
        self.model.as_ref().map(|m| m.column_count()).unwrap_or(0)
    }
}

impl_widget_delegate!(TableWidget, base);

simple_control!(GridWidget, WidgetKind::Grid);
simple_control!(ChartWidget, WidgetKind::Chart);
