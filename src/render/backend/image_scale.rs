// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The software backend's resampling blit: the body of `PaintBackend::draw_image_scaled` and of the
//! `RenderCommand::DrawImageScaled` routing behind it.
//!
//! # Why these are free functions rather than method bodies
//!
//! Two places in `paint.rs` need the same geometry: the borrowed fast path (a widget drawing
//! straight into the software backend) and the command route (a recorder, a region bucket or a
//! batch replay handing the same command over). Both are *about* image scaling rather than about
//! the trait, so they live here and each call site keeps a one-line delegation. The split follows
//! the subject, not a line count.
//!
//! # Which backends resample where
//!
//! * The **software** backend samples during the blit: the image primitive already performs
//!   bilinear sampling from a source grid into a destination quadrilateral, so no temporary buffer
//!   is allocated — an animated frame costs no per-frame copy.
//! * Every **other** backend consumes [`RenderCommand::DrawImageScaled`], which carries the source
//!   grid with the pixels. A deferred backend keeps the command and decides: the SVG writer embeds
//!   the raster at its own extent inside a destination-sized `<image>`, and the software surface
//!   behind a batch replay samples it exactly as the fast path does.
//!
//! Both paths delegate the identity case (source and destination the same size) to a plain blit,
//! so an image that already matches its rectangle is byte-identical to what it painted before
//! scaling existed.

use crate::core::Point;

/// Software implementation: sample the source grid during the blit, no temporary buffer.
///
/// `source_width`/`source_height` describe the grid `data` was laid out in; the destination is the
/// rectangle at `(x, y)` with `width` x `height`. Degenerate extents and the identity case are
/// handled before any sampling happens.
pub(crate) fn blit_scaled_software(
    surface: &mut crate::render::SoftwareSurface,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    source_width: u32,
    source_height: u32,
    data: &[u8],
) {
    if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
        return;
    }
    if width == source_width && height == source_height {
        surface.draw_image(x, y, width, height, data);
        return;
    }
    let right = x.saturating_add(width as i32);
    let bottom = y.saturating_add(height as i32);
    // Perimeter order: top-left, top-right, bottom-right, bottom-left.
    let corners =
        [Point::new(x, y), Point::new(right, y), Point::new(right, bottom), Point::new(x, bottom)];
    surface.draw_image_quad(corners, source_width, source_height, data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Size};

    /// Source and destination the same size is the identity: the bytes are blitted untouched.
    #[test]
    fn the_identity_case_delegates_to_a_plain_blit() {
        let source = [10u8, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 1, 2, 3, 255];

        let mut plain = crate::render::SoftwareSurface::new(Size::new(2, 2), 1.0);
        plain.begin_frame(Color::BLACK);
        plain.draw_image(0, 0, 2, 2, &source);
        plain.end_frame();

        let mut scaled = crate::render::SoftwareSurface::new(Size::new(2, 2), 1.0);
        scaled.begin_frame(Color::BLACK);
        blit_scaled_software(&mut scaled, 0, 0, 2, 2, 2, 2, &source);
        scaled.end_frame();

        assert_eq!(plain.frame_rgba(), scaled.frame_rgba(), "the identity blit must not resample");
    }

    /// Degenerate extents draw nothing at all, rather than a zero-sized blit.
    #[test]
    fn degenerate_extents_are_ignored() {
        let mut surface = crate::render::SoftwareSurface::new(Size::new(2, 2), 1.0);
        surface.begin_frame(Color::WHITE);
        blit_scaled_software(&mut surface, 0, 0, 0, 2, 2, 2, &[255; 16]);
        blit_scaled_software(&mut surface, 0, 0, 2, 2, 0, 0, &[]);
        surface.end_frame();
        assert!(
            surface.frame_rgba().as_chunks::<4>().0.iter().all(|p| p[..3] == [255, 255, 255]),
            "nothing was drawn, so the frame is the clear colour"
        );
    }
}
