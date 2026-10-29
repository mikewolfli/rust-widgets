// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The reusable markdown find/replace bar.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::collections::HashMap;

/// The find bar's state.
#[derive(Debug, Clone, Default)]
pub(crate) struct FindState {
    pub(crate) visible: bool,
    pub(crate) query: String,
    /// Byte offset of the current match, when one is highlighted.
    pub(crate) current: Option<usize>,
}

/// One replacement the host is asked to apply.
///
/// Ranges are byte offsets into the document the bar was told about through
/// [`FindBar::set_text`], in document order. A host applies them back-to-front so an earlier
/// replacement does not shift a later range; `all` highlights are carried in one request rather
/// than one signal per match, because a signal has no way to say "and three more".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceRequest {
    /// Byte ranges to replace, in ascending document order.
    pub ranges: Vec<(usize, usize)>,
    /// Replacement text for every range.
    pub text: String,
}

/// Which field of a [`FindBar`] has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FindFocus {
    /// The query field; the default.
    #[default]
    Query,
    /// The replacement field.
    Replace,
}

/// Height of one field row in the find bar.
const FIND_ROW_HEIGHT: i32 = 26;
/// Side of a toggle cell (`Aa`, `W`).
const FIND_TOGGLE_SIDE: i32 = 18;

/// A reusable find/replace bar.
///
/// It is a control, not a dialog: it owns the query, the matching policy and the match list for a
/// document **snapshot** the host hands it, and reports navigation and replacement requests. It
/// does not own the document. That split is why [`Self::replace`] emits a
/// [`ReplaceRequest`] instead of mutating text it never received: a bar that edited a copy would
/// report success for an edit the document never saw.
pub struct FindBar {
    base: BaseWidget,
    visible: bool,
    query: String,
    replacement: String,
    case_sensitive: bool,
    whole_word: bool,
    /// Whether the replacement row is painted.
    replace_visible: bool,
    /// The document snapshot the matches address.
    haystack: String,
    /// `haystack` as `(byte offset, char)` pairs, rebuilt only when the snapshot changes.
    ///
    /// The matcher used to rebuild this vector on every keystroke of the query; the snapshot is
    /// what changes it, so it is built once in [`Self::set_text`].
    char_index: Vec<(usize, char)>,
    /// How many ranges the last replacement actually changed, as reported by the host.
    last_replaced: usize,
    /// Every match, in document order.
    matches: Vec<(usize, usize)>,
    /// The active match, as an index into `matches`.
    current: Option<usize>,
    /// The painted `current/total` counter, keyed by the pair it was formatted from.
    counter_cache: RefCell<Option<((usize, usize), String)>>,
    /// The painted `"N replaced"` receipt, keyed by the count it was formatted from.
    replaced_cache: (usize, String),
    /// Which field typing goes to.
    focus: FindFocus,
    /// Emitted with the active match's byte range, or `(0, 0)` when there is no match.
    pub search_changed: Signal1<(usize, usize)>,
    /// Emitted when the reader asks for a replacement.
    pub replace_requested: Signal1<ReplaceRequest>,
}

