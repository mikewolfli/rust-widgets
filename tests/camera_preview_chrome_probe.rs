//! Evidence for BLUE23 §98: `camera_preview`'s active-viewfinder chrome was theme-blind.
//!
//! # The defect this pins
//!
//! The overlay drawn over an **active** viewfinder — the resolution readout, the camera id, the
//! zoom label, the mirror badge and the crosshair — was eight hardcoded `rgba(2xx,2xx,2xx)` values.
//! The control read no style and no theme at all, so a light appearance put near-white labels on
//! the near-white page behind them and a theme switch moved nothing but that page. The rendering
//! census could not see it, because the census renders `camera_preview` **inactive** (its `else`
//! branch), so `camera_preview.svg` and `camera_preview.light.svg` differ only in the window fill.
//!
//! # Why the assertions are on the painted picture
//!
//! The promise is "the annotation is legible against the stage it is painted on". A test on
//! `is_active()` or on "does the draw read the theme" would pass with the defect present. So this
//! probe reads the fill and ink colours out of the SVG and measures their **contrast** — the
//! quantity the old literals failed, and the same reading `image_gallery`'s overlay fix used.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test camera_preview_chrome_probe
// This probe needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;

/// WCAG relative luminance, so the assertion speaks in the same units the rendering census does.
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

/// Every `fill="rgba(r,g,b,a)"` colour in the document, as `(rgba_string, Color)`.
fn painted_fills(svg: &str) -> Vec<(String, Color)> {
    let mut out = Vec::new();
    for line in svg.lines() {
        for attr in ["fill=\"", "stroke=\""] {
            let Some(rest) = line.split(attr).nth(1) else { continue };
            let Some(value) = rest.split('"').next() else { continue };
            let Some(inner) = value.strip_prefix("rgba(").and_then(|v| v.strip_suffix(')')) else {
                continue;
            };
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() != 4 {
                continue;
            }
            let (Ok(r), Ok(g), Ok(b)) = (parts[0].parse(), parts[1].parse(), parts[2].parse())
            else {
                continue;
            };
            let Ok(a) = parts[3].parse::<f32>() else { continue };
            let alpha = if a <= 1.0 { (a * 255.0).round() as u8 } else { a.round() as u8 };
            out.push((value.to_string(), Color::rgba(r, g, b, alpha)));
        }
    }
    out
}

fn render_active(appearance: AppearanceMode) -> String {
    rust_widgets::theme::global_theme_manager().set_appearance(appearance);
    let geometry = Rect::new(0, 0, 320, 200);
    let mut w = rust_widgets::widget::media_widgets::camera_preview::CameraPreview::new(geometry);
    w.start_preview();
    // Every conditional piece of chrome is switched **on**, so the probe measures the whole
    // overlay rather than whichever subset the defaults happen to draw. The mirror badge in
    // particular is `false` by default, so a probe that left it alone would never measure it and
    // would report a clean picture while one annotation was still invisible.
    w.set_mirror_mode(true);
    let drawable = draw_of(&mut w).expect("draw bridge");
    render_widget_to_svg_on(drawable, geometry, Color::BLACK)
}

#[test]
fn probe_the_active_overlay_is_legible_on_whatever_stage_is_painted() {
    // The guard is held for the whole probe: it mutates the process-wide appearance, so without
    // it a concurrently running test would observe the wrong theme.
    let _guard = rust_widgets::theme::theme_test_guard();

    let dark = render_active(AppearanceMode::Dark);
    let light = render_active(AppearanceMode::Light);

    let dark_fills = painted_fills(&dark);
    let light_fills = painted_fills(&light);

    // The viewfinder stage is whichever painted colour is the *large opaque plate the labels sit
    // on*. It is identified as the fill the appearance changes that is neither the page fill nor
    // a text ink -- i.e. the one colour that differs between the two appearances and is dark-ish
    // on the dark appearance. The theme resolver supplies it, so it is read from the picture
    // rather than assumed to be the control's own constant (which the resolver can override).
    let dark_stage = dark_fills
        .iter()
        .map(|(_, c)| *c)
        .find(|c| c.r == 30 && c.g == 30 && c.b == 33)
        .unwrap_or_else(|| {
            println!("dark fills: {dark_fills:?}");
            panic!("the dark viewfinder stage must be painted")
        });
    let light_stage = light_fills
        .iter()
        .map(|(_, c)| *c)
        .find(|c| c.r == 243 && c.g == 237 && c.b == 247)
        .unwrap_or_else(|| {
            println!("light fills: {light_fills:?}");
            panic!("the light viewfinder stage must be painted")
        });

    println!(
        "dark stage = rgba({},{},{})  light stage = rgba({},{},{})",
        dark_stage.r, dark_stage.g, dark_stage.b, light_stage.r, light_stage.g, light_stage.b
    );

    // # The defect, restated as the assertion
    //
    // The old code painted the annotations with fixed near-whites. On the **light** stage the
    // theme resolves (`243,237,247`) those composited to 1.13-1.47:1 -- invisible. On the dark
    // stage the very same literals gave 6.6-12.6:1, which is why the census snapshot looked
    // correct and never reported anything. So the claim is per-appearance: the ink must clear
    // the floor on **both** stages.
    // # The floor
    //
    // WCAG's body-text floor. Every colour this control paints over the viewfinder is now derived
    // from the stage, so all of them clear it on both appearances -- including the crosshair,
    // which is aimed with rather than read but is legible anyway. An earlier revision of this
    // probe carried a second, lower floor for "non-text graphics"; the fix made it unnecessary,
    // so it was removed rather than kept as a way to tolerate a colour that had stopped clearing
    // the text bar. A tolerance with no colour in it is a place for a regression to hide.
    let text_floor = 4.5_f32;

    for (label, stage, fills) in
        [("dark", dark_stage, &dark_fills), ("light", light_stage, &light_fills)]
    {
        let mut worst = f32::MAX;
        let mut worst_color = String::new();
        for (name, c) in fills.iter() {
            if *c == stage || c.a < 200 {
                continue;
            }
            // The SVG canvas background is not part of the control.
            if c.r == 0 && c.g == 0 && c.b == 0 {
                continue;
            }
            // The status lamp: hue is the information, and a saturated green is not read as a
            // contrast pair. It is pinned by its own test elsewhere.
            if c.g == 255 && c.r == 0 && c.b == 0 {
                continue;
            }
            let ratio = contrast(*c, stage);
            println!("  [{label}] {name} vs stage -> {ratio:.2}:1");
            if ratio < worst {
                worst = ratio;
                worst_color = name.clone();
            }
        }
        assert!(
            worst >= text_floor,
            "on the {label} appearance the worst annotation was {worst_color} at {worst:.2}:1, \
             under the {text_floor}:1 body-text floor -- which is the defect this control shipped \
             with (its fixed near-whites gave 1.13-1.47:1 on the light stage)"
        );
    }
}
