// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Open buffers, activation and the modified flag.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Tabs ────────────────────────────────────────────────────────────────

    /// Returns the open buffers.
    ///
    /// This is the explicit whole-text form: every buffer's full contents are copied. A
    /// host that only needs titles and modified flags should call
    /// [`Self::buffer_summaries`], which allocates a title per tab and nothing else. The
    /// active slot is filled from the live mirror first, because the mirror is where the
    /// active document lives while it is active (J-P12).
    pub fn buffers(&self) -> Vec<EditorBuffer> {
        let model = self.model.borrow();
        let mut buffers = model.all_buffers.clone();
        if let Some(active) = buffers.get_mut(model.active_tab) {
            active.text = model.text.borrow().clone();
        }
        buffers
    }

    /// Returns `(title, modified, language)` for every open buffer, without its text.
    ///
    /// A tab strip needs no document contents, and `buffers()` cloned every open file to
    /// answer it — megabytes per frame for a host that redraws its tab bar from this call.
    pub fn buffer_summaries(&self) -> Vec<(String, bool, Option<LanguageId>)> {
        let model = self.model.borrow();
        model
            .all_buffers
            .iter()
            .enumerate()
            .map(|(index, buffer)| {
                let modified = if index == model.active_tab { model.dirty } else { buffer.modified };
                (buffer.title.clone(), modified, buffer.language)
            })
            .collect()
    }

    /// Opens a new buffer and activates it, returning its index.
    pub fn open_buffer(&mut self, title: impl Into<String>, text: impl Into<String>) -> usize {
        let index = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.push(EditorBuffer::new(title, text));
            model.all_buffers.len() - 1
        };
        self.buffer_histories.push(UndoStack::new());
        self.buffer_views.push(BufferViewState::default());
        self.activate_buffer(index);
        index
    }

    /// Returns the active buffer index.
    /// Returns the editor's state for a host that delegates to it.
    pub fn active_buffer(&self) -> usize {
        self.model.borrow().active_tab
    }

    /// Activates a buffer, restoring that buffer's own history and view.
    ///
    /// A tab switch is navigation, not an edit, so it no longer pushes a
    /// `TabSwitchCommand`: undo used to walk back through tabs before it could
    /// undo text. Each buffer keeps its own undo stack, caret and scroll state
    /// instead.
    pub fn activate_buffer(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        let before = self.active_buffer();
        self.switch_buffer_state(before, index);
        self.model.borrow_mut().apply_tab_switch(index);
        self.after_buffer_switch(index);
        true
    }

    /// Activates a buffer without recording an undo entry.
    ///
    /// History is per buffer, so a switch cannot make one document's commands
    /// replay against another; nothing has to be cleared.
    pub fn activate_buffer_untracked(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        let before = self.active_buffer();
        self.switch_buffer_state(before, index);
        self.model.borrow_mut().apply_tab_switch(index);
        self.after_buffer_switch(index);
        true
    }

    /// Saves the active buffer's caret, scroll and folds into its slot.
    fn save_active_view(&mut self) {
        let active = self.active_buffer();
        let view = BufferViewState {
            cursor: self.cursor,
            extra_cursors: self.extra_cursors.clone(),
            goal: self.goal,
            scroll_line: self.scroll_line,
            scroll_column: self.scroll_column,
            scroll_visual_row: self.scroll_visual_row,
            folds: self.model.borrow().folds.clone(),
        };
        match self.buffer_views.get_mut(active) {
            Some(slot) => *slot = view,
            None => self.buffer_views.push(view),
        }
    }

    /// Saves the outgoing buffer's view and swaps its history for the incoming one.
    fn switch_buffer_state(&mut self, from: usize, to: usize) {
        self.save_active_view();
        let mut incoming = UndoStack::new();
        if let Some(slot) = self.buffer_histories.get_mut(to) {
            core::mem::swap(slot, &mut incoming);
        }
        let outgoing = core::mem::replace(&mut self.undo_stack, incoming);
        match self.buffer_histories.get_mut(from) {
            Some(slot) => *slot = outgoing,
            None => self.buffer_histories.push(outgoing),
        }
        self.active_typing = None;
    }

    pub(super) fn after_buffer_switch(&mut self, index: usize) {
        let view = self.buffer_views.get(index).cloned().unwrap_or_default();
        self.cursor = view.cursor;
        self.extra_cursors = view.extra_cursors;
        self.goal = view.goal;
        self.scroll_line = view.scroll_line;
        self.scroll_column = view.scroll_column;
        self.scroll_visual_row = view.scroll_visual_row;
        {
            let mut model = self.model.borrow_mut();
            model.folds = view.folds;
            model.normalize_folds();
        }
        self.find = FindState {
            visible: self.find.visible,
            replace_visible: self.find.replace_visible,
            ..FindState::default()
        };
        self.dismiss_completion();
        // A different buffer is a different document: every cached state and every
        // minimap bucket belongs to the buffer being left.
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        self.completion_index = None;
        // Locked ranges are document coordinates too, so they are dropped rather
        // than inherited by the buffer being entered. A host that re-locks the
        // new buffer re-adds them after the switch.
        self.read_only_spans.clear();
        self.refresh_derived_state();
        // A switch replaces every line, so it is described as a wholesale change rather
        // than by cloning the new document into the signal payload (J-P12).
        self.text_change.emit(TextChange::document(self.line_count()));
        self.tab_changed.emit(index);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes a buffer. The last remaining buffer is emptied instead.
    pub fn close_buffer(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count {
            return false;
        }
        if count == 1 {
            self.set_text(String::new());
            return true;
        }
        // A closed buffer's history and view belong to that document and are
        // dropped with it; the surviving buffers keep their own.
        if index < self.buffer_histories.len() {
            self.buffer_histories.remove(index);
        }
        if index < self.buffer_views.len() {
            self.buffer_views.remove(index);
        }
        self.active_typing = None;

        // Persist the active buffer's text to its own slot *before* the index
        // shifts, unless the active buffer itself is the one being closed — its
        // text must not be written into a surviving slot.
        let closed_is_active = self.model.borrow().active_tab == index;
        if !closed_is_active {
            self.save_active_view();
            self.model.borrow_mut().persist_active();
        }

        let next_active = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.remove(index);
            if model.active_tab > index {
                model.active_tab -= 1;
            } else if model.active_tab >= model.all_buffers.len() {
                model.active_tab = model.all_buffers.len().saturating_sub(1);
            }
            model.active_tab
        };

        if closed_is_active {
            // The closed buffer's history goes away with it; the survivor's own
            // history becomes the active one.
            let mut incoming = UndoStack::new();
            if let Some(slot) = self.buffer_histories.get_mut(next_active) {
                core::mem::swap(slot, &mut incoming);
            }
            self.undo_stack = incoming;
        }
        self.model.borrow_mut().load_buffer(next_active);
        self.after_buffer_switch(next_active);
        true
    }

    /// Returns `true` when the active buffer has unsaved edits.
    pub fn is_modified(&self) -> bool {
        // A maintained flag, not a comparison: `saved` holds a full copy of the
        // document, so `saved != text` is one pass over the file.
        self.model.borrow().dirty
    }

    /// Marks the active buffer as saved.
    pub fn mark_saved(&mut self) {
        let current = self.model.borrow().text.borrow().clone();
        let mut model = self.model.borrow_mut();
        model.saved = current.clone();
        model.dirty = false;
        let active = model.active_tab;
        if let Some(buffer) = model.all_buffers.get_mut(active) {
            buffer.text = current;
            buffer.modified = false;
        }
    }

    /// Renames the active buffer.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        let mut model = self.model.borrow_mut();
        let active = model.active_tab;
        if let Some(buffer) = model.all_buffers.get_mut(active) {
            buffer.title = title;
        }
    }

    /// Returns the active buffer title.
    pub fn title(&self) -> String {
        let model = self.model.borrow();
        model
            .all_buffers
            .get(model.active_tab)
            .map(|buffer| buffer.title.clone())
            .unwrap_or_default()
    }
}
