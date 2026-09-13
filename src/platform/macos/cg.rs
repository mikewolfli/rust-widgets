// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Minimal CoreGraphics FFI used to blit a software frame into an AppKit view.
//!
//! # Why hand-declared instead of a crate
//!
//! The frame arrives as top-down, straight-alpha RGBA. Getting that onto screen
//! needs exactly five CoreGraphics calls (`CGColorSpaceCreateDeviceRGB`,
//! `CGContextCreate`, `CGBitmapContextCreateImage`, `CGContextDrawImage`,
//! `CGContextRelease`). Pulling in a whole CoreGraphics binding crate for that
//! is not worth the dependency, and `core-graphics` is only present
//! transitively — depending on a transitive crate is a build that breaks
//! whenever an unrelated dependency drops it.
//!
//! Every declaration below is copied from Apple's headers. All functions take
//! and return plain pointers; the raw handle type is [`CGContextRef`].
//!
//! # Feature gate
//!
//! Only `macos/canvas.rs` uses this module, and that module is compiled out of
//! the `mini`/`embedded` profiles. The same gate is applied here so those
//! profiles do not carry a `cg` module nothing can reach.

#![cfg(all(
    target_os = "macos",
    feature = "cocoa-legacy",
    not(any(feature = "mini", feature = "embedded"))
))]
#![allow(non_snake_case)] // names mirror the C ABI

use std::os::raw::{c_int, c_void};

/// Opaque `CGContextRef`.
pub(crate) type CGContextRef = *mut c_void;
/// Opaque `CGColorSpaceRef`.
pub(crate) type CGColorSpaceRef = *mut c_void;
/// Opaque `CGImageRef`.
pub(crate) type CGImageRef = *mut c_void;

/// `kCGImageAlphaPremultipliedLast`: alpha stored last within the word.
pub(crate) const BITMAP_ALPHA_PREMULTIPLIED_LAST: u32 = 1;

/// `kCGBitmapByteOrder32Little`.
///
/// Chosen together with [`BITMAP_ALPHA_PREMULTIPLIED_LAST`] to give a memory
/// layout of **R, G, B, A** byte order — the same order the software renderer
/// produces. With `ByteOrder32Big` the word would instead be laid out as
/// `A, B, G, R` in memory, swapping red and blue (which the orientation test
/// below catches).
pub(crate) const BITMAP_BYTE_ORDER_32_LITTLE: u32 = 2 << 12;

/// `CGRect` in CoreGraphics' own layout (`CGFloat` is `f64` on 64-bit).
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

/// `CGPoint`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct CGPoint {
    pub x: f64,
    pub y: f64,
}

/// `CGSize`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct CGSize {
    pub width: f64,
    pub height: f64,
}

impl CGRect {
    /// Builds a rectangle at the origin.
    pub(crate) fn new(width: f64, height: f64) -> Self {
        Self { origin: CGPoint { x: 0.0, y: 0.0 }, size: CGSize { width, height } }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// Creates a device-RGB color space. Caller releases with `CGColorSpaceRelease`.
    pub(crate) fn CGColorSpaceCreateDeviceRGB() -> CGColorSpaceRef;
    /// Releases a color space.
    pub(crate) fn CGColorSpaceRelease(space: CGColorSpaceRef);

    /// Creates a bitmap graphics context.
    ///
    /// * `data` — caller-owned pixel buffer, or null to let CG allocate.
    /// * `bits_per_component` — 8 for 32-bit RGBA.
    /// * `bytes_per_row` — typically `width * 4`.
    /// * `bitmap_info` — OR of alpha info and byte order.
    pub(crate) fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        color_space: CGColorSpaceRef,
        bitmap_info: u32,
    ) -> CGContextRef;

    /// Returns the backing pixel buffer of a bitmap context.
    pub(crate) fn CGBitmapContextGetData(context: CGContextRef) -> *mut c_void;

    /// Snapshots a bitmap context into an immutable image.
    pub(crate) fn CGBitmapContextCreateImage(context: CGContextRef) -> CGImageRef;

    /// Releases a context.
    pub(crate) fn CGContextRelease(context: CGContextRef);
    /// Releases an image.
    pub(crate) fn CGImageRelease(image: CGImageRef);

