// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Document text and edit application.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

/// A shared, clonable handle to one editor document.
///
/// Two [`CodeEditor`] views created with [`CodeEditor::with_document`] from clones of the same
/// handle share the document's **buffers**, their **text**, the **revision counter**, the
/// **fold-free text index** and every buffer's **undo history**. Each view keeps its own active
/// buffer, caret, selection, extra cursors, scroll position and fold regions, so one document
/// set can be shown twice, at two tabs and two places, without either side copying the text.
///
/// # What is shared and what is not
///
/// | State | Owner |
/// |-------|-------|
/// | the buffer list, each buffer's text, revision, undo history | the document (this handle) |
/// | active buffer, caret, selection, extra cursors, scroll, folds | each view |
///
/// # Thread affinity
///
/// A buffer lives in `Rc<RefCell<…>>`, so this handle is `!Send`/`!Sync` exactly like the
/// widget. A host that needs to read the text off the UI thread must copy it first and marshal
/// the work back, the same way it does for every other widget.
#[derive(Debug, Clone)]
pub struct CodeEditorDocument {
    /// Every buffer of this document, in tab order. Never empty.
    slots: Rc<RefCell<Vec<EditorBufferSlot>>>,
    /// The revision counter shared by every view of this document.
    ///
    /// A revision is a property of the *document*, not of a view, which is why it cannot stay
    /// on the widget's own state once two views can edit the same text.
    revision: Rc<core::cell::Cell<u64>>,
    /// Monotonic source of buffer ids. Never reused, so a closed id cannot be mistaken for a
    /// reopened buffer.
    next_id: Rc<core::cell::Cell<u64>>,
    /// Bumped whenever the buffer list changes (open/close).
    ///
    /// A view compares it with the value it last reconciled against: equal means its active
    /// index and its per-buffer snapshots are still current, so `sync_with_document` is an
    /// integer comparison rather than a walk over the tab list.
    generation: Rc<core::cell::Cell<u64>>,
}

