//! Evidence for BLUE23 §95: does the selected thumbnail paint the **accent** role?
//!
//! The code said "the selected thumbnail is the accent" while reading `theme.background_color` —
//! the gallery's own surface. This shows the control with images, selects one, and reports the
//! fills, so "does the accent reach the pixels" is measured rather than inferred from the branch.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test image_gallery_accent_probe -- --nocapture

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::coercion::widget_as_mut;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::view_widgets::image_gallery::ImageGallery;
use rust_widgets::widget::Widget;
use rust_widgets::widget::WidgetFactory;

fn fills(widget: &mut dyn Widget, geometry: Rect) -> Vec<String> {
    let drawable = draw_of(widget).expect("draw bridge");
    let svg = render_widget_to_svg_on(drawable, geometry, Color::BLACK);
    let mut out: Vec<String> = svg
        .lines()
        .filter(|line| line.contains("fill=\"rgba"))
        .filter_map(|line| {
            line.split("fill=\"").nth(1).and_then(|rest| rest.split('"').next()).map(str::to_string)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn probe_the_selected_thumbnail_is_the_accent() {
    let _guard = rust_widgets::theme::theme_test_guard();
    rust_widgets::theme::global_theme_manager().set_appearance(AppearanceMode::Dark);

    // The accent the preset declares, so the assertion can name it.
    let accent = rust_widgets::theme::global_theme_manager()
        .current_theme()
        .map(|active| active.colors.primary)
        .expect("a preset theme");
    let accent_rgba = format!("rgba({},{},{},1.00)", accent.r, accent.g, accent.b);

    let geometry = Rect::new(0, 0, 360, 260);
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create("image_gallery", geometry, "x").expect("gallery");
    {
        let gallery = widget_as_mut::<ImageGallery>(widget.as_mut()).expect("gallery");
        for name in ["a.png", "b.png", "c.png", "d.png"] {
            gallery.add_image(name, None);
        }
        // The first image is selected on add, so this moves the selection off it — the assertion is
        // about the *selected* thumbnail, and it must not be satisfied by the default one.
        gallery.next_image();
    }

    let painted = fills(widget.as_mut(), geometry);
    println!("accent role = {accent_rgba}");
    println!("painted fills = {painted:?}");

    assert!(
        painted.contains(&accent_rgba),
        "the selected thumbnail must be painted in the theme's accent ({accent_rgba}); it is not \
         in the picture, so the selection reads as the panel it sits on. Fills: {painted:?}"
    );
}
