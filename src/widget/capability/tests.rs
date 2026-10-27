// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// The module-level gate, so tooling that scans file text recognises this as a test module.
// The parent declares `mod tests;` under `#[cfg(test)]`, which the compiler sees but a text scan
// of this file does not — and a scan that cannot tell a test module from production code reports
// its `Mutex` as widget state (`tools/check_locking.sh`). Same line as
// `special_widgets/toast/tests.rs`, which records the reasoning in full.
#![cfg(test)]

use super::*;
use crate::core::Rect;
use crate::widget::view_widgets::virtual_list::VirtualList;

/// `same_key` must be exactly `normalize_key(left) == normalize_key(right)`.
///
/// Every schema lookup uses it instead of building both normalised strings — one allocation per
/// *candidate schema* before — so an answer that differed even for one spelling would change which
/// property or event a declaration resolved to. The Unicode entry is the one that makes this more
/// than a case-insensitive compare: `İ` lower-cases to two `char`s, which only an iterator-based
/// comparison gets right without materialising it.
#[test]
fn same_key_agrees_with_normalising_both_names() {
    let spellings = [
        "clicked",
        "Clicked",
        "CLICKED",
        "click-ed",
        "click ed",
        "click_ed",
        "value_changed",
        "value-changed",
        "VALUE CHANGED",
        "valuechanged",
        "",
        "_",
        "-",
        "  ",
        "selection-changed",
        "selection_changed",
        "İstanbul",
        "istanbul",
        "İ",
        "i̇",
    ];
    for left in spellings {
        for right in spellings {
            assert_eq!(
                same_key(left, right),
                normalize_key(left) == normalize_key(right),
                "same_key disagreed for {left:?} vs {right:?}"
            );
        }
    }
}

/// The shared factory is built once and answers the same table as a fresh one.
///
/// Pins the property the hot paths rely on: `shared()` must not quietly rebuild the ~190-control
/// registry (a second build would make the pointer comparison fail), and it must not diverge
/// from `new_with_defaults()` (the equality check).
#[test]
fn the_shared_factory_is_built_once_and_matches_a_fresh_registry() {
    let first = WidgetFactory::shared();
    let second = WidgetFactory::shared();
    assert!(core::ptr::eq(first, second), "shared() rebuilt the registry instead of reusing it");

    let fresh = WidgetFactory::new_with_defaults();
    assert_eq!(
        first.capabilities().len(),
        fresh.capabilities().len(),
        "the shared table must contain every core registration"
    );
    assert_eq!(first.widget_names(), fresh.widget_names());
}

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
    assert_eq!(breadcrumb.kind(), WidgetKind::Breadcrumb);

    let split_button = factory
        .create("splitbutton", rect, "Menu")
        .expect("split button must be created via alias");
    assert_eq!(split_button.kind(), WidgetKind::ToolButton);

    let segmented = factory
        .create("segmentedcontrol", rect, "")
        .expect("segmented control must be created via alias");
    // Its own kind, not `ToggleButton`'s: the two share no behaviour, and reporting the
    // toggle's kind made `capability_by_kind` answer two controls for one kind.
    assert_eq!(segmented.kind(), WidgetKind::SegmentedControl);

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
fn new_widget_kinds_resolve_their_own_kind() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 200, 120);

    let tree_table = factory.create("tree_table", rect, "").expect("tree table must be created");
    assert_eq!(tree_table.kind(), WidgetKind::TreeTable);

    let breadcrumb = factory.create("breadcrumb", rect, "").expect("breadcrumb must be created");
    assert_eq!(breadcrumb.kind(), WidgetKind::Breadcrumb);

    let pad = factory.create("signature_pad", rect, "").expect("signature pad must be created");
    assert_eq!(pad.kind(), WidgetKind::SignaturePad);

    let zone = factory.create("drop_zone", rect, "").expect("drop zone must be created");
    assert_eq!(zone.kind(), WidgetKind::DropZone);

    // `dialog` used to be an alias of `popup_window`, so `create("dialog")` built a
    // `PopupWindow`. It is now its own control and must report `WidgetKind::Dialog`.
    let dialog = factory.create("dialog", rect, "").expect("dialog must be created");
    assert_eq!(dialog.kind(), WidgetKind::Dialog);
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

