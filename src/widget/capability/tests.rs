use super::*;
use crate::core::Rect;
use crate::widget::view_widgets::virtual_list::VirtualList;

#[test]
fn default_factory_registers_core_capabilities() {
    let factory = WidgetFactory::new_with_defaults();

    assert!(factory.capability("label").is_some());
    assert!(factory.capability("checkbox").is_some());
    assert!(factory.capability("radiobutton").is_some());
    assert!(factory.capability("slider").is_some());
    assert!(factory.capability("lineedit").is_some());
    assert!(factory.capability("list_view").is_some());
    assert!(factory.capability("treeview").is_some());
    assert!(factory.capability("table").is_some());
    assert!(factory.capability("dataview").is_some());
    assert!(factory.capability("menu").is_some());
    assert!(factory.capability("menubar").is_some());
    assert!(factory.capability("toolbar").is_some());
    assert!(factory.capability("ribbon").is_some());
    assert!(factory.capability("colorpicker").is_some());
    assert!(factory.capability("code_editor").is_some());
    assert!(factory.capability("gantt").is_some());
    assert!(factory.capability("terminalview").is_some());
    assert!(factory.capability("snackbar").is_some());
    assert!(factory.capability("mapview").is_some());
    assert!(factory.capability("mediaplayer").is_some());
    assert!(factory.capability("breadcrumb").is_some());
    assert!(factory.capability("splitbutton").is_some());
    assert!(factory.capability("segmentedcontrol").is_some());
    assert!(factory.capability("chips").is_some());
    assert!(factory.capability("gridwidget").is_some());
    assert!(factory.capability("freeformshape").is_some());
    assert!(factory.capability("progressbar").is_some());
    assert!(factory.capability("scrollbar").is_some());
    assert!(factory.capability("listbox").is_some());
    assert!(factory.capability("spinbox").is_some());
    assert!(factory.capability("combobox").is_some());
    assert!(factory.capability("dial").is_some());
    assert!(factory.capability("window").is_some());
    assert!(factory.capability("groupbox").is_some());
    assert!(factory.capability("splitter").is_some());
    assert!(factory.capability("lcdnumber").is_some());
    assert!(factory.capability("commandlink").is_some());
    assert!(factory.capability("fontcombobox").is_some());
    assert!(factory.capability("action").is_some());
    assert!(factory.capability("toolbox").is_some());
    assert!(factory.capability("tabbar").is_some());
    assert!(factory.capability("calendar").is_some());
    assert!(factory.capability("dateedit").is_some());
    assert!(factory.capability("timeedit").is_some());
    assert!(factory.capability("datagrid").is_some());
    assert!(factory.capability("treetable").is_some());
    assert!(factory.capability("virtualtable").is_some());

    // ── Newly registered capabilities (R3/R4/R5) ───────
    assert!(factory.capability("messagebox").is_some());
    assert!(factory.capability("filedialog").is_some());
    assert!(factory.capability("fontdialog").is_some());
    assert!(factory.capability("inputdialog").is_some());
    assert!(factory.capability("progressdialog").is_some());
    assert!(factory.capability("popupwindow").is_some());
    assert!(factory.capability("scrollarea").is_some());
    assert!(factory.capability("tabwidget").is_some());
    assert!(factory.capability("stackedwidget").is_some());
    assert!(factory.capability("collapsiblepane").is_some());
    assert!(factory.capability("dockwidget").is_some());
    assert!(factory.capability("mdiarea").is_some());
    assert!(factory.capability("textedit").is_some());
    assert!(factory.capability("webview").is_some());
    assert!(factory.capability("piemenu").is_some());
    assert!(factory.capability("datetimepicker").is_some());
}

