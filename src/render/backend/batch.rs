#![allow(dead_code)]

//! Batch rendering primitives.
//!
//! Provides types for organizing draw commands into batches that can be
//! recorded once and replayed efficiently by the renderer.

use crate::core::{Color, ObjectId, Point, Rect};

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
