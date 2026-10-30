// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Glyph painting: the 8x8 coverage cell loop, the coverage upload the GPU text pipeline reads,
//! and the CPU composite of a text box.

use super::super::types::{PixelRect, Rgba8};
use super::fills::{blend_cpu_rgba8_with_mode, PixelSink};
use crate::render::BlendMode;
use std::cell::RefCell;

thread_local! {
    /// Reusable glyph coverage scratch for [`draw_text_cpu_rgba8`], retained across text
    /// commands instead of a per-command allocation (G-222). It is re-zeroed per command, so a
    /// reused buffer starts exactly where the fresh `vec![0u8; n]` it replaced did.
    static TEXT_COVERAGE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Calls `visit` once per glyph of `text` with its cell origin and the 8x8 coverage buffer.
///
/// This is the **one** definition of the text grid: the glyph cell is 8x8, characters fill the box
/// left-to-right then top-to-bottom, and each glyph's ink comes from the active font stack through
/// `paint_active`. The CPU rasteriser blends the coverage it receives; the encoder copies it into
/// the coverage texture it uploads. Keeping both on this loop is what stops the two paths from
/// disagreeing about where a character sits or what shape it has.
pub(crate) fn for_each_text_cell(
    rect: PixelRect,
    text: &str,
    mut visit: impl FnMut(i32, i32, &[u8]),
) {
    let glyph_w = 8i32;
    let glyph_h = 8i32;
    let columns = (rect.width as i32 / glyph_w).max(1);
    let rows = (rect.height as i32 / glyph_h).max(1);
    let cell = crate::render::text::Cell::new(glyph_w as u32, glyph_h as u32);
    // One scratch for the whole string, reused per glyph: a glyph is rasterised, consumed, and
    // overwritten. That keeps this path free of per-glyph allocation and of any glyph cache —
    // the same contract the software rasteriser honours (BLUE23 §0A.4, constraint 3). The
    // scratch itself is retained across commands (G-222).
    let mut coverage = TEXT_COVERAGE.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
    coverage.clear();
    coverage.resize(cell.area(), 0);
    for (char_index, scalar) in text.chars().enumerate() {
        let grid_index = char_index as i32;
        if grid_index >= columns * rows {
            break;
        }
        let col = grid_index % columns;
        let row = grid_index / columns;
        let origin_x = rect.x + col * glyph_w;
        let origin_y = rect.y + row * glyph_h;
        if crate::render::text::paint_active(scalar, cell, &mut coverage).is_none() {
            continue;
        }
        visit(origin_x, origin_y, &coverage[..cell.area()]);
    }
    // Hand the scratch (and its capacity) back for the next text command (G-222).
    TEXT_COVERAGE.with(|cell| *cell.borrow_mut() = std::mem::take(&mut coverage));
}

/// Paints the text box's glyph coverage into `out`, one byte per pixel of `rect`.
///
/// `out` must be exactly `rect.width * rect.height` bytes. The encoder uploads this as the coverage
/// texture its fragment shader multiplies by the text colour, so glyph rasterisation stays on the
/// same code the CPU rasteriser uses while the per-pixel compositing moves to the GPU.
pub(crate) fn text_coverage_into(rect: PixelRect, text: &str, out: &mut [u8]) {
    out.fill(0);
    let width = rect.width as i32;
    let height = rect.height as i32;
    for_each_text_cell(rect, text, |origin_x, origin_y, coverage| {
        for py in 0..8i32 {
            let target_y = origin_y - rect.y + py;
            if target_y < 0 || target_y >= height {
                continue;
            }
            for px in 0..8i32 {
                let target_x = origin_x - rect.x + px;
                if target_x < 0 || target_x >= width {
                    continue;
                }
                let value = coverage[(py * 8 + px) as usize];
                if value != 0 {
                    out[target_y as usize * width as usize + target_x as usize] = value;
                }
            }
        }
    });
}

pub(crate) fn draw_text_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    text: &str,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width, mode: blend };
    let (x_start, y_start) = (clip_rect.x, clip_rect.y);
    let (x_end, y_end) = (clip_rect.right(), clip_rect.bottom());
    for_each_text_cell(rect, text, |origin_x, origin_y, coverage| {
        for py in 0..8i32 {
            let screen_y = origin_y + py;
            if screen_y < y_start || screen_y >= y_end {
                continue;
            }
            for px in 0..8i32 {
                let screen_x = origin_x + px;
                if screen_x < x_start || screen_x >= x_end {
                    continue;
                }
                let value = coverage[(py * 8 + px) as usize];
                if value == 0 {
                    continue;
                }
                if value == 255 {
                    sink.set(screen_x as u32, screen_y as u32, color);
                } else {
                    blend_cpu_rgba8_with_mode(
                        sink.pixels,
                        width,
                        screen_x as u32,
                        screen_y as u32,
                        color,
                        value as f32 / 255.0,
                        blend,
                    );
                }
            }
        }
    });
}