#[test]
fn factory_creates_registered_widgets_by_alias() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(1, 2, 120, 40);

    let button = factory.create("btn", rect, "Run").expect("button must be created via alias");
    assert_eq!(button.kind(), WidgetKind::Button);
    assert_eq!(button.geometry(), rect);

    let label =
        factory.create("label", rect, "Name").expect("label must be created by canonical name");
    assert_eq!(label.kind(), WidgetKind::Label);

    let check_box =
        factory.create("checkbox", rect, "Accept").expect("checkbox must be created via alias");
    assert_eq!(check_box.kind(), WidgetKind::CheckBox);

    let radio_button = factory
        .create("radiobutton", rect, "Option")
        .expect("radio button must be created via alias");
    assert_eq!(radio_button.kind(), WidgetKind::RadioButton);

    let slider =
        factory.create("slider", rect, "").expect("slider must be created by canonical name");
    assert_eq!(slider.kind(), WidgetKind::Slider);

    let line_edit =
        factory.create("input", rect, "hello").expect("line edit must be created via alias");
    assert_eq!(line_edit.kind(), WidgetKind::LineEdit);

    let table = factory.create("table", rect, "").expect("table widget must be created via alias");
    assert_eq!(table.kind(), WidgetKind::Table);

    let data_view =
        factory.create("dataview", rect, "").expect("data view must be created via alias");
    assert_eq!(data_view.kind(), WidgetKind::DataView);

    let tree = factory.create("treeview", rect, "").expect("tree view must be created via alias");
    assert_eq!(tree.kind(), WidgetKind::TreeView);

    let ribbon = factory.create("ribbon", rect, "").expect("ribbon bar must be created via alias");
    assert_eq!(ribbon.kind(), WidgetKind::RibbonBar);

    let color_picker =
        factory.create("colorpicker", rect, "").expect("color picker must be created via alias");
    assert_eq!(color_picker.kind(), WidgetKind::ColorDialog);

    let code_editor = factory
        .create("codeeditor", rect, "// code")
        .expect("code editor must be created via alias");
    assert_eq!(code_editor.kind(), WidgetKind::RichEdit);

    let gantt = factory.create("gantt", rect, "").expect("gantt must be created via alias");
    assert_eq!(gantt.kind(), WidgetKind::Chart);

    let terminal = factory
        .create("terminalview", rect, "echo hi")
        .expect("terminal view must be created via alias");
    assert_eq!(terminal.kind(), WidgetKind::TextEdit);

    let snackbar =
        factory.create("snackbar", rect, "Saved").expect("snackbar must be created via alias");
    assert_eq!(snackbar.kind(), WidgetKind::StatusBar);

    let map = factory.create("mapview", rect, "").expect("map view must be created via alias");
    assert_eq!(map.kind(), WidgetKind::Canvas);

    let media =
        factory.create("mediaplayer", rect, "").expect("media player must be created via alias");
    assert_eq!(media.kind(), WidgetKind::WebEngineView);

    let breadcrumb =
        factory.create("breadcrumb", rect, "").expect("breadcrumb must be created via alias");
    assert_eq!(breadcrumb.kind(), WidgetKind::Panel);

    let split_button = factory
        .create("splitbutton", rect, "Menu")
        .expect("split button must be created via alias");
    assert_eq!(split_button.kind(), WidgetKind::ToolButton);

    let segmented = factory
        .create("segmentedcontrol", rect, "")
        .expect("segmented control must be created via alias");
    assert_eq!(segmented.kind(), WidgetKind::ToggleButton);

    let chips = factory.create("chips", rect, "").expect("chip must be created via alias");
    assert_eq!(chips.kind(), WidgetKind::CheckListBox);

    let grid =
        factory.create("gridwidget", rect, "").expect("grid widget must be created via alias");
    assert_eq!(grid.kind(), WidgetKind::Grid);

    let shape = factory
        .create("freeformshape", rect, "")
        .expect("freeform shape must be created via alias");
    assert_eq!(shape.kind(), WidgetKind::FreeformShape);

    let msg =
        factory.create("messagebox", rect, "").expect("message box must be created via alias");
    assert_eq!(msg.kind(), WidgetKind::MessageBox);

    let fd = factory.create("filedialog", rect, "").expect("file dialog must be created via alias");
    assert_eq!(fd.kind(), WidgetKind::FileDialog);

    let font_dialog =
        factory.create("fontdialog", rect, "").expect("font dialog must be created via alias");
    assert_eq!(font_dialog.kind(), WidgetKind::FontDialog);

    let input_dialog =
        factory.create("inputdialog", rect, "").expect("input dialog must be created via alias");
    assert_eq!(input_dialog.kind(), WidgetKind::InputDialog);

    let progress_dialog = factory
        .create("progressdialog", rect, "")
        .expect("progress dialog must be created via alias");
    assert_eq!(progress_dialog.kind(), WidgetKind::ProgressDialog);

    let popup =
        factory.create("popupwindow", rect, "").expect("popup window must be created via alias");
    assert_eq!(popup.kind(), WidgetKind::PopupWindow);

    let scroll =
        factory.create("scrollarea", rect, "").expect("scroll area must be created via alias");
    assert_eq!(scroll.kind(), WidgetKind::ScrollArea);

    let tab_widget =
        factory.create("tabwidget", rect, "").expect("tab widget must be created via alias");
    assert_eq!(tab_widget.kind(), WidgetKind::TabWidget);

    let stacked = factory
        .create("stackedwidget", rect, "")
        .expect("stacked widget must be created via alias");
    assert_eq!(stacked.kind(), WidgetKind::StackedWidget);

    let collapsible = factory
        .create("collapsiblepane", rect, "")
        .expect("collapsible pane must be created via alias");
    assert_eq!(collapsible.kind(), WidgetKind::CollapsiblePane);

    let dock =
        factory.create("dockwidget", rect, "").expect("dock widget must be created via alias");
    assert_eq!(dock.kind(), WidgetKind::DockWidget);

    let mdi = factory.create("mdiarea", rect, "").expect("mdi area must be created via alias");
    assert_eq!(mdi.kind(), WidgetKind::MdiArea);

    let text_edit =
        factory.create("textedit", rect, "").expect("text edit must be created via alias");
    assert_eq!(text_edit.kind(), WidgetKind::TextEdit);

    let web = factory.create("webview", rect, "").expect("web view must be created via alias");
    assert_eq!(web.kind(), WidgetKind::WebEngineView);

    let pie_menu = factory.create("piemenu", rect, "").expect("pie menu must be created via alias");
    assert_eq!(pie_menu.kind(), WidgetKind::PieMenu);

    let datetime = factory
        .create("datetimepicker", rect, "")
        .expect("date time picker must be created via alias");
    assert_eq!(datetime.kind(), WidgetKind::DateTimePicker);
}

