// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Splitter widget.
use crate::core::{Color, Orientation, Rect};
use crate::event::{DragPayload, DragSession};
use crate::layout::{splitter::SplitterLayout, Layout};
use crate::object::ObjectId;
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_orientation, orientation_to_str};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::save_state::SaveState;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

use super::theme_snapshot::ThemeSnapshot;

// ── Handle dragging, expressed through `event::dnd` ─────────────────────────
//
// The press/move/release machine used to be hand-written here: a `DragState`
// struct holding the start position, a `MouseMove` arm recomputing the delta, and
// a `MouseRelease` arm clearing it. That is the same shape 28 files in this crate
// had, so the state now lives in `DragSession` and this control only supplies the
// domain decisions — which handle was grabbed, and what a new ratio should be.

/// What a splitter drag is carrying.
///
/// The payload identifies the *handle* being dragged rather than a data item, so a
/// future drop target (a "snap this pane's size to that preset" zone, say) can tell
/// which divider the drag came from.
const SPLITTER_DRAG_TYPE: &str = "splitter_handle";

/// The distance a press must travel before it counts as a handle drag rather than
/// a click on the divider.
///
/// Two pixels: the handle is only five wide, so a larger threshold would make the
/// first few pixels of every drag feel unresponsive.
const SPLITTER_DRAG_THRESHOLD: i32 = 2;

/// The default width of the divider between adjacent panes, in logical pixels.
///
/// This is **one** number with three consumers: the layout reserves it between panes, the
/// painter fills it, and the press hit test accepts a grab inside it. They used to disagree —
/// the layout reserved nothing and the draw and drag each restated `5` — which is why the
/// panes' rectangles met edge to edge and the divider was painted over them. A caller can
/// change it at runtime with [`Splitter::set_handle_width`].
const HANDLE_WIDTH: u32 = 5;

/// The pane ratios captured when a handle drag began, plus the handle index.
///
/// Kept beside the session rather than inside the payload because it is the
/// splitter's own layout, not something a drop target would ever read.
#[derive(Debug, Clone)]
struct HandleDrag {
    /// Index of the handle (the gap between pane `idx` and `idx+1`).
    handle_index: usize,
    /// Pane ratios at drag start (snapshot), so the move rule is absolute rather
    /// than accumulating rounding error across frames.
    start_ratios: Vec<f32>,
}

