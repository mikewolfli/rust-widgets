// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The command walk: framebuffer sizing, the clip stack, and the one loop that turns a
//! [`WgpuDrawCommand`] stream into pixels.
//!
//! Every command here either drives the running clip state or delegates to a kernel in
//! [`super::fills`] / [`super::text`]; nothing in this file decides what a shape looks like.

use super::super::commands::WgpuDrawCommand;
use super::super::types::PixelRect;
use super::fills::{
    bbox_for_circle, bbox_for_path, blend_mode_from_id, box_shadow_cpu_rgba8, clear_cpu_rgba8,
    clip_segment_to_rect, draw_arc_cpu_rgba8, draw_circle_cpu_rgba8, draw_gradient_cpu_rgba8,
    draw_image_scaled_cpu_rgba8, draw_line_cpu_rgba8, draw_path_cpu_rgba8,
    draw_path_non_zero_cpu_rgba8, draw_radial_gradient_cpu_rgba8, fill_circle_cpu_rgba8,
    fill_rect_cpu_rgba8_blended, fill_rounded_rect_cpu_rgba8, rect_for_line, stroke_rect_edges,
    stroke_rounded_rect_cpu_rgba8, ArcParams, CpuPathDraw, GradientAxis,
};
use super::text::draw_text_cpu_rgba8;
use crate::render::BlendMode;

/// Byte length of a `width × height` RGBA8 frame, with the overflow and 4 GiB bound checks shared
/// by both entry points (N-S-66).
pub(crate) fn framebuffer_byte_len(width: u32, height: u32) -> Result<usize, String> {
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

pub(crate) fn effective_clip(
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
pub(crate) fn combine(own: &Option<PixelRect>, enclosing: Option<PixelRect>) -> Option<PixelRect> {
    match (own, enclosing) {
        (Some(own), Some(enclosing)) => {
            Some(own.intersect(enclosing).unwrap_or(PixelRect { x: 0, y: 0, width: 0, height: 0 }))
        }
        (Some(own), None) => Some(*own),
        (None, enclosing) => enclosing,
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::types::Rgba8;
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
        let full = rasterize_draw_commands_rgba8(20, 20, core::slice::from_ref(&shadow))
            .expect("rasterizes");
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
