// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What a sampled screen pixel means, independent of how it was read (SP-119).
//!
//! # Why the policy is not next to each binding
//!
//! Two macOS backends read pixels through different bindings: `platform/macos/cg.rs` hand-declares
//! the CoreGraphics calls the canvas already uses, and `platform/macos_objc2/native.rs` goes
//! through the `objc2-core-graphics` bindings. A third platform will add a third way. The *rule*
//! for turning the four bytes into an answer is the same fact for all of them, so it is written
//! once here rather than restated — and silently re-derived — beside each binding (principle #54).
//!
//! # The rule
//!
//! A composited screen pixel is opaque. An alpha of zero does not mean "a transparent pixel", it
//! means the read produced a cleared buffer: CoreGraphics answers a rectangle outside every display
//! with a transparent 1×1 image rather than null. (Measured on macOS 15:
//! `CGWindowListCreateImage` at `(-100_000, -100_000)` returns a cleared image.) Reporting
//! `rgba(0, 0, 0, 0)` from that would hand a picker a colour no pixel on screen has, which is
//! exactly what [`Platform::sample_screen_color`](crate::platform::Platform::sample_screen_color)'s
//! `None` exists to avoid.

/// A sampled pixel, or `None` when the read produced no pixel at all.
///
/// `None` covers a point outside every display, a point on a window this process may not read, and
/// a host where the capture grant is missing — the caller's answer is the same for all three: fall
/// back to sampling its own painted surface.
pub(crate) fn presentable(pixel: (u8, u8, u8, u8)) -> Option<(u8, u8, u8, u8)> {
    (pixel.3 != 0).then_some(pixel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An opaque sample is a real pixel and is passed through unchanged.
    #[test]
    fn an_opaque_sample_is_presented_as_read() {
        assert_eq!(presentable((12, 34, 56, 255)), Some((12, 34, 56, 255)));
    }

    /// A cleared sample is an absence, not a transparent black pixel.
    #[test]
    fn a_cleared_sample_is_an_absence() {
        assert_eq!(presentable((0, 0, 0, 0)), None);
        // Any colour at alpha zero is still a buffer that was cleared rather than read.
        assert_eq!(presentable((255, 128, 64, 0)), None);
    }
}
