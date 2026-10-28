//! `FindReplaceBar` — the shared find/replace strip for editors and lists (COMP-A31).
//!
//! # Why a companion
//!
//! A code editor, a log view and a table each grow the same strip: a query field, previous and
//! next buttons, a close button, and — when the host asks for it — a replacement field with
//! "Replace" and "All" actions. The strip owns no document. It reports the five user intents
//! ([`FindReplaceBar::find_next`], [`FindReplaceBar::find_previous`],
//! [`FindReplaceBar::replace_one`], [`FindReplaceBar::replace_all`] and
//! [`FindReplaceBar::closed`]) and the host applies them to whatever it is searching.
//!
//! The match counts are host-supplied. A find bar cannot know how many matches a regex has
//! without the document, so [`FindReplaceBar::set_match_count`] is the single place the host
//! reports both the total and the current index. This keeps the strip independent of the
//! search engine and lets the host update the counts after every navigation.

use crate::compat::{lock, Arc, Mutex, String};
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::key_codes::{BACKSPACE, ENTER, ESCAPE, LINE_FEED};
use crate::event::{Event, EventHandler};
use crate::impl_draw_bridge;
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{EventSignalRef, GenericSignal};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::helpers::{accent_color, surface_ink};

/// Inset around the strip's fields and buttons.
const BAR_INSET: u32 = 4;
/// Space between adjacent controls.
const BAR_GAP: u32 = 4;
/// Width of a square navigation button.
const NAV_BUTTON_WIDTH: u32 = 28;
/// Width of the "Replace" button.
const REPLACE_BUTTON_WIDTH: u32 = 76;
/// Width of the "All" button.
const REPLACE_ALL_BUTTON_WIDTH: u32 = 44;
/// Shift bit in an event modifier mask (the widget-layer convention).
const SHIFT_MODIFIER: u32 = crate::shortcut::Modifiers::SHIFT.bits() as u32;

/// Which text field receives keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarField {
    Query,
    Replacement,
}

/// A hit-tested part of the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BarHit {
    Query,
    Replacement,
    Previous,
    Next,
    ReplaceOne,
    ReplaceAll,
    Close,
}

/// The rectangles the strip paints and hit-tests.
#[derive(Debug, Clone, Copy)]
struct BarLayout {
    query: Rect,
    replacement: Option<Rect>,
    previous: Rect,
    next: Rect,
    close: Rect,
    replace_one: Option<Rect>,
    replace_all: Option<Rect>,
}

/// A find/replace strip that reports navigation and replacement intents.
///
/// The strip is hidden until [`FindReplaceBar::open`] and stops handling input after
/// [`FindReplaceBar::close`]. Keeping the hidden state in the control rather than in the host
/// means Escape has exactly one implementation.
pub struct FindReplaceBar {
    base: BaseWidget,
    query: String,
    replacement: String,
    match_case: bool,
    use_regex: bool,
    /// The reported `(total, current)` match counters, shared with a bound editor (COMP-A8).
    ///
    /// An editor attaches with `&FindReplaceBar` and cannot call [`Self::set_match_count`], which
    /// needs `&mut self`. The editor writes this cell, and the accessors read it, so the counts it
    /// pushes are visible immediately.
    match_counts: Arc<Mutex<(usize, usize)>>,
    /// The `(query, replacement, match_case, use_regex)` inputs a bound editor reads when it
    /// applies a reported intent.
    ///
    /// The find/navigation signals carry no payload, so this mirror is how the host learns what to
    /// search for. It is kept in step by the setters and by [`Self::handle_event`], which owns the
    /// keyboard edits to the two text buffers.
    context: Arc<Mutex<(String, String, bool, bool)>>,
    replace_visible: bool,
    open: bool,
    active_field: Option<BarField>,
    /// Emitted when the host should select the next match.
    pub find_next: GenericSignal,
    /// Emitted when the host should select the previous match.
    pub find_previous: GenericSignal,
    /// Emitted when the host should replace the current match.
    pub replace_one: GenericSignal,
    /// Emitted when the host should replace every match.
    pub replace_all: GenericSignal,
    /// Emitted once when the strip closes, whether by Escape or the close button.
    pub closed: GenericSignal,
}