/// Splitter widget with deterministic pane-ratio distribution contract.
///
/// Delegates layout calculations to [`SplitterLayout`].
pub struct Splitter {
    base: BaseWidget,
    layout: SplitterLayout,
    /// Emitted after a pane ratio change is **committed** — a drag release or an explicit
    /// ratio/state mutation — with the full ratio vector in pane order. Ratios are relative
    /// weights, not pixels. Live drag feedback uses [`Self::pane_layout_preview`].
    pub pane_layout_changed: Signal1<Vec<f32>>,
    /// Emitted while a handle drag changes the ratios, so a preview can follow
    /// the pointer without treating every move as a committed layout change.
    pub pane_layout_preview: Signal1<Vec<f32>>,
    /// Emitted when the splitter is switched between horizontal and vertical,
    /// with the new orientation.
    pub orientation_changed: Signal1<Orientation>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// The shared drag state machine, active only while a handle is being dragged.
    drag_session: Option<DragSession>,
    /// The splitter-specific snapshot a drag started with.
    drag_state: Option<HandleDrag>,
    active_pane: Option<usize>,
    /// Width of the divider between adjacent panes, in logical pixels.
    handle_width: u32,
    /// Per-pane collapsed state, parallel to the layout's pane list.
    collapsed: Vec<bool>,
    /// Per-pane maximum extent, parallel to the layout's pane list (C-22).
    ///
    /// `None` is "no limit", which is the historical behaviour. The cap is also pushed into the
    /// engine's solve inputs (C-018), so the wrapper's mirror is what the public getter reports
    /// rather than the only enforcement point.
    max_pane_sizes: Vec<Option<u32>>,
    /// The ratio each pane had before it was collapsed, restored on expansion.
    pre_collapse_ratios: Vec<f32>,
    /// The last dragged-pair ratio thousandths emitted as a preview; throttles the
    /// preview to a visible change rather than one event per pointer sample.
    preview_mark: Option<(i64, i64)>,
}
impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            // The gap is not decoration: it is the divider's grab area. With no gap the panes
            // meet at the boundary and a press on the visible handle lands on a pane instead
            // of the splitter, so the drag can never start.
            layout: SplitterLayout::new(Orientation::Horizontal, HANDLE_WIDTH),
            pane_layout_changed: Signal1::new(),
            pane_layout_preview: Signal1::new(),
            orientation_changed: Signal1::new(),
            registry: None,
            drag_session: None,
            drag_state: None,
            active_pane: None,
            handle_width: HANDLE_WIDTH,
            collapsed: Vec::new(),
            max_pane_sizes: Vec::new(),
            pre_collapse_ratios: Vec::new(),
            preview_mark: None,
        }
    }
    /// Returns splitter orientation.
    pub fn orientation(&self) -> Orientation {
        self.layout.orientation()
    }
    /// Sets splitter orientation and emits change signal on transition.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.layout.orientation() == orientation {
            return;
        }
        self.layout.set_orientation(orientation);
        self.orientation_changed.emit(orientation);
        // Every pane rectangle is recomputed along the other axis.
        self.place_panes();
        self.base.request_redraw();
    }
    /// Returns pane count.
    pub fn pane_count(&self) -> usize {
        self.layout.pane_count()
    }
    /// Returns pane ids in stable order.
    pub fn pane_ids(&self) -> &[ObjectId] {
        self.layout.pane_ids()
    }
    /// Returns ratio for pane index.
    pub fn ratio(&self, index: usize) -> Option<f32> {
        self.layout.ratio(index)
    }
    /// Returns every pane ratio, in pane order.
    ///
    /// These are the layout's relative weights — the same values [`Self::ratio`] exposes — so a
    /// host can hand them to `Settings::set_split_ratios` unchanged and restore them on the next
    /// launch with [`Self::set_ratios`].
    pub fn ratios(&self) -> Vec<f32> {
        self.layout.ratios().to_vec()
    }
    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        let index = self.layout.add_pane(pane_id, stretch);
        self.collapsed.push(false);
        self.max_pane_sizes.push(None);
        self.pre_collapse_ratios.push((stretch.max(1)) as f32);
        // A pane is an ordinary tree child: the tree painter draws it after the splitter's
        // track and the hit test reaches it directly, so no draw-closure registry is needed.
        self.base.add_child_linked(pane_id);
        crate::set_widget_visible(pane_id, true);
        self.place_panes();
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        index
    }
    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        let index = self.layout.pane_ids().iter().position(|id| *id == pane_id);
        if !self.layout.remove_pane(pane_id) {
            return false;
        }
        if let Some(index) = index {
            if index < self.collapsed.len() {
                self.collapsed.remove(index);
            }
            if index < self.max_pane_sizes.len() {
                self.max_pane_sizes.remove(index);
            }
            if index < self.pre_collapse_ratios.len() {
                self.pre_collapse_ratios.remove(index);
            }
        }
        self.base.remove_child_linked(pane_id);
        // A removed pane is no longer part of the split; leaving it visible would let a host that
        // renders by id paint it over the remaining panes.
        crate::set_widget_visible(pane_id, false);
        self.place_panes();
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }
    /// Sets ratio for pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        // C-22: an explicit write honours the pane's maximum too, so a host cannot put a pane
        // over a cap the user cannot drag it past.
        let ratio = self.clamped_ratio(index, ratio);
        if !self.layout.set_ratio(index, ratio) {
            return false;
        }
        // A positive ratio is an explicit expansion; keep the collapse flag in step.
        if ratio > 0.0 {
            if let Some(slot) = self.collapsed.get_mut(index) {
                *slot = false;
            }
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.place_panes();
        // The engine's normalisation can move the produced extent away from the ratio-space cap, so
        // the actual rectangle decides whether the pane is inside its maximum.
        self.converge_pane_to_maximum(index);
        self.base.request_redraw();
        true
    }
    /// Sets all pane ratios. Returns false when the slice length differs from the pane count.
    ///
    /// A host restores a saved layout by calling this before the splitter is shown, using the
    /// vector [`Self::ratios`] returned in the previous session.
    pub fn set_ratios(&mut self, ratios: &[f32]) -> bool {
        let clamped = self.clamp_ratios_to_max(ratios);
        if !self.layout.set_ratios(clamped) {
            return false;
        }
        // A zero weight is the collapsed representation; a positive one is expanded.
        for (index, ratio) in ratios.iter().enumerate() {
            if let Some(slot) = self.collapsed.get_mut(index) {
                *slot = *ratio <= 0.0;
            }
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.place_panes();
        self.base.request_redraw();
        true
    }
    /// Normalizes ratios to sum to 1.
    pub fn normalize_ratios(&mut self) {
        self.layout.normalize_ratios();
    }

    /// Sets the minimum extent of pane `index`.
    ///
    /// Returns `false` when the index is out of range. A pane is never dragged below its
    /// minimum while the splitter can still pay every minimum.
    pub fn set_min_pane_size(&mut self, index: usize, minimum: u32) -> bool {
        if !self.layout.set_min_size(index, minimum) {
            return false;
        }
        self.place_panes();
        self.base.request_redraw();
        true
    }

    /// The minimum extent of pane `index`.
    pub fn min_pane_size(&self, index: usize) -> u32 {
        self.layout.min_size(index)
    }

    /// Sets the maximum extent of pane `index` (C-22).
    ///
    /// Returns `false` for an out-of-range index. The cap is enforced in pixels at every ratio
    /// write: a drag stops when the pane reaches it, and an explicit [`Self::set_ratio`] is clamped
    /// to it. `0` is a legal maximum (a permanently collapsed pane).
    pub fn set_max_pane_size(&mut self, index: usize, maximum: u32) -> bool {
        if index >= self.pane_count() {
            return false;
        }
        if self.max_pane_sizes.len() < self.pane_count() {
            self.max_pane_sizes.resize(self.pane_count(), None);
        }
        if self.max_pane_sizes[index] == Some(maximum) {
            return true;
        }
        self.max_pane_sizes[index] = Some(maximum);
        // C-018: the solve reads the same cap, so the arrangement it produces after this write is
        // already inside the constraint rather than pulled back by the wrapper afterwards.
        let _ = self.layout.set_max_size(index, maximum);
        // Converge the current ratios onto the new cap rather than leaving the pane over it
        // until the next drag.
        let ratios = self.layout.ratios().to_vec();
        let clamped = self.clamp_ratios_to_max(&ratios);
        self.layout.set_ratios(clamped);
        self.place_panes();
        self.base.request_redraw();
        true
    }

    /// The maximum extent of pane `index`, or `None` when it is unbounded.
    pub fn max_pane_size(&self, index: usize) -> Option<u32> {
        self.max_pane_sizes.get(index).copied().flatten()
    }

    /// Sets whether pane `index` may collapse to zero.
    pub fn set_pane_collapsible(&mut self, index: usize, collapsible: bool) -> bool {
        self.layout.set_collapsible(index, collapsible)
    }

    /// Whether pane `index` may collapse.
    pub fn pane_collapsible(&self, index: usize) -> bool {
        self.layout.collapsible(index)
    }

    /// The width of the divider between adjacent panes, in logical pixels.
    pub fn handle_width(&self) -> u32 {
        self.handle_width
    }

    /// Sets the divider width. The layout reserves the same extent between panes, so the
    /// painted handle, its grab area and the gap can never disagree.
    pub fn set_handle_width(&mut self, width: u32) {
        let width = width.max(1);
        if self.handle_width == width {
            return;
        }
        self.rebuild_layout_with_spacing(width);
        self.handle_width = width;
        self.place_panes();
        self.base.request_redraw();
    }

    /// Rebuilds the layout at a new pane gap, preserving panes, weights, minimums and
    /// collapse permissions. `SplitterLayout` has no spacing setter, so the wrapper replays
    /// the pane list into a fresh layout rather than mutating it in place.
    fn rebuild_layout_with_spacing(&mut self, spacing: u32) {
        let count = self.layout.pane_count();
        let ids = self.layout.pane_ids().to_vec();
        let ratios = self.layout.ratios().to_vec();
        let minimums: Vec<u32> = (0..count).map(|index| self.layout.min_size(index)).collect();
        let maximums: Vec<u32> = (0..count).map(|index| self.layout.max_size(index)).collect();
        let permissions: Vec<bool> =
            (0..count).map(|index| self.layout.collapsible(index)).collect();
        let mut rebuilt = SplitterLayout::new(self.layout.orientation(), spacing);
        for pane_id in &ids {
            rebuilt.add_pane(*pane_id, 1);
        }
        let _ = rebuilt.set_ratios(ratios);
        for index in 0..count {
            let _ = rebuilt.set_min_size(index, minimums[index]);
            let _ = rebuilt.set_max_size(index, maximums[index]);
            let _ = rebuilt.set_collapsible(index, permissions[index]);
        }
        self.layout = rebuilt;
    }

    /// Whether pane `index` is currently collapsed.
    pub fn pane_collapsed(&self, index: usize) -> bool {
        self.collapsed.get(index).copied().unwrap_or(false)
    }

    /// The indices of every collapsed pane, in pane order.
    pub fn collapsed_panes(&self) -> Vec<usize> {
        self.collapsed
            .iter()
            .enumerate()
            .filter_map(|(index, collapsed)| collapsed.then_some(index))
            .collect()
    }

    /// Collapses or expands pane `index`.
    ///
    /// Collapsing stores the pane's current weight and sets it to zero, so the remaining panes
    /// take the freed space; expanding restores that weight. Returns `false` when the index is
    /// out of range or the pane is not collapsible.
    pub fn set_pane_collapsed(&mut self, index: usize, collapsed: bool) -> bool {
        if index >= self.layout.pane_count() || !self.layout.collapsible(index) {
            return false;
        }
        if self.pane_collapsed(index) == collapsed {
            return true;
        }
        if collapsed {
            let ratio = self.layout.ratio(index).unwrap_or(0.0).max(0.0);
            if let Some(slot) = self.pre_collapse_ratios.get_mut(index) {
                *slot = ratio;
            }
            self.layout.set_ratio(index, 0.0);
        } else {
            let restored = self.pre_collapse_ratios.get(index).copied().unwrap_or(1.0);
            self.layout.set_ratio(index, if restored > 0.0 { restored } else { 1.0 });
        }
        if let Some(slot) = self.collapsed.get_mut(index) {
            *slot = collapsed;
        }
        self.place_panes();
        self.base.request_redraw();
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }

    /// Gives every pane an equal share.
    ///
    /// This is the double-click-a-divider action when the adjacent pane cannot collapse, and
    /// the "reset layout" operation a host may expose. It emits `pane_layout_changed` once and
    /// repaints.
    pub fn equalize_panes(&mut self) {
        let count = self.pane_count();
        if count == 0 {
            return;
        }
        let ratios: Vec<f32> = (0..count).map(|_| 1.0).collect();
        let _ = self.layout.set_ratios(ratios);
        self.layout.normalize_ratios();
        for index in 0..count {
            if let Some(slot) = self.collapsed.get_mut(index) {
                *slot = false;
            }
            if let Some(slot) = self.pre_collapse_ratios.get_mut(index) {
                *slot = 1.0;
            }
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.place_panes();
        self.base.request_redraw();
    }

    /// Serialises the pane layout (ratios, minimums and collapse flags).
    ///
    /// The layout's own `SPL1` blob is followed by a wrapper trailer carrying the divider
    /// width and the per-pane collapsed bits, so a restored session keeps both. A blob without
    /// the trailer (produced before this wrapper existed) still restores.
    pub fn save_state(&self) -> Vec<u8> {
        let mut out = self.layout.save_state();
        out.extend_from_slice(b"SPW1");
        out.extend_from_slice(&self.handle_width.to_le_bytes());
        out.extend_from_slice(&(self.collapsed.len() as u32).to_le_bytes());
        for collapsed in &self.collapsed {
            out.push(u8::from(*collapsed));
        }
        out
    }

    /// Restores a state produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob or a pane count that no longer matches,
    /// leaving the current layout untouched.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        const HEADER: usize = 8;
        if state.len() < HEADER || &state[..4] != b"SPL1" {
            return false;
        }
        let count = u32::from_le_bytes([state[4], state[5], state[6], state[7]]) as usize;
        let layout_len = HEADER + count * 9;
        if state.len() < layout_len || count != self.layout.pane_count() {
            return false;
        }
        let (layout_state, trailer) = if state.len() == layout_len {
            (&state[..layout_len], None)
        } else {
            // marker(4) + handle width(4) + collapsed count(4) + one bit byte per pane.
            const MARKER: usize = 12;
            let expected = layout_len + MARKER + count;
            if state.len() != expected || &state[layout_len..layout_len + 4] != b"SPW1" {
                return false;
            }
            let width = u32::from_le_bytes([
                state[layout_len + 4],
                state[layout_len + 5],
                state[layout_len + 6],
                state[layout_len + 7],
            ]);
            let collapsed_count = u32::from_le_bytes([
                state[layout_len + 8],
                state[layout_len + 9],
                state[layout_len + 10],
                state[layout_len + 11],
            ]) as usize;
            if collapsed_count != count {
                return false;
            }
            let bits_start = layout_len + 12;
            let bits = state[bits_start..bits_start + count].to_vec();
            (&state[..layout_len], Some((width, bits)))
        };
        if let Some((width, _)) = &trailer {
            // Rebuild before restoring: the replay uses the pane list the caller has already
            // added, and the ratio bytes then land in the new layout.
            self.rebuild_layout_with_spacing((*width).max(1));
            self.handle_width = (*width).max(1);
        }
        if !self.layout.restore_state(layout_state) {
            return false;
        }
        if let Some((_, bits)) = trailer {
            for (index, bit) in bits.into_iter().enumerate() {
                if let Some(slot) = self.collapsed.get_mut(index) {
                    *slot = bit != 0;
                }
            }
        } else {
            // A legacy blob has no collapse bits; a zero weight is the only representation it
            // could carry, so derive the flags from the restored weights.
            for index in 0..count {
                if let Some(slot) = self.collapsed.get_mut(index) {
                    *slot = self.layout.ratio(index).unwrap_or(1.0) <= 0.0;
                }
            }
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.place_panes();
        self.base.request_redraw();
        true
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.base.request_redraw();
    }

    fn pane_rects(&self) -> Vec<(ObjectId, Rect)> {
        let mut rects = Vec::new();
        self.layout.update(self.base.geometry(), &mut |id, rect| rects.push((id, rect)));
        rects
    }

    /// The extent the pane ratios divide: the control's axis extent minus the reserved gaps.
    fn ratio_extent(&self) -> f32 {
        let rect = self.base.geometry();
        let full =
            if self.orientation() == Orientation::Horizontal { rect.width } else { rect.height }
                as f32;
        let gaps = self.pane_count().saturating_sub(1) as f32 * self.handle_width as f32;
        (full - gaps).max(0.0)
    }

    /// Clamps a pane's ratio so its pixel extent does not exceed its maximum (C-22).
    fn clamped_ratio(&self, index: usize, ratio: f32) -> f32 {
        let Some(max) = self.max_pane_size(index) else {
            return ratio;
        };
        let extent = self.ratio_extent();
        let total: f32 = self.layout.ratios().iter().copied().sum();
        if extent <= 0.0 || total <= 0.0 {
            return ratio;
        }
        // `ratio / total * extent` is the pane's pixel width; solving that for `max` gives the
        // weight cap. Keeping the total unchanged leaves every other pane's share intact.
        ratio.min((max as f32 / extent) * total)
    }

    /// Clamps every ratio whose pane exceeds its maximum (C-22).
    fn clamp_ratios_to_max(&self, ratios: &[f32]) -> Vec<f32> {
        let extent = self.ratio_extent();
        let total: f32 = ratios.iter().copied().sum();
        if extent <= 0.0 || total <= 0.0 {
            return ratios.to_vec();
        }
        ratios
            .iter()
            .enumerate()
            .map(|(index, ratio)| match self.max_pane_size(index) {
                Some(max) => ratio.min((max as f32 / extent) * total),
                None => *ratio,
            })
            .collect()
    }

    /// Scales `index`'s weight down until the produced pane rectangle fits its maximum (C-22).
    ///
    /// The ratio-space cap above is the first approximation; the engine may normalise the weights
    /// after a write, so the authoritative answer is the rectangle it actually produced. Scaling by
    /// `max / produced` is a single-step convergence: a pane's pixel extent is monotonic in its own
    /// weight, and the scaled result can only undershoot the cap, never overshoot it.
    fn converge_pane_to_maximum(&mut self, index: usize) {
        let Some(max) = self.max_pane_size(index) else {
            return;
        };
        let horizontal = self.orientation() == Orientation::Horizontal;
        let produced = self
            .pane_rects()
            .get(index)
            .map(|(_, rect)| if horizontal { rect.width } else { rect.height })
            .unwrap_or(0);
        if produced == 0 || produced <= max {
            return;
        }
        let Some(current) = self.layout.ratio(index) else {
            return;
        };
        let scaled = current * (max as f32 / produced as f32);
        let _ = self.layout.set_ratio(index, scaled);
        self.place_panes();
    }

    /// Places every pane in the rectangle the layout gives it.
    ///
    /// The panes are ordinary tree children, so the tree painter draws them and the hit test
    /// reaches them directly; this is the placement half of that model, not a second drawing
    /// path. The layout reserves [`HANDLE_WIDTH`] between adjacent panes, which is what keeps
    /// the divider's grab area free of pane rectangles — a press there therefore lands on the
    /// splitter and can begin a drag.
    fn place_panes(&self) {
        #[cfg(not(alloc_frugal))]
        for (pane_id, rect) in self.pane_rects() {
            // C-P07: only the panes whose rectangle actually changed are written. A handle drag
            // changes the two adjacent panes, while this loop used to rewrite all of them on
            // every pointer sample — a global registry write per pane per event.
            if crate::widget_geometry(pane_id) == Some((rect.x, rect.y, rect.width, rect.height)) {
                continue;
            }
            crate::set_widget_geometry(pane_id, rect.x, rect.y, rect.width, rect.height);
        }
    }

    /// The dividers this control paints, one per gap between panes.
    ///
    /// The divider belongs at the pane boundary, so this takes the edge between pane `index` and
    /// pane `index + 1`: `pane[index].end() + spacing / 2`, the midpoint of the gap the layout
    /// reserves. The layout reads a stored ratio as a **relative weight**
    /// (`weights[i] / sum(weights)`), which is why the boundary is read from the produced
    /// rectangles rather than accumulated from the weights: the old accumulation only agreed with
    /// the panes while the weights happened to sum to `1.0`, and two equal panes on a 300 px
    /// splitter put the painted handle at `x=300` while the real boundary was at `x=150`.
    ///
    /// With fewer than two panes there is no boundary to draw between, so a single affordance is
    /// placed at the middle of the control. That keeps the divider — which *is* the drag handle —
    /// discoverable on an empty splitter, while never inventing a position that disagrees with the
    /// pane layout.
    fn divider_centers(&self) -> Vec<i32> {
        self.divider_centers_from(&self.pane_rects())
    }

    /// The divider centres for an already-computed pane arrangement (C-P06).
    ///
    /// The full derivation asks the layout once and derives every centre by differencing
    /// adjacent rectangles. The old form asked the layout again for each gap — `(n - 1)` full
    /// `SplitterLayout::update` runs per draw — which is O(n²) for n panes.
    fn divider_centers_from(&self, rects: &[(ObjectId, Rect)]) -> Vec<i32> {
        let count = self.pane_count();
        if count < 2 {
            let rect = self.base.geometry();
            return crate::compat::vec![match self.orientation() {
                Orientation::Horizontal => rect.x + rect.width as i32 / 2,
                Orientation::Vertical => rect.y + rect.height as i32 / 2,
            }];
        }
        let mut centers = Vec::with_capacity(count.saturating_sub(1));
        for index in 0..count - 1 {
            let (Some((_, left)), Some((_, right))) = (rects.get(index), rects.get(index + 1))
            else {
                continue;
            };
            centers.push(match self.orientation() {
                Orientation::Horizontal => {
                    let left_end = left.x + left.width as i32;
                    left_end + (right.x - left_end) / 2
                }
                Orientation::Vertical => {
                    let left_end = left.y + left.height as i32;
                    left_end + (right.y - left_end) / 2
                }
            });
        }
        centers
    }
}
impl SaveState for Splitter {
    fn state_version(&self) -> u16 {
        // The version stays 1: the wrapper trailer is self-describing (the `SPW1` marker and an
        // explicit byte count), and a blob without it is still accepted, so an older saved
        // session keeps restoring through the same gate.
        1
    }

    fn save_state(&self) -> Vec<u8> {
        Splitter::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        Splitter::restore_state(self, state)
    }
}

impl Widget for Splitter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn save_state_dyn(&self) -> Option<&dyn SaveState> {
        Some(self)
    }

    fn save_state_dyn_mut(&mut self) -> Option<&mut dyn SaveState> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }

    /// Moves the splitter and re-places every pane.
    ///
    /// A layout positions the splitter through the same geometry entry point, so this is where
    /// a window or panel resize reaches the panes rather than leaving them in the old box.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_panes();
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
            "pane_layout_changed" => Some(EventSignalRef::mapped(
                "pane_layout_changed",
                &self.pane_layout_changed,
                // The declared shape is `ListScalar` of `Float`, so the payload arrives as the
                // ratio vector rather than its `Debug` spelling — a subscriber that read the
                // schema can read each pane's ratio as a number.
                |v| {
                    CapabilityValue::Tuple(
                        v.iter().map(|ratio| CapabilityValue::Float(*ratio as f64)).collect(),
                    )
                },
            )),
            "orientation_changed" => Some(EventSignalRef::mapped(
                "orientation_changed",
                &self.orientation_changed,
                |v| CapabilityValue::String(orientation_to_str(*v).to_string()),
            )),
            "pane_layout_preview" => Some(EventSignalRef::mapped(
                "pane_layout_preview",
                &self.pane_layout_preview,
                |ratios| {
                    CapabilityValue::Tuple(
                        ratios
                            .iter()
                            .map(|ratio| CapabilityValue::Float(f64::from(*ratio)))
                            .collect(),
                    )
                },
            )),
            // Preview is not a published hub event; it is exposed as a Rust signal for a host
            // drawing live drag feedback. Committed changes still use `pane_layout_changed`.
            _ => None,
        }
    }
}

