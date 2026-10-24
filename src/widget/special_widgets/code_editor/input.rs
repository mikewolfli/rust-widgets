// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard, IME, pointer and wheel plumbing for the [`CodeEditor`].
//!
//! This module is the *translation layer* between platform events and the
//! editing commands in `editor.rs`: it decides which command a key chord means,
//! routes IME commits into the document, and turns pointer coordinates into
//! carets, selections and fold toggles. It owns no document state of its own.

use super::editor::CodeEditor;
use super::types::TextPosition;
use crate::core::Point;
use crate::widget::Widget;
use alloc::string::String;

/// CONTROL modifier bit used by this platform layer.
const MODIFIER_CONTROL: u32 = 2;
/// SHIFT modifier bit used by this platform layer.
const MODIFIER_SHIFT: u32 = 1;
/// ALT modifier bit used by this platform layer.
const MODIFIER_ALT: u32 = 4;
/// META modifier bit used by this platform layer.
///
/// The bit the platform sets for `Command` on macOS and the Windows/Super key
/// elsewhere. Without it `Cmd+C` was not a copy on macOS even though the context
/// menu advertised it.
const MODIFIER_META: u32 = 8;

impl CodeEditor {
    // ── Input plumbing ──────────────────────────────────────────────────────

    /// Applies a committed text or IME payload.
    pub fn input_text(&mut self, text: &str) {
        if !self.is_editable() || text.is_empty() {
            return;
        }
        if self.find.visible {
            let printable: String = text.chars().filter(|ch| !ch.is_control()).collect();
            if !printable.is_empty() {
                self.append_find_query(&printable);
            }
            return;
        }
        let printable: String = text.chars().filter(|ch| *ch != '\r').collect();
        if printable.is_empty() {
            return;
        }
        // Auto-pairing only applies to a single typed character, never to a
        // paste or an IME payload.
        if self.config.auto_close_brackets && printable.chars().count() == 1 {
            if let Some(typed) = printable.chars().next() {
                if self.try_auto_pair(typed) {
                    return;
                }
            }
        }
        // Overwrite mode replaces the character after the caret instead of
        // inserting before it. A newline is never overwritten, and a multi-character
        // payload (paste, IME commit) keeps insert semantics.
        if self.overwrite
            && !self.has_selection()
            && printable.chars().count() == 1
            && self.char_at(self.cursor.head).is_some_and(|ch| ch != '\n')
        {
            let start = self.cursor.head;
            let end = self.step_horizontal(start, 1);
            let head = TextPosition::new(start.line, start.column + 1);
            self.splice_range(start, end, &printable, head);
            self.auto_trigger_completion();
            return;
        }
        self.insert(&printable);
        self.auto_trigger_completion();
    }

    fn auto_trigger_completion(&mut self) {
        let head = self.cursor.head;
        let prefix_len = head.column.saturating_sub(self.prefix_start_column());
        if prefix_len >= 2 {
            self.trigger_completion();
        } else {
            self.dismiss_completion();
        }
    }

