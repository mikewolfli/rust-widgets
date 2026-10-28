// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TerminalFindBar` — a find strip for [`TerminalView`] (COMP-A2).
//!
//! # Why this is a companion
//!
//! A terminal's scrollback is its log, and the first thing a reader does with a failing log is
//! search it. `TerminalView` owns the buffer but has no search UI; the web view, the markdown
//! editor and the data grid each grew their own strip. This control is the terminal's.
//!
//! The bar owns the query and the navigation cursor. The terminal owns the text, and exposes
//! [`TerminalView::find_matches`] so the bar scans the real scrollback rather than a copy. A
//! widget cannot hold a borrow of another, so [`TerminalFindBar::attach`] snapshots the match set
//! and the rows for display, while the navigation methods take the terminal explicitly and
//! rescan it.
//!
//! # Regex
//!
//! The crate has no `regex` dependency, so this build has no regular-expression engine. The
//! `use_regex` flag is accepted for API parity and matching stays literal;
//! [`TerminalFindBar::regex_supported`] reports `false` so a host can disable the toggle rather
//! than assume it works.

use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::key_codes::{BACKSPACE, ESCAPE};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::special_widgets::terminal_view::TerminalView;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Inset around the strip's content.
const BAR_INSET: i32 = 4;
/// Gap between the query field and the match counter.
const BAR_GAP: i32 = 4;
/// Width reserved for the match counter.
const COUNTER_WIDTH: u32 = 64;
/// The width of the accent stripe that carries the highlight colour.
const ACCENT_STRIPE: u32 = 3;

/// Whether this build can interpret `use_regex` as a regular expression.
///
/// Always `false`: the crate compiles no regex engine, so matching is literal. Exposed as a
/// constant and through [`TerminalFindBar::regex_supported`] so a host disables the option
/// instead of discovering the limitation from results.
pub const REGEX_SUPPORTED: bool = false;

/// Resolves the strip's surface, border and ink.
fn find_bar_palette(style: &crate::style::WidgetStyle) -> (Color, Color, Color) {
    let theme = crate::style::resolved_theme_style_shared("terminal_find_bar");
    let ink = style
        .text_color
        .or_else(|| theme.as_ref().and_then(|t| t.text_color))
        .unwrap_or(Color::rgb(45, 58, 74));
    let border = style
        .border_color
        .or_else(|| theme.as_ref().and_then(|t| t.border_color))
        .unwrap_or(Color::rgb(168, 184, 201));
    let surface = style
        .background_color
        .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        .unwrap_or(Color::rgb(245, 247, 250));
    (surface, border, ink)
}

/// A search strip for a terminal's scrollback.
///
/// The bar is hidden until [`Self::open`]. Hiding it does not touch the terminal, and neither
/// does showing it: only [`Self::attach`], [`Self::find_next_in`] and
/// [`Self::find_previous_in`] read the terminal, and none of them writes to it.
pub struct TerminalFindBar {
    base: BaseWidget,
    query: String,
    case_sensitive: bool,
    use_regex: bool,
    match_count: usize,
    /// The zero-based index of the match the bar last selected, if any.
    current_match: Option<usize>,
    /// The colour painted in the accent stripe and used for the current match.
    highlight_color: Option<Color>,
    /// Whether navigation wraps at the ends.
    wrap_search: bool,
    visible: bool,
    /// Whether [`Self::attach`] has seen a terminal.
    attached: bool,
    /// The match set from the last scan, as `(row, column)` pairs.
    matches: Vec<(usize, usize)>,
    /// The plain text of every scrollback row at the last scan.
    rows: Vec<String>,
    /// Emitted with `(total, current)` whenever a scan changes or reports the match set.
    ///
    /// `current` is zero-based and is `0` when there is no current match (including when the
    /// total is `0`).
    pub on_matches_changed: Signal1<(usize, usize)>,
}

