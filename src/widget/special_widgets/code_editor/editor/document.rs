// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Document text and edit application.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

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
        self.revision
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
        if expected != self.revision {
            return Err(EditRejection::StaleRevision { expected, actual: self.revision });
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
            return Ok(self.revision);
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
        Ok(self.revision)
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
            self.undo_stack.push(Box::new(ValueCommand::new(
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
        self.revision = self.revision.wrapping_add(1);
        self.text_changed.emit(next);
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
        // write back. The document is materialised once, for the `text_changed`
        // signal: subscribers receive the new text, and that contract is what makes
        // the signal useful. The comparison against the buffer is only meaningful
        // for the rebuilt path — an in-place edit is applied by definition.
        let (after, already_applied) = match after {
            Some(text) => {
                let same = {
                    let model = self.model.borrow();
                    let current = model.text.borrow();
                    current.as_str() == text.as_str()
                };
                (text, same)
            }
            None => (self.model.borrow().text.borrow().clone(), true),
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
            self.model.borrow_mut().set_text(after.clone());
        }
        // The match cache can be updated from the edit's line range instead of a
        // full rescan. `dirty` is `(first_line, lines_now, lines_before)`: the
        // lines the edit replaced at that position. It is `None` when the caller
        // could not describe the change, which falls back to a full rescan.
        let dirty = match (&checkpoint.range, &removed) {
            (Some((start, _removed_len, inserted)), Some(removed_text)) => {
                let first = self.line_of_byte_offset(*start);
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
        let changed = match &before_snapshot {
            Some(before) => before != &after,
            None => true,
        };
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
            self.revision = self.revision.wrapping_add(1);
        }
        self.text_changed.emit(after);
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
                self.undo_stack.push(Box::new(command));
                self.last_checkpoint_bytes = inserted.len();
                return;
            }
        }
        self.active_typing = None;
        self.last_checkpoint_bytes = match (range, removed) {
            (Some((start, _removed_len, inserted)), Some(removed)) => {
                let bytes = removed.len() + inserted.len();
                self.undo_stack.push(Box::new(TextRangeCommand::new(
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
                self.undo_stack.push(Box::new(ValueCommand::new(
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
