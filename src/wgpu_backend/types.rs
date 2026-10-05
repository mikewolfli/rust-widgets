// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core types for WGPU backend.
/// Integer rectangle in pixel space.
///
/// The origin is the framebuffer's top-left corner and the y axis grows
/// downwards, matching the coordinate space of `crate::core::Rect` and of the
/// rasterizer's scanline loops. `width` and `height` are unsigned while `x` and
/// `y` are signed, because a rectangle may legitimately start off-screen;
/// callers that need the *stored* extent beyond the visible area should note
/// that a clipped rectangle is re-derived from its edges rather than merely
/// truncated, so `width`/`height` always agree with `right`/`bottom`.
///
/// This type has no GPU representation. The wgpu vertex and uniform data used
/// by the renderer is built from plain `f32` arrays, so no `#[repr(C)]`
/// guarantee, padding or alignment constraint applies here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    /// Left edge in physical pixels. May be negative; the rasterizer clips
    /// negative columns away rather than wrapping them.
    pub x: i32,
    /// Top edge in physical pixels. May be negative, with the same clipping
    /// behaviour as `x`.
    pub y: i32,
    /// Width in pixels. Zero-sized rectangles are skipped by every draw
    /// command, and a rectangle with a zero dimension never intersects
    /// anything, including itself.
    pub width: u32,
    /// Height in pixels. Zero-sized rectangles are skipped by every draw
    /// command.
    pub height: u32,
}
impl PixelRect {
    /// Returns the exclusive right edge, i.e. `x + width`.
    ///
    /// The computation is done in `i64` and saturates to `i32::MAX`/`i32::MIN`
    /// rather than wrapping. The old form did `self.width as i32` first, so a
    /// width of `u32::MAX` became `-1` and `x + width` produced a *left-shifted*
    /// right edge (for `x = 0`, `right() == -1`), which broke every clip and
    /// bounding-box computation that used it (N-S-67).
    pub fn right(self) -> i32 {
        saturating_i32(self.x as i64 + self.width as i64)
    }
    /// Returns the exclusive bottom edge, i.e. `y + height`.
    ///
    /// Widened to `i64` like [`PixelRect::right`], so a `u32::MAX` height clamps
    /// to `i32::MAX` instead of wrapping (N-S-67).
    pub fn bottom(self) -> i32 {
        saturating_i32(self.y as i64 + self.height as i64)
    }
    /// Returns a copy of this rectangle moved by `offset`.
    ///
    /// Saturating, like [`PixelRect::right`]: a translation that would overflow
    /// clamps at the `i32` bounds rather than wrapping to a negative coordinate.
    /// Used by the rasteriser to apply the command stream's accumulated
    /// `Translate` offset to both geometry and clip rectangles.
    pub fn translate(self, offset: (i32, i32)) -> PixelRect {
        PixelRect {
            x: self.x.saturating_add(offset.0),
            y: self.y.saturating_add(offset.1),
            width: self.width,
            height: self.height,
        }
    }
    /// Returns the overlapping region of two rectangles, or `None` when they do
    /// not overlap.
    ///
    /// The result is the intersection of the two rectangles' half-open extents,
    /// so it is at most as large as either input and is `None` whenever the
    /// overlap is empty or degenerate — note that touching edges produce **no**
    /// overlap, because the test is `right <= left || bottom <= top` on the
    /// exclusive edges.
    ///
    /// A rectangle whose stored `width` is `0` can still report a non-empty
    /// overlap in the other axis, but the result's `width` is derived from the
    /// clamped edges, so the returned rectangle is drawn with the intersected
    /// extent rather than the original.
    pub fn intersect(self, other: PixelRect) -> Option<PixelRect> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= left || bottom <= top {
            return None;
        }
        Some(PixelRect {
            x: left,
            y: top,
            width: (right as i64 - left as i64) as u32,
            height: (bottom as i64 - top as i64) as u32,
        })
    }
}

/// Clamps an `i64` coordinate into the `i32` pixel space.
///
/// `PixelRect` stores coordinates as `i32` but computes edges in `i64` so a
/// `u32` width/height cannot wrap into a negative value. This is the single place
/// the widened value is narrowed back, saturating at the `i32` bounds.
fn saturating_i32(value: i64) -> i32 {
    value.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}
/// 8-bit RGBA color.
///
/// Channels are straight (non-premultiplied) and sRGB-encoded, i.e. the values
/// are the on-screen bytes rather than linear-light intensities. `a` is
/// opacity, not coverage: the rasterizer's draw commands write all four
/// channels directly without source-over blending, so a pixel is only actually
/// transparent when a command wrote `0` into it — a drawn `a == 0` still
/// overwrites the destination with an opaque black byte pattern.
///
/// The layout is the same as `crate::core::Color` and as the framebuffer bytes
/// returned by `crate::widget::runtime::render_frame`, which makes this the
/// hand-off type between the paint pipeline and this backend.
///
/// `Rgba8` has no GPU representation: the wgpu uniform buffers are written as
/// four `f32` values in `0.0..=1.0`, produced by dividing each channel by 255.
/// The conversion is not lossless in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    /// Red channel, `0`..=`255`, sRGB-encoded.
    pub r: u8,
    /// Green channel, `0`..=`255`, sRGB-encoded.
    pub g: u8,
    /// Blue channel, `0`..=`255`, sRGB-encoded.
    pub b: u8,
    /// Alpha (opacity), `0`..=`255`. Straight, not premultiplied, so it is
    /// independent of the other three channels.
    pub a: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// N-S-67: `right`/`bottom` must widen before adding, so a `u32::MAX` extent
    /// clamps to `i32::MAX` instead of wrapping through `u32 as i32` to `-1`.
    #[test]
    fn right_and_bottom_widen_before_adding() {
        let rect = PixelRect { x: 0, y: 0, width: u32::MAX, height: u32::MAX };
        assert_eq!(rect.right(), i32::MAX, "width u32::MAX must clamp, not wrap to -1");
        assert_eq!(rect.bottom(), i32::MAX, "height u32::MAX must clamp, not wrap to -1");

        // A negative origin with a huge width still takes the widened path: the
        // sum is far past i32::MAX and clamps.
        let rect = PixelRect { x: -10, y: -10, width: u32::MAX, height: u32::MAX };
        assert_eq!(rect.right(), i32::MAX);
        assert_eq!(rect.bottom(), i32::MAX);

        // Ordinary rectangles are unchanged.
        let rect = PixelRect { x: 3, y: 4, width: 5, height: 6 };
        assert_eq!((rect.right(), rect.bottom()), (8, 10));
    }

    /// The intersection must derive its extent from the clamped edges and must
    /// not overflow when an input width is `u32::MAX`.
    #[test]
    fn intersect_handles_huge_extents() {
        let huge = PixelRect { x: 0, y: 0, width: u32::MAX, height: u32::MAX };
        let small = PixelRect { x: 2, y: 2, width: 4, height: 4 };
        let overlap = huge.intersect(small).expect("small lies inside the huge rect");
        assert_eq!(overlap, small);
    }
}
