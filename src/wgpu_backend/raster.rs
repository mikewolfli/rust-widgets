// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CPU rasterization helpers for WGPU backend.
use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
// The blend-mode enum is defined once in the render layer and reused here rather than
// mirrored, so the two backends cannot disagree about the set of modes (principle #54).
use crate::render::BlendMode;
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

    // The stateful half of the command stream. `WgpuDrawCommand` carries a
    // *per-command* `clip` field, so this is what `PushClip` accumulates into; each
    // command's own field is then intersected against it below. Keeping it here
    // rather than in every arm is what makes nested clipping work.
    //
    // There is no translation state to track: unlike `BatchCommand`, the GPU command
    // stream has no `Translate` variant, and geometry arrives already positioned.
    let mut clip_stack: Vec<PixelRect> = Vec::new();
    let mut current_clip: Option<PixelRect> = None;
    // Sticky blend mode, set by `SetBlendMode` and applied by every later command.
    // Starts `Normal`, which is a plain source-over write — the behaviour every
    // command had before the mode was honoured, so existing streams are unchanged.
    let mut blend = BlendMode::Normal;

    for (index, command) in commands.iter().enumerate() {
        match command {
            WgpuDrawCommand::PushClip { rect } => {
                // Intersect rather than replace: a nested clip can only ever shrink
                // the visible area. When the new rect does not overlap the current
                // clip the result is an empty region, which suppresses drawing
                // entirely rather than silently ignoring the inner clip.
                let next = match current_clip {
                    Some(existing) => existing.intersect(*rect).unwrap_or(PixelRect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    }),
                    None => *rect,
                };
                clip_stack.push(current_clip.unwrap_or(framebuffer));
                current_clip = Some(next);
                continue;
            }
            WgpuDrawCommand::PopClip => {
                // An unbalanced pop is a caller bug, not a reason to abort the frame:
                // report it and leave the clip as-is so the remaining commands still
                // render. Silently underflowing would be worse, because every later
                // command would draw unclipped.
                match clip_stack.pop() {
                    Some(restored) => {
                        current_clip = if restored == framebuffer { None } else { Some(restored) };
                    }
                    None => log::warn!(
                        "[wgpu-raster] PopClip without a matching PushClip (command {index}); \
                         the current clip is left unchanged"
                    ),
                }
                continue;
            }
            _ => {}
        }

        // Every drawing arm below intersects its own `clip` field with the running
        // clip, so `effective_clip` takes both.
        match command {
            WgpuDrawCommand::Clear { color } => {
                clear_cpu_rgba8(&mut pixels, *color);
            }
            WgpuDrawCommand::FillRect { rect, color, clip } => {
                let mut draw_rect = match rect.intersect(framebuffer) {
                    Some(value) => value,
                    None => continue,
                };
                match effective_clip(framebuffer, draw_rect, combine(clip, current_clip)) {
                    Some(value) => draw_rect = value,
                    None => continue,
                }
                fill_rect_cpu_rgba8_blended(&mut pixels, width, draw_rect, *color, blend);
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
                    match effective_clip(framebuffer, draw_rect, combine(clip, current_clip)) {
                        Some(value) => draw_rect = value,
                        None => continue,
                    }
                    fill_rect_cpu_rgba8_blended(&mut pixels, width, draw_rect, *color, blend);
                }
            }
            WgpuDrawCommand::DrawText { rect, text, color, clip } => {
                if rect.width == 0 || rect.height == 0 || text.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
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
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
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
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                fill_rounded_rect_cpu_rgba8(&mut pixels, width, *rect, *radius, *color, clip_rect);
            }
            WgpuDrawCommand::StrokeRoundedRect { rect, radius, color, thickness, clip } => {
                if *radius == 0 || *thickness == 0 || rect.width == 0 || rect.height == 0 {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
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
                let clip_rect = effective_clip(
                    framebuffer,
                    rect_for_line(*from, *to),
                    combine(clip, current_clip),
                );
                draw_line_cpu_rgba8(&mut pixels, width, *from, *to, *color, *line_width, clip_rect);
            }
            WgpuDrawCommand::FillCircle { center, radius: r, color, clip } => {
                if *r == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r);
                let clip_rect =
                    effective_clip(framebuffer, circle_bounds, combine(clip, current_clip));
                fill_circle_cpu_rgba8(&mut pixels, width, *center, *r, *color, clip_rect);
            }
            WgpuDrawCommand::DrawCircle { center, radius: r, color, width: circle_width, clip } => {
                if *r == 0 || *circle_width == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r);
                let clip_rect =
                    effective_clip(framebuffer, circle_bounds, combine(clip, current_clip));
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
                let clip_rect =
                    effective_clip(framebuffer, arc_bounds, combine(clip, current_clip));
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
                let clip_rect =
                    effective_clip(framebuffer, path_bounds, combine(clip, current_clip));
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
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                draw_gradient_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    gradient_data,
                    clip_rect,
                    GradientAxis::Vertical,
                    blend,
                );
            }
            WgpuDrawCommand::FillLinearGradient { rect, gradient_data, clip } => {
                if rect.width == 0 || rect.height == 0 || gradient_data.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                // The linear counterpart runs left-to-right. The variant carries no
                // angle, so horizontal is the only direction it can mean; a caller
                // wanting a vertical ramp uses `DrawGradient`.
                draw_gradient_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    gradient_data,
                    clip_rect,
                    GradientAxis::Horizontal,
                    blend,
                );
            }
            WgpuDrawCommand::FillRadialGradient { rect, gradient_data, clip } => {
                if rect.width == 0 || rect.height == 0 || gradient_data.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                // The variant carries no centre or radius, so the geometry is derived
                // from `rect`: the gradient is inscribed in it, centred, with the outer
                // radius reaching the nearest edge. That is the only reading the payload
                // supports (see the note on the variant).
                draw_radial_gradient_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    gradient_data,
                    clip_rect,
                    blend,
                );
            }
            // Handled in the stateful pre-pass above, which updates the clip stack
            // and skips the drawing match entirely.
            WgpuDrawCommand::PushClip { .. } | WgpuDrawCommand::PopClip => {}
            WgpuDrawCommand::SetBlendMode { mode } => {
                // Sticky from here on, like the software backend: it applies to every
                // subsequent draw until changed.
                blend = blend_mode_from_id(*mode);
            }
            WgpuDrawCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, clip } => {
                if rect.width == 0 || rect.height == 0 {
                    continue;
                }
                // The shadow body sits at `rect` offset by (offset_x, offset_y).
                let shadow = PixelRect {
                    x: rect.x.saturating_add(*offset_x),
                    y: rect.y.saturating_add(*offset_y),
                    width: rect.width,
                    height: rect.height,
                };
                let clip_rect = effective_clip(framebuffer, shadow, combine(clip, current_clip));
                let Some(visible) = clip_rect else {
                    continue;
                };
                // Fill the shadow, then blur it. Blurring the *filled* colour (rather
                // than blurring an alpha mask) matches the software backend and keeps
                // the kernel simple: the region is uniform before the pass.
                fill_rect_cpu_rgba8_blended(&mut pixels, width, visible, *color, blend);
                if *blur_radius > 0 {
                    box_blur_region_cpu(&mut pixels, width, height, visible, *blur_radius);
                }
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

/// Intersects a command's own `clip` field with the enclosing `PushClip` region.
///
/// Both may be present, in which case the effective clip is their intersection —
/// that is what nesting means. Returns `None` when neither is set, which callers
/// read as "only the framebuffer bounds".
///
/// A non-overlapping pair yields a zero-area rectangle rather than `None`, so the
/// caller's `intersect` fails and the command is skipped. Mapping it to `None`
/// instead would mean the same thing to `effective_clip`, but making emptiness
/// explicit keeps `None` meaning exactly "no clip requested".
fn combine(own: &Option<PixelRect>, enclosing: Option<PixelRect>) -> Option<PixelRect> {
    match (own, enclosing) {
        (Some(own), Some(enclosing)) => {
            Some(own.intersect(enclosing).unwrap_or(PixelRect { x: 0, y: 0, width: 0, height: 0 }))
        }
        (Some(own), None) => Some(*own),
        (None, enclosing) => enclosing,
    }
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
        // The glyph's rectangles come from `crate::render::glyph_rects` — the same derivation the
        // software rasteriser fills and the SVG backend emits, so a glyph lands on the same pixels
        // in all three renderers. This path used to walk the 8x8 table itself, which meant a face
        // added by a feature (a CJK face, a vector face) was invisible to it, and that it fell back
        // to `'?'` for an uncovered character while the other two fell back to the box glyph — the
        // GPU path's idea of "unsupported" being a third answer nobody chose.
        let rects: Vec<(i32, i32, i32, i32)> =
            crate::render::glyph_rects(scalar, origin_x, origin_y, glyph_w as u32, glyph_h as u32)
                .collect();
        for (x0, y0, x1, y1) in rects {
            for py in y0..y1 {
                for px in x0..x1 {
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
    axis: GradientAxis,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    // gradient_data is treated as a colour-stop table: [r,g,b,a, r,g,b,a, ...]
    // interpolated across the rect along `axis`.
    let stops = gradient_data.len() / 4;
    if stops == 0 {
        return;
    }
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());

    // The span the ramp is spread over, and how far along it a coordinate is.
    // Both are in `f64` because the ratio is then scaled by the stop count; doing
    // it in `f32` loses the low bits on wide rectangles.
    let span = match axis {
        GradientAxis::Vertical => (rect.height as f64 - 1.0).max(1.0),
        GradientAxis::Horizontal => (rect.width as f64 - 1.0).max(1.0),
    };
    let origin = match axis {
        GradientAxis::Vertical => rect.y as f64,
        GradientAxis::Horizontal => rect.x as f64,
    };

    for y in y_start..y_end {
        for x in x_start..x_end {
            let coordinate = match axis {
                GradientAxis::Vertical => y as f64,
                GradientAxis::Horizontal => x as f64,
            };
            let t = ((coordinate - origin) / span).clamp(0.0, 1.0);
            let idx = ((stops as f64 - 1.0) * t).clamp(0.0, (stops - 1) as f64) as usize;
            let color = Rgba8 {
                r: gradient_data[idx * 4],
                g: gradient_data[idx * 4 + 1],
                b: gradient_data[idx * 4 + 2],
                a: gradient_data[idx * 4 + 3],
            };
            blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
        }
    }
}

/// Which direction a gradient ramp advances along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GradientAxis {
    /// Top to bottom — what [`WgpuDrawCommand::DrawGradient`] means.
    Vertical,
    /// Left to right — what [`WgpuDrawCommand::FillLinearGradient`] means.
    Horizontal,
}

/// Fills `rect` with a radial ramp of `gradient_data` stop colours.
///
/// The variant carries no centre or radius, so the geometry is derived from `rect`:
/// the ramp is centred in the rectangle and its outer radius reaches the nearest
/// edge, which keeps the whole ellipse inside the requested area. Pixels beyond the
/// outer radius take the **last** stop colour rather than being clipped away, so a
/// radial fill always covers `rect` completely — a partially painted fill would look
/// like a rendering bug at the call site.
fn draw_radial_gradient_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    gradient_data: &[u8],
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let stops = gradient_data.len() / 4;
    if stops == 0 {
        return;
    }
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());

    let centre_x = rect.x as f64 + (rect.width as f64 - 1.0) / 2.0;
    let centre_y = rect.y as f64 + (rect.height as f64 - 1.0) / 2.0;
    // Nearest edge, so the gradient fits inside the rectangle on both axes.
    let radius = ((rect.width as f64).min(rect.height as f64) / 2.0).max(1.0);

    for y in y_start..y_end {
        for x in x_start..x_end {
            let dx = x as f64 - centre_x;
            let dy = y as f64 - centre_y;
            let t = ((dx * dx + dy * dy).sqrt() / radius).clamp(0.0, 1.0);
            let idx = ((stops as f64 - 1.0) * t).clamp(0.0, (stops - 1) as f64) as usize;
            let color = Rgba8 {
                r: gradient_data[idx * 4],
                g: gradient_data[idx * 4 + 1],
                b: gradient_data[idx * 4 + 2],
                a: gradient_data[idx * 4 + 3],
            };
            blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
        }
    }
}

