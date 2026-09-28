//! Evidence for BLUE23 §90: does a hovered `spin_box` paint differently from a resting one?
//!
//! The step buttons are the whole affordance of a spin box, and a hover that changes nothing
//! means a user cannot tell they are pressable. This renders the control at rest and hovered
//! and reports the button-well fill from each, so "does hover reach the pixels?" is measured
//! rather than asserted from the presence of a branch.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test spinbox_hover_probe -- --nocapture

// Needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::capability::coercion::widget_as_mut;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::input_widgets::spinbox::SpinBox;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::Widget;

/// Renders the control and returns every `<rect` fill in drawing order.
fn fills(widget: &mut SpinBox) -> Vec<String> {
    let geometry = Rect::new(0, 0, 160, 32);
    let svg = {
        let drawable = draw_of(widget as &mut dyn Widget).expect("draw bridge");
        render_widget_to_svg_on(drawable, geometry, Color::BLACK)
    };
    svg.lines()
        .filter(|line| line.contains("<rect") && line.contains("fill="))
        .filter_map(|line| {
            line.split("fill=\"").nth(1).and_then(|rest| rest.split('"').next()).map(str::to_string)
        })
        .collect()
}

#[test]
fn probe_spin_box_hover_reaches_the_pixels() {
    let _guard = rust_widgets::theme::theme_test_guard();
    rust_widgets::theme::global_theme_manager()
        .set_appearance(rust_widgets::theme::AppearanceMode::Light);

    let geometry = Rect::new(0, 0, 160, 32);
    let mut widget: Box<dyn Widget> = Box::new(SpinBox::new(geometry));

    let at_rest = {
        let spin = widget_as_mut::<SpinBox>(widget.as_mut()).expect("spin box");
        fills(spin)
    };

    // Hover the control the way the runtime does: `BaseWidget` sets `hovered` from
    // `MouseEnter` (a `MouseMove` only re-tests an active press), so a probe that sent a
    // move would measure the resting state and "confirm" a hover that never happened.
    widget.handle_event(&Event::MouseEnter { pos: Point::new(60, 16) });
    let hovered = {
        let spin = widget_as_mut::<SpinBox>(widget.as_mut()).expect("spin box");
        fills(spin)
    };

    println!("at rest : {at_rest:?}");
    println!("hovered : {hovered:?}");

    assert!(
        !at_rest.is_empty(),
        "the spin box must paint a field and two button wells; nothing was drawn"
    );
    assert_ne!(
        at_rest, hovered,
        "hovering the spin box must change what is painted — the step buttons are its whole \
         affordance, so a hover that paints the resting fill leaves a user unable to tell they \
         are pressable. Before the fix both renders were identical."
    );
}
