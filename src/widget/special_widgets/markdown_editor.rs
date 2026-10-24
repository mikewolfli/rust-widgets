// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MarkdownEditor widget.

use crate::core::{Color, HorizontalAlignment, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::Signal1;
use crate::undo::{CommandDescription, CommandId, UndoCommand, UndoStack};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::{effective_font, estimate_text_width};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// The top of the first text row below the header band.
const TEXT_TOP: i32 = 36;
/// The pitch between text rows.
const TEXT_ROW_HEIGHT: i32 = 16;
/// The bottom inset a row has to clear to be painted.
const TEXT_BOTTOM_INSET: i32 = 8;

/// An undo step that records only the bytes that differ between two document states.
///
/// A whole-document snapshot would store both versions on every keystroke. The common
/// prefix and suffix are found on character boundaries, so the command keeps only the
/// changed run and an offset, and an undo touches O(edit) bytes instead of O(document).
struct MarkdownTextDeltaCommand {
    id: CommandId,
    target: Rc<RefCell<String>>,
    offset: usize,
    removed: String,
    inserted: String,
}

impl MarkdownTextDeltaCommand {
    /// Builds a command for a mutation that has already been applied.
    fn new(target: Rc<RefCell<String>>, before: &str, after: &str) -> Self {
        let mut prefix = 0usize;
        for (old, new) in before.chars().zip(after.chars()) {
            if old != new {
                break;
            }
            prefix += old.len_utf8();
        }
        let mut suffix = 0usize;
        for ((_, old), (_, new)) in
            before[prefix..].char_indices().rev().zip(after[prefix..].char_indices().rev())
        {
            if old != new {
                break;
            }
            suffix += old.len_utf8();
        }
        Self {
            id: CommandId(
                NEXT_MARKDOWN_COMMAND_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            ),
            target,
            offset: prefix,
            removed: before[prefix..before.len() - suffix].to_string(),
            inserted: after[prefix..after.len() - suffix].to_string(),
        }
    }

    /// Replaces the recorded range with `insert`.
    fn splice(&self, remove_len: usize, insert: &str) {
        let mut text = self.target.borrow_mut();
        let start = self.offset.min(text.len());
        let end = (start + remove_len).min(text.len());
        if start <= end {
            text.replace_range(start..end, insert);
        }
    }
}

/// Counter for [`MarkdownTextDeltaCommand`] ids.
static NEXT_MARKDOWN_COMMAND_ID: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(1);

impl UndoCommand for MarkdownTextDeltaCommand {
    fn id(&self) -> CommandId {
        self.id
    }

    fn description(&self) -> CommandDescription {
        CommandDescription {
            text: "Edit text".to_string(),
            timestamp_ms: 0,
            command_type: "markdown_editor_text",
        }
    }

    fn execute(&mut self) -> Result<(), String> {
        self.splice(self.removed.len(), &self.inserted.clone());
        Ok(())
    }

    fn undo(&mut self) -> Result<(), String> {
        self.splice(self.inserted.len(), &self.removed.clone());
        Ok(())
    }
}

/// One heading in the document outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownOutlineEntry {
    /// Zero-based line index.
    pub line: usize,
    /// Heading level, `1..=6`.
    pub level: u8,
    /// Heading text without its `#` markers.
    pub title: String,
}

/// Lightweight markdown editor with preview toggle and metrics.
pub struct MarkdownEditor {
    base: BaseWidget,
    text: String,
    preview_mode: bool,
    cursor_line: usize,
    /// Character offset of the caret within [`Self::cursor_line`].
    cursor_col: usize,
    /// The first text line the viewport paints.
    ///
    /// The editor used to paint a fixed ten lines, so a longer document simply ended at row ten.
    /// This is the scroll state the wheel drives; the paint path starts here and takes only the
    /// rows that fit.
    first_visible_line: usize,
    /// Emitted when text changes.
    pub text_changed: Signal1<String>,
    /// Emitted when preview mode changes.
    pub preview_mode_changed: Signal1<bool>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl MarkdownEditor {
    /// Creates editor.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::RichEdit, geometry, "MarkdownEditor"),
            text: String::new(),
            preview_mode: false,
            cursor_line: 0,
            cursor_col: 0,
            first_visible_line: 0,
            text_changed: Signal1::new(),
            preview_mode_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Returns markdown text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets markdown text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let next = text.into();
        self.commit_text(next);
    }

