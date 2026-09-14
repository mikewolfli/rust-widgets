// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The portable host backend: a drawing surface with no operating system behind it.
//!
//! # Which builds use it
//!
//! | Build | Why it is the right host |
//! |---|---|
//! | `mini` | The profile's whole point is to omit OS integration and run alloc-frugally. |
//! | `embedded` on a host without a backend | There is an OS window, but no OS *control* to map onto. |
//! | a target with no backend module | The honest fallback: a surface and nothing more. |
//!
//! These three are one fact, not three: **the host supplies a window and a
//! drawing surface, and the library paints every control**. Encoding them as one
//! backend is what lets `mini` and `embedded` differ only in a memory policy
//! (see [`crate::platform::profile`]) instead of in their control semantics.
//!
//! # Why it is thin
//!
//! It contributes only what no other layer can supply for itself:
//!
//! * an in-memory `Platform` implementation, reached through [`Self::instance`];
//! * a frame-buffer *row* copy helper for hosts that have a surface but no
//!   graphics API ([`copy_rows`]).
//!
//! It deliberately does **not** carry per-control assets. An image, font or icon
//! loader here would be a second registry beside [`crate::widget::runtime`], and
//! the duplication of that registry is precisely what BLUE15 §10.3 removed.

pub mod surface;

pub use surface::FrameBuffer;

use crate::core::PlatformFamily;
use crate::platform::stub::StubPlatform;

/// The surface a portable host exposes: a window plus a pixel buffer.
///
/// `None` for either dimension means the host has no surface at all, which is
/// the truthful answer before a window exists and in a profile that never
/// creates one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceGeometry {
    /// Width of the drawable area, in physical pixels.
    pub width: u32,
    /// Height of the drawable area, in physical pixels.
    pub height: u32,
    /// Bytes between the start of two consecutive rows.
    ///
    /// Kept separate from `width * 4` because a GPU backbuffer is often padded to
    /// an alignment, and a row copy that assumed tight packing would shear the
    /// image on exactly those hosts.
    pub stride: usize,
}

impl SurfaceGeometry {
    /// Creates a tightly packed surface geometry.
    pub const fn tight(width: u32, height: u32) -> Self {
        Self { width, height, stride: width as usize * 4 }
    }
}

/// Copies `src` into the sub-rectangle of `dst` described by `geometry`.
///
/// This is the portable host's one real service. A host with a frame buffer but a
/// different byte layout (padding, a top-down vs bottom-up origin) cannot hand the
/// buffer to the widget layer directly; it reshapes it here instead of the widget
/// layer learning each host's layout.
///
/// The copy is bounds-checked in both directions:
///
/// * a region that does not fit `src` or `dst` is refused rather than clipped,
///   because a silently clipped frame is a rendering bug that hides itself;
/// * `geometry.stride` must be at least one tightly packed row, otherwise the rows
///   would overlap and the result would depend on copy order.
///
/// Returns `false` when nothing was copied.
pub fn copy_rows(src: &[u8], dst: &mut [u8], geometry: SurfaceGeometry) -> bool {
    let (width, height, stride) =
        (geometry.width as usize, geometry.height as usize, geometry.stride);
    if width == 0 || height == 0 {
        // A zero-sized surface has no pixels to copy; that is a valid no-op
        // request, not a failure of the caller's data.
        return true;
    }
    let row_bytes = width * 4;
    if stride < row_bytes {
        return false;
    }
    let Some(needed) = required_end(stride, height, row_bytes) else {
        return false;
    };
    if src.len() < needed || dst.len() < needed {
        return false;
    }
    for row in 0..height {
        let start = row * stride;
        dst[start..start + row_bytes].copy_from_slice(&src[start..start + row_bytes]);
    }
    true
}

/// The byte index just past the last visible row of a `stride * height` frame.
///
/// Shared with [`FrameBuffer`](crate::platform::portable::FrameBuffer) so that
/// "what counts as the end of a frame" has one definition rather than two that
/// can disagree about whether a padded stride's final row counts.
pub(crate) fn required_end(stride: usize, height: usize, row_bytes: usize) -> Option<usize> {
    if height == 0 {
        return Some(0);
    }
    stride.checked_mul(height - 1)?.checked_add(row_bytes)
}

