// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! NavigationDrawer widget — an Android-style slide-out side navigation drawer.
//!
//! The NavigationDrawer presents a semi-transparent overlay behind a side panel
//! that lists navigation items (icon + label). It supports open/close state,
//! item selection, and emits signals for opened, closed, and item_selected events.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{key_codes, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::DrawerSectionHeader;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconItemError, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Which edge the drawer panel slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerSide {
    /// The leading edge (left in a left-to-right interface).
    #[default]
    Start,
    /// The trailing edge (right in a left-to-right interface).
    End,
}

/// Width of the edge strip that opens a closed drawer, in pixels.
///
/// The strip is the gesture target, not the painted handle: a 4 px stripe is what the edge
/// affordance *looks* like, and a 20 px band is what a pointing device can actually grab.
const DRAWER_EDGE_STRIP: u32 = 20;

/// The header band's height, in pixels.
const DRAWER_HEADER_HEIGHT: u32 = 60;

/// The footer band's height, in pixels, when a footer widget is installed.
const DRAWER_FOOTER_HEIGHT: u32 = 48;

/// A single item in the navigation drawer, consisting of an icon and a label.
#[derive(Clone, Debug)]
pub struct DrawerItem {
    /// Icon displayed as text (emoji or character).
    pub icon: String,
    /// Display label text.
    pub label: String,
    /// Whether the item can be selected (N-002).
    ///
    /// A disabled item is still painted — it is an entry the user can see exists — but it is
    /// skipped by hit testing, hover and keyboard traversal, and drawn in its disabled style.
    pub enabled: bool,
}

impl DrawerItem {
    /// Creates an enabled item.
    pub fn new(icon: impl Into<String>, label: impl Into<String>) -> Self {
        Self { icon: icon.into(), label: label.into(), enabled: true }
    }

