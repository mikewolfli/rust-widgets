// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard, IME, pointer and wheel plumbing for the [`CodeEditor`].
//!
//! This module is the *translation layer* between platform events and the
//! editing commands in `editor.rs`: it decides which command a key chord means,
//! routes IME commits into the document, and turns pointer coordinates into
//! carets, selections and fold toggles. It owns no document state of its own.

use super::editor::CodeEditor;
use crate::core::Point;
use crate::widget::Widget;
use alloc::string::String;

/// CONTROL modifier bit used by this platform layer.
const MODIFIER_CONTROL: u32 = 2;
/// SHIFT modifier bit used by this platform layer.
const MODIFIER_SHIFT: u32 = 1;
/// ALT modifier bit used by this platform layer.
const MODIFIER_ALT: u32 = 4;

impl CodeEditor {
    // ── Input plumbing ──────────────────────────────────────────────────────

    /// Applies a committed text or IME payload.
    pub fn input_text(&mut self, text: &str) {
        if self.config.read_only || text.is_empty() {
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
        let ctrl = modifiers & MODIFIER_CONTROL != 0;
        let shift = modifiers & MODIFIER_SHIFT != 0;
        let alt = modifiers & MODIFIER_ALT != 0;

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
            // Multi-cursor carets.
            38 if alt => {
                self.add_cursor_above();
            }
            40 if alt => {
                self.add_cursor_below();
            }
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
            33 => {
                let rows = self.visible_rows();
                for _ in 0..rows {
                    self.move_cursor_with_selection(-1, 0, shift);
                }
            }
            34 => {
                let rows = self.visible_rows();
                for _ in 0..rows {
                    self.move_cursor_with_selection(1, 0, shift);
                }
            }
            // Editing.
            8 => {
                if alt {
                    self.delete_word_backward();
                } else {
                    self.backspace();
                }
            }
            46 => self.delete_forward(),
            13 => {
                if self.find.visible {
                    self.find_next(shift);
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
            90 if ctrl => {
                let _ = self.undo();
            }
            89 if ctrl => {
                let _ = self.redo();
            }
            // Multi-caret and line commands (Zed / VS Code bindings).
            68 if ctrl && shift => self.duplicate_line(),
            68 if ctrl => {
                self.select_next_occurrence();
            }
            76 if ctrl && shift => {
                self.select_all_occurrences();
            }
            75 if ctrl && shift => self.delete_line(),
            // Join Lines is `J`; it must be matched before the plain-`J` arm or
            // the later arm swallows it (the two arms share a key and the first
            // match wins).
            74 if ctrl => self.join_lines(),
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
            // Alt+Up / Alt+Down move the current line (Zed / VS Code bindings).
            116 if alt => {
                let direction = if shift { 1 } else { -1 };
                self.move_line(direction);
            }
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
        self.base.set_mouse_pressed(true);
        self.mouse_selecting = true;
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
        if let Some(line) = self.hit_fold_marker(pos) {
            self.toggle_fold(line);
            return;
        }
        if let Some(position) = self.position_at_point(pos) {
            self.move_caret_to(position, false);
        }
    }

    pub(crate) fn pointer_drag(&mut self, pos: Point) {
        if !self.mouse_selecting {
            return;
        }
        if let Some(position) = self.position_at_point(pos) {
            self.move_caret_to(position, true);
        }
    }

    pub(crate) fn pointer_release(&mut self) {
        self.mouse_selecting = false;
        self.base.set_mouse_pressed(false);
    }

    fn hit_menu_row(&self, pos: Point) -> Option<usize> {
        let rect = self.context_menu_rect()?;
        index_in_rows(pos, rect, self.context_menu.items.len())
    }

    fn hit_completion_row(&self, pos: Point) -> Option<usize> {
        let rect = self.completion_rect();
        index_in_rows(pos, rect, self.completion.items.len())
    }

    fn hit_fold_marker(&self, pos: Point) -> Option<usize> {
        let rect = self.geometry();
        let width = self.fold_marker_width();
        if pos.x < rect.x || pos.x >= rect.x + width {
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
