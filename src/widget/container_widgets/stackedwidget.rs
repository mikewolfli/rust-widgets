// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stacked widget.
use crate::core::{Color, ObjectId, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::save_state::SaveState;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::place::place_content_child;
use super::theme_snapshot::ThemeSnapshot;
use crate::layout::stack::StackLayout;

/// Stacked widget.
pub struct StackedWidget {
    base: BaseWidget,
    /// The page model and the geometry authority are the same object: the stack decides
    /// which page is visible, and `update_slide` produces that page's rectangle (including
    /// the slide offsets) so the placement cannot disagree with the selection.
    layout: StackLayout,
    /// Emitted with the new visible index when the selected child changes.
    /// Not emitted for inactive children, and not emitted when the same index is
    /// re-applied; it is not emitted when the widget list is mutated.
    pub current_changed: Signal1<usize>,
    /// Duration of a page slide, in milliseconds. `0` disables the transition.
    transition_ms: u32,
    /// Slide progress, `0.0` showing the previous page and `1.0` the current one.
    transition: f32,
    /// The page being slid out, while a transition is active.
    previous_page: Option<usize>,
}
impl StackedWidget {
    /// Creates a stacked widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StackedWidget, geometry, "StackedWidget"),
            layout: StackLayout::new(),
            current_changed: Signal1::new(),
            transition_ms: 0,
            transition: 1.0,
            previous_page: None,
        }
    }
    /// Adds a widget.
    ///
    /// Adding a widget that is already a page returns that page's index instead of
    /// appending a duplicate: a loader mounts a declared child under its container first
    /// and then routes it here, and two entries for one id would paint it twice and
    /// desynchronise [`Self::count`] from the tree.
    pub fn add_widget(&mut self, widget: ObjectId) -> usize {
        if let Some(index) = self.index_of(widget) {
            return index;
        }
        self.base.add_child_linked(widget);
        self.layout.insert_at(self.layout.item_count(), widget);
        let index = self.layout.item_count().saturating_sub(1);
        self.sync_pages();
        self.base.request_redraw();
        index
    }
    /// Inserts a widget at position.
    /// Inserts a widget before the given index.
    ///
    /// An `index` past the end appends rather than panicking, matching
    /// [`Self::add_widget`] and the rest of the container family (`tab_bar`,
    /// `pie_menu`, `kanban_board` all clamp).
    pub fn insert_widget(&mut self, index: usize, widget: ObjectId) {
        // A widget already in the stack is a no-op, matching `add_widget`: a second entry for
        // one id would paint it twice and make `remove_widget` leave a phantom page.
        if self.index_of(widget).is_some() {
            return;
        }
        let index = index.min(self.layout.item_count());
        self.base.add_child_linked(widget);
        // The stack's own insert keeps the selected page on the same widget. A running slide names
        // pages by index too, so it is remapped by the same rule — or dropped, for an insert that
        // splits the pair — instead of sliding the wrong widget out.
        self.remap_previous_page_for_insert(index);
        self.layout.insert_at(index, widget);
        self.sync_pages();
        self.base.request_redraw();
    }
    /// Shifts the outgoing page's index after an insertion.
    ///
    /// A running slide names pages by index, exactly like the selection the layout repairs. An
    /// insert at or before the outgoing page moves it one slot down; without this the transition
    /// slid the *wrong* widget out (or painted the current page twice).
    fn remap_previous_page_for_insert(&mut self, index: usize) {
        if self.previous_page.is_some_and(|previous| index <= previous) {
            self.previous_page = self.previous_page.map(|previous| previous + 1);
        }
    }

    /// Drops or shifts the outgoing page's index after a removal.
    fn remap_previous_page_for_remove(&mut self, index: usize) {
        self.previous_page = match self.previous_page {
            Some(previous) if previous == index => None,
            Some(previous) if previous > index => Some(previous - 1),
            other => other,
        };
        if self.previous_page.is_none() {
            self.transition = 1.0;
        }
    }

    /// Removes a widget.
    pub fn remove_widget(&mut self, widget: ObjectId) {
        if let Some(index) = self.index_of(widget) {
            self.remove_widget_at(index);
        }
    }

    /// Removes the page at `index` (C-23).
    ///
    /// Returns `false` when `index` is out of range, so a host that only has an index does
    /// not have to reverse it through [`Self::index_of`] first. The effect is exactly the
    /// one [`Self::remove_widget`] has: the child is unlinked, hidden, the selection is
    /// repaired, and the page model is re-applied. That method now delegates here, so the
    /// two addresses cannot drift.
    pub fn remove_widget_at(&mut self, index: usize) -> bool {
        // The stack repairs the selection: removing a page before it keeps the same page
        // visible, and removing the selected page selects the one that took its place.
        let Some(widget) = self.layout.remove_at(index) else {
            return false;
        };
        // A running slide may be naming the page that just left (or one after it, which shifted).
        self.remap_previous_page_for_remove(index);
        self.base.remove_child_linked(widget);
        // A removed page is no longer part of the stack; clear its visibility too, so a host
        // that renders by id cannot paint it over the selected one.
        crate::set_widget_visible(widget, false);
        self.sync_pages();
        self.base.request_redraw();
        true
    }
    /// Returns number of widgets.
    pub fn count(&self) -> usize {
        self.layout.item_count()
    }

    /// Returns the number of widgets (alias for count).
    pub fn widget_count(&self) -> usize {
        self.layout.item_count()
    }

    /// Sets the current widget by its ObjectId.
    pub fn set_current_widget(&mut self, id: ObjectId) {
        if let Some(index) = self.index_of(id) {
            self.set_current_index(index);
        }
    }
    /// Returns the selected child index, or `None` when the stack is empty.
    pub fn selected_index(&self) -> Option<usize> {
        let index = self.layout.current_index();
        (index < self.layout.item_count()).then_some(index)
    }

    /// Returns current widget index.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> usize {
        self.layout.current_index()
    }
    /// Sets current widget index.
    ///
    /// # Disabled contract
    ///
    /// `current_changed` is a *user-visible* transition: a subscriber typically reloads or
    /// saves whatever the page shows. When the widget is disabled the page is not
    /// reachable by the user, so announcing the move would be "reporting success for
    /// something that did not happen" -- the exact silent failure the `enabled`
    /// contract forbids. The index still moves (the host asked for it), but the signal
    /// does not fire; callers that need to know why can ask
    /// [`Self::current_changed_suppression_reason`].
    pub fn set_current_index(&mut self, index: usize) {
        let previous = self.layout.current_index();
        if index >= self.layout.item_count() || previous == index {
            return;
        }
        self.layout.try_set_current_index(index);
        if self.transition_ms > 0 {
            self.previous_page = Some(previous);
            self.transition = 0.0;
        } else {
            self.previous_page = None;
            self.transition = 1.0;
        }
        self.sync_pages();
        self.base.request_redraw();
        if !self.base.is_enabled() {
            // See `current_changed_suppression_reason`: a disabled container must not
            // announce a page the user cannot reach.
            return;
        }
        self.current_changed.emit(index);
    }

    /// Serialises the selected page for session restore (RW-64).
    ///
    /// The page *list* belongs to the host's model; only the selection is user state, so only
    /// the index is stored. A restore clamps an index past the current last page, because a
    /// saved preference is not an assertion that the page set did not shrink.
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(5);
        state.push(STACKED_WIDGET_STATE_VERSION);
        state.extend_from_slice(&(self.layout.current_index() as u32).to_le_bytes());
        state
    }

    /// Restores a blob produced by [`Self::save_state`].
    ///
    /// Returns `false` for a malformed blob or an empty stack, leaving the control untouched.
    pub fn restore_state(&mut self, state: &[u8]) -> bool {
        if state.len() != 5 || state[0] != STACKED_WIDGET_STATE_VERSION {
            return false;
        }
        if self.layout.item_count() == 0 {
            return false;
        }
        let index = u32::from_le_bytes([state[1], state[2], state[3], state[4]]) as usize;
        let index = index.min(self.layout.item_count() - 1);
        // A restore is a state assignment, not a user navigation, so it lands on the page
        // directly rather than starting a slide from whatever was selected before.
        self.layout.set_current_index(index);
        self.previous_page = None;
        self.transition = 1.0;
        self.sync_pages();
        self.base.request_redraw();
        true
    }

    /// Applies the page model to the mounted children: the current page is shown and
    /// fills the stack, every other page is hidden.
    ///
    /// # Why this is the control's job
    ///
    /// A stacked widget is *defined* by showing exactly one page. Linking the children
    /// was only half the model: the tree painter draws every visible child, so without
    /// this pass all pages painted on top of each other — the container looked like a
    /// panel holding several overlapping panels. Hiding the inactive pages is also what
    /// keeps a click from reaching a page the user cannot see, because the hit test skips
    /// hidden children.
    ///
    /// The write goes through the public visibility entry (the same one `TabWidget`
    /// uses), so the flag the painter reads and the one a host reads cannot disagree.
    fn sync_pages(&self) {
        for index in 0..self.layout.item_count() {
            let Some(widget_id) = self.layout.item_at(index) else {
                continue;
            };
            let shown = index == self.layout.current_index() || self.previous_page == Some(index);
            crate::set_widget_visible(widget_id, shown);
        }
        self.place_current_page();
    }

    /// Places the current page in the stack's own rectangle.
    ///
    /// A stacked widget has no chrome of its own — the whole control box is the page —
    /// so "placing" is moving the selected child onto the control's geometry. The
    /// rectangle (and the slide offsets, when a transition is running) comes from
    /// [`StackLayout::update_slide`], which is also the model that owns the page list, so
    /// the selection and the drawn rectangle cannot drift apart.
    fn place_current_page(&self) {
        let rect = self.geometry();
        self.layout.update_slide(
            rect,
            self.previous_page,
            self.transition,
            &mut |widget_id, child_rect| place_content_child(Some(widget_id), child_rect),
        );
    }

    /// Sets the page-slide duration. `0` (the default) snaps, as before.
    pub fn set_transition_duration(&mut self, milliseconds: u32) {
        if self.transition_ms == milliseconds {
            return;
        }
        self.transition_ms = milliseconds;
        if milliseconds == 0 {
            self.previous_page = None;
            self.transition = 1.0;
            self.sync_pages();
        }
    }

    /// The page-slide duration in milliseconds.
    pub fn transition_duration(&self) -> u32 {
        self.transition_ms
    }

    /// How far the slide has progressed: `0.0` previous, `1.0` current.
    pub fn transition_progress(&self) -> f32 {
        self.transition
    }

    /// Reports why [`Self::set_current_index`] would **not** emit `current_changed`
    /// for `index`, or `None` when the signal will fire.
    ///
    /// This exists so a host that did not see `current_changed` can distinguish "the
    /// widget is disabled" from "the index did not actually change" without reading
    /// private state. It returns a reason for exactly the two cases that suppress the
    /// signal; an index that does not change at all is not a suppressed transition.
    pub fn current_changed_suppression_reason(&self, index: usize) -> Option<&'static str> {
        if !self.base.is_enabled() {
            return Some(
                "current_changed suppressed: the widget is disabled, so the newly selected \
                 page is not reachable by the user",
            );
        }
        if index >= self.layout.item_count() {
            return Some("current_changed not emitted: the index is out of range");
        }
        None
    }

    /// Returns current widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.layout.item_at(self.layout.current_index())
    }
    /// Returns widget at index.
    pub fn widget(&self, index: usize) -> Option<ObjectId> {
        self.layout.item_at(index)
    }
    /// Returns index of widget.
    pub fn index_of(&self, widget: ObjectId) -> Option<usize> {
        (0..self.layout.item_count()).find(|&index| self.layout.item_at(index) == Some(widget))
    }
}
// Implement Widget trait
impl Widget for StackedWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    /// Advances the page slide; `true` while it is still moving.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if self.transition_ms == 0 || self.previous_page.is_none() {
            return false;
        }
        let step = delta_ms as f32 / self.transition_ms as f32;
        self.transition = (self.transition + step).min(1.0);
        if self.transition >= 1.0 {
            self.previous_page = None;
            // The slide is over, so the outgoing page must go with it. Clearing the index alone
            // left its `visible` flag (and its last slide geometry) in place, a ghost page over the
            // stack until some later page operation called `sync_pages`.
            self.sync_pages();
            self.base.request_frame_repaint();
            return false;
        }
        self.place_current_page();
        self.base.request_frame_repaint();
        true
    }

    fn is_animating(&self) -> bool {
        self.transition_ms > 0 && self.previous_page.is_some()
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        // C-019: the visible page is the content this control shows, so its hint is the stack's
        // wish. The historical 300x200 is the floor when no page is mounted.
        self.current_widget()
            .and_then(|widget| {
                crate::widget::runtime::with_widget(widget, |widget| widget.size_hint())
            })
            .unwrap_or(Size::new(300, 200))
    }

    /// Moves the stack and re-places the visible page.
    ///
    /// A layout positions the stack through this entry point, so this is where a window
    /// or panel resize reaches the visible page.
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_current_page();
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
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    fn save_state_dyn(&self) -> Option<&dyn SaveState> {
        Some(self)
    }

    fn save_state_dyn_mut(&mut self) -> Option<&mut dyn SaveState> {
        Some(self)
    }
}