impl TerminalFindBar {
    /// Creates a closed bar with an empty query.
    pub fn new(geometry: Rect) -> Self {
        Self {
            // `TerminalView` has no dedicated `WidgetKind` variant yet, so the bar registers under
            // the existing find-strip kind, exactly as `FindReplaceBar` does.
            base: BaseWidget::new(WidgetKind::SearchBar, geometry, "TerminalFindBar"),
            query: String::new(),
            case_sensitive: false,
            use_regex: false,
            match_count: 0,
            current_match: None,
            highlight_color: None,
            wrap_search: true,
            visible: false,
            attached: false,
            matches: Vec::new(),
            rows: Vec::new(),
            on_matches_changed: Signal1::new(),
        }
    }

    /// Shows the bar.
    pub fn open(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.base.request_redraw();
    }

    /// Hides the bar.
    ///
    /// The terminal is untouched; the match set and cursor stay where they were, so reopening
    /// resumes the same search.
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.base.request_redraw();
    }

    /// Whether the bar is showing.
    pub fn is_open(&self) -> bool {
        self.visible
    }

    /// Replaces the query.
    ///
    /// The previously reported counts are invalidated, because they describe the old query. The
    /// next [`Self::attach`] or navigation call scans the terminal again.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();
        if self.query == query {
            return;
        }
        self.query = query;
        self.invalidate_matches();
    }

    /// Returns the query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Sets whether matching is case-sensitive.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive == case_sensitive {
            return;
        }
        self.case_sensitive = case_sensitive;
        self.invalidate_matches();
    }

    /// Returns whether matching is case-sensitive.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets whether the query is interpreted as a regular expression.
    ///
    /// The flag is stored and published, but this build has no regex engine, so matching stays
    /// literal. See [`Self::regex_supported`].
    pub fn set_use_regex(&mut self, use_regex: bool) {
        if self.use_regex == use_regex {
            return;
        }
        self.use_regex = use_regex;
        self.invalidate_matches();
    }

    /// Returns whether the query was asked to be a regular expression.
    pub fn use_regex(&self) -> bool {
        self.use_regex
    }

    /// Whether this build can interpret [`Self::use_regex`] as a regular expression.
    ///
    /// Always `false`; see [`REGEX_SUPPORTED`].
    pub const fn regex_supported() -> bool {
        REGEX_SUPPORTED
    }

    /// Reports the total match count and the zero-based current match.
    ///
    /// The current index is clamped into the total; a total of zero clears it. A host that runs
    /// its own search engine uses this to feed the strip, exactly as `FindReplaceBar` does.
    pub fn set_match_count(&mut self, count: usize, current: usize) {
        let current = if count == 0 { None } else { Some(current.min(count - 1)) };
        if self.match_count == count && self.current_match == current {
            return;
        }
        self.match_count = count;
        self.current_match = current;
        self.base.request_redraw();
    }

    /// Returns the reported total match count.
    pub fn match_count(&self) -> usize {
        self.match_count
    }

    /// Returns the zero-based current match, if one has been selected.
    pub fn current_match(&self) -> Option<usize> {
        self.current_match
    }

    /// Sets the colour painted for the search result.
    pub fn set_highlight_color(&mut self, color: Color) {
        if self.highlight_color == Some(color) {
            return;
        }
        self.highlight_color = Some(color);
        self.base.request_redraw();
    }

    /// Returns the explicit highlight colour, or `None` while the accent colour is used.
    pub fn highlight_color(&self) -> Option<Color> {
        self.highlight_color
    }

    /// Sets whether navigation wraps at the ends.
    pub fn set_wrap_search(&mut self, wrap: bool) {
        if self.wrap_search == wrap {
            return;
        }
        self.wrap_search = wrap;
    }

    /// Returns whether navigation wraps at the ends.
    pub fn wrap_search(&self) -> bool {
        self.wrap_search
    }

    /// The match set from the last scan.
    pub fn matches(&self) -> &[(usize, usize)] {
        &self.matches
    }

    /// The plain text of one snapshotted scrollback row.
    pub fn attached_row_text(&self, row: usize) -> Option<&str> {
        self.rows.get(row).map(String::as_str)
    }

    /// Whether [`Self::attach`] has seen a terminal.
    pub fn is_attached(&self) -> bool {
        self.attached
    }

    /// Scans `terminal` for the current query and snapshot its matches and rows.
    ///
    /// The result is reported through [`Self::on_matches_changed`] even when it is empty, so a
    /// host that attaches a pre-filled bar always receives a count. The terminal's content is
    /// not read into a document that can be modified: only the text is copied.
    pub fn attach(&mut self, terminal: &mut TerminalView) {
        self.attached = true;
        let _ = self.rescan(terminal);
        self.emit_matches();
        self.base.request_redraw();
    }

    /// Stops describing the attached terminal and clears the snapshot.
    ///
    /// The query is kept, so re-attaching resumes it.
    pub fn detach(&mut self) {
        if !self.attached {
            return;
        }
        self.attached = false;
        self.matches.clear();
        self.rows.clear();
        self.match_count = 0;
        self.current_match = None;
        self.emit_matches();
        self.base.request_redraw();
    }

    /// Rescans `terminal` and selects the next match, wrapping when
    /// [`Self::set_wrap_search`] is on.
    ///
    /// Returns the selected `(row, column)` or `None` when there is no match. A terminal's
    /// content is never changed.
    pub fn find_next_in(&mut self, terminal: &mut TerminalView) -> Option<(usize, usize)> {
        let before = (self.match_count, self.current_match);
        let total = self.rescan(terminal);
        if total == 0 {
            self.report_if_changed(before);
            return None;
        }
        let next = match self.current_match {
            Some(index) if index + 1 < total => index + 1,
            Some(_) if self.wrap_search => 0,
            Some(_) => return None,
            None => 0,
        };
        self.select(next)
    }

    /// Rescans `terminal` and selects the previous match, wrapping when
    /// [`Self::set_wrap_search`] is on.
    ///
    /// With no current match the last match is selected, mirroring
    /// [`Self::find_next_in`] starting at the first.
    pub fn find_previous_in(&mut self, terminal: &mut TerminalView) -> Option<(usize, usize)> {
        let before = (self.match_count, self.current_match);
        let total = self.rescan(terminal);
        if total == 0 {
            self.report_if_changed(before);
            return None;
        }
        let previous = match self.current_match {
            Some(0) if self.wrap_search => total - 1,
            Some(0) => return None,
            Some(index) => index - 1,
            None => total - 1,
        };
        self.select(previous)
    }

    /// The rectangle the query text occupies.
    pub fn query_rect(&self) -> Rect {
        let rect = self.geometry();
        let inset = BAR_INSET;
        let available = (rect.width as i32 - inset * 2).max(0);
        let counter_width = COUNTER_WIDTH.min(available as u32);
        let counter_x = rect.x + inset + (available - counter_width as i32);
        let width = (counter_x - BAR_GAP - (rect.x + inset)).max(0) as u32;
        let height = (rect.height as i32 - inset * 2).max(0) as u32;
        Rect::new(rect.x + inset, rect.y + inset, width, height)
    }

    /// The rectangle the match counter occupies.
    pub fn counter_rect(&self) -> Rect {
        let rect = self.geometry();
        let inset = BAR_INSET;
        let available = (rect.width as i32 - inset * 2).max(0);
        let width = COUNTER_WIDTH.min(available as u32);
        let x = rect.x + inset + (available - width as i32);
        let height = (rect.height as i32 - inset * 2).max(0) as u32;
        Rect::new(x, rect.y + inset, width, height)
    }

    /// Replaces the match set and the row snapshot from `terminal`.
    fn rescan(&mut self, terminal: &TerminalView) -> usize {
        self.matches = terminal.find_matches(&self.query, self.case_sensitive, self.use_regex);
        self.rows.clear();
        for row in 0..terminal.lines().len() {
            if let Some(text) = terminal.plain_row_text(row) {
                self.rows.push(text);
            }
        }
        self.match_count = self.matches.len();
        self.current_match = match self.current_match {
            Some(current) if self.match_count > 0 => Some(current.min(self.match_count - 1)),
            _ => None,
        };
        self.match_count
    }

    /// Selects match `index`, reporting it and returning its location.
    fn select(&mut self, index: usize) -> Option<(usize, usize)> {
        let location = *self.matches.get(index)?;
        self.current_match = Some(index);
        self.emit_matches();
        self.base.request_redraw();
        Some(location)
    }

    /// Clears the counts and snapshot because the query or its options moved.
    fn invalidate_matches(&mut self) {
        self.match_count = 0;
        self.current_match = None;
        self.matches.clear();
        self.base.request_redraw();
    }

    /// Emits the current report.
    fn emit_matches(&self) {
        self.on_matches_changed.emit((self.match_count, self.current_match.unwrap_or(0)));
    }

    /// Emits only when the counts or the cursor moved.
    fn report_if_changed(&self, before: (usize, Option<usize>)) {
        if before != (self.match_count, self.current_match) {
            self.emit_matches();
        }
    }
}