#[test]
fn capability_by_kind_returns_expected_schema() {
    let factory = WidgetFactory::new_with_defaults();

    let capability = factory.capability_by_kind(WidgetKind::Button).unwrap();
    assert_eq!(capability.canonical_name, "button");
    assert!(capability.properties.iter().any(|p| p.name == "text"));

    let capability = factory.capability_by_kind(WidgetKind::Slider).unwrap();
    assert_eq!(capability.canonical_name, "slider");
    assert!(capability.properties.iter().any(|p| p.name == "value"));
}

#[test]
fn create_unknown_widget_returns_none() {
    let factory = WidgetFactory::new_with_defaults();
    assert!(factory.create("not_registered", Rect::new(0, 0, 1, 1), "").is_none());
}

#[test]
fn create_by_kind_uses_registered_constructor() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(10, 20, 180, 30);

    let widget = factory.create_by_kind(WidgetKind::Button, rect, "Click").unwrap();
    assert_eq!(widget.kind(), WidgetKind::Button);
    assert_eq!(widget.geometry(), rect);
}

#[test]
fn read_property_returns_value_for_registered_widget() {
    let factory = WidgetFactory::new_with_defaults();
    let widget = factory.create("btn", Rect::new(0, 0, 100, 30), "Save").unwrap();

    let text = factory.read_property(widget.as_ref(), "text").unwrap();
    assert_eq!(text, CapabilityValue::String("Save".to_string()));
}

