// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas widget for custom drawing operations.
//!
//! The `Canvas` widget provides a drawing surface where users can push
//! rendering commands (lines, circles, rectangles, paths, text) and have
//! them replayed during the `draw()` pass. It also emits mouse interaction
//! signals so callers can respond to clicks, drags, and hover events on
//! the drawing area.
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{PaintBackend, RenderCommand, RenderContext};
use crate::signal::GenericSignal;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::collections::VecDeque;
use core::ops::Deref;

/// A uniform scale and translation applied to every drawing command a [`Canvas`] stores.
///
/// The transform is applied when a command is added, not when it is painted, so the stored
/// geometry is already in surface coordinates: hit tests, exports and the commands themselves
/// all read the same space. Changing the transform therefore affects commands added afterwards;
/// a caller that wants to move existing content clears and re-adds it, which is what a drawing
/// surface's undo model expects.
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
}

/// One pointer event that carries both the legacy payload-free signal and the typed payload.
///
/// # Why both signals are kept
///
/// The field type used to be [`GenericSignal`], so existing callers connect zero-argument
/// slots (and the deliberate example `canvas.mouse_pressed.connect_scoped(..)` does exactly
/// that). The published event, however, must carry *where* and *which button*, which the
/// widget trait already declares as `(Point, u32)`. Rather than delete the old signal, this
/// type keeps it and emits the typed one alongside it: old subscribers still fire, and a
/// dynamic subscriber receives the payload the schema declares.
///
/// [`Self::deref`] hands out the legacy signal, so every existing method call
/// (`connect`, `connect_scoped`, `disconnect`, `slot_count`, …) keeps compiling unchanged.
#[derive(Clone, Default)]
pub struct CanvasPointerEvent {
    /// The payload-free compatibility signal.
    unit: GenericSignal,
    /// The typed signal the published event resolves to.
    payload: Signal1<(Point, u32)>,
}

impl CanvasPointerEvent {
    /// Creates an event pair with no subscribers.
    pub fn new() -> Self {
        Self { unit: GenericSignal::new(), payload: Signal1::new() }
    }

    /// The typed signal carrying `(position, button)`.
    pub fn payload_signal(&self) -> &Signal1<(Point, u32)> {
        &self.payload
    }

    /// Emits the event: the legacy signal first, then the typed one.
    pub fn emit_at(&self, position: Point, button: u32) {
        self.unit.emit();
        self.payload.emit((position, button));
    }
}

impl Deref for CanvasPointerEvent {
    type Target = GenericSignal;

    fn deref(&self) -> &Self::Target {
        &self.unit
    }
}

/// One stored drawing command, tagged with the order it was added in.
///
/// The sequence number is what lets undo work across layers: the *globally*
/// newest command can be found without assuming the last command of the last
/// layer, because layers are reordered independently of insertion order.
struct StoredCommand {
    seq: u64,
    command: RenderCommand,
}

/// One canvas layer: a named, independently orderable and hideable command list.
///
/// A `VecDeque` rather than a `Vec` because the command limit evicts from the
/// front on every push once the cap is reached; `Vec::remove(0)` shifted the whole
/// layer on each of those pushes.
struct CanvasLayer {
    id: u64,
    name: String,
    visible: bool,
    commands: VecDeque<StoredCommand>,
}

/// A command removed by undo, retained so redo can put it back.
struct UndoneCommand {
    layer_id: u64,
    command: RenderCommand,
}

/// Canvas widget for custom drawing operations.
pub struct Canvas {
    base: BaseWidget,
    /// Layers in painting order (back to front).
    layers: Vec<CanvasLayer>,
    /// The layer new commands are appended to; `None` means the default layer.
    current_layer: Option<u64>,
    /// Next layer identity; identities are stable across reorders and removals.
    next_layer_id: u64,
    /// Next command sequence number, used to locate the newest command globally.
    next_command_seq: u64,
    /// The transform applied to commands as they are added.
    transform: CanvasTransform,
    /// Saved transforms, restored by [`Canvas::pop_transform`].
    transform_stack: Vec<CanvasTransform>,
    /// Undo steps. Each step is the list of `(layer id, command seq)` references
    /// recorded since the last step boundary; a group makes one step.
    undo_stack: VecDeque<Vec<(u64, u64)>>,
    /// Commands removed by undo, available to redo. Bounded by `command_limit`.
    redo_stack: VecDeque<Vec<UndoneCommand>>,
    /// Commands pushed since `begin_group`, if a group is open.
    pending_group: Option<Vec<(u64, u64)>>,
    /// The most commands the canvas retains; the oldest is dropped past this.
    command_limit: usize,
    /// Whether the wheel zooms the transform.
    zoom_enabled: bool,
    /// Global anti-aliasing switch applied to every stored command.
    anti_alias: bool,
    /// Draw a background grid on the canvas surface.
    show_grid: bool,
    /// Grid cell size in surface pixels.
    grid_step: u32,
    /// Snap incoming coordinates to the grid before they are stored.
    snap_to_grid: bool,
    /// Smallest allowed zoom scale.
    zoom_min: f32,
    /// Largest allowed zoom scale.
    zoom_max: f32,
    /// Multiplicative wheel zoom step (must be greater than one).
    zoom_step: f32,
    /// Emitted when a context menu is requested, carrying the pointer position.
    pub context_menu_requested: Signal1<Point>,
    /// Track mouse position for interaction signals.
    last_mouse_pos: Point,
    /// Emitted when mouse is pressed on the canvas, with the pointer position and button.
    pub mouse_pressed: CanvasPointerEvent,
    /// Emitted when mouse is released on the canvas, with the pointer position and button.
    pub mouse_released: CanvasPointerEvent,
    /// Emitted when mouse moves over the canvas, carrying the current cursor position.
    pub mouse_moved: crate::signal::Signal1<Point>,
    /// Emitted when a double-click occurs on the canvas, with the pointer position and button.
    pub double_clicked: CanvasPointerEvent,
    /// Emitted when the layer set, order or visibility changes, with the layer count.
    pub layer_changed: Signal1<usize>,
    /// Emitted when the stored content changes (push, clear, undo, redo), with the command count.
    pub content_changed: Signal1<usize>,
    /// Emitted when the zoom scale changes.
    pub zoom_changed: Signal1<f32>,
    /// Emitted with the index of the command picked by a primary press.
    pub command_picked: Signal1<usize>,
}

