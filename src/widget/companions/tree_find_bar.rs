// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TreeFindBar` — the query surface the tree family was missing (CMP4-01).
//!
//! # Why a companion
//!
//! `TreeView` and `TreeTable` publish selection, activation and expansion, and load children
//! incrementally, but they own no query surface: a host with a large resource tree had to build
//! its own search UI. The grid family already has [`GridFindBar`], and the editor family has
//! [`FindReplaceBar`]; this is the same partner for a hierarchical row list.
//!
//! # What the bar owns, and what the host owns
//!
//! The bar owns the query, the matching predicate and the match cursor. It does not own the
//! tree: [`TreeSearchSource`] supplies the row count and the label of each row, and the bar
//! reports every landed match through [`TreeFindBar::match_changed`]. The host applies that
//! index through its existing selection / focus / `ensure_visible` API, which is the same
//! division [`GridFindBar`] uses for tables.
//!
//! # Matching
//!
//! [`FilterOperator`] is reused, so a tree find and the grid find cannot disagree about what
//! "contains" means. Navigation wraps at both ends, which is the default the grid bar shipped
//! with.

use crate::compat::{Arc, String};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes::{BACKSPACE, ENTER, ESCAPE};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::view_widgets::filter_expr::FilterOperator;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The flattened rows a tree find bar searches.
///
/// The host projects its tree onto the rows it is currently willing to search (for a lazily
/// loaded tree, the rows it has loaded). The bar never walks the tree itself: doing so would
/// duplicate the host's loading policy and could ask for children it has not fetched.
pub trait TreeSearchSource: Send + Sync {
    /// How many rows are currently searchable.
    fn row_count(&self) -> usize;
    /// The label searched for `row`, or `""` when the row has no text.
    fn row_label(&self, row: usize) -> &str;
}

/// An incremental find bar bound to a tree's flattened row source.
pub struct TreeFindBar {
    base: BaseWidget,
    source: Option<Arc<dyn TreeSearchSource>>,
    query: String,
    case_sensitive: bool,
    match_mode: FilterOperator,
    /// The row the last `find_next` / `find_previous` landed on.
    current_match: Option<usize>,
    /// Whether the host should scroll the matched row into view.
    ///
    /// The bar cannot scroll a tree it does not own; the flag travels as a published property so
    /// the host that receives `match_changed` can read the user's preference.
    auto_scroll: bool,
    visible: bool,
    placeholder: String,
    /// The uncommitted IME composition drawn inline after the query.
    preedit: String,
    /// The painted prompt/query text, keyed by the query it was built from.
    display_cache: crate::compat::RefCell<Option<(String, String)>>,
    /// Emitted with the matched row's index.
    pub match_changed: Signal1<usize>,
}

impl TreeFindBar {
    /// Creates a closed find bar covering `geometry`.
    pub fn new(geometry: Rect) -> Self {
        // The capability table declares this host `visible` by default; `contains_point` keeps a
        // closed bar from taking a press, and `close` also clears the base flag.
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "TreeFindBar"),
            source: None,
            query: String::new(),
            case_sensitive: false,
            match_mode: FilterOperator::Contains,
            current_match: None,
            auto_scroll: true,
            visible: false,
            placeholder: "Find…".to_string(),
            preedit: String::new(),
            display_cache: crate::compat::RefCell::new(None),
            match_changed: Signal1::new(),
        }
    }

    /// Binds the flattened rows the bar searches.
    pub fn set_data_source(&mut self, source: Arc<dyn TreeSearchSource>) {
        self.source = Some(source);
        self.current_match = None;
    }

    /// Opens the bar and focuses its query.
    pub fn open(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.base.show();
        let id = crate::widget::runtime::registry_id_of(self.base.id()).unwrap_or(self.base.id());
        let _ = crate::widget::runtime::focus_widget(id);
        self.base.request_redraw();
    }

    /// Closes the bar, clearing the query so a later open starts fresh.
    pub fn close(&mut self) {
        if !self.visible && self.query.is_empty() {
            return;
        }
        self.visible = false;
        self.query.clear();
        self.preedit.clear();
        self.current_match = None;
        self.base.hide();
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
        self.base.request_redraw();
    }

    /// The match mode.
    pub fn match_mode(&self) -> FilterOperator {
        self.match_mode
    }

    /// Sets the match mode.
    pub fn set_match_mode(&mut self, mode: FilterOperator) {
        self.match_mode = mode;
        self.current_match = None;
        self.base.request_redraw();
    }

    /// Whether the host should scroll a matched row into view.
    pub fn auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    /// Sets whether the host should scroll a matched row into view.
    pub fn set_auto_scroll(&mut self, auto_scroll: bool) {
        if self.auto_scroll != auto_scroll {
            self.auto_scroll = auto_scroll;
            self.base.request_redraw();
        }
    }

    /// The row the last search landed on, if any.
    pub fn current_match(&self) -> Option<usize> {
        self.current_match
    }

    /// Whether `row` matches the current query.
    pub fn row_matches(&self, row: usize) -> bool {
        let Some(source) = self.source.as_ref() else {
            return false;
        };
        if self.query.is_empty() || row >= source.row_count() {
            return false;
        }
        let label = source.row_label(row);
        if self.case_sensitive {
            matches_case_sensitive(Some(label), &self.query, self.match_mode)
        } else {
            self.match_mode.accepts(Some(label), &self.query)
        }
    }

    /// Finds the next matching row after the current one, wrapping at the end.
    pub fn find_next(&mut self) -> Option<usize> {
        let rows = self.source.as_ref().map(|source| source.row_count()).unwrap_or(0);
        if rows == 0 || self.query.is_empty() {
            return None;
        }
        let start = self.current_match.map_or(0, |row| row.saturating_add(1));
        for step in 0..rows {
            let row = (start + step) % rows;
            if self.row_matches(row) {
                self.current_match = Some(row);
                self.match_changed.emit(row);
                self.base.request_redraw();
                return Some(row);
            }
        }
        None
    }

    /// Finds the previous matching row before the current one, wrapping at the start.
    pub fn find_previous(&mut self) -> Option<usize> {
        let rows = self.source.as_ref().map(|source| source.row_count()).unwrap_or(0);
        if rows == 0 || self.query.is_empty() {
            return None;
        }
        let start = self.current_match.unwrap_or(rows);
        for step in 0..rows {
            let row = (start + rows - step - 1) % rows;
            if self.row_matches(row) {
                self.current_match = Some(row);
                self.match_changed.emit(row);
                self.base.request_redraw();
                return Some(row);
            }
        }
        None
    }
}