impl FindBar {
    /// Creates a closed find bar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FindBar, geometry, "FindBar"),
            visible: false,
            query: String::new(),
            replacement: String::new(),
            case_sensitive: false,
            whole_word: false,
            replace_visible: false,
            haystack: String::new(),
            char_index: Vec::new(),
            last_replaced: 0,
            matches: Vec::new(),
            current: None,
            counter_cache: RefCell::new(None),
            replaced_cache: (0, String::new()),
            focus: FindFocus::Query,
            search_changed: Signal1::new(),
            replace_requested: Signal1::new(),
        }
    }

    /// Whether the bar is showing.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Shows the bar and focuses the query field.
    pub fn open(&mut self) {
        self.visible = true;
        self.focus = FindFocus::Query;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Hides the bar without touching the document or the active match.
    pub fn close(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Points the bar at `text` and recomputes the match list.
    ///
    /// The bar keeps the snapshot rather than a live handle because it has no way to observe a
    /// document edit; the host calls this after every change, which is also the moment the
    /// replacement it applied becomes visible to the next search.
    pub fn set_text(&mut self, text: &str) {
        self.haystack.clear();
        self.haystack.push_str(text);
        self.char_index = self.haystack.char_indices().collect();
        self.recompute_matches();
    }

    /// The document snapshot the matches address.
    pub fn text(&self) -> &str {
        &self.haystack
    }

    /// The current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Replaces the query and recomputes the matches.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// Whether matching is case-sensitive.
    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets the case-sensitivity policy.
    pub fn set_case_sensitive(&mut self, sensitive: bool) {
        if self.case_sensitive == sensitive {
            return;
        }
        self.case_sensitive = sensitive;
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// Whether matches must be whole words.
    pub fn whole_word(&self) -> bool {
        self.whole_word
    }

    /// Sets the whole-word policy.
    pub fn set_whole_word(&mut self, whole_word: bool) {
        if self.whole_word == whole_word {
            return;
        }
        self.whole_word = whole_word;
        self.recompute_matches();
        self.base.request_redraw();
    }

    /// The replacement text.
    pub fn replace_text(&self) -> &str {
        &self.replacement
    }

    /// Sets the replacement text.
    pub fn set_replace_text(&mut self, text: impl Into<String>) {
        self.replacement = text.into();
        self.base.request_redraw();
    }

    /// Whether the replacement row is painted.
    pub fn replace_visible(&self) -> bool {
        self.replace_visible
    }

    /// Shows or hides the replacement row.
    pub fn set_replace_visible(&mut self, visible: bool) {
        if self.replace_visible == visible {
            return;
        }
        self.replace_visible = visible;
        if !visible {
            self.focus = FindFocus::Query;
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many matches the current query has.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// The active match's byte range, when one is selected.
    pub fn current_range(&self) -> Option<(usize, usize)> {
        self.current.and_then(|index| self.matches.get(index).copied())
    }

    /// Selects the next match, wrapping at the end.
    ///
    /// Returns whether a match was selected; an empty match list reports `false` and emits the
    /// "no match" range, so a subscriber is told the search came up empty rather than left with
    /// the previous result.
    pub fn find_next(&mut self) -> bool {
        if self.matches.is_empty() {
            self.current = None;
            self.search_changed.emit((0, 0));
            return false;
        }
        let next = match self.current {
            Some(index) => (index + 1) % self.matches.len(),
            None => 0,
        };
        self.select_match(next);
        true
    }

    /// Selects the previous match, wrapping at the start.
    pub fn find_previous(&mut self) -> bool {
        if self.matches.is_empty() {
            self.current = None;
            self.search_changed.emit((0, 0));
            return false;
        }
        let next = match self.current {
            Some(0) => self.matches.len() - 1,
            Some(index) => index - 1,
            None => self.matches.len() - 1,
        };
        self.select_match(next);
        true
    }

    /// Selects match `index` and reports its range.
    fn select_match(&mut self, index: usize) {
        self.current = Some(index);
        if let Some(range) = self.current_range() {
            self.search_changed.emit(range);
        }
        self.base.request_redraw();
    }

    /// Recomputes `matches` and keeps the active match when it still exists.
    fn recompute_matches(&mut self) {
        let previous = self.current.and_then(|index| self.matches.get(index).copied());
        self.matches = find_matches(
            &self.haystack,
            &self.char_index,
            &self.query,
            self.case_sensitive,
            self.whole_word,
        );
        self.current = previous.and_then(|range| self.matches.iter().position(|hit| *hit == range));
        // Nothing is selected automatically: a query the reader is still typing is not yet a
        // navigation, and starting a match early would make the first `find_next` skip it.
    }

    /// How many ranges the host last reported as replaced.
    pub fn last_replaced_count(&self) -> usize {
        self.last_replaced
    }

    /// Records how many ranges a host-side replacement changed.
    ///
    /// The bar cannot observe the document, so the count is reported back to it by whoever applied
    /// the [`ReplaceRequest`]. Keeping it here is what makes the result visible in the bar instead
    /// of only in the host's state.
    pub fn set_last_replaced(&mut self, count: usize) {
        if self.last_replaced == count {
            return;
        }
        self.last_replaced = count;
        // The receipt is part of the bar's state, not the frame: rebuild it here so `draw` can
        // paint the cached text instead of formatting a new string every pass.
        self.replaced_cache =
            if count == 0 { (0, String::new()) } else { (count, format!("{count} replaced")) };
        self.base.request_redraw();
    }

    /// Asks the host to replace the active match.
    ///
    /// Returns whether a request was emitted. The match list is left alone: the text it addresses
    /// has not changed yet, and a bar that renumbered its matches before the document did would
    /// point the next request at the wrong bytes.
    pub fn replace(&mut self) -> bool {
        let Some(range) = self.current_range() else {
            return false;
        };
        self.replace_requested
            .emit(ReplaceRequest { ranges: vec![range], text: self.replacement.clone() });
        true
    }

    /// Asks the host to replace every match in one request.
    pub fn replace_all(&mut self) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        self.replace_requested
            .emit(ReplaceRequest { ranges: self.matches.clone(), text: self.replacement.clone() });
        true
    }

    /// The first row's box.
    fn query_band(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, FIND_ROW_HEIGHT.min(rect.height as i32) as u32)
    }

    /// The replacement row's box, when it is shown.
    fn replace_band(&self) -> Option<Rect> {
        if !self.replace_visible {
            return None;
        }
        let rect = self.geometry();
        let top = rect.y + FIND_ROW_HEIGHT;
        let height = rect.height as i32 - FIND_ROW_HEIGHT;
        (height > 0).then(|| Rect::new(rect.x, top, rect.width, height as u32))
    }

    /// The `Aa` toggle box.
    fn case_toggle_rect(&self) -> Rect {
        let band = self.query_band();
        Rect::new(
            band.x + band.width as i32 - 2 * FIND_TOGGLE_SIDE - 6,
            band.y + (band.height as i32 - FIND_TOGGLE_SIDE) / 2,
            FIND_TOGGLE_SIDE as u32,
            FIND_TOGGLE_SIDE as u32,
        )
    }

    /// The whole-word toggle box.
    fn word_toggle_rect(&self) -> Rect {
        let band = self.query_band();
        Rect::new(
            band.x + band.width as i32 - FIND_TOGGLE_SIDE - 3,
            band.y + (band.height as i32 - FIND_TOGGLE_SIDE) / 2,
            FIND_TOGGLE_SIDE as u32,
            FIND_TOGGLE_SIDE as u32,
        )
    }

    /// The box the query text occupies, between the left inset and the toggles.
    fn query_text_rect(&self) -> Rect {
        let band = self.query_band();
        let right = band.width as i32 - 2 * FIND_TOGGLE_SIDE - 12;
        Rect::new(band.x + 6, band.y, right.max(8) as u32, band.height)
    }

    /// The "replace" action cell on the replacement row.
    fn replace_button_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        Some(Rect::new(
            band.x + band.width as i32 - 74,
            band.y + 3,
            42,
            (band.height as i32 - 6).max(1) as u32,
        ))
    }

    /// The "replace all" action cell on the replacement row.
    fn replace_all_button_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        Some(Rect::new(
            band.x + band.width as i32 - 30,
            band.y + 3,
            28,
            (band.height as i32 - 6).max(1) as u32,
        ))
    }

    /// The box the replacement text occupies, left of the two action cells.
    fn replace_text_rect(&self) -> Option<Rect> {
        let band = self.replace_band()?;
        let width = (band.width as i32 - 86).max(8);
        Some(Rect::new(band.x + 6, band.y, width as u32, band.height))
    }
}

