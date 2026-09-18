// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Render commands for composing widget visuals.
use crate::compat::{String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::style::Gradient;

/// Compositing blend mode for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Source pixels replace the destination unchanged.
    Normal,
    /// Multiplication of source and destination; the result is never lighter
    /// than either input.
    Multiply,
    /// Complements the multiplication of the inverted colors; the result is
    /// never darker than either input.
    Screen,
    /// `Multiply` or `Screen` depending on the destination color, increasing
    /// contrast while preserving highlights and shadows.
    Overlay,
    /// Keeps the darker of source and destination per channel.
    Darken,
    /// Keeps the lighter of source and destination per channel.
    Lighten,
    /// Brightens the destination to reflect the source, dividing by the
    /// inverted destination color.
    ColorDodge,
    /// Darkens the destination to reflect the source, dividing the inverted
    /// source by the destination color.
    ColorBurn,
    /// `Multiply` or `Screen` depending on the source color.
    HardLight,
    /// A softened `HardLight` that behaves like a diffuse light source and
    /// never produces pure black or pure white.
    SoftLight,
    /// Absolute difference between source and destination; inverting either
    /// input inverts the result.
    Difference,
    /// Like `Difference` but with lower contrast.
    Exclusion,
    /// Preserves the luminosity and saturation of the destination while taking
    /// the hue of the source.
    Hue,
    /// Preserves the hue and luminosity of the destination while taking the
    /// saturation of the source.
    Saturation,
    /// Preserves the luminosity of the destination while taking the hue and
    /// saturation of the source; tints the destination.
    Color,
    /// Preserves the hue and saturation of the destination while taking the
    /// luminosity of the source.
    Luminosity,
}