/// The case-sensitive counterpart of [`FilterOperator::accepts`] for the text operators.
fn matches_case_sensitive(label: Option<&str>, operand: &str, mode: FilterOperator) -> bool {
    let Some(label) = label else {
        return mode == FilterOperator::NotContains;
    };
    match mode {
        FilterOperator::Contains => label.contains(operand),
        FilterOperator::Equals => label == operand,
        FilterOperator::StartsWith => label.starts_with(operand),
        FilterOperator::EndsWith => label.ends_with(operand),
        FilterOperator::NotContains => !label.contains(operand),
        // A numeric comparison has no case; the shared predicate is the only answer.
        other => other.accepts(Some(label), operand),
    }
}

impl Widget for TreeFindBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    /// The query/replace fields edit text, so composition text belongs to this control even
    /// though its kind is the shared `Panel` (U3-02).
    fn accepts_ime_input(&self) -> bool {
        true
    }
    /// A closed bar paints nothing, so it must not accept a press over its rectangle.
    fn contains_point(&self, point: Point) -> bool {
        self.visible && self.base.contains_point_with_touch_expansion(point)
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        Size::new(260, 28)
    }
    /// A find bar owns the keyboard while it is open, so Tab must be able to reach it.
    ///
    /// Its kind is `Panel`, which `WidgetKind::wants_keyboard_focus` does not list; without this
    /// override the bar could not be focused by click or by Tab, and Ctrl+F left the keys in the
    /// document behind it.
    fn is_focusable(&self) -> bool {
        true
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "match_changed" => {
                Some(EventSignalRef::mapped("match_changed", &self.match_changed, |row| {
                    CapabilityValue::UInt(*row as u64)
                }))
            }
            _ => None,
        }
    }
}

impl Draw for TreeFindBar {
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
        let font = effective_font(self.base.style());
        let query_empty = self.query.is_empty();
        // The displayed text changes only with the query, so a still bar reuses the last frame's
        // `String` instead of cloning the query on every paint.
        let mut cache = self.display_cache.borrow_mut();
        if cache.as_ref().is_none_or(|(query, _)| query != &self.query) {
            let text = if query_empty { self.placeholder.clone() } else { self.query.clone() };
            *cache = Some((self.query.clone(), text));
        }
        let label = cache.as_ref().map(|(_, text)| text.as_str()).unwrap_or_default();
        let text_box = Rect::new(rect.x + 6, rect.y, rect.width.saturating_sub(12), rect.height);
        context.draw_text_fitted_ellipsis(
            text_box,
            label,
            font,
            if query_empty { ink.with_alpha(140) } else { ink },
            HorizontalAlignment::Left,
        );
        // The uncommitted IME composition is painted after the query with an underline, so a
        // CJK composition is visible before it is confirmed. The anchor and the run are both
        // bounded by the query's own text box: `rect.x + 6 + query_width` ran past the bar for a
        // query longer than it, and the parent's clip then erased the composition the user was
        // still typing.
        if !self.preedit.is_empty() {
            let query_width = context.measure_text(&self.query, font).width as i32;
            let preedit_x = crate::widget::text_utils::composition_x(query_width, text_box);
            let line = context.text_line(rect, font);
            let preedit_width = (context.measure_text(&self.preedit, font).width as i32)
                .min(text_box.right() - preedit_x)
                .max(0);
            context.push_clip(text_box.x, text_box.y, text_box.width, text_box.height);
            context.draw_text(
                Point::new(preedit_x, line.y),
                &self.preedit,
                font,
                ink,
                HorizontalAlignment::Left,
            );
            let underline_y = line.y + line.height as i32 - 1;
            context.draw_line(
                Point::new(preedit_x, underline_y),
                Point::new(preedit_x + preedit_width, underline_y),
                ink,
            );
            context.pop_clip();
        }

