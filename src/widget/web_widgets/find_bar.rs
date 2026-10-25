// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `WebFindBar` — a find bar for a web view (COMP-L03).
//!
//! `WebEngineView` already exposes `find_text` / `find_next` / `find_previous`, but nothing
//! visible calls them. This companion is the visible entry point: query text, match counter, a
//! case-sensitivity toggle and close/next/previous actions, each as a request signal the host
//! wires to the view.
//!
//! This is the web family's own find bar, distinct from the markdown editor's
//! `special_widgets::markdown_editor::FindBar` (which searches an editor buffer, not a page).
//!
//! Registration surface (out of scope for this file): canonical name `find_bar`, kind
//! [`WidgetKind::FindBar`].

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// A web-page find bar.
pub struct WebFindBar {
    base: BaseWidget,
    query: String,
    match_index: usize,
    match_count: usize,
    case_sensitive: bool,
    visible: bool,
    /// Emitted with `(query, case_sensitive)` when the query should be run.
    pub search_requested: Signal1<(String, bool)>,
    /// Emitted when the next match is requested.
    pub next_requested: GenericSignal,
    /// Emitted when the previous match is requested.
    pub previous_requested: GenericSignal,
    /// Emitted when the bar closes.
    pub closed: GenericSignal,
}

impl WebFindBar {
    /// Creates a closed find bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FindBar, geometry, "WebFindBar"),
            query: String::new(),
            match_index: 0,
            match_count: 0,
            case_sensitive: false,
            visible: false,
            search_requested: Signal1::new(),
            next_requested: GenericSignal::new(),
            previous_requested: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    /// Opens the bar.
    pub fn open(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.base.request_redraw();
    }

    /// Closes the bar and tells the host.
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.base.request_redraw();
        self.closed.emit();
    }

    /// Whether the bar is open.
    pub fn is_open(&self) -> bool {
        self.visible
    }

    /// The current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the query and requests a search for it.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();
        if self.query == query {
            return;
        }
        self.query = query;
        self.base.request_redraw();
        self.search_requested.emit((self.query.clone(), self.case_sensitive));
    }

    /// The `(one-based current, total)` match progress reported by the host.
    pub fn match_progress(&self) -> (usize, usize) {
        (self.match_index, self.match_count)
    }

    /// Reports the match count and the selected index, one-based (`0` means none).
    pub fn set_match_progress(&mut self, index: usize, count: usize) {
        let index = index.min(count);
        if self.match_index == index && self.match_count == count {
            return;
        }
        self.match_index = index;
        self.match_count = count;
        self.base.request_redraw();
    }

    /// Whether matching is case sensitive.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Turns case sensitivity on or off and re-runs the current query.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive == case_sensitive {
            return;
        }
        self.case_sensitive = case_sensitive;
        self.base.request_redraw();
        if !self.query.is_empty() {
            self.search_requested.emit((self.query.clone(), self.case_sensitive));
        }
    }

    /// Requests the next match, wrapping at the end.
    pub fn next_match(&mut self) {
        self.next_requested.emit();
    }

    /// Requests the previous match, wrapping at the start.
    pub fn previous_match(&mut self) {
        self.previous_requested.emit();
    }
}

impl Widget for WebFindBar {
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
        Size::new(280, 32)
    }
    impl_draw_bridge!();
}

impl Draw for WebFindBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(250, 250, 250, 255));
        let font = &role_font(style, 12.0);
        let fill = if self.visible { surface } else { surface.with_alpha(60) };
        context.fill_rounded_rect(rect, 6, fill);
        context.draw_rounded_rect_stroke(rect, 6, ink.with_alpha(60), 1);
        if !self.visible {
            return;
        }
        // The query line, fitted to the box the action controls leave.
        let actions = 84u32;
        let text_width = rect.width.saturating_sub(actions + 12);
        let shown = if self.query.is_empty() { "Find in page" } else { &self.query };
        context.draw_text_fitted_cow(
            Rect::new(rect.x + 8, rect.y + 8, text_width, rect.height.saturating_sub(16)),
            shown,
            font,
            if self.query.is_empty() { ink.with_alpha(120) } else { ink },
            HorizontalAlignment::Left,
        );
        // Match counter and the action controls, laid out from the trailing edge: case toggle,
        // next, previous, then the counter.
        let right = rect.right();
        let counter = format!("{}/{}", self.match_index, self.match_count);
        context.draw_text_fitted_cow(
            Rect::new(right - 112, rect.y + 8, 36, rect.height.saturating_sub(16)),
            &counter,
            font,
            ink.with_alpha(160),
            HorizontalAlignment::Center,
        );
        for (x, icon) in [(right - 70, IconName::ChevronLeft), (right - 46, IconName::ChevronRight)]
        {
            crate::widget::draw_icon_at(
                context,
                Rect::new(x, rect.y + 8, 16, rect.height.saturating_sub(16)),
                ink,
                icon,
            );
        }
        context.draw_text_fitted_cow(
            Rect::new(right - 24, rect.y + 8, 16, rect.height.saturating_sub(16)),
            "Aa",
            font,
            if self.case_sensitive { ink } else { ink.with_alpha(90) },
            HorizontalAlignment::Center,
        );
    }
}

impl EventHandler for WebFindBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            // Escape closes the search UI, the standard browser behaviour.
            Event::KeyPress { key: 27, .. } => self.close(),
            // Enter advances; Shift+Enter goes back (the shift bit is bit 0 in this crate).
            Event::KeyPress { key: 13, modifiers } => {
                if *modifiers & 1 != 0 {
                    self.previous_match();
                } else {
                    self.next_match();
                }
            }
            Event::KeyPress { key: 8, .. } => {
                if self.query.pop().is_some() {
                    let query = self.query.clone();
                    self.base.request_redraw();
                    self.search_requested.emit((query, self.case_sensitive));
                }
            }
            Event::TextInput { text } => {
                self.query.push_str(text);
                let query = self.query.clone();
                self.base.request_redraw();
                self.search_requested.emit((query, self.case_sensitive));
            }
            Event::MousePress { pos, button: 1, .. } => {
                if !self.geometry().contains_point(*pos) {
                    return;
                }
                // The trailing controls mirror the painted icons: previous, next, case.
                let right = self.geometry().right();
                if pos.x >= right - 24 {
                    self.set_case_sensitive(!self.case_sensitive);
                } else if pos.x >= right - 48 {
                    self.next_match();
                } else if pos.x >= right - 72 {
                    self.previous_match();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_query_and_match_counter_are_bounded_by_the_reported_count() {
        let mut bar = WebFindBar::new(Rect::new(0, 0, 320, 36));
        bar.open();
        assert!(bar.is_open());
        bar.set_query("needle");
        assert_eq!(bar.query(), "needle");
        bar.set_match_progress(9, 3);
        assert_eq!(bar.match_progress(), (3, 3));
        bar.close();
        assert!(!bar.is_open());
    }
}
