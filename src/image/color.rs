// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Color space conversions for image data.

use crate::image::format::{ColorSpace, ImageData};

/// BT.709 luminosity of an RGB triple, rounded to a `u8`.
///
/// Shared by every layout so the grayscale conversion has one definition of "how bright is this
/// pixel" instead of one per branch (and so a fix to the weights cannot miss a branch).
fn luminosity(r: f32, g: f32, b: f32) -> u8 {
    (0.2126 * r + 0.7152 * g + 0.0722 * b).round().clamp(0.0, 255.0) as u8
}

/// Convert pixel data to grayscale using luminosity weights.
///
/// Colour layouts are reduced to `Grayscale8`. The input is validated against its `ImageData`
/// layout **and** the `width`×`height` geometry before any pixel is read: a buffer that is
/// truncated (or too long) for its declared geometry is an `Err`, not a silently shorter
/// `Grayscale8` image. Grayscale inputs are already grayscale, so they are returned unchanged
/// (idempotent, at their own depth) rather than being re-read as if they held RGB triplets.
pub fn to_grayscale(
    data: ImageData,
    width: u32,
    height: u32,
) -> Result<(ImageData, u32, u32), String> {
    let total =
        (width as usize).checked_mul(height as usize).ok_or("grayscale dimensions overflow")?;
    let expected =
        total.checked_mul(data.bytes_per_pixel()).ok_or("grayscale byte count overflow")?;
    if data.as_bytes().len() != expected {
        return Err(format!(
            "grayscale input is {width}x{height} at {} bytes per pixel, which needs {expected} \
             bytes, but the buffer holds {} — pass a buffer matching the geometry",
            data.bytes_per_pixel(),
            data.as_bytes().len()
        ));
    }

    let gray = match &data {
        // Already grayscale: converting again must not read three bytes as if they were RGB,
        // so these return the pixels unchanged.
        ImageData::Grayscale8(_) | ImageData::Grayscale16(_) => return Ok((data, width, height)),
        ImageData::Rgba8(d) => d
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| luminosity(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect(),
        ImageData::Rgb8(d) => d
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| luminosity(p[0] as f32, p[1] as f32, p[2] as f32))
            .collect(),
        ImageData::Rgba16(d) => d
            .as_chunks::<8>()
            .0
            .iter()
            .map(|p| {
                let r = (u16::from_be_bytes([p[0], p[1]]) >> 8) as u8;
                let g = (u16::from_be_bytes([p[2], p[3]]) >> 8) as u8;
                let b = (u16::from_be_bytes([p[4], p[5]]) >> 8) as u8;
                luminosity(r as f32, g as f32, b as f32)
            })
            .collect(),
        ImageData::Rgb16(d) => d
            .as_chunks::<6>()
            .0
            .iter()
            .map(|p| {
                let r = (u16::from_be_bytes([p[0], p[1]]) >> 8) as u8;
                let g = (u16::from_be_bytes([p[2], p[3]]) >> 8) as u8;
                let b = (u16::from_be_bytes([p[4], p[5]]) >> 8) as u8;
                luminosity(r as f32, g as f32, b as f32)
            })
            .collect(),
    };
    Ok((ImageData::Grayscale8(gray), width, height))
}

/// Convert RGBA to RGB by removing the alpha channel.
pub fn rgba_to_rgb(data: &[u8], width: u32, height: u32) -> Result<ImageData, String> {
    let total = (width as usize).checked_mul(height as usize).ok_or("RGBA dimensions overflow")?;
    let expected = total.checked_mul(4).ok_or("RGBA byte count overflow")?;
    if data.len() != expected {
        return Err(format!(
            "RGBA input is {width}x{height}, which needs {expected} RGBA8 bytes, but the buffer \
             holds {} — pass a buffer matching the geometry",
            data.len()
        ));
    }
    let mut rgb = Vec::with_capacity(total * 3);
    for px in data.as_chunks::<4>().0 {
        rgb.extend_from_slice(&px[..3]);
    }
    Ok(ImageData::Rgb8(rgb))
}

/// Adjust brightness. Delta in range -255..255.
pub fn adjust_brightness(data: &mut [u8], delta: i32) {
    for pixel in data.as_chunks_mut::<4>().0 {
        for val in pixel.iter_mut().take(3) {
            *val = ((*val as i32 + delta).clamp(0, 255)) as u8;
        }
    }
}

/// Adjust contrast. Factor in range 0.0..3.0.
pub fn adjust_contrast(data: &mut [u8], factor: f32) {
    let factor = factor.max(0.0);
    for pixel in data.as_chunks_mut::<4>().0 {
        for val in pixel.iter_mut().take(3) {
            let new_val =
                (((*val as f32 - 128.0) * factor + 128.0).round()).clamp(0.0, 255.0) as i32;
            *val = new_val.clamp(0, 255) as u8;
        }
    }
}