impl CodeEditorDocument {
    /// Creates a document holding one `untitled` buffer with `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let model: SharedModel = Rc::new(RefCell::new(EditorModel::new()));
        model.borrow_mut().set_text(text.into());
        let slot = EditorBufferSlot {
            id: 1,
            title: String::from("untitled"),
            language: None,
            model,
            history: Rc::new(RefCell::new(UndoStack::new())),
        };
        Self {
            slots: Rc::new(RefCell::new(vec![slot])),
            revision: Rc::new(core::cell::Cell::new(0)),
            next_id: Rc::new(core::cell::Cell::new(2)),
            generation: Rc::new(core::cell::Cell::new(0)),
        }
    }

    /// Returns the text of the document's **first** buffer.
    ///
    /// The handle has no active-buffer notion — that is view state. For the one-buffer
    /// document [`CodeEditorDocument::new`] creates this is the document text; a view reads
    /// *its* active buffer through [`CodeEditor::text`].
    pub fn text(&self) -> String {
        self.slot_model(0).borrow().text.borrow().clone()
    }

    /// Returns the document's revision counter.
    ///
    /// Every view of the document reads the same value, so a worker result keyed by this
    /// revision is usable from any of them.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Replaces the text of the document's **first** buffer as one committed change.
    ///
    /// The change is visible to every view on its next read, and the revision advances so a
    /// stale worker result is refused.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.slot_model(0).borrow_mut().set_text(text.into());
        self.revision.set(self.revision.get().wrapping_add(1));
    }

    // ── Crate-internal access ────────────────────────────────────────────────

    /// The revision handle, for a view that wants to read it without a borrow.
    pub(crate) fn revision_handle(&self) -> Rc<core::cell::Cell<u64>> {
        Rc::clone(&self.revision)
    }

    /// Number of buffers in the document.
    pub(crate) fn buffer_count(&self) -> usize {
        self.slots.borrow().len()
    }

    /// The stable id of the buffer at `index`.
    pub(crate) fn buffer_id(&self, index: usize) -> Option<u64> {
        self.slots.borrow().get(index).map(|slot| slot.id)
    }

    /// The current position of the buffer with `id`.
    ///
    /// `None` means the buffer was closed. Positions shift when an earlier buffer is closed;
    /// the id does not, which is why the view stores the id and looks the position up here.
    pub(crate) fn index_of(&self, id: u64) -> Option<usize> {
        self.slots.borrow().iter().position(|slot| slot.id == id)
    }

    /// Every buffer id, in tab order (ascending, because ids are never reused).
    pub(crate) fn buffer_ids(&self) -> Vec<u64> {
        self.slots.borrow().iter().map(|slot| slot.id).collect()
    }

    /// The buffer-list generation, bumped on every open and close.
    pub(crate) fn generation(&self) -> u64 {
        self.generation.get()
    }

    /// The text model of buffer `index`.
    ///
    /// # Panics
    ///
    /// A document always holds at least one buffer: [`Self::remove_buffer`] refuses to remove
    /// the last, and construction starts with one. An out-of-range index here is therefore a
    /// bug in a caller, not a state a host can produce, and failing loudly is better than
    /// handing back a detached model that edits go into and nobody reads.
    pub(crate) fn slot_model(&self, index: usize) -> SharedModel {
        Rc::clone(
            &self
                .slots
                .borrow()
                .get(index)
                .expect("a document always holds at least one buffer")
                .model,
        )
    }

    /// The undo history of buffer `index`, shared by every view of it.
    pub(crate) fn slot_history(&self, index: usize) -> Rc<RefCell<UndoStack>> {
        Rc::clone(
            &self
                .slots
                .borrow()
                .get(index)
                .expect("a document always holds at least one buffer")
                .history,
        )
    }

    /// Appends a buffer and returns its index.
    pub(crate) fn push_buffer(&self, title: String, text: String) -> usize {
        let model: SharedModel = Rc::new(RefCell::new(EditorModel::new()));
        model.borrow_mut().set_text(text);
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        let slot = EditorBufferSlot {
            id,
            title,
            language: None,
            model,
            history: Rc::new(RefCell::new(UndoStack::new())),
        };
        let mut slots = self.slots.borrow_mut();
        slots.push(slot);
        let index = slots.len() - 1;
        drop(slots);
        self.generation.set(self.generation.get().wrapping_add(1));
        index
    }

    /// Removes buffer `index`.
    ///
    /// Refuses to remove the last buffer and reports `false`; a document is never empty. Every
    /// view re-homes its active buffer through [`CodeEditor::sync_with_document`].
    pub(crate) fn remove_buffer(&self, index: usize) -> bool {
        let removed = {
            let mut slots = self.slots.borrow_mut();
            if slots.len() <= 1 || index >= slots.len() {
                return false;
            }
            slots.remove(index);
            true
        };
        if removed {
            self.generation.set(self.generation.get().wrapping_add(1));
        }
        removed
    }

    /// Applies an undo-stack limit to every buffer's history.
    pub(crate) fn set_history_limit(&self, limit: usize) {
        for slot in self.slots.borrow().iter() {
            slot.history.borrow_mut().set_max_capacity(limit);
        }
    }

    /// Copies every buffer out as the public value type.
    pub(crate) fn buffer_snapshots(&self) -> Vec<EditorBuffer> {
        self.slots
            .borrow()
            .iter()
            .map(|slot| {
                let model = slot.model.borrow();
                let text = model.text.borrow().clone();
                EditorBuffer {
                    title: slot.title.clone(),
                    text,
                    language: slot.language,
                    modified: model.dirty,
                }
            })
            .collect()
    }

    /// `(title, modified, language)` for every buffer.
    pub(crate) fn buffer_summaries(&self) -> Vec<(String, bool, Option<LanguageId>)> {
        self.slots
            .borrow()
            .iter()
            .map(|slot| (slot.title.clone(), slot.model.borrow().dirty, slot.language))
            .collect()
    }

    /// The title of buffer `index`.
    pub(crate) fn title_at(&self, index: usize) -> Option<String> {
        self.slots.borrow().get(index).map(|slot| slot.title.clone())
    }

    /// Renames buffer `index`.
    pub(crate) fn set_title_at(&self, index: usize, title: String) {
        if let Some(slot) = self.slots.borrow_mut().get_mut(index) {
            slot.title = title;
        }
    }

    /// The language override of buffer `index`.
    pub(crate) fn language_at(&self, index: usize) -> Option<Option<LanguageId>> {
        self.slots.borrow().get(index).map(|slot| slot.language)
    }

    /// Sets the language override of buffer `index`, reporting whether it exists.
    pub(crate) fn set_language_at(&self, index: usize, language: Option<LanguageId>) -> bool {
        match self.slots.borrow_mut().get_mut(index) {
            Some(slot) => {
                slot.language = language;
                true
            }
            None => false,
        }
    }
}

impl CodeEditor {
    /// Returns a shared handle to this editor's document.
    ///
    /// The editor keeps using the document; a second view is created from the returned handle
    /// with [`CodeEditor::with_document`].
    pub fn document(&self) -> CodeEditorDocument {
        self.document.clone()
    }

    /// Consumes the editor and returns its document handle.
    ///
    /// The editor's view state (caret, scroll, folds) is dropped with it; the document's
    /// buffers, their text, revision and history live on in the handle.
    pub fn into_document(self) -> CodeEditorDocument {
        self.document.clone()
    }

    /// Creates a view onto an existing [`CodeEditorDocument`].
    ///
    /// The new view starts on the first buffer, at the top, with no folds. Its caret,
    /// selection, extra cursors and scroll position are private to it, so two views of one
    /// document can sit at different places — and on different buffers — at once.
    ///
    /// This view's configured undo limit is applied to every buffer's history, because the
    /// limit belongs to the document the two views share.
    pub fn with_document(geometry: Rect, document: CodeEditorDocument) -> Self {
        let mut editor = Self::new(geometry);
        editor.adopt_document(document);
        editor
    }