/// Combines a source colour with the destination pixel using `mode`.
///
/// `Normal` writes the source directly, which is what every other draw helper does,
/// so it stays on the fast path and existing streams are unaffected.
fn blend_pixel_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    source: Rgba8,
    mode: BlendMode,
) {
    if mode == BlendMode::Normal {
        set_pixel_cpu_rgba8(pixels, width, x, y, source);
        return;
    }
    // Widened to `usize` before multiplying, for the same reason
    // `set_pixel_cpu_rgba8` documents: in `u32` the product wraps for a large `y`,
    // which in debug panics and in release writes a correctly-in-range but wrong
    // pixel. This is the read-modify-write sibling of that function and had been
    // left on the wrapping form.
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    // `offset + 3` can itself overflow `usize` on a 32-bit target, so the check is
    // on the last written index rather than on the offset.
    let Some(last) = offset.checked_add(3) else {
        return;
    };
    if last >= pixels.len() {
        return;
    }
    let dest = Rgba8 {
        r: pixels[offset],
        g: pixels[offset + 1],
        b: pixels[offset + 2],
        a: pixels[offset + 3],
    };
    let blended = blend_channels(source, dest, mode);
    pixels[offset] = blended.r;
    pixels[offset + 1] = blended.g;
    pixels[offset + 2] = blended.b;
    pixels[offset + 3] = blended.a;
}