/// The writable half of [`schema_defaults_are_readable_and_writable_when_declared`].
///
/// That gate proves a declared `readable` property answers its schema default, but
/// nothing proved the `writable` half: a property could be published as writable while
/// every write either failed or was silently discarded. For every writable property
/// this writes the value a fresh instance **reports**, then reads it back — the
/// round trip a consumer actually performs — and, when the schema declares a
/// non-`Null` static default, writes that too. A `Null` default is skipped for the
/// second half: a control whose fresh value is materialised at construction (a clock
/// editing control starts at "now") has no static value to write, and the first half
/// already proves its reported value round-trips. Kinds the factory cannot construct
/// (they need constructor arguments beyond the generic ones) are skipped — the same
/// reason the factory has a fallible `create`.
#[cfg(not(alloc_frugal))]
#[test]
fn schema_defaults_write_back_through_the_property_route() {
    let factory = WidgetFactory::new_with_defaults();
    for capability in factory.capabilities() {
        let name = capability.canonical_name;
        let Some(mut widget) = factory.create(name, Rect::new(0, 0, 240, 120), "") else {
            continue;
        };
        for prop in capability.properties {
            if !prop.writable {
                continue;
            }
            let Ok(reported) = factory.read_property(widget.as_ref(), prop.name) else {
                continue;
            };
            write_and_assert_read_back(&factory, widget.as_mut(), name, prop.name, reported);
            if let Ok(default) = factory.default_property_value(name, prop.name) {
                if default != CapabilityValue::Null {
                    write_and_assert_read_back(&factory, widget.as_mut(), name, prop.name, default);
                }
            }
        }
    }
}

/// Writes `value` to `prop` on `widget` and asserts the property route reports it back.
#[cfg(not(alloc_frugal))]
fn write_and_assert_read_back(
    factory: &WidgetFactory,
    widget: &mut dyn Widget,
    name: &str,
    prop: &str,
    value: CapabilityValue,
) {
    factory.write_property(widget, prop, value.clone()).unwrap_or_else(|error| {
        panic!(
            "{name}::{prop} is declared writable but rejected its own value {value:?}: {error:?}"
        )
    });
    let read_back = factory.read_property(widget, prop);
    match (&value, read_back) {
        // A float property stores an `f32`, while the schema default is an `f64`
        // literal: `0.8` is not representable in `f32`, so exact equality would
        // reject a perfectly faithful round-trip. Compare at the precision the
        // property actually stores.
        (CapabilityValue::Float(expected), Ok(CapabilityValue::Float(actual))) => {
            assert_eq!(
                *expected as f32, actual as f32,
                "{name}::{prop} should read back the float it was just written"
            );
        }
        (expected, actual) => {
            assert_eq!(
                actual,
                Ok(expected.clone()),
                "{name}::{prop} should read back the value it was just written"
            );
        }
    }
}

