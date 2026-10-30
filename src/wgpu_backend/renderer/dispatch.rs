// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What the GPU claims, and how a rectangle becomes an instance.
//!
//! Split out of [`super::encode`] so that file stays about *encoding* a stream the dispatcher has
//! already accepted: this module answers "can the GPU draw all of this without help?" and owns the
//! one record layout a filled rectangle uses.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{
    bbox_for_path, clip_segment_to_rect, combine, effective_clip, for_each_line_stamp,
    rect_for_line, rounded_rect_row_spans, stroke_rect_edges,
};
use super::super::types::PixelRect;
use super::super::types::Rgba8;
use super::encode::INSTANCE_BYTES;

/// The shape fragment shader's mode selector.
pub(super) const SHAPE_CIRCLE_STROKE: f32 = 0.0;
pub(super) const SHAPE_ARC_WEDGE: f32 = 1.0;
pub(super) const SHAPE_ARC_STROKE: f32 = 2.0;
pub(super) const SHAPE_ROUNDED_OUTLINE: f32 = 3.0;

/// Appends one instanced-quad record for `rect` filled with `color`.
///
/// This is the whole of the 2D fill: the fragment shader interpolates the record's own colour, so
/// a clip, a stroke edge and a rounded-rectangle row span are all "just rectangles" here — and
/// therefore byte-identical to what the CPU rasteriser writes when it fills the same pixel span.
pub(super) fn push_instance(instances: &mut Vec<u8>, rect: PixelRect, color: Rgba8) {
    let (x, y) = (rect.x, rect.y);
    let (x1, y1) = (rect.right(), rect.bottom());
    let mut record = [0u8; INSTANCE_BYTES];
    for (corner, (px, py)) in [(x, y), (x1, y), (x1, y1), (x, y1)].iter().enumerate() {
        record[corner * 8..corner * 8 + 4].copy_from_slice(&(*px as f32).to_le_bytes());
        record[corner * 8 + 4..corner * 8 + 8].copy_from_slice(&(*py as f32).to_le_bytes());
    }
    for (channel, value) in [color.r, color.g, color.b, color.a].iter().enumerate() {
        record[32 + channel * 4..32 + channel * 4 + 4]
            .copy_from_slice(&((*value as f64 / 255.0) as f32).to_le_bytes());
    }
    instances.extend_from_slice(&record);
}

/// Appends one instance per stamped square of a line, narrowed exactly as the CPU blends them.
///
/// The CPU walks Bresenham from the segment **clamped to the visible rect** and stamps a square of
/// `2 * half_w` pixels at every step, so one instance per step is the same picture — which is why a
/// diagonal line no longer has to stay on the CPU. Clamping the segment first is not an
/// optimisation of the picture: it is what the CPU does, and it also bounds the walk.
pub(super) fn push_line_instances(
    instances: &mut Vec<u8>,
    framebuffer: PixelRect,
    clip: Option<PixelRect>,
    stream_clip: Option<PixelRect>,
    from: (i32, i32),
    to: (i32, i32),
    width: u32,
    color: Rgba8,
) {
    let Some(clip_rect) =
        effective_clip(framebuffer, rect_for_line(from, to, width), combine(&clip, stream_clip))
    else {
        return;
    };
    let Some((from, to)) = clip_segment_to_rect(from, to, &clip_rect) else { return };
    push_line_stamps(instances, framebuffer, Some(clip_rect), from, to, width, color);
}

/// Appends one instance per stamped square of `from → to`, clipped to `clip` (already narrowed).
fn push_line_stamps(
    instances: &mut Vec<u8>,
    framebuffer: PixelRect,
    clip: Option<PixelRect>,
    from: (i32, i32),
    to: (i32, i32),
    width: u32,
    color: Rgba8,
) {
    for_each_line_stamp(from, to, width, |x, y, half| {
        // The square the CPU fills, in the same inclusive `centre - half + 1 ..= centre + half`
        // rule. Saturation only matters for arithmetic that a clip will discard anyway.
        let left = x - half + 1;
        let top = y - half + 1;
        let size = (2 * half).clamp(0, u32::MAX as i64);
        let rect = PixelRect {
            x: left.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y: top.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            width: size as u32,
            height: size as u32,
        };
        push_clipped_instance(instances, framebuffer, clip, None, rect, color);
    });
}

/// Appends the instance for `rect` narrowed by the command's own clip and the running stream
/// clip, or appends nothing when the intersection is empty.
fn push_clipped_instance(
    instances: &mut Vec<u8>,
    framebuffer: PixelRect,
    clip: Option<PixelRect>,
    stream_clip: Option<PixelRect>,
    rect: PixelRect,
    color: Rgba8,
) {
    let Some(draw_rect) = effective_clip(framebuffer, rect, combine(&clip, stream_clip)) else {
        return;
    };
    push_instance(instances, draw_rect, color);
}

