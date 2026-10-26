// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Batch rendering primitives.
//!
//! Provides types for organizing draw commands into batches that can be
//! recorded once and replayed efficiently by the renderer.

use crate::compat::{fmt, Arc, HashMap, Rc, Vec, VecDeque};

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::render::{
    RenderCommand, ShapedText, SoftwareRenderConfig, SoftwareSurface, TextMetrics,
};

use super::paint::{route_command_to_surface, PaintBackend, SoftwarePaintBackend};
#[cfg(feature = "3d")]
use crate::render::math3d::{Mat3, Mat4, Transform, TransformStack, TRANSFORM_STACK_MAX};

/// A zero-copy [`PaintBackend`] view over a borrowed [`SoftwareSurface`].
///
/// # Why this exists (D09-RENDER-05)
///
/// `SoftwarePaintBackend::replay` has to read its own `batch_state` while painting
/// into its own `surface`. Borrowing `self` for both is rejected by the compiler,
/// and the historical workaround was to `clone()` the whole batch state — which
/// copied every batch and every image regardless of which batch was replayed.
/// Passing the surface alone as `&mut SoftwareSurface` would split the borrow, but
/// the surface has no `PaintBackend` impl of its own. This view supplies one, so
/// the replay path can paint through the surface while the batch maps stay
/// borrowed immutably. Because the adapter is `#[repr(transparent)]`, its address
/// is the address of the referenced surface: the cast is sound and free.
#[repr(transparent)]
struct SurfacePaintBackend<'a>(&'a mut SoftwareSurface);

impl PaintBackend for SurfacePaintBackend<'_> {
    fn begin_frame(&mut self, clear: Color) {
        self.0.begin_frame(clear);
    }
    fn end_frame(&mut self) {
        self.0.end_frame();
    }
    fn execute_command(&mut self, command: &RenderCommand) {
        // Delegate to the backend's own command match so the two paths cannot drift;
        // reproducing the match here would be a second copy of the same routing.
        route_command_to_surface(self.0, command);
    }
    fn fill_path_non_zero(
        &mut self,
        points: &[Point],
        contours: &[(usize, usize)],
        color: Color,
        offset_x: i32,
        offset_y: i32,
    ) -> Result<(), crate::render::CompoundPathError> {
        self.0.fill_path_non_zero_borrowed(points, contours, color, offset_x, offset_y)
    }
    fn size(&self) -> Size {
        self.0.size()
    }
    fn set_size(&mut self, size: Size) {
        self.0.resize(size);
    }
    fn dpi_scale(&self) -> f32 {
        self.0.dpi_scale()
    }
    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.0.set_dpi_scale(dpi_scale);
    }
    fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
        self.0.measure_text(text, font)
    }
    fn shape_text(&self, text: &str, font: &Font) -> Rc<ShapedText> {
        self.0.shape_text(text, font)
    }
    fn frame_rgba(&self) -> &[u8] {
        self.0.frame_rgba()
    }
    fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
        self.0.apply_render_config(config);
    }
    fn render_config(&self) -> SoftwareRenderConfig {
        self.0.render_config()
    }
}

/// Opaque identifier for a recorded batch of draw commands.
///
/// A `BatchId` is created when a batch is recorded and can later be
/// used to replay that batch without re-recording the individual commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BatchId(pub u64);

impl BatchId {
    /// Creates a new `BatchId` from a raw u64 value.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the raw u64 value backing this identifier.
    pub const fn get(&self) -> u64 {
        self.0
    }
}

impl From<u64> for BatchId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Errors that can occur during batch recording operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchError {
    /// Attempted to record a command without an open batch.
    /// Call `begin_batch()` first.
    NoActiveBatch,
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchError::NoActiveBatch => {
                write!(f, "called record() without an open batch; call begin_batch() first")
            }
        }
    }
}

impl core::error::Error for BatchError {}

/// A single draw command that can be recorded into a batch.
///
/// Each variant describes a primitive operation the renderer can replay.
/// Commands are replayed in the order they were recorded, so later commands
/// paint over earlier ones. Two commands are stateful rather than immediate:
/// [`BatchCommand::Translate`] and [`BatchCommand::SetOpacity`] accumulate into
/// the replay transform and affect every *subsequent* command in the batch,
/// exactly like their counterparts in an immediate-mode draw list.
#[derive(Debug, Clone)]
pub enum BatchCommand {
    /// Fill a rectangle with a solid colour.
    ///
    /// `color`'s alpha is multiplied by the active opacity, and the rectangle
    /// is offset by the active translation.
    FillRect {
        /// Target rectangle in batch-local pixel coordinates.
        rect: Rect,
        /// Fill colour.
        color: Color,
    },
    /// Save the current affine transform and compose `matrix` onto it.
    ///
    /// The saved transform is restored by [`BatchCommand::PopTransform`]. If the fixed-capacity
    /// save stack is already full the command is dropped with a warning and the current matrix is
    /// left untouched, so a malformed batch cannot abort a replay.
    #[cfg(feature = "3d")]
    PushTransform {
        /// Local affine composed after the current transform.
        matrix: Mat3,
    },
    /// Restore the affine transform saved by the matching [`BatchCommand::PushTransform`].
    ///
    /// An unbalanced pop leaves the current transform unchanged and logs a warning.
    #[cfg(feature = "3d")]
    PopTransform,
    /// Save the current transform and compose a **projective** matrix onto it.
    ///
    /// This is the pseudo-3D form of [`BatchCommand::PushTransform`]: `matrix` may rotate the
    /// recorded geometry out of the `z = 0` plane and include a perspective term. A rectangle
    /// filled under it becomes a projectively divided quad drawn as two antialiased triangles,
    /// and an image becomes a homography-mapped quad. The matching pop is
    /// [`BatchCommand::PopProjectiveTransform`], but either pop restores either push (the save
    /// stack is shared) so a caller cannot leave a matrix behind by closing the wrong kind.
    #[cfg(feature = "3d")]
    PushProjectiveTransform {
        /// Homogeneous transform composed after the current one.
        matrix: Mat4,
    },
    /// Restore the transform saved by the matching [`BatchCommand::PushProjectiveTransform`].
    #[cfg(feature = "3d")]
    PopProjectiveTransform,
    /// Stroke a rectangular border.
    ///
    /// Translated by the active offset; `width` is in pixels and is truncated
    /// to an integer by the replay path.
    StrokeRect {
        /// Rectangle whose outline is stroked.
        rect: Rect,
        /// Stroke colour, with alpha scaled by the active opacity.
        color: Color,
        /// Stroke width in pixels.
        width: f32,
    },
    /// Draw a line between two points.
    ///
    /// Both endpoints are offset by the active translation; `width` is in
    /// pixels and is truncated to an integer by the replay path.
    DrawLine {
        /// Start point, in batch-local pixel coordinates.
        from: Point,
        /// End point, in batch-local pixel coordinates.
        to: Point,
        /// Stroke colour, with alpha scaled by the active opacity.
        color: Color,
        /// Stroke width in pixels.
        width: f32,
    },
    /// Draw an image identified by its resource id.
    ///
    /// The replay path looks the id up in the backend's image cache, which
    /// must already contain raw RGBA bytes for it; an unknown id logs a warning
    /// and the command is dropped. The per-command `opacity` is multiplied by
    /// the active opacity and, when the product is below `1.0`, baked into the
    /// image's alpha bytes before drawing.
    DrawImage {
        /// Destination rectangle, offset by the active translation.
        rect: Rect,
        /// Cache key of the source image.
        image_id: ObjectId,
        /// Multiplier in `0.0..=1.0` applied to the image's alpha channel.
        opacity: f32,
    },
    /// Draw a clipped region of an image.
    ///
    /// `source_width` / `source_height` describe the full source image the
    /// `source` rectangle indexes into. They are stored with the command because
    /// the image cache holds only raw pixel bytes, which do not say how many
    /// pixels a row has (G-14).
    DrawImageSubrect {
        /// Destination rectangle on the surface.
        dest: Rect,
        /// Sub-rectangle of the source image to draw, in source pixels.
        source: Rect,
        /// Full source image width in pixels.
        source_width: u32,
        /// Full source image height in pixels.
        source_height: u32,
        /// Cache key of the source image.
        image_id: ObjectId,
        /// Multiplier in `0.0..=1.0` applied to the image's alpha channel.
        opacity: f32,
    },
    /// Draw text at the given position.
    ///
    /// Replay renders with the batch's fixed default font family and
    /// `font_size`, left-aligned, so the recorded text carries no font or
    /// alignment of its own.
    DrawText {
        /// Baseline start point, offset by the active translation.
        position: Point,
        /// Text to draw, shared so recording and replay never copy the label (G-12).
        text: Arc<str>,
        /// Text colour, with alpha scaled by the active opacity.
        color: Color,
        /// Font size in logical points.
        font_size: f32,
    },
    /// Push a clipping rectangle – subsequent commands are clipped.
    ///
    /// Translated by the active offset, and intersected with any enclosing
    /// clip. Must be balanced by a matching [`BatchCommand::PopClip`].
    PushClip {
        /// Clip rectangle in batch-local pixel coordinates.
        rect: Rect,
    },
    /// Pop the most recent clipping rectangle.
    PopClip,
    /// Apply a translation offset to all subsequent commands.
    ///
    /// The offsets accumulate rather than replace, and survive until the end of
    /// the batch: there is no "reset translation" command. Recorded clips are
    /// not affected retroactively.
    Translate {
        /// Additional X offset in pixels, added to the current translation.
        dx: f32,
        /// Additional Y offset in pixels, added to the current translation.
        dy: f32,
    },
    /// Apply an opacity multiplier to all subsequent commands.
    ///
    /// The multiplier is applied cumulatively (it multiplies the running
    /// value) and only affects alpha-bearing primitives, i.e. fills, strokes,
    /// lines, text, and images.
    SetOpacity {
        /// Multiplier in `0.0..=1.0`; `1.0` leaves alpha unchanged.
        opacity: f32,
    },
}

