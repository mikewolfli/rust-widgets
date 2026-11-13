// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas widget for custom drawing operations.
//!
//! The `Canvas` widget provides a drawing surface where users can push
//! rendering commands (lines, circles, rectangles, paths, text) and have
//! them replayed during the `draw()` pass. It also emits mouse interaction
//! signals so callers can respond to clicks, drags, and hover events on
//! the drawing area.

// The canvas-only type declarations and the additive C-11'/RW-29..RW-33 seams live in the
// `canvas/ext.rs` file. `#[path]` declares it as a child of `widget`, which is what lets it read
// the canvas's private state without widening a field; the file stays beside `widget.rs` so the
// canvas module's files are one flat set (principle #9).
#[path = "ext.rs"]
mod ext;

// The command mapping (transform, grid snapping, bounds, culling) lives in `canvas/command.rs`;
// `#[path]` keeps the canvas files one flat set while the implementation stays under the
// 2000-line structure limit (principle #9).
#[path = "command.rs"]
mod command;

use command::{
    color_key, command_bounds, logical_viewport, rects_intersect, transform_command,
    without_anti_aliasing,
};

pub use ext::{
    CanvasDropEvent, CanvasDropHandler, CanvasImage, CanvasRulerTick, CanvasTag, CanvasTransform,
    GridSpace, GuideAxis,
};

