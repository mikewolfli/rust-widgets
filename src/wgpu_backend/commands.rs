// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! WGPU draw commands.
use super::types::{PixelRect, Rgba8};
/// Feature-gated GPU draw command list.
///
/// A `Vec<WgpuDrawCommand>` is an ordered, retained frame description rather
/// than an immediate-mode sequence: nothing is touched on the GPU while the
/// list is being built. The whole list is replayed later by
/// `rasterize_draw_commands_rgba8`, which executes the variants in order into a
/// single framebuffer, so a later command paints over an earlier one.
///
/// Any variant carrying a `clip` field is clipped by the intersection of all
/// three of the following, in this precedence:
///
/// 1. the framebuffer itself (`0, 0, width, height`),
/// 2. the variant's own `rect` (or the shape's bounding box for variants
///    without a `rect`),
/// 3. that variant's `clip`, when it is `Some`.
///
/// The `clip` field is therefore per-command and purely a *tightening* of the
/// shape's own bounds; it cannot enlarge them. It **composes** with
/// [`WgpuDrawCommand::PushClip`]: when a clip is pushed, the effective region is the
/// intersection of the two, so a command is drawn inside both.
///
/// Colours are [`Rgba8`] with straight (non-premultiplied) alpha in the
/// sRGB-encoded 0..=255 space. By default a command writes all four channels
/// directly (no source-over blending), so an alpha of `0` leaves an opaque black
/// pixel rather than a transparent one. [`WgpuDrawCommand::SetBlendMode`] changes
/// that for the colour channels of every following command, and starts at `Normal`
/// for each stream so this default is what an unaware caller gets.
///
/// # Stateful commands
///
/// Three variants change rasteriser state for the commands that follow them rather
/// than drawing anything themselves: [`WgpuDrawCommand::PushClip`] /
/// [`WgpuDrawCommand::PopClip`] (the clip stack) and
/// [`WgpuDrawCommand::SetBlendMode`] (the blend mode). All are sticky, so a stream
/// that sends one and never undoes it affects the rest of the frame.
#[derive(Debug, Clone, PartialEq)]
pub enum WgpuDrawCommand {
    /// Replace the *entire* framebuffer with a single colour, ignoring every
    /// `clip` and `rect` in the frame.
    ///
    /// This is a whole-surface state change rather than a shape, so its effect
    /// depends entirely on position in the list: commands recorded before it
    /// are erased, commands after it draw onto the new background. Frames that
    /// want a background should therefore place `Clear` first.
    Clear {
        /// Colour written to all four channels of every pixel. Straight alpha;
        /// no blending is performed.
        color: Rgba8,
    },
    /// Fill an axis-aligned rectangle with a single colour.
    FillRect {
        /// Region to fill, in physical pixels relative to the framebuffer's
        /// top-left origin. A partially off-screen rectangle is clipped to the
        /// framebuffer; a zero-sized or fully off-screen one draws nothing.
        rect: PixelRect,
        /// Fill colour, written to all four channels without blending.
        color: Rgba8,
        /// Optional clip that further restricts `rect`; `None` means the
        /// framebuffer is the only bound.
        clip: Option<PixelRect>,
    },
    /// Outline a rectangle with four solid edge bands.
    ///
    /// The stroke is drawn as four overlapping bands — top, bottom, left and
    /// right, each `thickness` pixels wide and spanning the full rectangle
    /// width/height — and the bands lie *inside* `rect`, so the outline never
    /// extends past the rectangle's bounds. The left and right bands have their
    /// height shortened by `thickness` at each end so the corners are not drawn
    /// twice. The effective thickness is clamped to `min(thickness, width,
    /// height)`, so a stroke thicker than half the rectangle degenerates into a
    /// solid fill.
    StrokeRect {
        /// Outer bounds of the outline, in physical pixels.
        rect: PixelRect,
        /// Colour of each edge band, drawn without blending.
        color: Rgba8,
        /// Edge width in pixels; `0` skips the command entirely.
        thickness: u32,
        /// Optional clip that further restricts the rectangle.
        clip: Option<PixelRect>,
    },
    /// Draw a single line of text as an 8x8 bitmap font.
    ///
    /// `rect` is a *text box*, not a baseline: glyphs are laid out on a fixed
    /// 8x8 pixel grid starting at the box's top-left corner, filling it left to
    /// right then top to bottom at `columns * rows` glyphs, where `columns =
    /// max(rect.width / 8, 1)` and `rows = max(rect.height / 8, 1)`. Text longer
    /// than that grid is truncated. Each glyph is drawn at one pixel per bitmap
    /// dot, so the text does not scale with the box — it is cropped and clipped
    /// instead.
    ///
    /// Characters outside the embedded font are substituted with `'?'`.
    /// Combining marks and variation selectors still advance a grid cell.
    /// A zero-width box, a zero-height box or an empty string draws nothing.
    DrawText {
        /// Text box in physical pixels; also the layout grid's origin.
        rect: PixelRect,
        /// The string to render, one glyph per grid cell.
        text: String,
        /// Colour of every set bitmap dot, drawn without blending.
        color: Rgba8,
        /// Optional clip that further restricts the text box.
        clip: Option<PixelRect>,
    },
    /// Draw an RGBA image, scaled to fill `rect`.
    ///
    /// Scaling is nearest-neighbour with a deterministic integer mapping
    /// (`src = local * source_dim / dest_dim`, clamped to the last row/column),
    /// so the result is reproducible across runs and platforms. Stretching
    /// changes the aspect ratio; the image is never letterboxed.
    ///
    /// The source is `image_width * image_height` tightly packed RGBA8 pixels,
    /// row-major, top row first, with straight alpha — the same layout as the
    /// framebuffer, and the same as the RGBA bytes returned by
    /// `crate::widget::runtime::render_frame`.
    ///
    /// Degenerate inputs are handled asymmetrically: a zero-sized destination
    /// or a zero-sized source draws nothing, but a payload whose length is not
    /// exactly `image_width * image_height * 4` is a hard error — the
    /// rasterizer returns `Err` and abandons the whole frame rather than
    /// drawing a partial frame.
    DrawImage {
        /// Destination rectangle in physical pixels.
        rect: PixelRect,
        /// Source pixels, RGBA8, row-major, top row first; must be exactly
        /// `image_width * image_height * 4` bytes.
        rgba8: Vec<u8>,
        /// Source width in pixels, used to compute the 4-bytes-per-pixel
        /// stride of `rgba8`.
        image_width: u32,
        /// Source height in pixels, i.e. the number of rows in `rgba8`.
        image_height: u32,
        /// Optional clip that further restricts the destination.
        clip: Option<PixelRect>,
    },
    /// Fill a rectangle with rounded corners.
    ///
    /// Each corner is a quarter disc of `radius` pixels whose centre is inset
    /// `radius` from both edges; the corner test is `dx*dx + dy*dy <= r*r`, so
    /// the corner pixels are hard-edged with no anti-aliasing. The effective
    /// radius is clamped to half the shorter side, which makes an oversized
    /// radius degrade gracefully into a stadium/ellipse shape rather than
    /// producing a malformed outline.
    FillRoundedRect {
        /// Region to fill, in physical pixels; also the outer bounds of the
        /// rounded shape.
        rect: PixelRect,
        /// Corner radius in pixels, clamped to half the shorter side; `0`
        /// skips the command, as does a zero-sized rectangle.
        radius: u32,
        /// Fill colour, drawn without blending.
        color: Rgba8,
        /// Optional clip that further restricts the rectangle.
        clip: Option<PixelRect>,
    },
    /// Outline a rectangle with rounded corners.
    ///
    /// The outline is the ring between two concentric rounded rectangles
    /// sharing `radius`: an outer one at `radius + thickness` and an inner one
    /// at `radius - thickness` (saturating at `0`). The stroke therefore
    /// straddles the `rect` boundary — it extends `thickness` pixels *outside*
    /// `rect` — unlike [`WgpuDrawCommand::StrokeRect`], whose bands lie inside.
    ///
    /// The ring is only evaluated where the outer shape's corner zones and
    /// straight edge sections meet, and pixels are written with no
    /// anti-aliasing. Note that the straight sections are positioned by their
    /// geometric relationship to the corners rather than by summing their
    /// widths, so a `thickness` greater than `radius` can leave gaps between
    /// the straight and corner sections; radii at least as large as the
    /// thickness are the well-formed case.
    StrokeRoundedRect {
        /// Outer bounds of the shape in physical pixels; the stroke straddles
        /// this boundary.
        rect: PixelRect,
        /// Corner radius in pixels. `0`, a `0` thickness or a zero-sized
        /// rectangle skips the command.
        radius: u32,
        /// Colour of the ring, drawn without blending.
        color: Rgba8,
        /// Stroke width in pixels, used as the ring's half-width on each side
        /// of the `rect` boundary.
        thickness: u32,
        /// Optional clip that further restricts the shape (this clip also trims
        /// the outward half of the stroke).
        clip: Option<PixelRect>,
    },
    /// Draw a line segment between two points.
    ///
    /// The segment is rasterized with Bresenham's algorithm, so it is
    /// hard-edged with no anti-aliasing, and its thickness comes from stamping
    /// a square of side `2 * max(width / 2, 1)` around each stepped pixel. The
    /// square is centred on the *upper-left* of the current pixel, so an
    /// even-width line is biased one pixel up and left of the ideal centreline.
    ///
    /// Both endpoints are inclusive, and the endpoints' order does not matter.
    DrawLine {
        /// First endpoint in physical pixels.
        from: (i32, i32),
        /// Second endpoint in physical pixels.
        to: (i32, i32),
        /// Stroke colour, drawn without blending.
        color: Rgba8,
        /// Stroke width in pixels; `0` skips the command.
        width: u32,
        /// Optional clip that further restricts the segment; it is intersected
        /// with the segment's bounding box before rasterizing.
        clip: Option<PixelRect>,
    },
    /// Fill a solid disc.
    ///
    /// A pixel is painted when `dx*dx + dy*dy <= radius*radius` about `center`,
    /// so the edge is hard-edged, one pixel of aliasing wide. `center` is a
    /// pixel coordinate and may lie outside the framebuffer.
    FillCircle {
        /// Disc centre in physical pixels.
        center: (i32, i32),
        /// Disc radius in pixels; `0` skips the command.
        radius: u32,
        /// Fill colour, drawn without blending.
        color: Rgba8,
        /// Optional clip that further restricts the disc; it is intersected with
        /// the circle's bounding box first.
        clip: Option<PixelRect>,
    },
    /// Outline a circle with a ring of the given stroke width.
    ///
    /// A pixel is painted when its distance from `center` is at most
    /// `radius + width` and at least `radius - width`. Thus `width` is the
    /// ring's *half*-width measured from `radius`: the ring spans
    /// `2 * width` pixels across and straddles the nominal radius, so `radius`
    /// is a centreline rather than an inner or outer edge. The edge is
    /// hard-edged, with no anti-aliasing.
    DrawCircle {
        /// Circle centre in physical pixels.
        center: (i32, i32),
        /// Radius in pixels, marking the stroke's centreline; `0` skips the
        /// command.
        radius: u32,
        /// Stroke colour, drawn without blending.
        color: Rgba8,
        /// Half-width of the ring in pixels; `0` skips the command.
        width: u32,
        /// Optional clip that further restricts the ring; it is intersected with
        /// the circle's bounding box first.
        clip: Option<PixelRect>,
    },
    /// Draw a circular arc, i.e. a partial circle.
    ///
    /// Angles are in **radians**, measured clockwise from the positive x axis
    /// (screen space, where y grows downwards), so `0` points right, `PI/2`
    /// points down, and both angles increase clockwise on screen.
    ///
    /// The sweep runs from `start_angle` in the direction of increasing angle
    /// to `end_angle`, and the sweep may wrap through `2*PI`. When the two
    /// angles are equal within `0.001` radians the arc is treated as a *full*
    /// circle, not as an empty sweep — so an arc that the caller intended as
    /// zero-length draws a whole circle instead.
    ///
    /// The arc is drawn either as a filled pie wedge (the disc clipped to the
    /// sweep) or as a stroked band one pixel wide on each side of `radius`
    /// (i.e. the ring spans `radius - 1` to `radius + 1`), selected by `filled`.
    /// This means the apparent stroke width for `filled: false` is fixed at two
    /// pixels and cannot be configured per command. A `0` radius skips the
    /// command.
    DrawArc {
        /// Arc centre in physical pixels.
        center: (i32, i32),
        /// Radius in pixels; also the centreline of the stroked band; `0` skips
        /// the command.
        radius: u32,
        /// Sweep start angle in radians, clockwise from the positive x axis.
        start_angle: f32,
        /// Sweep end angle in radians, same convention as `start_angle`. Equal
        /// to `start_angle` within `0.001` means a full circle.
        end_angle: f32,
        /// Fill colour for the wedge, or stroke colour for the band.
        color: Rgba8,
        /// `true` fills the pie wedge back to `center`; `false` strokes the band
        /// around `radius` at a fixed two-pixel width.
        filled: bool,
        /// Optional clip that further restricts the arc; it is intersected with
        /// the circle's bounding box first.
        clip: Option<PixelRect>,
    },
    /// Draw a path through a sequence of points.
    ///
    /// ## `filled: true` (requires at least 3 points)
    ///
    /// The polygon is filled with a scanline algorithm and the even-odd rule,
    /// implying a closing edge from the last point back to the first regardless
    /// of `closed`. Span endpoints are the integer intersections of the
    /// polygon's edges with the scanline, so the fill is inclusive of the left
    /// endpoint and exclusive of the right one.
    ///
    /// ## `filled: false`
    ///
    /// The points are joined by [`WgpuDrawCommand::DrawLine`] segments of width
    /// `width`, each with Bresenham rasterization. `closed` adds the final
    /// segment from the last point back to the first.
    ///
    /// ## Degenerate input
    ///
    /// Fewer than two points skips the command. A stroke width of `0` is passed
    /// through to the segment rasterizer, which substitutes a width of one
    /// pixel, so a zero-width stroke still draws. Filling a polygon with fewer
    /// than three points falls back to the stroked path. A concave polygon is
    /// filled by the even-odd rule, so self-intersections create holes rather
    /// than being wound out.
    DrawPath {
        /// Vertices in physical pixels, connected in list order. Fewer than two
        /// points skips the command.
        points: Vec<(i32, i32)>,
        /// Adds a closing segment from the last point back to the first when
        /// stroking. Ignored when `filled` is true, because a fill is
        /// implicitly closed.
        closed: bool,
        /// Stroke colour, or fill colour when `filled` is true.
        color: Rgba8,
        /// Fills the polygon spanned by `points` instead of stroking it.
        filled: bool,
        /// Stroke width in pixels, passed to the segment rasterizer; `0` draws
        /// a one-pixel-wide stroke rather than nothing.
        width: u32,
        /// Optional clip that further restricts the path; it is intersected with
        /// the path's bounding box first.
        clip: Option<PixelRect>,
    },
    /// Fill a rectangle with a vertical multi-stop gradient.
    ///
    /// That direction is worth stating because it is easy to guess wrong:
    /// `gradient_data` is a table of colours which is mapped top-to-bottom
    /// across `rect`, *not* left-to-right and not along an arbitrary vector.
    /// (The pipeline's `RenderCommand::DrawGradient` carries a full
    /// `crate::style::Gradient` with real geometry; this command is the
    /// `WgpuDrawCommand` equivalent for the vertical case.)
    ///
    /// The table is tightly packed 8-bit RGBA, four bytes per colour, with no
    /// stop positions: every stop is assumed to be evenly spaced. Linear
    /// interpolation is *not* performed — each scanline selects the nearest
    /// stop and fills with that colour flat, and the scanline picks by the
    /// stop's left edge rather than by rounding to the closest one, so the
    /// transition lands half a stop early. A table whose length is not a
    /// multiple of four has its trailing 1..=3 bytes ignored, and an empty
    /// table draws nothing, as does a zero-sized rectangle.
    ///
    /// The gradient is mapped onto `rect` itself, so it is unaffected by
    /// clipping: a clip narrows the visible region but does not rescale or
    /// re-phase the ramp.
    DrawGradient {
        /// Region to fill, in physical pixels; the ramp's top edge is this
        /// rectangle's top edge and the ramp's bottom edge its bottom edge.
        rect: PixelRect,
        /// Evenly spaced colour stops, four bytes each, in the framebuffer's
        /// top-to-bottom order.
        gradient_data: Vec<u8>,
        /// Optional clip that further restricts the rectangle without altering
        /// the ramp.
        clip: Option<PixelRect>,
    },
    /// Push a clipping rectangle onto the clip stack.
    ///
    /// **Implemented.** The rasterizer maintains a clip stack; pushing intersects
    /// `rect` with the current clip, so nesting can only shrink the visible area.
    /// Each drawing command's own `clip` field is intersected with the active clip
    /// in turn, which means the two mechanisms compose rather than override.
    ///
    /// A push that does not overlap the current clip yields an empty region, so
    /// subsequent drawing is suppressed entirely rather than ignoring the clip.
    /// Remember to pop: an unbalanced push keeps the clip in force for the rest of
    /// the batch.
    PushClip {
        /// Clip rectangle in physical pixels.
        rect: PixelRect,
    },
    /// Pop the most recent clipping rectangle from the clip stack.
    ///
    /// **Implemented**, matching [`WgpuDrawCommand::PushClip`]. An unbalanced pop
    /// (one without a matching push) is logged and leaves the clip unchanged, so the
    /// rest of the batch still renders; it is diagnosed rather than ignored.
    PopClip,
    /// Draw a filled gradient rectangle that ramps **left to right**.
    ///
    /// The horizontal counterpart of [`WgpuDrawCommand::DrawGradient`], which ramps
    /// top to bottom. Use this one for a horizontal ramp; the two are otherwise the
    /// same colour-stop table (`[r,g,b,a, …]`).
    ///
    /// The variant carries no angle, so "linear" can only mean axis-aligned here; a
    /// caller needing an arbitrary direction composites the axis-aligned ramp itself.
    FillLinearGradient {
        /// Region to fill, in physical pixels.
        rect: PixelRect,
        /// Colour-stop table, `[r,g,b,a, …]` per stop, interpolated across `rect`.
        gradient_data: Vec<u8>,
        /// Optional clip that further restricts the fill.
        clip: Option<PixelRect>,
    },
    /// Draw a filled **radial** gradient inscribed in `rect`.
    ///
    /// The ramp is centred in the rectangle with an outer radius reaching the nearest
    /// edge, so the gradient fits inside the requested area. Pixels beyond that radius
    /// take the final stop colour rather than being clipped away, so this always
    /// covers `rect` completely.
    ///
    /// Note that the variant carries no centre or radius field: the geometry is
    /// derived from `rect`, which is the only reading the payload supports. A caller
    /// needing an off-centre or elliptical ramp must size and position `rect` itself.
    FillRadialGradient {
        /// Region to fill, in physical pixels; also defines the gradient geometry.
        rect: PixelRect,
        /// Colour-stop table, `[r,g,b,a, …]` per stop, indexed by distance from the centre.
        gradient_data: Vec<u8>,
        /// Optional clip that further restricts the fill.
        clip: Option<PixelRect>,
    },
    /// Set the compositing blend mode for all following draw commands.
    ///
    /// **Sticky**, like the software backend: it applies until changed, and starts at
    /// `Normal` (a plain source-over write) for every stream, so an existing stream
    /// that never sends this command renders exactly as before.
    ///
    /// It affects the colour channels only; source alpha is written unchanged in
    /// every mode, because these modes describe how colours compose, not how coverage
    /// does.
    SetBlendMode {
        /// Blend mode tag: the `as u8` discriminant of `crate::render::BlendMode`.
        /// An unrecognised value is logged and treated as `Normal` rather than
        /// silently picking a different mode.
        mode: u8,
    },
    /// Draw a drop shadow behind `rect`.
    ///
    /// The shadow occupies `rect` translated by `(offset_x, offset_y)`, filled with
    /// `color` and then box-blurred by `blur_radius` (0 disables the blur). The blur
    /// matches the software backend's kernel, so the two backends agree.
    ///
    /// The `spread` field that `RenderCommand::BoxShadow` carries is not present here:
    /// a caller that needs the shadow's bounds widened should expand `rect` before
    /// sending it.
    BoxShadow {
        /// The shadow's bounds before offsetting, in physical pixels.
        rect: PixelRect,
        /// Shadow colour; conventionally translucent.
        color: Rgba8,
        /// Horizontal offset in pixels.
        offset_x: i32,
        /// Vertical offset in pixels.
        offset_y: i32,
        /// Blur radius in pixels; 0 leaves the shadow with hard edges.
        blur_radius: u32,
        /// Optional clip that further restricts the shadow.
        clip: Option<PixelRect>,
    },
}