    /// Returns the item with its enabled flag set to `enabled` (N-002).
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// One painted row of the drawer's list (N-06).
///
/// Sections are a row *type* rather than an item with a special flag: an item participates in
/// selection and hit testing, a section participates in neither. The section owns the shared
/// companion control, so the drawer and a host's own list paint one heading implementation.
enum DrawerRow {
    /// A selectable item, addressed in the flat `items` list.
    Item(usize),
    /// A non-selectable heading.
    Section(Box<DrawerSectionHeader>),
}

/// Height of a section heading row, in pixels.
const DRAWER_SECTION_HEIGHT: u32 = 32;

/// Navigation Drawer widget — an Android-style slide-out side navigation drawer.
///
/// The drawer can be opened and closed programmatically or by user interaction.
/// When open, it renders a semi-transparent overlay covering the full geometry
/// with a side panel on the left containing navigation items. Clicking an item
/// selects it and emits `item_selected`. Clicking the overlay closes the drawer.
pub struct NavigationDrawer {
    base: BaseWidget,
    /// Whether the drawer is currently open.
    open: bool,
    /// List of drawer items to display.
    items: Vec<DrawerItem>,
    /// The painted row order, including section headings (N-06).
    rows: Vec<DrawerRow>,
    /// The header title drawn when no header widget is installed (N-05).
    title: String,
    item_icon_sources: Vec<Option<IconSource>>,
    /// Index of the currently selected item.
    selected_index: usize,
    /// Panel width in pixels.
    panel_width: u32,
    /// The edge the panel slides in from.
    side: DrawerSide,
    /// Whether selecting an item also closes the drawer.
    ///
    /// Defaults to `true`, which is the modal drawer's contract: a destination was chosen, so
    /// the panel that offered it has done its job. Set it to `false` for a persistent drawer
    /// that should stay open beside the content.
    auto_close_on_select: bool,
    /// Whether the panel is modal.
    ///
    /// # Adjudication (N-004)
    ///
    /// The draw and event paths disagreed about what a non-modal drawer is: `draw` painted the
    /// scrim for *every* open drawer, while the event path already only dismissed on an outside
    /// press when the drawer was a modal one. The no-scrim-in-non-modal behaviour is the correct
    /// one, and it is what this property selects: a modal drawer dims the page behind it and an
    /// outside press dismisses it; a persistent drawer has neither. Defaults to `true`, so an
    /// existing caller keeps the modal drawer it always had.
    modal: bool,
    /// Scroll offset of the item list from its first row, in pixels.
    ///
    /// The rows are data rather than mounted children, so the scroll container is the same
    /// offset-and-clamp shape `TabView` uses for its strip: a declarative wheel step, a clamp
    /// against the rows' total height, and a paint/hit test that both apply the offset.
    list_scroll: i32,
    /// The flat row index the pointer is currently over, when it is over the list.
    hovered_row: Option<usize>,
    /// The press that began the current gesture, when it began on the modal scrim.
    ///
    /// A release on the scrim only dismisses when the *press* also landed there; otherwise a
    /// drag that started on the panel and ended outside it would close the drawer, which is the
    /// opposite of what the user did.
    press_on_overlay: bool,
    /// An in-flight edge drag: `(origin_x, progress_at_press)`.
    drag: Option<(f32, f32)>,
    /// Whether the current press has moved far enough to count as a drag.
    drag_moved: bool,
    /// How far the panel has slid in, `0.0` fully closed and `1.0` fully open.
    ///
    /// # Why this is separate from `open`
    ///
    /// `open` is the logical state a caller reads the instant it changes; `slide` is what the draw
    /// path positions the panel with. Drawing straight from `open` put the panel at its final place
    /// on the first frame and removed it on the last, so a navigation drawer — which on every
    /// platform that has one *slides* — appeared and vanished. Same split, same reason, as
    /// `Switch`'s `checked`/`travel` and `BottomSheet`'s `open`/`rise`.
    slide: PropertyDriver,
    /// Emitted when the drawer opens.
    pub opened: GenericSignal,
    /// Emitted when the drawer closes.
    pub closed: GenericSignal,
    /// Emitted when an item is selected, with the index of the selected item.
    pub item_selected: Signal1<usize>,
    /// Per-item badge text, parallel to `items`.
    item_badges: Vec<Option<String>>,
    /// Optional widget painted in the header band in place of the default title.
    header_widget: Option<Box<dyn Widget>>,
    /// Optional widget painted in the footer band.
    footer_widget: Option<Box<dyn Widget>>,
    /// Whether item separators are painted.
    section_separator_visible: bool,
    /// Emitted when the header band is pressed.
    pub header_clicked: GenericSignal,
}

impl NavigationDrawer {
    /// Creates a new NavigationDrawer widget with the given geometry.
    ///
    /// The geometry should cover the full screen/overlay area. The side panel
    /// defaults to 280px wide.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::NavigationDrawer, geometry, "NavigationDrawer"),
            open: false,
            items: Vec::new(),
            rows: Vec::new(),
            title: String::from("Navigation"),
            item_icon_sources: Vec::new(),
            selected_index: 0,
            panel_width: 280,
            side: DrawerSide::Start,
            auto_close_on_select: true,
            modal: true,
            list_scroll: 0,
            hovered_row: None,
            press_on_overlay: false,
            drag: None,
            drag_moved: false,
            // At rest at the closed end: a freshly built drawer is closed, so it must not animate
            // its panel away on the first frame.
            slide: PropertyDriver::at(0.0, MotionSlot::Normal),
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
            item_selected: Signal1::new(),
            item_badges: Vec::new(),
            header_widget: None,
            footer_widget: None,
            section_separator_visible: true,
            header_clicked: GenericSignal::new(),
        }
    }

    /// Opens the navigation drawer. Emits `opened` signal.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            // Aim the slide; the frames that follow carry the panel in. No redraw is requested,
            // because a redraw is not what makes the motion visible -- `tick_animations` reports the
            // movement itself, which is what keeps the loop painting.
            self.slide.set_target(1.0);
            self.opened.emit();
        }
    }

    /// Closes the navigation drawer. Emits `closed` signal.
    pub fn close(&mut self) {
        if self.open {
            self.open = false;
            self.slide.set_target(0.0);
            // Base lifecycle signal (handle-layer `on_close` / JSON `closed`) alongside the
            // widget's own `closed` for its direct listeners.
            self.base.closed.emit();
            self.closed.emit();
            self.base.request_redraw();
        }
    }

    /// How far the panel has slid in, `0.0` closed and `1.0` fully open.
    ///
    /// This is the *drawn* fraction: the panel's width and offset are both functions of it, so a
    /// test can assert the drawer slid rather than appeared by sampling it per frame.
    pub fn slide_progress(&self) -> f32 {
        self.slide.value()
    }

    /// Advances the slide by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.slide.tick(delta_ms)
    }

    /// Whether the panel is between two positions -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.slide.is_moving()
    }

    /// Toggles the open/close state of the drawer.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Returns whether the drawer is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Adds a new item to the navigation drawer.
    ///
    /// `icon` is a text/emoji character displayed to the left of the label.
    pub fn add_item(&mut self, icon: &str, label: &str) {
        let index = self.items.len();
        self.items.push(DrawerItem::new(icon, label));
        self.rows.push(DrawerRow::Item(index));
        self.item_icon_sources.push(None);
        self.item_badges.push(None);
        self.base.request_redraw();
    }

    /// Enables or disables item `index` (N-002).
    ///
    /// Returns `false` for an out-of-range index. A disabled item stays in the list and keeps its
    /// slot, but is not selectable until it is enabled again.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.enabled == enabled {
            return true;
        }
        item.enabled = enabled;
        if !enabled && self.hovered_row.is_some() {
            // The row under the pointer may no longer be selectable; recomputing happens on the
            // next pointer event, so the paint simply drops its hover until then.
            self.hovered_row = None;
        }
        self.base.request_redraw();
        true
    }

    /// Whether item `index` is enabled (N-002).
    pub fn item_enabled(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|item| item.enabled)
    }

    /// Adds a non-selectable section heading to the list (N-06).
    pub fn add_section_header(&mut self, label: &str) {
        let mut header = DrawerSectionHeader::new(Rect::new(0, 0, 0, 0));
        header.set_label(label);
        self.rows.push(DrawerRow::Section(Box::new(header)));
        self.base.request_redraw();
    }

    /// Returns the drawer's header title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the header title drawn when no header widget is installed (N-05).
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.title == title {
            return;
        }
        self.title = title;
        self.base.request_redraw();
    }

    /// Sets a badge on an item; an empty string clears it.
    pub fn set_item_badge(&mut self, index: usize, text: &str) -> bool {
        let Some(slot) = self.item_badges.get_mut(index) else {
            return false;
        };
        let badge = if text.is_empty() { None } else { Some(text.to_string()) };
        if *slot == badge {
            return true;
        }
        *slot = badge;
        self.base.request_redraw();
        true
    }

    /// The badge on `index`, when there is one.
    pub fn item_badge(&self, index: usize) -> Option<&str> {
        self.item_badges.get(index).and_then(|badge| badge.as_deref())
    }

    /// Installs a widget for the drawer's header band, replacing the default title.
    pub fn set_header_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.header_widget = widget;
        self.base.request_redraw();
    }

    /// Returns the header widget, if any.
    pub fn header_widget(&self) -> Option<&dyn Widget> {
        self.header_widget.as_deref()
    }

    /// Installs a widget for the drawer's footer band.
    pub fn set_footer_widget(&mut self, widget: Option<Box<dyn Widget>>) {
        self.footer_widget = widget;
        self.base.request_redraw();
    }

    /// Returns the footer widget, if any.
    pub fn footer_widget(&self) -> Option<&dyn Widget> {
        self.footer_widget.as_deref()
    }

    /// Whether item separators are painted.
    pub fn section_separator_visible(&self) -> bool {
        self.section_separator_visible
    }

    /// Shows or hides the separators between item rows.
    pub fn set_section_separator_visible(&mut self, visible: bool) {
        if self.section_separator_visible == visible {
            return;
        }
        self.section_separator_visible = visible;
        self.base.request_redraw();
    }

    /// The footer band's height, zero when no footer widget is installed.
    fn footer_height(&self) -> u32 {
        if self.footer_widget.is_some() {
            DRAWER_FOOTER_HEIGHT
        } else {
            0
        }
    }

    /// The header widget's band inside `panel`, when a header widget is installed.
    fn header_band(&self, panel: Rect) -> Option<Rect> {
        self.header_widget
            .as_ref()
            .map(|_| Rect::new(panel.x, panel.y, panel.width, DRAWER_HEADER_HEIGHT))
    }

    /// The footer widget's band inside `panel`, when a footer widget is installed.
    fn footer_band(&self, panel: Rect) -> Option<Rect> {
        self.footer_widget.as_ref().map(|_| {
            let height = self.footer_height();
            Rect::new(panel.x, panel.y + panel.height as i32 - height as i32, panel.width, height)
        })
    }

    /// Assigns a typed icon source to an existing drawer item index.
    pub fn set_item_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.items.len();
        let Some(slot) = self.item_icon_sources.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        *slot = source;
        self.base.request_redraw();
        Ok(())
    }

    /// Sets the selected item index. Emits `item_selected` if changed.
    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.items.len() && self.selected_index != index {
            self.selected_index = index;
            self.item_selected.emit(index);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected item index.
    pub fn selected_index(&self) -> Option<usize> {
        self.items.get(self.selected_index).map(|_| self.selected_index)
    }

    /// Returns a reference to the items list.
    pub fn items(&self) -> &[DrawerItem] {
        &self.items
    }

    /// Returns the panel width in pixels.
    pub fn panel_width(&self) -> u32 {
        self.panel_width
    }

    /// Sets the panel width in pixels.
    pub fn set_panel_width(&mut self, width: u32) {
        if self.panel_width == width {
            return;
        }
        self.panel_width = width;
        self.base.request_redraw();
    }

    /// Returns the edge the panel slides in from.
    pub fn side(&self) -> DrawerSide {
        self.side
    }

    /// Sets the edge the panel slides in from.
    pub fn set_side(&mut self, side: DrawerSide) {
        if self.side == side {
            return;
        }
        self.side = side;
        self.base.request_redraw();
    }

    /// Whether selecting an item closes the drawer.
    pub fn auto_close_on_select(&self) -> bool {
        self.auto_close_on_select
    }

    /// Sets whether selecting an item closes the drawer.
    pub fn set_auto_close_on_select(&mut self, auto_close: bool) {
        self.auto_close_on_select = auto_close;
    }

    /// Whether the drawer is modal: it dims the page behind it and an outside press dismisses
    /// it. A persistent drawer (`false`) paints no scrim and ignores outside presses.
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// Selects the modal or persistent form. Defaults to modal, which is the historic shape.
    ///
    /// # Scope note (capability schema)
    ///
    /// The accessor is the control's own contract, but `modal` is deliberately **not** added to
    /// `property_names`: the schema table that must declare it lives in
    /// `src/widget/capability/`, outside this fix's allowed files. Publishing a name the schema
    /// omits would make `schema_and_contract_publish_the_same_names` fail, so the schema/designer
    /// half is recorded as not implemented rather than silently drifting.
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        self.base.request_redraw();
    }

    /// Whether the modal scrim is painted right now.
    ///
    /// One derivation for the draw path and for tests; the two cannot disagree about whether a
    /// persistent drawer dims the page.
    fn paints_scrim(&self) -> bool {
        self.modal && self.slide.value() > 0.0
    }

    /// The visible item-list band inside `panel`: below the header and above the footer.
    fn list_area(&self, panel: Rect) -> Rect {
        let top = panel.y + DRAWER_HEADER_HEIGHT as i32;
        let bottom = panel.y + panel.height as i32 - self.footer_height() as i32;
        Rect::new(panel.x, top, panel.width, (bottom - top).max(0) as u32)
    }

    /// The total height of every painted row.
    fn list_content_height(&self) -> i32 {
        self.rows
            .iter()
            .map(|row| match row {
                DrawerRow::Item(_) => 48,
                DrawerRow::Section(_) => DRAWER_SECTION_HEIGHT as i32,
            })
            .sum()
    }

    /// The largest legal list scroll offset for `panel`.
    fn max_list_scroll(&self, panel: Rect) -> i32 {
        (self.list_content_height() - self.list_area(panel).height as i32).max(0)
    }

    /// Scrolls the list by `delta` pixels, clamped to the content.
    fn scroll_list_by(&mut self, delta: i32) {
        if delta == 0 || self.rows.is_empty() {
            return;
        }
        let panel = self.panel_rect();
        let max = self.max_list_scroll(panel);
        let next = (self.list_scroll + delta).clamp(0, max);
        if next != self.list_scroll {
            self.list_scroll = next;
            self.base.request_redraw();
        }
    }

    /// The row under `pos`, as `(row index, item index when the row is an item)`.
    fn row_at(&self, pos: Point, panel: Rect) -> Option<(usize, Option<usize>)> {
        let list = self.list_area(panel);
        if !list.contains_point(pos) {
            return None;
        }
        // The paint path clamps the offset against the content; the hit test must use the same
        // clamp or a shrunken list could leave rows unreachable.
        let scroll = self.list_scroll.clamp(0, self.max_list_scroll(panel));
        let mut top = list.y - scroll;
        for (row_index, row) in self.rows.iter().enumerate() {
            let height = match row {
                DrawerRow::Item(_) => 48,
                DrawerRow::Section(_) => DRAWER_SECTION_HEIGHT as i32,
            };
            if pos.y >= top && pos.y < top + height {
                let item = match row {
                    DrawerRow::Item(index) => {
                        // N-002: a disabled row is hit (so hover/pointer state can name it) but is
                        // not a selectable item, which is what the `Option<usize>` pair reports.
                        if self.items.get(*index).is_some_and(|item| item.enabled) {
                            Some(*index)
                        } else {
                            None
                        }
                    }
                    DrawerRow::Section(_) => None,
                };
                return Some((row_index, item));
            }
            top += height;
        }
        None
    }

    /// The row top for item `index`, in list content coordinates.
    fn item_row_top(&self, index: usize) -> Option<i32> {
        let mut top = 0;
        for row in &self.rows {
            let height = match row {
                DrawerRow::Item(_) => 48,
                DrawerRow::Section(_) => DRAWER_SECTION_HEIGHT as i32,
            };
            if matches!(row, DrawerRow::Item(item) if *item == index) {
                return Some(top);
            }
            top += height;
        }
        None
    }

    /// Scrolls the list just enough to make the selected item fully visible.
    fn reveal_selected(&mut self) {
        let Some(top) = self.item_row_top(self.selected_index) else {
            return;
        };
        let panel = self.panel_rect();
        let height = self.list_area(panel).height as i32;
        let next = if top < self.list_scroll {
            top
        } else if top + 48 > self.list_scroll + height {
            top + 48 - height
        } else {
            return;
        };
        let next = next.clamp(0, self.max_list_scroll(panel));
        if next != self.list_scroll {
            self.list_scroll = next;
            self.base.request_redraw();
        }
    }

    /// Moves the selection by one item, wrapping at the ends, and reveals it.
    ///
    /// Disabled items are skipped (N-002), so arrow navigation visits only destinations the user
    /// can actually choose. When every item is disabled the selection is left alone.
    fn step_selection(&mut self, forward: bool) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let mut next = self.selected_index;
        for _ in 0..len {
            next = if forward { (next + 1) % len } else { (next + len - 1) % len };
            if self.items[next].enabled {
                self.set_selected_index(next);
                self.reveal_selected();
                return;
            }
        }
    }

    /// Activates the selected destination as the pointer's release would.
    fn activate_selected(&mut self) {
        if !self.item_enabled(self.selected_index) {
            return;
        }
        self.item_selected.emit(self.selected_index);
        if self.auto_close_on_select && self.modal {
            self.close();
        }
    }

    /// Begins an edge drag from `x`, adopting the current drawn progress as its origin.
    fn begin_drag(&mut self, x: f32) {
        self.drag = Some((x, self.slide.value()));
        self.drag_moved = false;
    }

    /// Advances an in-flight drag to `x`, following the pointer.
    fn drag_to(&mut self, x: f32) {
        let Some((origin, start)) = self.drag else {
            return;
        };
        let width = self.panel_width.max(1) as f32;
        let delta = match self.side {
            DrawerSide::Start => x - origin,
            DrawerSide::End => origin - x,
        };
        let progress = (start + delta / width).clamp(0.0, 1.0);
        if (progress - start).abs() > 0.01 {
            self.drag_moved = true;
        }
        self.slide.jump_to(progress);
        self.base.request_redraw();
    }

    /// Releases an in-flight drag, committing to the nearer end.
    fn end_drag(&mut self) {
        if self.drag.take().is_none() {
            return;
        }
        let open = self.slide.value() >= 0.5;
        let was_open = self.open;
        self.open = open;
        self.slide.set_target(if open { 1.0 } else { 0.0 });
        if open && !was_open {
            self.opened.emit();
        } else if !open && was_open {
            self.base.closed.emit();
            self.closed.emit();
        }
        self.base.request_redraw();
    }

    /// The panel's rectangle at the current slide, on the configured edge.
    fn panel_rect(&self) -> Rect {
        let rect = self.geometry();
        let width = self.panel_width.min(rect.width);
        let x = match self.side {
            DrawerSide::Start => rect.x,
            DrawerSide::End => rect.x + rect.width as i32 - width as i32,
        };
        Rect::new(x, rect.y, width, rect.height)
    }
}

