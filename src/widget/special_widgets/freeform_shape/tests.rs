// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tests for the freeform-shape geometry and the control that draws it.

// The module-level gate, so tooling that scans file text recognises this as a test
// module. `mod.rs` declares `mod tests;` under `#[cfg(test)]`, which the compiler
// sees but a text scan of this file does not.
#![cfg(test)]

use super::shape::FreeformShapeWidget;
use super::types::{BubbleTailDirection, PathSegment, ShapePath};
use crate::core::{Color, Point, Rect};
use crate::event::EventHandler;
use crate::widget::{Widget, WidgetKind};
use std::sync::{Arc, Mutex};

#[test]
fn freeform_shape_creation_defaults() {
    let shape = FreeformShapeWidget::new(Rect::new(10, 20, 200, 150), ShapePath::Heart);
    assert_eq!(shape.path(), &ShapePath::Heart);
    assert_eq!(shape.fill_color(), Color::rgb(200, 220, 255));
    assert_eq!(shape.stroke_color(), Some(Color::rgb(80, 120, 200)));
    assert_eq!(shape.stroke_width(), 2);
    assert!(shape.base().is_visible());
    assert!(shape.base().is_enabled());
}

#[test]
fn freeform_shape_set_path() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    let polygon =
        ShapePath::Polygon(vec![Point::new(10, 10), Point::new(90, 10), Point::new(50, 90)]);
    shape.set_path(polygon.clone());
    assert_eq!(shape.path(), &polygon);
}

#[test]
fn freeform_shape_set_fill_color() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    shape.set_fill_color(Color::rgb(255, 0, 0));
    assert_eq!(shape.fill_color(), Color::rgb(255, 0, 0));
}

#[test]
fn freeform_shape_set_stroke_color() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    shape.set_stroke_color(Some(Color::rgb(0, 255, 0)));
    assert_eq!(shape.stroke_color(), Some(Color::rgb(0, 255, 0)));

    shape.set_stroke_color(None);
    assert_eq!(shape.stroke_color(), None);
}

#[test]
fn freeform_shape_set_stroke_width() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    shape.set_stroke_width(5);
    assert_eq!(shape.stroke_width(), 5);
}

#[test]
fn freeform_shape_contains_heart_center() {
    // Heart in 100x100 rect — center at (50,50) should be inside
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    assert!(shape.contains(Point::new(50, 50)));
}

#[test]
fn freeform_shape_contains_heart_outside_rect() {
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    // Point outside the bounding rect
    assert!(!shape.contains(Point::new(150, 150)));
}

#[test]
fn freeform_shape_contains_polygon_center() {
    let triangle =
        ShapePath::Polygon(vec![Point::new(0, 0), Point::new(100, 0), Point::new(50, 100)]);
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), triangle);
    // Center of the bounding rect is near (50, 30) which is inside the triangle
    assert!(shape.contains(Point::new(50, 30)));
}

#[test]
fn freeform_shape_contains_rounded_rect() {
    let shape =
        FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::RoundedRect { radius: 10 });
    // Top-left inside rounded rect (beyond corner radius)
    assert!(shape.contains(Point::new(15, 15)));
    // Outside bounding rect
    assert!(!shape.contains(Point::new(200, 200)));
}

#[test]
fn freeform_shape_contains_zero_dimensions() {
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 0, 0), ShapePath::Heart);
    assert!(!shape.contains(Point::new(0, 0)));
}

#[test]
fn freeform_shape_rounded_rect_zero_radius_equal_rect() {
    let shape =
        FreeformShapeWidget::new(Rect::new(0, 0, 50, 50), ShapePath::RoundedRect { radius: 0 });
    // Should act as a plain rect hit-test
    assert!(shape.contains(Point::new(25, 25)));
    assert!(!shape.contains(Point::new(-1, 25)));
}

#[test]
fn freeform_shape_bubble_contains_body() {
    let shape = FreeformShapeWidget::new(
        Rect::new(10, 10, 100, 80),
        ShapePath::Bubble { tail_direction: BubbleTailDirection::Right },
    );
    // Inside the body
    assert!(shape.contains(Point::new(60, 50)));
}