impl Widget for TerminalFindBar {
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
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(320, 30),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "matches_changed" | "on_matches_changed" => {
                Some(EventSignalRef::mapped("matches_changed", &self.on_matches_changed, |value| {
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::UInt(value.0 as u64),
                        CapabilityValue::UInt(value.1 as u64),
                    ])
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for TerminalFindBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive)),
            "use_regex" => Ok(CapabilityValue::Bool(self.use_regex)),
            "wrap_search" => Ok(CapabilityValue::Bool(self.wrap_search)),
            "match_count" => Ok(CapabilityValue::UInt(self.match_count as u64)),
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "query" => {
                self.set_query(expect_string(value)?);
                Ok(())
            }
            "case_sensitive" => {
                self.set_case_sensitive(expect_bool(value)?);
                Ok(())
            }
            "use_regex" => {
                self.set_use_regex(expect_bool(value)?);
                Ok(())
            }
            "wrap_search" => {
                self.set_wrap_search(expect_bool(value)?);
                Ok(())
            }
            // The count is produced by a scan of the terminal, not set by a host: a write here
            // would be overwritten by the next `attach` or navigation call.
            "match_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            // The bar keeps its own visibility flag, and `draw`/`handle_event` read it directly.
            // `open`/`close` are the two halves of the switch and each requests a redraw, so a
            // property write repaints without the host forwarding a frame.
            "visible" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "query",
            "case_sensitive",
            "use_regex",
            "wrap_search",
            "match_count",
            "visible",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for TerminalFindBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            Event::KeyPress { key: ESCAPE, .. } => self.close(),
            Event::TextInput { text } => {
                if text.is_empty() {
                    return;
                }
                self.query.push_str(text);
                self.invalidate_matches();
            }
            Event::KeyPress { key: BACKSPACE, .. } => {
                if self.query.pop().is_none() {
                    return;
                }
                self.invalidate_matches();
            }
            _ => { /* Other events are not relevant. */ }
        }
    }
}