/// Case- and word-aware, non-overlapping literal search.
///
/// A literal search is what a find bar offers: the regex-capable editor has its own matcher, and
/// a second engine here would be a second set of semantics to keep in step. Offsets are byte
/// offsets into `hay`, so a host can splice the ranges directly.
fn find_matches(
    hay: &str,
    hay_chars: &[(usize, char)],
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    if needle.is_empty() || hay.is_empty() {
        return Vec::new();
    }
    let needle_chars: Vec<char> = needle.chars().collect();
    let len = needle_chars.len();
    if len == 0 || hay_chars.len() < len {
        return Vec::new();
    }
    let same = |a: char, b: char| {
        if case_sensitive {
            a == b
        } else {
            a.to_lowercase().eq(b.to_lowercase())
        }
    };
    // Boyer-Moore-Horspool's bad-character table, keyed by the case-folded last character. A
    // colliding fold only makes the shift shorter than the true one, which is safe: the matcher
    // still verifies the whole candidate before reporting it.
    let fold = |ch: char| {
        if case_sensitive {
            ch
        } else {
            ch.to_lowercase().next().unwrap_or(ch)
        }
    };
    let mut skip: HashMap<char, usize> = HashMap::new();
    for (index, ch) in needle_chars[..len - 1].iter().enumerate() {
        skip.insert(fold(*ch), len - 1 - index);
    }
    let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut matches = Vec::new();
    let mut index = 0usize;
    while index + len <= hay_chars.len() {
        let last = hay_chars[index + len - 1].1;
        if same(last, needle_chars[len - 1])
            && (0..len - 1).all(|offset| same(hay_chars[index + offset].1, needle_chars[offset]))
        {
            let starts_word = !whole_word || index == 0 || !is_word(hay_chars[index - 1].1);
            let ends_word =
                !whole_word || index + len == hay_chars.len() || !is_word(hay_chars[index + len].1);
            if starts_word && ends_word {
                let start = hay_chars[index].0;
                let end = hay_chars.get(index + len).map_or(hay.len(), |(offset, _)| *offset);
                matches.push((start, end));
                index += len;
                continue;
            }
        }
        let step = skip.get(&fold(last)).copied().unwrap_or(len);
        index += step.max(1);
    }
    matches
}

