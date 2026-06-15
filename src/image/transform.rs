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
        return Err("Target dimensions must be > 0".into());
    }
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => return Err("Resize requires RGBA8 data".into()),
    };
    let total = (dst_w * dst_h) as usize;
    let mut out = Vec::with_capacity(total * 4);

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
                let off = ((y * src_w + x) * 4 + c as u32) as usize;
                pixels.get(off).copied().unwrap_or(0)
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
        _ => return Err("Crop requires RGBA8 data".into()),
    };
    if x + w > src_w || y + h > src_h || w == 0 || h == 0 {
        return Err("Crop region exceeds image bounds".into());
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in 0..h {
        let src_off = ((y + row) * src_w + x) as usize * 4;
        let count = w as usize * 4;
        let end = (src_off + count).min(pixels.len());
        out.extend_from_slice(&pixels[src_off..end]);
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
        _ => return Err("Rotate requires RGBA8 data".into()),
    };
    match degrees % 360 {
        0 => Ok((data, src_w, src_h)),
        90 => {
            let mut out = Vec::with_capacity((src_w * src_h * 4) as usize);
            for x in 0..src_w {
                for y in (0..src_h).rev() {
                    let off = ((y * src_w + x) * 4) as usize;
                    let end = (off + 4).min(pixels.len());
                    out.extend_from_slice(&pixels[off..end]);
                }
            }
            Ok((ImageData::Rgba8(out), src_h, src_w))
        }
        180 => {
            let mut out = Vec::with_capacity(pixels.len());
            for y in (0..src_h).rev() {
                for x in (0..src_w).rev() {
                    let off = ((y * src_w + x) * 4) as usize;
                    let end = (off + 4).min(pixels.len());
                    out.extend_from_slice(&pixels[off..end]);
                }
            }
            Ok((ImageData::Rgba8(out), src_w, src_h))
        }
        270 => {
            let mut out = Vec::with_capacity((src_w * src_h * 4) as usize);
            for x in (0..src_w).rev() {
                for y in 0..src_h {
                    let off = ((y * src_w + x) * 4) as usize;
                    let end = (off + 4).min(pixels.len());
                    out.extend_from_slice(&pixels[off..end]);
                }
            }
            Ok((ImageData::Rgba8(out), src_h, src_w))
        }
        _ => Err("Rotation must be 0, 90, 180, or 270 degrees".into()),
    }
}

/// Flip RGBA image horizontally (mirror).
pub fn flip_horizontal(data: ImageData, w: u32, h: u32) -> Result<ImageData, String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => return Err("Flip requires RGBA8 data".into()),
    };
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
        _ => return Err("Flip requires RGBA8 data".into()),
    };
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
}