        // Tab and Shortcut focus must be visible here as on every other control (U3-04). The bar
        // opens focused, so without a ring "open and focused" painted the same constant surface as
        // "open and unfocused". A closed bar returns before this point and is unchanged.
        let ring_radius = self.base.style().resolved_corner_radii().to_array()[0];
        self.base.draw_focus_ring(context, rect, ring_radius);
    }
}

impl EventHandler for TreeFindBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            Event::KeyPress { key: ESCAPE, .. } => self.close(),
            Event::KeyPress { key: ENTER, modifiers } => {
                // Enter advances; the shift bit means "go back", matching the grid bar.
                if *modifiers & 1 != 0 {
                    let _ = self.find_previous();
                } else {
                    let _ = self.find_next();
                }
            }
            Event::KeyPress { key: BACKSPACE, .. } => {
                if self.query.pop().is_some() {
                    self.current_match = None;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } => {
                self.preedit.clear();
                self.query.push_str(text);
                self.current_match = None;
                self.base.request_redraw();
            }
            Event::ImeCommit { text } => {
                self.preedit.clear();
                self.query.push_str(text);
                self.current_match = None;
                self.base.request_redraw();
            }
            Event::ImePreedit { text, .. } => {
                self.preedit = text.clone();
                self.base.request_redraw();
            }
            _ => {}
        }
    }
}

impl WidgetProperties for TreeFindBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive)),
            "match_mode" => Ok(CapabilityValue::String(self.match_mode.as_str().to_string())),
            "current_match" => {
                Ok(CapabilityValue::UInt(self.current_match.map_or(0, |row| row as u64)))
            }
            "auto_scroll" => Ok(CapabilityValue::Bool(self.auto_scroll)),
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
            "match_mode" => {
                let token = expect_string(value)?;
                let mode =
                    FilterOperator::from_name(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_match_mode(mode);
                Ok(())
            }
            "current_match" => {
                // Read-only: the match cursor is the search's own output. Answer the read-only
                // verdict before inspecting the payload, which is what the declaration test
                // expects from a property marked read-only.
                let _ = value;
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "auto_scroll" => {
                self.set_auto_scroll(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "query",
            "case_sensitive",
            "match_mode",
            "current_match",
            "auto_scroll",
            BASE_PROPERTY_NAMES
        ]
    }

    fn property_tokens(&self, name: &str) -> &'static [&'static str] {
        match name {
            "match_mode" => FilterOperator::TOKENS,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rows(Vec<&'static str>);

    impl TreeSearchSource for Rows {
        fn row_count(&self) -> usize {
            self.0.len()
        }
        fn row_label(&self, row: usize) -> &str {
            self.0.get(row).copied().unwrap_or("")
        }
    }

    fn bar() -> TreeFindBar {
        let mut bar = TreeFindBar::new(Rect::new(0, 0, 260, 28));
        bar.set_data_source(Arc::new(Rows(vec!["alpha", "beta", "alphabet", "gamma"])));
        bar.open();
        bar
    }

    #[test]
    fn find_next_wraps_and_reports_each_match() {
        let mut bar = bar();
        bar.set_query("alpha");
        assert_eq!(bar.find_next(), Some(0));
        assert_eq!(bar.find_next(), Some(2));
        assert_eq!(bar.find_next(), Some(0), "navigation wraps at the end");
    }

    #[test]
    fn find_previous_wraps_and_case_sensitivity_is_honoured() {
        let mut bar = bar();
        bar.set_query("alpha");
        assert_eq!(bar.find_previous(), Some(2));
        bar.set_query("ALPHA");
        assert_eq!(bar.find_next(), Some(0), "default matching folds case");
        bar.set_case_sensitive(true);
        assert_eq!(bar.find_next(), None);
    }

    #[test]
    fn the_published_properties_route_through_the_bar() {
        let mut bar = bar();
        bar.set("query", CapabilityValue::String("beta".into())).unwrap();
        bar.set("match_mode", CapabilityValue::String("starts_with".into())).unwrap();
        assert_eq!(bar.get("match_mode").unwrap(), CapabilityValue::String("starts_with".into()));
        assert_eq!(
            bar.set("current_match", CapabilityValue::UInt(1)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    /// A Tab-focused bar paints a different frame from an open but unfocused one (W5-28).
    ///
    /// The bar opens focused and painted a constant surface, so the two states were visually
    /// identical. This compares two renders of the same open bar and fails if the ring is not
    /// painted for keyboard focus.
    #[test]
    fn keyboard_focus_changes_the_painted_bar() {
        use crate::event::{Event, EventHandler, FocusReason};

        let _guard = crate::style::theme_test_guard();
        let mut bar = bar();
        let resting = crate::widget::svg::render_to_svg(&mut bar);

        bar.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        let focused = crate::widget::svg::render_to_svg(&mut bar);
        assert_ne!(resting, focused, "a Tab-focused find bar must paint a distinct frame");
    }
}
