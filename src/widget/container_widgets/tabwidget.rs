// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tab widget.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect};
use crate::event::{DragPayload, DragSession, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;

use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(feature = "image")]
use crate::widget::Image;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
/// Tab widget.
pub struct TabWidget {
    base: BaseWidget,
    tabs: Vec<Tab>,
    current_index: usize,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    /// Index of the tab a move gesture started on, and the drag state machine that decides
    /// whether the gesture has travelled far enough to count as a drag.
    ///
    /// Two pieces because they answer two different questions: the session says *whether*
    /// the pointer has moved past the click threshold, and the index says *which tab* is
    /// being carried. Tracking only the index (as `TabBar` did until it gained a session)
    /// makes "click" and "drag" the same gesture, so a stray pixel of movement while
    /// selecting a tab would silently reorder it.
    drag_session: Option<DragSession>,
    dragging_from: Option<usize>,
    /// Emitted with the new index when the selected tab changes; not emitted
    /// when the same index is re-applied.
    pub current_changed: Signal1<usize>,
    /// Emitted when the user requests that the tab at this absolute index be
    /// closed. The `closable` flag only gates hit-testing of the close button;
    /// this widget does not remove the tab itself — the host must handle the
    /// request and call `remove_tab`, so the index is still valid when emitted.
    pub tab_close_requested: Signal1<usize>,
    /// Emitted when a movable tab is dragged to a new index; the payload is
    /// `(old_index, new_index)`.
    ///
    /// `movable` was declared but read by nothing, and this signal did not exist, so a host
    /// could neither turn reordering on nor learn that it had happened. The drag gesture in
    /// [`TabWidget::handle_event`] is what emits it.
    pub tab_moved: Signal1<(usize, usize)>,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
/// Tab information.
pub struct Tab {
    title: String,
    #[cfg(feature = "image")]
    icon: Option<Image>,
    tooltip: String,
    enabled: bool,
    widget: Option<ObjectId>,
}
/// Tab position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabPosition {
    /// Tabs at the top
    #[default]
    North,
    /// Tabs at the bottom
    South,
    /// Tabs at the left
    West,
    /// Tabs at the right
    East,
}

impl TabPosition {
    /// Parses a property token, accepting exactly the spellings
    /// [`tab_position_token`] publishes.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "north" => Some(TabPosition::North),
            "south" => Some(TabPosition::South),
            "west" => Some(TabPosition::West),
            "east" => Some(TabPosition::East),
            _ => None,
        }
    }
}