/// Maps the numeric blend-mode tag carried by the command stream to its enum.
///
/// The tag is the enum's own `as u8` discriminant, but the stream is plain bytes so
/// a value with no corresponding variant is possible. An unknown tag falls back to
/// [`BlendMode::Normal`] and is logged: substituting a blend mode silently would
/// change pixels the caller asked for, so it is reported rather than guessed.
fn blend_mode_from_id(mode: u8) -> BlendMode {
    const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];
    ALL.iter().copied().find(|m| *m as u8 == mode).unwrap_or_else(|| {
        log::warn!("[wgpu-raster] unknown blend mode tag {mode}; falling back to Normal");
        BlendMode::Normal
    })
}

/// Applies the blend formula to one source/destination pair.
///
/// The separable modes are evaluated per channel over the 0..=1 range. The four
/// non-separable ones (`Hue`, `Saturation`, `Color`, `Luminosity`) cannot be: they
/// are defined in terms of the triple's luminance, so they are handled together in
/// the `Luminosity`-family branch below rather than inside the channel closure.
fn blend_channels(source: Rgba8, dest: Rgba8, mode: BlendMode) -> Rgba8 {
    /// Per-channel separable blend functions, on the 0..=1 scale.
    fn separable(mode: BlendMode, s: f32, d: f32) -> f32 {
        match mode {
            BlendMode::Normal => s,
            BlendMode::Multiply => s * d,
            BlendMode::Screen => s + d - s * d,
            BlendMode::Overlay => {
                if d <= 0.5 {
                    2.0 * s * d
                } else {
                    1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                }
            }
            BlendMode::Darken => s.min(d),
            BlendMode::Lighten => s.max(d),
            BlendMode::ColorDodge => {
                if d >= 1.0 {
                    1.0
                } else {
                    (s / (1.0 - d)).min(1.0)
                }
            }
            BlendMode::ColorBurn => {
                if d <= 0.0 {
                    0.0
                } else {
                    1.0 - ((1.0 - s) / d).min(1.0)
                }
            }
            BlendMode::HardLight => {
                if s <= 0.5 {
                    2.0 * s * d
                } else {
                    1.0 - 2.0 * (1.0 - s) * (1.0 - d)
                }
            }
            BlendMode::SoftLight => {
                // W3C compositing spec: the d <= 0.25 branch is a plain product,
                // above it a softened dodge with a square-root step at d > 0.5.
                if d <= 0.25 {
                    let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                    d - (1.0 - 2.0 * s) * delta
                } else if d <= 0.5 {
                    let delta = ((16.0 * d - 12.0) * d + 4.0) * d;
                    d + (2.0 * s - 1.0) * (delta - d)
                } else {
                    d + (2.0 * s - 1.0) * (d.sqrt() - d)
                }
            }
            BlendMode::Difference => (s - d).abs(),
            BlendMode::Exclusion => s + d - 2.0 * s * d,
            // Reached only for the non-separable four, which `blend_channels`
            // intercepts before calling this.
            BlendMode::Hue | BlendMode::Saturation | BlendMode::Color | BlendMode::Luminosity => s,
        }
    }

    let to_f = |v: u8| v as f32 / 255.0;
    let (sr, sg, sb) = (to_f(source.r), to_f(source.g), to_f(source.b));
    let (dr, dg, db) = (to_f(dest.r), to_f(dest.g), to_f(dest.b));

    let (r, g, b) = match mode {
        // In Photoshop's naming, `Luminosity` takes the *source* luminance with the
        // destination hue/saturation; `Color` takes source hue/saturation with the
        // destination luminance; `Hue` and `Saturation` each take one component.
        BlendMode::Luminosity => set_luminosity((dr, dg, db), luminance(sr, sg, sb)),
        BlendMode::Color => set_luminosity((sr, sg, sb), luminance(dr, dg, db)),
        BlendMode::Hue => {
            set_luminosity(clip_color((sr, sg, sb), luminance(dr, dg, db)), luminance(dr, dg, db))
        }
        BlendMode::Saturation => set_luminosity(
            set_saturation((dr, dg, db), saturation(sr, sg, sb)),
            luminance(dr, dg, db),
        ),
        _ => (separable(mode, sr, dr), separable(mode, sg, dg), separable(mode, sb, db)),
    };

    // Alpha follows `Normal` semantics for every mode: the crate's blend modes
    // describe colour composition, and making them alter coverage as well would
    // change what is drawn, not just how it is coloured.
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba8 { r: q(r), g: q(g), b: q(b), a: source.a }
}

