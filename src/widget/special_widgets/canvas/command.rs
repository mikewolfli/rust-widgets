// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Command mapping for [`Canvas`](super::Canvas): transform, grid snapping, bounds and culling.
//!
//! These functions read only a [`RenderCommand`] and the canvas transform, so they live beside
//! `widget.rs` instead of inside it rather than enlarging the widget implementation past the
//! 2000-line structure limit (principle #9). Replay, export and the layer cache all call the same
//! mapping, which is what keeps a zoom or a snap from disagreeing between those paths.

use crate::compat::Arc;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::render::RenderCommand;

use super::CanvasTransform;

fn snap_scalar(value: i32, step: i32) -> i32 {
    let step = step.max(1);
    (value as f32 / step as f32).round() as i32 * step
}

/// Packs a colour into the low 32 bits of a layer-cache key, so a theme change that moves the
/// background or the grid colour also invalidates the cached grid.
pub(super) fn color_key(color: Color) -> u64 {
    (u64::from(color.r) << 24)
        | (u64::from(color.g) << 16)
        | (u64::from(color.b) << 8)
        | u64::from(color.a)
}

/// Maps a logical command through `transform` into surface coordinates.
///
/// This is the one place the canvas applies its transform, so replay, export and the cached
/// copy in [`StoredCommand::replay`] cannot disagree about what a zoom does.
pub(super) fn transform_command(
    command: &RenderCommand,
    transform: CanvasTransform,
) -> RenderCommand {
    let width = |value: u32| (value as f32 * transform.scale).round().max(1.0) as u32;
    match command {
        RenderCommand::FillRect { rect, color } => {
            RenderCommand::FillRect { rect: transform.rect(*rect), color: *color }
        }
        RenderCommand::DrawRect { rect, color } => {
            RenderCommand::DrawRect { rect: transform.rect(*rect), color: *color }
        }
        RenderCommand::FillRoundedRect { rect, radii, color } => RenderCommand::FillRoundedRect {
            rect: transform.rect(*rect),
            radii: radii.map(&width),
            color: *color,
        },
        RenderCommand::DrawLine { from, to, color } => RenderCommand::DrawLine {
            from: transform.point(*from),
            to: transform.point(*to),
            color: *color,
        },
        RenderCommand::DrawLineAA { from, to, color } => RenderCommand::DrawLineAA {
            from: transform.point(*from),
            to: transform.point(*to),
            color: *color,
        },
        RenderCommand::DrawLineStroke { from, to, color, width: stroke } => {
            RenderCommand::DrawLineStroke {
                from: transform.point(*from),
                to: transform.point(*to),
                color: *color,
                width: width(*stroke),
            }
        }
        RenderCommand::FillCircle { center, radius, color } => RenderCommand::FillCircle {
            center: transform.point(*center),
            radius: width(*radius),
            color: *color,
        },
        RenderCommand::FillCircleAA { center, radius, color } => RenderCommand::FillCircleAA {
            center: transform.point(*center),
            radius: width(*radius),
            color: *color,
        },
        RenderCommand::DrawCircle { center, radius, color } => RenderCommand::DrawCircle {
            center: transform.point(*center),
            radius: width(*radius),
            color: *color,
        },
        RenderCommand::DrawCircleStroke { center, radius, color, width: stroke } => {
            RenderCommand::DrawCircleStroke {
                center: transform.point(*center),
                radius: width(*radius),
                color: *color,
                width: width(*stroke),
            }
        }
        RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
            RenderCommand::DrawArc {
                center: transform.point(*center),
                radius: width(*radius),
                start_angle: *start_angle,
                end_angle: *end_angle,
                color: *color,
                filled: *filled,
            }
        }
        RenderCommand::DrawPath { points, closed, color, filled, width: stroke } => {
            RenderCommand::DrawPath {
                points: points
                    .iter()
                    .map(|point| transform.point(*point))
                    .collect::<Vec<_>>()
                    .into(),
                closed: *closed,
                color: *color,
                filled: *filled,
                width: width(*stroke),
            }
        }
        RenderCommand::DrawPath3 { a, b, c, closed, color, filled, width: stroke } => {
            RenderCommand::DrawPath3 {
                a: transform.point(*a),
                b: transform.point(*b),
                c: transform.point(*c),
                closed: *closed,
                color: *color,
                filled: *filled,
                width: width(*stroke),
            }
        }
        RenderCommand::DrawText { origin, text, font, color, alignment } => {
            RenderCommand::DrawText {
                origin: transform.point(*origin),
                text: Arc::clone(text),
                font: font.clone(),
                color: *color,
                alignment: *alignment,
            }
        }
        RenderCommand::DrawImage { x, y, width: w, height: h, data } => RenderCommand::DrawImage {
            x: transform.point(Point::new(*x, *y)).x,
            y: transform.point(Point::new(*x, *y)).y,
            width: width(*w),
            height: width(*h),
            data: Arc::clone(data),
        },
        RenderCommand::DrawImageScaled {
            x,
            y,
            width: w,
            height: h,
            source_width,
            source_height,
            data,
        } => RenderCommand::DrawImageScaled {
            x: transform.point(Point::new(*x, *y)).x,
            y: transform.point(Point::new(*x, *y)).y,
            width: width(*w),
            height: width(*h),
            // The source grid is a property of the pixels, not of the transform.
            source_width: *source_width,
            source_height: *source_height,
            data: Arc::clone(data),
        },
        other => other.clone(),
    }
}

