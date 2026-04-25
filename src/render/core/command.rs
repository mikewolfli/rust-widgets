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
    /// Draw an image at the specified position and size.
    DrawImage {
        /// Top-left screen position in logical pixels.
        x: i32,
        /// Top-left screen position in logical pixels.
        y: i32,
        /// Image width in logical pixels.
        width: u32,
        /// Image height in logical pixels.
        height: u32,
        /// RGBA pixel data (4 bytes per pixel).
        data: Vec<u8>,
    },
    /// Push a clipping rectangle onto the clip stack.
    PushClip {
        /// Left edge in logical pixels.
        x: i32,
        /// Top edge in logical pixels.
        y: i32,
        /// Clip width in logical pixels.
        width: u32,
        /// Clip height in logical pixels.
        height: u32,
    },
    /// Pop the top clipping rectangle from the clip stack.
    PopClip,
}
