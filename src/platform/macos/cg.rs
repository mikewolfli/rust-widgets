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

#![cfg(all(target_os = "macos", feature = "cocoa-legacy", widgets_unstripped))]
#![allow(non_snake_case)] // names mirror the C ABI

use std::cell::RefCell;
use std::os::raw::{c_int, c_void};

/// Opaque `CGContextRef`.
pub(crate) type CGContextRef = *mut c_void;
/// Opaque `CGColorSpaceRef`.
pub(crate) type CGColorSpaceRef = *mut c_void;
/// Opaque `CGImageRef`.
pub(crate) type CGImageRef = *mut c_void;

/// `kCGImageAlphaPremultipliedLast`: alpha stored last within the word.
pub(crate) const BITMAP_ALPHA_PREMULTIPLIED_LAST: u32 = 1;

/// `kCGImageAlphaPremultipliedFirst`: alpha stored first within the word.
///
/// Test-only: the regression test draws into the **canonical** little-endian BGRA bitmap
/// (premultiplied first + 32-bit little) rather than into a buffer carrying the same flags as
/// the source. A destination that shares the source's flags cannot detect a byte-order mistake:
/// the conversion and the raw read-back cancel it out.
#[cfg(test)]
pub(crate) const BITMAP_ALPHA_PREMULTIPLIED_FIRST: u32 = 2;

/// `kCGBitmapByteOrder32Big`.
///
/// Chosen together with [`BITMAP_ALPHA_PREMULTIPLIED_LAST`] to give a memory layout of
/// **R, G, B, A** — the same order the software renderer produces. The pairing is not
/// interchangeable: `PremultipliedLast | ByteOrder32Little` normalises to **A, B, G, R** in
/// memory, so writing R, G, B, A into such a buffer puts the alpha byte in the red channel.
///
/// # How that was found, after it had shipped
///
/// Every window on screen came out tinted with the theme's error red on a mid-grey frame, while
/// the same frame dumped straight from `drawRect:` was correct. The round-trip test below could
/// not see it because it built its *destination* bitmap with the same flags as the source: the
/// conversion and the raw read-back cancelled. `blit_draws_the_picture_the_display_shows` draws
/// into the little-endian BGRA context AppKit actually composites and fails on the old pairing
/// with `(255, 40, 30, 200)` where `(200, 30, 40, 255)` was written.
pub(crate) const BITMAP_BYTE_ORDER_32_BIG: u32 = 4 << 12;

/// `kCGBitmapByteOrder32Little`.
///
/// Test-only: it names the little-endian BGRA buffer AppKit composites into, which is the
/// destination of `blit_draws_the_picture_the_display_shows`. Paired with
/// [`BITMAP_ALPHA_PREMULTIPLIED_LAST`] the same flag means `A, B, G, R` in memory — the source
/// pairing bug this file's tests now pin, so no production path may use it.
#[cfg(test)]
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

/// The reusable CoreGraphics bitmap behind [`blit_rgba`] (G-601).
///
/// A blit used to create a colour space and a whole-window bitmap context per frame and release
/// them at the end. The context's storage is overwritten in full each frame, so it can live for as
/// long as the mount's size does; the cache is keyed by that size and drops the objects when the
/// window is resized or the thread exits.
struct BitmapCache {
    context: CGContextRef,
    color_space: CGColorSpaceRef,
    width: u32,
    height: u32,
}

impl BitmapCache {
    fn new(width: u32, height: u32) -> Option<Self> {
        // SAFETY: both are plain CoreGraphics constructors; each result is checked for null and
        // the colour space is released when the context cannot be created.
        unsafe {
            let color_space = CGColorSpaceCreateDeviceRGB();
            if color_space.is_null() {
                return None;
            }
            let context = CGBitmapContextCreate(
                std::ptr::null_mut(),
                width as usize,
                height as usize,
                8,
                width as usize * 4,
                color_space,
                BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_BIG,
            );
            if context.is_null() {
                CGColorSpaceRelease(color_space);
                return None;
            }
            Some(Self { context, color_space, width, height })
        }
    }
}

impl Drop for BitmapCache {
    fn drop(&mut self) {
        // SAFETY: both handles were created by this cache and are released exactly once here.
        unsafe {
            CGContextRelease(self.context);
            CGColorSpaceRelease(self.color_space);
        }
    }
}

thread_local! {
    static BITMAP_CACHE: RefCell<Option<BitmapCache>> = const { RefCell::new(None) };
}

