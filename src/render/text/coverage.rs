// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A shaped run's glyph coverage, in the layout the reference text renderer uses.
//!
//! # Why this exists
//!
//! The GPU text pipeline consumes one coverage byte per destination pixel and multiplies it by the
//! text colour. Until this module existed the only producer of that coverage was the backend's
//! fixed 8x8 character grid, which is **not** the layout a control's text uses: the reference
//! renderer shapes the line, advances a pen by each cluster's own advance and paints a glyph into a
//! cell as wide as that advance. Bridging `DrawText` to the grid would have moved every label by a
//! few pixels, so the command stayed on the reference rasteriser instead.
//!
//! This module produces the same coverage the reference compositor would have consumed:
//!
//! 1. shape the line with [`shape_line`], the one cluster derivation both backends use;
//! 2. derive the line box from [`TextMetrics::for_font`], exactly as the software surface does;
//! 3. rasterise each cluster into a cell of `advance × line height` through the same
//!    `paint_active` entry point the software surface uses;
//! 4. apply the same display-transfer ramp the software compositor applies to glyph ink, so the
//!    coverage a GPU multiplies by the colour has the same weight as the reference's alpha.
//!
//! The result is one rectangle of coverage bytes plus its origin. Nothing about the *drawing* is
//! reimplemented here: the glyph shapes, the advances and the ramp all come from the code the
//! reference renderer already uses.
//!
//! # Why the result is cached
//!
//! A static label is rasterised again on every frame without help: the software path pays that
//! cost, and a recording backend would pay it a second time only to hand the bytes to the GPU. The
//! answer depends on `(text, font, scale, face epoch)`, never on the origin or the alignment — the
//! ink's offset from the *adjusted* origin is the same for every placement — so a bounded cache
//! keyed that way removes the per-frame rasterisation for every label that did not change, and the
//! cached bytes are shared by refcount instead of copied into each command.
//!
//! A cache is only sound if it can be invalidated: the entry records the text-measurement epoch,
//! which the runtime-font registration path bumps, so a face that changes resolution makes every
//! older entry unreachable rather than silently stale.

use crate::compat::Arc;
use crate::core::{Font, HorizontalAlignment, Point};
#[cfg(not(feature = "text-shaping"))]
use crate::render::text::paint_active;
use crate::render::text::{shape_line, Cell};
use crate::render::TextMetrics;
use core::cell::RefCell;

/// One shaped text box's coverage, ready to upload as an `R8Unorm` texture.
///
/// `width * height == bytes.len()`; a zero-sized box is the honest answer for a string that paints
/// no ink (only whitespace, or a face that produced nothing), and its consumer skips it.
#[derive(Debug, Clone)]
pub(crate) struct TextCoverage {
    /// Left edge of the coverage box, in the same pixel space as the text origin.
    pub(crate) x: i32,
    /// Top edge of the coverage box.
    pub(crate) y: i32,
    /// Box width in pixels.
    pub(crate) width: u32,
    /// Box height in pixels.
    pub(crate) height: u32,
    /// One display-encoded coverage byte per pixel, row-major.
    pub(crate) bytes: Arc<[u8]>,
}

/// One cache entry: the ink box relative to the adjusted origin, so one entry serves every
/// alignment and every placement of the same `(text, font, scale)`.
struct CachedCoverage {
    epoch: u64,
    key: (u64, u64),
    /// The run's rounded total advance. Kept with the entry so a hit can resolve the alignment
    /// start without shaping the line again.
    advance: u32,
    left: i32,
    width: u32,
    height: u32,
    bytes: Arc<[u8]>,
}

/// Entries retained before the oldest is evicted. A window's labels number in the dozens, so this
/// holds the steady-state working set while staying bounded on a machine that streams new text.
const CACHE_LIMIT: usize = 128;

thread_local! {
    static COVERAGE_CACHE: RefCell<Vec<CachedCoverage>> = const { RefCell::new(Vec::new()) };
}

