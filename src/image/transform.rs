// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Image transform operations — resize, crop, rotate, flip.

use crate::image::format::ImageData;

/// Resize RGBA image using bilinear interpolation.
pub fn resize(
    data: ImageData,
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Result<ImageData, String> {
    if dst_w == 0 || dst_h == 0 {
        return Err(format!("resize target must be at least 1x1 pixels, got {dst_w}x{dst_h}"));
    }
    // The target byte count must fit in `usize`; reject oversized dimensions rather
    // than let a hostile `dst_w`/`dst_h` wrap the multiply and reallocate unbounded.
    let total = (dst_w as usize)
        .checked_mul(dst_h as usize)
        .and_then(|px| px.checked_mul(4))
        .ok_or("resize target dimensions overflow")?;
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => {
            return Err(format!(
                "resize only accepts RGBA8 data, got {}; convert the image to Rgba8 first",
                data.variant_name()
            ))
        }
    };
    let expected_len = (src_w as usize)
        .checked_mul(src_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Source dimensions overflow")?;
    if pixels.len() != expected_len || src_w == 0 || src_h == 0 {
        return Err(format!(
            "resize source is {src_w}x{src_h}, which needs {} RGBA8 bytes, but the buffer \
             holds {} — pass a buffer matching src_w x src_h",
            expected_len,
            pixels.len()
        ));
    }
    let mut out = Vec::with_capacity(total);

    let x_ratio = src_w as f32 / dst_w as f32;
    let y_ratio = src_h as f32 / dst_h as f32;

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = dx as f32 * x_ratio;
            let sy = dy as f32 * y_ratio;
            let x1 = (sx as u32).min(src_w.saturating_sub(1));
            let y1 = (sy as u32).min(src_h.saturating_sub(1));
            let x2 = (x1 + 1).min(src_w.saturating_sub(1));
            let y2 = (y1 + 1).min(src_h.saturating_sub(1));

            let x_frac = sx - x1 as f32;
            let y_frac = sy - y1 as f32;

            let get_pixel = |x: u32, y: u32, c: usize| -> u8 {
                let off = (y as usize * src_w as usize + x as usize) * 4 + c;
                pixels[off]
            };

            for c in 0..4 {
                let v00 = get_pixel(x1, y1, c) as f32;
                let v10 = get_pixel(x2, y1, c) as f32;
                let v01 = get_pixel(x1, y2, c) as f32;
                let v11 = get_pixel(x2, y2, c) as f32;

                let v0 = v00 * (1.0 - x_frac) + v10 * x_frac;
                let v1 = v01 * (1.0 - x_frac) + v11 * x_frac;
                let val = v0 * (1.0 - y_frac) + v1 * y_frac;
                out.push(val as u8);
            }
        }
    }
    Ok(ImageData::Rgba8(out))
}

/// Crop a rectangular region from an RGBA image.
pub fn crop(
    data: ImageData,
    src_w: u32,
    src_h: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Result<ImageData, String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => {
            return Err(format!(
                "crop only accepts RGBA8 data, got {}; convert the image to Rgba8 first",
                data.variant_name()
            ))
        }
    };
    let expected_len = (src_w as usize)
        .checked_mul(src_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Source dimensions overflow")?;
    if pixels.len() != expected_len || w == 0 || h == 0 {
        return Err(format!(
            "crop source is {src_w}x{src_h}, which needs {expected_len} RGBA8 bytes, but the \
             buffer holds {} (and the crop must be at least 1x1, got {w}x{h})",
            pixels.len()
        ));
    }
    if x > src_w || w > src_w - x || y > src_h || h > src_h - y {
        return Err(format!(
            "crop region {w}x{h} at ({x}, {y}) does not fit inside the {src_w}x{src_h} image; \
             x + w must be <= {src_w} and y + h must be <= {src_h}"
        ));
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let src_off = ((y + row) * src_w + x) as usize * 4;
        let count = w as usize * 4;
        out.extend_from_slice(&pixels[src_off..src_off + count]);
    }
    Ok(ImageData::Rgba8(out))
}

