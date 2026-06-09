//! CPU rasterization helpers for WGPU backend.
use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
use font8x8::{UnicodeFonts, BASIC_FONTS};
pub fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
pub fn rasterize_draw_commands_rgba8(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("width/height must be > 0".to_string());
    }
    let framebuffer = PixelRect { x: 0, y: 0, width, height };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for (index, command) in commands.iter().enumerate() {
        match command {
            WgpuDrawCommand::Clear { color } => {
                clear_cpu_rgba8(&mut pixels, *color);
            }
            WgpuDrawCommand::FillRect { rect, color, clip } => {
                let mut draw_rect = match rect.intersect(framebuffer) {
                    Some(value) => value,
                    None => continue,
                };
                if let Some(clip_rect) = clip {
                    draw_rect = match draw_rect.intersect(*clip_rect) {
                        Some(value) => value,
                        None => continue,
                    };
                }
                fill_rect_cpu_rgba8(&mut pixels, width, draw_rect, *color);
            }
            WgpuDrawCommand::StrokeRect { rect, color, thickness, clip } => {
                if *thickness == 0 {
                    continue;
                }
                for edge_rect in stroke_rect_edges(*rect, *thickness) {
                    let mut draw_rect = match edge_rect.intersect(framebuffer) {
                        Some(value) => value,
                        None => continue,
                    };
                    if let Some(clip_rect) = clip {
                        draw_rect = match draw_rect.intersect(*clip_rect) {
                            Some(value) => value,
                            None => continue,
                        };
                    }
                    fill_rect_cpu_rgba8(&mut pixels, width, draw_rect, *color);
                }
            }
            WgpuDrawCommand::DrawText { rect, text, color, clip } => {
                if rect.width == 0 || rect.height == 0 || text.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_text_cpu_rgba8(&mut pixels, width, *rect, text, *color, clip_rect);
            }
            WgpuDrawCommand::DrawImage { rect, rgba8, image_width, image_height, clip } => {
                if rect.width == 0 || rect.height == 0 || *image_width == 0 || *image_height == 0 {
                    continue;
                }
                if rgba8.len() != (*image_width as usize) * (*image_height as usize) * 4 {
                    return Err(format!(
                        "invalid DrawImage payload at index {index}: expected {} bytes, got {}",
                        (*image_width as usize) * (*image_height as usize) * 4,
                        rgba8.len()
                    ));
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_image_scaled_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    rgba8,
                    *image_width,
                    *image_height,
                    clip_rect,
                );
            }
            WgpuDrawCommand::FillRoundedRect { rect, radius, color, clip } => {
                if *radius == 0 || rect.width == 0 || rect.height == 0 {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                fill_rounded_rect_cpu_rgba8(&mut pixels, width, *rect, *radius, *color, clip_rect);
            }
            WgpuDrawCommand::StrokeRoundedRect { rect, radius, color, thickness, clip } => {
                if *radius == 0 || *thickness == 0 || rect.width == 0 || rect.height == 0 {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                stroke_rounded_rect_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    *radius,
                    *color,
                    *thickness,
                    clip_rect,
                );
            }
            WgpuDrawCommand::DrawLine { from, to, color, width: line_width, clip } => {
                if *line_width == 0 {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, rect_for_line(*from, *to), *clip);
                draw_line_cpu_rgba8(&mut pixels, width, *from, *to, *color, *line_width, clip_rect);
            }
            WgpuDrawCommand::FillCircle { center, radius: r, color, clip } => {
                if *r == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r);
                let clip_rect = effective_clip(framebuffer, circle_bounds, *clip);
                fill_circle_cpu_rgba8(&mut pixels, width, *center, *r, *color, clip_rect);
            }
            WgpuDrawCommand::DrawCircle { center, radius: r, color, width: circle_width, clip } => {
                if *r == 0 || *circle_width == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r);
                let clip_rect = effective_clip(framebuffer, circle_bounds, *clip);
                draw_circle_cpu_rgba8(
                    &mut pixels,
                    width,
                    *center,
                    *r,
                    *color,
                    *circle_width,
                    clip_rect,
                );
            }
            WgpuDrawCommand::DrawArc {
                center,
                radius: r,
                start_angle,
                end_angle,
                color,
                filled,
                clip,
            } => {
                if *r == 0 {
                    continue;
                }
                let arc_bounds = bbox_for_circle(*center, *r);
                let clip_rect = effective_clip(framebuffer, arc_bounds, *clip);
                draw_arc_cpu_rgba8(
                    &mut pixels,
                    width,
                    &ArcParams {
                        center: *center,
                        radius: *r,
                        start_angle: *start_angle,
                        end_angle: *end_angle,
                        color: *color,
                        filled: *filled,
                    },
                    clip_rect,
                );
            }
            WgpuDrawCommand::DrawPath {
                points,
                closed,
                color,
                filled,
                width: path_width,
                clip,
            } => {
                if points.len() < 2 {
                    continue;
                }
                let path_bounds = bbox_for_path(points);
                let clip_rect = effective_clip(framebuffer, path_bounds, *clip);
                draw_path_cpu_rgba8(
                    &mut pixels,
                    width,
                    points,
                    *closed,
                    *color,
                    *filled,
                    *path_width,
                    clip_rect,
                );
            }
            WgpuDrawCommand::DrawGradient { rect, gradient_data, clip } => {
                if rect.width == 0 || rect.height == 0 || gradient_data.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, *clip);
                draw_gradient_cpu_rgba8(&mut pixels, width, *rect, gradient_data, clip_rect);
            }
            WgpuDrawCommand::PushClip { .. } | WgpuDrawCommand::PopClip => {
                // Clip stack management is not yet supported in the CPU rasterizer.
                // These are accepted as no-ops.
            }
            WgpuDrawCommand::FillLinearGradient { .. }
            | WgpuDrawCommand::FillRadialGradient { .. } => {
                // Gradient commands — defer to DrawGradient path in production
            }
            WgpuDrawCommand::SetBlendMode { .. } => {
                // Blend mode — not yet supported in CPU rasterizer
            }
            WgpuDrawCommand::BoxShadow { .. } => {
                // Box shadow — not yet supported in CPU rasterizer
            }
        }
    }
    Ok(pixels)
}
fn effective_clip(
    framebuffer: PixelRect,
    rect: PixelRect,
    clip: Option<PixelRect>,
) -> Option<PixelRect> {
    let mut clipped = rect.intersect(framebuffer)?;
    if let Some(clip_rect) = clip {
        clipped = clipped.intersect(clip_rect)?;
    }
    Some(clipped)
}
fn clear_cpu_rgba8(pixels: &mut [u8], color: Rgba8) {
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = color.r;
        chunk[1] = color.g;
        chunk[2] = color.b;
        chunk[3] = color.a;
    }
}
fn fill_rect_cpu_rgba8(pixels: &mut [u8], width: u32, rect: PixelRect, color: Rgba8) {
    let row_bytes = width as usize * 4;
    let x_start = rect.x as usize;
    let y_start = rect.y as usize;
    let x_end = (rect.x as usize) + rect.width as usize;
    let y_end = (rect.y as usize) + rect.height as usize;
    for y in y_start..y_end {
        let row_start = y * row_bytes;
        for x in x_start..x_end {
            let offset = row_start + (x * 4);
            pixels[offset] = color.r;
            pixels[offset + 1] = color.g;
            pixels[offset + 2] = color.b;
            pixels[offset + 3] = color.a;
        }
    }
}
fn stroke_rect_edges(rect: PixelRect, thickness: u32) -> [PixelRect; 4] {
    let t = thickness.min(rect.width).min(rect.height);
    [
        PixelRect { x: rect.x, y: rect.y, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.bottom() - t as i32, width: rect.width, height: t },
        PixelRect { x: rect.x, y: rect.y, width: t, height: rect.height },
        PixelRect { x: rect.right() - t as i32, y: rect.y, width: t, height: rect.height },
    ]
}
fn draw_text_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    text: &str,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let glyph_w = 8i32;
    let glyph_h = 8i32;
    let columns = (rect.width as i32 / glyph_w).max(1);
    let rows = (rect.height as i32 / glyph_h).max(1);
    for (char_index, scalar) in text.chars().enumerate() {
        let grid_index = char_index as i32;
        if grid_index >= columns * rows {
            break;
        }
        let col = grid_index % columns;
        let row = grid_index / columns;
        let origin_x = rect.x + col * glyph_w;
        let origin_y = rect.y + row * glyph_h;
        let glyph = BASIC_FONTS.get(scalar).or_else(|| BASIC_FONTS.get('?')).unwrap_or([0; 8]);
        for (gy, bits) in glyph.iter().enumerate() {
            for gx in 0..8 {
                if ((bits >> gx) & 1) == 0 {
                    continue;
                }
                let px = origin_x + gx;
                let py = origin_y + gy as i32;
                if px < clip_rect.x
                    || py < clip_rect.y
                    || px >= clip_rect.right()
                    || py >= clip_rect.bottom()
                {
                    continue;
                }
                set_pixel_cpu_rgba8(pixels, width, px as u32, py as u32, color);
            }
        }
    }
}
fn draw_image_scaled_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    source_rgba8: &[u8],
    source_width: u32,
    source_height: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let x_start = clip_rect.x;
    let y_start = clip_rect.y;
    let x_end = clip_rect.right();
    let y_end = clip_rect.bottom();
    for y in y_start..y_end {
        let local_y = (y - rect.y) as u32;
        let src_y = ((local_y as u64 * source_height as u64) / rect.height as u64)
            .min(source_height.saturating_sub(1) as u64) as u32;
        for x in x_start..x_end {
            let local_x = (x - rect.x) as u32;
            let src_x = ((local_x as u64 * source_width as u64) / rect.width as u64)
                .min(source_width.saturating_sub(1) as u64) as u32;
            let src_offset = ((src_y * source_width + src_x) * 4) as usize;
            let color = Rgba8 {
                r: source_rgba8[src_offset],
                g: source_rgba8[src_offset + 1],
                b: source_rgba8[src_offset + 2],
                a: source_rgba8[src_offset + 3],
            };
            set_pixel_cpu_rgba8(pixels, width, x as u32, y as u32, color);
        }
    }
}
fn rect_for_line(from: (i32, i32), to: (i32, i32)) -> PixelRect {
    let x = from.0.min(to.0);
    let y = from.1.min(to.1);
    let w = (from.0.max(to.0) - x).unsigned_abs();
    let h = (from.1.max(to.1) - y).unsigned_abs();
    PixelRect { x, y, width: w.max(1), height: h.max(1) }
}
fn bbox_for_circle(center: (i32, i32), radius: u32) -> PixelRect {
    let r = radius as i32;
    PixelRect { x: center.0 - r, y: center.1 - r, width: (r * 2) as u32, height: (r * 2) as u32 }
}
fn bbox_for_path(points: &[(i32, i32)]) -> PixelRect {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for &(px, py) in points {
        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
    }
    if min_x > max_x || min_y > max_y {
        return PixelRect { x: 0, y: 0, width: 0, height: 0 };
    }
    PixelRect { x: min_x, y: min_y, width: (max_x - min_x) as u32, height: (max_y - min_y) as u32 }
}
fn fill_rounded_rect_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    radius: u32,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let r = radius.min(rect.width / 2).min(rect.height / 2) as i32;
    let r2 = r * r;
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    for y in y_start..y_end {
        for x in x_start..x_end {
            // Check if pixel is inside the rounded rect:
            // top-left corner
            let inside = if x < rect.x + r && y < rect.y + r {
                let dx = rect.x + r - x;
                let dy = rect.y + r - y;
                dx * dx + dy * dy <= r2
            } else if x >= rect.right() - r && y < rect.y + r {
                let dx = x - (rect.right() - r - 1);
                let dy = rect.y + r - y;
                dx * dx + dy * dy <= r2
            } else if x < rect.x + r && y >= rect.bottom() - r {
                let dx = rect.x + r - x;
                let dy = y - (rect.bottom() - r - 1);
                dx * dx + dy * dy <= r2
            } else if x >= rect.right() - r && y >= rect.bottom() - r {
                let dx = x - (rect.right() - r - 1);
                let dy = y - (rect.bottom() - r - 1);
                dx * dx + dy * dy <= r2
            } else {
                true
            };
            if inside {
                set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
            }
        }
    }
}
#[allow(clippy::nonminimal_bool)]
fn stroke_rounded_rect_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    radius: u32,
    color: Rgba8,
    thickness: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let r = radius.min(rect.width / 2).min(rect.height / 2) as i32;
    let t = thickness as i32;
    let outer_r2 = (r + t) * (r + t);
    let inner_r2 = (r - t).max(0) * (r - t).max(0);
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    for y in y_start..y_end {
        for x in x_start..x_end {
            let in_corner_zone = (x < rect.x + r + t && y < rect.y + r + t)
                || (x >= rect.right() - r - t && y < rect.y + r + t)
                || (x < rect.x + r + t && y >= rect.bottom() - r - t)
                || (x >= rect.right() - r - t && y >= rect.bottom() - r - t);
            let visible = if in_corner_zone {
                // Four corner quadrants
                let (cx, cy) = if x < rect.x + r + t && y < rect.y + r + t {
                    (rect.x + r, rect.y + r)
                } else if x >= rect.right() - r - t && y < rect.y + r + t {
                    (rect.right() - r - 1, rect.y + r)
                } else if x < rect.x + r + t && y >= rect.bottom() - r - t {
                    (rect.x + r, rect.bottom() - r - 1)
                } else {
                    (rect.right() - r - 1, rect.bottom() - r - 1)
                };
                let dx = x - cx;
                let dy = y - cy;
                let d2 = dx * dx + dy * dy;
                d2 <= outer_r2 && d2 >= inner_r2
            } else {
                // Straight edge sections
                let on_top = y < rect.y + r + t && y >= rect.y + r;
                let on_bottom = y >= rect.bottom() - r - t && y < rect.bottom() - r;
                let on_left = x < rect.x + r + t && x >= rect.x + r;
                let on_right = x >= rect.right() - r - t && x < rect.right() - r;
                (on_top || on_bottom) && x >= rect.x + r && x < rect.right() - r
                    || (on_left || on_right) && y >= rect.y + r && y < rect.bottom() - r
            };
            if visible {
                set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
            }
        }
    }
}
fn draw_line_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    from: (i32, i32),
    to: (i32, i32),
    color: Rgba8,
    line_width: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let (mut x0, mut y0) = from;
    let (x1, y1) = to;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let half_w = (line_width as i32 / 2).max(1);
    loop {
        // Draw a small square around each point for thickness
        for thick_y in (y0 - half_w + 1)..=(y0 + half_w) {
            for thick_x in (x0 - half_w + 1)..=(x0 + half_w) {
                if thick_x >= clip_rect.x
                    && thick_x < clip_rect.right()
                    && thick_y >= clip_rect.y
                    && thick_y < clip_rect.bottom()
                {
                    set_pixel_cpu_rgba8(pixels, fb_width, thick_x as u32, thick_y as u32, color);
                }
            }
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}
fn fill_circle_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    center: (i32, i32),
    radius: u32,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let r = radius as i32;
    let r2 = r * r;
    let x_start = clip_rect.x.max(center.0 - r);
    let y_start = clip_rect.y.max(center.1 - r);
    let x_end = clip_rect.right().min(center.0 + r + 1);
    let y_end = clip_rect.bottom().min(center.1 + r + 1);
    for y in y_start..y_end {
        for x in x_start..x_end {
            let dx = x - center.0;
            let dy = y - center.1;
            if dx * dx + dy * dy <= r2 {
                set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
            }
        }
    }
}
fn draw_circle_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    center: (i32, i32),
    radius: u32,
    color: Rgba8,
    width: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let r = radius as i32;
    let outer_r2 = (r + width as i32) * (r + width as i32);
    let inner_r2 = (r - width as i32).max(0) * (r - width as i32).max(0);
    let x_start = clip_rect.x.max(center.0 - r - width as i32);
    let y_start = clip_rect.y.max(center.1 - r - width as i32);
    let x_end = clip_rect.right().min(center.0 + r + width as i32 + 1);
    let y_end = clip_rect.bottom().min(center.1 + r + width as i32 + 1);
    for y in y_start..y_end {
        for x in x_start..x_end {
            let dx = x - center.0;
            let dy = y - center.1;
            let d2 = dx * dx + dy * dy;
            if d2 <= outer_r2 && d2 >= inner_r2 {
                set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
            }
        }
    }
}
/// Arc drawing parameters for `draw_arc_cpu_rgba8`.
pub(crate) struct ArcParams {
    /// Center of the arc.
    pub center: (i32, i32),
    /// Radius of the arc.
    pub radius: u32,
    /// Start angle in radians.
    pub start_angle: f32,
    /// End angle in radians.
    pub end_angle: f32,
    /// Fill color.
    pub color: Rgba8,
    /// Whether the arc is filled (vs stroked).
    pub filled: bool,
}

