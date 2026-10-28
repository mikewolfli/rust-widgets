// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Render commands for composing widget visuals.
use crate::compat::{Arc, Vec};
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

/// How a track matte's captured drawing is turned into a coverage mask.
///
/// The four spellings are the Lottie `tt` values (1..=4). They are a rendering primitive rather
/// than a Lottie detail: an alpha matte uses the source's alpha, a luma matte its luminance, and
/// the inverted forms use `255 - value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatteMode {
    /// Use the matte's alpha channel.
    Alpha,
    /// Use `255 - alpha`.
    AlphaInverted,
    /// Use the matte's luminance times its alpha.
    Luma,
    /// Use `255 - luminance`.
    LumaInverted,
}

/// Which text decorations a [`RenderCommand::DrawTextDecorated`] carries.
///
/// A bitfield rather than three booleans: a span can want an underline *and* a
/// strikethrough at once, and the flags compose with a single `|`. The zero
/// value is `NONE` and draws the text exactly as [`RenderCommand::DrawText`]
/// would, so the field's default is the current behaviour (G-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextDecoration(pub u8);

impl TextDecoration {
    /// No decoration: the glyphs alone, identical to `DrawText`.
    pub const NONE: Self = Self(0);
    /// A horizontal rule under the text's baseline.
    pub const UNDERLINE: Self = Self(1 << 0);
    /// A horizontal rule through the middle of the glyph box.
    pub const STRIKETHROUGH: Self = Self(1 << 1);
    /// A zigzag rule under the baseline (a spell-checker squiggle).
    pub const WAVY: Self = Self(1 << 2);

    /// Returns whether `flag` is set.
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag == flag && flag != 0
    }

    /// Returns whether any decoration bit is set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl core::ops::BitOr for TextDecoration {
    type Output = Self;

    /// Unions two decoration sets, so a caller can write
    /// `TextDecoration::UNDERLINE | TextDecoration::WAVY`.
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl core::ops::BitOrAssign for TextDecoration {
    /// Adds the right-hand side's flags into this set.
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// The repeating mark/gap period a [`RenderCommand::DrawPathDashed`] stroke
/// walks along its path (G-014).
///
/// `mark` is the length of inked run in pixels and `gap` the length of the
/// hole after it, both in path length. A `mark` of zero is rejected at draw
/// time (it would contribute no ink); a `gap` of zero degenerates to a solid
/// stroke, which a caller should express with [`RenderCommand::DrawPath`]
/// instead. The pattern restarts at each vertex, the same convention the
/// border dash pattern uses per edge, so the two agree on shared geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashPattern {
    /// Length of one inked run, in pixels.
    pub mark: u32,
    /// Length of the gap after each run, in pixels.
    pub gap: u32,
}

impl DashPattern {
    /// The toolkit's default dash: a mark three times the stroke width with a
    /// gap of two, matching [`crate::style::BorderStyle::Dashed`]'s period.
    pub fn dashed(width: u32) -> Self {
        Self { mark: (width * 3).max(1), gap: (width * 2).max(1) }
    }
}

/// How the ends of a stroke terminate (G-017).
///
/// The default is [`LineCap::Butt`], which is what every stroke in this crate
/// has always drawn: the brush stops at the endpoint's own coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// The stroke ends flush at each endpoint. The historical behaviour.
    #[default]
    Butt,
    /// Each endpoint gains a half-disc of the stroke's width.
    Round,
    /// Each endpoint is extended outward by half the stroke's width, so the
    /// stroke ends in a square the width of the line.
    Square,
}