    /// Re-points this editor at `document`, discarding its current view state.
    ///
    /// Used by [`CodeEditor::with_document`]. Every buffer gets a fresh snapshot slot keyed by
    /// its stable id, so a later close cannot shift one buffer's caret onto another.
    fn adopt_document(&mut self, document: CodeEditorDocument) {
        self.document = document;
        self.revision = self.document.revision_handle();
        self.active_index = 0;
        self.active_buffer_id = self.document.buffer_id(0).unwrap_or(1);
        self.seen_generation = self.document.generation();
        self.model = self.document.slot_model(0);
        self.undo_stack = self.document.slot_history(0);
        self.buffer_views.clear();
        for id in self.document.buffer_ids() {
            self.buffer_views.insert(id, BufferViewState::default());
        }
        self.folds.clear();
        self.hidden_ranges.clear();
        self.cursor = Cursor::default();
        self.extra_cursors.clear();
        self.goal = CursorGoal::default();
        self.scroll_line = 0;
        self.scroll_column = 0;
        self.scroll_visual_row = 0;
        self.active_typing = None;
        self.completion_index = None;
        *self.bracket_cache.borrow_mut() = None;
        self.minimap_line_count = 0;
        self.scale = DocumentScale::for_lines(self.line_count());
        self.document.set_history_limit(self.config.undo_stack_limit);
        self.invalidate_line_states();
        self.refresh_derived_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Re-homes this view after the shared document's buffer set changed under it.
    ///
    /// Opening or closing a buffer mutates the document, which every view shares. Because the
    /// view stores the buffer's **stable id** rather than its position, a close elsewhere neither
    /// changes which buffer this view shows nor moves its saved caret: the position is looked up
    /// again here and the per-buffer snapshots already belong to the right buffer. Only when the
    /// active buffer itself was closed elsewhere does the selection move, to the neighbour that
    /// took its place.
    ///
    /// The paint and input paths call this at their entry point; a host that drives the editor
    /// directly calls it before reading or editing.
    ///
    /// Returns `true` when the active buffer changed. A change emits the same signals a
    /// [`CodeEditor::activate_buffer`] does (`tab_changed`, `text_change`, cursor/selection).
    pub fn sync_with_document(&mut self) -> bool {
        if self.seen_generation == self.document.generation() {
            return false;
        }
        self.seen_generation = self.document.generation();
        self.reconcile_active_buffer()
    }

    /// Re-resolves the active buffer against the current slot list.
    ///
    /// Shared by [`CodeEditor::sync_with_document`] and the close path. Snapshots of buffers that
    /// no longer exist are pruned here, so a long-lived view does not accumulate them.
    ///
    /// Returns `true` when the active buffer changed.
    pub(super) fn reconcile_active_buffer(&mut self) -> bool {
        let count = self.document.buffer_count();
        if count == 0 {
            return false;
        }
        // Ids ascend with tab order, so this is a binary search per snapshot.
        let ids = self.document.buffer_ids();
        self.buffer_views.retain(|id, _| ids.binary_search(id).is_ok());
        let index = match self.document.index_of(self.active_buffer_id) {
            Some(index) => index,
            // The active buffer was closed elsewhere: take the position it used to hold, which
            // is now the following buffer (or the last one when it was the final tab).
            None => self.active_index.min(count - 1),
        };
        let Some(id) = self.document.buffer_id(index) else { return false };
        let changed = id != self.active_buffer_id;
        self.active_index = index;
        if changed {
            self.active_buffer_id = id;
            self.model = self.document.slot_model(index);
            self.undo_stack = self.document.slot_history(index);
            self.after_buffer_switch(index);
        }
        changed
    }
}

impl CodeEditor {
    // ── Text access ─────────────────────────────────────────────────────────

    /// Returns the current buffer text.
    pub fn text(&self) -> String {
        self.model.borrow().text.borrow().clone()
    }

    /// The document's revision counter, bumped on every committed text change (BLUE26 B26.08).
    ///
    /// # How a host uses it
    ///
    /// Record this value when handing the text to an asynchronous worker (a linter, an indexer), and
    /// compare it on return: if it moved, the worker's positions describe text that is gone. The
    /// counter starts at `0` and only increases, so "unchanged" is an equality test and never a
    /// guess.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    /// Returns the line terminator the document was loaded with.
    pub fn line_ending(&self) -> LineEnding {
        self.model.borrow().line_ending()
    }

    /// Records the terminator the document should be saved with.
    ///
    /// The buffer itself always uses `\n`; this is metadata a host reads when it
    /// writes the file back.
    pub fn set_line_ending(&mut self, ending: LineEnding) {
        self.model.borrow_mut().set_line_ending(ending);
    }

