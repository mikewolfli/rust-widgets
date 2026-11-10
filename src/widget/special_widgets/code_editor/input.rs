// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard, IME, pointer and wheel plumbing for the [`CodeEditor`].
//!
//! This module is the *translation layer* between platform events and the
//! editing commands in `editor.rs`: it decides which command a key chord means,
//! routes IME commits into the document, and turns pointer coordinates into
//! carets, selections and fold toggles. It owns no document state of its own.

use super::editor::CodeEditor;
use super::grid::LineGrid;
use super::types::{Cursor, FindBarAction, TextPosition, MIN_TOUCH_TARGET};
use crate::core::Point;
use crate::widget::Widget;
use alloc::string::String;
use alloc::vec::Vec;

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
        if text.is_empty() {
            return;
        }
        // The find/goto bar is chrome, not document content: a read-only document can
        // still be searched, so this branch must come before the editable gate.
        if self.find.visible {
            let printable: String = text.chars().filter(|ch| !ch.is_control()).collect();
            if self.goto_mode {
                // The goto bar accepts line numbers only; a pasted letter must not
                // silently become part of a number that never parses.
                let digits: String = printable.chars().filter(|ch| ch.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    self.find.query.push_str(&digits);
                    self.base.request_redraw();
                }
                return;
            }
            if !printable.is_empty() {
                self.append_find_query(&printable);
            }
            return;
        }
        if !self.is_editable() {
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
        // Windows reports AltGr as Ctrl+Alt. Every single-modifier shortcut below must
        // ignore that pair, or typing an AltGr character (e.g. `ź` as AltGr+X) would also
        // cut the selection / undo / select all before the character is delivered.
        let ctrl_chord = ctrl && !alt;

        if self.context_menu.visible && self.handle_menu_key(key, ctrl).is_some() {
            return;
        }
        if self.completion.visible && self.handle_completion_key(key).is_some() {
            return;
        }

        match key {
            // Word-wise movement and selection (Zed/VS Code bindings) must be
            // matched before the plain arrow arms or they become unreachable.
            37 if ctrl_chord => self.move_word(-1, shift),
            39 if ctrl_chord => self.move_word(1, shift),
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
                if ctrl_chord {
                    self.move_to_document_edge(-1, shift);
                } else {
                    self.move_to_line_edge(-1, shift);
                }
            }
            35 => {
                if ctrl_chord {
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
                } else if ctrl_chord {
                    if shift {
                        self.insert_line_above();
                    } else {
                        self.insert_line_below();
                    }
                } else {
                    self.insert_newline();
                }
            }
            // A snippet session owns Tab/Shift+Tab while it runs. The fallback is
            // the unchanged indent/outdent behaviour, so a session that cannot move
            // (no stops, or the first stop) never eats the gesture.
            9 => {
                if self.completion.visible {
                    self.accept_completion();
                } else if self.context_menu.visible {
                    let id =
                        self.context_menu.items.get(self.context_menu.selected).map(|item| item.id);
                    if let Some(id) = id {
                        self.activate_menu_item(id);
                    }
                } else if self.is_in_snippet() && shift && self.previous_placeholder() {
                    // The session owns Shift+Tab while it runs; the fallback below
                    // is the unchanged indent/outdent behaviour.
                } else if self.is_in_snippet() && !shift && self.next_placeholder() {
                    // The session owns Tab while it runs.
                } else if alt || shift {
                    self.outdent_selection();
                } else {
                    self.insert_tab();
                }
            }
            27 | 0x84 => {
                // `0x84` is the X11-style ESC some platform layers map; both codes have to
                // leave the same state, including the rectangular gesture and the caret set.
                if self.context_menu.visible {
                    self.close_context_menu();
                } else if self.completion.visible {
                    self.dismiss_completion();
                } else if self.find.visible {
                    self.close_find();
                } else if self.cancel_snippet_session() {
                    // Escape leaves the snippet but keeps the text that was inserted,
                    // which is the difference between abandoning navigation and
                    // undoing the expansion.
                } else if self.column_select_anchor.is_some() {
                    // Escape leaves the rectangular gesture and keeps the primary caret's
                    // line as an ordinary selection, so the block editing can still be
                    // continued one line at a time (ED-04).
                    self.column_select_anchor = None;
                    self.column_selecting = false;
                    self.collapse_cursors();
                } else {
                    // Escape collapses the caret set, as in Zed/VS Code.
                    self.collapse_cursors();
                }
            }
            // Ctrl/Meta chords.
            90 if ctrl_chord && shift => {
                let _ = self.redo();
            }
            90 if ctrl_chord => {
                let _ = self.undo();
            }
            89 if ctrl_chord && shift => {
                let _ = self.redo();
            }
            89 if ctrl_chord => {
                let _ = self.redo();
            }
            // Find next/previous, the F3 convention (framework F-key range).
            1122 => {
                let _ = self.find_next(shift);
            }
            // Multi-caret and line commands (Zed / VS Code bindings).
            68 if ctrl_chord && shift => self.duplicate_line(),
            68 if ctrl_chord => {
                self.select_next_occurrence();
            }
            // Ctrl+L selects the caret's line.
            76 if ctrl_chord && !shift => self.select_line(self.cursor.head.line),
            76 if ctrl_chord && shift => {
                self.select_all_occurrences();
            }
            75 if ctrl_chord && shift => self.delete_line(),
            // Join Lines is `J`; it must be matched before the plain-`J` arm or
            // the later arm swallows it (the two arms share a key and the first
            // match wins).
            74 if ctrl_chord => self.join_lines(),
            // Transpose the two characters around the caret.
            84 if ctrl_chord => self.transpose_characters(),
            65 if ctrl_chord => self.select_all(),
            67 if ctrl_chord => {
                self.copy();
            }
            88 if ctrl_chord => {
                self.cut();
            }
            86 if ctrl_chord => {
                self.paste();
            }
            70 if ctrl_chord => self.open_find(false),
            72 if ctrl_chord => self.open_find(true),
            // Go to line, reusing the find bar's input row.
            71 if ctrl_chord => self.open_goto_bar(),
            47 if ctrl_chord => self.toggle_line_comment(),
            32 if ctrl_chord => {
                self.trigger_completion();
            }
            73 if ctrl_chord => self.indent_selection(),
            91 if ctrl_chord => {
                let line = self.cursor.head.line;
                self.toggle_fold(line);
            }
            93 if ctrl_chord => self.unfold_all(),
            61 if ctrl_chord => self.scroll_by(self.visible_rows() as isize),
            45 if ctrl_chord => self.scroll_by(-(self.visible_rows() as isize)),
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
                if self.goto_mode {
                    self.commit_goto();
                } else {
                    self.find_next(shift);
                }
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
                if !self.goto_mode && self.find.replace_visible {
                    self.find.focus_replace = !self.find.focus_replace;
                    self.base.request_redraw();
                }
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
        // The find bar is chrome above the buffer: its buttons are commands, never a
        // caret placement, and its two text rows take focus on a press so the keyboard
        // path follows what the reader clicked (ED-01).
        if self.find.visible {
            if let Some(action) = self.find_bar_action_at(pos) {
                self.mouse_selecting = false;
                self.apply_find_bar_action(action);
                return;
            }
            if self.find_bar_rect().is_some_and(|bar| bar.contains(pos)) {
                self.mouse_selecting = false;
                let row =
                    (pos.y - (self.geometry().y + self.tab_strip_height())) / MIN_TOUCH_TARGET + 1;
                let focus_replace = self.find.replace_visible && row > 1;
                if self.find.focus_replace != focus_replace {
                    self.find.focus_replace = focus_replace;
                    self.base.request_redraw();
                }
                return;
            }
        }
        if self.context_menu.visible {
            self.mouse_selecting = false;
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
            self.mouse_selecting = false;
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
            self.mouse_selecting = false;
            self.toggle_fold(line);
            return;
        }
        // A press on a line number selects that line, the way Zed's gutter does: the
        // gutter is the one place where a click can only mean "this line", and
        // dragging from it is how a range of whole lines is chosen.
        if let Some(line) = self.hit_gutter_line(pos) {
            // A gutter press is a line gesture, never a text-selection gesture: the
            // branch below starts its own drag, and the breakpoint arm returns without
            // one. Leaving `mouse_selecting` set turned a later 1 px move into a text
            // selection expanding from the caret.
            self.mouse_selecting = false;
            // Ctrl (or Command) on the gutter is the debugger gesture: it toggles a
            // breakpoint instead of selecting the line.
            if modifiers & (MODIFIER_CONTROL | MODIFIER_META) != 0 {
                self.toggle_breakpoint(line);
                return;
            }
            self.gutter_dragging = true;
            self.gutter_anchor = Some(line);
            // A marker-bearing line reports activation, so a host can open the
            // diagnostic or breakpoint it just clicked.
            if self.is_breakpoint(line) || self.markers.iter().any(|marker| marker.line == line) {
                self.marker_activated.emit(line);
            }
            // And the marker itself is reported by index, which is what "jump to the
            // problem I clicked" needs — the line alone does not say *which* of several
            // markers on it was meant (ED-02).
            if let Some(index) = self.markers.iter().position(|marker| marker.line == line) {
                self.marker_clicked.emit(index);
            }
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
                self.column_select_anchor = None;
                self.select_line(position.line);
            } else if shift {
                // Shift+click extends the selection from the existing anchor.
                self.move_caret_to(position, true);
            } else if modifiers & MODIFIER_ALT != 0 {
                // Alt+drag is the rectangular-selection gesture (ED-04).
                self.begin_column_selection(position);
            } else if modifiers & MODIFIER_CONTROL != 0 {
                self.add_cursor_at(position);
            } else if self.selection_contains(position) {
                // A press inside the selection starts a drag, not a new caret.
                self.mouse_selecting = false;
                self.dragging_text = true;
                self.drag_origin = Some(position);
            } else {
                self.column_select_anchor = None;
                self.move_caret_to(position, false);
            }
        }
    }

    pub(crate) fn pointer_drag(&mut self, pos: Point) {
        // Hover is tracked even when no button is down: it is what reveals the close
        // affordance on a tab, washes it as the pointer passes over, and shows a
        // diagnostic's full message beside the line that carries it (ED-03).
        self.update_hovered_tab(pos);
        self.update_hovered_diagnostic(pos);
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
            // A minimap drag keeps following the pointer after it leaves the strip.
            // The point is converted through the strip's own row scale so it means the
            // same row the bars do; `scroll_to_fraction` clamps the ends.
            let fraction = self.minimap_fraction_for_y(pos.y);
            self.scroll_to_fraction(fraction);
            return;
        }
        if self.dragging_text {
            self.last_pointer_position = Some(pos);
            return;
        }
        if self.column_selecting {
            self.update_column_selection(pos);
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

    /// Drops every pointer-driven latch without committing anything.
    ///
    /// A window can lose focus mid-gesture (alt-tab, a modal dialog, a platform
    /// grab), and the closing release event then never arrives. The stale latch made
    /// the next pointer move extend a selection or drag text the reader had already
    /// let go of. `FocusLost` routes here.
    pub(crate) fn cancel_pointer_gestures(&mut self) {
        self.column_selecting = false;
        self.column_select_anchor = None;
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

    pub(crate) fn pointer_release(&mut self) {
        if self.dragging_text {
            if let Some(pos) = self.last_pointer_position {
                if let Some(target) = self.position_at_point(pos) {
                    self.move_selection_to(target);
                }
            }
        }
        self.cancel_pointer_gestures();
    }

    /// Tracks which line's diagnostic message is under the pointer.
    ///
    /// The caret line already shows its message inline; every other line showed only a
    /// coloured rule, which left the message unreachable without host code walking
    /// `markers()`. Hovering that line now reveals it in place.
    pub(crate) fn update_hovered_diagnostic(&mut self, pos: Point) {
        let next = if self.markers.is_empty() {
            None
        } else {
            self.position_at_point(pos)
                .map(|position| position.line)
                .filter(|line| self.markers.iter().any(|marker| marker.line == *line))
        };
        if next != self.hovered_diagnostic {
            self.hovered_diagnostic = next;
            self.base.request_redraw();
        }
    }

    fn hit_menu_row(&self, pos: Point) -> Option<usize> {
        let rect = self.context_menu_rect()?;
        index_in_rows(pos, rect, self.context_menu.items.len())
    }

    /// Starts a rectangular selection anchored at `position`.
    fn begin_column_selection(&mut self, position: TextPosition) {
        self.column_select_anchor = Some(position);
        self.column_selecting = true;
        self.mouse_selecting = false;
        self.cursor = Cursor { head: position, anchor: position };
        self.extra_cursors.clear();
        // Placing the caret explicitly cancels the sticky goal column, like every other
        // "put the caret here" path; otherwise the next vertical move jumps back to the
        // column the user chose before the Alt-click.
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// Extends the rectangular selection to the line and column under the pointer.
    ///
    /// The rectangle is expressed in **grid columns**, so a tab or a CJK glyph widens the
    /// band exactly as it is drawn; each covered line then converts those columns back to
    /// its own character indices. Every covered line becomes a caret whose anchor and head
    /// are the rectangle's two edges, which is what lets every editing command (insert,
    /// delete, indent) and the single undo entry work on the block unchanged (ED-04).
    fn update_column_selection(&mut self, pos: Point) {
        let Some(anchor) = self.column_select_anchor else { return };
        let Some(focus) = self.position_at_point(pos) else { return };
        let anchor_column = self.visual_column_at(anchor);
        let focus_column = self.visual_column_at(focus);
        let from_column = anchor_column.min(focus_column);
        let to_column = anchor_column.max(focus_column);
        let (start_line, end_line) = if anchor.line <= focus.line {
            (anchor.line, focus.line)
        } else {
            (focus.line, anchor.line)
        };
        let mut carets: Vec<Cursor> = Vec::new();
        for line in start_line..=end_line {
            if self.is_hidden(line) {
                continue;
            }
            let text = self.line_text_borrowed(line);
            let grid = LineGrid::new(&text, self.config.tab_width);
            let start = grid.char_at_or_after_column(from_column);
            let end = grid.char_at_or_after_column(to_column);
            carets.push(Cursor {
                head: TextPosition::new(line, end),
                anchor: TextPosition::new(line, start),
            });
        }
        if carets.is_empty() {
            return;
        }
        // The focus line's caret is the primary one, matching where the pointer is. The
        // vector is in ascending line order, so `pop` named the *end* line and an upward
        // drag put the primary caret on the anchor line instead of under the pointer.
        let primary = carets
            .iter()
            .position(|caret| caret.head.line == focus.line)
            .map(|index| carets.remove(index))
            .or_else(|| carets.pop())
            .unwrap_or(self.cursor);
        self.cursor = primary;
        self.extra_cursors = carets;
        self.goal.active = false;
        self.emit_cursor_and_selection();
        self.base.request_redraw();
    }

    /// The grid column of a document position.
    fn visual_column_at(&self, position: TextPosition) -> usize {
        let text = self.line_text_borrowed(position.line);
        LineGrid::new(&text, self.config.tab_width).column_of_char(position.column)
    }

    /// The find-bar action whose button contains `pos`.
    fn find_bar_action_at(&self, pos: Point) -> Option<FindBarAction> {
        self.find_bar_buttons()
            .into_iter()
            .find(|(_, button)| button.contains(pos))
            .map(|(action, _)| action)
    }

    /// Runs one find-bar action.
    ///
    /// Every arm defers to the existing command rather than re-implementing it, so the
    /// button and the keyboard shortcut cannot diverge: a toggle writes the same
    /// [`SearchOptions`] the shortcut writes, and the four actions call the same
    /// find/replace entry points (ED-01).
    fn apply_find_bar_action(&mut self, action: FindBarAction) {
        match action {
            FindBarAction::Close => self.close_find(),
            FindBarAction::ToggleReplace => {
                self.find.replace_visible = !self.find.replace_visible;
                if !self.find.replace_visible {
                    self.find.focus_replace = false;
                }
                self.base.request_layout();
                self.base.request_redraw();
            }
            FindBarAction::ToggleCase
            | FindBarAction::ToggleWholeWord
            | FindBarAction::ToggleRegex => {
                let mut options = self.find.options;
                match action {
                    FindBarAction::ToggleCase => options.case_sensitive = !options.case_sensitive,
                    FindBarAction::ToggleWholeWord => options.whole_word = !options.whole_word,
                    _ => options.use_regex = !options.use_regex,
                }
                self.set_search_options(options);
            }
            FindBarAction::Previous => {
                self.find_next(true);
            }
            FindBarAction::Next => {
                self.find_next(false);
            }
            FindBarAction::Replace => {
                self.replace_current();
            }
            FindBarAction::ReplaceAll => {
                self.replace_all();
            }
        }
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

    /// Where in the minimap strip `pos` is, as a fraction of the scrollable range.
    ///
    /// `None` outside the strip, or when the minimap is disabled. The strip paints the
    /// document compacted at the top (`minimap_row_scale`), so the point is converted
    /// through that same scale: dividing by the full strip height put a click on the
    /// bar for line 33 at line 6.
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
        Some(self.minimap_fraction_for_y(pos.y))
    }

    /// The fraction of the scrollable range the minimap row under `y` names.
    ///
    /// A drag keeps following the pointer after it leaves the strip, so this takes a raw
    /// `y` and clamps, while [`Self::minimap_fraction_at`] rejects points outside it.
    fn minimap_fraction_for_y(&self, y: i32) -> f32 {
        let rect = self.geometry();
        let top = rect.y + self.text_origin_y();
        let row = ((y - top) as f32 / self.minimap_row_scale()).max(0.0);
        let rows = self.visible_rows().max(1);
        let max_first = self.total_visual_rows().saturating_sub(rows);
        if max_first == 0 {
            return 0.0;
        }
        // `scroll_to_fraction` multiplies by `max_first`, so normalising by it as well
        // puts the clicked row exactly at the top of the viewport.
        (row / max_first as f32).clamp(0.0, 1.0)
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
        // The same inverse the text hit-test uses: `floor(local / h)` disagreed with the
        // painter's `round(k * h)` at fractional row heights, so the top pixel of a line
        // selected the line above it.
        let row = self.coordinate_map().row_at_y(pos.y);
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
        let row = self.coordinate_map().row_at_y(pos.y);
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