/// Invert pixel colors (negative effect).
pub fn invert(data: &mut [u8]) {
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel[0] = 255 - pixel[0];
        pixel[1] = 255 - pixel[1];
        pixel[2] = 255 - pixel[2];
    }
}

/// Convert RGB to HSL values.
///
/// The hue is always returned in `[0, 360)`: the red-wheel sector (where `max == r`) computes
/// `(g - b) / diff`, which is negative for colours like magenta `(255, 0, 255)`, and is wrapped
/// with [`f32::rem_euclid`] so `-60°` becomes `300°` instead of a signed remainder that the
/// inverse would misread.
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
        let sector = if max == r {
            // `rem_euclid` keeps the result in [0, 6) even when `g - b` is negative, so the
            // hue wraps around the wheel instead of going negative.
            ((g - b) / diff).rem_euclid(6.0)
        } else if max == g {
            (b - r) / diff + 2.0
        } else {
            (r - g) / diff + 4.0
        };
        (sector * 60.0, s)
    };
    (h, s, l)
}

/// Convert HSL to RGB.
///
/// The hue is normalised into `[0, 360)` before the sector is chosen, so a caller that passes a
/// negative hue (or a hue ≥ 360°) lands on the correct 60-degree sector rather than the mirror of
/// it. Saturation and lightness are clamped to `[0, 1]`.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
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
    (
        ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

/// Convert between color spaces with actual pixel data transformation.
pub fn convert_between_color_spaces(
    data: ImageData,
    width: u32,
    height: u32,
    source: ColorSpace,
    target: ColorSpace,
) -> Result<(ImageData, u32, u32), String> {
    if source == target {
        return Ok((data, width, height));
    }

    let total_pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or("color-space conversion dimensions overflow")?;

    match (source, target) {
        // The gamma pair operates on RGBA8 only: alpha is passed through and only the RGB
        // triple is expanded/compressed. Reading any other layout's bytes as four channels
        // would treat a 3-byte RGB or a 1-byte grayscale pixel as if it were RGBA, so those
        // layouts are refused here (call `to_rgba8` first if that is what the caller wants).
        (ColorSpace::Srgb, ColorSpace::LinearRgb) | (ColorSpace::LinearRgb, ColorSpace::Srgb) => {
            let ImageData::Rgba8(rgba) = &data else {
                return Err(format!(
                    "{} -> {} conversion requires RGBA8 data, got {}; convert the image to \
                     Rgba8 first",
                    source_name(source),
                    source_name(target),
                    data.variant_name()
                ));
            };
            let expected = total_pixels.checked_mul(4).ok_or("RGBA byte count overflow")?;
            if rgba.len() != expected {
                return Err(format!(
                    "RGBA input is {width}x{height}, which needs {expected} bytes, but the \
                     buffer holds {} — pass a buffer matching the geometry",
                    rgba.len()
                ));
            }
            let to_linear = source == ColorSpace::Srgb;
            let mut out = Vec::with_capacity(expected);
            for px in rgba.as_chunks::<4>().0 {
                for &channel in px.iter().take(3) {
                    out.push(if to_linear {
                        srgb_to_linear(channel)
                    } else {
                        linear_to_srgb(channel)
                    });
                }
                out.push(px[3]); // alpha passthrough
            }
            Ok((ImageData::Rgba8(out), width, height))
        }
        (_, ColorSpace::Grayscale) => to_grayscale(data, width, height),
        _ => Err(format!("Unsupported color space conversion: {source:?} -> {target:?}")),
    }
}

/// The name of a color space, for error messages that must say *which* conversion was refused.
fn source_name(space: ColorSpace) -> &'static str {
    match space {
        ColorSpace::Srgb => "Srgb",
        ColorSpace::AdobeRgb => "AdobeRgb",
        ColorSpace::LinearRgb => "LinearRgb",
        ColorSpace::ProPhotoRgb => "ProPhotoRgb",
        ColorSpace::DisplayP3 => "DisplayP3",
        ColorSpace::Grayscale => "Grayscale",
        ColorSpace::Cmyk => "Cmyk",
        ColorSpace::Unknown => "Unknown",
    }
}

