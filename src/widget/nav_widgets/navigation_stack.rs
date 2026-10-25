// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! NavigationStack widget — a push/pop page navigation container.
//!
//! The NavigationStack widget manages a stack of pages (widgets) and displays the
//! topmost page along with a navigation bar. It supports push, pop, and pop-to-root
//! operations, similar to SwiftUI NavigationStack or UINavigationController.

use crate::core::{Color, HorizontalAlignment, Point, Rect, TextDirection};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Events emitted by NavigationStack when the page stack changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationEvent {
    /// A new page was pushed onto the stack.
    Pushed,
    /// The top page was popped from the stack.
    Popped,
    /// All pages were popped back to the root.
    PoppedToRoot,
}

/// Height of the navigation bar in logical pixels.
const NAV_BAR_HEIGHT: u32 = 44;

/// Width of one trailing action cell in the navigation bar.
const NAV_ACTION_WIDTH: u32 = 64;

/// Gap between the last action cell and the bar's trailing edge.
const NAV_ACTION_MARGIN: i32 = 8;

/// NavigationStack widget — a page-based navigation container.
///
/// Manages a stack of pages where only the topmost page is visible.
/// A navigation bar at the top shows the current page title and a back button
/// when there are pages below the top.
pub struct NavigationStack {
    base: BaseWidget,
    pages: Vec<Box<dyn Widget>>,
    navigation_bar_title: String,
    /// How far the content region is slid, `-1.0` arriving from the left (a pop) through `0.0`
    /// settled to `1.0` arriving from the right (a push).
    ///
    /// See `draw`: the pages are laid out in the content rect and painted under this
    /// transition offset, with the lower page trailing at 30% for parallax.
    slide: PropertyDriver,
    /// The writing direction the back affordance is placed by.
    ///
    /// The back button is the bar's **leading** affordance, so a right-to-left interface puts
    /// it at the right edge. Both the paint path and the hit test read this field, so the
    /// affordance cannot be drawn on one side and be tappable on the other.
    direction: TextDirection,
    /// An in-flight interactive pop gesture: `(start_x, start_slide)`.
    edge_drag: Option<(f32, f32)>,
    /// Per-page titles, addressed like `pages`; `None` falls back to the global title.
    page_titles: Vec<Option<String>>,
    /// Trailing action labels painted at the bar's end.
    actions: Vec<String>,
    /// Whether the back affordance is painted and hit-tested at all.
    back_visible: bool,
    /// Emitted with the index of a trailing action when it is pressed.
    pub action_index_pressed: Signal1<usize>,
    /// Emitted when the navigation state changes.
    pub navigation_changed: Signal1<NavigationEvent>,
}

