// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

    // `colorpicker` is the same control as `color_picker`, so it reports the picker's
    // own kind since BLUE16 phase E-6. It used to report `ColorDialog` because the
    // picker had no kind of its own; the test is updated rather than the behaviour,
    // because reporting "dialog" for a control with no window was the bug.
    let color_picker =
        factory.create("colorpicker", rect, "").expect("color picker must be created via alias");
    assert_eq!(color_picker.kind(), WidgetKind::ColorPicker);

    // The dialog is a separate control reached by its own name, and it still reports
    // `ColorDialog`.
    let color_dialog =
        factory.create("color_dialog", rect, "").expect("colour dialog must be registered");
    assert_eq!(color_dialog.kind(), WidgetKind::ColorDialog);

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
    // A `Chip` is its own kind. It previously reported `CheckListBox` — a type alias
    // for `ListBox` — which made the kind→control lookup ambiguous and left
    // `WidgetKind::Chip` with no capability at all.
    assert_eq!(chips.kind(), WidgetKind::Chip);

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

#[cfg(not(alloc_frugal))]
#[test]
fn schema_defaults_are_readable_and_writable_when_declared() {
    let factory = WidgetFactory::new_with_defaults();
    for capability in factory.capabilities() {
        for prop in capability.properties {
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

#[cfg(not(alloc_frugal))]
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

// ---------------------------------------------------------------------------
// Controls that had a complete implementation but were never registered
// ---------------------------------------------------------------------------

/// The six controls that used to be constructible only by naming their Rust
/// type must now resolve through the factory by every name they advertise.
///
/// # Why the assertion is on `create`, not on `capability`
///
/// Declaring a capability and registering a constructor are separate statements,
/// and the defect being fixed was exactly a control that had the former and not
/// the latter. Only `create` proves both are present.
#[test]
fn unregistered_controls_are_constructible_by_canonical_name_and_alias() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 240, 160);

    let cases: &[(&str, &str, WidgetKind)] = &[
        ("timeline_widget", "timeline", WidgetKind::Chart),
        ("timeline_view", "timeline_view", WidgetKind::Chart),
        ("command_palette", "command_box", WidgetKind::ListView),
        ("notification_center", "notifications", WidgetKind::ListView),
        ("diff_viewer", "diff", WidgetKind::Table),
        ("markdown_editor", "md_editor", WidgetKind::RichEdit),
        ("toast_stack", "toasts", WidgetKind::PopupWindow),
        ("grid_table", "gridtable", WidgetKind::GridTable),
        // The four controls added in BLUE16 Phase E-2. Their Rust-side construction
        // paths are order-independent, but only these rows prove the *name* a
        // counterparty would use actually resolves.
        ("number_picker", "picker", WidgetKind::NumberPicker),
        ("otp_input", "otp", WidgetKind::OtpInput),
        ("banner", "notice", WidgetKind::Banner),
        ("pagination", "page_numbers", WidgetKind::Pagination),
        // `RadarChart` (BLUE17 Phase D-3) is a kind of its own rather than a
        // `chart_type` token, because its data model (series over *dimensions*)
        // is not the same shape as `ChartWidget`'s (a value over its index).
        ("radar_chart", "radar", WidgetKind::RadarChart),
        // `KanbanBoard` (BLUE17 Phase D-1) is two-level (columns of cards) with
        // cross-container moves, which no existing control's model carries.
        ("kanban_board", "kanban", WidgetKind::KanbanBoard),
        // `Cascader` (BLUE17 Phase D-5) selects a *path* of varying depth, where
        // `dropdown` selects one index into a flat list.
        ("cascader", "cascade", WidgetKind::Cascader),
        // `QueryBuilder` (BLUE17 Phase D-6) renders the same `FilterExpr` the grid
        // evaluates; the flat `Vec<ColumnFilter>` model could not express it.
        ("query_builder", "filter_builder", WidgetKind::QueryBuilder),
        // `EmojiPicker` (BLUE17 Phase D-4) is the picker shell; its glyph table is
        // supplied by the caller, so it ships no data of its own.
        ("emoji_picker", "emoji", WidgetKind::EmojiPicker),
        // `Mention` (BLUE17 Phase D-2) completes the token before the caret and
        // holds several mentions, where `auto_complete_edit` replaces the whole field.
        ("mention", "at_mention", WidgetKind::Mention),
    ];

    for (name, alias, kind) in cases {
        let widget = factory
            .create(name, rect, "")
            .unwrap_or_else(|| panic!("{name} must be constructible through the factory"));
        assert_eq!(widget.kind(), *kind, "{name} reported the wrong kind");

        let via_alias = factory
            .create(alias, rect, "")
            .unwrap_or_else(|| panic!("{alias} must resolve to the same control as {name}"));
        assert_eq!(via_alias.kind(), *kind, "{alias} reported the wrong kind");
    }
}

/// A registered control must also be reachable through its own property
/// contract, and the factory must resolve it back to *its own* capability.
///
/// # Why the read is asserted, not just the registration
///
/// Three of these controls share a `WidgetKind` with an existing one
/// (`command_palette`/`notification_center` with `list_view`, `diff_viewer` with
/// `table_widget`, `markdown_editor` with `rich_edit`, `timeline_widget` with
/// `chart`, `toast_stack` with `popup_window`). Registering without a type-based
/// tie-break makes the lookup fall through to an empty schema, so the control
/// answers `UnknownWidget` for a property it really has — the exact failure this
/// test exists to catch.
#[test]
fn unregistered_controls_publish_their_own_properties() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 240, 160);

    let cases: &[(&str, &str)] = &[
        ("timeline_widget", "item_count"),
        ("command_palette", "filtered_count"),
        ("notification_center", "unread_count"),
        ("diff_viewer", "change_count"),
        ("markdown_editor", "word_count"),
        ("toast_stack", "toast_count"),
        ("grid_table", "row_count"),
        ("number_picker", "row_count"),
        ("otp_input", "length"),
        ("banner", "action_count"),
        ("pagination", "page_count"),
    ];

    for (name, property) in cases {
        let widget = factory
            .create(name, rect, "")
            .unwrap_or_else(|| panic!("{name} must be constructible through the factory"));
        let value = factory.read_property(widget.as_ref(), property).unwrap_or_else(|error| {
            panic!("{name} must answer its own property {property}, got {error:?}")
        });
        // The point is that the name is *answered by this control*, not that every
        // control starts at zero: `number_picker` legitimately reports the size of
        // its default `0..=100` range. A read that resolved to a sibling's schema
        // would fail above with `UnknownWidget`.
        assert!(
            matches!(value, CapabilityValue::UInt(_)),
            "{name}::{property} must report a count, got {value:?}"
        );
    }
}

