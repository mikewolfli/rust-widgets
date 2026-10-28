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
//! Each is its own `WidgetKind`, so a host can address either one by name.
//!
//! Registration surface: canonical name `web_find_bar` (alias `page_find_bar`), kind
//! [`WidgetKind::WebFindBar`], registered in the factory like any other companion.

use rust_widgets::compat::Arc;
use rust_widgets::impl_draw_bridge;
use rust_widgets::core::{Color, HorizontalAlignment, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, GenericSignal, Signal1};
use rust_widgets::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use crate::web_engine::{WebEngineView, WebFindRequest};
use rust_widgets::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

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
    /// The live wiring to a view's find queue, when one is attached (MW-014).
    binding: Option<WebFindBinding>,
}

/// The subscription that feeds a `WebEngineView`'s find queue (MW-014).
///
/// The bar emits requests and the view applies them; this binding is only the set of connections
/// that join the two, dropped when the bar is detached or re-attached.
struct WebFindBinding {
    _scope: ConnectionScope,
}

impl WebFindBar {
    /// Creates a closed find bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebFindBar, geometry, "WebFindBar"),
            query: String::new(),
            match_index: 0,
            match_count: 0,
            case_sensitive: false,
            visible: false,
            search_requested: Signal1::new(),
            next_requested: GenericSignal::new(),
            previous_requested: GenericSignal::new(),
            closed: GenericSignal::new(),
            binding: None,
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

    /// Wires this bar's request signals to `view`'s find queue (MW-014).
    ///
    /// After attaching, `search_requested` becomes a search request, and the next/previous
    /// signals become match steps; the view applies them on its next tick or paint through
    /// [`WebEngineView::poll_find_requests`]. A view cannot be borrowed by a signal callback, so
    /// the queue is the join, exactly as the pagination and column-chooser bindings do it.
    ///
    /// Attaching twice replaces the previous wiring rather than doubling it. A query already in
    /// the bar is run immediately, so a host that opens the bar with text preloaded sees the
    /// matches without a second keystroke.
    pub fn attach(&mut self, view: &mut WebEngineView) {
        self.detach();
        let queue = view.find_request_queue();
        let scope = ConnectionScope::new();

        let search_queue = Arc::clone(&queue);
        self.search_requested.connect_scoped(&scope, move |value| {
            let (query, case_sensitive) = &*value;
            rust_widgets::compat::lock(&search_queue).push_back(WebFindRequest::Search {
                needle: query.clone(),
                case_sensitive: *case_sensitive,
            });
        });

        let next_queue = Arc::clone(&queue);
        self.next_requested.connect_scoped(&scope, move || {
            rust_widgets::compat::lock(&next_queue).push_back(WebFindRequest::Next);
        });

        let previous_queue = Arc::clone(&queue);
        self.previous_requested.connect_scoped(&scope, move || {
            rust_widgets::compat::lock(&previous_queue).push_back(WebFindRequest::Previous);
        });

        self.binding = Some(WebFindBinding { _scope: scope });
        if !self.query.is_empty() {
            self.apply_to(view);
        }
    }

    /// Stops feeding a view.
    pub fn detach(&mut self) {
        self.binding = None;
    }

    /// Whether the bar is attached to a view's find queue.
    pub fn is_attached(&self) -> bool {
        self.binding.is_some()
    }

    /// Runs this bar's query against `view` and adopts the reported match progress.
    ///
    /// This is the missing join between the companion and the engine: before it existed the bar
    /// emitted `search_requested` and the view's `find_text` was reachable only by hand, so neither
    /// called the other (MW-014). The view's whole-word option is honoured, so the bar and the
    /// engine agree about the options the match was made with.
    pub fn apply_to(&mut self, view: &mut WebEngineView) -> (usize, usize) {
        if self.query.is_empty() {
            view.clear_find();
            self.set_match_progress(0, 0);
            return (0, 0);
        }
        let whole_word = view.find_whole_word();
        let (count, _selected) =
            view.find_text_with_options(self.query(), self.case_sensitive, whole_word, false);
        let (index, _) = view.find_progress();
        self.set_match_progress(index, count);
        (index, count)
    }

