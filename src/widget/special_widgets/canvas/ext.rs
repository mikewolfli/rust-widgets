// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas-only support types and the additive seams layered on top of [`Canvas`].
//!
//! The canvas crossed the 2000-line production limit, so its type declarations and the
//! additive C-11'/RW-29..RW-33 seams live here. This module is a child of `canvas`, which is
//! what lets it read the canvas's private state without widening any field to the rest of the
//! crate. It defines **only** canvas-related items: no other widget and no shared
//! infrastructure belongs here.

use crate::compat::{Arc, String, Vec};
use crate::core::{Color, Point, Rect};
use crate::event::dnd::{DragPayload, DropEffect, DropTarget};
use crate::platform::CursorIcon;
use crate::render::{RenderCommand, RenderContext};
use crate::signal::Signal1;
use crate::widget::save_state::SaveState;
use alloc::collections::VecDeque;

use super::command::{command_bounds, rects_intersect, snap_command, transform_command};
use super::{next_layer_cache_key, Canvas, COMMAND_CHUNK_SIZE};

/// A uniform scale and translation applied to every drawing command a [`Canvas`] stores.
///
/// Commands are stored in the **logical** coordinates the caller drew them in, and the transform
/// is applied when the canvas is painted, exported or hit-tested. That is what makes a wheel zoom
/// affect the ink already on the surface: applying it at push time froze every earlier command in
/// the old scale, so `zoom_changed` reported a zoom the picture never showed.
///
/// The replay transform of each command is cached, so the scale-and-translate is computed once per
/// command per transform value rather than once per command per frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasTransform {
    /// Uniform scale, clamped to a positive value.
    pub scale: f32,
    /// Translation in surface pixels.
    pub offset: Point,
}

impl Default for CanvasTransform {
    fn default() -> Self {
        Self { scale: 1.0, offset: Point::new(0, 0) }
    }
}

impl CanvasTransform {
    /// A transform with `scale` and no translation.
    pub fn scaled(scale: f32) -> Self {
        Self { scale: if scale.is_finite() { scale.max(0.01) } else { 1.0 }, ..Self::default() }
    }

    /// Maps one point through the transform.
    pub fn point(&self, point: Point) -> Point {
        Point::new(
            self.offset.x + (point.x as f32 * self.scale).round() as i32,
            self.offset.y + (point.y as f32 * self.scale).round() as i32,
        )
    }

    /// Maps one rectangle through the transform.
    pub fn rect(&self, rect: Rect) -> Rect {
        let origin = self.point(Point::new(rect.x, rect.y));
        Rect::new(
            origin.x,
            origin.y,
            (rect.width as f32 * self.scale).round().max(1.0) as u32,
            (rect.height as f32 * self.scale).round().max(1.0) as u32,
        )
    }

    /// Maps one **surface** point back into the logical space commands are stored in.
    ///
    /// The hit test reads this so it can compare a pointer against the commands exactly as the
    /// caller drew them, instead of walking the transformed copies the paint path builds.
    pub fn inverse_point(&self, point: Point) -> Point {
        let scale = if self.scale.is_finite() && self.scale > 0.0 { self.scale } else { 1.0 };
        Point::new(
            ((point.x - self.offset.x) as f32 / scale).round() as i32,
            ((point.y - self.offset.y) as f32 / scale).round() as i32,
        )
    }

    /// The transform that maps surface coordinates back to logical coordinates.
    ///
    /// Used by model-space-agnostic snapping: a screen-space grid is applied to the mapped
    /// command and then brought back through this transform. The result is a close inverse of
    /// [`Self::point`] rather than an exact one, because the forward mapping rounds.
    pub fn inverse(&self) -> Self {
        let scale = if self.scale.is_finite() && self.scale > 0.0 { 1.0 / self.scale } else { 1.0 };
        Self {
            scale,
            offset: Point::new(
                (-(self.offset.x as f32) * scale).round() as i32,
                (-(self.offset.y as f32) * scale).round() as i32,
            ),
        }
    }
}

/// A host-assigned identity attached to a group of commands.
///
/// The tag is opaque to the canvas: it is whatever identifier the host uses for the node the
/// group draws (a node id, a selection id, a scene-graph key). It is carried by every command
/// pushed inside [`Canvas::begin_group_with_tag`] until [`Canvas::end_group`], survives redo,
/// and is what [`Canvas::hit_test_tagged`] answers with.
pub type CanvasTag = u64;

/// The coordinate space a canvas grid size is interpreted in (RW-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridSpace {
    /// The step is measured in surface pixels; snapping follows the current zoom and pan.
    #[default]
    Screen,
    /// The step is measured in the stored commands' model coordinates.
    Model,
}

/// Which axis a guide line marks (RW-32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideAxis {
    /// A horizontal line at a model-space `y` position.
    Horizontal,
    /// A vertical line at a model-space `x` position.
    Vertical,
}