/// `Splitter`'s property contract.
impl WidgetProperties for Splitter {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "pane_count" => Ok(CapabilityValue::UInt(self.pane_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            // Derived from the registered panes.
            "pane_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SPLITTER_PROPERTIES`.
        property_names_of!["orientation", "pane_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for Splitter {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // The splitter's own surface and its handles resolve the explicit style first, then the
        // theme's resolved style for this control, and only then a literal. Both were literals, so
        // a light/dark switch left the divider unchanged — the rendering census reported the control
        // as theme-blind. It also painted nothing at all until it had a second pane, so a freshly
        // constructed splitter reported `ink = 0`; the track below is drawn in every state.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant. `ThemeSnapshot` folds the resolved style and the palette roles into
        // one visit for the frame (C-P01).
        let theme = ThemeSnapshot::capture("splitter");
        let style = self.base.style();
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = match theme.palette {
            Some(active) => (active.background, active.foreground, active.secondary),
            None => (
                crate::core::Color::rgb(240, 240, 240),
                crate::core::Color::BLACK,
                crate::core::Color::rgb(158, 158, 158),
            ),
        };
        let ink = style
            .text_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `splitter` is absent from `WidgetRole::for_kind_name`'s table, so it classifies as
        // `Surface` and the active theme writes the window fill into `style.background_color`. A
        // track painted in that colour would be byte-identical to the frame behind it, so a
        // resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let track = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let handle_fill = track.blend(&ink, 0.14);
        let handle_border = style
            .border_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != handle_fill)
            .unwrap_or_else(|| track.blend(&secondary, 0.45));

        // The track is the surface the panes sit on, drawn before the panes so a splitter with no
        // panes yet — or one whose registry is empty — still shows where it is.
        context.face_with_gradient_corners(
            rect,
            track,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );

        // C-P06: one layout pass for the pane rectangles, shared by the draw and by the divider
        // positions derived from the same rectangles.
        let pane_rects = self.pane_rects();
        if let Some(ref registry) = self.registry {
            for (pane_id, pane_rect) in &pane_rects {
                context.push_clip(pane_rect.x, pane_rect.y, pane_rect.width, pane_rect.height);
                registry.borrow_mut().draw_widget(*pane_id, context);
                context.pop_clip();
            }
        }
        // Draw splitter handles between panes.
        //
        // The count guard used to be `pane_count() > 1`, so a splitter with no panes yet —
        // which is exactly what `Splitter::new` builds, and what the factory produces — drew
        // no handle at all. It then rendered as a featureless slab and no chrome, `detail = 0`
        // in the rendering census. A scroll view and SwiftUI's `HSplitView` all draw the
        // divider regardless of how many panes exist, because the divider *is* the affordance —
        // an invisible one is a control a user cannot find or drag.
        //
        // The positions come from `divider_centers`, which asks the *layout* where the pane
        // boundaries are. Deriving them here from raw ratios was a defect: the layout reads a
        // ratio as a relative weight (`weight / sum`), so two equal panes put the boundary at
        // the middle while the raw-weight accumulation put the handle at the right edge.
        let handle_width = self.handle_width as i32;
        let centers = self.divider_centers_from(&pane_rects);
        match self.orientation() {
            Orientation::Horizontal => {
                // Draw vertical splitter handles, one in each gap between panes.
                for x in centers {
                    let handle_rect =
                        Rect::new(x - handle_width / 2, rect.y, handle_width as u32, rect.height);
                    context.fill_rect(handle_rect, handle_fill);
                    context.draw_rect(handle_rect, handle_border);
                }
            }
            Orientation::Vertical => {
                // Draw horizontal splitter handles, one in each gap between panes.
                for y in centers {
                    let handle_rect =
                        Rect::new(rect.x, y - handle_width / 2, rect.width, handle_width as u32);
                    context.fill_rect(handle_rect, handle_fill);
                    context.draw_rect(handle_rect, handle_border);
                }
            }
        }
    }
}
impl crate::event::EventHandler for Splitter {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos, button, .. }
                if *button == 1 && self.pane_count() > 1 =>
            {
                self.begin_handle_drag(*pos);
            }
            crate::event::Event::MouseMove { pos } if self.drag_session.is_some() => {
                self.update_handle_drag(*pos);
            }
            crate::event::Event::MouseRelease { button, .. } if *button == 1 => {
                self.end_handle_drag();
            }
            crate::event::Event::MouseDoubleClick { pos, button: 1 } => {
                let coordinate = match self.orientation() {
                    Orientation::Horizontal => pos.x,
                    Orientation::Vertical => pos.y,
                };
                let hit = self.divider_centers().iter().position(|center| {
                    (coordinate - *center).abs() <= self.handle_width as i32 / 2
                });
                if let Some(index) = hit {
                    // Double-clicking a divider collapses the trailing pane; when that pane is
                    // not collapsible, fall back to the historical equalize action.
                    let trailing = index + 1;
                    if trailing < self.pane_count() && self.pane_collapsible(trailing) {
                        let collapsed = !self.pane_collapsed(trailing);
                        let _ = self.set_pane_collapsed(trailing, collapsed);
                    } else {
                        self.equalize_panes();
                    }
                }
            }
            _ => {}
        }
        if let Some(ref reg) = self.registry {
            let target = match event {
                crate::event::Event::MousePress { pos, .. }
                | crate::event::Event::MouseRelease { pos, .. }
                | crate::event::Event::MouseMove { pos } => {
                    self.pane_rects().iter().position(|(_, pane)| {
                        pos.x >= pane.x
                            && pos.x < pane.x + pane.width as i32
                            && pos.y >= pane.y
                            && pos.y < pane.y + pane.height as i32
                    })
                }
                _ => self.active_pane,
            };
            if let Some(index) = target {
                if let Some(pane_id) = self.pane_ids().get(index) {
                    let _ = reg.borrow_mut().forward_event(*pane_id, event);
                }
            }
        }
    }
}