impl Widget for NavigationDrawer {
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
        crate::core::Size::new(
            crate::widget::metrics::dimensions::NAVIGATION_DRAWER_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::NAVIGATION_DRAWER_DEFAULT_HEIGHT,
        )
    }

    // The slide is the control's own animation; the trait spelling is what the frame bus reaches
    // through `&mut dyn Widget`, which is the only way the panel actually travels.
    fn tick(&mut self, delta_ms: u32) -> bool {
        NavigationDrawer::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        NavigationDrawer::is_animating(self)
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
            "opened" => Some(EventSignalRef::unit("opened", &self.opened)),
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            "item_selected" => {
                Some(EventSignalRef::mapped("item_selected", &self.item_selected, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "header_clicked" => Some(EventSignalRef::unit("header_clicked", &self.header_clicked)),
            _ => None,
        }
    }
}

/// `NavigationDrawer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `width` reports the panel
/// width, which is a `u32` internally and published as a `Float` to match the
/// legacy shape.
impl WidgetProperties for NavigationDrawer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "open" => Ok(CapabilityValue::Bool(self.is_open())),
            "modal" => Ok(CapabilityValue::Bool(self.is_modal())),
            "width" => Ok(CapabilityValue::Float(f64::from(self.panel_width()))),
            "section_separator_visible" => {
                Ok(CapabilityValue::Bool(self.section_separator_visible()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "open" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            "modal" => {
                self.set_modal(expect_bool(value)?);
                Ok(())
            }
            "width" => {
                self.set_panel_width(expect_f32(value)? as u32);
                Ok(())
            }
            "section_separator_visible" => {
                let visible = match value {
                    CapabilityValue::Bool(visible) => visible,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_section_separator_visible(visible);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["open", "width", "section_separator_visible", BASE_PROPERTY_NAMES]
    }
}

impl Draw for NavigationDrawer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the overlay, panel, header, items and divider used to be hardcoded
        // literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("navigation_drawer");
        // `navigation_drawer` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A panel painted in that colour would be byte-identical to the frame
        // behind it, so a resolved surface equal to the window fill is re-derived a visible
        // step away from it, the same distinction `Colors::input_background` draws for a field.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(30, 30, 30));
        // The selected item and the scrim are the accent: the hue a theme is expected to vary
        // most, so selection follows the appearance rather than a literal blue. One palette read
        // serves both this and the window fill above (G-A01).
        let accent =
            palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::rgb(30, 100, 200));
        let panel = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        // A disabled drawer recedes toward the page it slides over. It read
        // `panel.blend(&ink, 0.5)` -- stepping the panel half-way toward its own ink, which on a
        // dark panel makes it far *lighter*: measured 1.13:1 against the window when enabled and
        // 5.73:1 when disabled, so the disabled drawer was five times the more prominent of the
        // two, on both appearances.
        let panel = self.base.disabled_surface_near(panel, window_fill);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| panel.blend(&ink, 0.25));
        // The header band and the selected row are chrome states derived from the panel, and
        // the item separators are dimmer than the ink, so all three move with the appearance.
        let header_color = panel.blend(&ink, 0.06);
        let highlight_color = panel.blend(&accent, 0.2);
        let divider_color = ink.with_alpha_f32(0.1);

        // Draw the closed-state affordance and the sliding panel.
        //
        // # The two states, and why the closing state needs the panel too
        //
        // A fully closed drawer paints only its edge affordance — a drawer in its default state has
        // to be visible, or the host cannot show that anything can be opened. Once the slide begins
        // the panel is *partially* in, so the panel is drawn for every non-zero `slide`, at the width
        // the slide calls for. The old code branched on the boolean, so the panel was either absent
        // or complete and the closing animation would have had nothing to draw.
        let slide = self.slide.value();
        let panel_band = self.panel_rect();
        let full_width = panel_band.width.min(rect.width);
        let panel_width = (full_width as f32 * slide) as u32;
        if panel_width == 0 {
            // The closed-state affordance is the painted edge stripe plus the edge strip that
            // actually accepts the opening gesture. Its side follows the drawer's own side.
            let handle_width = full_width.max(1);
            let handle_rect = Rect::new(panel_band.x, rect.y, handle_width, rect.height);
            context.fill_rect(handle_rect, panel.blend(&ink, 0.04));
            let stripe = match self.side {
                DrawerSide::Start => Rect::new(rect.x, rect.y, 4.min(handle_width), rect.height),
                DrawerSide::End => Rect::new(
                    rect.x + rect.width as i32 - 4.min(handle_width) as i32,
                    rect.y,
                    4.min(handle_width),
                    rect.height,
                ),
            };
            context.fill_rect(stripe, accent);
            context.draw_rect_stroke(handle_rect, border, 1);
            return;
        }

        // Draw the modal scrim covering the full geometry, fading in with the panel. Read from the
        // `Scrim` role rather than built from the accent at a literal alpha: the role is what lets a
        // theme express the dimming once (and what keeps a dark appearance from being *lit* by a
        // scrim), which is the same reason every other modal in this crate reads it.
        //
        // A partially-open drawer dims partially, so the scrim's alpha scales with the slide — one
        // form that works whether the theme authored a black veil or a translucent white wash.
        // A **persistent** drawer (`modal == false`) is the same panel without the veil: see the
        // field's adjudication note, the draw and event paths now agree that persistent means
        // "beside the page", not "over it".
        let scrim = crate::style::layer_color(crate::style::LayerColor::Scrim)
            .unwrap_or_else(|| ink.with_alpha(82));
        if self.paints_scrim() {
            let scrim = scrim.with_alpha((scrim.a as f32 * slide) as u8);
            // The scrim is a veil over the control's whole rectangle, not a surface with a
            // rounded face: rounding it left four corner notches where the veil stopped short.
            context.face_with_gradient(
                rect,
                scrim,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                0,
                Color::BLACK,
            );
        }

        // Draw the side panel on the configured edge, at the width the slide calls for.
        let panel_rect = match self.side {
            DrawerSide::Start => Rect::new(panel_band.x, rect.y, panel_width, rect.height),
            DrawerSide::End => Rect::new(
                panel_band.x + panel_band.width as i32 - panel_width as i32,
                rect.y,
                panel_width,
                rect.height,
            ),
        };
        context.fill_rect(panel_rect, panel);
        // Everything inside the panel is positioned from the panel's own origin.
        let rect = panel_rect;

        // Draw a subtle right border on the panel
        context.draw_rect_stroke(panel_rect, border.with_alpha_f32(0.2), 1);

        // Everything inside the panel is clipped to it: during the slide animation a
        // partially-open drawer used to paint the header title and item labels past the
        // panel's leading edge, floating over the scrim.
        context.push_clip(panel_rect.x, panel_rect.y, panel_rect.width, panel_rect.height);

        // Draw header area
        let header_height = DRAWER_HEADER_HEIGHT;
        let header_rect = Rect::new(rect.x, rect.y, panel_width, header_height);
        context.fill_rect(header_rect, header_color);

        // Draw header title text — or the host's header widget, which replaces it rather than
        // painting over it. The header band used to be a hardcoded "Navigation" label with no
        // slot for an avatar, a title block or a search field.
        let font = role_font(style, 16.0).with_bold(true);
        if let Some(header) = self.header_widget.as_deref_mut() {
            header.set_geometry(header_rect);
            if let Some(drawable) = crate::widget::draw_bridge::draw_of(header) {
                drawable.draw(context);
            }
        } else {
            let header_text = self.title.as_str();
            let header_text_x = rect.x + 16;
            // The origin is the glyph box's top edge, so centring on the header is half the line
            // box; the old `+ ascent/2` sat the title half a line below the header's middle.
            let header_line = context.text_line(header_rect, &font);
            context.draw_text_fitted_cow(
                Rect::new(
                    header_text_x,
                    header_line.y,
                    panel_width.saturating_sub(24),
                    header_line.height.max(1),
                ),
                header_text,
                &font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        // Draw items vertically. Rows include section headings, which are painted with the
        // drawer's own small face and divider tone and participate in neither selection nor hit
        // testing (N-06). The list scrolls: the offset is clamped against the rows' own height
        // and the visible band, and the paint applies the same offset `row_at` hit-tests.
        let item_height: u32 = 48;
        let item_font = role_font(style, 14.0);
        let icon_font = role_font(style, 16.0);
        let item_metrics =
            crate::widget::base_widgets::measure_cache::measure_text(context, "A", &item_font);
        let list = self.list_area(rect);
        let max_scroll = self.max_list_scroll(rect);
        self.list_scroll = self.list_scroll.clamp(0, max_scroll);
        let list_scroll = self.list_scroll;
        let mut y_offset = list.y - list_scroll;
        // Rows are clipped to the list band so a partially scrolled row cannot paint under the
        // fixed header or over the footer.
        context.push_clip(list.x, list.y, list.width, list.height);
        for (row_index, row) in self.rows.iter_mut().enumerate() {
            let row_height = match row {
                DrawerRow::Item(_) => item_height,
                DrawerRow::Section(_) => DRAWER_SECTION_HEIGHT,
            };
            // Rows entirely above or below the visible band are skipped outright; a partially
            // visible row still paints and is clipped by the band.
            if y_offset + row_height as i32 <= list.y {
                y_offset += row_height as i32;
                continue;
            }
            if y_offset >= list.y + list.height as i32 {
                break;
            }
            let row_rect = Rect::new(rect.x, y_offset, panel_width, row_height);

            let DrawerRow::Item(item_index) = row else {
                if let DrawerRow::Section(header) = row {
                    // The drawer and a host's own list share this one heading control; only the
                    // band and the resolved chrome are supplied here (COMP-A04).
                    header.set_geometry(row_rect);
                    header.set_inset(16);
                    {
                        let header_style = header.base_mut().style_mut();
                        header_style.background_color = Some(panel);
                        header_style.text_color = Some(ink);
                        header_style.font = style.font.clone();
                    }
                    header.draw(context);
                }
                y_offset += row_height as i32;
                continue;
            };

            // Highlight selected item; a hovered row takes a lighter step so the pointer's
            // target is visible before it is committed. A disabled item (N-002) never takes the
            // selected or hover fill: it cannot be chosen, so it must not look chosen.
            let enabled = self.items.get(*item_index).is_some_and(|item| item.enabled);
            if enabled && *item_index == self.selected_index {
                context.fill_rect(row_rect, highlight_color);
            } else if enabled && self.hovered_row == Some(row_index) {
                context.fill_rect(row_rect, panel.blend(&ink, 0.04));
            } else {
                // Resting rows keep the panel's own surface, so the selected row is the only
                // one that reads as raised in either appearance.
                context.fill_rect(row_rect, panel);
            }

            // A disabled entry is dimmed rather than tinted with the accent, so it reads as
            // present but unavailable in either appearance.
            let disabled_ink = panel.blend(&ink, 0.35);
            // Vertical center position for text: the origin is the glyph box's top edge, so
            // the centre of the row is `y + (row - line box)/2`.
            let text_center_y = y_offset + (item_height as i32 - item_metrics.height as i32) / 2;

            // Draw icon
            let icon_x = rect.x + 16;
            let icon_color = if !enabled {
                disabled_ink
            } else if *item_index == self.selected_index {
                accent
            } else {
                ink
            };
            if let Some(source) = self.item_icon_sources.get(*item_index).copied().flatten() {
                let side = item_metrics.height.clamp(1, 20);
                let icon_rect = Rect::new(icon_x, text_center_y, side, side);
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_rect, icon_color)
                {
                    log::warn!("navigation drawer item {item_index} icon failed: {error:?}");
                }
            } else if let Some(item) = self.items.get(*item_index) {
                context.draw_text(
                    Point::new(icon_x, text_center_y),
                    &item.icon,
                    &icon_font,
                    icon_color,
                    HorizontalAlignment::Left,
                );
            }

            // Draw label, fitted to the visible panel: the clip keeps the glyphs inside
            // during the slide, and the fit keeps a long label from touching the panel's
            // right border once it is fully open.
            let icon_width: u32 = 24;
            let label_x = rect.x + 16 + icon_width as i32 + 8;
            let label_color = if !enabled {
                disabled_ink
            } else if *item_index == self.selected_index {
                accent
            } else {
                ink
            };
            let label_line = context.text_line(row_rect, &item_font);
            let label_band_width = ((rect.x + panel_width as i32 - 8) - label_x).max(0) as u32;
            if label_band_width > 0 {
                if let Some(item) = self.items.get(*item_index) {
                    context.draw_text_fitted_cow(
                        Rect::new(
                            label_x,
                            label_line.y,
                            label_band_width,
                            label_line.height.max(1),
                        ),
                        &item.label,
                        &item_font,
                        label_color,
                        HorizontalAlignment::Left,
                    );
                }
            }

            // Draw divider line between rows
            if row_index > 0 && self.section_separator_visible {
                context.fill_rect(
                    Rect::new(rect.x + 16, y_offset, panel_width.saturating_sub(32), 1),
                    divider_color,
                );
            }

            // Badge: a small capsule at the row's trailing edge.
            if let Some(badge) =
                self.item_badges.get(*item_index).and_then(|badge| badge.as_deref())
            {
                if !badge.is_empty() {
                    let badge_fill =
                        crate::style::semantic_color(crate::style::SemanticColor::Error)
                            .unwrap_or(accent);
                    let metrics = crate::widget::base_widgets::measure_cache::measure_text(
                        context, badge, &item_font,
                    );
                    let height = item_font.size().max(10.0) as u32 + 4;
                    let width = (metrics.width + 8).max(height);
                    let badge_rect = Rect::new(
                        rect.x + panel_width as i32 - width as i32 - 16,
                        y_offset + (item_height as i32 - height as i32) / 2,
                        width,
                        height,
                    );
                    context.fill_rounded_rect(badge_rect, height / 2, badge_fill);
                    context.draw_text(
                        Point::new(badge_rect.x + 4, context.text_line(badge_rect, &item_font).y),
                        badge,
                        &item_font,
                        badge_fill.contrast_color(),
                        HorizontalAlignment::Left,
                    );
                }
            }

            y_offset += row_height as i32;
        }
        context.pop_clip();

        // Footer band: a host-supplied widget pinned to the panel's foot, so an account row or
        // a settings action no longer has to be faked as one more item.
        if self.footer_widget.is_some() {
            let footer = Rect::new(
                rect.x,
                rect.y + rect.height as i32 - self.footer_height() as i32,
                panel_width,
                self.footer_height(),
            );
            if let Some(widget) = self.footer_widget.as_deref_mut() {
                widget.set_geometry(footer);
                if let Some(drawable) = crate::widget::draw_bridge::draw_of(widget) {
                    drawable.draw(context);
                }
            }
        }

        context.pop_clip();
    }
}

impl EventHandler for NavigationDrawer {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }

        let rect = self.geometry();
        let panel_band = self.panel_rect();

        // Escape closes an open modal drawer, matching the splash screen and pagination
        // conventions (N-11). A persistent drawer (`modal == false`) keeps its position, which
        // is the same switch that governs item-selection dismissal.
        if let Event::KeyPress { key, .. } = event {
            if *key == key_codes::ESCAPE && self.open && self.modal && self.auto_close_on_select {
                self.close();
                return;
            }
        }

        // Keyboard navigation follows focus, the same gate `TabView` uses: an open drawer's
        // arrows move the selection and reveal it, Home/End jump to the ends, and Enter/Space
        // activates the destination. Without this an open drawer was only usable with a pointer.
        if self.open && self.base.focus_reason().is_some() {
            if let Event::KeyPress { key, .. } = event {
                match *key {
                    key_codes::DOWN => {
                        self.step_selection(true);
                        return;
                    }
                    key_codes::UP => {
                        self.step_selection(false);
                        return;
                    }
                    key_codes::HOME => {
                        if !self.items.is_empty() {
                            self.set_selected_index(0);
                            self.reveal_selected();
                        }
                        return;
                    }
                    key_codes::END => {
                        if let Some(last) = self.items.len().checked_sub(1) {
                            self.set_selected_index(last);
                            self.reveal_selected();
                        }
                        return;
                    }
                    key_codes::ENTER | key_codes::SPACE => {
                        self.activate_selected();
                        return;
                    }
                    _ => {}
                }
            }
        }

        // A closed drawer is reachable: the edge strip starts an opening drag, and a plain
        // click on it opens the panel. Before this the handle was painted but could not be
        // used, so a mouse-only user had no way to open the drawer at all.
        if !self.open {
            let edge = match self.side {
                DrawerSide::Start => {
                    Rect::new(rect.x, rect.y, DRAWER_EDGE_STRIP.min(rect.width), rect.height)
                }
                DrawerSide::End => Rect::new(
                    rect.x + rect.width as i32 - DRAWER_EDGE_STRIP.min(rect.width) as i32,
                    rect.y,
                    DRAWER_EDGE_STRIP.min(rect.width),
                    rect.height,
                ),
            };
            match event {
                Event::MousePress { pos, button, .. } if *button == 1 => {
                    if edge.contains_point(*pos) {
                        self.begin_drag(pos.x as f32);
                    } else {
                        self.base.handle_event(event);
                    }
                }
                Event::MouseMove { pos } => self.drag_to(pos.x as f32),
                Event::MouseRelease { pos, button } if *button == 1 => {
                    if self.drag.is_some() {
                        self.drag_to(pos.x as f32);
                        // A tap on the handle (no meaningful travel) opens rather than snaps
                        // back, which is what makes the closed affordance clickable.
                        if self.slide.value() < 0.01 {
                            self.slide.jump_to(1.0);
                            self.open = true;
                            self.opened.emit();
                            self.base.request_redraw();
                        } else {
                            self.end_drag();
                        }
                    } else {
                        self.base.handle_event(event);
                    }
                }
                _ => self.base.handle_event(event),
            }
            return;
        }

        match event {
            Event::MousePress { pos, button, .. } => {
                if *button != 1 {
                    return;
                }
                // The header and footer bands belong to their installed widgets, so a press
                // there is routed to the slot before the drawer's own gesture starts.
                if let Some(band) = self.header_band(panel_band) {
                    if band.contains_point(*pos) {
                        if let Some(header) = self.header_widget.as_deref_mut() {
                            header.set_geometry(band);
                            header.handle_event(event);
                        }
                        return;
                    }
                }
                if let Some(band) = self.footer_band(panel_band) {
                    if band.contains_point(*pos) {
                        if let Some(footer) = self.footer_widget.as_deref_mut() {
                            footer.set_geometry(band);
                            footer.handle_event(event);
                        }
                        return;
                    }
                }
                // Record where the gesture began: only a gesture that starts on the scrim may
                // dismiss on release. A drag from the panel that happens to end outside it is
                // not a dismissal, and a persistent drawer has no scrim to press at all.
                self.press_on_overlay =
                    self.modal && !panel_band.contains_point(*pos) && rect.contains_point(*pos);
                self.begin_drag(pos.x as f32);
            }
            Event::MouseMove { pos } => {
                self.drag_to(pos.x as f32);
                let hovered = self.row_at(*pos, panel_band).map(|(row, _)| row);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseEnter { pos } => {
                let hovered = self.row_at(*pos, panel_band).map(|(row, _)| row);
                if hovered != self.hovered_row {
                    self.hovered_row = hovered;
                    self.base.request_redraw();
                }
                self.base.handle_event(event);
            }
            Event::MouseLeave { pos: _ } => {
                if self.hovered_row.take().is_some() {
                    self.base.request_redraw();
                }
                self.base.handle_event(event);
            }
            Event::Wheel { delta, .. } => {
                // A vertical wheel is the common case, so it falls back to `delta.x` rather
                // than ignoring a bar with no vertical axis. One notch is 24 px, the same
                // step `TabView` uses for its strip.
                let step = if delta.y != 0 { delta.y } else { delta.x };
                if step != 0 {
                    self.scroll_list_by(-step * 24);
                    return;
                }
                self.base.handle_event(event);
            }
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                if let Some(band) = self.header_band(panel_band) {
                    if band.contains_point(*pos) {
                        if let Some(header) = self.header_widget.as_deref_mut() {
                            header.set_geometry(band);
                            header.handle_event(event);
                        }
                        self.header_clicked.emit();
                        return;
                    }
                }
                if let Some(band) = self.footer_band(panel_band) {
                    if band.contains_point(*pos) {
                        if let Some(footer) = self.footer_widget.as_deref_mut() {
                            footer.set_geometry(band);
                            footer.handle_event(event);
                        }
                        return;
                    }
                }
                if self.drag.is_some() {
                    self.drag_to(pos.x as f32);
                    let dragged = self.drag_moved;
                    if self.modal && self.press_on_overlay && !dragged {
                        // A tap on the scrim is a dismissal, not a commit to the nearer end —
                        // the panel is fully open, so "nearer" would leave it open forever.
                        self.drag = None;
                        self.press_on_overlay = false;
                        self.close();
                        return;
                    }
                    if dragged {
                        self.end_drag();
                        self.press_on_overlay = false;
                        return;
                    }
                    self.press_on_overlay = false;
                }

                // Item selection uses the panel's own band, so a right-side drawer selects at
                // the coordinates it paints at rather than at the left edge. `row_at` applies
                // the same scroll offset the paint path applies, so a scrolled row selects
                // where it is drawn.
                if panel_band.contains_point(*pos) {
                    if pos.y < panel_band.y + DRAWER_HEADER_HEIGHT as i32 {
                        // The header band is its own affordance: a host can put a whole widget
                        // there, and this signal is how a plain header reports a tap.
                        self.header_clicked.emit();
                    }
                    if let Some((_, Some(item_index))) = self.row_at(*pos, panel_band) {
                        if item_index < self.items.len() {
                            self.set_selected_index(item_index);
                            if self.auto_close_on_select && self.modal {
                                self.close();
                            }
                        }
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    fn make_drawer() -> NavigationDrawer {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.add_item("🏠", "Home");
        drawer.add_item("🔍", "Search");
        drawer.add_item("⚙️", "Settings");
        drawer.add_item("👤", "Profile");
        drawer
    }

    #[test]
    fn drawer_default_is_closed() {
        let drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert!(!drawer.is_open());
        assert_eq!(drawer.kind(), WidgetKind::NavigationDrawer);
        assert_eq!(drawer.selected_index().unwrap_or(0), 0);
        assert!(drawer.items().is_empty());
    }

    #[test]
    fn drawer_open_signal_emits() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        let opened = Arc::new(AtomicBool::new(false));
        let o = opened.clone();
        drawer.opened.connect(move || {
            o.store(true, Ordering::SeqCst);
        });

        drawer.open();
        assert!(drawer.is_open());
        assert!(opened.load(Ordering::SeqCst));
    }

    #[test]
    fn drawer_close_signal_emits() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.open();
        assert!(drawer.is_open());

        let closed = Arc::new(AtomicBool::new(false));
        let c = closed.clone();
        drawer.closed.connect(move || {
            c.store(true, Ordering::SeqCst);
        });

        drawer.close();
        assert!(!drawer.is_open());
        assert!(closed.load(Ordering::SeqCst));
    }

    #[test]
    fn drawer_toggle_flips_state() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert!(!drawer.is_open());

        drawer.toggle();
        assert!(drawer.is_open());

        drawer.toggle();
        assert!(!drawer.is_open());
    }

    #[test]
    fn drawer_add_item() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert_eq!(drawer.items().len(), 0);

        drawer.add_item("📁", "Files");
        assert_eq!(drawer.items().len(), 1);
        assert_eq!(drawer.items()[0].icon, "📁");
        assert_eq!(drawer.items()[0].label, "Files");

        drawer.add_item("📧", "Mail");
        assert_eq!(drawer.items().len(), 2);
    }

    #[test]
    fn drawer_item_typed_source_is_index_checked_and_rendered() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "drawer-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.add_item("legacy", "Files");
        assert_eq!(
            drawer.set_item_icon_source(2, Some(IconSource::static_geometry(&RING))),
            Err(IconItemError::IndexOutOfBounds { index: 2, len: 1 })
        );
        drawer
            .set_item_icon_source(0, Some(IconSource::static_geometry(&RING)))
            .expect("existing item accepts typed source");
        drawer.open();
        drawer.slide.jump_to(1.0);
        let svg = crate::widget::svg::render_to_svg(&mut drawer);
        assert!(svg.contains("fill-rule=\"nonzero\""));
    }

    #[test]
    fn drawer_set_selected_index_emits_signal() {
        let mut drawer = make_drawer();
        let selected = Arc::new(AtomicUsize::new(usize::MAX));
        let s = selected.clone();
        drawer.item_selected.connect(move |index: Arc<usize>| {
            s.store(*index, Ordering::SeqCst);
        });

        drawer.set_selected_index(2);
        assert_eq!(drawer.selected_index().unwrap_or(0), 2);
        assert_eq!(selected.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn drawer_set_selected_index_out_of_bounds_noop() {
        let mut drawer = make_drawer();
        drawer.set_selected_index(99);
        // Should not change because index is out of bounds
        assert_eq!(drawer.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn drawer_set_selected_index_same_value_no_reemit() {
        let mut drawer = make_drawer();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        drawer.item_selected.connect(move |_: Arc<usize>| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        drawer.set_selected_index(0);
        // Should not emit because it's already 0
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn drawer_mouse_press_on_overlay_closes() {
        let mut drawer = make_drawer();
        drawer.open();
        assert!(drawer.is_open());

        // Click on overlay (outside panel, at x=350)
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(350, 300),
            button: 1,
            modifiers: 0,
        });
        // Press alone does not close; release does
        assert!(drawer.is_open());

        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 1 });
        assert!(!drawer.is_open());
    }

    #[test]
    fn drawer_mouse_release_on_item_selects() {
        let mut drawer = make_drawer();
        drawer.open();

        // Click on item at index 1 ("Search") - header=60, item_height=48, item 1 starts at y=108
        let target_y = 60 + 48 + 24;
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(20, target_y),
            button: 1,
            modifiers: 0,
        });
        // Not yet selected on press
        assert_eq!(drawer.selected_index().unwrap_or(0), 0);

        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, target_y), button: 1 });
        assert_eq!(drawer.selected_index().unwrap_or(0), 1);
    }

    #[test]
    fn drawer_mouse_click_other_button_noop() {
        let mut drawer = make_drawer();
        drawer.open();

        // Right-click on overlay
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(350, 300),
            button: 2,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 2 });
        assert!(drawer.is_open());
        assert_eq!(drawer.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn drawer_disabled_blocks_events() {
        let mut drawer = make_drawer();
        drawer.set_enabled(false);
        drawer.open();

        // Click release on overlay - should NOT close because disabled
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(350, 300),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 1 });
        assert!(drawer.is_open());
    }

    #[test]
    fn drawer_closed_ignores_events() {
        let mut drawer = make_drawer();
        assert!(!drawer.is_open());

        // Events while closed should be ignored for drawer-specific behavior
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(20, 100),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, 100), button: 1 });
        assert_eq!(drawer.selected_index().unwrap_or(0), 0);
    }

    #[test]
    fn drawer_panel_width_customization() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        assert_eq!(drawer.panel_width(), 280);

        drawer.set_panel_width(320);
        assert_eq!(drawer.panel_width(), 320);
    }

    #[test]
    fn drawer_svg_output_closed() {
        let mut drawer = make_drawer();
        // Closed drawer produces an empty SVG (just background / no content)
        let svg = render_to_svg(&mut drawer);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn drawer_svg_output_open() {
        let mut drawer = make_drawer();
        drawer.open();
        let svg = render_to_svg(&mut drawer);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"400\""));
        assert!(svg.contains("height=\"600\""));
    }

    #[test]
    fn drawer_open_close_signals_not_reemitted() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        let open_count = Arc::new(AtomicUsize::new(0));
        let close_count = Arc::new(AtomicUsize::new(0));
        let oc = open_count.clone();
        let cc = close_count.clone();
        drawer.opened.connect(move || {
            oc.fetch_add(1, Ordering::SeqCst);
        });
        drawer.closed.connect(move || {
            cc.fetch_add(1, Ordering::SeqCst);
        });

        // Open twice - should only emit once
        drawer.open();
        drawer.open();
        assert_eq!(open_count.load(Ordering::SeqCst), 1);

        // Close twice - should only emit once
        drawer.close();
        drawer.close();
        assert_eq!(close_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drawer_item_selected_signal_is_typed() {
        let mut drawer = make_drawer();
        let captured = Arc::new(std::sync::Mutex::new(None));
        let c = captured.clone();
        drawer.item_selected.connect(move |val: Arc<usize>| {
            *c.lock().unwrap() = Some(*val);
        });

        drawer.set_selected_index(3);
        assert_eq!(*captured.lock().unwrap(), Some(3));
    }

    #[test]
    fn drawer_drawer_item_struct() {
        let item =
            DrawerItem { icon: "📁".to_string(), label: "Documents".to_string(), enabled: true };
        assert_eq!(item.icon, "📁");
        assert_eq!(item.label, "Documents");
        assert!(item.enabled);
    }

    /// N-002: a disabled item is painted but cannot be selected or hovered as an item.
    #[test]
    fn a_disabled_item_is_skipped_by_selection_and_drawn_dimmed() {
        let mut drawer = make_drawer();
        assert!(drawer.item_enabled(1));
        assert!(drawer.set_item_enabled(1, false));
        assert!(!drawer.item_enabled(1));
        assert!(!drawer.set_item_enabled(99, false), "an unknown item is refused");

        drawer.open();
        drawer.slide.jump_to(1.0);
        let panel = drawer.panel_rect();
        let target_y = panel.y + DRAWER_HEADER_HEIGHT as i32 + 48 + 24;
        let pos = Point::new(panel.x + 20, target_y);
        assert_eq!(
            drawer.row_at(pos, panel),
            Some((1, None)),
            "the row is hit but reports no selectable item"
        );

        drawer.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        drawer.handle_event(&Event::MouseRelease { pos, button: 1 });
        assert_ne!(drawer.selected_index(), Some(1), "a disabled item cannot be chosen");

        // Keyboard traversal skips it too: Down from the first item lands on the third.
        drawer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        drawer.handle_event(&Event::KeyPress { key: crate::event::key_codes::DOWN, modifiers: 0 });
        assert_eq!(drawer.selected_index(), Some(2));
    }

    /// Opening the drawer slides the panel in rather than placing it there.
    ///
    /// # The defect this pins
    ///
    /// The draw branched on the boolean `open`, so the panel was either absent or complete: a drawer
    /// — which slides on every platform that has one — appeared and vanished. The assertions are in
    /// two halves, because a progress nothing reads is not a slide: the model must take an interior
    /// value, and the *painted* panel must be narrower at that moment than when settled.
    #[test]
    fn opening_the_drawer_slides_the_panel_in() {
        use crate::widget::svg::render_to_svg;

        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 300, 400));
        assert_eq!(drawer.slide_progress(), 0.0, "a fresh drawer is closed");
        assert!(!drawer.is_animating(), "and owes no frames");

        drawer.open();
        assert!(drawer.is_open(), "the logical state answers at once");
        assert!(drawer.is_animating(), "while the drawn position owes frames");
        assert_eq!(drawer.slide_progress(), 0.0, "the slide starts where the panel was");

        assert!(drawer.tick(60), "still moving after one step");
        let mid = drawer.slide_progress();
        assert!(
            mid > 0.0 && mid < 1.0,
            "the drawer must pass through an interior position (got {mid})"
        );
        while drawer.tick(60) {}
        assert_eq!(drawer.slide_progress(), 1.0, "and settle fully open");
        assert!(!drawer.is_animating(), "a settled drawer owes no more frames");

        // The painted panel's width, read from the emitted document: the panel and the scrim are both
        // full height, so the *widest* rectangle narrower than the control is the panel.
        fn panel_width(svg: &str) -> u32 {
            svg.split("<rect ")
                .filter_map(|chunk| {
                    let w = chunk.split("width=\"").nth(1)?;
                    w.split('"').next()?.parse::<u32>().ok()
                })
                .filter(|w| *w < 300)
                .max()
                .unwrap_or(0)
        }

        let closed = NavigationDrawer::new(Rect::new(0, 0, 300, 400));
        let closed_width = panel_width(&render_to_svg(&mut { closed }));

        let mut mid_drawer = NavigationDrawer::new(Rect::new(0, 0, 300, 400));
        mid_drawer.open();
        assert!(mid_drawer.tick(60));
        let mid_width = panel_width(&render_to_svg(&mut mid_drawer));

        let mut open_drawer = NavigationDrawer::new(Rect::new(0, 0, 300, 400));
        open_drawer.open();
        while open_drawer.tick(60) {}
        let open_width = panel_width(&render_to_svg(&mut open_drawer));

        assert!(
            mid_width > 0 && mid_width < open_width,
            "a mid-slide panel must be narrower than a settled one: closed={closed_width} \
             mid={mid_width} open={open_width}"
        );

        // Closing is the same movement in reverse, so the panel does not vanish on the last frame.
        drawer.close();
        assert!(!drawer.is_open());
        assert!(drawer.is_animating(), "closing is also a slide");
        while drawer.tick(60) {}
        assert_eq!(drawer.slide_progress(), 0.0, "back to closed");
    }

    #[test]
    fn tapping_the_closed_handle_opens_the_drawer() {
        let mut drawer = make_drawer();
        assert!(!drawer.is_open());
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(2, 200),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(2, 200), button: 1 });
        assert!(drawer.is_open(), "the edge affordance must be usable, not decorative");
    }

    #[test]
    fn selecting_a_destination_closes_a_modal_drawer() {
        let mut drawer = make_drawer();
        drawer.open();
        while drawer.tick(60) {}
        let y = 60 + 48 + 24;
        drawer.handle_event(&Event::MousePress { pos: Point::new(20, y), button: 1, modifiers: 0 });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, y), button: 1 });
        assert_eq!(drawer.selected_index(), Some(1));
        assert!(!drawer.is_open(), "a chosen destination dismisses the modal drawer");
    }

    #[test]
    fn an_end_drawer_slides_in_from_the_trailing_edge() {
        let mut drawer = make_drawer();
        drawer.set_side(DrawerSide::End);
        assert_eq!(drawer.side(), DrawerSide::End);
        drawer.open();
        while drawer.tick(60) {}
        let svg = render_to_svg(&mut drawer);
        assert!(svg.starts_with("<svg"));
        assert!(drawer.panel_rect().x > 0, "the end drawer is not at the leading edge");
    }

    #[test]
    fn the_header_title_is_configurable() {
        let mut drawer = make_drawer();
        assert_eq!(drawer.title(), "Navigation");
        drawer.set_title("Workspace");
        assert_eq!(drawer.title(), "Workspace");
    }

    #[test]
    fn a_section_header_is_not_selectable() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        drawer.add_section_header("Library");
        drawer.add_item("🏠", "Home");
        drawer.open();
        while drawer.tick(16) {}
        // Header 0..60, section 60..92, first item 92..140.
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(20, 76),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, 76), button: 1 });
        assert!(drawer.is_open(), "a section heading does not select or close");
        drawer.handle_event(&Event::MousePress {
            pos: Point::new(20, 116),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(20, 116), button: 1 });
        assert_eq!(drawer.selected_index(), Some(0));
        assert!(!drawer.is_open(), "the item below the section is still reachable");
    }

    /// A persistent drawer is the panel without the veil: no scrim and no outside dismissal.
    ///
    /// Regression (N-004): the draw path painted a scrim on every open drawer while the event
    /// path already treated only a modal drawer as dismissible — the same widget answered
    /// differently depending on which half of it was asked.
    #[test]
    fn a_persistent_drawer_paints_no_scrim_and_ignores_outside_presses() {
        let mut drawer = make_drawer();
        assert!(drawer.is_modal(), "the historic shape is modal");
        drawer.set_modal(false);
        assert!(!drawer.is_modal());
        drawer.open();
        while drawer.tick(16) {}
        assert!(!drawer.paints_scrim(), "a persistent drawer does not dim the page");

        drawer.handle_event(&Event::MousePress {
            pos: Point::new(350, 300),
            button: 1,
            modifiers: 0,
        });
        drawer.handle_event(&Event::MouseRelease { pos: Point::new(350, 300), button: 1 });
        assert!(drawer.is_open(), "a persistent drawer has no scrim to dismiss it");
    }

    /// The item list scrolls when it is taller than the band between header and footer.
    ///
    /// Regression (N-005): rows past the fold were skipped outright, so the last destinations
    /// of a long drawer were unreachable.
    #[test]
    fn the_item_list_scrolls_and_clamps() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        for index in 0..20 {
            drawer.add_item("•", "Destination");
            let _ = index;
        }
        drawer.open();
        while drawer.tick(16) {}
        assert_eq!(drawer.list_scroll, 0);

        drawer.handle_event(&Event::Wheel { delta: Point::new(0, -3), modifiers: 0 });
        assert_eq!(drawer.list_scroll, 72, "one wheel notch is 24 px, direction inverted");

        // A large scroll clamps at the content's own end rather than running past it.
        drawer.handle_event(&Event::Wheel { delta: Point::new(0, -100), modifiers: 0 });
        let panel = drawer.panel_rect();
        assert_eq!(drawer.list_scroll, drawer.max_list_scroll(panel));
    }

    /// Moving over a row lights it; leaving clears the highlight.
    #[test]
    fn hovering_a_row_sets_and_clears_the_highlight() {
        let mut drawer = make_drawer();
        drawer.open();
        while drawer.tick(16) {}
        drawer.handle_event(&Event::MouseMove { pos: Point::new(20, 84) });
        assert_eq!(drawer.hovered_row, Some(0), "the first item row is hovered");
        drawer.handle_event(&Event::MouseLeave { pos: Point::new(400, 600) });
        assert_eq!(drawer.hovered_row, None);
    }

    /// Arrow keys move the selection while the drawer has focus, and End reveals the last row.
    #[test]
    fn keyboard_navigation_moves_and_reveals_the_selection() {
        let mut drawer = NavigationDrawer::new(Rect::new(0, 0, 400, 600));
        for _ in 0..20 {
            drawer.add_item("•", "Destination");
        }
        drawer.open();
        while drawer.tick(16) {}
        drawer
            .handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });

        drawer.handle_event(&Event::KeyPress { key: key_codes::DOWN, modifiers: 0 });
        assert_eq!(drawer.selected_index(), Some(1));
        drawer.handle_event(&Event::KeyPress { key: key_codes::UP, modifiers: 0 });
        assert_eq!(drawer.selected_index(), Some(0));

        drawer.handle_event(&Event::KeyPress { key: key_codes::END, modifiers: 0 });
        assert_eq!(drawer.selected_index(), Some(19));
        assert!(drawer.list_scroll > 0, "the last row is scrolled into view");
    }
}