/// The label resolution prefers a control's dedicated caption over its content.
///
/// # Why this is a general assertion and not a `floating_label` special case
///
/// Every constructor takes a `text` parameter that becomes the control's label, and the
/// shared helper resolves "the label" by probing the known spellings in order. For a
/// control with both a `text` (content) and a `label` (caption) property — `floating_label`
/// is the one such control — resolving to the first hit put the caption into the content
/// slot and left the caption slot empty, so the control that exists to demonstrate a
/// floating label had nothing to float.
///
/// # What this pins, and what it deliberately does not
///
/// It pins the *choice*: for a control that answers more than one of the known label
/// spellings, the most descriptive one must win. That is what `floating_label` needed —
/// it answers both `label` and `text`, and the caption has to outrank the content or the
/// field has nothing to float. It does not assert that every constructor writes its text
/// anywhere (several genuinely ignore the parameter — `web_engine_view` is empty on a fresh
/// instance), nor that every control's answer is `label`: `message_box` truly answers both
/// `text` and `message`, and either is a legitimate label for it.
#[cfg(not(alloc_frugal))]
#[test]
fn a_control_that_answers_a_caption_and_a_content_is_labelled_by_the_caption() {
    use crate::widget::capability::CapabilityValue as Value;

    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;
    for capability in factory.capabilities() {
        let Some(mut widget) =
            factory.create(capability.canonical_name, Rect::new(0, 0, 64, 48), "Sample")
        else {
            continue;
        };
        let Some(properties) = widget.properties_dyn_mut() else {
            continue;
        };
        // A control is ambiguous only when it answers more than one of the known spellings
        // with a string.
        let answered: alloc::vec::Vec<&'static str> =
            crate::control_backend::custom::LABEL_PROPERTY_NAMES
                .iter()
                .copied()
                .filter(|name| matches!(properties.get(name), Ok(Value::String(_))))
                .collect();
        if answered.len() < 2 {
            continue;
        }
        checked += 1;
        // The first name in the published order is the most descriptive, so it must be the
        // one the writer picks — otherwise the constructor's text and the control's caption
        // are two different properties.
        assert_eq!(
            crate::control_backend::custom::widget_label_property_name(properties),
            Some(answered[0]),
            "{} answers {answered:?}, so the most descriptive one must win",
            capability.canonical_name
        );
    }
    assert!(
        checked > 0,
        "no control answers both a caption and a content name, so this assertion proves \
         nothing about the ordering"
    );

    // The control this ordering exists for: it answers both, and the caption must win.
    let mut floating = factory
        .create("floating_label", Rect::new(0, 0, 200, 48), "Sample")
        .expect("`floating_label` is a registered control");
    let properties = floating.properties_dyn_mut().expect("it declares properties");
    assert_eq!(
        crate::control_backend::custom::widget_label_property_name(properties),
        Some("label"),
        "a floating-label field must be labelled by its caption, not by its content"
    );
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
fn every_kind_with_a_constructor_has_a_resolvable_factory_name() {
    // `Panel` is a real `WidgetKind` with a real constructor, but it has no capability
    // row of its own: `panel_capability()` is keyed on `GroupBox` (the same control
    // under another name, exactly as `DockPanel` is to `DockWidget`). It therefore fell
    // through to the `other` arm of `alias_factory_name` and returned `""`.
    //
    // An unresolvable name is not inert: `theme::apply_active_theme` bails out on it, so a
    // `Panel`-kinded widget silently received no theme role. This test pins the
    // resolution so a future kind added without a capability row fails here rather than
    // losing its theme silently.
    //
    // The lookup returns `Option` rather than `""`, so "could not be named" is now a
    // distinct outcome the caller must handle, instead of an empty string it had to
    // remember to test for.
    let factory = WidgetFactory::new_with_defaults();
    for kind in [WidgetKind::Panel, WidgetKind::DockPanel] {
        let name = crate::widget::capability::factory_name_for_kind(kind).unwrap_or_else(|| {
            panic!(
                "{kind:?} resolves to no factory name, so `theme::apply_active_theme` \
                     will skip it; add an alias row in `alias_factory_name`"
            )
        });
        // A name the factory rejects would be equally broken, so the resolved spelling
        // has to be one the factory actually answers to.
        assert!(
            factory.capability(name).is_some(),
            "factory_name_for_kind({kind:?}) = {name:?}, but the factory has no such name"
        );
    }

    // `MenuItem` is the documented exception: `kind.rs` declares it `kind-role: child`
    // ("created by their owning `Menu`, never constructed directly from a factory
    // name"), so it has no capability row *and* no factory name by design. It must
    // still resolve to a name, because the theme layer classifies on that name rather
    // than on the factory lookup.
    let menu_item = crate::widget::capability::factory_name_for_kind(WidgetKind::MenuItem);
    assert_eq!(
        menu_item,
        Some("menu_item"),
        "MenuItem must still be nameable for theme classification"
    );
}

#[test]
fn a_kind_alias_reaches_the_same_capability_as_its_canonical_spelling() {
    // `context_menu` is the role spelling of `menu` (`src/widget/mod.rs` records the decision
    // with `pub type ContextMenu = Menu;`), so the two names are two rows over **one control**:
    // same kind, same property table, same published events. They are distinct canonical names
    // — each is addressable in its own right — which is the `table_widget` / `table` shape.
    let factory = WidgetFactory::new_with_defaults();
    let by_role = factory.capability("context_menu").expect("context_menu must resolve");
    let by_name = factory.capability("menu").expect("menu must resolve");
    assert_eq!(by_role.kind, by_name.kind, "one control reports one kind");
    assert_eq!(
        by_role.properties, by_name.properties,
        "two names over one control must publish one property table"
    );
    let role_events: Vec<&str> = by_role.events.iter().map(|event| event.name).collect();
    let name_events: Vec<&str> = by_name.events.iter().map(|event| event.name).collect();
    assert_eq!(role_events, name_events, "and one event list");
}

#[test]
fn capability_manifest_exports_defaults_and_metadata() {
    let factory = WidgetFactory::new_with_defaults();

    let manifest =
        factory.capability_manifest("table").expect("table manifest should be exportable");

    assert_eq!(manifest.kind, WidgetKind::Table);
    // `table` is the canonical name; `table_widget` and `tablewidget` reach the same
    // control through `normalize_key` (which strips `_`), not through an alias row —
    // see `tests/capability_alias_hygiene_test.rs`. The list is therefore empty, and
    // that is the correct state: every alias it used to carry was inert.
    assert_eq!(manifest.canonical_name, "table");
    assert!(
        manifest.aliases.is_empty(),
        "`table` needs no alias: `normalize_key` already accepts `table_widget` / \
         `tablewidget`, so an alias row would be a no-op (found: {:?})",
        manifest.aliases
    );
    // and those spellings must still resolve.
    assert!(factory.capability("table_widget").is_some());
    assert!(factory.capability("tablewidget").is_some());
    assert!(manifest.events.iter().any(|event| event.name == "selection_changed"));
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

/// Every schema default must agree with the control the schema describes.
///
/// # Why this is a test and not a comment
///
/// A schema default is load-bearing, not documentation: `view::apply`'s
/// `resolve_null_reset` substitutes it for every `CapabilityValue::Null` patch, so a
/// wrong default is a wrong *value* reaching a live control — dropping a `visible`
/// binding in a view manifest would hide a control the manifest never mentioned.
///
/// The `visible` entry in `access.rs` had drifted from four controls at once. It
/// excluded `Tooltip`/`Popover`/`StatusBar` on the theory that those kinds read bare
/// `visible` as their own popup state, but all three route the name to the base flag
/// and publish popup state under a *different* name (`shown`, `message`). The result
/// was a schema saying `false` for controls whose live answer is `true`.
///
/// This asserts the invariant directly for the property that broke, and asserts the
/// weaker "no default is reported for a property the control does not publish"
/// invariant across the whole registry.
#[test]
fn declared_visible_default_matches_the_constructed_control() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 200, 60);

    let mut checked = 0usize;
    for capability in factory.capabilities() {
        let name = capability.canonical_name;
        let Ok(declared) = factory.default_property_value(name, "visible") else {
            continue;
        };
        let Some(widget) = factory.create(name, rect, "") else {
            continue;
        };
        let Ok(live) = factory.read_property(widget.as_ref(), "visible") else {
            continue;
        };
        assert_eq!(
            declared, live,
            "`{name}` declares `visible` default {declared:?} but a freshly \
             constructed control reports {live:?}"
        );
        checked += 1;
    }
    assert!(checked >= 20, "expected the `visible` default to cover many kinds, got {checked}");
}