impl Splitter {
    /// The splitter's extent along the axis its handles travel, in pixels.
    ///
    /// A horizontal splitter divides width; a vertical one divides height. Named
    /// once so the three drag helpers cannot disagree about which dimension a
    /// handle moves in.
    fn primary_extent(&self) -> f32 {
        let rect = self.base.geometry();
        if self.orientation() == Orientation::Horizontal {
            rect.width as f32
        } else {
            rect.height as f32
        }
    }

    /// The pointer's position along the handle axis, relative to the widget.
    fn primary_offset(&self, pos: crate::core::Point) -> f32 {
        let rect = self.base.geometry();
        if self.orientation() == Orientation::Horizontal {
            pos.x as f32 - rect.x as f32
        } else {
            pos.y as f32 - rect.y as f32
        }
    }

    /// Starts a handle drag when `pos` lands on a divider.
    ///
    /// Opens a [`DragSession`] — the shared state machine — and records the
    /// splitter-specific snapshot beside it. Pressing away from any divider leaves
    /// no session, so a subsequent move does not resize anything.
    ///
    /// The hit test uses the same [`Self::divider_centers`] derivation the fill uses, so
    /// the grab area is exactly the handle the user can see.
    fn begin_handle_drag(&mut self, pos: crate::core::Point) {
        // C-P06: one layout pass serves both the hit test and the divider positions.
        let rects = self.pane_rects();
        if let Some(index) = rects.iter().position(|(_, pane)| {
            pos.x >= pane.x
                && pos.x < pane.x + pane.width as i32
                && pos.y >= pane.y
                && pos.y < pane.y + pane.height as i32
        }) {
            self.active_pane = Some(index);
        }

        let pos_primary = self.primary_offset(pos);
        for (index, center) in self.divider_centers_from(&rects).into_iter().enumerate() {
            let center = match self.orientation() {
                Orientation::Horizontal => center as f32 - self.base.geometry().x as f32,
                Orientation::Vertical => center as f32 - self.base.geometry().y as f32,
            };
            if (pos_primary - center).abs() <= self.handle_width as f32 / 2.0 {
                // The payload names the handle, so a future drop target can tell
                // which divider the drag came from.
                let payload =
                    DragPayload::new(SPLITTER_DRAG_TYPE, index.to_string()).with_origin(pos);
                self.drag_session = Some(DragSession::begin(payload, pos));
                self.drag_state = Some(HandleDrag {
                    handle_index: index,
                    start_ratios: self.layout.ratios().to_vec(),
                });
                self.preview_mark = None;
                break;
            }
        }
    }

