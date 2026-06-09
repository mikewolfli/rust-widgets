//! Render commands for composing widget visuals.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::style::Gradient;

/// Compositing blend mode for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

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
        alignment: HorizontalAlignment,
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
    /// Draw a filled gradient rectangle.
    DrawGradient {
        rect: Rect,
        gradient: Gradient,
    },
    /// Draw an arc (partial circle).
    DrawArc {
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        filled: bool,
    },
    /// Draw a path defined by a list of points.
    DrawPath {
        points: Vec<Point>,
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    },
    /// Draw a drop shadow behind a rectangle (BLUE11 R5.3).
    BoxShadow {
        /// The shadow's rectangular bounds.
        rect: Rect,
        /// Shadow color with alpha.
        color: Color,
        /// Horizontal offset in pixels.
        offset_x: i32,
        /// Vertical offset in pixels.
        offset_y: i32,
        /// Blur radius in pixels.
        blur_radius: u32,
        /// Spread radius in pixels (positive expands, negative contracts).
        spread: i32,
    },
    /// Apply a Gaussian blur to the current clip region (BLUE11 R5.4).
    Blur {
        /// Blur radius in pixels.
        radius: u32,
    },
    /// Set a clip path from a list of points (BLUE11 R5.5).
    ClipPath {
        /// Points defining the clip path.
        points: Vec<Point>,
    },
    /// Set blend mode for subsequent draw commands (BLUE11 R5.6).
    SetBlendMode {
        mode: BlendMode,
    },
    /// Draw a conic (angular/sweep) gradient (BLUE11 R5.7).
    DrawConicGradient {
        /// Center point of the gradient.
        center: Point,
        /// Starting angle in radians.
        start_angle: f32,
        /// Color stops as (position [0,1], color) pairs.
        stops: Vec<(f32, Color)>,
    },
}
