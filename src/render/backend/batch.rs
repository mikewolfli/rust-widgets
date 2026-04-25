#![allow(dead_code)]

//! Batch rendering primitives.
//!
//! Provides types for organizing draw commands into batches that can be
//! recorded once and replayed efficiently by the renderer.

use std::collections::HashMap;

use crate::core::{Color, Font, ObjectId, Point, Rect};
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

/// A single draw command that can be recorded into a batch.
///
/// Each variant describes a primitive operation the renderer can replay.
#[derive(Debug, Clone)]
pub enum BatchCommand {
    /// Fill a rectangle with a solid colour.
    FillRect { rect: Rect, color: Color },
    /// Stroke a rectangular border.
    StrokeRect {
        rect: Rect,
        color: Color,
        width: f32,
    },
    /// Draw a line between two points.
    DrawLine {
        from: Point,
        to: Point,
        color: Color,
        width: f32,
    },
    /// Draw an image identified by its resource id.
    DrawImage {
        rect: Rect,
        image_id: ObjectId,
        opacity: f32,
    },
    /// Draw a clipped region of an image.
    DrawImageSubrect {
        dest: Rect,
        source: Rect,
        image_id: ObjectId,
        opacity: f32,
    },
    /// Draw text at the given position.
    DrawText {
        position: Point,
        text: String,
        color: Color,
        font_size: f32,
    },
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
/// ```ignore
/// fn render(batcher: &mut impl BatchRenderer) {
///     let batch_id = batcher.begin_batch();
///     batcher.record(BatchCommand::FillRect {
///         rect: Rect::new(0, 0, 100, 100),
///         color: Color::rgb(255, 0, 0),
///     });
///     batcher.end_batch();
///     batcher.replay(batch_id);
/// }
/// ```
pub trait BatchRenderer {
    /// Begin recording a new batch. Returns the batch id.
    fn begin_batch(&mut self) -> BatchId;

    /// Finish recording the current batch.
    fn end_batch(&mut self);

    /// Record a single command into the currently open batch.
    fn record(&mut self, cmd: BatchCommand);

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
        Self {
            next_id: 0,
            current_batch: None,
            batches: HashMap::new(),
            images: HashMap::new(),
        }
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
    /// # Panics
    ///
    /// Panics if there is no open batch (i.e. `begin_batch` has not been
    /// called, or `end_batch` has already been called).
    pub(crate) fn record(&mut self, cmd: BatchCommand) {
        let id = self
            .current_batch
            .expect("called record() without an open batch; call begin_batch() first");
        if let Some(cmds) = self.batches.get_mut(&id) {
            cmds.push(cmd);
        }
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
            BatchCommand::FillRect { rect, color } => RenderCommand::FillRect {
                rect: *rect,
                color: *color,
            },

            BatchCommand::StrokeRect { rect, color, width } => RenderCommand::DrawRectStroke {
                rect: *rect,
                color: *color,
                width: *width as u32,
            },

            BatchCommand::DrawLine {
                from,
                to,
                color,
                width,
            } => RenderCommand::DrawLineStroke {
                from: *from,
                to: *to,
                color: *color,
                width: *width as u32,
            },

            BatchCommand::DrawImage {
                rect,
                image_id,
                opacity: _opacity,
            } => {
                let data = images.get(image_id).cloned().unwrap_or_default();
                RenderCommand::DrawImage {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width as u32,
                    height: rect.height as u32,
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
                    width: dest.width as u32,
                    height: dest.height as u32,
                    data,
                }
            }

            BatchCommand::DrawText {
                position,
                text,
                color,
                font_size,
            } => {
                let font = Font::simple(BATCH_DEFAULT_FONT_FAMILY, *font_size);
                RenderCommand::DrawText {
                    origin: *position,
                    text: text.clone(),
                    font,
                    color: *color,
                }
            }

            BatchCommand::PushClip { rect } => RenderCommand::PushClip {
                x: rect.x,
                y: rect.y,
                width: rect.width as u32,
                height: rect.height as u32,
            },

            BatchCommand::PopClip => RenderCommand::PopClip,

            // Translate / SetOpacity have no direct RenderCommand equivalent
            // in the current command set. They are skipped during replay.
            // Backends that need these semantics should implement them at a
            // higher layer (e.g. transform stack in the scene).
            BatchCommand::Translate { .. } | BatchCommand::SetOpacity { .. } => {
                // Emit a no-op placeholder that does nothing.
                RenderCommand::FillRect {
                    rect: Rect::new(0, 0, 0, 0),
                    color: Color::TRANSPARENT,
                }
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

    fn record(&mut self, cmd: BatchCommand) {
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
