//! Does a disabled surface read as **receded**, on both appearances?
//!
//! # The defect this pins
//!
//! Several controls dim a disabled surface by blending it toward a **fixed white**
//! (`base_bg.blend(&Color::WHITE, 0.35)`). On a dark appearance that moves the surface *up* in
//! luminance, i.e. away from the window behind it — so the disabled control is painted **more**
//! prominently than the enabled one, which is the opposite of the state it is meant to express.
//! The pattern was measured at `1.13:1` enabled versus `3.63:1` disabled against the window, i.e.
//! three times the prominence, while on the light appearance it barely moved at all.
//!
//! The crate's own shared weight and derivation already fix this: step the surface toward its
//! **own contrast colour** (`BaseWidget::disabled_ink_on`), which recedes on both appearances.
//!
//! # The assertion
//!
//! Prominence is measured as the surface's contrast **against the window fill** it sits on: a
//! disabled control must be *no more* prominent than the same control enabled, on both
//! appearances. A control with no separate surface is skipped.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test disabled_surface_probe -- --nocapture

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::WidgetFactory;

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

/// The control's own surface, as painted.
///
/// # Why this needs a tolerance
///
/// A control's surface is not always its whole rectangle: `search_bar` reserves a band for its
/// button, so the field is narrower than the control. The match is therefore "a full-height,
/// full-width-or-nearly fill", and the widest candidate wins. Matching the exact box alone
/// silently reported "no surface" for that control -- which reads as a pass, and is the kind of
/// silent skip this probe exists to avoid.
fn surface_of(svg: &str, width: u32, height: u32) -> Option<Color> {
    let full_height = format!("height=\"{height}\"");
    let mut best: Option<(u32, Color)> = None;
    for line in svg.lines() {
        if !line.contains("fill=\"") || !line.contains(&full_height) {
            continue;
        }
        let Some(rest) = line.split("fill=\"").nth(1) else { continue };
        let Some(v) = rest.split('"').next() else { continue };
        let Some(inner) = v.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) else {
            continue;
        };
        let p: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if p.len() != 4 || p[3] != "1.00" {
            continue;
        }
        let (Ok(r), Ok(g), Ok(b)) = (p[0].parse(), p[1].parse(), p[2].parse()) else {
            continue;
        };
        let c = Color::rgb(r, g, b);
        // The renderer's canvas plate is the same box and is written first.
        if c.r == 0 && c.g == 0 && c.b == 0 {
            continue;
        }
        let w: u32 = line
            .split("width=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        // At least three quarters of the control's width: wide enough to be the field rather than
        // a glyph run or an icon plate.
        if w * 4 < width * 3 {
            continue;
        }
        if best.is_none_or(|(bw, _)| w > bw) {
            best = Some((w, c));
        }
    }
    best.map(|(_, c)| c)
}

fn render(
    name: &str,
    geometry: Rect,
    disabled: bool,
    appearance: AppearanceMode,
) -> Option<(String, Color)> {
    rust_widgets::theme::global_theme_manager().set_appearance(appearance);
    // The window fill is read **here**, immediately after the appearance is set, so it belongs to
    // the same appearance as the render below. Reading it in the caller after both renders had
    // already run returned the *other* appearance's window -- which silently inverted every
    // contrast figure and made the assertion compare a surface against the wrong page.
    let window = rust_widgets::style::theme_manager()
        .current_theme()
        .map(|active| active.colors.background)
        .unwrap_or(Color::BLACK);
    let factory = WidgetFactory::new_with_defaults();
    let mut w = factory.create(name, geometry, "x")?;
    if disabled {
        factory.write_property(w.as_mut(), "enabled", CapabilityValue::Bool(false)).ok()?;
    }
    let drawable = draw_of(w.as_mut()).expect("draw bridge");
    Some((render_widget_to_svg_on(drawable, geometry, Color::BLACK), window))
}

