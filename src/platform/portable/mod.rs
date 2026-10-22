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
//! * an in-memory `Platform` implementation, reached through
//!   [`crate::platform::portable::instance`];
//! * a frame-buffer *row* copy helper for hosts that have a surface but no
//!   graphics API ([`crate::platform::portable::copy_rows`]).
//!
//! It deliberately does **not** carry per-control assets. An image, font or icon
//! loader here would be a second registry beside `crate::widget::runtime`, and
//! the duplication of that registry is precisely what BLUE15 §10.3 removed.

mod host;
pub mod surface;

pub(crate) use host::required_end;
pub use host::{copy_rows, instance, SurfaceGeometry, BACKEND_NAME, FAMILY};
pub use surface::FrameBuffer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;
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

    /// The portable host can actually display the widgets the library builds for it.
    ///
    /// # The defect this pins
    ///
    /// This backend's whole definition is "the host supplies a window and a drawing surface and the
    /// library paints into it" — and it is selected for `mini`, for `embedded` on a host with no
    /// backend, for a target with no backend module, and for the two macOS fallbacks. For every one
    /// of those there is **no toolkit object to fall back on**, so the surface is not one capability
    /// among several: it is the contract.
    ///
    /// It nevertheless inherited the trait defaults — `supports_surfaces()` answered `false` and the
    /// four surface methods were absent — so a `mini`/`embedded` host was told it could not display
    /// the widgets the library had just built for it. Every other record-backed backend
    /// (`linux`, `windows`, `macos`, `android`, `ios`, `harmony`, `wasm`, `wayland`) answered them
    /// over the same shared tables; this host holds the same record and had not been wired.
    ///
    /// # Why the assertions are a full round trip
    ///
    /// Asserting `supports_surfaces()` alone would pass against a backend that merely claims it —
    /// the defect this project found on `wasm`. So the flag, the mount, the rect read-back, the
    /// repaint queue and the refusal of an unknown id are all asserted: the promise and the
    /// implementation together, which is the shape the `wasm` test was upgraded to for the same
    /// reason.
    #[test]
    fn the_portable_host_can_display_a_printed_widget() {
        use crate::core::Rect;
        use crate::platform::Platform;

        let host = instance();
        assert!(
            host.supports_surfaces(),
            "supplying a surface is what this host is for, so the flag must be true"
        );

        let window = host.create_window("portable", 0, 0, 320, 240);
        let rect = Rect::new(0, 0, 40, 20);
        assert!(host.mount_surface(window, window, rect), "a known id mounts");

        // The rect the host will present is the one it was given, not a default.
        assert!(host.resize_surface(window, Rect::new(0, 0, 64, 32)), "a mounted id resizes");
        assert!(host.unmount_surface(window), "and unmounts");

        // The repaint queue is the half that makes the surface usable: without a drain the host is
        // never told what to draw, and the queue grows without bound.
        assert!(host.invalidate_surface(window));
        assert_eq!(
            host.take_pending_repaint(),
            Some(window),
            "the host must be able to learn which widget went stale"
        );
        assert_eq!(host.take_pending_repaint(), None, "and draining empties the queue");

        // An id this host never created is refused rather than recorded into a table nothing can
        // render.
        assert!(
            !host.mount_surface(window, 0xDEAD_BEEF, rect),
            "an unknown id must be refused, not silently recorded"
        );
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