/// How two stroked segments meet at a shared vertex (G-017).
///
/// The default is [`LineJoin::Miter`], which is what today's per-segment
/// brush overlap produces: the two segments' brushes cover the corner
/// between them, exactly as every polyline stroke in this crate has drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// The segments' own brushes cover the corner. The historical behaviour.
    #[default]
    Miter,
    /// Each interior vertex gains a disc of the stroke's width, rounding the
    /// corner off.
    Round,
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
    /// Fill the rectangle `rect` with `color`, rounding the corners to
    /// `radii` pixels. The effective radii are resolved per CSS
    /// `border-radius` (proportional scaling when adjacent corners overrun an
    /// edge), so the shape never degenerates; four equal values behave exactly
    /// as the historical single `radius` did.
    FillRoundedRect {
        /// Region to fill in logical pixels.
        rect: Rect,
        /// Per-corner radii in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01).
        radii: [u32; 4],
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
        /// Per-corner radii in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01).
        radii: [u32; 4],
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
        /// Outer per-corner radii in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01).
        radii: [u32; 4],
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
        /// Outer per-corner radii in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01).
        radii: [u32; 4],
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
        ///
        /// Shared rather than owned so recording, batching and replay all hand over the same
        /// bytes by refcount (G-12). A deferred backend that keeps the command therefore never
        /// copies a label per frame.
        text: Arc<str>,
        /// Font family, size, weight and style used to rasterize the glyphs.
        font: Font,
        /// Glyph color.
        color: Color,
        /// How `text` is placed horizontally relative to `origin.x`.
        alignment: HorizontalAlignment,
    },
    /// Draw a single line of text with baseline decorations (G-013).
    ///
    /// This is [`RenderCommand::DrawText`] plus a [`TextDecoration`] bitfield that
    /// asks the backend to also draw an underline, a strikethrough and/or a wavy
    /// underline under/through the glyphs. It is a separate variant rather than a
    /// field on `DrawText` so every existing construction site stays valid: the
    /// plain variant remains exactly what it was, and callers that want decoration
    /// opt in explicitly.
    DrawTextDecorated {
        /// Anchor point in logical pixels. `y` is the top of the text box;
        /// horizontal placement is controlled by `alignment`.
        origin: Point,
        /// Text to draw; supports combining marks and variation selectors.
        text: Arc<str>,
        /// Font family, size, weight and style used to rasterize the glyphs.
        font: Font,
        /// Glyph color.
        color: Color,
        /// How `text` is placed horizontally relative to `origin.x`.
        alignment: HorizontalAlignment,
        /// Bitfield naming the decorations to draw in addition to the glyphs.
        decoration: TextDecoration,
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
    /// Push a rounded-rectangle clip onto the clip stack (G-015).
    ///
    /// This is [`RenderCommand::PushClip`] with rounded corners: everything drawn
    /// until the matching [`RenderCommand::PopClip`] is masked to the rounded
    /// region instead of the axis-aligned rectangle. The backend computes the
    /// corner coverage from the same path machinery a filled rounded rect uses, so
    /// the mask geometry matches [`RenderCommand::FillRoundedRect`]. It is a
    /// separate variant rather than a field on `PushClip` so the historical
    /// rectangle command keeps its exact shape.
    PushClipRoundedRect {
        /// Left edge in logical pixels.
        x: i32,
        /// Top edge in logical pixels.
        y: i32,
        /// Clip width in logical pixels.
        width: u32,
        /// Clip height in logical pixels.
        height: u32,
        /// Per-corner radii in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01). Radii that overrun an edge are
        /// resolved per CSS `border-radius` like a filled rounded rect.
        radii: [u32; 4],
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
        ///
        /// A gradient is immutable once built, so recording, batching and replay share one
        /// handle instead of deep-copying its `stops` vector (G-12).
        gradient: Arc<Gradient>,
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
        ///
        /// Shared rather than owned so recording, region bucketing and replay all hand over the
        /// same vertices by refcount, exactly like [`CompoundPath`] (G-02).
        points: Arc<[Point]>,
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
    /// Stroke a polyline with a repeating dash pattern (G-014).
    ///
    /// This is the stroked form of [`RenderCommand::DrawPath`] with a
    /// [`DashPattern`]: each open run between vertices is broken into alternating
    /// `mark` and `gap` pixel runs (a closed path also dashes the closing edge).
    /// It is a separate variant rather than an `Option<DashPattern>` field on
    /// `DrawPath` so every existing `DrawPath` construction and destructure stays
    /// valid untouched.
    DrawPathDashed {
        /// Vertices in logical pixels, connected in order. Fewer than two points
        /// draws nothing.
        points: Arc<[Point]>,
        /// Connects the last point back to the first before dashing.
        closed: bool,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels; a zero width draws nothing.
        width: u32,
        /// Alternating mark/gap lengths in pixels along the polyline.
        dash: DashPattern,
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
    /// Draw a drop shadow behind a rounded rectangle (G-016).
    ///
    /// This is [`RenderCommand::BoxShadow`] for a shadow caster whose corners are
    /// rounded: the blur source is the rounded rectangle itself rather than the
    /// bounding box, so a rounded card no longer gets rectangular shadow corners
    /// spilling past its outline. The radii describe the caster's corners and are
    /// resolved per CSS `border-radius` before use.
    BoxShadowRounded {
        /// The shadow caster's rectangular bounds.
        rect: Rect,
        /// Per-corner radii of the caster in pixels, ordered top-left, top-right,
        /// bottom-right, bottom-left (STY-01).
        radii: [u32; 4],
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
        /// Points defining the clip path, shared so bucketing and replay never copy them (G-02).
        points: Arc<[Point]>,
    },
    /// Set blend mode for subsequent draw commands (BLUE11 R5.6).
    SetBlendMode {
        /// Mode applied to later draw commands.
        mode: BlendMode,
    },
    /// Begin capturing a track matte inside `bounds`.
    ///
    /// The commands between this push and the matching [`RenderCommand::PopMatte`] draw the
    /// **matte source** into its own region. The pop turns that drawing into a coverage mask; the
    /// commands after it draw the **matted layer**, which [`RenderCommand::EndMatte`] composites
    /// through the mask over the backdrop captured at the push.
    ///
    /// # Why this is a primitive rather than a sequence of existing commands
    ///
    /// A layer ([`RenderCommand::PushLayer`]) captures a backdrop and composites a foreground, but
    /// it has no way to express “multiply the foreground's alpha by a second drawing”. Splitting a
    /// matte into a layer plus a clip would look like the effect while producing the wrong pixels
    /// wherever the mask is partial, so the composition gets its own pair of commands.
    PushMatte {
        /// Region of the surface the matte and its layer cover, in logical pixels.
        bounds: Rect,
        /// How the captured drawing becomes coverage.
        mode: MatteMode,
    },
    /// Finish the matte drawing started by [`RenderCommand::PushMatte`].
    ///
    /// The region is cleared for the matted layer's foreground; the captured backdrop and mask
    /// stay pending until [`RenderCommand::EndMatte`].
    PopMatte,
    /// Composite the matted layer through the mask and restore the backdrop.
    EndMatte,

    // ── Recorder-owned payloads (G-802) ──
    //
    // These four variants are produced **only** by `RenderContext`'s region recorder. Their
    // payloads live in the recorder's frame arenas, so recording a path or a label allocates
    // nothing; `RegionBuckets::flush` resolves each slice and calls the backend's by-reference
    // entry point. They never reach a backend's `execute_command`, which is why the arms there
    // assert rather than paint.
    /// A [`RenderCommand::DrawPath`] whose vertices are `path_arena[start..start + len]`.
    DrawPathRecorded {
        /// First index into the recorder's point arena.
        start: usize,
        /// Number of vertices.
        len: usize,
        /// Bounds computed while recording, so region replay needs no second scan.
        bounds: Rect,
        /// Connects the last point back to the first when stroking.
        closed: bool,
        /// Stroke color, or fill color when `filled` is true.
        color: Color,
        /// Fills the polygon instead of stroking it.
        filled: bool,
        /// Stroke width in pixels.
        width: u32,
    },
    /// A [`RenderCommand::DrawPathDashed`] whose vertices are in the recorder's arena.
    DrawPathDashedRecorded {
        /// First index into the recorder's point arena.
        start: usize,
        /// Number of vertices.
        len: usize,
        /// Bounds computed while recording.
        bounds: Rect,
        /// Connects the last point back to the first before dashing.
        closed: bool,
        /// Stroke color.
        color: Color,
        /// Stroke width in pixels.
        width: u32,
        /// Alternating mark/gap lengths in pixels.
        dash: DashPattern,
    },
    /// A [`RenderCommand::FillPathNonZero`] whose payload is in the recorder's arenas.
    FillPathNonZeroRecorded {
        /// First index into the recorder's point arena.
        point_start: usize,
        /// Number of vertices.
        point_len: usize,
        /// First index into the recorder's contour arena.
        contour_start: usize,
        /// Number of contours.
        contour_len: usize,
        /// Bounds computed while recording.
        bounds: Rect,
        /// Fill colour.
        color: Color,
    },
    /// A [`RenderCommand::DrawText`] whose text is `text_arena[text_start..text_start + text_len]`.
    DrawTextRecorded {
        /// Baseline start, per `alignment`.
        origin: Point,
        /// First byte index into the recorder's text arena.
        text_start: usize,
        /// Byte length.
        text_len: usize,
        /// Bounds computed while recording.
        bounds: Rect,
        /// The face the text is painted with.
        font: Font,
        /// Text colour.
        color: Color,
        /// Horizontal placement relative to `origin`.
        alignment: HorizontalAlignment,
    },
    /// Draw a conic (angular/sweep) gradient (BLUE11 R5.7).
    DrawConicGradient {
        /// Center point of the gradient.
        center: Point,
        /// Starting angle in radians.
        start_angle: f32,
        /// Color stops as (position \[0,1\], color) pairs.
        ///
        /// Shared rather than owned: a conic ramp is immutable once built, so recording, region
        /// bucketing and replay hand over one slice by refcount instead of copying its stops
        /// (G-01).
        stops: Arc<[(f32, Color)]>,
    },
}

impl RenderCommand {
    /// The device-space rectangle this command can affect, when it is bounded.
    ///
    /// # Why this exists
    ///
    /// A dirty-region pass has to decide which recorded commands belong to which damaged
    /// rectangle. Measuring each primitive once here is what lets
    /// [`crate::render::RenderContext`] bucket a single widget-tree traversal by region instead of
    /// re-emitting the whole tree once per region (G-04).
    ///
    /// `None` means "the extent is not expressible from the command alone" (a clip pop, a blend
    /// mode, a state change). Such a command is replayed for every region rather than being
    /// dropped, because dropping a state change would change the pixels that follow it. A command
    /// whose bounds can only be estimated is deliberately over-estimated: including a command in a
    /// region it does not touch costs work, excluding one it does touch is a wrong picture.
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Self::FillRect { rect, .. }
            | Self::DrawRect { rect, .. }
            | Self::DrawRectStroke { rect, .. }
            | Self::FillRoundedRect { rect, .. }
            | Self::FillRoundedRectAA { rect, .. }
            | Self::DrawRoundedRectStroke { rect, .. }
            | Self::DrawRoundedRectStrokeAA { rect, .. }
            | Self::BoxShadow { rect, .. }
            | Self::BoxShadowRounded { rect, .. }
            | Self::DrawGradient { rect, .. }
            | Self::PushLayer { bounds: rect, .. }
            | Self::PushLayerGroup { bounds: rect, .. }
            | Self::PushMatte { bounds: rect, .. } => Some(*rect),
            Self::DrawImage { x, y, width, height, .. } => Some(Rect::new(*x, *y, *width, *height)),
            #[cfg(feature = "3d")]
            Self::FillQuad { corners, .. }
            | Self::FillQuadAA { corners, .. }
            | Self::StrokeQuad { corners, .. } => Some(bounds_of_corners(corners)),
            #[cfg(feature = "3d")]
            Self::DrawImageQuad { corners, width, height, .. } => {
                let mut rect = bounds_of_corners(corners);
                if rect.width == 0 {
                    rect.width = *width;
                }
                if rect.height == 0 {
                    rect.height = *height;
                }
                Some(rect)
            }
            #[cfg(feature = "3d")]
            Self::DrawMesh3D { viewport, .. } => Some(*viewport),
            Self::DrawLine { from, to, .. } | Self::DrawLineAA { from, to, .. } => {
                Some(bounds_of_points(&[*from, *to], 1))
            }
            Self::DrawLineStroke { from, to, width, .. }
            | Self::DrawLineStrokeAA { from, to, width, .. } => {
                Some(bounds_of_points(&[*from, *to], *width))
            }
            Self::FillCircle { center, radius, .. }
            | Self::FillCircleAA { center, radius, .. }
            | Self::DrawCircle { center, radius, .. } => {
                Some(bounds_around_point(*center, *radius))
            }
            Self::DrawCircleStroke { center, radius, width, .. } => {
                Some(bounds_around_point(*center, radius.saturating_add(*width)))
            }
            Self::DrawArc { center, radius, .. } => Some(bounds_around_point(*center, *radius)),
            Self::DrawText { origin, text, font, alignment, .. }
            | Self::DrawTextDecorated { origin, text, font, alignment, .. } => {
                // The generous box already pads one em on every side, which covers
                // an underline/wavy stroke sitting below the baseline.
                Some(bounds_of_text(*origin, text, font, *alignment))
            }
            Self::PushClip { x, y, width, height }
            | Self::PushClipRoundedRect { x, y, width, height, .. } => {
                Some(Rect::new(*x, *y, *width, *height))
            }
            Self::DrawPath { points, width, .. } | Self::DrawPathDashed { points, width, .. } => {
                Some(bounds_of_points(points, *width))
            }
            Self::FillPathNonZero { path, .. } => Some(bounds_of_points(path.points(), 0)),
            Self::DrawPathRecorded { bounds, .. }
            | Self::DrawPathDashedRecorded { bounds, .. }
            | Self::FillPathNonZeroRecorded { bounds, .. }
            | Self::DrawTextRecorded { bounds, .. } => Some(*bounds),
            Self::ClipPath { points } => Some(bounds_of_points(points, 0)),
            // State commands, and conic gradients whose extent comes from the clip rather than
            // from the command. Replayed in every region.
            Self::PopClip
            | Self::PopLayer
            | Self::PopMatte
            | Self::EndMatte
            | Self::Blur { .. }
            | Self::SetBlendMode { .. }
            | Self::DrawConicGradient { .. } => None,
        }
    }
}

