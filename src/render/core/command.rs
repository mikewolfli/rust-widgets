//! Render commands for composing widget visuals.
use crate::core::{Color, Font, Point, Rect};

/// Draw command recorded by a render layer.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    FillRect {
        rect: Rect,
        color: Color,
    },
    DrawRect {
        rect: Rect,
        color: Color,
    },
    DrawRectStroke {
        rect: Rect,
        color: Color,
        width: u32,
    },
    FillRoundedRect {
        rect: Rect,
        radius: u32,
        color: Color,
    },
    FillRoundedRectAA {
        rect: Rect,
        radius: u32,
        color: Color,
    },
    DrawRoundedRectStroke {
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawRoundedRectStrokeAA {
        rect: Rect,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawLine {
        from: Point,
        to: Point,
        color: Color,
    },
    DrawLineAA {
        from: Point,
        to: Point,
        color: Color,
    },
    DrawLineStrokeAA {
        from: Point,
        to: Point,
        color: Color,
        width: u32,
    },
    DrawLineStroke {
        from: Point,
        to: Point,
        color: Color,
        width: u32,
    },
    FillCircle {
        center: Point,
        radius: u32,
        color: Color,
    },
    FillCircleAA {
        center: Point,
        radius: u32,
        color: Color,
    },
    DrawCircle {
        center: Point,
        radius: u32,
        color: Color,
    },
    DrawCircleStroke {
        center: Point,
        radius: u32,
        color: Color,
        width: u32,
    },
    DrawText {
        origin: Point,
        text: String,
        font: Font,
        color: Color,
    },
}
