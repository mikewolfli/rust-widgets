// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Splitter widget.
use crate::core::{Orientation, Rect};
use crate::event::{DragPayload, DragSession};
use crate::layout::{splitter::SplitterLayout, Layout};
use crate::object::ObjectId;
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_orientation, orientation_to_str};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

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
    /// Emitted after a pane ratio changes, with the full ratio vector in pane
    /// order. Ratios are relative weights, not pixels.
    pub pane_layout_changed: Signal1<Vec<f32>>,
    /// Emitted when the splitter is switched between horizontal and vertical,
    /// with the new orientation.
    pub orientation_changed: Signal1<Orientation>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// The shared drag state machine, active only while a handle is being dragged.
    drag_session: Option<DragSession>,
    /// The splitter-specific snapshot a drag started with.
    drag_state: Option<HandleDrag>,
    active_pane: Option<usize>,
}
impl Splitter {
    /// Creates an empty splitter with horizontal orientation.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Splitter, geometry, "Splitter"),
            layout: SplitterLayout::new(Orientation::Horizontal, 0),
            pane_layout_changed: Signal1::new(),
            orientation_changed: Signal1::new(),
            registry: None,
            drag_session: None,
            drag_state: None,
            active_pane: None,
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
    /// Adds one pane and returns assigned index.
    pub fn add_pane(&mut self, pane_id: ObjectId, stretch: u32) -> usize {
        let index = self.layout.add_pane(pane_id, stretch);
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        index
    }
    /// Removes one pane by object id.
    pub fn remove_pane(&mut self, pane_id: ObjectId) -> bool {
        if !self.layout.remove_pane(pane_id) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        true
    }
    /// Sets ratio for pane index.
    pub fn set_ratio(&mut self, index: usize, ratio: f32) -> bool {
        if !self.layout.set_ratio(index, ratio) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.base.request_redraw();
        true
    }
    /// Sets all pane ratios.
    pub fn set_ratios(&mut self, ratios: Vec<f32>) -> bool {
        if !self.layout.set_ratios(ratios) {
            return false;
        }
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
        }
        self.base.request_redraw();
        true
    }
    /// Normalizes ratios to sum to 1.
    pub fn normalize_ratios(&mut self) {
        self.layout.normalize_ratios();
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
}
impl Widget for Splitter {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("splitter");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.secondary)
                }
                None => (
                    crate::core::Color::rgb(240, 240, 240),
                    crate::core::Color::BLACK,
                    crate::core::Color::rgb(158, 158, 158),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
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
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != handle_fill)
            .unwrap_or_else(|| track.blend(&secondary, 0.45));

        // The track is the surface the panes sit on, drawn before the panes so a splitter with no
        // panes yet — or one whose registry is empty — still shows where it is.
        context.fill_rect(rect, track);

        if let Some(ref registry) = self.registry {
            for (pane_id, pane_rect) in self.pane_rects() {
                context.push_clip(pane_rect.x, pane_rect.y, pane_rect.width, pane_rect.height);
                registry.borrow_mut().draw_widget(pane_id, context);
                context.pop_clip();
            }
        }
        // Draw splitter handles between panes
        let handle_width = 5;
        match self.orientation() {
            Orientation::Horizontal => {
                // Draw vertical splitter handles
                if self.pane_count() > 1 {
                    let total_width = rect.width as f32;
                    let mut x = rect.x as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        x += total_width * ratio;
                        let handle_rect = Rect::new(
                            x as i32 - handle_width / 2,
                            rect.y,
                            handle_width as u32,
                            rect.height,
                        );
                        // Draw splitter handle
                        context.fill_rect(handle_rect, handle_fill);
                        context.draw_rect(handle_rect, handle_border);
                    }
                }
            }
            Orientation::Vertical => {
                // Draw horizontal splitter handles
                if self.pane_count() > 1 {
                    let total_height = rect.height as f32;
                    let mut y = rect.y as f32;
                    for i in 0..self.pane_count() - 1 {
                        let ratio = self.ratio(i).unwrap_or(0.0);
                        y += total_height * ratio;
                        let handle_rect = Rect::new(
                            rect.x,
                            y as i32 - handle_width / 2,
                            rect.width,
                            handle_width as u32,
                        );
                        // Draw splitter handle
                        context.fill_rect(handle_rect, handle_fill);
                        context.draw_rect(handle_rect, handle_border);
                    }
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
            crate::event::Event::MousePress { pos, button }
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
    fn begin_handle_drag(&mut self, pos: crate::core::Point) {
        const HANDLE_WIDTH: f32 = 5.0;

        if let Some(index) = self.pane_rects().iter().position(|(_, pane)| {
            pos.x >= pane.x
                && pos.x < pane.x + pane.width as i32
                && pos.y >= pane.y
                && pos.y < pane.y + pane.height as i32
        }) {
            self.active_pane = Some(index);
        }

        let total = self.primary_extent();
        let pos_primary = self.primary_offset(pos);
        let mut accumulated = 0.0;
        for index in 0..self.pane_count().saturating_sub(1) {
            if let Some(ratio) = self.ratio(index) {
                accumulated += ratio * total;
            }
            if (pos_primary - accumulated).abs() <= HANDLE_WIDTH / 2.0 {
                // The payload names the handle, so a future drop target can tell
                // which divider the drag came from.
                let payload =
                    DragPayload::new(SPLITTER_DRAG_TYPE, index.to_string()).with_origin(pos);
                self.drag_session = Some(DragSession::begin(payload, pos));
                self.drag_state = Some(HandleDrag {
                    handle_index: index,
                    start_ratios: self.layout.ratios().to_vec(),
                });
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
        self.layout.set_ratio(index, new_left * scale);
        self.layout.set_ratio(index + 1, new_right * scale);
        if self.pane_layout_changed.slot_count() > 0 {
            self.pane_layout_changed.emit(self.layout.ratios().to_vec());
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
        self.layout.normalize_ratios();
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

    #[test]
    fn splitter_add_and_remove_pane() {
        let mut sp = Splitter::new(Rect::new(0, 0, 300, 200));
        let pane_id: ObjectId = 1;
        let idx = sp.add_pane(pane_id, 1);
        assert_eq!(idx, 0);
        assert_eq!(sp.pane_count(), 1);
        assert_eq!(sp.pane_ids(), &[1]);
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
        sp.add_pane(1, 1);
        sp.add_pane(2, 1);
        sp
    }

    /// The x coordinate of the divider between two equal panes.
    ///
    /// Derived from the layout rather than hard-coded, so the test keeps describing
    /// "the divider" if the layout rule changes.
    fn divider_x(sp: &Splitter) -> i32 {
        let total = sp.geometry().width as f32;
        (sp.ratio(0).unwrap_or(0.0) * total).round() as i32 + sp.geometry().x
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
        sp.add_pane(1, 1);
        sp.add_pane(2, 1);

        let total = sp.geometry().height as f32;
        let divider_y = (sp.ratio(0).expect("ratio 0") * total).round() as i32;
        let before = sp.ratio(0).expect("ratio 0");

        sp.handle_event(&crate::event::Event::mouse_press(100, divider_y, 1));
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
}