/// Writing through the contract must reach the control's real state, so the
/// registered name is not merely a constructor alias.
///
/// # Why a round trip is the evidence
///
/// A write that returns `Ok` but stores nothing would satisfy a weaker test. The
/// assertion is that a subsequent read reports the written value, which can only
/// happen if the setter ran the control's own method.
#[test]
fn unregistered_controls_round_trip_writable_properties() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 240, 160);

    let mut timeline = factory.create("timeline_widget", rect, "").expect("timeline");
    factory
        .write_property(timeline.as_mut(), "row_height", CapabilityValue::UInt(32))
        .expect("timeline row_height should be writable");
    assert_eq!(
        factory.read_property(timeline.as_ref(), "row_height"),
        Ok(CapabilityValue::UInt(32))
    );

    let mut palette = factory.create("command_palette", rect, "").expect("palette");
    factory
        .write_property(palette.as_mut(), "query", CapabilityValue::String("open".to_string()))
        .expect("palette query should be writable");
    assert_eq!(
        factory.read_property(palette.as_ref(), "query"),
        Ok(CapabilityValue::String("open".to_string()))
    );

    let mut diff = factory.create("diff_viewer", rect, "").expect("diff");
    factory
        .write_property(diff.as_mut(), "left_text", CapabilityValue::String("a".to_string()))
        .expect("diff left_text should be writable");
    factory
        .write_property(diff.as_mut(), "right_text", CapabilityValue::String("b".to_string()))
        .expect("diff right_text should be writable");
    assert_eq!(
        factory.read_property(diff.as_ref(), "left_text"),
        Ok(CapabilityValue::String("a".to_string()))
    );
    assert_eq!(
        factory.read_property(diff.as_ref(), "right_text"),
        Ok(CapabilityValue::String("b".to_string()))
    );

    let mut markdown = factory.create("markdown_editor", rect, "").expect("markdown editor");
    factory
        .write_property(markdown.as_mut(), "preview_mode", CapabilityValue::Bool(true))
        .expect("markdown preview_mode should be writable");
    assert_eq!(
        factory.read_property(markdown.as_ref(), "preview_mode"),
        Ok(CapabilityValue::Bool(true))
    );

    let mut toasts = factory.create("toast_stack", rect, "").expect("toast stack");
    factory
        .write_property(toasts.as_mut(), "row_height", CapabilityValue::UInt(48))
        .expect("toast row_height should be writable");
    assert_eq!(factory.read_property(toasts.as_ref(), "row_height"), Ok(CapabilityValue::UInt(48)));

    let mut grid_table = factory.create("grid_table", rect, "").expect("grid table");
    factory
        .write_property(grid_table.as_mut(), "row_height", CapabilityValue::UInt(36))
        .expect("grid table row_height should be writable");
    assert_eq!(
        factory.read_property(grid_table.as_ref(), "row_height"),
        Ok(CapabilityValue::UInt(36))
    );
    factory
        .write_property(
            grid_table.as_mut(),
            "selection_mode",
            CapabilityValue::String("row".to_string()),
        )
        .expect("grid table selection_mode should be writable");
    assert_eq!(
        factory.read_property(grid_table.as_ref(), "selection_mode"),
        Ok(CapabilityValue::String("row".to_string()))
    );
}