/// Rec. 601 luminance of an RGB triple on the 0..=1 scale.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.3 * r + 0.59 * g + 0.11 * b
}

/// Maximum-minus-minimum of an RGB triple, i.e. the saturation proxy the
/// non-separable blend modes are defined over.
fn saturation(r: f32, g: f32, b: f32) -> f32 {
    r.max(g).max(b) - r.min(g).min(b)
}

/// Rescales `rgb` so its luminance becomes `target`, preserving hue and saturation.
///
/// This is the standard "set luminance" primitive the non-separable modes are built
/// from. A flat triple has no colour to preserve, so every channel takes the target —
/// the usual fallback, and it also avoids dividing by a zero channel spread.
fn set_luminosity(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let current = luminance(r, g, b);
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let spread = max - min;
    if spread.abs() < f32::EPSILON {
        return (target, target, target);
    }
    // Shift the whole triple so its luminance lands on `target`; the offset keeps
    // the channel differences (hue and saturation) unchanged.
    let shift = target - current;
    (r + shift, g + shift, b + shift)
}

/// Pulls each channel of `rgb` back into 0..=1 without changing its luminance.
///
/// Needed after a non-separable blend, which can leave a channel out of range. The
/// comparison is against the triple's own luminance rather than a fixed pivot, so
/// the luminance survives the correction.
fn clip_color(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    if min < 0.0 {
        // Scale about the luminance: everything below it moves up proportionally.
        let denom = target - min;
        if denom.abs() < f32::EPSILON {
            return (target, target, target);
        }
        let adjust = |v: f32| target + (v - target) * (target / denom);
        return (adjust(r), adjust(g), adjust(b));
    }
    if max > 1.0 {
        let denom = max - target;
        if denom.abs() < f32::EPSILON {
            return (target, target, target);
        }
        let adjust = |v: f32| target + (v - target) * ((1.0 - target) / denom);
        return (adjust(r), adjust(g), adjust(b));
    }
    rgb
}

