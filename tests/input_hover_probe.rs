//! Evidence for BLUE23 §90: the M1 hover gap across the `input_widgets` group.
//!
//! The appendix's input group shared one defect — a field that painted its resting fill
//! whatever the pointer did, so a user could not tell an interactive control from a static
//! one. Measured before the fix: of the 27 controls in `input_widgets/`, only `command_link`,
//! `line_edit` and `range_slider` read `is_hovered()` at all.
//!
//! This checks the family, not one control: each named widget is rendered at rest and hovered,
//! and its fills must differ.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test input_hover_probe -- --nocapture

// Needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::Widget;
use rust_widgets::widget::WidgetFactory;

/// The fills a control paints, in drawing order, under the given appearance.
fn fills(widget: &mut dyn Widget, geometry: Rect) -> Vec<String> {
    let svg = {
        let drawable = draw_of(widget).expect("draw bridge");
        rust_widgets::widget::svg::render_widget_to_svg_on(drawable, geometry, Color::BLACK)
    };
    svg.lines()
        .filter(|line| line.contains("fill="))
        .filter_map(|line| {
            line.split("fill=\"").nth(1).and_then(|rest| rest.split('"').next()).map(str::to_string)
        })
        .collect()
}

/// Every control in this group that is a pressable *field*.
const FIELDS: &[&str] = &[
    "spin_box",
    "combo_box",
    "editable_combo_box",
    "multi_select_combo_box",
    "search_box",
    "search_bar",
    "otp_input",
];

#[test]
fn probe_every_input_field_responds_to_the_pointer() {
    let _guard = rust_widgets::theme::theme_test_guard();
    let geometry = Rect::new(0, 0, 200, 32);
    let factory = WidgetFactory::new_with_defaults();
    let mut failures = Vec::new();

    for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
        rust_widgets::theme::global_theme_manager().set_appearance(appearance);
        for name in FIELDS {
            let Some(mut widget) = factory.create(name, geometry, "Sample") else {
                failures
                    .push(format!("{name}: the registry publishes it but create returned None"));
                continue;
            };
            let at_rest = fills(widget.as_mut(), geometry);

            // `BaseWidget` sets `hovered` from `MouseEnter`; a `MouseMove` only re-tests an
            // active press, so a probe that sent a move would measure the resting state.
            widget.handle_event(&Event::MouseEnter { pos: Point::new(40, 16) });
            let hovered = fills(widget.as_mut(), geometry);

            if at_rest.is_empty() {
                failures.push(format!("{name}: painted nothing at all"));
                continue;
            }
            if at_rest == hovered {
                failures.push(format!(
                    "{name} ({appearance:?}): hovering painted exactly the resting frame"
                ));
            } else {
                println!("{name} ({appearance:?}):");
                println!("  rest    {at_rest:?}");
                println!("  hovered {hovered:?}");
            }
        }
    }

    assert!(
        failures.is_empty(),
        "every pressable field must show the pointer. A field that paints the resting frame \
         when hovered is indistinguishable from a static label:\n  {}",
        failures.join("\n  ")
    );
}