#[test]
fn probe_a_disabled_surface_recedes_on_both_appearances() {
    let _guard = rust_widgets::theme::theme_test_guard();

    // # Which controls this can measure
    //
    // Only the ones whose field is painted without extra input. The four media shells
    // (`animated_image`, `lottie_widget`, `rive_widget`, `hero_animation`) paint that surface
    // **inside** their loaded-content branch, so with no frames loaded they draw their empty state
    // instead and the surface this probe is meant to check is never painted. They are listed
    // separately and asserted **unmeasured**, so a silent skip cannot read as a pass -- which is
    // how the empty-state plate was mistaken for the shell on the first run.
    let cases: &[(&str, u32, u32)] = &[
        ("search_bar", 240, 40),
        ("masked_edit", 240, 40),
        ("search_box", 240, 40),
        ("tag_input", 240, 40),
        ("navigation_drawer", 200, 160),
        // `hero_animation` paints its shell unconditionally, so its disabled branch is reachable
        // without supplying content -- confirmed by the surface differing between the two states.
        ("hero_animation", 200, 120),
    ];
    let needs_content: &[(&str, u32, u32)] =
        &[("animated_image", 200, 120), ("lottie_widget", 200, 120), ("rive_widget", 200, 120)];

    for (name, w, h) in cases {
        let geometry = Rect::new(0, 0, *w, *h);
        for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
            let (Some((on_svg, window)), Some((off_svg, window_off))) = (
                render(name, geometry, false, appearance),
                render(name, geometry, true, appearance),
            ) else {
                continue;
            };
            assert_eq!(
                window, window_off,
                "the appearance must not change between the two renders"
            );

            // The surface is the fill whose rectangle **is** the field. Reported as a list so the
            // reader can confirm which one was matched -- a guess was wrong three times here.
            let on = surface_of(&on_svg, *w, *h);
            let off = surface_of(&off_svg, *w, *h);
            println!("[{name}/{appearance:?}] window rgba({},{},{})", window.r, window.g, window.b);
            for (tag, c) in [("enabled ", on), ("disabled", off)] {
                match c {
                    Some(c) => println!(
                        "    {tag} surface rgba({},{},{}) -> vs window {:.2}:1",
                        c.r,
                        c.g,
                        c.b,
                        contrast(c, window)
                    ),
                    None => println!("    {tag} no surface matched"),
                }
            }

            let (Some(on), Some(off)) = (on, off) else {
                panic!(
                    "{name} painted no surface this probe can identify, so its disabled \
                     recede is **unmeasured** rather than verified"
                );
            };
            let (p_on, p_off) = (contrast(on, window), contrast(off, window));
            assert!(
                p_off <= p_on + 0.01,
                "{name}'s disabled surface is MORE prominent than its enabled one on the \
                 {appearance:?} appearance ({p_off:.2}:1 disabled vs {p_on:.2}:1 enabled). A \
                 disabled fill must recede toward the page; this is the \
                 `base_bg.blend(&Color::WHITE, 0.35)` defect, where a dark surface is moved up \
                 toward white and away from the window it sits in."
            );
        }
    }

    // The shells: asserted to be unmeasurable *here*, with the reason, rather than quietly
    // omitted. Their disabled surface needs loaded content, so a probe that cannot supply frames,
    // a Lottie document or a Rive document cannot reach the branch.
    for (name, w, h) in needs_content {
        let geometry = Rect::new(0, 0, *w, *h);
        let (Some((on_svg, _)), Some((off_svg, _))) = (
            render(name, geometry, false, AppearanceMode::Dark),
            render(name, geometry, true, AppearanceMode::Dark),
        ) else {
            continue;
        };
        assert_eq!(
            surface_of(&on_svg, *w, *h),
            surface_of(&off_svg, *w, *h),
            "{name} has no loaded content, so it paints its empty state in both states: its \
             disabled shell is **not** exercised and must not be reported as passing"
        );
    }
}