/// Trait implemented by renderers that can record and replay draw batches.
///
/// # Usage
///
/// ```text
/// fn render(batcher: &mut impl BatchRenderer) -> Result<(), BatchError> {
///     let batch_id = batcher.begin_batch();
///     batcher.record(BatchCommand::FillRect {
///         rect: Rect::new(0, 0, 100, 100),
///         color: Color::rgb(255, 0, 0),
///     })?;
///     batcher.end_batch();
///     batcher.replay(batch_id);
///     Ok(())
/// }
/// ```
pub trait BatchRenderer {
    /// Opens a new, empty batch and returns its identifier.
    ///
    /// Identifiers are allocated monotonically per renderer and are never
    /// reused, so a stale id simply becomes "not found" rather than aliasing a
    /// newer batch. Only one batch is open at a time: calling this while
    /// another batch is open abandons the previous one without closing it, and
    /// subsequent [`BatchRenderer::record`] calls go to the new batch.
    fn begin_batch(&mut self) -> BatchId;

    /// Closes the batch opened by the last [`BatchRenderer::begin_batch`].
    ///
    /// The recorded commands are retained for later
    /// [`BatchRenderer::replay`]; nothing is drawn here. Recording after this
    /// call fails with `BatchError::NoActiveBatch`. Calling it with no open
    /// batch is a no-op.
    fn end_batch(&mut self);

    /// Appends one command to the currently open batch.
    ///
    /// Commands are stored verbatim and in order; no drawing happens until
    /// [`BatchRenderer::replay`] is called.
    ///
    /// # Errors
    ///
    /// Returns `BatchError::NoActiveBatch` when there is no open batch, i.e.
    /// [`BatchRenderer::begin_batch`] was not called or
    /// [`BatchRenderer::end_batch`] has already closed it.
    fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError>;

    /// Re-draws a previously recorded batch immediately.
    ///
    /// Commands are executed in recorded order, so later commands overdraw
    /// earlier ones. Batches are not consumed and can be replayed repeatedly.
    /// Replaying an id that was never created or has been destroyed is a
    /// no-op.
    fn replay(&mut self, id: BatchId);

    /// Removes a batch and discards its recorded commands.
    ///
    /// If `id` is the batch currently being recorded, recording is closed as
    /// well, so subsequent [`BatchRenderer::record`] calls fail. Unknown ids
    /// are ignored. Ids are not recycled, so `id` never becomes valid again.
    fn destroy_batch(&mut self, id: BatchId);

    /// Returns `true` while `id` still refers to a live batch.
    ///
    /// A batch is live from its [`BatchRenderer::begin_batch`] until
    /// [`BatchRenderer::destroy_batch`].
    fn contains_batch(&self, id: BatchId) -> bool;

    /// Returns the number of live batches, including any currently open one.
    fn batch_count(&self) -> usize;
}

/// Default font family used when replaying a `DrawText` batch command.
const BATCH_DEFAULT_FONT_FAMILY: &str = "Arial";

/// Extension data held alongside the batch implementation on
/// [`SoftwarePaintBackend`].
///
/// This struct is stored as a field of the backend and provides all
/// the bookkeeping needed to satisfy the `BatchRenderer` trait.
#[derive(Debug, Clone)]
pub(crate) struct BatchState {
    /// Incrementing counter used to generate fresh `BatchId` values.
    next_id: u64,
    /// Active batch being recorded, if any.
    current_batch: Option<BatchId>,
    /// All recorded batches, keyed by `BatchId`.
    batches: HashMap<BatchId, Vec<BatchCommand>>,
    /// Optional image data cache mapping `ObjectId` → RGBA pixel bytes.
    /// Populated externally before replay so that `DrawImage` /
    /// `DrawImageSubrect` commands can be translated into `RenderCommand`s.
    ///
    /// The bytes are shared (`Arc`), so replay clones a refcount instead of the
    /// whole RGBA payload (G-02).
    pub(crate) images: HashMap<ObjectId, Arc<[u8]>>,
}

impl BatchState {
    /// Creates a fresh, empty batch state.
    pub(crate) fn new() -> Self {
        Self { next_id: 0, current_batch: None, batches: HashMap::new(), images: HashMap::new() }
    }

    /// Begin recording a new batch. Returns the batch id.
    pub(crate) fn begin_batch(&mut self) -> BatchId {
        let id = BatchId::new(self.next_id);
        self.next_id += 1;
        self.batches.insert(id, Vec::new());
        self.current_batch = Some(id);
        id
    }

    /// Finish recording the current batch.
    pub(crate) fn end_batch(&mut self) {
        self.current_batch = None;
    }

    /// Record a single command into the currently open batch.
    ///
    /// # Errors
    ///
    /// Returns `Err(BatchError::NoActiveBatch)` if there is no open batch
    /// (i.e. `begin_batch` has not been called, or `end_batch` has already
    /// been called).
    pub(crate) fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError> {
        let id = self.current_batch.ok_or(BatchError::NoActiveBatch)?;
        if let Some(cmds) = self.batches.get_mut(&id) {
            cmds.push(cmd);
        }
        Ok(())
    }

    /// Remove a batch and free its resources.
    pub(crate) fn destroy_batch(&mut self, id: BatchId) {
        if self.current_batch == Some(id) {
            self.current_batch = None;
        }
        self.batches.remove(&id);
    }

    /// Check whether a batch id is still valid.
    pub(crate) fn contains_batch(&self, id: BatchId) -> bool {
        self.batches.contains_key(&id)
    }

    /// Return the number of currently recorded batches.
    pub(crate) fn batch_count(&self) -> usize {
        self.batches.len()
    }
}

/// The screen-space bounding box of four mapped corners.
#[cfg(feature = "3d")]
fn bounds_of(corners: &[Point; 4]) -> Rect {
    let min_x = corners.iter().map(|p| p.x).min().unwrap_or(0);
    let min_y = corners.iter().map(|p| p.y).min().unwrap_or(0);
    let max_x = corners.iter().map(|p| p.x).max().unwrap_or(0);
    let max_y = corners.iter().map(|p| p.y).max().unwrap_or(0);
    Rect::new(
        min_x,
        min_y,
        max_x.saturating_sub(min_x).max(0) as u32,
        max_y.saturating_sub(min_y).max(0) as u32,
    )
}

/// Accumulated transform state that tracks translation, opacity and (with the `3d` feature)
/// the current affine matrix across consecutive [`BatchCommand`]s.
///
/// With `3d` off the type is exactly the 2D state it has always been: two offsets and an
/// opacity. The affine field and its save stack exist only in a `3d` build, which is what keeps
/// the default binary unchanged.
#[derive(Debug, Clone, Copy)]
struct TransformState {
    /// Accumulated X offset.
    dx: f32,
    /// Accumulated Y offset.
    dy: f32,
    /// Accumulated opacity multiplier (1.0 = fully opaque).
    opacity: f32,
    /// Current affine matrix, composed by `PushTransform`/`PopTransform`.
    #[cfg(feature = "3d")]
    matrix: Mat3,
    /// Current full homogeneous transform, including any pseudo-3D rotation.
    ///
    /// Equal to `Mat4::from_affine(matrix)` while only affine pushes are active. A
    /// [`BatchCommand::PushProjectiveTransform`] composes a perspective matrix here, and the
    /// geometry arms switch to the projected-quad path for as long as that is true.
    #[cfg(feature = "3d")]
    projective: Mat4,
    /// Saved matrices for balanced push/pop pairs. One stack serves both push forms because a
    /// pop must restore whatever was active, whichever kind of push it closes.
    #[cfg(feature = "3d")]
    stack: TransformStack,
}

impl TransformState {
    /// Whether a projective (pseudo-3D) transform is active.
    ///
    /// The two representations are kept consistent: every affine push composes the same affine
    /// into both, so a projective matrix can only differ from the affine when a projective push
    /// is on the stack.
    #[cfg(feature = "3d")]
    fn is_projective(&self) -> bool {
        self.projective != Mat4::from_affine(self.matrix)
    }

