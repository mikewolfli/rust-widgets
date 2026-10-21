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
    pub fn buffers(&self) -> Vec<EditorBuffer> {
        self.model.borrow().all_buffers.clone()
    }

    /// Opens a new buffer and activates it, returning its index.
    pub fn open_buffer(&mut self, title: impl Into<String>, text: impl Into<String>) -> usize {
        let index = {
            let mut model = self.model.borrow_mut();
            model.all_buffers.push(EditorBuffer::new(title, text));
            model.all_buffers.len() - 1
        };
        self.activate_buffer(index);
        index
    }

    /// Returns the active buffer index.
    /// Returns the editor's state for a host that delegates to it.
    pub fn active_buffer(&self) -> usize {
        self.model.borrow().active_tab
    }

    /// Activates a buffer, pushing an undoable tab-switch entry.
    pub fn activate_buffer(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        let before = self.active_buffer();
        self.undo_stack.push(Box::new(TabSwitchCommand {
            editor: Rc::clone(&self.model),
            before,
            after: index,
        }));
        self.model.borrow_mut().apply_tab_switch(index);
        self.after_buffer_switch(index);
        true
    }

    /// Activates a buffer without recording an undo entry.
    pub fn activate_buffer_untracked(&mut self, index: usize) -> bool {
        let count = self.model.borrow().all_buffers.len();
        if index >= count || index == self.active_buffer() {
            return false;
        }
        self.model.borrow_mut().apply_tab_switch(index);
        // An untracked switch records no `TabSwitchCommand`, so any range or
        // snapshot commands still on the stack belong to the previous document.
        // Replaying them against this one would splice by the wrong offsets, so
        // the history is invalidated on the cross-document boundary.
        self.undo_stack.clear();
        self.after_buffer_switch(index);
        true
    }

    pub(super) fn after_buffer_switch(&mut self, index: usize) {
        self.cursor = Cursor::default();
        self.goal = CursorGoal::default();
        self.scroll_line = 0;
        self.scroll_column = 0;
        self.scroll_visual_row = 0;
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
        // Locked ranges are document coordinates too, so they are dropped rather
        // than inherited by the buffer being entered. A host that re-locks the
        // new buffer re-adds them after the switch.
        self.read_only_spans.clear();
        self.refresh_derived_state();
        self.text_changed.emit(self.text());
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
        // Closing a tab mutates slot indices and removes a document, so any
        // pending history — which may reference a removed slot or the removed
        // document's text mirror — is invalidated rather than left to mis-target
        // a surviving buffer.
        self.undo_stack.clear();

        // Persist the active buffer's text to its own slot *before* the index
        // shifts, unless the active buffer itself is the one being closed — its
        // text must not be written into a surviving slot.
        let closed_is_active = self.model.borrow().active_tab == index;
        if !closed_is_active {
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
