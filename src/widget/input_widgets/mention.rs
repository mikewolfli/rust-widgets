// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mention — `@`-triggered candidate completion inside free text.
//!
//! # Why this is not an `AutoCompleteEdit` mode
//!
//! `AutoCompleteEdit` matches its suggestions against **the whole text**
//! (`auto_complete_edit.rs`: `text: String`, `filter_suggestions()` over the entire
//! field) and replaces the whole value on selection. A mention is a different
//! model:
//!
//! | | `AutoCompleteEdit` | `Mention` |
//! |---|---|---|
//! | matched against | the whole field text | **the token before the caret** |
//! | trigger | typing anything | **one trigger character** (`@`) |
//! | what a choice replaces | the entire text | **only the active token** |
//! | caret after a choice | end of the field | **after the inserted mention** |
//! | more than one | impossible | **many mentions in one field** |
//!
//! The last row is the one that makes them genuinely different types: a mention
//! field holds *several* completed references plus surrounding prose, so the value
//! is not "the text" but a text with recognised spans inside it. Trying to express
//! that through one `text: String` and a whole-field replacement would mean the
//! caller re-implementing the token logic outside the control.
//!
//! # What it shares rather than reimplements
//!
//! The suggestion list, the highlighted index, the up/down/enter/Escape keyboard
//! contract and the popup's drawing follow `AutoCompleteEdit`'s conventions, so a
//! caller switching between them does not relearn the interaction.

//! # Reachability
//!
//! Registered in the widget factory as `mention` (aliases `mention_edit`,
//! `at_mention`), so it is reachable by name from the declarative JSON path
//! (`"mention"`), from a CSS selector (`Mention`), and through the typed
//! `create_mention` on `ControlBackend`.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A person who can be mentioned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionCandidate {
    /// Stable identifier; what a completed mention resolves to.
    pub id: String,
    /// The text drawn in the candidate list and inserted into the field.
    pub display: String,
    /// An optional secondary line, e.g. a role or address.
    pub description: String,
}

impl MentionCandidate {
    /// Creates a candidate with `id` and `display`, and no description.
    pub fn new(id: impl Into<String>, display: impl Into<String>) -> Self {
        Self { id: id.into(), display: display.into(), description: String::new() }
    }

    /// Sets the secondary line.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Whether this candidate matches the typed query, case-insensitively.
    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        self.display.to_lowercase().contains(&query)
            || self.id.to_lowercase().contains(&query)
            || self.description.to_lowercase().contains(&query)
    }
}

/// One completed mention inside the field's text.
///
/// Recorded so a caller can read back *which* references the text contains without
/// re-parsing it — and so the control can keep the spans consistent when the text
/// is edited around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedMention {
    /// The candidate's id.
    pub id: String,
    /// Byte offset in [`Mention::text`] where the mention starts.
    pub start: usize,
    /// Byte offset just past the mention.
    pub end: usize,
}

/// The height of one candidate row.
const ROW_HEIGHT: u32 = 24;

/// How many candidates the popup shows at once.
const MAX_VISIBLE: usize = 6;

/// The height the caret is drawn with, and its x offset inside the field.
const CARET_HEIGHT: u32 = 16;

/// Mention — `@`-triggered completion inside free text.
pub struct Mention {
    base: BaseWidget,
    /// The field's text.
    text: String,
    /// The caret's byte offset into `text`.
    caret: usize,
    /// The character that starts a mention.
    trigger: char,
    /// Every candidate, in declaration order.
    candidates: Vec<MentionCandidate>,
    /// Candidate indices the popup is currently offering, in order.
    filtered: Vec<usize>,
    /// The highlighted candidate, as an index into `filtered`.
    highlighted: Option<usize>,
    /// The byte offset of the trigger that opened the current mention.
    ///
    /// Retained rather than recomputed so the replacement span is exactly what the
    /// user is looking at, even if the caret has moved within the token.
    trigger_offset: Option<usize>,
    /// The mentions currently present in `text`.
    completed: Vec<CompletedMention>,
    /// Emitted when text changes, with the new text.
    pub text_changed: Signal1<String>,
    /// Emitted when a mention is completed, with the candidate's id.
    pub mention_inserted: Signal1<String>,
    /// Emitted when the popup opens or closes, with the new state.
    pub popup_toggled: Signal1<bool>,
}