/// Version tag of the blob produced by [`StackedWidget::save_state`].
const STACKED_WIDGET_STATE_VERSION: u8 = 1;

/// RW-64: the selected page is the stacked widget's user-visible state.
impl SaveState for StackedWidget {
    fn state_version(&self) -> u16 {
        u16::from(STACKED_WIDGET_STATE_VERSION)
    }

    fn save_state(&self) -> Vec<u8> {
        StackedWidget::save_state(self)
    }

    fn restore_state(&mut self, state: &[u8]) -> bool {
        StackedWidget::restore_state(self, state)
    }
}

/// `StackedWidget`'s property contract.
impl WidgetProperties for StackedWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "widget_count" => Ok(CapabilityValue::UInt(self.widget_count() as u64)),
            "current_index" => Ok(CapabilityValue::UInt(self.selected_index().unwrap_or(0) as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            // Derived from the child list.
            "widget_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `STACKED_WIDGET_PROPERTIES`.
        property_names_of!["widget_count", "current_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `stacked_widget` publishes.
    ///
    /// `add_widget` / `remove_widget` address a child by `ObjectId` and are not published (no
    /// property can carry a widget reference), so they answer `UnknownCommand` rather than
    /// pretending a payload would complete them. `set_current_index` is published, and the shared
    /// default answers it with "needs a payload" because the value travels through the property
    /// route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        self.default_command(name)
    }
}

impl EventHandler for StackedWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}
impl Draw for StackedWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        // The page surface resolves explicit style first, then the theme's resolved style
        // for this control, and only then a derived tint. The theme step is what makes an
        // appearance switch visible; the fill used to be a hardcoded white, so light and
        // dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no
        // guard is held across the draw (the mutex is not re-entrant).
        //
        // The resolved style and the window fill used to be two independent manager
        // visits; `ThemeSnapshot` folds them into one read for the frame (C-P01).
        let theme = ThemeSnapshot::capture("stacked_widget");
        let style = self.base.style();
        // The stack's interior is a container surface, and neither step above can supply
        // one: `stacked_widget` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — **the window's
        // own fill**. Painting that leaves the stack byte-identical to the frame behind it,
        // which is the exact invisible-surface defect the census exists to catch. A resolved
        // surface equal to the window fill is therefore re-derived a visible step away from
        // it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = theme.window_fill();
        let ink = style
            .text_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let themed_surface = match style
            .background_color
            .or_else(|| theme.style.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        // Draw background
        context.face_with_gradient_corners(
            rect,
            themed_surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};

    /// Helper to create unique test ObjectIds by wrapping simple constants.
    fn widget_id_1() -> ObjectId {
        1001
    }
    fn widget_id_2() -> ObjectId {
        1002
    }

    #[test]
    fn inserting_an_existing_page_is_a_noop() {
        let mut stack = StackedWidget::new(Rect::new(0, 0, 200, 200));
        stack.add_widget(widget_id_1());
        stack.insert_widget(0, widget_id_1());
        assert_eq!(stack.count(), 1, "a duplicate page id would paint the same widget twice");
    }
    fn widget_id_3() -> ObjectId {
        1003
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────

    #[test]
    fn stacked_widget_creation_defaults() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.widget_count(), 0);
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
        assert_eq!(sw.current_widget(), None);
        assert!(sw.is_visible());
        assert!(sw.is_enabled());
        assert_eq!(sw.geometry(), Rect::new(0, 0, 300, 200));
        assert!(sw.children().is_empty());
    }

    #[test]
    fn stacked_widget_empty_current_widget_is_none() {
        let sw = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(sw.current_widget(), None);
        // Even though current_index is 0, an empty widgets vec means no current widget.
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
        assert_eq!(sw.widget(0), None);
        assert_eq!(sw.widget(1), None);
    }

    #[test]
    fn stacked_widget_empty_index_of_returns_none() {
        let sw = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(sw.index_of(widget_id_1()), None);
    }

    // ── 2. Adding pages (add_widget) ──────────────────────────────────────

    #[test]
    fn stacked_widget_add_widget_returns_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let idx = sw.add_widget(widget_id_1());
        assert_eq!(idx, 0);
        assert_eq!(sw.count(), 1);
    }

    #[test]
    fn stacked_widget_add_multiple_widgets() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        assert_eq!(sw.count(), 3);
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_2()));
        assert_eq!(sw.widget(2), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_add_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        let children = sw.children();
        assert_eq!(children.len(), 2);
        assert!(children.contains(&widget_id_1()));
        assert!(children.contains(&widget_id_2()));
    }

    #[test]
    fn stacked_widget_current_index_is_zero_after_first_add() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        // current_index stays 0 after add
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
        assert_eq!(sw.current_widget(), Some(widget_id_1()));
    }

    // ── 3. Setting current index ──────────────────────────────────────────

    #[test]
    fn stacked_widget_set_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1);
        assert_eq!(sw.selected_index().unwrap_or(0), 1);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_set_current_index_out_of_bounds_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.set_current_index(5); // out of bounds
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn stacked_widget_insert_into_empty_selects_first_widget() {
        let mut widget = StackedWidget::new(Rect::new(0, 0, 100, 100));
        widget.insert_widget(0, widget_id_1());
        assert_eq!(widget.selected_index().unwrap_or(0), 0);
        assert_eq!(widget.current_widget(), Some(widget_id_1()));
    }

    #[test]
    fn stacked_widget_set_current_index_same_value_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(0); // already 0
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn stacked_widget_set_current_index_first_tab() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(2);
        sw.set_current_index(0);
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
        assert_eq!(sw.current_widget(), Some(widget_id_1()));
    }

    #[test]
    fn stacked_widget_set_current_widget_by_id() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_widget(widget_id_3());
        assert_eq!(sw.selected_index().unwrap_or(0), 2);
        assert_eq!(sw.current_widget(), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_set_current_widget_unknown_id_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.set_current_widget(9999); // unknown id
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
    }

    // ── 4. Page count ─────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_count_after_add_and_remove() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.widget_count(), 0);
        sw.add_widget(widget_id_1());
        assert_eq!(sw.count(), 1);
        assert_eq!(sw.widget_count(), 1);
        sw.add_widget(widget_id_2());
        assert_eq!(sw.count(), 2);
        assert_eq!(sw.widget_count(), 2);
    }

    // ── 5. Removing pages ─────────────────────────────────────────────────

    #[test]
    fn stacked_widget_remove_widget() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());

        sw.remove_widget(widget_id_2());
        assert_eq!(sw.count(), 2);
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_3()));
    }

    #[test]
    fn stacked_widget_remove_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.remove_widget(widget_id_1());
        let children = sw.children();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0], widget_id_2());
    }

    #[test]
    fn stacked_widget_remove_widget_nonexistent_is_noop() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.remove_widget(9999); // not present
        assert_eq!(sw.count(), 1);
    }

    #[test]
    fn stacked_widget_remove_widget_adjusts_current_index_when_below() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(2); // last widget
                                 // Remove first widget (index 0 < current_index 2)
        sw.remove_widget(widget_id_1());
        // current_index should decrement from 2 to 1
        assert_eq!(sw.selected_index().unwrap_or(0), 1);
        assert_eq!(sw.current_widget(), Some(widget_id_3()));
    }

    /// Removing the current page shows its successor, or the new last page.
    ///
    /// This is the `StackLayout` rule the widget delegates to. It used to show the predecessor;
    /// the rule is documented and pinned here so the two layers cannot drift again.
    #[test]
    fn stacked_widget_removing_the_current_page_selects_the_successor() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(1);
        sw.remove_widget(widget_id_2());
        assert_eq!(sw.current_widget(), Some(widget_id_3()), "the successor took the slot");

        sw.set_current_index(1);
        sw.remove_widget(widget_id_3());
        assert_eq!(sw.current_widget(), Some(widget_id_1()), "the new last page is selected");
    }

    #[test]
    fn stacked_widget_remove_widget_above_current_index_does_not_change_it() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        sw.set_current_index(0);
        // Remove last widget (index 2 > current_index 0)
        sw.remove_widget(widget_id_3());
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn stacked_widget_remove_all_widgets_resets_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1);
        sw.remove_widget(widget_id_1());
        sw.remove_widget(widget_id_2());
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
        assert_eq!(sw.current_widget(), None);
    }

    // ── 6. Insert widget ──────────────────────────────────────────────────

    #[test]
    fn stacked_widget_insert_widget_at_beginning() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.insert_widget(0, widget_id_3());
        assert_eq!(sw.count(), 3);
        assert_eq!(sw.widget(0), Some(widget_id_3()));
        assert_eq!(sw.widget(1), Some(widget_id_1()));
        assert_eq!(sw.widget(2), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_insert_widget_shifts_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(1); // currently at widget_id_2
                                 // Insert at index 0, current_index (1) >= 0, so it increments to 2
        sw.insert_widget(0, widget_id_3());
        assert_eq!(sw.selected_index().unwrap_or(0), 2);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_insert_widget_after_current_index_no_change() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_current_index(0);
        // Insert at end (> current_index), current_index stays 0
        sw.insert_widget(2, widget_id_3());
        assert_eq!(sw.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn stacked_widget_insert_widget_updates_children() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.insert_widget(0, widget_id_2());
        let children = sw.children();
        // Both widget_id_1 and widget_id_2 should be children
        assert_eq!(children.len(), 2);
        assert!(children.contains(&widget_id_1()));
        assert!(children.contains(&widget_id_2()));
    }

    // ── 7. Clear (removing all widgets) ───────────────────────────────────

    #[test]
    fn stacked_widget_index_of_after_operations() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());
        assert_eq!(sw.index_of(widget_id_1()), Some(0));
        assert_eq!(sw.index_of(widget_id_2()), Some(1));
        assert_eq!(sw.index_of(widget_id_3()), Some(2));
        assert_eq!(sw.index_of(9999), None);

        // After removal, indices shift
        sw.remove_widget(widget_id_2());
        assert_eq!(sw.index_of(widget_id_3()), Some(1));
    }

    // ── 8. Getting current widget / page ──────────────────────────────────

    #[test]
    fn stacked_widget_current_widget_after_add_and_set() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        // Initially no current widget
        assert_eq!(sw.current_widget(), None);

        sw.add_widget(widget_id_1());
        assert_eq!(sw.current_widget(), Some(widget_id_1()));

        sw.add_widget(widget_id_2());
        // Still on first widget
        assert_eq!(sw.current_widget(), Some(widget_id_1()));

        sw.set_current_index(1);
        assert_eq!(sw.current_widget(), Some(widget_id_2()));
    }

    #[test]
    fn stacked_widget_widget_at_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        assert_eq!(sw.widget(0), Some(widget_id_1()));
        assert_eq!(sw.widget(1), Some(widget_id_2()));
        assert_eq!(sw.widget(2), None);
    }

    // ── 9. Geometry delegation ────────────────────────────────────────────

    #[test]
    fn stacked_widget_geometry_delegation() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.geometry(), Rect::new(0, 0, 300, 200));

        sw.set_geometry(Rect::new(10, 20, 400, 300));
        assert_eq!(sw.geometry(), Rect::new(10, 20, 400, 300));
    }

    #[test]
    fn stacked_widget_geometry_position_and_size() {
        let mut sw = StackedWidget::new(Rect::new(5, 10, 200, 150));
        assert_eq!(sw.position(), Point::new(5, 10));
        assert_eq!(sw.size(), Size::new(200, 150));

        sw.set_position(Point::new(20, 30));
        assert_eq!(sw.geometry(), Rect::new(20, 30, 200, 150));

        sw.set_size(Size::new(300, 200));
        assert_eq!(sw.geometry(), Rect::new(20, 30, 300, 200));
    }

    /// The page rectangle is produced by `StackLayout::update_slide`, so the mounted child's
    /// runtime geometry equals the stack's own box and follows a resize. This is the behaviour
    /// the layout authority replaced the hand-written offsets with.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_page_is_placed_at_the_stack_box() {
        use crate::widget::base_widgets::label::Label;

        let id = crate::widget::runtime::register(Box::new(Label::new(
            "page".to_string(),
            Rect::new(0, 0, 10, 10),
        )))
        .expect("the test thread has a widget runtime");
        let mut sw = StackedWidget::new(Rect::new(30, 40, 200, 120));
        sw.add_widget(id);
        assert_eq!(crate::widget_geometry(id), Some((30, 40, 200, 120)));

        sw.set_geometry(Rect::new(10, 20, 320, 240));
        assert_eq!(crate::widget_geometry(id), Some((10, 20, 320, 240)));

        crate::widget::runtime::unregister(id);
    }

    // ── 10. Visibility ────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_visibility() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert!(sw.is_visible());

        sw.hide();
        assert!(!sw.is_visible());

        sw.show();
        assert!(sw.is_visible());

        sw.set_visible(false);
        assert!(!sw.is_visible());

        sw.set_visible(true);
        assert!(sw.is_visible());
    }

    #[test]
    fn stacked_widget_enabled_state() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert!(sw.is_enabled());

        sw.set_enabled(false);
        assert!(!sw.is_enabled());

        sw.set_enabled(true);
        assert!(sw.is_enabled());
    }

    // ── 11. ID / Kind ─────────────────────────────────────────────────────

    #[test]
    fn stacked_widget_kind() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.kind(), WidgetKind::StackedWidget);
    }

    #[test]
    fn stacked_widget_id_is_unique() {
        let sw1 = StackedWidget::new(Rect::new(0, 0, 100, 100));
        let sw2 = StackedWidget::new(Rect::new(0, 0, 100, 100));
        assert_ne!(sw1.id(), sw2.id(), "Each StackedWidget must have a unique id");
    }

    #[test]
    fn stacked_widget_class_name() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(sw.accessible_name(), "StackedWidget");
    }

    // ── 12. SVG draw output ───────────────────────────────────────────────

    #[test]
    fn stacked_widget_draw_produces_svg_output() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"200\""));
        assert!(svg.contains("fill=")); // white background fill
        assert!(svg.len() > 50);
    }

    // ── 13. Signal accessors ─────────────────────────────────────────────

    #[test]
    fn stacked_widget_current_changed_signal_emits_on_set() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.add_widget(widget_id_3());

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        sw.current_changed.connect(move |idx| {
            e.store(*idx, Ordering::SeqCst);
        });

        sw.set_current_index(1);
        assert_eq!(emitted.load(Ordering::SeqCst), 1);

        sw.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);

        // Setting to same value does NOT emit
        sw.set_current_index(2);
        assert_eq!(emitted.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn stacked_widget_current_changed_not_emitted_for_out_of_bounds() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());

        let emitted = Arc::new(AtomicUsize::new(0));
        let e = emitted.clone();
        sw.current_changed.connect(move |idx| {
            e.store(*idx, Ordering::SeqCst);
        });

        // Out of bounds - should not emit
        sw.set_current_index(10);
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn stacked_widget_base_signals_accessible() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        // These are all delegated through BaseWidget
        let _clicked = sw.clicked_signal();
        let _changed = sw.changed_signal();
        let _hover = sw.hover_signal();
        let _mouse_down = sw.mouse_down_signal();
        let _mouse_up = sw.mouse_up_signal();
        let _key_down = sw.key_down_signal();
        let _key_up = sw.key_up_signal();
        let _focus_gained = sw.focus_gained_signal();
        let _focus_lost = sw.focus_lost_signal();
        let _redraw = sw.redraw_requested_signal();
        let _layout = sw.layout_requested_signal();
        // Smoke test signals are properly connected
        _redraw.emit();
        _layout.emit();
        _changed.emit();
        _clicked.emit();
    }

    // ── 14. Event and draw robustness ────────────────────────────────────

    #[test]
    fn stacked_widget_event_is_noop_for_a_timer_tick() {
        use crate::event::Event;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        // Should not panic
        sw.handle_event(&Event::timer(0));
    }

    #[test]
    fn stacked_widget_draw_with_a_page_produces_svg() {
        use crate::render::PaintBackend;
        use crate::render::RenderContext;
        use crate::render::SvgPaintBackend;

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());

        // draw with a page and no mounted surface should not panic
        let size = Size::new(300, 200);
        let mut backend = SvgPaintBackend::new(size);
        backend.begin_frame(crate::core::Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sw.draw(&mut ctx);
        }
        backend.end_frame();
        let svg = backend.finish();
        assert!(svg.starts_with("<svg"));
    }

    // ── Enabled contract (BLUE19 T-5 follow-up) ───────────────────────────
    //
    // `StackedWidget` is a page container: it has no input of its own, but it does own
    // three programmatic mutators (`set_current_index`, `add_widget`, `insert_widget`)
    // that `current_changed` subscribers act on. A disabled control must not keep
    // announcing those transitions -- that is exactly the "reported success for
    // something that did not happen" failure the `enabled` contract exists to prevent
    // (`set_enabled(false)` hides the page from the user, so an index change they
    // cannot see must not reach a handler that reacts to it).
    //
    // The guard is deliberately placed in the *emitters* rather than in `handle_event`: a
    // container with no interaction of its own cannot be made correct by an event-handler
    // gate, which is why this file was allowlisted as non-interactive before. The
    // programmatic hole it left is what these tests pin down.

    #[test]
    fn stacked_widget_disabled_does_not_emit_current_changed_on_set_current_index() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());

        sw.set_enabled(false);
        sw.set_current_index(1);

        assert_eq!(sw.selected_index().unwrap_or(0), 1, "the visible page still moves");
    }

    #[test]
    fn stacked_widget_disabled_set_current_index_reports_why() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_enabled(false);

        let reason = sw.current_changed_suppression_reason(1);
        assert!(
            reason.is_some(),
            "a disabled widget must be able to report why it did not announce the change"
        );
        assert!(reason.unwrap().contains("disabled"));

        sw.set_enabled(true);
        assert!(sw.current_changed_suppression_reason(1).is_none());
    }

    #[test]
    fn stacked_widget_enabled_set_current_index_announces() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());

        sw.set_current_index(1);

        // The enabled path is what `current_changed` is for; the disabled path is the
        // one that must stay silent.
        assert_eq!(sw.selected_index().unwrap_or(0), 1);
        assert!(sw.current_changed_suppression_reason(1).is_none());
    }

    /// The guard above is only worth having if the signal really is wired up: a test
    /// that observes nothing fails identically whether the emit is suppressed or the
    /// subscriber was never connected. This pins the positive case to a real
    /// subscription, so the negative case above is meaningful.
    #[test]
    fn stacked_widget_current_changed_reaches_a_subscriber_only_when_enabled() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let seen = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let total = Arc::new(AtomicUsize::new(0));

        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        {
            let seen = seen.clone();
            let total = total.clone();
            sw.current_changed.connect(move |index| {
                total.fetch_add(1, Ordering::SeqCst);
                seen.lock().unwrap().push(*index);
            });
        }

        sw.set_current_index(1);
        assert_eq!(*seen.lock().unwrap(), vec![1], "the enabled path must reach the subscriber");

        sw.set_enabled(false);
        sw.set_current_index(0);
        assert_eq!(sw.selected_index().unwrap_or(0), 0, "the page still moves");
        assert_eq!(
            total.load(Ordering::SeqCst),
            1,
            "a disabled container must not announce a page the user cannot reach"
        );
        assert_eq!(*seen.lock().unwrap(), vec![1]);
    }

    #[test]
    fn stacked_widget_disabled_set_current_widget_is_also_gated() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_enabled(false);

        // `set_current_widget` routes through `set_current_index`, so the guard must hold
        // for the by-id entry point too, not just the by-index one.
        sw.set_current_widget(widget_id_2());
        assert_eq!(sw.selected_index().unwrap_or(0), 1);
        assert!(
            sw.current_changed_suppression_reason(1).is_some(),
            "the suppression must remain queryable so the host is not left guessing"
        );
    }

    #[test]
    fn stacked_widget_suppression_reason_reports_out_of_range() {
        let sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        // No widgets at all: index 0 cannot be selected, and the reason says which of the
        // two suppression cases applies.
        assert_eq!(
            sw.current_changed_suppression_reason(0),
            Some("current_changed not emitted: the index is out of range")
        );
    }

    /// An index past the end appends instead of panicking.
    ///
    /// `insert_widget` passed the caller's index straight to `Vec::insert`, which panics
    /// when it exceeds the length. The rest of the container family (`tab_bar`,
    /// `pie_menu`, `kanban_board`) clamps, so a caller that asked to insert past the end
    /// crashed here and appended everywhere else.
    #[test]
    fn stacked_widget_insert_past_the_end_appends() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 300, 200));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.insert_widget(9, widget_id_3());
        assert_eq!(sw.count(), 3);
        sw.set_current_widget(widget_id_3());
        assert_eq!(sw.selected_index().unwrap_or(0), 2, "past the end lands last");
    }

    /// A transition keeps the previous page visible and slides both pages over frames.
    #[test]
    fn a_page_transition_slides_instead_of_snapping() {
        use crate::event::EventHandler;
        let mut sw = StackedWidget::new(Rect::new(0, 0, 200, 100));
        sw.add_widget(widget_id_1());
        sw.add_widget(widget_id_2());
        sw.set_transition_duration(100);
        sw.set_current_index(1);
        assert_eq!(sw.transition_progress(), 0.0);
        assert!(sw.is_animating());
        assert!(sw.tick(50));
        assert!(sw.transition_progress() > 0.0);
        assert!(!sw.tick(100));
        assert_eq!(sw.transition_progress(), 1.0);
        assert!(!sw.is_animating());
        // A disabled stack still moves; the signal gate is covered elsewhere.
        sw.set_current_index(0);
        let _ = sw.selected_index();
        sw.handle_event(&crate::event::Event::MouseMove { pos: crate::core::Point::new(1, 1) });
    }

    /// C-23: removal by index behaves exactly like removal by id.
    #[test]
    fn remove_widget_at_matches_remove_widget() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 200, 120));
        sw.add_widget(1001);
        sw.add_widget(1002);
        sw.add_widget(1003);
        sw.set_current_index(2);
        assert!(!sw.remove_widget_at(9), "an out-of-range index is refused");
        assert!(sw.remove_widget_at(1));
        assert_eq!(sw.count(), 2);
        assert_eq!(sw.index_of(1002), None);
        assert_eq!(sw.selected_index(), Some(1), "the selection follows the removal");
    }

    /// C-019: the hint follows the visible page rather than a constant.
    #[test]
    fn the_size_hint_follows_the_current_page() {
        use crate::widget::base_widgets::label::Label;

        let label = Label::new("page".to_string(), Rect::new(0, 0, 160, 70));
        let expected = label.size_hint();
        let id =
            crate::widget::runtime::register(Box::new(label)).expect("the test control mounts");
        let mut stack = StackedWidget::new(Rect::new(0, 0, 300, 200));
        stack.add_widget(id);
        assert_eq!(stack.size_hint(), expected);
        let _ = crate::widget::runtime::unregister(id);
    }

    /// RW-64: the selection round-trips through the save-state bytes.
    #[test]
    fn the_stacked_selection_round_trips() {
        use crate::widget::base_widgets::label::Label;

        let pages: Vec<_> = (0..3)
            .map(|index| {
                crate::widget::runtime::register(Box::new(Label::new(
                    std::format!("page {index}"),
                    Rect::new(0, 0, 100, 40),
                )))
                .expect("the page mounts")
            })
            .collect();
        let mut stack = StackedWidget::new(Rect::new(0, 0, 300, 200));
        for page in &pages {
            stack.add_widget(*page);
        }
        stack.set_current_index(2);
        let saved = SaveState::save_state(&stack);
        assert_eq!(SaveState::state_version(&stack), 1);
        stack.set_current_index(0);
        assert!(SaveState::restore_state(&mut stack, &saved));
        assert_eq!(stack.selected_index(), Some(2));
        assert!(!SaveState::restore_state(&mut stack, &[1, 0]));
        assert_eq!(stack.selected_index(), Some(2), "a refused blob changes nothing");
        for page in pages {
            let _ = crate::widget::runtime::unregister(page);
        }
    }

    /// The completing tick must hide the outgoing page, not just forget its index.
    ///
    /// `tick` cleared `previous_page` but left the page's `visible` flag set, so the
    /// outgoing page kept painting (at its last slide geometry) over the stack until some
    /// later page operation happened to call `sync_pages`.
    #[test]
    fn a_completed_transition_hides_the_outgoing_page() {
        use crate::widget::base_widgets::label::Label;
        use crate::widget::runtime;

        let outgoing = runtime::register(Box::new(Label::new(
            "outgoing".to_string(),
            Rect::new(0, 0, 100, 40),
        )))
        .expect("the outgoing page mounts");
        let incoming = runtime::register(Box::new(Label::new(
            "incoming".to_string(),
            Rect::new(0, 0, 100, 40),
        )))
        .expect("the incoming page mounts");

        let mut sw = StackedWidget::new(Rect::new(0, 0, 200, 100));
        sw.add_widget(outgoing);
        sw.add_widget(incoming);
        sw.set_transition_duration(100);
        sw.set_current_index(1);

        assert!(sw.is_animating(), "the page change starts a slide");
        assert!(crate::is_widget_visible(outgoing), "both pages paint during the slide");
        assert!(crate::is_widget_visible(incoming));

        assert!(!sw.tick(100), "the completing tick ends the animation");
        assert!(!sw.is_animating());
        assert!(
            !crate::is_widget_visible(outgoing),
            "the outgoing page must not stay painted after the slide completes"
        );
        assert!(crate::is_widget_visible(incoming), "the current page stays visible");

        let _ = runtime::unregister(outgoing);
        let _ = runtime::unregister(incoming);
    }

    /// A page inserted during a slide must carry the outgoing index with it.
    ///
    /// The slide names pages by index, exactly like the selection the layout repairs. An
    /// insert at or before the outgoing page shifts it one slot down; a removal of that page
    /// drops the slide instead of leaving a stale index naming a page that no longer exists.
    #[test]
    fn an_insert_during_a_transition_does_not_leave_a_stale_previous_page() {
        let mut sw = StackedWidget::new(Rect::new(0, 0, 200, 100));
        sw.add_widget(2001);
        sw.add_widget(2002);
        sw.add_widget(2003);
        sw.set_transition_duration(100);
        sw.set_current_index(1); // slide 2001 (0) -> 2002 (1)
        assert_eq!(sw.previous_page, Some(0));
        assert!(sw.is_animating());

        // Inserting before the outgoing page shifts both the selection and the slide.
        sw.insert_widget(0, 2004);
        assert_eq!(sw.widget(1), Some(2001), "the outgoing page moved one slot down");
        assert_eq!(
            sw.previous_page,
            Some(1),
            "the slide follows the outgoing page, not the old slot"
        );
        assert_eq!(sw.selected_index(), Some(2), "2002 is still the selected page");

        // Removing the outgoing page drops the slide rather than naming a page that is gone.
        assert!(sw.remove_widget_at(1), "the outgoing page is the one that left");
        assert_eq!(sw.previous_page, None, "there is no outgoing page left to slide");
        assert!(!sw.is_animating(), "dropping the outgoing page ends the transition");
        assert_eq!(sw.transition_progress(), 1.0);
    }
}
