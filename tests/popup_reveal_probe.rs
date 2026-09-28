//! Evidence for BLUE23 §93: does opening a `dropdown_menu` actually animate the list?
//!
//! The list used to appear at its full height on the frame `expanded` flipped, so opening was a
//! hard cut. This opens it, samples the painted picture across frames, and reports the clipped
//! extent, so "does the reveal reach the pixels" is measured rather than inferred from the tick
//! returning `true`.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test popup_reveal_probe -- --nocapture

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::coercion::widget_as_mut;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::menu_toolbar::dropdown_menu::DropdownMenu;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::Widget;
use rust_widgets::widget::WidgetFactory;

/// Renders the control and returns its SVG.
fn render(widget: &mut dyn Widget, geometry: Rect) -> String {
    let drawable = draw_of(widget).expect("draw bridge");
    render_widget_to_svg_on(drawable, geometry, Color::BLACK)
}

#[test]
fn probe_opening_the_list_takes_interior_frames() {
    let _guard = rust_widgets::theme::theme_test_guard();
    rust_widgets::theme::global_theme_manager().set_appearance(AppearanceMode::Dark);
    let geometry = Rect::new(0, 0, 200, 32);
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("dropdown_menu", geometry, "Sample").expect("dropdown");
    {
        // The registry publishes an **empty** list (`create_dropdown_menu` adds no items), so a
        // probe that did not fill it would measure a control whose list never draws at all —
        // which is what the first version of this test did, and it read as "the reveal is broken".
        let menu = widget_as_mut::<DropdownMenu>(widget.as_mut()).expect("dropdown menu");
        for name in ["Alpha", "Beta", "Gamma", "Delta"] {
            menu.add_item(rust_widgets::widget::menu_toolbar::dropdown_menu::DropdownItem::new(
                name, name,
            ));
        }
    }

    let field_only = render(widget.as_mut(), geometry);

    {
        let menu = widget_as_mut::<DropdownMenu>(widget.as_mut()).expect("dropdown menu");
        menu.expand();
    }
    assert!(widget.is_animating(), "opening must owe frames");

    // The first frame of the transition.
    let _ = widget.tick(1);
    let first = render(widget.as_mut(), geometry);

    // Somewhere in the middle.
    let _ = widget.tick(60);
    let middle = render(widget.as_mut(), geometry);

    while widget.is_animating() {
        let _ = widget.tick(16);
    }
    let settled = render(widget.as_mut(), geometry);

    println!(
        "field only : {} bytes, {} clip(s)",
        field_only.len(),
        field_only.matches("clip").count()
    );
    println!("first frame: {} bytes, {} clip(s)", first.len(), first.matches("clip").count());
    println!("middle     : {} bytes, {} clip(s)", middle.len(), middle.matches("clip").count());
    println!("settled    : {} bytes, {} clip(s)", settled.len(), settled.matches("clip").count());

    assert_ne!(
        first, middle,
        "the list must be painted differently a millisecond in and 60 ms in; identical pictures \
         mean the reveal never reached the draw path (the `segmented_control` teleport)"
    );
    assert_ne!(
        field_only, settled,
        "a settled open list must differ from the closed field, or the reveal hid the list"
    );
    assert!(
        !widget.is_animating(),
        "a settled list owes no frames, or the host would schedule them forever"
    );
}