/// Draw command recorded by a render layer.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Fill the rectangle `rect` with `color`.
    FillRect {
        /// Region to fill in logical pixels.
        rect: Rect,
        /// Fill color.
        color: Color,
    },
    /// Outline the rectangle `rect` with a one-pixel-wide stroke.
    DrawRect {
        /// Rectangle to outline in logical pixels.
        rect: Rect,
        /// Stroke color.
        color: Color,
    },
    /// Outline the rectangle `rect` with a stroke `width` pixels wide.
    /// Draws nothing when `width` is zero or the rectangle is empty.
    DrawRectStroke {
        /// Rectangle to outline in logical pixels.
        rect: Rect,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Fill the rectangle `rect` with `color`, rounding each corner to
    /// `radius` pixels. The effective radius is clamped to half the shorter
    /// side, so the shape never degenerates.
    FillRoundedRect {
        /// Region to fill in logical pixels.
        rect: Rect,
        /// Corner radius in pixels, clamped to half the shorter side.
        radius: u32,
        /// Fill color.
        color: Color,
    },
    /// Same as `FillRoundedRect` but computes the corner coverage from a
    /// sample grid instead of a single sample, using the backend's
    /// anti-aliasing sample count. Visually identical on a conforming
    /// backend and only more expensive; mainly useful for large radii.
    FillRoundedRectAA {
        /// Region to fill in logical pixels.
        rect: Rect,
        /// Corner radius in pixels, clamped to half the shorter side.
        radius: u32,
        /// Fill color.
        color: Color,
    },
    /// Outline the rounded rectangle `rect` with a stroke `width` pixels
    /// wide. The stroke is the area between the outer rounded rect and the
    /// same rect inset by `width`, so it is drawn inside the bounds.
    /// Draws nothing when `width` is zero or the rectangle is empty.
    DrawRoundedRectStroke {
        /// Rectangle to outline in logical pixels.
        rect: Rect,
        /// Outer corner radius in pixels.
        radius: u32,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Same as `DrawRoundedRectStroke` but derives the inner and outer
    /// coverage from a sample grid, using the backend's anti-aliasing sample
    /// count for sharper curved strokes.
    DrawRoundedRectStrokeAA {
        /// Rectangle to outline in logical pixels.
        rect: Rect,
        /// Outer corner radius in pixels.
        radius: u32,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Draw a one-pixel-wide line segment between `from` and `to` using
    /// Bresenham rasterization, so the line is hard-edged and unfilled at the
    /// endpoints. Only the line's length and direction matter, not the
    /// order of the endpoints.
    DrawLine {
        /// First endpoint in logical pixels.
        from: Point,
        /// Second endpoint in logical pixels.
        to: Point,
        /// Stroke color.
        color: Color,
    },
    /// Draw a one-pixel-wide, anti-aliased line segment between `from` and
    /// `to`.
    DrawLineAA {
        /// First endpoint in logical pixels.
        from: Point,
        /// Second endpoint in logical pixels.
        to: Point,
        /// Stroke color.
        color: Color,
    },
    /// Draw an anti-aliased line segment between `from` and `to` with an
    /// explicit stroke `width`.
    DrawLineStrokeAA {
        /// First endpoint in logical pixels.
        from: Point,
        /// Second endpoint in logical pixels.
        to: Point,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Draw a hard-edged line segment between `from` and `to` with an
    /// explicit stroke `width`.
    DrawLineStroke {
        /// First endpoint in logical pixels.
        from: Point,
        /// Second endpoint in logical pixels.
        to: Point,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Fill the disc centered on `center` with the given `radius`. The edge is
    /// hard-edged (the outline is drawn without coverage blending), so a small
    /// radius looks blocky; prefer `FillCircleAA` when the circle is large
    /// enough for the aliasing to show.
    FillCircle {
        /// Center in logical pixels.
        center: Point,
        /// Circle radius in pixels; a zero radius draws nothing.
        radius: u32,
        /// Fill color.
        color: Color,
    },
    /// Fill the disc centered on `center`, anti-aliasing the edge with the
    /// backend's sample grid.
    FillCircleAA {
        /// Center in logical pixels.
        center: Point,
        /// Circle radius in pixels; a zero radius draws nothing.
        radius: u32,
        /// Fill color.
        color: Color,
    },
    /// Outline the circle centered on `center` with a one-pixel-wide stroke.
    /// The ring's centerline lies on `radius`, and the outline is
    /// anti-aliased even though this variant has no `AA` suffix.
    DrawCircle {
        /// Center in logical pixels.
        center: Point,
        /// Circle radius in pixels; a zero radius draws nothing.
        radius: u32,
        /// Stroke color.
        color: Color,
    },
    /// Outline the circle centered on `center` with a stroke `width` pixels
    /// wide. The ring straddles `radius` (it spans `radius - width/2` to
    /// `radius + width/2`), so it is not confined to the circle's interior.
    DrawCircleStroke {
        /// Center in logical pixels.
        center: Point,
        /// Circle radius in pixels, marking the stroke centerline.
        radius: u32,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Draw a single line of text.
    DrawText {
        /// Anchor point in logical pixels. `y` is the top of the text box;
        /// horizontal placement is controlled by `alignment`.
        origin: Point,
        /// Text to draw; supports combining marks and variation selectors.
        text: String,
        /// Font family, size, weight and style used to rasterize the glyphs.
        font: Font,
        /// Glyph color.
        color: Color,
        /// How `text` is placed horizontally relative to `origin.x`.
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
        /// Region to fill in logical pixels.
        rect: Rect,
        /// Gradient stops and direction; see [`crate::style::Gradient`] for how the
        /// stops are mapped onto `rect`.
        gradient: Gradient,
    },
    /// Draw an arc (partial circle).
    DrawArc {
        /// Center in logical pixels.
        center: Point,
        /// Arc radius in pixels; a zero radius draws nothing.
        radius: u32,
        /// Sweep start angle in radians, measured clockwise from the positive
        /// x axis. Normalized into `[0, 2π)` before use.
        start_angle: f32,
        /// Sweep end angle in radians, same convention as `start_angle`. When
        /// it is not greater than `start_angle` the arc wraps through 0, so a
        /// `start` of 350° and an `end` of 10° sweeps 20°, not 340°.
        end_angle: f32,
        /// Stroke color, or fill color when `filled` is true.
        color: Color,
        /// Fills the pie wedge back to the center instead of stroking the arc.
        filled: bool,
    },
    /// Draw a path defined by a list of points.
    DrawPath {
        /// Vertices in logical pixels, connected in order. Fewer than two
        /// points draws nothing.
        points: Vec<Point>,
        /// Connects the last point back to the first when stroking. Ignored
        /// when `filled` is true, because a fill is always closed implicitly.
        closed: bool,
        /// Stroke color, or fill color when `filled` is true.
        color: Color,
        /// Fills the polygon spanned by `points` with a scanline rasterizer
        /// instead of stroking it.
        filled: bool,
        /// Stroke width in pixels; ignored when `filled` is true.
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
        /// Mode applied to later draw commands.
        mode: BlendMode,
    },
    /// Draw a conic (angular/sweep) gradient (BLUE11 R5.7).
    DrawConicGradient {
        /// Center point of the gradient.
        center: Point,
        /// Starting angle in radians.
        start_angle: f32,
        /// Color stops as (position \[0,1\], color) pairs.
        stops: Vec<(f32, Color)>,
    },
}