impl FindReplaceBar {
    /// Creates a closed bar with empty query and replacement text.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SearchBar, geometry, "FindReplaceBar"),
            query: String::new(),
            replacement: String::new(),
            match_case: false,
            use_regex: false,
            match_counts: Arc::new(Mutex::new((0, 0))),
            context: Arc::new(Mutex::new((String::new(), String::new(), false, false))),
            replace_visible: false,
            open: false,
            active_field: None,
            find_next: GenericSignal::new(),
            find_previous: GenericSignal::new(),
            replace_one: GenericSignal::new(),
            replace_all: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    /// Shows the strip and focuses the query field.
    pub fn open(&mut self) {
        if self.open {
            return;
        }
        self.open = true;
        self.active_field = Some(BarField::Query);
        self.base.request_redraw();
    }

    /// Hides the strip and emits [`Self::closed`] once.
    ///
    /// A close while already closed is a no-op, so a host that forwards both Escape and a
    /// toolbar action cannot observe two close notices for one dismissal.
    pub fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.active_field = None;
        self.base.request_redraw();
        self.closed.emit();
    }

    /// Returns whether the strip is visible.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Replaces the query text.
    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();
        if self.query == query {
            return;
        }
        self.query = query;
        self.sync_context();
        self.base.request_redraw();
    }

    /// Replaces the replacement text.
    pub fn set_replacement(&mut self, replacement: impl Into<String>) {
        let replacement = replacement.into();
        if self.replacement == replacement {
            return;
        }
        self.replacement = replacement;
        self.sync_context();
        self.base.request_redraw();
    }

    /// Sets whether matching is case-sensitive.
    pub fn set_match_case(&mut self, match_case: bool) {
        if self.match_case == match_case {
            return;
        }
        self.match_case = match_case;
        self.sync_context();
        self.base.request_redraw();
    }

    /// Sets whether the query is interpreted as a regular expression.
    pub fn set_use_regex(&mut self, use_regex: bool) {
        if self.use_regex == use_regex {
            return;
        }
        self.use_regex = use_regex;
        self.sync_context();
        self.base.request_redraw();
    }

    /// Reports the total match count and the zero-based current match.
    ///
    /// The current index is clamped into the reported total; a total of zero resets it to
    /// zero. The total is stored as given, because "3 of 5" and "0 of 0" are both legitimate
    /// answers from a search that has finished.
    pub fn set_match_count(&mut self, match_count: usize, current_match: usize) {
        let current_match = if match_count == 0 { 0 } else { current_match.min(match_count - 1) };
        {
            let mut counts = lock(&self.match_counts);
            if *counts == (match_count, current_match) {
                return;
            }
            *counts = (match_count, current_match);
        }
        self.base.request_redraw();
    }

    /// The shared match counters a bound editor writes, plus the signal that repaints this strip.
    ///
    /// The pair is `(total, current)` with `current` zero-based. A writer emits the returned signal
    /// so a reported count becomes visible without the host forwarding a frame.
    pub fn match_counts_handle(&self) -> (Arc<Mutex<(usize, usize)>>, GenericSignal) {
        (Arc::clone(&self.match_counts), self.base.redraw_requested_signal().clone())
    }

    /// The shared `(query, replacement, match_case, use_regex)` inputs a bound editor reads.
    ///
    /// The navigation signals carry no payload, so this is how an attached editor learns what to
    /// search for. It reflects the values in force when the signal was emitted.
    pub fn context_handle(&self) -> Arc<Mutex<(String, String, bool, bool)>> {
        Arc::clone(&self.context)
    }

    /// Mirrors the two text buffers and the two match policies into the shared cell.
    fn sync_context(&self) {
        let mut context = lock(&self.context);
        *context = (self.query.clone(), self.replacement.clone(), self.match_case, self.use_regex);
    }

    /// Shows or hides the replacement row.
    pub fn set_replace_visible(&mut self, visible: bool) {
        if self.replace_visible == visible {
            return;
        }
        self.replace_visible = visible;
        if !visible && self.active_field == Some(BarField::Replacement) {
            self.active_field = Some(BarField::Query);
        }
        self.base.request_redraw();
    }

    /// Returns the query text.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the replacement text.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// Returns whether matching is case-sensitive.
    pub fn match_case(&self) -> bool {
        self.match_case
    }

    /// Returns whether the query is a regular expression.
    pub fn use_regex(&self) -> bool {
        self.use_regex
    }

    /// Returns the reported total match count.
    pub fn match_count(&self) -> usize {
        lock(&self.match_counts).0
    }

    /// Returns the zero-based current match.
    pub fn current_match(&self) -> usize {
        lock(&self.match_counts).1
    }

    /// Returns whether the replacement row is shown.
    pub fn replace_visible(&self) -> bool {
        self.replace_visible
    }

    /// Computes the rectangles shared by painting and hit testing.
    fn layout(&self) -> BarLayout {
        let rect = self.geometry();
        let inset = BAR_INSET as i32;
        let gap = BAR_GAP as i32;

        let top_band = if self.replace_visible { rect.height / 2 } else { rect.height };
        let top_height = top_band.saturating_sub(BAR_INSET * 2).max(1);
        let top_y = rect.y.saturating_add(inset);

        let close = Rect::new(
            rect.right().saturating_sub(inset + NAV_BUTTON_WIDTH as i32),
            top_y,
            NAV_BUTTON_WIDTH,
            top_height,
        );
        let next = Rect::new(
            close.x.saturating_sub(gap + NAV_BUTTON_WIDTH as i32),
            top_y,
            NAV_BUTTON_WIDTH,
            top_height,
        );
        let previous = Rect::new(
            next.x.saturating_sub(gap + NAV_BUTTON_WIDTH as i32),
            top_y,
            NAV_BUTTON_WIDTH,
            top_height,
        );
        let query_width =
            previous.x.saturating_sub(gap).saturating_sub(rect.x.saturating_add(inset)).max(0)
                as u32;
        let query = Rect::new(rect.x.saturating_add(inset), top_y, query_width, top_height);

        if !self.replace_visible {
            return BarLayout {
                query,
                replacement: None,
                previous,
                next,
                close,
                replace_one: None,
                replace_all: None,
            };
        }

        let bottom_band = rect.height.saturating_sub(top_band);
        let bottom_height = bottom_band.saturating_sub(BAR_INSET * 2).max(1);
        let bottom_y = rect.y.saturating_add(top_band as i32).saturating_add(inset);

        let replace_all = Rect::new(
            rect.right().saturating_sub(inset + REPLACE_ALL_BUTTON_WIDTH as i32),
            bottom_y,
            REPLACE_ALL_BUTTON_WIDTH,
            bottom_height,
        );
        let replace_one = Rect::new(
            replace_all.x.saturating_sub(gap + REPLACE_BUTTON_WIDTH as i32),
            bottom_y,
            REPLACE_BUTTON_WIDTH,
            bottom_height,
        );
        let replacement_width =
            replace_one.x.saturating_sub(gap).saturating_sub(rect.x.saturating_add(inset)).max(0)
                as u32;
        let replacement =
            Rect::new(rect.x.saturating_add(inset), bottom_y, replacement_width, bottom_height);

        BarLayout {
            query,
            replacement: Some(replacement),
            previous,
            next,
            close,
            replace_one: Some(replace_one),
            replace_all: Some(replace_all),
        }
    }

    /// The control under `pos`, or `None` for empty space or a closed strip.
    fn hit_test(&self, pos: Point) -> Option<BarHit> {
        if !self.open || !self.geometry().contains_point(pos) {
            return None;
        }
        let layout = self.layout();
        if layout.close.contains_point(pos) {
            return Some(BarHit::Close);
        }
        if layout.next.contains_point(pos) {
            return Some(BarHit::Next);
        }
        if layout.previous.contains_point(pos) {
            return Some(BarHit::Previous);
        }
        if let Some(rect) = layout.replace_one {
            if rect.contains_point(pos) {
                return Some(BarHit::ReplaceOne);
            }
        }
        if let Some(rect) = layout.replace_all {
            if rect.contains_point(pos) {
                return Some(BarHit::ReplaceAll);
            }
        }
        if layout.query.contains_point(pos) {
            return Some(BarHit::Query);
        }
        if let Some(rect) = layout.replacement {
            if rect.contains_point(pos) {
                return Some(BarHit::Replacement);
            }
        }
        None
    }

    /// The buffer the keyboard currently writes to.
    fn active_text_mut(&mut self) -> Option<&mut String> {
        match self.active_field {
            Some(BarField::Query) => Some(&mut self.query),
            Some(BarField::Replacement) => Some(&mut self.replacement),
            None => None,
        }
    }
}

