// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `GridFindBar` — the incremental find bar the table family was missing (COMP-F02).
//!
//! A table's type-ahead jumps by the first character of the first column. It cannot ask "which
//! row contains `invoice`", cannot step through several matches and cannot restrict the search to
//! some columns. This companion is that bar: a query, a match mode, next/previous with wrapping,
//! and the same [`FilterOperator`] predicate `DataGrid`'s filter uses, so a find and a filter
//! cannot disagree about what "contains" means.
//!
//! # Wiring
//!
//! A host routes Ctrl+F to [`GridFindBar::open`] and forwards `match_found` to the grid's
//! selection + `ensure_visible`. The bar owns no table, so it works with every member of the
//! family (DataGrid, GridTable, VirtualTable).

use crate::core::{Color, HorizontalAlignment, Rect};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::view_widgets::data_source::IncrementalTableDataSource;
use crate::widget::view_widgets::filter_expr::FilterOperator;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::sync::Arc;

/// An incremental find bar bound to a table data source.
pub struct GridFindBar {
    base: BaseWidget,
    source: Option<Arc<dyn IncrementalTableDataSource>>,
    query: String,
    case_sensitive: bool,
    match_mode: FilterOperator,
    auto_wrap: bool,
    visible: bool,
    searchable_columns: Vec<usize>,
    /// The row the last `find_next`/`find_previous` landed on.
    current_match: Option<usize>,
    /// Emitted with the matched row's projection index.
    pub match_found: Signal1<usize>,
    /// Emitted when a search reaches the end without a match.
    pub no_match: Signal1<()>,
    /// Emitted when the bar closes.
    pub closed: Signal1<()>,
}

impl GridFindBar {
    /// Creates a closed find bar covering `geometry`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "GridFindBar"),
            source: None,
            query: String::new(),
            case_sensitive: false,
            match_mode: FilterOperator::Contains,
            auto_wrap: true,
            visible: false,
            searchable_columns: Vec::new(),
            current_match: None,
            match_found: Signal1::new(),
            no_match: Signal1::new(),
            closed: Signal1::new(),
        }
    }

    /// Binds the source the bar searches.
    pub fn set_data_source(&mut self, source: Arc<dyn IncrementalTableDataSource>) {
        self.source = Some(source);
        self.current_match = None;
    }

    /// Opens the bar and focuses its query.
    pub fn open(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.base.request_redraw();
    }

    /// Closes the bar, clearing the query so a later open starts fresh.
    pub fn close(&mut self) {
        if !self.visible && self.query.is_empty() {
            return;
        }
        self.visible = false;
        self.query.clear();
        self.current_match = None;
        self.closed.emit(());
        self.base.request_redraw();
    }

    /// Whether the bar is open.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// The current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the query and restarts the search from the first row.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.current_match = None;
        self.base.request_redraw();
    }

    /// Whether matching is case-sensitive.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets case sensitivity.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
        self.current_match = None;
    }

    /// The match mode.
    pub fn match_mode(&self) -> FilterOperator {
        self.match_mode
    }

    /// Sets the match mode.
    pub fn set_match_mode(&mut self, mode: FilterOperator) {
        self.match_mode = mode;
        self.current_match = None;
    }

    /// Whether a search wraps from the end back to the start.
    pub fn auto_wrap(&self) -> bool {
        self.auto_wrap
    }

    /// Sets whether a search wraps.
    pub fn set_auto_wrap(&mut self, auto_wrap: bool) {
        self.auto_wrap = auto_wrap;
    }

    /// The columns searched; empty means every column.
    pub fn searchable_columns(&self) -> &[usize] {
        &self.searchable_columns
    }

    /// Restricts the search to `columns`; an empty list means every column.
    pub fn set_searchable_columns(&mut self, columns: Vec<usize>) {
        self.searchable_columns = columns;
        self.current_match = None;
    }

    /// The row the last search landed on, if any.
    pub fn current_match(&self) -> Option<usize> {
        self.current_match
    }

    /// Whether `row` matches the current query.
    ///
    /// The predicate is [`FilterOperator::accepts`] for the case-insensitive modes; the
    /// case-sensitive path compares the two strings directly for the text operators, because the
    /// shared predicate is deliberately case-folding.
    pub fn row_matches(&self, row: usize) -> bool {
        let Some(source) = self.source.as_ref() else {
            return false;
        };
        if self.query.is_empty() {
            return false;
        }
        let columns = source.column_count();
        let candidates: Vec<usize> = if self.searchable_columns.is_empty() {
            (0..columns).collect()
        } else {
            self.searchable_columns.iter().copied().filter(|column| *column < columns).collect()
        };
        candidates.into_iter().any(|column| {
            let cell = source.data(row, column);
            if self.case_sensitive {
                matches_case_sensitive(cell.as_deref(), &self.query, self.match_mode)
            } else {
                self.match_mode.accepts(cell.as_deref(), &self.query)
            }
        })
    }

    /// Finds the next matching row after the current one, wrapping when enabled.
    pub fn find_next(&mut self) -> Option<usize> {
        let rows = self.source.as_ref().map(|source| source.row_count()).unwrap_or(0);
        if rows == 0 || self.query.is_empty() {
            self.no_match.emit(());
            return None;
        }
        let start = self.current_match.map_or(0, |row| row.saturating_add(1));
        for step in 0..rows {
            let row = start.saturating_add(step);
            let row = if row >= rows {
                if !self.auto_wrap {
                    break;
                }
                row % rows
            } else {
                row
            };
            if self.row_matches(row) {
                self.current_match = Some(row);
                self.match_found.emit(row);
                self.base.request_redraw();
                return Some(row);
            }
        }
        self.no_match.emit(());
        None
    }

    /// Finds the previous matching row before the current one, wrapping when enabled.
    pub fn find_previous(&mut self) -> Option<usize> {
        let rows = self.source.as_ref().map(|source| source.row_count()).unwrap_or(0);
        if rows == 0 || self.query.is_empty() {
            self.no_match.emit(());
            return None;
        }
        let start = self.current_match.unwrap_or(rows);
        for step in 0..rows {
            let row = match start.checked_sub(step + 1) {
                Some(row) => row,
                None => {
                    if !self.auto_wrap {
                        break;
                    }
                    (start + rows - step - 1) % rows
                }
            };
            if self.row_matches(row) {
                self.current_match = Some(row);
                self.match_found.emit(row);
                self.base.request_redraw();
                return Some(row);
            }
        }
        self.no_match.emit(());
        None
    }

    /// Clears the query and the current match without closing the bar.
    pub fn clear(&mut self) {
        self.query.clear();
        self.current_match = None;
        self.base.request_redraw();
    }
}