impl Canvas {
    /// Creates a new canvas widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "Canvas"),
            layers: Vec::new(),
            current_layer: None,
            next_layer_id: 1,
            next_command_seq: 1,
            transform: CanvasTransform::default(),
            transform_stack: Vec::new(),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            pending_group: None,
            command_limit: 100_000,
            zoom_enabled: true,
            anti_alias: true,
            show_grid: false,
            grid_step: 20,
            snap_to_grid: false,
            zoom_min: 0.1,
            zoom_max: 16.0,
            zoom_step: 1.1,
            context_menu_requested: Signal1::new(),
            last_mouse_pos: Point::new(0, 0),
            mouse_pressed: CanvasPointerEvent::new(),
            mouse_released: CanvasPointerEvent::new(),
            mouse_moved: crate::signal::Signal1::new(),
            double_clicked: CanvasPointerEvent::new(),
            layer_changed: Signal1::new(),
            content_changed: Signal1::new(),
            zoom_changed: Signal1::new(),
            command_picked: Signal1::new(),
        }
    }

    /// The typed pointer-press signal that the published `mouse_pressed` event resolves to.
    ///
    /// The declared payload is `(position in parent coordinates, pointer button)`, the same
    /// pair the base widget's own pointer signal carries.
    pub fn mouse_pressed_signal(&self) -> &Signal1<(Point, u32)> {
        self.mouse_pressed.payload_signal()
    }

    /// The typed pointer-release signal behind the published `mouse_released` event.
    pub fn mouse_released_signal(&self) -> &Signal1<(Point, u32)> {
        self.mouse_released.payload_signal()
    }

    /// The typed double-click signal behind the published `double_clicked` event.
    pub fn double_clicked_signal(&self) -> &Signal1<(Point, u32)> {
        self.double_clicked.payload_signal()
    }

    /// Remove all stored drawing commands and every layer.
    pub fn clear(&mut self) {
        self.layers.clear();
        self.current_layer = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pending_group = None;
        self.content_changed.emit(0);
        self.layer_changed.emit(0);
        self.base.request_redraw();
    }

    /// Returns the number of stored drawing commands across every layer.
    pub fn command_count(&self) -> usize {
        self.layers.iter().map(|layer| layer.commands.len()).sum()
    }

    /// Returns how many layers exist.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Starts using (or creates) the layer named `name` and returns its index.
    ///
    /// Commands pushed from now on belong to this layer instead of the previous
    /// one. Creating a layer emits [`Self::layer_changed`]; selecting an existing
    /// layer does not, because nothing about the layer set changed.
    pub fn begin_layer(&mut self, name: impl Into<String>) -> usize {
        let name = name.into();
        if let Some(index) = self.layers.iter().position(|layer| layer.name == name) {
            self.current_layer = Some(self.layers[index].id);
            return index;
        }
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(CanvasLayer { id, name, visible: true, commands: VecDeque::new() });
        self.current_layer = Some(id);
        self.layer_changed.emit(self.layers.len());
        self.base.request_redraw();
        self.layers.len() - 1
    }

    /// Shows or hides the layer at `index`, returning whether it exists.
    pub fn set_layer_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(layer) = self.layers.get_mut(index) else { return false };
        if layer.visible == visible {
            return true;
        }
        layer.visible = visible;
        self.layer_changed.emit(self.layers.len());
        self.base.request_redraw();
        true
    }

    /// Returns whether the layer at `index` is visible.
    pub fn is_layer_visible(&self, index: usize) -> bool {
        self.layers.get(index).map(|layer| layer.visible).unwrap_or(false)
    }

    /// Moves the layer at `from` to `to` in painting order, returning whether it moved.
    pub fn move_layer(&mut self, from: usize, to: usize) -> bool {
        if from >= self.layers.len() || to > self.layers.len() {
            return false;
        }
        if from == to {
            return true;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to.min(self.layers.len()), layer);
        self.layer_changed.emit(self.layers.len());
        self.base.request_redraw();
        true
    }

    /// Removes the layer at `index` and every command it holds.
    ///
    /// Commands removed this way cannot be undone: a layer removal is a structural
    /// change, not a drawing step, so history entries that referenced it are dropped.
    pub fn remove_layer(&mut self, index: usize) -> bool {
        if index >= self.layers.len() {
            return false;
        }
        let layer = self.layers.remove(index);
        for step in &mut self.undo_stack {
            step.retain(|(layer_id, _)| *layer_id != layer.id);
        }
        self.undo_stack.retain(|step| !step.is_empty());
        self.redo_stack.retain_mut(|step| {
            step.retain(|undone| undone.layer_id != layer.id);
            !step.is_empty()
        });
        if self.current_layer == Some(layer.id) {
            self.current_layer = None;
        }
        let count = self.command_count();
        self.layer_changed.emit(self.layers.len());
        self.content_changed.emit(count);
        self.base.request_redraw();
        true
    }

    /// Returns the transform applied to newly added commands.
    pub fn transform(&self) -> CanvasTransform {
        self.transform
    }

    /// Replaces the transform applied to newly added commands.
    pub fn set_transform(&mut self, transform: CanvasTransform) {
        if self.transform == transform {
            return;
        }
        let scale_changed = self.transform.scale != transform.scale;
        self.transform = transform;
        if scale_changed {
            self.zoom_changed.emit(self.transform.scale);
        }
        self.base.request_redraw();
    }

    /// Saves the current transform, restoring it with [`Self::pop_transform`].
    pub fn push_transform(&mut self) {
        self.transform_stack.push(self.transform);
    }

    /// Restores the transform saved by the matching [`Self::push_transform`].
    pub fn pop_transform(&mut self) {
        if let Some(transform) = self.transform_stack.pop() {
            self.transform = transform;
            self.base.request_redraw();
        }
    }

    /// Enables or disables wheel zoom.
    pub fn set_zoom_enabled(&mut self, enabled: bool) {
        if self.zoom_enabled == enabled {
            return;
        }
        self.zoom_enabled = enabled;
    }

    /// Returns whether wheel zoom is enabled.
    pub fn is_zoom_enabled(&self) -> bool {
        self.zoom_enabled
    }

    /// Returns whether anti-aliasing is enabled globally.
    pub fn anti_alias(&self) -> bool {
        self.anti_alias
    }

    /// Enables or disables anti-aliasing for every command added afterwards.
    ///
    /// Turning it off also rewrites the anti-aliased commands already stored, so
    /// the switch is global rather than only affecting future drawing. Turning it
    /// back on affects future commands; the plain forms an earlier disable wrote
    /// are not recoverable, because the flag is a rendering policy and not part of
    /// the recorded command.
    pub fn set_anti_alias(&mut self, enabled: bool) {
        if self.anti_alias == enabled {
            return;
        }
        self.anti_alias = enabled;
        if !enabled {
            for layer in &mut self.layers {
                let commands = core::mem::take(&mut layer.commands);
                layer.commands = commands
                    .into_iter()
                    .map(|mut stored| {
                        stored.command = without_anti_aliasing(stored.command);
                        stored
                    })
                    .collect();
            }
        }
        self.base.request_redraw();
    }

    /// Returns whether the canvas draws a background grid.
    pub fn show_grid(&self) -> bool {
        self.show_grid
    }

    /// Shows or hides the background grid.
    pub fn set_show_grid(&mut self, show: bool) {
        if self.show_grid == show {
            return;
        }
        self.show_grid = show;
        self.base.request_redraw();
    }

    /// Returns the grid cell size in pixels.
    pub fn grid_step(&self) -> u32 {
        self.grid_step
    }

    /// Sets the grid cell size, clamped to at least one pixel.
    pub fn set_grid_step(&mut self, step: u32) {
        let step = step.max(1);
        if self.grid_step == step {
            return;
        }
        self.grid_step = step;
        self.base.request_redraw();
    }

    /// Returns whether incoming coordinates snap to the grid.
    pub fn snap_to_grid(&self) -> bool {
        self.snap_to_grid
    }

    /// Enables or disables grid snapping for incoming coordinates.
    pub fn set_snap_to_grid(&mut self, snap: bool) {
        if self.snap_to_grid == snap {
            return;
        }
        self.snap_to_grid = snap;
    }

    /// Returns the allowed zoom range as `(min, max)`.
    pub fn zoom_range(&self) -> (f32, f32) {
        (self.zoom_min, self.zoom_max)
    }

    /// Sets the allowed zoom range, rejecting a non-finite or inverted pair.
    ///
    /// The current scale is clamped into the new range and
    /// [`Self::zoom_changed`] fires when that changes it.
    pub fn set_zoom_range(&mut self, min: f32, max: f32) -> bool {
        if !min.is_finite() || !max.is_finite() || min <= 0.0 || min >= max {
            return false;
        }
        self.zoom_min = min;
        self.zoom_max = max;
        let scale = self.transform.scale.clamp(min, max);
        if scale != self.transform.scale {
            self.transform.scale = scale;
            self.zoom_changed.emit(scale);
            self.base.request_redraw();
        }
        true
    }

    /// Returns the multiplicative wheel zoom step.
    pub fn zoom_step(&self) -> f32 {
        self.zoom_step
    }

    /// Sets the multiplicative wheel zoom step, clamped to a meaningful range.
    ///
    /// A step of exactly `1.0` would make the wheel do nothing, so the stored
    /// value is kept above one.
    pub fn set_zoom_step(&mut self, step: f32) {
        if !step.is_finite() {
            return;
        }
        self.zoom_step = step.clamp(1.01, 10.0);
    }

    /// Sets how many commands the canvas retains (clamped to at least one).
    ///
    /// Both the undo and redo histories use the same bound, so a long session of
    /// undoing cannot grow memory without limit either.
    pub fn set_command_limit(&mut self, limit: usize) {
        self.command_limit = limit.max(1);
        self.trim_to_limit();
        while self.undo_stack.len() > self.command_limit {
            self.undo_stack.pop_front();
        }
        while self.redo_stack.len() > self.command_limit {
            self.redo_stack.pop_front();
        }
    }

    /// Returns the command limit.
    pub fn command_limit(&self) -> usize {
        self.command_limit
    }

    /// Begins a command group; every command pushed until [`Self::end_group`]
    /// becomes one undo step.
    ///
    /// Returns `false` when a group is already open (groups do not nest).
    pub fn begin_group(&mut self) -> bool {
        if self.pending_group.is_some() {
            return false;
        }
        self.pending_group = Some(Vec::new());
        true
    }

    /// Ends the open command group, returning whether it contained any command.
    pub fn end_group(&mut self) -> bool {
        let Some(group) = self.pending_group.take() else { return false };
        if group.is_empty() {
            return false;
        }
        self.undo_stack.push_back(group);
        self.trim_undo_stack();
        true
    }

    /// Removes the most recently added command, returning whether one was undone.
    pub fn undo(&mut self) -> bool {
        loop {
            let Some(step) = self.undo_stack.pop_back() else { return false };
            let mut removed = Vec::new();
            // Undo the newest command of the step first, so a grouped stroke comes
            // apart in the reverse order it was drawn.
            for (layer_id, seq) in step.into_iter().rev() {
                if let Some(command) = self.take_command(layer_id, seq) {
                    removed.push(UndoneCommand { layer_id, command });
                }
            }
            if removed.is_empty() {
                // Everything in this step was already evicted by the command limit.
                continue;
            }
            removed.reverse();
            self.redo_stack.push_back(removed);
            self.trim_redo_stack();
            self.content_changed.emit(self.command_count());
            self.base.request_redraw();
            return true;
        }
    }

    /// Re-applies the command step most recently removed by [`Self::undo`].
    pub fn redo(&mut self) -> bool {
        loop {
            let Some(step) = self.redo_stack.pop_back() else { return false };
            let mut restored = Vec::new();
            for undone in step {
                let Some(layer_index) = self.layer_index_by_id(undone.layer_id) else { continue };
                let seq = self.next_command_seq;
                self.next_command_seq += 1;
                self.layers[layer_index]
                    .commands
                    .push_back(StoredCommand { seq, command: undone.command });
                restored.push((undone.layer_id, seq));
            }
            if restored.is_empty() {
                continue;
            }
            self.undo_stack.push_back(restored);
            self.trim_undo_stack();
            self.trim_to_limit();
            self.content_changed.emit(self.command_count());
            self.base.request_redraw();
            return true;
        }
    }

    /// Returns whether there is an undone command to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Returns how many undo steps are available.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns how many redo steps are available.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Returns whether a command step can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns the layer index whose stable identity is `id`.
    fn layer_index_by_id(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|layer| layer.id == id)
    }

    /// Returns the index of the layer commands are appended to, creating the
    /// default layer when none was selected.
    fn ensure_layer(&mut self) -> usize {
        if let Some(index) = self.current_layer.and_then(|id| self.layer_index_by_id(id)) {
            return index;
        }
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(CanvasLayer {
            id,
            name: String::from("default"),
            visible: true,
            commands: VecDeque::new(),
        });
        self.current_layer = Some(id);
        self.layer_changed.emit(self.layers.len());
        self.layers.len() - 1
    }

    /// Removes one command by its layer identity and sequence number.
    fn take_command(&mut self, layer_id: u64, seq: u64) -> Option<RenderCommand> {
        let layer_index = self.layer_index_by_id(layer_id)?;
        let commands = &mut self.layers[layer_index].commands;
        let index = commands.iter().position(|stored| stored.seq == seq)?;
        commands.remove(index).map(|stored| stored.command)
    }

    /// Index of the layer holding the globally oldest command.
    fn oldest_command_layer(&self) -> Option<usize> {
        let mut oldest: Option<(usize, u64)> = None;
        for (layer_index, layer) in self.layers.iter().enumerate() {
            if let Some(first) = layer.commands.front() {
                if oldest.is_none_or(|(_, seq)| first.seq < seq) {
                    oldest = Some((layer_index, first.seq));
                }
            }
        }
        oldest.map(|(layer_index, _)| layer_index)
    }

    /// Drops the oldest commands until the limit is respected, and removes their
    /// now-dead references from the history so undo cannot resurrect them.
    fn trim_to_limit(&mut self) {
        while self.command_count() > self.command_limit {
            let Some(layer_index) = self.oldest_command_layer() else { break };
            let layer_id = self.layers[layer_index].id;
            let Some(removed) = self.layers[layer_index].commands.pop_front() else { break };
            let seq = removed.seq;
            for step in &mut self.undo_stack {
                step.retain(|(id, entry_seq)| !(*id == layer_id && *entry_seq == seq));
            }
            self.undo_stack.retain(|step| !step.is_empty());
            if let Some(group) = &mut self.pending_group {
                group.retain(|(id, entry_seq)| !(*id == layer_id && *entry_seq == seq));
            }
        }
    }

    /// Keeps the undo history within the same bound as the command list.
    fn trim_undo_stack(&mut self) {
        while self.undo_stack.len() > self.command_limit {
            self.undo_stack.pop_front();
        }
    }

    /// Keeps the redo history within the same bound as the command list.
    fn trim_redo_stack(&mut self) {
        while self.redo_stack.len() > self.command_limit {
            self.redo_stack.pop_front();
        }
    }

    /// Applies the current transform, snapping and anti-aliasing policy to `command`
    /// and stores it in the current layer.
    fn push_command(&mut self, command: RenderCommand) {
        let transform = self.transform;
        let mut command = match command {
            RenderCommand::FillRect { rect, color } => {
                RenderCommand::FillRect { rect: transform.rect(rect), color }
            }
            RenderCommand::DrawRect { rect, color } => {
                RenderCommand::DrawRect { rect: transform.rect(rect), color }
            }
            RenderCommand::FillRoundedRect { rect, radius, color } => {
                RenderCommand::FillRoundedRect {
                    rect: transform.rect(rect),
                    radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                    color,
                }
            }
            RenderCommand::DrawLine { from, to, color } => RenderCommand::DrawLine {
                from: transform.point(from),
                to: transform.point(to),
                color,
            },
            RenderCommand::DrawLineAA { from, to, color } => RenderCommand::DrawLineAA {
                from: transform.point(from),
                to: transform.point(to),
                color,
            },
            RenderCommand::DrawLineStroke { from, to, color, width } => {
                RenderCommand::DrawLineStroke {
                    from: transform.point(from),
                    to: transform.point(to),
                    color,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            RenderCommand::FillCircle { center, radius, color } => RenderCommand::FillCircle {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::FillCircleAA { center, radius, color } => RenderCommand::FillCircleAA {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::DrawCircle { center, radius, color } => RenderCommand::DrawCircle {
                center: transform.point(center),
                radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                color,
            },
            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                RenderCommand::DrawCircleStroke {
                    center: transform.point(center),
                    radius: (radius as f32 * transform.scale).round().max(1.0) as u32,
                    color,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                RenderCommand::DrawPath {
                    points: points.into_iter().map(|point| transform.point(point)).collect(),
                    closed,
                    color,
                    filled,
                    width: (width as f32 * transform.scale).round().max(1.0) as u32,
                }
            }
            other => other,
        };
        if self.snap_to_grid {
            command = snap_command(command, self.grid_step.max(1) as i32);
        }
        if !self.anti_alias {
            command = without_anti_aliasing(command);
        }

        let layer_index = self.ensure_layer();
        let layer_id = self.layers[layer_index].id;
        let seq = self.next_command_seq;
        self.next_command_seq += 1;
        self.layers[layer_index].commands.push_back(StoredCommand { seq, command });
        self.redo_stack.clear();
        let reference = (layer_id, seq);
        if let Some(group) = &mut self.pending_group {
            group.push(reference);
        } else {
            self.undo_stack.push_back(vec![reference]);
            self.trim_undo_stack();
        }
        self.trim_to_limit();
        self.content_changed.emit(self.command_count());
        self.base.request_redraw();
    }

    /// Exports the visible layer stack to a PNG image.
    ///
    /// `width` and `height` are the output size in pixels; `scale` is the DPI
    /// scale used while replaying (it affects text and stroke metrics, not the
    /// output dimensions). Returns the encoded PNG bytes, or an error when the
    /// rasteriser or the encoder rejects the request.
    pub fn export_png(&self, width: u32, height: u32, scale: f32) -> Result<Vec<u8>, String> {
        if width == 0 || height == 0 {
            return Err("cannot export a zero-sized canvas".into());
        }
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let size = crate::core::Size::new(width, height);
        let mut backend = crate::render::SoftwarePaintBackend::new(size, scale);
        backend.begin_frame(Color::TRANSPARENT);
        {
            let mut context = RenderContext::new(&mut backend);
            for layer in &self.layers {
                if !layer.visible {
                    continue;
                }
                for stored in &layer.commands {
                    context.execute_command_ref(&stored.command);
                }
            }
        }
        backend.end_frame();
        let rgba = backend.surface().frame_rgba().to_vec();
        let image = crate::image::DecodedImage::new(
            crate::image::ImageFormat::Png,
            crate::image::ImageData::Rgba8(rgba),
            width,
            height,
        );
        crate::image::encode(&image, crate::image::ImageFormat::Png)
    }

    /// Returns the index of the topmost drawn command containing `point`.
    ///
    /// Hidden layers are skipped, as they are when painting. The index addresses
    /// the canvas's flattened command order (all layers, in painting order), so
    /// it stays stable when a layer is hidden or shown.
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        let mut offsets = Vec::with_capacity(self.layers.len());
        let mut offset = 0usize;
        for layer in &self.layers {
            offsets.push(offset);
            offset += layer.commands.len();
        }
        for (layer_index, layer) in self.layers.iter().enumerate().rev() {
            if !layer.visible {
                continue;
            }
            for (command_index, stored) in layer.commands.iter().enumerate().rev() {
                if let Some(bounds) = command_bounds(&stored.command) {
                    if bounds.contains_point(point) {
                        return Some(offsets[layer_index] + command_index);
                    }
                }
            }
        }
        None
    }

    /// Exports the canvas to an SVG document.
    ///
    /// Uses the crate's shared SVG backend, so the export and the on-screen paint read the same
    /// command list rather than a second renderer that could drift.
    pub fn export_svg(&mut self) -> String {
        crate::widget::svg::render_to_svg(self)
    }

    // ── Drawing API ──────────────────────────────────────────────────────

    /// Fill the entire canvas area with a solid color.
    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::FillRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a rectangle outline.
    pub fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.push_command(RenderCommand::DrawRect { rect, color });
        self.base.request_redraw();
    }

    /// Draw a line segment from `from` to `to`.
    pub fn draw_line(&mut self, from: Point, to: Point, color: Color) {
        self.push_command(RenderCommand::DrawLine { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with anti-aliasing.
    pub fn draw_line_aa(&mut self, from: Point, to: Point, color: Color) {
        self.push_command(RenderCommand::DrawLineAA { from, to, color });
        self.base.request_redraw();
    }

    /// Draw a line segment with explicit stroke width.
    pub fn draw_line_stroke(&mut self, from: Point, to: Point, color: Color, width: u32) {
        self.push_command(RenderCommand::DrawLineStroke { from, to, color, width });
        self.base.request_redraw();
    }

    /// Fill a circle at `center` with `radius`.
    pub fn fill_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Fill a circle with anti-aliasing.
    pub fn fill_circle_aa(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillCircleAA { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline.
    pub fn draw_circle(&mut self, center: Point, radius: u32, color: Color) {
        self.push_command(RenderCommand::DrawCircle { center, radius, color });
        self.base.request_redraw();
    }

    /// Draw a circle outline with explicit stroke width.
    pub fn draw_circle_stroke(&mut self, center: Point, radius: u32, color: Color, width: u32) {
        self.push_command(RenderCommand::DrawCircleStroke { center, radius, color, width });
        self.base.request_redraw();
    }

    /// Fill a rounded rectangle.
    pub fn fill_rounded_rect(&mut self, rect: Rect, radius: u32, color: Color) {
        self.push_command(RenderCommand::FillRoundedRect { rect, radius, color });
        self.base.request_redraw();
    }

    /// Draw text at the specified position.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &crate::core::Font, color: Color) {
        self.push_command(RenderCommand::DrawText {
            origin,
            text: text.to_string(),
            font: font.clone(),
            color,
            alignment: crate::core::HorizontalAlignment::Left,
        });
        self.base.request_redraw();
    }

    /// Draw an arc (partial circle).
    pub fn draw_arc(
        &mut self,
        center: Point,
        radius: u32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        filled: bool,
    ) {
        self.push_command(RenderCommand::DrawArc {
            center,
            radius,
            start_angle,
            end_angle,
            color,
            filled,
        });
        self.base.request_redraw();
    }

    /// Draw a polyline or polygon path.
    pub fn draw_path(
        &mut self,
        points: Vec<Point>,
        closed: bool,
        color: Color,
        filled: bool,
        width: u32,
    ) {
        self.push_command(RenderCommand::DrawPath { points, closed, color, filled, width });
        self.base.request_redraw();
    }

    /// Returns the last tracked mouse position.
    pub fn last_mouse_pos(&self) -> Point {
        self.last_mouse_pos
    }
}

impl Widget for Canvas {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "mouse_pressed" => Some(EventSignalRef::mapped(
                "mouse_pressed",
                self.mouse_pressed_signal(),
                |value| {
                    let (position, button) = value;
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(format!("{},{}", position.x, position.y)),
                        CapabilityValue::UInt(*button as u64),
                    ]))
                },
            )),
            "mouse_released" => Some(EventSignalRef::mapped(
                "mouse_released",
                self.mouse_released_signal(),
                |value| {
                    let (position, button) = value;
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(format!("{},{}", position.x, position.y)),
                        CapabilityValue::UInt(*button as u64),
                    ]))
                },
            )),
            "mouse_moved" => Some(EventSignalRef::mapped("mouse_moved", &self.mouse_moved, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "double_clicked" => Some(EventSignalRef::mapped(
                "double_clicked",
                self.double_clicked_signal(),
                |value| {
                    let (position, button) = value;
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::String(format!("{},{}", position.x, position.y)),
                        CapabilityValue::UInt(*button as u64),
                    ]))
                },
            )),
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(format!("{},{}", v.x, v.y)),
            )),
            "layer_changed" => {
                Some(EventSignalRef::mapped("layer_changed", &self.layer_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "content_changed" => {
                Some(EventSignalRef::mapped("content_changed", &self.content_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "zoom_changed" => {
                Some(EventSignalRef::mapped("zoom_changed", &self.zoom_changed, |v| {
                    CapabilityValue::Float(*v as f64)
                }))
            }
            "command_picked" => {
                Some(EventSignalRef::mapped("command_picked", &self.command_picked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `Canvas`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, including
/// the `f32` → `f64` widening on read and the "write one half, keep the other"
/// behaviour of the `center_x` / `center_y` setters.
///
/// `WidgetKind::Canvas` is the kind the capability layer pairs with `MapView`, so
/// the `center_*` / `zoom` / `marker_count` names belong to that control, not to
/// this one. `canvas_capability` publishes an empty schema for `Canvas` itself,
/// so this contract inherits the shared four and owns nothing beyond them.
impl WidgetProperties for Canvas {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "command_count" => Ok(CapabilityValue::UInt(self.command_count() as u64)),
            "layer_count" => Ok(CapabilityValue::UInt(self.layer_count() as u64)),
            "anti_alias" => Ok(CapabilityValue::Bool(self.anti_alias())),
            "show_grid" => Ok(CapabilityValue::Bool(self.show_grid())),
            "grid_step" => Ok(CapabilityValue::UInt(self.grid_step() as u64)),
            "snap_to_grid" => Ok(CapabilityValue::Bool(self.snap_to_grid())),
            "zoom_range" => {
                let (min, max) = self.zoom_range();
                Ok(CapabilityValue::Tuple(crate::compat::Vec::from([
                    CapabilityValue::Float(min as f64),
                    CapabilityValue::Float(max as f64),
                ])))
            }
            "zoom_step" => Ok(CapabilityValue::Float(self.zoom_step() as f64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Derived from the recorded command list, which is mutated through the
            // drawing methods rather than by assigning a count.
            "command_count" | "layer_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "anti_alias" => {
                self.set_anti_alias(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            "show_grid" => {
                self.set_show_grid(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            "grid_step" => {
                let step = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_grid_step(step);
                Ok(())
            }
            "snap_to_grid" => {
                self.set_snap_to_grid(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            "zoom_range" => {
                let bounds = crate::widget::capability::coercion::expect_number_list(value)?;
                if bounds.len() != 2 {
                    return Err(CapabilityAccessError::TypeMismatch);
                }
                if self.set_zoom_range(bounds[0] as f32, bounds[1] as f32) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "zoom_step" => {
                let step = crate::widget::capability::coercion::expect_f32(value)?;
                if !step.is_finite() || step <= 1.0 {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                self.set_zoom_step(step);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "command_count",
            "layer_count",
            "anti_alias",
            "show_grid",
            "grid_step",
            "snap_to_grid",
            "zoom_range",
            "zoom_step",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `canvas` publishes.
    ///
    /// `clear` discards the recorded drawing commands and needs no argument, so it
    /// executes here. `set_zoom` and `set_center` belong to the `MapView` control
    /// that shares this kind, so they carry a value this contract has no writer for
    /// and are refused as [`CapabilityAccessError::OutOfRange`].
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_zoom" | "set_center" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Canvas {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();

        // The canvas surface and its frame are chrome, so they resolve explicit
        // style first, then the theme, and only then a literal. Without the theme
        // step the surface stayed white in both appearances, which made a light/dark
        // switch invisible.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("canvas");
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.2));

        // Draw canvas background
        context.face_with_gradient(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        if self.show_grid {
            let step = self.grid_step.max(1);
            let grid_color = border.with_alpha(96);
            let mut x = rect.x;
            while x <= rect.x + rect.width as i32 {
                context.draw_line(
                    Point::new(x, rect.y),
                    Point::new(x, rect.y + rect.height as i32),
                    grid_color,
                );
                x += step as i32;
            }
            let mut y = rect.y;
            while y <= rect.y + rect.height as i32 {
                context.draw_line(
                    Point::new(rect.x, y),
                    Point::new(rect.x + rect.width as i32, y),
                    grid_color,
                );
                y += step as i32;
            }
        }
        // Replay every visible layer in painting order. Borrowed: the layers are the
        // canvas's own content, so cloning each command per frame only duplicated its
        // text and point vectors. A command whose bounds lie outside the control is
        // culled: a drawing surface that has been panned keeps every off-screen mark,
        // and replaying them costs the rasteriser a full pass each for pixels nobody
        // can see.
        for layer in &self.layers {
            if !layer.visible {
                continue;
            }
            for stored in &layer.commands {
                if let Some(bounds) = command_bounds(&stored.command) {
                    if !rects_intersect(rect, bounds) {
                        continue;
                    }
                }
                context.execute_command_ref(&stored.command);
            }
        }
        // Draw border to make canvas area visible
        context.draw_rect(rect, border);
    }

    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

impl EventHandler for Canvas {
    fn handle_event(&mut self, event: &Event) {
        // Forwarded *before* the match, so the base records the pointer facts (`hovered`,
        // `grabbed`, focus) that `widget_state()` reads. Without it the theme's `"canvas:hover"`
        // key could never match, which is the same defect `list_view` and `data_grid` had: a
        // control whose `handle_event` replaces the base's rather than extending it.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(true);
                self.mouse_pressed.emit_at(*pos, *button);
                if let Some(index) = self.hit_test(*pos) {
                    self.command_picked.emit(index);
                }
            }
            Event::MouseRelease { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(false);
                self.mouse_released.emit_at(*pos, *button);
            }
            Event::MouseMove { pos } if self.geometry().contains(*pos) => {
                self.last_mouse_pos = *pos;
                self.mouse_moved.emit(*pos);
            }
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.double_clicked.emit_at(*pos, *button);
            }
            Event::Wheel { delta, .. } if self.zoom_enabled && delta.y != 0 => {
                // Zoom about the pointer, so the ink under the cursor stays under it.
                let anchor = self.last_mouse_pos;
                let factor = if delta.y > 0 { 1.0 / self.zoom_step } else { self.zoom_step };
                let previous = self.transform.scale;
                let scale = (previous * factor).clamp(self.zoom_min, self.zoom_max);
                if scale != previous {
                    let ratio = scale / previous;
                    self.transform.offset = Point::new(
                        anchor.x
                            - ((anchor.x - self.transform.offset.x) as f32 * ratio).round() as i32,
                        anchor.y
                            - ((anchor.y - self.transform.offset.y) as f32 * ratio).round() as i32,
                    );
                    self.transform.scale = scale;
                    self.zoom_changed.emit(scale);
                    self.base.request_redraw();
                }
            }
            Event::ContextMenu { pos } => {
                self.context_menu_requested.emit(*pos);
            }
            Event::KeyPress { key, modifiers } => {
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) {
                    match crate::shortcut::Key::from_key_code(*key) {
                        Some(crate::shortcut::Key::Z) => {
                            self.undo();
                        }
                        Some(crate::shortcut::Key::Y) => {
                            self.redo();
                        }
                        _ => {}
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

/// Rounds one coordinate to the nearest grid multiple.
fn snap_scalar(value: i32, step: i32) -> i32 {
    let step = step.max(1);
    (value as f32 / step as f32).round() as i32 * step
}

/// Snaps a point to the canvas grid.
fn snap_point(point: Point, step: i32) -> Point {
    Point::new(snap_scalar(point.x, step), snap_scalar(point.y, step))
}

/// Snaps a rectangle's origin and size to the canvas grid.
fn snap_rect(rect: Rect, step: i32) -> Rect {
    let step = step.max(1);
    Rect::new(
        snap_scalar(rect.x, step),
        snap_scalar(rect.y, step),
        snap_scalar(rect.width as i32, step).max(1) as u32,
        snap_scalar(rect.height as i32, step).max(1) as u32,
    )
}

/// Applies grid snapping to the coordinates a command carries.
fn snap_command(command: RenderCommand, step: i32) -> RenderCommand {
    match command {
        RenderCommand::FillRect { rect, color } => {
            RenderCommand::FillRect { rect: snap_rect(rect, step), color }
        }
        RenderCommand::DrawRect { rect, color } => {
            RenderCommand::DrawRect { rect: snap_rect(rect, step), color }
        }
        RenderCommand::FillRoundedRect { rect, radius, color } => {
            RenderCommand::FillRoundedRect { rect: snap_rect(rect, step), radius, color }
        }
        RenderCommand::DrawLine { from, to, color } => RenderCommand::DrawLine {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
        },
        RenderCommand::DrawLineAA { from, to, color } => RenderCommand::DrawLineAA {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
        },
        RenderCommand::DrawLineStroke { from, to, color, width } => RenderCommand::DrawLineStroke {
            from: snap_point(from, step),
            to: snap_point(to, step),
            color,
            width,
        },
        RenderCommand::FillCircle { center, radius, color } => {
            RenderCommand::FillCircle { center: snap_point(center, step), radius, color }
        }
        RenderCommand::FillCircleAA { center, radius, color } => {
            RenderCommand::FillCircleAA { center: snap_point(center, step), radius, color }
        }
        RenderCommand::DrawCircle { center, radius, color } => {
            RenderCommand::DrawCircle { center: snap_point(center, step), radius, color }
        }
        RenderCommand::DrawCircleStroke { center, radius, color, width } => {
            RenderCommand::DrawCircleStroke {
                center: snap_point(center, step),
                radius,
                color,
                width,
            }
        }
        RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
            RenderCommand::DrawArc {
                center: snap_point(center, step),
                radius,
                start_angle,
                end_angle,
                color,
                filled,
            }
        }
        RenderCommand::DrawPath { points, closed, color, filled, width } => {
            RenderCommand::DrawPath {
                points: points.into_iter().map(|point| snap_point(point, step)).collect(),
                closed,
                color,
                filled,
                width,
            }
        }
        RenderCommand::DrawText { origin, text, font, color, alignment } => {
            RenderCommand::DrawText {
                origin: snap_point(origin, step),
                text,
                font,
                color,
                alignment,
            }
        }
        other => other,
    }
}

/// Replaces anti-aliased variants with their plain counterparts.
///
/// The canvas-wide switch must win over a per-command variant, otherwise a
/// caller that disabled anti-aliasing would still get it on every command that
/// happened to use the `*AA` constructor.
fn without_anti_aliasing(command: RenderCommand) -> RenderCommand {
    match command {
        RenderCommand::DrawLineAA { from, to, color } => {
            RenderCommand::DrawLine { from, to, color }
        }
        RenderCommand::FillCircleAA { center, radius, color } => {
            RenderCommand::FillCircle { center, radius, color }
        }
        other => other,
    }
}

/// Converts a `(position, button)` pointer payload into the declared capability value.
///
/// The schema declares a mixed pair: the position travels as its `"x,y"` spelling (a point has
/// no `PropertyValueKind` of its own) and the button as `UInt`. A `Tuple` keeps both components
/// individually readable, which a single formatted string would not.
/// The axis-aligned bounds of a command, when it has any.
fn command_bounds(command: &RenderCommand) -> Option<Rect> {
    match command {
        RenderCommand::FillRect { rect, .. } | RenderCommand::DrawRect { rect, .. } => Some(*rect),
        RenderCommand::FillRoundedRect { rect, .. } => Some(*rect),
        RenderCommand::FillCircle { center, radius, .. }
        | RenderCommand::FillCircleAA { center, radius, .. }
        | RenderCommand::DrawCircle { center, radius, .. }
        | RenderCommand::DrawCircleStroke { center, radius, .. } => Some(Rect::new(
            center.x - *radius as i32,
            center.y - *radius as i32,
            radius.saturating_mul(2).max(1),
            radius.saturating_mul(2).max(1),
        )),
        RenderCommand::DrawLine { from, to, .. }
        | RenderCommand::DrawLineAA { from, to, .. }
        | RenderCommand::DrawLineStroke { from, to, .. } => Some(Rect::new(
            from.x.min(to.x),
            from.y.min(to.y),
            (from.x - to.x).unsigned_abs().max(1),
            (from.y - to.y).unsigned_abs().max(1),
        )),
        RenderCommand::DrawPath { points, .. } => {
            let first = points.first()?;
            let (mut left, mut top, mut right, mut bottom) = (first.x, first.y, first.x, first.y);
            for point in points {
                left = left.min(point.x);
                top = top.min(point.y);
                right = right.max(point.x);
                bottom = bottom.max(point.y);
            }
            Some(Rect::new(
                left,
                top,
                (right - left).unsigned_abs().max(1),
                (bottom - top).unsigned_abs().max(1),
            ))
        }
        RenderCommand::DrawArc { center, radius, .. } => Some(Rect::new(
            center.x - *radius as i32,
            center.y - *radius as i32,
            radius.saturating_mul(2).max(1),
            radius.saturating_mul(2).max(1),
        )),
        _ => None,
    }
}

/// Whether two rectangles overlap.
fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width as i32
        && b.x < a.x + a.width as i32
        && a.y < b.y + b.height as i32
        && b.y < a.y + a.height as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Font, Rect};
    use crate::widget::svg::render_to_svg;

    #[test]
    fn canvas_creation_defaults() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.command_count(), 0);
        assert_eq!(canvas.geometry(), Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.kind(), WidgetKind::Canvas);
    }

    #[test]
    fn canvas_fill_rect_adds_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_line_and_circle() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        canvas.fill_circle(Point::new(50, 50), 10, Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_clear_removes_all_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(0, 0, 100, 100), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        assert_eq!(canvas.command_count(), 2);
        canvas.clear();
        assert_eq!(canvas.command_count(), 0);
    }

    #[test]
    fn canvas_draw_path_stores_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let points =
            vec![Point::new(0, 0), Point::new(100, 0), Point::new(100, 100), Point::new(0, 100)];
        canvas.draw_path(points, true, Color::BLUE, false, 2);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_text_stores_command() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_text(Point::new(10, 20), "Hello", &Font::default_ui(), Color::BLACK);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_draw_produces_svg_output() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::BLUE);
        canvas.draw_line(Point::new(0, 0), Point::new(100, 100), Color::RED);
        let svg = render_to_svg(&mut canvas);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    #[test]
    fn canvas_signal_accessors() {
        let canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let _ = &canvas.mouse_pressed;
        let _ = &canvas.mouse_released;
        let _ = &canvas.mouse_moved;
        let _ = &canvas.double_clicked;
    }

    #[test]
    fn canvas_last_mouse_position_tracked() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
        canvas.handle_event(&Event::MouseMove { pos: Point::new(42, 73) });
        assert_eq!(canvas.last_mouse_pos(), Point::new(42, 73));
    }

    #[test]
    fn canvas_disabled_state_blocks_events() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_enabled(false);
        // Events should be ignored when disabled
        canvas.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: 1,
            modifiers: 0,
        });
        // Signal should NOT have been emitted (disabled)
        // We just verify no panic and state unchanged
        assert_eq!(canvas.last_mouse_pos(), Point::new(0, 0));
    }

    #[test]
    fn canvas_double_click_emits_signal() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.handle_event(&Event::MouseDoubleClick { pos: Point::new(50, 50), button: 1 });
        assert_eq!(canvas.last_mouse_pos(), Point::new(50, 50));
    }

    #[test]
    fn canvas_draw_arc_and_draw_circle_stroke() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_arc(Point::new(100, 50), 30, 0.0, std::f32::consts::PI, Color::RED, false);
        canvas.draw_circle_stroke(Point::new(50, 50), 20, Color::BLUE, 3);
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_fill_rounded_rect_and_line_aa() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rounded_rect(Rect::new(5, 5, 50, 50), 8, Color::rgba(255, 0, 0, 128));
        canvas.draw_line_aa(Point::new(0, 0), Point::new(100, 50), Color::GREEN);
        assert_eq!(canvas.command_count(), 2);
    }
    /// The canvas forwards pointer events to its base, so hover becomes a state it can report.
    ///
    /// Regression: `handle_event` *replaced* the base's rather than extending it, so
    /// `MouseEnter`/`MouseLeave` never reached `BaseWidget`, `widget_state()` could never be
    /// `Hover`, and the theme's `"canvas:hover"` key was dead — the same defect `list_view` and
    /// `data_grid` had. The assertion is the state the contract publishes, not a signal.
    #[test]
    fn the_canvas_reports_hover_because_it_forwards_to_its_base() {
        use crate::style::WidgetState;

        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.widget_state(), WidgetState::Normal);

        canvas.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert_eq!(
            canvas.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );

        canvas.handle_event(&Event::MouseLeave { pos: Point::new(500, 500) });
        assert_eq!(canvas.widget_state(), WidgetState::Normal, "and that it left");
    }

    #[test]
    fn canvas_layers_order_visibility_and_removal() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.begin_layer("background");
        canvas.fill_rect(Rect::new(0, 0, 50, 50), Color::BLUE);
        canvas.begin_layer("foreground");
        canvas.fill_rect(Rect::new(10, 10, 50, 50), Color::RED);
        assert_eq!(canvas.layer_count(), 2);
        assert_eq!(canvas.command_count(), 2);

        // The topmost (foreground) command is picked first.
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(1));
        canvas.set_layer_visible(1, false);
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(0), "a hidden layer is skipped");

        assert!(canvas.set_layer_visible(1, true));
        assert!(canvas.move_layer(1, 0));
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(1), "the background is on top now");
        assert!(canvas.remove_layer(0));
        assert_eq!(canvas.layer_count(), 1);
        assert_eq!(canvas.command_count(), 1);
    }

    #[test]
    fn canvas_groups_are_one_undo_step() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert!(canvas.begin_group());
        canvas.draw_line(Point::new(0, 0), Point::new(5, 5), Color::RED);
        canvas.draw_line(Point::new(5, 5), Point::new(10, 0), Color::RED);
        assert!(canvas.end_group());
        assert_eq!(canvas.undo_count(), 1, "a stroke is one history step");
        assert!(canvas.undo());
        assert_eq!(canvas.command_count(), 0);
        assert!(canvas.redo());
        assert_eq!(canvas.command_count(), 2);
    }

    #[test]
    fn canvas_redo_history_is_bounded() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_command_limit(3);
        for index in 0..6 {
            canvas.fill_rect(Rect::new(index * 2, 0, 4, 4), Color::BLUE);
        }
        assert_eq!(canvas.command_count(), 3);
        while canvas.undo() {}
        assert!(canvas.redo_count() <= 3, "the redo stack obeys the command limit");
    }

    #[test]
    fn canvas_global_anti_alias_degrades_aa_commands() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_anti_alias(false);
        canvas.draw_line_aa(Point::new(0, 0), Point::new(10, 10), Color::RED);
        canvas.fill_circle_aa(Point::new(20, 20), 5, Color::BLUE);
        assert!(matches!(canvas.layers[0].commands[0].command, RenderCommand::DrawLine { .. }));
        assert!(matches!(canvas.layers[0].commands[1].command, RenderCommand::FillCircle { .. }));
    }

    #[test]
    fn canvas_snapping_rounds_stored_coordinates() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_grid_step(10);
        canvas.set_snap_to_grid(true);
        canvas.fill_rect(Rect::new(12, 18, 5, 5), Color::BLUE);
        match canvas.layers[0].commands[0].command {
            RenderCommand::FillRect { rect, .. } => {
                assert_eq!((rect.x, rect.y), (10, 20));
            }
            _ => panic!("expected a filled rectangle"),
        }
    }

    #[test]
    fn canvas_export_png_produces_a_png() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 40, 30));
        canvas.fill_rect(Rect::new(5, 5, 10, 10), Color::RED);
        let bytes = canvas.export_png(40, 30, 1.0).expect("a valid export");
        assert!(bytes.starts_with(b"\x89PNG"));
    }

    #[test]
    fn canvas_new_properties_round_trip() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert_eq!(canvas.get("anti_alias").unwrap(), CapabilityValue::Bool(true));
        canvas.set("anti_alias", CapabilityValue::Bool(false)).unwrap();
        assert!(!canvas.anti_alias());
        canvas.set("show_grid", CapabilityValue::Bool(true)).unwrap();
        assert!(canvas.show_grid());
        canvas
            .set(
                "zoom_range",
                CapabilityValue::Tuple(crate::compat::Vec::from([
                    CapabilityValue::Float(0.5),
                    CapabilityValue::Float(8.0),
                ])),
            )
            .unwrap();
        assert_eq!(canvas.zoom_range(), (0.5, 8.0));
    }
}