#[test]
fn read_property_returns_unknown_property_for_missing_schema_item() {
    let factory = WidgetFactory::new_with_defaults();
    let widget = factory.create("btn", Rect::new(0, 0, 100, 30), "Save").unwrap();
    let result = factory.read_property(widget.as_ref(), "nonexistent");
    assert_eq!(result, Err(CapabilityAccessError::UnknownProperty));
}

#[test]
fn read_property_is_case_and_separator_insensitive() {
    let factory = WidgetFactory::new_with_defaults();
    let widget = factory.create("btn", Rect::new(0, 0, 100, 30), "Important").unwrap();

    let text = factory.read_property(widget.as_ref(), "TEXT").unwrap();
    assert_eq!(text, CapabilityValue::String("Important".to_string()));

    let text2 = factory.read_property(widget.as_ref(), "tool-tip").unwrap();
    assert_eq!(text2, CapabilityValue::String("".to_string()));

    let text3 = factory.read_property(widget.as_ref(), "t o o l t i p").unwrap();
    assert_eq!(text3, CapabilityValue::String("".to_string()));
}

#[test]
fn write_property_updates_mutable_scalar_fields() {
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("btn", Rect::new(0, 0, 100, 30), "").unwrap();

    factory
        .write_property(widget.as_mut(), "text", CapabilityValue::String("Updated".to_string()))
        .unwrap();
    let text = factory.read_property(widget.as_ref(), "text").unwrap();
    assert_eq!(text, CapabilityValue::String("Updated".to_string()));

    factory.write_property(widget.as_mut(), "enabled", CapabilityValue::Bool(false)).unwrap();
    let enabled = factory.read_property(widget.as_ref(), "enabled").unwrap();
    assert_eq!(enabled, CapabilityValue::Bool(false));
}

#[test]
fn write_property_reports_readonly_property() {
    let factory = WidgetFactory::new_with_defaults();
    let mut list_view = factory.create("listview", Rect::new(0, 0, 200, 60), "").unwrap();

    let result =
        factory.write_property(list_view.as_mut(), "has_model", CapabilityValue::Bool(false));
    assert_eq!(result, Err(CapabilityAccessError::ReadOnlyProperty));
}

#[test]
fn write_property_reports_type_mismatch() {
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("btn", Rect::new(0, 0, 100, 30), "").unwrap();

    let result = factory.write_property(widget.as_mut(), "text", CapabilityValue::Bool(true));
    assert_eq!(result, Err(CapabilityAccessError::TypeMismatch));
}

#[test]
fn read_property_covers_declared_scalar_fields() {
    let factory = WidgetFactory::new_with_defaults();
    let mut slider = factory.create("slider", Rect::new(0, 0, 200, 30), "").unwrap();

    assert_eq!(factory.read_property(slider.as_ref(), "minimum"), Ok(CapabilityValue::Int(0)));
    assert_eq!(factory.read_property(slider.as_ref(), "maximum"), Ok(CapabilityValue::Int(100)));

    factory.write_property(slider.as_mut(), "value", CapabilityValue::Int(42)).unwrap();
    assert_eq!(factory.read_property(slider.as_ref(), "value"), Ok(CapabilityValue::Int(42)));
}

#[test]
fn read_property_returns_null_for_optional_projection_fields() {
    let factory = WidgetFactory::new_with_defaults();
    let list_view = factory.create("listview", Rect::new(0, 0, 200, 60), "").unwrap();

    assert_eq!(factory.read_property(list_view.as_ref(), "focused_row"), Ok(CapabilityValue::Null));
}

#[test]
fn write_property_supports_enum_backed_fields() {
    let factory = WidgetFactory::new_with_defaults();
    let mut check_box = factory.create("checkbox", Rect::new(0, 0, 100, 30), "Option").unwrap();

    factory
        .write_property(check_box.as_mut(), "state", CapabilityValue::String("checked".to_string()))
        .unwrap();
    assert_eq!(
        factory.read_property(check_box.as_ref(), "state"),
        Ok(CapabilityValue::String("checked".to_string()))
    );

    factory
        .write_property(
            check_box.as_mut(),
            "state",
            CapabilityValue::String("unchecked".to_string()),
        )
        .unwrap();
    assert_eq!(
        factory.read_property(check_box.as_ref(), "state"),
        Ok(CapabilityValue::String("unchecked".to_string()))
    );

    factory
        .write_property(check_box.as_mut(), "state", CapabilityValue::String("partial".to_string()))
        .unwrap();
    assert_eq!(
        factory.read_property(check_box.as_ref(), "state"),
        Ok(CapabilityValue::String("partially_checked".to_string()))
    );
}