/// The token `tab_position` is carried as, matching the schema row's accepted spellings.
fn tab_position_token(position: TabPosition) -> &'static str {
    match position {
        TabPosition::North => "north",
        TabPosition::South => "south",
        TabPosition::West => "west",
        TabPosition::East => "east",
    }
}
/// Tab shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabShape {
    /// Rounded tabs
    #[default]
    Rounded,
    /// Triangular tabs
    Triangular,
    /// Rectangular tabs
    Rectangular,
}
impl Tab {
    /// Creates a new tab.
    pub fn new(title: String) -> Self {
        Self {
            title,
            #[cfg(feature = "image")]
            icon: None,
            tooltip: String::new(),
            enabled: true,
            widget: None,
        }
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }
    #[cfg(feature = "image")]
    /// Returns icon.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }
    #[cfg(feature = "image")]
    /// Sets icon.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon;
    }
    /// Returns tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }
    /// Sets tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }
    /// Returns whether tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    /// Sets enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
    }
}
impl TabWidget {
    /// Creates a tab widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabWidget, geometry, "TabWidget"),
            tabs: Vec::new(),
            current_index: 0,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            drag_session: None,
            dragging_from: None,
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            tab_moved: Signal1::new(),
            registry: None,
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
        self.base.request_redraw();
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Adds a tab.
    pub fn add_tab(&mut self, title: String, widget: Option<ObjectId>) -> usize {
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.tabs.push(tab);
        self.tabs.len().saturating_sub(1)
    }
    /// Inserts a tab at position.
    pub fn insert_tab(&mut self, index: usize, title: String, widget: Option<ObjectId>) {
        let was_empty = self.tabs.is_empty();
        let mut tab = Tab::new(title);
        tab.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
        self.tabs.insert(index, tab);
        if !was_empty && self.current_index >= index {
            self.current_index += 1;
        }
    }
    /// Removes a tab.
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            if let Some(widget_id) = self.tabs[index].widget {
                self.base.remove_child(widget_id);
            }
            self.tabs.remove(index);
            if self.current_index >= index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.tabs.is_empty() {
                self.current_index = 0;
            }
        }
    }
    /// Returns number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
    /// Returns current tab index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }
    /// Sets current tab index.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.tabs.len() && self.current_index != index {
            self.current_index = index;
            self.current_changed.emit(index);
            self.base.request_redraw();
        }
    }
    /// Returns current tab widget.
    pub fn current_widget(&self) -> Option<ObjectId> {
        self.tabs.get(self.current_index).and_then(|tab| tab.widget)
    }
    /// Returns tab at index.
    pub fn tab(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }
    /// Returns mutable tab at index.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut Tab> {
        self.tabs.get_mut(index)
    }
    /// Returns the text of the tab at the given index.
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.title.as_str())
    }
    /// Sets the text of the tab at the given index.
    pub fn set_tab_text(&mut self, index: usize, text: String) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.title = text;
        }
        self.base.request_redraw();
    }
    /// Returns tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }
    /// Sets tab position.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        self.tab_position = position;
        self.base.request_redraw();
    }
    /// Returns tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }
    /// Sets tab shape.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        self.tab_shape = shape;
        self.base.request_redraw();
    }
    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }
    /// Sets closable state.
    pub fn set_closable(&mut self, closable: bool) {
        self.closable = closable;
        self.base.request_redraw();
    }
    /// Returns whether tabs are movable.
    ///
    /// When true, pressing a tab and dragging the pointer past a neighbour's midpoint
    /// reorders the tabs and emits [`Self::tab_moved`]. When false the same gesture only
    /// selects, so the flag decides whether a drag has any effect at all.
    pub fn movable(&self) -> bool {
        self.movable
    }
    /// Sets movable state.
    ///
    /// A gesture already in progress is abandoned: turning reordering off mid-drag must
    /// not let the release that follows still move a tab, which is what dropping the live
    /// drag state here prevents.
    pub fn set_movable(&mut self, movable: bool) {
        self.movable = movable;
        if !movable {
            self.cancel_tab_drag();
        }
        self.base.request_redraw();
    }
    /// Moves the tab at `from` to `to` and emits [`Self::tab_moved`] with `(from, to)`.
    ///
    /// # Why `to` is clamped rather than rejected
    ///
    /// A drag reports positions from pointer coordinates, and a pointer past the last tab
    /// is "dropped at the end" — the intent — not a caller bug. An out-of-range `from`,
    /// however, can only be a caller error, so it is rejected without emitting.
    ///
    /// # Why the selection travels with the tab
    ///
    /// `current_index` is a slot, but the user's mental model is that the *page* moved.
    /// Leaving the index alone would silently switch to whichever tab took the slot the
    /// dragged one vacated.
    ///
    /// Returns `true` when a move happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() {
            return false;
        }
        let to = to.min(self.tabs.len() - 1);
        if to == from {
            return false;
        }

        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        self.current_index = match self.current_index {
            c if c == from => to,
            c if from < c && c <= to => c - 1,
            c if to <= c && c < from => c + 1,
            c => c,
        };

        self.tab_moved.emit((from, to));
        self.base.request_redraw();
        true
    }
    /// Starts a move gesture on the tab under `pos`.
    ///
    /// Only a movable, enabled tab arms a drag, so a click that happens to jitter still
    /// selects and nothing else. The session is opened with the tab's index as its payload
    /// so a future drop target can tell which tab is being carried.
    fn begin_tab_drag(&mut self, pos: Point) {
        if !self.movable {
            return;
        }
        let Some(index) = self.tab_at_position(pos) else {
            return;
        };
        if !self.tabs[index].enabled {
            return;
        }
        let payload = DragPayload::new(TAB_DRAG_TYPE, index.to_string()).with_origin(pos);
        self.drag_session = Some(DragSession::begin(payload, pos));
        self.dragging_from = Some(index);
    }
    /// Reorders the tabs as the pointer moves past a neighbour's midpoint.
    ///
    /// # Why the move is applied live rather than on release
    ///
    /// Qt's `QTabBar` swaps the two tabs the moment the pointer crosses the midpoint, so
    /// the strip the user sees while dragging is the arrangement they will get. Deferring
    /// it to the release makes the gesture feel unresponsive and gives no feedback about
    /// where the tab will land.
    fn update_tab_drag(&mut self, pos: Point) {
        let (active, from) = {
            let Some(session) = self.drag_session.as_mut() else {
                return;
            };
            session.update(pos, TAB_DRAG_THRESHOLD);
            (session.is_active(), self.dragging_from)
        };
        if !active {
            return;
        }
        let Some(from) = from else {
            return;
        };
        // The neighbour the pointer has passed: a move to the right lands on the next tab
        // once the pointer is past *that* tab's midpoint, and symmetrically to the left.
        // Deriving the target from geometry rather than from the dragged tab's own rect is
        // what lets a fast drag cross several neighbours without stalling on each one.
        let Some(target) = self.tab_at_position(pos) else {
            return;
        };
        if target == from {
            return;
        }
        // Move one step toward the target rather than jumping the whole distance: the tabs
        // between the two indices have to shift by one, which a single `move_tab(from,
        // target)` accomplishes, and the live `move_tab` re-emits so the host sees each
        // swap the user saw.
        self.move_tab(from, target);
        self.dragging_from = Some(target);
    }
    /// Ends a move gesture, keeping the reordering already applied.
    ///
    /// The session is closed without a further move: `update_tab_drag` already placed the
    /// tab under the pointer, so acting on the release position again would double-apply
    /// the last step.
    fn end_tab_drag(&mut self) {
        self.cancel_tab_drag();
    }
    /// Drops any in-progress move gesture without reordering further.
    fn cancel_tab_drag(&mut self) {
        self.drag_session = None;
        self.dragging_from = None;
    }
    /// Returns tab rectangle at index.
    ///
    /// Tab width is **measured**, not a constant. The literal `100` meant a title longer than
    /// ~13 characters was clipped at a fixed point and a two-character title reserved the same
    /// 100 px as a nine-character one, so the strip's appearance had nothing to do with its
    /// contents. Measuring the titles and clamping the result (the same `[40, 200]` window
    /// `tab_bar` uses) is what makes the band a function of what the tabs say.
    ///
    /// The computed width is also what the **overflow** rule divides up: when the tabs no
    /// longer fit the strip, every tab gets `width / count` so they all stay visible, rather
    /// than the later ones being drawn past the control's own right edge (the SVG backend emits
    /// absolute coordinates, so those tabs simply left the picture).
    fn tab_widths(&self) -> crate::compat::Vec<i32> {
        let rect = self.geometry();
        let count = self.tabs.len();
        if count == 0 {
            return crate::compat::Vec::new();
        }
        let measured: crate::compat::Vec<i32> = self
            .tabs
            .iter()
            .map(|tab| {
                (tab.title.chars().count() as i32 * TAB_CHAR_WIDTH + TAB_TEXT_PADDING)
                    .clamp(MIN_TAB_WIDTH, MAX_TAB_WIDTH)
            })
            .collect();
        let total: i32 = measured.iter().sum::<i32>() + TAB_SPACING * (count as i32 - 1);
        let available = match self.tab_position {
            TabPosition::North | TabPosition::South => rect.width as i32,
            TabPosition::West | TabPosition::East => rect.height as i32,
        };
        if total <= available {
            return measured;
        }
        // Overflow: share the strip equally so every tab remains inside it.
        let share =
            ((available - TAB_SPACING * (count as i32 - 1)) / count as i32).max(MIN_TAB_WIDTH / 2);
        crate::compat::vec![share; count]
    }

    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.geometry();
        let widths = self.tab_widths();
        // Offset by the widths of the tabs before this one, so a measured strip is laid out in
        // sequence rather than on a fixed step.
        let offset: i32 = widths.iter().take(index).sum::<i32>() + TAB_SPACING * index as i32;
        let tab_width = *widths.get(index)?;
        let tab_height = TAB_HEIGHT;
        match self.tab_position {
            TabPosition::North => {
                Some(Rect::new(rect.x + offset, rect.y, tab_width as u32, tab_height as u32))
            }
            TabPosition::South => Some(Rect::new(
                rect.x + offset,
                rect.y + rect.height as i32 - tab_height,
                tab_width as u32,
                tab_height as u32,
            )),
            TabPosition::West => {
                Some(Rect::new(rect.x, rect.y + offset, tab_width as u32, tab_height as u32))
            }
            TabPosition::East => Some(Rect::new(
                rect.x + rect.width as i32 - tab_width,
                rect.y + offset,
                tab_width as u32,
                tab_height as u32,
            )),
        }
    }
    /// Returns content rectangle.
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let tab_height = TAB_HEIGHT;
        match self.tab_position {
            TabPosition::North => Rect::new(
                rect.x,
                rect.y + tab_height,
                rect.width,
                rect.height.saturating_sub(tab_height as u32),
            ),
            TabPosition::South => {
                Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(tab_height as u32))
            }
            TabPosition::West => Rect::new(
                rect.x + tab_height,
                rect.y,
                rect.width.saturating_sub(tab_height as u32),
                rect.height,
            ),
            TabPosition::East => {
                Rect::new(rect.x, rect.y, rect.width.saturating_sub(tab_height as u32), rect.height)
            }
        }
    }
    /// Returns index of tab at position.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                if tab_rect.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }
}
/// Height of the tab strip, in logical pixels.
const TAB_HEIGHT: i32 = 24;