    /// Inserts `text` at the caret and leaves the caret after the inserted run.
    ///
    /// This is the entry point for platform text input (`Event::TextInput` and
    /// `Event::ImeCommit`). The write goes through `commit_text`, so it is one undo step
    /// and emits `text_changed` exactly like every other text mutation.
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        // Control characters are protocol, not keystrokes: a `\n` delivered through
        // `TextInput` must not split a line, and an escape must not enter the document.
        // This is the same filter `Mention` applies to the same event.
        let filtered: String = text.chars().filter(|ch| !ch.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        let mut next = String::with_capacity(self.text.len() + filtered.len());
        next.push_str(&self.text[..at]);
        next.push_str(&filtered);
        next.push_str(&self.text[at..]);
        self.cursor_col += filtered.chars().count();
        self.commit_text(next);
    }

    /// Appends one line to markdown.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        let mut next = self.text.clone();
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str(line.as_ref());
        self.set_text(next);
        self.cursor_line = self.edit_line_count().saturating_sub(1);
        self.cursor_col = self.current_line_char_len();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Toggles preview mode.
    pub fn toggle_preview_mode(&mut self) {
        self.preview_mode = !self.preview_mode;
        self.preview_mode_changed.emit(self.preview_mode);
        self.base.request_redraw();
    }

    /// Sets preview mode.
    pub fn set_preview_mode(&mut self, preview_mode: bool) {
        if self.preview_mode == preview_mode {
            return;
        }
        self.preview_mode = preview_mode;
        self.preview_mode_changed.emit(preview_mode);
        self.base.request_redraw();
    }

    /// Returns whether preview mode is enabled.
    pub fn preview_mode(&self) -> bool {
        self.preview_mode
    }

