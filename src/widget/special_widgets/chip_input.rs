// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ChipInput` — a text line whose committed tokens become chips (COMP-H02).
//!
//! [`Chip`] is a selection/reorder control: its items arrive through [`Chip::set_items`] and
//! nothing in its event surface types text. The "type and it becomes a token" workflow (recipients,
//! filter terms, tags) therefore needs a partner control, which is this one.
//!
//! The chip band is a real [`Chip`], not a second row renderer: it owns the pill layout, the
//! record-shaped `items` property ([`CHIP_ITEM_SCHEMA`](super::CHIP_ITEM_SCHEMA)), hover and the
//! keyboard path. This control adds the text line, the commit rules and the signals; the two cannot
//! disagree about what a chip looks like because there is only one drawer.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{Chip, ChipItem};

/// Height of the text line below the chip band.
const INPUT_ROW_HEIGHT: u32 = 26;

/// A chip band plus the text line that feeds it.
pub struct ChipInput {
    base: BaseWidget,
    band: Chip,
    text: String,
    placeholder: String,
    separator: char,
    max_count: usize,
    allow_duplicates: bool,
    /// The caret, in characters.
    cursor: usize,
    /// Emitted with the id of a chip this control committed.
    pub chip_committed: Signal1<String>,
    /// Emitted with the id of a chip removed through the text line.
    pub chip_removed: Signal1<String>,
    /// Emitted with the text line after every change.
    pub text_changed: Signal1<String>,
    /// Emitted with the current item count when a commit is refused by `max_count`.
    pub limit_reached: Signal1<usize>,
    /// Emitted with the labels of every committed chip after a change.
    ///
    /// This is the control's route onto the event bus: `tags_changed` is the event the shared
    /// `tag_input` capability already declares, and the spelling is the same one `TagInput` uses
    /// (SP-121).
    pub tags_changed: Signal1<Vec<String>>,
}

impl ChipInput {
    /// Creates an empty input with `,` as its separator and no item limit.
    pub fn new(geometry: Rect) -> Self {
        let band = Chip::new(geometry);
        Self {
            base: BaseWidget::new(WidgetKind::TextEdit, geometry, "ChipInput"),
            band,
            text: String::new(),
            placeholder: String::new(),
            separator: ',',
            max_count: 0,
            allow_duplicates: false,
            cursor: 0,
            chip_committed: Signal1::new(),
            chip_removed: Signal1::new(),
            text_changed: Signal1::new(),
            limit_reached: Signal1::new(),
            tags_changed: Signal1::new(),
        }
    }

    /// The committed items.
    pub fn items(&self) -> &[ChipItem] {
        self.band.items()
    }

    /// Replaces the committed items.
    pub fn set_items(&mut self, items: Vec<ChipItem>) {
        self.band.set_items(items);
        self.emit_tags_changed();
        self.base.request_redraw();
    }

    /// The labels of every committed chip.
    pub fn tags(&self) -> Vec<String> {
        self.band.items().iter().map(|item| item.label.clone()).collect()
    }

    /// Publishes the live tag list after an item change.
    fn emit_tags_changed(&mut self) {
        let tags = self.tags();
        self.tags_changed.emit(tags);
    }

    /// The chip band, for a host that wants its selection or its signals.
    pub fn chip_band(&self) -> &Chip {
        &self.band
    }

    /// The chip band, mutably.
    pub fn chip_band_mut(&mut self) -> &mut Chip {
        &mut self.band
    }