/// One accepted drop, delivered to the host (C-11').
///
/// The payload is the runtime's untouched [`DragPayload`], so the host can read the same
/// `type_id`/`item_id`/`label` the drag source published. Both coordinate forms are provided:
/// `position` is the pointer in the canvas's surface coordinates, and `model` is the same point
/// mapped through the canvas transform into the coordinates stored commands use.
#[derive(Debug, Clone)]
pub struct CanvasDropEvent {
    /// The payload the drag carried.
    pub payload: DragPayload,
    /// The drop point in the canvas's surface coordinates.
    pub position: Point,
    /// The drop point mapped through the canvas transform, in stored-command coordinates.
    pub model: Point,
}

/// A host sink that decides what an accepted drop means for the source.
///
/// The callback runs once per accepted drop. Its return value is reported to the drag source, so
/// [`DropEffect::Move`] lets a library panel remove its original entry while
/// [`DropEffect::Copy`] keeps it.
pub type CanvasDropHandler = Arc<dyn Fn(&CanvasDropEvent) -> DropEffect + Send + Sync>;

/// One stored drawing command, tagged with the order it was added in.
///
/// The sequence number is what lets undo work across layers: the *globally*
/// newest command can be found without assuming the last command of the last
/// layer, because layers are reordered independently of insertion order.
pub(super) struct StoredCommand {
    pub(super) seq: u64,
    /// The command in the caller's logical coordinates, which is the geometry hit tests,
    /// exports and the undo history all address.
    pub(super) command: RenderCommand,
    /// The same command mapped through a transform, together with the transform it was built
    /// from. `None` until the first paint, and rebuilt whenever the canvas transform changes;
    /// see [`CanvasTransform`] for why the mapping does not happen at push time.
    pub(super) replay: Option<(CanvasTransform, RenderCommand)>,
    /// The host tag of the group this command was pushed inside, if any.
    pub(super) tag: Option<CanvasTag>,
    /// Whether this command belongs to the decoration/overlay layer. Overlay commands are kept
    /// out of undo, hit testing and both export paths; the flag is read defensively so the
    /// exclusion cannot be lost if the storage is shared later.
    pub(super) overlay: bool,
}

impl StoredCommand {
    /// The command as it should be replayed under `transform`, materialising and caching the
    /// mapping when the cached one was built for a different transform.
    pub(super) fn replay_under(&mut self, transform: CanvasTransform) -> &RenderCommand {
        let fresh = matches!(&self.replay, Some((cached, _)) if *cached == transform);
        if !fresh {
            let mapped = transform_command(&self.command, transform);
            self.replay = Some((transform, mapped));
        }
        match self.replay.as_ref() {
            Some((_, command)) => command,
            // Unreachable: the block above always stores a pair. Answering with the logical
            // command keeps a corrupt cache from taking the paint down.
            None => &self.command,
        }
    }
}

/// A contiguous block of a layer's commands and their combined logical bounds.
///
/// `bounds` is `None` when any command in the block has no cullable bounds, which means the
/// block must always be replayed (SP-110).
pub(super) struct CommandChunk {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) bounds: Option<Rect>,
}

/// One canvas layer: a named, independently orderable and hideable command list.
///
/// A `VecDeque` rather than a `Vec` because the command limit evicts from the
/// front on every push once the cap is reached; `Vec::remove(0)` shifted the whole
/// layer on each of those pushes.
pub(super) struct CanvasLayer {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) visible: bool,
    pub(super) commands: VecDeque<StoredCommand>,
    /// Identity for the render layer cache. A new key is handed out whenever the layer's own
    /// content or the canvas transform changes, which is the assertion the cache relies on.
    pub(super) cache_key: u64,
    /// Bumped whenever a command is added, removed or rewritten; keys the chunk index.
    pub(super) revision: u64,
    /// The chunk index, rebuilt lazily when `revision` moves (SP-110).
    pub(super) chunks: core::cell::RefCell<Option<(u64, Vec<CommandChunk>)>>,
}

impl CanvasLayer {
    /// Gives the layer a fresh render-cache identity and invalidates its chunk index.
    pub(super) fn touch(&mut self) {
        self.cache_key = next_layer_cache_key();
        self.revision = self.revision.wrapping_add(1);
    }

    /// Rebuilds the chunk bounds when the command list has changed.
    pub(super) fn ensure_chunks(&self) {
        if self.chunks.borrow().as_ref().is_some_and(|(revision, _)| *revision == self.revision) {
            return;
        }
        let mut chunks = Vec::new();
        let mut start = 0usize;
        let mut bounds: Option<Rect> = None;
        let mut uncullable = false;
        for (index, stored) in self.commands.iter().enumerate() {
            match command_bounds(&stored.command) {
                Some(command) => {
                    bounds = Some(match bounds {
                        Some(current) => current.union(&command),
                        None => command,
                    });
                }
                None => uncullable = true,
            }
            if (index + 1) % COMMAND_CHUNK_SIZE == 0 {
                chunks.push(CommandChunk {
                    start,
                    end: index + 1,
                    bounds: if uncullable { None } else { bounds },
                });
                start = index + 1;
                bounds = None;
                uncullable = false;
            }
        }
        if start < self.commands.len() {
            chunks.push(CommandChunk {
                start,
                end: self.commands.len(),
                bounds: if uncullable { None } else { bounds },
            });
        }
        *self.chunks.borrow_mut() = Some((self.revision, chunks));
    }
}