/// Renders a top-down RGBA frame into an existing CoreGraphics context.
///
/// Handles three details the caller should not have to think about:
///
/// 1. **Premultiplication** — the frame is straight alpha; CoreGraphics'
///    `PremultipliedLast` layout is not, so RGB is scaled by alpha.
/// 2. **Row order** — the frame is top-down, CG draws bottom-up, so the CTM is
///    flipped around the destination height.
/// 3. **Allocation ownership** — the color space and bitmap context are retained per thread for
///    the current mount size (G-601); the image is released here on every path.
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

    let color_space = BITMAP_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let stale =
            slot.as_ref().is_none_or(|cache| cache.width != width || cache.height != height);
        if stale {
            *slot = BitmapCache::new(width, height);
        }
        slot.as_ref().map(|cache| cache.color_space)
    });
    let Some(color_space) = color_space else {
        log::error!("[macos] canvas: could not create the bitmap context for {width}x{height}");
        return false;
    };
    // The cache holds both handles for the rest of the blit; `color_space` only proves creation
    // succeeded, the context below is what receives the pixels.
    let _ = color_space;
    let bitmap = BITMAP_CACHE.with(|slot| slot.borrow().as_ref().map(|cache| cache.context));
    let Some(bitmap) = bitmap else {
        return false;
    };

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

// ── Screen sampling (SP-119) ────────────────────────────────────────────────

/// `kCGWindowListOptionOnScreenOnly`: composite only what is currently on screen.
pub(crate) const WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
/// `kCGWindowImageDefault`: the default image options (no shadow, no bounding box).
pub(crate) const WINDOW_IMAGE_DEFAULT: u32 = 0;

/// An opaque `CFTypeRef`, and the CFData the pixel bytes arrive in.
pub(crate) type CFTypeRef = *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// Composites the on-screen windows intersecting `bounds` into one image.
    ///
    /// A 1×1 `bounds` is the single-pixel read an eyedropper needs. The call answers null when the
    /// process has no Screen Recording permission, which is why it is the whole "can I sample?"
    /// test rather than a guess.
    fn CGWindowListCreateImage(
        bounds: CGRect,
        list_option: u32,
        window_id: u32,
        image_option: u32,
    ) -> CGImageRef;

    /// Creates an event source-independent event; passing a null source is legal.
    fn CGEventCreate(source: CFTypeRef) -> CFTypeRef;
    /// The event's location in the global display space (top-left origin, y down).
    fn CGEventGetLocation(event: CFTypeRef) -> CGPoint;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    /// Releases a CoreFoundation object.
    fn CFRelease(value: CFTypeRef);
}

/// Samples one pixel of the composited screen at global `(x, y)`.
///
/// Returns top-down `(r, g, b, a)`, or `None` when the pixel cannot be read — no Screen Recording
/// permission, nothing on screen there, or a CoreGraphics object that could not be created. The
/// pixel is drawn into a 1×1 bitmap the backend owns so the byte order is the one this file
/// already declares, instead of depending on whatever private layout
/// `CGWindowListCreateImage` chose for the snapshot.
///
/// # Why the `None` is honest rather than a fallback colour
///
/// macOS gives every process a *thumbnail* of another application's window even when the pixel
/// bytes are unavailable, so there is no case where inventing a colour would be correct. A picker
/// that receives `None` falls back to sampling its own painted surface, which is a real value.
pub(crate) fn sample_screen_pixel(x: f64, y: f64) -> Option<(u8, u8, u8, u8)> {
    // SAFETY: every CoreGraphics object created here is released on every path before return;
    // the only reads are the four bytes of the bitmap this function created.
    unsafe {
        let image = CGWindowListCreateImage(
            CGRect { origin: CGPoint { x, y }, size: CGSize { width: 1.0, height: 1.0 } },
            WINDOW_LIST_ON_SCREEN_ONLY,
            0,
            WINDOW_IMAGE_DEFAULT,
        );
        if image.is_null() {
            return None;
        }
        let color_space = CGColorSpaceCreateDeviceRGB();
        if color_space.is_null() {
            CGImageRelease(image);
            return None;
        }
        let context = CGBitmapContextCreate(
            std::ptr::null_mut(),
            1,
            1,
            8,
            4,
            color_space,
            BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_BIG,
        );
        CGColorSpaceRelease(color_space);
        if context.is_null() {
            CGImageRelease(image);
            return None;
        }
        // Nearest-neighbour: the source may be 1×1 at the display's pixel scale, and averaging is
        // not what a pixel read means.
        CGContextSetInterpolationQuality(context, 0);
        CGContextDrawImage(context, CGRect::new(1.0, 1.0), image);
        let data = CGBitmapContextGetData(context) as *const u8;
        let sampled = if data.is_null() {
            None
        } else {
            let pixel = std::slice::from_raw_parts(data, 4);
            // Shared with the objc2 backend; see `platform::screen_sample` for why an alpha-zero
            // read is an absence rather than a colour.
            crate::platform::screen_sample::presentable((pixel[0], pixel[1], pixel[2], pixel[3]))
        };
        CGContextRelease(context);
        CGImageRelease(image);
        sampled
    }
}

