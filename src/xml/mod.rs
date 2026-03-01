//! XML layout loading and lookup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::core::Rect;
use crate::widget::{
    Button, Canvas, ChartWidget, CheckBox, ComboBox, Dialog, GridWidget, GroupBox, Label,
    LineEdit, ListBox, Menu, MenuBar, Panel, PopupWindow, ProgressBar, RadioButton, ScrollBar,
    Slider, StackWidget, StatusBar, TabWidget, TableWidget, TextEdit, ToolBar, TreeView, Widget,
    Window,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlElement {
    /// Optional element id used for lookup/indexing.
    pub id: Option<String>,
    /// Widget class/type name.
    pub class: String,
    /// Arbitrary element attributes mapped as properties.
    pub properties: HashMap<String, String>,
    /// Nested child elements.
    pub children: Vec<XmlElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlLayout {
    pub root: XmlElement,
}

#[derive(Default)]
pub struct XmlLayoutLoader {
    layouts: HashMap<String, XmlLayout>,
}

pub struct WidgetRegistry {
    /// Runtime widget instances by generated id.
    widgets: HashMap<u64, Box<dyn Widget>>,
    /// Optional name/id index to generated widget id.
    index_by_name: HashMap<String, u64>,
}

/// Bound layout wrapper for mixed declarative + imperative UI workflows.
pub struct BoundLayout {
    /// Root widget id of instantiated layout tree.
    root_id: u64,
    /// Backing widget registry.
    registry: WidgetRegistry,
}

impl WidgetRegistry {
    /// Create empty widget registry.
    pub fn new() -> Self {
        Self {
            widgets: HashMap::new(),
            index_by_name: HashMap::new(),
        }
    }

    /// Insert widget and optional symbolic name.
    pub fn insert(&mut self, id_name: Option<&str>, widget: Box<dyn Widget>) -> u64 {
        let widget_id = widget.id();
        if let Some(name) = id_name {
            self.index_by_name.insert(name.to_string(), widget_id);
        }
        self.widgets.insert(widget_id, widget);
        widget_id
    }

    /// Resolve widget id by symbolic name.
    pub fn id_by_name(&self, name: &str) -> Option<u64> {
        self.index_by_name.get(name).copied()
    }

    pub fn widget(&self, id: u64) -> Option<&(dyn Widget + '_)> {
        if let Some(widget) = self.widgets.get(&id) {
            Some(widget.as_ref())
        } else {
            None
        }
    }

    pub fn widget_mut(&mut self, id: u64) -> Option<&mut (dyn Widget + '_)> {
        if let Some(widget) = self.widgets.get_mut(&id) {
            Some(widget.as_mut())
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.widgets.len()
    }

    /// Return true when symbolic id exists.
    pub fn contains_name(&self, name: &str) -> bool {
        self.index_by_name.contains_key(name)
    }

    /// Remove symbolic name mapping if present.
    pub fn remove_name(&mut self, name: &str) -> bool {
        self.index_by_name.remove(name).is_some()
    }

    /// Remove widget by id from registry and unlink name mapping.
    pub fn remove_widget(&mut self, id: u64) -> bool {
        if self.widgets.remove(&id).is_none() {
            return false;
        }
        self.index_by_name.retain(|_, mapped_id| *mapped_id != id);
        true
    }
}

impl BoundLayout {
    /// Create a bound layout from root id and registry.
    pub fn new(root_id: u64, registry: WidgetRegistry) -> Self {
        Self { root_id, registry }
    }

    /// Return root widget id.
    pub fn root_id(&self) -> u64 {
        self.root_id
    }

    /// Resolve widget id by declarative id name.
    pub fn id(&self, name: &str) -> Option<u64> {
        self.registry.id_by_name(name)
    }

    /// Get immutable widget by declarative id name.
    pub fn widget_by_name(&self, name: &str) -> Option<&(dyn Widget + '_)> {
        let id = self.id(name)?;
        self.registry.widget(id)
    }

    /// Get mutable widget by declarative id name.
    pub fn widget_by_name_mut(&mut self, name: &str) -> Option<&mut (dyn Widget + '_)> {
        let id = self.id(name)?;
        self.registry.widget_mut(id)
    }

    /// Update widget tooltip by declarative id name.
    pub fn set_tooltip_by_name(&mut self, name: &str, tooltip: impl Into<String>) -> Result<(), String> {
        let Some(widget) = self.widget_by_name_mut(name) else {
            return Err(format!("widget '{name}' not found"));
        };
        widget.set_tooltip(tooltip.into());
        Ok(())
    }

    /// Update visibility by declarative id name.
    pub fn set_visible_by_name(&mut self, name: &str, visible: bool) -> Result<(), String> {
        let Some(widget) = self.widget_by_name_mut(name) else {
            return Err(format!("widget '{name}' not found"));
        };
        if visible {
            widget.show();
        } else {
            widget.hide();
        }
        Ok(())
    }

    /// Attach an imperative widget under declarative parent and optionally bind a symbolic id.
    pub fn add_imperative_widget(
        &mut self,
        parent_name: &str,
        bind_name: Option<&str>,
        mut widget: Box<dyn Widget>,
    ) -> Result<u64, String> {
        let Some(parent_id) = self.id(parent_name) else {
            return Err(format!("parent widget '{parent_name}' not found"));
        };

        if let Some(name) = bind_name {
            if self.registry.contains_name(name) {
                return Err(format!("widget name '{name}' already exists"));
            }
        }

        widget.set_parent(Some(parent_id));
        let widget_id = self.registry.insert(bind_name, widget);

        if let Some(parent_widget) = self.registry.widget_mut(parent_id) {
            parent_widget.add_child(widget_id);
        }

        Ok(widget_id)
    }

    /// Remove a widget by declarative id name and detach from its parent if possible.
    pub fn remove_widget_by_name(&mut self, name: &str) -> Result<(), String> {
        let Some(widget_id) = self.id(name) else {
            return Err(format!("widget '{name}' not found"));
        };

        let parent_id = self
            .registry
            .widget(widget_id)
            .and_then(|widget| widget.parent());

        if let Some(parent_id) = parent_id {
            if let Some(parent_widget) = self.registry.widget_mut(parent_id) {
                parent_widget.remove_child(widget_id);
            }
        }

        let _ = self.registry.remove_name(name);
        let _ = self.registry.remove_widget(widget_id);
        Ok(())
    }

    /// Access underlying registry (immutable).
    pub fn registry(&self) -> &WidgetRegistry {
        &self.registry
    }

    /// Access underlying registry (mutable).
    pub fn registry_mut(&mut self) -> &mut WidgetRegistry {
        &mut self.registry
    }
}

impl XmlLayoutLoader {
    /// Create empty loader with no cached layouts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load layout from JSON or XML file path.
    pub fn load_layout(&mut self, name: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        if path.ends_with(".json") {
            let layout: XmlLayout = serde_json::from_str(&content)?;
            self.layouts.insert(name.to_string(), layout);
            return Ok(());
        }
        self.load_layout_from_xml_str(name, &content)
    }

    /// Parse and cache layout directly from XML string.
    pub fn load_layout_from_xml_str(&mut self, name: &str, xml: &str) -> Result<(), Box<dyn std::error::Error>> {
        let doc = roxmltree::Document::parse(xml)?;
        let root_node = doc.root_element();
        let layout = XmlLayout {
            root: Self::build_element(&root_node),
        };
        self.layouts.insert(name.to_string(), layout);
        Ok(())
    }

    pub fn get_layout(&self, name: &str) -> Option<&XmlLayout> {
        self.layouts.get(name)
    }

    /// Instantiate cached layout into concrete widget objects.
    pub fn instantiate_layout(&self, name: &str) -> Result<WidgetRegistry, String> {
        let layout = self
            .layouts
            .get(name)
            .ok_or_else(|| format!("layout '{name}' not found"))?;

        let mut registry = WidgetRegistry::new();
        self.instantiate_recursive(&layout.root, None, &mut registry)?;
        Ok(registry)
    }

    /// Instantiate cached layout and return bound wrapper for mixed declarative/imperative usage.
    pub fn instantiate_bound_layout(&self, name: &str) -> Result<BoundLayout, String> {
        let layout = self
            .layouts
            .get(name)
            .ok_or_else(|| format!("layout '{name}' not found"))?;

        let mut registry = WidgetRegistry::new();
        let root_id = self.instantiate_recursive(&layout.root, None, &mut registry)?;
        Ok(BoundLayout::new(root_id, registry))
    }

    /// Find element in cached layout by declared id.
    pub fn find_element_by_id(&self, layout_name: &str, id: &str) -> Option<&XmlElement> {
        self.layouts
            .get(layout_name)
            .and_then(|layout| Self::find_element_recursive(&layout.root, id))
    }

    fn build_element(node: &roxmltree::Node<'_, '_>) -> XmlElement {
        let mut properties = HashMap::new();
        for attr in node.attributes() {
            if attr.name() != "id" && attr.name() != "class" {
                properties.insert(attr.name().to_string(), attr.value().to_string());
            }
        }
        XmlElement {
            id: node.attribute("id").map(ToString::to_string),
            class: node.attribute("class").unwrap_or(node.tag_name().name()).to_string(),
            properties,
            children: node
                .children()
                .filter(|child| child.is_element())
                .map(|child| Self::build_element(&child))
                .collect(),
        }
    }

    fn find_element_recursive<'a>(element: &'a XmlElement, id: &str) -> Option<&'a XmlElement> {
        if element.id.as_deref() == Some(id) {
            return Some(element);
        }
        element
            .children
            .iter()
            .find_map(|child| Self::find_element_recursive(child, id))
    }

    fn instantiate_recursive(
        &self,
        element: &XmlElement,
        parent_id: Option<u64>,
        registry: &mut WidgetRegistry,
    ) -> Result<u64, String> {
        let mut widget = create_widget_from_element(element);
        widget.set_parent(parent_id);
        let this_id = registry.insert(element.id.as_deref(), widget);

        if let Some(parent) = parent_id {
            if let Some(parent_widget) = registry.widget_mut(parent) {
                parent_widget.add_child(this_id);
            }
        }

        for child in &element.children {
            self.instantiate_recursive(child, Some(this_id), registry)?;
        }

        Ok(this_id)
    }
}

fn create_widget_from_element(element: &XmlElement) -> Box<dyn Widget> {
    let rect = parse_rect(&element.properties);
    let class = element.class.to_lowercase();
    let text = element
        .properties
        .get("text")
        .cloned()
        .unwrap_or_default();
    let title = element
        .properties
        .get("title")
        .cloned()
        .unwrap_or_else(|| text.clone());

    match class.as_str() {
        "window" => Box::new(Window::new(title, rect)),
        "dialog" => Box::new(Dialog::new(rect)),
        "popupwindow" | "popup" => Box::new(PopupWindow::new(rect)),
        "button" => Box::new(Button::new(text, rect)),
        "checkbox" => Box::new(CheckBox::new(rect)),
        "radiobutton" => Box::new(RadioButton::new(rect)),
        "label" => Box::new(Label::new(text, rect)),
        "lineedit" => Box::new(LineEdit::new(rect)),
        "textedit" => Box::new(TextEdit::new(rect)),
        "combobox" => Box::new(ComboBox::new(rect)),
        "listbox" => Box::new(ListBox::new(rect)),
        "treeview" => Box::new(TreeView::new(rect)),
        "progressbar" => Box::new(ProgressBar::new(rect)),
        "slider" => Box::new(Slider::new(rect)),
        "scrollbar" => Box::new(ScrollBar::new(rect)),
        "panel" => Box::new(Panel::new(rect)),
        "groupbox" => Box::new(GroupBox::new(rect)),
        "tabwidget" => Box::new(TabWidget::new(rect)),
        "stackwidget" => Box::new(StackWidget::new(rect)),
        "menubar" => Box::new(MenuBar::new(rect)),
        "menu" => Box::new(Menu::new(rect)),
        "toolbar" => Box::new(ToolBar::new(rect)),
        "statusbar" => Box::new(StatusBar::new(rect)),
        "canvas" => Box::new(Canvas::new(rect)),
        "table" | "tablewidget" => Box::new(TableWidget::new(rect)),
        "grid" | "gridwidget" => Box::new(GridWidget::new(rect)),
        "chart" | "chartwidget" => Box::new(ChartWidget::new(rect)),
        _ => Box::new(Panel::new(rect)),
    }
}

fn parse_rect(properties: &HashMap<String, String>) -> Rect {
    let parse_i32 = |key: &str, default: i32| {
        properties
            .get(key)
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(default)
    };
    let parse_u32 = |key: &str, default: u32| {
        properties
            .get(key)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(default)
    };

    Rect {
        x: parse_i32("x", 0),
        y: parse_i32("y", 0),
        width: parse_u32("width", 120),
        height: parse_u32("height", 36),
    }
}


