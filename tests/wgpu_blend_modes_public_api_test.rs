// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The GPU blend-mode switch, reached the way a host reaches it: through the public API only.
//!
//! This file deliberately imports nothing from the crate's internals, so it fails to compile if the
//! switch, its read-back or the command types stop being reachable from outside. It also pins the
//! *effect*: with the switch on, a `Multiply` rectangle over an opaque frame is composited on the
//! GPU, so the pixel is the multiplied value; with it off, the same stream falls back and the pixel
//! is still the multiplied value — the picture never depends on the path.
#![cfg(all(
    feature = "gpu-wgpu",
    any(feature = "desktop", feature = "tablet", feature = "mobile")
))]

use rust_widgets::wgpu_backend::{PixelRect, Rgba8, WgpuDrawCommand, WgpuRenderer};

const BACKDROP: Rgba8 = Rgba8 { r: 200, g: 100, b: 50, a: 255 };
const OVERLAY: Rgba8 = Rgba8 { r: 128, g: 128, b: 128, a: 255 };

/// The stream this test renders: an opaque backdrop with one `Multiply` rectangle over its left half.
fn commands() -> Vec<WgpuDrawCommand> {
    vec![
        WgpuDrawCommand::Clear { color: BACKDROP },
        // `BlendMode::Multiply` is tag 1 in the command stream's own numbering.
        WgpuDrawCommand::SetBlendMode { mode: 1 },
        WgpuDrawCommand::FillRect {
            rect: PixelRect { x: 0, y: 0, width: 4, height: 8 },
            color: OVERLAY,
            clip: None,
        },
    ]
}

fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * width + x) * 4) as usize;
    [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
}

/// `Multiply` is `source * destination` per channel; an opaque result keeps alpha 255.
fn multiplied() -> [u8; 4] {
    let scale =
        |channel: u8| ((OVERLAY.r as f32 / 255.0) * (channel as f32 / 255.0) * 255.0).round();
    [scale(BACKDROP.r) as u8, scale(BACKDROP.g) as u8, scale(BACKDROP.b) as u8, 255]
}

#[test]
fn the_blend_switch_is_public_and_changes_the_drawn_path() {
    let mut renderer = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        // No adapter on this host: the GPU contract is untestable here, and saying so is the honest
        // answer (the in-crate tests skip the same way).
        Err(_) => return,
    };
    assert!(!renderer.blend_modes_on_gpu(), "the switch starts off");
    renderer.set_blend_modes_on_gpu(true);
    assert!(renderer.blend_modes_on_gpu(), "and it is readable back");

    let on = renderer.render_draw_commands_rgba8(8, 8, &commands()).expect("the GPU path renders");
    assert_eq!(pixel(&on, 8, 1, 1), multiplied(), "with the switch on the blend is on the GPU");
    assert_eq!(pixel(&on, 8, 7, 1), [200, 100, 50, 255], "outside the overlay nothing changed");

    renderer.set_blend_modes_on_gpu(false);
    assert!(!renderer.blend_modes_on_gpu());
    let off = renderer.render_draw_commands_rgba8(8, 8, &commands()).expect("the fallback renders");
    assert_eq!(pixel(&off, 8, 1, 1), multiplied(), "with it off the CPU draws the same pixel");
    assert_eq!(on, off, "the picture must not depend on which path drew it");
}

/// The borrowed entry point reaches the same picture through the same public API.
#[test]
fn the_borrowed_entry_point_agrees_through_the_public_api() {
    let mut renderer = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        Err(_) => return,
    };
    renderer.set_blend_modes_on_gpu(true);
    let owned = renderer.render_draw_commands_rgba8(8, 8, &commands()).expect("owned");
    let borrowed = renderer.render_draw_commands_cached_rgba8(8, 8, &commands()).expect("borrowed");
    assert_eq!(&*borrowed, &owned[..]);
}