    /// The current text line.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the text line.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.cursor = self.text.chars().count();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// The placeholder painted while the line is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        let placeholder = placeholder.into();
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder = placeholder;
        self.base.request_redraw();
    }

    /// The separator a typed run is split on.
    pub fn separator(&self) -> char {
        self.separator
    }

    /// Sets the separator character.
    pub fn set_separator(&mut self, separator: char) {
        self.separator = separator;
    }

    /// The item limit; zero means unlimited.
    pub fn max_count(&self) -> usize {
        self.max_count
    }

    /// Sets the item limit; zero means unlimited.
    pub fn set_max_count(&mut self, max_count: usize) {
        self.max_count = max_count;
    }

    /// Whether a label may be committed twice.
    pub fn allow_duplicates(&self) -> bool {
        self.allow_duplicates
    }

    /// Allows or refuses duplicate labels.
    pub fn set_allow_duplicates(&mut self, allow: bool) {
        self.allow_duplicates = allow;
    }

    /// Commits the current line, splitting it on [`Self::separator`].
    ///
    /// Returns how many chips were added. A token that is empty, already present (when duplicates
    /// are refused) or beyond [`Self::max_count`] is skipped; reaching the limit also emits
    /// [`Self::limit_reached`], which is the difference between "nothing to add" and "the limit
    /// stopped it".
    pub fn commit(&mut self) -> usize {
        let raw = core::mem::take(&mut self.text);
        self.cursor = 0;
        let mut added = 0usize;
        for token in raw.split(self.separator) {
            let label = token.trim();
            if label.is_empty() {
                continue;
            }
            if self.max_count > 0 && self.band.items().len() >= self.max_count {
                self.limit_reached.emit(self.band.items().len());
                break;
            }
            if !self.allow_duplicates && self.band.items().iter().any(|item| item.label == label) {
                continue;
            }
            let id = self.unique_id(label);
            let mut items = self.band.items().to_vec();
            items.push(ChipItem::new(id.clone(), label));
            self.band.set_items(items);
            self.chip_committed.emit(id);
            added += 1;
        }
        self.text_changed.emit(String::new());
        if added > 0 {
            self.emit_tags_changed();
        }
        self.base.request_redraw();
        added
    }

    /// An id that no committed item uses, derived from `label`.
    fn unique_id(&self, label: &str) -> String {
        if !self.band.items().iter().any(|item| item.id == label) {
            return label.to_string();
        }
        let mut suffix = 2usize;
        loop {
            let candidate = format!("{label}#{suffix}");
            if !self.band.items().iter().any(|item| item.id == candidate) {
                return candidate;
            }
            suffix += 1;
        }
    }

    /// Removes every chip, reporting each removal.
    pub fn clear(&mut self) {
        let removed: Vec<String> = self.band.items().iter().map(|item| item.id.clone()).collect();
        self.band.set_items(Vec::new());
        for id in removed {
            self.chip_removed.emit(id);
        }
        self.emit_tags_changed();
        self.base.request_redraw();
    }

    /// Removes the last chip, reporting it.
    fn remove_last(&mut self) {
        let mut items = self.band.items().to_vec();
        if let Some(item) = items.pop() {
            self.band.set_items(items);
            self.chip_removed.emit(item.id);
            self.emit_tags_changed();
            self.base.request_redraw();
        }
    }

    /// Inserts `text` at the caret.
    fn insert(&mut self, text: &str) {
        let mut chars: Vec<char> = self.text.chars().collect();
        let at = self.cursor.min(chars.len());
        chars.splice(at..at, text.chars());
        self.text = chars.into_iter().collect();
        self.cursor = at + text.chars().count();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Deletes the character before the caret.
    fn backspace(&mut self) {
        if self.text.is_empty() {
            self.remove_last();
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        if self.cursor == 0 || self.cursor > chars.len() {
            return;
        }
        chars.remove(self.cursor - 1);
        self.cursor -= 1;
        self.text = chars.into_iter().collect();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// The band's rectangle: the control above the text line.
    fn band_rect(&self) -> Rect {
        let rect = self.geometry();
        let input = if rect.height > INPUT_ROW_HEIGHT { INPUT_ROW_HEIGHT } else { rect.height };
        Rect::new(rect.x, rect.y, rect.width, rect.height - input)
    }

    /// The text line's rectangle.
    fn input_rect(&self) -> Rect {
        let rect = self.geometry();
        let height = INPUT_ROW_HEIGHT.min(rect.height);
        Rect::new(rect.x, rect.y + rect.height as i32 - height as i32, rect.width, height)
    }
}

impl Widget for ChipInput {
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
        Size::new(240, 24 + INPUT_ROW_HEIGHT)
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves every published event of this control onto the bus.
    ///
    /// `tag_input` is the shared capability this control is registered under (its aliases include
    /// `chip_input`). `tags_changed` was the only name it published before SP-121; the four
    /// signals the control emits itself are now published alongside it and declared in
    /// `capability/event_payloads.rs`, so a host can subscribe to a commit, a removal, a text
    /// edit and a refused commit by name instead of reaching for the concrete type.
    ///
    /// The payload conversions follow the capability convention: text is `String`, and the item
    /// count carried by [`Self::limit_reached`] is `UInt` rather than a signed `Int` because a
    /// count can never be negative.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "chip_committed" => {
                Some(EventSignalRef::mapped("chip_committed", &self.chip_committed, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            "chip_removed" => {
                Some(EventSignalRef::mapped("chip_removed", &self.chip_removed, |id| {
                    CapabilityValue::String(id.clone())
                }))
            }
            "limit_reached" => {
                Some(EventSignalRef::mapped("limit_reached", &self.limit_reached, |count| {
                    CapabilityValue::UInt(*count as u64)
                }))
            }
            "tags_changed" => {
                Some(EventSignalRef::mapped("tags_changed", &self.tags_changed, |tags| {
                    // The declared shape is `ListScalar` of `String`, so the payload arrives as the
                    // tag list rather than its `Debug` spelling — a subscriber can read each tag
                    // as its own string.
                    CapabilityValue::Tuple(
                        tags.iter().map(|tag| CapabilityValue::String(tag.clone())).collect(),
                    )
                }))
            }
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |text| {
                    CapabilityValue::String(text.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for ChipInput {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The record contract is the band's own property, so a document that describes chips
            // for `Chip` describes them for this control with no second schema.
            "items" => WidgetProperties::get(&self.band, "items"),
            "item_count" => Ok(CapabilityValue::UInt(self.band.items().len() as u64)),
            "text" => Ok(CapabilityValue::String(self.text.clone())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder.clone())),
            "separator" => Ok(CapabilityValue::String(self.separator.to_string())),
            "max_count" => Ok(CapabilityValue::UInt(self.max_count as u64)),
            "allow_duplicates" => Ok(CapabilityValue::Bool(self.allow_duplicates)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "items" => WidgetProperties::set(&mut self.band, "items", value),
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            "separator" => {
                let token = expect_string(value)?;
                let separator = token.chars().next().ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_separator(separator);
                Ok(())
            }
            "max_count" => {
                self.set_max_count(expect_usize(value)?);
                Ok(())
            }
            "allow_duplicates" => {
                self.set_allow_duplicates(expect_bool(value)?);
                Ok(())
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "text",
            "placeholder",
            "separator",
            "max_count",
            "allow_duplicates",
            BASE_PROPERTY_NAMES
        ]
    }

    /// `commit` and `clear` take no argument, so they are the two commands this control runs.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "commit" => {
                if self.commit() > 0 {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "clear" => {
                self.clear();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ChipInput {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The band's geometry is refreshed before a press is resolved against it, so a resized
        // control hit-tests the pills actually on screen.
        let band = self.band_rect();
        if self.band.base().geometry() != band {
            self.band.base_mut().set_geometry(band);
        }
        match event {
            Event::TextInput { text } | Event::ImeCommit { text } => {
                let filtered: String = text.chars().filter(|ch| !ch.is_control()).collect();
                if filtered.is_empty() {
                    return;
                }
                // A typed separator commits what is before it and starts a new token.
                if filtered.contains(self.separator) {
                    let mut pieces = filtered.split(self.separator).peekable();
                    if let Some(first) = pieces.next() {
                        if !first.is_empty() {
                            self.insert(first);
                        }
                    }
                    self.commit();
                    for piece in pieces {
                        if !piece.is_empty() {
                            self.insert(piece);
                        }
                    }
                    return;
                }
                self.insert(&filtered);
            }
            Event::KeyPress { key, .. } => {
                let pressed = crate::shortcut::Key::from_key_code(*key);
                match pressed {
                    Some(crate::shortcut::Key::Enter) => {
                        self.commit();
                    }
                    Some(crate::shortcut::Key::Backspace) => self.backspace(),
                    Some(crate::shortcut::Key::Escape) => {
                        if !self.text.is_empty() {
                            self.text.clear();
                            self.cursor = 0;
                            self.text_changed.emit(String::new());
                            self.base.request_redraw();
                        }
                    }
                    Some(crate::shortcut::Key::Left) => {
                        self.cursor = self.cursor.saturating_sub(1);
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Right) => {
                        self.cursor = (self.cursor + 1).min(self.text.chars().count());
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::Home) => {
                        self.cursor = 0;
                        self.base.request_redraw();
                    }
                    Some(crate::shortcut::Key::End) => {
                        self.cursor = self.text.chars().count();
                        self.base.request_redraw();
                    }
                    _ => {}
                }
            }
            Event::MousePress { pos, .. } => {
                // The chips are real controls; the text line only takes the press when it is
                // outside the band.
                if band.contains_point(*pos) {
                    self.band.handle_event(event);
                } else if self.input_rect().contains_point(*pos) {
                    self.base.request_redraw();
                }
            }
            Event::MouseMove { pos } | Event::MouseRelease { pos, .. } => {
                if self.band.base().geometry().contains_point(*pos) {
                    self.band.handle_event(event);
                }
            }
            Event::MouseLeave { .. } => self.band.handle_event(event),
            _ => {}
        }
    }
}

impl Draw for ChipInput {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let band = self.band_rect();
        if self.band.base().geometry() != band {
            self.band.base_mut().set_geometry(band);
        }
        let input = self.input_rect();

        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("text_edit");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| Color::border_of(surface));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let font = effective_font(style);

        context.face_with_gradient(
            rect,
            surface,
            style.background_gradient.as_ref(),
            style.surface.unwrap_or_default(),
            style.border_radius.unwrap_or(0),
            Color::BLACK,
        );
        crate::widget::draw::stroke_surface_border(
            context,
            rect,
            self.style().resolved_corner_radii().to_array(),
            border,
            1,
        );
        self.band.draw(context);

        // The text line: a rule above it separates entry from what has been committed.
        context.draw_line(
            Point::new(input.x, input.y),
            Point::new(input.x + input.width as i32, input.y),
            border,
        );
        let caret_y = input.y + (input.height as i32 - 14) / 2;
        let text =
            if self.text.is_empty() { self.placeholder.as_str() } else { self.text.as_str() };
        let text_ink = if self.text.is_empty() { ink.with_alpha(120) } else { ink };
        context.draw_text(
            Point::new(input.x + 6, caret_y),
            text,
            font,
            text_ink,
            HorizontalAlignment::Left,
        );
        // The caret is placed from the measured prefix, so it tracks in-line editing rather than
        // always sitting at the end of the string.
        let prefix: String = self.text.chars().take(self.cursor).collect();
        let caret_x = input.x + 6 + estimate_text_width(&prefix, font, 1.0) as i32;
        if self.base.is_enabled() {
            context.fill_rect(Rect::new(caret_x, caret_y - 1, 1, 15), ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// The live tag list is published after a commit, a removal and a clear, and the control
    /// resolves the event the shared capability declares (SP-121).
    #[test]
    fn tags_changed_publishes_after_every_item_change() {
        let mut input = ChipInput::new(Rect::new(0, 0, 240, 60));
        let seen = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let sink = Arc::clone(&seen);
        input.tags_changed.connect(move |tags| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*tags).clone());
            }
        });

        input.set_text("alpha,beta");
        assert_eq!(input.commit(), 2);

        // A refused duplicate adds nothing, so no event is published for it.
        input.set_text("beta");
        assert_eq!(input.commit(), 0);

        input.clear();
        assert_eq!(
            seen.lock().expect("tags lock poisoned").clone(),
            vec![vec![String::from("alpha"), String::from("beta")], Vec::new()]
        );

        // The declared event resolves to this signal, so a hub subscription reaches it.
        assert!(input.event_signal_dyn("tags_changed").is_some());
    }

    /// SP-121: every signal this control emits resolves by its published name, and nothing else
    /// does — the capability row and the arm set name the same five events.
    #[test]
    fn every_chip_input_event_resolves_by_name() {
        let input = ChipInput::new(Rect::new(0, 0, 240, 60));
        for name in
            ["chip_committed", "chip_removed", "limit_reached", "tags_changed", "text_changed"]
        {
            assert!(input.event_signal_dyn(name).is_some(), "{name} must resolve");
        }
        assert!(input.event_signal_dyn("no_such_event").is_none());
    }
}
