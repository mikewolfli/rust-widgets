// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `CupertinoNavigationStack` — the title stack that pairs with a Cupertino navigation bar.
//!
//! The bar publishes `back_pressed` and an edge-swipe as bare signals, so keeping the title,
//! the back button and the page sequence in sync is left to the host. This companion owns that
//! bookkeeping: [`Self::push`] / [`Self::pop`] update the title and the back affordance, and
//! `popped` announces the title that was left (COMP-L07).
//!
//! The crate already has [`crate::widget::nav_widgets::navigation_stack::NavigationStack`], which
//! hosts page *widgets* and draws the outgoing/incoming pages. This type is the Cupertino
//! companion: it carries titles only, because the bar it pairs with renders the transition itself.
//!
//! Registration surface (out of scope for this file): canonical name `navigation_stack`, kind
//! [`WidgetKind::NavigationStack`].

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::cupertino::CupertinoNavigationBar;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A title stack bound to a Cupertino navigation bar.
pub struct CupertinoNavigationStack {
    base: BaseWidget,
    /// Page titles, root first. Always has at least one entry.
    titles: Vec<String>,
    animated: bool,
    /// Emitted with the title that was popped.
    pub popped: Signal1<String>,
    /// Emitted with the new depth after a push or pop.
    pub stack_changed: Signal1<usize>,
}

impl CupertinoNavigationStack {
    /// Creates a stack with `root` as its only page.
    pub fn new(geometry: Rect, root: impl Into<String>) -> Self {
        Self {
            base: BaseWidget::new(
                WidgetKind::NavigationStack,
                geometry,
                "CupertinoNavigationStack",
            ),
            titles: vec![root.into()],
            animated: true,
            popped: Signal1::new(),
            stack_changed: Signal1::new(),
        }
    }

    /// The number of pages on the stack, including the root.
    pub fn depth(&self) -> usize {
        self.titles.len()
    }

    /// The current (top) page's title.
    pub fn current_title(&self) -> &str {
        self.titles.last().map(String::as_str).unwrap_or("")
    }

    /// Whether pushes and pops animate.
    pub fn animated(&self) -> bool {
        self.animated
    }

    /// Turns transition animation on or off.
    pub fn set_animated(&mut self, animated: bool) {
        self.animated = animated;
    }

    /// The titles from the root to the top.
    pub fn titles(&self) -> &[String] {
        &self.titles
    }

    /// Pushes a new page title onto the stack.
    pub fn push(&mut self, title: &str) {
        self.titles.push(title.to_string());
        self.base.request_redraw();
        self.stack_changed.emit(self.titles.len());
    }

    /// Pops the top page, returning whether anything was removed.
    ///
    /// The root page cannot be popped: a navigation stack with no page has no title and no
    /// meaningful back state, so the call is a no-op at depth 1.
    pub fn pop(&mut self) -> bool {
        if self.titles.len() <= 1 {
            return false;
        }
        let Some(title) = self.titles.pop() else {
            return false;
        };
        self.base.request_redraw();
        self.popped.emit(title);
        self.stack_changed.emit(self.titles.len());
        true
    }

    /// Pops every page above the root.
    pub fn pop_to_root(&mut self) {
        if self.titles.len() <= 1 {
            return;
        }
        let top = self.titles.last().cloned().unwrap_or_default();
        self.titles.truncate(1);
        self.base.request_redraw();
        self.popped.emit(top);
        self.stack_changed.emit(self.titles.len());
    }

    /// Replaces the root title without touching the rest of the stack.
    pub fn set_root_title(&mut self, title: &str) {
        if let Some(root) = self.titles.first_mut() {
            *root = title.to_string();
            self.base.request_redraw();
        }
    }

    /// Runs the stack's back action, which is what a bar's `back_pressed` should call.
    pub fn handle_back(&mut self) -> bool {
        self.pop()
    }

    /// Synchronises a navigation bar with the stack: its title, its back affordance and the
    /// back caption (which names the page the user returns to).
    pub fn bind_navigation_bar(&mut self, bar: &mut CupertinoNavigationBar) {
        bar.set_title(self.current_title());
        bar.show_back_button(self.depth() > 1);
        if let Some(parent) = self.titles.iter().rev().nth(1) {
            bar.set_back_button_text(parent);
        }
        self.base.request_redraw();
    }
}

impl Widget for CupertinoNavigationStack {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(160, 24)
    }
    impl_draw_bridge!();
}

impl EventHandler for CupertinoNavigationStack {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // A click on the stack's own indicator area is its back action; the host's bar wires
        // its `back_pressed` to `handle_back` for the normal path.
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.geometry().contains_point(*pos) {
                self.handle_back();
            }
        }
    }
}

impl Draw for CupertinoNavigationStack {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(30, 30, 30, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(245, 245, 245, 255));
        let font = &role_font(style, 13.0);
        context.fill_rect(rect, surface);
        // One tick per page, with the top page's title beside the stack.
        let dot = 5i32;
        let mut x = rect.x + 4;
        for index in 0..self.titles.len() {
            let active = index + 1 == self.titles.len();
            context.fill_circle(
                crate::core::Point::new(x, rect.y + rect.height as i32 / 2),
                dot as u32,
                if active { ink } else { ink.with_alpha(90) },
            );
            x += dot * 3;
        }
        context.draw_text_fitted_cow(
            Rect::new(
                x + 4,
                rect.y + 2,
                rect.right().saturating_sub(x + 4) as u32,
                rect.height.saturating_sub(4),
            ),
            self.current_title(),
            font,
            ink,
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_and_root_guard() {
        let mut stack = CupertinoNavigationStack::new(Rect::new(0, 0, 160, 24), "Home");
        assert_eq!(stack.depth(), 1);
        assert!(!stack.pop(), "the root cannot be popped");
        stack.push("Detail");
        assert_eq!(stack.current_title(), "Detail");
        let popped = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = popped.clone();
        stack.popped.connect(move |title: std::sync::Arc<String>| {
            *sink.lock().unwrap() = (*title).clone();
        });
        assert!(stack.pop());
        assert_eq!(*popped.lock().unwrap(), "Detail");
        assert_eq!(stack.current_title(), "Home");
    }
}
