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
    /// The computation saturates rather than wrapping, so a rectangle that
    /// would overflow `i32` clamps to `i32::MAX` instead of producing a
    /// negative edge. The right edge is exclusive: a rectangle at `x = 0` with
    /// `width = 1` covers column `0` only.
    pub fn right(self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }
    /// Returns the exclusive bottom edge, i.e. `y + height`.
    ///
    /// Saturating, like [`PixelRect::right`]. The bottom edge is exclusive: a
    /// rectangle at `y = 0` with `height = 1` covers row `0` only.
    pub fn bottom(self) -> i32 {
        self.y.saturating_add(self.height as i32)
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
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        })
    }
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