/// The case-sensitive counterpart of [`FilterOperator::accepts`] for the text operators.
fn matches_case_sensitive(cell: Option<&str>, operand: &str, mode: FilterOperator) -> bool {
    let Some(cell) = cell else {
        return mode == FilterOperator::NotContains;
    };
    match mode {
        FilterOperator::Contains => cell.contains(operand),
        FilterOperator::Equals => cell == operand,
        FilterOperator::StartsWith => cell.starts_with(operand),
        FilterOperator::EndsWith => cell.ends_with(operand),
        FilterOperator::NotContains => !cell.contains(operand),
        // A numeric comparison has no case; the shared predicate is the only answer.
        other => other.accepts(Some(cell), operand),
    }
}

impl Widget for GridFindBar {
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
        crate::core::Size::new(260, 28)
    }
    crate::impl_draw_bridge!();
}

impl Draw for GridFindBar {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.visible {
            return;
        }
        let rect = self.base.geometry();
        let palette = crate::style::palette_shared();
        let surface =
            palette.as_ref().map(|colors| colors.surface_container).unwrap_or(Color::WHITE);
        let ink = palette.as_ref().map(|colors| colors.foreground).unwrap_or(Color::BLACK);
        let border = palette
            .as_ref()
            .map(|colors| colors.outline_variant)
            .unwrap_or_else(|| surface.blend(&ink, 0.25));
        context.fill_rounded_rect(rect, 4, surface);
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        let font = crate::widget::metrics::effective_font(self.base.style());
        let label = if self.query.is_empty() { "Find…".to_string() } else { self.query.clone() };
        context.draw_text_fitted_cow(
            Rect::new(rect.x + 6, rect.y, rect.width.saturating_sub(12), rect.height),
            &label,
            font,
            if self.query.is_empty() { ink.with_alpha(140) } else { ink },
            HorizontalAlignment::Left,
        );
    }
}

impl crate::event::EventHandler for GridFindBar {
    fn handle_event(&mut self, event: &crate::event::Event) {
        use crate::event::key_codes::{BACKSPACE, ENTER, ESCAPE};
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            crate::event::Event::KeyPress { key: ESCAPE, .. } => self.close(),
            crate::event::Event::KeyPress { key: ENTER, modifiers } => {
                // Enter advances; the shift bit means "go back", matching the web find bar.
                if *modifiers & 1 != 0 {
                    let _ = self.find_previous();
                } else {
                    let _ = self.find_next();
                }
            }
            // The text channel: the bar used to ignore typed characters entirely, so the only
            // way to set a query was the programmatic setter and the control was unusable from
            // the keyboard (V-009).
            crate::event::Event::KeyPress { key: BACKSPACE, .. } => {
                if self.query.pop().is_some() {
                    self.forward_typed_query();
                }
            }
            crate::event::Event::TextInput { text } => {
                self.query.push_str(text);
                self.forward_typed_query();
            }
            // An IME commit is the same channel: the composed text becomes query text.
            crate::event::Event::ImeCommit { text } => {
                self.query.push_str(text);
                self.forward_typed_query();
            }
            _ => {}
        }
    }
}

