// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Render commands for composing widget visuals.
use crate::compat::{Arc, String, Vec};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
#[cfg(feature = "3d")]
use crate::render::math3d::Mat4;
#[cfg(feature = "3d")]
use crate::render::mesh3d::{Camera, Mesh3D};
use crate::style::Gradient;

/// A packed set of closed contours used by a non-zero winding fill.
///
/// Contour ranges are half-open indexes into `points`. The constructor checks
/// every range so render backends can consume the shape without revalidating it.
#[derive(Debug, Clone, PartialEq)]
pub struct CompoundPath {
    points: Arc<[Point]>,
    contours: Arc<[(usize, usize)]>,
}

/// Why a compound path could not be constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompoundPathError {
    /// No contours were supplied.
    NoContours,
    /// A contour is empty, too short to enclose an area, overlaps its predecessor, or is out of
    /// range. The contained value is its zero-based index.
    InvalidContour(usize),
}

impl CompoundPath {
    /// Creates a compound path after validating its packed contour ranges.
    pub fn new(
        points: Vec<Point>,
        contours: Vec<(usize, usize)>,
    ) -> Result<Self, CompoundPathError> {
        Self::validate_ranges(points.len(), &contours)?;
        Ok(Self { points: points.into(), contours: contours.into() })
    }

    /// Checks contour ranges without taking ownership of the geometry.
    pub(crate) fn validate_ranges(
        points_len: usize,
        contours: &[(usize, usize)],
    ) -> Result<(), CompoundPathError> {
        if contours.is_empty() {
            return Err(CompoundPathError::NoContours);
        }
        let mut previous_end = 0;
        for (index, &(start, end)) in contours.iter().enumerate() {
            if start < previous_end || start >= end || end > points_len || end - start < 3 {
                return Err(CompoundPathError::InvalidContour(index));
            }
            previous_end = end;
        }
        Ok(())
    }

    /// Returns the packed device-space vertices.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Returns the half-open vertex range of each contour.
    pub fn contours(&self) -> &[(usize, usize)] {
        &self.contours
    }
}

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

/// What a [`RenderCommand::PushLayer`] does with the backdrop it captured.
///
/// A layer is a pure 2D composition primitive: the commands between `PushLayer` and
/// `PopLayer` are drawn into their own region, and the backdrop captured at the push is
/// processed before the two are composited again. Nothing here depends on the `3d` feature,
/// so a material can be drawn in a build that has no matrix pipeline at all.
///
/// The blur radius is a Gaussian's 2-sigma point, the same convention
/// [`RenderCommand::Blur`] and the box-shadow blur use, so a material and a shadow that
/// declare the same radius are equally soft.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerEffect {
    /// Blur the captured backdrop and composite the layer's foreground over it.
    BackdropBlur {
        /// Blur radius in pixels; `0` leaves the backdrop sharp.
        radius: u32,
    },
    /// Blur the captured backdrop, tint it, and composite the foreground over the result.
    BackdropBlurTint {
        /// Blur radius in pixels; `0` tints the sharp backdrop.
        radius: u32,
        /// Colour laid over the blurred backdrop.
        tint: Color,
        /// How strongly `tint` replaces the backdrop, in `0.0..=1.0`. Values outside the
        /// range are clamped, and a non-finite value is treated as `0.0`.
        tint_weight: f32,
    },
}

impl LayerEffect {
    /// The blur radius this effect requests, in pixels.
    pub fn radius(self) -> u32 {
        match self {
            Self::BackdropBlur { radius } | Self::BackdropBlurTint { radius, .. } => radius,
        }
    }