#[test]
fn freeform_shape_star_contains_center() {
    let shape = FreeformShapeWidget::new(
        Rect::new(0, 0, 100, 100),
        ShapePath::Star { points: 5, inner_radius: 0.4 },
    );
    // Center of star is always inside
    assert!(shape.contains(Point::new(50, 50)));
}

#[test]
fn freeform_shape_custom_path_empty() {
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Custom(vec![]));
    // Empty custom path should not contain anything
    assert!(!shape.contains(Point::new(50, 50)));
}

#[test]
fn freeform_shape_custom_path_triangle() {
    let segs = vec![
        PathSegment::MoveTo(Point::new(0, 0)),
        PathSegment::LineTo(Point::new(100, 0)),
        PathSegment::LineTo(Point::new(50, 100)),
        PathSegment::Close,
    ];
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Custom(segs));
    assert!(shape.contains(Point::new(50, 30)));
    assert!(!shape.contains(Point::new(200, 200)));
}

#[test]
fn freeform_shape_geometry_delegation() {
    let rect = Rect::new(5, 10, 300, 200);
    let shape = FreeformShapeWidget::new(rect, ShapePath::Heart);
    assert_eq!(shape.geometry(), rect);
    assert_eq!(shape.position(), Point::new(5, 10));
    assert_eq!(shape.size(), crate::core::Size::new(300, 200));
}

#[test]
fn freeform_shape_visibility() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    assert!(shape.is_visible());
    shape.set_visible(false);
    assert!(!shape.is_visible());
    shape.set_visible(true);
    assert!(shape.is_visible());
}

#[test]
fn freeform_shape_enabled_state() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    assert!(shape.is_enabled());
    shape.set_enabled(false);
    assert!(!shape.is_enabled());
}

#[test]
fn freeform_shape_kind() {
    let shape = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    assert_eq!(shape.kind(), WidgetKind::FreeformShape);
}

#[test]
fn freeform_shape_ids_are_unique() {
    let a = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    let b = FreeformShapeWidget::new(Rect::new(0, 0, 100, 100), ShapePath::Heart);
    assert_ne!(a.id(), b.id());
}

#[test]
fn freeform_shape_clicked_signal_emits_on_click() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    let count = Arc::new(Mutex::new(0u32));
    let sink = count.clone();
    shape.clicked.connect(move || {
        if let Ok(mut c) = sink.lock() {
            *c += 1;
        }
    });

    // Press inside shape
    let press = crate::event::Event::MousePress { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&press);

    // Release inside shape
    let release = crate::event::Event::MouseRelease { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&release);

    let got = *count.lock().unwrap();
    assert_eq!(got, 1, "clicked signal should fire once on press+release inside");
}

#[test]
fn freeform_shape_click_outside_no_signal() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    let count = Arc::new(Mutex::new(0u32));
    let sink = count.clone();
    shape.clicked.connect(move || {
        if let Ok(mut c) = sink.lock() {
            *c += 1;
        }
    });

    // Press outside
    let press = crate::event::Event::MousePress { pos: Point::new(999, 999), button: 1 };
    shape.handle_event(&press);

    let release = crate::event::Event::MouseRelease { pos: Point::new(999, 999), button: 1 };
    shape.handle_event(&release);

    let got = *count.lock().unwrap();
    assert_eq!(got, 0, "click should not fire for outside press+release");
}

#[test]
fn freeform_shape_hovered_signal_emits_on_mouse_move() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    let states = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = states.clone();
    shape.hovered_changed.connect(move |h| {
        if let Ok(mut s) = sink.lock() {
            s.push(*h);
        }
    });

    // Move into shape
    let ev = crate::event::Event::MouseMove { pos: Point::new(100, 100) };
    shape.handle_event(&ev);

    // Move out of shape
    let ev2 = crate::event::Event::MouseMove { pos: Point::new(999, 999) };
    shape.handle_event(&ev2);

    let got = states.lock().ok().map(|g| g.clone()).unwrap_or_default();
    assert_eq!(got, vec![true, false], "hovered_changed should fire true then false");
}