/// Horizontal gap between adjacent tabs.
const TAB_SPACING: i32 = 2;

/// Widest a measured tab may become, and narrowest it may stay.
///
/// The same window `tab_bar` clamps to, so the two tab controls agree about what a tab looks
/// like even though they lay their bands out differently.
const MIN_TAB_WIDTH: i32 = 40;
const MAX_TAB_WIDTH: i32 = 200;

/// Padding added to a measured title before it is clamped.
const TAB_TEXT_PADDING: i32 = 24;

/// Width charged per character when measuring a tab title.
///
/// The renderer's advance model, not a `len()` estimate: this crate's shaper gives one
/// cluster per `char` at 0.6 em for Latin text, and a tab title is a label the user reads.
/// `len()` on a UTF-8 `String` counts bytes, so a CJK title measured by it would reserve
/// four times the width it draws.
const TAB_CHAR_WIDTH: i32 = 8;

/// Side of a tab's close button, in logical pixels.
const CLOSE_SIZE: i32 = 12;

/// The drag payload type a tab move carries.
///
/// Named rather than an empty string so a drop target (or a future cross-control reorder)
/// can recognise that this gesture is a tab move and not, say, a card drag. The payload's
/// item id is the dragged tab's index, spelled as the same decimal the property layer uses.
const TAB_DRAG_TYPE: &str = "tab_widget_tab";

/// How far the pointer must travel before a press counts as a drag.
///
/// Without a threshold every click that jitters by one pixel would reorder a tab, because
/// the press and the move are the same gesture to the OS.
const TAB_DRAG_THRESHOLD: i32 = 4;