/// Returns the portable host's `Platform` implementation.
///
/// The value is a `StubPlatform` because that type already supplies the whole
/// contract for a host with no OS: an in-memory `BackendState` that answers text,
/// geometry, enabled/visible and the numeric control properties for any id the
/// library mounts, plus an IME bridge and drag-and-drop queues for tests. Sharing
/// it keeps one implementation of "a host with no OS" instead of two that must
/// agree.
pub fn instance() -> &'static StubPlatform {
    crate::platform::stub::stub_platform_singleton()
}

/// The platform family a portable host reports.
///
/// `Embedded` is the accurate word: there is no windowing system, no window
/// manager and no control toolkit. Reporting `Desktop` would make adaptive code
/// pick desktop defaults for a host that cannot honour them.
pub const FAMILY: PlatformFamily = PlatformFamily::Embedded;

/// The backend name a portable host reports.
pub const BACKEND_NAME: &str = "portable";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::types::Platform;

    #[test]
    fn tight_geometry_packs_rows() {
        assert_eq!(SurfaceGeometry::tight(0, 0).stride, 0);
        assert_eq!(SurfaceGeometry::tight(64, 48).stride, 64 * 4);
        assert_eq!(SurfaceGeometry::tight(64, 48).width, 64);
        assert_eq!(SurfaceGeometry::tight(64, 48).height, 48);
    }

    #[test]
    fn instance_reports_the_portable_identity() {
        let host = instance();
        assert_eq!(host.backend_name(), BACKEND_NAME);
        assert_eq!(host.family(), FAMILY);
    }

    #[test]
    fn copy_rows_moves_a_tightly_packed_frame() {
        let geometry = SurfaceGeometry::tight(2, 2);
        let src: Vec<u8> = (0..16).collect();
        let mut dst = vec![0u8; 16];
        assert!(copy_rows(&src, &mut dst, geometry));
        assert_eq!(dst, src);
    }

    #[test]
    fn copy_rows_honours_a_padded_stride() {
        // Two rows of 2 pixels (8 bytes) with 4 bytes of padding between them.
        let geometry = SurfaceGeometry { width: 2, height: 2, stride: 12 };
        let src = vec![1u8; 12 * 2];
        let mut dst = vec![0u8; 12 * 2];
        assert!(copy_rows(&src, &mut dst, geometry));
        assert_eq!(dst[..8], [1u8; 8]);
        assert_eq!(dst[8..12], [0u8; 4], "padding must not be written");
        assert_eq!(dst[16..20], [1u8; 4]);
    }

    #[test]
    fn copy_rows_refuses_a_region_that_does_not_fit() {
        let geometry = SurfaceGeometry::tight(4, 4);
        let src = vec![0u8; 16];
        let mut dst = vec![0u8; 64];
        assert!(!copy_rows(&src, &mut dst, geometry), "short source must be refused");
        let src = vec![0u8; 64];
        let mut dst = vec![0u8; 16];
        assert!(!copy_rows(&src, &mut dst, geometry), "short destination must be refused");
    }

    #[test]
    fn copy_rows_refuses_a_stride_narrower_than_a_row() {
        let geometry = SurfaceGeometry { width: 4, height: 1, stride: 8 };
        let src = vec![0u8; 16];
        let mut dst = vec![0u8; 16];
        assert!(!copy_rows(&src, &mut dst, geometry), "overlapping rows must be refused");
    }

    #[test]
    fn copy_rows_treats_a_zero_sized_surface_as_a_no_op() {
        let geometry = SurfaceGeometry { width: 0, height: 8, stride: 0 };
        assert!(copy_rows(&[], &mut [], geometry));
        let geometry = SurfaceGeometry { width: 8, height: 0, stride: 32 };
        assert!(copy_rows(&[], &mut [], geometry));
    }
}
