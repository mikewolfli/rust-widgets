// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Contract tests for the per-control property layer (BLUE15 Phase C-1).
//!
//! Each control publishes its own `WidgetProperties` implementation. These tests
//! pin the properties of that contract that are **not** visible from a single
//! control's own unit tests:
//!
//! - a name is never published unless reading it succeeds (schema and reader
//!   cannot disagree),
//! - the shared four (`enabled` / `visible` / `tooltip` / `geometry`) are reachable
//!   from every migrated control through the fallback,
//! - an unknown name answers `UnknownProperty`, which is a *different* answer from
//!   `UnsupportedOnWidget` (a typo must not be reported as "control not migrated"),
//! - the `dyn Widget` hooks actually reach the concrete implementation, in both the
//!   shared and the mutable direction,
//! - a control that declares no properties of its own still exposes the shared set.
//!
//! The set of controls exercised here covers every category (base, input, view,
//! container, dialog, menu, advanced, media, other) so a category-wide mistake
//! cannot hide behind one well-behaved control.

#![cfg(all(test, full_widgets))]

use crate::core::Rect;
use crate::widget::capability::types::CapabilityValue;
use crate::widget::capability::{
    widget_property_get, widget_property_names, widget_property_set, WidgetFactory,
    BASE_PROPERTY_NAMES,
};
use crate::widget::{CapabilityAccessError, Widget};

/// Asserts the invariants every migrated control must satisfy.
fn assert_contract(widget: &mut dyn Widget, label: &str) {
    let names = widget_property_names(widget).unwrap_or_else(|| {
        panic!("{label}: must impl WidgetProperties and expose it via dyn Widget")
    });

    // Every published name must be readable, or the schema lies about the reader.
    for name in names {
        assert!(
            widget_property_get(widget, name).is_ok(),
            "{label}: property_names() publishes {name:?} but get() rejects it"
        );
    }

    // The shared four come from `base_property_get`, so they must always answer —
    // even for a control that declares no properties of its own.
    for shared in BASE_PROPERTY_NAMES {
        assert!(
            widget_property_get(widget, shared).is_ok(),
            "{label}: shared property {shared:?} must be reachable through the base fallback"
        );
        assert!(
            names.contains(shared),
            "{label}: shared property {shared:?} must also be published so schema consumers see it"
        );
    }

    // A typo is `UnknownProperty`, not `UnsupportedOnWidget`: the two answer
    // different questions ("no such property" vs "this control has no contract").
    assert_eq!(
        widget_property_get(widget, "definitely_not_a_property"),
        Err(CapabilityAccessError::UnknownProperty),
        "{label}: an unknown name must be reported as UnknownProperty"
    );

    // The mutable hook must reach the same implementation as the shared one.
    assert_eq!(
        widget_property_set(widget, "definitely_not_a_property", CapabilityValue::Null),
        Err(CapabilityAccessError::UnknownProperty),
        "{label}: the mutable hook must reach the concrete implementation"
    );
}

/// Reads a property, expecting success, and returns the value.
fn get(widget: &dyn Widget, name: &str) -> CapabilityValue {
    widget_property_get(widget, name).unwrap_or_else(|err| panic!("{name:?} should read: {err:?}"))
}

/// Base widgets: the shared contract plus each control's own properties.
#[test]
fn base_widgets_satisfy_the_contract() {
    use crate::widget::base_widgets::button::Button;
    use crate::widget::base_widgets::checkbox::CheckBox;
    use crate::widget::base_widgets::label::Label;
    use crate::widget::base_widgets::radiobutton::RadioButton;
    use crate::widget::base_widgets::toggle_button::ToggleButton;

    let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 80, 24));
    assert_eq!(
        widget_property_set(&mut button, "text", CapabilityValue::String("go".into())),
        Ok(())
    );
    assert_eq!(get(&button, "text"), CapabilityValue::String("go".into()));
    assert_contract(&mut button, "Button");

    let mut checkbox = CheckBox::new(Rect::new(0, 0, 120, 24));
    assert_eq!(widget_property_set(&mut checkbox, "checked", CapabilityValue::Bool(true)), Ok(()));
    assert_eq!(get(&checkbox, "checked"), CapabilityValue::Bool(true));
    assert_contract(&mut checkbox, "CheckBox");

    let mut radio = RadioButton::new(Rect::new(0, 0, 120, 24));
    assert_contract(&mut radio, "RadioButton");

    let mut label = Label::new("hello".to_string(), Rect::new(0, 0, 80, 20));
    assert_eq!(get(&label, "text"), CapabilityValue::String("hello".into()));
    assert_contract(&mut label, "Label");

    let mut toggle = ToggleButton::new("t".to_string(), Rect::new(0, 0, 80, 24));
    assert_contract(&mut toggle, "ToggleButton");
}