    /// Replaces the half-open character range `[start, end)` with `text`, but only if `expected` is
    /// still the current [`revision`](Self::revision) (BLUE26 B26.08).
    ///
    /// # Why a version-guarded edit is the primitive a tool host needs
    ///
    /// An automatic fix — a formatter, an import sorter, a quick-fix — computes a replacement from a
    /// snapshot of the text and then applies it some time later. If the user typed in between, the
    /// computed range now covers different text and a blind write would corrupt the document. Passing
    /// the revision the fix was computed against lets the editor refuse a stale edit rather than
    /// apply it, which is the only safe ordering when the two cannot be made atomic.
    ///
    /// # Why positions are `(line, column)` rather than byte offsets
    ///
    /// Column is a **character** index, matching every other position in this editor
    /// ([`TextPosition`]). A byte offset would depend on the encoding of the surrounding text and
    /// would place a caret inside a multi-byte character on a later edit; the character column is
    /// the coordinate the rest of the API already uses.
    ///
    /// # Returns
    ///
    /// * `Ok(new_revision)` — the edit was applied; the caller can chain a second edit against it.
    /// * `Err(StaleRevision { expected, actual })` — the document moved on; the caller re-reads and
    ///   recomputes. Nothing was written.
    /// * `Err(ReadOnly)` — the editor or the range is locked; nothing was written.
    pub fn replace_range(
        &mut self,
        expected: u64,
        start: TextPosition,
        end: TextPosition,
        text: &str,
    ) -> Result<u64, EditRejection> {
        if expected != self.revision.get() {
            return Err(EditRejection::StaleRevision { expected, actual: self.revision.get() });
        }
        if !self.is_editable() || !self.is_range_editable(start, end) {
            return Err(EditRejection::ReadOnly);
        }
        // Normalise so a backwards range edits the same span as a forwards one — the caller may hand
        // over a selection whose anchor is after its head.
        let (start, end) = if start <= end { (start, end) } else { (end, start) };
        // A replace that removes nothing and inserts nothing is not an edit. Short-circuiting it
        // here keeps the revision honest without relying on the splice path to notice, which it
        // cannot: an empty in-place splice leaves the model identical and reports nothing different
        // (BLUE26 B26.08).
        if start == end && text.is_empty() {
            return Ok(self.revision.get());
        }
        // The caret lands at the end of the inserted text, spelled the same way `splice_payload`
        // spells it, so a guarded edit and a typed one leave the caret in the same place.
        let newlines = text.matches('\n').count();
        let head = if newlines == 0 {
            TextPosition::new(start.line, start.column + text.chars().count())
        } else {
            TextPosition::new(start.line + newlines, trailing_line_len(text))
        };
        self.splice_and_record(start, end, text, head);
        // `splice_and_record` declines a no-op edit, so the revision may be unchanged; report the
        // current value either way, which is what a chaining caller needs.
        Ok(self.revision.get())
    }

    /// Replaces the whole document, pushing one undo checkpoint.
    ///
    /// A `\r\n` document is stored with `\n` line endings; the original
    /// terminator is reported by [`CodeEditor::line_ending`].
    pub fn set_text(&mut self, text: impl Into<String>) {
        let raw = text.into();
        let next = normalize_line_endings(raw.clone());
        if self.text() == next && !raw.contains('\r') {
            return;
        }
        self.active_typing = None;
        let before = self.text();
        if !self.restoring_history {
            let target = self.model.borrow().text.clone();
            self.undo_stack.borrow_mut().push(Box::new(ValueCommand::new(
                target,
                before,
                next.clone(),
                "Edit text",
                "code_editor_text",
            )));
        }
        self.model.borrow_mut().set_text(raw);
        // Wholesale replacement invalidates every cached state; there is no
        // meaningful "from line N" because no line survived unchanged. Locked
        // spans are positions in the *old* document, so they are dropped rather
        // than silently re-anchored to unrelated text; a host that replaces the
        // document also rebuilds its spans.
        self.read_only_spans.clear();
        // Bookmarks are document coordinates too; a wholesale replacement makes
        // them point at unrelated text. Breakpoints are anchored the same way.
        self.bookmarks.clear();
        self.breakpoints.clear();
        self.invalidate_line_states();
        self.minimap_line_count = 0;
        // The word index describes text that no longer exists.
        self.completion_index = None;
        self.scale = DocumentScale::for_lines(self.line_count());
        // A level that forbids editing has to be reflected in the buffer the
        // widget already gates on, so every mutation entry point agrees without
        // repeating the check.
        self.clamp_cursor_to_document();
        self.refresh_derived_state();
        // A wholesale replacement is a committed change like any other, so it advances the revision.
        // `set_text` does not route through `record_and_settle_inner` (it writes the model directly
        // and clears caches), so the bump is repeated here rather than inherited — otherwise a host
        // that captured the revision before a `set_text` would see the document change under an
        // unchanged counter (BLUE26 B26.08).
        self.revision.set(self.revision.get().wrapping_add(1));
        self.text_changed.emit(next);
        self.text_change.emit(TextChange::document(self.line_count()));
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one line, preserving the existing document.
    pub fn append_line(&mut self, line: impl AsRef<str>) {
        let mut next = self.text();
        if !next.is_empty() {
            next.push('\n');
        }
        next.push_str(line.as_ref());
        self.set_text(next);
        let last = self.line_count().saturating_sub(1);
        self.move_caret_to(TextPosition::new(last, self.line_len(last)), false);
    }

    /// Returns the number of document lines (always >= 1).
    pub fn line_count(&self) -> usize {
        self.model.borrow().line_count()
    }

    /// Returns one line, or `None` when out of range.
    pub fn line_text(&self, line: usize) -> Option<String> {
        self.model.borrow().lines.get(line).cloned()
    }

    /// Borrows one line for the life of the returned guard, or an empty string when out of range.
    ///
    /// # Why a guard rather than `&str`
    ///
    /// The model lives behind a `RefCell`, so a returned `&str` could not outlive the borrow
    /// guard. The paint path needs the line across several helper calls, and cloning it once per
    /// visible line per frame was that cost this avoids. The guard is a **shared** borrow, so the
    /// helpers it calls may take `&self` and read the same model again; nothing on that path
    /// takes a mutable borrow.
    pub(crate) fn line_text_borrowed(&self, line: usize) -> core::cell::Ref<'_, str> {
        core::cell::Ref::map(self.model.borrow(), |model| {
            model.lines.get(line).map(String::as_str).unwrap_or("")
        })
    }