// Implement Widget trait
impl Widget for TabWidget {
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

/// `TabWidget`'s property contract.
///
/// The doc comment here used to record that `closable`, `movable` and `tab_position` were
/// declared by `TAB_WIDGET_PROPERTIES` but answered by nothing, "so they stay exactly as
/// they were (not served)". That was a declaration nothing reads: the schema offered a
/// property the control refused, so a host could neither enable dragging nor learn whether
/// it was on. All three are served now, and `movable` additionally drives the drag gesture
/// in [`TabWidget::handle_event`] rather than merely being stored.
impl WidgetProperties for TabWidget {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tab_count" => Ok(CapabilityValue::UInt(self.count() as u64)),
            "current_index" => Ok(CapabilityValue::UInt(self.current_index() as u64)),
            // Symmetric with the writer below: a value that can be set can be read back.
            // Without this arm the property would be write-only, which is the one-directional
            // contract rule #97 forbids.
            "text" | "title" => Ok(CapabilityValue::String(
                self.tabs.first().map(|tab| tab.title.clone()).unwrap_or_default(),
            )),
            "closable" => Ok(CapabilityValue::Bool(self.closable())),
            "movable" => Ok(CapabilityValue::Bool(self.movable())),
            "tab_position" => {
                Ok(CapabilityValue::String(tab_position_token(self.tab_position).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_usize(value)?);
                Ok(())
            }
            // The label route reaches every other control by name, and `create_tab_widget`
            // passes the caller's text through it. Refusing `text`/`title` here meant that
            // text was silently dropped: the factory built a tab widget, the shared label
            // helper called `set("text", ...)`, and this match fell through to
            // `base_property_set` — which has no such arm, so the title never reached a tab.
            // Applying it to the first tab makes the control answer the same property name
            // its constructor's parameter describes.
            "text" | "title" => {
                let text = expect_string(value)?;
                if let Some(first) = self.tabs.first_mut() {
                    first.title = text;
                } else {
                    self.add_tab(text, None);
                }
                self.base.request_redraw();
                Ok(())
            }
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            // An unknown token is a parse failure, not a different placement: accepting
            // `"North"` and keeping `North` anyway would report success for a write that
            // changed nothing. The accepted spellings are the schema row's.
            "tab_position" => {
                let token = expect_string(value)?;
                let position =
                    TabPosition::from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_tab_position(position);
                Ok(())
            }
            // Derived from the tab list; the old writer had no arm for it either.
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tab_count",
            "current_index",
            "text",
            "title",
            "closable",
            "movable",
            "tab_position",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tab_widget` publishes.
    ///
    /// `add_tab` takes the title and the optional page widget and `remove_tab`
    /// takes the index to remove, so neither can complete without a payload: they
    /// are refused as [`CapabilityAccessError::OutOfRange`], meaning the name is
    /// valid and the argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tab" | "remove_tab" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for TabWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The move gesture runs before the content forwarding below and reads only the tab
        // strip, so a drag that started on a tab never reaches the page widget — which is
        // what makes the strip behave as one control rather than as a set of drop targets.
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        // Check if the click is on the close button area
                        if self.closable {
                            let close_size = 12;
                            if let Some(tab_rect) = self.tab_rect(index) {
                                let close_x = tab_rect.x + tab_rect.width as i32 - close_size - 5;
                                let close_y =
                                    tab_rect.y + (tab_rect.height as i32 - close_size) / 2;
                                let close_rect = Rect::new(
                                    close_x,
                                    close_y,
                                    close_size as u32,
                                    close_size as u32,
                                );
                                if close_rect.contains(*pos) {
                                    self.tab_close_requested.emit(index);
                                    return;
                                }
                            }
                        }
                        self.set_current_index(index);
                        // Selecting on press and *arming* the drag are separate: the
                        // selection has already happened, and only a pointer that travels
                        // past the threshold turns the same gesture into a reorder.
                        self.begin_tab_drag(*pos);
                    }
                }
            }
            Event::MouseMove { pos } => self.update_tab_drag(*pos),
            Event::MouseRelease { .. } => self.end_tab_drag(),
            _ => {}
        }
        let allow_child_event = match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos } => self.content_rect().contains(*pos),
            _ => true,
        };
        // Forward content events only to the current widget.
        if allow_child_event {
            if let Some(widget_id) = self.current_widget() {
                if let Some(ref reg) = self.registry {
                    reg.borrow_mut().set_widget_geometry(widget_id, self.content_rect());
                    reg.borrow_mut().forward_event(widget_id, event);
                }
            }
        }
    }
}
impl Draw for TabWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let _rect = self.geometry();
        let content_rect = self.content_rect();

        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("tab_widget");
        // `tab_widget` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The content area's fill is therefore a step toward
        // the foreground, so the page reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(255, 255, 255));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(200, 200, 200));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let content_background = resolved.blend(&text_color, 0.08);
        // Tab chrome is derived from the resolved pair: an inactive tab is pressed
        // toward the foreground, a disabled one further still, and the current tab
        // stays at the content colour so it reads as connected to its page.
        let inactive_tab = content_background.blend(&text_color, 0.06);
        let disabled_tab = content_background.blend(&text_color, 0.14);
        // A disabled tab's label is muted against the tab's own fill, which is what makes it
        // read as unavailable. This is used for the label; the fill above is the other half
        // of the same signal, so both come from the resolved pair.
        let disabled_text = text_color.blend(&disabled_tab, 0.5);
        // The close affordance is secondary chrome, not a second literal.
        let close_color = text_color.blend(&content_background, 0.4);

        // Draw content background
        context.fill_rect(content_rect, content_background);
        // Draw content border
        context.draw_rect(content_rect, border);
        // Draw tabs
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                let tab = &self.tabs[i];
                let is_current = i == self.current_index;
                let is_enabled = tab.enabled;
                // Draw tab background
                let bg_color = if !is_enabled {
                    disabled_tab
                } else if is_current {
                    content_background
                } else {
                    inactive_tab
                };
                match self.tab_shape {
                    TabShape::Rounded => {
                        let radius = 4;
                        context.fill_rounded_rect(tab_rect, radius, bg_color);
                        // A current tab shares its edge with the content area, so its
                        // outline uses the surface border; a plain tab is outlined more
                        // faintly.
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_rounded_rect_stroke(tab_rect, radius, border_color, 1);
                    }
                    TabShape::Triangular => {
                        // Real triangular tab: apex at the top-center, base along
                        // the bottom edge of the tab strip.
                        let apex_x = tab_rect.x + tab_rect.width as i32 / 2;
                        let base_y = tab_rect.y + tab_rect.height as i32;
                        let points = [
                            Point::new(apex_x, tab_rect.y),
                            Point::new(tab_rect.x, base_y),
                            Point::new(tab_rect.x + tab_rect.width as i32, base_y),
                        ];
                        context.draw_path(&points, true, bg_color, true, 0);
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_path(&points, true, border_color, false, 1);
                    }
                    _ => {
                        context.fill_rect(tab_rect, bg_color);
                        let border_color = if !is_enabled || is_current {
                            border
                        } else {
                            border.blend(&content_background, 0.5)
                        };
                        context.draw_rect(tab_rect, border_color);
                    }
                };
                // Draw tab text.
                //
                // Centred on both axes and bounded to the tab. The previous form put the glyph
                // origin at the tab's midpoint and asked for `Left`, so a title started at the
                // tab's centre and ran off its right edge, with its top edge on the tab's
                // vertical midpoint. `draw_text_fitted` with `Center` states the intent and
                // elides a title that cannot fit, instead of letting it leave the control.
                let text_color = if !is_enabled { disabled_text } else { text_color };
                let font = Font::default();
                let tab_line = context.text_line(tab_rect, &font);
                let title_band = Rect {
                    x: tab_rect.x,
                    y: tab_line.y,
                    width: tab_rect.width.saturating_sub(if self.closable {
                        (CLOSE_SIZE + 10) as u32
                    } else {
                        0
                    }),
                    height: tab_line.height,
                };
                context.draw_text_fitted(
                    title_band,
                    &tab.title,
                    &font,
                    text_color,
                    HorizontalAlignment::Center,
                );
                // Draw close button if closable
                if self.closable {
                    // Vertically centred on the title's own line box rather than on the tab's
                    // middle: the two ruled the same row and used to disagree by half a line.
                    let close_x = tab_rect.x + tab_rect.width as i32 - CLOSE_SIZE - 5;
                    let close_y = tab_line.y + (tab_line.height as i32 - CLOSE_SIZE) / 2;
                    context.draw_line(
                        Point::new(close_x, close_y),
                        Point::new(close_x + CLOSE_SIZE, close_y + CLOSE_SIZE),
                        close_color,
                    );
                    context.draw_line(
                        Point::new(close_x + CLOSE_SIZE, close_y),
                        Point::new(close_x, close_y + CLOSE_SIZE),
                        close_color,
                    );
                }
            }
        }
        // Draw current widget via registry
        if let Some(widget_id) = self.current_widget() {
            if let Some(ref reg) = self.registry {
                reg.borrow_mut().set_widget_geometry(widget_id, content_rect);
                context.push_clip(
                    content_rect.x,
                    content_rect.y,
                    content_rect.width,
                    content_rect.height,
                );
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_clip();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect, Size};
    use crate::widget::svg::{render_to_svg, render_widget_to_svg};
    use std::sync::{Arc, Mutex};

    /// Helper to create unique test ObjectIds.
    fn wid1() -> ObjectId {
        1001
    }
    fn wid2() -> ObjectId {
        1002
    }

    // ── 1. Creation defaults ──────────────────────────────────────────────────

    #[test]
    fn tabwidget_creation_defaults() {
        let tw = TabWidget::new(Rect::new(10, 20, 400, 300));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
        assert_eq!(tw.geometry(), Rect::new(10, 20, 400, 300));
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.current_index(), 0);
        assert!(tw.current_widget().is_none());
        assert_eq!(tw.tab_position(), TabPosition::North);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
        assert!(!tw.closable());
        assert!(!tw.movable());
        assert!(tw.is_visible());
        assert!(tw.is_enabled());
        assert!(tw.children().is_empty());
        assert!(tw.registry().is_none());
    }

    // ── 2. Adding tabs ────────────────────────────────────────────────────────

    #[test]
    fn tabwidget_add_tab_returns_index_and_increments_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let idx0 = tw.add_tab("Tab A".to_string(), None);
        assert_eq!(idx0, 0);
        assert_eq!(tw.count(), 1);

        let idx1 = tw.add_tab("Tab B".to_string(), Some(wid1()));
        assert_eq!(idx1, 1);
        assert_eq!(tw.count(), 2);

        let idx2 = tw.add_tab("Tab C".to_string(), Some(wid2()));
        assert_eq!(idx2, 2);
        assert_eq!(tw.count(), 3);
    }

    #[test]
    fn tabwidget_add_tab_with_widget_updates_children() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("With Widget".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    #[test]
    fn tabwidget_add_tab_without_widget_does_not_add_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("No Widget".to_string(), None);
        assert!(tw.children().is_empty());
    }

    // ── 3. Inserting tabs at specific index ───────────────────────────────────

    #[test]
    fn tabwidget_insert_tab_at_front_shifts_indices() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.insert_tab(0, "Inserted".to_string(), None);
        assert_eq!(tw.count(), 3);
        assert_eq!(tw.tab_text(0), Some("Inserted"));
        assert_eq!(tw.tab_text(1), Some("A"));
        assert_eq!(tw.tab_text(2), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_at_end_acts_like_add() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.insert_tab(1, "B".to_string(), None);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(1), Some("B"));
    }

    #[test]
    fn tabwidget_insert_tab_shifts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(1);
        tw.insert_tab(0, "X".to_string(), None);
        // current_index was 1, now should be 2 because a tab was inserted before it
        assert_eq!(tw.current_index(), 2);
    }

    #[test]
    fn tabwidget_insert_tab_with_widget_adds_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.insert_tab(0, "X".to_string(), Some(wid1()));
        let children = tw.children();
        assert_eq!(children.len(), 1);
        assert!(children.contains(&wid1()));
    }

    // ── 4. Removing tabs ─────────────────────────────────────────────────────

    #[test]
    fn tabwidget_remove_tab_reduces_count() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        assert_eq!(tw.count(), 3);

        tw.remove_tab(1);
        assert_eq!(tw.count(), 2);
        assert_eq!(tw.tab_text(0), Some("A"));
        assert_eq!(tw.tab_text(1), Some("C"));
    }

    #[test]
    fn tabwidget_remove_tab_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(5); // out of bounds
        assert_eq!(tw.count(), 1);
    }

    #[test]
    fn tabwidget_remove_last_tab_resets_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_remove_tab_adjusts_current_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        tw.remove_tab(2);
        assert_eq!(tw.current_index(), 1);
    }

    #[test]
    fn tabwidget_remove_tab_with_widget_removes_child() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        assert_eq!(tw.children().len(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.children().len(), 1);
        assert!(!tw.children().contains(&wid1()));
    }

    // ── 5. Getting / setting current index ────────────────────────────────────

    #[test]
    fn tabwidget_set_current_index_normal() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), Some(wid1()));
        tw.add_tab("B".to_string(), Some(wid2()));
        tw.set_current_index(1);
        assert_eq!(tw.current_index(), 1);
        assert_eq!(tw.current_widget(), Some(wid2()));
    }

    #[test]
    fn tabwidget_set_current_index_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.set_current_index(10);
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_set_current_index_same_value_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.set_current_index(0); // already 0
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_current_widget_on_empty_returns_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.current_widget(), None);
    }

    #[test]
    fn tabwidget_set_current_index_round_trip() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        tw.add_tab("C".to_string(), None);
        tw.set_current_index(2);
        assert_eq!(tw.current_index(), 2);
        tw.set_current_index(0);
        assert_eq!(tw.current_index(), 0);
        tw.set_current_index(1);
        assert_eq!(tw.current_index(), 1);
    }

    // ── 6. Tab count after operations ─────────────────────────────────────────

    #[test]
    fn tabwidget_count_after_mixed_operations() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.count(), 0);
        tw.add_tab("A".to_string(), None);
        assert_eq!(tw.count(), 1);
        tw.add_tab("B".to_string(), None);
        assert_eq!(tw.count(), 2);
        tw.insert_tab(1, "C".to_string(), None);
        assert_eq!(tw.count(), 3);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 2);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 1);
        tw.remove_tab(0);
        assert_eq!(tw.count(), 0);
    }

    // ── 7. Setting tab text and tooltip ───────────────────────────────────────

    #[test]
    fn tabwidget_set_tab_text_updates_and_retrieves() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Original".to_string(), None);
        assert_eq!(tw.tab_text(0), Some("Original"));
        tw.set_tab_text(0, "Updated".to_string());
        assert_eq!(tw.tab_text(0), Some("Updated"));
    }

    #[test]
    fn tabwidget_set_tab_text_out_of_bounds_is_noop() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_text(0, "Nope".to_string());
        // No panic, no change
        assert_eq!(tw.tab_text(0), None);
    }

    #[test]
    fn tabwidget_tab_text_returns_none_for_invalid_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Only".to_string(), None);
        assert_eq!(tw.tab_text(1), None);
        assert_eq!(tw.tab_text(usize::MAX), None);
    }

    #[test]
    fn tabwidget_tab_tooltip_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        assert_eq!(tab.tooltip(), "");
        tab.set_tooltip("Helpful hint".to_string());
        assert_eq!(tab.tooltip(), "Helpful hint");
        let tab_ref = tw.tab(0).unwrap();
        assert_eq!(tab_ref.tooltip(), "Helpful hint");
    }

    // ── 8. Enabling / disabling tabs ──────────────────────────────────────────

    #[test]
    fn tabwidget_tab_enabled_default() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_disable_tab_via_tab_mut() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(false);
        assert!(!tw.tab(0).unwrap().is_enabled());
        // Re-enable
        let tab = tw.tab_mut(0).unwrap();
        tab.set_enabled(true);
        assert!(tw.tab(0).unwrap().is_enabled());
    }

    #[test]
    fn tabwidget_tab_title_via_tab_api() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Hello".to_string(), None);
        assert_eq!(tw.tab(0).unwrap().title(), "Hello");
        tw.tab_mut(0).unwrap().set_title("World".to_string());
        assert_eq!(tw.tab(0).unwrap().title(), "World");
    }

    // ── 9. Tab position and shape configuration ───────────────────────────────

    #[test]
    fn tabwidget_tab_position_default_is_north() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_set_tab_position_cycle() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_position(TabPosition::South);
        assert_eq!(tw.tab_position(), TabPosition::South);
        tw.set_tab_position(TabPosition::West);
        assert_eq!(tw.tab_position(), TabPosition::West);
        tw.set_tab_position(TabPosition::East);
        assert_eq!(tw.tab_position(), TabPosition::East);
        tw.set_tab_position(TabPosition::North);
        assert_eq!(tw.tab_position(), TabPosition::North);
    }

    #[test]
    fn tabwidget_tab_shape_default_is_rounded() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    #[test]
    fn tabwidget_set_tab_shape() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_tab_shape(TabShape::Triangular);
        assert_eq!(tw.tab_shape(), TabShape::Triangular);
        tw.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tw.tab_shape(), TabShape::Rectangular);
        tw.set_tab_shape(TabShape::Rounded);
        assert_eq!(tw.tab_shape(), TabShape::Rounded);
    }

    #[test]
    fn tabwidget_closable_movable_default_false() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(!tw.closable());
        assert!(!tw.movable());
        tw.set_closable(true);
        tw.set_movable(true);
        assert!(tw.closable());
        assert!(tw.movable());
        tw.set_closable(false);
        tw.set_movable(false);
        assert!(!tw.closable());
        assert!(!tw.movable());
    }

    // ── 10. Min / max size via Widget trait ───────────────────────────────────

    #[test]
    fn tabwidget_min_size_default_none() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert_eq!(tw.min_size(), None);
        assert_eq!(tw.max_size(), None);
    }

    #[test]
    fn tabwidget_set_min_max_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.set_min_size(Some(Size::new(100, 80)));
        tw.set_max_size(Some(Size::new(800, 600)));
        assert_eq!(tw.min_size(), Some(Size::new(100, 80)));
        assert_eq!(tw.max_size(), Some(Size::new(800, 600)));
    }

    // ── 11. Signal accessors ──────────────────────────────────────────────────

    #[test]
    fn tabwidget_current_changed_signal_emits_on_set_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.current_changed.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });

        tw.set_current_index(1);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_same_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(0); // same as default
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_current_changed_not_emitted_for_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);

        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        tw.current_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<usize>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        tw.set_current_index(5); // out of bounds
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tabwidget_tab_close_requested_signal_accessible() {
        let tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        let emitted = Arc::new(std::sync::Mutex::new(None));
        tw.tab_close_requested.connect({
            let emitted = Arc::clone(&emitted);
            move |idx: Arc<usize>| {
                *emitted.lock().unwrap() = Some(*idx);
            }
        });
        // Manually emit — tabwidget doesn't auto-close, but the signal is public
        tw.tab_close_requested.emit(1usize);
        assert_eq!(*emitted.lock().unwrap(), Some(1));
    }

    // ── 12. Geometry delegation ───────────────────────────────────────────────

    #[test]
    fn tabwidget_geometry_via_widget_trait() {
        let tw = TabWidget::new(Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
        assert_eq!(tw.geometry(), Rect::new(5, 10, 200, 150));
    }

    #[test]
    fn tabwidget_set_geometry_via_widget_trait() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        tw.set_geometry(Rect::new(20, 30, 500, 400));
        assert_eq!(tw.geometry(), Rect::new(20, 30, 500, 400));
        assert_eq!(tw.position(), Point::new(20, 30));
        assert_eq!(tw.size(), Size::new(500, 400));
    }

    #[test]
    fn tabwidget_set_position_and_size() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 200, 100));
        tw.set_position(Point::new(50, 60));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(200, 100));

        tw.set_size(Size::new(300, 150));
        assert_eq!(tw.position(), Point::new(50, 60));
        assert_eq!(tw.size(), Size::new(300, 150));
    }

    // ── 13. Widget ID and kind ────────────────────────────────────────────────

    #[test]
    fn tabwidget_kind_is_tabwidget() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.kind(), WidgetKind::TabWidget);
    }

    #[test]
    fn tabwidget_id_is_unique() {
        let tw1 = TabWidget::new(Rect::new(0, 0, 100, 100));
        let tw2 = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_ne!(tw1.id(), tw2.id());
    }

    #[test]
    fn tabwidget_accessible_name_falls_back_to_kind() {
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_name(), "TabWidget");
    }

    #[test]
    fn tabwidget_accessible_role_is_kind() {
        use crate::platform::accessibility::AccessibleRole;
        let tw = TabWidget::new(Rect::new(0, 0, 100, 100));
        assert_eq!(tw.accessible_role(), AccessibleRole::TabGroup);
    }

    // ── 14. Disabled state blocks events ──────────────────────────────────────

    #[test]
    fn tabwidget_disabled_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);
        // Disable the entire widget
        tw.set_enabled(false);

        // Click on tab 1's position
        // tab_rect(0) with North position at Rect(0,0,300,200) starts at x=0, y=0
        let event = Event::mouse_press(0, 0, 1);
        tw.handle_event(&event);

        // current_index should still be 0 because the widget is disabled
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_enabled_switches_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // The second tab's position comes from the control's own layout, not from a literal:
        // tab widths are measured from their titles, so a hardcoded x would pin this test to one
        // font's metrics and break every time a title changed. Reading `tab_rect` is also the
        // stronger assertion — it checks that a click lands on the tab the control believes is
        // there, which is the property that matters.
        let second = tw.tab_rect(1).expect("two tabs were added, so index 1 has a rectangle");
        let click_x = second.x + second.width as i32 / 2;
        let event = Event::mouse_press(click_x, second.y + 2, 1);
        tw.handle_event(&event);

        assert_eq!(tw.current_index(), 1);
    }

    #[test]
    fn tabwidget_disabled_tab_does_not_switch_on_mouse_press() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);
        // Disable tab at index 1
        tw.tab_mut(1).unwrap().set_enabled(false);

        tw.set_current_index(0);

        // Click on tab 1's position
        let event = Event::mouse_press(102, 0, 1);
        tw.handle_event(&event);

        // Should NOT switch because tab 1 is disabled
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_right_click_does_not_switch() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Right-click on tab 1's position
        let event = Event::mouse_press(102, 0, 3);
        tw.handle_event(&event);

        // Should NOT switch because only button 1 (left click) triggers switch
        assert_eq!(tw.current_index(), 0);
    }

    #[test]
    fn tabwidget_click_outside_tabs_does_not_change_index() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("A".to_string(), None);
        tw.add_tab("B".to_string(), None);

        tw.set_current_index(0);

        // Click far to the right of any tab
        let event = Event::mouse_press(500, 0, 1);
        tw.handle_event(&event);

        assert_eq!(tw.current_index(), 0);
    }

    // ── 15. SVG output verification ───────────────────────────────────────────

    #[test]
    fn tabwidget_svg_output_via_render_to_svg() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Home".to_string(), None);
        tw.add_tab("Settings".to_string(), None);

        let svg = render_to_svg(&mut tw);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
        assert!(svg.contains("width=\"300\""), "SVG should contain width=\"300\"");
        assert!(svg.contains("height=\"200\""), "SVG should contain height=\"200\"");
        // Should contain tab text
        assert!(svg.contains("Home"));
        assert!(svg.contains("Settings"));
        // Should contain fill and stroke attributes from the rendering
        assert!(svg.contains("fill=") || svg.contains("stroke="));
    }

    #[test]
    fn tabwidget_svg_output_with_explicit_geometry() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 120, 80));
        tw.add_tab("X".to_string(), None);

        let svg = render_widget_to_svg(&mut tw, Rect::new(0, 0, 120, 80));
        assert!(svg.contains("width=\"120\""));
        assert!(svg.contains("height=\"80\""));
    }

    // ── 16. Tab accessor (tab / tab_mut) ──────────────────────────────────────

    #[test]
    fn tabwidget_tab_accessor_out_of_bounds() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.tab(0).is_none());
        assert!(tw.tab_mut(0).is_none());
    }

    #[test]
    fn tabwidget_tab_mut_allows_widget_assignment() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        tw.tab_mut(0).unwrap().set_widget(Some(wid1()));
        // Widget assigned to tab 0 is also the current widget (index 0)
        assert_eq!(tw.tab(0).unwrap().widget(), Some(wid1()));
        assert_eq!(tw.current_widget(), Some(wid1()));
    }

    // ── 17. Registry round-trip ───────────────────────────────────────────────

    #[test]
    fn tabwidget_registry_set_and_get() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(tw.registry().is_none());
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        tw.set_registry(reg.clone());
        assert!(tw.registry().is_some());
        // Verify pointer identity
        assert!(Rc::ptr_eq(&reg, tw.registry().unwrap()));
    }

    // ── 18. Tab icon set/get ───────────────────────────────────────────────────

    #[cfg(feature = "image")]
    #[test]
    fn tabwidget_tab_icon_default_none() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        tw.add_tab("Tab".to_string(), None);
        assert!(tw.tab(0).unwrap().icon().is_none());
    }

    // ── 19. Movable tabs (drag to reorder) ─────────────────────────────────────

    /// Builds a strip of `count` tabs with wide, well-separated bands.
    fn movable_strip(count: usize) -> TabWidget {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 200));
        for i in 0..count {
            tw.add_tab(format!("T{i}"), None);
        }
        tw.set_movable(true);
        tw
    }

    fn titles(tw: &TabWidget) -> Vec<String> {
        (0..tw.count()).map(|i| tw.tab_text(i).unwrap().to_string()).collect()
    }

    #[test]
    fn tabwidget_move_tab_reorders_and_emits() {
        let mut tw = movable_strip(3);
        let captured = Arc::new(Mutex::new(None::<(usize, usize)>));
        tw.tab_moved.connect({
            let captured = Arc::clone(&captured);
            move |value: Arc<(usize, usize)>| {
                *captured.lock().unwrap() = Some(*value);
            }
        });

        assert!(tw.move_tab(0, 2));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));

        // A move to the same index is not a move, so nothing is emitted.
        assert!(!tw.move_tab(1, 1));
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));

        // An out-of-range source is a caller error and does not reorder or emit.
        assert!(!tw.move_tab(99, 0));
        assert_eq!(*captured.lock().unwrap(), Some((0, 2)));
    }

    #[test]
    fn tabwidget_move_tab_clamps_the_target_to_the_last_tab() {
        let mut tw = movable_strip(3);
        // "Dropped past the end" is "moved to the end", not a refusal.
        assert!(tw.move_tab(0, 99));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
    }

    #[test]
    fn tabwidget_move_tab_keeps_the_selection_with_its_page() {
        let mut tw = movable_strip(3);
        tw.set_current_index(2);
        // Dragging the current tab must not leave the selection on the slot it vacated.
        assert!(tw.move_tab(2, 0));
        assert_eq!(titles(&tw), vec!["T2", "T0", "T1"]);
        assert_eq!(tw.current_index(), 0);

        // And dragging another tab across the selection shifts it by one slot.
        tw.set_current_index(0); // the dragged tab
        assert!(tw.move_tab(2, 0));
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
        assert_eq!(tw.current_index(), 1);
    }

    /// A press on a tab of a *movable* strip that then travels reorders it.
    #[test]
    fn tabwidget_drag_reorders_a_movable_tab() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);

        tw.handle_event(&Event::MousePress { pos: press, button: 1 });
        // A press alone must not reorder: the gesture is still a click at this point.
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);

        // Travel past the drag threshold and onto the third tab.
        tw.handle_event(&Event::MouseMove {
            pos: Point::new(third.x + third.width as i32 / 2, press.y),
        });
        assert_eq!(
            titles(&tw),
            vec!["T1", "T2", "T0"],
            "crossing two neighbours must carry the tab to the far end"
        );

        tw.handle_event(&Event::MouseRelease {
            pos: Point::new(third.x + third.width as i32 / 2, press.y),
            button: 1,
        });
        // The release settles the gesture; the live reorder already happened.
        assert_eq!(titles(&tw), vec!["T1", "T2", "T0"]);
    }

    /// The whole point of the flag: with it off, the very same gesture must not reorder.
    #[test]
    fn tabwidget_drag_does_nothing_when_not_movable() {
        let mut tw = TabWidget::new(Rect::new(0, 0, 600, 200));
        for i in 0..3 {
            tw.add_tab(format!("T{i}"), None);
        }
        assert!(!tw.movable());

        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let y = first.y + 5;
        tw.handle_event(&Event::MousePress {
            pos: Point::new(first.x + first.width as i32 / 2, y),
            button: 1,
        });
        tw.handle_event(&Event::MouseMove { pos: Point::new(third.x + third.width as i32 / 2, y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
        // The press still selected, which is the behaviour a non-movable strip keeps.
        assert_eq!(tw.current_index(), 0);
    }

    /// A press-and-jitter must stay a click rather than a reorder.
    #[test]
    fn tabwidget_a_jitter_below_the_threshold_does_not_reorder() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let press = Point::new(first.x + first.width as i32 / 2, first.y + 5);
        tw.handle_event(&Event::MousePress { pos: press, button: 1 });
        tw.handle_event(&Event::MouseMove { pos: Point::new(press.x + 1, press.y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
    }

    /// Turning `movable` off mid-drag must abandon the gesture, not finish it.
    #[test]
    fn tabwidget_set_movable_false_cancels_a_live_drag() {
        let mut tw = movable_strip(3);
        let first = tw.tab_rect(0).expect("tab 0 has a band");
        let third = tw.tab_rect(2).expect("tab 2 has a band");
        let y = first.y + 5;
        tw.handle_event(&Event::MousePress {
            pos: Point::new(first.x + first.width as i32 / 2, y),
            button: 1,
        });
        tw.handle_event(&Event::MouseMove {
            pos: Point::new(first.x + first.width as i32 / 2 + 10, y),
        });
        tw.set_movable(false);
        tw.handle_event(&Event::MouseMove { pos: Point::new(third.x + third.width as i32 / 2, y) });
        assert_eq!(titles(&tw), vec!["T0", "T1", "T2"]);
    }

    #[test]
    fn tabwidget_movable_is_a_served_property() {
        use crate::widget::capability::properties_trait::{
            widget_property_get, widget_property_set,
        };
        use crate::widget::capability::CapabilityValue;

        let mut tw = TabWidget::new(Rect::new(0, 0, 300, 200));
        assert!(crate::widget::capability::widget_property_names(&tw)
            .expect("a tab widget declares properties")
            .contains(&"movable"));
        widget_property_set(&mut tw, "movable", CapabilityValue::Bool(true)).unwrap();
        assert!(tw.movable());
        assert_eq!(widget_property_get(&tw, "movable"), Ok(CapabilityValue::Bool(true)));

        // `tab_position` and `closable` round-trip through their tokens too.
        widget_property_set(&mut tw, "tab_position", CapabilityValue::String("south".to_string()))
            .unwrap();
        assert_eq!(tw.tab_position(), TabPosition::South);
        assert_eq!(
            widget_property_get(&tw, "tab_position"),
            Ok(CapabilityValue::String("south".to_string()))
        );
        assert!(widget_property_set(
            &mut tw,
            "tab_position",
            CapabilityValue::String("South".to_string())
        )
        .is_err());
        assert_eq!(tw.tab_position(), TabPosition::South);
    }
}
