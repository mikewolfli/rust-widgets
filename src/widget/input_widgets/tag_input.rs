// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! TagInput widget — a text input that creates tags/chips on Enter or comma, with removable tags.
//!
//! The TagInput presents a text field where the user types tag text, pressing Enter or comma
//! to create a tag chip. Each tag is displayed as a rounded chip with an X button for removal.
//! It emits `tags_changed` with the current list of tags on every change.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TAG_INPUT_COMMAND_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
struct TagInputState {
    tags: Vec<String>,
    input_buffer: String,
}

struct TagInputStateCommand {
    id: CommandId,
    target: Rc<RefCell<TagInputState>>,
    before: TagInputState,
    after: TagInputState,
}

impl TagInputStateCommand {
    fn new(
        target: Rc<RefCell<TagInputState>>,
        before: TagInputState,
        after: TagInputState,
    ) -> Self {
        Self {
            id: CommandId(NEXT_TAG_INPUT_COMMAND_ID.fetch_add(1, Ordering::Relaxed)),
            target,
            before,
            after,
        }
    }
}

impl UndoCommand for TagInputStateCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit tags".to_string(),
            timestamp_ms: 0,
            command_type: "tag_input_state",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.after.clone();
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        *self.target.borrow_mut() = self.before.clone();
        Ok(())
    }
}

/// Horizontal padding between tag chip and container edge, or between chips.
const TAG_PADDING: i32 = 6;
/// Vertical padding inside the tag area.
const TAG_VERTICAL_PADDING: i32 = 4;
/// Gap between tag chips.
const TAG_GAP: i32 = 4;
/// Line height for the tag row.
const TAG_HEIGHT: i32 = 24;
/// Close button radius inside each tag chip.
const TAG_CLOSE_RADIUS: i32 = 7;
/// Corner radius of each tag chip.
const TAG_CHIP_RADIUS: u32 = 12;
/// Minimum input area width.
const MIN_INPUT_WIDTH: i32 = 60;
/// TagInput widget — a text input that creates tags/chips on Enter or comma.
pub struct TagInput {
    base: BaseWidget,
    tags: Vec<String>,
    input_buffer: String,
    placeholder: String,
    focused: bool,
    /// The input caret's blink state, advanced by [`TagInput::tick`].
    cursor_blink: crate::style::CursorBlink,
    /// Emitted when the tags list changes, providing the full list of tags.
    pub tags_changed: Signal1<Vec<String>>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<TagInputState>>,
}