/// Rescales `rgb` to carry the given saturation, preserving luminance.
fn set_saturation(rgb: (f32, f32, f32), target: f32) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    if max <= min {
        // Degenerate (greyscale) input: nothing to scale.
        return rgb;
    }
    let current = saturation(r, g, b);
    let scale = if current.abs() < f32::EPSILON {
        return rgb;
    } else {
        target / current
    };
    let mid = luminance(r, g, b);
    (mid + (r - mid) * scale, mid + (g - mid) * scale, mid + (b - mid) * scale)
}

/// Fills `rect` with `color`, compositing through `mode`.
///
/// The blended counterpart of [`fill_rect_cpu_rgba8`], used where the current blend
/// mode must be honoured.
fn fill_rect_cpu_rgba8_blended(
    pixels: &mut [u8],
    width: u32,
    rect: PixelRect,
    color: Rgba8,
    mode: BlendMode,
) {
    if mode == BlendMode::Normal {
        fill_rect_cpu_rgba8(pixels, width, rect, color);
        return;
    }
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if x < 0 || y < 0 {
                continue;
            }
            blend_pixel_cpu_rgba8(pixels, width, x as u32, y as u32, color, mode);
        }
    }
}

/// Box-blurs `region` in place using a separable moving average.
///
/// A squared kernel rather than a true Gaussian: it is what the software backend
/// uses, so the two paths agree, and it is separable (two 1-D passes) which keeps it
/// O(radius) per pixel instead of O(radius²).
fn box_blur_region_cpu(pixels: &mut [u8], width: u32, height: u32, region: PixelRect, radius: u32) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    // The blur reads neighbours outside `region`, so clamp the working area to the
    // framebuffer first — otherwise the edge rows would read from a smaller buffer
    // and the last row/column would be lost.
    let x0 = region.x.max(0) as usize;
    let y0 = region.y.max(0) as usize;
    let x1 = (region.right().max(0) as usize).min(width as usize);
    let y1 = (region.bottom().max(0) as usize).min(height as usize);
    if x1 <= x0 || y1 <= y0 {
        return;
    }

    let r = radius as usize;
    // Expand by the radius so the ramp at the shadow's edge has source pixels, then
    // clamp to the framebuffer.
    let ex0 = x0.saturating_sub(r);
    let ey0 = y0.saturating_sub(r);
    let ex1 = (x1 + r).min(width as usize);
    let ey1 = (y1 + r).min(height as usize);
    let ew = ex1 - ex0;
    let eh = ey1 - ey0;
    if ew == 0 || eh == 0 {
        return;
    }

    let w = width as usize;
    let mut temp = vec![0u8; ew * eh * 4];
    for y in 0..eh {
        let src = ((ey0 + y) * w + ex0) * 4;
        let dst = y * ew * 4;
        temp[dst..dst + ew * 4].copy_from_slice(&pixels[src..src + ew * 4]);
    }

    // Horizontal pass, reading `temp` and writing `temp`.
    let mut scratch = vec![0u8; ew * eh * 4];
    for y in 0..eh {
        for x in 0..ew {
            let sx = ex0 + x;
            let lo = sx.saturating_sub(r).saturating_sub(ex0);
            let hi = (sx + r).saturating_sub(ex0).min(ew - 1);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for kx in lo..=hi {
                let i = (y * ew + kx) * 4;
                for c in 0..4 {
                    sums[c] += temp[i + c] as u32;
                }
                count += 1;
            }
            let di = (y * ew + x) * 4;
            for c in 0..4 {
                scratch[di + c] = (sums[c] / count.max(1)) as u8;
            }
        }
    }

    // Vertical pass, reading `scratch` and writing back into the framebuffer.
    for x in 0..ew {
        for y in 0..eh {
            let sy = ey0 + y;
            let lo = sy.saturating_sub(r).saturating_sub(ey0);
            let hi = (sy + r).saturating_sub(ey0).min(eh - 1);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for ky in lo..=hi {
                let i = (ky * ew + x) * 4;
                for c in 0..4 {
                    sums[c] += scratch[i + c] as u32;
                }
                count += 1;
            }
            let dst = ((ey0 + y) * w + (ex0 + x)) * 4;
            for c in 0..4 {
                pixels[dst + c] = (sums[c] / count.max(1)) as u8;
            }
        }
    }
}

