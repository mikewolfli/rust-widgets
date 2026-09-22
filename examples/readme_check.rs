// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Compile-checks the code samples in `README.md`.
//!
//! # Why the samples live in an example rather than in doc comments
//!
//! A README snippet is a *claim about the API*, and a claim that is never compiled drifts.
//! Each block below is copied from the README verbatim apart from the surrounding `fn`, so a
//! signature change that breaks the documentation fails this build instead of shipping.

use rust_widgets::core::{Color, Rect};
use rust_widgets::layout::{AxisHints, ChildInfo, FlexLayout, Hints, Layout, LayoutParams};
use rust_widgets::style::WidgetStyle;
use rust_widgets::widget::base_widgets::button::Button;
use rust_widgets::widget::Widget;

/// The "Hello, control" block.
fn hello_control() -> String {
    // A control is a value placed in a `Rect`. Rendering it to SVG is the shortest path
    // to seeing it; a backend only changes where the pixels go, not how the control draws.
    let mut button = Button::new("Click me".to_string(), Rect::new(0, 0, 160, 36));

    // Style is a plain struct of optional values, so an unset field inherits instead of
    // overwriting what the theme resolved for this control.
    let style = WidgetStyle {
        background_color: Some(Color::rgb(33, 150, 243)),
        text_color: Some(Color::WHITE),
        border_radius: Some(6),
        ..WidgetStyle::default()
    };
    button.set_style(style);

    rust_widgets::widget::svg::render_to_svg(&mut button)
}

/// The "Laying out a control" block.
fn lay_out_a_control(widget_id: rust_widgets::core::ObjectId, rect: Rect) -> Vec<(u64, Rect)> {
    // 120 px wide minimum, would like 200, never past 400; free to stretch horizontally.
    let hints = Hints { width: AxisHints::new(120, 200, 400), height: AxisHints::fixed(32) };
    let children = [ChildInfo::new(widget_id, hints).with_params(LayoutParams::filled())];

    let mut layout = FlexLayout::new();
    layout.add_widget(widget_id, 1);
    let mut out = Vec::new();
    layout.arrange(rect, &children, &mut |id, child_rect| out.push((id, child_rect)));
    out
}

fn main() {
    let svg = hello_control();
    assert!(svg.starts_with("<svg"), "the README's first example must render");

    let placed = lay_out_a_control(1, Rect::new(0, 0, 400, 60));
    assert_eq!(placed.len(), 1, "the README's layout example must place its child");
    // `fill` on the major axis (a row's width) means the child absorbs the leftover room —
    // so it takes the whole row rather than its 200 px preferred width.
    assert_eq!(placed[0].1.width, 400);
    println!("readme samples: ok");
}
