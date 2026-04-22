//! Comprehensive widget tests

use rust_widgets::core::{Point, Rect, Size};
use rust_widgets::test::{TestHarness, WidgetTester};
use rust_widgets::widget::{Button, Label, Slider, TextEdit, Widget};

#[test]
fn test_label_creation() {
    let label = Label::new("Test Label".to_string(), Rect::new(10, 10, 100, 30));

    assert_eq!(label.text(), "Test Label");
    assert_eq!(label.geometry(), Rect::new(10, 10, 100, 30));
    assert!(label.is_visible());
    assert!(label.is_enabled());
}

#[test]
fn test_label_visibility() {
    let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));

    label.hide();
    assert!(!label.is_visible());

    label.show();
    assert!(label.is_visible());
}

#[test]
fn test_button_creation() {
    let button = Button::new("Click Me".to_string(), Rect::new(10, 10, 100, 40));

    assert_eq!(button.text(), "Click Me");
    assert_eq!(button.geometry(), Rect::new(10, 10, 100, 40));
}

#[test]
fn test_button_activation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let mut button = Button::new("Test".to_string(), Rect::new(0, 0, 100, 40));
    let click_count = Arc::new(AtomicUsize::new(0));
    let click_count_clone = click_count.clone();

    button.activated.connect(move || {
        click_count_clone.fetch_add(1, Ordering::SeqCst);
    });

    // Simulate mouse press and release to trigger activation
    let mut harness = TestHarness::new();
    harness.send_mouse_click(50, 20, 0);
    harness.dispatch_to(&mut button);

    assert_eq!(click_count.load(Ordering::SeqCst), 1);
}

#[test]
fn test_slider_value() {
    let mut slider = Slider::new(Rect::new(0, 0, 200, 30));

    slider.set_range(0, 100);
    slider.set_value(50);

    assert_eq!(slider.value(), 50);

    slider.set_value(150);
    assert_eq!(slider.value(), 100); // Clamped to max

    slider.set_value(-10);
    assert_eq!(slider.value(), 0); // Clamped to min
}

#[test]
fn test_slider_range() {
    let mut slider = Slider::new(Rect::new(0, 0, 200, 30));

    assert_eq!(slider.min(), 0);
    assert_eq!(slider.max(), 100);

    slider.set_range(10, 200);
    assert_eq!(slider.min(), 10);
    assert_eq!(slider.max(), 200);
}

#[test]
fn test_text_edit_text() {
    let mut text_edit = TextEdit::new(Rect::new(0, 0, 200, 30));

    text_edit.set_text("Hello World".to_string());
    assert_eq!(text_edit.text(), "Hello World");

    text_edit.set_text("".to_string());
    assert!(text_edit.text().is_empty());
}

#[test]
fn test_widget_geometry() {
    let mut label = Label::new("Test".to_string(), Rect::new(10, 20, 100, 50));

    assert_eq!(label.position(), Point::new(10, 20));
    assert_eq!(label.size(), Size::new(100, 50));

    label.set_position(Point::new(30, 40));
    assert_eq!(label.position(), Point::new(30, 40));

    label.set_size(Size::new(200, 100));
    assert_eq!(label.size(), Size::new(200, 100));
}

#[test]
fn test_widget_enabled_state() {
    let mut button = Button::new("Test".to_string(), Rect::new(0, 0, 100, 40));

    assert!(button.is_enabled());

    button.set_enabled(false);
    assert!(!button.is_enabled());

    button.set_enabled(true);
    assert!(button.is_enabled());
}

#[test]
fn test_event_dispatch() {
    let mut harness = TestHarness::new();
    let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));

    harness.send_mouse_move(50, 15);
    harness.send_key_press(65, 0);

    let handled = harness.dispatch_to(&mut label);

    assert_eq!(handled, 2);
}

#[test]
fn test_widget_tester() {
    let button = Button::new("Test".to_string(), Rect::new(0, 0, 100, 40));
    let tester = WidgetTester::new(button);

    tester
        .assert_visible()
        .assert_enabled()
        .assert_geometry(Rect::new(0, 0, 100, 40));
}

#[test]
fn test_multiple_widgets() {
    let label1 = Label::new("Label 1".to_string(), Rect::new(0, 0, 100, 30));
    let label2 = Label::new("Label 2".to_string(), Rect::new(100, 0, 100, 30));
    let label3 = Label::new("Label 3".to_string(), Rect::new(200, 0, 100, 30));

    // Test that widgets don't overlap
    assert!(!label1.geometry().intersects(&label2.geometry()));
    assert!(!label2.geometry().intersects(&label3.geometry()));
    assert!(!label1.geometry().intersects(&label3.geometry()));
}

#[test]
fn test_widget_resize() {
    let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));

    label.set_size(Size::new(200, 60));

    assert_eq!(label.size(), Size::new(200, 60));
    assert_eq!(label.geometry(), Rect::new(0, 0, 200, 60));
}

#[test]
fn test_widget_move() {
    let mut label = Label::new("Test".to_string(), Rect::new(0, 0, 100, 30));

    label.set_position(Point::new(50, 100));

    assert_eq!(label.position(), Point::new(50, 100));
    assert_eq!(label.geometry(), Rect::new(50, 100, 100, 30));
}