    /// The tint and its weight, or `None` for an untinted blur.
    pub fn tint(self) -> Option<(Color, f32)> {
        match self {
            Self::BackdropBlur { .. } => None,
            Self::BackdropBlurTint { tint, tint_weight, .. } => Some((tint, tint_weight)),
        }
    }
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
    /// Fill the quadrilateral spanned by `corners` with `color`.
    ///
    /// Corners are in logical pixels, in perimeter order. A back end fills the shape the
    /// four points enclose rather than the axis-aligned box around them, which is what lets
    /// an affine transform (feature `3d`) rotate or shear a rectangle without reducing it
    /// to its bounding box. A degenerate or self-intersecting set of corners has no
    /// well-defined interior and may draw nothing.
    #[cfg(feature = "3d")]
    FillQuad {
        /// Quadrilateral vertices in logical pixels, in perimeter order.
        corners: [Point; 4],
        /// Fill color.
        color: Color,
    },
    /// Fill a projectively divided quadrilateral as two antialiased triangles.
    ///
    /// [`RenderCommand::FillQuad`] is the affine case, where opposite edges stay parallel; this
    /// variant is for a quad produced by a perspective divide, where they need not. The software
    /// backend rasterises the two triangles with per-pixel coverage taken from the projected
    /// edges, so a turned card has a clean boundary instead of the stair-step its bounding box
    /// would produce. A back end that cannot antialias may treat it exactly like
    /// [`RenderCommand::FillQuad`] (a hard-edged polygon), which is a correct picture with a
    /// slightly rougher edge.
    #[cfg(feature = "3d")]
    FillQuadAA {
        /// Quadrilateral vertices in logical pixels, in perimeter order.
        corners: [Point; 4],
        /// Fill color.
        color: Color,
    },
    /// Draw an indexed triangle mesh through a camera, with a depth buffer.
    ///
    /// This is the only command whose geometry is genuinely three-dimensional. `model` places the
    /// mesh in world space, `camera` owns the eye and the projection, and `viewport` is the
    /// rectangle of the render target the camera looks through (in that target's pixel
    /// coordinates).
    ///
    /// # Depth
    ///
    /// Every backend tests an `f32` depth per pixel in the normalised `0..=1` range, where `0` is
    /// the near plane and `1` the far plane: smaller is nearer. The software rasteriser keeps one
    /// depth buffer per frame on the surface, so the outcome does not depend on the order mesh
    /// commands are recorded in. A two-dimensional command recorded after a mesh paints over it in
    /// painter order, because a flat control has no depth to compare.
    ///
    /// # Back faces
    ///
    /// No triangle is culled. The depth buffer decides what is visible, which keeps a mesh valid
    /// when its winding is inconsistent or when a caller deliberately wants a two-sided surface.
    /// The SVG backend has no depth buffer, so it sorts the triangles far-to-near instead
    /// (painter's algorithm); with opaque faces on a closed mesh the two produce the same picture,
    /// and with intersecting geometry the SVG is an approximation where the software path is exact.
    #[cfg(feature = "3d")]
    DrawMesh3D {
        /// The indexed triangle mesh to draw.
        mesh: Mesh3D,
        /// Model matrix placing the mesh in world space.
        model: Mat4,
        /// Eye position, orientation and projection.
        camera: Camera,
        /// Target rectangle the camera projects into, in target pixels.
        viewport: Rect,
    },
    /// Outline the quadrilateral spanned by `corners`.
    ///
    /// The stroked counterpart of [`RenderCommand::FillQuad`]: a rectangle whose corners an
    /// affine transform moved is no longer a rectangle, so a stroked `DrawRectStroke` under a
    /// rotation has to become four connected edges rather than the outline of the bounding box.
    /// The stroke width is in destination pixels and does not scale with the transform, matching
    /// [`RenderCommand::DrawLineStroke`].
    #[cfg(feature = "3d")]
    StrokeQuad {
        /// Quadrilateral vertices in logical pixels, in perimeter order.
        corners: [Point; 4],
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
    },
    /// Draw an image mapped onto the quadrilateral spanned by `corners`.
    ///
    /// `width` and `height` describe the **source** pixel grid, which is also the grid the
    /// image occupied before the transform: the batch path builds an image from the rectangle's
    /// own width and height, so the two agree. `corners` are the destination positions of that
    /// grid's top-left, top-right, bottom-right and bottom-left corners. A parallelogram is
    /// solved with an affine inverse; a quad whose opposite edges are not parallel (the result
    /// of a projective divide) is solved with a homography, so a perspective-turned card keeps
    /// its texture aligned with its projected corners.
    #[cfg(feature = "3d")]
    DrawImageQuad {
        /// Destination corners in logical pixels, in perimeter order.
        corners: [Point; 4],
        /// Source image width in pixels.
        width: u32,
        /// Source image height in pixels.
        height: u32,
        /// RGBA pixel data (4 bytes per pixel).
        ///
        /// Shared rather than owned so replaying a recorded batch can hand the same pixels to
        /// the backend without copying them (G-02).
        data: Arc<[u8]>,
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
        /// RGBA pixel data (4 bytes per pixel), shared so replay does not copy it (G-02).
        data: Arc<[u8]>,
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
    /// Begin a composed layer inside `bounds`.
    ///
    /// The current surface region is captured as the layer's backdrop and the region is
    /// cleared, so every command until the matching [`RenderCommand::PopLayer`] paints only
    /// the layer's foreground. On the pop, `effect` is applied to the captured backdrop and
    /// the foreground is composited over it. Layers nest, and an unbalanced pop is ignored
    /// rather than closing someone else's layer.
    ///
    /// This is deliberately **not** gated on `3d`: material/vibrancy composition is a 2D
    /// capability, and a build without the matrix pipeline must still draw a blurred
    /// backdrop.
    PushLayer {
        /// Region of the surface the layer covers, in logical pixels.
        bounds: Rect,
        /// How to process the captured backdrop, or `None` to draw the foreground directly over
        /// the untouched backdrop.
        effect: Option<LayerEffect>,
    },
    /// End the innermost [`RenderCommand::PushLayer`] and composite it.
    PopLayer,
    /// Begin a composed layer with a subtree opacity and an optional replay cache.
    ///
    /// This is [`RenderCommand::PushLayer`] plus the two composition controls a declarative UI
    /// needs for a subtree: `opacity` scales the layer's own foreground as one unit (a child and
    /// its background fade together instead of each primitive blending separately), and
    /// `cache_key` lets the renderer reuse the layer's foreground from the previous frame.
    ///
    /// # Cache contract
    ///
    /// A non-`None` `cache_key` is a caller assertion that the commands between this push and its
    /// matching [`RenderCommand::PopLayer`] produce the same foreground as the previous frame with
    /// the same key, the same `bounds` and the same `opacity`. When that holds the backend may
    /// discard the enclosed commands and composite the cached foreground; when it does not the
    /// caller must pass a different key (a version counter, a content hash, a frame number for
    /// animated content). The key is opaque to the renderer.
    ///
    /// The pop is still `PopLayer`: the two push forms share one stack, so a layer cannot be left
    /// open by choosing the wrong close command.
    PushLayerGroup {
        /// Region of the surface the layer covers, in logical pixels.
        bounds: Rect,
        /// How to process the captured backdrop, or `None` to composite over the untouched
        /// backdrop.
        effect: Option<LayerEffect>,
        /// Subtree opacity in `0.0..=1.0`; values outside that range are clamped.
        opacity: f32,
        /// Caller-supplied cache identity, or `None` to redraw the layer every frame.
        cache_key: Option<u64>,
    },
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
    /// Fill multiple closed contours together using the non-zero winding rule.
    ///
    /// Unlike [`RenderCommand::DrawPath`], all contours participate in one fill, so
    /// oppositely-wound contours form holes instead of being painted over by later commands.
    FillPathNonZero {
        /// Validated packed contours in logical pixels.
        path: CompoundPath,
        /// Fill colour.
        color: Color,
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