    /// Returns line count.
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            0
        } else {
            self.text.lines().count()
        }
    }

    /// Returns word count.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// Returns heading count.
    pub fn heading_count(&self) -> usize {
        self.text.lines().filter(|line| line.trim_start().starts_with('#')).count()
    }

    /// Returns headings in document order, as navigation targets.
    pub fn outline(&self) -> Vec<MarkdownOutlineEntry> {
        self.text
            .lines()
            .enumerate()
            .filter_map(|(line, text)| {
                let trimmed = text.trim_start();
                let level = trimmed.chars().take_while(|ch| *ch == '#').count();
                (level > 0).then_some(MarkdownOutlineEntry {
                    line,
                    level: level.min(6) as u8,
                    title: trimmed[level..].trim().to_string(),
                })
            })
            .collect()
    }

    /// Moves the caret to outline entry `index`.
    pub fn goto_outline(&mut self, index: usize) -> bool {
        let Some(entry) = self.outline().get(index).cloned() else {
            return false;
        };
        self.cursor_line = entry.line;
        self.cursor_col = 0;
        // Scroll the target into view rather than leaving it off screen.
        if entry.line < self.first_visible_line {
            self.first_visible_line = entry.line;
        } else if entry.line >= self.first_visible_line + self.visible_line_count() {
            self.first_visible_line = entry.line + 1 - self.visible_line_count();
        }
        self.base.request_redraw();
        true
    }

    /// Returns current line index.
    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    /// Returns the caret's character offset within the current line.
    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Reverts the most recent markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise requests layout and redraw and clamps `cursor_line`.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone markdown text mutation.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise requests layout and redraw.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is a markdown text mutation to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether there is an undone markdown text mutation to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.clamp_cursor();
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Number of editable lines.
    ///
    /// This differs from [`Self::line_count`] only for a trailing newline: `str::lines`
    /// drops the final empty line, but an editor must be able to keep the caret on it,
    /// which is exactly where Enter at the end of the document leaves it.
    fn edit_line_count(&self) -> usize {
        self.text.split('\n').count()
    }

    /// How many text rows fit below the header band.
    ///
    /// At least one, so a short editor still paints the line it is on instead of dividing to
    /// zero or scrolling everything away.
    fn visible_line_count(&self) -> usize {
        let usable = self.geometry().height as i32 - TEXT_TOP - TEXT_BOTTOM_INSET;
        if usable < 0 {
            return 1;
        }
        (usable / TEXT_ROW_HEIGHT + 1) as usize
    }

    /// The first line the viewport actually starts at, clamped to the last full window.
    fn visible_start(&self) -> usize {
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        self.first_visible_line.min(max_first)
    }

    /// Scrolls the viewport by `lines` rows toward the end of the document.
    ///
    /// Positive values reveal later lines, negative values reveal earlier ones. Returns whether
    /// the viewport moved, so the caller repaints only on a change.
    fn scroll_lines(&mut self, lines: i32) -> bool {
        if lines == 0 {
            return false;
        }
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        let next =
            (self.first_visible_line as i64 + lines as i64).clamp(0, max_first as i64) as usize;
        if next == self.first_visible_line {
            return false;
        }
        self.first_visible_line = next;
        true
    }

    /// Byte span of the caret's line in `text`.
    ///
    /// Returns an empty span at the end of the document when the caret is out of range, so
    /// callers cannot index past the string even before [`Self::clamp_cursor`] runs.
    fn cursor_line_span(&self) -> (usize, usize) {
        let mut start = 0usize;
        for (idx, line) in self.text.split('\n').enumerate() {
            if idx == self.cursor_line {
                return (start, start + line.len());
            }
            start += line.len() + 1;
        }
        (self.text.len(), self.text.len())
    }

    /// Byte offset of the caret in `text`.
    fn cursor_byte_offset(&self) -> usize {
        let (start, end) = self.cursor_line_span();
        let line = &self.text[start..end];
        start
            + line
                .char_indices()
                .nth(self.cursor_col)
                .map(|(offset, _)| offset)
                .unwrap_or(line.len())
    }

    /// Character length of the caret's line.
    fn current_line_char_len(&self) -> usize {
        let (start, end) = self.cursor_line_span();
        self.text[start..end].chars().count()
    }

    /// Keeps `cursor_line` / `cursor_col` inside the document.
    fn clamp_cursor(&mut self) {
        self.cursor_line = self.cursor_line.min(self.edit_line_count().saturating_sub(1));
        self.clamp_cursor_col();
    }

    /// Keeps `cursor_col` inside the caret's line.
    fn clamp_cursor_col(&mut self) {
        self.cursor_col = self.cursor_col.min(self.current_line_char_len());
    }

    /// Applies a new document state through the undo snapshot route.
    ///
    /// Every text mutation funnels through here so a single place owns the history
    /// snapshot, the `restoring_history` guard, the caret clamp and the signal.
    fn commit_text(&mut self, next: String) {
        if self.text == next {
            return;
        }
        let before = core::mem::take(&mut self.text);
        self.text = next;
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(MarkdownTextDeltaCommand::new(
                self.history_target.clone(),
                &before,
                &self.text,
            )));
        }
        self.clamp_cursor();
        self.text_changed.emit(self.text.clone());
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Deletes the character before the caret, merging lines at a line start.
    fn backspace_at_cursor(&mut self) {
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        if self.cursor_col > 0 {
            let (start, _) = self.cursor_line_span();
            let removed = self.text[start..at].chars().next_back().map(char::len_utf8).unwrap_or(0);
            let mut next = String::with_capacity(self.text.len());
            next.push_str(&self.text[..at - removed]);
            next.push_str(&self.text[at..]);
            self.cursor_col -= 1;
            self.commit_text(next);
        } else if self.cursor_line > 0 {
            let (start, _) = self.cursor_line_span();
            let merged_col =
                self.text[..start - 1].chars().rev().take_while(|&ch| ch != '\n').count();
            let mut next = String::with_capacity(self.text.len());
            next.push_str(&self.text[..start - 1]);
            next.push_str(&self.text[start..]);
            self.cursor_line -= 1;
            self.cursor_col = merged_col;
            self.commit_text(next);
        }
    }

    /// Splits the caret's line at the caret.
    fn insert_newline_at_cursor(&mut self) {
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        let mut next = String::with_capacity(self.text.len() + 1);
        next.push_str(&self.text[..at]);
        next.push('\n');
        next.push_str(&self.text[at..]);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.commit_text(next);
    }

    /// Moves the caret one character left, wrapping to the previous line's end.
    fn move_cursor_left(&mut self) {
        self.clamp_cursor();
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.base.request_redraw();
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.current_line_char_len();
            self.base.request_redraw();
        }
    }

    /// Moves the caret one character right, wrapping to the next line's start.
    fn move_cursor_right(&mut self) {
        self.clamp_cursor();
        if self.cursor_col < self.current_line_char_len() {
            self.cursor_col += 1;
            self.base.request_redraw();
        } else if self.cursor_line + 1 < self.edit_line_count() {
            self.cursor_line += 1;
            self.cursor_col = 0;
            self.base.request_redraw();
        }
    }

    fn move_cursor(&mut self, delta: isize) {
        let lines = self.edit_line_count();
        let current = self.cursor_line as isize;
        let next = (current + delta).clamp(0, lines.saturating_sub(1) as isize) as usize;
        if next != self.cursor_line {
            self.cursor_line = next;
            // A column past the new line's end would put the caret outside the text, so
            // vertical movement keeps the column but clamps it to the line it landed on.
            self.clamp_cursor_col();
            self.base.request_redraw();
        }
    }
}