impl EventHandler for FindReplaceBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.open {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                let hit = self.hit_test(*pos);
                match hit {
                    Some(BarHit::Query) => self.active_field = Some(BarField::Query),
                    Some(BarHit::Replacement) => self.active_field = Some(BarField::Replacement),
                    Some(BarHit::Previous) => self.find_previous.emit(),
                    Some(BarHit::Next) => self.find_next.emit(),
                    Some(BarHit::ReplaceOne) => self.replace_one.emit(),
                    Some(BarHit::ReplaceAll) => self.replace_all.emit(),
                    Some(BarHit::Close) => self.close(),
                    None => {}
                }
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => {
                if *key == ENTER || *key == LINE_FEED {
                    if *modifiers & SHIFT_MODIFIER != 0 {
                        self.find_previous.emit();
                    } else {
                        self.find_next.emit();
                    }
                } else if *key == ESCAPE {
                    self.close();
                } else if *key == BACKSPACE {
                    if let Some(text) = self.active_text_mut() {
                        text.pop();
                        self.base.request_redraw();
                    }
                }
            }
            Event::TextInput { text } => {
                if let Some(buffer) = self.active_text_mut() {
                    buffer.push_str(text);
                    self.base.request_redraw();
                }
            }
            _ => {}
        }
        // The keyboard edits above write the buffers directly, so the shared mirror is refreshed
        // after every event rather than only in the `&mut self` setters.
        self.sync_context();
    }
}

