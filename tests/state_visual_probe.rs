//! Do a control's **disabled** and **hover** states reach the screen?
//!
//! # The defect this pins
//!
//! W5-03 found 83 controls whose input gate closed when disabled while the picture stayed
//! identical, and W5-04 found 87 with no hover feedback at all. Both are the same shape: the
//! interaction contract is enforced and the appearance is not, so a reader sees a live control
//! that silently refuses input, or a clickable one that never answers the pointer.
//!
//! The mechanism is centralised — [`BaseWidget::paint_disabled_veil`] and
//! [`BaseWidget::paint_hover_wash`] derive from the same `DISABLED_VEIL_ALPHA` /
//! `HOVER_FILL_BLEND` weights the rest of the crate uses — but each draw still has to call it.
//! This probe is that call's evidence: for every listed control the two renders must differ.
//!
//! Run with:
//!   cargo test --no-default-features --features desktop --test state_visual_probe

#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::{Color, Point, Rect};
use rust_widgets::event::Event;
use rust_widgets::widget::capability::types::CapabilityValue;
use rust_widgets::widget::draw_bridge::draw_of;
use rust_widgets::widget::svg::render_widget_to_svg_on;
use rust_widgets::widget::WidgetFactory;

/// Render one control, optionally disabled and optionally hovered.
fn render(name: &str, geometry: Rect, disabled: bool, hovered: bool) -> Option<String> {
    let factory = WidgetFactory::new_with_defaults();
    let mut widget = factory.create(name, geometry, "x")?;
    rust_widgets::widget::sample_fill::apply(name, widget.as_mut());
    if disabled {
        factory.write_property(widget.as_mut(), "enabled", CapabilityValue::Bool(false)).ok()?;
    }
    if hovered {
        widget.handle_event(&Event::MouseEnter { pos: Point::new(8, 8) });
    }
    let drawable = draw_of(widget.as_mut()).expect("draw bridge");
    Some(render_widget_to_svg_on(drawable, geometry, Color::BLACK))
}

fn assert_state_is_visible(name: &str, w: u32, h: u32, disabled: bool) {
    let geometry = Rect::new(0, 0, w, h);
    let rest =
        render(name, geometry, false, false).unwrap_or_else(|| panic!("{name} did not construct"));
    let changed = if disabled {
        render(name, geometry, true, false)
    } else {
        render(name, geometry, false, true)
    }
    .unwrap_or_else(|| panic!("{name} did not construct"));
    assert!(rest.contains("fill="), "{name} painted nothing at all");
    let what = if disabled { "disabled" } else { "hovered" };
    assert_ne!(
        rest, changed,
        "{name} paints an identical picture when {what}: the state is tracked but invisible on \
         screen. Its draw must call `paint_disabled_veil` / `paint_hover_wash`."
    );
}

/// A representative sample of the W5-03 roster: one control from each drawing family.
#[test]
fn disabled_controls_paint_a_different_frame() {
    for (name, w, h) in [
        // Display / chrome.
        ("meter", 200u32, 200u32),
        ("lcd_number", 160, 60),
        ("progress_bar", 200, 20),
        ("tag", 120, 32),
        ("badge", 80, 32),
        ("scroll_bar", 16, 200),
        // Inputs.
        ("combo_box", 200, 36),
        ("spin_box", 160, 36),
        ("number_picker", 140, 36),
        ("rich_edit", 240, 120),
        ("inplace_editor", 200, 32),
        ("cascader", 200, 36),
        // Containers / views.
        ("accordion", 240, 160),
        ("tab_view", 240, 160),
        ("tab_widget", 240, 160),
        ("tool_box", 240, 160),
        ("list_view", 240, 160),
        ("scroll_area", 240, 160),
        ("safe_area", 240, 120),
        // Overlays / dialogs.
        ("banner", 240, 64),
        ("splash_screen", 240, 160),
        ("popover", 240, 160),
        ("popup_window", 240, 160),
        ("bottom_sheet", 240, 160),
        ("modal_bottom_sheet", 240, 160),
        ("message_box", 240, 120),
        // Charts.
        ("heatmap", 240, 160),
        ("radar_chart", 240, 200),
        ("progress_bar", 200, 20),
    ] {
        assert_state_is_visible(name, w, h, true);
    }
}

/// A representative sample of the W5-04 roster: controls that previously did not answer the
/// pointer at all.
#[test]
fn hovered_controls_paint_a_different_frame() {
    for (name, w, h) in [
        ("legend", 200u32, 120u32),
        ("speed_dial", 160, 160),
        ("swatch", 160, 96),
        ("chart_data_table", 240, 160),
        ("calendar_mark_layer", 240, 160),
        ("command_palette", 320, 200),
        ("breadcrumb", 240, 36),
        ("chip_input", 240, 36),
        ("barcode_scanner", 240, 160),
        ("color_picker", 240, 160),
        ("group_box", 240, 160),
        ("date_time_edit", 240, 36),
        ("font_combo_box", 200, 36),
        ("swipe_actions", 240, 80),
        ("toast_host", 240, 80),
        ("notification_center", 240, 200),
    ] {
        assert_state_is_visible(name, w, h, false);
    }
}