impl NavigationStack {
    /// Creates a new NavigationStack widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::NavigationStack, geometry, "NavigationStack"),
            pages: Vec::new(),
            navigation_bar_title: String::new(),
            slide: PropertyDriver::at(0.0, MotionSlot::Normal),
            direction: TextDirection::default(),
            edge_drag: None,
            page_titles: Vec::new(),
            actions: Vec::new(),
            back_visible: true,
            action_index_pressed: Signal1::new(),
            navigation_changed: Signal1::new(),
        }
    }

    /// Pushes a new page onto the navigation stack.
    /// The new page becomes the visible topmost page.
    pub fn push(&mut self, page: Box<dyn Widget>) {
        self.pages.push(page);
        // A pushed page starts with no title of its own and inherits the global one, which is
        // what a stack that never calls `set_page_title` has always shown.
        self.page_titles.push(None);
        // The new page arrives from the trailing edge, then settles. `jump_to` sets the starting
        // offset so the next frame is the first frame of the movement rather than a snap.
        self.slide.jump_to(1.0);
        self.slide.set_target(0.0);
        self.navigation_changed.emit(NavigationEvent::Pushed);
        self.base.request_redraw();
    }

    /// Returns the index of the current (topmost) page, or `None` when the
    /// stack is empty.
    pub fn current_page_index(&self) -> Option<usize> {
        self.pages.len().checked_sub(1)
    }

    /// Pops the topmost page from the stack and returns it.
    /// Returns `None` if there is only one page (the root) or the stack is empty.
    pub fn pop(&mut self) -> Option<Box<dyn Widget>> {
        if self.pages.len() <= 1 {
            return None;
        }
        let popped = self.pages.pop();
        if self.page_titles.len() > self.pages.len() {
            self.page_titles.truncate(self.pages.len());
        }
        // A pop arrives from the *leading* edge, which is the opposite of a push: that difference
        // is the whole information content of the transition, and a single direction would make a
        // back gesture look identical to a forward one.
        self.slide.jump_to(-1.0);
        self.slide.set_target(0.0);
        self.navigation_changed.emit(NavigationEvent::Popped);
        self.base.request_redraw();
        popped
    }

    /// Returns a reference to the current (topmost) page, or `None` if the stack is empty.
    pub fn current_page(&self) -> Option<&dyn Widget> {
        self.pages.last().map(|p| p.as_ref())
    }

    /// Returns a mutable reference to the current (topmost) page, or `None` if the stack is empty.
    pub fn current_page_mut(&mut self) -> Option<&mut dyn Widget> {
        self.pages.last_mut().map(|p| p.as_mut())
    }

    /// Returns the number of pages in the stack.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Returns whether the stack has more than one page (i.e., popping is possible).
    pub fn can_pop(&self) -> bool {
        self.pages.len() > 1
    }

    /// Pops all pages except the root page (the first page).
    pub fn pop_to_root(&mut self) {
        if self.pages.is_empty() {
            return;
        }
        let changed = self.pages.len() > 1;
        self.pages.drain(1..);
        self.page_titles.truncate(self.pages.len());
        if changed {
            // A pop-to-root is a pop: it arrives from the same leading edge, over frames.
            self.slide.jump_to(-1.0);
            self.slide.set_target(0.0);
        }
        self.navigation_changed.emit(NavigationEvent::PoppedToRoot);
        self.base.request_redraw();
    }

    /// Returns the writing direction the back affordance is placed by.
    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    /// Sets the writing direction, which mirrors the back affordance and its hit region.
    pub fn set_direction(&mut self, direction: TextDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
    }

    /// The back affordance's rectangle: the leading end of the navigation bar.
    fn back_rect(&self) -> Rect {
        let bar = self.nav_bar_rect();
        let x = if self.direction.is_right_to_left() { bar.right() - 60 } else { bar.x };
        Rect::new(x, bar.y, 60.min(bar.width), NAV_BAR_HEIGHT)
    }

    /// Returns the current navigation bar title.
    pub fn navigation_bar_title(&self) -> &str {
        &self.navigation_bar_title
    }

    /// Sets the navigation bar title.
    pub fn set_navigation_bar_title(&mut self, title: &str) {
        if self.navigation_bar_title == title {
            return;
        }
        self.navigation_bar_title = title.to_string();
        self.base.request_redraw();
    }

    /// Sets the title of the page at `index`.
    ///
    /// A per-page title is the only thing that makes a navigation bar describe the page it sits
    /// on: the single global title stayed on screen while the stack moved beneath it. A page
    /// without its own title keeps falling back to the global one.
    pub fn set_page_title(&mut self, index: usize, title: &str) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        if self.page_titles.len() <= index {
            self.page_titles.resize(index + 1, None);
        }
        self.page_titles[index] = Some(title.to_string());
        self.base.request_redraw();
        true
    }

    /// The title set for the page at `index`, when it has its own.
    pub fn page_title(&self, index: usize) -> Option<&str> {
        self.page_titles.get(index).and_then(|title| title.as_deref())
    }

    /// Replaces the trailing action labels.
    ///
    /// The labels use the same contract as `AppBar::set_actions`, so a host configures both bars
    /// the same way; the crate has no `AppBarAction` type, so a label slice is the family's
    /// existing vocabulary.
    pub fn set_actions(&mut self, actions: Vec<String>) {
        if self.actions == actions {
            return;
        }
        self.actions = actions;
        self.base.request_redraw();
    }

    /// The trailing action labels.
    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    /// Whether the back affordance is shown when the stack can pop.
    pub fn back_visible(&self) -> bool {
        self.back_visible
    }

    /// Shows or hides the back affordance.
    pub fn set_back_visible(&mut self, visible: bool) {
        if self.back_visible == visible {
            return;
        }
        self.back_visible = visible;
        self.base.request_redraw();
    }

    /// The trailing action boxes, ordered from the bar's trailing edge inward.
    ///
    /// Both the paint path and the press arm read this one derivation, so a label cannot be
    /// painted where it cannot be tapped.
    fn action_boxes(&self) -> Vec<(usize, Rect)> {
        let bar = self.nav_bar_rect();
        if self.actions.is_empty() || bar.width == 0 {
            return Vec::new();
        }
        let mut right = bar.right() - NAV_ACTION_MARGIN;
        let mut boxes = Vec::with_capacity(self.actions.len());
        for index in 0..self.actions.len() {
            let left = (right - NAV_ACTION_WIDTH as i32).max(bar.x);
            let width = (right - left).max(0) as u32;
            if width == 0 {
                break;
            }
            boxes.push((index, Rect::new(left, bar.y, width, bar.height)));
            right = left - 2;
        }
        boxes
    }

    /// The action index whose box covers `pos`.
    fn action_at(&self, pos: crate::core::Point) -> Option<usize> {
        self.action_boxes()
            .into_iter()
            .find(|(_, rect)| rect.contains_point(pos))
            .map(|(index, _)| index)
    }

    /// Returns the content area rect (below the navigation bar).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x,
            rect.y + NAV_BAR_HEIGHT as i32,
            rect.width,
            rect.height.saturating_sub(NAV_BAR_HEIGHT),
        )
    }

    /// Returns the navigation bar rect.
    fn nav_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, NAV_BAR_HEIGHT.min(rect.height))
    }

    /// Returns the title to display in the navigation bar.
    fn display_title(&self) -> String {
        if let Some(title) = self
            .current_page_index()
            .and_then(|index| self.page_title(index))
            .filter(|title| !title.is_empty())
        {
            return title.to_string();
        }
        if !self.navigation_bar_title.is_empty() {
            self.navigation_bar_title.clone()
        } else if let Some(page) = self.current_page() {
            format!("{:?}", page.kind())
        } else {
            "Navigation".to_string()
        }
    }
}

