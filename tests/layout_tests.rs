//! Layout system tests

use rust_widgets::core::{Rect, Size};
use rust_widgets::layout::{
    FlowLayout, FlowDirection, FlowAlignment, FlowLayoutConfig,
    AbsolutePosition, AbsoluteLayout, AbsoluteAnchor
};
use rust_widgets::test::LayoutTester;

#[test]
fn test_flow_layout_config() {
    let config = FlowLayoutConfig {
        direction: FlowDirection::Horizontal,
        alignment: FlowAlignment::Start,
        spacing: 10,
        padding: 8,
        wrap: true,
    };

    assert_eq!(config.direction, FlowDirection::Horizontal);
    assert_eq!(config.alignment, FlowAlignment::Start);
    assert_eq!(config.spacing, 10);
    assert_eq!(config.padding, 8);
    assert!(config.wrap);
}

#[test]
fn test_flow_layout_creation() {
    let layout = FlowLayout::new();

    assert_eq!(layout.child_count(), 0);
}

#[test]
fn test_flow_layout_with_config() {
    let config = FlowLayoutConfig {
        direction: FlowDirection::Vertical,
        alignment: FlowAlignment::Center,
        spacing: 20,
        padding: 16,
        wrap: false,
    };

    let layout = FlowLayout::with_config(config);

    assert_eq!(layout.child_count(), 0);
}

#[test]
fn test_absolute_layout() {
    let container = Rect::new(0, 0, 400, 300);
    let layout = AbsoluteLayout::new();

    // Test basic positioning
    let positions = vec![
        AbsolutePosition::new(10, 20),
        AbsolutePosition::new(100, 50),
    ];

    let sizes = vec![
        Size::new(80, 30),
        Size::new(80, 30),
    ];

    let rects = layout.calculate_positions(&container, &positions, &sizes);

    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0], Rect::new(10, 20, 80, 30));
    assert_eq!(rects[1], Rect::new(100, 50, 80, 30));
}

#[test]
fn test_absolute_layout_centered() {
    let container = Rect::new(0, 0, 400, 300);
    let layout = AbsoluteLayout::new();

    // Center anchor with (0, 0) offset means the rect's center is at (0, 0)
    let positions = vec![
        AbsolutePosition::new(0, 0).with_anchor(AbsoluteAnchor::Center, 0, 0),
    ];

    let sizes = vec![
        Size::new(100, 50),
    ];

    let rects = layout.calculate_positions(&container, &positions, &sizes);

    assert_eq!(rects.len(), 1);
    // Center anchor with (0, 0) offset means the rect's center is at (0, 0)
    // So x = 0 - 50 = -50, y = 0 - 25 = -25
    assert_eq!(rects[0].x, -50);
    assert_eq!(rects[0].y, -25);
    assert_eq!(rects[0].width, 100);
    assert_eq!(rects[0].height, 50);
}

#[test]
fn test_layout_tester_no_overlap() {
    let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));

    let positions = vec![
        Rect::new(0, 0, 100, 50),
        Rect::new(100, 0, 100, 50),
        Rect::new(200, 0, 100, 50),
    ];

    tester.assert_no_overlap(&positions);
}

#[test]
fn test_layout_tester_fits_in_container() {
    let tester = LayoutTester::new(Rect::new(0, 0, 400, 300));

    let positions = vec![
        Rect::new(0, 0, 100, 50),
        Rect::new(100, 50, 100, 50),
        Rect::new(200, 100, 100, 50),
    ];

    tester.assert_fits_in_container(&positions);
}

#[test]
fn test_absolute_position_offset() {
    let pos = AbsolutePosition::new(10, 20);

    assert_eq!(pos.x, 10);
    assert_eq!(pos.y, 20);
}

#[test]
fn test_flow_direction_variants() {
    let h = FlowDirection::Horizontal;
    let v = FlowDirection::Vertical;

    assert_ne!(h, v);
}

#[test]
fn test_flow_alignment_variants() {
    let start = FlowAlignment::Start;
    let center = FlowAlignment::Center;
    let end = FlowAlignment::End;
    let space_between = FlowAlignment::SpaceBetween;
    let space_around = FlowAlignment::SpaceAround;

    assert_ne!(start, center);
    assert_ne!(center, end);
    assert_ne!(end, space_between);
    assert_ne!(space_between, space_around);
}

#[test]
fn test_absolute_anchor_variants() {
    let tl = AbsoluteAnchor::TopLeft;
    let tr = AbsoluteAnchor::TopRight;
    let bl = AbsoluteAnchor::BottomLeft;
    let br = AbsoluteAnchor::BottomRight;
    let center = AbsoluteAnchor::Center;

    assert_ne!(tl, tr);
    assert_ne!(tr, bl);
    assert_ne!(bl, br);
    assert_ne!(br, center);
}