    fn apply_to_rect(&self, rect: &Rect) -> Rect {
        #[cfg(feature = "3d")]
        {
            if self.is_projective() {
                if let Some(corners) = self.apply_to_projective_quad(rect) {
                    return bounds_of(&corners);
                }
                // The quad crosses the eye plane and cannot be divided; drawing its bounding box
                // would put ink where the geometry is not, so collapse it to nothing.
                return Rect::new(0, 0, 0, 0);
            }
            if self.matrix != Mat3::IDENTITY {
                // A rotated or sheared rectangle is no longer a rectangle. The non-quad
                // primitives that still travel through this helper (strokes, images, clips,
                // text anchors) take the bounding box of the mapped corners rather than
                // silently ignoring the transform.
                return bounds_of(&self.apply_to_quad(rect));
            }
        }
        Rect::new(
            (rect.x as f32 + self.dx).round() as i32,
            (rect.y as f32 + self.dy).round() as i32,
            rect.width,
            rect.height,
        )
    }

    fn apply_to_point(&self, pt: &Point) -> Point {
        #[cfg(feature = "3d")]
        let (x, y) = {
            if self.is_projective() {
                match self.map_projective(pt.x as f32, pt.y as f32) {
                    Some((x, y)) => (x + self.dx, y + self.dy),
                    // A point on or behind the eye plane is not drawable; parking it at the
                    // origin is the least surprising fallback for a text anchor or line end.
                    None => return Point::new(0, 0),
                }
            } else {
                let (x, y) = self.map_xy(pt.x as f32, pt.y as f32);
                (x + self.dx, y + self.dy)
            }
        };
        #[cfg(not(feature = "3d"))]
        let (x, y) = (pt.x as f32 + self.dx, pt.y as f32 + self.dy);
        Point::new(x.round() as i32, y.round() as i32)
    }

    /// Maps all four rectangle corners through the affine matrix and the translation.
    #[cfg(feature = "3d")]
    fn apply_to_quad(&self, rect: &Rect) -> [Point; 4] {
        let x0 = rect.x as f32;
        let y0 = rect.y as f32;
        let x1 = x0 + rect.width as f32;
        let y1 = y0 + rect.height as f32;
        let source = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let mut corners = [Point::new(0, 0); 4];
        for (slot, (x, y)) in corners.iter_mut().zip(source) {
            let (mx, my) = self.map_xy(x, y);
            *slot = Point::new((mx + self.dx).round() as i32, (my + self.dy).round() as i32);
        }
        corners
    }

    /// Maps a point through the affine matrix, without translation or rounding.
    #[cfg(feature = "3d")]
    fn map_xy(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.matrix.m;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// Maps a point through the full homogeneous matrix and divides by `w`.
    ///
    /// Returns `None` for `w <= 0`: those points are on or behind the eye plane and have no
    /// screen position. Callers drop such geometry instead of drawing a mirrored or infinite
    /// projection.
    #[cfg(feature = "3d")]
    fn map_projective(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let (px, py, w) = self.projective.project_row(x, y);
        if !w.is_finite() || w <= f32::EPSILON {
            return None;
        }
        Some((px / w, py / w))
    }

    /// Maps a rectangle's corners through the projective matrix, or `None` when any corner is
    /// not projectable.
    #[cfg(feature = "3d")]
    fn apply_to_projective_quad(&self, rect: &Rect) -> Option<[Point; 4]> {
        let x0 = rect.x as f32;
        let y0 = rect.y as f32;
        let x1 = x0 + rect.width as f32;
        let y1 = y0 + rect.height as f32;
        let source = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
        let mut corners = [Point::new(0, 0); 4];
        for (slot, (x, y)) in corners.iter_mut().zip(source) {
            let (mx, my) = self.map_projective(x, y)?;
            *slot = Point::new((mx + self.dx).round() as i32, (my + self.dy).round() as i32);
        }
        Some(corners)
    }

    /// The approximate screen-space scale of the projective transform, from the projected lengths
    /// of a unit square's `x` and `y` edges.
    ///
    /// Used only by primitives without corners (text, line width). Text under a perspective is a
    /// documented approximation: it scales and moves with the transform but its baseline stays
    /// upright and its glyphs are not warped.
    #[cfg(feature = "3d")]
    fn projective_length_scale(&self) -> f32 {
        let origin = self.map_projective(0.0, 0.0);
        let unit_x = self.map_projective(1.0, 0.0);
        let unit_y = self.map_projective(0.0, 1.0);
        let (Some(origin), Some(unit_x), Some(unit_y)) = (origin, unit_x, unit_y) else {
            return 1.0;
        };
        let x = (unit_x.0 - origin.0).hypot(unit_x.1 - origin.1);
        let y = (unit_y.0 - origin.0).hypot(unit_y.1 - origin.1);
        let scale = (x + y) * 0.5;
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
    }

    /// Maps a point through the full homogeneous matrix, or `None` when it cannot be projected.
    #[cfg(feature = "3d")]
    fn try_apply_to_point(&self, pt: &Point) -> Option<Point> {
        if !self.is_projective() {
            return Some(self.apply_to_point(pt));
        }
        let (x, y) = self.map_projective(pt.x as f32, pt.y as f32)?;
        Some(Point::new((x + self.dx).round() as i32, (y + self.dy).round() as i32))
    }

    fn apply_to_color(&self, color: &Color) -> Color {
        // `self.opacity` can be NaN when a caller fed a non-finite `SetOpacity` (see below), and
        // `NaN.round() as u8` is 0, so the colour would silently vanish. Clamping first turns any
        // out-of-range value into a defined endpoint and any NaN into 0, which is the honest
        // "fully transparent" rather than "undefined turned into an arbitrary byte".
        Color {
            r: color.r,
            g: color.g,
            b: color.b,
            a: (color.a as f32 * self.opacity).clamp(0.0, 255.0) as u8,
        }
    }
}

impl Default for TransformState {
    fn default() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            opacity: 1.0,
            #[cfg(feature = "3d")]
            matrix: Mat3::IDENTITY,
            #[cfg(feature = "3d")]
            projective: Mat4::IDENTITY,
            #[cfg(feature = "3d")]
            stack: TransformStack::new(),
        }
    }
}