impl TagInput {
    /// Creates a new TagInput widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TagInput, geometry, "TagInput"),
            tags: Vec::new(),
            input_buffer: String::new(),
            placeholder: "Type and press Enter\u{2026}".to_string(),
            focused: false,
            cursor_blink: crate::style::CursorBlink::new(),
            tags_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(TagInputState {
                tags: Vec::new(),
                input_buffer: String::new(),
            })),
        }
    }

    /// Adds a tag to the list. Trims the input and ignores empty tags.
    /// Emits `tags_changed` if a tag was actually added.
    pub fn add_tag(&mut self, tag: &str) {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            return;
        }
        // Avoid duplicate tags
        if self.tags.iter().any(|t| t == trimmed) {
            return;
        }
        let before = self.snapshot_state();
        self.tags.push(trimmed.to_string());
        self.record_state_change(before);
        self.tags_changed.emit(self.tags.clone());
        self.base.request_redraw();
    }

    /// Removes a tag at the given index.
    /// Emits `tags_changed` if a tag was actually removed.
    pub fn remove_tag(&mut self, index: usize) {
        if index < self.tags.len() {
            let before = self.snapshot_state();
            self.tags.remove(index);
            self.record_state_change(before);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Returns a slice of all current tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Returns the placeholder shown while the input area is empty.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder shown while the input area is empty.
    pub fn set_placeholder(&mut self, placeholder: &str) {
        self.placeholder = placeholder.to_string();
        self.base.request_redraw();
    }

    /// Clears all tags. Emits `tags_changed` if the list was non-empty.
    pub fn clear_tags(&mut self) {
        if !self.tags.is_empty() {
            let before = self.snapshot_state();
            self.tags.clear();
            self.input_buffer.clear();
            self.record_state_change(before);
            self.tags_changed.emit(self.tags.clone());
            self.base.request_redraw();
        }
    }

    /// Returns whether this tag input currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets the focused state.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            // The caret only blinks in a focused field, so the blink follows the same flag the
            // draw pass reads rather than a second notion of "active" that could disagree.
            if focused {
                self.cursor_blink.start();
            } else {
                self.cursor_blink.stop();
            }
            self.base.request_redraw();
            if focused {
                self.base.focus_gained.emit();
            } else {
                self.base.focus_lost.emit();
            }
        }
    }

    /// Advances the input caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention. Before it existed this control drew a solid
    /// caret and advertised a `Cursor blink interval` constant that was not defined anywhere.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.focused {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    /// Commits the current input buffer as a tag and clears the buffer.
    fn commit_input(&mut self) {
        let before = self.snapshot_state();
        let old_tags = self.tags.clone();
        let text = self.input_buffer.trim().to_string();
        if !text.is_empty() && !self.tags.iter().any(|tag| tag == &text) {
            self.tags.push(text);
        }
        self.input_buffer.clear();
        self.record_state_change(before);
        if self.tags != old_tags {
            self.tags_changed.emit(self.tags.clone());
        }
        self.base.request_redraw();
    }

    /// Reverts the most recent tag/input-buffer change.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `tags_changed` if the tag list actually differs.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_state();
        true
    }

    /// Reapplies the most recently undone tag/input-buffer change.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `tags_changed` if the tag list actually differs.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_state();
        true
    }

    /// Returns whether there is a tag/input-buffer change to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone tag/input-buffer change to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    // ── Private helpers ──

    fn snapshot_state(&self) -> TagInputState {
        TagInputState { tags: self.tags.clone(), input_buffer: self.input_buffer.clone() }
    }

    fn record_state_change(&mut self, before: TagInputState) {
        let after = self.snapshot_state();
        if before == after {
            return;
        }
        *self.history_target.borrow_mut() = after.clone();
        self.undo_stack.push(Box::new(TagInputStateCommand::new(
            self.history_target.clone(),
            before,
            after,
        )));
    }

    fn restore_history_state(&mut self) {
        let previous_tags = self.tags.clone();
        let state = self.history_target.borrow().clone();
        self.tags = state.tags;
        self.input_buffer = state.input_buffer;
        if self.tags != previous_tags {
            self.tags_changed.emit(self.tags.clone());
        }
        self.base.request_redraw();
    }

    fn push_input_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let before = self.snapshot_state();
        self.input_buffer.push_str(text);
        self.record_state_change(before);
        self.base.request_redraw();
    }

    /// Returns the close button center for a tag chip at the given pixel position.
    fn tag_close_center(&self, chip_x: i32, chip_width: i32, chip_y: i32) -> Point {
        Point::new(chip_x + chip_width - TAG_PADDING - TAG_CLOSE_RADIUS, chip_y + TAG_HEIGHT / 2)
    }

    /// Hit-tests whether a point is within the close button of any tag chip.
    /// Returns the tag index if a close button was hit, or `None`.
    fn hit_tag_close(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let mut current_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let max_width = rect.width as i32 - TAG_PADDING * 2;

        for (index, tag) in self.tags.iter().enumerate() {
            let chip_width = self.compute_chip_width(tag).min(max_width);

            // Check if the close button within this chip was hit
            let close_center = self.tag_close_center(current_x, chip_width, chip_y);
            let dx = (pos.x - close_center.x) as i64;
            let dy = (pos.y - close_center.y) as i64;
            if dx * dx + dy * dy <= (TAG_CLOSE_RADIUS as i64 + 2) * (TAG_CLOSE_RADIUS as i64 + 2) {
                return Some(index);
            }

            current_x += chip_width + TAG_GAP;
        }
        None
    }

    /// Computes the width of a tag chip based on its text content.
    fn compute_chip_width(&self, tag: &str) -> i32 {
        let text_width = tag.len() as i32 * 7 + 12; // approximate pixel width
                                                    // Add space for close button + padding
        text_width + TAG_PADDING * 2 + TAG_CLOSE_RADIUS * 2 + 4
    }
}

