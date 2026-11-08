// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The designer's document history: one undo/redo command plus the stack wrapper that holds it
//! (DSG-010).
//!
//! # Reachability
//!
//! **State:** Production caller: `tools/designer_generate.rs`, which opens the project document
//! through [`DesignerSession`] and generates from the session's text. The session is the
//! document-plus-history pair a designer shell drives; the CLI is the in-repo shell that reaches
//! it today, so the undo API is constructed by real code rather than only by tests. Removal
//! condition: when the CLI stops holding its document in a session, or when no caller wants undo
//! integration. BD-04 asked for a reachable path instead of an unlabelled public surface.
//!
//! # Why it wraps the existing undo stack
//!
//! `src/undo/` already owns capacity pruning, merge policy, save-point tracking and the
//! failed-undo restore rule. A designer-specific stack would be a second implementation of those
//! rules and would drift. This module only adapts a document edit into an
//! [`UndoCommand`](crate::undo::UndoCommand) and forwards the operations the designer exposes.

// ── Undo integration (DSG-010) ───────────────────────────────

/// Environment variable naming the designer session's autosave file (EX4-02).
///
/// Unset or empty means "autosave off"; the same shape the i18n hot-reload watcher uses for its
/// directory variable, so one convention covers both optional filesystem subsystems.
pub const AUTOSAVE_ENV_VAR: &str = "RUST_WIDGETS_DESIGNER_AUTOSAVE";

/// One designer document mutation, wrapped as an undo/redo command.
///
/// # Why the document is a shared cell
///
/// The designer edits a document the same way a text control edits its buffer: a command records a
/// before/after pair and replays it in both directions. Keeping the document in an `Rc<RefCell<_>>`
/// lets the command own a handle to it without making the history own the document (a history that
/// outlives one document would otherwise keep it alive). The command assumes the edit has **already
/// been applied** when it is recorded — the same contract [`TextSpliceCommand`] uses, so the stack
/// does not execute a change a second time.
///
/// [`TextSpliceCommand`]: crate::undo::TextSpliceCommand
pub struct DocumentEditCommand {
    id: crate::undo::CommandId,
    document: crate::compat::Rc<crate::compat::RefCell<crate::compat::String>>,
    before: crate::compat::String,
    after: crate::compat::String,
    description: &'static str,
}

static NEXT_DOCUMENT_EDIT_ID: crate::compat::atomic::AtomicU64 =
    crate::compat::atomic::AtomicU64::new(1);

impl DocumentEditCommand {
    /// Records a mutation from `before` to `after` over `document`.
    pub fn new(
        document: crate::compat::Rc<crate::compat::RefCell<crate::compat::String>>,
        before: impl Into<crate::compat::String>,
        after: impl Into<crate::compat::String>,
        description: &'static str,
    ) -> Self {
        Self {
            id: crate::undo::CommandId(
                NEXT_DOCUMENT_EDIT_ID.fetch_add(1, crate::compat::atomic::Ordering::Relaxed),
            ),
            document,
            before: before.into(),
            after: after.into(),
            description,
        }
    }

    /// The document this command writes to.
    pub fn document(&self) -> &crate::compat::Rc<crate::compat::RefCell<crate::compat::String>> {
        &self.document
    }
}

impl crate::undo::UndoCommand for DocumentEditCommand {
    fn id(&self) -> crate::undo::CommandId {
        self.id
    }

    fn description(&self) -> crate::undo::CommandDescription {
        crate::undo::CommandDescription {
            text: crate::compat::String::from(self.description),
            timestamp_ms: 0,
            command_type: "designer_document_edit",
        }
    }

    fn execute(&mut self) -> Result<(), crate::compat::String> {
        *self.document.borrow_mut() = self.after.clone();
        Ok(())
    }

    fn undo(&mut self) -> Result<(), crate::compat::String> {
        *self.document.borrow_mut() = self.before.clone();
        Ok(())
    }
}

/// The designer's undo/redo history, backed by the crate's existing [`UndoStack`](crate::undo::UndoStack).
///
/// # Why this wraps rather than reimplements
///
/// `src/undo/` already owns capacity pruning, merge policy, save-point tracking and the
/// failed-undo restore rule. A designer-specific stack would be a second implementation of those
/// rules and would drift. This wrapper only adapts a document edit into an [`UndoCommand`] and
/// forwards the operations the designer exposes.
pub struct DesignerHistory {
    stack: crate::undo::UndoStack,
}

impl DesignerHistory {
    /// Creates an empty history with the undo stack's default capacity.
    pub fn new() -> Self {
        Self { stack: crate::undo::UndoStack::new() }
    }

