//! Integration tests for rust_widgets.
//!
//! These tests exercise the library's core APIs (types, geometry, events, error,
//! quality, i18n, layout, memory, clipboard, and style) which work without a
//! running platform backend.

use rust_widgets::core::*;
use rust_widgets::error::*;
use rust_widgets::event::*;
#[cfg(not(feature = "embedded"))]
use rust_widgets::i18n::*;
use rust_widgets::quality::*;

// ---------------------------------------------------------------------------
// Core type integration
// ---------------------------------------------------------------------------

#[test]
fn test_point_size_rect() {
    let pt = Point::new(10, 20);
    assert_eq!(pt.x, 10);
    assert_eq!(pt.y, 20);

    let sz = Size::new(800, 600);
    assert_eq!(sz.width, 800);
    assert_eq!(sz.height, 600);

    let rect = Rect::new(10, 20, 800, 600);
    assert_eq!(rect.x, 10);
    assert_eq!(rect.y, 20);
    assert_eq!(rect.width, 800);
    assert_eq!(rect.height, 600);
}

#[test]
fn test_color_basics() {
    let color = Color::rgb(255, 128, 64);
    assert_eq!(color.r, 255);
    assert_eq!(color.g, 128);
    assert_eq!(color.b, 64);
    assert_eq!(color.a, 255);
}

#[test]
fn test_color_named_constants() {
    assert!(Color::BLACK.is_dark());
    assert!(Color::WHITE.is_light());
}