impl Widget for NavigationStack {
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
        crate::core::Size::new(400, 600)
    }

    /// Advances the content region's push/pop slide; `true` while it still owes frames.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.slide.tick(delta_ms)
    }

    fn is_animating(&self) -> bool {
        self.slide.is_moving()
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
            "navigation_changed" => {
                Some(EventSignalRef::mapped("navigation_changed", &self.navigation_changed, |v| {
                    CapabilityValue::String(format!("{v:?}"))
                }))
            }
            "action_index_pressed" => Some(EventSignalRef::mapped(
                "action_index_pressed",
                &self.action_index_pressed,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            _ => None,
        }
    }
}

/// `NavigationStack`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `page_count` is derived from
/// the page stack, so it is readable but read-only.
impl WidgetProperties for NavigationStack {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "page_count" => Ok(CapabilityValue::UInt(self.page_count() as u64)),
            "current_page" => match self.current_page_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "back_visible" => Ok(CapabilityValue::Bool(self.back_visible())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The stack has no random-access setter: pages move only through
            // `push` / `pop`, so an out-of-range index is refused rather than
            // silently ignored, and a valid one is reached by popping down to it.
            "current_page" => match value {
                // `Null` is the getter's "no page" answer and means an empty stack —
                // the only state in which this model reports no current page. Every
                // page still leaves through `pop`, the single mutation path.
                CapabilityValue::Null => {
                    while self.page_count() > 0 {
                        self.pop();
                    }
                    Ok(())
                }
                other => {
                    let target = expect_usize(other)?;
                    if target >= self.page_count() {
                        return Err(CapabilityAccessError::UnsupportedOnWidget);
                    }
                    while self.page_count() > target + 1 {
                        self.pop();
                    }
                    Ok(())
                }
            },
            "page_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "back_visible" => {
                let visible = match value {
                    CapabilityValue::Bool(visible) => visible,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_back_visible(visible);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["page_count", "current_page", "back_visible", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `navigation_stack` publishes.
    ///
    /// A page can only come from the caller, so `push` needs one and is refused as
    /// [`CapabilityAccessError::OutOfRange`], as does `set_current_page`, which names
    /// the page to land on.
    ///
    /// # Why an empty stack is `OutOfRange`
    ///
    /// `pop` has nothing to remove at the root, and the honest report for that is
    /// `OutOfRange` rather than `Ok(())`: the trait's documented meaning of success is
    /// "the command ran", and a bare `pop` on an empty stack did not run. It is also
    /// the answer the rest of the crate gives to the same shape — `ListBox`'s
    /// index-addressed commands and `GanttWidget::select_task` all report a
    /// missing/unavailable argument this way — so a caller does not have to learn a
    /// second convention for page stacks.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "pop" => {
                if self.pop().is_some() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "push" => Err(CapabilityAccessError::OutOfRange),
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for NavigationStack {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then a literal. The theme step is what makes an appearance
        // switch visible; the bar fill, the title colour and the content fill used to be
        // hardcoded literals, so light and dark rendered identically.
        //
        // The theme reads take and release the global manager's lock internally, so no guard
        // is held across the draw (the mutex is not re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("navigation_stack");
        // `navigation_stack` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A bar painted in that colour would be byte-identical to the frame behind
        // it, so a resolved surface equal to the window fill is re-derived a visible step
        // away from it, the same distinction `Colors::input_background` draws for a field.
        let palette = crate::style::palette_shared();
        let window_fill = palette.as_ref().map(|colors| colors.background).unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the back affordance follows the appearance rather than staying a literal blue. One
        // palette read serves both this and the window fill above (G-A01).
        let accent = palette.as_ref().map(|colors| colors.primary).unwrap_or(Color::PRIMARY);
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.08),
        };
        let separator = window_fill.blend(&ink, 0.2);
        // The content sits on the window's own surface, one step *under* the bar, so the two
        // regions read as separate in either appearance.
        let content_surface = window_fill;

        // ── Draw Navigation Bar ──
        let nav_rect = self.nav_bar_rect();
        // Nav bar background
        context.fill_rect(nav_rect, surface);
        // Nav bar bottom border
        context.draw_line(
            Point::new(nav_rect.x, nav_rect.y + nav_rect.height as i32 - 1),
            Point::new(nav_rect.x + nav_rect.width as i32, nav_rect.y + nav_rect.height as i32 - 1),
            separator,
        );

        // Back button (if can_pop and the host still wants it)
        if self.can_pop() && self.back_visible {
            let back = self.back_rect();
            // The **effective font** — the resolved theme/caller font — for the title below, and
            // for the icon's line box. The fixed 13/15 px sans-serif faces this used to build are
            // gone (D09-STYLE-01).
            let back_font = effective_font(style);
            let back_color = self.base.disabled_ink_on(accent, surface);
            let back_line = context.text_line(back, back_font);
            // The back affordance is the same `ArrowLeft` outline the app bar paints, so one
            // return glyph exists in the family rather than a text label here and an icon there
            // (N-09). The box follows the line height and is inset from the leading edge; in RTL
            // the `back_rect` already starts at the trailing edge, so the box is mirrored inside
            // it.
            let side = back_line.height.max(1);
            let icon_x = if self.direction.is_right_to_left() {
                back.right() - 8 - side as i32
            } else {
                back.x + 8
            };
            let icon_rect = Rect::new(icon_x, back_line.y, side, side);
            crate::widget::draw_icon_at(context, icon_rect, back_color, IconName::ArrowLeft);
        }

        // Title
        let title_font = effective_font(style);
        let title = self.display_title();
        let text_color = self.base.disabled_ink_on(ink, surface);
        let metrics =
            crate::widget::base_widgets::measure_cache::measure_text(context, &title, title_font);
        // The title is centred in the space the trailing actions leave, so a label cannot paint
        // over an action on a narrow bar.
        let actions_reserve = self.actions.len() as i32 * (NAV_ACTION_WIDTH as i32 + 2);
        let title_centre = nav_rect.x + (nav_rect.width as i32 - actions_reserve) / 2;
        let title_x = title_centre - metrics.width as i32 / 2;
        let title_line = context.text_line(nav_rect, title_font);
        context.draw_text_fitted_cow(
            Rect::new(
                title_x.max(nav_rect.x + 4),
                title_line.y,
                (nav_rect.width as i32 - actions_reserve - 8).max(1) as u32,
                title_line.height,
            ),
            &title,
            title_font,
            text_color,
            HorizontalAlignment::Center,
        );

        // ── Trailing actions ──
        //
        // The boxes come from `action_boxes`, the same derivation `action_at` hit-tests, so a
        // button is tappable exactly where it is painted.
        for (index, box_rect) in self.action_boxes() {
            let Some(label) = self.actions.get(index) else { continue };
            context.draw_text_fitted_cow(
                context.text_line(box_rect, title_font),
                label,
                title_font,
                accent,
                HorizontalAlignment::Center,
            );
        }

        // ── Draw content area background and pages ──
        //
        // # Why the content slides rather than swapping
        //
        // A push or pop changed the page in one frame: `pages.push`/`pages.pop` then a redraw, with
        // nothing in between. On a control whose entire purpose is *spatial* — the stack is a
        // history, and the back affordance means "the page to the left" — a hard swap gives the user
        // no way to see which direction they moved. The content region is therefore drawn slid
        // toward the edge the transition came from, and the pages themselves move with it.
        let content = self.content_rect();
        if content.width > 0 && content.height > 0 {
            // `slide` runs from `-1` (a pop, arriving from the left) through `0` (settled) to `1`
            // (a push, arriving from the right). Multiplying by the content width gives the offset,
            // so the movement is proportional to the region rather than a fixed number of pixels.
            let slide = self.slide.value();
            let offset = (slide * content.width as f32) as i32;
            context.push_offset(offset, 0);
            context.fill_rect(content, content_surface);
            context.pop_offset();

            let page_count = self.pages.len();
            if page_count > 0 {
                // During a transition the outgoing page travels at 30% of the incoming page's
                // distance, in the opposite direction. That parallax is what reads as two screens
                // rather than one screen sliding over a blank region.
                if slide != 0.0 && page_count > 1 {
                    let previous_offset = -(((slide * content.width as f32) * 0.3) as i32);
                    let previous = &mut self.pages[page_count - 2];
                    // The page is painted into its own frame, so its geometry is the content
                    // rect; the transition is expressed entirely by the context offset.
                    previous.set_geometry(content);
                    context.push_offset(previous_offset, 0);
                    if let Some(drawable) = previous.as_draw_mut() {
                        drawable.draw(context);
                    }
                    context.pop_offset();
                }

                let current = &mut self.pages[page_count - 1];
                current.set_geometry(content);
                context.push_offset(offset, 0);
                if let Some(drawable) = current.as_draw_mut() {
                    drawable.draw(context);
                }
                context.pop_offset();
            }
        }
    }
}

impl EventHandler for NavigationStack {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
                if *button == 1 {
                    // A trailing action is an affordance of this bar, so it is checked before
                    // forwarding anything to a page beneath it.
                    if let Some(index) = self.action_at(*pos) {
                        self.action_index_pressed.emit(index);
                        return;
                    }
                    // Check if the back button was clicked
                    if self.can_pop() && self.back_visible {
                        let back_rect = self.back_rect();
                        if back_rect.contains_point(*pos) {
                            self.pop();
                            return;
                        }
                    }

                    // An edge drag starts an interactive pop: pressing within the leading
                    // strip of the content region and dragging inward follows the finger.
                    let content = self.content_rect();
                    let edge = if self.direction.is_right_to_left() {
                        Rect::new(
                            content.right() - 24,
                            content.y,
                            24.min(content.width),
                            content.height,
                        )
                    } else {
                        Rect::new(content.x, content.y, 24.min(content.width), content.height)
                    };
                    if self.can_pop() && self.back_visible && edge.contains_point(*pos) {
                        // An edge drag always begins at the fully-popped end and follows the
                        // pointer toward the settled end; it never starts from a half-settled
                        // transition, which belongs to a previous gesture.
                        self.edge_drag = Some((pos.x as f32, -1.0));
                        return;
                    }

                    // Forward to current page
                    if content.contains_point(*pos) {
                        if let Some(page) = self.pages.last_mut() {
                            // The page is painted inside the content rect, so it must be
                            // hit-tested there too; otherwise it answers at a stale origin.
                            page.set_geometry(content);
                            page.handle_event(event);
                        }
                    }
                }
            }
            Event::MouseMove { pos } => {
                if let Some((start, start_slide)) = self.edge_drag {
                    let content = self.content_rect();
                    let width = content.width.max(1) as f32;
                    let dx = if self.direction.is_right_to_left() {
                        start - pos.x as f32
                    } else {
                        pos.x as f32 - start
                    };
                    // A leftward (for LTR) drag here would undo a pop, so the follow is clamped
                    // to the pop direction only.
                    let progress = (start_slide + dx / width).clamp(-1.0, 0.0);
                    self.slide.jump_to(progress);
                    self.base.request_redraw();
                    return;
                }
                let content = self.content_rect();
                if content.contains_point(*pos) {
                    if let Some(page) = self.pages.last_mut() {
                        page.set_geometry(content);
                        page.handle_event(event);
                    }
                }
            }
            Event::MouseRelease { pos, button } => {
                if *button != 1 {
                    return;
                }
                if let Some((_, start_slide)) = self.edge_drag.take() {
                    let progress = self.slide.value();
                    // One third of the content width is the iOS commit point, expressed as a
                    // fraction of the slide so a relayout moves the threshold with the region.
                    if progress <= -1.0 / 3.0 {
                        if self.pop().is_some() {
                            self.slide.jump_to(progress);
                            self.slide.set_target(0.0);
                        }
                    } else {
                        self.slide.jump_to(progress);
                        self.slide.set_target(start_slide.clamp(-1.0, 0.0));
                    }
                    self.base.request_redraw();
                    return;
                }
                let content = self.content_rect();
                if content.contains_point(*pos) {
                    if let Some(page) = self.pages.last_mut() {
                        page.set_geometry(content);
                        page.handle_event(event);
                    }
                }
            }
            _ => {
                let content = self.content_rect();
                if let Some(page) = self.pages.last_mut() {
                    page.set_geometry(content);
                    page.handle_event(event);
                }
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Font, Size};
    use crate::render::{
        PaintBackend, RenderCommand, ShapedText, SoftwarePaintBackend, SoftwareRenderConfig,
        TextMetrics,
    };
    use crate::widget::display_widgets::badge::Badge;
    use std::sync::{Arc, Mutex};