/// The smallest rectangle containing all four corners.
#[cfg(feature = "3d")]
fn bounds_of_corners(corners: &[Point; 4]) -> Rect {
    let min_x = corners.iter().map(|point| point.x).min().unwrap_or(0);
    let min_y = corners.iter().map(|point| point.y).min().unwrap_or(0);
    let max_x = corners.iter().map(|point| point.x).max().unwrap_or(min_x);
    let max_y = corners.iter().map(|point| point.y).max().unwrap_or(min_y);
    Rect::new(
        min_x,
        min_y,
        max_x.saturating_sub(min_x).max(0) as u32,
        max_y.saturating_sub(min_y).max(0) as u32,
    )
}

/// The bounding box of `points`, expanded by `stroke` pixels on each side.
pub(crate) fn bounds_of_points(points: &[Point], stroke: u32) -> Rect {
    let Some(first) = points.first() else {
        return Rect::new(0, 0, 0, 0);
    };
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x;
    let mut max_y = first.y;
    for point in &points[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    let pad = stroke as i32;
    Rect::new(
        min_x.saturating_sub(pad),
        min_y.saturating_sub(pad),
        max_x.saturating_sub(min_x).max(0) as u32 + stroke.saturating_mul(2),
        max_y.saturating_sub(min_y).max(0) as u32 + stroke.saturating_mul(2),
    )
}

/// The square around `center` that a circle of `radius` can touch.
fn bounds_around_point(center: Point, radius: u32) -> Rect {
    let radius = radius as i32;
    Rect::new(
        center.x.saturating_sub(radius),
        center.y.saturating_sub(radius),
        (radius.saturating_mul(2)).max(0) as u32,
        (radius.saturating_mul(2)).max(0) as u32,
    )
}

/// A deliberately generous box around a text run.
///
/// A shaped advance is the backend's answer, and asking for it here would put a measurement on
/// the recording path. One em per scalar plus one em of slack is an upper bound for ordinary UI
/// text and, because it is only used to decide which region a command is replayed into, erring
/// large is safe: the command may be replayed by one extra region, never omitted from one it
/// touches.
pub(crate) fn bounds_of_text(
    origin: Point,
    text: &str,
    font: &Font,
    alignment: HorizontalAlignment,
) -> Rect {
    let em = font.size().max(1.0);
    let scalars = text.chars().count().max(1) as f32;
    let width = (scalars * em + em * 2.0).round().max(1.0) as u32;
    let height = (em * 2.0).round().max(1.0) as u32;
    let x = match alignment {
        HorizontalAlignment::Left => origin.x,
        HorizontalAlignment::Center => origin.x.saturating_sub(width as i32 / 2),
        HorizontalAlignment::Right => origin.x.saturating_sub(width as i32),
    };
    Rect::new(x.saturating_sub(em as i32), origin.y.saturating_sub(em as i32), width, height)
}
