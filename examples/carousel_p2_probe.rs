// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Throwaway diagnostic: what does the empty `carousel` actually paint, and over what?
//!
//! P2 reports "visible_against_its_surface == false" for `carousel`, which means either
//! it painted nothing over the theme background, or its dominant colour *is* the theme
//! background. This prints the numbers so the fix is chosen from evidence.

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::theme::{global_theme_manager, AppearanceMode, Theme};
use rust_widgets::widget::census::{install_preset_appearances, CENSUS_RECT, CENSUS_TEXT};
use rust_widgets::widget::{draw_bridge::draw_of, WidgetFactory};

fn count(frame: &[u8], background: Color) -> (u32, Option<Color>, u32) {
    let bg = (background.r, background.g, background.b);
    let mut counts: std::collections::HashMap<(u8, u8, u8), u32> = std::collections::HashMap::new();
    let mut non_bg = 0u32;
    for px in frame.chunks_exact(4) {
        let (r, g, b, a) = (px[0], px[1], px[2], px[3]);
        if a == 0 {
            continue;
        }
        if (r, g, b) != bg {
            non_bg += 1;
            *counts.entry((r, g, b)).or_insert(0) += 1;
        }
    }
    let dominant = counts.iter().max_by_key(|(_, c)| **c).map(|(rgb, _)| *rgb);
    let dominant_count = dominant.map(|rgb| counts[&rgb]).unwrap_or(0);
    (dominant_count, dominant.map(|(r, g, b)| Color::rgb(r, g, b)), non_bg - dominant_count)
}

fn main() {
    install_preset_appearances();
    let factory = WidgetFactory::new_with_defaults();

    for appearance in [AppearanceMode::Light, AppearanceMode::Dark] {
        global_theme_manager().set_appearance(appearance);
        let surface = global_theme_manager()
            .current_theme()
            .map(|t| t.colors.background)
            .unwrap_or(Color::WHITE);
        let mut widget = factory
            .create("carousel", CENSUS_RECT, CENSUS_TEXT)
            .expect("carousel must be registered");

        let mut backend = SoftwarePaintBackend::new(
            Size { width: CENSUS_RECT.width, height: CENSUS_RECT.height },
            1.0,
        );
        backend.begin_frame(surface);
        if let Some(drawable) = draw_of(&mut *widget) {
            let mut ctx = RenderContext::new(&mut backend);
            drawable.draw(&mut ctx);
        }
        backend.end_frame();

        let (dominant, dominant_color, detail) = count(backend.frame_rgba(), surface);
        println!("{appearance:?}: surface={surface:?} dominant_count={dominant} dominant={dominant_color:?} non_dominant_painted={detail}");
    }
    let _ = Theme::default();
    let _ = Rect::new(0, 0, 1, 1);
}