    /// Returns the line containing `offset` in the joined document.
    pub fn line_of_byte_offset(&self, offset: usize) -> usize {
        self.model.borrow().line_of_byte_offset(offset)
    }

    /// [`Self::line_of_byte_offset`] through the model's prefix index.
    ///
    /// The edit path already built that index while locating the splice, so the lookup is
    /// a binary search over it instead of a walk over every line above the edit — which was
    /// an `O(document)` scan on every keystroke (J-P03).
    fn line_of_byte_offset_cached(&self, offset: usize) -> usize {
        self.model.borrow_mut().line_of_byte_offset_fast(offset)
    }

    /// Returns the byte offset of `position` in the joined document text.
    ///
    /// The position is clamped, so the result is always a valid offset into [`Self::text`].
    /// This is the pair of [`Self::position_of`]: a host mapping a byte-range diagnostic or a
    /// source span onto the caret can convert one way and back without maintaining a second
    /// column calculation. The model's prefix index answers the line start in `O(log lines)`.
    pub fn offset_of(&self, position: TextPosition) -> usize {
        self.model.borrow_mut().total_byte_offset(position)
    }

    /// Returns the [`TextPosition`] containing byte `offset` in the joined document text.
    ///
    /// The offset is clamped to the document, so an offset past the end reports the last
    /// position. This is the inverse of [`Self::offset_of`]; the line is located through the
    /// prefix index in `O(log lines)`, so the lookup does not grow with the document.
    pub fn position_of(&self, offset: usize) -> TextPosition {
        self.model.borrow_mut().position_of_byte_offset(offset)
    }

    /// Returns the character length of a line, or 0 when out of range.
    pub fn line_len(&self, line: usize) -> usize {
        self.model.borrow().line_len(line)
    }

    // ── Editing primitives ──────────────────────────────────────────────────

    /// Inserts text at the caret, replacing any selection.
    pub fn insert(&mut self, text: &str) {
        if !self.is_editable() || text.is_empty() {
            return;
        }
        let payload = self.expand_tabs(text);
        if self.has_multiple_cursors() {
            let carets = self.carets_for_edit();
            let edits = self.insertion_edits(&carets, &payload);
            self.apply_caret_edits(&carets, edits);
            return;
        }
        self.splice_payload(&payload);
    }