/// Reversed bounds must not produce an inverted viewport.
///
/// `TimelineWidget::set_viewport` raises `end` above `start`, so writing an
/// `end` below the live `start` must clamp rather than store a negative span — a
/// span that would otherwise make the projection divide by a non-positive range.
#[test]
fn timeline_viewport_write_never_inverts_the_range() {
    let factory = WidgetFactory::new_with_defaults();
    let mut timeline = factory
        .create("timeline_widget", Rect::new(0, 0, 240, 160), "")
        .expect("timeline must be constructible");

    factory
        .write_property(timeline.as_mut(), "viewport_start", CapabilityValue::Int(50))
        .expect("viewport_start should be writable");
    factory
        .write_property(timeline.as_mut(), "viewport_end", CapabilityValue::Int(10))
        .expect("viewport_end should be writable");

    let start = match factory.read_property(timeline.as_ref(), "viewport_start") {
        Ok(CapabilityValue::Int(value)) => value,
        other => panic!("viewport_start must read back as Int, got {other:?}"),
    };
    let end = match factory.read_property(timeline.as_ref(), "viewport_end") {
        Ok(CapabilityValue::Int(value)) => value,
        other => panic!("viewport_end must read back as Int, got {other:?}"),
    };
    assert!(end > start, "an inverted write must be clamped: start={start}, end={end}");
}

/// Read-only names must answer `ReadOnlyProperty` rather than accepting a write.
///
/// # Why this is asserted per control
///
/// `selected_index` / derived counts have no setter, so accepting a write would
/// silently discard the caller's value while reporting success.
#[test]
fn derived_names_reject_writes_on_newly_registered_controls() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 240, 160);

    let cases: &[(&str, &str, CapabilityValue)] = &[
        ("timeline_widget", "item_count", CapabilityValue::UInt(1)),
        ("command_palette", "filtered_count", CapabilityValue::UInt(1)),
        ("notification_center", "unread_count", CapabilityValue::UInt(1)),
        ("diff_viewer", "change_count", CapabilityValue::UInt(1)),
        ("markdown_editor", "word_count", CapabilityValue::UInt(1)),
        ("toast_stack", "toast_count", CapabilityValue::UInt(1)),
        ("grid_table", "row_count", CapabilityValue::UInt(1)),
        ("otp_input", "is_complete", CapabilityValue::Bool(true)),
        ("banner", "dismissed", CapabilityValue::Bool(true)),
        ("pagination", "page_count", CapabilityValue::UInt(1)),
    ];

    for (name, property, value) in cases {
        let mut widget = factory
            .create(name, rect, "")
            .unwrap_or_else(|| panic!("{name} must be constructible through the factory"));
        assert_eq!(
            factory.write_property(widget.as_mut(), property, value.clone()),
            Err(CapabilityAccessError::ReadOnlyProperty),
            "{name}::{property} must be declared read-only"
        );
    }
}