    /// Records an already-applied document edit on the undo stack.
    pub fn record(
        &mut self,
        document: &crate::compat::Rc<crate::compat::RefCell<crate::compat::String>>,
        before: &str,
        after: &str,
        description: &'static str,
    ) {
        self.stack.push(crate::compat::Box::new(DocumentEditCommand::new(
            crate::compat::Rc::clone(document),
            before,
            after,
            description,
        )));
    }

    /// Reverses the most recent edit.
    pub fn undo(&mut self) -> Result<(), crate::compat::String> {
        self.stack.undo()
    }

    /// Re-applies the most recently undone edit.
    pub fn redo(&mut self) -> Result<(), crate::compat::String> {
        self.stack.redo()
    }

    /// Whether there is an edit to reverse.
    pub fn can_undo(&self) -> bool {
        self.stack.can_undo()
    }

    /// Whether there is an undone edit to re-apply.
    pub fn can_redo(&self) -> bool {
        self.stack.can_redo()
    }

    /// The undo stack itself, for a host that needs its save-point or description API.
    pub fn stack(&self) -> &crate::undo::UndoStack {
        &self.stack
    }
}

impl Default for DesignerHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// An editing session: one document plus the undo history over it.
///
/// # What this adds over [`DesignerHistory`]
///
/// [`DesignerHistory`] records an edit over a document the caller owns. A session owns both, so a
/// caller that loads, edits and undoes a document does not have to keep the `Rc<RefCell<_>>` and
/// the history in step itself — the two cannot point at different documents. That is the shape the
/// generator CLI uses: it opens the project through the session and reads [`Self::text`] to
/// generate, so the document it generates from is the one the history edits.
///
/// The session deliberately keeps the same edit contract as [`DesignerHistory::record`]: an edit is
/// applied **before** it is recorded, which is why [`Self::apply`] writes the document and then
/// records the before/after pair.
pub struct DesignerSession {
    document: crate::compat::Rc<crate::compat::RefCell<crate::compat::String>>,
    history: DesignerHistory,
}

impl DesignerSession {
    /// Opens an empty session.
    pub fn new() -> Self {
        Self::open(crate::compat::String::new())
    }

    /// Opens a session over `document`.
    pub fn open(document: impl Into<crate::compat::String>) -> Self {
        Self {
            document: crate::compat::Rc::new(crate::compat::RefCell::new(document.into())),
            history: DesignerHistory::new(),
        }
    }

    /// The session's current document text.
    pub fn text(&self) -> crate::compat::String {
        self.document.borrow().clone()
    }