/// Rotate RGBA image by 90, 180, or 270 degrees.
pub fn rotate(
    data: ImageData,
    src_w: u32,
    src_h: u32,
    degrees: u32,
) -> Result<(ImageData, u32, u32), String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => {
            return Err(format!(
                "rotate only accepts RGBA8 data, got {}; convert the image to Rgba8 first",
                data.variant_name()
            ))
        }
    };
    let expected_len = (src_w as usize)
        .checked_mul(src_h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Source dimensions overflow")?;
    if pixels.len() != expected_len {
        return Err(format!(
            "rotate source is {src_w}x{src_h}, which needs {expected_len} RGBA8 bytes, but the \
             buffer holds {}",
            pixels.len()
        ));
    }
    match degrees % 360 {
        0 => Ok((data, src_w, src_h)),
        90 => {
            let mut out = Vec::with_capacity((src_w * src_h * 4) as usize);
            for x in 0..src_w {
                for y in (0..src_h).rev() {
                    let off = ((y * src_w + x) * 4) as usize;
                    out.extend_from_slice(&pixels[off..off + 4]);
                }
            }
            Ok((ImageData::Rgba8(out), src_h, src_w))
        }
        180 => {
            let mut out = Vec::with_capacity(pixels.len());
            for y in (0..src_h).rev() {
                for x in (0..src_w).rev() {
                    let off = ((y * src_w + x) * 4) as usize;
                    out.extend_from_slice(&pixels[off..off + 4]);
                }
            }
            Ok((ImageData::Rgba8(out), src_w, src_h))
        }
        270 => {
            let mut out = Vec::with_capacity((src_w * src_h * 4) as usize);
            for x in (0..src_w).rev() {
                for y in 0..src_h {
                    let off = ((y * src_w + x) * 4) as usize;
                    out.extend_from_slice(&pixels[off..off + 4]);
                }
            }
            Ok((ImageData::Rgba8(out), src_h, src_w))
        }
        _ => Err(format!("rotation angle must be 0, 90, 180 or 270 degrees, got {degrees}")),
    }
}

/// Flip RGBA image horizontally (mirror).
pub fn flip_horizontal(data: ImageData, w: u32, h: u32) -> Result<ImageData, String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => {
            return Err(format!(
                "flip only accepts RGBA8 data, got {}; convert the image to Rgba8 first",
                data.variant_name()
            ))
        }
    };
    let expected_len = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Image dimensions overflow")?;
    if pixels.len() != expected_len {
        return Err(format!(
            "flip image is {w}x{h}, which needs {expected_len} RGBA8 bytes, but the buffer \
             holds {}",
            pixels.len()
        ));
    }
    let mut out = pixels.clone();
    for y in 0..h {
        for x in 0..w / 2 {
            let a = ((y * w + x) * 4) as usize;
            let b = ((y * w + (w - 1 - x)) * 4) as usize;
            for c in 0..4 {
                out.swap(a + c, b + c);
            }
        }
    }
    Ok(ImageData::Rgba8(out))
}

/// Flip RGBA image vertically.
pub fn flip_vertical(data: ImageData, w: u32, h: u32) -> Result<ImageData, String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => {
            return Err(format!(
                "flip only accepts RGBA8 data, got {}; convert the image to Rgba8 first",
                data.variant_name()
            ))
        }
    };
    let expected_len = (w as usize)
        .checked_mul(h as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("Image dimensions overflow")?;
    if pixels.len() != expected_len {
        return Err(format!(
            "flip image is {w}x{h}, which needs {expected_len} RGBA8 bytes, but the buffer \
             holds {}",
            pixels.len()
        ));
    }
    let mut out = pixels.clone();
    let row_size = (w as usize) * 4;
    for y in 0..h / 2 {
        let a = (y as usize) * row_size;
        let b = ((h - 1 - y) as usize) * row_size;
        for c in 0..row_size {
            out.swap(a + c, b + c);
        }
    }
    Ok(ImageData::Rgba8(out))
}