/// Builds the reference renderer's coverage for one `DrawText` command.
///
/// Returns `None` when the run cannot be represented as single-channel coverage: a colour glyph
/// carries its own RGB ink, and squeezing it into an alpha mask would paint the text colour over a
/// coloured emoji. The caller then keeps the frame on the reference rasteriser, which is the
/// honest answer rather than a silently wrong picture.
///
/// `scale` is device pixels per logical pixel, the same value the recording backend was constructed
/// with. `origin`, `alignment` and `font` are the command's own fields.
pub(crate) fn shaped_text_coverage(
    origin: Point,
    text: &str,
    font: &Font,
    alignment: HorizontalAlignment,
    scale: f32,
) -> Option<TextCoverage> {
    let line = TextMetrics::for_font(font, scale);
    if line.height == 0 {
        return Some(TextCoverage {
            x: origin.x,
            y: origin.y,
            width: 0,
            height: 0,
            bytes: Arc::from([]),
        });
    }

    let epoch = crate::render::pipeline::text_metrics_epoch();
    let key = crate::render::pipeline::text_fingerprint(text, font, scale);
    let hit = COVERAGE_CACHE.with(|cache| {
        cache.borrow().iter().find(|entry| entry.epoch == epoch && entry.key == key).map(|entry| {
            (entry.advance, entry.left, entry.width, entry.height, Arc::clone(&entry.bytes))
        })
    });
    if let Some((advance, left, width, height, bytes)) = hit {
        let adjusted_origin_x = adjusted_origin_x(origin.x, advance, alignment);
        return Some(TextCoverage {
            x: adjusted_origin_x.saturating_add(left),
            y: origin.y,
            width,
            height,
            bytes,
        });
    }

    let shaped = shape_line(text, font, scale);
    let width = shaped.advance().round() as u32;
    if width == 0 {
        return Some(TextCoverage {
            x: origin.x,
            y: origin.y,
            width: 0,
            height: 0,
            bytes: Arc::from([]),
        });
    }
    let metrics = TextMetrics { width, ..line };
    let adjusted_origin_x = adjusted_origin_x(origin.x, metrics.width, alignment);
    let (left, width, height, bytes) = rasterise_run(&shaped, &metrics, adjusted_origin_x, font)?;
    if width > 0 && height > 0 {
        COVERAGE_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if cache.len() >= CACHE_LIMIT {
                cache.remove(0);
            }
            cache.push(CachedCoverage {
                epoch,
                key,
                advance: metrics.width,
                left,
                width,
                height,
                bytes: Arc::clone(&bytes),
            });
        });
    }
    Some(TextCoverage {
        x: adjusted_origin_x.saturating_add(left),
        y: origin.y,
        width,
        height,
        bytes,
    })
}

/// The pen start for a run of `width` pixels whose anchor is `origin_x`.
fn adjusted_origin_x(origin_x: i32, width: u32, alignment: HorizontalAlignment) -> i32 {
    match alignment {
        HorizontalAlignment::Left => origin_x,
        HorizontalAlignment::Center => origin_x - (width / 2) as i32,
        HorizontalAlignment::Right => origin_x - width as i32,
    }
}

/// Rasterises every cluster of `shaped` into one coverage box.
///
/// Returns `(left, width, height, bytes)` where `left` is relative to `adjusted_origin_x`, or
/// `None` when a colour glyph appears. A run with no ink returns a zero-sized box.
fn rasterise_run(
    shaped: &crate::render::ShapedText,
    metrics: &TextMetrics,
    adjusted_origin_x: i32,
    font: &Font,
) -> Option<(i32, u32, u32, Arc<[u8]>)> {
    let cell_height = metrics.height.max(1) as i32;
    // First pass: the union of every cluster's cell. The pen advances by an `f32` and each glyph is
    // rounded, so a cell can start one pixel before the previous one ends; the union is what keeps
    // that rounding inside the uploaded texture.
    let mut pen_x = adjusted_origin_x as f32;
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut any_ink = false;
    for cluster in shaped.clusters() {
        let glyph_width = cluster.advance.max(1.0).round() as i32;
        let display = display_char(&cluster.text);
        if display.is_some_and(|ch| !ch.is_whitespace()) {
            any_ink = true;
            let x = pen_x.round() as i32;
            min_x = min_x.min(x);
            max_x = max_x.max(x.saturating_add(glyph_width));
        }
        pen_x += cluster.advance;
    }
    if !any_ink {
        return Some((0, 0, 0, Arc::from([])));
    }
    let box_width = (max_x - min_x).max(0) as u32;
    let box_height = cell_height.max(0) as u32;
    if box_width == 0 || box_height == 0 {
        return Some((min_x - adjusted_origin_x, 0, 0, Arc::from([])));
    }
    let mut bytes = vec![0u8; box_width as usize * box_height as usize];
    let mut coverage: Vec<u8> = Vec::new();

    // Second pass: paint each cluster into the union box. The cell, the pen position and the
    // italic shear are the ones `blend_coverage_bitmap` applies; only the destination differs.
    let italic = font.is_italic();
    let mut pen_x = adjusted_origin_x as f32;
    for cluster in shaped.clusters() {
        let glyph_width = cluster.advance.max(1.0).round() as i32;
        let glyph_height = cell_height.max(1);
        if let Some(ch) = display_char(&cluster.text) {
            if !ch.is_whitespace() && glyph_width > 0 {
                let cell = Cell::new(glyph_width as u32, glyph_height as u32);
                let area = cell.area();
                if coverage.len() < area {
                    coverage.resize(area, 0);
                }
                coverage[..area].fill(0);
                #[cfg(feature = "text-shaping")]
                let painted = crate::render::text::paint_active_at_weight(
                    ch,
                    cell,
                    &mut coverage,
                    font.weight(),
                );
                #[cfg(not(feature = "text-shaping"))]
                let painted = paint_active(ch, cell, &mut coverage);
                if let Some(painted) = painted {
                    if painted.is_color() {
                        // Colour ink has no single-channel form.
                        return None;
                    }
                    let x = pen_x.round() as i32;
                    for py in 0..glyph_height {
                        let shift = if italic {
                            (((glyph_height as f32 * 0.5) - py as f32) * 0.2).round() as i32
                        } else {
                            0
                        };
                        for px in 0..glyph_width {
                            let source = px - shift;
                            if source < 0 || source >= glyph_width {
                                continue;
                            }
                            let value = coverage[(py * glyph_width + source) as usize];
                            if value == 0 {
                                continue;
                            }
                            let target_x = x + px - min_x;
                            if target_x < 0 || target_x >= box_width as i32 {
                                continue;
                            }
                            let index = py as usize * box_width as usize + target_x as usize;
                            bytes[index] = crate::render::pipeline::glyph_coverage_byte(value);
                        }
                    }
                }
            }
        }
        pen_x += cluster.advance;
    }
    Some((min_x - adjusted_origin_x, box_width, box_height, Arc::from(bytes)))
}

