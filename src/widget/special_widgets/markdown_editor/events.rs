// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Widget, property, event and key handling for `MarkdownEditor`.

use super::*;
use crate::core::{Point, Rect};
use crate::event::wheel::WheelScroll;
use crate::event::{Event, EventHandler};
use crate::shortcut::Key;
use crate::widget::capability::coercion::{expect_bool, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::{BaseWidget, Widget};
use crate::{impl_widget_property_hooks, property_names_of};

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

    /// One frame of the caret's blink. The frame bus calls this; nothing else does.
    fn tick(&mut self, delta_ms: u32) -> bool {
        MarkdownEditor::tick(self, delta_ms)
    }

    /// A blinking caret always owes frames while it runs.
    fn is_animating(&self) -> bool {
        self.cursor_blink.is_running()
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
                    // The changed run is a structured payload: offset, the removed and inserted
                    // text, then the document length after the edit (SP-001).
                    CapabilityValue::Tuple(vec![
                        CapabilityValue::UInt(v.offset as u64),
                        CapabilityValue::String(v.removed.clone()),
                        CapabilityValue::String(v.inserted.clone()),
                        CapabilityValue::UInt(v.len as u64),
                    ])
                }))
            }
            "preview_mode_changed" => Some(EventSignalRef::mapped(
                "preview_mode_changed",
                &self.preview_mode_changed,
                |v| CapabilityValue::Bool(*v),
            )),
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "heading_activated" => {
                Some(EventSignalRef::mapped("heading_activated", &self.heading_activated, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
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
            "show_scrollbar" => Ok(CapabilityValue::Bool(self.show_scrollbar)),
            "gfm_extensions" => Ok(CapabilityValue::Bool(self.gfm_extensions)),
            "code_highlight" => Ok(CapabilityValue::Bool(self.code_highlight)),
            "undo_stack_limit" => Ok(CapabilityValue::UInt(self.undo_stack_limit as u64)),
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
            "show_scrollbar" => {
                self.set_show_scrollbar(expect_bool(value)?);
                Ok(())
            }
            "gfm_extensions" => {
                self.set_gfm_extensions(expect_bool(value)?);
                Ok(())
            }
            "code_highlight" => {
                self.set_code_highlight(expect_bool(value)?);
                Ok(())
            }
            "undo_stack_limit" => {
                self.set_undo_limit(expect_usize(value)?);
                Ok(())
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
            "show_scrollbar",
            "gfm_extensions",
            "code_highlight",
            "undo_stack_limit",
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
            self.cursor_blink.stop();
            return;
        }
        if !self.cursor_blink.is_running() {
            self.cursor_blink.start();
        }
        // The side panel and find bar are hit-tested in this widget's coordinates, so their boxes
        // are refreshed before the press is resolved.
        self.layout_companions();

        match event {
            // The find bar owns text input while it is open: otherwise typing a query would edit
            // the document behind it. The mounted bar is kept in step through the same query, so
            // the painted counter always describes the text Enter will search.
            Event::TextInput { text } if self.find.visible => {
                self.find.query.push_str(text);
                self.find.current = None;
                self.find_bar.set_query(self.find.query.clone());
                self.base.request_redraw();
            }
            Event::ImeCommit { text } if self.find.visible => {
                self.find.query.push_str(text);
                self.find.current = None;
                self.find_bar.set_query(self.find.query.clone());
                self.base.request_redraw();
            }
            // Printable text and IME commits are what the platform delivers for typing; a
            // handler with only a `KeyPress` arm never sees them. Both insert at the caret
            // through the same undo-guarded path as `set_text` (the filter mirrors
            // `Mention`, which refuses control characters and empty runs).
            Event::TextInput { text }
                if text.chars().all(|ch| !ch.is_control()) && !text.is_empty() =>
            {
                self.preedit = None;
                self.insert_text_at_cursor(text);
            }
            Event::ImeCommit { text } => {
                self.preedit = None;
                self.insert_text_at_cursor(text);
            }
            Event::ImePreedit { text, cursor } => {
                // The composition has to be visible before it is committed, and the caret inside
                // it is what anchors the candidate window.
                self.preedit =
                    Some(Preedit { text: text.clone(), cursor: (*cursor).min(text.len()) });
                self.base.request_redraw();
            }
            Event::KeyPress { key, modifiers } => self.handle_key(*key, *modifiers),
            // Wheel: positive `delta.y` moves the content down, so the viewport moves toward the
            // start. Three lines per notch is the desktop default shared with the other text
            // controls.
            Event::MousePress { pos, button: 1, modifiers, .. } => {
                let rect = self.geometry();
                // The mounted outline is a real control: a press in its column selects the row and
                // navigates to its line, exactly as a standalone panel would.
                if let Some(panel_rect) = self.outline_rect() {
                    if panel_rect.contains_point(*pos) {
                        if let Some(index) = self.outline_panel.entry_at(*pos) {
                            if let Some(line) = self.outline_panel.goto(index) {
                                self.heading_activated.emit(index);
                                let _ = self.goto_line(line);
                            }
                        }
                        return;
                    }
                }
                // The mounted find bar owns presses inside its own band (the `Aa`/`W` toggles and
                // the query band); the document caret must not move for them.
                if self.find.visible && self.find_bar_rect().contains_point(*pos) {
                    self.find_bar.handle_event(event);
                    self.find.query = self.find_bar.query().to_string();
                    self.base.request_redraw();
                    return;
                }
                if self.show_scrollbar && self.scrollbar_track_contains(*pos) {
                    let on_thumb = self.scrollbar_thumb().is_some_and(|thumb| {
                        pos.x >= thumb.x
                            && pos.x < thumb.x + thumb.width as i32
                            && pos.y >= thumb.y
                            && pos.y < thumb.y + thumb.height as i32
                    });
                    if on_thumb {
                        self.dragging_scrollbar = true;
                    } else {
                        self.scroll_to_track(pos.y);
                    }
                    return;
                }
                let row = (pos.y - rect.y - TEXT_TOP) / TEXT_ROW_HEIGHT;
                if row < 0 {
                    return;
                }
                let line = self.visible_start() + row as usize;
                if line >= self.edit_line_count() {
                    return;
                }
                // The column is resolved against the cached line tables, not by rescanning the
                // document: the old form walked every line above the hit row on every press and
                // drag move, and it resolved the column through a different path than the caret.
                let (column, line_chars) = self.hit_column(line, pos.x - rect.x - 12);
                let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                    .contains(crate::shortcut::Modifiers::SHIFT);
                self.cursor_line = line;
                self.cursor_col = column.min(line_chars);
                if !shift {
                    self.selection_anchor = Some((self.cursor_line, self.cursor_col));
                    self.apply_selection(None);
                }
                self.selecting = true;
                if self.preview_mode {
                    // A heading row in preview is a navigation target, so a click on it announces
                    // which outline entry it is.
                    let heading =
                        self.outline().iter().position(|entry| entry.line == self.cursor_line);
                    if let Some(index) = heading {
                        self.heading_activated.emit(index);
                        let _ = self.goto_outline(index);
                    }
                }
                self.base.request_redraw();
            }
            Event::MouseMove { pos } => {
                if self.dragging_scrollbar {
                    self.scroll_to_track(pos.y);
                    return;
                }
                if self.selecting {
                    let rect = self.geometry();
                    let row = (pos.y - rect.y - TEXT_TOP) / TEXT_ROW_HEIGHT;
                    if row >= 0 {
                        let line = (self.visible_start() + row as usize)
                            .min(self.edit_line_count().saturating_sub(1));
                        // Same table, same answer as the press that started the drag.
                        let (column, line_chars) = self.hit_column(line, pos.x - rect.x - 12);
                        self.cursor_line = line;
                        self.cursor_col = column.min(line_chars);
                        self.sync_selection_from_anchor();
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseRelease { .. } => {
                self.selecting = false;
                self.dragging_scrollbar = false;
            }
            Event::MouseLeave { .. } => {
                // A release outside the control is not delivered to it, so leaving is the last
                // signal that the drag ended.
                self.selecting = false;
                self.dragging_scrollbar = false;
            }
            Event::Resize { .. } => {
                self.cursor_line = self.cursor_line.min(self.edit_line_count().saturating_sub(1));
                self.clamp_cursor_col();
                self.base.request_layout();
                self.base.request_redraw();
            }
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

impl MarkdownEditor {
    /// Routes one key press to the editor or to the open find bar.
    pub(crate) fn handle_key(&mut self, key: u32, modifiers: u32) {
        let shift = crate::shortcut::Modifiers::from_event_bits(modifiers)
            .contains(crate::shortcut::Modifiers::SHIFT);
        // The shared predicate, not a raw `PRIMARY` bit test: on a Desktop host
        // Control is the primary accelerator and only the Command bit sets
        // `Modifiers::PRIMARY` on its own.
        let primary = crate::shortcut::event_carries_primary(modifiers);

        if self.find.visible {
            match key {
                8 => {
                    self.find.query.pop();
                    self.find.current = None;
                    self.find_bar.set_query(self.find.query.clone());
                }
                13 => {
                    let query = self.find.query.clone();
                    if !self.find_next(&query) {
                        self.find.current = None;
                    }
                    let _ = self.find_bar.find_next();
                }
                27 => self.close_find(),
                _ if primary_shortcut_pressed(key, modifiers, Key::F) => self.close_find(),
                _ => { /* The find bar ignores editing chords it does not own. */ }
            }
            self.base.request_redraw();
            return;
        }

        if primary {
            if primary_shortcut_pressed(key, modifiers, Key::A) {
                self.select_all();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::C) {
                self.copy();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::X) {
                self.cut();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::V) {
                self.paste();
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::F) {
                self.open_find();
                return;
            }
            // Inline formatting: the same actions the mounted toolbar calls, so a chord and its
            // button cannot produce different markdown.
            if primary_shortcut_pressed(key, modifiers, Key::B) {
                self.wrap_selection("**", "**");
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::I) {
                self.wrap_selection("*", "*");
                return;
            }
            if primary_shortcut_pressed(key, modifiers, Key::K) {
                self.wrap_selection("[", "](url)");
                return;
            }
        }
        match key {
            _ if primary_shortcut_pressed(key, modifiers, Key::Z) => {
                let _ = self.undo();
            }
            _ if primary_shortcut_pressed(key, modifiers, Key::Y) => {
                let _ = self.redo();
            }
            // The second redo chord every desktop editor accepts; the handler used to answer
            // only primary+Y, so the habitual primary+Shift+Z did nothing.
            _ if primary_shift_shortcut(key, modifiers, Key::Z) => {
                let _ = self.redo();
            }
            9 => {
                // Tab indents; Shift+Tab outdents. With no selection the caret gains one indent
                // level, which is what markdown's nested lists and code blocks need.
                if shift {
                    self.shift_line_indent(false);
                } else if self.selection.is_some() {
                    self.shift_line_indent(true);
                } else {
                    self.replace_selection_or_caret("  ");
                }
            }
            8 => self.backspace_at_cursor(),
            13 => self.insert_newline_at_cursor(),
            46 | 127 => self.delete_forward(),
            36 => {
                if shift {
                    self.begin_shift_selection();
                    self.cursor_col = 0;
                    self.sync_selection_from_anchor();
                    self.base.request_redraw();
                } else {
                    self.move_to_line_edge(true);
                }
            }
            35 => {
                if shift {
                    self.begin_shift_selection();
                    self.cursor_col = self.current_line_char_len();
                    self.sync_selection_from_anchor();
                    self.base.request_redraw();
                } else {
                    self.move_to_line_edge(false);
                }
            }
            33 => {
                self.scroll_page(-1);
            }
            34 => {
                self.scroll_page(1);
            }
            37 => {
                if shift {
                    self.begin_shift_selection();
                } else {
                    self.selection_anchor = None;
                    self.apply_selection(None);
                }
                self.move_cursor_left();
                if shift {
                    self.sync_selection_from_anchor();
                }
            }
            39 => {
                if shift {
                    self.begin_shift_selection();
                } else {
                    self.selection_anchor = None;
                    self.apply_selection(None);
                }
                self.move_cursor_right();
                if shift {
                    self.sync_selection_from_anchor();
                }
            }
            38 => self.move_vertical(-1, shift),
            40 => self.move_vertical(1, shift),
            80 | 112 if primary_shortcut_pressed(key, modifiers, Key::P) => {
                self.toggle_preview_mode()
            }
            _ => { /* Other keys are not relevant */ }
        }
    }

    /// Deletes the character after the caret, or merges the next line at a line end.
    pub(crate) fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.clamp_cursor();
        let at = self.cursor_byte_offset();
        let Some(ch) = self.text[at..].chars().next() else {
            return;
        };
        let mut next = String::with_capacity(self.text.len());
        next.push_str(&self.text[..at]);
        next.push_str(&self.text[at + ch.len_utf8()..]);
        self.commit_text(next);
    }

    /// Moves the caret by whole lines, extending the selection when `shift` is held.
    pub(crate) fn move_vertical(&mut self, delta: isize, shift: bool) {
        if shift {
            self.begin_shift_selection();
        } else {
            self.selection_anchor = None;
            self.apply_selection(None);
        }
        self.move_cursor(delta);
        if shift {
            self.sync_selection_from_anchor();
        }
        self.reveal_caret();
    }

    /// Remembers the caret as the selection anchor if no selection is being extended yet.
    pub(crate) fn begin_shift_selection(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some((self.cursor_line, self.cursor_col));
        }
    }

    /// Recomputes the selection from the anchor and the current caret.
    pub(crate) fn sync_selection_from_anchor(&mut self) {
        let Some((anchor_line, anchor_col)) = self.selection_anchor else {
            self.apply_selection(None);
            return;
        };
        let anchor = self.doc_char_offset(anchor_line, anchor_col);
        let caret = self.doc_char_offset_of_caret();
        let (start, end) = if anchor <= caret { (anchor, caret) } else { (caret, anchor) };
        self.apply_selection((start != end).then_some((start, end)));
    }

    /// The document character offset of `(line, column)`.
    pub(crate) fn doc_char_offset(&self, line: usize, column: usize) -> usize {
        self.with_line_index(|index| {
            let start = index.char_starts.get(line).copied().unwrap_or_else(|| index.char_len());
            let length = index.char_lens.get(line).copied().unwrap_or(0);
            start + column.min(length)
        })
    }

    /// A glyph advance, measured at most once per size.
    /// The scrollbar track, or `None` when hidden or the document fits.
    pub(crate) fn scrollbar_track(&self) -> Option<Rect> {
        if !self.show_scrollbar {
            return None;
        }
        let total = self.line_count();
        if total <= self.visible_line_count().max(1) {
            return None;
        }
        let rect = self.geometry();
        let top = rect.y + TEXT_TOP;
        let bottom = rect.y + rect.height as i32 - TEXT_BOTTOM_INSET;
        if bottom <= top {
            return None;
        }
        // The track stops where the outline panel begins, so the two pieces of chrome never
        // paint over one another.
        let outline_width = self.outline_rect().map_or(0, |panel| panel.width as i32);
        Some(Rect::new(
            rect.x + rect.width as i32 - 10 - outline_width,
            top,
            6,
            (bottom - top) as u32,
        ))
    }

    /// The rectangle the mounted outline panel occupies, when it is shown and fits.
    pub(crate) fn outline_rect(&self) -> Option<Rect> {
        if !self.show_outline {
            return None;
        }
        let rect = self.geometry();
        // Half the editor is the most a side panel may take; below this the text column would be
        // too narrow to read, so the panel is simply not mounted. The threshold is a named metric
        // (a fit decision), not a bare literal in the paint path.
        if rect.width < crate::widget::metrics::OUTLINE_PANEL_MIN_EDITOR_WIDTH
            || rect.height < TEXT_TOP as u32 + 48
        {
            return None;
        }
        let width = self.outline_panel.width().min(rect.width / 2).max(1);
        Some(Rect::new(
            rect.x + rect.width as i32 - width as i32,
            rect.y + TEXT_TOP,
            width,
            rect.height - TEXT_TOP as u32 - TEXT_BOTTOM_INSET as u32,
        ))
    }

    /// The rectangle the mounted find bar occupies.
    pub(crate) fn find_bar_rect(&self) -> Rect {
        let rect = self.geometry();
        let height = self.find_bar.size_hint().height;
        Rect::new(rect.x + 8, rect.y + 4, rect.width.saturating_sub(16), height)
    }

    /// Places the mounted companions inside the editor's own rectangle.
    pub(crate) fn layout_companions(&mut self) {
        let find_bar = self.find_bar_rect();
        if self.find_bar.base().geometry() != find_bar {
            self.find_bar.base_mut().set_geometry(find_bar);
        }
        if let Some(outline) = self.outline_rect() {
            if self.outline_panel.base().geometry() != outline {
                self.outline_panel.base_mut().set_geometry(outline);
            }
        }
    }

    /// The scrollbar thumb, sized and placed from the scroll state.
    pub(crate) fn scrollbar_thumb(&self) -> Option<Rect> {
        let track = self.scrollbar_track()?;
        let total = self.line_count().max(1);
        let visible = self.visible_line_count().max(1).min(total);
        let height = ((track.height as i64 * visible as i64) / total as i64).max(12) as u32;
        let height = height.min(track.height);
        let range = total.saturating_sub(visible);
        let travel = track.height.saturating_sub(height) as i64;
        let top = self.visible_start().min(range);
        let y = track.y + if range > 0 { (travel * top as i64 / range as i64) as i32 } else { 0 };
        Some(Rect::new(track.x, y, track.width, height))
    }

    /// Whether `pos` falls on the painted scrollbar track.
    pub(crate) fn scrollbar_track_contains(&self, pos: Point) -> bool {
        self.scrollbar_track().is_some_and(|track| {
            pos.x >= track.x
                && pos.x < track.x + track.width as i32
                && pos.y >= track.y
                && pos.y < track.y + track.height as i32
        })
    }

    /// Scrolls so the first visible line is where `y` points on the track.
    pub(crate) fn scroll_to_track(&mut self, y: i32) {
        let Some(track) = self.scrollbar_track() else {
            return;
        };
        let ratio = ((y - track.y) as f32 / track.height.max(1) as f32).clamp(0.0, 1.0);
        let max_first = self.line_count().saturating_sub(self.visible_line_count());
        let next = (ratio * max_first as f32).round() as usize;
        if next != self.first_visible_line {
            self.first_visible_line = next.min(max_first);
            self.base.request_redraw();
        }
    }
}

/// Whether the host's primary accelerator **plus Shift** is held for `expected`.
///
/// `primary_shortcut_pressed` compares the modifier set exactly, so the Shift bit is removed
/// before asking it: the remaining modifiers are what a plain primary chord carries.
fn primary_shift_shortcut(key: u32, modifiers: u32, expected: Key) -> bool {
    let shift = crate::shortcut::Modifiers::SHIFT;
    let pressed = crate::shortcut::Modifiers::from_event_bits(modifiers);
    pressed.contains(shift)
        && primary_shortcut_pressed(key, modifiers & !(shift.bits() as u32), expected)
}