impl Mention {
    /// Creates an empty mention field.
    ///
    /// Defaults: empty text, caret at 0, the `@` trigger, no candidates, popup closed.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Mention, geometry, "Mention"),
            text: String::new(),
            caret: 0,
            trigger: '@',
            candidates: Vec::new(),
            filtered: Vec::new(),
            highlighted: None,
            trigger_offset: None,
            completed: Vec::new(),
            text_changed: Signal1::new(),
            mention_inserted: Signal1::new(),
            popup_toggled: Signal1::new(),
        }
    }

    /// The field's text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret's byte offset.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The trigger character.
    pub fn trigger(&self) -> char {
        self.trigger
    }

    /// Sets the trigger character.
    ///
    /// A whitespace trigger is refused: the token walker stops at whitespace, so a
    /// space could never start a token and the popup would never open — a trigger
    /// that cannot trigger is worse than a rejected call.
    pub fn set_trigger(&mut self, trigger: char) -> bool {
        if trigger.is_whitespace() {
            return false;
        }
        self.trigger = trigger;
        self.refresh();
        self.base.request_redraw();
        true
    }

    /// Replaces the candidate list.
    ///
    /// `completed` is re-derived from the new list, so a mention whose candidate no
    /// longer exists stops being reported as a mention rather than lingering as an
    /// id nothing can resolve.
    pub fn set_candidates(&mut self, candidates: Vec<MentionCandidate>) {
        self.candidates = candidates;
        self.refresh();
        let known: Vec<&str> = self.candidates.iter().map(|c| c.id.as_str()).collect();
        self.completed.retain(|mention| known.contains(&mention.id.as_str()));
        self.base.request_redraw();
    }

    /// The candidate list.
    pub fn candidates(&self) -> &[MentionCandidate] {
        &self.candidates
    }

    /// The mentions present in the text.
    pub fn completed_mentions(&self) -> &[CompletedMention] {
        &self.completed
    }

    /// Whether the candidate popup is showing.
    pub fn is_popup_open(&self) -> bool {
        self.trigger_offset.is_some()
    }

    /// The candidates the popup is offering.
    pub fn visible_candidates(&self) -> Vec<&MentionCandidate> {
        self.filtered.iter().filter_map(|index| self.candidates.get(*index)).collect()
    }

    /// The highlighted candidate's index within [`Self::visible_candidates`].
    pub fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }

    /// The query currently being completed, i.e. the text after the trigger.
    ///
    /// `None` when no popup is open, so a caller cannot mistake the whole field for
    /// a query.
    pub fn query(&self) -> Option<&str> {
        let start = self.trigger_offset?;
        // The token runs from just past the trigger to the caret.
        self.text.get(start + self.trigger.len_utf8()..self.caret)
    }

    /// Replaces the text and re-derives the mention state.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.caret = self.text.len();
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Inserts `ch` at the caret and moves the caret past it.
    pub fn insert_char(&mut self, ch: char) {
        if self.caret > self.text.len() || !self.text.is_char_boundary(self.caret) {
            // A caret that is not on a character boundary cannot be spliced; moving it
            // to the end is the only safe repair, and it is reported rather than done
            // silently by a panic.
            self.caret = self.text.len();
        }
        self.text.insert(self.caret, ch);
        self.caret += ch.len_utf8();
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Removes the character before the caret.
    ///
    /// Returns whether anything was removed.
    pub fn backspace(&mut self) -> bool {
        if self.caret == 0 || !self.text.is_char_boundary(self.caret) {
            return false;
        }
        let previous = self.previous_char_boundary(self.caret);
        self.text.replace_range(previous..self.caret, "");
        self.caret = previous;
        self.refresh();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// The byte offset of the character boundary before `index`.
    fn previous_char_boundary(&self, index: usize) -> usize {
        let mut cursor = index.saturating_sub(1);
        while cursor > 0 && !self.text.is_char_boundary(cursor) {
            cursor -= 1;
        }
        cursor
    }

    /// Moves the caret one character left.
    pub fn move_caret_left(&mut self) {
        if self.caret > 0 {
            self.caret = self.previous_char_boundary(self.caret);
            self.refresh();
            self.base.request_redraw();
        }
    }

    /// Moves the caret one character right.
    pub fn move_caret_right(&mut self) {
        if self.caret < self.text.len() {
            let mut cursor = self.caret + 1;
            while cursor < self.text.len() && !self.text.is_char_boundary(cursor) {
                cursor += 1;
            }
            self.caret = cursor;
            self.refresh();
            self.base.request_redraw();
        }
    }

    /// Completes the mention with `candidate_index`, replacing the active token.
    ///
    /// Returns the candidate's id. When no popup is open, or the index names no
    /// visible candidate, nothing changes and `None` is returned — a completion the
    /// user did not ask for would corrupt prose.
    pub fn complete(&mut self, candidate_index: usize) -> Option<String> {
        let start = self.trigger_offset?;
        let candidate_id = {
            let index = *self.filtered.get(candidate_index)?;
            self.candidates.get(index)?.id.clone()
        };
        let display = self
            .filtered
            .get(candidate_index)
            .and_then(|index| self.candidates.get(*index))
            .map(|candidate| candidate.display.clone())?;

        // Replace from the trigger to the caret: the trigger itself is consumed, which
        // is the convention every mention UI uses (`@al` becomes `@Alice`, not
        // `@@Alice`).
        let replaced = format!("{}{}", self.trigger, display);
        self.text.replace_range(start..self.caret, &replaced);
        self.caret = start + replaced.len();
        self.trigger_offset = None;
        self.filtered.clear();
        self.highlighted = None;
        self.rebuild_completed();
        self.text_changed.emit(self.text.clone());
        self.mention_inserted.emit(candidate_id.clone());
        self.popup_toggled.emit(false);
        self.base.request_redraw();
        Some(candidate_id)
    }

    /// Opens the popup at the caret, as typing the trigger does.
    ///
    /// Returns whether it opened. A caller with a toolbar button needs the same
    /// entry point the keyboard has.
    pub fn open_popup(&mut self) -> bool {
        self.trigger_offset = None;
        // Insert the trigger, then treat it exactly as typed input would.
        self.insert_char(self.trigger);
        self.is_popup_open()
    }

    /// Closes the popup without completing.
    pub fn close_popup(&mut self) {
        if self.trigger_offset.take().is_some() {
            self.filtered.clear();
            self.highlighted = None;
            self.popup_toggled.emit(false);
            self.base.request_redraw();
        }
    }

    /// Moves the popup's highlight by `delta`, wrapping at both ends.
    fn move_highlight(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            self.highlighted = None;
            return;
        }
        let count = self.filtered.len() as i32;
        let next = match self.highlighted {
            Some(current) => (current as i32 + delta).rem_euclid(count),
            // Nothing highlighted yet: a forward move enters at the start, a backward
            // one at the end. Offsetting from an assumed 0 would skip the first row.
            None if delta >= 0 => 0,
            None => count - 1,
        };
        self.highlighted = Some(next as usize);
        self.base.request_redraw();
    }

    /// Re-derives the active token, the visible candidates and the completed mentions.
    ///
    /// The single place that reads the text, so the popup's contents, the
    /// replacement span and the reported mentions cannot disagree.
    ///
    /// `popup_toggled` fires only on an actual **change** of openness, so a caller
    /// can drive a show/hide animation from it. Emitting on every keystroke while
    /// the popup stayed open would make the signal a keystroke counter rather than a
    /// state-change notification.
    fn refresh(&mut self) {
        self.completed.clear();
        let was_open = self.trigger_offset.is_some();
        let token = self.active_token();
        match token {
            Some((trigger_offset, query_start)) => {
                self.trigger_offset = Some(trigger_offset);
                let query = self.text.get(query_start..self.caret).unwrap_or("");
                self.filtered = self
                    .candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| candidate.matches(query))
                    .map(|(index, _)| index)
                    .collect();
                if self.filtered.is_empty() {
                    // An empty popup reads as a broken control, so it closes — and
                    // that is a state change, reported as one.
                    self.trigger_offset = None;
                    self.highlighted = None;
                } else {
                    // The highlight resets to the first candidate on every edit, so it
                    // always points at the top of what the user currently sees.
                    self.highlighted = Some(0);
                }
            }
            None => {
                self.trigger_offset = None;
                self.filtered.clear();
                self.highlighted = None;
            }
        }
        let is_open = self.trigger_offset.is_some();
        if is_open != was_open {
            self.popup_toggled.emit(is_open);
        }
        self.rebuild_completed();
    }

    /// The trigger that opens the token containing the caret, if any.
    ///
    /// Returns `(trigger_offset, query_start)`. Walks backwards from the caret to the
    /// nearest trigger, stopping at whitespace: a token cannot contain a space, so a
    /// trigger before one is a *finished* mention and must not re-open the popup.
    fn active_token(&self) -> Option<(usize, usize)> {
        let before_caret = self.text.get(..self.caret)?;
        // Find the last trigger before the caret.
        let trigger_offset = before_caret.rfind(self.trigger)?;
        let after_trigger = &before_caret[trigger_offset + self.trigger.len_utf8()..];
        // A whitespace between the trigger and the caret ends the token.
        if after_trigger.contains(char::is_whitespace) {
            return None;
        }
        // A trigger that is itself preceded by a non-space, non-trigger character
        // would make `foo@bar` a mention, which is an email address, not a mention.
        // Only a trigger at the start or after whitespace opens the popup.
        if trigger_offset > 0 {
            let preceding = before_caret[..trigger_offset].chars().next_back();
            if !preceding.is_some_and(char::is_whitespace) {
                return None;
            }
        }
        Some((trigger_offset, trigger_offset + self.trigger.len_utf8()))
    }

    /// Rebuilds the completed-mention list from the text.
    ///
    /// Scans for `trigger` + a display string that matches a known candidate, which
    /// is what keeps the reported mentions true after the text is edited around them
    /// rather than only when a completion was performed.
    fn rebuild_completed(&mut self) {
        self.completed.clear();
        let mut search_from = 0usize;
        while let Some(relative) = self.text[search_from..].find(self.trigger) {
            let start = search_from + relative;
            // Only a trigger at the start of the text or after whitespace can begin a
            // mention; this is the same rule `active_token` applies.
            if start > 0 {
                let preceding = self.text[..start].chars().next_back();
                if !preceding.is_some_and(char::is_whitespace) {
                    search_from = start + self.trigger.len_utf8();
                    continue;
                }
            }
            let after = &self.text[start + self.trigger.len_utf8()..];
            // The longest matching display wins, so two candidates whose displays
            // share a prefix resolve to the one the text actually contains.
            let mut best: Option<(&MentionCandidate, usize)> = None;
            for candidate in &self.candidates {
                if after.starts_with(&candidate.display) {
                    let length = candidate.display.len();
                    if best.is_none_or(|(_, best_length)| length > best_length) {
                        best = Some((candidate, length));
                    }
                }
            }
            match best {
                Some((candidate, length)) => {
                    let end = start + self.trigger.len_utf8() + length;
                    self.completed.push(CompletedMention { id: candidate.id.clone(), start, end });
                    search_from = end;
                }
                // A trigger with no matching candidate is ordinary text (an email, a
                // handle the caller does not know); skipping one character avoids an
                // infinite loop without pretending it is a mention.
                None => search_from = start + self.trigger.len_utf8(),
            }
        }
    }

    /// The rectangle of the popup row at `index`.
    fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.filtered.len() {
            return None;
        }
        let rect = self.geometry();
        Some(Rect::new(
            rect.x,
            rect.y + rect.height as i32 + (index as u32 * ROW_HEIGHT) as i32,
            rect.width,
            ROW_HEIGHT,
        ))
    }

    /// The popup index under `pos`, when the pointer is over a row.
    fn row_at(&self, pos: Point) -> Option<usize> {
        let rows = self.filtered.len().min(MAX_VISIBLE);
        (0..rows).find(|index| self.row_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }

    /// The x offset of the caret, in pixels, drawn from the field's left padding.
    ///
    /// Measured by the caller's context rather than computed here, because glyph
    /// widths are a font fact this module does not own.
    fn caret_x(&self, context: &RenderContext) -> i32 {
        let before = self.text.get(..self.caret).unwrap_or("");
        let font = Font::simple("Sans", 12.0);
        context.measure_text(before, &font).width as i32
    }
}