/// Paints one text field.
///
/// Nine arguments is the honest shape here: a field is painted from its rectangle, its text, its
/// placeholder, its focus state and four resolved theme colours, and bundling them into a struct
/// would add a type with no other user. The same allowance `android/platform_impl.rs` records for
/// its multi-argument creators.
#[allow(clippy::too_many_arguments)]
fn paint_field(
    context: &mut RenderContext,
    rect: Rect,
    text: &str,
    placeholder: &str,
    focused: bool,
    font: &Font,
    surface: Color,
    ink: Color,
    accent: Color,
) {
    context.fill_rect(rect, surface.blend(&ink, 0.05));
    context.draw_rect(rect, if focused { accent } else { surface.blend(&ink, 0.22) });
    let (label, color) =
        if text.is_empty() { (placeholder, surface.blend(&ink, 0.50)) } else { (text, ink) };
    let inner =
        Rect::new(rect.x.saturating_add(6), rect.y, rect.width.saturating_sub(12), rect.height);
    context.draw_text_fitted_cow(inner, label, font, color, HorizontalAlignment::Left);
}

/// Paints one button.
fn paint_button(
    context: &mut RenderContext,
    rect: Rect,
    label: &str,
    font: &Font,
    surface: Color,
    ink: Color,
) {
    context.fill_rounded_rect(rect, 3, surface.blend(&ink, 0.08));
    context.draw_text_fitted_cow(rect, label, font, ink, HorizontalAlignment::Center);
}