    /// Advances `view` to the next match and reports it through the bar.
    pub fn next_in(&mut self, view: &mut WebEngineView) -> Option<(usize, usize)> {
        let (index, total) = view.find_next()?;
        self.set_match_progress(index + 1, total);
        Some((index + 1, total))
    }

    /// Advances `view` to the previous match and reports it through the bar.
    pub fn previous_in(&mut self, view: &mut WebEngineView) -> Option<(usize, usize)> {
        let (index, total) = view.find_previous()?;
        self.set_match_progress(index + 1, total);
        Some((index + 1, total))
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
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(280, 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves this control's published event names to the signals that emit them.
    ///
    /// `connect_event` validates a name against the capability table; only this method joins
    /// that name to the signal the control actually emits, so every published name gets an
    /// arm here and the arm set cannot drift from the published set (the gate compares them).
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "search_requested" => Some(EventSignalRef::mapped(
                "search_requested",
                &self.search_requested,
                |(query, case_sensitive)| {
                    CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                        CapabilityValue::String(query.clone()),
                        CapabilityValue::Bool(*case_sensitive),
                    ]))
                },
            )),
            "next_requested" => Some(EventSignalRef::unit("next_requested", &self.next_requested)),
            "previous_requested" => {
                Some(EventSignalRef::unit("previous_requested", &self.previous_requested))
            }
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            _ => None,
        }
    }
}

