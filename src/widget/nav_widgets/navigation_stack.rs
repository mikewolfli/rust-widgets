// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! NavigationStack widget — a push/pop page navigation container.
//!
//! The NavigationStack widget manages a stack of pages (widgets) and displays the
//! topmost page along with a navigation bar. It supports push, pop, and pop-to-root
//! operations, similar to SwiftUI NavigationStack or UINavigationController.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    /// See `draw` for why this is the content *frame* and not the page: the page's own widgets are
    /// laid out by the host, so this control animates the region they sit in.
    slide: PropertyDriver,
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
            navigation_changed: Signal1::new(),
        }
    }

    /// Pushes a new page onto the navigation stack.
    /// The new page becomes the visible topmost page.
    pub fn push(&mut self, page: Box<dyn Widget>) {
        self.pages.push(page);
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
        self.pages.drain(1..);
        self.navigation_changed.emit(NavigationEvent::PoppedToRoot);
        self.base.request_redraw();
    }

    /// Returns the current navigation bar title.
    pub fn navigation_bar_title(&self) -> &str {
        &self.navigation_bar_title
    }

    /// Sets the navigation bar title.
    pub fn set_navigation_bar_title(&mut self, title: &str) {
        self.navigation_bar_title = title.to_string();
        self.base.request_redraw();
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
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The stack has no random-access setter: pages move only through
            // `push` / `pop`, so an out-of-range index is refused rather than
            // silently ignored, and a valid one is reached by popping down to it.
            "current_page" => {
                let target = expect_usize(value)?;
                if target >= self.page_count() {
                    return Err(CapabilityAccessError::UnsupportedOnWidget);
                }
                while self.page_count() > target + 1 {
                    self.pop();
                }
                Ok(())
            }
            "page_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["page_count", "current_page", BASE_PROPERTY_NAMES]
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
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("navigation_stack");
        // `navigation_stack` is absent from `WidgetRole::for_kind_name`'s table, so it
        // classifies as `Surface` and resolves to `theme.colors.background` — the window's
        // own fill. A bar painted in that colour would be byte-identical to the frame behind
        // it, so a resolved surface equal to the window fill is re-derived a visible step
        // away from it, the same distinction `Colors::input_background` draws for a field.
        let window_fill = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.background)
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The accent is the theme's `primary`: the hue a theme is expected to vary most, so
        // the back affordance follows the appearance rather than staying a literal blue.
        let accent = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::PRIMARY);
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

        // Back button (if can_pop)
        if self.can_pop() {
            let back_text = "< Back";
            let back_font = Font::simple("sans-serif", 13.0);
            let back_color = self.base.disabled_ink_on(accent, surface);
            // The line box is derived from the *font in use*, not from a literal. `+ 14` was shared
            // by this 13 pt label and the 15 pt title below, so it was the correct descent for
            // neither: a glyph origin is the top edge of its box, so an offset that does not come
            // from the font puts one of the two rows off its own middle line. `text_line` is the
            // crate's one derivation for "where does this font's line sit in this band".
            let back_line = context.text_line(nav_rect, &back_font);
            context.draw_text(
                Point::new(nav_rect.x + 8, back_line.y),
                back_text,
                &back_font,
                back_color,
                HorizontalAlignment::Left,
            );
        }

        // Title
        let title_font = Font::simple("sans-serif", 15.0);
        let title = self.display_title();
        let text_color = self.base.disabled_ink_on(ink, surface);
        let metrics = context.measure_text(&title, &title_font);
        let title_x = nav_rect.x + (nav_rect.width as i32 - metrics.width as i32) / 2;
        let title_line = context.text_line(nav_rect, &title_font);
        context.draw_text(
            Point::new(title_x.max(nav_rect.x + 4), title_line.y),
            &title,
            &title_font,
            text_color,
            HorizontalAlignment::Left,
        );

        // ── Draw content area background ──
        //
        // # Why the content slides rather than swapping
        //
        // A push or pop changed the page in one frame: `pages.push`/`pages.pop` then a redraw, with
        // nothing in between. On a control whose entire purpose is *spatial* — the stack is a
        // history, and the back affordance means "the page to the left" — a hard swap gives the user
        // no way to see which direction they moved. The content region is therefore drawn slid
        // toward the edge the transition came from, which is the one thing `draw` can honestly
        // animate: the page's own contents are composed by the host's layout, so this control moves
        // the *frame* they sit in and the host moves the contents with it.
        let content = self.content_rect();
        if content.width > 0 && content.height > 0 {
            // `slide` runs from `-1` (a pop, arriving from the left) through `0` (settled) to `1`
            // (a push, arriving from the right). Multiplying by the content width gives the offset,
            // so the movement is proportional to the region rather than a fixed number of pixels.
            let offset = (self.slide.value() * content.width as f32) as i32;
            context.push_offset(offset, 0);
            context.fill_rect(content, content_surface);
            context.pop_offset();
        }
    }
}

impl EventHandler for NavigationStack {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Check if the back button was clicked
                    if self.can_pop() {
                        let nav_rect = self.nav_bar_rect();
                        let back_rect = Rect::new(nav_rect.x, nav_rect.y, 60, NAV_BAR_HEIGHT);
                        if back_rect.contains_point(*pos) {
                            self.pop();
                            return;
                        }
                    }

                    // Forward to current page
                    let content = self.content_rect();
                    if content.contains_point(*pos) {
                        if let Some(page) = self.pages.last_mut() {
                            page.handle_event(event);
                        }
                    }
                }
            }
            Event::MouseRelease { pos, button: _ } | Event::MouseMove { pos } => {
                let content = self.content_rect();
                if content.contains_point(*pos) {
                    if let Some(page) = self.pages.last_mut() {
                        page.handle_event(event);
                    }
                }
            }
            _ => {
                if let Some(page) = self.pages.last_mut() {
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
    use crate::widget::display_widgets::badge::Badge;
    use std::sync::{Arc, Mutex};

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
}