/// Snaps a point to the canvas grid.
fn snap_point(point: Point, step: i32) -> Point {
    Point::new(snap_scalar(point.x, step), snap_scalar(point.y, step))
}

/// Snaps a rectangle's origin and size to the canvas grid.
fn snap_rect(rect: Rect, step: i32) -> Rect {
    let step = step.max(1);
    Rect::new(
        snap_scalar(rect.x, step),
        snap_scalar(rect.y, step),
        snap_scalar(rect.width as i32, step).max(1) as u32,
        snap_scalar(rect.height as i32, step).max(1) as u32,
    )
}

/// Applies grid snapping to the coordinates a command carries.
pub(super) fn snap_command(command: RenderCommand, step: i32) -> RenderCommand {
    match command {
        RenderCommand::FillRect { rect, color } => {
            RenderCommand::FillRect { rect: snap_rect(rect, step), color }
        }
        RenderCommand::DrawRect { rect, color } => {
            RenderCommand::DrawRect { rect: snap_rect(rect, step), color }
        }
        RenderCommand::FillRoundedRect { rect, radii, color } => {
            RenderCommand::FillRoundedRect { rect: snap_rect(rect, step), radii, color }
        }
        RenderCommand::DrawLine { from, to, color } => RenderCommand::DrawLine {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
        },
        RenderCommand::DrawLineAA { from, to, color } => RenderCommand::DrawLineAA {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
        },
        RenderCommand::DrawLineStroke { from, to, color, width } => RenderCommand::DrawLineStroke {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
            width,
        },
        RenderCommand::FillCircle { center, radius, color } => {
            RenderCommand::FillCircle { center: snap_point(center, step), radius, color }
        }
        RenderCommand::FillCircleAA { center, radius, color } => {
            RenderCommand::FillCircleAA { center: snap_point(center, step), radius, color }
        }
        RenderCommand::DrawCircle { center, radius, color } => {
            RenderCommand::DrawCircle { center: snap_point(center, step), radius, color }
        }
        RenderCommand::DrawCircleStroke { center, radius, color, width } => {
            RenderCommand::DrawCircleStroke {
                center: snap_point(center, step),
                radius,
                color,
                width,
            }
        }
        RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
            RenderCommand::DrawArc {
                center: snap_point(center, step),
                radius,
                start_angle,
                end_angle,
                color,
                filled,
            }
        }
        RenderCommand::DrawImage { x, y, width, height, data } => {
            let rect = snap_rect(Rect::new(x, y, width, height), step);
            RenderCommand::DrawImage {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                data,
            }
        }
        RenderCommand::DrawImageScaled {
            x,
            y,
            width,
            height,
            source_width,
            source_height,
            data,
        } => {
            let rect = snap_rect(Rect::new(x, y, width, height), step);
            RenderCommand::DrawImageScaled {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                source_width,
                source_height,
                data,
            }
        }
        RenderCommand::DrawPath { points, closed, color, filled, width } => {
            RenderCommand::DrawPath {
                points: points
                    .iter()
                    .map(|point| snap_point(*point, step))
                    .collect::<Vec<_>>()
                    .into(),
                closed,
                color,
                filled,
                width,
            }
        }
        RenderCommand::DrawPath3 { a, b, c, closed, color, filled, width } => {
            RenderCommand::DrawPath3 {
                a: snap_point(a, step),
                b: snap_point(b, step),
                c: snap_point(c, step),
                closed,
                color,
                filled,
                width,
            }
        }
        RenderCommand::DrawText { origin, text, font, color, alignment } => {
            RenderCommand::DrawText {
                origin: snap_point(origin, step),
                text,
                font,
                color,
                alignment,
            }
        }
        other => other,
    }
}

/// Replaces anti-aliased variants with their plain counterparts.
///
/// The canvas-wide switch must win over a per-command variant, otherwise a
/// caller that disabled anti-aliasing would still get it on every command that
/// happened to use the `*AA` constructor.
pub(super) fn without_anti_aliasing(command: RenderCommand) -> RenderCommand {
    match command {
        RenderCommand::DrawLineAA { from, to, color } => {
            RenderCommand::DrawLine { from, to, color }
        }
        RenderCommand::FillCircleAA { center, radius, color } => {
            RenderCommand::FillCircle { center, radius, color }
        }
        other => other,
    }
}