fn draw_arc_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    arc: &ArcParams,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let r = arc.radius as i32;
    let r2 = r * r;
    let x_start = clip_rect.x.max(arc.center.0 - r);
    let y_start = clip_rect.y.max(arc.center.1 - r);
    let x_end = clip_rect.right().min(arc.center.0 + r + 1);
    let y_end = clip_rect.bottom().min(arc.center.1 + r + 1);
    let (sa, ea) = if (arc.start_angle - arc.end_angle).abs() < 0.001 {
        // Full circle
        (0.0, std::f32::consts::TAU)
    } else {
        let sa = arc.start_angle.rem_euclid(std::f32::consts::TAU);
        let mut ea = arc.end_angle.rem_euclid(std::f32::consts::TAU);
        // Normalise to a [sa, sa+TAU) range so ea > sa
        if ea <= sa {
            ea += std::f32::consts::TAU;
        }
        (sa, ea)
    };
    let is_full_circle = (ea - sa) >= std::f32::consts::TAU - 0.001;
    for y in y_start..y_end {
        for x in x_start..x_end {
            let dx = x - arc.center.0;
            let dy = y - arc.center.1;
            let d2 = dx * dx + dy * dy;
            let inside_circle = if arc.filled {
                d2 <= r2
            } else {
                let outer_r2 = (r + 1) * (r + 1);
                let inner_r2 = (r - 1).max(0) * (r - 1).max(0);
                d2 <= outer_r2 && d2 >= inner_r2
            };
            if !inside_circle {
                continue;
            }
            if !is_full_circle {
                let angle = (dy as f64).atan2(dx as f64) as f32;
                let angle = angle.rem_euclid(std::f32::consts::TAU);
                let angle = if angle < sa { angle + std::f32::consts::TAU } else { angle };
                if angle < sa || angle > ea {
                    continue;
                }
            }
            set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, arc.color);
        }
    }
}
fn draw_path_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    points: &[(i32, i32)],
    closed: bool,
    color: Rgba8,
    filled: bool,
    path_width: u32,
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    if filled && points.len() >= 3 {
        // Simple scanline fill for convex polygons
        let min_y = clip_rect.y.max(points.iter().map(|&(_, y)| y).min().unwrap_or(0));
        let max_y = clip_rect.bottom().min(points.iter().map(|&(_, y)| y).max().unwrap_or(0) + 1);
        let n = points.len();
        for y in min_y..max_y {
            let mut intersections = Vec::new();
            for i in 0..n {
                let (x1, y1) = points[i];
                let (x2, y2) = points[(i + 1) % n];
                if (y1 <= y && y2 > y) || (y2 <= y && y1 > y) {
                    let t = (y - y1) as f64 / (y2 - y1) as f64;
                    let ix = x1 as f64 + t * (x2 - x1) as f64;
                    intersections.push(ix as i32);
                }
            }
            intersections.sort_unstable();
            for pair in intersections.chunks(2) {
                if pair.len() == 2 {
                    let x_start = pair[0].max(clip_rect.x);
                    let x_end = pair[1].min(clip_rect.right());
                    for x in x_start..x_end {
                        set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
                    }
                }
            }
        }
    } else {
        // Draw line segments
        let iter_len = if closed { points.len() } else { points.len() - 1 };
        for i in 0..iter_len {
            let from = points[i];
            let to = points[(i + 1) % points.len()];
            let line_clip = if path_width > 0 {
                let r = rect_for_line(from, to);
                r.intersect(clip_rect)
            } else {
                None
            };
            draw_line_cpu_rgba8(pixels, fb_width, from, to, color, path_width, line_clip);
        }
    }
}
fn draw_gradient_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    gradient_data: &[u8],
    clip_rect: Option<PixelRect>,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    // gradient_data is treated as a colour-stop table: [r,g,b,a, r,g,b,a, ...]
    // linearly interpolated from top to bottom of the rect
    let stops = gradient_data.len() / 4;
    if stops == 0 {
        return;
    }
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    for y in y_start..y_end {
        let t =
            if rect.height <= 1 { 0.0 } else { ((y - rect.y) as f64) / ((rect.height - 1) as f64) };
        let idx = ((stops as f64 - 1.0) * t).clamp(0.0, (stops - 1) as f64) as usize;
        let color = Rgba8 {
            r: gradient_data[idx * 4],
            g: gradient_data[idx * 4 + 1],
            b: gradient_data[idx * 4 + 2],
            a: gradient_data[idx * 4 + 3],
        };
        for x in x_start..x_end {
            set_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color);
        }
    }
}
fn set_pixel_cpu_rgba8(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Rgba8) {
    let offset = ((y * width + x) * 4) as usize;
    pixels[offset] = color.r;
    pixels[offset + 1] = color.g;
    pixels[offset + 2] = color.b;
    pixels[offset + 3] = color.a;
}
