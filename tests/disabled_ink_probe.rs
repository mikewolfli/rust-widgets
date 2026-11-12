//! Does a disabled control actually **look** disabled?
//!
//! # The defect this pins
//!
//! `U3-03` found 26 controls that gated every handler on `base.is_enabled()` and painted the
//! **same bytes** in both states. The interaction contract was kept and the appearance was not:
//! a reader saw a live control that silently refused input. Each of them now derives its ink
//! through `BaseWidget::disabled_ink_on`, which steps the ink toward the surface's contrast
//! colour — the crate's one disabled-ink rule (`disabled_surface_probe.rs` pins the *fill*
//! direction; this file pins the *ink* direction).
//!
//! # The assertion
//!
//! For every listed control, the disabled render must differ from the enabled one. A control
//! that paints the same picture is either not wired to the rule or the rule resolves to the
//! same colour on the active appearance — both are the "silently refuses input" shape.
//!
//! Controls whose content is the feature are filled through
//! [`rust_widgets::widget::sample_fill`], the same verification-only path the snapshots use, so
//! the comparison is made on the rows the control exists to draw rather than on an empty frame.
//! `sample_fill::apply` returning `false` for one of them would mean the render below is vacuous,
//! so the lists are split and the split is asserted rather than assumed.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test disabled_ink_probe

// The full registry and the theme layer are compiled out of the reduced `mini`/`embedded`
// profiles, so this probe is gated rather than rewritten against APIs those profiles lack.
#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Rect};
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::WidgetFactory;

/// Render one control with `enabled` set as asked.
fn render(name: &str, geometry: Rect, disabled: bool) -> Option<String> {
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create(name, geometry, "x")?;
    // Verification-only sample data: a control whose rows are its feature must be measured on
    // those rows. `apply` is a no-op for the controls that draw without content.
    rust_widgets::widget::sample_fill::apply(name, widget.as_mut());
    if disabled {
        factory.write_property(widget.as_mut(), "enabled", CapabilityValue::Bool(false)).ok()?;
    }
    let drawable = draw_of(widget.as_mut()).expect("draw bridge");
    Some(render_widget_to_svg_on(drawable, geometry, Color::BLACK))
}

/// Assert that one control's disabled render differs from its enabled render.
fn assert_disabled_is_visible(name: &str, w: u32, h: u32) {
    let geometry = Rect::new(0, 0, w, h);
    let on = render(name, geometry, false).unwrap_or_else(|| panic!("{name} did not construct"));
    let off = render(name, geometry, true).unwrap_or_else(|| panic!("{name} did not construct"));
    assert!(on.contains("fill="), "{name} painted nothing at all");
    assert_ne!(
        on, off,
        "{name} paints an identical picture enabled and disabled: the state is enforced in the \
         handler but invisible on screen (U3-03). Its draw must derive its ink through \
         `BaseWidget::disabled_ink_on`."
    );
}

#[test]
fn the_controls_that_draw_without_content_show_their_disabled_state() {
    // These paint their own chrome from factory defaults; `sample_fill` has no arm for them,
    // and that is fine — the paint under test is already there.
    for (name, w, h) in [
        ("color_well", 120u32, 64u32),
        ("pagination", 240, 40),
        ("swatch", 160, 96),
        // T3-40: the four controls the U3-03 roster gained after this probe was written. Their
        // draw shows chrome even without sample content, so the ink assertion is meaningful here.
        ("roller", 120, 160),
        ("date_edit", 240, 40),
        ("time_edit", 160, 40),
        ("date_time_edit", 240, 40),
    ] {
        assert_disabled_is_visible(name, w, h);
    }
}

#[test]
fn the_controls_whose_rows_are_the_feature_show_their_disabled_state() {
    // Every one of these has a `sample_fill` arm. The arm is asserted below, so a control whose
    // arm was removed (and whose render therefore became an empty frame) cannot pass as measured.
    // The two text-editor companions are registered only with their own features, so they are
    // added under the same `cfg` as the registration rather than skipped silently.
    #[allow(unused_mut)]
    let mut cases: Vec<(&str, u32, u32)> = vec![
        ("data_grid", 240, 160),
        ("tree_view", 240, 160),
        ("dropdown", 240, 40),
        ("range_preset_bar", 240, 40),
        ("command_palette", 240, 160),
        ("segmented_control", 240, 40),
        ("breadcrumb", 240, 32),
        ("dial", 96, 96),
        ("menu_panel", 200, 96),
        ("speed_dial", 200, 160),
        ("layer_panel", 200, 160),
        ("playlist_panel", 200, 160),
        ("filter_row", 240, 40),
        ("drum_column", 80, 160),
        ("chip_input", 240, 64),
        ("emoji_picker", 240, 200),
        ("query_builder", 240, 160),
    ];
    #[cfg(feature = "markdown-editor")]
    cases.push(("markdown_outline_panel", 200, 160));
    #[cfg(any(feature = "code-editor", feature = "markdown-editor"))]
    cases.push(("find_bar", 260, 40));

    for (name, w, h) in cases {
        let geometry = Rect::new(0, 0, w, h);
        let factory = WidgetFactory::new_with_defaults();
        let mut widget = factory
            .create(name, geometry, "x")
            .unwrap_or_else(|| panic!("{name} is not a registered kind"));
        assert!(
            rust_widgets::widget::sample_fill::apply(name, widget.as_mut()),
            "{name} has no `sample_fill` arm, so its disabled state would be measured on an empty \
             frame. Add the arm or move the control to the no-content list."
        );
        assert_disabled_is_visible(name, w, h);
    }
}