/// A schema default must never name a property the control does not publish.
///
/// `default_property_value` resolves through `capability().kind` to the shared
/// per-kind tables, so a kind whose row is shared by two controls (`table` and
/// `virtual_list` both declare `WidgetKind::Table`) can hand one control the other's
/// defaults. This checks the property *name* is one the capability advertises, which
/// is the part that is checkable without constructing every kind.
#[test]
fn declared_defaults_are_published_properties() {
    let factory = WidgetFactory::new_with_defaults();

    let mut violations = Vec::new();
    for capability in factory.capabilities() {
        let name = capability.canonical_name;
        for property in capability.properties {
            let Ok(value) = factory.default_property_value(name, property.name) else {
                continue;
            };
            let published = capability
                .properties
                .iter()
                .any(|schema| normalize_key(schema.name) == normalize_key(property.name));
            if !published {
                violations.push(format!("{name}::{} -> {value:?}", property.name));
            }
        }
    }
    assert!(violations.is_empty(), "defaults declared for unpublished properties: {violations:?}");
}

// ── Extension registration (BLUE24 §12 U-7) ──────────────────────────────────

/// An extension registering a *new* name succeeds, and the control is then constructible.
#[test]
fn an_extension_can_register_a_new_name() {
    let mut factory = WidgetFactory::new_with_defaults();
    let capability = WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "vendor_zz_badge",
        aliases: &["vendor_zz_short"],
        properties: &[],
        events: &[],
        ..default_capability_for_tests()
    };
    factory
        .try_register(capability, |geometry, text| {
            Box::new(crate::widget::base_widgets::label::Label::new(text.to_string(), geometry))
        })
        .expect("a name no control uses is free");

    assert!(factory.capability("vendor_zz_badge").is_some());
    assert!(factory.capability("vendor_zz_short").is_some(), "its alias resolves too");
    assert!(factory.create("vendor_zz_badge", Rect::new(0, 0, 10, 10), "x").is_some());
}

/// Registering a name a core control already answers to is **refused**, not silently applied.
///
/// # The defect this pins
///
/// `WidgetFactory::register` overwrites on collision, so a plugin claiming `"label"` would replace
/// the core label — and the symptom would not be a failed load but a control that looks wrong, in a
/// build whose only difference is that a plugin was enabled.
#[test]
fn an_extension_cannot_shadow_a_core_control() {
    let mut factory = WidgetFactory::new_with_defaults();
    let before = factory.capabilities().len();

    let capability = WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "label",
        aliases: &[],
        properties: &[],
        events: &[],
        ..default_capability_for_tests()
    };
    let error = factory
        .try_register(capability, |geometry, text| {
            Box::new(crate::widget::base_widgets::label::Label::new(text.to_string(), geometry))
        })
        .expect_err("the name belongs to a core control");
    assert_eq!(
        error,
        RegistrationError::NameTaken { name: "label", existing: "label" },
        "and the error names both sides"
    );
    assert_eq!(factory.capabilities().len(), before, "nothing was registered");
}