    /// Writes the session's document to `path`, creating parent directories as needed (EX4-02).
    ///
    /// Returns the filesystem error as text; a designer shell shows it rather than pretending the
    /// save happened. The write goes through `std::fs` so the location is the caller's, not an
    /// ambient working directory.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), crate::compat::String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| {
                    crate::compat::format!("could not create {}: {error}", parent.display())
                })?;
            }
        }
        std::fs::write(path, self.text())
            .map_err(|error| crate::compat::format!("could not write {}: {error}", path.display()))
    }

    /// Opens a session over the document at `path` (EX4-02).
    ///
    /// The loaded text becomes the session's base state; the undo history starts empty because no
    /// edit has been made in this session yet.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self, crate::compat::String> {
        let path = path.as_ref();
        let document = std::fs::read_to_string(path).map_err(|error| {
            crate::compat::format!("could not read {}: {error}", path.display())
        })?;
        Ok(Self::open(document))
    }

    /// The autosave destination from `RUST_WIDGETS_DESIGNER_AUTOSAVE`, when set.
    ///
    /// The gating shape is the one the i18n hot-reload watcher uses: one environment variable whose
    /// absence means “off”, read at the point of use rather than stored in the session. An empty
    /// value is also “off”, so an env file that exports the name without a path cannot make every
    /// edit write to the current directory.
    pub fn autosave_path_from_env() -> Option<std::path::PathBuf> {
        let value = std::env::var(AUTOSAVE_ENV_VAR).ok()?;
        if value.is_empty() {
            return None;
        }
        Some(std::path::PathBuf::from(value))
    }

    /// Saves to the configured autosave path, reporting whether a write was attempted (EX4-02).
    ///
    /// `Ok(false)` means autosave is not configured. A configured but unwritable path returns the
    /// error; [`Self::apply`] logs it and keeps the edit, because losing the user's edit to a full
    /// disk would be worse than a stale autosave.
    pub fn save_if_configured(&self) -> Result<bool, crate::compat::String> {
        match Self::autosave_path_from_env() {
            Some(path) => {
                self.save(path)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// The shared document cell, for a caller that needs to observe it in place.
    pub fn document(&self) -> &crate::compat::Rc<crate::compat::RefCell<crate::compat::String>> {
        &self.document
    }

    /// The undo history over the session's document.
    pub fn history(&self) -> &DesignerHistory {
        &self.history
    }

    /// Replaces the document with `after`, recording the transition as an undoable edit.
    ///
    /// A no-op edit (the text is unchanged) is not recorded: an undo step that reverses nothing
    /// would make the first Ctrl+Z appear to do nothing.
    pub fn apply(&mut self, after: impl Into<crate::compat::String>, description: &'static str) {
        let after = after.into();
        let before = self.document.borrow().clone();
        if before == after {
            return;
        }
        *self.document.borrow_mut() = after.clone();
        self.history.record(&self.document, &before, &after, description);
        self.autosave_after(description);
    }

    /// Reverses the most recent edit.
    ///
    /// The autosave file follows the document in both directions, so an undo that is not written
    /// back would leave the disk holding an edit the session has already reversed.
    pub fn undo(&mut self) -> Result<(), crate::compat::String> {
        self.history.undo()?;
        self.autosave_after("undo");
        Ok(())
    }

    /// Re-applies the most recently undone edit.
    pub fn redo(&mut self) -> Result<(), crate::compat::String> {
        self.history.redo()?;
        self.autosave_after("redo");
        Ok(())
    }

    /// Writes the opt-in autosave file, warning rather than failing the edit.
    ///
    /// With the environment variable unset this is one lookup and no I/O. A configured but
    /// unwritable path is logged because the edit is still the user's intent; discarding it over a
    /// stale autosave would be worse.
    fn autosave_after(&self, description: &str) {
        if let Err(error) = self.save_if_configured() {
            log::warn!("[designer] autosave failed after {description}: {error}");
        }
    }

    /// Whether there is an edit to reverse.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Whether there is an undone edit to re-apply.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
}

impl Default for DesignerSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod undo_tests {
    use super::*;
    use crate::compat::{Rc, RefCell, String};

    /// EX4-02: a session round-trips through a real file, and a loaded document is a clean base.
    #[test]
    fn a_session_round_trips_through_a_file() {
        let mut path = std::env::temp_dir();
        path.push(crate::compat::format!(
            "rust-widgets-designer-{}-{}.json",
            std::process::id(),
            line!()
        ));
        let session = DesignerSession::open("{\"a\":2}");
        session.save(&path).expect("the temporary file is writable");

        let reopened = DesignerSession::open_path(&path).expect("the saved file reopens");
        assert_eq!(reopened.text(), "{\"a\":2}");
        assert!(!reopened.can_undo(), "a freshly loaded document is the base state, not an edit");
        let _ = std::fs::remove_file(&path);
    }

    /// DSG-010: a designer edit enters the existing undo stack and replays both directions.
    #[test]
    fn a_designer_edit_is_undone_and_redone() {
        let document = Rc::new(RefCell::new(String::from("{\"text\":\"before\"}")));
        let before = document.borrow().clone();
        *document.borrow_mut() = String::from("{\"text\":\"after\"}");
        let after = document.borrow().clone();

        let mut history = DesignerHistory::new();
        history.record(&document, &before, &after, "Set text");
        assert!(history.can_undo());
        assert!(!history.can_redo());

        history.undo().expect("the edit is undoable");
        assert_eq!(*document.borrow(), before);
        assert!(history.can_redo());

        history.redo().expect("the edit is redoable");
        assert_eq!(*document.borrow(), after);
    }

    /// A second document cannot be replayed by the first document's history by accident.
    #[test]
    fn a_history_is_bound_to_the_document_it_recorded() {
        let first = Rc::new(RefCell::new(String::from("first")));
        let second = Rc::new(RefCell::new(String::from("second")));
        let mut history = DesignerHistory::new();
        history.record(&first, "first", "first-edited", "Edit first");
        *first.borrow_mut() = String::from("first-edited");

        history.undo().expect("undo targets the recorded document");
        assert_eq!(*first.borrow(), "first");
        assert_eq!(*second.borrow(), "second", "the other document is untouched");
    }

    /// A session owns its document, so an applied edit is both visible through `text` and undoable.
    #[test]
    fn a_session_applies_and_undoes_its_own_document() {
        let mut session = DesignerSession::open("before");
        session.apply("after", "Set text");
        assert_eq!(session.text(), "after");
        assert!(session.can_undo());
        session.undo().expect("the edit is undoable");
        assert_eq!(session.text(), "before");
        session.redo().expect("the edit is redoable");
        assert_eq!(session.text(), "after");
    }

    /// An edit that changes nothing is not recorded, so the first undo is a real change.
    #[test]
    fn a_no_op_session_edit_is_not_recorded() {
        let mut session = DesignerSession::open("same");
        session.apply("same", "No change");
        assert!(!session.can_undo());
    }
}