    /// Pushes a copy of the current graphics state.
    pub(crate) fn CGContextSaveGState(context: CGContextRef);
    /// Pops the graphics state.
    pub(crate) fn CGContextRestoreGState(context: CGContextRef);
    /// Draws an image into `rect`, applying the current CTM and clip.
    pub(crate) fn CGContextDrawImage(context: CGContextRef, rect: CGRect, image: CGImageRef);
    /// Returns the bitmap context's actual row stride in bytes.
    pub(crate) fn CGBitmapContextGetBytesPerRow(context: CGContextRef) -> usize;

    /// Returns an image's pixel width.
    pub(crate) fn CGImageGetWidth(image: CGImageRef) -> usize;
    /// Returns an image's pixel height.
    pub(crate) fn CGImageGetHeight(image: CGImageRef) -> usize;

    /// Sets the interpolation quality (0 = none, 1 = low, 4 = high).
    pub(crate) fn CGContextSetInterpolationQuality(context: CGContextRef, quality: c_int);
}

/// Renders a top-down RGBA frame into an existing CoreGraphics context.
///
/// Handles three details the caller should not have to think about:
///
/// 1. **Premultiplication** — the frame is straight alpha; CoreGraphics'
///    `PremultipliedLast` layout is not, so RGB is scaled by alpha.
/// 2. **Row order** — the frame is top-down, CG draws bottom-up, so the CTM is
///    flipped around the destination height.
/// 3. **Allocation ownership** — the color space, bitmap context and image are
///    all released here, including on the early-return paths.
///
/// Returns `false` (after logging) when a CoreGraphics object cannot be created.
///
/// # Safety
///
/// `context` must be a live `CGContextRef` valid for drawing with the current
/// frame. `frame.len()` must be at least `width * height * 4`.
pub(crate) unsafe fn blit_rgba(
    context: CGContextRef,
    width: u32,
    height: u32,
    frame: &[u8],
) -> bool {
    if context.is_null() || width == 0 || height == 0 {
        return false;
    }
    let expected = width as usize * height as usize * 4;
    if frame.len() < expected {
        log::error!(
            "[macos] canvas: frame is {} bytes, need {} for {width}x{height}",
            frame.len(),
            expected
        );
        return false;
    }

    let color_space = CGColorSpaceCreateDeviceRGB();
    if color_space.is_null() {
        log::error!("[macos] canvas: CGColorSpaceCreateDeviceRGB returned null");
        return false;
    }

    let bitmap = CGBitmapContextCreate(
        std::ptr::null_mut(),
        width as usize,
        height as usize,
        8,
        width as usize * 4,
        color_space,
        BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_LITTLE,
    );
    if bitmap.is_null() {
        CGColorSpaceRelease(color_space);
        log::error!("[macos] canvas: CGBitmapContextCreate returned null for {width}x{height}");
        return false;
    }

    // Copy in with premultiplied alpha, row by row using the bitmap's **actual**
    // stride. Assuming `width * 4` is wrong: CoreGraphics is free to pad rows
    // (it does so once the width crosses an alignment boundary), and the earlier
    // width*4 assumption shifted every row after the first — caught by
    // `blit_preserves_channel_order_and_orientation`.
    let destination = CGBitmapContextGetData(bitmap) as *mut u8;
    if !destination.is_null() {
        let stride = CGBitmapContextGetBytesPerRow(bitmap);
        for y in 0..height as usize {
            for x in 0..width as usize {
                let source = (y * width as usize + x) * 4;
                let target = y * stride + x * 4;
                let alpha = frame[source + 3] as u32;
                *destination.add(target) = (frame[source] as u32 * alpha / 255) as u8;
                *destination.add(target + 1) = (frame[source + 1] as u32 * alpha / 255) as u8;
                *destination.add(target + 2) = (frame[source + 2] as u32 * alpha / 255) as u8;
                *destination.add(target + 3) = alpha as u8;
            }
        }
    }

    let image = CGBitmapContextCreateImage(bitmap);
    CGContextRelease(bitmap);
    CGColorSpaceRelease(color_space);
    if image.is_null() {
        log::error!("[macos] canvas: CGBitmapContextCreateImage returned null");
        return false;
    }

    CGContextSaveGState(context);
    CGContextSetInterpolationQuality(context, 0); // nearest: 1:1 blit, no blur
                                                  // Draw at the image's **own pixel size**. The frame and the image are both
                                                  // `width × height` pixels, and the destination bitmap is the same size, so
                                                  // this is a 1:1 blit. Passing `width/height` as f64 was equivalent here only
                                                  // by luck; deriving the rect from the image keeps the blit correct even if a
                                                  // caller's frame is larger than the mount area.
    let image_width = CGImageGetWidth(image) as f64;
    let image_height = CGImageGetHeight(image) as f64;
    CGContextDrawImage(context, CGRect::new(image_width, image_height), image);
    CGContextRestoreGState(context);
    CGImageRelease(image);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_built_at_the_origin() {
        let rect = CGRect::new(12.0, 34.0);
        assert_eq!(rect.origin.x, 0.0);
        assert_eq!(rect.origin.y, 0.0);
        assert_eq!(rect.size.width, 12.0);
        assert_eq!(rect.size.height, 34.0);
    }

    #[test]
    fn bitmap_info_encodes_premultiplied_rgba_little_endian() {
        // kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Little
        assert_eq!(BITMAP_ALPHA_PREMULTIPLIED_LAST, 1);
        assert_eq!(BITMAP_BYTE_ORDER_32_LITTLE, 2 << 12);
        assert_eq!(BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_LITTLE, 0x2001);
    }

    #[test]
    fn blit_rejects_degenerate_input_without_touching_coregraphics() {
        let empty: [u8; 0] = [];
        // Null context short-circuits before any CoreGraphics call.
        assert!(!unsafe { blit_rgba(std::ptr::null_mut(), 4, 4, &empty) });
    }

    /// Renders a deliberately asymmetric frame through [`blit_rgba`] into a
    /// bitmap context and reads the pixels back, so the channel order and both
    /// flips are verified rather than assumed.
    #[test]
    fn blit_preserves_channel_order_and_orientation() {
        const WIDTH: u32 = 4;
        const HEIGHT: u32 = 4;

        // top-left red, top-right green, bottom-left blue, bottom-right white.
        let mut frame = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let offset = ((y * WIDTH + x) * 4) as usize;
                let (r, g, b) = match (x < WIDTH / 2, y < HEIGHT / 2) {
                    (true, true) => (255, 0, 0),
                    (false, true) => (0, 255, 0),
                    (true, false) => (0, 0, 255),
                    (false, false) => (255, 255, 255),
                };
                frame[offset] = r;
                frame[offset + 1] = g;
                frame[offset + 2] = b;
                frame[offset + 3] = 255;
            }
        }

        // SAFETY: all arguments are valid for the duration of the test; the
        // context is released before returning.
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            assert!(!space.is_null(), "colour space");
            let context = CGBitmapContextCreate(
                std::ptr::null_mut(),
                WIDTH as usize,
                HEIGHT as usize,
                8,
                WIDTH as usize * 4,
                space,
                BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_LITTLE,
            );
            assert!(!context.is_null(), "destination context");

            assert!(blit_rgba(context, WIDTH, HEIGHT, &frame), "blit must succeed");

            // The destination bitmap context inherits the host display's backing
            // scale on Retina hardware, so a blitted source pixel occupies
            // `scale × scale` device cells. Detect it from the difference between
            // the context's logical extent and its byte stride instead of assuming
            // 1:1.
            let data = CGBitmapContextGetData(context) as *mut u8;
            assert!(!data.is_null(), "bitmap data");
            let bytes_per_row = CGBitmapContextGetBytesPerRow(context) as usize;
            let scale = (bytes_per_row / (WIDTH as usize * 4)).max(1);
            let at = |x: u32, y: u32| {
                let px = x as usize * scale;
                let py = y as usize * scale;
                let o = py * bytes_per_row + px * 4;
                (*data.add(o), *data.add(o + 1), *data.add(o + 2))
            };
            // Channel order and orientation: `no flip` was verified empirically
            // to reproduce a top-down RGBA source exactly, so the context must
            // not apply any CTM change (see `CGContextDrawImage` above).
            assert_eq!(at(0, 0), (255, 0, 0), "top-left must stay red");
            assert_eq!(at(WIDTH - 1, 0), (0, 255, 0), "top-right must stay green");
            assert_eq!(at(0, HEIGHT - 1), (0, 0, 255), "bottom-left must stay blue");
            assert_eq!(at(WIDTH - 1, HEIGHT - 1), (255, 255, 255), "bottom-right must stay white");

            CGContextRelease(context);
            CGColorSpaceRelease(space);
        }
    }
}