/// A command removed by undo, retained so redo can put it back.
pub(super) struct UndoneCommand {
    pub(super) layer_id: u64,
    pub(super) command: RenderCommand,
    pub(super) tag: Option<CanvasTag>,
    pub(super) overlay: bool,
}

/// A bitmap a canvas can place as one undoable, layer-owned command (SP-51).
///
/// Pixels are RGBA8 in row-major order and shared, so the same decoded image can be placed
/// several times — and replayed every frame — without a copy. `width`/`height` describe the
/// source buffer; the destination rectangle is the one passed to [`Canvas::draw_image`].
#[derive(Clone, Debug)]
pub struct CanvasImage {
    /// Source pixels, `width * height * 4` bytes.
    pub data: Arc<[u8]>,
    /// Source width in pixels.
    pub width: u32,
    /// Source height in pixels.
    pub height: u32,
}

impl CanvasImage {
    /// Wraps RGBA8 `pixels` of `width`×`height`, or `None` when the buffer is too short.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = width as usize * height as usize * 4;
        (width > 0 && height > 0 && pixels.len() >= expected).then(|| Self {
            data: Arc::from(pixels),
            width,
            height,
        })
    }
}

/// One row of the hit-test index: the flattened command index, its logical bounds and the tag
/// of the group that drew it.
pub(super) type CanvasHitEntry = (usize, Rect, Option<CanvasTag>);

/// The hit-test index: the command-set revision it was built for, and one row per visible
/// command.
///
/// Named for the same reason as `ChildPlacements`: the tuple is written twice (the field and the
/// rebuild) and a bare `(u64, Vec<(usize, Rect, Option<CanvasTag>)>)` is a
/// `clippy::type_complexity` finding.
pub(super) type CanvasHitCache = (u64, Vec<CanvasHitEntry>);

/// The thickness of a ruler strip, in surface pixels.
const RULER_THICKNESS: i32 = 18;

/// The minimum distance between ruler ticks, so a one-pixel grid cannot spin the tick loop.
const RULER_TICK_MIN_SPACING: i32 = 8;

/// Version tag of the blob produced by [`Canvas::save_state`].
///
/// Version 2 added the ruler unit (RW-124). A blob written by version 1 has no byte for it and is
/// refused rather than half-read, because guessing "grid step" for a document that chose another
/// unit would change the ticks the host draws.
const CANVAS_STATE_VERSION: u8 = 2;

impl SaveState for Canvas {
    fn state_version(&self) -> u16 {
        CANVAS_STATE_VERSION as u16
    }

    fn save_state(&self) -> Vec<u8> {
        Canvas::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        Canvas::restore_state(self, state)
    }
}

impl DropTarget for Canvas {
    /// A canvas accepts a drop only when the host installed a seam: a drop handler or at least
    /// one `drop_received` subscriber. Without either, refusing keeps a canvas that happens to
    /// sit under a drag from silently swallowing the payload.
    fn can_accept(&self, _payload: &DragPayload) -> bool {
        self.drop_enabled()
    }

    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect {
        if !self.can_accept(payload) {
            return DropEffect::None;
        }
        let event = CanvasDropEvent {
            payload: payload.clone(),
            position: pos,
            // The stored commands are in the control's own space, which is what `hit_test` maps a
            // window point into as well; mapping the raw window point put every drop off by the
            // canvas's own origin.
            model: self.to_drawing_space(pos),
        };
        // A handler owns the effect decision; without one the drop is a non-destructive copy.
        let effect = match self.drop_handler.as_ref() {
            Some(handler) => handler(&event),
            None => DropEffect::Copy,
        };
        self.drop_received.emit(event);
        self.base.request_redraw();
        effect
    }

    /// The whole canvas is the landing zone; where inside it the item lands is the host's
    /// decision, so the preview is the surface rectangle rather than an invented cell.
    fn preview_rect(&self, payload: &DragPayload, _pos: Point) -> Option<Rect> {
        self.can_accept(payload).then(|| self.base.geometry())
    }
}

/// One ruler tick in surface pixels with the model coordinate its label names (RW-124).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasRulerTick {
    /// Which ruler the tick belongs to: `Vertical` is the top strip (a vertical tick line),
    /// `Horizontal` is the left strip.
    pub axis: GuideAxis,
    /// Where the tick lands in the control's own pixels.
    pub surface: i32,
    /// The design coordinate the label should name.
    pub model: i32,
    /// `model` rendered as text, so a host need not format it itself.
    pub label: String,
}