    /// Handles a key press. `modifiers` uses the platform modifier bitfield.
    pub fn handle_key_press(&mut self, key: u32, modifiers: u32) {
        let shift = modifiers & MODIFIER_SHIFT != 0;
        let alt = modifiers & MODIFIER_ALT != 0;
        // The primary accelerator is Control on Windows/Linux and Command on
        // macOS. Treating only Control as primary made every documented `Cmd+…`
        // binding unreachable on macOS.
        let ctrl = modifiers & (MODIFIER_CONTROL | MODIFIER_META) != 0;

        if self.context_menu.visible && self.handle_menu_key(key, ctrl).is_some() {
            return;
        }
        if self.completion.visible && self.handle_completion_key(key).is_some() {
            return;
        }

        match key {
            // Word-wise movement and selection (Zed/VS Code bindings) must be
            // matched before the plain arrow arms or they become unreachable.
            37 if ctrl => self.move_word(-1, shift),
            39 if ctrl => self.move_word(1, shift),
            // Multi-cursor carets: Control+Alt+Up/Down, the VS Code gesture.
            38 if ctrl && alt => {
                self.add_cursor_above();
            }
            40 if ctrl && alt => {
                self.add_cursor_below();
            }
            // Moving the caret line is Alt+Up/Down, as in every mainstream editor.
            // F5 used to carry this binding, which collided with the debugger key.
            38 if alt => self.move_line(-1),
            40 if alt => self.move_line(1),
            // Arrow keys and paging.
            37 => self.move_cursor_with_selection(0, -1, shift),
            39 => self.move_cursor_with_selection(0, 1, shift),
            38 => self.move_cursor_with_selection(-1, 0, shift),
            40 => self.move_cursor_with_selection(1, 0, shift),
            36 => {
                if ctrl {
                    self.move_to_document_edge(-1, shift);
                } else {
                    self.move_to_line_edge(-1, shift);
                }
            }
            35 => {
                if ctrl {
                    self.move_to_document_edge(1, shift);
                } else {
                    self.move_to_line_edge(1, shift);
                }
            }
            // A page is one visual-row batch, not a loop of single-line moves.
            33 => self.move_page(-1, shift),
            34 => self.move_page(1, shift),
            // Editing.
            8 => {
                if ctrl || alt {
                    self.delete_word_backward();
                } else {
                    self.backspace();
                }
            }
            46 => self.delete_forward(),
            // `Insert` toggles overwrite mode.
            45 if !ctrl => {
                self.toggle_overwrite_mode();
            }
            13 => {
                if self.find.visible {
                    self.find_next(shift);
                } else if ctrl {
                    if shift {
                        self.insert_line_above();
                    } else {
                        self.insert_line_below();
                    }
                } else {
                    self.insert_newline();
                }
            }
            9 => {
                if self.completion.visible {
                    self.accept_completion();
                } else if self.context_menu.visible {
                    let id =
                        self.context_menu.items.get(self.context_menu.selected).map(|item| item.id);
                    if let Some(id) = id {
                        self.activate_menu_item(id);
                    }
                } else if alt {
                    self.outdent_selection();
                } else {
                    self.insert_tab();
                }
            }
            27 => {
                if self.context_menu.visible {
                    self.close_context_menu();
                } else if self.completion.visible {
                    self.dismiss_completion();
                } else if self.find.visible {
                    self.close_find();
                } else {
                    // Escape collapses the caret set, as in Zed/VS Code.
                    self.collapse_cursors();
                }
            }
            // Escape is `27`, but the widget also accepts `0x84` (X11-style ESC)
            // for platform layers that map it that way.
            0x84 => {
                self.dismiss_completion();
                self.close_context_menu();
                self.base.request_redraw();
            }
            // Ctrl/Meta chords.
            90 if ctrl && shift => {
                let _ = self.redo();
            }
            90 if ctrl => {
                let _ = self.undo();
            }
            89 if ctrl && shift => {
                let _ = self.redo();
            }
            89 if ctrl => {
                let _ = self.redo();
            }
            // Find next/previous, the F3 convention (framework F-key range).
            1122 => {
                let _ = self.find_next(shift);
            }
            // Multi-caret and line commands (Zed / VS Code bindings).
            68 if ctrl && shift => self.duplicate_line(),
            68 if ctrl => {
                self.select_next_occurrence();
            }
            // Ctrl+L selects the caret's line.
            76 if ctrl && !shift => self.select_line(self.cursor.head.line),
            76 if ctrl && shift => {
                self.select_all_occurrences();
            }
            75 if ctrl && shift => self.delete_line(),
            // Join Lines is `J`; it must be matched before the plain-`J` arm or
            // the later arm swallows it (the two arms share a key and the first
            // match wins).
            74 if ctrl => self.join_lines(),
            // Transpose the two characters around the caret.
            84 if ctrl => self.transpose_characters(),
            65 if ctrl => self.select_all(),
            67 if ctrl => {
                self.copy();
            }
            88 if ctrl => {
                self.cut();
            }
            86 if ctrl => {
                self.paste();
            }
            70 if ctrl => self.open_find(false),
            72 if ctrl => self.open_find(true),
            47 if ctrl => self.toggle_line_comment(),
            32 if ctrl => {
                self.trigger_completion();
            }
            73 if ctrl => self.indent_selection(),
            91 if ctrl => {
                let line = self.cursor.head.line;
                self.toggle_fold(line);
            }
            93 if ctrl => self.unfold_all(),
            61 if ctrl => self.scroll_by(self.visible_rows() as isize),
            45 if ctrl => self.scroll_by(-(self.visible_rows() as isize)),
            _ => {}
        }
    }

