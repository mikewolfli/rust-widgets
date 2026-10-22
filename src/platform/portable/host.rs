// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The portable host's surface geometry, row copy and `Platform` instance.
//!
//! Kept out of `mod.rs` (principle #8). Everything here is the host's entire service:
//! reshape a caller's frame buffer into the library's layout, and answer the `Platform`
//! contract with a stub.

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
    ///
    /// The stride is computed with checked arithmetic and **saturates** to `usize::MAX` on
    /// overflow. A `width` whose `width * 4` cannot be represented (possible on a 32-bit target
    /// for a width at or above `1 << 30`) would otherwise wrap to a small stride *here*, before
    /// [`copy_rows`] or `FrameBuffer::resize` ever see the value — the very overflow those
    /// checked paths exist to refuse. Saturating keeps a wrapped, too-small stride from being handed
    /// on: a saturated stride is >= any real row length, so downstream `required_end` arithmetic
    /// overflows and the geometry is rejected rather than silently mis-described. This has to stay a
    /// `const fn` because callers build geometries in `const` contexts.
    pub const fn tight(width: u32, height: u32) -> Self {
        let stride = match (width as usize).checked_mul(4) {
            Some(bytes) => bytes,
            None => usize::MAX,
        };
        Self { width, height, stride }
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
    // `width * 4` on 32-bit `usize` overflows for a width at or above `1 << 30`. That overflow
    // happens *before* `required_end`'s checked arithmetic, so the later check cannot repair it: in
    // debug the multiply panics, in release it wraps to a small (or zero) `row_bytes`, and a stride
    // that is actually far too narrow passes the `stride < row_bytes` test. Computing the row length
    // with `checked_mul` and refusing on overflow turns an illegal size into an explicit failure.
    let Some(row_bytes) = width.checked_mul(4) else {
        return false;
    };
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
