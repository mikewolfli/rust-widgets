use rust_widgets::core::Rect;
use rust_widgets::widget::{Button, Widget, WidgetKind};

#[test]
fn widget_structure_button_exposes_expected_kind_and_geometry() {
    let button = Button::new("Run".to_string(), Rect::new(10, 20, 120, 36));
    assert_eq!(button.kind(), WidgetKind::Button);
    assert_eq!(button.geometry(), Rect::new(10, 20, 120, 36));
}

#[test]
fn widget_structure_button_has_distinct_object_ids() {
    let a = Button::new("A".to_string(), Rect::new(0, 0, 80, 24));
    let b = Button::new("B".to_string(), Rect::new(0, 0, 80, 24));
    assert_ne!(a.id(), b.id());
}