/// Translate a single [`BatchCommand`] into a [`RenderCommand`].
///
/// Some batch commands carry higher-level semantics not directly represented by
/// the low-level `RenderCommand` enum. In those cases the translation makes
/// reasonable assumptions (e.g. using the default UI font family with the
/// requested size for text, or embedding image data looked up from the cache).
///
/// Returns `None` for commands that only update the transform state (e.g.
/// [`BatchCommand::Translate`], [`BatchCommand::SetOpacity`]).
///
/// # Why this is a free function, not a `BatchState` method (D09-RENDER-05)
///
/// The replay path used to `clone()` the whole [`BatchState`] before calling
/// `BatchState::replay`, because a method on `self` could not take the shared
/// borrow of `batches`/`images` that translation needs *and* the mutable borrow
/// of `backend` that painting needs at the same time. That clone deep-copied
/// every unrelated batch command and every cached image byte on each replay, so
/// replaying one small batch cost the size of the entire state. Made free, it
/// takes the two maps as separate borrows, which lets [`replay_into`] split
/// `SoftwarePaintBackend`'s fields and pass them without any clone.
fn transform_command(
    cmd: &BatchCommand,
    images: &HashMap<ObjectId, Arc<[u8]>>,
    state: &mut TransformState,
) -> Option<RenderCommand> {
    match cmd {
        BatchCommand::FillRect { rect, color } => {
            #[cfg(feature = "3d")]
            {
                // A projective transform has no affine equivalent: the rectangle becomes a
                // general quad whose opposite edges may differ in length. It is emitted through
                // the antialiased quad command so the projected edge, not its bounding box, is
                // what gets coverage.
                if state.is_projective() {
                    return state.apply_to_projective_quad(rect).map(|corners| {
                        RenderCommand::FillQuadAA { corners, color: state.apply_to_color(color) }
                    });
                }
                // A non-identity affine means the four corners no longer coincide with the
                // axis-aligned rectangle, so the command becomes a quad. The identity case
                // keeps the exact 2D rounding path below.
                if state.matrix != Mat3::IDENTITY {
                    return Some(RenderCommand::FillQuad {
                        corners: state.apply_to_quad(rect),
                        color: state.apply_to_color(color),
                    });
                }
            }
            Some(RenderCommand::FillRect {
                rect: state.apply_to_rect(rect),
                color: state.apply_to_color(color),
            })
        }

        #[cfg(feature = "3d")]
        BatchCommand::PushTransform { matrix } => {
            let saved = Transform::from_matrix(state.projective);
            if !state.stack.push(saved) {
                log::warn!(
                    "[batch] PushTransform rejected: the save stack already holds its maximum of {} entries",
                    TRANSFORM_STACK_MAX
                );
                return None;
            }
            state.matrix = state.matrix.then(matrix);
            state.projective = state.projective.then(&Mat4::from_affine(*matrix));
            None
        }

        #[cfg(feature = "3d")]
        BatchCommand::PopTransform => {
            match state.stack.pop() {
                Some(saved) => {
                    state.matrix = saved.affine();
                    state.projective = saved.matrix();
                }
                None => log::warn!("[batch] PopTransform with an empty save stack"),
            }
            None
        }

        #[cfg(feature = "3d")]
        BatchCommand::PushProjectiveTransform { matrix } => {
            let saved = Transform::from_matrix(state.projective);
            if !state.stack.push(saved) {
                log::warn!(
                    "[batch] PushProjectiveTransform rejected: the save stack already holds its maximum of {} entries",
                    TRANSFORM_STACK_MAX
                );
                return None;
            }
            state.projective = state.projective.then(matrix);
            // The affine view is used only by primitives without corners (text size, clip boxes,
            // line endpoints); the geometry arms above select the projective path from
            // `is_projective`, so this derivative cannot make them take the affine branch.
            state.matrix = Transform::from_matrix(state.projective).affine();
            None
        }

        #[cfg(feature = "3d")]
        BatchCommand::PopProjectiveTransform => {
            match state.stack.pop() {
                Some(saved) => {
                    state.matrix = saved.affine();
                    state.projective = saved.matrix();
                }
                None => log::warn!("[batch] PopProjectiveTransform with an empty save stack"),
            }
            None
        }

        BatchCommand::StrokeRect { rect, color, width } => {
            #[cfg(feature = "3d")]
            {
                if state.is_projective() {
                    return state.apply_to_projective_quad(rect).map(|corners| {
                        RenderCommand::StrokeQuad {
                            corners,
                            color: state.apply_to_color(color),
                            width: (*width).round().max(1.0) as u32,
                        }
                    });
                }
                // The outline of an affine-mapped rectangle is four connected edges, not the
                // outline of its bounding box: the box's corners are where the shape *is not*.
                if state.matrix != Mat3::IDENTITY {
                    return Some(RenderCommand::StrokeQuad {
                        corners: state.apply_to_quad(rect),
                        color: state.apply_to_color(color),
                        width: (*width).round().max(1.0) as u32,
                    });
                }
            }
            Some(RenderCommand::DrawRectStroke {
                rect: state.apply_to_rect(rect),
                color: state.apply_to_color(color),
                width: (*width).round().max(1.0) as u32,
            })
        }

        BatchCommand::DrawLine { from, to, color, width } => Some(RenderCommand::DrawLineStroke {
            from: state.apply_to_point(from),
            to: state.apply_to_point(to),
            color: state.apply_to_color(color),
            width: (*width).round().max(1.0) as u32,
        }),

        BatchCommand::DrawImage { rect, image_id, opacity } => {
            let Some(source) = images.get(image_id).cloned() else {
                log::warn!("[batch] DrawImage references unknown image id {image_id}; dropping");
                return None;
            };
            if source.is_empty() {
                log::warn!("[batch] DrawImage references an empty image id {image_id}; dropping");
                return None;
            }
            // Test-only accounting of how many source pixel bytes this translation
            // copies. It is the observable the D09-RENDER-05 regression test asserts
            // on: replaying a batch must not copy image data belonging to batches that
            // are not being replayed. Compiled out of production builds (principle #28).
            #[cfg(test)]
            note_image_bytes_translated(source.len());
            let applied_rect = state.apply_to_rect(rect);
            let combined_opacity = state.opacity * opacity;
            // RenderCommand::DrawImage has no alpha channel, so a partial opacity is baked into
            // the image's alpha bytes. The baked variant is cached by source identity + opacity
            // (G-02), so replaying the same faded image frame after frame copies it once.
            let data = if combined_opacity < 1.0 && source.len() % 4 == 0 {
                baked_image(&source, combined_opacity)
            } else {
                source
            };
            #[cfg(feature = "3d")]
            {
                // A rotated image keeps its pixels: the destination is the mapped quad, and the
                // source grid is the rectangle the 2D path would have stretched the data across.
                // Under a projective transform the quad is a general quadrilateral, which the
                // surface maps with a homography rather than an affine inverse.
                if state.is_projective() {
                    return state.apply_to_projective_quad(rect).map(|corners| {
                        RenderCommand::DrawImageQuad {
                            corners,
                            width: rect.width,
                            height: rect.height,
                            data,
                        }
                    });
                }
                if state.matrix != Mat3::IDENTITY {
                    return Some(RenderCommand::DrawImageQuad {
                        corners: state.apply_to_quad(rect),
                        width: rect.width,
                        height: rect.height,
                        data,
                    });
                }
            }
            Some(RenderCommand::DrawImage {
                x: applied_rect.x,
                y: applied_rect.y,
                width: applied_rect.width,
                height: applied_rect.height,
                data,
            })
        }

        BatchCommand::DrawImageSubrect {
            dest,
            source,
            source_width,
            source_height,
            image_id,
            opacity,
        } => {
            let Some(data) = images.get(image_id) else {
                log::warn!(
                    "[batch] DrawImageSubrect references unknown image id {image_id}; dropping"
                );
                return None;
            };
            // Crop the requested source rectangle into its own tightly packed RGBA buffer and
            // stretch that to `dest`. Replaying the full image would draw the wrong pixels, and
            // dropping it (the old behaviour) lost the command entirely (G-14).
            let Some(cropped) =
                crop_source_rect_cached(data, *source_width, *source_height, *source)
            else {
                log::warn!(
                    "[batch] DrawImageSubrect (image={image_id}) has an empty or out-of-bounds \
                     source rectangle {source:?} for a {source_width}x{source_height} image; dropping"
                );
                return None;
            };
            #[cfg(test)]
            note_image_bytes_translated(cropped.len());
            let applied_rect = state.apply_to_rect(dest);
            let combined_opacity = state.opacity * opacity;
            let data: Arc<[u8]> = cropped;
            let data = if combined_opacity < 1.0 && data.len().is_multiple_of(4) {
                baked_image(&data, combined_opacity)
            } else {
                data
            };
            Some(RenderCommand::DrawImage {
                x: applied_rect.x,
                y: applied_rect.y,
                width: applied_rect.width,
                height: applied_rect.height,
                data,
            })
        }

        BatchCommand::DrawText { position, text, color, font_size } => {
            // A glyph has no corners to transform, so under an affine it follows the transform's
            // **length scale**: a rotated-and-scaled card keeps its captions in proportion instead
            // of drawing them at their untransformed size. The baseline stays upright and the
            // anchor moves with the transform — rotating each glyph is a text-layout feature this
            // path does not have, and pretending otherwise by shearing the run would be worse than
            // the honest upright approximation.
            #[cfg(feature = "3d")]
            let scale = if state.is_projective() {
                state.projective_length_scale()
            } else if state.matrix == Mat3::IDENTITY {
                1.0
            } else {
                state.matrix.length_scale()
            };
            #[cfg(not(feature = "3d"))]
            let scale = 1.0f32;
            // Interned family: the batch replays the same default face for every label, so a
            // per-label `Font::simple` allocated the family name each time (G-005).
            let font =
                Font::simple_interned(BATCH_DEFAULT_FONT_FAMILY, (*font_size * scale).max(1.0));
            #[cfg(feature = "3d")]
            let origin = state.try_apply_to_point(position)?;
            #[cfg(not(feature = "3d"))]
            let origin = state.apply_to_point(position);
            Some(RenderCommand::DrawText {
                origin,
                text: Arc::clone(text),
                font,
                color: state.apply_to_color(color),
                alignment: HorizontalAlignment::Left,
            })
        }

        BatchCommand::PushClip { rect } => {
            let applied = state.apply_to_rect(rect);
            Some(RenderCommand::PushClip {
                x: applied.x,
                y: applied.y,
                width: applied.width,
                height: applied.height,
            })
        }

        BatchCommand::PopClip => Some(RenderCommand::PopClip),

        // Translate and SetOpacity update the transform state but
        // produce no visible output.
        BatchCommand::Translate { dx, dy } => {
            state.dx += dx;
            state.dy += dy;
            None
        }
        BatchCommand::SetOpacity { opacity } => {
            // A non-finite opacity is not a meaningful factor: `NaN` poisons every later
            // multiplication and `Infinity` saturates every colour to one endpoint. Reject it at
            // the source so the batch keeps its last good opacity instead of silently losing the
            // colour for every command that follows.
            if opacity.is_finite() {
                state.opacity = (state.opacity * opacity).clamp(0.0, 1.0);
            } else {
                log::warn!(
                    "[batch] SetOpacity({opacity}) rejected: opacity must be finite; keeping {}",
                    state.opacity
                );
            }
            None
        }
    }
}

/// Key for the alpha-baked image cache: source address, source length and opacity.
type BakedImageKey = (usize, usize, u32);
/// A cached variant: the source kept alive plus the baked bytes.
type BakedImageEntry = (Arc<[u8]>, Arc<[u8]>);

thread_local! {
    // `HashMap::new` is const only under alloc_frugal (BTreeMap); the std profile cannot be const.
    /// Alpha-baked image variants plus the FIFO order their keys are evicted in.
    ///
    /// Each entry holds the source `Arc` as well as the baked bytes. Keeping the source alive is
    /// what makes the address a stable identity: without it the allocation could be freed and a
    /// different image could reuse the same address. Bounded because a frame can fade many images.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static BAKED_IMAGES: core::cell::RefCell<(VecDeque<BakedImageKey>, HashMap<BakedImageKey, BakedImageEntry>)> =
        core::cell::RefCell::new((VecDeque::new(), HashMap::new()));
}

/// How many alpha-baked image variants one thread keeps before evicting the oldest.
const BAKED_IMAGE_CACHE_LIMIT: usize = 32;

