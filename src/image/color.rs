//! Color space conversions for image data.

use crate::image::format::ImageData;

/// Convert RGBA8 pixel data to grayscale using luminosity weights.
pub fn to_grayscale(
    data: ImageData,
    width: u32,
    height: u32,
) -> Result<(ImageData, u32, u32), String> {
    let pixels = match &data {
        ImageData::Rgba8(d) => d,
        _ => data.as_bytes(),
    };
    let total = (width * height) as usize;
    let bpp = data.bytes_per_pixel();
    let mut gray = Vec::with_capacity(total);
    for i in 0..total {
        let off = i * bpp;
        if off + 2 < pixels.len() {
            let r = pixels[off] as f32;
            let g = pixels[off + 1] as f32;
            let b = pixels[off + 2] as f32;
            // BT.709 luminosity weights
            let lum = (0.2126 * r + 0.7152 * g + 0.0722 * b) as u8;
            gray.push(lum);
        }
    }
    Ok((ImageData::Grayscale8(gray), width, height))
}

/// Convert RGBA to RGB by removing alpha channel.
pub fn rgba_to_rgb(data: &[u8], width: u32, height: u32) -> Result<ImageData, String> {
    let total = (width * height) as usize;
    let mut rgb = Vec::with_capacity(total * 3);
    for i in 0..total {
        let off = i * 4;
        if off + 3 < data.len() {
            rgb.push(data[off]);
            rgb.push(data[off + 1]);
            rgb.push(data[off + 2]);
        }
    }
    Ok(ImageData::Rgb8(rgb))
}

/// Adjust brightness. Delta in range -255..255.
pub fn adjust_brightness(data: &mut [u8], delta: i32) {
    for pixel in data.chunks_exact_mut(4) {
        for val in pixel.iter_mut().take(3) {
            *val = ((*val as i32 + delta).clamp(0, 255)) as u8;
        }
    }
}

/// Adjust contrast. Factor in range 0.0..3.0.
pub fn adjust_contrast(data: &mut [u8], factor: f32) {
    let factor = factor.max(0.0);
    for pixel in data.chunks_exact_mut(4) {
        for val in pixel.iter_mut().take(3) {
            let new_val = ((*val as f32 - 128.0) * factor + 128.0) as i32;
            *val = new_val.clamp(0, 255) as u8;
        }
    }
}

/// Invert pixel colors (negative effect).
pub fn invert(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }
}

/// Convert RGBA to HSL values.
pub fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let diff = max - min;
    let l = (max + min) / 2.0;
    let (h, s) = if diff.abs() < f32::EPSILON {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 { diff / (2.0 - max - min) } else { diff / (max + min) };
        let h = if max == r {
            ((g - b) / diff) % 6.0
        } else if max == g {
            (b - r) / diff + 2.0
        } else {
            (r - g) / diff + 4.0
        };
        (h * 60.0, s)
    };
    (h, s, l)
}

/// Convert HSL to RGBA.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (((r1 + m) * 255.0) as u8, ((g1 + m) * 255.0) as u8, ((b1 + m) * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_grayscale() {
        let data = ImageData::Rgba8(vec![255, 0, 0, 255, 0, 255, 0, 255]);
        let (gray, w, h) = to_grayscale(data, 2, 1).unwrap();
        assert_eq!(w, 2);
        assert_eq!(h, 1);
        if let ImageData::Grayscale8(g) = gray {
            assert_eq!(g.len(), 2);
        }
    }

    #[test]
    fn test_rgb_to_hsl() {
        let (h, s, l) = rgb_to_hsl(255, 0, 0);
        assert!((h - 0.0).abs() < 1.0 || (h - 360.0).abs() < 1.0);
        assert!((s - 1.0).abs() < 0.01);
        assert!((l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_hsl_to_rgb() {
        let (r, g, b) = hsl_to_rgb(0.0, 1.0, 0.5);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_adjust_brightness() {
        let mut data = vec![100, 100, 100, 255, 50, 50, 50, 255];
        adjust_brightness(&mut data, 50);
        assert_eq!(data[0], 150);
        assert_eq!(data[4], 100);
    }

    #[test]
    fn test_invert() {
        let mut data = vec![255, 128, 64, 255];
        invert(&mut data);
        assert_eq!(data[0], 0);
        assert_eq!(data[1], 127);
        assert_eq!(data[2], 191);
    }

    #[test]
    fn test_rgba_to_rgb() {
        let data = vec![255, 0, 0, 255, 0, 255, 0, 128];
        let rgb = rgba_to_rgb(&data, 2, 1).unwrap();
        if let ImageData::Rgb8(d) = rgb {
            assert_eq!(d.len(), 6);
            assert_eq!(&d[0..3], &[255, 0, 0]);
        }
    }
}
