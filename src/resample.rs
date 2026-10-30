// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Bilinear RGBA8 resampling, shared by the image layer and the render layer.
//!
//! # Why this is a crate-root module rather than part of `crate::image`
//!
//! `crate::image` is compiled only when the `image` feature is on (decoding pulls the decoders in),
//! but the *render* layer needs to resample pixels in every profile: a widget can hand the paint
//! layer raw RGBA8 bytes from a host tile or a captured frame without the image feature being on,
//! and `PaintBackend::draw_image_scaled` is the entry that turns "pixels of one size" into "the
//! rectangle of another". Keeping the math here — pure bytes in, pure bytes out, no image types —
//! is what lets both layers use one implementation instead of a gated copy each.
//!
//! The bilinear weights are the same ones the software surface's quad sampler uses: a scaled image
//! is softened by the resample, never block-copied.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/image/transform.rs:39` (re-exported as
//! `image::transform::resize_rgba8`) and `src/widget/icon_tint.rs:198` (the icon tint's resample).

use crate::compat::{format, String, Vec};

/// Converts RGBA8 bytes from one extent to another with bilinear interpolation.
///
/// `Err` describes which invariant the caller broke — a zero target, dimensions that overflow
/// `usize`, or a source buffer that does not match `src_w x src_h`. Nothing is allocated before the
/// source is validated.
pub fn resize_rgba8(
    pixels: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Result<Vec<u8>, String> {
    if dst_w == 0 || dst_h == 0 {
        return Err(format!("resize target must be at least 1x1 pixels, got {dst_w}x{dst_h}"));
    }
    // The target byte count must fit in `usize`; reject oversized dimensions rather
    // than let a hostile `dst_w`/`dst_h` wrap the multiply and reallocate unbounded.
    let total = (dst_w as usize)
        .checked_mul(dst_h as usize)
        .and_then(|px| px.checked_mul(4))
        .ok_or("resize target dimensions overflow")?;
    let expected_len = (src_w as usize)
        .checked_mul(src_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Source dimensions overflow")?;
    if pixels.len() != expected_len || src_w == 0 || src_h == 0 {
        return Err(format!(
            "resize source is {src_w}x{src_h}, which needs {} RGBA8 bytes, but the buffer \
             holds {} — pass a buffer matching src_w x src_h",
            expected_len,
            pixels.len()
        ));
    }
    let mut out = Vec::with_capacity(total);

    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;

    // Precompute the source mapping for each destination column/row. The previous version
    // recomputed both ratios, the clamps and the fractional parts for every output pixel, and
    // built a `get_pixel` closure inside the inner loop.
    let mut x_map: Vec<(u32, u32, f32)> = Vec::with_capacity(dst_w as usize);
    for dx in 0..dst_w {
        let sx = dx as f32 * x_ratio;
        let x1 = (sx as u32).min(src_w.saturating_sub(1));
        let x2 = (x1 + 1).min(src_w.saturating_sub(1));
        x_map.push((x1, x2, sx - x1 as f32));
    }
    let mut y_map: Vec<(u32, u32, f32)> = Vec::with_capacity(dst_h as usize);
    for dy in 0..dst_h {
        let sy = dy as f32 * y_ratio;
        let y1 = (sy as u32).min(src_h.saturating_sub(1));
        let y2 = (y1 + 1).min(src_h.saturating_sub(1));
        y_map.push((y1, y2, sy - y1 as f32));
    }

    let stride = src_w as usize * 4;
    for &(y1, y2, y_frac) in &y_map {
        let row1 = y1 as usize * stride;
        let row2 = y2 as usize * stride;
        for &(x1, x2, x_frac) in &x_map {
            let a = row1 + x1 as usize * 4;
            let b = row1 + x2 as usize * 4;
            let c = row2 + x1 as usize * 4;
            let d = row2 + x2 as usize * 4;
            for channel in 0..4 {
                let v00 = pixels[a + channel] as f32;
                let v10 = pixels[b + channel] as f32;
                let v01 = pixels[c + channel] as f32;
                let v11 = pixels[d + channel] as f32;

                let v0 = v00 * (1.0 - x_frac) + v10 * x_frac;
                let v1 = v01 * (1.0 - x_frac) + v11 * x_frac;
                let val = v0 * (1.0 - y_frac) + v1 * y_frac;
                out.push(val as u8);
            }
        }
    }
    Ok(out)
}