/// Returns `source` with every alpha byte scaled by `opacity`, reusing a previously baked variant
/// when the same source allocation is faded by the same factor (G-02).
fn baked_image(source: &Arc<[u8]>, opacity: f32) -> Arc<[u8]> {
    let key = (source.as_ptr() as usize, source.len(), opacity.to_bits());
    if let Some(hit) =
        BAKED_IMAGES.with(|cache| cache.borrow().1.get(&key).map(|(_, baked)| Arc::clone(baked)))
    {
        return hit;
    }
    let mut baked = source.to_vec();
    for pixel in baked.as_chunks_mut::<4>().0 {
        pixel[3] = (pixel[3] as f32 * opacity).round().clamp(0.0, 255.0) as u8;
    }
    let baked: Arc<[u8]> = Arc::from(baked);
    BAKED_IMAGES.with(|cache| {
        let mut cache = cache.borrow_mut();
        // Evict the oldest entry instead of clearing the table: a fade cycle rotates through a
        // small set of opacities, and clearing threw every warm variant away (G-205).
        if cache.0.len() >= BAKED_IMAGE_CACHE_LIMIT {
            if let Some(oldest) = cache.0.pop_front() {
                cache.1.remove(&oldest);
            }
        }
        cache.0.push_back(key);
        cache.1.insert(key, (Arc::clone(source), Arc::clone(&baked)));
    });
    baked
}

/// Copies `source`'s requested sub-rectangle into a tightly packed RGBA buffer.
///
/// The rectangle is clipped to the source image; `None` means the crop is empty or the source
/// buffer does not match the dimensions it claims. `source_width`/`source_height` are the
/// metadata a batch records with the command (G-14).
fn crop_source_rect(
    data: &[u8],
    source_width: u32,
    source_height: u32,
    source: Rect,
) -> Option<Vec<u8>> {
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let expected = (source_width as usize)
        .checked_mul(source_height as usize)
        .and_then(|pixels| pixels.checked_mul(4))?;
    if data.len() < expected {
        log::warn!(
            "[batch] DrawImageSubrect source buffer is {} bytes but {source_width}x{source_height} \
             needs {expected}; dropping",
            data.len()
        );
        return None;
    }
    let left = i64::from(source.x).max(0);
    let top = i64::from(source.y).max(0);
    let right = (i64::from(source.x) + i64::from(source.width)).min(i64::from(source_width));
    let bottom = (i64::from(source.y) + i64::from(source.height)).min(i64::from(source_height));
    if right <= left || bottom <= top {
        return None;
    }
    let width = (right - left) as usize;
    let height = (bottom - top) as usize;
    let mut cropped = vec![0u8; width * height * 4];
    let source_stride = source_width as usize * 4;
    let row_bytes = width * 4;
    for row in 0..height {
        let src = (top as usize + row) * source_stride + left as usize * 4;
        let dst = row * row_bytes;
        cropped[dst..dst + row_bytes].copy_from_slice(&data[src..src + row_bytes]);
    }
    Some(cropped)
}

/// Key for the cropped-subrect cache: source address, source dimensions and the crop rectangle.
///
/// The dimensions are part of the key because they define the stride the crop is read with; two
/// sources with the same address cannot have different dimensions while the `Arc` is held, but
/// including them makes the key self-describing.
type CroppedImageKey = (usize, u32, u32, i32, i32, u32, u32);
/// A cropped variant: the source kept alive plus the cropped bytes.
type CroppedImageEntry = (Arc<[u8]>, Arc<[u8]>);

thread_local! {
    // `HashMap::new` is const only under alloc_frugal (BTreeMap); the std profile cannot be const.
    /// Cropped sub-rectangles, keyed by the source allocation's address, dimensions and rectangle.
    ///
    /// A batch that replays every frame crops the same source rectangle every frame. Keeping the
    /// result by refcount makes the crop once per content instead of once per replay (G-04).
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static CROPPED_IMAGES: core::cell::RefCell<(VecDeque<CroppedImageKey>, HashMap<CroppedImageKey, CroppedImageEntry>)> =
        core::cell::RefCell::new((VecDeque::new(), HashMap::new()));
}

/// How many cropped sub-rectangles one thread keeps before evicting the oldest (G-04).
const CROPPED_IMAGE_CACHE_LIMIT: usize = 32;

/// Returns the cached crop of `source`'s `rect`, copying it at most once per content (G-04).
///
/// The key uses the source `Arc`'s address and the cache keeps the `Arc` alive, so the address
/// cannot be recycled by a different image while the entry exists.
fn crop_source_rect_cached(
    source: &Arc<[u8]>,
    source_width: u32,
    source_height: u32,
    rect: Rect,
) -> Option<Arc<[u8]>> {
    let key = (
        source.as_ptr() as usize,
        source_width,
        source_height,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    if let Some(hit) = CROPPED_IMAGES
        .with(|cache| cache.borrow().1.get(&key).map(|(_, cropped)| Arc::clone(cropped)))
    {
        return Some(hit);
    }
    let cropped: Arc<[u8]> = crop_source_rect(source, source_width, source_height, rect)?.into();
    CROPPED_IMAGES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.0.len() >= CROPPED_IMAGE_CACHE_LIMIT {
            if let Some(oldest) = cache.0.pop_front() {
                cache.1.remove(&oldest);
            }
        }
        cache.0.push_back(key);
        cache.1.insert(key, (Arc::clone(source), Arc::clone(&cropped)));
    });
    Some(cropped)
}

// Test-only counter of source image bytes copied by `transform_command`.
//
// # Why a counter rather than a global allocator shim (D09-RENDER-05)
//
// The defect is that replaying one batch used to deep-`clone()` the whole
// `BatchState` — every batch's commands and every cached image. The image bytes
// dominate that cost and are the part a test can count without a custom
// `GlobalAlloc`. This counter is compiled only under `cfg(test)`, so it adds no
// work to a release build, and the regression test resets it around the replay it
// is measuring.
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static IMAGE_BYTES_TRANSLATED: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Adds `bytes` to the test-only translation counter. (D09-RENDER-05)
#[cfg(test)]
fn note_image_bytes_translated(bytes: usize) {
    IMAGE_BYTES_TRANSLATED.with(|cell| cell.set(cell.get() + bytes));
}

/// Resets the test-only translation counter and returns its previous value. (D09-RENDER-05)
#[cfg(test)]
fn take_image_bytes_translated() -> usize {
    IMAGE_BYTES_TRANSLATED.with(|cell| cell.replace(0))
}

/// Replay batch `id` from `batches`/**`images`** into `surface` without cloning
/// the batch state.
///
/// # Why this does not clone (D09-RENDER-05)
///
/// [`SoftwarePaintBackend`] holds its batch data in `batch_state`, a field of the
/// backend itself. A plain `self.batch_state.replay(self, id)` borrows `self`
/// immutably (for the lookup) and mutably (for painting) in the same call, which
/// the borrow checker rejects — hence the historical `clone()`. Replaying into
/// the backend's **surface** field instead splits the borrow: the two batch maps
/// come from `batch_state` and the destination from `surface`, so the lookup
/// borrows only what it reads. Replay time and allocations are therefore
/// proportional to the replayed batch, not to the total size of the backend.
///
/// Commands are painted through [`SurfacePaintBackend`], a zero-copy view over the
/// surface, so the output is byte-for-byte what the backend's own command path
/// produces.
fn replay_into(
    batches: &HashMap<BatchId, Vec<BatchCommand>>,
    images: &HashMap<ObjectId, Arc<[u8]>>,
    surface: &mut SoftwareSurface,
    id: BatchId,
) {
    let Some(cmds) = batches.get(&id) else {
        return;
    };
    let mut state = TransformState::default();
    let mut backend = SurfacePaintBackend(surface);
    for cmd in cmds {
        if let Some(rc) = transform_command(cmd, images, &mut state) {
            backend.execute_command(&rc);
        }
    }
}

impl BatchRenderer for SoftwarePaintBackend {
    /// Opens a new batch in this backend's batch state.
    fn begin_batch(&mut self) -> BatchId {
        self.batch_state.begin_batch()
    }

    /// Closes the current batch, keeping its commands available for replay.
    fn end_batch(&mut self) {
        self.batch_state.end_batch()
    }

    /// Appends a command to the current batch.
    ///
    /// # Errors
    ///
    /// Returns `BatchError::NoActiveBatch` when no batch is open.
    fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError> {
        self.batch_state.record(cmd)
    }

    /// Replays the batch through this backend's own paint path.
    ///
    /// # What changed (D09-RENDER-05)
    ///
    /// This used to `clone()` the entire `BatchState` before replaying, which
    /// deep-copied every recorded batch and every cached image on the backend —
    /// so replaying one small batch cost as much as the whole state. It now
    /// borrows the batch data and the surface as separate fields of `self`
    /// (through `replay_into`), so nothing is cloned and the cost tracks only
    /// the target batch. Replay semantics are unchanged: commands are translated
    /// in order against the same transform/opacity state and drawn through the
    /// same `PaintBackend::execute_command` path.
    fn replay(&mut self, id: BatchId) {
        // Split the borrow of `self` so the batch data (read) and the paint target
        // (mutated) are distinct fields, and neither is the whole of `self` —
        // otherwise the immutable and mutable borrows overlap and the compiler
        // rejects the call (D09-RENDER-05).
        let SoftwarePaintBackend { surface, batch_state } = self;
        let BatchState { batches, images, .. } = batch_state;
        replay_into(batches, images, surface, id);
    }

