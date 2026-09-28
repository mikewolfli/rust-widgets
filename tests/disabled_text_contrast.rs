//! Ground-truth measurement of disabled **text** contrast, on real pixels.
//!
//! The ranking script is a *model*; this is a *measurement*. It exists because the model has
//! already been wrong once (an inverted blend direction reported an already-fixed control as
//! broken). Every site is confirmed here before it is changed.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test disabled_text_contrast -- --nocapture

// This probe needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]


use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;

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

fn fills(svg: &str) -> Vec<Color> {
    let mut out = Vec::new();
    for line in svg.lines() {
        for attr in ["fill=\"", "stroke=\""] {
            let Some(rest) = line.split(attr).nth(1) else { continue };
            let Some(v) = rest.split('"').next() else { continue };
            let Some(inner) = v.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) else {
                continue;
            };
            let p: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if p.len() != 4 {
                continue;
            }
            let (Ok(r), Ok(g), Ok(b)) = (p[0].parse(), p[1].parse(), p[2].parse()) else {
                continue;
            };
            let a: f32 = p[3].parse().unwrap_or(1.0);
            let alpha = if a <= 1.0 { (a * 255.0).round() as u8 } else { a.round() as u8 };
            out.push(Color::rgba(r, g, b, alpha));
        }
    }
    out
}

/// Renders a control at the given appearance, optionally disabled, and reports every painted
/// colour's contrast against the most likely panel (the largest opaque non-canvas fill).
fn survey(
    name: &str,
    geometry: Rect,
    disabled: bool,
    appearance: AppearanceMode,
) -> Vec<(String, f32)> {
    rust_widgets::theme::global_theme_manager().set_appearance(appearance);
    let factory = rust_widgets::widget::WidgetFactory::new_with_defaults();
    let Some(mut w) = factory.create(name, geometry, "x") else {
        println!("  {name}: not constructible");
        return Vec::new();
    };
    if disabled {
        // Through the factory's public write path, which is what a host manifest applies.
        factory
            .write_property(w.as_mut(), "enabled", CapabilityValue::Bool(false))
            .expect("enabled is writable");
    }
    let drawable = draw_of(w.as_mut()).expect("draw bridge");
    let svg = render_widget_to_svg_on(drawable, geometry, Color::BLACK);
    let painted = fills(&svg);
    // The panel: the first opaque, non-black, non-canvas colour (the control's own face).
    // # Against which colour is an ink measured?
    //
    // A control can paint **several** regions (a header band over a content area, a field inside a
    // panel), and an ink is only meaningful against the region it is actually drawn on. Comparing
    // every colour against one "panel" reported a header ink as 1.21:1 when the header it sits on
    // gives it 6.08:1 -- a probe artefact, not a defect.
    //
    // So each colour is measured against every *other* colour in the picture, and reports the best
    // contrast it has against any of them. An ink legible on some surface the control draws is
    // legible; one legible on none of the others is the defect. The `c != other` guard is what
    // keeps a colour from being compared with itself (which would read 1.00:1 and swamp the report).
    let mut out = Vec::new();
    for c in &painted {
        if (c.r == 0 && c.g == 0 && c.b == 0) || c.a < 200 {
            continue;
        }
        let best = painted
            .iter()
            .filter(|other| **other != *c && other.a >= 200)
            .map(|other| contrast(*c, *other))
            .fold(0.0_f32, f32::max);
        out.push((format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a), best));
    }
    out
}

#[test]
fn probe_disabled_text_contrast_on_real_pixels() {
    let _guard = rust_widgets::theme::theme_test_guard();

    let cases: &[(&str, u32, u32)] = &[
        ("collapsible_pane", 240, 120),
        ("app_bar", 320, 64),
        ("action", 160, 40),
        ("sparkline", 200, 60),
        ("progress_circle", 64, 64),
    ];

    // # The controls whose *disabled text* was below the body-text floor
    //
    // Only these owe 4.5:1. A chart series or a progress arc is a **graphic on the window**, not
    // text on a panel: `sparkline`'s disabled line is the fixed `DISABLED_FOREGROUND` and measures
    // 7.37:1 against either window fill, which is why the fixed grey is not universally wrong --
    // it fails specifically as *panel text*, where it cannot follow the appearance.
    let text_controls = ["collapsible_pane", "app_bar"];

    for (name, w, h) in cases {
        let geometry = Rect::new(0, 0, *w, *h);
        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            let rows = survey(name, geometry, true, appearance);
            let worst = rows.iter().map(|(_, r)| *r).fold(f32::INFINITY, f32::min);
            let worst_text = if worst.is_finite() {
                format!("{worst:.2}:1")
            } else {
                "(no distinct ink painted)".to_string()
            };
            println!("[{name} / {appearance:?} disabled] worst = {worst_text}");
            for (c, r) in &rows {
                println!("    {c} -> {r:.2}:1");
            }
            if text_controls.contains(name) {
                assert!(
                    worst >= 4.5,
                    "{name}'s disabled text measured {worst:.2}:1 on the {appearance:?} \
                     appearance, under the 4.5:1 body-text floor. This is the defect the probe \
                     pins: `collapsible_pane` blended its panel half-way toward its own ink \
                     (1.03:1) and `app_bar` used the appearance-blind `DISABLED_FOREGROUND` \
                     (2.48:1 on light)"
                );
            }
        }
    }
}