impl Widget for FindBar {
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
        let height = if self.replace_visible {
            (FIND_ROW_HEIGHT * 2) as u32
        } else {
            FIND_ROW_HEIGHT as u32
        };
        crate::core::Size::new(360, height)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves the bar's two events; see `DiffViewer`'s note for why the arms are explicit.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "search_changed" => {
                Some(EventSignalRef::mapped("search_changed", &self.search_changed, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::UInt(v.0 as u64),
                        CapabilityValue::UInt(v.1 as u64),
                    ]))
                }))
            }
            "replace_requested" => {
                Some(EventSignalRef::mapped("replace_requested", &self.replace_requested, |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::Tuple(
                            v.ranges
                                .iter()
                                .map(|range| {
                                    CapabilityValue::Tuple(crate::compat::Vec::from([
                                        CapabilityValue::UInt(range.0 as u64),
                                        CapabilityValue::UInt(range.1 as u64),
                                    ]))
                                })
                                .collect(),
                        ),
                        CapabilityValue::String(v.text.clone()),
                    ]))
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for FindBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "case_sensitive" => Ok(CapabilityValue::Bool(self.case_sensitive)),
            "whole_word" => Ok(CapabilityValue::Bool(self.whole_word)),
            "replace_text" => Ok(CapabilityValue::String(self.replacement.clone())),
            "visible" => Ok(CapabilityValue::Bool(self.visible)),
            "replace_visible" => Ok(CapabilityValue::Bool(self.replace_visible)),
            "match_count" => Ok(CapabilityValue::UInt(self.matches.len() as u64)),
            "current_range" => Ok(match self.current_range() {
                Some((start, end)) => CapabilityValue::String(format!("{start},{end}")),
                None => CapabilityValue::Null,
            }),
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
            "whole_word" => {
                self.set_whole_word(expect_bool(value)?);
                Ok(())
            }
            "replace_text" => {
                self.set_replace_text(expect_string(value)?);
                Ok(())
            }
            "visible" => {
                if expect_bool(value)? {
                    self.open();
                } else {
                    self.close();
                }
                Ok(())
            }
            "replace_visible" => {
                self.set_replace_visible(expect_bool(value)?);
                Ok(())
            }
            "match_count" | "current_range" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "query",
            "case_sensitive",
            "whole_word",
            "replace_text",
            "visible",
            "replace_visible",
            "match_count",
            "current_range",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for FindBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.case_toggle_rect().contains_point(*pos) {
                    self.set_case_sensitive(!self.case_sensitive);
                    return;
                }
                if self.word_toggle_rect().contains_point(*pos) {
                    self.set_whole_word(!self.whole_word);
                    return;
                }
                if self.replace_button_rect().is_some_and(|cell| cell.contains_point(*pos)) {
                    self.replace();
                    return;
                }
                if self.replace_all_button_rect().is_some_and(|cell| cell.contains_point(*pos)) {
                    self.replace_all();
                    return;
                }
                if self.replace_band().is_some_and(|band| band.contains_point(*pos)) {
                    self.focus = FindFocus::Replace;
                    self.base.request_redraw();
                    return;
                }
                if self.query_band().contains_point(*pos) {
                    self.focus = FindFocus::Query;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                let target = match self.focus {
                    FindFocus::Query => &mut self.query,
                    FindFocus::Replace => &mut self.replacement,
                };
                target.extend(text.chars().filter(|character| !character.is_control()));
                if matches!(self.focus, FindFocus::Query) {
                    self.recompute_matches();
                }
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                match *key {
                    crate::event::key_codes::ESCAPE => self.close(),
                    crate::event::key_codes::TAB => {
                        if self.replace_visible {
                            self.focus = match self.focus {
                                FindFocus::Query => FindFocus::Replace,
                                FindFocus::Replace => FindFocus::Query,
                            };
                            self.base.request_redraw();
                        }
                    }
                    crate::event::key_codes::ENTER => {
                        if shift {
                            self.find_previous();
                        } else {
                            self.find_next();
                        }
                    }
                    crate::event::key_codes::BACKSPACE => {
                        let target = match self.focus {
                            FindFocus::Query => &mut self.query,
                            FindFocus::Replace => &mut self.replacement,
                        };
                        target.pop();
                        if matches!(self.focus, FindFocus::Query) {
                            self.recompute_matches();
                        }
                        self.base.request_redraw();
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => {}
        }
    }
}

