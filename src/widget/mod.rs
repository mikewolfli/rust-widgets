//! Widget models and controls.

use std::sync::Arc;

use crate::core::{Alignment, ObjectId, Rect};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::signal::{GenericSignal, Signal1};
use crate::style::WidgetStyle;

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

pub trait Widget: EventHandler {
    /// Get stable widget id.
    fn id(&self) -> ObjectId;
    /// Get widget runtime kind.
    fn kind(&self) -> WidgetKind;
    fn geometry(&self) -> Rect;
    fn set_geometry(&mut self, geometry: Rect);
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
    /// Emitted when a click-like interaction is received.
    pub clicked: GenericSignal,
    /// Emitted when widget internal value/state changes.
    pub changed: GenericSignal,
}

impl BaseWidget {
    /// Create base widget state and core signals.
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

pub trait ListModel: Send + Sync {
    /// Number of rows exposed by model.
    fn row_count(&self) -> usize;
    /// Data for row index, if present.
    fn data(&self, row: usize) -> Option<String>;
}

pub trait TreeModel: Send + Sync {
    fn node_count(&self) -> usize;
    fn node_path(&self, index: usize) -> Option<String>;
}

pub struct VecTreeModel {
    paths: Vec<String>,
}

impl VecTreeModel {
    pub fn new(paths: Vec<String>) -> Self {
        Self { paths }
    }

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

pub struct SortFilterTreeModel {
    /// Underlying source tree model.
    source: Arc<dyn TreeModel>,
    /// Optional case-insensitive substring filter.
    filter_text: Option<String>,
    /// Ascending path sort flag.
    sort_ascending: bool,
}

impl SortFilterTreeModel {
    pub fn new(source: Arc<dyn TreeModel>) -> Self {
        Self {
            source,
            filter_text: None,
            sort_ascending: true,
        }
    }

    pub fn set_filter_text(&mut self, text: Option<String>) {
        self.filter_text = text;
    }

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

pub trait TableModel: Send + Sync {
    /// Number of rows.
    fn row_count(&self) -> usize;
    /// Number of columns.
    fn column_count(&self) -> usize;
    /// Cell value at row/column.
    fn data(&self, row: usize, col: usize) -> Option<String>;
    /// Header label for a column.
    fn header(&self, col: usize) -> Option<String>;
}

/// Sort order for table view projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Asc,
    Desc,
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
    pub fn new(source: Arc<dyn TableModel>) -> Self {
        Self {
            source,
            filter_text: None,
            sort_column: None,
            sort_order: SortOrder::Asc,
        }
    }

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

pub struct TreeView {
    base: BaseWidget,
    /// Optional bound tree model.
    model: Option<Arc<dyn TreeModel>>,
    /// Fallback path storage used when no external model is bound.
    fallback_nodes: Vec<String>,
    /// View-side selected node index.
    selected_node: Option<usize>,
}

impl TreeView {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TreeView, geometry, "TreeView"),
            model: None,
            fallback_nodes: Vec::new(),
            selected_node: None,
        }
    }

    pub fn set_model(&mut self, model: Arc<dyn TreeModel>) {
        self.model = Some(model);
    }

    /// Backward-compatible imperative insertion when no external model is used.
    pub fn add_node(&mut self, node: impl Into<String>) {
        self.fallback_nodes.push(node.into());
    }

    pub fn node_count(&self) -> usize {
        self.model
            .as_ref()
            .map(|model| model.node_count())
            .unwrap_or(self.fallback_nodes.len())
    }

    pub fn node_path(&self, index: usize) -> Option<String> {
        self.model
            .as_ref()
            .and_then(|model| model.node_path(index))
            .or_else(|| self.fallback_nodes.get(index).cloned())
    }

    pub fn select_node(&mut self, index: usize) -> bool {
        if index < self.node_count() {
            self.selected_node = Some(index);
            true
        } else {
            false
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_node = None;
    }

    pub fn selected_node(&self) -> Option<usize> {
        self.selected_node
    }
}

impl_widget_delegate!(TreeView, base);

pub struct TableWidget {
    base: BaseWidget,
    /// Optional bound data model.
    model: Option<Arc<dyn TableModel>>,
    /// View-side selected row index.
    selected_row: Option<usize>,
}

impl TableWidget {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Table, geometry, "TableWidget"),
            model: None,
            selected_row: None,
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

    /// Read table header text by view column.
    pub fn header(&self, col: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.header(col))
    }

    /// Read table cell value by view row/column.
    pub fn cell(&self, row: usize, col: usize) -> Option<String> {
        self.model.as_ref().and_then(|m| m.data(row, col))
    }

    /// Select one row in the current view projection.
    pub fn select_row(&mut self, row: usize) -> bool {
        if row < self.row_count() {
            self.selected_row = Some(row);
            true
        } else {
            false
        }
    }

    /// Clear current row selection.
    pub fn clear_selection(&mut self) {
        self.selected_row = None;
    }

    /// Current selected row index.
    pub fn selected_row(&self) -> Option<usize> {
        self.selected_row
    }
}

impl_widget_delegate!(TableWidget, base);

simple_control!(GridWidget, WidgetKind::Grid);
simple_control!(ChartWidget, WidgetKind::Chart);