/// Pushes model-space ticks that fall inside the visible range.
///
/// `step` is a positive model distance. Positions are mapped through `to_surface`; a tick whose
/// mapped pixel repeats the previous one is skipped, which keeps a low zoom from emitting dozens
/// of labels on one pixel.
fn push_model_ticks(
    out: &mut Vec<(i32, i32)>,
    first_model: i32,
    last_model: i32,
    step: i32,
    to_surface: impl Fn(i32) -> i32,
) {
    if step <= 0 {
        return;
    }
    let (min, max) = if first_model <= last_model {
        (first_model, last_model)
    } else {
        (last_model, first_model)
    };
    let mut model = min.div_euclid(step) * step;
    let mut previous_surface: Option<i32> = None;
    while model <= max {
        let surface = to_surface(model);
        if previous_surface != Some(surface) {
            out.push((surface, model));
            previous_surface = Some(surface);
        }
        model = match model.checked_add(step) {
            Some(next) => next,
            None => break,
        };
    }
}

impl Canvas {
    /// Installs (or clears) the host's drop sink.
    ///
    /// Installing a handler also opts the canvas into the runtime's drag coordinator: with a
    /// handler or a `drop_received` subscriber present, [`crate::event::dnd::DropTarget::can_accept`]
    /// answers `true` and a drop is delivered. The handler's return value is reported to the
    /// drag source; without one the drop is reported as a copy.
    pub fn set_drop_handler(&mut self, handler: Option<CanvasDropHandler>) {
        self.drop_handler = handler;
    }

    /// Returns whether a drop would be accepted right now.
    pub fn drop_enabled(&self) -> bool {
        self.base.is_enabled()
            && (self.drop_handler.is_some() || self.drop_received.slot_count() > 0)
    }

    /// The signal that carries every accepted drop (C-11').
    ///
    /// The payload is [`CanvasDropEvent`], which names the dragged payload and both the surface
    /// and model coordinates of the drop. It is an accessor rather than a public field because the
    /// canvas capability's published-event table is owned elsewhere and this seam is additive.
    pub fn drop_received_signal(&self) -> &Signal1<CanvasDropEvent> {
        &self.drop_received
    }

    /// The signal that carries `(flattened command index, tag)` for a tagged pick (RW-29).
    ///
    /// The plain [`Canvas::command_picked`] index still fires for every command, tagged or not;
    /// this accessor is the identity-aware counterpart.
    pub fn command_picked_tagged_signal(&self) -> &Signal1<(usize, CanvasTag)> {
        &self.command_picked_tagged
    }

    /// Opens the decoration/overlay layer (RW-30).
    ///
    /// Commands pushed until [`Self::end_overlay_layer`] are painted on top of every business
    /// layer but do not join the undo history, the hit-test index, or either export. The layer is
    /// persistent: `begin_overlay_layer` does not clear it, so a host that rebuilds decorations
    /// every frame should call [`Self::clear_overlay`] first.
    ///
    /// Returns `false` when the overlay is already open (sessions do not nest).
    pub fn begin_overlay_layer(&mut self) -> bool {
        if self.overlay_open {
            return false;
        }
        self.overlay_open = true;
        true
    }

    /// Closes the overlay layer, returning whether a session was open.
    pub fn end_overlay_layer(&mut self) -> bool {
        if !self.overlay_open {
            return false;
        }
        self.overlay_open = false;
        true
    }

    /// Returns whether commands are currently routed to the overlay layer.
    pub fn is_overlay_open(&self) -> bool {
        self.overlay_open
    }

    /// Returns how many decoration commands the overlay layer holds.
    pub fn overlay_command_count(&self) -> usize {
        self.overlay_commands.len()
    }

    /// Discards every decoration command while keeping the overlay session state.
    pub fn clear_overlay(&mut self) {
        if self.overlay_commands.is_empty() {
            return;
        }
        self.overlay_commands.clear();
        self.base.request_redraw();
    }

    /// Returns the coordinate space snapping uses (RW-31).
    pub fn grid_space(&self) -> GridSpace {
        self.grid_space
    }

    /// Selects the coordinate space snapping uses. The default is [`GridSpace::Screen`].
    ///
    /// The space also decides where the painted grid and ruler ticks sit, so a change has to
    /// request a repaint rather than wait for some later event to happen to cause one.
    pub fn set_grid_space(&mut self, space: GridSpace) {
        if self.grid_space == space {
            return;
        }
        self.grid_space = space;
        self.base.request_redraw();
    }

    /// Applies the canvas grid to `command` in the selected space.
    ///
    /// Screen-space snapping maps the command to surface pixels, snaps there and maps it back,
    /// so the step means the same distance at every zoom level. Model-space snapping rounds the
    /// stored coordinates directly, so a grid step of 10 is 10 model units regardless of zoom.
    pub(super) fn snap_command_in_space(&self, command: RenderCommand) -> RenderCommand {
        let step = self.grid_step.max(1) as i32;
        match self.grid_space {
            GridSpace::Model => snap_command(command, step),
            GridSpace::Screen => {
                let screen = transform_command(&command, self.transform);
                let snapped = snap_command(screen, step);
                transform_command(&snapped, self.transform.inverse())
            }
        }
    }