impl Draw for TerminalFindBar {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let (surface, border, ink) = find_bar_palette(style);
        let accent = self.highlight_color.unwrap_or_else(|| {
            crate::style::palette_shared()
                .as_ref()
                .map(|colors| colors.primary)
                .unwrap_or(Color::PRIMARY)
        });
        let font = effective_font(style);

        context.fill_rounded_rect(rect, 2, surface);
        crate::widget::draw::stroke_surface_border(context, rect, [2, 2, 2, 2], border, 1);
        context.fill_rect(Rect::new(rect.x, rect.y, ACCENT_STRIPE, rect.height), accent);

        let query_rect = self.query_rect();
        if self.query.is_empty() {
            // A muted prompt rather than an empty field, so an open bar says what it wants.
            context.draw_text_line(
                query_rect,
                "Find in scrollback",
                font,
                ink.with_alpha(120),
                HorizontalAlignment::Left,
            );
        } else {
            context.draw_text_line(query_rect, &self.query, font, ink, HorizontalAlignment::Left);
        }

        let counter_rect = self.counter_rect();
        let counter = if self.match_count == 0 {
            String::from("0/0")
        } else {
            format!("{}/{}", self.current_match.map_or(0, |index| index + 1), self.match_count)
        };
        context.draw_text_line(counter_rect, &counter, font, accent, HorizontalAlignment::Right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn terminal_with(lines: &[&str]) -> TerminalView {
        let mut terminal = TerminalView::new(Rect::new(0, 0, 500, 260));
        for line in lines {
            terminal.append_output(*line);
        }
        terminal
    }

    fn reports(bar: &mut TerminalFindBar) -> Arc<Mutex<Vec<(usize, usize)>>> {
        let emitted = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let sink = emitted.clone();
        bar.on_matches_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((value.0, value.1));
            }
        });
        emitted
    }

    /// Attach scans the real buffer, reports `(total, 0)`, and leaves the terminal untouched.
    #[test]
    fn attaching_reports_the_match_set_and_leaves_the_terminal_alone() {
        let mut terminal =
            terminal_with(&["\u{1b}[31merror\u{1b}[0m at line", "no error here", "error again"]);
        let before = terminal.lines().to_vec();
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        let emitted = reports(&mut bar);
        bar.set_query("error");
        bar.attach(&mut terminal);

        assert_eq!(bar.match_count(), 3);
        assert_eq!(bar.matches(), &[(0, 0), (1, 3), (2, 0)]);
        assert_eq!(bar.current_match(), None, "attaching reports, it does not select");
        assert_eq!(
            emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec![(3, 0)]
        );
        assert_eq!(terminal.lines(), before.as_slice(), "a scan must not rewrite the buffer");
        assert_eq!(bar.attached_row_text(0), Some("error at line"));

        bar.open();
        assert!(bar.is_open());
        bar.close();
        assert!(!bar.is_open());
        assert_eq!(terminal.lines(), before.as_slice(), "visibility must not rewrite the buffer");
    }

    /// Forward and backward navigation walk the matches and wrap at the ends.
    #[test]
    fn navigation_walks_forward_and_back_with_wrapping() {
        let mut terminal = terminal_with(&["a a", "a"]);
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        bar.set_query("a");
        bar.attach(&mut terminal);

        assert_eq!(bar.find_next_in(&mut terminal), Some((0, 0)));
        assert_eq!(bar.current_match(), Some(0));
        assert_eq!(bar.find_next_in(&mut terminal), Some((0, 2)));
        assert_eq!(bar.find_next_in(&mut terminal), Some((1, 0)));
        assert_eq!(bar.find_next_in(&mut terminal), Some((0, 0)), "next wraps");
        assert_eq!(bar.find_previous_in(&mut terminal), Some((1, 0)), "previous wraps");
    }

    /// With wrapping off, navigation stops at the ends instead of cycling.
    #[test]
    fn wrap_search_off_stops_at_the_ends() {
        let mut terminal = terminal_with(&["a a", "a"]);
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        bar.set_query("a");
        bar.set_wrap_search(false);
        bar.attach(&mut terminal);

        assert_eq!(bar.find_next_in(&mut terminal), Some((0, 0)));
        assert_eq!(bar.find_next_in(&mut terminal), Some((0, 2)));
        assert_eq!(bar.find_next_in(&mut terminal), Some((1, 0)));
        assert_eq!(bar.find_next_in(&mut terminal), None);
        assert_eq!(bar.current_match(), Some(2), "a refused step keeps the selection");

        assert_eq!(bar.find_previous_in(&mut terminal), Some((0, 2)));
        assert_eq!(bar.find_previous_in(&mut terminal), Some((0, 0)));
        assert_eq!(bar.find_previous_in(&mut terminal), None);
    }

    /// A host-supplied count is clamped into the total, and a zero total clears the cursor.
    #[test]
    fn set_match_count_clamps_the_current_index() {
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        bar.set_match_count(5, 9);
        assert_eq!(bar.match_count(), 5);
        assert_eq!(bar.current_match(), Some(4));
        bar.set_match_count(2, 0);
        assert_eq!(bar.current_match(), Some(0));
        bar.set_match_count(0, 3);
        assert_eq!(bar.match_count(), 0);
        assert_eq!(bar.current_match(), None);
    }

    /// An empty query has no matches, and navigation reports that honestly.
    #[test]
    fn an_empty_query_reports_no_matches() {
        let mut terminal = terminal_with(&["something"]);
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        let emitted = reports(&mut bar);
        bar.attach(&mut terminal);

        assert_eq!(bar.match_count(), 0);
        assert_eq!(bar.find_next_in(&mut terminal), None);
        assert_eq!(bar.find_previous_in(&mut terminal), None);
        assert_eq!(
            emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec![(0, 0)],
            "attach reports once; a fruitless step reports nothing new"
        );
    }

    /// The strip's two regions stay inside its rectangle even when the rectangle is tiny.
    #[test]
    fn query_and_counter_rects_stay_inside_a_small_bar() {
        let bar = TerminalFindBar::new(Rect::new(10, 20, 10, 6));
        let query = bar.query_rect();
        let counter = bar.counter_rect();
        assert!(query.x >= 10 && query.right() <= 20, "the query box stays inside");
        assert!(counter.x >= 10 && counter.right() <= 20, "the counter stays inside");
        assert_eq!(query.y, 24);
        assert_eq!(counter.y, 24);
        assert!(query.right() <= counter.x, "the two regions do not overlap");

        let zero = TerminalFindBar::new(Rect::new(0, 0, 0, 0));
        assert_eq!(zero.query_rect().width, 0);
        assert_eq!(zero.counter_rect().width, 0);
    }

    /// No regex engine is compiled in; the flag is stored but matching stays literal.
    #[test]
    fn use_regex_is_reported_unsupported_and_matches_literally() {
        assert!(!TerminalFindBar::regex_supported());
        let mut terminal = terminal_with(&["abc a.c"]);
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        bar.set_use_regex(true);
        assert!(bar.use_regex());
        bar.set_query("a.c");
        bar.attach(&mut terminal);

        assert_eq!(bar.match_count(), 1, "the dot is literal, not a wildcard");
        assert_eq!(bar.matches(), &[(0, 4)]);
    }

    /// Detaching clears the snapshot and reports the empty set once.
    #[test]
    fn detaching_clears_the_snapshot() {
        let mut terminal = terminal_with(&["error"]);
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        let emitted = reports(&mut bar);
        bar.set_query("error");
        bar.attach(&mut terminal);
        assert!(bar.is_attached());

        bar.detach();
        assert!(!bar.is_attached());
        assert!(bar.matches().is_empty());
        assert_eq!(bar.attached_row_text(0), None);
        assert_eq!(bar.query(), "error", "the query survives a detach");
        assert_eq!(
            emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default(),
            vec![(1, 0), (0, 0)]
        );
    }

    /// Escape closes an open bar, and text input edits the query in place.
    #[test]
    fn keyboard_input_edits_the_query_and_escape_closes() {
        let mut bar = TerminalFindBar::new(Rect::new(0, 0, 320, 30));
        bar.open();
        bar.handle_event(&Event::text_input("err"));
        assert_eq!(bar.query(), "err");
        bar.handle_event(&Event::key_press(BACKSPACE, 0));
        assert_eq!(bar.query(), "er");
        bar.handle_event(&Event::key_press(ESCAPE, 0));
        assert!(!bar.is_open());
    }
}