    /// Handles a key while the context menu is open. `Some(())` means consumed.
    fn handle_menu_key(&mut self, key: u32, ctrl: bool) -> Option<()> {
        match key {
            38 => {
                self.move_menu_selection(-1);
                Some(())
            }
            40 => {
                self.move_menu_selection(1);
                Some(())
            }
            27 | 0x84 => {
                self.close_context_menu();
                Some(())
            }
            13 | 32 if !ctrl => {
                let id =
                    self.context_menu.items.get(self.context_menu.selected).map(|item| item.id);
                if let Some(id) = id {
                    self.activate_menu_item(id);
                }
                Some(())
            }
            _ => None,
        }
    }

    /// Handles a key while the completion popup is open.
    fn handle_completion_key(&mut self, key: u32) -> Option<()> {
        match key {
            38 | 33 => {
                self.move_completion_selection(-1);
                Some(())
            }
            40 | 34 => {
                self.move_completion_selection(1);
                Some(())
            }
            13 | 9 => {
                self.accept_completion();
                Some(())
            }
            27 | 0x84 => {
                self.dismiss_completion();
                Some(())
            }
            _ => None,
        }
    }

    /// Handles a key while the find bar owns the keyboard.
    fn handle_find_key(&mut self, key: u32, shift: bool) -> bool {
        match key {
            13 => {
                self.find_next(shift);
                true
            }
            8 => {
                self.backspace_find_query();
                true
            }
            27 | 0x84 => {
                self.close_find();
                true
            }
            9 => {
                self.find.focus_replace = !self.find.focus_replace;
                self.base.request_redraw();
                true
            }
            _ => false,
        }
    }

    /// Applies a key press coming from the event loop.
    pub(crate) fn dispatch_key(&mut self, key: u32, modifiers: u32) {
        if self.find.visible {
            let shift = modifiers & MODIFIER_SHIFT != 0;
            let ctrl = modifiers & MODIFIER_CONTROL != 0;
            if !ctrl && self.handle_find_key(key, shift) {
                return;
            }
        }
        self.handle_key_press(key, modifiers);
    }

    // ── Pointer handling (invoked from the `Draw`/`EventHandler` side) ───────────

    pub(crate) fn pointer_press(&mut self, pos: Point) {
        self.pointer_press_with_modifiers(pos, 0);
    }

    /// A press with the modifier state the platform reported.
    ///
    /// `Ctrl` + click adds a caret, the Zed/VS Code multi-cursor gesture. On macOS a
    /// host that uses Command for this can pass the same bit; the editor only needs
    /// "a modifier was held".
    pub(crate) fn pointer_press_with_modifiers(&mut self, pos: Point, modifiers: u32) {
        self.base.set_mouse_pressed(true);
        self.mouse_selecting = true;
        self.last_pointer_position = Some(pos);
        let shift = modifiers & MODIFIER_SHIFT != 0;
        if self.context_menu.visible {
            self.dismiss_completion();
            match self.hit_menu_row(pos) {
                Some(index) => {
                    self.context_menu.selected = index;
                    let id = self.context_menu.items.get(index).map(|item| item.id);
                    if let Some(id) = id {
                        self.activate_menu_item(id);
                    }
                }
                None => self.close_context_menu(),
            }
            return;
        }
        if self.completion.visible {
            if let Some(index) = self.hit_completion_row(pos) {
                self.completion.selected = index;
                self.accept_completion();
                return;
            }
            self.dismiss_completion();
        }
        // The tab strip is chrome above the buffer: a press there is a tab command,
        // never a caret placement. The close box is the smaller target inside the
        // tab, and it is checked first (`tab_at` reports which one was hit).
        if let Some((index, close)) = self.tab_at(pos) {
            self.mouse_selecting = false;
            if close {
                self.close_buffer(index);
            } else {
                // A tab click navigates; it is not an edit of either document, so it
                // must not push a `TabSwitchCommand` (undo belongs to the buffer).
                self.activate_buffer_untracked(index);
            }
            return;
        }
        if let Some(line) = self.hit_fold_marker(pos) {
            self.toggle_fold(line);
            return;
        }
        // A press on a line number selects that line, the way Zed's gutter does: the
        // gutter is the one place where a click can only mean "this line", and
        // dragging from it is how a range of whole lines is chosen.
        if let Some(line) = self.hit_gutter_line(pos) {
            self.mouse_selecting = false;
            self.gutter_dragging = true;
            self.gutter_anchor = Some(line);
            self.select_line(line);
            return;
        }
        // The scrollbar is a control, not a decoration: a press on its track moves
        // the viewport to that fraction, and the following motion keeps scrolling.
        if let Some(fraction) = self.scrollbar_fraction_at(pos) {
            self.mouse_selecting = false;
            self.scrollbar_dragging = true;
            self.scroll_to_fraction(fraction);
            return;
        }
        // The minimap is a scroll target, like Zed's: a click anywhere in the strip
        // moves the viewport to that fraction of the document.
        if let Some(fraction) = self.minimap_fraction_at(pos) {
            // A minimap press is a scroll gesture, not a text selection: clearing the
            // selection latch is what keeps the following motion from extending one.
            self.mouse_selecting = false;
            self.minimap_dragging = true;
            self.scroll_to_fraction(fraction);
            return;
        }
        if let Some(position) = self.position_at_point(pos) {
            // A third press at the same spot completes a triple-click and selects
            // the line. The widget has no clock, so the double-click event marks
            // the spot and the next press at that spot is the third.
            let triple = self.double_click_at.take().is_some_and(|anchor| {
                (anchor.x - pos.x).abs() <= 2 && (anchor.y - pos.y).abs() <= 2
            });
            if triple {
                self.select_line(position.line);
            } else if shift {
                // Shift+click extends the selection from the existing anchor.
                self.move_caret_to(position, true);
            } else if modifiers & MODIFIER_CONTROL != 0 {
                self.add_cursor_at(position);
            } else if self.selection_contains(position) {
                // A press inside the selection starts a drag, not a new caret.
                self.mouse_selecting = false;
                self.dragging_text = true;
                self.drag_origin = Some(position);
            } else {
                self.move_caret_to(position, false);
            }
        }
    }