/// The scalar a cluster paints, skipping the marks that have no glyph of their own.
fn display_char(text: &str) -> Option<char> {
    text.chars().find(|ch| {
        !crate::render::text::is_combining_mark(*ch)
            && !crate::render::text::is_variation_selector(*ch)
    })
}

#[cfg(test)]
mod tests {
    use super::shaped_text_coverage;
    use crate::core::{Font, HorizontalAlignment, Point};

    #[test]
    fn a_label_produces_coverage_at_the_line_box_size() {
        let font = Font::default();
        let coverage =
            shaped_text_coverage(Point::new(4, 6), "Hi", &font, HorizontalAlignment::Left, 1.0)
                .expect("the default face paints coverage, not colour");
        assert!(coverage.width > 0 && coverage.height > 0);
        assert_eq!(coverage.bytes.len(), coverage.width as usize * coverage.height as usize);
        assert_eq!(coverage.y, 6);
        assert!(coverage.bytes.iter().any(|byte| *byte > 0), "the run must carry ink");
    }

    #[test]
    fn alignment_moves_the_start_but_not_the_ink() {
        let font = Font::default();
        let left = shaped_text_coverage(
            Point::new(20, 4),
            "cancel",
            &font,
            HorizontalAlignment::Left,
            1.0,
        )
        .expect("coverage");
        let right = shaped_text_coverage(
            Point::new(20, 4),
            "cancel",
            &font,
            HorizontalAlignment::Right,
            1.0,
        )
        .expect("coverage");
        let centered = shaped_text_coverage(
            Point::new(20, 4),
            "cancel",
            &font,
            HorizontalAlignment::Center,
            1.0,
        )
        .expect("coverage");
        assert!(left.x > right.x, "right alignment starts further left");
        assert!(centered.x < left.x && centered.x > right.x);
        assert_eq!(left.bytes, right.bytes, "the ink itself does not depend on alignment");
    }

    #[test]
    fn whitespace_paints_nothing() {
        let font = Font::default();
        let coverage =
            shaped_text_coverage(Point::new(0, 0), "   ", &font, HorizontalAlignment::Left, 1.0)
                .expect("no colour ink in whitespace");
        assert_eq!(coverage.width, 0);
        assert!(coverage.bytes.is_empty());
    }

    #[test]
    fn a_repeated_label_shares_the_cached_ink() {
        let font = Font::default();
        let first =
            shaped_text_coverage(Point::new(4, 4), "cached", &font, HorizontalAlignment::Left, 1.0)
                .expect("coverage");
        let second = shaped_text_coverage(
            Point::new(40, 30),
            "cached",
            &font,
            HorizontalAlignment::Left,
            1.0,
        )
        .expect("coverage");
        assert_eq!(first.x, 4);
        assert_eq!(second.x, 40);
        assert_eq!(second.y, 30);
        assert!(
            crate::compat::Arc::ptr_eq(&first.bytes, &second.bytes),
            "a cache hit must share the bytes instead of rasterising them again"
        );
    }
}
