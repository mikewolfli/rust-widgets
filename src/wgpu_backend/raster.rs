// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CPU rasterization helpers for WGPU backend.
use super::commands::WgpuDrawCommand;
use super::types::{PixelRect, Rgba8};
use std::cell::RefCell;

thread_local! {
    /// Reusable shadow layer buffer. Shadows are drawn per command, so a per-command zeroed
    /// layer allocation made every shadowed control pay twice (G-11). Fully rewritten on each
    /// use, so a same-sized buffer needs no clear (G-67).
    static SHADOW_LAYER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Reusable horizontal-pass buffer for [`box_blur_layer_rgba8`]. Separate from the layer so
    /// a nested shadow still has its own scratch; reused at the same size without a zero fill
    /// because the pass overwrites every pixel (G-67).
    static SHADOW_SCRATCH: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Reusable glyph coverage scratch for [`draw_text_cpu_rgba8`], retained across text
    /// commands instead of a per-command allocation (G-222). It is re-zeroed per command, so a
    /// reused buffer starts exactly where the fresh `vec![0u8; n]` it replaced did.
    static TEXT_COVERAGE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
// The blend-mode enum is defined once in the render layer and reused here rather than
// mirrored, so the two backends cannot disagree about the set of modes (principle #54).
use crate::render::BlendMode;
pub fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}
/// Byte length of a `width × height` RGBA8 frame, with the overflow and 4 GiB bound checks shared
/// by both entry points (N-S-66).
fn framebuffer_byte_len(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err("width/height must be > 0".to_string());
    }
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
    Ok(byte_len)
}

