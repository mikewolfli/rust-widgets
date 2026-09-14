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
    widget_property_get, widget_property_names, widget_property_set, BASE_PROPERTY_NAMES,
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