impl GridFindBar {
    /// Restarts the search over the query the text channel just produced.
    ///
    /// A typed edit invalidates the remembered match (the row it named may no longer match), so
    /// the search restarts at the first row and forwards the result through `match_found` /
    /// `no_match`, the same signals a programmatic [`Self::find_next`] uses.
    fn forward_typed_query(&mut self) {
        self.current_match = None;
        self.base.request_redraw();
        if !self.query.is_empty() {
            let _ = self.find_next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rows(Vec<Vec<&'static str>>);

    impl IncrementalTableDataSource for Rows {
        fn row_count(&self) -> usize {
            self.0.len()
        }
        fn column_count(&self) -> usize {
            self.0.first().map_or(0, Vec::len)
        }
        fn data(&self, row: usize, column: usize) -> Option<String> {
            self.0.get(row).and_then(|line| line.get(column)).map(|cell| (*cell).to_string())
        }
    }

    fn bar() -> GridFindBar {
        let mut bar = GridFindBar::new(Rect::new(0, 0, 260, 28));
        bar.set_data_source(Arc::new(Rows(vec![
            vec!["alpha", "one"],
            vec!["beta", "two"],
            vec!["bravo", "three"],
            vec!["charlie", "four"],
        ])));
        bar
    }

    #[test]
    fn next_and_previous_walk_the_matches_with_wrapping() {
        let mut bar = bar();
        bar.set_query("b");
        assert_eq!(bar.find_next(), Some(1), "beta is the first match after row 0");
        assert_eq!(bar.find_next(), Some(2), "bravo follows");
        assert_eq!(bar.find_next(), Some(1), "wraps back to beta");
        assert_eq!(bar.find_previous(), Some(2), "previous wraps backwards");

        bar.set_auto_wrap(false);
        bar.set_query("beta");
        assert_eq!(bar.find_next(), Some(1));
        assert_eq!(bar.find_next(), None, "no second beta, and wrapping is off");
    }

    #[test]
    fn searchable_columns_restrict_the_match() {
        let mut bar = bar();
        bar.set_searchable_columns(vec![1]);
        bar.set_query("three");
        assert_eq!(bar.find_next(), Some(2), "the second column is searched");
        bar.set_query("alpha");
        assert_eq!(bar.find_next(), None, "the first column is excluded");
    }

    #[test]
    fn case_sensitivity_changes_the_answer() {
        let mut bar = bar();
        bar.set_query("ALPHA");
        assert_eq!(bar.find_next(), Some(0), "case-insensitive by default");
        bar.set_case_sensitive(true);
        bar.set_query("ALPHA");
        assert_eq!(bar.find_next(), None);
        bar.set_query("alpha");
        assert_eq!(bar.find_next(), Some(0));
    }

    /// V-009: typed text, an IME commit and Backspace all reach the query.
    ///
    /// The bar previously consumed only Escape and Enter, so the keyboard could not enter a
    /// query at all. Each edit restarts the search and forwards its result through
    /// `match_found` / `no_match`, the same signal a programmatic `find_next` uses.
    #[test]
    fn typed_input_reaches_the_query_and_forwards_the_match() {
        use crate::event::{Event, EventHandler};

        let mut bar = bar();
        bar.open();
        let matched = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&matched);
        bar.match_found.connect(move |row| {
            *sink.lock().expect("match sink") = Some(*row);
        });

        bar.handle_event(&Event::TextInput { text: "bravo".into() });
        assert_eq!(bar.query(), "bravo");
        assert_eq!(*matched.lock().expect("match sink"), Some(2), "the match is forwarded");

        bar.handle_event(&Event::KeyPress {
            key: crate::event::key_codes::BACKSPACE,
            modifiers: 0,
        });
        assert_eq!(bar.query(), "brav", "Backspace removes one character");

        bar.handle_event(&Event::ImeCommit { text: "o".into() });
        assert_eq!(bar.query(), "bravo", "an IME commit extends the same query");
    }

    #[test]
    fn open_and_close_emit_the_lifecycle_signals() {
        let mut bar = bar();
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = std::sync::Arc::clone(&closed);
        bar.closed.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
        bar.open();
        assert!(bar.is_visible());
        bar.set_query("alpha");
        bar.close();
        assert!(!bar.is_visible());
        assert!(bar.query().is_empty());
        assert_eq!(closed.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