impl Widget for Mention {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(240, 30)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Mention`'s property contract.
///
/// The candidates and the text are written through `set_candidates` / `set_text`;
/// the property layer reports the derived counts and the editable scalars, following
/// the same convention as the other list-valued controls.
impl WidgetProperties for Mention {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "trigger" => Ok(CapabilityValue::String(self.trigger().to_string())),
            "candidate_count" => Ok(CapabilityValue::UInt(self.candidates().len() as u64)),
            "visible_candidate_count" => {
                Ok(CapabilityValue::UInt(self.visible_candidates().len() as u64))
            }
            "mention_count" => Ok(CapabilityValue::UInt(self.completed_mentions().len() as u64)),
            "popup_open" => Ok(CapabilityValue::Bool(self.is_popup_open())),
            "caret" => Ok(CapabilityValue::UInt(self.caret() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                let text = expect_string(value)?;
                self.set_text(text);
                Ok(())
            }
            "trigger" => {
                let text = expect_string(value)?;
                let mut chars = text.chars();
                let (Some(trigger), None) = (chars.next(), chars.next()) else {
                    // A trigger is one character; accepting a longer string would make
                    // "which character fired" unanswerable.
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                if self.set_trigger(trigger) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::TypeMismatch)
                }
            }
            "popup_open" => {
                // Read-only in spirit: opening without a typed trigger would show a
                // popup whose query does not exist. `open_popup` is the real entry.
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            // Derived from the text and the candidate list.
            "candidate_count" | "visible_candidate_count" | "mention_count" | "caret" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `MENTION_PROPERTIES`.
        property_names_of![
            "text",
            "trigger",
            "candidate_count",
            "visible_candidate_count",
            "mention_count",
            "popup_open",
            "caret",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `mention` publishes.
    ///
    /// `open_popup` is the zero-argument action and goes through the control's own
    /// method, which inserts the trigger and runs the same path typed input would. On a
    /// fresh control that leaves the popup open, and answers `Ok(())`.
    ///
    /// `complete` picks a candidate by index, so a bare command has no candidate to
    /// commit; `set_candidates` and `set_trigger` assign state and need a payload. All
    /// three are answered through the property route — `candidate_count` and `trigger`
    /// are published scalars, and completion is driven by the popup's own selection.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "open_popup" => {
                self.open_popup();
                Ok(())
            }
            "set_candidates" | "set_trigger" | "complete" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Mention {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.style();
        let background = style.background_color.unwrap_or(Color::WHITE);
        let border = style.border_color.unwrap_or(Color::rgb(190, 192, 198));
        let text_color = style.text_color.unwrap_or(Color::rgb(40, 44, 52));

        context.fill_rounded_rect(rect, 4, background);
        context.draw_rounded_rect_stroke(rect, 4, border, 1);
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 20),
            &self.text,
            &Font::simple("Sans", 12.0),
            text_color,
            HorizontalAlignment::Left,
        );

        // Completed mentions are underlined, so the user can see which spans the
        // control recognises — including the ones typed by hand rather than chosen.
        // The underline is the theme's `primary`, the hue a theme varies most, rather than a
        // literal blue that stayed put on both appearances.
        let mention_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary)
            .unwrap_or(Color::rgb(66, 133, 244));
        for mention in &self.completed {
            let before = self.text.get(..mention.start).unwrap_or("");
            let span = self.text.get(mention.start..mention.end).unwrap_or("");
            let font = Font::simple("Sans", 12.0);
            let start_x = rect.x + 8 + context.measure_text(before, &font).width as i32;
            let width = context.measure_text(span, &font).width as i32;
            let underline_y = rect.y + 22;
            context.draw_line_stroke(
                Point::new(start_x, underline_y),
                Point::new(start_x + width, underline_y),
                mention_ink,
                1,
            );
        }

        if self.is_popup_open() {
            let caret_x = rect.x + 8 + self.caret_x(context);
            context.draw_line_stroke(
                Point::new(caret_x, rect.y + 8),
                Point::new(caret_x, rect.y + 8 + CARET_HEIGHT as i32),
                text_color,
                1,
            );
            self.draw_popup(context);
        }
    }
}

impl Mention {
    /// Draws the candidate popup below the field.
    ///
    /// # Why the popup reads the theme and the field's fallbacks do not
    ///
    /// The field's literals are *fallbacks*: `apply_active_theme` fills `style.background_color` and
    /// friends before this runs, so on a themed build the `unwrap_or` arms are unreachable. The popup
    /// is different — it painted `Color::WHITE`, a fixed grey border and two fixed inks
    /// unconditionally, so a dark build opened a white list under a dark field. It is fixed the same
    /// way `dropdown`'s list and `cascader`'s columns were: a popup is one step above the page
    /// (`surface_container`), its border is the weak separator (`outline_variant`), a highlighted row
    /// is a *selection* (the accent pair), and the secondary description is the theme's weak ink.
    fn draw_popup(&self, context: &mut RenderContext) {
        let rows = self.filtered.len().min(MAX_VISIBLE);
        if rows == 0 {
            return;
        }
        let rect = self.geometry();
        let popup = Rect::new(
            rect.x,
            rect.y + rect.height as i32,
            rect.width,
            (rows as u32 * ROW_HEIGHT).max(ROW_HEIGHT),
        );
        // One guard over the theme reads, released before drawing: `theme_manager()` is a
        // non-reentrant mutex and the context's accessors cannot take it.
        let (popup_surface, separator, highlight, on_highlight, row_ink, weak_ink) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.surface_container,
                    active.colors.outline_variant,
                    active.colors.primary,
                    active.colors.primary.contrast_color(),
                    active.colors.foreground,
                    active.colors.secondary,
                ),
                None => (
                    Color::WHITE,
                    Color::rgb(190, 192, 198),
                    Color::rgb(232, 240, 254),
                    Color::rgb(40, 44, 52),
                    Color::rgb(40, 44, 52),
                    Color::rgb(140, 144, 152),
                ),
            }
        };
        context.fill_rounded_rect(popup, 4, popup_surface);
        context.draw_rounded_rect_stroke(popup, 4, separator, 1);

        for index in 0..rows {
            let Some(row) = self.row_rect(index) else {
                continue;
            };
            let Some(candidate) = self.filtered.get(index).and_then(|i| self.candidates.get(*i))
            else {
                continue;
            };
            let highlighted = self.highlighted == Some(index);
            if highlighted {
                context.fill_rect(row, highlight);
            }
            let ink = if highlighted { on_highlight } else { row_ink };
            context.draw_text(
                Point::new(row.x + 8, row.y + 16),
                &candidate.display,
                &Font::simple("Sans", 11.0),
                ink,
                HorizontalAlignment::Left,
            );
            if !candidate.description.is_empty() {
                context.draw_text(
                    Point::new(row.x + 120, row.y + 16),
                    &candidate.description,
                    &Font::simple("Sans", 10.0),
                    // The description is a hint beside a value, so on a highlighted row it damps
                    // toward that row's fill rather than staying the page's weak ink.
                    if highlighted { on_highlight.blend(&highlight, 0.3) } else { weak_ink },
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for Mention {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                let rect = self.geometry();
                if rect.contains_point(*pos) {
                    // Clicking the field puts the caret at the end, which is where a
                    // text cursor lands in a single-line field with no glyph metrics.
                    self.caret = self.text.len();
                    self.refresh();
                    self.base.request_redraw();
                    return;
                }
                if self.is_popup_open() {
                    if let Some(index) = self.row_at(*pos) {
                        self.complete(index);
                    } else {
                        // A click outside the popup and outside the field dismisses,
                        // which is what every popup list does.
                        self.close_popup();
                    }
                }
            }
            // Typing goes to the field, and a trigger character opens the popup as a
            // side effect of `refresh` — which is what makes the trigger model work
            // without a separate "detect @" step.
            Event::TextInput { text }
                if text.chars().all(|ch| !ch.is_control()) && !text.is_empty() =>
            {
                for ch in text.chars() {
                    self.insert_char(ch);
                }
            }
            Event::KeyDown((key, _)) | Event::KeyPress { key, .. } => match *key {
                8 => {
                    self.backspace();
                }
                37 => self.move_caret_left(),
                39 => self.move_caret_right(),
                // Up/Down drive the popup when it is open, and do nothing when it is
                // not — a single-line field has no vertical caret movement, so
                // consuming the keys would be surprising.
                38 if self.is_popup_open() => self.move_highlight(-1),
                40 if self.is_popup_open() => self.move_highlight(1),
                13 if self.is_popup_open() => {
                    let index = self.highlighted.unwrap_or(0);
                    self.complete(index);
                }
                27 => self.close_popup(),
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{PaintBackend, SoftwarePaintBackend};

    fn candidates() -> Vec<MentionCandidate> {
        vec![
            MentionCandidate::new("alice", "Alice").with_description("Engineering"),
            MentionCandidate::new("alicia", "Alicia").with_description("Design"),
            MentionCandidate::new("bob", "Bob").with_description("Support"),
        ]
    }

    fn mention() -> Mention {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        m.set_candidates(candidates());
        m
    }

    /// Renders and returns the RGBA frame.
    ///
    /// Gated like its consumer: the software backend and `Size` only exist behind a device
    /// profile with `software`, so an ungated helper is a dead-code warning (or a hard error in a
    /// `no_std` profile) for a build that never calls it.
    fn render(m: &mut Mention, size: Size) -> Vec<u8> {
        render_on(m, size, Color::WHITE)
    }

    /// Renders over an explicit backdrop, which is the surface the control would sit on.
    fn render_on(m: &mut Mention, size: Size, backdrop: Color) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        m.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    /// Types `text` one character at a time, as a keyboard would.
    fn type_text(m: &mut Mention, text: &str) {
        for ch in text.chars() {
            m.insert_char(ch);
        }
    }

    #[test]
    fn mention_creation_defaults() {
        let m = Mention::new(Rect::new(0, 0, 240, 30));
        assert_eq!(m.kind(), WidgetKind::Mention);
        assert_eq!(m.text(), "");
        assert_eq!(m.caret(), 0);
        assert_eq!(m.trigger(), '@');
        assert!(m.candidates().is_empty());
        assert!(!m.is_popup_open());
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_trigger_opens_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert!(m.is_popup_open(), "the trigger must open the popup");
        assert_eq!(m.query(), Some(""));
        assert_eq!(m.visible_candidates().len(), 3);
    }

    /// The candidate popup follows the appearance, not just the field.
    ///
    /// # The defect this pins
    ///
    /// `draw_popup` painted `Color::WHITE`, a fixed grey border, a fixed pale highlight and two
    /// fixed inks — none of which moved with the theme — while the field above it read `style.*`.
    /// So a dark build showed a white suggestion list hanging off a dark field, the same
    /// "half themed" shape `dropdown` had.
    ///
    /// # Why the assertion names the popup's own pixel
    ///
    /// A frame-wide comparison passes even with the literals restored, because the field and the
    /// backdrop differ between appearances too. The sample point is inside the popup band, on a row
    /// with no glyph there, so the pixel it reads is the popup surface and nothing else.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_candidate_popup_follows_the_appearance() {
        let _guard = crate::theme::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let size = Size::new(240, 160);

        let popup_pixel = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let mut m = mention();
            type_text(&mut m, "@");
            crate::theme::apply_theme_to_widget(&mut m);
            let frame = render_on(&mut m, size, backdrop);
            // The field is 30 tall, so the popup starts below it. Half the popup's width and a y
            // near its bottom edge are clear of any glyph — the row text is drawn from `y + 16`.
            let x = 240 / 2;
            let y = 30 + 3 * ROW_HEIGHT - 3;
            pixel(&frame, size, x, y)
        };

        let dark = popup_pixel(crate::theme::AppearanceMode::Dark);
        let light = popup_pixel(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the candidate popup must follow the appearance; both were {dark:?}"
        );
        assert_ne!(
            dark,
            [255, 255, 255, 255],
            "the candidate popup must not be the fixed white the defect used"
        );
    }

    #[test]
    fn mention_typing_filters_by_query() {
        let mut m = mention();
        type_text(&mut m, "@al");
        assert_eq!(m.query(), Some("al"));
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 2, "Alice and Alicia both start with 'al'");
        assert_eq!(visible[0].display, "Alice");
    }

    #[test]
    fn mention_query_narrows_to_one() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "bob");
    }

    #[test]
    fn mention_matches_description_as_well_as_display() {
        let mut m = mention();
        type_text(&mut m, "@Design");
        let visible = m.visible_candidates();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "alicia");
    }

    #[test]
    fn mention_with_no_match_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@zzzz");
        // An empty popup reads as a broken control, so it closes instead.
        assert!(!m.is_popup_open());
        assert!(m.visible_candidates().is_empty());
    }

    #[test]
    fn mention_completion_replaces_the_token_including_the_trigger() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        let id = m.complete(0).expect("a candidate is highlighted");
        assert_eq!(id, "alice");
        // `@ali` becomes `@Alice` — the trigger is consumed, not doubled.
        assert_eq!(m.text(), "@Alice");
        assert_eq!(m.caret(), "@Alice".len(), "the caret lands after the mention");
        assert!(!m.is_popup_open(), "completing closes the popup");
    }

    #[test]
    fn mention_completion_keeps_surrounding_prose() {
        let mut m = mention();
        type_text(&mut m, "hello ");
        type_text(&mut m, "@bob");
        m.complete(0);
        assert_eq!(m.text(), "hello @Bob");
    }

    #[test]
    fn mention_records_the_completed_mention() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        assert!(m.complete(0).is_some(), "a candidate must be highlighted to complete");
        // Prose after the mention does not disturb the recorded span.
        type_text(&mut m, " please review");

        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "bob");
        // The span covers the trigger and the display.
        assert_eq!(&m.text()[completed[0].start..completed[0].end], "@Bob");
        assert_eq!(m.text(), "@Bob please review");
    }

    #[test]
    fn mention_supports_several_mentions_in_one_field() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        type_text(&mut m, " and ");
        type_text(&mut m, "@ali");
        m.complete(0);

        assert_eq!(m.text(), "@Bob and @Alice");
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].id, "bob");
        assert_eq!(completed[1].id, "alice");
    }

    #[test]
    fn mention_does_not_open_mid_word() {
        let mut m = mention();
        // `@` inside a word makes an email address, not a mention.
        type_text(&mut m, "user@example");
        assert!(!m.is_popup_open(), "a mid-word trigger is not a mention");
    }

    #[test]
    fn mention_does_not_reopen_after_a_space() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        type_text(&mut m, " ");
        assert!(!m.is_popup_open(), "a completed mention must not reopen");
    }

    #[test]
    fn mention_a_trigger_after_whitespace_opens() {
        let mut m = mention();
        type_text(&mut m, "hi ");
        type_text(&mut m, "@");
        assert!(m.is_popup_open());
    }

    #[test]
    fn mention_backspace_updates_the_query() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        assert_eq!(m.visible_candidates().len(), 2);
        m.backspace();
        assert_eq!(m.query(), Some("al"));
        m.backspace();
        assert_eq!(m.query(), Some("a"));
    }

    #[test]
    fn mention_backspacing_the_trigger_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert!(m.is_popup_open());
        assert!(m.backspace());
        assert!(!m.is_popup_open(), "no trigger means no popup");
        assert_eq!(m.text(), "");
    }

    #[test]
    fn mention_backspace_at_zero_does_nothing() {
        let mut m = mention();
        assert!(!m.backspace());
    }

    #[test]
    fn mention_backspace_handles_multibyte_characters() {
        let mut m = mention();
        // A CJK character is three bytes; a naive `caret - 1` would split it and
        // panic on the slice.
        type_text(&mut m, "你");
        assert_eq!(m.text(), "你");
        assert_eq!(m.caret(), 3);
        assert!(m.backspace());
        assert_eq!(m.text(), "");
        assert_eq!(m.caret(), 0);
    }

    #[test]
    fn mention_highlight_moves_and_wraps() {
        let mut m = mention();
        type_text(&mut m, "@");
        assert_eq!(m.highlighted(), Some(0));

        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(1));
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(2));
        // Wraps at the end.
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), Some(0));
        m.handle_event(&Event::KeyDown((38, 0)));
        assert_eq!(m.highlighted(), Some(2));
    }

    #[test]
    fn mention_enter_chooses_the_highlighted_candidate() {
        let mut m = mention();
        type_text(&mut m, "@");
        m.handle_event(&Event::KeyDown((40, 0))); // highlight the second
        m.handle_event(&Event::KeyDown((13, 0)));
        assert_eq!(m.text(), "@Alicia");
    }

    #[test]
    fn mention_arrows_do_not_apply_when_the_popup_is_closed() {
        let mut m = mention();
        type_text(&mut m, "plain");
        // A single-line field has no vertical caret movement, so consuming the keys
        // would be surprising.
        m.handle_event(&Event::KeyDown((40, 0)));
        assert_eq!(m.highlighted(), None);
        assert_eq!(m.text(), "plain");
    }

    #[test]
    fn mention_escape_closes_without_completing() {
        let mut m = mention();
        type_text(&mut m, "@ali");
        m.handle_event(&Event::KeyDown((27, 0)));
        assert!(!m.is_popup_open());
        assert_eq!(m.text(), "@ali", "cancelling must not change the text");
    }

    #[test]
    fn mention_horizontal_arrows_move_the_caret() {
        let mut m = mention();
        type_text(&mut m, "abc");
        assert_eq!(m.caret(), 3);
        m.handle_event(&Event::KeyDown((37, 0)));
        assert_eq!(m.caret(), 2);
        m.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(m.caret(), 3);
        // Clamped at both ends.
        m.handle_event(&Event::KeyDown((39, 0)));
        assert_eq!(m.caret(), 3);
    }

    #[test]
    fn mention_click_on_a_row_completes_it() {
        let mut m = mention();
        type_text(&mut m, "@");
        let row = m.row_rect(1).expect("row 1");
        m.handle_event(&Event::mouse_press(row.x + 20, row.y + 10, 1));
        assert_eq!(m.text(), "@Alicia");
    }

    #[test]
    fn mention_click_outside_closes_the_popup() {
        let mut m = mention();
        type_text(&mut m, "@");
        m.handle_event(&Event::mouse_press(5000, 5000, 1));
        assert!(!m.is_popup_open());
    }

    #[test]
    fn mention_click_on_the_field_puts_the_caret_at_the_end() {
        let mut m = mention();
        type_text(&mut m, "abc");
        m.move_caret_left();
        assert_eq!(m.caret(), 2);
        m.handle_event(&Event::mouse_press(100, 15, 1));
        assert_eq!(m.caret(), 3);
    }

    #[test]
    fn mention_disabled_ignores_typing() {
        let mut m = mention();
        m.set_enabled(false);
        m.handle_event(&Event::TextInput { text: "@".to_string() });
        assert_eq!(m.text(), "");
    }

    #[test]
    fn mention_open_popup_from_code_inserts_the_trigger() {
        let mut m = mention();
        // A toolbar button needs the same entry point the keyboard has.
        assert!(m.open_popup());
        assert_eq!(m.text(), "@");
        assert!(m.is_popup_open());
    }

    #[test]
    fn mention_open_popup_with_no_candidates_does_nothing() {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        assert!(!m.open_popup());
        assert!(!m.is_popup_open());
    }

    #[test]
    fn mention_signals_fire() {
        let mut m = mention();
        let inserted = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let toggles = std::sync::Arc::new(std::sync::Mutex::new(Vec::<bool>::new()));
        let sink = inserted.clone();
        m.mention_inserted.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*id).clone());
            }
        });
        let sink = toggles.clone();
        m.popup_toggled.connect(move |open| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*open);
            }
        });

        type_text(&mut m, "@bob");
        m.complete(0);

        assert_eq!(*inserted.lock().expect("signal lock poisoned"), vec!["bob"]);
        assert_eq!(*toggles.lock().expect("signal lock poisoned"), vec![true, false]);
    }

    #[test]
    fn mention_set_text_derives_the_state() {
        let mut m = mention();
        m.set_text("hey @ali".to_string());
        assert_eq!(m.caret(), "hey @ali".len());
        assert!(m.is_popup_open(), "set_text must derive the token state too");
        assert_eq!(m.visible_candidates().len(), 2);
    }

    #[test]
    fn mention_set_candidates_drops_vanished_mentions() {
        let mut m = mention();
        type_text(&mut m, "@bob");
        m.complete(0);
        assert_eq!(m.completed_mentions().len(), 1);

        // A mention whose candidate is gone would report an id nothing resolves.
        m.set_candidates(vec![MentionCandidate::new("alice", "Alice")]);
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_rebuilds_mentions_typed_by_hand() {
        let mut m = mention();
        // Typed rather than chosen: the control must still recognise it, which is what
        // makes the underline honest.
        m.set_text("cc @Bob and @Alice".to_string());
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 2);
        assert_eq!(completed[0].id, "bob");
        assert_eq!(completed[1].id, "alice");
    }

    #[test]
    fn mention_ignores_a_trigger_with_no_matching_candidate() {
        let mut m = mention();
        // An email address is not a mention, and neither is an unknown handle.
        m.set_text("mail me at a@b.com".to_string());
        assert!(m.completed_mentions().is_empty());
    }

    #[test]
    fn mention_prefers_the_longest_matching_display() {
        let mut m = Mention::new(Rect::new(0, 0, 240, 30));
        m.set_candidates(vec![
            MentionCandidate::new("al", "Al"),
            MentionCandidate::new("alan", "Alan"),
        ]);
        // `@Alan` matches both displays as a prefix; the longer one is what the text
        // contains.
        m.set_text("@Alan".to_string());
        let completed = m.completed_mentions();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "alan");
    }

    #[test]
    fn mention_set_trigger_changes_the_character() {
        let mut m = mention();
        assert!(m.set_trigger('#'));
        assert_eq!(m.trigger(), '#');
        type_text(&mut m, "#bo");
        assert!(m.is_popup_open());
        assert_eq!(m.visible_candidates().len(), 1);
    }

    #[test]
    fn mention_set_trigger_refuses_whitespace() {
        let mut m = mention();
        // The token walker stops at whitespace, so a space trigger could never fire.
        assert!(!m.set_trigger(' '));
        assert_eq!(m.trigger(), '@');
    }

    // ── Drawing ─────────────────────────────────────────────────────────────

    #[test]
    fn mention_draw_closed_paints_without_panicking() {
        let mut m = mention();
        type_text(&mut m, "hi");
        let rgba = render(&mut m, Size::new(240, 30));
        assert!(!rgba.is_empty());
    }

    #[test]
    fn mention_popup_differs_from_the_closed_frame() {
        let mut m = mention();
        let closed = render(&mut m, Size::new(240, 120));
        type_text(&mut m, "@");
        let open = render(&mut m, Size::new(240, 120));
        assert_ne!(closed, open, "the popup must be visible");
    }

    #[test]
    fn mention_highlight_is_visible() {
        let mut m = mention();
        type_text(&mut m, "@");
        let first = render(&mut m, Size::new(240, 120));
        m.handle_event(&Event::KeyDown((40, 0)));
        let second = render(&mut m, Size::new(240, 120));
        assert_ne!(first, second, "moving the highlight must be visible");
    }

    #[test]
    fn mention_completed_underline_is_visible() {
        let mut m = mention();
        m.set_text("hi".to_string());
        let plain = render(&mut m, Size::new(240, 30));
        m.set_text("@Bob".to_string());
        let mentioned = render(&mut m, Size::new(240, 30));
        assert_ne!(plain, mentioned);
    }

    #[test]
    fn mention_draw_zero_geometry_does_not_panic() {
        let mut m = mention();
        type_text(&mut m, "@");
        let rgba = render(&mut m, Size::new(4, 4));
        assert!(!rgba.is_empty());
    }

    // ── Property contract ───────────────────────────────────────────────────

    #[test]
    fn mention_text_property_round_trips() {
        let mut m = mention();
        m.set("text", CapabilityValue::String("@bob hi".to_string())).unwrap();
        assert_eq!(m.text(), "@bob hi");
        assert_eq!(m.get("text").unwrap(), CapabilityValue::String("@bob hi".to_string()));
    }

    #[test]
    fn mention_trigger_property_accepts_one_character_only() {
        let mut m = mention();
        m.set("trigger", CapabilityValue::String("#".to_string())).unwrap();
        assert_eq!(m.trigger(), '#');
        // A longer string would make "which character fired" unanswerable.
        assert!(m.set("trigger", CapabilityValue::String("##".to_string())).is_err());
        assert!(m.set("trigger", CapabilityValue::String(String::new())).is_err());
        // And a whitespace trigger is refused through the property too.
        assert!(m.set("trigger", CapabilityValue::String(" ".to_string())).is_err());
    }

    #[test]
    fn mention_derived_properties_are_read_only() {
        let mut m = mention();
        m.set_text("@bob".to_string());
        assert_eq!(m.get("candidate_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(m.get("visible_candidate_count").unwrap(), CapabilityValue::UInt(1));
        assert_eq!(m.get("popup_open").unwrap(), CapabilityValue::Bool(true));

        for name in
            ["candidate_count", "visible_candidate_count", "mention_count", "caret", "popup_open"]
        {
            assert_eq!(
                m.set(name, CapabilityValue::UInt(9)),
                Err(CapabilityAccessError::ReadOnlyProperty),
                "{name} must be read-only"
            );
        }
    }

    #[test]
    fn mention_candidate_matches_id_description_and_display() {
        let candidate = MentionCandidate::new("alice", "Alice").with_description("Engineering");
        assert!(candidate.matches("alice"));
        assert!(candidate.matches("Alice"));
        assert!(candidate.matches("engineering"));
        assert!(candidate.matches(""));
        assert!(!candidate.matches("bob"));
    }
}