    pub(crate) fn pointer_drag(&mut self, pos: Point) {
        // Hover is tracked even when no button is down: it is what reveals the close
        // affordance on a tab and washes it as the pointer passes over.
        self.update_hovered_tab(pos);
        if self.gutter_dragging {
            self.last_pointer_position = Some(pos);
            if let Some(line) = self.hit_gutter_line(pos) {
                self.select_line_range(line);
            }
            return;
        }
        if self.scrollbar_dragging {
            self.last_pointer_position = Some(pos);
            if let Some(fraction) = self.scrollbar_fraction_at(pos) {
                self.scroll_to_fraction(fraction);
            }
            return;
        }
        if self.minimap_dragging {
            // A minimap drag keeps following the pointer after it leaves the strip,
            // clamped by `scroll_to_fraction`.
            let rect = self.geometry();
            let top = rect.y + self.text_origin_y();
            let bottom = rect.y + rect.height as i32 - self.status_bar_height();
            let height = (bottom - top).max(1) as f32;
            let fraction = ((pos.y - top) as f32 / height).clamp(0.0, 1.0);
            self.scroll_to_fraction(fraction);
            return;
        }
        if self.dragging_text {
            self.last_pointer_position = Some(pos);
            return;
        }
        if !self.mouse_selecting {
            return;
        }
        // A motion that has not moved is the toolkit echoing the press. Treating it
        // as a drag collapsed a double-click selection to the click point.
        if self.last_pointer_position == Some(pos) {
            return;
        }
        self.last_pointer_position = Some(pos);
        if let Some(position) = self.position_at_point(pos) {
            self.move_caret_to(position, true);
        }
    }

    pub(crate) fn pointer_release(&mut self) {
        if self.dragging_text {
            if let Some(pos) = self.last_pointer_position {
                if let Some(target) = self.position_at_point(pos) {
                    self.move_selection_to(target);
                }
            }
        }
        self.mouse_selecting = false;
        self.minimap_dragging = false;
        self.scrollbar_dragging = false;
        self.gutter_dragging = false;
        self.gutter_anchor = None;
        self.dragging_text = false;
        self.drag_origin = None;
        self.last_pointer_position = None;
        self.base.set_mouse_pressed(false);
    }

    fn hit_menu_row(&self, pos: Point) -> Option<usize> {
        let rect = self.context_menu_rect()?;
        index_in_rows(pos, rect, self.context_menu.items.len())
    }

    /// Which tab is under `pos`, and whether the point is on its close box.
    ///
    /// Reads the rectangles the painter published; a click in the strip before the
    /// first frame is ignored rather than estimated from a character count.
    fn tab_at(&self, pos: Point) -> Option<(usize, bool)> {
        let slots = self.tab_hits.borrow();
        let slot = slots.iter().find(|slot| slot.rect.contains_point(pos))?;
        let close = slot.close.width > 0 && slot.close.contains_point(pos);
        Some((slot.buffer, close))
    }