    /// A page whose paint is a single flat rectangle, so a recorded command identifies it
    /// without depending on any other widget's rendering.
    struct TestPage {
        base: BaseWidget,
        color: Color,
        event_geometries: Arc<Mutex<Vec<Rect>>>,
    }

    impl TestPage {
        fn new(color: Color) -> Self {
            Self {
                base: BaseWidget::new(WidgetKind::Label, Rect::new(0, 0, 1, 1), "TestPage"),
                color,
                event_geometries: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn geometries_sink(&self) -> Arc<Mutex<Vec<Rect>>> {
            Arc::clone(&self.event_geometries)
        }
    }

    impl Widget for TestPage {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        impl_draw_bridge!();
    }

    impl Draw for TestPage {
        fn draw(&mut self, context: &mut RenderContext) {
            context.fill_rect(self.geometry(), self.color);
        }
    }

    impl EventHandler for TestPage {
        /// Records the geometry the event arrived with, which is what "hit-testing matches
        /// paint" means for a page: the rectangle it was told it occupies.
        fn handle_event(&mut self, _event: &Event) {
            if let Ok(mut seen) = self.event_geometries.lock() {
                seen.push(self.geometry());
            }
        }
    }

    /// A backend that records every command in draw order.
    struct Recorder {
        commands: Vec<RenderCommand>,
        inner: SoftwarePaintBackend,
    }

    impl Recorder {
        fn new(size: Size) -> Self {
            Self { commands: Vec::new(), inner: SoftwarePaintBackend::new(size, 1.0) }
        }
    }

    impl PaintBackend for Recorder {
        fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }
        fn end_frame(&mut self) {
            self.inner.end_frame();
        }
        fn execute_command(&mut self, command: &RenderCommand) {
            self.commands.push(command.clone());
            self.inner.execute_command(command);
        }
        fn size(&self) -> Size {
            self.inner.size()
        }
        fn set_size(&mut self, size: Size) {
            self.inner.set_size(size);
        }
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, scale: f32) {
            self.inner.set_dpi_scale(scale);
        }
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            self.inner.measure_text(text, font)
        }
        fn shape_text(&self, text: &str, font: &Font) -> crate::compat::Rc<ShapedText> {
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
        fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
            self.inner.apply_render_config(config);
        }
    }

    /// Draws `stack` once and returns every flat fill it emitted, in paint order.
    fn recorded_fills(stack: &mut NavigationStack) -> Vec<(Rect, Color)> {
        let mut backend = Recorder::new(Size::new(400, 600));
        {
            let mut context = RenderContext::new(&mut backend);
            stack.draw(&mut context);
        }
        backend
            .commands
            .iter()
            .filter_map(|command| match command {
                RenderCommand::FillRect { rect, color } => Some((*rect, *color)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn navigation_stack_default_creation() {
        let stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert_eq!(stack.kind(), WidgetKind::NavigationStack);
        assert_eq!(stack.page_count(), 0);
        assert!(!stack.can_pop());
        assert!(stack.current_page().is_none());
    }

    #[test]
    fn navigation_stack_push_and_pop() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop()); // Only one page, can't pop

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 2);
        assert!(stack.can_pop());

        let popped = stack.pop();
        assert!(popped.is_some());
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop());
    }

    #[test]
    fn navigation_stack_pop_to_root() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 3);

        stack.pop_to_root();
        assert_eq!(stack.page_count(), 1);
        assert!(!stack.can_pop());
    }

    #[test]
    fn navigation_stack_navigation_changed_signal() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        let events = Arc::new(Mutex::new(Vec::new()));

        stack.navigation_changed.connect({
            let events = Arc::clone(&events);
            move |event: Arc<NavigationEvent>| {
                events.lock().unwrap().push(event.as_ref().clone());
            }
        });

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.pop();
        stack.pop_to_root();

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 4);
        assert_eq!(captured[0], NavigationEvent::Pushed);
        assert_eq!(captured[1], NavigationEvent::Pushed);
        assert_eq!(captured[2], NavigationEvent::Popped);
        assert_eq!(captured[3], NavigationEvent::PoppedToRoot);
    }

    #[test]
    fn navigation_stack_current_page() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert!(stack.current_page().is_none());

        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert!(stack.current_page().is_some());
    }

    #[test]
    fn navigation_stack_back_button_click() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        stack.push(Box::new(Badge::new(Rect::new(0, 44, 100, 24))));
        assert_eq!(stack.page_count(), 2);

        // Click on back button area (left side of nav bar)
        stack.handle_event(&Event::mouse_press(5, 10, 1));
        assert_eq!(stack.page_count(), 1);
    }

    #[test]
    fn navigation_stack_set_title() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        assert_eq!(stack.navigation_bar_title(), "");

        stack.set_navigation_bar_title("Settings");
        assert_eq!(stack.navigation_bar_title(), "Settings");
    }

    /// The stack used to translate a background rectangle and never call `draw` on a page,
    /// so a mounted page was invisible no matter what it painted.
    #[test]
    fn navigation_stack_draws_the_current_page() {
        let _guard = crate::style::theme_test_guard();
        let page_color = Color::rgba(10, 20, 30, 255);
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(page_color)));

        let content = Rect::new(0, 44, 400, 556);
        let fills = recorded_fills(&mut stack);
        assert!(
            fills.iter().any(|(_, color)| *color == page_color),
            "the pushed page's own paint commands must reach the frame"
        );
        assert_eq!(
            stack.current_page().expect("one pushed page").geometry(),
            content,
            "the current page is laid out in the content rect before it paints"
        );
    }

    /// A push leaves the slide at `1.0`, so the first frame of the transition carries the
    /// incoming page at 100% and the page beneath it at 30% in the opposite direction.
    #[test]
    fn navigation_stack_transition_draws_both_pages_with_parallax() {
        let _guard = crate::style::theme_test_guard();
        let root_color = Color::rgba(40, 50, 60, 255);
        let top_color = Color::rgba(70, 80, 90, 255);
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(root_color)));
        stack.push(Box::new(TestPage::new(top_color)));

        let content = Rect::new(0, 44, 400, 556);
        let fills = recorded_fills(&mut stack);
        let root = fills
            .iter()
            .find(|(_, color)| *color == root_color)
            .expect("the outgoing page stays on screen during a push");
        let top = fills
            .iter()
            .find(|(_, color)| *color == top_color)
            .expect("the incoming page is painted during a push");

        let parallax = (content.width as f32 * 0.3) as i32;
        assert_eq!(root.0.x, content.x - parallax, "the lower page trails at 30%");
        assert_eq!(top.0.x, content.x + content.width as i32, "the top page starts 100% out");
    }

    /// A pop mirrors the push: the remaining top page arrives from the left at 100% while
    /// the page beneath it trails to the right at 30%.
    #[test]
    fn navigation_stack_pop_transition_draws_both_remaining_pages() {
        let _guard = crate::style::theme_test_guard();
        let lower_color = Color::rgba(21, 42, 63, 255);
        let top_color = Color::rgba(84, 95, 106, 255);
        let popped_color = Color::rgba(117, 128, 139, 255);
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(lower_color)));
        stack.push(Box::new(TestPage::new(top_color)));
        stack.push(Box::new(TestPage::new(popped_color)));

        stack.pop();
        let content = Rect::new(0, 44, 400, 556);
        let fills = recorded_fills(&mut stack);
        let lower = fills
            .iter()
            .find(|(_, color)| *color == lower_color)
            .expect("the page beneath the top one is painted during a pop");
        let top = fills
            .iter()
            .find(|(_, color)| *color == top_color)
            .expect("the remaining top page is painted during a pop");

        let parallax = (content.width as f32 * 0.3) as i32;
        assert_eq!(lower.0.x, content.x + parallax, "the lower page trails at 30%");
        assert_eq!(top.0.x, content.x - content.width as i32, "the top page starts 100% out");
        assert!(
            !fills.iter().any(|(_, color)| *color == popped_color),
            "the popped page is gone from the stack and from the frame"
        );
    }

    #[test]
    fn navigation_stack_pop_to_root_draws_only_the_root() {
        let _guard = crate::style::theme_test_guard();
        let root_color = Color::rgba(11, 22, 33, 255);
        let leaf_color = Color::rgba(44, 55, 66, 255);
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(root_color)));
        stack.push(Box::new(TestPage::new(leaf_color)));

        stack.pop_to_root();
        assert_eq!(stack.page_count(), 1);

        let fills = recorded_fills(&mut stack);
        assert!(
            fills.iter().any(|(_, color)| *color == root_color),
            "the root page is still painted"
        );
        assert!(
            !fills.iter().any(|(_, color)| *color == leaf_color),
            "popped pages must not paint"
        );
    }

    /// Forwarded events carry the content-rect geometry, so a page hit-tests where it painted.
    #[test]
    fn navigation_stack_forwards_events_with_the_content_geometry() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        let page = TestPage::new(Color::rgba(90, 90, 90, 255));
        let seen = page.geometries_sink();
        stack.push(Box::new(page));

        stack.handle_event(&Event::MousePress { pos: Point::new(20, 60), button: 1, modifiers: 0 });

        let content = Rect::new(0, 44, 400, 556);
        assert_eq!(
            *seen.lock().expect("geometry sink"),
            vec![content],
            "the page must be told the same rect it is painted in"
        );
    }

    #[test]
    fn pop_to_root_plays_the_same_transition_as_a_single_pop() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        while stack.tick(16) {}
        stack.pop_to_root();
        assert_eq!(stack.page_count(), 1);
        assert!(stack.is_animating(), "returning to the root is a movement, not a swap");
    }

    #[test]
    fn a_right_to_left_bar_puts_the_back_affordance_at_the_trailing_edge() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        let ltr = stack.back_rect();
        stack.set_direction(TextDirection::RightToLeft);
        let rtl = stack.back_rect();
        assert!(rtl.x > ltr.x, "the mirrored back zone moves to the other edge");
    }

    #[test]
    fn an_edge_drag_pops_the_page_it_follows() {
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.handle_event(&Event::MousePress { pos: Point::new(4, 300), button: 1, modifiers: 0 });
        stack.handle_event(&Event::MouseMove { pos: Point::new(200, 300) });
        stack.handle_event(&Event::MouseRelease { pos: Point::new(200, 300), button: 1 });
        assert_eq!(stack.page_count(), 1, "a long edge drag commits the pop");
    }

    #[test]
    fn the_back_affordance_is_an_icon_not_a_text_label() {
        let _guard = crate::style::theme_test_guard();
        let mut stack = NavigationStack::new(Rect::new(0, 0, 400, 600));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        stack.push(Box::new(TestPage::new(Color::BLACK)));
        while stack.tick(16) {}
        let svg = crate::widget::svg::render_to_svg(&mut stack);
        assert!(
            !svg.contains("Back"),
            "the back affordance is the shared ArrowLeft outline, not a text label"
        );
    }
}