    /// Applies a move to the ratios of the handle currently being dragged.
    ///
    /// The delta is measured from the **session's** start, not from the previous
    /// frame, so the rule is absolute: a move that is replayed (a coalesced event,
    /// a re-delivery) produces the same layout rather than doubling the change.
    fn update_handle_drag(&mut self, pos: crate::core::Point) {
        // The session is touched first, then dropped: the ratio update needs
        // `self.layout` mutably, so holding a borrow of `self.drag_session` across
        // it would not compile. Reading what is needed up front also makes the
        // function's inputs explicit.
        let (is_active, start) = {
            let Some(session) = self.drag_session.as_mut() else {
                return;
            };
            // The drag only becomes active past the threshold; before that the
            // gesture is still a candidate click and must not resize anything.
            session.update(pos, SPLITTER_DRAG_THRESHOLD);
            (session.is_active(), session.start())
        };
        if !is_active {
            return;
        }
        let Some(snapshot) = self.drag_state.clone() else {
            return;
        };

        let total = self.primary_extent();
        if total <= 0.0 {
            return;
        }
        // Delta is taken from the session's recorded start, so a caller cannot
        // invent one and so a re-delivered move is idempotent.
        let delta = self.primary_offset(pos) - self.primary_offset(start);
        let index = snapshot.handle_index;
        let left = snapshot.start_ratios.get(index).copied().unwrap_or(0.0);
        let right = snapshot.start_ratios.get(index + 1).copied().unwrap_or(0.0);

        // Convert pixels to ratio units, clamp both sides at zero so a handle can
        // reach its neighbour's edge but not cross it, then rescale the pair back
        // to the weight it started with. That rescale is what keeps the *other*
        // panes exactly as they were: only the two adjacent ratios move.
        let ratio_delta = delta / total;
        let new_left = (left + ratio_delta).max(0.0);
        let new_right = (right - ratio_delta).max(0.0);
        let pair_sum = left + right;
        if pair_sum <= 0.0 {
            return;
        }
        let new_pair_sum = new_left + new_right;
        // Both sides pinned at zero (a zero-width pane pair) has no scale to
        // recover; leaving the ratios alone is the honest answer rather than
        // dividing by zero.
        if new_pair_sum <= 0.0 {
            return;
        }
        let scale = pair_sum / new_pair_sum;
        // C-22: a pane at its maximum stops the pair there, so the cap is a real drag limit
        // rather than a value a later write can silently exceed.
        let extent = self.ratio_extent();
        let total: f32 = self.layout.ratios().iter().copied().sum();
        let mut left_ratio = new_left * scale;
        let mut right_ratio = new_right * scale;
        if extent > 0.0 && total > 0.0 {
            if let Some(max) = self.max_pane_size(index) {
                left_ratio = left_ratio.min((max as f32 / extent) * total);
            }
            if let Some(max) = self.max_pane_size(index + 1) {
                right_ratio = right_ratio.min((max as f32 / extent) * total);
            }
        }
        self.layout.set_ratio(index, left_ratio);
        self.layout.set_ratio(index + 1, right_ratio);
        self.converge_pane_to_maximum(index);
        self.converge_pane_to_maximum(index + 1);
        // The panes follow the ratios while the handle is moving, not only on release. The
        // committed signal waits for release: a per-move `pane_layout_changed` made one gesture
        // look like hundreds of layout changes and allocated a ratio vector per pointer sample.
        // `pane_layout_preview` is the live channel, throttled to a visible ratio step.
        // The preview payload is a `Vec<f32>` because that is the published signal's type; the
        // per-move cost that remains is that payload, and it is only allocated on a visible ratio
        // step (throttled above) rather than on every pointer sample.
        self.place_panes();
        if self.pane_layout_preview.slot_count() > 0 {
            let mark = ((new_left * 1000.0).round() as i64, (new_right * 1000.0).round() as i64);
            if self.preview_mark != Some(mark) {
                self.preview_mark = Some(mark);
                self.pane_layout_preview.emit(self.layout.ratios().to_vec());
            }
        }
    }