impl Draw for FindReplaceBar {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }
        let rect = self.geometry();
        let style = self.style();
        let (surface, ink) = surface_ink(style);
        let accent = accent_color();
        let font = effective_font(style);
        let layout = self.layout();

        context.fill_rect(rect, surface);
        context.draw_rect(rect, surface.blend(&ink, 0.18));

        paint_field(
            context,
            layout.query,
            &self.query,
            "Find",
            self.active_field == Some(BarField::Query),
            font,
            surface,
            ink,
            accent,
        );
        paint_button(context, layout.previous, "<", font, surface, ink);
        paint_button(context, layout.next, ">", font, surface, ink);
        paint_button(context, layout.close, "x", font, surface, ink);

        if let Some(replacement) = layout.replacement {
            paint_field(
                context,
                replacement,
                &self.replacement,
                "Replace with",
                self.active_field == Some(BarField::Replacement),
                font,
                surface,
                ink,
                accent,
            );
        }
        if let Some(button) = layout.replace_one {
            paint_button(context, button, "Replace", font, surface, ink);
        }
        if let Some(button) = layout.replace_all {
            paint_button(context, button, "All", font, surface, ink);
        }
    }
}

impl Widget for FindReplaceBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        crate::widget::metrics::ControlMetrics::implicit_size(
            Size::new(0, 0),
            crate::style::EdgeOffsets::all(0),
            Size::new(420, if self.replace_visible { 64 } else { 32 }),
        )
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "find_next" => Some(EventSignalRef::unit("find_next", &self.find_next)),
            "find_previous" => Some(EventSignalRef::unit("find_previous", &self.find_previous)),
            "replace_one" => Some(EventSignalRef::unit("replace_one", &self.replace_one)),
            "replace_all" => Some(EventSignalRef::unit("replace_all", &self.replace_all)),
            "closed" => Some(EventSignalRef::unit("closed", &self.closed)),
            _ => None,
        }
    }
}

