// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reusable frame buffer for the portable host surface.
//!
//! [`FrameBuffer::resize`] only allocates when the requested geometry is larger
//! than the current one, so a control that repaints at a steady size allocates
//! **nothing** per frame. The previous behaviour ("a fresh `Vec` per frame") cost
//! roughly `width * height * 4` bytes per painted control per frame — about 480
//! MB/s at 168 controls, 1000×1000 and 60 Hz — which presents as stutter and is
//! easy to misread as "the painting approach is too slow" (BLUE15 §10.4/R-3).

use crate::platform::portable::{required_end, SurfaceGeometry};

/// A frame buffer that grows but never shrinks.
///
/// Shrinking would release the memory a later, larger frame has to ask for again;
/// the high-water mark of a widget surface is stable in practice, so keeping it is
/// the cheaper trade. Content beyond the current geometry is stale but never
/// exposed: [`Self::frame_mut`] hands out exactly the current region.
pub struct FrameBuffer {
    pixels: Vec<u8>,
    geometry: SurfaceGeometry,
}

impl FrameBuffer {
    /// Creates an empty buffer; nothing is allocated until the first resize.
    pub fn new() -> Self {
        Self { pixels: Vec::new(), geometry: SurfaceGeometry::tight(0, 0) }
    }

    /// Ensures the buffer can hold `geometry`, allocating only if it grew.
    ///
    /// Returns `true` when the geometry is usable. A stride narrower than one
    /// tightly packed row cannot describe a frame, so it is refused and the
    /// previous geometry is kept.
    pub fn resize(&mut self, geometry: SurfaceGeometry) -> bool {
        if geometry.stride < geometry.width as usize * 4 {
            return false;
        }
        let Some(needed) = required_bytes(geometry) else {
            return false;
        };
        if self.pixels.len() < needed {
            self.pixels.resize(needed, 0);
        }
        self.geometry = geometry;
        true
    }

    /// The geometry the buffer is currently presenting.
    pub const fn geometry(&self) -> SurfaceGeometry {
        self.geometry
    }

    /// Bytes this buffer has allocated. Diagnostics and tests read this to prove
    /// that repeated frames of the same size do not allocate.
    pub fn allocated_bytes(&self) -> usize {
        self.pixels.len()
    }

    /// The current frame, tightly described by [`Self::geometry`].
    pub fn frame(&self) -> &[u8] {
        let end = self.visible_bytes();
        &self.pixels[..end]
    }

    /// The current frame, mutable. Handed to the painting backend.
    pub fn frame_mut(&mut self) -> &mut [u8] {
        let end = self.visible_bytes();
        &mut self.pixels[..end]
    }

    /// Clears the current frame to transparent black.
    pub fn clear(&mut self) {
        let end = self.visible_bytes();
        self.pixels[..end].fill(0);
    }

    /// Bytes this buffer was asked to hold for the current geometry.
    ///
    /// This is the last row's end, not `stride * height`: a host with a padded
    /// stride does not need the padding after its final row, and counting it
    /// would report an allocation larger than the buffer's real demand.
    fn visible_bytes(&self) -> usize {
        required_bytes(self.geometry).unwrap_or(0)
    }
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Bytes needed to hold `geometry`, or `None` if it cannot be expressed.
fn required_bytes(geometry: SurfaceGeometry) -> Option<usize> {
    if geometry.width == 0 {
        return Some(0);
    }
    required_end(geometry.stride, geometry.height as usize, geometry.width as usize * 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_then_frame_describes_the_geometry() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry::tight(4, 3)));
        assert_eq!(buffer.frame().len(), 4 * 3 * 4);
        assert_eq!(buffer.geometry(), SurfaceGeometry::tight(4, 3));
    }

    #[test]
    fn clear_resets_every_visible_pixel() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry::tight(2, 2)));
        buffer.frame_mut().fill(0xff);
        buffer.clear();
        assert!(buffer.frame().iter().all(|byte| *byte == 0));
    }

    /// The acceptance criterion for BLUE15 R-3: allocation does not depend on how
    /// many frames are painted, only on the largest geometry ever requested.
    #[test]
    fn repeated_frames_of_the_same_size_do_not_allocate() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry::tight(64, 64)));
        let after_first = buffer.allocated_bytes();
        for _ in 0..1000 {
            buffer.clear();
            assert!(buffer.resize(SurfaceGeometry::tight(64, 64)));
        }
        assert_eq!(buffer.allocated_bytes(), after_first);
    }

    /// A padded stride must not need the padding after its last row.
    #[test]
    fn padded_stride_does_not_allocate_trailing_padding() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry { width: 2, height: 2, stride: 12 }));
        assert_eq!(buffer.frame().len(), 12 + 8);
    }

    #[test]
    fn shrinking_geometry_keeps_the_allocation_but_not_the_frame() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry::tight(8, 8)));
        let allocated = buffer.allocated_bytes();
        assert!(buffer.resize(SurfaceGeometry::tight(2, 2)));
        assert_eq!(buffer.allocated_bytes(), allocated, "the high-water mark is kept");
        assert_eq!(buffer.frame().len(), 2 * 2 * 4, "only the current region is exposed");
    }

    #[test]
    fn a_stride_narrower_than_a_row_is_refused_and_changes_nothing() {
        let mut buffer = FrameBuffer::new();
        assert!(buffer.resize(SurfaceGeometry::tight(4, 4)));
        assert!(!buffer.resize(SurfaceGeometry { width: 4, height: 4, stride: 8 }));
        assert_eq!(buffer.geometry(), SurfaceGeometry::tight(4, 4));
    }
}
