//! XML layout loading and lookup.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use crate::core::{Color, Rect};
use crate::style::{Margin, Padding, WidgetStyle};
use crate::widget::{
    Button, Canvas, ChartWidget, CheckBox, ComboBox, Dialog, GridWidget, GroupBox, Label, LineEdit,
    ListBox, Menu, MenuBar, Panel, PopupWindow, ProgressBar, RadioButton, ScrollBar, Slider,
    StatusBar, TabWidget, TableModel, TableWidget, TextEdit, ToolBar, TreeModel, TreeView, Widget,
};
use crate::window::Window;
/// Declarative widget node parsed from XML/JSON layout sources.
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
/// Root XML layout document wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlLayout {
    /// Root element of this layout.
    pub root: XmlElement,
}
/// Loader/cache for named XML or JSON layouts.
#[derive(Default)]
pub struct XmlLayoutLoader {
    layouts: HashMap<String, XmlLayout>,
    table_models: HashMap<String, Arc<dyn TableModel>>,
    tree_models: HashMap<String, Arc<dyn TreeModel>>,
}
/// Runtime registry storing instantiated widgets and optional name index.
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
    /// Returns immutable widget by runtime id.
    pub fn widget(&self, id: u64) -> Option<&(dyn Widget + '_)> {
        if let Some(widget) = self.widgets.get(&id) {
            Some(widget.as_ref())
        } else {
            None
        }
    }
    /// Returns mutable widget by runtime id.
    pub fn widget_mut(&mut self, id: u64) -> Option<&mut (dyn Widget + '_)> {
        if let Some(widget) = self.widgets.get_mut(&id) {
            Some(widget.as_mut())
        } else {
            None
        }
    }
    /// Returns number of registered widgets.
    pub fn len(&self) -> usize {
        self.widgets.len()
    }
    /// Returns true if there are no registered widgets.
    pub fn is_empty(&self) -> bool {
        self.widgets.is_empty()
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
impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
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
    pub fn set_tooltip_by_name(
        &mut self,
        name: &str,
        tooltip: impl Into<String>,
    ) -> Result<(), String> {
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
    pub fn load_layout(
        &mut self,
        name: &str,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        if path.ends_with(".json") {
            let layout: XmlLayout = serde_json::from_str(&content)?;
            self.layouts.insert(name.to_string(), layout);
            return Ok(());
        }
        self.load_layout_from_xml_str(name, &content)
    }
    /// Parse and cache layout directly from XML string.
    pub fn load_layout_from_xml_str(
        &mut self,
        name: &str,
        xml: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let doc = roxmltree::Document::parse(xml)?;
        let root_node = doc.root_element();
        let layout = XmlLayout {
            root: Self::build_element(&root_node),
        };
        self.layouts.insert(name.to_string(), layout);
        Ok(())
    }
    /// Returns a cached layout by name.
    pub fn get_layout(&self, name: &str) -> Option<&XmlLayout> {
        self.layouts.get(name)
    }
    /// Register a named table model for declarative `model="..."` binding.
    pub fn register_table_model(&mut self, name: impl Into<String>, model: Arc<dyn TableModel>) {
        self.table_models.insert(name.into(), model);
    }
    /// Register a named tree model for declarative `model="..."` binding.
    pub fn register_tree_model(&mut self, name: impl Into<String>, model: Arc<dyn TreeModel>) {
        self.tree_models.insert(name.into(), model);
    }
    /// Returns true if a table model with `name` is registered.
    pub fn has_table_model(&self, name: &str) -> bool {
        self.table_models.contains_key(name)
    }
    /// Returns true if a tree model with `name` is registered.
    pub fn has_tree_model(&self, name: &str) -> bool {
        self.tree_models.contains_key(name)
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
            let attr_name: &str = attr.name();
            if attr_name != "id" && attr_name != "class" {
                properties.insert(attr_name.to_string(), attr.value().to_string());
            }
        }
        XmlElement {
            id: node.attribute("id").map(ToString::to_string),
            class: node
                .attribute("class")
                .unwrap_or(node.tag_name().name())
                .to_string(),
            properties,
            children: node
                .children()
                .filter(|child: &roxmltree::Node| child.is_element())
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
        let mut widget = self.create_widget_from_element(element);
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
    fn create_widget_from_element(&self, element: &XmlElement) -> Box<dyn Widget> {
        let rect = parse_rect(&element.properties);
        let class = element.class.to_lowercase();
        let text = element.properties.get("text").cloned().unwrap_or_default();
        let title = element
            .properties
            .get("title")
            .cloned()
            .unwrap_or_else(|| text.clone());
        let mut widget: Box<dyn Widget> = match class.as_str() {
            "window" => Box::new(Window::new(title, rect)),
            "dialog" => Box::new(Dialog::new("Dialog".to_string(), rect)),
            "popupwindow" | "popup" => Box::new(PopupWindow::new(rect)),
            "button" => Box::new(Button::new(text, rect)),
            "checkbox" => {
                let mut checkbox = CheckBox::new(rect);
                if let Some(checked) = parse_bool_property(&element.properties, "checked") {
                    checkbox.set_checked(checked);
                }
                Box::new(checkbox)
            }
            "radiobutton" => {
                let mut radio = RadioButton::new(rect);
                if let Some(checked) = parse_bool_property(&element.properties, "checked") {
                    radio.set_checked(checked);
                }
                Box::new(radio)
            }
            "label" => Box::new(Label::new(text, rect)),
            "lineedit" => {
                let mut line_edit = LineEdit::new(rect);
                if let Some(value) = element.properties.get("value") {
                    line_edit.set_text(value.clone());
                } else if let Some(text_value) = element.properties.get("text") {
                    line_edit.set_text(text_value.clone());
                }
                Box::new(line_edit)
            }
            "textedit" => {
                let mut text_edit = TextEdit::new(rect);
                if let Some(value) = element.properties.get("value") {
                    text_edit.set_text(value.clone());
                } else if let Some(text_value) = element.properties.get("text") {
                    text_edit.set_text(text_value.clone());
                }
                Box::new(text_edit)
            }
            "combobox" => {
                let mut combo = ComboBox::new(rect);
                if let Some(items) = element.properties.get("items") {
                    for item in items
                        .split(',')
                        .map(|part| part.trim())
                        .filter(|part| !part.is_empty())
                    {
                        combo.add_item(item.to_string());
                    }
                }
                if let Some(index) = parse_usize_property(&element.properties, "current_index") {
                    combo.set_current_index(index);
                }
                Box::new(combo)
            }
            "listbox" => {
                let mut list = ListBox::new(rect);
                if let Some(items) = element.properties.get("items") {
                    for item in items
                        .split(',')
                        .map(|part| part.trim())
                        .filter(|part| !part.is_empty())
                    {
                        list.add_item(item.to_string());
                    }
                }
                Box::new(list)
            }
            "treeview" => {
                let mut tree = TreeView::new(rect);
                if let Some(model_name) = resolve_model_name(&element.properties) {
                    if let Some(model) = self.tree_models.get(model_name) {
                        tree.set_model(Arc::clone(model));
                    }
                }
                Box::new(tree)
            }
            "progressbar" => {
                let mut progress = ProgressBar::new(rect);
                if let Some(value) = parse_u32_property(&element.properties, "value") {
                    progress.set_value(value);
                }
                Box::new(progress)
            }
            "slider" => {
                let mut slider = Slider::new(rect);
                if let Some(value) = parse_i32_property(&element.properties, "value") {
                    slider.set_value(value);
                }
                Box::new(slider)
            }
            "scrollbar" => {
                let mut scrollbar = ScrollBar::new(rect);
                if let Some(value) = parse_i32_property(&element.properties, "value") {
                    scrollbar.set_value(value);
                }
                Box::new(scrollbar)
            }
            "panel" => Box::new(Panel::new(rect)),
            "groupbox" => Box::new(GroupBox::new(rect)),
            "tabwidget" => Box::new(TabWidget::new(rect)),
            "menubar" => Box::new(MenuBar::new(rect)),
            "menu" => Box::new(Menu::new(rect)),
            "toolbar" => Box::new(ToolBar::new(rect)),
            "statusbar" => Box::new(StatusBar::new(rect)),
            "canvas" => Box::new(Canvas::new(rect)),
            "table" | "tablewidget" => {
                let mut table = TableWidget::new(rect);
                if let Some(model_name) = resolve_model_name(&element.properties) {
                    if let Some(model) = self.table_models.get(model_name) {
                        table.set_model(Arc::clone(model));
                    }
                }
                Box::new(table)
            }
            "grid" | "gridwidget" => Box::new(GridWidget::new(rect)),
            "chart" | "chartwidget" => Box::new(ChartWidget::new(rect)),
            _ => Box::new(Panel::new(rect)),
        };
        apply_common_properties(&mut widget, &element.properties);
        widget
    }
}
fn resolve_model_name(properties: &HashMap<String, String>) -> Option<&str> {
    properties
        .get("model")
        .or_else(|| properties.get("model_ref"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}
fn apply_common_properties(widget: &mut Box<dyn Widget>, properties: &HashMap<String, String>) {
    if let Some(tooltip) = properties.get("tooltip") {
        widget.set_tooltip(tooltip.clone());
    }
    if let Some(enabled) = parse_bool_property(properties, "enabled") {
        widget.set_enabled(enabled);
    }
    if let Some(visible) = parse_bool_property(properties, "visible") {
        if visible {
            widget.show();
        } else {
            widget.hide();
        }
    }
    let style = parse_widget_style(properties);
    if style != WidgetStyle::default() {
        widget.set_style(style);
    }
}
fn parse_widget_style(properties: &HashMap<String, String>) -> WidgetStyle {
    let background_color = parse_color_property(properties, "style.background")
        .or_else(|| parse_color_property(properties, "background_color"));
    let text_color = parse_color_property(properties, "style.text")
        .or_else(|| parse_color_property(properties, "text_color"));
    let border_color = parse_color_property(properties, "style.border")
        .or_else(|| parse_color_property(properties, "border_color"));
    let border_width = parse_u32_property(properties, "style.border_width")
        .or_else(|| parse_u32_property(properties, "border_width"))
        .unwrap_or(0);
    let border_radius = parse_u32_property(properties, "style.border_radius")
        .or_else(|| parse_u32_property(properties, "border_radius"))
        .unwrap_or(0);
    let padding = parse_u32_property(properties, "style.padding")
        .or_else(|| parse_u32_property(properties, "padding"))
        .map(Padding::all)
        .unwrap_or_default();
    let margin = parse_u32_property(properties, "style.margin")
        .or_else(|| parse_u32_property(properties, "margin"))
        .map(Margin::all)
        .unwrap_or_default();
    WidgetStyle {
        background_color,
        text_color,
        border_color,
        border_width,
        border_radius,
        padding,
        margin,
        ..Default::default()
    }
}
fn parse_bool_property(properties: &HashMap<String, String>, key: &str) -> Option<bool> {
    let value = properties.get(key)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
fn parse_u32_property(properties: &HashMap<String, String>, key: &str) -> Option<u32> {
    properties.get(key)?.trim().parse::<u32>().ok()
}
fn parse_usize_property(properties: &HashMap<String, String>, key: &str) -> Option<usize> {
    properties.get(key)?.trim().parse::<usize>().ok()
}
fn parse_i32_property(properties: &HashMap<String, String>, key: &str) -> Option<i32> {
    properties.get(key)?.trim().parse::<i32>().ok()
}
fn parse_color_property(properties: &HashMap<String, String>, key: &str) -> Option<Color> {
    Color::parse_hex(properties.get(key)?)
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
    Rect::new(
        parse_i32("x", 0),
        parse_i32("y", 0),
        parse_u32("width", 120),
        parse_u32("height", 36),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_instantiation_applies_common_widget_properties() {
        let mut loader = XmlLayoutLoader::new();
        loader
            .load_layout_from_xml_str(
                "layout",
                r##"
                <window id="root" class="window" x="0" y="0" width="320" height="200" title="root">
                    <button
                        id="btn"
                        class="button"
                        x="10" y="10" width="80" height="24"
                        text="Click"
                        tooltip="tip"
                        visible="false"
                        enabled="false"
                        style.background="#112233"
                        style.text="#AABBCC"
                        style.border="#334455"
                        style.border_width="2"
                        style.border_radius="4"
                        style.padding="3"
                        style.margin="5" />
                </window>
                    "##,
            )
            .expect("load xml");
        let registry = loader.instantiate_layout("layout").expect("instantiate");
        let button_id = registry.id_by_name("btn").expect("button id exists");
        let button = registry.widget(button_id).expect("button exists");
        assert_eq!(button.tooltip(), "tip");
        assert!(!button.is_visible());
        assert!(!button.is_enabled());
        let style = button.style();
        assert_eq!(
            style.background_color,
            Some(Color {
                r: 0x11,
                g: 0x22,
                b: 0x33,
                a: 255
            })
        );
        assert_eq!(
            style.text_color,
            Some(Color {
                r: 0xAA,
                g: 0xBB,
                b: 0xCC,
                a: 255
            })
        );
        assert_eq!(
            style.border_color,
            Some(Color {
                r: 0x33,
                g: 0x44,
                b: 0x55,
                a: 255
            })
        );
        assert_eq!(style.border_width, 2);
        assert_eq!(style.border_radius, 4);
        assert_eq!(style.padding, Padding::all(3));
        assert_eq!(style.margin, Margin::all(5));
    }
    #[test]
    fn xml_instantiation_accepts_short_and_alpha_hex_colors() {
        let mut loader = XmlLayoutLoader::new();
        loader
            .load_layout_from_xml_str(
                "hex_layout",
                r##"
                <window id="root" class="window" x="0" y="0" width="320" height="200" title="root">
                    <button
                        id="btn"
                        class="button"
                        x="10" y="10" width="80" height="24"
                        style.background="#abc"
                        style.text="#11223344"
                        style.border="#0F08" />
                </window>
                    "##,
            )
            .expect("load xml");
        let registry = loader
            .instantiate_layout("hex_layout")
            .expect("instantiate");
        let button_id = registry.id_by_name("btn").expect("button id exists");
        let button = registry.widget(button_id).expect("button exists");
        let style = button.style();
        assert_eq!(
            style.background_color,
            Some(Color::rgba(0xAA, 0xBB, 0xCC, 0xFF))
        );
        assert_eq!(style.text_color, Some(Color::rgba(0x11, 0x22, 0x33, 0x44)));
        assert_eq!(
            style.border_color,
            Some(Color::rgba(0x00, 0xFF, 0x00, 0x88))
        );
    }
    #[test]
    fn xml_instantiation_applies_state_value_properties() {
        let mut loader = XmlLayoutLoader::new();
        loader
            .load_layout_from_xml_str(
                "state_layout",
                r#"
                <window id="root" class="window" x="0" y="0" width="320" height="200" title="root">
                    <checkbox id="check" class="checkbox" checked="true" x="10" y="10" width="20" height="20" />
                    <lineedit id="line" class="lineedit" value="Alice" x="10" y="40" width="100" height="24" />
                </window>
                "#,
            )
            .expect("load xml");
        let registry = loader
            .instantiate_layout("state_layout")
            .expect("instantiate");
        let check_id = registry.id_by_name("check").expect("check id");
        let line_id = registry.id_by_name("line").expect("line id");
        let check = registry.widget(check_id).expect("check exists");
        let line = registry.widget(line_id).expect("line exists");
        assert!(check.is_visible());
        assert!(line.is_enabled());
    }
    #[test]
    fn xml_loader_registers_table_and_tree_models() {
        let mut loader = XmlLayoutLoader::new();
        loader.register_table_model(
            "main_table",
            Arc::new(crate::widget::VecTableModel::new(
                vec!["Name".to_string()],
                vec![vec!["Alice".to_string()]],
            )),
        );
        loader.register_tree_model(
            "main_tree",
            Arc::new(crate::widget::VecTreeModel::new(vec!["Root".to_string()])),
        );
        assert!(loader.has_table_model("main_table"));
        assert!(loader.has_tree_model("main_tree"));
        assert!(!loader.has_table_model("missing"));
        assert!(!loader.has_tree_model("missing"));
    }
    #[test]
    fn xml_instantiation_with_model_binding_attributes_succeeds() {
        let mut loader = XmlLayoutLoader::new();
        loader.register_table_model(
            "users",
            Arc::new(crate::widget::VecTableModel::new(
                vec!["Name".to_string()],
                vec![vec!["Alice".to_string()], vec!["Bob".to_string()]],
            )),
        );
        loader.register_tree_model(
            "filesystem",
            Arc::new(crate::widget::VecTreeModel::new(vec![
                "/".to_string(),
                "/tmp".to_string(),
            ])),
        );
        loader
            .load_layout_from_xml_str(
                "model_layout",
                r#"
                <window id="root" class="window" x="0" y="0" width="320" height="200" title="root">
                    <table id="table" class="table" x="10" y="10" width="120" height="60" model="users" />
                    <treeview id="tree" class="treeview" x="10" y="80" width="120" height="60" model_ref="filesystem" />
                </window>
                "#,
            )
            .expect("load xml");
        let registry = loader
            .instantiate_layout("model_layout")
            .expect("instantiate");
        assert!(registry.id_by_name("table").is_some());
        assert!(registry.id_by_name("tree").is_some());
    }
    #[test]
    fn resolve_model_name_prefers_model_then_model_ref() {
        let mut properties = HashMap::new();
        properties.insert("model_ref".to_string(), "fallback".to_string());
        properties.insert("model".to_string(), "primary".to_string());
        assert_eq!(resolve_model_name(&properties), Some("primary"));
        properties.remove("model");
        assert_eq!(resolve_model_name(&properties), Some("fallback"));
    }
}