impl Widget for MarkdownEditor {
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
        crate::core::Size::new(500, 300)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "preview_mode_changed" => Some(EventSignalRef::mapped(
                "preview_mode_changed",
                &self.preview_mode_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            _ => None,
        }
    }
}

/// `MarkdownEditor`'s property contract.
///
/// `text` writes go through `set_text`, which keeps the undo stack and the
/// emitted `text_changed` signal in step with the stored document; writing the
/// field directly would make the editor's own history wrong. `cursor_line` is
/// read-only because the editor owns caret placement.
impl WidgetProperties for MarkdownEditor {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "preview_mode" => Ok(CapabilityValue::Bool(self.preview_mode())),
            "line_count" => Ok(CapabilityValue::UInt(self.line_count() as u64)),
            "word_count" => Ok(CapabilityValue::UInt(self.word_count() as u64)),
            "heading_count" => Ok(CapabilityValue::UInt(self.heading_count() as u64)),
            "cursor_line" => Ok(CapabilityValue::UInt(self.cursor_line() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => match value {
                CapabilityValue::String(text) => {
                    self.set_text(text);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "preview_mode" => match value {
                CapabilityValue::Bool(enabled) => {
                    self.set_preview_mode(enabled);
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "line_count" | "word_count" | "heading_count" | "cursor_line" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "preview_mode",
            "line_count",
            "word_count",
            "heading_count",
            "cursor_line",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `markdown_editor` publishes.
    ///
    /// `toggle_preview_mode` and `undo` are the genuine zero-argument actions here;
    /// `undo` reports `false` when there is nothing to undo, which is the "could not
    /// handle it" case, so it is answered with
    /// [`CapabilityAccessError::OutOfRange`] rather than a success that did nothing.
    /// `append_line` takes the line to append and `set_text` / `set_preview_mode`
    /// assign state through the property route, so those are refused the same way —
    /// the names are right and the payload is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle_preview_mode" => {
                self.toggle_preview_mode();
                Ok(())
            }
            "undo" => {
                if self.undo() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "append_line" | "set_text" | "set_preview_mode" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MarkdownEditor {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            // Printable text and IME commits are what the platform delivers for typing; a
            // handler with only a `KeyPress` arm never sees them. Both insert at the caret
            // through the same undo-guarded path as `set_text` (the filter mirrors
            // `Mention`, which refuses control characters and empty runs).
            Event::TextInput { text }
                if text.chars().all(|ch| !ch.is_control()) && !text.is_empty() =>
            {
                self.insert_text_at_cursor(text);
            }
            Event::ImeCommit { text } => {
                self.insert_text_at_cursor(text);
            }
            Event::KeyPress { key, modifiers } => match *key {
                // Undo and redo are the platform primary chord: Control+Z/Y on
                // Windows and Linux, Command+Z/Y on macOS. A shifted chord is a
                // different binding and stays unbound here.
                _ if primary_shortcut_pressed(*key, *modifiers, Key::Z) => {
                    let _ = self.undo();
                }
                _ if primary_shortcut_pressed(*key, *modifiers, Key::Y) => {
                    let _ = self.redo();
                }
                8 => self.backspace_at_cursor(),
                13 => self.insert_newline_at_cursor(),
                37 => self.move_cursor_left(),
                39 => self.move_cursor_right(),
                38 => self.move_cursor(-1),
                40 => self.move_cursor(1),
                80 | 112 if primary_shortcut_pressed(*key, *modifiers, Key::P) => {
                    self.toggle_preview_mode()
                }
                _ => { /* Other keys are not relevant */ }
            },
            // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
            // start. Three lines per notch is the desktop default shared with the other text
            // controls.
            Event::MousePress { pos, button: 1, .. } => {
                // Clicking a text row moves the caret to the nearest character boundary on it.
                let rect = self.geometry();
                let row = (pos.y - rect.y - TEXT_TOP) / TEXT_ROW_HEIGHT;
                if row >= 0 {
                    let line = self.visible_start() + row as usize;
                    if line < self.edit_line_count() {
                        self.cursor_line = line;
                        let font = effective_font(self.base.style());
                        let x = pos.x - rect.x - 12;
                        let text = self.text.split('\n').nth(line).unwrap_or("");
                        let mut offset = 0i32;
                        self.cursor_col = 0;
                        for ch in text.chars() {
                            let width = estimate_text_width(&ch.to_string(), font, 1.0) as i32;
                            if x < offset + width / 2 {
                                break;
                            }
                            offset += width;
                            self.cursor_col += 1;
                        }
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseMove { .. } => {}
            Event::Wheel { delta, modifiers, .. } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 3);
                if self.scroll_lines(-scroll.lines_y) {
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for MarkdownEditor {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch changed nothing on screen, because the surface, the border
        // and every piece of text were hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("markdown_editor");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(252, 252, 253));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(194, 201, 213));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(59, 72, 92));
        // The header is the dimmest ink that still reads, and the caret line is the
        // accent: both derived from the resolved ink and surface, so they move with
        // the appearance instead of being fixed literals.
        //
        // The caret line is the accent colour because it *means* "the line being edited",
        // so the hue is part of the information and must survive. It cannot be pushed
        // straight onto the surface unchecked, though: on the light appearance the editor
        // surface resolves to a mid grey and `colors.primary` is the saturated Material
        // blue, which measured 1.51:1 — the very line the caret is on became the least
        // readable line in the editor. `legible_on` keeps the hue and guarantees the ratio.
        let header_ink = ink.blend(&surface, 0.35);
        let cursor_ink = crate::style::theme_manager()
            .current_theme()
            .map(|active| active.colors.primary.legible_on(surface, 4.5))
            .unwrap_or_else(|| ink.contrast_color());

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, border);

        // One pass over the text for all three counts, and the header is built from them.
        // `line_count`/`word_count`/`heading_count` each scanned the whole document, so the
        // header cost three full passes plus a `format!` on every repaint of a ten-line pane.
        let (line_total, word_total, heading_total) =
            self.text.lines().fold((0usize, 0usize, 0usize), |(lines, words, headings), line| {
                (
                    lines + 1,
                    words + line.split_whitespace().count(),
                    headings + usize::from(line.trim_start().starts_with('#')),
                )
            });
        let header = if self.preview_mode {
            format!(
                "Markdown Preview  lines:{line_total} words:{word_total} headings:{heading_total}"
            )
        } else {
            format!("Markdown Edit  lines:{line_total} words:{word_total} headings:{heading_total}")
        };
        // Fitted to the control's width: the header is a long line of counts, and a narrow
        // editor previously drew it straight past its own right edge.
        let header_band = Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), 14);
        context.draw_text_fitted_cow(
            header_band,
            &header,
            effective_font(style),
            header_ink,
            HorizontalAlignment::Left,
        );

        for (row, line) in
            self.text.lines().skip(self.visible_start()).take(self.visible_line_count()).enumerate()
        {
            let idx = self.visible_start() + row;
            let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
            // Rows below the control are not painted, which keeps a short editor from drawing
            // past its own bottom edge.
            if y > rect.y + rect.height as i32 - TEXT_BOTTOM_INSET {
                break;
            }
            let color = if idx == self.cursor_line { cursor_ink } else { ink };
            let band =
                Rect::new(rect.x + 12, y, rect.width.saturating_sub(24), TEXT_ROW_HEIGHT as u32);
            if self.preview_mode {
                draw_markdown_line(context, band, line, effective_font(style), color, cursor_ink);
            } else {
                // Fitted to the pane, so a long line ends in an ellipsis inside the control
                // instead of painting over the widget beside it.
                context.draw_text_fitted_cow(
                    band,
                    line,
                    effective_font(style),
                    color,
                    HorizontalAlignment::Left,
                );
            }
        }

        // Visible caret: a 1 px bar at the caret's line and column.
        //
        // `cursor_col` is a character offset, so the x position comes from measuring the
        // line prefix with the same estimator the layout uses — a byte-per-8px formula
        // would put the bar in the middle of a CJK glyph. Preview mode paints no caret
        // because there is nothing to edit there. The row's y matches the text rows
        // above, so the caret also lands on the blank trailing line that Enter creates.
        if !self.preview_mode {
            let start = self.visible_start();
            if let Some(row) =
                self.cursor_line.checked_sub(start).filter(|row| *row < self.visible_line_count())
            {
                let (line_start, line_end) = self.cursor_line_span();
                let prefix: String =
                    self.text[line_start..line_end].chars().take(self.cursor_col).collect();
                let x =
                    rect.x + 12 + estimate_text_width(&prefix, effective_font(style), 1.0) as i32;
                let y = rect.y + TEXT_TOP + (row as i32) * TEXT_ROW_HEIGHT;
                if y + 14 <= rect.y + rect.height as i32 && x <= rect.x + rect.width as i32 - 12 {
                    context.fill_rect(Rect::new(x, y - 2, 1, 16), cursor_ink);
                }
            }
        }
    }
}

/// Draws one preview row with the inline markdown the document actually uses.
///
/// Headings grow with their level, bullet items get a marker, `**bold**` is drawn with a
/// one-pixel overprint (the same trick the rest of the crate uses where the face has one
/// weight), `` `code` `` gets a tinted backing, and `[text](url)` is drawn in the accent with
/// its target omitted. The previous preview only removed leading `#` characters, so bold,
/// italics, code, lists and links all arrived as their source syntax.
fn draw_markdown_line(
    context: &mut RenderContext,
    band: Rect,
    line: &str,
    font: &crate::core::Font,
    color: Color,
    accent: Color,
) {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count().min(6);
    let (font, text) = if level > 0 {
        let size = font.size() + (7 - level) as f32;
        (font.with_size(size), trimmed[level..].trim_start())
    } else {
        (font.clone(), line)
    };
    let mut x = band.x;
    let right = band.x + band.width as i32;
    let bullet = text.starts_with("- ") || text.starts_with("* ") || text.starts_with("+ ");
    if bullet {
        let marker = "•";
        context.draw_text(
            crate::core::Point::new(x, band.y),
            marker,
            &font,
            accent,
            HorizontalAlignment::Left,
        );
        x += context.measure_text(marker, &font).width as i32 + 4;
    }
    let body = if bullet { &text[2..] } else { text };
    let mut chars = body.chars().peekable();
    let mut run = String::new();
    let draw_run = |context: &mut RenderContext, run: &mut String, x: &mut i32, bold: bool| {
        if run.is_empty() {
            return;
        }
        let text = core::mem::take(run);
        context.draw_text(
            crate::core::Point::new(*x, band.y),
            &text,
            &font,
            color,
            HorizontalAlignment::Left,
        );
        *x += context.measure_text(&text, &font).width as i32;
        if bold {
            context.draw_text(
                crate::core::Point::new(*x - 1, band.y),
                &text,
                &font,
                color,
                HorizontalAlignment::Left,
            );
        }
    };
    while let Some(ch) = chars.next() {
        if x >= right {
            break;
        }
        match ch {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                draw_run(context, &mut run, &mut x, false);
                let mut inner = String::new();
                while let Some(next) = chars.next() {
                    if next == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        break;
                    }
                    inner.push(next);
                }
                draw_run(context, &mut inner, &mut x, true);
            }
            '`' => {
                draw_run(context, &mut run, &mut x, false);
                let mut inner = String::new();
                for next in chars.by_ref() {
                    if next == '`' {
                        break;
                    }
                    inner.push(next);
                }
                let width = context.measure_text(&inner, &font).width;
                context.fill_rect(
                    Rect::new(x - 2, band.y, width + 4, band.height.max(1)),
                    color.with_alpha(24),
                );
                context.draw_text(
                    crate::core::Point::new(x, band.y),
                    &inner,
                    &font,
                    color,
                    HorizontalAlignment::Left,
                );
                x += width as i32 + 2;
            }
            '[' => {
                draw_run(context, &mut run, &mut x, false);
                let mut label = String::new();
                let mut is_link = false;
                while let Some(next) = chars.next() {
                    if next == ']' {
                        if chars.peek() == Some(&'(') {
                            chars.next();
                            for target in chars.by_ref() {
                                if target == ')' {
                                    break;
                                }
                            }
                            is_link = true;
                        }
                        break;
                    }
                    label.push(next);
                }
                context.draw_text(
                    crate::core::Point::new(x, band.y),
                    &label,
                    &font,
                    if is_link { accent } else { color },
                    HorizontalAlignment::Left,
                );
                x += context.measure_text(&label, &font).width as i32;
            }
            _ => run.push(ch),
        }
    }
    draw_run(context, &mut run, &mut x, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::input_widgets::shortcut_editor::{
        non_primary_modifier_event_bits, primary_modifier_event_bits,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn metrics_follow_text_changes() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# Title\nhello world\n## Details");

        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.word_count(), 6);
        assert_eq!(editor.heading_count(), 2);
    }

    #[test]
    fn preview_toggle_emits_signal() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        let states = Arc::new(Mutex::new(Vec::<bool>::new()));
        let sink = states.clone();
        editor.preview_mode_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*value);
            }
        });

        editor.toggle_preview_mode();
        editor.toggle_preview_mode();

        let got = states.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![true, false]);
    }

    #[test]
    fn arrow_keys_move_cursor() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("a\nb\nc");

        editor.handle_event(&Event::key_press(40, 0));
        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor_line(), 2);

        editor.handle_event(&Event::key_press(38, 0));
        assert_eq!(editor.cursor_line(), 1);
    }

    #[test]
    fn undo_redo_restores_markdown_text() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("# One");
        editor.append_line("body");

        assert!(editor.can_undo());
        assert!(editor.undo());
        assert_eq!(editor.text(), "# One");
        assert!(editor.can_redo());
        assert!(editor.redo());
        assert_eq!(editor.text(), "# One\nbody");
    }

    #[test]
    fn keyboard_shortcuts_drive_markdown_history() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("draft");
        editor.set_text("final");

        editor.handle_event(&Event::KeyPress { key: 90, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.text(), "draft");
        editor.handle_event(&Event::KeyPress { key: 89, modifiers: primary_modifier_event_bits() });
        assert_eq!(editor.text(), "final");
    }

    #[test]
    fn keyboard_shortcuts_reject_shifted_and_non_primary_chords() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("draft");
        editor.set_text("final");

        let primary = primary_modifier_event_bits();
        // Shift+Primary+Z is a different chord, so it is not plain undo.
        editor.handle_event(&Event::key_press(90, primary | 0b0001));
        assert_eq!(editor.text(), "final");

        // The accelerator that is not primary on this host does not undo.
        editor.handle_event(&Event::key_press(90, non_primary_modifier_event_bits()));
        assert_eq!(editor.text(), "final");

        editor.handle_event(&Event::key_press(90, primary));
        assert_eq!(editor.text(), "draft");
        editor.handle_event(&Event::key_press(89, primary));
        assert_eq!(editor.text(), "final");
    }

    #[test]
    fn text_input_inserts_text_and_emits_signal() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = seen.clone();
        editor.text_changed.connect(move |value| {
            if let Ok(mut guard) = sink.lock() {
                guard.push((*value).clone());
            }
        });

        editor.handle_event(&Event::text_input("# Hello"));

        assert_eq!(editor.text(), "# Hello");
        let got = seen.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["# Hello".to_string()]);
    }

    #[test]
    fn ime_commit_inserts_cjk_text() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.handle_event(&Event::ime_commit("你好"));

        assert_eq!(editor.text(), "你好");
        assert_eq!(editor.cursor_col(), 2);
    }

    #[test]
    fn typed_text_can_be_undone_and_redone() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.handle_event(&Event::text_input("first"));
        editor.handle_event(&Event::ime_commit(" second"));
        assert_eq!(editor.text(), "first second");

        assert!(editor.undo());
        assert_eq!(editor.text(), "first");
        assert!(editor.undo());
        assert_eq!(editor.text(), "");

        assert!(editor.redo());
        assert!(editor.redo());
        assert_eq!(editor.text(), "first second");
    }

    #[test]
    fn enter_splits_and_backspace_merges_lines() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("ab");

        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(13, 0));
        assert_eq!(editor.text(), "ab\n");
        assert_eq!(editor.cursor_line(), 1);
        assert_eq!(editor.cursor_col(), 0);

        editor.handle_event(&Event::key_press(8, 0));
        assert_eq!(editor.text(), "ab");
        assert_eq!(editor.cursor_line(), 0);
        assert_eq!(editor.cursor_col(), 2);

        editor.handle_event(&Event::key_press(37, 0));
        editor.handle_event(&Event::key_press(8, 0));
        assert_eq!(editor.text(), "b");
        assert_eq!(editor.cursor_col(), 0);
    }

    #[test]
    fn arrow_keys_clamp_cursor_column_to_line_length() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("long line\nx");

        editor.handle_event(&Event::key_press(39, 0));
        editor.handle_event(&Event::key_press(39, 0));
        assert_eq!(editor.cursor_col(), 2);

        editor.handle_event(&Event::key_press(40, 0));
        assert_eq!(editor.cursor_line(), 1);
        assert_eq!(editor.cursor_col(), 1);
    }

    /// The wheel scrolls the visible line window and clamps at both ends.
    ///
    /// # The defect this pins
    ///
    /// The paint path stopped at a fixed `take(10)`, so a longer document could not be reached at
    /// all. This drives the same window the paint path now starts from.
    #[test]
    fn markdown_editor_wheel_scrolls_and_clamps() {
        let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 100));
        editor.set_text(text.join("\n"));
        assert_eq!(editor.first_visible_line, 0);

        // One notch toward the end reveals three lines.
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 3, "a downward notch moves the viewport down");

        // The other direction returns to the top and clamps there.
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 0);
        editor.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
        assert_eq!(editor.first_visible_line, 0, "the viewport clamps at the first line");

        // Scrolling on stops at the last full window.
        let max_first = editor.line_count() - editor.visible_line_count();
        for _ in 0..20 {
            editor.handle_event(&Event::Wheel {
                delta: crate::core::Point::new(0, -1),
                modifiers: 0,
            });
        }
        assert_eq!(editor.first_visible_line, max_first, "the viewport clamps at the last window");
    }

    /// A whole-text replacement still records only the changed middle for undo.
    #[test]
    fn set_text_undoes_a_paragraph_change_without_losing_the_rest() {
        let mut editor = MarkdownEditor::new(Rect::new(0, 0, 420, 240));
        editor.set_text("alpha\nbeta\ngamma");
        editor.set_text("alpha\nBETA 你好\ngamma");

        assert!(editor.undo());
        assert_eq!(editor.text(), "alpha\nbeta\ngamma");
        assert!(editor.redo());
        assert_eq!(editor.text(), "alpha\nBETA 你好\ngamma");
    }
}