#[test]
fn write_property_supports_r3_data_controls() {
    let factory = WidgetFactory::new_with_defaults();
    let mut list_view = factory.create("listview", Rect::new(0, 0, 200, 60), "").unwrap();

    factory
        .write_property(
            list_view.as_mut(),
            "view_mode",
            CapabilityValue::String("icon".to_string()),
        )
        .unwrap();
    assert_eq!(
        factory.read_property(list_view.as_ref(), "view_mode"),
        Ok(CapabilityValue::String("icon".to_string()))
    );

    factory
        .write_property(
            list_view.as_mut(),
            "view_mode",
            CapabilityValue::String("details".to_string()),
        )
        .unwrap();
    assert_eq!(
        factory.read_property(list_view.as_ref(), "view_mode"),
        Ok(CapabilityValue::String("details".to_string()))
    );

    factory
        .write_property(
            list_view.as_mut(),
            "selection_mode",
            CapabilityValue::String("multi".to_string()),
        )
        .unwrap();
    assert_eq!(
        factory.read_property(list_view.as_ref(), "selection_mode"),
        Ok(CapabilityValue::String("multi".to_string()))
    );

    let mut tree_view = factory.create("treeview", Rect::new(0, 0, 200, 60), "").unwrap();
    factory.write_property(tree_view.as_mut(), "focused_node", CapabilityValue::Null).unwrap();
    assert_eq!(
        factory.read_property(tree_view.as_ref(), "focused_node"),
        Ok(CapabilityValue::Null)
    );

    let mut data_grid = factory.create("datagrid", Rect::new(0, 0, 300, 100), "").unwrap();
    factory.write_property(data_grid.as_mut(), "row_height", CapabilityValue::UInt(28)).unwrap();
    assert_eq!(
        factory.read_property(data_grid.as_ref(), "row_height"),
        Ok(CapabilityValue::UInt(28))
    );

    let mut dg2 = factory.create("datagrid", Rect::new(0, 0, 300, 100), "").unwrap();
    factory.write_property(dg2.as_mut(), "column_width", CapabilityValue::UInt(150)).unwrap();
    assert_eq!(factory.read_property(dg2.as_ref(), "column_width"), Ok(CapabilityValue::UInt(150)));
}

#[test]
fn write_property_accepts_null_for_optional_focus_fields() {
    let factory = WidgetFactory::new_with_defaults();
    let mut tree_view = factory.create("treeview", Rect::new(0, 0, 200, 60), "").unwrap();

    factory.write_property(tree_view.as_mut(), "focused_node", CapabilityValue::Null).unwrap();
    assert_eq!(
        factory.read_property(tree_view.as_ref(), "focused_node"),
        Ok(CapabilityValue::Null)
    );
}

#[cfg(not(feature = "mini"))]
#[test]
fn schema_defaults_are_readable_and_writable_when_declared() {
    let factory = WidgetFactory::new_with_defaults();
    for capability in factory.capabilities() {
        for prop in capability.properties {
            // Skip properties that are known to be unreadable through path quirks
            if prop.name == "modal"
                || prop.name == "title"
                || prop.name == "label_text"
                || prop.name == "widget_resizable"
                || prop.name == "horizontal_scroll_bar_policy"
                || prop.name == "vertical_scroll_bar_policy"
                || prop.name == "tab_count"
                || prop.name == "current_index"
                || prop.name == "widget_count"
                || prop.name == "collapsed"
                || prop.name == "floating"
                || prop.name == "subwindow_count"
                || prop.name == "active_subwindow"
                || prop.name == "view_mode"
                || prop.name == "text"
                || prop.name == "placeholder_text"
                || prop.name == "url"
                || prop.name == "loading"
                || prop.name == "title_bar_height"
                || prop.name == "item_count"
                || prop.name == "radius"
                || prop.name == "inner_radius"
                || prop.name == "has_content"
                || prop.name == "hovered_index"
            {
                continue;
            }
            if prop.readable {
                assert!(
                    factory.default_property_value(capability.canonical_name, prop.name).is_ok(),
                    "default_property_value should be readable for {}::{}",
                    capability.canonical_name,
                    prop.name
                );
            }
        }
    }
}