    /// Ends the handle drag, normalising the ratios and clearing the session.
    ///
    /// The normalisation is what keeps the ratios summing to one after a series of
    /// rescaled pairs, so a later `ratio(i)` reads a meaningful fraction rather
    /// than an unnormalised weight.
    fn end_handle_drag(&mut self) {
        if self.drag_session.take().is_none() {
            return;
        }
        self.drag_state = None;
        self.preview_mark = None;
        self.layout.normalize_ratios();
        self.place_panes();
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
    }

    /// Whether a handle drag is currently in progress.
    ///
    /// Public so a caller (or a test) can observe the drag state without reading
    /// the private session, which is what lets the drag be asserted rather than
    /// inferred from the ratios it produced.
    pub fn is_dragging_handle(&self) -> bool {
        self.drag_session.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Orientation, Rect};
    use crate::event::EventHandler as _;
    use crate::object::ObjectId;

    #[test]
    fn splitter_creation_defaults() {
        let sp = Splitter::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sp.geometry(), Rect::new(0, 0, 300, 200));
        assert_eq!(sp.orientation(), Orientation::Horizontal);
        assert_eq!(sp.pane_count(), 0);
    }

    /// Pane ids used by these tests.
    ///
    /// They sit far above the object-id sequence on purpose: `add_child_linked` refuses a child
    /// whose id equals the container's own id, and the object counter is global, so a literal `1`
    /// made these tests depend on how many objects earlier tests had allocated. A high id cannot
    /// collide with the splitter's own.
    const TEST_PANE_A: ObjectId = 1 << 40;
    const TEST_PANE_B: ObjectId = (1 << 40) + 1;

    #[test]
    fn splitter_add_and_remove_pane() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        let pane_id: ObjectId = TEST_PANE_A;
        let idx = sp.add_pane(pane_id, 1);
        assert_eq!(idx, 0);
        assert_eq!(sp.pane_count(), 1);
        assert_eq!(sp.pane_ids(), &[TEST_PANE_A]);
        assert!(sp.remove_pane(pane_id));
        assert_eq!(sp.pane_count(), 0);
    }

    #[test]
    fn splitter_set_orientation() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        sp.set_orientation(Orientation::Vertical);
        assert_eq!(sp.orientation(), Orientation::Vertical);
    }

    // ── A-3: the handle drag now runs on `DragSession` ─────────────────────

    /// Two equal panes across a 300px horizontal splitter.
    fn two_pane_splitter() -> Splitter {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        sp.add_pane(TEST_PANE_A, 1);
        sp.add_pane(TEST_PANE_B, 1);
        sp
    }

    /// A saved layout survives a rebuild: read the weights, build a fresh splitter, restore them.
    #[test]
    fn splitter_set_ratios_round_trips_through_a_rebuild() {
        let mut source = two_pane_splitter();
        let saved = [0.25f32, 0.75];
        assert!(source.set_ratios(&saved));
        assert_eq!(source.ratios(), saved.to_vec());

        let saved_ratios = source.ratios();
        let mut rebuilt = Splitter::new(source.geometry());
        rebuilt.add_pane(TEST_PANE_A, 1);
        rebuilt.add_pane(TEST_PANE_B, 1);
        assert!(rebuilt.set_ratios(&saved_ratios));
        assert_eq!(
            rebuilt.ratios(),
            saved_ratios,
            "a splitter restored from saved weights must split its panes identically"
        );
    }

    /// The x coordinate of the divider between two equal panes.
    ///
    /// Read from the control's own `divider_centers`, which is the same derivation the
    /// fill, the hit test and the drag use. It used to recompute `ratio(0) * width`
    /// here — the *raw-weight* arithmetic the control itself had, which put the handle
    /// at the right edge instead of between the panes. Recomputing it in the test meant
    /// the two wrong derivations agreed with each other and the defect stayed green.
    fn divider_x(sp: &Splitter) -> i32 {
        sp.divider_centers().first().copied().unwrap_or(sp.geometry().x)
    }

    /// The divider is painted and grabbed in the gap between the panes.
    ///
    /// `add_pane` stores its `stretch` as a **relative weight**, and the layout divides
    /// by the sum, so two equal panes split the pane budget evenly. The control used to
    /// accumulate the raw weights instead, which put the handle at 300 — off the right
    /// edge and nowhere near the panes it separates. The gap is the handle's own width:
    /// with no gap the panes met edge to edge and covered the grab area, so a press on
    /// the visible divider landed on a pane instead of the splitter.
    #[test]
    fn the_divider_sits_between_the_panes() {
        let sp = two_pane_splitter();
        // The panes themselves, from the layout the control lays them out with.
        let mut rects = crate::compat::Vec::new();
        sp.layout.update(sp.geometry(), &mut |id, rect| rects.push((id, rect)));
        let (first, second) = (rects[0].1, rects[1].1);
        let first_end = first.x + first.width as i32;

        assert_eq!(
            second.x - first_end,
            HANDLE_WIDTH as i32,
            "the layout must reserve the handle's grab area between panes"
        );
        assert_eq!(
            first.width + second.width + HANDLE_WIDTH,
            sp.geometry().width,
            "the panes and the handle tile the splitter exactly"
        );

        let divider = divider_x(&sp);
        assert!(divider > first_end, "the divider starts after the first pane");
        assert!(divider < second.x, "and ends before the second");
        // The painted handle is `center - width/2 .. center - width/2 + width`, which has to
        // be exactly the gap the hit test accepts.
        assert_eq!(divider - HANDLE_WIDTH as i32 / 2, first_end);
        assert_eq!(divider + HANDLE_WIDTH as i32 / 2 + 1, second.x);
    }

    /// A pane is part of the splitter's tree, so the tree painter draws it and the hit test
    /// can reach it without a draw-closure registry.
    #[test]
    fn panes_are_linked_as_children() {
        let mut sp = two_pane_splitter();
        assert_eq!(sp.children(), &[TEST_PANE_A, TEST_PANE_B]);
        assert!(sp.remove_pane(TEST_PANE_A));
        assert_eq!(sp.children(), &[TEST_PANE_B]);
    }

    #[test]
    fn splitter_drag_opens_and_closes_a_session() {
        let mut sp = two_pane_splitter();
        assert!(!sp.is_dragging_handle());

        sp.handle_event(&crate::event::Event::mouse_press(divider_x(&sp), 100, 1));
        assert!(sp.is_dragging_handle(), "pressing the divider opens a drag session");

        sp.handle_event(&crate::event::Event::mouse_release(50, 100, 1));
        assert!(!sp.is_dragging_handle(), "releasing closes it");
    }

    #[test]
    fn splitter_press_away_from_a_divider_opens_no_session() {
        let mut sp = two_pane_splitter();
        // x=20 is inside the first pane, nowhere near the divider.
        sp.handle_event(&crate::event::Event::mouse_press(20, 100, 1));
        assert!(!sp.is_dragging_handle());
    }

    #[test]
    fn splitter_drag_moves_the_ratio_towards_the_pointer() {
        let mut sp = two_pane_splitter();
        let before = sp.ratio(0).expect("ratio 0");
        let divider = divider_x(&sp);

        sp.handle_event(&crate::event::Event::mouse_press(divider, 100, 1));
        // Drag 60px to the right: the first pane must grow.
        sp.handle_event(&crate::event::Event::mouse_move(divider + 60, 100));
        let during = sp.ratio(0).expect("ratio 0");

        assert!(during > before, "a rightward drag grows the leading pane: {before} -> {during}");
    }

    #[test]
    fn splitter_drag_below_the_threshold_does_not_resize() {
        let mut sp = two_pane_splitter();
        let before = sp.ratio(0).expect("ratio 0");
        let divider = divider_x(&sp);

        sp.handle_event(&crate::event::Event::mouse_press(divider, 100, 1));
        // One pixel is under the 2px drag threshold: still a click.
        sp.handle_event(&crate::event::Event::mouse_move(divider + 1, 100));
        assert_eq!(
            sp.ratio(0).expect("ratio 0"),
            before,
            "a sub-threshold move must not resize anything"
        );
    }

    #[test]
    fn splitter_drag_cannot_invert_the_pane_pair() {
        let mut sp = two_pane_splitter();
        let divider = divider_x(&sp);
        // The two panes start with equal weights; the pair's total is what a drag
        // must preserve.
        let start_pair_sum = sp.ratio(0).expect("ratio 0") + sp.ratio(1).expect("ratio 1");

        sp.handle_event(&crate::event::Event::mouse_press(divider, 100, 1));
        // Drag far past the far edge: the first pane may reach the whole weight but
        // the second may not become negative.
        sp.handle_event(&crate::event::Event::mouse_move(divider + 500, 100));
        let first = sp.ratio(0).expect("ratio 0");
        let second = sp.ratio(1).expect("ratio 1");
        assert!(first >= 0.0, "a ratio cannot go negative: {first}");
        assert!(second >= 0.0, "a ratio cannot go negative: {second}");
        assert_eq!(second, 0.0, "dragging past the edge collapses the trailing pane");
        // The *weights* are preserved during the drag; `normalize_ratios` on
        // release is what turns them into fractions. A drag that broke this would
        // silently change the split of every other pane in the widget.
        assert!(
            (first + second - start_pair_sum).abs() < 0.01,
            "a drag preserves the pair's total weight: {first} + {second} vs {start_pair_sum}"
        );

        // And releasing normalises them, so a later read is a real fraction.
        sp.handle_event(&crate::event::Event::mouse_release(divider + 500, 100, 1));
        let first = sp.ratio(0).expect("ratio 0");
        let second = sp.ratio(1).expect("ratio 1");
        assert!(
            (first + second - 1.0).abs() < 0.01,
            "releasing normalises the pair to one: {first} + {second} = {}",
            first + second
        );
        assert!(first > 0.99, "the leading pane took the whole width: {first}");
    }

    #[test]
    fn splitter_orientation_selects_which_axis_a_handle_moves_on() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        sp.set_orientation(Orientation::Vertical);
        sp.add_pane(TEST_PANE_A, 1);
        sp.add_pane(TEST_PANE_B, 1);

        // The divider's y comes from the control's own derivation, the same one the fill
        // and the hit test use. Recomputing `ratio(0) * height` here (the raw weight) put
        // it at 200 — the bottom edge — while the panes actually meet at 100.
        let divider_y = sp.divider_centers()[0];
        assert_eq!(divider_y, 100, "two equal panes meet halfway down a 200px splitter");
        let before = sp.ratio(0).expect("ratio 0");

        sp.handle_event(&crate::event::Event::mouse_press(100, divider_y, 1));
        assert!(sp.is_dragging_handle(), "the divider is grabbable where it is drawn");
        sp.handle_event(&crate::event::Event::mouse_move(100, divider_y + 40));
        assert!(sp.ratio(0).expect("ratio 0") > before, "a vertical splitter's handle moves on y");
    }

    #[test]
    fn splitter_disabled_ignores_a_drag() {
        let mut sp = two_pane_splitter();
        let before = sp.ratio(0).expect("ratio 0");
        let divider = divider_x(&sp);
        sp.set_enabled(false);

        sp.handle_event(&crate::event::Event::mouse_press(divider, 100, 1));
        assert!(!sp.is_dragging_handle(), "a disabled splitter must not start a drag");
        sp.handle_event(&crate::event::Event::mouse_move(divider + 60, 100));
        assert_eq!(sp.ratio(0).expect("ratio 0"), before);
    }

    #[test]
    fn splitter_repeated_move_is_idempotent() {
        // The delta is measured from the session start, so delivering the same move
        // twice must not double the resize. This is the property that makes a
        // coalesced or re-delivered event harmless.
        let mut sp = two_pane_splitter();
        let divider = divider_x(&sp);

        sp.handle_event(&crate::event::Event::mouse_press(divider, 100, 1));
        sp.handle_event(&crate::event::Event::mouse_move(divider + 40, 100));
        let once = sp.ratio(0).expect("ratio 0");
        sp.handle_event(&crate::event::Event::mouse_move(divider + 40, 100));
        let twice = sp.ratio(0).expect("ratio 0");

        assert!(
            (once - twice).abs() < f32::EPSILON,
            "a re-delivered move must be a no-op: {once} vs {twice}"
        );
    }

    /// Minimums and collapse flags delegate to the layout, a double-click on a divider
    /// equalizes the panes, and a saved state round-trips.
    #[test]
    fn minimums_state_and_double_click_equalize_are_wired() {
        let mut sp = two_pane_splitter();
        assert!(sp.set_min_pane_size(0, 24));
        assert_eq!(sp.min_pane_size(0), 24);
        assert!(!sp.set_min_pane_size(9, 1));
        assert!(sp.set_pane_collapsible(1, false));
        assert!(!sp.pane_collapsible(1));

        let state = sp.save_state();
        let mut restored = two_pane_splitter();
        assert!(restored.restore_state(&state));
        assert_eq!(restored.min_pane_size(0), 24);
        assert!(!restored.pane_collapsible(1));
        assert!(!restored.restore_state(b"bad"));

        sp.set_ratio(0, 3.0);
        sp.equalize_panes();
        assert!((sp.ratio(0).expect("ratio") - 0.5).abs() < 1e-6);

        sp.set_ratio(0, 3.0);
        let divider = divider_x(&sp);
        sp.handle_event(&crate::event::Event::MouseDoubleClick {
            pos: crate::core::Point::new(divider, 50),
            button: 1,
        });
        assert!((sp.ratio(0).expect("ratio") - 0.5).abs() < 1e-6);
    }

    /// C-22: a pane maximum is a real drag and write limit, not a stored number.
    #[test]
    fn a_pane_maximum_caps_a_ratio_write() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        sp.add_pane(TEST_PANE_A, 1);
        sp.add_pane(TEST_PANE_B, 1);
        assert_eq!(sp.max_pane_size(0), None, "unbounded by default");
        assert!(sp.set_max_pane_size(0, 50));
        assert!(sp.set_ratio(0, 0.9));
        let width = sp.pane_rects()[0].1.width;
        assert!(width <= 50, "the written ratio is clamped to the cap: {width}");
        assert!(!sp.set_max_pane_size(9, 10), "an unknown pane is refused");
    }
}