/// An **alias** collision is refused too: an alias is a name the factory resolves.
#[test]
fn an_extension_cannot_shadow_a_core_alias() {
    let factory = WidgetFactory::new_with_defaults();
    // Find a name that resolves to a control whose canonical name differs from it — an alias.
    let alias = factory
        .reserved_names()
        .into_iter()
        .find(|name| {
            factory.name_taken_by(name).is_some_and(|canonical| canonical != name.as_str())
        })
        .expect("the core table publishes at least one alias");

    // The property that matters: an alias reports as taken by its canonical owner, so
    // `try_register` refuses a plugin that claimed it. Without this, a plugin could shadow a
    // control by its alias rather than its name — the same defect, a different spelling.
    let owner = factory.name_taken_by(&alias).expect("the alias resolves");
    assert_ne!(owner, alias.as_str(), "it is an alias, owned by another control");

    let mut target = WidgetFactory::new_with_defaults();
    // An alias that *is* taken, stated on a capability whose canonical name is free — so a check
    // that only looked at the canonical name would let this through.
    let taken_alias: &'static str = "label";
    assert!(target.name_taken_by(taken_alias).is_some());
    let capability = WidgetCapability {
        kind: WidgetKind::Label,
        canonical_name: "vendor_zz_innocent",
        aliases: &["label"],
        properties: &[],
        events: &[],
        ..default_capability_for_tests()
    };
    let error = target
        .try_register(capability, |geometry, text| {
            Box::new(crate::widget::base_widgets::label::Label::new(text.to_string(), geometry))
        })
        .expect_err("the alias belongs to a core control");
    assert_eq!(error, RegistrationError::NameTaken { name: "label", existing: "label" });
    assert!(
        target.capability("vendor_zz_innocent").is_none(),
        "nothing was written: a partial registration would leave one control reachable by some of \
         its names and not others"
    );
}

/// The reserved-name list contains the core names, so a caller can check before building.
#[test]
fn reserved_names_include_the_core_table() {
    let factory = WidgetFactory::new_with_defaults();
    let names = factory.reserved_names();
    assert!(names.iter().any(|name| name == "label"), "a core name is reserved");
    assert!(
        names.iter().any(|name| name != "label" && factory.capability(name).is_some()),
        "aliases are reserved too, not only canonical names"
    );
    assert!(
        !names.iter().any(|name| factory.capability(name).is_none()),
        "every reserved name must actually resolve, or the list would protect nothing"
    );
}

/// A minimal capability for the registration tests to vary one field of.
fn default_capability_for_tests() -> WidgetCapability {
    WidgetFactory::new_with_defaults()
        .capability("label")
        .expect("the core table publishes label")
        .clone()
}