    /// Drops a batch and its recorded commands.
    ///
    /// If the id is the batch currently open, recording is closed too.
    fn destroy_batch(&mut self, id: BatchId) {
        self.batch_state.destroy_batch(id)
    }

    /// Returns whether the id still refers to a live batch.
    fn contains_batch(&self, id: BatchId) -> bool {
        self.batch_state.contains_batch(id)
    }

    /// Returns the number of live batches held by this backend.
    fn batch_count(&self) -> usize {
        self.batch_state.batch_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── BatchId construction & conversions ──────────────────────────────

    #[test]
    fn batch_id_new_and_get() {
        let id = BatchId::new(42);
        assert_eq!(id.get(), 42);
    }

    #[test]
    fn batch_id_from_u64() {
        let id: BatchId = 99u64.into();
        assert_eq!(id.get(), 99);
    }

    #[test]
    fn batch_id_equality_and_hash() {
        let a = BatchId::new(1);
        let b = BatchId::new(1);
        let c = BatchId::new(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn batch_id_copy_behavior() {
        let id = BatchId::new(7);
        let copied = id; // Copy
        assert_eq!(id, copied);
    }

    // ── BatchCommand variant construction ───────────────────────────────

    #[test]
    fn batch_command_fill_rect_roundtrip() {
        let cmd = BatchCommand::FillRect { rect: Rect::new(10, 20, 100, 200), color: Color::RED };
        match cmd {
            BatchCommand::FillRect { rect, color } => {
                assert_eq!(rect, Rect::new(10, 20, 100, 200));
                assert_eq!(color, Color::RED);
            }
            _ => panic!("expected FillRect variant"),
        }
    }

    #[test]
    fn batch_command_stroke_rect_roundtrip() {
        let cmd = BatchCommand::StrokeRect {
            rect: Rect::new(5, 5, 50, 50),
            color: Color::GREEN,
            width: 2.0,
        };
        match cmd {
            BatchCommand::StrokeRect { rect, color, width } => {
                assert_eq!(rect, Rect::new(5, 5, 50, 50));
                assert_eq!(color, Color::GREEN);
                assert!((width - 2.0).abs() < 1e-6);
            }
            _ => panic!("expected StrokeRect variant"),
        }
    }

    #[test]
    fn batch_command_draw_line_roundtrip() {
        let from = Point::new(0, 0);
        let to = Point::new(100, 100);
        let cmd = BatchCommand::DrawLine { from, to, color: Color::BLUE, width: 3.0 };
        match cmd {
            BatchCommand::DrawLine { from: f, to: t, color, width } => {
                assert_eq!(f, from);
                assert_eq!(t, to);
                assert_eq!(color, Color::BLUE);
                assert!((width - 3.0).abs() < 1e-6);
            }
            _ => panic!("expected DrawLine variant"),
        }
    }

    #[test]
    fn batch_command_draw_image_roundtrip() {
        let cmd =
            BatchCommand::DrawImage { rect: Rect::new(0, 0, 32, 32), image_id: 1u64, opacity: 0.8 };
        match cmd {
            BatchCommand::DrawImage { rect, image_id, opacity } => {
                assert_eq!(rect, Rect::new(0, 0, 32, 32));
                assert_eq!(image_id, 1u64);
                assert!((opacity - 0.8).abs() < 1e-6);
            }
            _ => panic!("expected DrawImage variant"),
        }
    }

    #[test]
    fn batch_command_draw_image_subrect_roundtrip() {
        let cmd = BatchCommand::DrawImageSubrect {
            dest: Rect::new(10, 10, 64, 64),
            source: Rect::new(0, 0, 32, 32),
            source_width: 128,
            source_height: 64,
            image_id: 2u64,
            opacity: 0.5,
        };
        match cmd {
            BatchCommand::DrawImageSubrect {
                dest,
                source,
                source_width,
                source_height,
                image_id,
                opacity,
            } => {
                assert_eq!(dest, Rect::new(10, 10, 64, 64));
                assert_eq!(source, Rect::new(0, 0, 32, 32));
                assert_eq!((source_width, source_height), (128, 64));
                assert_eq!(image_id, 2u64);
                assert!((opacity - 0.5).abs() < 1e-6);
            }
            _ => panic!("expected DrawImageSubrect variant"),
        }
    }

    #[test]
    fn batch_command_draw_text_roundtrip() {
        let cmd = BatchCommand::DrawText {
            position: Point::new(15, 30),
            text: Arc::from("Hello"),
            color: Color::WHITE,
            font_size: 16.0,
        };
        match cmd {
            BatchCommand::DrawText { position, text, color, font_size } => {
                assert_eq!(position, Point::new(15, 30));
                assert_eq!(&*text, "Hello");
                assert_eq!(color, Color::WHITE);
                assert!((font_size - 16.0).abs() < 1e-6);
            }
            _ => panic!("expected DrawText variant"),
        }
    }

    /// G-12: translating a recorded label hands over the same `Arc<str>`, not a copy of its bytes.
    #[test]
    fn replaying_a_label_shares_the_recorded_text() {
        let text: Arc<str> = Arc::from("a label shared by recording and replay");
        let recorded = BatchCommand::DrawText {
            position: Point::new(4, 8),
            text: Arc::clone(&text),
            color: Color::BLACK,
            font_size: 12.0,
        };
        let mut state = TransformState::default();
        let translated = transform_command(&recorded, &HashMap::new(), &mut state)
            .expect("a text command translates");
        match translated {
            RenderCommand::DrawText { text: replayed, .. } => {
                assert!(Arc::ptr_eq(&replayed, &text), "replay clones the refcount, not the text");
            }
            other => panic!("expected DrawText, got {other:?}"),
        }
    }

    #[test]
    fn batch_command_push_clip_roundtrip() {
        let cmd = BatchCommand::PushClip { rect: Rect::new(0, 0, 800, 600) };
        match cmd {
            BatchCommand::PushClip { rect } => {
                assert_eq!(rect, Rect::new(0, 0, 800, 600));
            }
            _ => panic!("expected PushClip variant"),
        }
    }

    #[test]
    fn batch_command_pop_clip_roundtrip() {
        let cmd = BatchCommand::PopClip;
        match cmd {
            BatchCommand::PopClip => {} // expected
            _ => panic!("expected PopClip variant"),
        }
    }

    #[test]
    fn batch_command_translate_roundtrip() {
        let cmd = BatchCommand::Translate { dx: 10.0, dy: 20.0 };
        match cmd {
            BatchCommand::Translate { dx, dy } => {
                assert!((dx - 10.0).abs() < 1e-6);
                assert!((dy - 20.0).abs() < 1e-6);
            }
            _ => panic!("expected Translate variant"),
        }
    }

    #[test]
    fn batch_command_set_opacity_roundtrip() {
        let cmd = BatchCommand::SetOpacity { opacity: 0.75 };
        match cmd {
            BatchCommand::SetOpacity { opacity } => {
                assert!((opacity - 0.75).abs() < 1e-6);
            }
            _ => panic!("expected SetOpacity variant"),
        }
    }

    /// With `3d` off, the replay transform state must stay exactly the two offsets plus the
    /// opacity: 12 bytes. A field that leaked into the default build would change this number
    /// silently and grow every batch replay's working set.
    #[cfg(not(feature = "3d"))]
    #[test]
    fn transform_state_is_byte_identical_when_3d_is_off() {
        assert_eq!(core::mem::size_of::<TransformState>(), 12);
    }

    #[cfg(feature = "3d")]
    #[test]
    fn a_pushed_transform_turns_a_fill_rect_into_a_quad_and_pop_restores_it() {
        let mut state = TransformState::default();
        let images = HashMap::new();
        let rotation = Mat3::translation(-5.0, -5.0)
            .then(&Mat3::rotation(core::f32::consts::FRAC_PI_4))
            .then(&Mat3::translation(5.0, 5.0));

        assert!(transform_command(
            &BatchCommand::PushTransform { matrix: rotation },
            &images,
            &mut state
        )
        .is_none());

        let rotated = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED },
            &images,
            &mut state,
        );
        match rotated {
            Some(RenderCommand::FillQuad { corners, .. }) => {
                assert_eq!(corners.len(), 4, "a rotated rectangle is four corners");
                assert_ne!(corners[0].x, corners[1].x, "the top edge is no longer axis-aligned");
            }
            _ => panic!("a non-identity transform must produce FillQuad"),
        }

        assert!(transform_command(&BatchCommand::PopTransform, &images, &mut state).is_none());
        let restored = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED },
            &images,
            &mut state,
        );
        assert!(
            matches!(restored, Some(RenderCommand::FillRect { .. })),
            "PopTransform must restore the identity and the 2D fill path"
        );
    }

    #[cfg(feature = "3d")]
    #[test]
    fn a_pushed_transform_turns_a_stroke_into_four_mapped_edges() {
        let mut state = TransformState::default();
        let images = HashMap::new();
        let rotation = Mat3::rotation(core::f32::consts::FRAC_PI_4);
        assert!(transform_command(
            &BatchCommand::PushTransform { matrix: rotation },
            &images,
            &mut state
        )
        .is_none());

        let stroked = transform_command(
            &BatchCommand::StrokeRect {
                rect: Rect::new(0, 0, 10, 10),
                color: Color::RED,
                width: 2.0,
            },
            &images,
            &mut state,
        );
        match stroked {
            Some(RenderCommand::StrokeQuad { corners, width, .. }) => {
                assert_eq!(width, 2, "the stroke width is in destination pixels");
                for index in 0..4 {
                    let a = corners[index];
                    let b = corners[(index + 1) % 4];
                    assert!(a != b, "edge {index} must have length");
                }
                assert_ne!(corners[0].y, corners[1].y, "a rotated edge is no longer axis-aligned");
            }
            _ => panic!("a rotated stroke must produce StrokeQuad, not a bounding box"),
        }
    }

    #[cfg(feature = "3d")]
    #[test]
    fn a_rotated_image_maps_its_source_grid_onto_the_quad() {
        let mut state = TransformState::default();
        let mut images = HashMap::new();
        images.insert(7u64, Arc::from(vec![255u8; 2 * 2 * 4]));
        let rotation = Mat3::rotation(core::f32::consts::FRAC_PI_4);
        assert!(transform_command(
            &BatchCommand::PushTransform { matrix: rotation },
            &images,
            &mut state
        )
        .is_none());

        let drawn = transform_command(
            &BatchCommand::DrawImage { rect: Rect::new(0, 0, 2, 2), image_id: 7, opacity: 1.0 },
            &images,
            &mut state,
        );
        match drawn {
            Some(RenderCommand::DrawImageQuad { corners, width, height, data }) => {
                assert_eq!((width, height), (2, 2), "the source grid is the untransformed rect");
                assert_eq!(data.len(), 2 * 2 * 4, "the pixels travel with the quad");
                assert_ne!(corners[0].x, corners[1].x, "the top edge rotated");
            }
            _ => panic!("a rotated image must produce DrawImageQuad"),
        }
    }

    #[cfg(feature = "3d")]
    #[test]
    fn text_under_an_affine_follows_the_transform_scale() {
        let mut state = TransformState::default();
        let images = HashMap::new();
        assert!(transform_command(
            &BatchCommand::PushTransform { matrix: Mat3::scaling(2.0, 2.0) },
            &images,
            &mut state
        )
        .is_none());

        let drawn = transform_command(
            &BatchCommand::DrawText {
                position: crate::core::Point::new(0, 0),
                text: Arc::from("hi"),
                color: Color::BLACK,
                font_size: 12.0,
            },
            &images,
            &mut state,
        );
        match drawn {
            Some(RenderCommand::DrawText { font, .. }) => {
                assert!(
                    (font.size() - 24.0).abs() < 0.01,
                    "a 2x scale must double the point size, got {}",
                    font.size()
                );
            }
            _ => panic!("text under an affine is still a DrawText"),
        }
    }

    #[cfg(feature = "3d")]
    #[test]
    fn an_unbalanced_pop_transform_leaves_the_matrix_untouched() {
        let mut state = TransformState::default();
        let images = HashMap::new();
        assert!(transform_command(&BatchCommand::PopTransform, &images, &mut state).is_none());
        assert_eq!(state.matrix, Mat3::IDENTITY);
    }

    // ── BatchState lifecycle ────────────────────────────────────────────

    #[test]
    fn batch_state_initial_state() {
        let state = BatchState::new();
        assert_eq!(state.batch_count(), 0);
        assert!(!state.contains_batch(BatchId::new(0)));
        assert!(state.current_batch.is_none());
    }

    #[test]
    fn batch_state_begin_end_batch_increments_id() {
        let mut state = BatchState::new();
        let id1 = state.begin_batch();
        assert_eq!(id1, BatchId::new(0));
        assert_eq!(state.batch_count(), 1);
        assert!(state.contains_batch(id1));
        state.end_batch();

        let id2 = state.begin_batch();
        assert_eq!(id2, BatchId::new(1));
        assert_eq!(state.batch_count(), 2);
        state.end_batch();
    }

    #[test]
    fn batch_state_record_commands() {
        let mut state = BatchState::new();
        let id = state.begin_batch();
        state
            .record(BatchCommand::FillRect { rect: Rect::new(0, 0, 50, 50), color: Color::RED })
            .unwrap();
        state.record(BatchCommand::PopClip).unwrap();
        state.end_batch();

        let cmds = state.batches.get(&id).unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], BatchCommand::FillRect { .. }));
        assert!(matches!(cmds[1], BatchCommand::PopClip));
    }

    #[test]
    fn batch_state_record_without_begin_returns_error() {
        let mut state = BatchState::new();
        let result = state.record(BatchCommand::PopClip);
        assert_eq!(result, Err(BatchError::NoActiveBatch));
    }

    #[test]
    fn batch_state_destroy_batch_removes_it() {
        let mut state = BatchState::new();
        let id = state.begin_batch();
        state.end_batch();
        assert_eq!(state.batch_count(), 1);

        state.destroy_batch(id);
        assert_eq!(state.batch_count(), 0);
        assert!(!state.contains_batch(id));
    }

    #[test]
    fn batch_state_destroy_batch_clears_current() {
        let mut state = BatchState::new();
        let id = state.begin_batch();
        state.destroy_batch(id); // destroys while still open
        assert!(state.current_batch.is_none());
    }

    #[test]
    fn batch_state_replay_nonexistent_id_is_noop() {
        // Replay goes through the backend's `BatchRenderer::replay`, which is the one
        // production path (D09-RENDER-05 removed the cloning `BatchState::replay`).
        let size = crate::core::Size::new(1, 1);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.replay(BatchId::new(999));
        // No panic and nothing painted is the whole assertion.
    }

    #[test]
    fn batch_state_translate_command_skip_translate_and_set_opacity() {
        let mut state = BatchState::new();
        let id = state.begin_batch();
        state.record(BatchCommand::Translate { dx: 5.0, dy: 5.0 }).unwrap();
        state.record(BatchCommand::SetOpacity { opacity: 0.5 }).unwrap();
        state.end_batch();

        // Translate/SetOpacity are recorded in the batch, but during replay
        // they update the transform state instead of emitting a RenderCommand.
        let cmds = state.batches.get(&id).unwrap();
        assert_eq!(cmds.len(), 2);
        assert!(matches!(cmds[0], BatchCommand::Translate { .. }));
        assert!(matches!(cmds[1], BatchCommand::SetOpacity { .. }));
    }

    // ── BatchRenderer trait via SoftwarePaintBackend ────────────────────

    #[test]
    fn batch_renderer_trait_begin_end_record() {
        let size = crate::core::Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let id = backend.begin_batch();
        backend
            .record(BatchCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED })
            .unwrap();
        backend.end_batch();
        assert!(backend.contains_batch(id));
    }

    #[test]
    fn batch_renderer_destroy_batch() {
        let size = crate::core::Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let id = backend.begin_batch();
        backend.end_batch();

        assert_eq!(backend.batch_count(), 1);
        backend.destroy_batch(id);
        assert_eq!(backend.batch_count(), 0);
    }

    #[test]
    fn batch_renderer_replay_fill_rect() {
        let size = crate::core::Size::new(50, 50);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);

        let id = backend.begin_batch();
        backend
            .record(BatchCommand::FillRect { rect: Rect::new(5, 5, 10, 10), color: Color::RED })
            .unwrap();
        backend.end_batch();

        backend.replay(id);
        backend.end_frame();

        // Verify pixel data was written at center of fill region
        let rgba = backend.frame_rgba();
        let stride = 50 * 4;
        // Pixel at (10, 10) should be RED
        let idx = 10 * stride + 10 * 4;
        assert_eq!(rgba[idx], 255); // R
        assert_eq!(rgba[idx + 1], 0); // G
        assert_eq!(rgba[idx + 2], 0); // B
        assert_eq!(rgba[idx + 3], 255); // A
    }

    /// Replaying one batch must not copy every other batch or image on the backend.
    ///
    /// # What this pins (D09-RENDER-05)
    ///
    /// `SoftwarePaintBackend::replay` used to `clone()` the whole `BatchState`
    /// before replaying, so replaying a small batch deep-copied every unrelated
    /// batch command *and* every cached image. This test observes the cost through
    /// the test-only image-byte counter: it replays a small batch while the backend
    /// also holds two large, unrelated image batches. The bytes copied by the
    /// replay must come only from the target batch, and must not grow when the
    /// unrelated images grow.
    #[test]
    fn replaying_a_batch_does_not_copy_unrelated_batches_or_images() {
        fn replay_and_measure(unrelated_image_bytes: usize) -> usize {
            let size = crate::core::Size::new(64, 64);
            let mut backend = SoftwarePaintBackend::new(size, 1.0);
            backend.begin_frame(Color::WHITE);

            // The target batch: one small fill, no image at all.
            let target = backend.begin_batch();
            backend
                .record(BatchCommand::FillRect { rect: Rect::new(2, 2, 8, 8), color: Color::RED })
                .unwrap();
            backend.end_batch();

            // Two unrelated batches, each holding a cached image of the given size.
            for image_index in 0..2u64 {
                let object_id = 1000 + image_index;
                backend
                    .batch_state
                    .images
                    .insert(object_id, Arc::from(vec![7u8; unrelated_image_bytes]));
                let other = backend.begin_batch();
                backend
                    .record(BatchCommand::DrawImage {
                        rect: Rect::new(0, 0, 64, 64),
                        image_id: object_id,
                        opacity: 1.0,
                    })
                    .unwrap();
                backend.end_batch();
                let _ = other;
            }

            // Measure only the target replay.
            let _ = take_image_bytes_translated();
            backend.replay(target);
            take_image_bytes_translated()
        }

        let small = replay_and_measure(64 * 64 * 4);
        let large = replay_and_measure(512 * 64 * 4);

        assert_eq!(small, 0, "replaying a batch with no image must copy no image bytes");
        assert_eq!(
            large, 0,
            "unrelated images must not be copied when a different batch is replayed"
        );
    }

    /// The non-cloning replay must produce the same pixels as recording and drawing
    /// the commands directly — the optimisation must not change output. (D09-RENDER-05)
    #[test]
    fn replay_matches_direct_execution_including_opacity_and_translate() {
        let size = crate::core::Size::new(32, 32);

        // Reference: the same commands executed directly, in order.
        let mut reference = SoftwarePaintBackend::new(size, 1.0);
        reference.begin_frame(Color::BLACK);
        reference.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 10, 10),
            color: Color::rgba(255, 0, 0, 128),
        });
        reference.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(4, 4, 6, 6),
            color: Color::rgba(0, 255, 0, 128),
        });
        reference.end_frame();
        let expected = reference.frame_rgba().to_vec();

        // Replayed: the same geometry/colour produced through batch translation with a
        // `Translate` applied to make sure the transform path is exercised too.
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::BLACK);
        let id = backend.begin_batch();
        backend
            .record(BatchCommand::FillRect {
                rect: Rect::new(0, 0, 10, 10),
                color: Color::rgba(255, 0, 0, 128),
            })
            .unwrap();
        backend.record(BatchCommand::Translate { dx: 3.0, dy: 3.0 }).unwrap();
        backend
            .record(BatchCommand::FillRect {
                rect: Rect::new(1, 1, 6, 6),
                color: Color::rgba(0, 255, 0, 128),
            })
            .unwrap();
        backend.end_batch();
        backend.replay(id);
        backend.end_frame();

        assert_eq!(
            backend.frame_rgba(),
            expected.as_slice(),
            "replay must paint the same pixels as executing the translated commands directly"
        );
    }

    #[test]
    fn batch_renderer_contains_batch_after_creation() {
        let size = crate::core::Size::new(10, 10);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let id = backend.begin_batch();
        backend
            .record(BatchCommand::DrawLine {
                from: Point::new(0, 0),
                to: Point::new(10, 10),
                color: Color::RED,
                width: 1.0,
            })
            .unwrap();
        backend.end_batch();

        assert!(backend.contains_batch(id));
        assert_eq!(backend.batch_count(), 1);
    }

    /// The pseudo-3D matrix the item API records: turn 45° about `Y` around the centre of a
    /// 100×100 control, with the plane placed in front of the eye.
    #[cfg(feature = "3d")]
    fn item_turn_matrix() -> Mat4 {
        use crate::render::math3d::Axis;

        // The same viewing distance the item API documents.
        let distance = 1024.0f32;
        Mat4::translation(-50.0, -50.0, 0.0)
            .then(&Mat4::rotation(Axis::Y, core::f32::consts::FRAC_PI_4))
            .then(&Mat4::translation(0.0, 0.0, -distance))
            .then(&Mat4::perspective(
                core::f32::consts::FRAC_PI_2,
                1.0,
                distance * 0.5,
                distance * 4.0,
            ))
            .then(&Mat4::from_affine(Mat3::scaling(distance, distance)))
            .then(&Mat4::translation(50.0, 50.0, 0.0))
    }

    /// A projective push turns a fill into a divided quad and reaches a real pixel.
    #[cfg(feature = "3d")]
    #[test]
    fn a_projective_fill_reaches_a_pixel_and_keeps_its_perspective() {
        use crate::core::Size;

        let images = crate::compat::HashMap::new();
        let mut state = TransformState::default();
        assert!(transform_command(
            &BatchCommand::PushProjectiveTransform { matrix: item_turn_matrix() },
            &images,
            &mut state,
        )
        .is_none());
        let command = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(0, 0, 100, 100), color: Color::RED },
            &images,
            &mut state,
        )
        .expect("a projected fill must produce a command");
        let corners = match command {
            RenderCommand::FillQuadAA { corners, color } => {
                assert_eq!(color, Color::RED);
                corners
            }
            other => panic!("a perspective fill must use the antialiased quad, got {other:?}"),
        };
        assert!(
            transform_command(&BatchCommand::PopProjectiveTransform, &images, &mut state).is_none()
        );
        assert!(!state.is_projective(), "the pop restores the flat state");

        // The turned card's vertical edges differ in length; an affine rotation would keep them
        // equal, so this is the evidence that the divide really happened.
        let left_height = (corners[0].y - corners[3].y).abs();
        let right_height = (corners[1].y - corners[2].y).abs();
        assert_ne!(left_height, right_height, "the two vertical edges must differ");

        let mut backend = SoftwarePaintBackend::new(Size::new(120, 120), 1.0);
        backend.begin_frame(Color::WHITE);
        backend.execute_command(&RenderCommand::FillQuadAA { corners, color: Color::RED });
        backend.end_frame();
        let frame = backend.frame_rgba();
        assert!(
            frame.as_chunks::<4>().0.iter().any(|pixel| pixel[0] > 200 && pixel[1] < 40),
            "the projected quad must leave red ink on the surface"
        );
    }

    /// An identity projective push leaves the pixels exactly as the 2D path does.
    ///
    /// This is the "identity is free" half of the pseudo-3D contract: pushing an identity matrix
    /// must not change the command kind, the rounding, or the raster path, so a caller that always
    /// records a projective push still gets the historical 2D picture when the control is flat.
    #[cfg(feature = "3d")]
    #[test]
    fn an_identity_projective_push_paints_like_the_2d_path() {
        use crate::core::Size;

        let images = crate::compat::HashMap::new();
        let mut with_push = TransformState::default();
        assert!(transform_command(
            &BatchCommand::PushProjectiveTransform { matrix: Mat4::IDENTITY },
            &images,
            &mut with_push,
        )
        .is_none());
        let pushed = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(2, 3, 8, 6), color: Color::RED },
            &images,
            &mut with_push,
        )
        .expect("a fill under an identity push still draws");
        let mut plain = TransformState::default();
        let direct = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(2, 3, 8, 6), color: Color::RED },
            &images,
            &mut plain,
        )
        .expect("a fill without a push draws");
        assert!(
            matches!(&pushed, RenderCommand::FillRect { .. })
                && matches!(&direct, RenderCommand::FillRect { .. }),
            "identity must not change the emitted command kind"
        );

        let paint = |command: &RenderCommand| {
            let mut backend = SoftwarePaintBackend::new(Size::new(16, 16), 1.0);
            backend.begin_frame(Color::WHITE);
            backend.execute_command(command);
            backend.end_frame();
            backend.frame_rgba().to_vec()
        };
        assert_eq!(
            paint(&pushed),
            paint(&direct),
            "identity must produce the same pixels through either path"
        );
    }

    /// A quad that crosses the eye plane is dropped rather than mirrored.
    #[cfg(feature = "3d")]
    #[test]
    fn a_projective_fill_behind_the_eye_is_dropped() {
        let images = crate::compat::HashMap::new();
        let mut state = TransformState::default();
        let mut behind = Mat4::IDENTITY;
        behind.m[15] = -1.0;
        assert!(transform_command(
            &BatchCommand::PushProjectiveTransform { matrix: behind },
            &images,
            &mut state,
        )
        .is_none());
        assert!(
            transform_command(
                &BatchCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED },
                &images,
                &mut state,
            )
            .is_none(),
            "a quad with no projectable corner must draw nothing"
        );
    }

    /// The affine-only path still uses `FillQuad` and never takes the projective branch.
    #[cfg(feature = "3d")]
    #[test]
    fn an_affine_push_still_emits_a_hard_quad() {
        let images = crate::compat::HashMap::new();
        let mut state = TransformState::default();
        let rotation = Mat3::rotation(core::f32::consts::FRAC_PI_6);
        assert!(transform_command(
            &BatchCommand::PushTransform { matrix: rotation },
            &images,
            &mut state,
        )
        .is_none());
        let command = transform_command(
            &BatchCommand::FillRect { rect: Rect::new(0, 0, 10, 10), color: Color::RED },
            &images,
            &mut state,
        )
        .expect("an affine fill produces a command");
        assert!(
            matches!(command, RenderCommand::FillQuad { .. }),
            "the affine path stays on the exact quad, got {command:?}"
        );
    }
}