use crate::compat::Arc;
use crate::core::{Color, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::{PaintBackend, RenderCommand, RenderContext};
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::{LayerEntry, LayerPanel};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::collections::VecDeque;
use core::cell::{Ref, RefCell};
use ext::{
    CanvasHitCache, CanvasHitEntry, CanvasLayer, CommandChunk, StoredCommand, UndoneCommand,
};

/// One pointer event that carries both the legacy payload-free signal and the typed payload.
///
/// Kept as a re-export of [`crate::widget::pointer_event::PointerEvent`] so the historical path
/// `canvas::CanvasPointerEvent` keeps working while every drawing surface shares one definition
/// (principle #54).
pub use crate::widget::pointer_event::PointerEvent as CanvasPointerEvent;

/// Hands out the identities the render layer cache uses to recognise an unchanged layer.
///
/// Globally unique rather than per canvas: the cache belongs to the surface, so two canvases
/// that both started at `1` would hit each other's cached pixels.
pub(super) fn next_layer_cache_key() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// How many commands one replay chunk covers.
pub(super) const COMMAND_CHUNK_SIZE: usize = 32;

/// Per-axis grid coordinates, reused across frames (W5-36).
type GridScratch = RefCell<(Vec<(i32, i32)>, Vec<(i32, i32)>)>;

/// Canvas widget for custom drawing operations.
pub struct Canvas {
    base: BaseWidget,
    /// Layers in painting order (back to front).
    layers: Vec<CanvasLayer>,
    /// An optional layer panel kept in step with this canvas's stack (gap 5).
    layer_panel: Option<LayerPanel>,
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
    /// Incrementally maintained total of `commands.len()` across every layer.
    ///
    /// `command_count` used to sum the whole layer stack on every push, so a stroke of `n`
    /// commands cost `O(n²)` (SP-P51). Six sites add or subtract; nothing else may touch the
    /// layer command lists.
    command_total: usize,
    /// Layer-cache identity of the painted background grid; a fresh key rebuilds it.
    grid_cache_key: u64,
    /// Reused grid coordinate lists, one per axis (W5-36).
    ///
    /// `draw_grid_lines` rebuilt both `(surface, model)` lists on every frame, and each entry is a
    /// multiple of the grid step, so the lists are derived state: the two buffers keep their
    /// capacity and `clear()` is the only reuse protocol, the same discipline
    /// `viewport_lines_scratch` uses in the code editor.
    grid_scratch: GridScratch,
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
    /// Emitted with `(flattened index, tag)` when a press picks a tagged command (RW-29).
    ///
    /// Reachable through [`Canvas::command_picked_tagged_signal`]. It stays out of the public
    /// field set because the canvas capability's event table cannot be extended from this module;
    /// a Rust host subscribes to the accessor, and the existing `command_picked` keeps working.
    command_picked_tagged: Signal1<(usize, CanvasTag)>,
    /// Emitted with the payload and both coordinate forms of an accepted drop (C-11').
    ///
    /// Reachable through [`Canvas::drop_received_signal`].
    drop_received: Signal1<CanvasDropEvent>,
    /// Host sink consulted for the drop effect; `None` accepts drops as copies.
    drop_handler: Option<CanvasDropHandler>,
    /// The tag commands pushed inside the open group carry, when one was supplied.
    pending_group_tag: Option<CanvasTag>,
    /// Overlay/decoration commands, excluded from undo, hit testing and export (RW-30).
    overlay_commands: VecDeque<StoredCommand>,
    /// Whether commands pushed now belong to the overlay layer.
    overlay_open: bool,
    /// True while an export replays the scene, so decorations stay out of the file.
    suppress_overlay: bool,
    /// The space `grid_step` is interpreted in (RW-31).
    grid_space: GridSpace,
    /// Whether the ruler strips are painted along the top and left edges (RW-32).
    rulers_visible: bool,
    /// The model-unit spacing between ruler ticks, or `0` to follow `grid_step` (RW-124).
    ruler_unit: u32,
    /// Alignment guides, in model coordinates (RW-32).
    guides: Vec<(GuideAxis, i32)>,
    /// Host-requested cursor shape, or `None` for the drawing-surface default (RW-33).
    cursor_icon_override: Option<crate::platform::CursorIcon>,
    /// Whether a middle-button or space-drag pan is allowed (RW-33).
    pan_enabled: bool,
    /// Whether a pan gesture is in progress.
    panning: bool,
    /// Whether the space bar is currently held.
    space_held: bool,
    /// Pointer position of the previous pan update.
    pan_last: Point,
    /// Bumped whenever the visible command set changes; keys the hit-test index (SP-111).
    hit_revision: u64,
    /// The `(flattened index, logical bounds, tag)` rows of every visible command.
    hit_cache: RefCell<Option<CanvasHitCache>>,
}

impl Canvas {
    /// Creates a new canvas widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Canvas, geometry, "Canvas"),
            layers: Vec::new(),
            layer_panel: None,
            current_layer: None,
            next_layer_id: 1,
            next_command_seq: 1,
            transform: CanvasTransform::default(),
            transform_stack: Vec::new(),
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            pending_group: None,
            command_limit: 100_000,
            command_total: 0,
            grid_cache_key: next_layer_cache_key(),
            grid_scratch: RefCell::new((Vec::new(), Vec::new())),
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
            command_picked_tagged: Signal1::new(),
            drop_received: Signal1::new(),
            drop_handler: None,
            pending_group_tag: None,
            overlay_commands: VecDeque::new(),
            overlay_open: false,
            suppress_overlay: false,
            grid_space: GridSpace::Screen,
            rulers_visible: false,
            ruler_unit: 0,
            guides: Vec::new(),
            cursor_icon_override: None,
            pan_enabled: false,
            panning: false,
            space_held: false,
            pan_last: Point::new(0, 0),
            hit_revision: 0,
            hit_cache: RefCell::new(None),
        }
    }

    /// Marks the hit-test index stale after the visible command set changes.
    fn invalidate_hit_cache(&mut self) {
        self.hit_revision = self.hit_revision.wrapping_add(1);
    }

    /// The cached `(index, bounds, tag)` rows of every visible command.
    fn hit_entries(&self) -> Ref<'_, Vec<CanvasHitEntry>> {
        if !self
            .hit_cache
            .borrow()
            .as_ref()
            .is_some_and(|(revision, _)| *revision == self.hit_revision)
        {
            let mut entries = Vec::new();
            let mut start = 0usize;
            for layer in &self.layers {
                if layer.visible {
                    for (index, stored) in layer.commands.iter().enumerate() {
                        if let Some(bounds) = command_bounds(&stored.command) {
                            entries.push((start + index, bounds, stored.tag));
                        }
                    }
                }
                start += layer.commands.len();
            }
            *self.hit_cache.borrow_mut() = Some((self.hit_revision, entries));
        }
        Ref::map(self.hit_cache.borrow(), |cache| {
            &cache.as_ref().expect("the hit index was just built").1
        })
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
        self.command_total = 0;
        self.invalidate_hit_cache();
        self.current_layer = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.pending_group = None;
        self.pending_group_tag = None;
        self.overlay_commands.clear();
        self.overlay_open = false;
        self.guides.clear();
        self.content_changed.emit(0);
        self.layer_changed.emit(0);
        self.sync_layer_panel();
        self.base.request_redraw();
    }

    /// Returns the number of stored drawing commands across every layer.
    pub fn command_count(&self) -> usize {
        self.command_total
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
            self.sync_layer_panel();
            return index;
        }
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(CanvasLayer {
            id,
            name,
            visible: true,
            commands: VecDeque::new(),
            cache_key: next_layer_cache_key(),
            revision: 0,
            chunks: RefCell::new(None),
        });
        self.current_layer = Some(id);
        self.layer_changed.emit(self.layers.len());
        self.sync_layer_panel();
        self.base.request_redraw();
        self.layers.len() - 1
    }

    /// Attaches a layer panel and fills it with the current stack.
    ///
    /// Returns whether a panel is attached afterwards; `None` detaches it. The host keeps the
    /// panel's own gestures (`layer_selected`, `visibility_toggled`) and applies them through
    /// [`Self::set_layer_visible`] / [`Self::move_layer`]; the canvas writes the model back into
    /// the panel on every structural change, so the two cannot disagree (gap 5).
    pub fn attach_layer_panel(&mut self, panel: Option<LayerPanel>) -> bool {
        self.layer_panel = panel;
        self.sync_layer_panel();
        self.layer_panel.is_some()
    }

    /// The attached layer panel, if any.
    pub fn layer_panel(&self) -> Option<&LayerPanel> {
        self.layer_panel.as_ref()
    }

    /// The attached layer panel for editing, if any.
    pub fn layer_panel_mut(&mut self) -> Option<&mut LayerPanel> {
        self.layer_panel.as_mut()
    }

    /// Pushes the canvas layer stack into the attached panel.
    fn sync_layer_panel(&mut self) {
        let entries: Vec<LayerEntry> = self
            .layers
            .iter()
            .map(|layer| LayerEntry::new(layer.id, layer.name.clone()).with_visible(layer.visible))
            .collect();
        let current_layer = self.current_layer;
        if let Some(panel) = self.layer_panel.as_mut() {
            panel.set_entries(entries);
            // The canvas's current layer is the panel's selection; the silent setter keeps a
            // model→view write from being reported back to the host as a user gesture.
            panel.set_selected_layer(current_layer);
        }
    }

    /// Shows or hides the layer at `index`, returning whether it exists.
    pub fn set_layer_visible(&mut self, index: usize, visible: bool) -> bool {
        let Some(layer) = self.layers.get_mut(index) else {
            return false;
        };
        if layer.visible == visible {
            return true;
        }
        layer.visible = visible;
        self.invalidate_hit_cache();
        self.layer_changed.emit(self.layers.len());
        self.sync_layer_panel();
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
        self.invalidate_hit_cache();
        self.layer_changed.emit(self.layers.len());
        self.sync_layer_panel();
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
        let removed_len = self.layers[index].commands.len();
        let layer = self.layers.remove(index);
        self.command_total = self.command_total.saturating_sub(removed_len);
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
        self.invalidate_hit_cache();
        self.layer_changed.emit(self.layers.len());
        self.content_changed.emit(count);
        self.sync_layer_panel();
        self.base.request_redraw();
        true
    }

    /// Returns the transform applied to newly added commands.
    pub fn transform(&self) -> CanvasTransform {
        self.transform
    }

    /// Replaces the transform applied to newly added commands.
    ///
    /// Every layer's replay cache is dropped: the transform is part of the picture, so a cached
    /// command list mapped through the previous value would paint the old scale.
    pub fn set_transform(&mut self, transform: CanvasTransform) {
        if self.transform == transform {
            return;
        }
        let scale_changed = self.transform.scale != transform.scale;
        self.transform = transform;
        self.invalidate_replay_caches();
        if scale_changed {
            self.zoom_changed.emit(self.transform.scale);
        }
        self.base.request_redraw();
    }

    /// Drops every cached replay mapping and gives each layer a fresh render-cache identity.
    fn invalidate_replay_caches(&mut self) {
        for layer in &mut self.layers {
            for stored in &mut layer.commands {
                stored.replay = None;
            }
            layer.cache_key = next_layer_cache_key();
        }
        self.grid_cache_key = next_layer_cache_key();
    }

    /// Saves the current transform, restoring it with [`Self::pop_transform`].
    pub fn push_transform(&mut self) {
        self.transform_stack.push(self.transform);
    }

    /// Restores the transform saved by the matching [`Self::push_transform`].
    pub fn pop_transform(&mut self) {
        if let Some(transform) = self.transform_stack.pop() {
            if self.transform != transform {
                self.transform = transform;
                self.invalidate_replay_caches();
                self.zoom_changed.emit(transform.scale);
            }
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
                        // The mapped copy was built from the anti-aliased form.
                        stored.replay = None;
                        stored
                    })
                    .collect();
                layer.touch();
            }
            // Decorations and the redo history are painted/replayed too, so they follow the same
            // global switch. Rewriting only the layers left a restored command anti-aliased while
            // the rest of the canvas was not.
            let overlay = core::mem::take(&mut self.overlay_commands);
            self.overlay_commands = overlay
                .into_iter()
                .map(|mut stored| {
                    stored.command = without_anti_aliasing(stored.command);
                    stored.replay = None;
                    stored
                })
                .collect();
            let redo = core::mem::take(&mut self.redo_stack);
            self.redo_stack = redo
                .into_iter()
                .map(|step| {
                    step.into_iter()
                        .map(|mut undone| {
                            undone.command = without_anti_aliasing(undone.command);
                            undone
                        })
                        .collect()
                })
                .collect();
            self.invalidate_hit_cache();
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
        self.grid_cache_key = next_layer_cache_key();
        self.base.request_redraw();
    }

    /// Returns the grid cell size in pixels.
    pub fn grid_step(&self) -> u32 {
        self.grid_step
    }

    /// Sets the grid cell size, clamped to at least one pixel.
    ///
    /// The upper clamp is `i32::MAX`: the paint and the ruler loop step with a signed `i32`, so a
    /// larger `u32` would be cast to a negative step and the loop would walk backwards forever — a
    /// hang in release and an overflow panic in debug.
    pub fn set_grid_step(&mut self, step: u32) {
        let step = step.clamp(1, i32::MAX as u32);
        if self.grid_step == step {
            return;
        }
        self.grid_step = step;
        self.grid_cache_key = next_layer_cache_key();
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
            self.invalidate_replay_caches();
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
        let before = self.command_count();
        let overlay_before = self.overlay_commands.len();
        self.trim_to_limit();
        while self.overlay_commands.len() > self.command_limit {
            self.overlay_commands.pop_front();
        }
        while self.undo_stack.len() > self.command_limit {
            self.undo_stack.pop_front();
        }
        while self.redo_stack.len() > self.command_limit {
            self.redo_stack.pop_front();
        }
        // A smaller limit can evict content and decorations. That is an observable change to the
        // document, so it is published and repainted exactly like an undo's own eviction; without
        // this, hosts kept a stale picture and a stale `command_count` mirror.
        if self.command_count() != before || self.overlay_commands.len() != overlay_before {
            self.invalidate_hit_cache();
            self.content_changed.emit(self.command_count());
            self.base.request_redraw();
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
        self.begin_group_impl(None)
    }

    /// Begins a command group whose commands carry `tag` as their host identity (RW-29).
    ///
    /// The tag travels with every command pushed before [`Self::end_group`] and survives redo,
    /// so [`Self::hit_test_tagged`] can answer which node is under the pointer without the host
    /// maintaining a parallel index. A second group cannot nest; the call answers `false` exactly
    /// like [`Self::begin_group`].
    pub fn begin_group_with_tag(&mut self, tag: CanvasTag) -> bool {
        self.begin_group_impl(Some(tag))
    }

    fn begin_group_impl(&mut self, tag: Option<CanvasTag>) -> bool {
        if self.pending_group.is_some() {
            return false;
        }
        self.pending_group = Some(Vec::new());
        self.pending_group_tag = tag;
        true
    }

    /// Ends the open command group, returning whether it contained any command.
    pub fn end_group(&mut self) -> bool {
        let Some(group) = self.pending_group.take() else {
            return false;
        };
        self.pending_group_tag = None;
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
            let Some(step) = self.undo_stack.pop_back() else {
                return false;
            };
            let mut removed = Vec::new();
            // Undo the newest command of the step first, so a grouped stroke comes
            // apart in the reverse order it was drawn.
            for (layer_id, seq) in step.into_iter().rev() {
                if let Some(stored) = self.take_command(layer_id, seq) {
                    removed.push(UndoneCommand {
                        layer_id,
                        command: stored.command,
                        tag: stored.tag,
                        overlay: stored.overlay,
                    });
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
            let Some(step) = self.redo_stack.pop_back() else {
                return false;
            };
            let mut restored = Vec::new();
            for undone in step {
                let Some(layer_index) = self.layer_index_by_id(undone.layer_id) else {
                    continue;
                };
                let seq = self.next_command_seq;
                self.next_command_seq += 1;
                self.layers[layer_index].commands.push_back(StoredCommand {
                    seq,
                    command: undone.command,
                    replay: None,
                    tag: undone.tag,
                    overlay: undone.overlay,
                });
                self.layers[layer_index].touch();
                self.command_total = self.command_total.saturating_add(1);
                restored.push((undone.layer_id, seq));
            }
            if restored.is_empty() {
                continue;
            }
            self.invalidate_hit_cache();
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
            cache_key: next_layer_cache_key(),
            revision: 0,
            chunks: RefCell::new(None),
        });
        self.current_layer = Some(id);
        self.layer_changed.emit(self.layers.len());
        self.layers.len() - 1
    }

    /// Removes one command by its layer identity and sequence number.
    fn take_command(&mut self, layer_id: u64, seq: u64) -> Option<StoredCommand> {
        let layer_index = self.layer_index_by_id(layer_id)?;
        let commands = &mut self.layers[layer_index].commands;
        let index = commands.iter().position(|stored| stored.seq == seq)?;
        let removed = commands.remove(index);
        if removed.is_some() {
            self.command_total = self.command_total.saturating_sub(1);
            self.layers[layer_index].touch();
            self.invalidate_hit_cache();
        }
        removed
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
            let Some(layer_index) = self.oldest_command_layer() else {
                break;
            };
            let layer_id = self.layers[layer_index].id;
            let Some(removed) = self.layers[layer_index].commands.pop_front() else {
                break;
            };
            self.layers[layer_index].touch();
            self.invalidate_hit_cache();
            self.command_total = self.command_total.saturating_sub(1);
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

    /// Applies the snapping and anti-aliasing policy to `command` and stores it in the current
    /// layer.
    ///
    /// The canvas transform is deliberately **not** applied here: commands keep the logical
    /// coordinates they were drawn in, and the transform is applied at replay, export and hit
    /// test. Baking it in at push time froze every earlier command at the scale it was drawn at,
    /// so zooming changed the reported scale and nothing on the surface (SP-P52).
    fn push_command(&mut self, command: RenderCommand) {
        let mut command = command;
        // Decorations are not content: they neither snap to the drawing grid nor join the
        // undo history, so a guide placed between two strokes cannot move one of them.
        let overlay = self.overlay_open;
        if !overlay && self.snap_to_grid {
            command = self.snap_command_in_space(command);
        }
        if !self.anti_alias {
            command = without_anti_aliasing(command);
        }

        let seq = self.next_command_seq;
        self.next_command_seq += 1;
        if overlay {
            self.overlay_commands.push_back(StoredCommand {
                seq,
                command,
                replay: None,
                tag: None,
                overlay: true,
            });
            // Decorations obey the same memory bound as content, so a host that rebuilds them
            // every frame cannot grow the list without limit.
            while self.overlay_commands.len() > self.command_limit {
                self.overlay_commands.pop_front();
            }
            self.base.request_redraw();
            return;
        }

        let layer_index = self.ensure_layer();
        let layer_id = self.layers[layer_index].id;
        let tag = self.pending_group_tag;
        self.layers[layer_index].commands.push_back(StoredCommand {
            seq,
            command,
            replay: None,
            tag,
            overlay: false,
        });
        self.layers[layer_index].touch();
        self.invalidate_hit_cache();
        self.command_total = self.command_total.saturating_add(1);
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
                    // The export goes through the same transform the paint does, so what the
                    // caller sees on the surface is what the file contains (SP-P52).
                    let mapped = transform_command(&stored.command, self.transform);
                    context.execute_command(mapped);
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
    ///
    /// The pointer is mapped back through the canvas transform and compared against the
    /// commands' logical geometry, which is the same geometry the caller drew. The walk is
    /// allocation-free: the flattened index is accumulated from the end of the layer stack
    /// rather than from a prebuilt offset table (SP-P53).
    pub fn hit_test(&self, point: Point) -> Option<usize> {
        let local = self.to_drawing_space(point);
        // The bounds are cached with the visible command set, so a press no longer walks every
        // path command to rebuild them; the vector is searched back-to-front for the topmost hit
        // (SP-111).
        self.hit_entries()
            .iter()
            .rev()
            .find(|(_, bounds, _)| bounds.contains_point(local))
            .map(|(index, _, _)| *index)
    }

    /// Returns the flattened index and host tag of the topmost **tagged** command under `point`.
    ///
    /// Untagged commands are skipped: the tagged query answers "which node is here", and a
    /// decoration or an untagged stroke has no node identity to give. A tagged command is one
    /// pushed inside [`Self::begin_group_with_tag`], and the tag survives redo.
    pub fn hit_test_tagged(&self, point: Point) -> Option<(usize, CanvasTag)> {
        let local = self.to_drawing_space(point);
        self.hit_entries().iter().rev().find_map(|(index, bounds, tag)| {
            tag.as_ref().filter(|_| bounds.contains_point(local)).map(|tag| (*index, *tag))
        })
    }

    /// Maps a window-space point into the space this canvas stores its commands in.
    ///
    /// Commands are in the canvas's **own** space — `(0, 0)` is its top-left corner — which is the
    /// same space every `RenderContext` primitive uses. A pointer event carries window
    /// coordinates, so the control's origin is subtracted once here and the zoom/pan transform is
    /// undone. Without the subtraction the hit test compared window points against local command
    /// bounds, so a canvas mounted away from the frame origin reported a miss for every mark.
    fn to_drawing_space(&self, point: Point) -> Point {
        let rect = self.geometry();
        self.transform.inverse_point(Point::new(point.x - rect.x, point.y - rect.y))
    }

    /// Maps a window point into the control's own surface space, without undoing the transform.
    ///
    /// The wheel anchor and a drop's model coordinate need the surface-relative pointer but not
    /// the inverse zoom, so they cannot reuse [`Self::to_drawing_space`] whole.
    fn local_pointer(&self, point: Point) -> Point {
        let rect = self.geometry();
        Point::new(point.x - rect.x, point.y - rect.y)
    }

    /// Exports the canvas to an SVG document.
    ///
    /// Uses the crate's shared SVG backend, so the export and the on-screen paint read the same
    /// command list rather than a second renderer that could drift. Decorations (overlay
    /// commands, guides and rulers) are suppressed for the replay: they are screen furniture,
    /// not document content, so the file equals the drawing.
    pub fn export_svg(&mut self) -> String {
        self.suppress_overlay = true;
        let svg = crate::widget::svg::render_to_svg(self);
        self.suppress_overlay = false;
        svg
    }

    // ── Drawing API ──────────────────────────────────────────────────────
    //
    // Coordinates are the canvas's **own**: `(0, 0)` is its top-left corner, exactly as every
    // `RenderContext` primitive is interpreted. The painter mounts each control in a surface of
    // its own, so a command written in window coordinates would land outside it — a canvas
    // mounted anywhere but the frame origin painted nothing but its background. `hit_test`
    // takes window coordinates and converts them here, so a press still resolves to the mark
    // under the pointer.

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
        self.push_command(RenderCommand::FillRoundedRect { rect, radii: [radius; 4], color });
        self.base.request_redraw();
    }

    /// Draw text at the specified position.
    pub fn draw_text(&mut self, origin: Point, text: &str, font: &crate::core::Font, color: Color) {
        self.push_command(RenderCommand::DrawText {
            origin,
            text: Arc::from(text),
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
        self.push_command(RenderCommand::DrawPath {
            points: points.into(),
            closed,
            color,
            filled,
            width,
        });
        self.base.request_redraw();
    }

    /// Places `image` in `rect` as one undoable, layer-owned command (SP-51).
    ///
    /// The pixels are referenced, not copied: the same decoded image can be placed several
    /// times and is replayed every frame without duplicating the buffer. Replay, hit culling,
    /// snapping, undo/redo, trimming and export all treat it like any other stored command.
    pub fn draw_image(&mut self, image: CanvasImage, rect: Rect) {
        // A caller that only knows where the image belongs can pass a zero-sized target and get
        // the image's own pixel size; either way the destination is one rectangle.
        let rect = if rect.width == 0 || rect.height == 0 {
            Rect::new(rect.x, rect.y, image.width, image.height)
        } else {
            rect
        };
        self.push_command(RenderCommand::DrawImage {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            data: image.data,
        });
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

    /// A canvas asks for the host override, then the pan affordance, then the drawing-surface
    /// default (RW-33).
    fn cursor_icon(&self) -> crate::platform::CursorIcon {
        self.effective_cursor_icon()
    }

    /// Opts the canvas into the runtime's drag coordinator (C-11').
    ///
    /// Acceptance is still gated by [`Canvas::drop_enabled`], so a canvas with no host seam
    /// refuses the payload instead of swallowing it.
    fn drop_target_mut(&mut self) -> Option<&mut dyn crate::event::dnd::DropTarget> {
        Some(self)
    }

    fn save_state_dyn(&self) -> Option<&dyn crate::widget::save_state::SaveState> {
        Some(self)
    }

    fn save_state_dyn_mut(&mut self) -> Option<&mut dyn crate::widget::save_state::SaveState> {
        Some(self)
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
        use crate::widget::pointer_event::pointer_capability_value;
        match name {
            "mouse_pressed" => Some(EventSignalRef::mapped(
                "mouse_pressed",
                self.mouse_pressed_signal(),
                pointer_capability_value,
            )),
            "mouse_released" => Some(EventSignalRef::mapped(
                "mouse_released",
                self.mouse_released_signal(),
                pointer_capability_value,
            )),
            "mouse_moved" => Some(EventSignalRef::mapped("mouse_moved", &self.mouse_moved, |v| {
                CapabilityValue::String(format!("{v:?}"))
            })),
            "double_clicked" => Some(EventSignalRef::mapped(
                "double_clicked",
                self.double_clicked_signal(),
                pointer_capability_value,
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
            "grid_space" => Ok(CapabilityValue::String(
                match self.grid_space() {
                    GridSpace::Screen => "screen",
                    GridSpace::Model => "model",
                }
                .into(),
            )),
            "rulers_visible" => Ok(CapabilityValue::Bool(self.rulers_visible())),
            "ruler_unit" => Ok(CapabilityValue::UInt(self.ruler_unit() as u64)),
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
            "grid_space" => {
                let token = crate::widget::capability::coercion::expect_string(value)?;
                let space = match token.as_str() {
                    "screen" => GridSpace::Screen,
                    "model" => GridSpace::Model,
                    _ => return Err(CapabilityAccessError::OutOfRange),
                };
                self.set_grid_space(space);
                Ok(())
            }
            "rulers_visible" => {
                self.set_rulers_visible(crate::widget::capability::coercion::expect_bool(value)?);
                Ok(())
            }
            "ruler_unit" => {
                let unit = crate::widget::capability::coercion::expect_u32(value)?;
                self.set_ruler_unit(unit);
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
            "grid_space",
            "rulers_visible",
            "ruler_unit",
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
            _ => self.default_command(name),
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
        context.face_with_gradient_corners(
            rect,
            background,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        if self.show_grid && rect.width > 0 && rect.height > 0 {
            let step = self.grid_step.max(1);
            let grid_color = border.with_alpha(96);
            // The grid is a fixed background: it only changes with the geometry, the step, the
            // resolved colours or — in model space — the transform. Wrapping it in a cached layer
            // means a still canvas re-uses the previous frame's pixels instead of re-issuing every
            // line (SP-P54).
            let key = self.grid_cache_key
                ^ color_key(background)
                ^ color_key(border)
                ^ ((step as u64) << 48)
                ^ self.grid_placement_key();
            context.push_layer_group(rect, None, 1.0, Some(key));
            self.draw_grid_lines(context, rect, grid_color);
            context.pop_layer();
        }
        // Replay every visible layer in painting order, each inside its own render-cached
        // layer group. A layer whose command list and transform have not changed since the
        // previous frame is reused as pixels by the backend, so a still surface costs one
        // composite per layer rather than one draw per command (SP-P54). Culling still runs on
        // the commands that are replayed: a drawing surface that has been panned keeps every
        // off-screen mark, and replaying them costs the rasteriser a full pass for pixels
        // nobody can see.
        let transform = self.transform;
        let viewport = logical_viewport(transform, rect);
        // The painter draws this control inside a surface whose origin is the control's own
        // top-left corner, and hands that translation over through the render context. Commands
        // are replayed with `execute_command_ref`, which is the **raw** dispatch path and does
        // not add the translation the way `RenderContext::fill_rect` does — so it is folded into
        // the replay transform here. Without this a canvas mounted anywhere but the frame origin
        // issued its marks at absolute window coordinates, inside a surface that starts at its
        // own corner: every mark fell outside the surface and the control painted nothing but
        // its background.
        let (offset_x, offset_y) = context.offset();
        let replay = CanvasTransform {
            scale: transform.scale,
            offset: Point::new(transform.offset.x + offset_x, transform.offset.y + offset_y),
        };
        for layer in &mut self.layers {
            if !layer.visible || layer.commands.is_empty() {
                continue;
            }
            layer.ensure_chunks();
            let key = layer.cache_key;
            context.push_layer_group(rect, None, 1.0, Some(key));
            // The chunk index answers "which blocks can be seen" in one bounds test each, so a
            // panned surface skips whole blocks instead of walking every command (SP-110).
            let chunks = layer.chunks.borrow();
            let chunk_list: &[CommandChunk] =
                chunks.as_ref().map(|(_, chunks)| chunks.as_slice()).unwrap_or(&[]);
            for chunk in chunk_list {
                if let Some(bounds) = chunk.bounds {
                    if !rects_intersect(viewport, bounds) {
                        continue;
                    }
                }
                for stored in layer.commands.range_mut(chunk.start..chunk.end) {
                    // Cull in the logical space the commands are stored in, then map for the
                    // surface: the replayed bounds already carry the offset, so comparing them
                    // against `rect` here would reject every mark that was mapped.
                    if let Some(bounds) = command_bounds(&stored.command) {
                        if !rects_intersect(rect, bounds) {
                            continue;
                        }
                    }
                    let command = stored.replay_under(replay);
                    context.execute_command_ref(command);
                }
            }
            drop(chunks);
            context.pop_layer();
        }
        // Decorations are a separate pass, never part of a business layer: that is what keeps
        // them out of undo, hit testing and both export paths by construction (RW-30/RW-32).
        self.draw_decorations(context, rect);
        // Draw border to make canvas area visible
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        // Every gesture over the surface is refused by the input gate while disabled, so the
        // drawing itself has to recede; the veil covers the layers, grid and frame just painted.
        self.base.paint_disabled_veil(context, rect);
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
            // A middle-button drag, or a primary drag while space is held, pans the view. The
            // arm is before the primary arm so a space-drag is never reported as a pick.
            Event::MousePress { pos, button, .. } if self.wants_pan_gesture(*button) => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(true);
                self.begin_pan(*pos);
                self.mouse_pressed.emit_at(*pos, *button);
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(true);
                self.mouse_pressed.emit_at(*pos, *button);
                // A pick is only meaningful when something observes it. Skipping the hit test
                // with no subscriber keeps a press on a large drawing from walking the index
                // for a signal nobody listens to (SP-111).
                if self.command_picked.slot_count() > 0 {
                    if let Some(index) = self.hit_test(*pos) {
                        self.command_picked.emit(index);
                    }
                }
                // The tagged query is a separate walk only when a host listens for it, so an
                // untagged canvas does not pay for the identity index.
                if self.command_picked_tagged.slot_count() > 0 {
                    if let Some((index, tag)) = self.hit_test_tagged(*pos) {
                        self.command_picked_tagged.emit((index, tag));
                    }
                }
            }
            Event::MouseRelease { pos, button, .. } => {
                self.last_mouse_pos = *pos;
                self.base.set_mouse_pressed(false);
                let ended_pan = self.end_pan();
                if *button == 1 || ended_pan {
                    self.mouse_released.emit_at(*pos, *button);
                }
            }
            Event::MouseMove { pos } if self.geometry().contains(*pos) => {
                self.last_mouse_pos = *pos;
                if self.is_panning() {
                    self.update_pan(*pos);
                }
                self.mouse_moved.emit(*pos);
            }
            Event::MouseDoubleClick { pos, button, .. } if *button == 1 => {
                self.last_mouse_pos = *pos;
                self.double_clicked.emit_at(*pos, *button);
            }
            Event::Wheel { delta, .. } if self.zoom_enabled && delta.y != 0 => {
                // Zoom about the pointer, so the ink under the cursor stays under it. The stored
                // offset is relative to the control's own surface, while the pointer is in window
                // coordinates: without subtracting the geometry the anchor was displaced by the
                // control's origin and the picture slid under the cursor.
                let anchor = self.local_pointer(self.last_mouse_pos);
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
                    // Every stored command is painted under the new transform, so the cached
                    // mappings and layer identities are stale from this line on.
                    self.invalidate_replay_caches();
                    self.zoom_changed.emit(scale);
                    self.base.request_redraw();
                }
            }
            Event::ContextMenu { pos } => {
                self.context_menu_requested.emit(*pos);
            }
            Event::KeyPress { key, modifiers } => match crate::shortcut::Key::from_key_code(*key) {
                Some(crate::shortcut::Key::Space) => self.note_space(true),
                Some(crate::shortcut::Key::Z)
                    if crate::shortcut::event_carries_primary(*modifiers) =>
                {
                    self.undo();
                }
                Some(crate::shortcut::Key::Y)
                    if crate::shortcut::event_carries_primary(*modifiers) =>
                {
                    self.redo();
                }
                _ => {}
            },
            Event::KeyRelease { key, .. }
                if crate::shortcut::Key::from_key_code(*key)
                    == Some(crate::shortcut::Key::Space) =>
            {
                self.note_space(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
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

    /// Gap 5: the layer panel mirrors the canvas stack on every structural change.
    #[test]
    fn a_layer_panel_follows_the_canvas_stack() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert!(canvas.attach_layer_panel(Some(LayerPanel::new(Rect::new(0, 0, 120, 160)))));
        canvas.begin_layer("ink");
        canvas.begin_layer("guide");
        {
            let panel = canvas.layer_panel().expect("attached");
            assert_eq!(panel.entries().len(), 2);
            assert_eq!(panel.entries()[0].name, "ink");
            assert_eq!(panel.entries()[1].name, "guide");
            assert_eq!(panel.selected_layer(), Some(2), "the current layer is selected");
        }

        assert!(canvas.set_layer_visible(0, false));
        assert!(!canvas.layer_panel().expect("attached").entries()[0].visible);

        assert!(canvas.move_layer(1, 0));
        assert_eq!(canvas.layer_panel().expect("attached").entries()[0].name, "guide");

        assert!(canvas.remove_layer(0));
        assert_eq!(canvas.layer_panel().expect("attached").entries().len(), 1);

        canvas.clear();
        assert!(canvas.layer_panel().expect("attached").entries().is_empty());
        assert_eq!(canvas.layer_panel().expect("attached").selected_layer(), None);
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
        canvas.handle_event(&Event::MouseDoubleClick {
            pos: Point::new(50, 50),
            button: 1,
            modifiers: 0,
        });
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

    /// SP-111: the hit index follows content and visibility changes without going stale.
    #[test]
    fn hit_index_tracks_content_and_layer_visibility() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.fill_rect(Rect::new(10, 10, 40, 40), Color::RED);
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(0));

        // A later command covering the same point is on top.
        canvas.fill_rect(Rect::new(15, 15, 40, 40), Color::BLUE);
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(1));

        // Hiding the layer removes both, and showing it restores the index.
        assert!(canvas.set_layer_visible(0, false));
        assert_eq!(canvas.hit_test(Point::new(20, 20)), None);
        assert!(canvas.set_layer_visible(0, true));
        assert_eq!(canvas.hit_test(Point::new(20, 20)), Some(1));

        canvas.clear();
        assert_eq!(canvas.hit_test(Point::new(20, 20)), None, "a cleared canvas picks nothing");
    }

    /// SP-110: commands are indexed in blocks whose bounds can be culled whole.
    #[test]
    fn replay_chunks_group_commands_by_their_bounds() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        for _ in 0..COMMAND_CHUNK_SIZE {
            canvas.fill_rect(Rect::new(0, 0, 5, 5), Color::RED);
        }
        for _ in 0..COMMAND_CHUNK_SIZE {
            canvas.fill_rect(Rect::new(10_000, 0, 5, 5), Color::BLUE);
        }

        canvas.layers[0].ensure_chunks();
        let chunks = canvas.layers[0].chunks.borrow();
        let chunks = chunks.as_ref().map(|(_, chunks)| chunks.as_slice()).unwrap_or(&[]);
        assert_eq!(chunks.len(), 2, "64 commands divided into two blocks");

        let viewport = logical_viewport(
            CanvasTransform { scale: 1.0, offset: Point::new(-10_000, 0) },
            Rect::new(0, 0, 200, 100),
        );
        assert!(!rects_intersect(viewport, chunks[0].bounds.expect("the near block has bounds")));
        assert!(rects_intersect(viewport, chunks[1].bounds.expect("the far block has bounds")));
    }

    /// RW-29: a tagged group gives every command it contains a host identity, and the tagged
    /// pick answer names that identity without changing the plain index contract.
    #[test]
    fn canvas_tagged_groups_expose_node_identity() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert!(canvas.begin_group_with_tag(7));
        canvas.fill_rect(Rect::new(10, 10, 20, 20), Color::RED);
        assert!(canvas.end_group());
        canvas.fill_rect(Rect::new(60, 60, 40, 40), Color::BLUE);

        assert_eq!(canvas.hit_test(Point::new(15, 15)), Some(0), "plain hit keeps working");
        assert_eq!(canvas.hit_test_tagged(Point::new(15, 15)), Some((0, 7)));
        assert_eq!(canvas.hit_test(Point::new(70, 70)), Some(1));
        assert_eq!(canvas.hit_test_tagged(Point::new(70, 70)), None, "untagged has no node id");
        assert_eq!(canvas.hit_test_tagged(Point::new(150, 90)), None);

        // The tag is part of the command, so undo/redo must not lose it.
        while canvas.undo() {}
        assert!(canvas.redo());
        assert!(canvas.redo());
        assert_eq!(canvas.hit_test_tagged(Point::new(15, 15)), Some((0, 7)));

        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        canvas.command_picked_tagged_signal().connect_ref(move |picked: &(usize, CanvasTag)| {
            *sink.lock().expect("the test slot locks") = Some(*picked);
        });
        canvas.handle_event(&Event::mouse_press(15, 15, 1));
        assert_eq!(*seen.lock().expect("the test slot locks"), Some((0, 7)));
    }

    /// RW-30: overlay commands paint but are not content — no history step, no hit, no export.
    #[test]
    fn canvas_overlay_commands_stay_out_of_history_and_export() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 40, 30));
        canvas.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        assert_eq!(canvas.undo_count(), 1);

        assert!(canvas.begin_overlay_layer());
        canvas.fill_rect(Rect::new(20, 20, 10, 10), Color::GREEN);
        assert!(canvas.end_overlay_layer());

        assert_eq!(canvas.command_count(), 1, "overlay commands are not content");
        assert_eq!(canvas.overlay_command_count(), 1);
        assert_eq!(canvas.undo_count(), 1, "the overlay added no history step");
        assert_eq!(canvas.hit_test(Point::new(25, 25)), None, "overlay is not hit-testable");

        let mut clean = Canvas::new(Rect::new(0, 0, 40, 30));
        clean.fill_rect(Rect::new(0, 0, 10, 10), Color::RED);
        assert_eq!(
            canvas.export_png(40, 30, 1.0).expect("a valid export"),
            clean.export_png(40, 30, 1.0).expect("a valid export"),
            "the PNG omits the overlay"
        );
        assert_eq!(canvas.export_svg(), clean.export_svg(), "the SVG omits the overlay");
        assert_ne!(
            crate::widget::svg::render_to_svg(&mut canvas),
            crate::widget::svg::render_to_svg(&mut clean),
            "the screen drawing still shows the overlay"
        );

        assert!(canvas.undo());
        assert_eq!(canvas.overlay_command_count(), 1, "undo leaves decorations alone");
        canvas.clear_overlay();
        assert_eq!(canvas.overlay_command_count(), 0);
    }

    /// RW-31: the selected grid space decides whether the step is surface pixels or model units.
    #[test]
    fn canvas_grid_space_selects_the_snap_coordinate_system() {
        let mut model = Canvas::new(Rect::new(0, 0, 200, 100));
        model.set_transform(CanvasTransform { scale: 2.0, offset: Point::new(0, 0) });
        model.set_grid_step(10);
        model.set_snap_to_grid(true);
        assert_eq!(model.grid_space(), GridSpace::Screen, "screen stays the default");
        model.set_grid_space(GridSpace::Model);
        model.draw_line(Point::new(6, 6), Point::new(6, 6), Color::RED);
        match model.layers[0].commands[0].command {
            RenderCommand::DrawLine { from, .. } => assert_eq!(from, Point::new(10, 10)),
            _ => panic!("expected a line"),
        }

        let mut screen = Canvas::new(Rect::new(0, 0, 200, 100));
        screen.set_transform(CanvasTransform { scale: 2.0, offset: Point::new(0, 0) });
        screen.set_grid_step(10);
        screen.set_snap_to_grid(true);
        screen.draw_line(Point::new(6, 6), Point::new(6, 6), Color::RED);
        match screen.layers[0].commands[0].command {
            RenderCommand::DrawLine { from, .. } => assert_eq!(from, Point::new(5, 5)),
            _ => panic!("expected a line"),
        }
    }

    /// RW-32: guides and rulers are screen furniture, so they draw but never export.
    #[test]
    fn canvas_guides_and_rulers_are_decorations_not_document_content() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 100, 80));
        assert!(!canvas.rulers_visible());
        canvas.set_rulers_visible(true);
        assert!(canvas.rulers_visible());
        canvas.draw_guide(GuideAxis::Vertical, 25);
        canvas.draw_guide(GuideAxis::Horizontal, 30);
        assert_eq!(canvas.guide_count(), 2);
        assert_eq!(canvas.guides(), &[(GuideAxis::Vertical, 25), (GuideAxis::Horizontal, 30)]);

        let mut clean = Canvas::new(Rect::new(0, 0, 100, 80));
        clean.set_rulers_visible(true);
        assert_ne!(
            crate::widget::svg::render_to_svg(&mut canvas),
            crate::widget::svg::render_to_svg(&mut clean),
            "the screen drawing shows the guides"
        );
        assert_eq!(canvas.export_svg(), clean.export_svg(), "the export omits decorations");

        canvas.clear_guides();
        assert_eq!(canvas.guide_count(), 0);
    }

    /// C-11': a canvas with a host seam accepts a drop and reports both coordinate forms.
    #[test]
    fn canvas_accepts_a_drop_and_reports_the_model_point() {
        use crate::event::dnd::{DragPayload, DropEffect, DropTarget};

        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        let payload = DragPayload::new("node", "button").with_label("Button");
        assert!(!DropTarget::can_accept(&canvas, &payload), "no seam means no drop");

        canvas.set_transform(CanvasTransform { scale: 2.0, offset: Point::new(10, 10) });
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        canvas.drop_received_signal().connect_ref(move |event: &CanvasDropEvent| {
            *sink.lock().expect("the test slot locks") =
                Some((event.payload.item_id.clone(), event.position, event.model));
        });
        canvas.set_drop_handler(Some(Arc::new(|event: &CanvasDropEvent| {
            assert_eq!(event.payload.type_id, "node");
            DropEffect::Move
        })));

        assert!(DropTarget::can_accept(&canvas, &payload));
        assert_eq!(
            DropTarget::on_drop(&mut canvas, &payload, Point::new(20, 20)),
            DropEffect::Move
        );
        let recorded = seen.lock().expect("the test slot locks").clone().expect("delivered");
        assert_eq!(recorded.0, "button");
        assert_eq!(recorded.1, Point::new(20, 20));
        assert_eq!(recorded.2, Point::new(5, 5), "the model point is inverse-transformed");
        assert!(DropTarget::preview_rect(&canvas, &payload, Point::new(20, 20)).is_some());
    }

    /// RW-33: middle-button and space-drag pan without turning the gesture into a pick, and the
    /// explicit cursor override is what the widget trait reports.
    #[test]
    fn canvas_pans_with_middle_or_space_drag_and_honours_a_cursor_override() {
        use crate::event::mouse_button;

        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        assert!(!canvas.is_pan_enabled());
        canvas.set_pan_enabled(true);
        assert!(canvas.is_pan_enabled());

        canvas.handle_event(&Event::MousePress {
            pos: Point::new(50, 50),
            button: mouse_button::MIDDLE,
            modifiers: 0,
        });
        assert!(canvas.is_panning());
        assert_eq!(Widget::cursor_icon(&canvas), crate::platform::CursorIcon::Move);
        canvas.handle_event(&Event::MouseMove { pos: Point::new(70, 60) });
        assert_eq!(canvas.transform().offset, Point::new(20, 10));
        canvas.handle_event(&Event::MouseRelease {
            pos: Point::new(70, 60),
            button: mouse_button::MIDDLE,
            modifiers: 0,
        });
        assert!(!canvas.is_panning());

        // Space + primary is the second pan gesture, and it must not report a pick.
        canvas.draw_rect(Rect::new(0, 0, 200, 100), Color::RED);
        let picked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = std::sync::Arc::clone(&picked);
        canvas.command_picked.connect_ref(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        canvas.handle_event(&Event::KeyPress { key: 32, modifiers: 0 });
        canvas.handle_event(&Event::MousePress {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert!(canvas.is_panning(), "space turns a primary drag into a pan");
        assert_eq!(picked.load(std::sync::atomic::Ordering::SeqCst), 0, "a pan is not a pick");
        canvas.handle_event(&Event::KeyRelease { key: 32, modifiers: 0 });
        canvas.handle_event(&Event::MouseRelease {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        });
        assert!(!canvas.is_panning());

        canvas.set_cursor_icon(crate::platform::CursorIcon::Pointer);
        assert_eq!(Widget::cursor_icon(&canvas), crate::platform::CursorIcon::Pointer);
        canvas.clear_cursor_icon();
        assert_eq!(Widget::cursor_icon(&canvas), crate::platform::CursorIcon::Crosshair);
    }

    /// RW-64 (Canvas only): the view configuration round-trips and a malformed blob is refused.
    #[test]
    fn canvas_save_state_round_trips_the_view_configuration() {
        use crate::widget::save_state::SaveState;

        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_transform(CanvasTransform { scale: 2.0, offset: Point::new(15, -5) });
        canvas.set_show_grid(true);
        canvas.set_grid_step(24);
        canvas.set_snap_to_grid(true);
        canvas.set_grid_space(GridSpace::Model);
        canvas.set_rulers_visible(true);
        canvas.set_ruler_unit(100);
        canvas.set_pan_enabled(true);
        canvas.draw_guide(GuideAxis::Vertical, 12);

        let blob = SaveState::save_state(&canvas);
        let mut restored = Canvas::new(Rect::new(0, 0, 200, 100));
        assert!(SaveState::restore_state(&mut restored, &blob));
        assert_eq!(restored.transform(), canvas.transform());
        assert!(restored.show_grid());
        assert_eq!(restored.grid_step(), 24);
        assert!(restored.snap_to_grid());
        assert_eq!(restored.grid_space(), GridSpace::Model);
        assert!(restored.rulers_visible());
        assert_eq!(restored.ruler_unit(), 100);
        assert!(restored.is_pan_enabled());
        assert_eq!(restored.guides(), canvas.guides());
        assert_eq!(SaveState::state_version(&canvas), 2);

        let before = restored.transform();
        assert!(!SaveState::restore_state(&mut restored, &[1, 2, 3]), "a short blob is refused");
        assert_eq!(restored.transform(), before, "a refused blob changes nothing");
    }

    /// RW-124: a model-space ruler names design units and maps them through the transform, so a
    /// zoomed canvas keeps ticks on the design origin instead of a surface lattice.
    #[test]
    fn model_space_ruler_ticks_follow_the_transform() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_grid_space(GridSpace::Model);
        canvas.set_ruler_unit(50);
        canvas.set_transform(CanvasTransform { scale: 2.0, offset: Point::new(10, 0) });

        let ticks = canvas.ruler_ticks();
        let vertical: Vec<_> =
            ticks.iter().filter(|tick| tick.axis == GuideAxis::Vertical).collect();
        assert!(
            vertical.iter().any(|tick| tick.model == 0 && tick.surface == 10),
            "the design origin must land at the transform's offset: {vertical:?}"
        );
        assert!(
            vertical.iter().any(|tick| tick.model == 50 && tick.surface == 110),
            "50 design units at 2x must be 100 surface pixels from the origin: {vertical:?}"
        );
    }

    /// A grid step above `i32::MAX` is clamped instead of becoming a negative paint step.
    ///
    /// # What this pins
    ///
    /// `set_grid_step` only floored at 1, and both grid loops cast the step to `i32`. A value
    /// above `i32::MAX` therefore became negative and `x += unit` walked backwards forever — a
    /// hang in release, an overflow panic in debug.
    #[test]
    fn a_grid_step_above_i32_max_still_paints_and_terminates() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_grid_step(u32::MAX);
        assert_eq!(canvas.grid_step(), i32::MAX as u32, "the step is clamped to the signed range");
        canvas.set_show_grid(true);
        let svg = render_to_svg(&mut canvas);
        assert!(svg.starts_with("<svg"), "the grid paints without walking off the i32 range");
    }

    /// The grid's coordinate lists are reused between frames, and a steady frame repaints the same
    /// lines (W5-36).
    ///
    /// # What this pins
    ///
    /// `draw_grid_lines` rebuilt both lists on every frame. They now live in `grid_scratch`; the
    /// second frame must reuse the same backing allocation (same start pointer) and paint the same
    /// picture, so a refill cannot append a stale tail after the viewport moves.
    #[test]
    fn the_grid_reuses_its_coordinate_buffers_between_frames() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.set_grid_step(20);
        canvas.set_show_grid(true);

        let first = render_to_svg(&mut canvas);
        let (xs_ptr, ys_ptr) = {
            let scratch = canvas.grid_scratch.borrow();
            assert!(!scratch.0.is_empty() && !scratch.1.is_empty(), "the grid has lines to paint");
            (scratch.0.as_ptr(), scratch.1.as_ptr())
        };

        let second = render_to_svg(&mut canvas);
        assert_eq!(first, second, "a steady frame paints the same grid");
        let scratch = canvas.grid_scratch.borrow();
        assert_eq!(
            (xs_ptr, ys_ptr),
            (scratch.0.as_ptr(), scratch.1.as_ptr()),
            "the second frame must refill the buffers, not allocate new ones"
        );
    }

    /// A line spanning the whole `i32` range has bounds, so it can be hit-tested.
    ///
    /// # What this pins
    ///
    /// The span was computed as `(from.x - to.x).unsigned_abs()` in `i32`, so the subtraction
    /// itself overflowed before the absolute value was taken. The probe sits inside the control
    /// and strictly inside the half-open box (`Rect::contains_point` excludes the max edge), which
    /// is where a reader would click the painted line.
    #[test]
    fn a_line_spanning_the_full_i32_range_has_bounds_instead_of_overflowing() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_line(Point::new(i32::MIN, 10), Point::new(i32::MAX, 10), Color::RED);
        assert_eq!(canvas.hit_test(Point::new(5, 10)), Some(0));
    }

    /// Text is part of the hit-test index, not only of the picture.
    ///
    /// # What this pins
    ///
    /// `command_bounds` had no `DrawText` arm, and `hit_entries` only indexes commands that have
    /// bounds, so a painted label could never be picked.
    #[test]
    fn a_text_command_is_part_of_the_hit_test_index() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_text(Point::new(10, 20), "Hello", &Font::default_ui(), Color::BLACK);
        assert_eq!(canvas.hit_test(Point::new(12, 22)), Some(0));
        assert_eq!(canvas.hit_test(Point::new(500, 500)), None);
    }

    /// Lowering the command limit publishes the eviction and repaints.
    ///
    /// # What this pins
    ///
    /// `set_command_limit` could drop content and overlay commands while emitting nothing, so a
    /// host mirroring `content_changed` kept a stale count and the canvas kept a stale picture.
    #[test]
    fn lowering_the_command_limit_publishes_the_eviction() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        for index in 0..5 {
            canvas.fill_rect(Rect::new(index * 4, 0, 2, 2), Color::BLUE);
        }
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = std::sync::Arc::clone(&seen);
        canvas.content_changed.connect(move |count| sink.lock().unwrap().push(*count));

        canvas.set_command_limit(2);
        assert_eq!(canvas.command_count(), 2);
        assert_eq!(*seen.lock().unwrap(), vec![2], "the eviction is published once");

        // Raising the limit evicts nothing, so it publishes nothing.
        seen.lock().unwrap().clear();
        canvas.set_command_limit(10);
        assert!(seen.lock().unwrap().is_empty());
    }

    /// Disabling anti-aliasing rewrites decorations and the redo history too.
    ///
    /// # What this pins
    ///
    /// Only `layers` was rewritten, so a decoration already on screen and a command restored
    /// from the redo stack stayed anti-aliased after the global switch said otherwise.
    #[test]
    fn disabling_anti_aliasing_also_rewrites_overlays_and_the_redo_history() {
        let mut canvas = Canvas::new(Rect::new(0, 0, 200, 100));
        canvas.draw_line_aa(Point::new(0, 0), Point::new(10, 10), Color::RED);
        canvas.begin_overlay_layer();
        canvas.fill_circle_aa(Point::new(20, 20), 5, Color::BLUE);
        canvas.end_overlay_layer();
        assert!(canvas.undo());

        canvas.set_anti_alias(false);
        assert!(matches!(canvas.overlay_commands[0].command, RenderCommand::FillCircle { .. }));
        assert!(matches!(canvas.redo_stack[0][0].command, RenderCommand::DrawLine { .. }));

        assert!(canvas.redo());
        assert!(matches!(canvas.layers[0].commands[0].command, RenderCommand::DrawLine { .. }));
    }

    /// Wheel zoom anchors on the pointer in the canvas's own space.
    ///
    /// # What this pins
    ///
    /// The anchor was the raw window point while `transform.offset` is surface-relative, so a
    /// canvas mounted away from the origin zoomed about a point displaced by its own position.
    #[test]
    fn wheel_zoom_anchors_on_the_surface_relative_pointer() {
        let mut canvas = Canvas::new(Rect::new(100, 50, 200, 100));
        canvas.handle_event(&Event::MouseMove { pos: Point::new(150, 80) });
        canvas.handle_event(&Event::wheel(0, 1, 0));

        // Local anchor (50, 30) at ratio 1/1.1 -> offset (5, 3), not (14, 7).
        assert_eq!(canvas.transform().offset, Point::new(5, 3));
        assert!((canvas.transform().scale - 1.0 / 1.1).abs() < 1e-6);
    }

    /// A drop's model point is mapped in the canvas's own space.
    ///
    /// # What this pins
    ///
    /// `CanvasDropEvent::model` inverse-transformed the raw window point, so a canvas at
    /// (100, 50) reported the drop as if the pointer had been that far outside it.
    #[test]
    fn a_drop_model_point_is_relative_to_the_canvas_surface() {
        use crate::event::dnd::{DragPayload, DropEffect, DropTarget};

        let mut canvas = Canvas::new(Rect::new(100, 50, 200, 100));
        canvas.set_drop_handler(Some(Arc::new(|_| DropEffect::Copy)));
        let payload = DragPayload::new("node", "button");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        canvas.drop_received_signal().connect_ref(move |event: &CanvasDropEvent| {
            *sink.lock().unwrap() = Some(event.model);
        });

        assert_eq!(
            DropTarget::on_drop(&mut canvas, &payload, Point::new(150, 90)),
            DropEffect::Copy
        );
        assert_eq!(*seen.lock().unwrap(), Some(Point::new(50, 40)));
    }
}