/// `connect_event` binds every spelling of a published name to the one signal the control emits.
///
/// # What this pins
///
/// Validation normalised the caller's spelling to find a match, but the hub connection used the raw
/// argument, so `connect_event("slider", "value-changed", …)` registered under `"value-changed"`
/// while the control emitted under `"value_changed"`. The subscription was accepted and then never
/// fired — the silent "valid but inert" failure rule #97 rules out. Binding through the canonical
/// name fixes it, and this test proves a hyphenated, upper-cased and space-separated spelling all
/// reach the same emitted signal.
#[test]
fn connect_event_binds_every_spelling_to_the_canonical_signal() {
    use crate::signal::CustomSignalHub;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    let factory = WidgetFactory::new_with_defaults();
    // The canonical spelling the capability publishes.
    let canonical = factory
        .capability("slider")
        .expect("slider is registered")
        .events
        .iter()
        .find(|schema| normalize_key(schema.name) == normalize_key("value_changed"))
        .expect("slider publishes value_changed")
        .name;

    for spelling in ["value_changed", "value-changed", "VALUE CHANGED", " Value_Changed "] {
        let hub = CustomSignalHub::new();
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_slot = Arc::clone(&fired);
        factory
            .connect_event("slider", spelling, &hub, move || {
                fired_slot.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap_or_else(|error| panic!("`{spelling}` must be a published event: {error:?}"));

        // The control emits under the canonical name; the subscription must hear it.
        hub.emit(canonical);
        assert_eq!(
            fired.load(Ordering::SeqCst),
            1,
            "a subscription made with `{spelling}` must reach the `{canonical}` signal"
        );
    }
}

/// W-302: the focus events every control inherits from `BaseWidget` are published names.
///
/// They are one shared row rather than 180 duplicated ones, so the lookup — not the capability
/// table — is what accepts them. A name the control really does not publish must still be refused.
#[test]
fn focus_events_are_published_for_every_control() {
    use crate::signal::CustomSignalHub;

    let factory = WidgetFactory::new_with_defaults();
    let hub = CustomSignalHub::new();
    for control in ["button", "label", "slider", "text_area", "data_grid"] {
        for event in ["focus_gained", "focus_lost"] {
            factory.event_is_subscribable(control, event, &hub).unwrap_or_else(|error| {
                panic!("`{control}` must accept the inherited `{event}`: {error:?}")
            });
        }
    }
    assert!(
        factory.event_is_subscribable("button", "no_such_focus_event", &hub).is_err(),
        "the shared focus rows must not make every name subscribable"
    );
}

/// The four interactive-looking controls with no published events publish none on purpose.
///
/// # Why this is a contract, not a gap (BLUE-issue E-14)
///
/// `ime_preedit` changes some internal state
/// when handled input arrives, but none of those state changes is a **semantic event a host wires a
/// behaviour to**: a peak-hold overlay, a pre-edit buffer and a popover's internal
/// dismissal are presentation state. Publishing them would invent an API the host never asked for and
/// that the library would then have to keep stable (rules #4 and #98: capability differences are not
/// invented, and a designer's wire set is the host's to decide). The sibling list controls do publish
/// events, so this is a deliberate line, and pinning it here keeps a later "every control should have
/// `clicked`" change from silently adding events the contract never promised.
///
/// `camera_preview` was in this list until it grew `error_occurred` and `permission_denied`, which are
/// host-visible failures rather than presentation state, so it now publishes them. `audio_visualizer`
/// (`level_changed`/`peak_exceeded`/`mode_changed`, MW-16) and `popover` (show/hide lifecycle,
/// DLG-12) left this list because a host wires those state changes to behaviour.
#[test]
fn controls_with_no_published_events_keep_none_on_purpose() {
    let factory = WidgetFactory::new_with_defaults();
    let name = "ime_preedit";
    let capability = factory
        .capability(name)
        .unwrap_or_else(|| panic!("`{name}` must be a registered capability"));
    assert!(
        capability.events.is_empty(),
        "`{name}` is documented as an internal-presentation control with no published events; \
         publishing {} would change the contract (BLUE-issue E-14)",
        capability.events.len()
    );
}

/// The validated text-input variants are real controls over the one `LineEdit` model.
///
/// `password_edit` installs the masking echo mode; `phone_input` / `email_input` install a
/// grammar that filters every path into the model and publishes `valid`. The property route
/// must see all of that, not just a differently-named plain field.
#[cfg(not(alloc_frugal))]
#[test]
fn the_validated_input_variants_publish_their_grammar() {
    let factory = WidgetFactory::new_with_defaults();
    let rect = Rect::new(0, 0, 220, 24);

    let password = factory.create("password_edit", rect, "").expect("password field");
    assert_eq!(
        factory.read_property(password.as_ref(), "echo_mode"),
        Ok(CapabilityValue::String("password".to_string())),
        "a password field masks by default, before any caller sets the mode"
    );

    let mut phone = factory.create("phone_input", rect, "").expect("phone field");
    // Empty is not an error.
    assert_eq!(factory.read_property(phone.as_ref(), "valid"), Ok(CapabilityValue::Bool(true)));
    // Letters never reach the model.
    factory
        .write_property(phone.as_mut(), "text", CapabilityValue::String("call me".to_string()))
        .expect("text is writable");
    assert_eq!(
        factory.read_property(phone.as_ref(), "text"),
        Ok(CapabilityValue::String(String::new()))
    );
    // A too-short value is reported through the property route.
    factory
        .write_property(phone.as_mut(), "text", CapabilityValue::String("123".to_string()))
        .expect("text is writable");
    assert_eq!(factory.read_property(phone.as_ref(), "valid"), Ok(CapabilityValue::Bool(false)));
    assert_ne!(
        factory.read_property(phone.as_ref(), "validation_message"),
        Ok(CapabilityValue::String(String::new())),
        "an invalid value carries its message"
    );
    factory
        .write_property(phone.as_mut(), "text", CapabilityValue::String("555 123 4567".to_string()))
        .expect("text is writable");
    assert_eq!(factory.read_property(phone.as_ref(), "valid"), Ok(CapabilityValue::Bool(true)));

    let mut email = factory.create("email_input", rect, "").expect("email field");
    factory
        .write_property(email.as_mut(), "text", CapabilityValue::String("name@domain".to_string()))
        .expect("text is writable");
    assert_eq!(factory.read_property(email.as_ref(), "valid"), Ok(CapabilityValue::Bool(false)));
    factory
        .write_property(
            email.as_mut(),
            "text",
            CapabilityValue::String("name@domain.com".to_string()),
        )
        .expect("text is writable");
    assert_eq!(factory.read_property(email.as_ref(), "valid"), Ok(CapabilityValue::Bool(true)));
    // The derived pair is not writable: the text already determines both.
    assert!(factory.write_property(email.as_mut(), "valid", CapabilityValue::Bool(false)).is_err());
}

/// Every alias spelling a control publishes resolves to that control's one published schema.
///
/// # What this pins
///
/// `EVENT_ALIASES` is a promise that `connect_event(alias, ..)` is equivalent to the canonical
/// spelling. Both directions are checked against every capability that publishes *either* side,
/// and the resolution must land on a row that really exists in `events`. A spelling that no
/// control publishes is caught by `every_event_alias_reaches_the_signal_its_control_emits`
/// below, which drives the real emission path.
#[test]
fn every_event_alias_resolves_to_one_published_schema_per_control() {
    let factory = WidgetFactory::new_with_defaults();
    let mut checked = 0usize;

    for capability in factory.capabilities() {
        for row in EVENT_ALIASES {
            let publishes_alias = capability.events.iter().any(|event| event.name == row.alias);
            let publishes_canonical =
                capability.events.iter().any(|event| event.name == row.canonical);
            if !publishes_alias && !publishes_canonical {
                continue;
            }
            checked += 1;

            let via_alias = find_event_schema(capability, row.alias).unwrap_or_else(|| {
                panic!(
                    "{}: the alias `{}` resolved to nothing even though the control publishes \
                     `{}` or `{}`",
                    capability.canonical_name, row.alias, row.alias, row.canonical
                )
            });
            let via_canonical = find_event_schema(capability, row.canonical).unwrap_or_else(|| {
                panic!(
                    "{}: the canonical `{}` resolved to nothing even though the control publishes \
                     `{}` or `{}`",
                    capability.canonical_name, row.canonical, row.alias, row.canonical
                )
            });
            assert_eq!(
                via_alias.name, via_canonical.name,
                "{}: `{}` and `{}` must reach the same published row",
                capability.canonical_name, row.alias, row.canonical
            );
            assert!(
                capability.events.iter().any(|event| event.name == via_alias.name),
                "{}: `{}` resolved to `{}`, which is not a published event",
                capability.canonical_name,
                row.alias,
                via_alias.name
            );
        }
    }

    assert!(
        checked >= 8,
        "the alias table must be backed by several controls; only {checked} control/alias pairs \
         resolved"
    );
}

/// Every alias reaches the signal its control actually emits.
///
/// # Why end-to-end rather than a name check
///
/// `connect_event` returning `Ok` proves only that the name is valid. This drives the full path
/// the runtime uses: `connect_event(alias, ..)` subscribes on a hub, `EventSignalBinder::forward_all`
/// wires every published name through `event_signal_dyn`, and a real control action triggers the
/// signal. If the alias resolved to a name the control does not emit under, the slot never runs and
/// the assertion fails with the control/alias pair named.
#[test]
fn every_event_alias_reaches_the_signal_its_control_emits() {
    use crate::event::{key_codes, Event, EventHandler};
    use crate::signal::{CustomSignalHub, EventSignalBinder};
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    fn check<W, F>(control: &str, alias: &str, widget: &mut W, trigger: F)
    where
        W: crate::widget::Widget,
        F: FnOnce(&mut W),
    {
        let hub = Arc::new(CustomSignalHub::new());
        let fired = Arc::new(AtomicUsize::new(0));
        let slot = Arc::clone(&fired);
        WidgetFactory::new_with_defaults()
            .connect_event(control, alias, &hub, move || {
                slot.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap_or_else(|error| panic!("`{control}` must publish `{alias}`: {error:?}"));

        let mut binder = EventSignalBinder::new(Arc::clone(&hub));
        assert!(
            binder.forward_all(&*widget) > 0,
            "{control}: `forward_all` wired nothing, so the alias cannot fire"
        );

        trigger(widget);
        assert_eq!(
            fired.load(Ordering::SeqCst),
            1,
            "`{control}` accepted `{alias}` but no signal fired under its resolved name"
        );
    }

    let rect = Rect::new(0, 0, 240, 120);

    // W-19: cancelled -> canceled.
    let mut button = crate::widget::base_widgets::button::Button::new("b".to_string(), rect);
    check("button", "cancelled", &mut button, |button| {
        button.press();
        button.cancel_gesture();
    });

    let mut dialog = crate::widget::dialog::progress_dialog::ProgressDialog::new(rect);
    check("progress_dialog", "cancelled", &mut dialog, |dialog| dialog.cancel());

    let mut search = crate::widget::input_widgets::search_bar::SearchBar::new(rect);
    search.set_focused(true);
    check("search_bar", "cancelled", &mut search, |search| {
        search.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
    });

    let mut otp = crate::widget::input_widgets::otp_input::OtpInput::new(rect);
    check("otp_input", "cancelled", &mut otp, |otp| {
        otp.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
    });

    let mut wizard = crate::widget::dialog::wizard::WizardDialog::new(rect);
    check("wizard_dialog", "cancelled", &mut wizard, |wizard| {
        wizard.handle_event(&Event::KeyPress { key: key_codes::ESCAPE, modifiers: 0 });
    });

    let mut alert =
        crate::widget::cupertino::core::CupertinoAlertDialog::new(Rect::new(0, 0, 400, 300));
    check("cupertino_alert_dialog", "cancelled", &mut alert, |alert| {
        alert.handle_event(&Event::MouseRelease {
            pos: crate::core::Point::new(100, 225),
            button: 1,
        });
    });

    let mut editor = crate::widget::input_widgets::inplace_editor::InplaceEditor::new("text", rect);
    check("inplace_editor", "edit_cancelled", &mut editor, |editor| {
        editor.start_edit();
        editor.finish_edit(false);
    });

    // W-20: selected_changed / current_index_changed -> current_changed.
    let mut bar = crate::widget::nav_widgets::bottom_navigation_bar::BottomNavigationBar::new(rect);
    bar.add_item("A", "one");
    bar.add_item("B", "two");
    check("bottom_navigation_bar", "selected_changed", &mut bar, |bar| {
        bar.set_selected_index(1);
    });

    let mut segmented = crate::widget::misc_widgets::segmented_button::SegmentedButton::new(rect);
    segmented.add_segment(crate::widget::misc_widgets::segmented_button::Segment::new("A"));
    segmented.add_segment(crate::widget::misc_widgets::segmented_button::Segment::new("B"));
    check("segmented_button", "selected_changed", &mut segmented, |segmented| {
        segmented.set_selected_index(Some(1));
    });

    let mut rail = crate::widget::cupertino::material::MaterialNavigationRail::new(rect);
    rail.add_item("A", "one");
    rail.add_item("B", "two");
    check("material_navigation_rail", "selected_changed", &mut rail, |rail| {
        rail.set_selected(1);
    });

    let mut combo = crate::widget::input_widgets::combobox::ComboBox::new(rect);
    combo.add_item("one".to_string());
    combo.add_item("two".to_string());
    check("combo_box", "current_changed", &mut combo, |combo| {
        combo.set_current_index(Some(1));
    });

    let mut fonts = crate::widget::input_widgets::font_combo_box::FontComboBox::new(rect);
    fonts.add_font("Arial".to_string());
    fonts.add_font("Courier".to_string());
    check("font_combo_box", "current_changed", &mut fonts, |fonts| {
        fonts.set_current_index(1);
    });

    let mut tag = crate::widget::display_widgets::tag::Tag::new("tag", rect);
    check("tag", "current_changed", &mut tag, |tag| tag.set_selected(true));

    // W-21: subject-last hover spellings -> subject-first emitted fields.
    let mut chart = crate::widget::display_widgets::mini_chart::MiniChart::new(rect);
    chart.set_data(vec![
        crate::widget::display_widgets::mini_chart::DataPoint::new(0u32, 10),
        crate::widget::display_widgets::mini_chart::DataPoint::new(1u32, 40),
    ]);
    check("mini_chart", "hovered_point_changed", &mut chart, |chart| {
        chart.handle_event(&Event::MouseMove { pos: crate::core::Point::new(115, 50) });
    });

    let mut picker = crate::widget::display_widgets::emoji_picker::EmojiPicker::new(rect);
    picker.set_categories(vec!["faces".to_string()]);
    picker.set_glyphs(vec![crate::widget::display_widgets::emoji_picker::EmojiGlyph::new(
        "grin", "😀", "faces",
    )]);
    check("emoji_picker", "hovered_glyph_changed", &mut picker, |picker| {
        // The grid starts below the 30 px search row and 26 px tab strip; the first
        // 34 px cell is (0, 56)..(34, 90).
        picker.handle_event(&Event::MouseMove { pos: crate::core::Point::new(10, 70) });
    });

    let mut menu = crate::widget::menu_toolbar::menu_bar::MenuBar::new(rect);
    menu.add_menu("File");
    menu.set_hovered(true);
    check("menu_bar", "hovered_entry", &mut menu, |menu| {
        menu.handle_event(&Event::MouseMove { pos: crate::core::Point::new(5, 10) });
    });

    let mut shape = crate::widget::special_widgets::freeform_shape::FreeformShapeWidget::new(
        Rect::new(0, 0, 100, 100),
        crate::widget::special_widgets::freeform_shape::ShapePath::Polygon(vec![
            crate::core::Point::new(0, 0),
            crate::core::Point::new(100, 0),
            crate::core::Point::new(50, 100),
        ]),
    );
    check("freeform_shape", "hovered_changed", &mut shape, |shape| {
        shape.handle_event(&Event::MouseMove { pos: crate::core::Point::new(50, 20) });
    });
}
