// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! What the GPU claims, and how a rectangle becomes an instance.
//!
//! Split out of [`super::encode`] so that file stays about *encoding* a stream the dispatcher has
//! already accepted: this module answers "can the GPU draw all of this without help?" and owns the
//! one record layout a filled rectangle uses.
use super::super::commands::WgpuDrawCommand;
use super::super::raster::{combine, effective_clip, rounded_rect_row_spans, stroke_rect_edges};
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

/// The rectangle an **axis-aligned** line covers, or `None` when it is diagonal or empty.
///
/// The CPU rasteriser stamps a square of `2 * half_w` pixels at every Bresenham step, with the
/// range inclusive at both ends (`x0 - half_w + 1 ..= x0 + half_w`). Along an axis those stamps
/// tile into exactly this rectangle — which is why the GPU can paint the line byte for byte. Along
/// a diagonal they do not, so such a line is reported as `None` and stays on the CPU rather than
/// being redrawn as a capsule of a different shape.
pub(super) fn axis_aligned_line_rect(
    from: (i32, i32),
    to: (i32, i32),
    width: u32,
) -> Option<PixelRect> {
    if width == 0 || (from.0 != to.0 && from.1 != to.1) {
        return None;
    }
    let half_w = (width as i64 / 2).max(1) as i32;
    if from.1 == to.1 {
        let left = from.0.min(to.0) - half_w + 1;
        let right = from.0.max(to.0) + half_w + 1;
        return Some(PixelRect {
            x: left,
            y: from.1 - half_w + 1,
            width: (right - left).max(0) as u32,
            height: (2 * half_w).max(0) as u32,
        });
    }
    let top = from.1.min(to.1) - half_w + 1;
    let bottom = from.1.max(to.1) + half_w + 1;
    Some(PixelRect {
        x: from.0 - half_w + 1,
        y: top,
        width: (2 * half_w).max(0) as u32,
        height: (bottom - top).max(0) as u32,
    })
}

/// Whether every segment of a stroked path is axis-aligned.
///
/// The CPU draws a stroked path as one `DrawLine` per segment, and an axis-aligned line is exactly
/// a rectangle; a path made only of such segments is therefore reproducible byte for byte. One
/// diagonal segment is enough to send the whole path back to the CPU, because that segment cannot
/// be drawn without changing its shape.
pub(super) fn stroke_is_axis_aligned(points: &[(i32, i32)], closed: bool) -> bool {
    if points.len() < 2 {
        return false;
    }
    let segments = if closed { points.len() } else { points.len() - 1 };
    (0..segments).all(|index| {
        let from = points[index];
        let to = points[(index + 1) % points.len()];
        from.0 == to.0 || from.1 == to.1
    })
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
        // See [`axis_aligned_line_rect`]: along an axis the CPU's stamped squares tile into
        // exactly one rectangle.
        WgpuDrawCommand::DrawLine { from, to, color, width, clip } => {
            let Some(rect) = axis_aligned_line_rect(*from, *to, *width) else { return true };
            push_clipped_instance(instances, framebuffer, *clip, stream_clip, rect, *color);
        }
        // One instance per axis-aligned segment, in path order. `width == 0` means one pixel to the
        // CPU's line helper, so the rectangle uses the same substituted width.
        WgpuDrawCommand::DrawPath { points, closed, color, filled: false, width, clip } => {
            if points.len() < 2 {
                return true;
            }
            let segments = if *closed { points.len() } else { points.len() - 1 };
            for index in 0..segments {
                let from = points[index];
                let to = points[(index + 1) % points.len()];
                let Some(rect) = axis_aligned_line_rect(from, to, (*width).max(1)) else {
                    continue;
                };
                push_clipped_instance(instances, framebuffer, *clip, stream_clip, rect, *color);
            }
        }
        _ => return false,
    }
    true
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
/// needs blending the pipelines do not implement. Text, path, arc and circle-stroke commands,
/// rounded-rectangle strokes, gradients and shadows stay on the CPU rasteriser, which remains the
/// fallback.
///
/// # What clipping costs, per class
///
/// A rectangle is narrowed to the clip, a circle or an image gets a `DrawStep::Scissor`, and both are
/// byte-identical to what the CPU rasteriser writes for the same command. The two classes therefore
/// need no clip-aware shader code at all.
///
/// An image whose payload does not match its declared grid is **not** dispatchable: the CPU
/// rasteriser turns that into an explicit `Err`, and the caller must not see one answer or the
/// other depending on which path the rest of the stream takes.
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
                && (*image_width as usize)
                    .checked_mul(*image_height as usize)
                    .and_then(|pixels| pixels.checked_mul(4))
                    == Some(rgba8.len())
        }
        WgpuDrawCommand::FillPathNonZero { points, contours, .. } => {
            points.len() >= 3 && !contours.is_empty()
        }
        WgpuDrawCommand::DrawPath { points, filled: true, .. } => points.len() >= 3,
        WgpuDrawCommand::DrawPath { points, closed, filled: false, .. } => {
            stroke_is_axis_aligned(points, *closed)
        }
        // An axis-aligned line is exactly a rectangle: the CPU stamps a `2*half_w` square at every
        // Bresenham step, and along a horizontal or vertical run those squares tile into one
        // rectangle. A diagonal line's stamps do not, so it stays with the CPU rasteriser rather
        // than being redrawn as a capsule and changing what the frame looks like.
        WgpuDrawCommand::DrawLine { from, to, width, .. } => {
            *width > 0 && (from.0 == to.0 || from.1 == to.1)
        }
        // An unblurred shadow is a fill of the offset body, which is exactly a rectangle. A blurred
        // one is a separate offscreen layer plus two box-blur passes and a composite, all of which
        // the renderer owns (`super::encode`), so it is dispatchable too.
        WgpuDrawCommand::BoxShadow { rect, .. } => rect.width > 0 && rect.height > 0,
        WgpuDrawCommand::SetBlendMode { mode } => *mode == 0,
    })
}