/// The pointer's position in the global display space (SP-119).
///
/// `CGEventGetLocation` answers in the same space [`sample_screen_pixel`] reads (top-left origin,
/// y down), so the two can be combined directly — unlike `NSEvent.mouseLocation`, which is
/// bottom-left relative to the primary screen and would need a flip the caller cannot see.
/// `None` when no event can be created, which needs no permission but can fail.
pub(crate) fn pointer_position() -> Option<(f64, f64)> {
    // SAFETY: a null source is the documented "create a plain event" form, and the event is
    // released before return.
    unsafe {
        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return None;
        }
        let location = CGEventGetLocation(event);
        CFRelease(event);
        Some((location.x, location.y))
    }
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
    fn bitmap_info_encodes_premultiplied_rgba() {
        // kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big
        assert_eq!(BITMAP_ALPHA_PREMULTIPLIED_LAST, 1);
        assert_eq!(BITMAP_BYTE_ORDER_32_BIG, 4 << 12);
        assert_eq!(BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_BIG, 0x4001);
    }

    #[test]
    fn blit_rejects_degenerate_input_without_touching_coregraphics() {
        let empty: [u8; 0] = [];
        // Null context short-circuits before any CoreGraphics call.
        // SAFETY: the null context is rejected before CoreGraphics is touched; `empty` is a valid zero-length slice.
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
                BITMAP_ALPHA_PREMULTIPLIED_LAST | BITMAP_BYTE_ORDER_32_BIG,
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

    /// The blit must produce the **picture CoreGraphics shows**, not merely a buffer that reads
    /// back the bytes it was handed.
    ///
    /// # Why the round-trip above cannot answer this
    ///
    /// It creates the destination bitmap with the *same* `bitmap_info` as the source. If that
    /// combination did not mean what its name says, `CGContextDrawImage` would convert into a
    /// wrong destination layout and the raw read-back would apply the inverse mistake — the two
    /// would cancel, and the test would pass while every window on screen was tinted. That is
    /// exactly what happened: `(200, 30, 40, 255)` reached the display as `(255, 40, 30, 200)`.
    /// This test removes the cancellation by drawing into the little-endian BGRA context AppKit
    /// composites into and reading the channels in that format's own order.
    #[test]
    fn blit_draws_the_picture_the_display_shows() {
        const WIDTH: u32 = 4;
        const HEIGHT: u32 = 4;

        // Two **non-primary** colours as well as the primaries: a byte-order swap maps pure red
        // onto pure red (255 is asymmetric only in green/blue), so an all-primary frame can hide
        // a swap. A mid grey would hide it entirely.
        let quadrants = [(200u8, 30u8, 40u8), (30, 200, 40), (30, 40, 200), (17, 200, 210)];
        let mut frame = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let offset = ((y * WIDTH + x) * 4) as usize;
                let (r, g, b) =
                    quadrants[((y >= HEIGHT / 2) as usize) * 2 + (x >= WIDTH / 2) as usize];
                frame[offset] = r;
                frame[offset + 1] = g;
                frame[offset + 2] = b;
                frame[offset + 3] = 255;
            }
        }

        // SAFETY: all arguments are valid for the duration of the test; every CoreGraphics
        // object created here is released before returning.
        unsafe {
            let space = CGColorSpaceCreateDeviceRGB();
            assert!(!space.is_null(), "colour space");
            let destination = CGBitmapContextCreate(
                std::ptr::null_mut(),
                WIDTH as usize,
                HEIGHT as usize,
                8,
                WIDTH as usize * 4,
                space,
                BITMAP_ALPHA_PREMULTIPLIED_FIRST | BITMAP_BYTE_ORDER_32_LITTLE,
            );
            assert!(!destination.is_null(), "BGRA destination context");

            assert!(blit_rgba(destination, WIDTH, HEIGHT, &frame), "blit must succeed");

            let data = CGBitmapContextGetData(destination) as *mut u8;
            assert!(!data.is_null(), "bitmap data");
            let stride = CGBitmapContextGetBytesPerRow(destination) as usize;
            // Little-endian BGRA: byte 0 is blue, byte 1 green, byte 2 red, byte 3 alpha.
            let at = |x: u32, y: u32| {
                let o = y as usize * stride + x as usize * 4;
                (*data.add(o + 2), *data.add(o + 1), *data.add(o), *data.add(o + 3))
            };

            for (index, (x, y)) in
                [(0u32, 0u32), (WIDTH - 1, 0), (0, HEIGHT - 1), (WIDTH - 1, HEIGHT - 1)]
                    .into_iter()
                    .enumerate()
            {
                let (r, g, b) = quadrants[index];
                assert_eq!(
                    at(x, y),
                    (r, g, b, 255),
                    "quadrant {index} must reach the display with its own colour"
                );
            }

            CGContextRelease(destination);
            CGColorSpaceRelease(space);
        }
    }
}