/// Packs the **rectangle class** of commands, whose entire GPU form is instanced rectangles.
///
/// Every arm narrows its geometry by the framebuffer and the running clip exactly as the CPU
/// rasteriser does, so packing them here rather than in the encoder keeps the "command becomes
/// instances" rule in one place. Returns `true` when the command belonged to this class and has
/// been fully packed — including the case where it packed nothing because it is empty or was
/// clipped away — and `false` when the caller still has to handle it.
pub(super) fn pack_rect_command(
    command: &WgpuDrawCommand,
    framebuffer: PixelRect,
    stream_clip: Option<PixelRect>,
    instances: &mut Vec<u8>,
) -> bool {
    match command {
        // The CPU rasteriser narrows the rectangle by the framebuffer and the running clip, then
        // fills the result; the same narrowed rectangle is the instance here, so clipping costs
        // this path no state at all.
        WgpuDrawCommand::FillRect { rect, color, clip } => {
            push_clipped_instance(instances, framebuffer, *clip, stream_clip, *rect, *color);
        }
        // The four edges the CPU arm fills, narrowed exactly as it narrows them.
        WgpuDrawCommand::StrokeRect { rect, color, thickness, clip } => {
            for edge in stroke_rect_edges(*rect, *thickness) {
                push_clipped_instance(instances, framebuffer, *clip, stream_clip, edge, *color);
            }
        }
        // One instance per row span, from the same integer rule the CPU arm uses. What moves to the
        // GPU is the per-pixel fill, not the geometry, so the corner arcs are identical rather
        // than approximated.
        WgpuDrawCommand::FillRoundedRect { rect, radius, color, clip } => {
            for span in rounded_rect_row_spans(*rect, *radius) {
                push_clipped_instance(instances, framebuffer, *clip, stream_clip, span, *color);
            }
        }
        // The shadow body is the rectangle translated by the offset, and an unblurred shadow
        // paints exactly that body — the CPU arm's own `fill_rect` of the intersected body, so
        // this is byte-identical.
        WgpuDrawCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius: 0, clip } => {
            let shadow = PixelRect {
                x: rect.x.saturating_add(*offset_x),
                y: rect.y.saturating_add(*offset_y),
                width: rect.width,
                height: rect.height,
            };
            push_clipped_instance(instances, framebuffer, *clip, stream_clip, shadow, *color);
        }
        // See [`push_line_instances`]: one instance per stamped square, at any angle.
        WgpuDrawCommand::DrawLine { from, to, color, width, clip } => {
            push_line_instances(
                instances,
                framebuffer,
                *clip,
                stream_clip,
                *from,
                *to,
                *width,
                *color,
            );
        }
        // One run of stamped squares per segment, in path order. The CPU gives each segment its own
        // `rect_for_line ∩ clip` window and draws nothing at all when the width is zero, so the same
        // window and the same early exit are used here.
        WgpuDrawCommand::DrawPath { points, closed, color, filled: false, width, clip } => {
            if points.len() < 2 || *width == 0 {
                return true;
            }
            let Some(window) = effective_clip(
                framebuffer,
                bbox_for_path(points, *width),
                combine(clip, stream_clip),
            ) else {
                return true;
            };
            let segments = if *closed { points.len() } else { points.len() - 1 };
            for index in 0..segments {
                let from = points[index];
                let to = points[(index + 1) % points.len()];
                let Some(segment_clip) = rect_for_line(from, to, *width).intersect(window) else {
                    continue;
                };
                push_line_stamps(
                    instances,
                    framebuffer,
                    Some(segment_clip),
                    from,
                    to,
                    *width,
                    *color,
                );
            }
        }
        _ => return false,
    }
    true
}