impl Widget for TagInput {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `TagInput`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The tag list is published as
/// the comma-joined string the schema declares and is read-only: there is no
/// writer for a collection through a single string, and the legacy write side had
/// no `tags` arm either, so `set` refuses the name rather than inventing a parse.
impl WidgetProperties for TagInput {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "tags" => Ok(CapabilityValue::String(self.tags().join(","))),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "tags" => Err(CapabilityAccessError::ReadOnlyProperty),
            "placeholder" => {
                self.set_placeholder(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["tags", "placeholder", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `tag_input` publishes.
    ///
    /// `add_tag` and `remove_tag` name a tag, so a bare command has nothing to act on:
    /// there is no "current tag" for either to fall back to, and inventing one would
    /// mean the command silently added or deleted a tag the caller never named. They are
    /// therefore answered through the property route. `set_placeholder` is the same
    /// shape — it assigns the placeholder string, `set` already carries the arm, and the
    /// `placeholder` property is published. A caller reaches all three through `set`.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tag" | "remove_tag" | "set_placeholder" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for TagInput {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // ── Background ──
        // Resolved from the style so a theme switch reaches this control; the three-state
        // ladder is *derived* from the resolved colour rather than being three fixed greys, so
        // the states stay distinguishable and the control follows the theme.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("tag_input");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        let accent = style.border_color.or(themed_border);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(235, 235, 235, 200));
        let bg_color = if !is_enabled {
            base_bg.blend(&Color::WHITE, 0.35)
        } else if self.focused {
            base_bg.blend(&accent.unwrap_or(Color::rgba(60, 140, 255, 200)), 0.18)
        } else {
            base_bg
        };
        context.fill_rounded_rect(rect, 6, bg_color);

        // — Focus border —
        if self.focused && is_enabled {
            context.draw_rounded_rect_stroke(
                rect,
                6,
                accent.unwrap_or(Color::rgba(60, 140, 255, 200)),
                2,
            );
        } else {
            context.draw_rounded_rect_stroke(
                rect,
                6,
                style.border_color.unwrap_or(Color::rgba(200, 200, 200, 160)),
                1,
            );
        }

        let mut current_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let max_width = rect.width as i32 - TAG_PADDING * 2;
        let default_font = crate::core::Font::default();

        // ── Draw tag chips ──
        for tag in &self.tags {
            let chip_width = self.compute_chip_width(tag).min(max_width);
            let chip_rect = Rect::new(current_x, chip_y, chip_width as u32, TAG_HEIGHT as u32);

            // Chip background
            // The chip is this control's own chrome — a filled pill holding a tag — so its
            // fill follows the resolved accent rather than a fixed Material blue. The close
            // button is then the chip's *inverse* (white circle, accent X), which is what a
            // "remove this" affordance looks like on a coloured pill; deriving it keeps the
            // pair legible whatever the theme's accent is.
            let chip_bg = if is_enabled {
                accent.unwrap_or(Color::rgb(25, 118, 210))
            } else {
                Color::rgba(180, 180, 180, 160)
            };
            context.fill_rounded_rect(chip_rect, TAG_CHIP_RADIUS, chip_bg);

            // Chip text: the contrasting ink for the fill, so it stays legible when the
            // theme's accent is light.
            let text_color = chip_bg.contrast_color();
            let text_x = current_x + TAG_PADDING;
            // The origin is the glyph box's top edge, so the row's centre is half the
            // difference of the line boxes — `chip_y + TAG_HEIGHT/2` began the box half a
            // line below the chip's middle.
            let tag_metrics = context.measure_text(tag, &default_font);
            let text_origin =
                Point::new(text_x, chip_y + (TAG_HEIGHT - tag_metrics.height as i32) / 2);
            context.draw_text(
                text_origin,
                tag,
                &default_font,
                text_color,
                HorizontalAlignment::Left,
            );

            // Close button circle. It is the chip's *contrast* colour rather than white: on a
            // light-accent theme a white circle on the chip is a low-contrast disc, and the
            // "remove this" affordance has to read on whatever accent the theme supplies.
            let close_center = self.tag_close_center(current_x, chip_width, chip_y);
            let close_bg = text_color;
            context.fill_circle(close_center, TAG_CLOSE_RADIUS as u32, close_bg);

            // X mark (two diagonal lines)
            let x_offset = (TAG_CLOSE_RADIUS as f32 * 0.45) as i32;
            let close_fg = chip_bg;
            context.draw_line(
                Point::new(close_center.x - x_offset, close_center.y - x_offset),
                Point::new(close_center.x + x_offset, close_center.y + x_offset),
                close_fg,
            );
            context.draw_line(
                Point::new(close_center.x + x_offset, close_center.y - x_offset),
                Point::new(close_center.x - x_offset, close_center.y + x_offset),
                close_fg,
            );

            current_x += chip_width + TAG_GAP;
        }

        // ── Input area ──
        let input_x = current_x;
        let input_width = (rect.width as i32 - input_x - TAG_PADDING).max(MIN_INPUT_WIDTH);
        let input_rect = Rect::new(input_x, chip_y, input_width as u32, TAG_HEIGHT as u32);

        // Input background (slightly inset).
        //
        // The field is a band *of* the tag box, so it is one step from the resolved background
        // rather than a white wash: the literal `rgba(255,255,255,180)` was a light-theme
        // assumption written as a colour, and on the dark appearance it bleached the box's own
        // panel. The same mistake on the inks made the placeholder — which is the only thing
        // the field shows when it is empty — measure 1.25:1 against the field it sits on.
        let base_ink = self
            .style()
            .text_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
            .unwrap_or_else(|| bg_color.contrast_color());
        let input_bg = if !is_enabled {
            bg_color.blend(&base_ink, 0.04)
        } else {
            bg_color.blend(&base_ink, 0.08)
        };
        context.fill_rounded_rect(input_rect, 4, input_bg);

        // Input text. The typed text is the field's ink; the placeholder is that same ink
        // dimmed, and both are pushed clear of the field so neither can disappear into it.
        let typed_color = base_ink.legible_on(input_bg, 4.5);
        let hint_color = typed_color.blend(&input_bg, 0.35).legible_on(input_bg, 4.5);
        let input_text_color = if !is_enabled {
            typed_color.blend(&input_bg, 0.6)
        } else if !self.input_buffer.is_empty() {
            typed_color
        } else {
            hint_color
        };
        let display_text = if self.input_buffer.is_empty() && self.tags.is_empty() {
            self.placeholder.as_str()
        } else if self.input_buffer.is_empty() {
            ""
        } else {
            &self.input_buffer
        };
        // The placeholder and the typed text are fitted to the input area, and the caret is
        // positioned from the **fitted** text's measured width. `input_buffer.len() * 7`
        // was wrong twice: `len()` counts bytes, so a CJK tag moved the caret four times too
        // far, and the advance the renderer uses is the font size, not 7. A caret that
        // disagrees with the glyphs beside it is a visible defect on its own.
        let input_band =
            Rect::new(input_x + 4, chip_y, input_rect.width.saturating_sub(8), TAG_HEIGHT as u32);
        // The placeholder/typed text goes in the input band's own line box, not at the band's
        // top edge. `draw_text_fitted` aligns horizontally only — its origin is `bounds.y`
        // unchanged — so handing it the chip-height band pinned the text to the top of the
        // field: `tag_input.svg` put a 14 px line at `y = 4` inside a band running `4..28`.
        // The chip labels in this same file already centred themselves correctly; the field
        // was the one place that had not been brought in line.
        let input_line = context.text_line(input_band, &default_font);
        let drawn = context.draw_text_fitted(
            input_line,
            display_text,
            &default_font,
            input_text_color,
            HorizontalAlignment::Left,
        );

        // ── Cursor (when focused and input is active) ──
        if self.focused && is_enabled && self.cursor_blink.is_visible() {
            let cursor_x = input_x + 4 + context.measure_text(&drawn, &default_font).width as i32;
            // The caret spans the line box it follows, so it marks the same row the text does.
            let cursor_y1 = input_line.y;
            let cursor_y2 = input_line.y + input_line.height as i32;
            // Clamped to the input box: a caret at the far right of a full buffer belongs on
            // the last pixel of the field, not past its edge.
            let cursor_x = cursor_x.min(input_rect.x + input_rect.width as i32 - 1);
            context.draw_line(
                Point::new(cursor_x, cursor_y1),
                Point::new(cursor_x, cursor_y2),
                Color::rgba(60, 140, 255, 200),
            );
        }
    }
}

impl EventHandler for TagInput {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                // Check if the click is on a tag's close button
                if let Some(tag_index) = self.hit_tag_close(*pos) {
                    self.remove_tag(tag_index);
                    return;
                }
                // Click anywhere else in the widget → gain focus
                self.set_focused(true);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                // No special release handling needed
            }
            Event::FocusGained => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                // Commit any pending input on focus loss
                if !self.input_buffer.is_empty() {
                    self.commit_input();
                }
                self.set_focused(false);
            }
            Event::KeyPress { key, modifiers } => {
                if !self.focused {
                    return;
                }
                if *key == 90 && *modifiers == 2 {
                    let _ = self.undo();
                    return;
                }
                if *key == 89 && *modifiers == 2 {
                    let _ = self.redo();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — remove last character from buffer
                        // If buffer is empty, remove the last tag
                        if !self.input_buffer.is_empty() {
                            let before = self.snapshot_state();
                            self.input_buffer.pop();
                            self.record_state_change(before);
                            self.base.request_redraw();
                        } else if !self.tags.is_empty() {
                            let before = self.snapshot_state();
                            self.tags.pop();
                            self.record_state_change(before);
                            self.tags_changed.emit(self.tags.clone());
                            self.base.request_redraw();
                        }
                    }
                    13 => {
                        // Enter — commit the current input as a tag
                        self.commit_input();
                    }
                    44 => {
                        // Comma — commit the current input as a tag
                        self.commit_input();
                    }
                    27 => {
                        // Escape — lose focus
                        self.set_focused(false);
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.push_input_text(&ch.to_string());
                            }
                        }
                    }
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if self.focused {
                    self.push_input_text(text);
                }
            }
            Event::ImePreedit { .. } => {}
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn tag_input_default_creation() {
        let ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(ti.tags().is_empty());
        assert!(!ti.is_focused());
        assert_eq!(ti.kind(), WidgetKind::TagInput);
        assert!(ti.is_enabled());
    }

    #[test]
    fn tag_input_add_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        assert_eq!(ti.tags(), &["rust"]);
    }

    #[test]
    fn tag_input_add_tag_trims_whitespace() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("  hello  ");
        assert_eq!(ti.tags(), &["hello"]);
    }

    #[test]
    fn tag_input_ignores_empty_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("");
        ti.add_tag("  ");
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_add_tag_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let captured = Arc::new(Mutex::new(None::<Vec<String>>));
        ti.tags_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Vec<String>>| {
                *captured.lock().unwrap() = Some(val.to_vec());
            }
        });

        ti.add_tag("rust");
        assert_eq!(*captured.lock().unwrap(), Some(vec!["rust".to_string()]));
    }

    #[test]
    fn tag_input_remove_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");
        ti.add_tag("gamma");
        assert_eq!(ti.tags().len(), 3);

        ti.remove_tag(1); // remove "beta"
        assert_eq!(ti.tags(), &["alpha", "gamma"]);
    }

    #[test]
    fn tag_input_remove_tag_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");

        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.remove_tag(0);
        assert_eq!(*count.lock().unwrap(), 1);
        assert_eq!(ti.tags(), &["beta"]);
    }

    #[test]
    fn tag_input_remove_tag_out_of_bounds() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("only");
        ti.remove_tag(5); // out of bounds — no-op
        assert_eq!(ti.tags().len(), 1);
    }

    #[test]
    fn tag_input_clear_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("a");
        ti.add_tag("b");
        ti.clear_tags();
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_clear_tags_emits_signal() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("data");

        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.clear_tags();
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn tag_input_clear_empty_does_not_emit() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let count = Arc::new(Mutex::new(0));
        ti.tags_changed.connect({
            let count = Arc::clone(&count);
            move |_: Arc<Vec<String>>| {
                *count.lock().unwrap() += 1;
            }
        });

        ti.clear_tags();
        assert_eq!(*count.lock().unwrap(), 0);
    }

    #[test]
    fn tag_input_keyboard_typing() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 104, modifiers: 0 }); // 'h'
        ti.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
        assert_eq!(ti.input_buffer, "hi");
    }

    #[test]
    fn tag_input_enter_creates_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 114, modifiers: 0 }); // 'r'
        ti.handle_event(&Event::KeyPress { key: 117, modifiers: 0 }); // 'u'
        ti.handle_event(&Event::KeyPress { key: 115, modifiers: 0 }); // 's'
        ti.handle_event(&Event::KeyPress { key: 116, modifiers: 0 }); // 't'
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter

        assert_eq!(ti.tags(), &["rust"]);
        assert!(ti.input_buffer.is_empty());
    }

    #[test]
    fn tag_input_comma_creates_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 103, modifiers: 0 }); // 'g'
        ti.handle_event(&Event::KeyPress { key: 111, modifiers: 0 }); // 'o'
        ti.handle_event(&Event::KeyPress { key: 44, modifiers: 0 }); // Comma

        assert_eq!(ti.tags(), &["go"]);
        assert!(ti.input_buffer.is_empty());
    }

    #[test]
    fn tag_input_backspace_removes_last_char_or_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        // Type "ab" then backspace removes 'b'
        ti.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        ti.handle_event(&Event::KeyPress { key: 98, modifiers: 0 }); // 'b'
        ti.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(ti.input_buffer, "a");

        // Commit and test backspace removes last tag
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
        assert_eq!(ti.tags(), &["a"]);

        // Backspace when buffer empty — removes last tag
        ti.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_undo_redo_restores_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");

        assert!(ti.can_undo());
        assert!(ti.undo());
        assert_eq!(ti.tags(), &["alpha"]);
        assert!(ti.can_redo());
        assert!(ti.redo());
        assert_eq!(ti.tags(), &["alpha", "beta"]);
    }

    #[test]
    fn tag_input_undo_redo_restores_input_buffer() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        ti.handle_event(&Event::text_input("你好"));
        assert_eq!(ti.input_buffer, "你好");

        ti.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(ti.input_buffer, "");
        ti.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
        assert_eq!(ti.input_buffer, "你好");
    }

    #[test]
    fn tag_input_commit_history_restores_complete_state() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        ti.handle_event(&Event::text_input("rust"));
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(ti.tags(), &["rust"]);
        assert_eq!(ti.input_buffer, "");

        assert!(ti.undo());
        assert!(ti.tags().is_empty());
        assert_eq!(ti.input_buffer, "rust");
    }

    #[test]
    fn tag_input_focus_on_click() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        assert!(!ti.is_focused());

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1 });
        assert!(ti.is_focused());
    }

    #[test]
    fn tag_input_focus_lost_on_escape() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);
        assert!(ti.is_focused());

        ti.handle_event(&Event::KeyPress { key: 27, modifiers: 0 }); // Escape
        assert!(!ti.is_focused());
    }

    #[test]
    fn tag_input_disabled_blocks_events() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_enabled(false);

        ti.handle_event(&Event::MousePress { pos: Point::new(50, 18), button: 1 });
        assert!(!ti.is_focused());
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_avoid_duplicate_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        ti.add_tag("rust"); // duplicate — should be ignored
        assert_eq!(ti.tags().len(), 1);
    }

    #[test]
    fn tag_input_multiple_tags() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("alpha");
        ti.add_tag("beta");
        ti.add_tag("gamma");
        assert_eq!(ti.tags(), &["alpha", "beta", "gamma"]);
    }

    #[test]
    fn tag_input_svg_output() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("rust");
        ti.add_tag("gui");

        let svg = crate::widget::svg::render_to_svg(&mut ti);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"36\""));
    }

    #[test]
    fn tag_input_svg_empty() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        let svg = crate::widget::svg::render_to_svg(&mut ti);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn tag_input_close_click_removes_tag() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.add_tag("removable");

        // Compute expected close button position for the first (and only) tag chip
        let rect = ti.geometry();
        let chip_x = rect.x + TAG_PADDING;
        let chip_y = rect.y + TAG_VERTICAL_PADDING;
        let chip_width =
            ti.compute_chip_width("removable").min(rect.width as i32 - TAG_PADDING * 2);
        let close_center = ti.tag_close_center(chip_x, chip_width, chip_y);

        // Click on the close button
        ti.handle_event(&Event::MousePress { pos: close_center, button: 1 });
        assert!(ti.tags().is_empty());
    }

    #[test]
    fn tag_input_keyboard_input_emits_signal_on_enter() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        let captured = Arc::new(Mutex::new(None::<Vec<String>>));
        ti.tags_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<Vec<String>>| {
                *captured.lock().unwrap() = Some(val.to_vec());
            }
        });

        ti.handle_event(&Event::KeyPress { key: 99, modifiers: 0 }); // 'c'
        ti.handle_event(&Event::KeyPress { key: 111, modifiers: 0 }); // 'o'
        ti.handle_event(&Event::KeyPress { key: 100, modifiers: 0 }); // 'd'
        ti.handle_event(&Event::KeyPress { key: 101, modifiers: 0 }); // 'e'
        ti.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter

        assert_eq!(*captured.lock().unwrap(), Some(vec!["code".to_string()]));
    }

    #[test]
    fn tag_input_focus_lost_commits_input() {
        let mut ti = TagInput::new(Rect::new(0, 0, 300, 36));
        ti.set_focused(true);

        ti.handle_event(&Event::KeyPress { key: 112, modifiers: 0 }); // 'p'
        ti.handle_event(&Event::KeyPress { key: 101, modifiers: 0 }); // 'e'
        ti.handle_event(&Event::KeyPress { key: 110, modifiers: 0 }); // 'n'

        // Focus lost — should commit pending input
        ti.handle_event(&Event::FocusLost);
        assert_eq!(ti.tags(), &["pen"]);
        assert!(!ti.is_focused());
    }
}
