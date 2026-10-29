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
    /// [`Self::buffer_summaries`], which allocates a title per tab and nothing else.
    pub fn buffers(&self) -> Vec<EditorBuffer> {
        self.document.buffer_snapshots()
    }

    /// Returns `(title, modified, language)` for every open buffer, without its text.
    ///
    /// A tab strip needs no document contents, and `buffers()` cloned every open file to
    /// answer it — megabytes per frame for a host that redraws its tab bar from this call.
    pub fn buffer_summaries(&self) -> Vec<(String, bool, Option<LanguageId>)> {
        self.document.buffer_summaries()
    }

    /// Opens a new buffer and activates it, returning its index.
    pub fn open_buffer(&mut self, title: impl Into<String>, text: impl Into<String>) -> usize {
        let index = self.document.push_buffer(title.into(), text.into());
        self.document
            .slot_history(index)
            .borrow_mut()
            .set_max_capacity(self.config.undo_stack_limit);
        if let Some(id) = self.document.buffer_id(index) {
            self.buffer_views.entry(id).or_default();
        }
        self.seen_generation = self.document.generation();
        self.activate_buffer(index);
        index
    }

    /// Returns the active buffer index.
    ///
    /// This is a *position*, so it moves when an earlier buffer is closed; the view follows its
    /// buffer by id internally. Use it for indexing [`Self::buffers`], not as an identity —
    /// [`Self::active_buffer_id`] is the identity.
    pub fn active_buffer(&self) -> usize {
        self.document.index_of(self.active_buffer_id).unwrap_or_else(|| {
            self.active_index.min(self.document.buffer_count().saturating_sub(1))
        })
    }

    /// Returns the stable id of the active buffer.
    ///
    /// A buffer keeps its id for its whole life: closing an earlier tab changes every position
    /// after it, but not this value. A host that stores which tab a pane shows should store the
    /// id and resolve it with [`Self::activate_buffer_id`]; an index would silently start
    /// meaning a different buffer the next time someone closes a tab.
    ///
    /// If *another* view closed this buffer, the id remains here until the next
    /// [`Self::sync_with_document`] (which `draw` and `handle_event` call) re-homes the view;
    /// no open buffer has it in the meantime.
    pub fn active_buffer_id(&self) -> u64 {
        self.active_buffer_id
    }

    /// Returns the stable id of every open buffer, in tab order.
    ///
    /// The order matches [`Self::buffers`] and [`Self::buffer_summaries`], so a host can zip the
    /// three without a second lookup. Ids ascend with tab order but are **not** positions: after
    /// a close the list is still ascending but no longer contiguous.
    pub fn buffer_ids(&self) -> Vec<u64> {
        self.document.buffer_ids()
    }

    /// Activates the buffer with the given stable id.
    ///
    /// Returns `false` when no open buffer has that id (it was closed, here or in another view)
    /// and when it is already the active one. This is the identity-based counterpart of
    /// [`Self::activate_buffer`], which takes a position.
    pub fn activate_buffer_id(&mut self, id: u64) -> bool {
        match self.document.index_of(id) {
            Some(index) => self.activate_buffer(index),
            None => false,
        }
    }

    /// Activates a buffer, restoring that buffer's own view and history.
    ///
    /// A tab switch is navigation, not an edit, so it pushes no undo entry. Each buffer
    /// keeps its own caret and scroll in this view's [`BufferViewState`], and its undo
    /// timeline lives in the shared document — so two views on the same buffer undo the
    /// same edits.
    pub fn activate_buffer(&mut self, index: usize) -> bool {
        let count = self.document.buffer_count();
        if index >= count {
            return false;
        }
        let Some(id) = self.document.buffer_id(index) else { return false };
        if id == self.active_buffer_id {
            self.active_index = index;
            return false;
        }
        self.save_outgoing_view();
        self.active_buffer_id = id;
        self.active_index = index;
        self.model = self.document.slot_model(index);
        self.undo_stack = self.document.slot_history(index);
        self.seen_generation = self.document.generation();
        self.after_buffer_switch(index);
        true
    }

    /// Activates a buffer without recording an undo entry.
    ///
    /// Kept for source compatibility: since the history belongs to the buffer rather than to
    /// the switch, no switch records anything, so this is exactly [`Self::activate_buffer`].
    pub fn activate_buffer_untracked(&mut self, index: usize) -> bool {
        self.activate_buffer(index)
    }

    /// Saves the active buffer's caret, scroll and folds into its slot.
    fn save_active_view(&mut self) {
        let view = BufferViewState {
            cursor: self.cursor,
            extra_cursors: self.extra_cursors.clone(),
            goal: self.goal,
            scroll_line: self.scroll_line,
            scroll_column: self.scroll_column,
            scroll_visual_row: self.scroll_visual_row,
            folds: self.folds.clone(),
        };
        self.buffer_views.insert(self.active_buffer_id, view);
    }

    /// Saves the outgoing buffer's view.
    ///
    /// The undo history needs no swap: it belongs to the buffer and lives in the shared
    /// document, so the caller simply re-points it after this returns.
    fn save_outgoing_view(&mut self) {
        self.save_active_view();
        self.active_typing = None;
    }

    pub(super) fn after_buffer_switch(&mut self, index: usize) {
        let id = self.document.buffer_id(index).unwrap_or(self.active_buffer_id);
        let view = self.buffer_views.get(&id).cloned().unwrap_or_default();
        self.cursor = view.cursor;
        self.extra_cursors = view.extra_cursors;
        self.goal = view.goal;
        self.scroll_line = view.scroll_line;
        self.scroll_column = view.scroll_column;
        self.scroll_visual_row = view.scroll_visual_row;
        self.folds = view.folds;
        self.normalize_folds();
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
        // The incoming buffer may be far larger or smaller than the outgoing one, and the
        // reduced-behaviour level is derived from the line count.
        self.scale = DocumentScale::for_lines(self.line_count());
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
        let count = self.document.buffer_count();
        if index >= count {
            return false;
        }
        if count == 1 {
            self.set_text(String::new());
            return true;
        }
        let Some(closed_id) = self.document.buffer_id(index) else { return false };
        let closed_is_active = closed_id == self.active_buffer_id;
        // Save the outgoing view before the list changes; it is keyed by buffer id, so the
        // removal below cannot attach it to a neighbour.
        if !closed_is_active {
            self.save_active_view();
        }
        self.active_typing = None;

        if !self.document.remove_buffer(index) {
            return false;
        }
        // This view's snapshot of the closed buffer is dead; the document dropped the buffer.
        self.buffer_views.remove(&closed_id);
        self.seen_generation = self.document.generation();
        // Re-resolve by id: an inactive close only moves the cached position, while closing
        // the active buffer re-homes to the tab that took its place.
        let _ = self.reconcile_active_buffer();
        true
    }

    /// Closes the buffer with the given stable id.
    ///
    /// The identity-based counterpart of [`Self::close_buffer`], which takes a position. Returns
    /// `false` when no open buffer has that id — already closed, here or in another view.
    ///
    /// Two cases match the position form: closing the **last** buffer empties it instead of
    /// removing it (so the document, and this buffer's id, survive), and closing the **active**
    /// buffer re-homes this view to the tab that took its place.
    pub fn close_buffer_id(&mut self, id: u64) -> bool {
        match self.document.index_of(id) {
            Some(index) => self.close_buffer(index),
            None => false,
        }
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
        model.saved = current;
        model.dirty = false;
    }

    /// Renames the active buffer.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.document.set_title_at(self.active_buffer(), title.into());
    }

    /// Returns the active buffer title.
    pub fn title(&self) -> String {
        self.document.title_at(self.active_buffer()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> CodeEditor {
        CodeEditor::new(Rect::new(0, 0, 800, 600))
    }

    /// A switch persists the outgoing buffer's text before loading the incoming one.
    ///
    /// This is the widget-level form of a case the model used to carry; the model no
    /// longer holds tabs, so the assertion lives where the behaviour does.
    #[test]
    fn switching_buffers_persists_the_outgoing_text() {
        let mut editor = editor();
        editor.set_text("alpha");
        let second = editor.open_buffer("second", "beta");
        assert_eq!(editor.text(), "beta");
        assert!(editor.activate_buffer(0));
        assert_eq!(editor.text(), "alpha");
        assert!(editor.activate_buffer(second));
        assert_eq!(editor.text(), "beta");
    }

    /// Activating an index that does not exist changes nothing.
    #[test]
    fn activating_an_unknown_buffer_is_a_noop() {
        let mut editor = editor();
        editor.set_text("alpha");
        assert!(!editor.activate_buffer(7));
        assert_eq!(editor.active_buffer(), 0);
        assert_eq!(editor.text(), "alpha");
    }
}
