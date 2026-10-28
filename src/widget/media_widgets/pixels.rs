// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared RGBA pixel helpers used by the media controls.
//!
//! Two controls (`AnimatedImage` and `HeroAnimation`) each carried their own
//! nearest-neighbour resampler. They were byte-for-byte identical, so one copy
//! lives here and both call into it (principle #101). The callers own the caching
//! policy: this module only computes.

use crate::compat::Vec;

/// Nearest-neighbour resample of an RGBA buffer into `width` x `height`.
///
/// A source buffer shorter than the declared geometry is read from its last
/// complete pixel rather than panicking, which is the behaviour the media
/// controls had before the implementations were merged.
pub(crate) fn nearest_scale(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    nearest_scale_into(data, source_width, source_height, width, height, &mut out);
    out
}

/// Resamples into `out`, reusing its capacity when the destination size is unchanged.
///
/// The buffer is only resized when its length disagrees with the destination geometry, so a
/// playback frame that keeps the same viewport re-samples in place instead of allocating.
pub(crate) fn nearest_scale_into(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    width: u32,
    height: u32,
    out: &mut Vec<u8>,
) {
    let length = (width as usize) * (height as usize) * 4;
    if out.len() != length {
        out.resize(length, 0);
    }
    if source_width == 0 || source_height == 0 || width == 0 || height == 0 {
        return;
    }
    for y in 0..height {
        let source_y = (y as u64 * source_height as u64 / height as u64) as u32;
        for x in 0..width {
            let source_x = (x as u64 * source_width as u64 / width as u64) as u32;
            let source = ((source_y as usize * source_width as usize + source_x as usize) * 4)
                .min(data.len().saturating_sub(4));
            let destination = (y as usize * width as usize + x as usize) * 4;
            out[destination..destination + 4].copy_from_slice(&data[source..source + 4]);
        }
    }
}