/// Applies an EXIF orientation tag to RGBA8 pixels, returning the corrected data and its new size.
///
/// The eight EXIF orientation values describe how the camera was held relative to the scene. Value
/// `1` is the identity; the others are the seven combinations of a rotation and a mirror. This maps
/// each one onto the primitives above ([`rotate`], [`flip_horizontal`], [`flip_vertical`]) so there
/// is one implementation of each transform.
///
/// # Why this matters
///
/// `Extract_exif` parses the tag but nothing applied it, so a photo shot in portrait on a camera
/// that stored it landscape (the common case, orientation 6) decoded and rendered rotated. Applying
/// the tag at decode time is the whole reason to parse it.
///
/// An unknown value (`0`, `9`, …) is returned **unchanged** rather than guessed at, matching how
/// this crate treats any other absent capability.
pub fn apply_exif_orientation(
    data: ImageData,
    w: u32,
    h: u32,
    orientation: u8,
) -> Result<(ImageData, u32, u32), String> {
    match orientation {
        // 1 = normal, 0/unknown = leave as-is.
        1 => Ok((data, w, h)),
        // 2 = mirrored horizontally.
        2 => Ok((flip_horizontal(data, w, h)?, w, h)),
        // 3 = rotated 180.
        3 => rotate(data, w, h, 180),
        // 4 = mirrored vertically.
        4 => Ok((flip_vertical(data, w, h)?, w, h)),
        // 5 = transposed (mirror across the top-left/bottom-right diagonal): flip H then rotate
        // 270 (a transpose maps (x, y) to (y, x); the 90 CW rotation above would map to the
        // *other* diagonal instead).
        5 => {
            let (flipped, fw, fh) = (flip_horizontal(data, w, h)?, w, h);
            rotate(flipped, fw, fh, 270)
        }
        // 6 = rotated 90 clockwise (the usual "portrait photo stored landscape").
        6 => rotate(data, w, h, 90),
        // 7 = transverse (mirror across the top-right/bottom-left diagonal): flip H then rotate 90.
        7 => {
            let (flipped, fw, fh) = (flip_horizontal(data, w, h)?, w, h);
            rotate(flipped, fw, fh, 90)
        }
        // 8 = rotated 90 counter-clockwise.
        8 => rotate(data, w, h, 270),
        // Any other value is not a defined orientation: return the pixels untouched.
        _ => Ok((data, w, h)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_data(w: u32, h: u32) -> ImageData {
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                pixels.push((x * 64) as u8);
                pixels.push((y * 64) as u8);
                pixels.push(128);
                pixels.push(255);
            }
        }
        ImageData::Rgba8(pixels)
    }

    #[test]
    fn test_resize_downscale() {
        let data = make_test_data(10, 10);
        let result = resize(data, 10, 10, 5, 5).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 5 * 5 * 4);
        }
    }

    #[test]
    fn test_resize_upscale() {
        let data = make_test_data(5, 5);
        let result = resize(data, 5, 5, 10, 10).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 10 * 10 * 4);
        }
    }

    #[test]
    fn test_crop_full() {
        let data = make_test_data(10, 10);
        let result = crop(data, 10, 10, 0, 0, 10, 10).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 10 * 10 * 4);
        }
    }

    #[test]
    fn test_crop_partial() {
        let data = make_test_data(10, 10);
        let result = crop(data, 10, 10, 2, 2, 4, 4).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 4 * 4 * 4);
        }
    }

    #[test]
    fn test_crop_bounds_error() {
        let data = make_test_data(10, 10);
        assert!(crop(data, 10, 10, 8, 8, 5, 5).is_err());
    }

    #[test]
    fn test_rotate_90() {
        let data = make_test_data(4, 3);
        let (rotated, w, h) = rotate(data, 4, 3, 90).unwrap();
        assert_eq!(w, 3);
        assert_eq!(h, 4);
        if let ImageData::Rgba8(r) = rotated {
            assert_eq!(r.len(), 3 * 4 * 4);
        }
    }

    #[test]
    fn test_rotate_180() {
        let data = make_test_data(4, 3);
        let (_rotated, w, h) = rotate(data.clone(), 4, 3, 180).unwrap();
        assert_eq!(w, 4);
        assert_eq!(h, 3);
    }

    #[test]
    fn test_flip_horizontal() {
        let data = make_test_data(4, 4);
        let result = flip_horizontal(data, 4, 4).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 4 * 4 * 4);
        }
    }

    #[test]
    fn test_flip_vertical() {
        let data = make_test_data(4, 4);
        let result = flip_vertical(data, 4, 4).unwrap();
        if let ImageData::Rgba8(r) = result {
            assert_eq!(r.len(), 4 * 4 * 4);
        }
    }

    #[test]
    fn test_resize_zero_error() {
        let data = make_test_data(10, 10);
        assert!(resize(data, 10, 10, 0, 0).is_err());
    }

    #[test]
    fn test_transforms_reject_short_rgba_data() {
        let short = ImageData::Rgba8(vec![0; 3]);
        assert!(resize(short.clone(), 1, 1, 2, 2).is_err());
        assert!(crop(short.clone(), 1, 1, 0, 0, 1, 1).is_err());
        assert!(rotate(short.clone(), 1, 1, 90).is_err());
        assert!(flip_horizontal(short.clone(), 1, 1).is_err());
        assert!(flip_vertical(short, 1, 1).is_err());
    }

    /// Reads pixel `(x, y)` out of an RGBA8 image as its RGB tuple.
    fn pixel(data: &ImageData, w: u32, x: u32, y: u32) -> (u8, u8, u8) {
        let ImageData::Rgba8(bytes) = data else { panic!("expected RGBA8") };
        let off = ((y * w + x) * 4) as usize;
        (bytes[off], bytes[off + 1], bytes[off + 2])
    }

    /// `apply_exif_orientation` must map each tag onto the right transform.
    ///
    /// Pins the defect: `extract_exif` parsed the orientation tag but nothing applied it, so a
    /// portrait photo stored landscape (tag 6) decoded and rendered rotated. The tests use a 2x1
    /// image whose two pixels differ, so a wrong transform cannot pass by symmetry.
    #[test]
    fn exif_orientation_is_applied() {
        // 2x1: left pixel red, right pixel blue.
        let src = ImageData::Rgba8(vec![255, 0, 0, 255, 0, 0, 255, 255]);

        // 1 = identity: unchanged.
        let (out, w, h) = apply_exif_orientation(src.clone(), 2, 1, 1).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixel(&out, 2, 0, 0), (255, 0, 0));
        assert_eq!(pixel(&out, 2, 1, 0), (0, 0, 255));

        // 3 = 180: red moves to the right, blue to the left.
        let (out, w, h) = apply_exif_orientation(src.clone(), 2, 1, 3).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixel(&out, 2, 0, 0), (0, 0, 255));
        assert_eq!(pixel(&out, 2, 1, 0), (255, 0, 0));

        // 6 = 90 CW: a 2x1 image becomes 1x2, and the left pixel goes to the top.
        let (out, w, h) = apply_exif_orientation(src.clone(), 2, 1, 6).unwrap();
        assert_eq!((w, h), (1, 2), "a 90-degree rotation swaps the dimensions");
        assert_eq!(pixel(&out, 1, 0, 0), (255, 0, 0), "the left pixel rotates to the top");
        assert_eq!(pixel(&out, 1, 0, 1), (0, 0, 255));

        // 2 = mirror horizontal: red and blue swap places without changing the size.
        let (out, w, h) = apply_exif_orientation(src.clone(), 2, 1, 2).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixel(&out, 2, 0, 0), (0, 0, 255));
        assert_eq!(pixel(&out, 2, 1, 0), (255, 0, 0));

        // An undefined tag leaves the pixels untouched rather than guessing.
        let (out, w, h) = apply_exif_orientation(src, 2, 1, 9).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(pixel(&out, 2, 0, 0), (255, 0, 0));
    }

    /// Reads an RGBA8 image as a grid of its red channel (top-to-bottom rows of left-to-right
    /// pixels), so an orientation test can compare whole images without index gymnastics.
    fn red_grid(data: &ImageData, w: u32, h: u32) -> Vec<Vec<u8>> {
        let ImageData::Rgba8(bytes) = data else { panic!("expected RGBA8") };
        (0..h).map(|y| (0..w).map(|x| bytes[((y * w + x) * 4) as usize]).collect()).collect()
    }

    /// All eight EXIF orientation tags, checked on a non-symmetric 3x2 matrix.
    ///
    /// Pins the defect: tags 5 (transpose) and 7 (transverse) were implemented with the two
    /// diagonal transforms swapped, which only a non-symmetric image can expose (a square image
    /// with symmetric content would pass either way).
    #[test]
    fn exif_orientation_maps_all_eight_tags_on_a_non_symmetric_matrix() {
        // 3 wide x 2 tall, pixels numbered 1..=6 row-major in the red channel.
        let src = ImageData::Rgba8(vec![
            1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255, 5, 0, 0, 255, 6, 0, 0, 255,
        ]);

        let cases: [(u8, u32, u32, Vec<Vec<u8>>); 8] = [
            (1, 3, 2, vec![vec![1, 2, 3], vec![4, 5, 6]]),
            (2, 3, 2, vec![vec![3, 2, 1], vec![6, 5, 4]]),
            (3, 3, 2, vec![vec![6, 5, 4], vec![3, 2, 1]]),
            (4, 3, 2, vec![vec![4, 5, 6], vec![1, 2, 3]]),
            (5, 2, 3, vec![vec![1, 4], vec![2, 5], vec![3, 6]]),
            (6, 2, 3, vec![vec![4, 1], vec![5, 2], vec![6, 3]]),
            (7, 2, 3, vec![vec![6, 3], vec![5, 2], vec![4, 1]]),
            (8, 2, 3, vec![vec![3, 6], vec![2, 5], vec![1, 4]]),
        ];

        for (tag, w, h, expected) in cases {
            let (out, ow, oh) = apply_exif_orientation(src.clone(), 3, 2, tag).unwrap();
            assert_eq!((ow, oh), (w, h), "orientation {tag} must produce {w}x{h}, got {ow}x{oh}");
            assert_eq!(
                red_grid(&out, w, h),
                expected,
                "orientation {tag} produced the wrong pixels"
            );
        }
    }
}