#[test]
fn test_color_lerp() {
    let a = Color::rgb(0, 0, 0);
    let b = Color::rgb(255, 255, 255);
    let mid = a.blend(&b, 0.5);
    // Midpoint of black -> white should be ~128 each channel
    assert!((mid.r as i16 - 128).abs() <= 2);
    assert!((mid.g as i16 - 128).abs() <= 2);
    assert!((mid.b as i16 - 128).abs() <= 2);
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

#[test]
fn test_rect_contains() {
    let rect = Rect::new(0, 0, 100, 100);
    assert!(rect.contains(Point::new(50, 50)));
    assert!(rect.contains(Point::new(0, 0)));
    assert!(rect.contains(Point::new(99, 99)));
    assert!(!rect.contains(Point::new(100, 100)));
    assert!(!rect.contains(Point::new(-1, 50)));
}

#[test]
fn test_rect_intersection() {
    let a = Rect::new(0, 0, 100, 100);
    let b = Rect::new(50, 50, 100, 100);
    let intersection = a.intersection(&b);
    assert!(intersection.is_some());
    let inter = intersection.unwrap();
    assert_eq!(inter.x, 50);
    assert_eq!(inter.y, 50);
    assert_eq!(inter.width, 50);
    assert_eq!(inter.height, 50);
}

#[test]
fn test_orientation() {
    assert_eq!(Orientation::Horizontal, Orientation::Horizontal);
    assert_eq!(Orientation::Vertical, Orientation::Vertical);
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

#[test]
fn test_event_mouse_press() {
    let press = Event::MousePress { pos: Point::new(10, 20), button: 0 };
    match press {
        Event::MousePress { pos, button } => {
            assert_eq!(pos.x, 10);
            assert_eq!(pos.y, 20);
            assert_eq!(button, 0);
        }
        _ => panic!("unexpected event variant"),
    }
}

#[test]
fn test_event_custom() {
    let custom = Event::Custom { name: "test".into(), payload: vec![1, 2, 3] };
    match custom {
        Event::Custom { ref name, ref payload } => {
            assert_eq!(name, "test");
            assert_eq!(payload, &[1, 2, 3]);
        }
        _ => panic!("unexpected event variant"),
    }
}

#[test]
fn test_event_priority() {
    assert_eq!(EventPriority::High as u8, 0);
    assert_eq!(EventPriority::Normal as u8, 1);
    assert_eq!(EventPriority::Idle as u8, 2);
}

// ---------------------------------------------------------------------------
// Error system
// ---------------------------------------------------------------------------

#[test]
fn test_error_id_constants() {
    assert_eq!(ErrorId::SUCCESS.0, 0);
    assert_eq!(ErrorId::NOT_IMPLEMENTED.0, 1);
    assert_eq!(ErrorId::INVALID_ARGUMENT.0, 3);
}

#[test]
fn test_rw_error_creation() {
    let err = RwError::new(ErrorId::NOT_IMPLEMENTED, "test feature");
    assert!(err.message.contains("test feature"));
    assert_eq!(err.id.0, 1);
}

#[test]
fn test_rw_error_not_implemented() {
    let err = RwError::not_implemented("my_feature");
    assert!(err.message.contains("my_feature"));
    assert_eq!(err.id, ErrorId::NOT_IMPLEMENTED);
}

// ---------------------------------------------------------------------------
// Quality management
// ---------------------------------------------------------------------------

#[test]
fn test_quality_manager_default() {
    let manager = QualityManager::new();
    assert_eq!(manager.quality_level(), QualityLevel::Medium);
    assert_eq!(manager.current_fps(), 0.0);
}

#[test]
fn test_quality_manager_custom_config() {
    let config = QualityConfig::default();
    let manager = QualityManager::with_config(config);
    assert_eq!(manager.quality_level(), QualityLevel::Medium);
}

#[test]
fn test_quality_level_ordering() {
    assert!(QualityLevel::Low < QualityLevel::Medium);
    assert!(QualityLevel::Medium < QualityLevel::High);
    assert!(QualityLevel::Low < QualityLevel::High);
}

#[test]
fn test_quality_manager_degrade() {
    let mut manager = QualityManager::new();
    // Simulate 5 slow frames to trigger degrade from Medium -> Low
    let slow_frame = 2.0 / 60.0; // 2x target frame duration
    for _ in 0..10 {
        manager.finish_frame_secs(slow_frame);
    }
    assert_eq!(manager.quality_level(), QualityLevel::Low);
}

// ---------------------------------------------------------------------------
// i18n basics (no file system needed)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "embedded"))]
#[test]
fn test_i18n_manager_create() {
    let manager = I18nManager::new();
    // Without translations, translate returns the key itself
    assert_eq!(manager.translate("hello"), "hello");
    assert_eq!(manager.translate(""), "");
}

#[cfg(not(feature = "embedded"))]
#[test]
fn test_i18n_manager_set_language() {
    let mut manager = I18nManager::new();
    manager.set_language("fr");
    assert_eq!(manager.current_language(), "fr");
}

// ---------------------------------------------------------------------------
// Error display
// ---------------------------------------------------------------------------

#[test]
fn test_rw_error_display_format() {
    let err = RwError::new(ErrorId::INVALID_ARGUMENT, "bad input");
    let display = format!("{}", err);
    assert!(display.contains("003")); // INVALID_ARGUMENT = 3
    assert!(display.contains("bad input"));
}

#[test]
fn test_rw_error_not_implemented_display() {
    let err = RwError::not_implemented("fancy_feature");
    let display = format!("{}", err);
    assert!(display.contains("001")); // NOT_IMPLEMENTED = 1
    assert!(display.contains("fancy_feature"));
}

// ---------------------------------------------------------------------------
// Widget creation chaining (no platform needed — purely logical)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mini"))]
#[test]
fn test_app_new() {
    // App can be created without a running platform
    use rust_widgets::app::App;
    let _app = App::new();
    // App::new() should not panic — construction succeeds without platform
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_app_with_config() {
    use rust_widgets::app::{App, AppConfig};
    let app = App::with_config(
        AppConfig::default().with_app_name("TestApp").with_organization("TestOrg"),
    );
    assert_eq!(app.config().app_name, "TestApp");
    assert_eq!(app.config().organization, "TestOrg");
    assert!(app.config().enable_i18n);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_app_with_config_i18n_disabled() {
    use rust_widgets::app::{App, AppConfig};
    let app = App::with_config(AppConfig::default().with_i18n(false));
    assert!(!app.config().enable_i18n);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_app_default_is_new() {
    use rust_widgets::app::App;
    let a = App::default();
    let b = App::new();
    // Both constructs the same — config should match defaults
    assert_eq!(a.config().app_name, b.config().app_name);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_window_handle_roundtrip() {
    use rust_widgets::app::WindowHandle;
    // Create a window handle from raw id — no platform needed.
    let win = WindowHandle::from_raw(42);
    assert_eq!(win.raw_id(), 42);
    // Create another handle from same raw id.
    let win2 = WindowHandle::from_raw(win.raw_id());
    assert_eq!(win2.raw_id(), win.raw_id());
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_widget_handle_trait_methods() {
    use rust_widgets::app::ButtonHandle;
    use rust_widgets::app::WidgetHandle;
    // ButtonHandle created from raw id (no platform needed)
    let btn = ButtonHandle::from_raw(1);
    assert_eq!(btn.raw_id(), 1);
    // Without a platform, is_visible/is_enabled return platform defaults
    // (stub returns false). Just verify the methods are callable.
    let _visible = btn.is_visible();
    let _enabled = btn.is_enabled();
    // Without platform, set_text stores nothing (stub returns ""), but should not crash
    btn.set_text("hello");
    btn.set_visible(true);
    btn.disable();
    btn.enable();
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_message_box_handle_no_widget_methods() {
    use rust_widgets::app::MessageBoxHandle;
    // MessageBoxHandle must NOT have set_text/enable/disable from the macro
    let mb = MessageBoxHandle::from_raw(99);
    assert_eq!(mb.raw_id(), 99);
    mb.set_title("Error");
    // Just verify it compiles and basic operations exist
    mb.show_modal();
    mb.close();
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_combo_box_handle_specific_ops() {
    use rust_widgets::app::ComboBoxHandle;
    let cb = ComboBoxHandle::from_raw(10);
    // Without platform these return defaults, but should not panic
    let _ = cb.add_item("A");
    let _ = cb.current_index();
    let _ = cb.item_count();
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_list_box_handle_specific_ops() {
    use rust_widgets::app::ListBoxHandle;
    let lb = ListBoxHandle::from_raw(11);
    let _ = lb.add_item("X");
    let _ = lb.remove_item(0);
    let _ = lb.clear_items();
    let _ = lb.current_index();
    let _ = lb.item_count();
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_handle_from_raw_and_back() {
    use rust_widgets::app::*;
    let btn = ButtonHandle::from_raw(7);
    assert_eq!(btn.raw_id(), 7);
    let cb = CheckBoxHandle::from_raw(8);
    assert_eq!(cb.raw_id(), 8);
    let slider = SliderHandle::from_raw(9);
    assert_eq!(slider.raw_id(), 9);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_handle_debug_and_clone() {
    use rust_widgets::app::ButtonHandle;
    let btn = ButtonHandle::from_raw(5);
    let btn2 = btn.clone(); // Clone
    assert_eq!(btn, btn2);
    let _ = format!("{:?}", btn); // Debug
}

// ---------------------------------------------------------------------------
// JSON declarative engine integration tests (BLUE4.md §11.2)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_loads_minimal_window() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{"window":{"id":"main","title":"Hello","width":400,"height":300}}"#;
    let layout = JsonLoader::load(json).expect("minimal window should load");
    assert!(!layout.is_empty(), "at least one widget registered");
    // The window's id is "main", findable via button() (WindowHandle not exposed as convenience)
    assert!(layout.id("main").is_some(), "window id should be registered");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_loads_button_and_label() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{
        \"window\": {
            \"id\": \"main\", \"title\": \"Test\", \"width\": 400, \"height\": 300,
            \"children\": [
                {\"button\": {\"id\": \"btn_ok\", \"text\": \"OK\", \"x\": 10, \"y\": 10, \"width\": 80, \"height\": 30}},
                {\"label\": {\"id\": \"status\", \"text\": \"Ready\", \"x\": 10, \"y\": 50, \"width\": 200, \"height\": 24}}
            ]
        }
    }"#;
    let json = json.replace('\\', "");
    let layout = JsonLoader::load(&json).expect("window with children should load");
    layout.button("btn_ok").expect("button should be findable");
    layout.label("status").expect("label should be findable");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_loads_with_layout_vbox() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{
        "window": {
            "id": "main", "title": "Layout", "width": 400, "height": 300,
            "layout": {"type": "vbox", "spacing": 8, "margin": 4},
            "children": [
                {"button": {"id": "btn1", "text": "One", "stretch": 1}},
                {"button": {"id": "btn2", "text": "Two", "stretch": 1}}
            ]
        }
    }"#;
    let layout = JsonLoader::load(json).expect("window with vbox should load");
    layout.button("btn1").expect("btn1 should exist");
    layout.button("btn2").expect("btn2 should exist");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_loads_all_widget_types() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{
        "window": {
            "id": "w", "title": "All", "width": 600, "height": 500,
            "children": [
                {"checkbox": {"id": "cb", "text": "Check", "checked": true, "x":0,"y":0,"width":100,"height":30}},
                {"radiobutton": {"id": "rb", "text": "Radio", "checked": false, "x":0,"y":40,"width":100,"height":30}},
                {"lineedit": {"id": "le", "value": "hello", "placeholder": "Type...", "x":0,"y":80,"width":200,"height":30}},
                {"combobox": {"id": "combo", "items": ["A","B","C"], "x":0,"y":120,"width":150,"height":30}},
                {"slider": {"id": "sl", "min": 0, "max": 100, "value": 50, "x":0,"y":160,"width":200,"height":30}},
                {"progressbar": {"id": "pb", "min": 0, "max": 100, "value": 75, "x":0,"y":200,"width":200,"height":30}},
                {"listbox": {"id": "lb", "items": ["X","Y"], "selection_mode": "single", "x":0,"y":240,"width":150,"height":80}},
                {"groupbox": {"id": "gb", "title": "Group", "x":0,"y":330,"width":200,"height":100}},
                {"grid": {"id": "g", "rows": 3, "columns": 4, "spacing": 2, "x":300,"y":0,"width":200,"height":200}}
            ]
        }
    }"#;
    let layout = JsonLoader::load(json).expect("all widget types should load");
    layout.checkbox("cb").expect("checkbox should exist");
    layout.radio_button("rb").expect("radiobutton should exist");
    layout.line_edit("le").expect("lineedit should exist");
    layout.combo_box("combo").expect("combobox should exist");
    layout.slider("sl").expect("slider should exist");
    layout.progress_bar("pb").expect("progressbar should exist");
    layout.list_box("lb").expect("listbox should exist");
    let grid_id = layout.id("g").expect("grid id should be registered");
    assert!(grid_id >= 1);
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_event_handler_fires() {
    use rust_widgets::json::{invoke_global_handler, register_global_handler, EventHandlerContext};
    use rust_widgets::WidgetTriggerEvent;

    let called = std::sync::atomic::AtomicBool::new(false);
    register_global_handler("test_handler", move |_: &EventHandlerContext| {
        called.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let ctx = EventHandlerContext::new(WidgetTriggerEvent {
        widget_id: 1,
        kind: rust_widgets::platform::WidgetTriggerKind::Clicked,
    });
    let invoked = invoke_global_handler("test_handler", &ctx);
    assert!(invoked, "registered handler should be invoked");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_unknown_handler_is_silent() {
    use rust_widgets::json::{invoke_global_handler, EventHandlerContext};
    use rust_widgets::WidgetTriggerEvent;
    let ctx = EventHandlerContext::new(WidgetTriggerEvent {
        widget_id: 1,
        kind: rust_widgets::platform::WidgetTriggerKind::Clicked,
    });
    let invoked = invoke_global_handler("does_not_exist", &ctx);
    assert!(!invoked, "unregistered handler should return false");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_parse_error_returns_error_message() {
    use rust_widgets::json::JsonLoader;
    let result = JsonLoader::load("not valid json");
    assert!(result.is_err(), "invalid JSON should produce an error");
    // The error type is String, not the layout
    match result {
        Err(msg) => assert!(msg.contains("JSON"), "error should mention JSON: {}", msg),
        Ok(_) => panic!("expected error"),
    }
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_widget_by_name_typed() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{"window":{"id":"main","title":"Typed","width":400,"height":300,"children":[{"button":{"id":"ok","text":"OK","x":0,"y":0,"width":80,"height":30}}]}}"#;
    let layout = JsonLoader::load(json).expect("should load");
    let _btn = layout.button("ok").expect("button should be findable by name");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_widget_by_name_not_found() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{"window":{"id":"main","title":"Test","width":400,"height":300}}"#;
    let layout = JsonLoader::load(json).expect("should load");
    let result = layout.widget_by_name::<rust_widgets::app::ButtonHandle>("nope");
    assert!(result.is_err(), "non-existent widget should return error");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_groupbox_title_property() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{"window":{"id":"w","title":"Main","width":400,"height":300,"children":[{"groupbox":{"id":"gb","title":"My Group","x":0,"y":0,"width":200,"height":100}}]}}"#;
    let layout = JsonLoader::load(json).expect("window with groupbox should load");
    layout.panel("gb").expect("groupbox should be findable");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_grid_with_spacing() {
    use rust_widgets::json::JsonLoader;
    let json = r##"{"grid":{"id":"g","rows":3,"columns":4,"spacing":4,"line_color":"#DCDCDC","width":300,"height":200}}"##;
    let layout = JsonLoader::load(json).expect("grid with spacing should load");
    let grid_id = layout.id("g").expect("grid id should be registered");
    assert!(grid_id >= 1, "grid should have a valid id");
}

#[cfg(not(feature = "mini"))]
#[test]
fn json_engine_convenience_methods() {
    use rust_widgets::json::JsonLoader;
    let json = r#"{
        "window": {
            "id": "main", "title": "Convenience", "width": 500, "height": 400,
            "children": [
                {"checkbox": {"id": "cb1", "x":0,"y":0,"width":100,"height":30}},
                {"lineedit": {"id": "le1", "x":0,"y":40,"width":200,"height":30}},
                {"slider": {"id": "sl1", "x":0,"y":80,"width":200,"height":30}},
                {"progressbar": {"id": "pb1", "x":0,"y":120,"width":200,"height":30}},
                {"combobox": {"id": "combo1", "x":0,"y":160,"width":150,"height":30}}
            ]
        }
    }"#;
    let layout = JsonLoader::load(json).expect("should load");
    layout.checkbox("cb1").expect("checkbox convenience");
    layout.line_edit("le1").expect("line_edit convenience");
    layout.slider("sl1").expect("slider convenience");
    layout.progress_bar("pb1").expect("progress_bar convenience");
    layout.combo_box("combo1").expect("combo_box convenience");
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_app_on_startup_and_shutdown_chaining() {
    use rust_widgets::app::App;
    // on_startup/on_shutdown return Self for chaining
    let _app = App::new().on_startup(|| { /* no-op */ }).on_shutdown(|| { /* no-op */ });
    // Chaining succeeds without panicking
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_dispatch_trigger_no_callback() {
    use rust_widgets::app::dispatch_trigger;
    use rust_widgets::platform::WidgetTriggerKind;
    // Without registered callback, should return false
    let result = dispatch_trigger(999, WidgetTriggerKind::Clicked);
    assert!(!result);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_all_widget_handle_types_exist() {
    // Compile-time check that all handle types are constructable
    use rust_widgets::app::*;
    let _ = ButtonHandle::from_raw(1);
    let _ = LabelHandle::from_raw(2);
    let _ = CheckBoxHandle::from_raw(3);
    let _ = RadioButtonHandle::from_raw(4);
    let _ = LineEditHandle::from_raw(5);
    let _ = ComboBoxHandle::from_raw(6);
    let _ = ListBoxHandle::from_raw(7);
    let _ = SliderHandle::from_raw(8);
    let _ = ProgressBarHandle::from_raw(9);
    let _ = PanelHandle::from_raw(10);
    let _ = SpinBoxHandle::from_raw(11);
    let _ = ListViewHandle::from_raw(12);
    let _ = ScrollAreaHandle::from_raw(13);
    let _ = MessageBoxHandle::from_raw(14);
}

#[cfg(not(feature = "mini"))]
#[test]
fn test_new_widgets_module_exports_exist() {
    // Verify that BLUE11 R10 new widgets are accessible from the widget crate root
    #[cfg(feature = "desktop")]
    {
        use rust_widgets::widget::{
            AppBar, Badge, BottomNavigationBar, BottomSheet, MobileDatePicker, NavigationDrawer,
            PullToRefresh, SearchBox, SkeletonLoader, Switch, FAB,
        };
        // Compile-time check: all types exist and are constructable
        let _ = Switch::new(rust_widgets::core::Rect::new(0, 0, 60, 30));
        let _ = SearchBox::new(rust_widgets::core::Rect::new(0, 0, 200, 30));
        let _ = Badge::new(rust_widgets::core::Rect::new(0, 0, 24, 24));
        let _ = FAB::new(rust_widgets::core::Rect::new(0, 0, 56, 56));
        let _ = SkeletonLoader::new(rust_widgets::core::Rect::new(0, 0, 200, 20));
        let _ = PullToRefresh::new(rust_widgets::core::Rect::new(0, 0, 300, 400));
        let _ = BottomSheet::new(rust_widgets::core::Rect::new(0, 0, 300, 400));
        let _ = BottomNavigationBar::new(rust_widgets::core::Rect::new(0, 0, 375, 56));
        let _ = NavigationDrawer::new(rust_widgets::core::Rect::new(0, 0, 300, 600));
        let _ = AppBar::new("Test", rust_widgets::core::Rect::new(0, 0, 375, 56));
        let _ = MobileDatePicker::new(rust_widgets::core::Rect::new(0, 0, 300, 200));
    }
}
