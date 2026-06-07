//! Pixel-level operations: draw_bitmap_glyph, glyph_bitmap, fill_pixels,
//! blend_pixel, set_pixel, pixel_bytes_len, and anti-aliased coverage/geometry helpers.
use crate::core::{Color, Point, Rect, Size};
use crate::render::TextCluster;
use font8x8::{UnicodeFonts, BASIC_FONTS};

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_bitmap_glyph(
    frame: &mut [u8],
    surface_width: u32,
    surface_height: u32,
    ch: char,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    color: Color,
) {
    if ch.is_whitespace() || width <= 0 || height <= 0 {
        return;
    }
    let glyph = glyph_bitmap(ch);
    for gy in 0..8i32 {
        let row = glyph[gy as usize];
        for gx in 0..8i32 {
            if row & (1u8 << gx) == 0 {
                continue;
            }
            let x0 = x + (gx * width) / 8;
            let mut x1 = x + ((gx + 1) * width) / 8;
            let y0 = y + (gy * height) / 8;
            let mut y1 = y + ((gy + 1) * height) / 8;
            if x1 <= x0 {
                x1 = x0 + 1;
            }
            if y1 <= y0 {
                y1 = y0 + 1;
            }
            for py in y0.max(0)..y1.min(surface_height as i32) {
                for px in x0.max(0)..x1.min(surface_width as i32) {
                    blend_pixel(frame, surface_width, px as u32, py as u32, color, 1.0);
                }
            }
        }
    }
}
pub(crate) fn glyph_bitmap(ch: char) -> [u8; 8] {
    if let Some(bitmap) = BASIC_FONTS.get(ch) {
        return bitmap;
    }
    if let Some(bitmap) = BASIC_FONTS.get(ch.to_ascii_uppercase()) {
        return bitmap;
    }
    if let Some(bitmap) = BASIC_FONTS.get(ch.to_ascii_lowercase()) {
        return bitmap;
    }
    [0b11111111, 0b10000001, 0b10111101, 0b10100101, 0b10111101, 0b10000001, 0b11111111, 0b00000000]
}
pub(crate) fn pixel_bytes_len(size: Size) -> usize {
    size.width.saturating_mul(size.height).saturating_mul(4) as usize
}
pub fn fill_pixels(pixels: &mut [u8], color: Color) {
    let chunk_size = 4;
    let color_arr = [color.r, color.g, color.b, color.a];
    for chunk in pixels.chunks_mut(chunk_size) {
        if chunk.len() == chunk_size {
            chunk.copy_from_slice(&color_arr);
        } else {
            chunk.copy_from_slice(&color_arr[..chunk.len()]);
        }
    }
}
pub(crate) fn set_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color) {
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    frame[idx] = color.r;
    frame[idx + 1] = color.g;
    frame[idx + 2] = color.b;
    frame[idx + 3] = color.a;
}
pub fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: Color, coverage: f32) {
    if coverage <= 0.0 {
        return;
    }
    let idx = ((y * width + x) * 4) as usize;
    if idx + 3 >= frame.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a <= 0.0 {
        frame[idx] = 0;
        frame[idx + 1] = 0;
        frame[idx + 2] = 0;
        frame[idx + 3] = 0;
        return;
    }
    let dst = &mut frame[idx..idx + 4];
    let src = [color.r, color.g, color.b, color.a];
    let src_f: [f32; 4] = [
        src[0] as f32 / 255.0,
        src[1] as f32 / 255.0,
        src[2] as f32 / 255.0,
        src[3] as f32 / 255.0,
    ];
    let dst_f: [f32; 4] = [
        dst[0] as f32 / 255.0,
        dst[1] as f32 / 255.0,
        dst[2] as f32 / 255.0,
        dst[3] as f32 / 255.0,
    ];
    let out_a = src_a + dst_f[3] * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        dst.copy_from_slice(&[0, 0, 0, 0]);
        return;
    }
    let out_r = (src_f[0] * src_a + dst_f[0] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_g = (src_f[1] * src_a + dst_f[1] * dst_f[3] * (1.0 - src_a)) / out_a;
    let out_b = (src_f[2] * src_a + dst_f[2] * dst_f[3] * (1.0 - src_a)) / out_a;
    dst[0] = (out_r * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[1] = (out_g * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[2] = (out_b * 255.0).round().clamp(0.0, 255.0) as u8;
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
pub(crate) fn circle_fill_coverage(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    (radius + 1.0 - distance).clamp(0.0, 1.0)
}
pub(crate) fn circle_fill_coverage_grid(
    px: i32,
    py: i32,
    center: Point,
    radius: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            coverage_sum += circle_fill_coverage(distance, radius);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn circle_stroke_coverage_grid(
    px: i32,
    py: i32,
    center: Point,
    radius: f32,
    stroke_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    // radius is the outer radius, stroke_width is the width of the ring
    let outer_radius = radius;
    let inner_radius = (radius - stroke_width).max(0.0);
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let dx = sample_x - center.x as f32;
            let dy = sample_y - center.y as f32;
            let distance = (dx * dx + dy * dy).sqrt();
            // Ring coverage: outside inner radius and inside outer radius
            let inner_coverage = circle_fill_coverage(distance, inner_radius);
            let outer_coverage = circle_fill_coverage(distance, outer_radius);
            // Ring is outer circle minus inner circle
            coverage_sum += (outer_coverage - inner_coverage).max(0.0);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn point_to_segment_distance(
    px: f32,
    py: f32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = px - ax;
    let apy = py - ay;
    let ab_len2 = abx * abx + aby * aby;
    if ab_len2 <= f32::EPSILON {
        let dx = px - ax;
        let dy = py - ay;
        return (dx * dx + dy * dy).sqrt();
    }
    let t = ((apx * abx + apy * aby) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + t * abx;
    let cy = ay + t * aby;
    let dx = px - cx;
    let dy = py - cy;
    (dx * dx + dy * dy).sqrt()
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn line_stroke_coverage_grid(
    px: i32,
    py: i32,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    half_width: f32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let total = sample_count * sample_count;
    let mut coverage_sum = 0.0f32;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = px as f32 + (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = py as f32 + (sy as f32 + 0.5) / sample_count as f32;
            let distance = point_to_segment_distance(sample_x, sample_y, ax, ay, bx, by);
            coverage_sum += (half_width + 0.5 - distance).clamp(0.0, 1.0);
        }
    }
    (coverage_sum / total as f32).clamp(0.0, 1.0)
}
pub(crate) fn cluster_ends_with_zwj(cluster: &TextCluster) -> bool {
    cluster.text.chars().last().map(|ch| ch == '\u{200D}').unwrap_or(false)
}
pub(crate) fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}
pub(crate) fn is_variation_selector(ch: char) -> bool {
    matches!(ch as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}
pub(crate) fn is_wide_scalar(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1100..=0x115F
            | 0x2329..=0x232A
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF
    )
}
pub(crate) fn estimate_cluster_advance(cluster: &str, font_size: f32, scale: f32) -> f32 {
    if cluster.trim().is_empty() {
        return (font_size * 0.33 * scale).max(1.0);
    }
    let has_wide = cluster.chars().any(is_wide_scalar);
    let factor = if has_wide { 1.0 } else { 0.6 };
    (font_size * factor * scale).max(1.0)
}
pub(crate) fn rounded_rect_effective_radius(rect: Rect, radius: u32) -> u32 {
    radius.min(rect.width / 2).min(rect.height / 2)
}
pub(crate) fn inset_rect(rect: Rect, inset: i32) -> Rect {
    let x = rect.x + inset;
    let y = rect.y + inset;
    let width = (rect.width as i32 - inset * 2).max(0) as u32;
    let height = (rect.height as i32 - inset * 2).max(0) as u32;
    Rect { x, y, width, height }
}
pub(crate) fn point_in_rounded_rect_f32(px: f32, py: f32, rect: Rect, radius: u32) -> bool {
    if rect.width == 0 || rect.height == 0 {
        return false;
    }
    let left = rect.x as f32;
    let top = rect.y as f32;
    let right = rect.x as f32 + rect.width as f32;
    let bottom = rect.y as f32 + rect.height as f32;
    if px < left || px >= right || py < top || py >= bottom {
        return false;
    }
    let r = rounded_rect_effective_radius(rect, radius) as f32;
    if r <= 0.0 {
        return true;
    }
    if (px >= left + r && px < right - r) || (py >= top + r && py < bottom - r) {
        return true;
    }
    let cx = if px < left + r {
        left + r
    } else if px >= right - r {
        right - r
    } else {
        px
    };
    let cy = if py < top + r {
        top + r
    } else if py >= bottom - r {
        bottom - r
    } else {
        py
    };
    let dx = px - cx;
    let dy = py - cy;
    dx * dx + dy * dy <= r * r
}
pub(crate) fn rounded_rect_coverage(px: i32, py: i32, rect: Rect, radius: u32) -> f32 {
    rounded_rect_coverage_grid(px, py, rect, radius, 2)
}
pub(crate) fn rounded_rect_coverage_grid(
    px: i32,
    py: i32,
    rect: Rect,
    radius: u32,
    grid: u8,
) -> f32 {
    let sample_count = grid.clamp(1, 8) as u32;
    let mut covered = 0u32;
    let total = sample_count * sample_count;
    for sy in 0..sample_count {
        for sx in 0..sample_count {
            let sample_x = (sx as f32 + 0.5) / sample_count as f32;
            let sample_y = (sy as f32 + 0.5) / sample_count as f32;
            if point_in_rounded_rect_f32(px as f32 + sample_x, py as f32 + sample_y, rect, radius) {
                covered += 1;
            }
        }
    }
    covered as f32 / total as f32
}