/// The positive extent between two signed coordinates.
///
/// The difference is formed in `i64`: `i32::MAX - i32::MIN` is outside `i32`, so the old
/// `(a - b).unsigned_abs()` overflowed before it could take the absolute value (a debug panic on
/// `draw_line(Point(0, 0), Point(i32::MIN, 0))`). The result always fits `u32`, because the
/// widest pair of `i32`s spans exactly `u32::MAX`.
fn axis_span(a: i32, b: i32) -> u32 {
    (i64::from(a) - i64::from(b)).unsigned_abs().max(1) as u32
}

/// The approximate painted box of a text command.
///
/// The canvas stores text as a command like any other mark, and the index only ever held
/// commands with bounds, so text painted and could never be picked. The shared text estimate
/// (the same one the controls measure with) gives it a box good enough for a hit test; the
/// alignment is applied the way the backends apply it.
fn text_bounds(
    origin: Point,
    text: &str,
    font: &crate::core::Font,
    alignment: HorizontalAlignment,
) -> Rect {
    let width = crate::widget::metrics::estimate_text_width(text, font, 1.0).max(1);
    let height = crate::widget::metrics::estimate_line_height(font, 1.0);
    let x = match alignment {
        HorizontalAlignment::Left => origin.x,
        HorizontalAlignment::Center => origin.x.saturating_sub(width as i32 / 2),
        HorizontalAlignment::Right => origin.x.saturating_sub(width as i32),
    };
    Rect::new(x, origin.y, width, height)
}

/// The axis-aligned bounds of a command, when it has any.
pub(super) fn command_bounds(command: &RenderCommand) -> Option<Rect> {
    match command {
        RenderCommand::FillRect { rect, .. } | RenderCommand::DrawRect { rect, .. } => Some(*rect),
        RenderCommand::FillRoundedRect { rect, .. } => Some(*rect),
        RenderCommand::FillCircle { center, radius, .. }
        | RenderCommand::FillCircleAA { center, radius, .. }
        | RenderCommand::DrawCircle { center, radius, .. }
        | RenderCommand::DrawCircleStroke { center, radius, .. } => Some(Rect::new(
            center.x - *radius as i32,
            center.y - *radius as i32,
            radius.saturating_mul(2).max(1),
            radius.saturating_mul(2).max(1),
        )),
        RenderCommand::DrawLine { from, to, .. }
        | RenderCommand::DrawLineAA { from, to, .. }
        | RenderCommand::DrawLineStroke { from, to, .. } => Some(Rect::new(
            from.x.min(to.x),
            from.y.min(to.y),
            axis_span(from.x, to.x),
            axis_span(from.y, to.y),
        )),
        RenderCommand::DrawPath { points, .. } => {
            let first = points.first()?;
            let (mut left, mut top, mut right, mut bottom) = (first.x, first.y, first.x, first.y);
            for point in points.iter() {
                left = left.min(point.x);
                top = top.min(point.y);
                right = right.max(point.x);
                bottom = bottom.max(point.y);
            }
            Some(Rect::new(left, top, axis_span(right, left), axis_span(bottom, top)))
        }
        RenderCommand::DrawPath3 { a, b, c, .. } => {
            let left = a.x.min(b.x).min(c.x);
            let top = a.y.min(b.y).min(c.y);
            let right = a.x.max(b.x).max(c.x);
            let bottom = a.y.max(b.y).max(c.y);
            Some(Rect::new(left, top, axis_span(right, left), axis_span(bottom, top)))
        }
        RenderCommand::DrawArc { center, radius, .. } => Some(Rect::new(
            center.x - *radius as i32,
            center.y - *radius as i32,
            radius.saturating_mul(2).max(1),
            radius.saturating_mul(2).max(1),
        )),
        RenderCommand::DrawImage { x, y, width, height, .. }
        | RenderCommand::DrawImageScaled { x, y, width, height, .. } => {
            Some(Rect::new(*x, *y, *width, *height))
        }
        RenderCommand::DrawText { origin, text, font, alignment, .. } => {
            Some(text_bounds(*origin, text, font, *alignment))
        }
        _ => None,
    }
}

/// The visible surface rectangle expressed in the commands' logical coordinates.
///
/// Chunk bounds are stored in logical space, so culling them against this rectangle avoids
/// transforming every block and the rounding that would come with it.
pub(super) fn logical_viewport(transform: CanvasTransform, rect: Rect) -> Rect {
    let top_left = transform.inverse_point(Point::new(rect.x, rect.y));
    let bottom_right = transform
        .inverse_point(Point::new(rect.x + rect.width as i32, rect.y + rect.height as i32));
    let x = top_left.x.min(bottom_right.x);
    let y = top_left.y.min(bottom_right.y);
    Rect::new(x, y, (bottom_right.x - x).max(0) as u32, (bottom_right.y - y).max(0) as u32)
}

/// Whether two rectangles overlap.
pub(super) fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width as i32
        && b.x < a.x + a.width as i32
        && a.y < b.y + b.height as i32
        && b.y < a.y + a.height as i32
}
