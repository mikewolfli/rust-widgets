//! Evidence for BLUE23 §89: what a **shown** tooltip actually paints.
//!
//! `snapshots/svg/tooltip.svg` shows the hidden/rest state, where `draw` composites the
//! bubble fully toward the window (`fade.value() == 0.0`), so the committed picture's bubble
//! fill *equals* the window fill and says nothing about the control. This test shows the
//! tooltip, settles the fade, and prints the chrome it drew, so the bubble's fill can be
//! compared against the window fill it sits on.
//!
//! It is the measurement that found the defect, kept runnable so the claim in `tooltip.rs`'s
//! `draw` docs stays checkable. Run with:
//!
//! ```text
//! cargo test --no-default-features --features desktop --test tooltip_paint_probe -- --nocapture
//! ```

// Needs the full widget registry and the theme layer; the reduced
// `mini`/`embedded` profiles compile both out, so the file is gated rather than
// rewritten to avoid APIs those profiles do not have.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Rect};
use rust_widgets::theme::AppearanceMode;
use rust_widgets::widget::capability::coercion::widget_as_mut;
use rust_widgets::widget::census::install_preset_appearances;
use rust_widgets::widget::dialog::Tooltip;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::WidgetFactory;

/// Renders a shown tooltip and prints its chrome, per appearance.
///
/// The printed fills are the evidence: before the fix both appearances printed
/// `rgba(40,40,40,0.86)` (1.27:1 against the dark window); after it, the dark appearance
/// prints a light bubble and the light appearance a dark one.
#[test]
fn probe_what_a_shown_tooltip_paints() {
    let _guard = rust_widgets::theme::theme_test_guard();
    install_preset_appearances();
    let geometry = Rect::new(0, 0, 240, 120);
    let factory = WidgetFactory::new_with_defaults();

    let mut bubbles = Vec::new();
    for appearance in [AppearanceMode::Dark, AppearanceMode::Light] {
        rust_widgets::theme::global_theme_manager().set_appearance(appearance);
        let mut widget = factory.create("tooltip", geometry, "hint").expect("tooltip");
        if let Some(tooltip) = widget_as_mut::<Tooltip>(widget.as_mut()) {
            tooltip.set_text("this is the hint");
            tooltip.show();
        }
        for _ in 0..40 {
            if !widget.is_animating() {
                break;
            }
            let _ = widget.tick(16);
        }
        let animating = widget.is_animating();
        let svg = {
            let drawable = draw_of(widget.as_mut()).expect("draw bridge");
            render_widget_to_svg_on(drawable, geometry, Color::BLACK)
        };
        println!("=== appearance={appearance:?} is_animating={animating}");
        for line in svg.lines() {
            let trimmed = line.trim();
            if trimmed.contains("<path") {
                println!("  <path ... glyph run>");
            } else if trimmed.starts_with("<rect") || trimmed.starts_with("<svg") {
                println!("  {trimmed}");
                if trimmed.contains("<rect") {
                    bubbles.push(trimmed.to_string());
                }
            }
        }
    }

    // The assertion is deliberately weak — the *relations* are asserted in
    // `tooltip::tests::a_shown_bubble_inverts_against_its_window`, next to the code they are
    // about. This one only fails if the control stopped painting a bubble at all, which would
    // make the printed evidence above misleading.
    assert!(
        !bubbles.is_empty(),
        "a shown tooltip must paint a bubble in every appearance; nothing was found"
    );
}