impl WidgetProperties for WebFindBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive)),
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            "match_index" => Ok(CapabilityValue::UInt(self.match_index as u64)),
            "match_count" => Ok(CapabilityValue::UInt(self.match_count as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // A query write is a search request: the setter emits `search_requested`, which is
            // the same thing typing into the field does.
            "query" => {
                self.set_query(expect_string(value)?);
                Ok(())
            }
            // Re-runs the current query under the new sensitivity, exactly like the toggle.
            "case_sensitive" => {
                self.set_case_sensitive(expect_bool(value)?);
                Ok(())
            }
            // The two halves of the visibility switch: `open` is silent, `close` reports via
            // the `closed` signal — the same report closing through the UI produces.
            "visible" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            // The match pair is two scalars because the property vocabulary has no paired-number
            // kind; each write takes the live value of the other axis, and the one-based index
            // keeps `0` = "no selection".
            "match_index" => {
                let index = expect_usize(value)?;
                self.set_match_progress(index, self.match_count);
                Ok(())
            }
            "match_count" => {
                let count = expect_usize(value)?;
                self.set_match_progress(self.match_index, count);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "query",
            "case_sensitive",
            "visible",
            "match_index",
            "match_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `web_find_bar` publishes.
    ///
    /// Each is the payload-free half of a request the host answers: `next`/`previous` emit
    /// their `*_requested` signals (the host runs the page search), and `close` hides the bar
    /// and reports through `closed`. A query change is a value-carrying operation, so it is a
    /// property write (`query`), not a command.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "next" => {
                self.next_match();
                Ok(())
            }
            "previous" => {
                self.previous_match();
                Ok(())
            }
            "close" => {
                self.close();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
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
            rust_widgets::widget::draw_icon_at(
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

    #[test]
    fn the_property_route_round_trips_and_reports_searches() {
        let mut bar = WebFindBar::new(Rect::new(0, 0, 320, 36));
        let searches = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = searches.clone();
        bar.search_requested.connect(move |value| {
            let (query, case_sensitive) = &*value;
            sink.lock().unwrap().push((query.clone(), *case_sensitive));
        });
        // Round trip: every writable property reads back the value that was written.
        bar.set("query", CapabilityValue::String("rust".into())).unwrap();
        bar.set("case_sensitive", CapabilityValue::Bool(true)).unwrap();
        bar.set("match_count", CapabilityValue::UInt(7)).unwrap();
        bar.set("match_index", CapabilityValue::UInt(2)).unwrap();
        assert_eq!(bar.get("query").unwrap(), CapabilityValue::String("rust".into()));
        assert_eq!(bar.get("case_sensitive").unwrap(), CapabilityValue::Bool(true));
        assert_eq!(bar.get("match_count").unwrap(), CapabilityValue::UInt(7));
        assert_eq!(bar.get("match_index").unwrap(), CapabilityValue::UInt(2));
        // A query write is a search request; the case toggle re-runs the current query.
        assert_eq!(
            searches.lock().unwrap().as_slice(),
            [("rust".to_string(), false), ("rust".to_string(), true)]
        );
        // The visibility write goes through open/close, so closing reports via `closed`.
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = closed.clone();
        bar.closed.connect(move || {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        bar.set("visible", CapabilityValue::Bool(true)).unwrap();
        assert!(bar.get("visible").unwrap() == CapabilityValue::Bool(true));
        bar.set("visible", CapabilityValue::Bool(false)).unwrap();
        assert_eq!(bar.get("visible").unwrap(), CapabilityValue::Bool(false));
        assert!(closed.load(std::sync::atomic::Ordering::SeqCst));
        // Read-only names refuse; unknown names are refused as unknown.
        assert!(bar.set("match_progress", CapabilityValue::UInt(1)).is_err());
        assert!(bar.get("no_such_property").is_err());
    }

    #[test]
    fn the_command_channel_emits_the_requests_the_host_answers() {
        let mut bar = WebFindBar::new(Rect::new(0, 0, 320, 36));
        bar.set("visible", CapabilityValue::Bool(true)).unwrap();
        let nexts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let prevs = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let closes = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let n = nexts.clone();
        bar.next_requested.connect(move || {
            n.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let p = prevs.clone();
        bar.previous_requested.connect(move || {
            p.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let c = closes.clone();
        bar.closed.connect(move || {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        bar.command("next").unwrap();
        bar.command("previous").unwrap();
        bar.command("close").unwrap();
        let order = std::sync::atomic::Ordering::SeqCst;
        assert_eq!(nexts.load(order), 1);
        assert_eq!(prevs.load(order), 1);
        assert_eq!(closes.load(order), 1);
        assert_eq!(bar.get("visible").unwrap(), CapabilityValue::Bool(false));
        assert_eq!(bar.command("no_such_command"), Err(CapabilityAccessError::UnknownCommand));
    }

    /// The companion drives the engine's find API and adopts its counter (MW-014).
    #[test]
    fn the_bar_searches_and_steps_a_web_view() {
        let mut view = WebEngineView::new(Rect::new(0, 0, 320, 240));
        view.load_html("<p>cat category cat</p>");
        let mut bar = WebFindBar::new(Rect::new(0, 0, 320, 36));
        bar.open();
        bar.set_query("cat");
        assert_eq!(bar.apply_to(&mut view), (1, 3));
        assert_eq!(bar.match_progress(), (1, 3));
        assert_eq!(bar.next_in(&mut view), Some((2, 3)));
        assert_eq!(bar.match_progress(), (2, 3));
        assert_eq!(bar.previous_in(&mut view), Some((1, 3)));

        // The engine's whole-word option narrows the same query to the two whole words.
        view.set_find_whole_word(true);
        assert_eq!(bar.apply_to(&mut view), (1, 2));
        assert_eq!(bar.match_progress(), (1, 2));
    }

    /// MW-014: the bar's signals feed the view's find API without a manual `apply_to`.
    #[test]
    fn attaching_the_bar_feeds_the_view_find_state() {
        let mut view = WebEngineView::new(Rect::new(0, 0, 320, 240));
        view.load_html("<p>cat category cat</p>");
        let mut bar = WebFindBar::new(Rect::new(0, 0, 320, 36));
        bar.open();
        bar.attach(&mut view);
        assert!(bar.is_attached());

        // A query write queues a search; the view applies it on its next poll.
        bar.set_query("cat");
        assert_eq!(view.poll_find_requests(), Some((1, 3)));
        assert_eq!(view.find_progress(), (1, 3), "the query reached the engine find state");

        // The step signals drive the engine's own next/previous methods.
        bar.next_match();
        assert_eq!(view.poll_find_requests(), Some((2, 3)));
        assert_eq!(view.find_progress(), (2, 3));
        bar.previous_match();
        assert_eq!(view.poll_find_requests(), Some((1, 3)));

        // The manual API keeps working beside the binding.
        assert_eq!(bar.apply_to(&mut view), (1, 3));
        assert_eq!(bar.next_in(&mut view), Some((2, 3)));

        // A detached bar queues nothing.
        bar.detach();
        assert!(!bar.is_attached());
        bar.set_query("dog");
        assert_eq!(view.poll_find_requests(), None);
    }
}