#[test]
fn freeform_shape_pressed_signal_emits_on_press() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    let states = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = states.clone();
    shape.pressed_changed.connect(move |p| {
        if let Ok(mut s) = sink.lock() {
            s.push(*p);
        }
    });

    // Press inside
    let press = crate::event::Event::MousePress { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&press);

    // Release
    let release = crate::event::Event::MouseRelease { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&release);

    let got = states.lock().ok().map(|g| g.clone()).unwrap_or_default();
    assert_eq!(got, vec![true, false], "pressed_changed should fire true then false");
}

#[test]
fn freeform_shape_disabled_ignores_events() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    shape.set_enabled(false);

    let count = Arc::new(Mutex::new(0u32));
    let sink = count.clone();
    shape.clicked.connect(move || {
        if let Ok(mut c) = sink.lock() {
            *c += 1;
        }
    });

    let press = crate::event::Event::MousePress { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&press);

    let release = crate::event::Event::MouseRelease { pos: Point::new(100, 100), button: 1 };
    shape.handle_event(&release);

    let got = *count.lock().unwrap();
    assert_eq!(got, 0, "disabled widget should not process events");
}

#[test]
fn freeform_shape_mouse_enter_and_leave() {
    let mut shape = FreeformShapeWidget::new(Rect::new(0, 0, 200, 200), ShapePath::Heart);
    let states = Arc::new(Mutex::new(Vec::<bool>::new()));
    let sink = states.clone();
    shape.hovered_changed.connect(move |h| {
        if let Ok(mut s) = sink.lock() {
            s.push(*h);
        }
    });

    let enter = crate::event::Event::MouseEnter { pos: Point::new(100, 100) };
    shape.handle_event(&enter);

    let leave = crate::event::Event::MouseLeave { pos: Point::new(100, 100) };
    shape.handle_event(&leave);

    let got = states.lock().ok().map(|g| g.clone()).unwrap_or_default();
    assert_eq!(got, vec![true, false], "MouseEnter/MouseLeave should fire hovered_changed");
}

#[test]
fn freeform_shape_bubble_tail_directions() {
    for dir in &[
        BubbleTailDirection::TopLeft,
        BubbleTailDirection::TopRight,
        BubbleTailDirection::BottomLeft,
        BubbleTailDirection::BottomRight,
        BubbleTailDirection::Left,
        BubbleTailDirection::Right,
        BubbleTailDirection::Top,
        BubbleTailDirection::Bottom,
    ] {
        let shape = FreeformShapeWidget::new(
            Rect::new(0, 0, 80, 60),
            ShapePath::Bubble { tail_direction: *dir },
        );
        // Center should be inside regardless of tail direction
        assert!(shape.contains(Point::new(40, 30)), "center should be inside for {:?}", dir);
    }
}

#[test]
fn freeform_shape_polygon_less_than_3_vertices() {
    let shape = FreeformShapeWidget::new(
        Rect::new(0, 0, 100, 100),
        ShapePath::Polygon(vec![Point::new(0, 0), Point::new(100, 0)]),
    );
    // 2 vertices can't form a polygon
    assert!(!shape.contains(Point::new(50, 25)));
}

#[test]
fn freeform_shape_star_few_points() {
    let shape = FreeformShapeWidget::new(
        Rect::new(0, 0, 100, 100),
        ShapePath::Star { points: 3, inner_radius: 0.5 },
    );
    assert!(shape.contains(Point::new(50, 50)));
}

#[test]
fn freeform_shape_path_enum_variants() {
    // Verify all variants can be constructed and compared
    let paths = [
        ShapePath::Heart,
        ShapePath::Star { points: 5, inner_radius: 0.4 },
        ShapePath::Polygon(vec![Point::new(0, 0), Point::new(50, 0), Point::new(25, 50)]),
        ShapePath::RoundedRect { radius: 8 },
        ShapePath::Bubble { tail_direction: BubbleTailDirection::Top },
        ShapePath::Custom(vec![
            PathSegment::MoveTo(Point::new(0, 0)),
            PathSegment::LineTo(Point::new(100, 0)),
            PathSegment::Close,
        ]),
    ];

    // Check all paths are different (no duplicates)
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            assert_ne!(paths[i], paths[j], "path variant {} should differ from {}", i, j);
        }
    }
}