    /// Returns whether the ruler strips are visible (RW-32).
    pub fn rulers_visible(&self) -> bool {
        self.rulers_visible
    }

    /// The model-unit spacing between ruler ticks (RW-124).
    ///
    /// `0` means "follow the grid step", which is the pre-existing behaviour.
    pub fn ruler_unit(&self) -> u32 {
        self.ruler_unit
    }

    /// Sets the model-unit spacing between ruler ticks (RW-124).
    ///
    /// With [`GridSpace::Model`] this is the distance between labels in the design's own units, so
    /// a 16-unit grid can carry a 100-unit ruler without the host re-deriving tick positions.
    /// `0` restores the grid-step default. The redraw is requested only when the value changes.
    pub fn set_ruler_unit(&mut self, unit: u32) {
        if self.ruler_unit == unit {
            return;
        }
        self.ruler_unit = unit;
        self.base.request_redraw();
    }

    /// The ruler ticks for the current geometry, in surface coordinates with model labels (RW-124).
    ///
    /// A host that draws its own ruler labels calls this instead of re-deriving the spacing: the
    /// returned `surface` is where the tick lands in the control's own pixels, and `model` is the
    /// design coordinate the label should name. Ticks are returned for both axes, top (`Vertical`
    /// guide axis, a vertical tick line) and left (`Horizontal`).
    ///
    /// In [`GridSpace::Screen`] the unit is a surface distance and the label is the model
    /// coordinate the transform maps that pixel back to. In [`GridSpace::Model`] the unit is a
    /// model distance, so the ticks stay on the same design coordinates at every zoom.
    pub fn ruler_ticks(&self) -> Vec<CanvasRulerTick> {
        let rect = self.base.geometry();
        let unit = self.effective_ruler_unit();
        let (xs, ys) = self.axis_ticks(rect, unit);
        let mut ticks = Vec::with_capacity(xs.len() + ys.len());
        for (surface, model) in xs {
            ticks.push(CanvasRulerTick {
                axis: GuideAxis::Vertical,
                surface,
                model,
                label: crate::compat::format!("{model}"),
            });
        }
        for (surface, model) in ys {
            ticks.push(CanvasRulerTick {
                axis: GuideAxis::Horizontal,
                surface,
                model,
                label: crate::compat::format!("{model}"),
            });
        }
        ticks
    }

    /// The unit a ruler tick interval uses, in the space [`Self::grid_space`] names.
    fn effective_ruler_unit(&self) -> i32 {
        let unit = if self.ruler_unit == 0 { self.grid_step } else { self.ruler_unit };
        unit.max(1) as i32
    }