impl WidgetProperties for FindReplaceBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "query" => Ok(CapabilityValue::String(self.query.clone())),
            "replacement" => Ok(CapabilityValue::String(self.replacement.clone())),
            "match_case" => Ok(CapabilityValue::Bool(self.match_case)),
            "use_regex" => Ok(CapabilityValue::Bool(self.use_regex)),
            "match_count" => Ok(CapabilityValue::UInt(self.match_count() as u64)),
            "current_match" => Ok(CapabilityValue::UInt(self.current_match() as u64)),
            "replace_visible" => Ok(CapabilityValue::Bool(self.replace_visible)),
            "open" => Ok(CapabilityValue::Bool(self.open)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "query" => {
                self.set_query(expect_string(value)?);
                Ok(())
            }
            "replacement" => {
                self.set_replacement(expect_string(value)?);
                Ok(())
            }
            "match_case" => {
                self.set_match_case(expect_bool(value)?);
                Ok(())
            }
            "use_regex" => {
                self.set_use_regex(expect_bool(value)?);
                Ok(())
            }
            "match_count" => {
                let count = expect_usize(value)?;
                self.set_match_count(count, self.current_match());
                Ok(())
            }
            "current_match" => {
                let current = expect_usize(value)?;
                self.set_match_count(self.match_count(), current);
                Ok(())
            }
            "replace_visible" => {
                self.set_replace_visible(expect_bool(value)?);
                Ok(())
            }
            "open" => {
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
            "replacement",
            "match_case",
            "use_regex",
            "match_count",
            "current_match",
            "replace_visible",
            "open",
            BASE_PROPERTY_NAMES
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Arc;
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Counts the emissions of a unit signal.
    fn count_units(signal: &GenericSignal) -> Arc<AtomicUsize> {
        let count = Arc::new(AtomicUsize::new(0));
        let handle = Arc::clone(&count);
        signal.connect(move || {
            handle.fetch_add(1, Ordering::SeqCst);
        });
        count
    }

    fn unit_count(count: &Arc<AtomicUsize>) -> usize {
        count.load(Ordering::SeqCst)
    }

    fn key(key: u32, modifiers: u32) -> Event {
        Event::KeyPress { key, modifiers }
    }

    #[test]
    fn open_and_close_are_edge_triggered() {
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        let closed = count_units(&bar.closed);
        assert!(!bar.is_open());

        bar.open();
        assert!(bar.is_open());
        bar.close();
        assert!(!bar.is_open());
        bar.close();
        assert_eq!(unit_count(&closed), 1, "a second close is a no-op");
    }

    #[test]
    fn enter_searches_forward_and_shift_enter_searches_backward() {
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.open();
        let next = count_units(&bar.find_next);
        let previous = count_units(&bar.find_previous);

        bar.handle_event(&key(ENTER, 0));
        assert_eq!(unit_count(&next), 1);
        assert_eq!(unit_count(&previous), 0);

        bar.handle_event(&key(ENTER, SHIFT_MODIFIER));
        assert_eq!(unit_count(&next), 1);
        assert_eq!(unit_count(&previous), 1);

        bar.handle_event(&key(LINE_FEED, 0));
        assert_eq!(unit_count(&next), 2, "the keypad Enter is the same intent");
    }

    #[test]
    fn escape_closes_the_bar_and_emits_closed_once() {
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.open();
        let closed = count_units(&bar.closed);

        bar.handle_event(&key(ESCAPE, 0));
        assert!(!bar.is_open());
        assert_eq!(unit_count(&closed), 1);

        // Input after the close is ignored rather than reported by a hidden strip.
        bar.handle_event(&key(ENTER, 0));
        assert_eq!(unit_count(&closed), 1);
    }

    #[test]
    fn match_counts_round_trip_and_are_clamped() {
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
        bar.set_match_count(5, 2);
        assert_eq!((bar.match_count(), bar.current_match()), (5, 2));

        bar.set_match_count(2, 9);
        assert_eq!((bar.match_count(), bar.current_match()), (2, 1));

        bar.set_match_count(0, 4);
        assert_eq!((bar.match_count(), bar.current_match()), (0, 0));

        bar.set_query("needle");
        bar.set_replacement("thread");
        bar.set_match_case(true);
        bar.set_use_regex(true);
        bar.set_match_count(3, 1);
        assert_eq!(bar.get("query"), Ok(CapabilityValue::String("needle".to_string())));
        assert_eq!(bar.get("replacement"), Ok(CapabilityValue::String("thread".to_string())));
        assert_eq!(bar.get("match_case"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(bar.get("use_regex"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(bar.get("match_count"), Ok(CapabilityValue::UInt(3)));
        assert_eq!(bar.get("current_match"), Ok(CapabilityValue::UInt(1)));
        assert!(bar.property_names().contains(&"open"));
    }

    #[test]
    fn buttons_are_hit_tested_and_emit_their_intent() {
        let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 64));
        bar.open();
        bar.set_replace_visible(true);
        let next = count_units(&bar.find_next);
        let previous = count_units(&bar.find_previous);
        let replace_one = count_units(&bar.replace_one);
        let replace_all = count_units(&bar.replace_all);
        let closed = count_units(&bar.closed);

        // Top row: previous 304..332, next 336..364, close 368..396.
        bar.handle_event(&Event::MousePress { pos: Point::new(310, 10), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MousePress { pos: Point::new(340, 10), button: 1, modifiers: 0 });
        // Bottom row: Replace 272..348, All 352..396.
        bar.handle_event(&Event::MousePress { pos: Point::new(280, 40), button: 1, modifiers: 0 });
        bar.handle_event(&Event::MousePress { pos: Point::new(360, 40), button: 1, modifiers: 0 });

        assert_eq!(unit_count(&previous), 1);
        assert_eq!(unit_count(&next), 1);
        assert_eq!(unit_count(&replace_one), 1);
        assert_eq!(unit_count(&replace_all), 1);

        bar.handle_event(&Event::MousePress { pos: Point::new(380, 10), button: 1, modifiers: 0 });
        assert_eq!(unit_count(&closed), 1);
        assert!(!bar.is_open());
    }
}