    /// Inserts text while honouring the configured indent unit for tabs.
    ///
    /// Every leading tab on every inserted line is expanded to one indent unit,
    /// and any remaining tab is expanded to the tab width, which is what the
    /// method promises: a caller handing over indented source gets the configured
    /// indentation rather than literal `\t` bytes when `insert_spaces` is on.
    pub fn insert_with_indent(&mut self, text: &str) {
        if !self.is_editable() || text.is_empty() {
            return;
        }
        let unit = self.indent_unit();
        let mut payload = String::with_capacity(text.len() + unit.len());
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                payload.push('\n');
            }
            let mut rest = line;
            while let Some(stripped) = rest.strip_prefix('\t') {
                payload.push_str(&unit);
                rest = stripped;
            }
            payload.push_str(rest);
        }
        self.insert(&payload);
    }

    pub(super) fn expand_tabs(&self, text: &str) -> String {
        if !self.config.insert_spaces || !text.contains('\t') {
            return text.to_string();
        }
        let unit = " ".repeat(self.config.tab_width);
        text.replace('\t', &unit)
    }

    /// Replaces the selection (or inserts) with `payload`, in one undo step.
    pub(super) fn splice_payload(&mut self, payload: &str) {
        let (start, end) = self.cursor.bounds();
        let newlines = payload.matches('\n').count();
        let head = if newlines == 0 {
            TextPosition::new(start.line, start.column + payload.chars().count())
        } else {
            TextPosition::new(start.line + newlines, trailing_line_len(payload))
        };
        self.splice_and_record(start, end, payload, head);
    }

    /// Splices the model and records the change as one undoable range edit.
    ///
    /// This is the code editor's only edit path: it derives the changed byte
    /// range from the same splice that applies it, so the history entry and the
    /// document cannot disagree. The alternative — snapshotting the whole
    /// document before and after — costs two full copies per keystroke.
    pub(super) fn splice_and_record(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        payload: &str,
        head: TextPosition,
    ) {
        // The single choke point for every range edit, so the locked-span guard
        // lives here: a splice that overlaps a generated region is refused before
        // anything is recorded or written. `is_editable` is already checked by the
        // public entry points, but they cannot see the clamped positions this path
        // derives, and a caller that passed through `splice_range` unchecked would
        // otherwise slip a generated-line edit past the lock.
        if !self.is_range_editable(start, end) {
            return;
        }
        // Capture the text being replaced **before** the splice runs. Slicing the
        // post-edit buffer with a pre-edit offset would record whatever now sits
        // there, so undo would restore the edit instead of the original — which is
        // exactly the defect this path had.
        let removed = {
            let mut model = self.model.borrow_mut();
            let start = model.clamp_position(start);
            let end = model.clamp_position(end);
            let from = model.total_byte_offset(start);
            let to = model.total_byte_offset(end).max(from);
            let text = model.text.borrow();
            text.get(from..to.min(text.len())).unwrap_or_default().to_string()
        };
        let (rebuilt, start_offset, removed_len) =
            self.model.borrow_mut().splice_with_range(start, end, payload);
        let checkpoint = Checkpoint {
            range: Some((start_offset, removed_len, payload.to_string())),
            removed: Some(removed),
            before: None,
        };
        self.record_and_settle_inner(rebuilt, checkpoint, head);
    }

    /// Applies an edit that has already been computed.
    ///
    /// Callers either spliced the model themselves (`splice_payload`,
    /// `splice_range`) or rebuilt the text by hand (line commands); this writes
    /// the result back so both paths agree, then records one undo checkpoint.
    pub(super) fn commit_edit(&mut self, before: String, after: String, head: TextPosition) {
        // A line transform that changes nothing must not mark the document dirty
        // or fire `text_changed`: the text is byte-for-byte what it already was,
        // so there is no edit to commit or undo.
        if before == after {
            return;
        }
        // A caller that rebuilt the text by hand has no offset to report, so the
        // range is recovered by diffing the two texts. The replaced text comes from
        // `before` directly: the range is in pre-edit coordinates, so slicing the
        // post-edit buffer with it would record the wrong bytes.
        let range = diff_range(&before, &after);
        // Line commands bypass `splice_and_record`, so the locked-span guard has to
        // be applied to the *diff* of the two texts: the byte span the command
        // rewrote is exactly what its positions would be checked against. Without
        // this, `delete_line` on a generated line would succeed.
        if let Some((start, removed_len, inserted)) = &range {
            // The touched region is `[start, start + removed_len + inserted.len())`
            // in the pre-edit text; the tails of that region are what the guard
            // needs as positions.
            let guard_start = position_in_text(&before, *start);
            let guard_end = position_in_text(&before, start + removed_len + inserted.len());
            // `start + removed_len + inserted.len()` can exceed the pre-edit
            // length, so `guard_end` is clamped to the document by the helper.
            if !self.is_range_editable(guard_start, guard_end) {
                return;
            }
        }
        let removed = range.as_ref().map(|(start, removed_len, _)| {
            before.get(*start..start + removed_len).unwrap_or_default().to_string()
        });
        let checkpoint = Checkpoint { range, removed, before: Some(before) };
        self.record_and_settle_inner(Some(after), checkpoint, head);
    }

    /// Writes the spliced text back, records one checkpoint and refreshes state.
    ///
    /// `after` is the rebuilt document when the splice had to rebuild it, or
    /// `None` when the model already applied the edit in place. `checkpoint`
    /// carries whatever the caller already knows about the edit: the byte range it
    /// replaced, the text that was there, and the pre-edit document. All three are
    /// optional because the edit paths know different amounts — typing knows the
    /// range, a line command knows only the before/after pair. The most specific
    /// description available is used, and anything missing is reconstructed from
    /// the buffer before it is overwritten.
    pub(super) fn record_and_settle_inner(
        &mut self,
        after: Option<String>,
        checkpoint: Checkpoint,
        head: TextPosition,
    ) {
        // An in-place edit already left the model correct, so there is nothing to
        // write back. A rebuilt edit arrives as `Some(text)` and is written below.
        //
        // The document is deliberately **not** materialised here: the previous form
        // cloned the whole buffer whenever `after` was `None` (which is every typed
        // keystroke, in-place or not) only to hand it to `text_changed`. The change is
        // now described by [`TextChange`] instead, so the per-keystroke cost is the
        // size of the edit rather than the size of the file.
        let (after_text, already_applied) = match after {
            Some(text) => {
                let same = {
                    let model = self.model.borrow();
                    let current = model.text.borrow();
                    current.as_str() == text.as_str()
                };
                (Some(text), same)
            }
            None => (None, true),
        };
        // Everything below that reads the "before" state has to happen before the
        // write. The recorded range is in pre-edit coordinates, so slicing the
        // post-edit buffer with it would capture whatever now sits there — for
        // `a-b-c` → `a+b+c` that turned the undo entry into the edited text, and
        // undoing restored the edit instead of the original.
        let removed = match checkpoint.removed {
            Some(text) => Some(text),
            None => match &checkpoint.range {
                Some((start, removed_len, _)) => {
                    let model = self.model.borrow();
                    let text = model.text.borrow();
                    let end = start.saturating_add(*removed_len).min(text.len());
                    Some(
                        if *start <= end
                            && text.is_char_boundary(*start)
                            && text.is_char_boundary(end)
                        {
                            text[*start..end].to_string()
                        } else {
                            String::new()
                        },
                    )
                }
                None => None,
            },
        };
        let before_snapshot = if checkpoint.before.is_some() {
            checkpoint.before
        } else if already_applied {
            None
        } else {
            Some(self.model.borrow().text.borrow().clone())
        };
        if !already_applied {
            if let Some(text) = &after_text {
                self.model.borrow_mut().set_text(text.clone());
            }
        }
        // The match cache can be updated from the edit's line range instead of a
        // full rescan. `dirty` is `(first_line, lines_now, lines_before)`: the
        // lines the edit replaced at that position. It is `None` when the caller
        // could not describe the change, which falls back to a full rescan.
        let dirty = match (&checkpoint.range, &removed) {
            (Some((start, _removed_len, inserted)), Some(removed_text)) => {
                let first = self.line_of_byte_offset_cached(*start);
                let lines_now = inserted.matches('\n').count() + 1;
                let lines_before = removed_text.matches('\n').count() + 1;
                Some((first, lines_now, lines_before))
            }
            _ => None,
        };
        // A locked range that sits *below* the edit has moved by the net line
        // count. Applying the shift here — where the edit's line range is already
        // derived for the match cache — keeps the lock statements and the match
        // cache from disagreeing about where the edit landed.
        if let Some((first, lines_now, lines_before)) = dirty {
            self.shift_read_only_spans_for_edit(first, lines_now, lines_before);
        }
        // Whether the document actually changed, decided while the pre-edit snapshot is still
        // owned here. Used both to skip a pointless dirty flag and to keep the revision honest
        // (BLUE26 B26.08): a splice that rewrites a line with identical bytes is legitimate but is
        // not a change, and reporting one would make a host invalidate cached work for nothing.
        let changed = match (&before_snapshot, &after_text) {
            (Some(before), Some(after)) => before != after,
            _ => true,
        };
        // The description is built before `push_checkpoint` consumes the range and the
        // removed text; it borrows both, so nothing is copied except the inserted payload
        // (which is the size of the edit, not of the document).
        let change = match (&dirty, &checkpoint.range) {
            (Some((first, lines_now, lines_before)), Some((_, _, text))) => {
                TextChange::lines(*first, *lines_before, *lines_now, text.as_str())
            }
            _ => TextChange::document(self.line_count()),
        };
        // Keep the completion word index in step with the document instead of rebuilding
        // it from the whole text on the next completion trigger (J-P11). The update is the
        // size of the edit plus the word around it; a caller that could not describe the
        // change drops the index and the next trigger builds it from the live text.
        let next_revision =
            if changed { self.revision.get().wrapping_add(1) } else { self.revision.get() };
        match (&checkpoint.range, &removed) {
            (Some((start, _, inserted)), Some(removed_text)) => {
                let model = self.model.borrow();
                let text = model.text.borrow();
                if let Some(index) = self.completion_index.as_mut() {
                    index.apply_edit(next_revision, &text, *start, removed_text, inserted);
                }
            }
            _ => self.completion_index = None,
        }
        if !self.restoring_history {
            self.push_checkpoint(checkpoint.range, removed, before_snapshot);
        }
        self.cursor = Cursor { head: self.clamp_position(head), anchor: self.clamp_position(head) };
        self.goal.active = false;
        // Re-lex only from the first line the edit could have changed. The
        // caret lands at or after the edited region in every caller, so taking
        // the minimum of the two ends is a safe lower bound: re-lexing a line or
        // two too many is harmless, missing one would leave stale colours.
        self.invalidate_line_states_from(self.cursor.head.line.min(head.line));
        // A keystroke inside the document is a modification by definition, so the
        // flag is set directly rather than re-derived by comparing the whole
        // document against its save point.
        self.model.borrow_mut().dirty = true;
        // A one-line edit cannot change any other line's bucket contribution
        // unless the longest line in its bucket was the one edited, so re-folding
        // the edited line is enough. A line count change invalidates the whole
        // bucket map and is caught by `prepare_minimap` on the next frame.
        self.patch_minimap_line(self.cursor.head.line.min(head.line));
        self.refresh_derived_state_after(dirty);
        // Every committed edit bumps the revision exactly once, here at the choke point rather than
        // at each entry point — the entry points that can decline an edit (a locked span, a no-op
        // replace) return before reaching here, so the counter tracks *accepted* changes only
        // (BLUE26 B26.08).
        if changed {
            self.revision.set(self.revision.get().wrapping_add(1));
        }
        // A whole-document payload is emitted only on the paths that already own one
        // (a hand-rebuilt text). Every other change is described by `text_change`.
        if let Some(text) = after_text {
            self.text_changed.emit(text);
        }
        self.text_change.emit(change);
        self.emit_cursor_and_selection();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Pushes one undo checkpoint for an edit.
    ///
    /// A range edit stores only the text it replaced; anything else falls back to
    /// `ValueCommand<String>`, which stores the whole document twice. Typing always
    /// takes the range path, so a keystroke's history cost is the size of the
    /// edit rather than the size of the file.
    pub(super) fn push_checkpoint(
        &mut self,
        range: Option<(usize, usize, String)>,
        removed: Option<String>,
        before_snapshot: Option<String>,
    ) {
        let target = self.model.borrow().text.clone();
        // A pure, single-line insertion can extend the run already on the stack.
        // `removed` is `Some("")` for an insertion, and `start` must be exactly
        // where the run ended; anything else (a deletion, a newline, a moved
        // caret) closes the run and starts a new checkpoint.
        if let (Some((start, 0, inserted)), Some(removed_text)) = (&range, &removed) {
            if removed_text.is_empty() && !inserted.contains('\n') {
                if let Some(run) = &self.active_typing {
                    let mut accum = run.borrow_mut();
                    if accum.start + accum.inserted.len() == *start {
                        accum.inserted.push_str(inserted);
                        self.last_checkpoint_bytes = 0;
                        return;
                    }
                }
                let command = TypingCommand::new(target, *start, inserted.clone());
                self.active_typing = Some(command.accum());
                self.undo_stack.borrow_mut().push(Box::new(command));
                self.last_checkpoint_bytes = inserted.len();
                return;
            }
        }
        self.active_typing = None;
        self.last_checkpoint_bytes = match (range, removed) {
            (Some((start, _removed_len, inserted)), Some(removed)) => {
                let bytes = removed.len() + inserted.len();
                self.undo_stack.borrow_mut().push(Box::new(TextRangeCommand::new(
                    target,
                    start,
                    removed,
                    inserted,
                    "code_editor_edit",
                )));
                bytes
            }
            // No usable range (the caller could not describe the change, or the
            // recorded offsets do not fit the pre-edit text): fall back to the
            // whole-document snapshot, which is correct, if costly.
            (_, _) => {
                let current = target.borrow().clone();
                let before = before_snapshot.unwrap_or_else(|| current.clone());
                let bytes = before.len() + current.len();
                self.undo_stack.borrow_mut().push(Box::new(ValueCommand::new(
                    target,
                    before,
                    current,
                    "Edit text",
                    "code_editor_edit",
                )));
                bytes
            }
        };
    }

    /// Drops cached lexer state for every line from `first` down.
    ///
    /// The token cache is dropped outright because the spans themselves may
    /// have changed. The entry state of `first` is **kept**: an edit below a
    /// line cannot change what that line starts inside, so resetting it to
    /// `Clean` would make the lexer read a block-comment continuation as fresh
    /// code. `ensure_line_states` re-derives forward from there and stops as
    /// soon as a line leaves the same state it already had.
    pub(crate) fn invalidate_line_states_from(&mut self, first: usize) {
        for slot in self.line_tokens.iter_mut().skip(first) {
            *slot = None;
        }
        self.walked_lines = self.walked_lines.min(first);
    }

    /// Applies an edit that replaces `[start, end)` without moving the caret
    /// through the payload (used by backward deletions and range operations).
    pub(crate) fn splice_range(
        &mut self,
        start: TextPosition,
        end: TextPosition,
        payload: &str,
        head: TextPosition,
    ) {
        if !self.is_editable() {
            return;
        }
        self.splice_and_record(start, end, payload, head);
    }
}
