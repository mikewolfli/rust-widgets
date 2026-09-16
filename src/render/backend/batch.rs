// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Batch rendering primitives.
//!
//! Provides types for organizing draw commands into batches that can be
//! recorded once and replayed efficiently by the renderer.

use crate::compat::HashMap;

use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::render::RenderCommand;

use super::paint::{PaintBackend, SoftwarePaintBackend};

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

impl std::fmt::Display for BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchError::NoActiveBatch => {
                write!(f, "called record() without an open batch; call begin_batch() first")
            }
        }
    }
}

impl std::error::Error for BatchError {}

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
    /// # Deprecated behaviour
    ///
    /// The current replay implementation drops this command and logs a warning,
    /// because the image cache stores raw pixel bytes without the source
    /// dimensions needed to compute the source rectangle. Record it only if a
    /// future backend is expected to support cropping.
    DrawImageSubrect {
        /// Destination rectangle on the surface.
        dest: Rect,
        /// Sub-rectangle of the source image to draw.
        source: Rect,
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
        /// Text to draw.
        text: String,
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
    /// call fails with [`BatchError::NoActiveBatch`]. Calling it with no open
    /// batch is a no-op.
    fn end_batch(&mut self);

    /// Appends one command to the currently open batch.
    ///
    /// Commands are stored verbatim and in order; no drawing happens until
    /// [`BatchRenderer::replay`] is called.
    ///
    /// # Errors
    ///
    /// Returns [`BatchError::NoActiveBatch`] when there is no open batch, i.e.
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
    pub(crate) images: HashMap<ObjectId, Vec<u8>>,
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

    /// Replay a previously recorded batch by its id.
    ///
    /// Iterates over the stored [`BatchCommand`]s, translates each one to
    /// the corresponding [`RenderCommand`], and calls `execute_command` on
    /// the provided backend.
    pub(crate) fn replay(&self, backend: &mut SoftwarePaintBackend, id: BatchId) {
        let Some(cmds) = self.batches.get(&id) else {
            return;
        };
        let mut state = TransformState::default();
        for cmd in cmds {
            if let Some(rc) = Self::translate_command(cmd, &self.images, &mut state) {
                PaintBackend::execute_command(backend, &rc);
            }
        }
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

/// Accumulated transform state that tracks translation and opacity
/// across consecutive [`BatchCommand`]s.
#[derive(Debug, Clone, Copy)]
struct TransformState {
    /// Accumulated X offset.
    dx: f32,
    /// Accumulated Y offset.
    dy: f32,
    /// Accumulated opacity multiplier (1.0 = fully opaque).
    opacity: f32,
}

impl TransformState {
    fn apply_to_rect(&self, rect: &Rect) -> Rect {
        Rect::new(
            (rect.x as f32 + self.dx) as i32,
            (rect.y as f32 + self.dy) as i32,
            rect.width,
            rect.height,
        )
    }

    fn apply_to_point(&self, pt: &Point) -> Point {
        Point::new((pt.x as f32 + self.dx) as i32, (pt.y as f32 + self.dy) as i32)
    }

    fn apply_to_color(&self, color: &Color) -> Color {
        Color { r: color.r, g: color.g, b: color.b, a: (color.a as f32 * self.opacity) as u8 }
    }
}

impl Default for TransformState {
    fn default() -> Self {
        Self { dx: 0.0, dy: 0.0, opacity: 1.0 }
    }
}

impl BatchState {
    /// Translate a single [`BatchCommand`] into a [`RenderCommand`].
    ///
    /// Some batch commands carry higher-level semantics not directly
    /// represented by the low-level `RenderCommand` enum. In those cases
    /// the translation makes reasonable assumptions (e.g. using the default
    /// UI font family with the requested size for text, or embedding image
    /// data looked up from the cache).
    ///
    /// Returns `None` for commands that only update the transform state
    /// (e.g. [`BatchCommand::Translate`], [`BatchCommand::SetOpacity`]).
    fn translate_command(
        cmd: &BatchCommand,
        images: &HashMap<ObjectId, Vec<u8>>,
        state: &mut TransformState,
    ) -> Option<RenderCommand> {
        match cmd {
            BatchCommand::FillRect { rect, color } => Some(RenderCommand::FillRect {
                rect: state.apply_to_rect(rect),
                color: state.apply_to_color(color),
            }),

            BatchCommand::StrokeRect { rect, color, width } => {
                Some(RenderCommand::DrawRectStroke {
                    rect: state.apply_to_rect(rect),
                    color: state.apply_to_color(color),
                    width: *width as u32,
                })
            }

            BatchCommand::DrawLine { from, to, color, width } => {
                Some(RenderCommand::DrawLineStroke {
                    from: state.apply_to_point(from),
                    to: state.apply_to_point(to),
                    color: state.apply_to_color(color),
                    width: *width as u32,
                })
            }

            BatchCommand::DrawImage { rect, image_id, opacity } => {
                let mut data = images.get(image_id).cloned().unwrap_or_default();
                if data.is_empty() {
                    log::warn!(
                        "[batch] DrawImage references unknown image id {image_id}; dropping"
                    );
                    return None;
                }
                let applied_rect = state.apply_to_rect(rect);
                let combined_opacity = state.opacity * opacity;
                // RenderCommand::DrawImage has no alpha channel, so a partial
                // opacity is baked into the image's alpha bytes (RGBA).
                if combined_opacity < 1.0 && data.len() % 4 == 0 {
                    for px in data.chunks_exact_mut(4) {
                        px[3] = (px[3] as f32 * combined_opacity).round().clamp(0.0, 255.0) as u8;
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

            BatchCommand::DrawImageSubrect { image_id, .. } => {
                // Source-region cropping needs the source image's dimensions,
                // which the image cache (ObjectId -> raw RGBA bytes) does not
                // carry. Emitting a stretched full-image would silently render
                // the wrong pixels, so drop the command and report it instead.
                log::warn!(
                    "[batch] DrawImageSubrect (image={image_id}) dropped: source-crop requires image dimensions metadata that is not stored"
                );
                None
            }

            BatchCommand::DrawText { position, text, color, font_size } => {
                let font = Font::simple(BATCH_DEFAULT_FONT_FAMILY, *font_size);
                Some(RenderCommand::DrawText {
                    origin: state.apply_to_point(position),
                    text: text.clone(),
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
                state.opacity *= opacity;
                None
            }
        }
    }
}

impl BatchRenderer for SoftwarePaintBackend {
    /// Opens a new batch in this backend's [`BatchState`].
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
    /// Returns [`BatchError::NoActiveBatch`] when no batch is open.
    fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError> {
        self.batch_state.record(cmd)
    }

    /// Replays the batch through this backend's own paint path.
    ///
    /// The batch state is cloned first so the immutable borrow taken by the
    /// lookup does not conflict with the mutable borrow required to paint;
    /// this makes replay O(number of commands) in allocations and means edits
    /// to the backend made during replay do not affect the commands remaining
    /// to be drawn.
    fn replay(&mut self, id: BatchId) {
        // Clone the state to avoid borrow issues, then replay.
        let state = self.batch_state.clone();
        state.replay(self, id);
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
    use crate::core::{Color, Point, Rect};

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
            image_id: 2u64,
            opacity: 0.5,
        };
        match cmd {
            BatchCommand::DrawImageSubrect { dest, source, image_id, opacity } => {
                assert_eq!(dest, Rect::new(10, 10, 64, 64));
                assert_eq!(source, Rect::new(0, 0, 32, 32));
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
            text: "Hello".to_string(),
            color: Color::WHITE,
            font_size: 16.0,
        };
        match cmd {
            BatchCommand::DrawText { position, text, color, font_size } => {
                assert_eq!(position, Point::new(15, 30));
                assert_eq!(text, "Hello");
                assert_eq!(color, Color::WHITE);
                assert!((font_size - 16.0).abs() < 1e-6);
            }
            _ => panic!("expected DrawText variant"),
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
        let state = BatchState::new();
        // Should not panic
        let size = crate::core::Size::new(1, 1);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        state.replay(&mut backend, BatchId::new(999));
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
}
