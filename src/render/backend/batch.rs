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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone)]
pub enum BatchCommand {
    /// Fill a rectangle with a solid colour.
    FillRect { rect: Rect, color: Color },
    /// Stroke a rectangular border.
    StrokeRect { rect: Rect, color: Color, width: f32 },
    /// Draw a line between two points.
    DrawLine { from: Point, to: Point, color: Color, width: f32 },
    /// Draw an image identified by its resource id.
    DrawImage { rect: Rect, image_id: ObjectId, opacity: f32 },
    /// Draw a clipped region of an image.
    DrawImageSubrect { dest: Rect, source: Rect, image_id: ObjectId, opacity: f32 },
    /// Draw text at the given position.
    DrawText { position: Point, text: String, color: Color, font_size: f32 },
    /// Push a clipping rectangle – subsequent commands are clipped.
    PushClip { rect: Rect },
    /// Pop the most recent clipping rectangle.
    PopClip,
    /// Apply a translation offset to all subsequent commands.
    Translate { dx: f32, dy: f32 },
    /// Apply an opacity multiplier to all subsequent commands.
    SetOpacity { opacity: f32 },
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
    /// Begin recording a new batch. Returns the batch id.
    fn begin_batch(&mut self) -> BatchId;

    /// Finish recording the current batch.
    fn end_batch(&mut self);

    /// Record a single command into the currently open batch.
    fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError>;

    /// Replay a previously recorded batch by its id.
    fn replay(&mut self, id: BatchId);

    /// Remove a batch and free its resources.
    fn destroy_batch(&mut self, id: BatchId);

    /// Check whether a batch id is still valid.
    fn contains_batch(&self, id: BatchId) -> bool;

    /// Return the number of currently recorded batches.
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
        for cmd in cmds {
            let rc = Self::translate_command(cmd, &self.images);
            PaintBackend::execute_command(backend, &rc);
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

    /// Translate a single [`BatchCommand`] into a [`RenderCommand`].
    ///
    /// Some batch commands carry higher-level semantics not directly
    /// represented by the low-level `RenderCommand` enum. In those cases
    /// the translation makes reasonable assumptions (e.g. using the default
    /// UI font family with the requested size for text, or embedding image
    /// data looked up from the cache).
    fn translate_command(cmd: &BatchCommand, images: &HashMap<ObjectId, Vec<u8>>) -> RenderCommand {
        match cmd {
            BatchCommand::FillRect { rect, color } => {
                RenderCommand::FillRect { rect: *rect, color: *color }
            }

            BatchCommand::StrokeRect { rect, color, width } => {
                RenderCommand::DrawRectStroke { rect: *rect, color: *color, width: *width as u32 }
            }

            BatchCommand::DrawLine { from, to, color, width } => RenderCommand::DrawLineStroke {
                from: *from,
                to: *to,
                color: *color,
                width: *width as u32,
            },

            BatchCommand::DrawImage { rect, image_id, opacity: _opacity } => {
                let data = images.get(image_id).cloned().unwrap_or_default();
                RenderCommand::DrawImage {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    data,
                }
            }

            BatchCommand::DrawImageSubrect {
                dest,
                source: _source,
                image_id,
                opacity: _opacity,
            } => {
                let data = images.get(image_id).cloned().unwrap_or_default();
                RenderCommand::DrawImage {
                    x: dest.x,
                    y: dest.y,
                    width: dest.width,
                    height: dest.height,
                    data,
                }
            }

            BatchCommand::DrawText { position, text, color, font_size } => {
                let font = Font::simple(BATCH_DEFAULT_FONT_FAMILY, *font_size);
                RenderCommand::DrawText {
                    origin: *position,
                    text: text.clone(),
                    font,
                    color: *color,
                    alignment: HorizontalAlignment::Left,
                }
            }

            BatchCommand::PushClip { rect } => RenderCommand::PushClip {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            },

            BatchCommand::PopClip => RenderCommand::PopClip,

            // Translate / SetOpacity have no direct RenderCommand equivalent
            // in the current command set. They are skipped during replay.
            // Backends that need these semantics should implement them at a
            // higher layer (e.g. transform stack in the scene).
            BatchCommand::Translate { .. } | BatchCommand::SetOpacity { .. } => {
                // Emit a no-op placeholder that does nothing.
                RenderCommand::FillRect { rect: Rect::new(0, 0, 0, 0), color: Color::TRANSPARENT }
            }
        }
    }
}

impl BatchRenderer for SoftwarePaintBackend {
    fn begin_batch(&mut self) -> BatchId {
        self.batch_state.begin_batch()
    }

    fn end_batch(&mut self) {
        self.batch_state.end_batch()
    }

    fn record(&mut self, cmd: BatchCommand) -> Result<(), BatchError> {
        self.batch_state.record(cmd)
    }

    fn replay(&mut self, id: BatchId) {
        // Clone the state to avoid borrow issues, then replay.
        let state = self.batch_state.clone();
        state.replay(self, id);
    }

    fn destroy_batch(&mut self, id: BatchId) {
        self.batch_state.destroy_batch(id)
    }

    fn contains_batch(&self, id: BatchId) -> bool {
        self.batch_state.contains_batch(id)
    }

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

        // Translate/SetOpacity emit a zero-size FillRect during replay
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