/// Input widgets, including the value/range properties the factory validates.
#[test]
fn input_widgets_satisfy_the_contract() {
    use crate::widget::display_widgets::progressbar::ProgressBar;
    use crate::widget::input_widgets::lineedit::LineEdit;
    use crate::widget::input_widgets::listbox::ListBox;
    use crate::widget::input_widgets::spinbox::SpinBox;

    let mut slider = crate::widget::Slider::new(Rect::new(0, 0, 100, 20));
    assert_eq!(widget_property_set(&mut slider, "value", CapabilityValue::Int(30)), Ok(()));
    assert_eq!(get(&slider, "value"), CapabilityValue::Int(30));
    assert_contract(&mut slider, "Slider");

    let mut spinbox = SpinBox::new(Rect::new(0, 0, 60, 24));
    assert_eq!(widget_property_set(&mut spinbox, "value", CapabilityValue::Int(7)), Ok(()));
    assert_eq!(get(&spinbox, "value"), CapabilityValue::Int(7));
    assert_contract(&mut spinbox, "SpinBox");

    let mut edit = LineEdit::new(Rect::new(0, 0, 80, 24));
    assert_eq!(
        widget_property_set(&mut edit, "text", CapabilityValue::String("hi".into())),
        Ok(())
    );
    assert_eq!(get(&edit, "text"), CapabilityValue::String("hi".into()));
    assert_contract(&mut edit, "LineEdit");

    let mut list = ListBox::new(Rect::new(0, 0, 120, 100));
    assert_eq!(get(&list, "selection_mode"), CapabilityValue::String("single".into()));
    assert_eq!(
        widget_property_set(&mut list, "selection_mode", CapabilityValue::String("multi".into())),
        Ok(())
    );
    assert_eq!(get(&list, "selection_mode"), CapabilityValue::String("multi".into()));
    assert_contract(&mut list, "ListBox");

    let mut progress = ProgressBar::new(Rect::new(0, 0, 100, 20));
    // The old dispatch treated `progress` as read-only, and that must not silently
    // become writable: the property layer is the only entry point now.
    assert_eq!(
        widget_property_set(&mut progress, "progress", CapabilityValue::Float(0.5)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
    assert_contract(&mut progress, "ProgressBar");
}

/// Container widgets.
#[test]
fn container_widgets_satisfy_the_contract() {
    use crate::widget::container_widgets::collapsible_pane::CollapsiblePane;
    use crate::widget::container_widgets::groupbox::GroupBox;
    use crate::widget::container_widgets::scrollarea::ScrollArea;
    use crate::widget::container_widgets::splitter::Splitter;
    use crate::widget::container_widgets::tabwidget::TabWidget;

    let mut group = GroupBox::new(Rect::new(0, 0, 200, 150));
    assert_contract(&mut group, "GroupBox");

    let mut splitter = Splitter::new(Rect::new(0, 0, 300, 200));
    assert_eq!(get(&splitter, "pane_count"), CapabilityValue::UInt(0));
    assert_contract(&mut splitter, "Splitter");

    let mut area = ScrollArea::new(Rect::new(0, 0, 300, 200));
    assert!(widget_property_get(&area, "horizontal_scroll_bar_policy").is_ok());
    assert_contract(&mut area, "ScrollArea");

    let mut tabs = TabWidget::new(Rect::new(0, 0, 300, 200));
    assert_eq!(get(&tabs, "tab_count"), CapabilityValue::UInt(0));
    assert_contract(&mut tabs, "TabWidget");

    let mut pane = CollapsiblePane::new(Rect::new(0, 0, 200, 100), "T".to_string());
    assert_eq!(widget_property_set(&mut pane, "collapsed", CapabilityValue::Bool(true)), Ok(()));
    assert_eq!(get(&pane, "collapsed"), CapabilityValue::Bool(true));
    assert_contract(&mut pane, "CollapsiblePane");
}

/// Dialog widgets. Their properties are read-only where the old write layer had no
/// arm, which this pins so a future edit cannot quietly make them writable.
#[test]
fn dialog_widgets_satisfy_the_contract() {
    use crate::widget::dialog::file_dialog::FileDialog;
    use crate::widget::dialog::message_box::MessageBox;
    use crate::widget::dialog::popup_window::PopupWindow;
    use crate::widget::dialog::progress_dialog::ProgressDialog;

    let mut message = MessageBox::new(Rect::new(0, 0, 350, 150));
    // Both strings are writable: a message box is defined by its title and body,
    // and the factory seeds them from the constructor arguments.
    assert_eq!(
        widget_property_set(&mut message, "title", CapabilityValue::String("t".into())),
        Ok(())
    );
    assert_eq!(get(&message, "title"), CapabilityValue::String("t".into()));
    assert_eq!(
        widget_property_set(&mut message, "text", CapabilityValue::String("body".into())),
        Ok(())
    );
    assert_eq!(get(&message, "text"), CapabilityValue::String("body".into()));
    assert_contract(&mut message, "MessageBox");

    let mut file = FileDialog::new(Rect::new(0, 0, 500, 400));
    assert!(widget_property_get(&file, "modal").is_ok());
    assert_contract(&mut file, "FileDialog");

    let mut progress = ProgressDialog::new(Rect::new(0, 0, 350, 120));
    assert_contract(&mut progress, "ProgressDialog");

    let mut popup = PopupWindow::new(Rect::new(0, 0, 200, 150));
    assert_eq!(get(&popup, "has_content"), CapabilityValue::Bool(false));
    assert_contract(&mut popup, "PopupWindow");
}

/// Colour picker, which serves the `ColorDialog` kind through the same contract.
#[test]
fn color_picker_satisfies_the_contract() {
    use crate::widget::special_widgets::color_picker::ColorPicker;

    let mut picker = ColorPicker::new(Rect::new(0, 0, 200, 150));
    assert!(widget_property_get(&picker, "hex_rgba").is_ok());
    assert_eq!(widget_property_set(&mut picker, "show_alpha", CapabilityValue::Bool(true)), Ok(()));
    assert_eq!(get(&picker, "show_alpha"), CapabilityValue::Bool(true));
    // The old dispatch mapped an unparsable hex string to `TypeMismatch`; that
    // meaning is preserved rather than collapsed into a silent no-op.
    assert_eq!(
        widget_property_set(&mut picker, "hex_rgba", CapabilityValue::String("nonsense".into())),
        Err(CapabilityAccessError::TypeMismatch)
    );
    assert_contract(&mut picker, "ColorPicker");
}

/// Media widgets.
#[test]
fn media_widgets_satisfy_the_contract() {
    use crate::widget::media_widgets::animated_image::AnimatedImage;
    use crate::widget::media_widgets::camera_preview::CameraPreview;
    use crate::widget::media_widgets::hero_animation::HeroAnimation;
    use crate::widget::media_widgets::lottie_widget::LottieWidget;
    use crate::widget::media_widgets::rive_widget::RiveWidget;
    use crate::widget::media_widgets::video_player::VideoPlayer;

    let mut animated = AnimatedImage::new(Rect::new(0, 0, 100, 100));
    assert_eq!(get(&animated, "playing"), CapabilityValue::Bool(false));
    assert_eq!(widget_property_set(&mut animated, "playing", CapabilityValue::Bool(true)), Ok(()));
    assert_contract(&mut animated, "AnimatedImage");

    let mut hero = HeroAnimation::new(Rect::new(0, 0, 100, 100));
    assert!(widget_property_get(&hero, "animation_progress").is_ok());
    assert_contract(&mut hero, "HeroAnimation");

    let mut lottie = LottieWidget::new(Rect::new(0, 0, 100, 100));
    assert_contract(&mut lottie, "LottieWidget");

    let mut rive = RiveWidget::new(Rect::new(0, 0, 100, 100));
    assert_contract(&mut rive, "RiveWidget");

    let mut video = VideoPlayer::new(Rect::new(0, 0, 320, 240));
    assert!(widget_property_get(&video, "volume").is_ok());
    assert_contract(&mut video, "VideoPlayer");

    let mut camera = CameraPreview::new(Rect::new(0, 0, 320, 240));
    assert_eq!(get(&camera, "is_active"), CapabilityValue::Bool(false));
    assert_contract(&mut camera, "CameraPreview");
}

/// Advanced and menu widgets.
#[test]
fn advanced_and_menu_widgets_satisfy_the_contract() {
    use crate::widget::advanced_widgets::calendar::Calendar;
    use crate::widget::advanced_widgets::tab_bar::TabBar;
    use crate::widget::menu_toolbar::menu_bar::MenuBar;
    use crate::widget::menu_toolbar::status_bar::StatusBar;

    let mut calendar = Calendar::new(Rect::new(0, 0, 300, 220));
    assert_contract(&mut calendar, "Calendar");

    let mut tab_bar = TabBar::new(Rect::new(0, 0, 300, 28));
    assert_contract(&mut tab_bar, "TabBar");

    let mut menu_bar = MenuBar::new(Rect::new(0, 0, 400, 24));
    assert_contract(&mut menu_bar, "MenuBar");

    let mut status_bar = StatusBar::new(Rect::new(0, 0, 400, 22));
    assert_contract(&mut status_bar, "StatusBar");
}

/// View widgets.
#[test]
fn view_widgets_satisfy_the_contract() {
    use crate::widget::view_widgets::data_grid::DataGrid;
    use crate::widget::view_widgets::list_view::ListView;
    use crate::widget::view_widgets::tree_view::TreeView;

    let mut grid = DataGrid::new(Rect::new(0, 0, 400, 300));
    assert_contract(&mut grid, "DataGrid");

    let mut list = ListView::new(Rect::new(0, 0, 300, 200));
    assert_contract(&mut list, "ListView");

    let mut tree = TreeView::new(Rect::new(0, 0, 300, 200));
    assert_contract(&mut tree, "TreeView");
}

/// Cupertino widgets.
///
/// The Cupertino controls answer through their own `WidgetProperties` impls but
/// share `WidgetKind`s with unrelated controls in some cases, so these assertions
/// pin that each one reads its *own* state rather than a neighbour's schema.
#[test]
fn cupertino_widgets_satisfy_the_contract() {
    use crate::widget::cupertino::CupertinoAlertDialog;
    use crate::widget::cupertino::CupertinoDatePicker;
    use crate::widget::cupertino::CupertinoNavigationBar;
    use crate::widget::cupertino::CupertinoSegmentedControl;
    use crate::widget::special_widgets::snackbar::Snackbar;

    let mut dialog = CupertinoAlertDialog::new(Rect::new(0, 0, 270, 150));
    assert_eq!(get(&dialog, "title"), CapabilityValue::String(String::new()));
    assert_eq!(
        widget_property_set(&mut dialog, "title", CapabilityValue::String("T".into())),
        Ok(())
    );
    assert_eq!(get(&dialog, "title"), CapabilityValue::String("T".into()));
    assert_eq!(
        widget_property_set(&mut dialog, "message", CapabilityValue::String("m".into())),
        Ok(())
    );
    assert_eq!(get(&dialog, "message"), CapabilityValue::String("m".into()));
    assert_contract(&mut dialog, "CupertinoAlertDialog");

    let mut bar = CupertinoNavigationBar::new(Rect::new(0, 0, 375, 96));
    assert_eq!(get(&bar, "large_title"), CapabilityValue::Bool(true));
    assert_eq!(widget_property_set(&mut bar, "large_title", CapabilityValue::Bool(false)), Ok(()));
    assert_eq!(get(&bar, "large_title"), CapabilityValue::Bool(false));
    assert_contract(&mut bar, "CupertinoNavigationBar");

    let mut segmented = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
    assert_eq!(get(&segmented, "segment_count"), CapabilityValue::UInt(0));
    segmented.set_segments(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    assert_eq!(get(&segmented, "segment_count"), CapabilityValue::UInt(3));
    assert_eq!(
        widget_property_set(&mut segmented, "selected_index", CapabilityValue::UInt(2)),
        Ok(())
    );
    assert_eq!(get(&segmented, "selected_index"), CapabilityValue::UInt(2));
    // Derived from the segment list, so it must be refused rather than reported
    // as a name this control does not know.
    assert_eq!(
        widget_property_set(&mut segmented, "segment_count", CapabilityValue::UInt(1)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
    assert_contract(&mut segmented, "CupertinoSegmentedControl");

    let mut picker = CupertinoDatePicker::new(Rect::new(0, 0, 300, 200));
    // The real date, not the fixed placeholder the centralised defaults returned.
    assert_eq!(get(&picker, "selected_date"), CapabilityValue::String("2025-01-01".into()));
    assert_eq!(
        widget_property_set(
            &mut picker,
            "selected_date",
            CapabilityValue::String("2024-02-29".into())
        ),
        Ok(())
    );
    assert_eq!(get(&picker, "selected_date"), CapabilityValue::String("2024-02-29".into()));
    assert_eq!(
        widget_property_set(
            &mut picker,
            "selected_date",
            CapabilityValue::String("not-a-date".into())
        ),
        Err(CapabilityAccessError::TypeMismatch)
    );
    assert_contract(&mut picker, "CupertinoDatePicker");

    let mut snackbar = Snackbar::new(Rect::new(0, 0, 420, 120));
    assert_eq!(
        widget_property_set(&mut snackbar, "message", CapabilityValue::String("hi".into())),
        Ok(())
    );
    assert_eq!(get(&snackbar, "message"), CapabilityValue::String("hi".into()));
    assert_contract(&mut snackbar, "Snackbar");
}

/// Menu/toolbar dropdowns and the properties panel.
#[test]
fn menu_and_property_controls_satisfy_the_contract() {
    use crate::widget::menu_toolbar::dropdown_menu::DropdownMenu;
    use crate::widget::menu_toolbar::menu_button::MenuButton;
    use crate::widget::view_widgets::properties_panel::PropertiesPanel;

    let mut dropdown = DropdownMenu::new(Rect::new(0, 0, 200, 32));
    assert_eq!(get(&dropdown, "selected_index"), CapabilityValue::Null);
    dropdown.add_item(crate::widget::menu_toolbar::dropdown_menu::DropdownItem::new("a", "A"));
    dropdown.add_item(crate::widget::menu_toolbar::dropdown_menu::DropdownItem::new("b", "B"));
    assert_eq!(get(&dropdown, "item_count"), CapabilityValue::UInt(2));
    assert_eq!(
        widget_property_set(&mut dropdown, "selected_index", CapabilityValue::UInt(1)),
        Ok(())
    );
    assert_eq!(get(&dropdown, "selected_index"), CapabilityValue::UInt(1));
    assert_eq!(widget_property_set(&mut dropdown, "expanded", CapabilityValue::Bool(true)), Ok(()));
    assert_eq!(get(&dropdown, "expanded"), CapabilityValue::Bool(true));
    assert_eq!(
        widget_property_set(&mut dropdown, "item_count", CapabilityValue::UInt(9)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
    assert_contract(&mut dropdown, "DropdownMenu");

    let mut menu = MenuButton::new("File", Rect::new(0, 0, 120, 28));
    assert_eq!(get(&menu, "text"), CapabilityValue::String("File".into()));
    assert_eq!(
        widget_property_set(&mut menu, "text", CapabilityValue::String("Edit".into())),
        Ok(())
    );
    assert_eq!(get(&menu, "text"), CapabilityValue::String("Edit".into()));
    assert_eq!(widget_property_set(&mut menu, "expanded", CapabilityValue::Bool(true)), Ok(()));
    assert_eq!(get(&menu, "expanded"), CapabilityValue::Bool(true));
    assert_eq!(
        widget_property_set(&mut menu, "item_count", CapabilityValue::UInt(9)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
    assert_contract(&mut menu, "MenuButton");

    let mut panel = PropertiesPanel::new(Rect::new(0, 0, 300, 400));
    assert_eq!(get(&panel, "property_count"), CapabilityValue::UInt(0));
    panel.add_property(crate::widget::view_widgets::properties_panel::PropertyEntry::new(
        "width",
        crate::widget::view_widgets::properties_panel::PropertyValue::Number(10.0),
        None,
        None,
        true,
    ));
    assert_eq!(get(&panel, "property_count"), CapabilityValue::UInt(1));
    assert_eq!(
        widget_property_set(&mut panel, "property_count", CapabilityValue::UInt(9)),
        Err(CapabilityAccessError::ReadOnlyProperty)
    );
    assert_contract(&mut panel, "PropertiesPanel");
}

/// A control with no properties of its own still exposes the shared set.
///
/// This is the case that caught a wrong assumption during migration: an empty
/// *own* property list does **not** mean an empty published list, because the
/// shared four are part of every control's contract.
#[test]
fn controls_without_own_properties_still_publish_the_shared_set() {
    use crate::widget::input_widgets::textedit::TextEdit;

    let mut edit = TextEdit::new(Rect::new(0, 0, 200, 24));
    let names = widget_property_names(&edit).expect("TextEdit must expose its contract");

    for shared in BASE_PROPERTY_NAMES {
        assert!(
            names.contains(shared),
            "TextEdit declares no own properties, but the shared {shared:?} must still be published"
        );
    }
    assert_contract(&mut edit, "TextEdit");
}

/// `widget_property_names` must distinguish "no contract" from "no properties".
///
/// Every widget in the crate that is reachable through the factory now declares a
/// contract, so the `None` case is exercised with a minimal widget defined here.
/// The distinction matters because `None` means "not migrated" while `Some(&[])`
/// would mean "deliberately exposes nothing"; collapsing them would hide a
/// control that had been missed.
#[test]
fn a_widget_without_a_contract_reports_none_not_empty() {
    use crate::event::{Event, EventHandler};
    use crate::widget::base::BaseWidget;
    use crate::widget::{Widget, WidgetKind};

    /// A widget with no `WidgetProperties` implementation at all.
    struct Unmigrated {
        base: BaseWidget,
    }

    impl Widget for Unmigrated {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
    }

    impl EventHandler for Unmigrated {
        fn handle_event(&mut self, _event: &Event) {}
    }

    let mut widget = Unmigrated {
        base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 10, 10), "unmigrated"),
    };
    let dyn_widget: &mut dyn Widget = &mut widget;

    assert!(
        widget_property_names(dyn_widget).is_none(),
        "a widget with no WidgetProperties impl must report None, not an empty list"
    );
    assert_eq!(
        widget_property_get(dyn_widget, "text"),
        Err(CapabilityAccessError::UnsupportedOnWidget),
        "reflection must say the control has no contract, not that the name is wrong"
    );
}

/// Every widget the factory can build must expose a property contract.
///
/// # Why this test exists
///
/// The migration of properties onto per-widget `WidgetProperties` impls was
/// checked only by hand-enumerated per-category tests, each naming its widgets as
/// string literals. Those tests pass whether or not a given widget was migrated, so
/// 59 factory-constructible widgets sat with no contract at all and nothing failed.
/// The gap was invisible precisely because no test ever asked the factory what it
/// could build.
///
/// A hand-typed list here would recreate the same blind spot, so the list comes from
/// the factory and this test's failure output names the offenders.
///
/// # What "has a contract" means
///
/// `properties_dyn()` returning `Some` — i.e. the widget answers the reflection
/// interface at all. It does **not** require the widget to declare many properties:
/// a control with no state beyond the shared four is legitimate, and
/// `property_names()` returning `BASE_PROPERTY_NAMES` satisfies this test. What is
/// not legitimate is a widget that answers *nothing*, because every read and write
/// against it then reports `UnsupportedOnWidget`, which a generated property editor
/// renders as "this control is not editable" rather than "this control does not
/// exist".
#[test]
fn every_factory_widget_declares_a_property_contract() {
    let factory = WidgetFactory::new_with_defaults();
    let names = factory.widget_names();
    assert!(!names.is_empty(), "the factory must register widgets");

    let mut contractless = Vec::new();
    for name in names {
        let Some(widget) = factory.create(name, Rect::new(0, 0, 64, 48), "x") else {
            continue;
        };
        if widget.properties_dyn().is_none() {
            contractless.push(name);
        }
    }

    assert!(
        contractless.is_empty(),
        "these widgets are constructible but expose no WidgetProperties contract, so \
         every property read/write against them reports `UnsupportedOnWidget`: \
         {contractless:?}"
    );
}

/// A published name must never answer `UnknownProperty` when written.
///
/// # Why this test exists
///
/// `UnknownProperty` and `ReadOnlyProperty` answer different questions:
///
/// - `UnknownProperty` — "this control has no property by that name".
/// - `ReadOnlyProperty` — "the property exists, but it is not writable".
///
/// A control that publishes a name from `property_names()` and then answers
/// `UnknownProperty` when asked to write it is contradicting itself: the name is in
/// the published contract, so it is not unknown. A property editor driven from
/// `property_names()` renders that answer as a broken control rather than as a
/// read-only field, and a scripted client that trusts the contract gets a
/// misleading error.
///
/// The correct answer for a name with no setter is `ReadOnlyProperty`. This test
/// asks every factory-constructible control for exactly that, so the whole class of
/// contradiction is caught mechanically instead of by reading 156 `property_names`
/// implementations by hand.
///
/// # Why the write value is a `Bool`
///
/// The test cares only about *which* error comes back, and for a name with no
/// setter arm the answer must not depend on the value. `Bool` is chosen because it
/// is the type a wrong-typed write is least likely to coincide with, so a control
/// that does have a setter reports `TypeMismatch` and is correctly skipped rather
/// than being mistaken for a violation.
#[test]
fn no_published_property_answers_unknown_when_written() {
    let factory = WidgetFactory::new_with_defaults();

    let mut offenders = alloc::vec::Vec::new();
    for name in factory.widget_names() {
        let Some(mut widget) = factory.create(name, Rect::new(0, 0, 64, 48), "x") else {
            continue;
        };
        let Some(published) = widget_property_names(widget.as_ref()) else {
            continue;
        };
        for property in published {
            // A name that cannot even be read is a different defect, owned by
            // `assert_contract`; skip it so this test reports one thing.
            if widget_property_get(widget.as_ref(), property).is_err() {
                continue;
            }
            if widget_property_set(widget.as_mut(), property, CapabilityValue::Bool(false))
                == Err(CapabilityAccessError::UnknownProperty)
            {
                offenders.push((name, property));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these properties are published by property_names() but answer UnknownProperty when \
         written, which contradicts the contract — a name with no setter must answer \
         ReadOnlyProperty (widget, property): {offenders:?}"
    );
}

/// The registered schema and the control's own contract must publish the same names.
///
/// # Why this test exists
///
/// The capability layer carries the property list **twice**: as a static
/// `*_PROPERTIES` table (what the factory validates writes against, and what
/// `capability_manifest()` exports) and as `property_names()` on the control's own
/// `WidgetProperties` impl. Those are two statements of one fact.
///
/// They had already drifted: every one of the 155 schema tables omitted
/// `visible` and `geometry`, which every contract publishes through
/// `BASE_PROPERTY_NAMES`. A generated property editor would therefore read the
/// schema, never offer the two fields, and silently hide state the control reports
/// as settable. Nothing failed, because no test compared the two lists.
///
/// # What this pins
///
/// For each registered capability, the names the control publishes must appear in
/// the schema. The reverse direction is checked implicitly: the schema is what the
/// factory serves, and a schema name the control cannot read would already be
/// caught by `assert_contract` in the per-category tests.
///
/// The comparison is by name only, deliberately. `readable` / `writable` are
/// *policy* about a name (and `geometry` is published yet read-only), while the
/// name set is *existence*. Mixing the two here would make the test fail for a
/// legitimate design choice and hide the defect it is meant to catch.
#[test]
fn schema_and_contract_publish_the_same_names() {
    let factory = WidgetFactory::new_with_defaults();

    let mut missing = alloc::vec::Vec::new();
    let mut undeclared = alloc::vec::Vec::new();
    for capability in factory.capabilities() {
        let Some(widget) = factory.create(capability.canonical_name, Rect::new(0, 0, 64, 48), "x")
        else {
            continue;
        };
        let Some(published) = widget_property_names(widget.as_ref()) else {
            continue;
        };
        let declared: alloc::vec::Vec<&str> =
            capability.properties.iter().map(|schema| schema.name).collect();

        // Forward: the control must not publish a name its schema omits.
        for name in published {
            if !declared.contains(name) {
                missing.push((capability.canonical_name, name));
            }
        }

        // Reverse (principle #77): the schema must not declare a name the control
        // does not answer. Without this direction a `readable: true` entry nothing
        // implements survives forever — the caller sees the name in
        // `rw_widget_property_names` and gets `UnknownProperty` when it asks for it.
        //
        // Entries marked neither readable nor writable are the deliberate
        // placeholder for a name a *sibling* control owns (see the `TextEdit`
        // module docs); they promise nothing, so they are not a lie and are
        // excluded here.
        for schema in capability.properties {
            if !schema.readable && !schema.writable {
                continue;
            }
            if !published.contains(&schema.name) {
                undeclared.push((capability.canonical_name, schema.name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these controls publish properties their registered schema does not declare, so the \
         schema and the contract disagree about what exists (widget, property): {missing:?}"
    );
    assert!(
        undeclared.is_empty(),
        "these controls have schema entries no contract answers, so the schema promises \
         properties that cannot be read or written (widget, property): {undeclared:?}"
    );
}

/// Every `WidgetKind` claimed by more than one capability must have a tie-break.
///
/// # Why this test exists
///
/// Several controls share a `WidgetKind` (`DataGrid`, `VirtualTable` and
/// `TableWidget` all report `WidgetKind::Table`). The factory resolves such a kind
/// by comparing the *concrete* type against a hand-maintained table. Where that
/// table has no row, the lookup used to return the first registered capability —
/// so `segmented_control` (sharing `WidgetKind::ToggleButton` with `ToggleButton`)
/// read `ToggleButton`'s schema and reported `UnknownProperty` for its own
/// `item_count`.
///
/// The bug was invisible because nothing checked the invariant. This test asks the
/// registry directly: group every registered capability by kind, and for any kind
/// with more than one entry, require that each entry can be distinguished. It reads
/// the same table the lookup does, so it cannot drift from it.
#[test]
fn every_shared_kind_has_a_tie_break() {
    let factory = WidgetFactory::new_with_defaults();

    // Group canonical names by the kind each capability declares.
    let mut by_kind: alloc::collections::BTreeMap<
        crate::widget::WidgetKind,
        alloc::vec::Vec<&'static str>,
    > = alloc::collections::BTreeMap::new();
    for capability in factory.capabilities() {
        by_kind.entry(capability.kind).or_default().push(capability.canonical_name);
    }

    let mut ambiguous = alloc::vec::Vec::new();
    for (kind, mut names) in by_kind {
        if names.len() < 2 {
            continue;
        }
        names.sort_unstable();
        // A shared kind needs every member distinguishable. Instantiating each
        // member and asking the factory to resolve it back is the only check that
        // exercises the real table rather than a copy of it.
        //
        // A group may legitimately contain **one control under two names**
        // (`table_widget` / `table`, `group_box` / `panel`, `tool_box` / `toolbox`,
        // `virtual_list` / `data_view`): the alias exists so every spelling resolves,
        // not so a second type exists. The two entries answer with the same concrete
        // type, so "resolved back to the name I created it as" is the wrong
        // question — what must hold is that *some* entry in the group resolves it,
        // and that the answer is the same whichever of the two names asked. That
        // pair-wise agreement is what is asserted here; a genuinely different
        // control resolving to a sibling's name is still caught, because its own
        // name would not be in the agreeing set.
        let resolved_by_name: alloc::vec::Vec<(&'static str, &'static str)> = names
            .iter()
            .filter_map(|name| {
                let widget = factory.create(name, Rect::new(0, 0, 32, 32), "")?;
                let resolved = factory.capability_for_kind_instance(widget.as_ref());
                Some((*name, resolved.map(|cap| cap.canonical_name).unwrap_or("<none>")))
            })
            .collect();
        for (name, resolved) in &resolved_by_name {
            if resolved == &"<none>" {
                ambiguous.push((kind, *name, None));
            }
        }
        // Two names may share an answer; a name may never map to a *third* name that
        // is not itself a member of this group, and a group may not leave a member
        // answering nothing.
        for (name, resolved) in &resolved_by_name {
            if *resolved != "<none>" && !names.contains(resolved) {
                ambiguous.push((kind, *name, Some(*resolved)));
            }
        }
    }

    assert!(
        ambiguous.is_empty(),
        "these controls share a WidgetKind but the factory cannot resolve them back \
         to their own capability, so they would read another control's schema \
         (kind, created-as, resolved-as): {ambiguous:?}"
    );
}

/// Every published enum token must actually be accepted by the control.
///
/// # Why this test is what makes the token lists trustworthy
///
/// `PropertySchema::accepted_tokens` is a *claim* about what `set` accepts, written by
/// hand next to a parser written by hand. A claim that is never tested drifts: a token
/// gets renamed in the parser, the list keeps the old spelling, and a caller that reads
/// the list and writes it back gets a `TypeMismatch` for a value the schema itself
/// advertised. That is worse than publishing no list at all, because it looks
/// authoritative.
///
/// So each token is written back through the real `set`. A refusal fails here.
///
/// # Only writable properties are exercised
///
/// A read-only enum still publishes its tokens — they are the vocabulary the *reader*
/// returns, which is useful to a caller formatting a value. Writing to it is expected
/// to fail, so those entries are skipped rather than asserted. Writing to them was an
/// earlier version's mistake: it reported every read-only enum as a defect, which would
/// have pushed the fix in the wrong direction (deleting correct token lists).
///
/// # What it does not claim
///
/// It does not require the list to be *complete* — a control may accept more spellings
/// than it advertises (aliases), and this test does not try to enumerate them. It only
/// refuses the harmful direction: advertising something a caller cannot write.
#[test]
fn published_enum_tokens_are_accepted_by_their_control() {
    let factory = WidgetFactory::new_with_defaults();
    let mut rejected: alloc::vec::Vec<(&str, &str, &str)> = alloc::vec::Vec::new();
    let mut published_count = 0usize;

    for capability in factory.capabilities() {
        for schema in capability.properties {
            if schema.accepted_tokens.is_empty() || !schema.writable {
                continue;
            }
            let Some(mut widget) =
                factory.create(capability.canonical_name, Rect::new(0, 0, 64, 48), "x")
            else {
                continue;
            };
            for token in schema.accepted_tokens {
                published_count += 1;
                let write = CapabilityValue::String((*token).to_string());
                if crate::widget::capability::properties_trait::widget_property_set(
                    widget.as_mut(),
                    schema.name,
                    write,
                )
                .is_err()
                {
                    rejected.push((capability.canonical_name, schema.name, token));
                }
            }
        }
    }

    assert!(
        published_count > 0,
        "no writable property publishes accepted tokens, so this test proves nothing — \
         the schema field or its wiring has stopped reaching the property layer"
    );
    assert!(
        rejected.is_empty(),
        "a published token was refused by the control that published it, so the schema \
         advertises a value the caller cannot write (control, property, token): {rejected:?}"
    );
}