/// Convert a single sRGB channel value (0-255) to linear.
fn srgb_to_linear(c: u8) -> u8 {
    let v = c as f32 / 255.0;
    let linear = if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    (linear * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Convert a single linear channel value (0-255) to sRGB.
fn linear_to_srgb(c: u8) -> u8 {
    let v = c as f32 / 255.0;
    let srgb = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
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

    /// A grayscale image is already grayscale: `to_grayscale` must return it unchanged, not read
    /// the first three bytes of every pixel and treat them as an RGB triple.
    #[test]
    fn to_grayscale_is_idempotent_for_gray_inputs() {
        let gray8 = ImageData::Grayscale8(vec![10, 20, 30]);
        let (out, w, h) = to_grayscale(gray8.clone(), 3, 1).unwrap();
        assert_eq!((w, h), (3, 1));
        assert_eq!(out, gray8, "8-bit grayscale must round-trip unchanged");

        // 16-bit grayscale: each pixel is two bytes, so reading three bytes would cross into the
        // next pixel. Idempotence means the 16-bit values are kept, not re-read as RGB.
        let gray16 = ImageData::Grayscale16(vec![0x00, 0x10, 0x00, 0x20]);
        let (out, w, h) = to_grayscale(gray16.clone(), 2, 1).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, gray16, "16-bit grayscale must round-trip unchanged");
    }

    /// A buffer that does not match its declared geometry is an error, not a shorter image.
    #[test]
    fn to_grayscale_rejects_truncated_and_oversized_buffers() {
        // One 1x1 RGBA pixel is 4 bytes; three is truncated.
        assert!(to_grayscale(ImageData::Rgba8(vec![1, 2, 3]), 1, 1).is_err());
        // Five bytes for a 1x1 image is oversized.
        assert!(to_grayscale(ImageData::Rgba8(vec![1, 2, 3, 4, 5]), 1, 1).is_err());
    }

    /// `rgba_to_rgb` must not silently drop a short tail; it must report the mismatch.
    #[test]
    fn rgba_to_rgb_rejects_truncation() {
        assert!(rgba_to_rgb(&[255, 0, 0, 255, 0], 2, 1).is_err());
        assert!(rgba_to_rgb(&[255, 0, 0, 255], 1, 1).is_ok());
    }

    /// The gamma bridge must validate layout and geometry before reading pixels.
    #[test]
    fn color_space_bridge_validates_layout_and_geometry() {
        // A truncated RGBA buffer (1x1 needs 4 bytes, got 1) must error, not return an empty image.
        assert!(convert_between_color_spaces(
            ImageData::Rgba8(vec![1]),
            1,
            1,
            ColorSpace::Srgb,
            ColorSpace::LinearRgb
        )
        .is_err());

        // A non-RGBA layout must be refused by name rather than read as four channels.
        let err = convert_between_color_spaces(
            ImageData::Rgb8(vec![1, 2, 3]),
            1,
            1,
            ColorSpace::Srgb,
            ColorSpace::LinearRgb,
        )
        .unwrap_err();
        assert!(err.contains("requires RGBA8"), "unexpected error: {err}");

        // The RGBA normal path still passes: 1x1 opaque red converts without error and keeps its
        // alpha byte.
        let (out, w, h) = convert_between_color_spaces(
            ImageData::Rgba8(vec![255, 0, 0, 255]),
            1,
            1,
            ColorSpace::Srgb,
            ColorSpace::LinearRgb,
        )
        .unwrap();
        assert_eq!((w, h), (1, 1));
        match out {
            ImageData::Rgba8(d) => {
                assert_eq!(d.len(), 4);
                assert_eq!(d[3], 255, "alpha must pass through unchanged");
            }
            other => panic!("expected RGBA8 output, got {}", other.variant_name()),
        }
    }

    /// The whole RGB wheel must round-trip through HSL within a small per-channel error.
    ///
    /// The threshold is 2 per channel: the conversions go through `f32` hue arithmetic and a final
    /// `.round()` on the way back, so one unit of rounding is expected; 2 leaves headroom while
    /// still catching a hue that lands in the wrong sector (which throws a channel off by ~255).
    #[test]
    fn rgb_hsl_round_trip_covers_the_wheel() {
        let wheel = [
            (255, 0, 0),   // red
            (255, 0, 255), // magenta: the case whose signed % made hue -60
            (0, 0, 255),   // blue
            (0, 255, 255), // cyan
            (0, 255, 0),   // green
            (255, 255, 0), // yellow
            (255, 255, 255),
            (0, 0, 0),
            (128, 64, 32),
        ];
        for (r, g, b) in wheel {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!(
                (r as i32 - r2 as i32).abs() <= 2
                    && (g as i32 - g2 as i32).abs() <= 2
                    && (b as i32 - b2 as i32).abs() <= 2,
                "({r},{g},{b}) round-tripped to ({r2},{g2},{b2}) via hue {h}"
            );
        }
    }

    /// The defect colour itself: magenta must not lose its blue channel.
    #[test]
    fn magenta_round_trips_without_losing_blue() {
        let (h, s, l) = rgb_to_hsl(255, 0, 255);
        assert!((0.0..360.0).contains(&h), "hue must be normalised, got {h}");
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert_eq!((r, g, b), (255, 0, 255), "magenta must round-trip exactly");
    }

    /// A negative hue handed to the inverse must be normalised, not mirrored.
    #[test]
    fn hsl_to_rgb_normalises_negative_hue() {
        assert_eq!(hsl_to_rgb(-60.0, 1.0, 0.5), hsl_to_rgb(300.0, 1.0, 0.5));
    }
}