impl Draw for FindBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(surface));
        let accent =
            crate::style::semantic_color(crate::style::SemanticColor::Info).unwrap_or(Color::BLUE);
        let font = effective_font(style);

        context.face_with_gradient_corners(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );

        let query_band = self.query_band();
        let text_rect = self.query_text_rect();
        let mut counter_cache = self.counter_cache.borrow_mut();
        let counter_key = (self.current.map_or(0, |index| index + 1), self.matches.len());
        if counter_cache.as_ref().is_none_or(|(key, _)| *key != counter_key) {
            let text = if self.matches.is_empty() {
                "0/0".to_string()
            } else {
                format!("{}/{}", counter_key.0, counter_key.1)
            };
            *counter_cache = Some((counter_key, text));
        }
        let counter = counter_cache.as_ref().map(|(_, text)| text.as_str()).unwrap_or_default();
        let counter_width = context.measure_text(counter, font).width as i32;
        context.draw_text(
            Point::new(text_rect.x, text_rect.y + 7),
            &crate::widget::text_utils::elide_end(
                &self.query,
                ((text_rect.width as i32 - counter_width - 8) / 7).max(1) as usize,
            ),
            font,
            if self.query.is_empty() { ink.with_alpha(120) } else { ink },
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(
                query_band.x + query_band.width as i32 - 2 * FIND_TOGGLE_SIDE - 12 - counter_width,
                query_band.y + 7,
            ),
            counter,
            font,
            ink.blend(&surface, 0.35),
            HorizontalAlignment::Left,
        );

        for (box_rect, label, active) in [
            (self.case_toggle_rect(), "Aa", self.case_sensitive),
            (self.word_toggle_rect(), "W", self.whole_word),
        ] {
            if active {
                context.fill_rounded_rect(box_rect, 3, surface.blend(&accent, 0.30));
                crate::widget::draw::stroke_surface_border(
                    context,
                    box_rect,
                    [3, 3, 3, 3],
                    accent,
                    1,
                );
            }
            context.draw_text(
                Point::new(box_rect.x + 4, box_rect.y + 3),
                label,
                font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        if let Some(band) = self.replace_band() {
            context.draw_line(
                Point::new(band.x, band.y),
                Point::new(band.x + band.width as i32, band.y),
                border,
            );
            if let Some(text_rect) = self.replace_text_rect() {
                context.draw_text(
                    Point::new(text_rect.x, text_rect.y + 7),
                    &crate::widget::text_utils::elide_end(
                        &self.replacement,
                        (text_rect.width as i32 / 7).max(1) as usize,
                    ),
                    font,
                    if self.replacement.is_empty() { ink.with_alpha(120) } else { ink },
                    HorizontalAlignment::Left,
                );
            }
            // The two replacement actions are controls on the bar, so a reader never has to know
            // that the host applies their request; the result count is the feedback that the
            // request reached the document.
            for (cell, label) in
                [(self.replace_button_rect(), "Repl"), (self.replace_all_button_rect(), "All")]
            {
                let Some(cell) = cell else {
                    continue;
                };
                let enabled = !self.matches.is_empty();
                context.fill_rounded_rect(
                    cell,
                    3,
                    if enabled { surface.blend(&accent, 0.25) } else { surface },
                );
                context.draw_rect(cell, if enabled { accent } else { border });
                context.draw_text_fitted_cow(
                    cell,
                    label,
                    font,
                    if enabled { ink } else { ink.with_alpha(120) },
                    HorizontalAlignment::Center,
                );
            }
            if self.last_replaced > 0 {
                let label = self.replaced_cache.1.as_str();
                context.draw_text(
                    Point::new(
                        band.x + band.width as i32
                            - 84
                            - context.measure_text(label, font).width as i32,
                        band.y + 7,
                    ),
                    label,
                    font,
                    ink.blend(&surface, 0.35),
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}