fn set_pixel_cpu_rgba8(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Rgba8) {
    // Widened to `usize` before multiplying: in `u32` the product wraps for a
    // large `y`, which in debug panics and in release silently writes a
    // correctly-in-range but wrong pixel. The bounds check then makes the
    // remaining out-of-range case (geometry beyond the framebuffer) a no-op
    // instead of a panic, matching the callers' pre-clipping expectation.
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    if offset + 3 >= pixels.len() {
        return;
    }
    pixels[offset] = color.r;
    pixels[offset + 1] = color.g;
    pixels[offset + 2] = color.b;
    pixels[offset + 3] = color.a;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads one pixel as RGBA, failing loudly if the coordinate is outside the image.
    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let offset = ((y * width + x) * 4) as usize;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]]
    }

    const RED: Rgba8 = Rgba8 { r: 255, g: 0, b: 0, a: 255 };

    fn rect(x: i32, y: i32, width: u32, height: u32) -> PixelRect {
        PixelRect { x, y, width, height }
    }

    /// A `PushClip` must actually restrict the commands that follow it.
    ///
    /// This is the regression guard for the capability being advertised as a no-op:
    /// the CPU rasteriser accepted `PushClip`/`PopClip`, drew nothing for them, and
    /// therefore painted *unclipped* — so any caller relying on a clip got pixels
    /// outside the region it asked for.
    #[test]
    fn push_clip_restricts_later_drawing() {
        let commands = vec![
            // A 10x10 red rectangle, clipped to its left half.
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 5, 10) },
            WgpuDrawCommand::FillRect { rect: rect(0, 0, 10, 10), color: RED, clip: None },
            WgpuDrawCommand::PopClip,
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands).expect("rasterizes");

        assert_eq!(pixel(&pixels, 10, 0, 0), [255, 0, 0, 255], "inside the clip must be painted");
        assert_eq!(
            pixel(&pixels, 10, 4, 9),
            [255, 0, 0, 255],
            "clip is inclusive of its last column"
        );
        assert_eq!(
            pixel(&pixels, 10, 5, 0),
            [0, 0, 0, 0],
            "outside the clip must stay untouched — painting here means PushClip was ignored"
        );
    }

    /// `PopClip` must restore the previous region, not clear it entirely.
    #[test]
    fn pop_clip_restores_the_previous_region() {
        let commands = vec![
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 5, 10) },
            WgpuDrawCommand::PopClip,
            WgpuDrawCommand::FillRect { rect: rect(0, 0, 10, 10), color: RED, clip: None },
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands).expect("rasterizes");

        assert_eq!(
            pixel(&pixels, 10, 9, 9),
            [255, 0, 0, 255],
            "after PopClip the whole framebuffer is drawable again"
        );
    }

    /// Nested clips must intersect, so the inner one can only shrink the region.
    #[test]
    fn nested_clips_intersect() {
        let commands = vec![
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 8, 8) },
            WgpuDrawCommand::PushClip { rect: rect(4, 4, 8, 8) },
            WgpuDrawCommand::FillRect { rect: rect(0, 0, 10, 10), color: RED, clip: None },
            WgpuDrawCommand::PopClip,
            WgpuDrawCommand::PopClip,
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands).expect("rasterizes");

        assert_eq!(pixel(&pixels, 10, 4, 4), [255, 0, 0, 255], "the overlap is painted");
        assert_eq!(
            pixel(&pixels, 10, 0, 0),
            [0, 0, 0, 0],
            "the outer-only region must not be painted: a nested clip can only shrink"
        );
        assert_eq!(pixel(&pixels, 10, 9, 9), [0, 0, 0, 0], "outside both clips stays untouched");
    }

    /// A command's own `clip` field and the pushed clip must compose, not override.
    ///
    /// Both mechanisms exist, and `combine` is where they meet; getting it wrong in
    /// either direction silently widens what is drawn.
    #[test]
    fn per_command_clip_composes_with_the_pushed_clip() {
        // Pushed clip is the left half; the command's own clip is the top half.
        // The intersection is the top-left quadrant.
        let commands = vec![
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 5, 10) },
            WgpuDrawCommand::FillRect {
                rect: rect(0, 0, 10, 10),
                color: RED,
                clip: Some(rect(0, 0, 10, 5)),
            },
            WgpuDrawCommand::PopClip,
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands).expect("rasterizes");

        assert_eq!(pixel(&pixels, 10, 1, 1), [255, 0, 0, 255], "in both clips");
        assert_eq!(pixel(&pixels, 10, 7, 1), [0, 0, 0, 0], "outside the pushed clip");
        assert_eq!(pixel(&pixels, 10, 1, 7), [0, 0, 0, 0], "outside the per-command clip");
    }

    /// A push that does not overlap the current clip must draw nothing at all.
    #[test]
    fn a_non_overlapping_push_suppresses_drawing() {
        let commands = vec![
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 4, 4) },
            WgpuDrawCommand::PushClip { rect: rect(6, 6, 4, 4) },
            WgpuDrawCommand::FillRect { rect: rect(0, 0, 10, 10), color: RED, clip: None },
            WgpuDrawCommand::PopClip,
            WgpuDrawCommand::PopClip,
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands).expect("rasterizes");

        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(
                    pixel(&pixels, 10, x, y),
                    [0, 0, 0, 0],
                    "disjoint clips leave an empty region, so nothing may be painted at ({x},{y})"
                );
            }
        }
    }

    /// An unbalanced `PopClip` must not abort the frame or underflow.
    #[test]
    fn an_unbalanced_pop_clip_is_tolerated() {
        let commands = vec![
            WgpuDrawCommand::PopClip,
            WgpuDrawCommand::FillRect { rect: rect(0, 0, 4, 4), color: RED, clip: None },
        ];
        let pixels = rasterize_draw_commands_rgba8(10, 10, &commands)
            .expect("an unbalanced pop is a caller bug, not a reason to fail the frame");

        assert_eq!(
            pixel(&pixels, 10, 0, 0),
            [255, 0, 0, 255],
            "drawing continues unclipped after the stray pop"
        );
    }

    // ── The four commands that used to be accepted-and-ignored ──────────

    /// `SetBlendMode` must actually change how later draws compose.
    ///
    /// The regression guard: the command was accepted and discarded, so the blend
    /// mode had no effect and callers got source-over pixels regardless of what they
    /// asked for.
    #[test]
    fn set_blend_mode_changes_subsequent_composition() {
        // A mid-grey destination, then a mid-grey source over it.
        let grey = Rgba8 { r: 128, g: 128, b: 128, a: 255 };
        let base = WgpuDrawCommand::FillRect { rect: rect(0, 0, 4, 4), color: grey, clip: None };

        // Normal write: the second fill just replaces the first.
        let normal = rasterize_draw_commands_rgba8(
            4,
            4,
            &[
                base.clone(),
                WgpuDrawCommand::FillRect { rect: rect(0, 0, 4, 4), color: grey, clip: None },
            ],
        )
        .expect("rasterizes");
        assert_eq!(pixel(&normal, 4, 0, 0), [128, 128, 128, 255], "Normal is a plain write");

        // Multiply of grey over grey is darker than either input.
        let multiplied = rasterize_draw_commands_rgba8(
            4,
            4,
            &[
                base,
                WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 },
                WgpuDrawCommand::FillRect { rect: rect(0, 0, 4, 4), color: grey, clip: None },
            ],
        )
        .expect("rasterizes");
        let got = pixel(&multiplied, 4, 0, 0);
        assert!(
            got[0] < 128,
            "Multiply must darken; got {got:?}, which means SetBlendMode was ignored"
        );
    }

    /// An unrecognised blend tag must fall back to `Normal`, not to nothing.
    #[test]
    fn an_unknown_blend_mode_falls_back_to_normal() {
        let grey = Rgba8 { r: 200, g: 100, b: 50, a: 255 };
        let pixels = rasterize_draw_commands_rgba8(
            4,
            4,
            &[
                WgpuDrawCommand::SetBlendMode { mode: 250 },
                WgpuDrawCommand::FillRect { rect: rect(0, 0, 4, 4), color: grey, clip: None },
            ],
        )
        .expect("rasterizes");
        assert_eq!(
            pixel(&pixels, 4, 0, 0),
            [200, 100, 50, 255],
            "an unknown tag must still draw, at full source colour"
        );
    }

    /// `FillLinearGradient` must vary **horizontally**, and `DrawGradient`
    /// vertically — the distinction the two variants exist to make.
    #[test]
    fn linear_and_plain_gradients_run_along_different_axes() {
        // Two stops: black then white.
        let ramp = vec![0u8, 0, 0, 255, 255, 255, 255, 255];

        let linear = rasterize_draw_commands_rgba8(
            8,
            8,
            &[WgpuDrawCommand::FillLinearGradient {
                rect: rect(0, 0, 8, 8),
                gradient_data: ramp.clone(),
                clip: None,
            }],
        )
        .expect("rasterizes");
        let left = pixel(&linear, 8, 0, 4)[0];
        let right = pixel(&linear, 8, 7, 4)[0];
        assert!(
            left < 32 && right > 223,
            "a linear gradient must ramp left to right, got left={left} right={right}"
        );
        assert_eq!(
            pixel(&linear, 8, 4, 0)[0],
            pixel(&linear, 8, 4, 7)[0],
            "a linear gradient must be constant down each column"
        );

        let vertical = rasterize_draw_commands_rgba8(
            8,
            8,
            &[WgpuDrawCommand::DrawGradient {
                rect: rect(0, 0, 8, 8),
                gradient_data: ramp,
                clip: None,
            }],
        )
        .expect("rasterizes");
        let top = pixel(&vertical, 8, 4, 0)[0];
        let bottom = pixel(&vertical, 8, 4, 7)[0];
        assert!(
            top < 32 && bottom > 223,
            "DrawGradient must ramp top to bottom, got top={top} bottom={bottom}"
        );
    }

    /// `FillRadialGradient` must paint the whole rect, brightest/darkest in the middle.
    ///
    /// The variant carries no centre or radius, so the ramp is inscribed in `rect`.
    /// The important half of this test is the last assertion: pixels beyond the outer
    /// radius take the final stop rather than being left untouched, so the fill always
    /// covers the area it was given.
    #[test]
    fn radial_gradient_is_centred_and_covers_the_whole_rect() {
        let ramp = vec![0u8, 0, 0, 255, 255, 255, 255, 255];
        let pixels = rasterize_draw_commands_rgba8(
            16,
            16,
            &[WgpuDrawCommand::FillRadialGradient {
                rect: rect(0, 0, 16, 16),
                gradient_data: ramp,
                clip: None,
            }],
        )
        .expect("rasterizes");

        let centre = pixel(&pixels, 16, 8, 8)[0];
        let edge = pixel(&pixels, 16, 0, 0)[0];
        assert!(centre < 64, "the centre takes the first stop, got {centre}");
        assert!(
            edge > 191,
            "a corner is past the outer radius and must take the last stop, got {edge} — \
             a lower value would mean the rect was not fully covered"
        );
    }

    /// `BoxShadow` must darken pixels outside the shadow's own rect, and `blur_radius`
    /// must spread that beyond the rect's bounds.
    #[test]
    fn box_shadow_draws_and_blur_extends_it() {
        let shadow = Rgba8 { r: 0, g: 0, b: 0, a: 255 };
        let base = WgpuDrawCommand::FillRect {
            rect: rect(0, 0, 20, 20),
            color: Rgba8 { r: 255, g: 255, b: 255, a: 255 },
            clip: None,
        };

        // Unblurred: offset by (4,4), so (5,5) is inside it and (0,0) is not.
        let sharp = rasterize_draw_commands_rgba8(
            20,
            20,
            &[
                base.clone(),
                WgpuDrawCommand::BoxShadow {
                    rect: rect(0, 0, 10, 10),
                    color: shadow,
                    offset_x: 4,
                    offset_y: 4,
                    blur_radius: 0,
                    clip: None,
                },
            ],
        )
        .expect("rasterizes");
        assert_eq!(pixel(&sharp, 20, 5, 5), [0, 0, 0, 255], "the shadow body is painted");
        assert_eq!(
            pixel(&sharp, 20, 0, 0),
            [255, 255, 255, 255],
            "without a blur the shadow must not reach its own origin"
        );

        // Blurred: the kernel pulls darkness into the surrounding pixels.
        let blurred = rasterize_draw_commands_rgba8(
            20,
            20,
            &[
                base,
                WgpuDrawCommand::BoxShadow {
                    rect: rect(0, 0, 10, 10),
                    color: shadow,
                    offset_x: 4,
                    offset_y: 4,
                    blur_radius: 3,
                    clip: None,
                },
            ],
        )
        .expect("rasterizes");
        let feather = pixel(&blurred, 20, 2, 2)[0];
        assert!(
            feather < 255,
            "a blur must darken pixels just outside the shadow body, got {feather} — 255 \
             means the blur radius was ignored"
        );
    }
}