    /// Records the tab the pointer is over and repaints when it changed.
    ///
    /// A repaint per motion event would be wasteful; a repaint per *change* is what
    /// the hover affordance costs.
    pub(crate) fn update_hovered_tab(&mut self, pos: Point) {
        let hovered = self.tab_at(pos).map(|(index, _)| index);
        if hovered != self.hovered_tab {
            self.hovered_tab = hovered;
            self.base.request_redraw();
        }
    }

    fn hit_completion_row(&self, pos: Point) -> Option<usize> {
        let rect = self.completion_rect();
        let visible = self.completion_visible_rows();
        let row = index_in_rows(pos, rect, visible)?;
        Some(self.completion.scroll + row)
    }

    /// Where in the minimap strip `pos` is, as a fraction of the document.
    ///
    /// `None` outside the strip, or when the minimap is disabled. The returned value
    /// is the fraction the **click point** represents, not a row index: the strip shows
    /// the whole document compressed, so the mapping is proportional.
    fn minimap_fraction_at(&self, pos: Point) -> Option<f32> {
        let width = self.minimap_width();
        if width == 0 {
            return None;
        }
        let rect = self.geometry();
        let left = rect.x + rect.width as i32 - width;
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        if pos.x < left || pos.x >= rect.x + rect.width as i32 || pos.y < top || pos.y >= bottom {
            return None;
        }
        let height = (bottom - top).max(1) as f32;
        Some(((pos.y - top) as f32 / height).clamp(0.0, 1.0))
    }

    fn hit_fold_marker(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        // The chevron column, not the leading edge of the gutter: the numbers live
        // between them, and clicking a line number must place the caret, not fold.
        let left = rect.x + self.fold_marker_offset();
        let width = self.fold_marker_width();
        if pos.x < left || pos.x >= left + width {
            return None;
        }
        let local_y = pos.y - rect.y - self.text_origin_y();
        if local_y < 0 {
            return None;
        }
        let row = self.scroll_visual_row + (local_y as f32 / self.line_height()).floor() as usize;
        let line = self.visual_row_to_line(row);
        if self.line_to_visual_row(line) != row {
            return None;
        }
        Some(line)
    }

    /// Where in the scrollbar `pos` is, as a fraction of the document.
    ///
    /// `None` when the scrollbar is not drawn (the document fits) or the point is
    /// outside it. The track is a real control: before this it was painted but a
    /// click on it placed a caret behind it.
    fn scrollbar_fraction_at(&self, pos: Point) -> Option<f32> {
        let total = self.total_visual_rows();
        let rows = self.visible_rows().max(1);
        if total <= rows {
            return None;
        }
        let width = self.scrollbar_width();
        let rect = self.geometry();
        let left = rect.x + rect.width as i32 - width - self.minimap_width();
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        if pos.x < left || pos.x >= left + width || pos.y < top || pos.y >= bottom {
            return None;
        }
        let height = (bottom - top).max(1) as f32;
        Some(((pos.y - top) as f32 / height).clamp(0.0, 1.0))
    }

    /// The line whose number is under `pos`, when the point is in the gutter.
    ///
    /// The gutter is not part of the text area (`position_at_point` rejects it), but
    /// it is a line-selection target in every editor a reader is used to: a press on
    /// the number selects the whole line, including its newline.
    pub(crate) fn hit_gutter_line(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let width = self.gutter_width();
        if pos.x < rect.x || pos.x >= rect.x + width {
            return None;
        }
        let top = rect.y + self.text_origin_y();
        let bottom = rect.y + rect.height as i32 - self.status_bar_height();
        if pos.y < top || pos.y >= bottom {
            return None;
        }
        let row =
            self.scroll_visual_row + ((pos.y - top) as f32 / self.line_height()).floor() as usize;
        let line = self.visual_row_to_line(row);
        if self.line_to_visual_row(line) != row {
            return None;
        }
        Some(line)
    }
}

/// Returns the row index inside a stack of [`super::types::MIN_TOUCH_TARGET`]-tall rows.
fn index_in_rows(pos: Point, rect: crate::core::Rect, row_count: usize) -> Option<usize> {
    if row_count == 0 {
        return None;
    }
    if pos.x < rect.x || pos.x >= rect.x + rect.width as i32 {
        return None;
    }
    if pos.y < rect.y {
        return None;
    }
    let row = ((pos.y - rect.y) as f32 / super::types::MIN_TOUCH_TARGET as f32).floor() as usize;
    if row < row_count {
        Some(row)
    } else {
        None
    }
}
