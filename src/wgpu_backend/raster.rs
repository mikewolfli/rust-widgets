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
    // Compute the buffer size in `usize` with checked arithmetic and bound it to
    // the largest size a single framebuffer may occupy. In `u32` the product
    // wraps (65536×16384 = 2^32); even in `usize` a 4 GiB framebuffer is not a
    // real render target, so it is refused with an explicit error rather than
    // attempted (N-S-66).
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("framebuffer {width}x{height} pixel count overflows usize"))?;
    let byte_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| format!("framebuffer {width}x{height} RGBA size overflows usize"))?;
    if byte_len > u32::MAX as usize {
        return Err(format!(
            "framebuffer {width}x{height} needs {byte_len} bytes, which exceeds the largest \
             supported framebuffer ({} bytes)",
            u32::MAX
        ));
    }
    let framebuffer = PixelRect { x: 0, y: 0, width, height };
    let mut pixels = vec![0u8; byte_len];

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
                draw_text_cpu_rgba8(&mut pixels, width, *rect, text, *color, clip_rect, blend);
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
                    blend,
                );
            }
            WgpuDrawCommand::FillRoundedRect { rect, radius, color, clip } => {
                if *radius == 0 || rect.width == 0 || rect.height == 0 {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                fill_rounded_rect_cpu_rgba8(
                    &mut pixels,
                    width,
                    *rect,
                    *radius,
                    *color,
                    clip_rect,
                    blend,
                );
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
                    blend,
                );
            }
            WgpuDrawCommand::DrawLine { from, to, color, width: line_width, clip } => {
                if *line_width == 0 {
                    continue;
                }
                let clip_rect = effective_clip(
                    framebuffer,
                    rect_for_line(*from, *to, *line_width),
                    combine(clip, current_clip),
                );
                draw_line_cpu_rgba8(
                    &mut pixels,
                    width,
                    *from,
                    *to,
                    *color,
                    *line_width,
                    clip_rect,
                    blend,
                );
            }
            WgpuDrawCommand::FillCircle { center, radius: r, color, clip } => {
                if *r == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r, 0);
                let clip_rect =
                    effective_clip(framebuffer, circle_bounds, combine(clip, current_clip));
                fill_circle_cpu_rgba8(&mut pixels, width, *center, *r, *color, clip_rect, blend);
            }
            WgpuDrawCommand::DrawCircle { center, radius: r, color, width: circle_width, clip } => {
                if *r == 0 || *circle_width == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r, *circle_width);
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
                    blend,
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
                let arc_bounds = bbox_for_circle(*center, *r, 1);
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
                    blend,
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
                let path_bounds = bbox_for_path(points, *path_width);
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
                    blend,
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
                // The body is drawn where it is visible, but the blur may reach
                // beyond it. The *paint* region is what the caller clipped to (or
                // the whole framebuffer when no clip is set): blurring must never
                // paint outside it (N-S-63).
                let Some(visible) =
                    effective_clip(framebuffer, shadow, combine(clip, current_clip))
                else {
                    continue;
                };
                let paint_region = match combine(clip, current_clip) {
                    Some(region) => match region.intersect(framebuffer) {
                        Some(clamped) => clamped,
                        None => continue,
                    },
                    None => framebuffer,
                };
                // Render the shadow into an *independent* layer and composite that
                // layer back, instead of blitting into the framebuffer and then
                // blurring in place. Blurring in place read the existing scene
                // pixels as source, so it smeared the background under the shadow
                // rather than blurring the shadow itself (N-S-63).
                box_shadow_cpu_rgba8(
                    &mut pixels,
                    width,
                    height,
                    visible,
                    paint_region,
                    *color,
                    *blur_radius,
                    blend,
                );
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width, mode: blend };
    let glyph_w = 8i32;
    let glyph_h = 8i32;
    let columns = (rect.width as i32 / glyph_w).max(1);
    let rows = (rect.height as i32 / glyph_h).max(1);
    let cell = crate::render::text::Cell::new(glyph_w as u32, glyph_h as u32);
    // One scratch for the whole string, reused per glyph: a glyph is rasterised, blended, and
    // overwritten. That is what keeps this path free of per-glyph allocation and of any glyph
    // cache — the same contract the software rasteriser honours (BLUE23 §0A.4, constraint 3).
    let mut coverage = vec![0u8; cell.area()];
    for (char_index, scalar) in text.chars().enumerate() {
        let grid_index = char_index as i32;
        if grid_index >= columns * rows {
            break;
        }
        let col = grid_index % columns;
        let row = grid_index / columns;
        let origin_x = rect.x + col * glyph_w;
        let origin_y = rect.y + row * glyph_h;
        // The glyph's ink comes from the **font stack**, through the same `paint` the software
        // rasteriser blends. This path used to walk the 8x8 table itself, which meant a face added
        // by a feature (a CJK face, a vector face) was invisible to it, and that it fell back to
        // `'?'` for an uncovered character while the other two fell back to the box glyph — the GPU
        // path's idea of "unsupported" being a third answer nobody chose.
        let Some(painted) = crate::render::text::paint_active(scalar, cell, &mut coverage) else {
            continue;
        };
        // The scratch is glyph-sized, so every pixel of it is inside the cell. Coverage arrives as
        // a value per pixel rather than as rectangles, so a face with antialiased ink would be drawn
        // correctly here without a line of new code.
        let _ = painted;
        for py in 0..glyph_h {
            let screen_y = origin_y + py;
            if screen_y < clip_rect.y || screen_y >= clip_rect.bottom() {
                continue;
            }
            for px in 0..glyph_w {
                let screen_x = origin_x + px;
                if screen_x < clip_rect.x || screen_x >= clip_rect.right() {
                    continue;
                }
                let value = coverage[(py * glyph_w + px) as usize];
                if value == 0 {
                    continue;
                }
                if value == 255 {
                    sink.set(screen_x as u32, screen_y as u32, color);
                } else {
                    blend_cpu_rgba8_with_mode(
                        sink.pixels,
                        width,
                        screen_x as u32,
                        screen_y as u32,
                        color,
                        value as f32 / 255.0,
                        blend,
                    );
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width, mode: blend };
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
            sink.set(x as u32, y as u32, color);
        }
    }
}
/// Bounding box of a line segment, *inclusive* of both endpoints and the stroke.
///
/// A pixel is covered when its centre lies in `[x, x+w)` / `[y, y+h)`, so the
/// inclusive extent from `min` to `max` is `max - min + 1`, not `max - min`. The
/// old half-open form left the last row/column of an axis-aligned line out — and
/// a horizontal or vertical line collapsed to a `0`-extent box that intersected
/// nothing, so the whole line vanished (N-S-65). The result is additionally
/// widened by half the line width on each side, and is always at least 1px in
/// each axis so a single point is still drawable.
fn rect_for_line(from: (i32, i32), to: (i32, i32), line_width: u32) -> PixelRect {
    let (min_x, max_x) = (from.0.min(to.0), from.0.max(to.0));
    let (min_y, max_y) = (from.1.min(to.1), from.1.max(to.1));
    // `draw_line_cpu_rgba8` spreads the stroke over `(center - half + 1 ..= center + half)`,
    // so the geometry reaches `half` pixels past the endpoint on each side.
    let half = (line_width as i64 / 2).max(0);
    let x0 = min_x as i64 - half;
    let y0 = min_y as i64 - half;
    // Inclusive extent: `max - min + 1`, plus the stroke on both sides.
    let w = (max_x as i64 - min_x as i64) + 1 + half * 2;
    let h = (max_y as i64 - min_y as i64) + 1 + half * 2;
    PixelRect {
        x: (x0.clamp(i32::MIN as i64, i32::MAX as i64)) as i32,
        y: (y0.clamp(i32::MIN as i64, i32::MAX as i64)) as i32,
        width: w.clamp(1, u32::MAX as i64) as u32,
        height: h.clamp(1, u32::MAX as i64) as u32,
    }
}
/// Bounding box of a circle *including* its stroke width.
///
/// `DrawCircle` strokes `width` pixels outside the given radius, so the box must
/// grow by `width` on every side; using only `2r` (the old behaviour) clipped the
/// outer half of the ring away. The box is also grown by one pixel so the
/// inclusive far edge (`center + r + width`) is actually covered (N-S-65).
fn bbox_for_circle(center: (i32, i32), radius: u32, stroke_width: u32) -> PixelRect {
    // Widened to i64 so a large radius/stroke cannot overflow before clamping.
    let reach = radius as i64 + stroke_width as i64 + 1;
    let r = reach.clamp(0, i32::MAX as i64) as i32;
    PixelRect {
        x: center.0.saturating_sub(r),
        y: center.1.saturating_sub(r),
        width: (r as i64 * 2).clamp(1, u32::MAX as i64) as u32,
        height: (r as i64 * 2).clamp(1, u32::MAX as i64) as u32,
    }
}
/// Bounding box of a polyline, inclusive of both endpoints and the stroke.
///
/// The inclusive extent is `max - min + 1` (so a horizontal or vertical path does
/// not collapse to zero height/width). When `stroke_width > 0` the box is widened
/// by half the stroke on each side, matching how the path is drawn; a degenerate
/// single-point box is forced to at least 1px in each axis so it remains
/// drawable (N-S-65).
fn bbox_for_path(points: &[(i32, i32)], stroke_width: u32) -> PixelRect {
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
    let half = (stroke_width as i64 / 2).max(0);
    let w = (max_x as i64 - min_x as i64) + 1 + half * 2;
    let h = (max_y as i64 - min_y as i64) + 1 + half * 2;
    let x = (min_x as i64 - half).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y = (min_y as i64 - half).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    PixelRect {
        x,
        y,
        width: w.clamp(1, u32::MAX as i64) as u32,
        height: h.clamp(1, u32::MAX as i64) as u32,
    }
}
fn fill_rounded_rect_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    rect: PixelRect,
    radius: u32,
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                sink.set(x as u32, y as u32, color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                sink.set(x as u32, y as u32, color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                    sink.set(thick_x as u32, thick_y as u32, color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                sink.set(x as u32, y as u32, color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                sink.set(x as u32, y as u32, color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
            sink.set(x as u32, y as u32, arc.color);
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
    blend: BlendMode,
) {
    let clip_rect = match clip_rect {
        Some(value) => value,
        None => return,
    };
    if filled && points.len() >= 3 {
        let mut sink = PixelSink { pixels, width: fb_width, mode: blend };
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
                        sink.set(x as u32, y as u32, color);
                    }
                }
            }
        }
    } else {
        // Draw line segments. The sink's borrow of `pixels` has ended with the
        // `if` branch, so the line helper can take the same framebuffer.
        let iter_len = if closed { points.len() } else { points.len() - 1 };
        for i in 0..iter_len {
            let from = points[i];
            let to = points[(i + 1) % points.len()];
            let line_clip = if path_width > 0 {
                let r = rect_for_line(from, to, path_width);
                r.intersect(clip_rect)
            } else {
                None
            };
            draw_line_cpu_rgba8(pixels, fb_width, from, to, color, path_width, line_clip, blend);
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

/// Draws a drop shadow as an independent, blurred layer and composites it back.
///
/// The shadow is rendered into a private RGBA layer the size of the expanded,
/// framebuffer-clamped blur area, blurred there, and only then written into the
/// framebuffer. Two regions matter (N-S-63):
///
/// * `body` — where the shadow's solid rectangle sits (already clipped to the
///   active clip).
/// * `paint_region` — how far the blurred layer is allowed to paint; it is the
///   caller's clip (or the framebuffer when none), so the blur can feather
///   *outside* the body but never outside what the caller clipped to.
///
/// Blurring the layer rather than the framebuffer also means the blur source is
/// only the shadow, not the scene behind it — a shadow over a textured
/// background must not smear that background.
fn box_shadow_cpu_rgba8(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    body: PixelRect,
    paint_region: PixelRect,
    color: Rgba8,
    blur_radius: u32,
    blend: BlendMode,
) {
    if width == 0 || height == 0 {
        return;
    }
    // Nothing to draw if the body does not intersect the framebuffer.
    let Some(visible) = body.intersect(PixelRect { x: 0, y: 0, width, height }) else { return };

    if blur_radius == 0 {
        // No blur: a plain fill of the body, respecting the blend mode.
        fill_rect_cpu_rgba8_blended(pixels, width, visible, color, blend);
        return;
    }

    let r = blur_radius as i64;
    // The layer must cover the body expanded by the blur radius, clamped to the
    // framebuffer, so the ramp has source pixels without reading outside the
    // allocation.
    let lx0 = (visible.x as i64 - r).max(0) as usize;
    let ly0 = (visible.y as i64 - r).max(0) as usize;
    let lx1 = ((visible.right() as i64) + r).clamp(0, width as i64) as usize;
    let ly1 = ((visible.bottom() as i64) + r).clamp(0, height as i64) as usize;
    if lx1 <= lx0 || ly1 <= ly0 {
        return;
    }
    let lw = lx1 - lx0;
    let lh = ly1 - ly0;

    // Independent, transparent shadow layer.
    let mut layer = vec![0u8; lw * lh * 4];
    // Fill the body (clamped to the layer) with the shadow colour.
    let bx0 = (visible.x as i64 - lx0 as i64).max(0) as usize;
    let by0 = (visible.y as i64 - ly0 as i64).max(0) as usize;
    let bx1 = ((visible.right() as i64 - lx0 as i64).clamp(0, lw as i64)) as usize;
    let by1 = ((visible.bottom() as i64 - ly0 as i64).clamp(0, lh as i64)) as usize;
    for y in by0..by1 {
        for x in bx0..bx1 {
            let o = (y * lw + x) * 4;
            layer[o] = color.r;
            layer[o + 1] = color.g;
            layer[o + 2] = color.b;
            layer[o + 3] = color.a;
        }
    }

    // Blur the layer in place (separable moving average, matching the software
    // backend's kernel).
    box_blur_layer_rgba8(&mut layer, lw, lh, blur_radius as usize);

    // Composite the layer back, but only inside `paint_region` (the caller's
    // clip, or the framebuffer): any pixel the caller clipped away must stay
    // exactly as it was (N-S-63). The body is opaque shadow colour, so a
    // source-over composite reads the layer's alpha as coverage.
    let Some(dst) = paint_region.intersect(PixelRect { x: 0, y: 0, width, height }) else { return };
    for y in dst.y..dst.bottom() {
        for x in dst.x..dst.right() {
            let lxo = x as i64 - lx0 as i64;
            let lyo = y as i64 - ly0 as i64;
            if lxo < 0 || lyo < 0 || lxo >= lw as i64 || lyo >= lh as i64 {
                continue;
            }
            let o = (lyo as usize * lw + lxo as usize) * 4;
            let alpha = layer[o + 3];
            if alpha == 0 {
                continue;
            }
            let src = Rgba8 { r: layer[o], g: layer[o + 1], b: layer[o + 2], a: alpha };
            if alpha == 255 {
                blend_pixel_cpu_rgba8(pixels, width, x as u32, y as u32, src, blend);
            } else {
                // Partial coverage from the blur ramp: source-over the scene.
                blend_cpu_rgba8(pixels, width, x as u32, y as u32, src, alpha as f32 / 255.0);
            }
        }
    }
}

/// Box-blurs an RGBA buffer in place using a separable moving average.
///
/// A squared kernel rather than a true Gaussian: it matches the software
/// backend, and being separable (two 1-D passes) keeps it O(radius) per pixel
/// instead of O(radius²). The buffer is a self-contained layer, so the blur
/// never reads the framebuffer (N-S-63).
fn box_blur_layer_rgba8(layer: &mut [u8], width: usize, height: usize, radius: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    let mut scratch = vec![0u8; layer.len()];
    // Horizontal pass: `layer` -> `scratch`.
    for y in 0..height {
        for x in 0..width {
            let lo = x.saturating_sub(radius);
            let hi = (x + radius).min(width - 1);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for kx in lo..=hi {
                let i = (y * width + kx) * 4;
                for c in 0..4 {
                    sums[c] += layer[i + c] as u32;
                }
                count += 1;
            }
            let di = (y * width + x) * 4;
            for c in 0..4 {
                scratch[di + c] = (sums[c] / count.max(1)) as u8;
            }
        }
    }
    // Vertical pass: `scratch` -> `layer`.
    for x in 0..width {
        for y in 0..height {
            let lo = y.saturating_sub(radius);
            let hi = (y + radius).min(height - 1);
            let mut sums = [0u32; 4];
            let mut count = 0u32;
            for ky in lo..=hi {
                let i = (ky * width + x) * 4;
                for c in 0..4 {
                    sums[c] += scratch[i + c] as u32;
                }
                count += 1;
            }
            let di = (y * width + x) * 4;
            for c in 0..4 {
                layer[di + c] = (sums[c] / count.max(1)) as u8;
            }
        }
    }
}

/// Writes pixels into the framebuffer through the rasteriser's active blend
/// mode.
///
/// `SetBlendMode` is sticky and promised to affect *every* following draw, but
/// only rects and gradients honoured it: text, images, rounded rects, lines,
/// circles, arcs and paths all called `set_pixel_cpu_rgba8` directly and thus
/// always wrote source-over. Threading this sink through the helpers gives every
/// variant the same blend behaviour without duplicating the dispatch (N-S-64).
struct PixelSink<'a> {
    pixels: &'a mut [u8],
    width: u32,
    mode: BlendMode,
}

impl PixelSink<'_> {
    /// Writes `color` at `(x, y)`, compositing with the active blend mode.
    ///
    /// `Normal` takes the fast direct-write path, so an unaware caller's output
    /// is byte-for-byte unchanged.
    #[inline]
    fn set(&mut self, x: u32, y: u32, color: Rgba8) {
        blend_pixel_cpu_rgba8(self.pixels, self.width, x, y, color, self.mode);
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

fn blend_cpu_rgba8(pixels: &mut [u8], width: u32, x: u32, y: u32, color: Rgba8, coverage: f32) {
    blend_cpu_rgba8_with_mode(pixels, width, x, y, color, coverage, BlendMode::Normal)
}

/// As [`blend_cpu_rgba8`], but first composites `color` with the destination
/// through `mode`.
///
/// The coverage path is used for anti-aliased glyph edges; without the mode it
/// would bypass `SetBlendMode` entirely (the opaque glyph interiors honour it,
/// but their antialiased edges did not), so a Multiply text draw came out
/// half-applied (N-S-64). The blended colour is computed against the current
/// destination and then source-over composited by `coverage`.
fn blend_cpu_rgba8_with_mode(
    pixels: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    color: Rgba8,
    coverage: f32,
    mode: BlendMode,
) {
    // The same source-over arithmetic `render::blend_pixel` performs, on this module's flat RGBA
    // byte order. It exists because this path blends un-premultiplied coverage from a glyph's
    // paint, which the opaque `set_pixel_cpu_rgba8` cannot express.
    if coverage <= 0.0 {
        return;
    }
    let offset = y as usize * width as usize + x as usize;
    let Some(offset) = offset.checked_mul(4) else {
        return;
    };
    if offset + 3 >= pixels.len() {
        return;
    }
    let src_a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
    if src_a >= 1.0 {
        pixels[offset] = color.r;
        pixels[offset + 1] = color.g;
        pixels[offset + 2] = color.b;
        pixels[offset + 3] = 255;
        return;
    }
    let dst = [pixels[offset], pixels[offset + 1], pixels[offset + 2], pixels[offset + 3]];
    // Apply the blend mode against the destination first, then composite the
    // result by coverage. `Normal` leaves the source unchanged.
    let color = if mode == BlendMode::Normal {
        color
    } else {
        let dest = Rgba8 { r: dst[0], g: dst[1], b: dst[2], a: dst[3] };
        blend_channels(color, dest, mode)
    };
    let mix = |src: u8, dst: u8| (src as f32 * src_a + dst as f32 * (1.0 - src_a)).round() as u8;
    pixels[offset] = mix(color.r, dst[0]);
    pixels[offset + 1] = mix(color.g, dst[1]);
    pixels[offset + 2] = mix(color.b, dst[2]);
    pixels[offset + 3] =
        (dst[3] as f32 + 255.0 * src_a * (1.0 - dst[3] as f32 / 255.0)).round() as u8;
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

    /// N-S-63: a blurred shadow must not paint outside the caller's clip, and must
    /// blur an independent shadow layer rather than the scene behind it.
    #[test]
    fn box_shadow_blur_respects_clip_and_does_not_blur_the_scene() {
        // A two-tone scene: the left half is white, the right half is red.
        let base = vec![
            WgpuDrawCommand::FillRect {
                rect: rect(0, 0, 10, 20),
                color: Rgba8 { r: 255, g: 255, b: 255, a: 255 },
                clip: None,
            },
            WgpuDrawCommand::FillRect {
                rect: rect(10, 0, 10, 20),
                color: Rgba8 { r: 255, g: 0, b: 0, a: 255 },
                clip: None,
            },
        ];
        // The shadow is clipped to the left half, so it may never touch the red
        // right half even though its blur radius would otherwise reach it.
        let mut commands = base;
        commands.push(WgpuDrawCommand::PushClip { rect: rect(0, 0, 10, 20) });
        commands.push(WgpuDrawCommand::BoxShadow {
            rect: rect(5, 5, 4, 4),
            color: Rgba8 { r: 0, g: 0, b: 0, a: 255 },
            offset_x: 0,
            offset_y: 0,
            blur_radius: 4,
            clip: None,
        });
        commands.push(WgpuDrawCommand::PopClip);
        let pixels = rasterize_draw_commands_rgba8(20, 20, &commands).expect("rasterizes");

        // Every pixel in the clipped-away right half must keep its original red.
        for y in 0..20 {
            for x in 10..20 {
                assert_eq!(
                    pixel(&pixels, 20, x, y),
                    [255, 0, 0, 255],
                    "the blur must not paint outside the clip at ({x},{y})"
                );
            }
        }

        // A shadow over a *uniform* background must not blur that background:
        // outside the shadow's feathered footprint the white must be pure.
        let white_far = pixel(&pixels, 20, 0, 19);
        assert_eq!(white_far, [255, 255, 255, 255], "far background stays untouched");
    }

    /// N-S-64: `SetBlendMode` must affect every draw variant, not just rects.
    #[test]
    fn set_blend_mode_applies_to_all_draw_variants() {
        // Multiply against a mid-grey destination must darken for each variant.
        let grey = Rgba8 { r: 128, g: 128, b: 128, a: 255 };
        let base = WgpuDrawCommand::FillRect { rect: rect(0, 0, 16, 16), color: grey, clip: None };

        // (name, command) pairs, each drawing opaque grey onto the grey base.
        let variants: Vec<(&str, WgpuDrawCommand)> = vec![
            (
                "DrawText",
                WgpuDrawCommand::DrawText {
                    rect: rect(0, 0, 16, 8),
                    text: "HH".to_string(),
                    color: grey,
                    clip: None,
                },
            ),
            (
                "DrawImage",
                WgpuDrawCommand::DrawImage {
                    rect: rect(0, 0, 16, 16),
                    rgba8: vec![128, 128, 128, 255].repeat(16 * 16),
                    image_width: 16,
                    image_height: 16,
                    clip: None,
                },
            ),
            (
                "FillRoundedRect",
                WgpuDrawCommand::FillRoundedRect {
                    rect: rect(0, 0, 16, 16),
                    radius: 4,
                    color: grey,
                    clip: None,
                },
            ),
            (
                "StrokeRoundedRect",
                WgpuDrawCommand::StrokeRoundedRect {
                    rect: rect(0, 0, 16, 16),
                    radius: 4,
                    color: grey,
                    thickness: 3,
                    clip: None,
                },
            ),
            (
                "DrawLine",
                WgpuDrawCommand::DrawLine {
                    from: (0, 8),
                    to: (15, 8),
                    color: grey,
                    width: 4,
                    clip: None,
                },
            ),
            (
                "FillCircle",
                WgpuDrawCommand::FillCircle { center: (8, 8), radius: 6, color: grey, clip: None },
            ),
            (
                "DrawCircle",
                WgpuDrawCommand::DrawCircle {
                    center: (8, 8),
                    radius: 6,
                    color: grey,
                    width: 2,
                    clip: None,
                },
            ),
            (
                "DrawArc",
                WgpuDrawCommand::DrawArc {
                    center: (8, 8),
                    radius: 6,
                    start_angle: 0.0,
                    end_angle: 0.0,
                    color: grey,
                    filled: true,
                    clip: None,
                },
            ),
            (
                "DrawPath",
                WgpuDrawCommand::DrawPath {
                    points: vec![(0, 0), (15, 15), (0, 15)],
                    closed: true,
                    color: grey,
                    filled: true,
                    width: 0,
                    clip: None,
                },
            ),
            (
                "BoxShadow",
                WgpuDrawCommand::BoxShadow {
                    rect: rect(0, 0, 16, 16),
                    color: grey,
                    offset_x: 0,
                    offset_y: 0,
                    blur_radius: 0,
                    clip: None,
                },
            ),
        ];

        for (name, command) in variants {
            // Normal: the variant paints the source grey over the grey base.
            let normal = rasterize_draw_commands_rgba8(16, 16, &[base.clone(), command.clone()])
                .unwrap_or_else(|e| panic!("{name} failed under Normal: {e}"));

            // Multiply over the grey base must produce a darker pixel somewhere.
            let multiplied = rasterize_draw_commands_rgba8(
                16,
                16,
                &[
                    base.clone(),
                    WgpuDrawCommand::SetBlendMode { mode: BlendMode::Multiply as u8 },
                    command,
                ],
            )
            .unwrap_or_else(|e| panic!("{name} failed under Multiply: {e}"));

            // Find any pixel the variant touched under Normal, then check it is
            // darker under Multiply — proving the mode reached that variant.
            let mut checked = false;
            for i in 0..16u32 {
                for j in 0..16u32 {
                    if pixel(&normal, 16, i, j) == pixel(&multiplied, 16, i, j) {
                        continue;
                    }
                    let got = pixel(&multiplied, 16, i, j);
                    assert!(got[0] < 128, "{name}: Multiply must darken; at ({i},{j}) got {got:?}");
                    checked = true;
                }
            }
            assert!(checked, "{name} drew nothing that Multiply could change");
        }
    }

    /// N-S-65: line/circle/path bounding boxes must be inclusive and include the
    /// stroke, so lines do not vanish and ring thickness is not clipped.
    #[test]
    fn geometry_bounds_are_inclusive_and_include_stroke() {
        // A horizontal line must be drawable (old bbox had height 0).
        let line = rect_for_line((0, 5), (10, 5), 4);
        assert!(line.height >= 1, "a horizontal line bbox must not collapse to 0 height");
        assert!(line.width >= 11, "the inclusive width covers both endpoints: {}", line.width);
        // Both endpoint orders give the same box.
        assert_eq!(rect_for_line((0, 5), (10, 5), 4), rect_for_line((10, 5), (0, 5), 4));

        // A single point still has a drawable 1px box.
        let point = rect_for_line((3, 7), (3, 7), 1);
        assert!(point.width >= 1 && point.height >= 1);

        // The circle bbox must grow with the stroke width.
        let thin = bbox_for_circle((10, 10), 5, 0);
        let thick = bbox_for_circle((10, 10), 5, 3);
        assert!(thick.width > thin.width, "a thicker ring has a wider bbox");
        assert!(thick.width >= (5 + 3) * 2, "the bbox must reach radius + stroke");

        // A path with a constant y must not collapse to 0 height, and a stroke
        // widens it.
        let flat = bbox_for_path(&[(0, 4), (8, 4), (16, 4)], 0);
        assert!(flat.height >= 1, "a horizontal path must have a drawable height");
        let stroked = bbox_for_path(&[(0, 4), (8, 4), (16, 4)], 4);
        assert!(stroked.height > flat.height, "the stroke widens the path bbox");
    }

    /// A circle drawn with a stroke must actually paint its full ring: the old
    /// `2r` bbox clipped the outer half away.
    #[test]
    fn draw_circle_ring_is_not_clipped_by_its_bbox() {
        // A thick ring near the top-left origin; the bbox must keep the outermost
        // pixels drawable.
        let pixels = rasterize_draw_commands_rgba8(
            40,
            40,
            &[WgpuDrawCommand::DrawCircle {
                center: (20, 20),
                radius: 10,
                color: RED,
                width: 4,
                clip: None,
            }],
        )
        .expect("rasterizes");
        // The rightmost extent of a ring of radius 10 + width 4 is x = 20 + 10 + 4.
        let mut max_x = 0;
        for x in 0..40u32 {
            if pixel(&pixels, 40, x, 20)[3] != 0 {
                max_x = x;
            }
        }
        assert!(max_x >= 33, "the ring's outer edge (x≈34) must be drawn, max_x={max_x}");
    }

    /// N-S-66: an overflowing framebuffer size must be an explicit error, not a
    /// wrapped allocation, and zero dimensions are rejected.
    #[test]
    fn rasterizer_rejects_overflow_and_zero_dimensions() {
        assert!(rasterize_draw_commands_rgba8(0, 10, &[]).is_err());
        assert!(rasterize_draw_commands_rgba8(10, 0, &[]).is_err());
        // 65536 × 16384 × 4 = 2^32, which overflows `u32` (and, on this check,
        // the reported product).  This must be an error before allocation.
        let err = rasterize_draw_commands_rgba8(65536, 16384, &[]).unwrap_err();
        assert!(
            err.contains("exceeds the largest supported framebuffer"),
            "expected an explicit size error, got: {err}"
        );
    }
}