/// Whether a `DrawImage`'s payload is exactly the grid its `image_width × image_height` declares.
///
/// The multiplication is checked: two large dimensions must not wrap into a small "expected" length
/// that a short buffer could accidentally satisfy.
fn image_payload_fits(image_width: u32, image_height: u32, rgba8: &[u8]) -> bool {
    (image_width as usize)
        .checked_mul(image_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        == Some(rgba8.len())
}

/// The first image whose payload does not match its declared grid, with the message the CPU
/// rasteriser would produce for it.
///
/// The command stream is a caller-owned list of bytes, so a payload can disagree with its grid.
/// The CPU rasteriser reports that as an error rather than drawing part of the image, and the GPU
/// path must report the *same* error — otherwise the answer would depend on which path happened to
/// draw the rest of the stream. Checking here, before the dispatcher decides, is what lets a stream
/// containing one such image still be answered by one path: the error comes first.
///
/// The skip order is the CPU's: an image command that draws nothing (an empty box or an empty
/// declared grid) returns before the payload is looked at, so such a stream is not an error even
/// when its payload is short.
pub(super) fn invalid_image_payload(commands: &[WgpuDrawCommand]) -> Option<String> {
    commands.iter().enumerate().find_map(|(index, command)| {
        let WgpuDrawCommand::DrawImage { rect, rgba8, image_width, image_height, .. } = command
        else {
            return None;
        };
        if rect.width == 0 || rect.height == 0 || *image_width == 0 || *image_height == 0 {
            return None;
        }
        if image_payload_fits(*image_width, *image_height, rgba8) {
            return None;
        }
        Some(format!(
            "invalid DrawImage payload at index {index}: expected {} bytes, got {}",
            (*image_width as usize) * (*image_height as usize) * 4,
            rgba8.len()
        ))
    })
}

/// Whether a command stream can be drawn entirely by the GPU pipelines (G-218).
///
/// The covered classes are: whole-frame clears, filled and stroked rectangles, filled circles,
/// filled rounded rectangles (as row-span instances), image draws, and the stream-state commands
/// [`WgpuDrawCommand::PushClip`] / [`WgpuDrawCommand::PopClip`] — a rectangle clip narrows the
/// geometry of the rectangles it covers and becomes a scissor for circles and images, both of which
/// are per-draw states rather than reasons to abandon the frame.
/// [`WgpuDrawCommand::SetBlendMode`] is allowed only at `Normal` (mode `0`), because every pipeline
/// writes with `REPLACE` blending except the image one, which composites source-over; any other mode
/// needs blending the pipelines do not implement.
///
/// Lines and stroked paths are claimed at any angle: the CPU's stamps are reproduced one instance
/// per Bresenham step, so a diagonal is no longer a reason to fall back.
///
/// # What clipping costs, per class
///
/// A rectangle is narrowed to the clip, a circle or an image gets a `DrawStep::Scissor`, and both are
/// byte-identical to what the CPU rasteriser writes for the same command. The two classes therefore
/// need no clip-aware shader code at all.
///
/// An image whose payload does not match its declared grid is **not** dispatchable. In practice the
/// entry point has already rejected such a stream through [`invalid_image_payload`], which reports
/// the CPU rasteriser's own error first; this arm is the second line of defence for a caller that
/// uses the predicate directly, so the encoder is never handed bytes it cannot upload.
pub(super) fn commands_gpu_dispatchable(commands: &[WgpuDrawCommand]) -> bool {
    commands.iter().all(|command| match command {
        WgpuDrawCommand::Clear { .. } => true,
        WgpuDrawCommand::FillRect { .. } => true,
        WgpuDrawCommand::StrokeRect { thickness, .. } => *thickness > 0,
        WgpuDrawCommand::FillCircle { .. } => true,
        WgpuDrawCommand::DrawCircle { radius, width, .. } => *radius > 0 && *width > 0,
        WgpuDrawCommand::DrawArc { radius, .. } => *radius > 0,
        WgpuDrawCommand::FillRoundedRect { radius, .. } => *radius > 0,
        WgpuDrawCommand::StrokeRoundedRect { radius, thickness, .. } => {
            *radius > 0 && *thickness > 0
        }
        WgpuDrawCommand::DrawGradient { rect, gradient_data, .. }
        | WgpuDrawCommand::FillLinearGradient { rect, gradient_data, .. } => {
            rect.width > 0 && rect.height > 0 && gradient_data.len() / 4 >= 1
        }
        WgpuDrawCommand::FillRadialGradient { rect, gradient_data, .. } => {
            rect.width > 0 && rect.height > 0 && gradient_data.len() / 4 >= 1
        }
        WgpuDrawCommand::DrawText { rect, text, .. } => {
            rect.width > 0 && rect.height > 0 && !text.is_empty()
        }
        WgpuDrawCommand::PushClip { .. } | WgpuDrawCommand::PopClip => true,
        WgpuDrawCommand::DrawImage { rgba8, image_width, image_height, .. } => {
            *image_width > 0
                && *image_height > 0
                && image_payload_fits(*image_width, *image_height, rgba8)
        }
        WgpuDrawCommand::FillPathNonZero { points, contours, .. } => {
            points.len() >= 3 && !contours.is_empty()
        }
        WgpuDrawCommand::DrawPath { points, filled: true, .. } => points.len() >= 3,
        // A stroked path is one `DrawLine` per segment on the CPU, and each of those is a run of
        // stamped squares the GPU can instance.
        WgpuDrawCommand::DrawPath { points, filled: false, .. } => points.len() >= 2,
        // An axis-aligned line's stamps tile into one rectangle; a diagonal's do not, but they are
        // still squares, so the GPU emits one instance per Bresenham step instead.
        WgpuDrawCommand::DrawLine { width, .. } => *width > 0,
        // An unblurred shadow is a fill of the offset body, which is exactly a rectangle. A blurred
        // one is a separate offscreen layer plus two box-blur passes and a composite, all of which
        // the renderer owns (`super::encode`), so it is dispatchable too.
        WgpuDrawCommand::BoxShadow { rect, .. } => rect.width > 0 && rect.height > 0,
        WgpuDrawCommand::SetBlendMode { mode } => *mode == 0,
    })
}
