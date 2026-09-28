//! Evidence for BLUE23 §99: `image_gallery`'s disabled empty-state label was unreadable.
//!
//! # The defect this pins
//!
//! The empty-state placeholder dims its label to signal "this gallery is disabled". It did that
//! with a local blend weight of `0.38`, which measured **3.56:1** on the dark panel and **2.63:1**
//! on the light one — both under the 4.5:1 body-text floor. So "disabled" read as *unreadable*
//! rather than as *receded*, and the lighter appearance was the worse of the two.
//!
//! The crate already had the answer: `dimensions::DISABLED_VEIL_ALPHA`, whose own documentation
//! records the same lesson from `label` and `frame` — the recede must be one shared weight or a
//! disabled label, frame and button stop looking alike. This control was the one place that
//! hand-rolled a different weight.
//!
//! # Why the assertion is on the painted picture
//!
//! The promise is "a disabled placeholder is still legible". A test on `is_enabled()` or on
//! "does the disabled branch run" passes with the defect present. So this probe reads the label
//! colour and the panel colour out of the SVG and measures their **contrast**.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test image_gallery_disabled_probe

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::view_widgets::image_gallery::ImageGallery;

fn luminance(c: Color) -> f32 {
    let f = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
}

fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Every `fill="rgba(...)"`, as parsed colours, in document order.
fn painted_fills(svg: &str) -> Vec<(String, Color)> {
    let mut out = Vec::new();
    for line in svg.lines() {
        let Some(rest) = line.split("fill=\"").nth(1) else { continue };
        let Some(value) = rest.split('"').next() else { continue };
        let Some(inner) = value.strip_prefix("rgba(").and_then(|v| v.strip_suffix(')')) else {
            continue;
        };
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() != 4 {
            continue;
        }
        let (Ok(r), Ok(g), Ok(b)) = (parts[0].parse(), parts[1].parse(), parts[2].parse()) else {
            continue;
        };
        let Ok(a) = parts[3].parse::<f32>() else { continue };
        let alpha = if a <= 1.0 { (a * 255.0).round() as u8 } else { a.round() as u8 };
        out.push((value.to_string(), Color::rgba(r, g, b, alpha)));
    }
    out
}

/// Renders the empty state, optionally disabled, and returns `(panel, label)` colours.
///
/// The empty state paints exactly two things: the placeholder panel and the label on it. They are
/// told apart by size -- the panel spans the control's width, the label's `<path>` is a text run --
/// so the panel is the full-width rect and the label is the remaining non-canvas fill.
fn empty_state_colors(appearance: AppearanceMode, disabled: bool) -> (Color, Color) {
    rust_widgets::theme::global_theme_manager().set_appearance(appearance);
    let geometry = Rect::new(0, 0, 240, 120);
    let mut w = ImageGallery::new(geometry);
    if disabled {
        w.set("enabled", CapabilityValue::Bool(false)).expect("enabled is writable");
    }
    assert!(w.images().is_empty(), "the probe measures the empty state");
    let drawable = draw_of(&mut w).expect("draw bridge");
    let svg = render_widget_to_svg_on(drawable, geometry, Color::BLACK);
    println!("--- {appearance:?} disabled={disabled}\n{svg}");

    let fills = painted_fills(&svg);
    // The canvas is the first `<rect>` and is fully opaque black in this renderer.
    let panel = fills
        .iter()
        .map(|(_, c)| *c)
        .find(|c| !(c.r == 0 && c.g == 0 && c.b == 0))
        .unwrap_or_else(|| panic!("the placeholder panel must be painted; fills={fills:?}"));
    let label = fills
        .iter()
        .map(|(_, c)| *c)
        .find(|c| *c != panel && !(c.r == 0 && c.g == 0 && c.b == 0))
        .unwrap_or_else(|| panic!("the placeholder label must be painted; fills={fills:?}"));
    (panel, label)
}

#[test]
fn probe_a_disabled_placeholder_label_is_still_legible() {
    let _guard = rust_widgets::theme::theme_test_guard();

    for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
        let (panel, label) = empty_state_colors(appearance, true);
        let ratio = contrast(label, panel);
        println!(
            "[{appearance:?}] disabled label rgba({},{},{}) on panel rgba({},{},{}) -> {ratio:.2}:1",
            label.r, label.g, label.b, panel.r, panel.g, panel.b
        );
        assert!(
            ratio >= 4.5,
            "a disabled placeholder label must still clear the 4.5:1 body-text floor on the \
             {appearance:?} appearance; it measured {ratio:.2}:1. The defect this pins used a \
             local blend weight of 0.38, which gave 3.56:1 (dark) and 2.63:1 (light)"
        );
    }
}

/// The other half: enabled must stay *more* prominent than disabled, or the state is not signalled.
#[test]
fn probe_enabled_is_more_prominent_than_disabled() {
    let _guard = rust_widgets::theme::theme_test_guard();

    for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
        let (panel_on, label_on) = empty_state_colors(appearance, false);
        let (panel_off, label_off) = empty_state_colors(appearance, true);
        let on = contrast(label_on, panel_on);
        let off = contrast(label_off, panel_off);
        println!("[{appearance:?}] enabled {on:.2}:1  disabled {off:.2}:1");
        assert!(
            off < on,
            "disabled must recede relative to enabled on the {appearance:?} appearance \
             (enabled {on:.2}:1, disabled {off:.2}:1); equal contrast would mean the state is \
             not signalled at all"
        );
    }
}