    /// The surface positions and model labels on both axes.
    fn axis_ticks(&self, rect: Rect, unit: i32) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        if rect.width == 0 || rect.height == 0 || unit <= 0 {
            return (xs, ys);
        }
        match self.grid_space {
            GridSpace::Model => {
                let scale = if self.transform.scale.is_finite() && self.transform.scale > 0.0 {
                    self.transform.scale
                } else {
                    1.0
                };
                // At a low zoom one model unit may be a fraction of a pixel; keep at least
                // `RULER_TICK_MIN_SPACING` surface pixels between labelled ticks.
                let min_model = ((RULER_TICK_MIN_SPACING as f32) / scale).ceil() as i32;
                let step = unit.max(min_model).max(1);
                let left = self.transform.inverse_point(Point::new(rect.x, 0)).x;
                let right =
                    self.transform.inverse_point(Point::new(rect.x + rect.width as i32, 0)).x;
                push_model_ticks(&mut xs, left, right, step, |x| {
                    self.transform.point(Point::new(x, 0)).x
                });
                let top = self.transform.inverse_point(Point::new(0, rect.y)).y;
                let bottom =
                    self.transform.inverse_point(Point::new(0, rect.y + rect.height as i32)).y;
                push_model_ticks(&mut ys, top, bottom, step, |y| {
                    self.transform.point(Point::new(0, y)).y
                });
            }
            GridSpace::Screen => {
                let step = unit.max(RULER_TICK_MIN_SPACING).max(1);
                let mut x = rect.x.div_euclid(step) * step;
                while x <= rect.x + rect.width as i32 {
                    let model = self.transform.inverse_point(Point::new(x, 0)).x;
                    xs.push((x, model));
                    x += step;
                }
                let mut y = rect.y.div_euclid(step) * step;
                while y <= rect.y + rect.height as i32 {
                    let model = self.transform.inverse_point(Point::new(0, y)).y;
                    ys.push((y, model));
                    y += step;
                }
            }
        }
        (xs, ys)
    }

    /// Draws the background grid on the surface (SP-P54).
    ///
    /// In model space the lines are the multiples of `grid_step` **design units**, mapped through
    /// the transform, so a zoomed or panned canvas shows the design grid at the design origin
    /// rather than a pixel lattice anchored to the control's corner (RW-124). Screen space keeps
    /// the historical pixel lattice.
    pub(super) fn draw_grid_lines(&self, context: &mut RenderContext, rect: Rect, color: Color) {
        let unit = self.grid_step.max(1) as i32;
        // The two coordinate lists only depend on the viewport, the step and (in model space) the
        // transform, and they are rebuilt every frame. They live in one pair of reused buffers so
        // a steady frame pays no allocation and the drawn lines are unchanged.
        let mut scratch = self.grid_scratch.borrow_mut();
        let (xs, ys) = &mut *scratch;
        xs.clear();
        ys.clear();
        match self.grid_space {
            GridSpace::Model => {
                let left = self.transform.inverse_point(Point::new(rect.x, 0)).x;
                let right =
                    self.transform.inverse_point(Point::new(rect.x + rect.width as i32, 0)).x;
                push_model_ticks(xs, left, right, unit, |x| {
                    self.transform.point(Point::new(x, 0)).x
                });
                let top = self.transform.inverse_point(Point::new(0, rect.y)).y;
                let bottom =
                    self.transform.inverse_point(Point::new(0, rect.y + rect.height as i32)).y;
                push_model_ticks(ys, top, bottom, unit, |y| {
                    self.transform.point(Point::new(0, y)).y
                });
            }
            GridSpace::Screen => {
                let mut x = rect.x;
                while x <= rect.x + rect.width as i32 {
                    xs.push((x, x));
                    x += unit;
                }
                let mut y = rect.y;
                while y <= rect.y + rect.height as i32 {
                    ys.push((y, y));
                    y += unit;
                }
            }
        }
        for &(surface, _) in xs.iter() {
            context.draw_line(
                Point::new(surface, rect.y),
                Point::new(surface, rect.y + rect.height as i32),
                color,
            );
        }
        for &(surface, _) in ys.iter() {
            context.draw_line(
                Point::new(rect.x, surface),
                Point::new(rect.x + rect.width as i32, surface),
                color,
            );
        }
    }

    /// A cache key that changes when a model-space grid's placement changes (RW-124).
    ///
    /// A screen-space grid depends only on the step; a model-space one also depends on the
    /// transform, and a cached layer without it would keep a grid the pan has moved.
    pub(super) fn grid_placement_key(&self) -> u64 {
        if self.grid_space != GridSpace::Model {
            return 0;
        }
        (self.transform.scale.to_bits() as u64).rotate_left(21)
            ^ (self.transform.offset.x as u32 as u64).rotate_left(9)
            ^ (self.transform.offset.y as u32 as u64)
    }

    /// Shows or hides the ruler strips along the top and left edges.
    pub fn set_rulers_visible(&mut self, visible: bool) {
        if self.rulers_visible == visible {
            return;
        }
        self.rulers_visible = visible;
        self.base.request_redraw();
    }

    /// Adds an alignment guide at `position` in model coordinates (RW-32).
    ///
    /// Guides are decorations: they live on the overlay layer, so they are not undoable, are not
    /// hit-testable and are omitted from PNG and SVG exports.
    pub fn draw_guide(&mut self, axis: GuideAxis, position: i32) {
        self.guides.push((axis, position));
        self.base.request_redraw();
    }

    /// Removes every alignment guide.
    pub fn clear_guides(&mut self) {
        if self.guides.is_empty() {
            return;
        }
        self.guides.clear();
        self.base.request_redraw();
    }

    /// Returns how many alignment guides are present.
    pub fn guide_count(&self) -> usize {
        self.guides.len()
    }

    /// Returns the alignment guides in insertion order.
    pub fn guides(&self) -> &[(GuideAxis, i32)] {
        &self.guides
    }

    /// Paints the overlay commands, guides and ruler strips (RW-30/RW-32).
    ///
    /// This is the single decoration pass, so both export paths and the hit test can exclude it
    /// by construction: `export_png` never calls it, and `export_svg` sets `suppress_overlay`
    /// while the SVG backend replays [`Canvas::draw`].
    pub(super) fn draw_decorations(&self, context: &mut RenderContext, rect: Rect) {
        if self.suppress_overlay {
            return;
        }
        let transform = self.transform;
        for stored in &self.overlay_commands {
            let mapped = transform_command(&stored.command, transform);
            if let Some(bounds) = command_bounds(&mapped) {
                if !rects_intersect(rect, bounds) {
                    continue;
                }
            }
            context.execute_command(mapped);
        }
        let guide_color = Color::rgba(0, 160, 255, 200);
        for (axis, position) in &self.guides {
            match axis {
                GuideAxis::Vertical => {
                    let x = transform.point(Point::new(*position, 0)).x;
                    if x >= rect.x && x < rect.x + rect.width as i32 {
                        context.draw_line(
                            Point::new(x, rect.y),
                            Point::new(x, rect.y + rect.height as i32 - 1),
                            guide_color,
                        );
                    }
                }
                GuideAxis::Horizontal => {
                    let y = transform.point(Point::new(0, *position)).y;
                    if y >= rect.y && y < rect.y + rect.height as i32 {
                        context.draw_line(
                            Point::new(rect.x, y),
                            Point::new(rect.x + rect.width as i32 - 1, y),
                            guide_color,
                        );
                    }
                }
            }
        }
        if self.rulers_visible {
            self.draw_rulers(context, rect);
        }
    }

    /// Paints the top and left ruler strips with ticks spaced by the ruler unit.
    fn draw_rulers(&self, context: &mut RenderContext, rect: Rect) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let thickness = RULER_THICKNESS.min(rect.height as i32);
        let side_width = RULER_THICKNESS.min(rect.width as i32);
        let band = Color::rgba(246, 246, 246, 235);
        let tick = Color::rgba(90, 90, 90, 180);
        context.execute_command(RenderCommand::FillRect {
            rect: Rect::new(rect.x, rect.y, rect.width, thickness.max(0) as u32),
            color: band,
        });
        context.execute_command(RenderCommand::FillRect {
            rect: Rect::new(rect.x, rect.y, side_width.max(0) as u32, rect.height),
            color: band,
        });
        // The ticks come from the same derivation `ruler_ticks` exposes, so a host that draws
        // labels from that list and the control that paints the ticks cannot disagree about
        // where a label belongs (RW-124).
        let unit = self.effective_ruler_unit();
        let (xs, ys) = self.axis_ticks(rect, unit);
        for (x, _) in xs {
            context.draw_line(
                Point::new(x, rect.y + thickness - 6),
                Point::new(x, rect.y + thickness),
                tick,
            );
        }
        for (y, _) in ys {
            context.draw_line(
                Point::new(rect.x + side_width - 6, y),
                Point::new(rect.x + side_width, y),
                tick,
            );
        }
    }

    /// Sets the cursor shape this control requests while hovered (RW-33).
    ///
    /// The value is the widget trait's own [`CursorIcon`], so a host does not need a second
    /// cursor vocabulary. A pan gesture takes precedence while it is active.
    pub fn set_cursor_icon(&mut self, icon: CursorIcon) {
        self.cursor_icon_override = Some(icon);
    }

    /// Drops the explicit cursor override, restoring the drawing-surface default.
    pub fn clear_cursor_icon(&mut self) {
        self.cursor_icon_override = None;
    }

    /// The explicit cursor override, when one was set.
    pub fn cursor_icon_override(&self) -> Option<CursorIcon> {
        self.cursor_icon_override
    }

    /// Enables or disables the middle-button / space-drag pan gesture (RW-33).
    pub fn set_pan_enabled(&mut self, enabled: bool) {
        self.pan_enabled = enabled;
        if !enabled {
            self.panning = false;
        }
    }

    /// Returns whether the pan gesture is allowed.
    pub fn is_pan_enabled(&self) -> bool {
        self.pan_enabled
    }

    /// Returns whether a pan gesture is in progress.
    pub fn is_panning(&self) -> bool {
        self.panning
    }

    /// The cursor the control actually asks for: pan affordance first, then the host override,
    /// then the drawing-surface default.
    pub(super) fn effective_cursor_icon(&self) -> CursorIcon {
        if self.panning || (self.space_held && self.pan_enabled) {
            CursorIcon::Move
        } else {
            self.cursor_icon_override.unwrap_or(CursorIcon::Crosshair)
        }
    }

    /// Records whether the space bar is held, which turns a primary drag into a pan.
    pub(super) fn note_space(&mut self, held: bool) {
        self.space_held = held;
    }

    /// Returns whether `button` starts a pan rather than a drawing gesture.
    pub(super) fn wants_pan_gesture(&self, button: u32) -> bool {
        self.pan_enabled
            && (button == crate::event::mouse_button::MIDDLE
                || (button == crate::event::mouse_button::PRIMARY && self.space_held))
    }

    /// Starts a pan gesture at `pos`.
    pub(super) fn begin_pan(&mut self, pos: Point) -> bool {
        if !self.pan_enabled {
            return false;
        }
        self.panning = true;
        self.pan_last = pos;
        true
    }

    /// Advances a pan gesture to `pos`, translating the canvas by the pointer delta.
    pub(super) fn update_pan(&mut self, pos: Point) -> bool {
        if !self.panning {
            return false;
        }
        let delta = Point::new(pos.x - self.pan_last.x, pos.y - self.pan_last.y);
        self.pan_last = pos;
        if delta.x == 0 && delta.y == 0 {
            return true;
        }
        self.transform.offset =
            Point::new(self.transform.offset.x + delta.x, self.transform.offset.y + delta.y);
        // The replay mapping embeds the offset, so every cached copy is stale now.
        self.invalidate_replay_caches();
        self.base.request_redraw();
        true
    }

    /// Ends a pan gesture, returning whether one was in progress.
    pub(super) fn end_pan(&mut self) -> bool {
        if !self.panning {
            return false;
        }
        self.panning = false;
        true
    }

    /// Serialises the view configuration for session restore (RW-64).
    ///
    /// Only user-visible view state is stored: the transform, grid, snapping, rulers, guides and
    /// pan switch. The drawing commands belong to the host's document and are deliberately left
    /// out, exactly as the canvas's own layer model is not a persistence format.
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(36 + self.guides.len() * 5);
        state.push(CANVAS_STATE_VERSION);
        state.extend_from_slice(&self.transform.scale.to_le_bytes());
        state.extend_from_slice(&self.transform.offset.x.to_le_bytes());
        state.extend_from_slice(&self.transform.offset.y.to_le_bytes());
        let mut flags = 0u8;
        flags |= u8::from(self.zoom_enabled);
        flags |= u8::from(self.anti_alias) << 1;
        flags |= u8::from(self.show_grid) << 2;
        flags |= u8::from(self.snap_to_grid) << 3;
        flags |= u8::from(self.grid_space == GridSpace::Model) << 4;
        flags |= u8::from(self.rulers_visible) << 5;
        flags |= u8::from(self.pan_enabled) << 6;
        state.push(flags);
        state.extend_from_slice(&self.grid_step.to_le_bytes());
        state.extend_from_slice(&self.ruler_unit.to_le_bytes());
        state.extend_from_slice(&self.zoom_min.to_le_bytes());
        state.extend_from_slice(&self.zoom_max.to_le_bytes());
        state.extend_from_slice(&self.zoom_step.to_le_bytes());
        let guide_count = self.guides.len().min(u16::MAX as usize);
        state.extend_from_slice(&(guide_count as u16).to_le_bytes());
        for (axis, position) in self.guides.iter().take(guide_count) {
            state.push(match axis {
                GuideAxis::Horizontal => 0,
                GuideAxis::Vertical => 1,
            });
            state.extend_from_slice(&position.to_le_bytes());
        }
        state
    }

    /// Restores a blob produced by [`Canvas::save_state`].
    ///
    /// Returns `false` and leaves the canvas untouched when the blob is truncated, carries a
    /// different version, or contains an invalid transform/zoom range.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() < 36 || state[0] != CANVAS_STATE_VERSION {
            return false;
        }
        let scale = f32::from_le_bytes([state[1], state[2], state[3], state[4]]);
        let offset_x = i32::from_le_bytes([state[5], state[6], state[7], state[8]]);
        let offset_y = i32::from_le_bytes([state[9], state[10], state[11], state[12]]);
        let flags = state[13];
        let grid_step = u32::from_le_bytes([state[14], state[15], state[16], state[17]]);
        let ruler_unit = u32::from_le_bytes([state[18], state[19], state[20], state[21]]);
        let zoom_min = f32::from_le_bytes([state[22], state[23], state[24], state[25]]);
        let zoom_max = f32::from_le_bytes([state[26], state[27], state[28], state[29]]);
        let zoom_step = f32::from_le_bytes([state[30], state[31], state[32], state[33]]);
        let guide_count = u16::from_le_bytes([state[34], state[35]]) as usize;
        if state.len() != 36 + guide_count * 5 {
            return false;
        }
        if !scale.is_finite()
            || scale <= 0.0
            || !zoom_min.is_finite()
            || !zoom_max.is_finite()
            || zoom_min <= 0.0
            || zoom_min >= zoom_max
            || !zoom_step.is_finite()
            || zoom_step <= 1.0
        {
            return false;
        }
        let mut guides = Vec::with_capacity(guide_count);
        let mut cursor = 36usize;
        for _ in 0..guide_count {
            let axis = match state[cursor] {
                0 => GuideAxis::Horizontal,
                1 => GuideAxis::Vertical,
                _ => return false,
            };
            let position = i32::from_le_bytes([
                state[cursor + 1],
                state[cursor + 2],
                state[cursor + 3],
                state[cursor + 4],
            ]);
            guides.push((axis, position));
            cursor += 5;
        }

        let anti_alias = flags & 0b0000_0010 != 0;
        self.transform = CanvasTransform { scale, offset: Point::new(offset_x, offset_y) };
        self.zoom_enabled = flags & 0b0000_0001 != 0;
        self.show_grid = flags & 0b0000_0100 != 0;
        self.snap_to_grid = flags & 0b0000_1000 != 0;
        self.grid_space =
            if flags & 0b0001_0000 != 0 { GridSpace::Model } else { GridSpace::Screen };
        self.rulers_visible = flags & 0b0010_0000 != 0;
        self.pan_enabled = flags & 0b0100_0000 != 0;
        self.panning = false;
        self.grid_step = grid_step.max(1);
        self.ruler_unit = ruler_unit;
        self.zoom_min = zoom_min;
        self.zoom_max = zoom_max;
        self.zoom_step = zoom_step;
        self.guides = guides;
        // The setter rewrites already-stored commands when the policy turns off; writing the
        // field directly would claim a policy the recorded commands do not follow.
        if self.anti_alias != anti_alias {
            self.set_anti_alias(anti_alias);
        }
        self.invalidate_replay_caches();
        self.base.request_redraw();
        true
    }
}