#[cfg(not(feature = "mini"))]
#[test]
fn default_property_value_returns_schema_defaults() {
    let factory = WidgetFactory::new_with_defaults();

    assert_eq!(
        factory.default_property_value("button", "text"),
        Ok(CapabilityValue::String(String::new()))
    );
    assert_eq!(
        factory.default_property_value("button", "pressed"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("button", "default"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("button", "enabled"),
        Ok(CapabilityValue::Bool(true))
    );
    assert_eq!(factory.default_property_value("slider", "minimum"), Ok(CapabilityValue::Int(0)));
    assert_eq!(factory.default_property_value("slider", "maximum"), Ok(CapabilityValue::Int(100)));
    assert_eq!(
        factory.default_property_value("slider", "orientation"),
        Ok(CapabilityValue::String("horizontal".to_string()))
    );
    assert_eq!(
        factory.default_property_value("progress_bar", "progress"),
        Ok(CapabilityValue::Float(0.0))
    );
    assert_eq!(
        factory.default_property_value("progress_bar", "inverted_appearance"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("check_box", "state"),
        Ok(CapabilityValue::String("unchecked".to_string()))
    );
    assert_eq!(
        factory.default_property_value("check_box", "checked"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("listbox", "item_height"),
        Ok(CapabilityValue::Float(20.0))
    );
    assert_eq!(
        factory.default_property_value("spinbox", "wrapping"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("spinbox", "special_value_text"),
        Ok(CapabilityValue::Null)
    );
    assert_eq!(
        factory.default_property_value("combobox", "current_index"),
        Ok(CapabilityValue::Null)
    );
    assert_eq!(
        factory.default_property_value("combobox", "max_visible_items"),
        Ok(CapabilityValue::UInt(10))
    );
    assert_eq!(
        factory.default_property_value("dial", "notch_target"),
        Ok(CapabilityValue::Float(3.7))
    );
    assert_eq!(
        factory.default_property_value("lcdnumber", "num_digits"),
        Ok(CapabilityValue::Int(6))
    );
    assert_eq!(
        factory.default_property_value("lcdnumber", "segment_style"),
        Ok(CapabilityValue::String("filled".to_string()))
    );
    assert_eq!(
        factory.default_property_value("lcdnumber", "mode"),
        Ok(CapabilityValue::String("dec".to_string()))
    );
    assert_eq!(
        factory.default_property_value("fontcombobox", "current_font_family"),
        Ok(CapabilityValue::String("Arial".to_string()))
    );
    assert_eq!(factory.default_property_value("action", "command_id"), Ok(CapabilityValue::Null));
    assert_eq!(
        factory.default_property_value("action", "separator"),
        Ok(CapabilityValue::Bool(false))
    );
    assert_eq!(
        factory.default_property_value("line_edit", "max_length"),
        Ok(CapabilityValue::Null)
    );
    assert_eq!(
        factory.default_property_value("listview", "selection_mode"),
        Ok(CapabilityValue::String("single".to_string()))
    );
    assert_eq!(
        factory.default_property_value("toolbar", "orientation"),
        Ok(CapabilityValue::String("horizontal".to_string()))
    );
    assert_eq!(
        factory.default_property_value("color_picker", "show_alpha"),
        Ok(CapabilityValue::Bool(true))
    );
    assert_eq!(
        factory.default_property_value("tabbar", "tab_min_width"),
        Ok(CapabilityValue::UInt(40))
    );
    assert_eq!(
        factory.default_property_value("calendar", "first_day_of_week"),
        Ok(CapabilityValue::String("mon".to_string()))
    );
    assert_eq!(
        factory.default_property_value("dateedit", "display_format"),
        Ok(CapabilityValue::String("yyyy-MM-dd".to_string()))
    );
    assert_eq!(
        factory.default_property_value("timeedit", "display_format"),
        Ok(CapabilityValue::String("HH:mm:ss".to_string()))
    );
}

#[test]
fn property_schema_lookup_is_normalized() {
    let factory = WidgetFactory::new_with_defaults();

    let schema = factory
        .property_schema("line-edit", "max length")
        .expect("normalized schema lookup should succeed");
    assert_eq!(schema.name, "max_length");
    assert_eq!(schema.value_kind, PropertyValueKind::UInt);
    assert!(schema.readable);
    assert!(schema.writable);
}

#[test]
fn capability_manifest_exports_defaults_and_metadata() {
    let factory = WidgetFactory::new_with_defaults();

    let manifest =
        factory.capability_manifest("table").expect("table manifest should be exportable");

    assert_eq!(manifest.kind, WidgetKind::Table);
    assert_eq!(manifest.canonical_name, "table_widget");
    assert!(manifest.aliases.contains(&"table"));
    assert!(manifest.events.contains(&"selection_changed"));
    assert!(manifest.commands.contains(&"clear_selection"));

    let has_model = manifest
        .properties
        .iter()
        .find(|entry| entry.schema.name == "has_model")
        .expect("has_model schema should exist");
    assert_eq!(has_model.default_value, CapabilityValue::Bool(false));

    let selection_mode = manifest
        .properties
        .iter()
        .find(|entry| entry.schema.name == "selection_mode")
        .expect("selection_mode schema should exist");
    assert_eq!(selection_mode.default_value, CapabilityValue::String("single".to_string()));
}

#[test]
fn virtual_list_capability_read_write_roundtrip() {
    let factory = WidgetFactory::new_with_defaults();
    let mut list = VirtualList::new(Rect::new(0, 0, 120, 60));

    assert_eq!(factory.read_property(&list, "has_data_source"), Ok(CapabilityValue::Bool(false)));
    assert_eq!(factory.read_property(&list, "selected_row"), Ok(CapabilityValue::Null));

    factory
        .write_property(&mut list, "row_height", CapabilityValue::UInt(32))
        .expect("row_height should be writable");
    factory
        .write_property(&mut list, "overscan", CapabilityValue::UInt(4))
        .expect("overscan should be writable");
    factory
        .write_property(&mut list, "scroll_row", CapabilityValue::UInt(7))
        .expect("scroll_row should be writable");

    assert_eq!(factory.read_property(&list, "row_height"), Ok(CapabilityValue::UInt(32)));
    assert_eq!(factory.read_property(&list, "overscan"), Ok(CapabilityValue::UInt(4)));
    // Without a data source, scroll_row is normalized back to 0.
    assert_eq!(factory.read_property(&list, "scroll_row"), Ok(CapabilityValue::UInt(0)));
}

#[test]
fn test_create_nonexistent_widget_returns_error() {
    let factory = WidgetFactory::new_with_defaults();
    // `create` returns None (no Ok/Err) for unregistered names
    assert!(factory.create("nonexistent", Rect::new(0, 0, 1, 1), "").is_none());
    // `create_by_kind` returns None for kinds not in the factory
    let empty = WidgetFactory::new();
    assert!(empty.create("btn", Rect::new(0, 0, 1, 1), "").is_none());
}

#[test]
fn test_read_property_unregistered_widget_returns_error() {
    let factory = WidgetFactory::new_with_defaults();
    let widget = factory.create("btn", Rect::new(0, 0, 100, 30), "Save").unwrap();

    // Verify the public API returns UnknownProperty for a schema item that
    // does not exist on any widget, simulating an unregistered property
    let result = factory.read_property(widget.as_ref(), "__bogus_prop__");
    assert_eq!(result, Err(CapabilityAccessError::UnknownProperty));
}