/// Rasterises `commands` into a caller-owned frame of exactly `width × height × 4` bytes.
///
/// The frame is zero-filled first: the picture starts from transparent black, and a reused
/// caller buffer cannot be assumed cleared (G-221). Rendering into caller storage is what lets
/// a renderer carry one framebuffer across frames; [`rasterize_draw_commands_rgba8`] is the
/// owning wrapper for callers that need the bytes back.
pub fn rasterize_draw_commands_into_rgba8(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
    pixels: &mut [u8],
) -> Result<(), String> {
    let byte_len = framebuffer_byte_len(width, height)?;
    if pixels.len() != byte_len {
        return Err(format!(
            "frame buffer is {} bytes but a {width}x{height} frame needs {byte_len}",
            pixels.len()
        ));
    }
    pixels.fill(0);
    let framebuffer = PixelRect { x: 0, y: 0, width, height };

    // The stateful half of the command stream. `WgpuDrawCommand` carries a *per-command*
    // `clip` field, so this is what `PushClip` accumulates into and what each command's own
    // field is intersected against below — keeping it here is what makes nested clipping
    // work. There is no translation state: the GPU stream has no `Translate` variant.
    let mut clip_stack: Vec<PixelRect> = Vec::new();
    let mut current_clip: Option<PixelRect> = None;
    // Sticky blend mode, set by `SetBlendMode` and applied by every later command. Starts
    // `Normal` — a plain source-over write, so existing streams are unchanged.
    let mut blend = BlendMode::Normal;

    for (index, command) in commands.iter().enumerate() {
        match command {
            WgpuDrawCommand::PushClip { rect } => {
                // Intersect rather than replace: a nested clip can only shrink the visible
                // area, and a non-overlapping rect becomes an empty region that suppresses
                // drawing rather than silently ignoring the inner clip.
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
                // An unbalanced pop is a caller bug, not a reason to abort the frame: report
                // it and leave the clip as-is, because silently underflowing would draw
                // every later command unclipped.
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

        // Every drawing arm intersects its own `clip` field with the running clip.
        match command {
            WgpuDrawCommand::Clear { color } => {
                clear_cpu_rgba8(&mut *pixels, *color);
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
                fill_rect_cpu_rgba8_blended(&mut *pixels, width, draw_rect, *color, blend);
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
                    fill_rect_cpu_rgba8_blended(&mut *pixels, width, draw_rect, *color, blend);
                }
            }
            WgpuDrawCommand::DrawText { rect, text, color, clip } => {
                if rect.width == 0 || rect.height == 0 || text.is_empty() {
                    continue;
                }
                let clip_rect = effective_clip(framebuffer, *rect, combine(clip, current_clip));
                draw_text_cpu_rgba8(&mut *pixels, width, *rect, text, *color, clip_rect, blend);
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
                    &mut *pixels,
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
                    &mut *pixels,
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
                    &mut *pixels,
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
                // Clamp the segment to the clip before stepping: a degenerate endpoint far
                // outside the framebuffer used to walk billions of no-op Bresenham steps
                // (and overflow the i32 error term) before reaching the visible region.
                let Some(clip_rect) = clip_rect else { continue };
                let Some((clipped_from, clipped_to)) = clip_segment_to_rect(*from, *to, &clip_rect)
                else {
                    continue;
                };
                draw_line_cpu_rgba8(
                    &mut *pixels,
                    width,
                    clipped_from,
                    clipped_to,
                    *color,
                    *line_width,
                    Some(clip_rect),
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
                fill_circle_cpu_rgba8(&mut *pixels, width, *center, *r, *color, clip_rect, blend);
            }
            WgpuDrawCommand::DrawCircle { center, radius: r, color, width: circle_width, clip } => {
                if *r == 0 || *circle_width == 0 {
                    continue;
                }
                let circle_bounds = bbox_for_circle(*center, *r, *circle_width);
                let clip_rect =
                    effective_clip(framebuffer, circle_bounds, combine(clip, current_clip));
                draw_circle_cpu_rgba8(
                    &mut *pixels,
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
                    &mut *pixels,
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
                    &mut *pixels,
                    width,
                    CpuPathDraw {
                        points,
                        closed: *closed,
                        color: *color,
                        filled: *filled,
                        path_width: *path_width,
                        clip_rect,
                        blend,
                    },
                );
            }
            WgpuDrawCommand::FillPathNonZero { points, contours, color, clip } => {
                if contours.is_empty() {
                    return Err("FillPathNonZero requires at least one contour".to_string());
                }
                let mut previous_end = 0;
                for (index, &(start, end)) in contours.iter().enumerate() {
                    if start < previous_end || start >= end || end > points.len() || end - start < 3
                    {
                        return Err(format!(
                            "FillPathNonZero contour {index} has invalid range {start}..{end} for {} points",
                            points.len()
                        ));
                    }
                    previous_end = end;
                }
                let path_bounds = bbox_for_path(points, 0);
                let clip_rect =
                    effective_clip(framebuffer, path_bounds, combine(clip, current_clip));
                draw_path_non_zero_cpu_rgba8(
                    &mut *pixels,
                    width,
                    points,
                    contours,
                    *color,
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
                    &mut *pixels,
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
                    &mut *pixels,
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
                    &mut *pixels,
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
                // The blur may reach beyond the body; the *paint* region is what the caller
                // clipped to (or the whole framebuffer), and blurring must never paint
                // outside it (N-S-63).
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
                // The shadow is rendered into an independent layer and composited back;
                // blurring in place read the scene as source and smeared the background
                // under the shadow (N-S-63).
                box_shadow_cpu_rgba8(
                    &mut *pixels,
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
    Ok(())
}

/// Owning wrapper around [`rasterize_draw_commands_into_rgba8`].
pub fn rasterize_draw_commands_rgba8(
    width: u32,
    height: u32,
    commands: &[WgpuDrawCommand],
) -> Result<Vec<u8>, String> {
    let mut pixels = vec![0u8; framebuffer_byte_len(width, height)?];
    rasterize_draw_commands_into_rgba8(width, height, commands, &mut pixels)?;
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

/// Intersects a command's own `clip` field with the enclosing `PushClip` region. `None` means
/// "no clip requested"; a non-overlapping pair becomes a zero-area rect so the draw is skipped
/// rather than silently unclipped.
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
    for pixel in pixels.as_chunks_mut::<4>().0 {
        *pixel = [color.r, color.g, color.b, color.a];
    }
}
fn fill_rect_cpu_rgba8(pixels: &mut [u8], width: u32, rect: PixelRect, color: Rgba8) {
    if width == 0 || rect.width == 0 || rect.height == 0 {
        return;
    }
    // Callers hand this helper a framebuffer-clipped rectangle; an unclipped one must be a
    // no-op rather than an out-of-range slice index.
    if rect.x < 0 || rect.y < 0 {
        return;
    }
    let row_bytes = width as usize * 4;
    let x_start = rect.x as usize;
    let y_start = rect.y as usize;
    let span = rect.width as usize * 4;
    let first = y_start * row_bytes + x_start * 4;
    let first_end = first.checked_add(span).filter(|end| *end <= pixels.len());
    let Some(first_end) = first_end else { return };
    // Expand the colour into one row, then copy that row into the following rows. A whole-row
    // `copy_within` replaces `width` four-byte stores per line with one run copy (G-69).
    for pixel in pixels[first..first_end].as_chunks_mut::<4>().0 {
        *pixel = [color.r, color.g, color.b, color.a];
    }
    let y_end = y_start + rect.height as usize;
    for y in (y_start + 1)..y_end {
        let dst = y * row_bytes + x_start * 4;
        if dst + span > pixels.len() {
            return;
        }
        pixels.copy_within(first..first_end, dst);
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
    // overwritten. That keeps this path free of per-glyph allocation and of any glyph cache —
    // the same contract the software rasteriser honours (BLUE23 §0A.4, constraint 3). The
    // scratch itself is retained across commands (G-222); re-zeroing below gives a reused
    // buffer exactly the fresh `vec![0u8; n]` start state.
    let mut coverage = TEXT_COVERAGE.with(|cell| std::mem::take(&mut *cell.borrow_mut()));
    coverage.clear();
    coverage.resize(cell.area(), 0);
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
        // rasteriser blends. This path used to walk the 8x8 table itself, so feature faces (CJK,
        // vector) were invisible to it and it answered "unsupported" a third way.
        let Some(painted) = crate::render::text::paint_active(scalar, cell, &mut coverage) else {
            continue;
        };
        // The scratch is glyph-sized, so every pixel of it is inside the cell, and coverage
        // arrives per pixel, so antialiased ink would draw correctly without new code.
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
    // Hand the scratch (and its capacity) back for the next text command (G-222).
    TEXT_COVERAGE.with(|cell| *cell.borrow_mut() = std::mem::take(&mut coverage));
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
    let Some(clip_rect) = clip_rect else {
        return;
    };
    let mut sink = PixelSink { pixels, width, mode: blend };
    let (x_start, y_start, x_end, y_end) =
        (clip_rect.x, clip_rect.y, clip_rect.right(), clip_rect.bottom());
    let (rw, sw) = (rect.width as u64, source_width as u64);
    let start = (x_start - rect.x) as u64 * sw;
    let (src_x_start, acc_start) = ((start / rw) as u32, start % rw);
    let last_y = source_height.saturating_sub(1) as u64;
    for y in y_start..y_end {
        let local_y = (y - rect.y) as u32;
        let src_y =
            ((local_y as u64 * source_height as u64) / rect.height as u64).min(last_y) as u32;
        let mut acc = acc_start;
        let mut src_x = src_x_start;
        for x in x_start..x_end {
            let src_offset = ((src_y * source_width + src_x) * 4) as usize;
            let pixel = &source_rgba8[src_offset..src_offset + 4];
            let color = Rgba8 { r: pixel[0], g: pixel[1], b: pixel[2], a: pixel[3] };
            sink.set(x as u32, y as u32, color);
            acc += sw;
            while acc >= rw {
                acc -= rw;
                src_x = src_x.saturating_add(1);
            }
        }
    }
}
/// Bounding box of a line segment, inclusive of both endpoints and the stroke: the pixel-centre
/// rule makes the extent `max - min + 1`, widened by half the line width per side and at least
/// 1px per axis (N-S-65).
///
/// Clamp a segment to `rect` (Liang–Barsky), returning the trimmed endpoints. An endpoint far
/// outside the framebuffer would walk billions of invisible pixels, so this bounds the walk;
/// inside endpoints are unchanged (t₀=0, t₁=1) and a miss is `None`.
fn clip_segment_to_rect(
    from: (i32, i32),
    to: (i32, i32),
    rect: &PixelRect,
) -> Option<((i32, i32), (i32, i32))> {
    let (x0, y0) = (from.0 as f64, from.1 as f64);
    let (dx, dy) = (to.0 as f64 - x0, to.1 as f64 - y0);
    // Inclusive pixel bounds: `right()`/`bottom()` are exclusive.
    let (left, top) = (rect.x as f64, rect.y as f64);
    let (right, bottom) = (rect.right() as f64 - 1.0, rect.bottom() as f64 - 1.0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    // (p, q) per boundary: the segment stays inside while p*t <= q.
    for (p, q) in [(-dx, x0 - left), (dx, right - x0), (-dy, y0 - top), (dy, bottom - y0)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return None;
        }
    }
    let clamp = |v: f64| v.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32;
    let start = (clamp(x0 + t0 * dx), clamp(y0 + t0 * dy));
    let end = (clamp(x0 + t1 * dx), clamp(y0 + t1 * dy));
    Some((start, end))
}

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
/// Bounding box of a circle *including* its stroke width: `DrawCircle` strokes `width`
/// pixels outside the radius, so the box must grow by `width` per side (the old `2r` clipped
/// the outer half of the ring away), plus one pixel so the inclusive far edge
/// (`center + r + width`) is covered (N-S-65).
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
/// Bounding box of a polyline, inclusive of both endpoints and the stroke: the extent is
/// `max - min + 1`, widened by half the stroke per side, and at least 1px per axis so a single
/// point stays drawable (N-S-65).
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
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    // The shape is convex and horizontal, so every row is a single span. Rows between the two
    // corner bands are the full width; a corner row's span is `[left + r - k, right - r - 1 + k]`
    // with `k = floor(sqrt(r^2 - dy^2))`, which is the same integer predicate the per-pixel form
    // evaluated, resolved once per row instead of once per pixel (G-803).
    let r2 = r * r;
    for y in y_start..y_end {
        let (span_start, span_end) = if r <= 0 {
            (rect.x, rect.right())
        } else if y >= rect.y + r && y < rect.bottom() - r {
            (rect.x, rect.right())
        } else {
            let dy = if y < rect.y + r { rect.y + r - y } else { y - (rect.bottom() - r - 1) };
            let remaining = r2 - dy * dy;
            if remaining < 0 {
                continue;
            }
            let k = (remaining as u32).isqrt() as i32;
            (rect.x + r - k, rect.right() - r + k)
        };
        let from = span_start.max(x_start);
        let to = span_end.min(x_end);
        for x in from..to {
            sink.set(x as u32, y as u32, color);
        }
    }
}

/// Whether pixel `(x, y)` is inside `rect` rounded with corner radius `r`. One predicate shared
/// by the fill and the outline, so the ring's straight sections and corner arcs cannot disagree.
fn inside_rounded_rect(rect: PixelRect, r: i32, x: i32, y: i32) -> bool {
    // Bounds first: the corner branches below also assume the point is inside, so `true` from the
    // middle branch would otherwise mean "inside" for a point above or left of the rectangle —
    // which is exactly the shape the outline's *inner* test is asked about.
    if x < rect.x || x >= rect.right() || y < rect.y || y >= rect.bottom() {
        return false;
    }
    if r <= 0 {
        return true;
    }
    let r2 = r * r;
    // Check if pixel is inside the rounded rect:
    // top-left corner
    if x < rect.x + r && y < rect.y + r {
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
    }
}
/// Paints `rect`'s outline as the **inside** band — `rect` minus `rect` inset by `thickness` —
/// the same shape the software and SVG backends paint. Deriving it from one predicate keeps the
/// straight sections and corner arcs on the same pixels, so a rounded outline has no open
/// corners.
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
    let inner_width = rect.width as i32 - 2 * t;
    let inner_height = rect.height as i32 - 2 * t;
    let inner = (inner_width > 0 && inner_height > 0).then_some(PixelRect {
        x: rect.x + t,
        y: rect.y + t,
        width: inner_width as u32,
        height: inner_height as u32,
    });
    let inner_r = (r - t).max(0);
    let x_start = clip_rect.x.max(rect.x);
    let y_start = clip_rect.y.max(rect.y);
    let x_end = clip_rect.right().min(rect.right());
    let y_end = clip_rect.bottom().min(rect.bottom());
    for y in y_start..y_end {
        for x in x_start..x_end {
            if !inside_rounded_rect(rect, r, x, y) {
                continue;
            }
            if let Some(inner) = inner {
                if inside_rounded_rect(inner, inner_r, x, y) {
                    continue;
                }
            }
            sink.set(x as u32, y as u32, color);
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
    // i64 keeps the deltas and the error term overflow-free for extreme endpoints (the
    // bounding-box helper is already widened the same way); the visible behaviour for
    // in-bounds geometry is unchanged.
    let dx = (x1 as i64 - x0 as i64).abs();
    let dy = -(y1 as i64 - y0 as i64).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let half_w = (line_width as i64 / 2).max(1);
    let x_lo = clip_rect.x as i64;
    let x_hi = clip_rect.right() as i64;
    let y_lo = clip_rect.y as i64;
    let y_hi = clip_rect.bottom() as i64;
    loop {
        // Draw a small square around each point for thickness, pre-clamped to the clip so an
        // extreme endpoint cannot overflow `y0 ± half_w` and the inner loop needs no
        // per-pixel bounds check.
        let thick_y = (y0 as i64 - half_w + 1).max(y_lo)..=((y0 as i64 + half_w).min(y_hi - 1));
        let thick_x = (x0 as i64 - half_w + 1).max(x_lo)..=((x0 as i64 + half_w).min(x_hi - 1));
        for thick_y in thick_y {
            for thick_x in thick_x.clone() {
                sink.set(thick_x as u32, thick_y as u32, color);
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
/// Largest integer whose square is at most `value` (integer square root).
///
/// The scanline circle paths need it once per row to turn `dx² + dy² <= r²` into an inclusive x
/// interval; `f64::sqrt` seeds it and correction loops make the answer exact.
fn isqrt_i128(value: i128) -> i128 {
    if value <= 0 {
        return 0;
    }
    let mut root = (value as f64).sqrt() as i128;
    while (root + 1) * (root + 1) <= value {
        root += 1;
    }
    while root * root > value {
        root -= 1;
    }
    root
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
    // All circle math in `i128`: `radius` is `u32`, so `r * r` exceeds `i64` near the top of
    // that range. The clip bounds the visible rows, but the implicit `dx * dx + dy * dy` test
    // is evaluated with the same widened type.
    let r = radius as i128;
    let r2 = r * r;
    let cx = center.0 as i128;
    let cy = center.1 as i128;
    let x_start = clip_rect.x.max((cx - r).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_start = clip_rect.y.max((cy - r).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_end =
        clip_rect.right().min((cx + r + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_end =
        clip_rect.bottom().min((cy + r + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_start = x_start as i128;
    let x_end = x_end as i128;
    // One square root per row yields both endpoints of the painted run, so the inner loop is a
    // contiguous fill instead of a per-pixel distance test (G-68).
    for y in y_start..y_end {
        let dy = y as i128 - cy;
        let remaining = r2 - dy * dy;
        if remaining < 0 {
            continue;
        }
        let half = isqrt_i128(remaining);
        let row_x0 = (cx - half).max(x_start);
        let row_x1 = (cx + half).min(x_end - 1);
        for x in row_x0..=row_x1 {
            sink.set(x as u32, y as u32, color);
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
    let r = radius as i128;
    let outer = r + width as i128;
    let inner = (r - width as i128).max(0);
    let outer_r2 = outer * outer;
    let inner_r2 = inner * inner;
    let cx = center.0 as i128;
    let cy = center.1 as i128;
    let x_start = clip_rect.x.max((cx - outer).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_start = clip_rect.y.max((cy - outer).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_end =
        clip_rect.right().min((cx + outer + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let y_end =
        clip_rect.bottom().min((cy + outer + 1).clamp(i32::MIN as i128, i32::MAX as i128) as i32);
    let x_start = x_start as i128;
    let x_end = x_end as i128;
    for y in y_start..y_end {
        let dy = y as i128 - cy;
        let dy2 = dy * dy;
        let remaining_outer = outer_r2 - dy2;
        if remaining_outer < 0 {
            continue;
        }
        let outer_half = isqrt_i128(remaining_outer);
        let row_x0 = (cx - outer_half).max(x_start);
        let row_x1 = (cx + outer_half).min(x_end - 1);
        if row_x0 > row_x1 {
            continue;
        }
        // The hole is the row of pixels with `dx * dx < inner_r2 - dy * dy`. For integer
        // `dx` that is `dx * dx <= remaining_inner - 1`, whose largest solution is
        // `isqrt(remaining_inner - 1)` (G-68).
        let remaining_inner = inner_r2 - dy2;
        if remaining_inner <= 0 {
            for x in row_x0..=row_x1 {
                sink.set(x as u32, y as u32, color);
            }
        } else {
            let inner_excl = isqrt_i128(remaining_inner - 1);
            let left_hi = (cx - inner_excl - 1).min(row_x1);
            if left_hi >= row_x0 {
                for x in row_x0..=left_hi {
                    sink.set(x as u32, y as u32, color);
                }
            }
            let right_lo = (cx + inner_excl + 1).max(row_x0);
            if right_lo <= row_x1 {
                for x in right_lo..=row_x1 {
                    sink.set(x as u32, y as u32, color);
                }
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
    // The same scanline kernel as `fill_circle_cpu_rgba8`/`draw_circle_cpu_rgba8` (G-68):
    // one square root per row yields the inside-circle run, so the angle test runs only on
    // candidate pixels instead of every bounding-box pixel (G-223). Circle math stays in
    // `i128` so `(r + 1) * (r + 1)` cannot overflow near the top of `u32`.
    let r = arc.radius as i128;
    let r2 = r * r;
    let outer_r2 = (r + 1) * (r + 1);
    let inner_r2 = (r - 1).max(0) * (r - 1).max(0);
    let cx = arc.center.0 as i128;
    let cy = arc.center.1 as i128;
    let bound = |v: i128| v.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
    let x_start = clip_rect.x.max(bound(cx - r)) as i128;
    let y_start = clip_rect.y.max(bound(cy - r)) as i128;
    let x_end = clip_rect.right().min(bound(cx + r + 1)) as i128;
    let y_end = clip_rect.bottom().min(bound(cy + r + 1)) as i128;
    let (sa, ea) = if (arc.start_angle - arc.end_angle).abs() < 0.001 {
        (0.0, std::f32::consts::TAU) // full circle: the angle test is skipped entirely
    } else {
        let sa = arc.start_angle.rem_euclid(std::f32::consts::TAU);
        let mut ea = arc.end_angle.rem_euclid(std::f32::consts::TAU);
        if ea <= sa {
            ea += std::f32::consts::TAU; // normalise to [sa, sa + TAU) so ea > sa
        }
        (sa, ea)
    };
    let is_full_circle = (ea - sa) >= std::f32::consts::TAU - 0.001;
    // Paints one row span, skipping pixels outside the arc's angular window. The test is
    // exactly the per-pixel arithmetic it replaces, so the painted set is unchanged; it now
    // runs only on pixels already inside the circle.
    let mut paint_span = |x0: i128, x1: i128, y: i128| {
        for x in x0..=x1 {
            if !is_full_circle {
                let angle = ((y - cy) as f64).atan2((x - cx) as f64) as f32;
                let angle = angle.rem_euclid(std::f32::consts::TAU);
                let angle = if angle < sa { angle + std::f32::consts::TAU } else { angle };
                if angle < sa || angle > ea {
                    continue;
                }
            }
            sink.set(x as u32, y as u32, arc.color);
        }
    };
    for y in y_start..y_end {
        let dy2 = (y - cy) * (y - cy);
        // Row candidates: the filled disc's run `|dx| <= isqrt(r2 - dy2)`, or for a stroke
        // the outer circle's run `|dx| <= isqrt(outer_r2 - dy2)`.
        let remaining = (if arc.filled { r2 } else { outer_r2 }) - dy2;
        if remaining < 0 {
            continue;
        }
        let half = isqrt_i128(remaining);
        let row_x0 = (cx - half).max(x_start);
        let row_x1 = (cx + half).min(x_end - 1);
        if row_x0 > row_x1 {
            continue;
        }
        // A stroke excludes its hole `dx * dx < inner_r2 - dy2`; the row paints the two run
        // segments flanking it (its largest excluded dx is `isqrt(remaining_inner - 1)`, G-68).
        let remaining_inner = if arc.filled { 0 } else { inner_r2 - dy2 };
        if remaining_inner > 0 {
            let inner_excl = isqrt_i128(remaining_inner - 1);
            paint_span(row_x0, (cx - inner_excl - 1).min(row_x1), y);
            paint_span((cx + inner_excl + 1).max(row_x0), row_x1, y);
        } else {
            paint_span(row_x0, row_x1, y);
        }
    }
}
struct CpuPathDraw<'a> {
    points: &'a [(i32, i32)],
    closed: bool,
    color: Rgba8,
    filled: bool,
    path_width: u32,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
}

fn draw_path_cpu_rgba8(pixels: &mut [u8], fb_width: u32, path: CpuPathDraw<'_>) {
    let CpuPathDraw { points, closed, color, filled, path_width, clip_rect, blend } = path;
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
        // One intersection buffer for the whole fill, cleared per scanline — the shape of the
        // sibling `draw_path_non_zero_cpu_rgba8` just below. A fresh `Vec` per scanline meant one
        // allocation per row of every filled outline (an icon, a vector decoration, a glyph
        // fallback), which is hundreds of allocations for one picture (REND-08).
        let mut intersections: Vec<i32> = Vec::with_capacity(n);
        for y in min_y..max_y {
            intersections.clear();
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

fn draw_path_non_zero_cpu_rgba8(
    pixels: &mut [u8],
    fb_width: u32,
    points: &[(i32, i32)],
    contours: &[(usize, usize)],
    color: Rgba8,
    clip_rect: Option<PixelRect>,
    blend: BlendMode,
) {
    let Some(clip_rect) = clip_rect else {
        return;
    };
    let min_y = clip_rect.y.max(
        contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|&(_, y)| y)
            .min()
            .unwrap_or(0),
    );
    let max_y = clip_rect.bottom().min(
        contours
            .iter()
            .flat_map(|&(start, end)| &points[start..end])
            .map(|&(_, y)| y)
            .max()
            .unwrap_or(-1)
            .saturating_add(1),
    );
    let mut intersections: Vec<(f64, i32)> =
        Vec::with_capacity(contours.iter().map(|&(start, end)| end - start).sum());
    let mut sink = PixelSink { pixels, width: fb_width, mode: blend };

    for y in min_y..max_y {
        intersections.clear();
        for &(start, end) in contours {
            for index in start..end {
                let (x1, y1) = points[index];
                let (x2, y2) = points[if index + 1 == end { start } else { index + 1 }];
                let winding = if y1 <= y && y2 > y {
                    1
                } else if y2 <= y && y1 > y {
                    -1
                } else {
                    continue;
                };
                let t = (y - y1) as f64 / (y2 - y1) as f64;
                intersections.push((x1 as f64 + t * (x2 - x1) as f64, winding));
            }
        }
        intersections.sort_by(|left, right| {
            left.0.partial_cmp(&right.0).unwrap_or(core::cmp::Ordering::Equal)
        });

        let mut winding = 0;
        let mut previous_x: Option<f64> = None;
        let mut index = 0;
        while index < intersections.len() {
            let x = intersections[index].0;
            if winding != 0 {
                if let Some(previous) = previous_x {
                    let x_start = (previous.ceil() as i32).max(clip_rect.x);
                    let x_end = (x.floor() as i32).min(clip_rect.right().saturating_sub(1));
                    for pixel_x in x_start..=x_end {
                        sink.set(pixel_x as u32, y as u32, color);
                    }
                }
            }
            let mut delta = 0;
            while index < intersections.len() && intersections[index].0 == x {
                delta += intersections[index].1;
                index += 1;
            }
            winding += delta;
            previous_x = Some(x);
        }
    }
}

/// Selects the nearest colour stop of `gradient_data` for a position `t` in `0.0..=1.0`.
///
/// Stops are evenly spaced and picked by their left edge, exactly as the previous per-pixel
/// expression did; only the arithmetic that produces `t` changed (G-68).
#[inline]
fn gradient_stop(gradient_data: &[u8], stops: usize, t: f32) -> Rgba8 {
    let idx = ((stops as f32 - 1.0) * t.clamp(0.0, 1.0)).clamp(0.0, (stops - 1) as f32) as usize;
    let base = idx * 4;
    Rgba8 {
        r: gradient_data[base],
        g: gradient_data[base + 1],
        b: gradient_data[base + 2],
        a: gradient_data[base + 3],
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

    // The span the ramp is spread over, and the reciprocal that replaces a per-pixel division.
    // `f32` is enough for screen-sized ramps and halves the arithmetic width (G-68).
    let (span, origin) = match axis {
        GradientAxis::Vertical => ((rect.height as f32 - 1.0).max(1.0), rect.y as f32),
        GradientAxis::Horizontal => ((rect.width as f32 - 1.0).max(1.0), rect.x as f32),
    };
    let inv_span = 1.0 / span;

    for y in y_start..y_end {
        match axis {
            GradientAxis::Vertical => {
                // A vertical ramp is constant across the row: one stop lookup serves the whole run.
                let t = ((y as f32 - origin) * inv_span).clamp(0.0, 1.0);
                let color = gradient_stop(gradient_data, stops, t);
                for x in x_start..x_end {
                    blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
                }
            }
            GradientAxis::Horizontal => {
                // The reciprocal is applied per pixel instead of dividing by the span; the
                // product is bounded per pixel, so it does not accumulate rounding like a
                // running sum would (G-68).
                for x in x_start..x_end {
                    let t = ((x as f32 - origin) * inv_span).clamp(0.0, 1.0);
                    let color = gradient_stop(gradient_data, stops, t);
                    blend_pixel_cpu_rgba8(pixels, fb_width, x as u32, y as u32, color, blend);
                }
            }
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

/// Fills `rect` with a radial ramp of `gradient_data` stop colours. The variant carries no
/// centre or radius, so the ramp is centred in `rect` and reaches the nearest edge; pixels
/// beyond it take the **last** stop so the fill always covers `rect` completely.
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

    let centre_x = rect.x as f32 + (rect.width as f32 - 1.0) / 2.0;
    let centre_y = rect.y as f32 + (rect.height as f32 - 1.0) / 2.0;
    // Nearest edge, so the gradient fits inside the rectangle on both axes.
    let radius = ((rect.width as f32).min(rect.height as f32) / 2.0).max(1.0);
    let inv_radius = 1.0 / radius;

    for y in y_start..y_end {
        // `dy` is constant for the row; only `dx` changes, so one `dy * dy` serves the row.
        let dy = y as f32 - centre_y;
        let dy2 = dy * dy;
        for x in x_start..x_end {
            let dx = x as f32 - centre_x;
            // The per-pixel division became a multiply by the reciprocal (G-68).
            let t = ((dx * dx + dy2).sqrt() * inv_radius).clamp(0.0, 1.0);
            let color = gradient_stop(gradient_data, stops, t);
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

/// Maps the numeric blend-mode tag carried by the command stream to its enum. The tag is the
/// enum's own `as u8` discriminant, but the stream is plain bytes so an unknown value is
/// possible: it falls back to [`BlendMode::Normal`] and is logged, because substituting a
/// blend mode silently would change pixels the caller asked for.
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

/// Applies the blend formula to one source/destination pair. The separable modes are
/// evaluated per channel over 0..=1; the four non-separable ones (`Hue`, `Saturation`,
/// `Color`, `Luminosity`) are defined on the triple's luminance and are handled together in
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
    if width == 0 {
        return;
    }
    // Intersect the source rectangle with the canvas once. The inner loop then runs entirely
    // inside the framebuffer, so the row base and the row length are its only invariants and no
    // per-pixel bounds or sign test is needed (G-69).
    let height = (pixels.len() / (width as usize * 4)) as u32;
    let Some(draw) = rect.intersect(PixelRect { x: 0, y: 0, width, height }) else { return };
    let row_bytes = width as usize * 4;
    let span = draw.width as usize * 4;
    for y in draw.y..draw.bottom() {
        let offset = y as usize * row_bytes + draw.x as usize * 4;
        for pixel in pixels[offset..offset + span].as_chunks_mut::<4>().0 {
            let dest = Rgba8 { r: pixel[0], g: pixel[1], b: pixel[2], a: pixel[3] };
            let blended = blend_channels(color, dest, mode);
            *pixel = [blended.r, blended.g, blended.b, blended.a];
        }
    }
}

/// Draws a drop shadow into a private layer the size of the clamped blur area, blurs it there,
/// then composites it back (N-S-63). `body` is the solid rectangle and `paint_region` bounds the
/// feathered paint, so the blur can spread outside the body but never outside the caller's clip;
/// blurring the layer keeps the scene behind the shadow out of the source.
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

    // Independent, transparent shadow layer, retained by capacity between calls.
    SHADOW_LAYER.with(|cell| {
        let mut layer = cell.borrow_mut();
        let needed = lw * lh * 4;
        if layer.len() != needed {
            layer.resize(needed, 0);
        }
        // Fill the body and clear the surrounding ring in one pass. Every byte of the layer is
        // written below, so the previous `clear` + `resize` cost a second full memset for nothing
        // (G-67).
        let bx0 = (visible.x as i64 - lx0 as i64).max(0) as usize;
        let by0 = (visible.y as i64 - ly0 as i64).max(0) as usize;
        let bx1 = ((visible.right() as i64 - lx0 as i64).clamp(0, lw as i64)) as usize;
        let by1 = ((visible.bottom() as i64 - ly0 as i64).clamp(0, lh as i64)) as usize;
        let body = [color.r, color.g, color.b, color.a];
        for y in 0..lh {
            let in_body_y = y >= by0 && y < by1;
            for x in 0..lw {
                let o = (y * lw + x) * 4;
                let pixel = if in_body_y && x >= bx0 && x < bx1 { body } else { [0, 0, 0, 0] };
                layer[o..o + 4].copy_from_slice(&pixel);
            }
        }

        // Blur the layer in place (separable moving average, matching the software
        // backend's kernel).
        box_blur_layer_rgba8(&mut layer[..], lw, lh, blur_radius as usize);

        // Composite the layer back, but only inside `paint_region` (the caller's
        // clip, or the framebuffer): any pixel the caller clipped away must stay
        // exactly as it was (N-S-63). The body is opaque shadow colour, so a
        // source-over composite reads the layer's alpha as coverage.
        let Some(dst) = paint_region.intersect(PixelRect { x: 0, y: 0, width, height }) else {
            return;
        };
        // Each row paints at most one contiguous span — the destination row clipped to the
        // layer — so the per-pixel layer bounds checks collapse into two span endpoints
        // (G-225). Rows outside the layer are skipped whole; the blend logic is unchanged.
        for y in dst.y..dst.bottom() {
            let lyo = y as i64 - ly0 as i64;
            if lyo < 0 || lyo >= lh as i64 {
                continue;
            }
            let x_lo = (dst.x as i64).max(lx0 as i64);
            let x_hi = (dst.right() as i64).min(lx0 as i64 + lw as i64);
            if x_lo >= x_hi {
                continue;
            }
            let row = lyo as usize * lw;
            for x in x_lo..x_hi {
                let o = (row + (x - lx0 as i64) as usize) * 4;
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
    });
}

/// Box-blurs an RGBA buffer in place using a separable moving average. The squared kernel
/// matches the software backend, and separability keeps it O(1) per pixel instead of O(radius)
/// (G-67); the buffer is a self-contained layer, so the framebuffer is never read (N-S-63).
fn box_blur_layer_rgba8(layer: &mut [u8], width: usize, height: usize, radius: usize) {
    if radius == 0 || width == 0 || height == 0 {
        return;
    }
    SHADOW_SCRATCH.with(|cell| {
        let mut scratch = cell.borrow_mut();
        let needed = layer.len();
        // The horizontal pass overwrites every pixel of the scratch, so a buffer that already
        // has the right length needs neither a resize nor a zero fill (G-67).
        if scratch.len() != needed {
            scratch.resize(needed, 0);
        }
        // Horizontal pass: `layer` -> `scratch`. The window starts as `[0, radius]` (clamped)
        // and slides right, adding the pixel that enters and removing the one that leaves.
        // Signed accumulators: the leaving pixel can be brighter than the entering one, so an
        // unsigned sum would underflow where the window stops growing and starts sliding.
        let half = radius;
        for y in 0..height {
            let row = y * width;
            let mut sums = [0i64; 4];
            for kx in 0..=half.min(width - 1) {
                let i = (row + kx) * 4;
                for c in 0..4 {
                    sums[c] += layer[i + c] as i64;
                }
            }
            for x in 0..width {
                let lo = x.saturating_sub(half);
                let hi = x.saturating_add(half).min(width - 1);
                let count = (hi - lo + 1) as i64;
                let di = (row + x) * 4;
                for c in 0..4 {
                    scratch[di + c] = (sums[c] / count) as u8;
                }
                if x + 1 == width {
                    break;
                }
                if x.saturating_add(1).saturating_add(half) < width {
                    let entering = (row + x + 1 + half) * 4;
                    for c in 0..4 {
                        sums[c] += layer[entering + c] as i64;
                    }
                }
                if x >= half {
                    let leaving = (row + x - half) * 4;
                    for c in 0..4 {
                        sums[c] -= layer[leaving + c] as i64;
                    }
                }
            }
        }
        // Vertical pass: `scratch` -> `layer`, with the same sliding window down each column.
        for x in 0..width {
            let mut sums = [0i64; 4];
            for ky in 0..=half.min(height - 1) {
                let i = (ky * width + x) * 4;
                for c in 0..4 {
                    sums[c] += scratch[i + c] as i64;
                }
            }
            for y in 0..height {
                let lo = y.saturating_sub(half);
                let hi = y.saturating_add(half).min(height - 1);
                let count = (hi - lo + 1) as i64;
                let di = (y * width + x) * 4;
                for c in 0..4 {
                    layer[di + c] = (sums[c] / count) as u8;
                }
                if y + 1 == height {
                    break;
                }
                if y.saturating_add(1).saturating_add(half) < height {
                    let entering = ((y + 1 + half) * width + x) * 4;
                    for c in 0..4 {
                        sums[c] += scratch[entering + c] as i64;
                    }
                }
                if y >= half {
                    let leaving = ((y - half) * width + x) * 4;
                    for c in 0..4 {
                        sums[c] -= scratch[leaving + c] as i64;
                    }
                }
            }
        }
    });
}

/// Writes pixels into the framebuffer through the rasteriser's active blend mode.
///
/// `SetBlendMode` is sticky and promised to affect *every* following draw, but only rects
/// and gradients honoured it — text, images, rounded rects, lines, circles, arcs and paths
/// all called `set_pixel_cpu_rgba8` directly and always wrote source-over. Threading this
/// sink through the helpers gives every variant the same blend behaviour (N-S-64).
struct PixelSink<'a> {
    pixels: &'a mut [u8],
    width: u32,
    mode: BlendMode,
}

impl PixelSink<'_> {
    /// Writes `color` at `(x, y)`, compositing with the active blend mode. `Normal` takes
    /// the fast direct-write path, so an unaware caller's output is byte-for-byte unchanged.
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

/// As [`blend_cpu_rgba8`], but first composites `color` with the destination through
/// `mode`. The coverage path serves anti-aliased glyph edges; without the mode it would
/// bypass `SetBlendMode` entirely (opaque glyph interiors honoured it, their edges did
/// not), so a Multiply text draw came out half-applied (N-S-64). The blended colour is
/// computed against the current destination, then source-over composited by `coverage`.
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

    /// Rendering into a caller buffer must produce exactly the owned wrapper's bytes, even
    /// when the buffer starts with a previous frame's contents (G-221).
    #[test]
    fn caller_buffer_rasterization_matches_the_owned_frame() {
        let commands = vec![
            WgpuDrawCommand::Clear { color: Rgba8 { r: 10, g: 20, b: 30, a: 255 } },
            WgpuDrawCommand::PushClip { rect: rect(0, 0, 16, 20) },
            WgpuDrawCommand::FillRect {
                rect: rect(0, 0, 20, 10),
                color: RED,
                clip: Some(rect(2, 2, 12, 12)),
            },
            WgpuDrawCommand::StrokeRect {
                rect: rect(4, 4, 12, 12),
                color: Rgba8 { r: 0, g: 255, b: 0, a: 255 },
                thickness: 2,
                clip: None,
            },
            WgpuDrawCommand::FillRoundedRect {
                rect: rect(6, 6, 8, 8),
                radius: 3,
                color: Rgba8 { r: 0, g: 0, b: 255, a: 200 },
                clip: None,
            },
            WgpuDrawCommand::DrawText {
                rect: rect(0, 0, 16, 16),
                text: "AB".into(),
                color: Rgba8 { r: 255, g: 255, b: 0, a: 255 },
                clip: None,
            },
            WgpuDrawCommand::FillCircle {
                center: (12, 12),
                radius: 5,
                color: Rgba8 { r: 255, g: 0, b: 255, a: 180 },
                clip: None,
            },
            WgpuDrawCommand::SetBlendMode { mode: 1 },
            WgpuDrawCommand::FillRect {
                rect: rect(8, 8, 8, 8),
                color: Rgba8 { r: 128, g: 128, b: 128, a: 255 },
                clip: None,
            },
            WgpuDrawCommand::PopClip,
        ];
        let owned = rasterize_draw_commands_rgba8(20, 20, &commands).expect("owned frame");
        // A non-zero fill proves the into-variant clears a reused buffer rather than drawing
        // over whatever the previous frame left behind.
        let mut reused = vec![0xA5u8; owned.len()];
        rasterize_draw_commands_into_rgba8(20, 20, &commands, &mut reused).expect("into frame");
        assert_eq!(reused, owned, "caller-buffer output must match the owned frame");
        assert!(rasterize_draw_commands_into_rgba8(20, 20, &commands, &mut reused[1..]).is_err());
    }

    #[test]
    fn non_zero_path_fill_preserves_oppositely_wound_holes() {
        let points = vec![(1, 1), (15, 1), (15, 15), (1, 15), (5, 5), (5, 11), (11, 11), (11, 5)];
        let pixels = rasterize_draw_commands_rgba8(
            16,
            16,
            &[WgpuDrawCommand::FillPathNonZero {
                points: points.into(),
                contours: vec![(0, 4), (4, 8)].into(),
                color: RED,
                clip: None,
            }],
        )
        .expect("valid compound path rasterizes");

        assert_eq!(pixel(&pixels, 16, 3, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [0, 0, 0, 0]);
    }

    #[test]
    fn non_zero_path_fill_rejects_invalid_contours() {
        let error = rasterize_draw_commands_rgba8(
            16,
            16,
            &[WgpuDrawCommand::FillPathNonZero {
                points: vec![(1, 1), (15, 1), (15, 15)].into(),
                contours: vec![(0, 4)].into(),
                color: RED,
                clip: None,
            }],
        )
        .expect_err("out-of-bounds contour must not be silently accepted");

        assert!(error.contains("invalid range"));
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

    /// A rounded outline must hug the rectangle's edge and stay connected round the corner.
    ///
    /// # The defect this pins
    ///
    /// The straight sections used to be painted `radius` pixels in from the edge (`y >= rect.y +
    /// r`), while the corner ring sat on the true corner centre. A 4 px corner therefore left a 4 px
    /// gap between the arc and the line it was supposed to meet: every rounded control had four
    /// open corners, and the top/bottom/left/right rules were drawn well inside the widget. The
    /// outline is now derived as "outer shape minus the shape inset by its width", so the two
    /// cannot disagree.
    #[test]
    fn rounded_stroke_hugs_the_edge_and_closes_its_corners() {
        let commands = vec![WgpuDrawCommand::StrokeRoundedRect {
            rect: rect(5, 5, 20, 20),
            radius: 4,
            color: RED,
            thickness: 1,
            clip: None,
        }];
        let pixels = rasterize_draw_commands_rgba8(40, 40, &commands).expect("rasterizes");

        // The top rule is on the first row of the rectangle, not `radius` px below it.
        for x in 9..=20 {
            assert_eq!(
                pixel(&pixels, 40, x, 5),
                [255, 0, 0, 255],
                "the top rule must sit on y = rect.y (x = {x})"
            );
        }
        assert_eq!(
            pixel(&pixels, 40, 5, 9),
            [255, 0, 0, 255],
            "the left rule meets the corner arc"
        );
        assert_eq!(pixel(&pixels, 40, 9, 5), [255, 0, 0, 255], "the top rule meets the corner arc");

        // The interior stays empty: a 1 px outline is a ring, not a fill.
        assert_eq!(
            pixel(&pixels, 40, 15, 15),
            [0, 0, 0, 0],
            "the middle of the ring is not painted"
        );

        // Nothing bleeds outside the rectangle the caller declared.
        assert_eq!(pixel(&pixels, 40, 4, 20), [0, 0, 0, 0], "nothing is painted left of rect.x");
        assert_eq!(pixel(&pixels, 40, 20, 4), [0, 0, 0, 0], "nothing is painted above rect.y");
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
                gradient_data: ramp.clone().into(),
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
                gradient_data: ramp.into(),
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
                gradient_data: ramp.into(),
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
                    text: "HH".into(),
                    color: grey,
                    clip: None,
                },
            ),
            (
                "DrawImage",
                WgpuDrawCommand::DrawImage {
                    rect: rect(0, 0, 16, 16),
                    rgba8: [128, 128, 128, 255].repeat(16 * 16).into(),
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
                    points: vec![(0, 0), (15, 15), (0, 15)].into(),
                    closed: true,
                    color: grey,
                    filled: true,
                    width: 0,
                    clip: None,
                },
            ),
            (
                "FillPathNonZero",
                WgpuDrawCommand::FillPathNonZero {
                    points: vec![(0, 0), (15, 0), (0, 15)].into(),
                    contours: vec![(0, 3)].into(),
                    color: grey,
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

    // --- Degenerate geometry: extreme coordinates must not overflow the i32
    // rasteriser arithmetic nor walk invisible pixels for billions of steps. ---

    /// Liang–Barsky trimming: in-bounds segments pass through unchanged, complete
    /// misses return `None`, and partial crossings are trimmed to the visible part.
    #[test]
    fn clip_segment_trims_extreme_endpoints() {
        let viewport = rect(0, 0, 100, 100);

        // Fully inside: returned unchanged (t₀=0, t₁=1).
        assert_eq!(clip_segment_to_rect((10, 10), (20, 20), &viewport), Some(((10, 10), (20, 20))));

        // Fully outside: no intersection with any boundary.
        assert_eq!(clip_segment_to_rect((-50, -50), (-10, -10), &viewport), None);
        assert_eq!(clip_segment_to_rect((200, 0), (300, 100), &viewport), None);

        // Crossing the whole viewport: trimmed to the border pixels.
        assert_eq!(
            clip_segment_to_rect((-1000, 50), (2000, 50), &viewport),
            Some(((0, 50), (99, 50)))
        );

        // One endpoint inside, one far below the viewport.
        assert_eq!(
            clip_segment_to_rect((50, 50), (50, 10_000), &viewport),
            Some(((50, 50), (50, 99)))
        );
    }

    /// A line whose endpoints sit at the i32 extremes must rasterize promptly and
    /// stay inside the framebuffer. Before the segment trim this walked ~2·10⁹
    /// invisible pixels (and overflowed the i32 error term) before reaching the
    /// visible region.
    #[test]
    fn extreme_endpoint_line_rasterizes_within_framebuffer() {
        let pixels = rasterize_draw_commands_rgba8(
            32,
            32,
            &[WgpuDrawCommand::DrawLine {
                from: (i32::MIN, 16),
                to: (i32::MAX, 16),
                color: RED,
                width: 1,
                clip: None,
            }],
        )
        .expect("rasterizes");

        // The visible middle of the segment is painted, edge to edge...
        assert_eq!(pixel(&pixels, 32, 0, 16), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 32, 15, 16), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 32, 31, 16), [255, 0, 0, 255]);
        // ...and nothing outside that row is touched.
        assert_eq!(pixel(&pixels, 32, 15, 15), [0, 0, 0, 0]);
        assert_eq!(pixel(&pixels, 32, 15, 17), [0, 0, 0, 0]);
    }

    /// Circles far outside the visible region must not panic on the (debug-build)
    /// `i32` overflow of `r * r` and must not paint a single pixel. The arc variant
    /// keeps its centre inside the viewport so the per-pixel `dx*dx + dy*dy` math
    /// really runs with a radius that overflows `i32` when squared.
    #[test]
    fn huge_offscreen_circles_do_not_panic_or_paint() {
        let pixels = rasterize_draw_commands_rgba8(
            16,
            16,
            &[
                // Right edge of the fill ends one pixel left of the viewport.
                WgpuDrawCommand::FillCircle {
                    center: (-2_000_000_001, 8),
                    radius: 2_000_000_000,
                    color: RED,
                    clip: None,
                },
                // Ring centred far past the top-right corner: the clip still walks
                // the viewport, every pixel must fail the ring test in i64.
                WgpuDrawCommand::DrawCircle {
                    center: (2_000_000_000, 2_000_000_000),
                    radius: 1_999_999_999,
                    color: RED,
                    width: 3,
                    clip: None,
                },
                // Ring centred inside the viewport with a radius that dwarfs it.
                WgpuDrawCommand::DrawArc {
                    center: (8, 8),
                    radius: 2_000_000_000,
                    start_angle: 0.0,
                    end_angle: 1.0,
                    color: RED,
                    filled: false,
                    clip: None,
                },
            ],
        )
        .expect("rasterizes");

        assert!(
            pixels.iter().all(|&b| b == 0),
            "offscreen / unreachable circles must not paint anything"
        );
    }

    // --- G-222/G-223/G-225 regression pins: the scratch-reuse and scanline reworks must
    // be byte-identical to the per-allocation / per-pixel forms they replaced. ---

    /// The glyph coverage scratch is retained across text commands (G-222); a reused buffer
    /// must render exactly what a fresh one did, with no ink from an earlier glyph leaking.
    #[test]
    fn text_coverage_reuse_renders_each_glyph_fresh() {
        let frame = |text: &str| {
            rasterize_draw_commands_rgba8(
                8,
                8,
                &[
                    WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 0 } },
                    WgpuDrawCommand::DrawText {
                        rect: rect(0, 0, 8, 8),
                        text: text.into(),
                        color: RED,
                        clip: Some(rect(0, 0, 8, 8)),
                    },
                ],
            )
            .expect("rasterizes")
        };
        let a_first = frame("A");
        let b = frame("B");
        let a_again = frame("A");
        assert_ne!(a_first, b, "different glyphs must paint differently");
        assert_eq!(a_first, a_again, "re-rendering after other text must be identical");

        // The same pin within one command: two cells sharing the retained scratch.
        let both = rasterize_draw_commands_rgba8(
            16,
            8,
            &[
                WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 0 } },
                WgpuDrawCommand::DrawText {
                    rect: rect(0, 0, 16, 8),
                    text: "AB".into(),
                    color: RED,
                    clip: Some(rect(0, 0, 16, 8)),
                },
            ],
        )
        .expect("rasterizes");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(
                    pixel(&both, 16, x, y),
                    pixel(&a_first, 8, x, y),
                    "'A' cell at ({x},{y})"
                );
                assert_eq!(
                    pixel(&both, 16, x + 8, y),
                    pixel(&b, 8, x, y),
                    "'B' cell at ({},{})",
                    x + 8,
                    y
                );
            }
        }
    }

    /// The scanline arc kernel (G-223) must paint exactly the set the old per-pixel
    /// definition defines: pixels passing the distance test whose angle is inside the
    /// window. This reference is that definition, written independently of the kernel.
    #[test]
    fn arc_matches_its_per_pixel_reference() {
        fn reference_arc(
            width: u32,
            height: u32,
            center: (i32, i32),
            radius: u32,
            start_angle: f32,
            end_angle: f32,
            filled: bool,
        ) -> Vec<u8> {
            let r = radius as i64;
            let r2 = r * r;
            let outer_r2 = (r + 1) * (r + 1);
            let inner_r2 = (r - 1).max(0) * (r - 1).max(0);
            let (sa, ea) = if (start_angle - end_angle).abs() < 0.001 {
                (0.0, std::f32::consts::TAU)
            } else {
                let sa = start_angle.rem_euclid(std::f32::consts::TAU);
                let mut ea = end_angle.rem_euclid(std::f32::consts::TAU);
                if ea <= sa {
                    ea += std::f32::consts::TAU;
                }
                (sa, ea)
            };
            let is_full_circle = (ea - sa) >= std::f32::consts::TAU - 0.001;
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            for y in 0..height as i64 {
                for x in 0..width as i64 {
                    let dx = x - center.0 as i64;
                    let dy = y - center.1 as i64;
                    let d2 = dx * dx + dy * dy;
                    // The old per-pixel loop iterated `x ∈ [max(clip.x, cx - r),
                    // min(clip.right(), cx + r + 1))` (and the same for `y`), so ring pixels
                    // at |dx| or |dy| of exactly r+1 were never reachable; with the arc fully
                    // inside the framebuffer the clip adds no further bound.
                    if dx.abs() > r || dy.abs() > r {
                        continue;
                    }
                    let inside = if filled { d2 <= r2 } else { d2 <= outer_r2 && d2 >= inner_r2 };
                    if !inside {
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
                    let o = (y as usize * width as usize + x as usize) * 4;
                    pixels[o..o + 4].copy_from_slice(&[RED.r, RED.g, RED.b, RED.a]);
                }
            }
            pixels
        }

        let tau = std::f32::consts::TAU;
        let cases = [
            (0.0f32, std::f32::consts::FRAC_PI_2), // quarter: the down-right octant boundary
            (0.0, std::f32::consts::PI),           // half circle
            (std::f32::consts::PI, std::f32::consts::FRAC_PI_2), // ea < sa: wraps through 0
            (-1.0, 1.0),                           // negative start normalises
            (tau - 0.5, tau + 0.5),                // window straddling the 2π wrap
            (0.0, 0.0),                            // full circle: angle test skipped
        ];
        for filled in [true, false] {
            for radius in [1u32, 3, 9] {
                for (start_angle, end_angle) in cases {
                    let drawn = rasterize_draw_commands_rgba8(
                        24,
                        24,
                        &[
                            WgpuDrawCommand::Clear { color: Rgba8 { r: 0, g: 0, b: 0, a: 0 } },
                            WgpuDrawCommand::DrawArc {
                                center: (12, 12),
                                radius,
                                start_angle,
                                end_angle,
                                color: RED,
                                filled,
                                clip: None,
                            },
                        ],
                    )
                    .expect("rasterizes");
                    let expected =
                        reference_arc(24, 24, (12, 12), radius, start_angle, end_angle, filled);
                    assert_eq!(
                        drawn, expected,
                        "arc r={radius} filled={filled} angles {start_angle}..{end_angle}"
                    );
                }
            }
        }
    }

    /// Shadow compositing (G-225) must only touch the caller's paint region, and a clipped
    /// render must equal the unclipped one inside it — the row-span batching may not shift
    /// a single blended pixel even where the layer extends past the clip.
    #[test]
    fn box_shadow_respects_the_paint_region_and_matches_the_unclipped_render() {
        let shadow = WgpuDrawCommand::BoxShadow {
            rect: rect(8, 8, 4, 4),
            color: RED,
            offset_x: 0,
            offset_y: 0,
            blur_radius: 2,
            clip: None,
        };
        let full = rasterize_draw_commands_rgba8(20, 20, &[shadow.clone()]).expect("rasterizes");
        // The clip contains the body (so the blurred source is identical in both renders)
        // but cuts the ramp: the layer reaches x/y 14, the paint region stops at 12.
        let clip = rect(0, 0, 12, 12);
        let clipped = rasterize_draw_commands_rgba8(
            20,
            20,
            &[WgpuDrawCommand::PushClip { rect: clip }, shadow, WgpuDrawCommand::PopClip],
        )
        .expect("rasterizes");
        let mut painted_inside = 0;
        for y in 0..20u32 {
            for x in 0..20u32 {
                let (a, b) = (pixel(&full, 20, x, y), pixel(&clipped, 20, x, y));
                if x < clip.right() as u32 && y < clip.bottom() as u32 {
                    assert_eq!(a, b, "the clip must not change the shadow at ({x},{y})");
                    if b != [0, 0, 0, 0] {
                        painted_inside += 1;
                    }
                } else {
                    assert_eq!(b, [0, 0, 0, 0], "clipped shadow must not paint at ({x},{y})");
                }
            }
        }
        assert!(painted_inside > 0, "the clipped render must paint something");
    }
}
