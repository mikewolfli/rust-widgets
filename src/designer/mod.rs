// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Mode 2: the designer's **generator** — a project document becomes Rust source (BLUE19 T-23).
//!
//! # The two modes this crate now has (D7)
//!
//! | Mode | Where | Cost | Gives |
//! |---|---|---|---|
//! | 1 | `crate::json` — interpret the document at run time | Links a parser, the name table, a layout kind table | Edit UI without recompiling |
//! | 2 | this module — generate Rust, compile it | A compile per UI change | No runtime interpreter, and it works on `mini`/`embedded`, where mode 1 does not compile at all |
//!
//! Both exist because they answer different questions. The design loop wants the first; shipping
//! wants the second. `ViewEngine::mount`'s `create` callback is the seam that makes them share one
//! runtime (BLUE19 §5.1.4) — this module generates a `View` and a `create`, mode 1 passes a
//! JSON-backed `create`, and nothing else in the program changes.
//!
//! # Where this module lives, and why it is gated
//!
//! The generator **parses** a project, so it needs `serde_json` and the parsed-project type, both of
//! which live behind `full_widgets`. Gating this module the same way is therefore not a choice about
//! importance — it is the module's actual dependency. The consequence, stated plainly:
//!
//! > **A `mini`/`embedded` device cannot run the generator. It is the *target* of one.**
//!
//! That is the honest division and it matches how the feature is used: a designer runs on a desktop
//! host, and `mini`/`embedded` receive the generated file. BLUE19 §5.3.4 says the same thing in
//! one line — "`mini`/`embedded` are generation **targets**, not runtime **environments**".
//!
//! What is *not* assumed is the ability to construct a target's controls:
//! `generator::availability` **asks the factory** instead of consulting a list, and reports a
//! name it cannot resolve rather than emitting code that would fail to compile on the target.
//!
//! # Reachability
//!
//! **State:** Production callers: `tests/generator_output_compiles_test.rs:44`,
//! `tests/mode_consistency_test.rs:128`. Those two are the *delivered* consumers of this round: the
//! first compiles a generated program for real on five profiles, the second asserts the two modes
//! describe the same UI. Beyond them the generator's caller is the designer, which does not exist in
//! this repository yet — so this module is **an enabling API rather than a used one**, and saying so
//! is more honest than claiming a design tool that is not here.
//!
//! That is the same position `crate::json` was in before this round (`src/json/mod.rs` Reachability),
//! with one difference worth stating: the generator's output is **verified by compilation**, so
//! "unused" here does not mean "unchecked". `tools/check_generator_output_compiles.sh` builds what
//! this module emits for `desktop`, `tablet`, `mobile`, `mini` and `embedded`, and
//! `tools/check_mode_consistency.sh` fails if the emitted tree drifts from mode 1's.
//!
//! # Where generated files live (D7-b-3)
//!
//! Generated sources are **committed** and written through `artifact::regenerate_into`;
//! `tools/check_generated_sources.sh` is the regenerate-and-compare gate that makes committing
//! safe. See `artifact` for the decision and its reasoning.
//!
//! # Scope
//!
//! `generator::generate` is a pure function: document text in, source text out, no filesystem and no
//! clock. That is what lets `tests/generator_output_compiles_test.rs` compile its output for real and
//! what makes two runs over one document byte-identical. Writing files is a separate concern with its
//! own failure modes, so it lives in `artifact` and the core stays pure.

pub mod artifact;
pub mod generator;

pub use artifact::{
    artifact_source, is_generated, plan_artifacts, regenerate_into, Artifact, ArtifactOutcome,
    ArtifactPaths, GENERATED_MARKER,
};
pub use generator::{
    availability, constructor_type_name, generate, is_style_only_property, is_wire_key,
    shared_wire_rule_count, Availability, GeneratedSource, GenerationGap, GenerationGapReason,
    GenerationReport, GenerationRequest, TargetProfile, DEFAULT_CHILD_CAPACITY,
    MINI_CHILD_CAPACITY,
};

// ── Undo integration (DSG-010) ───────────────────────────────

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

#[cfg(test)]
mod undo_tests {
    use super::*;
    use crate::compat::{Rc, RefCell, String};

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
}

#[cfg(all(test, declarative_view))]
mod wire_prefix_agreement_tests {
    /// The wire carrier's prefix must be the same string in `designer` and in `view`.
    ///
    /// # Why this is only built with the declarative layer
    ///
    /// The assertion exists to keep two spellings of one prefix in step, and one of
    /// them belongs to `crate::view`. On a build that opted the layer away
    /// (`desktop,no-declarative-view`) there is nothing to compare against, so the test
    /// is gated on `declarative_view` rather than being compiled into a failing `use`.
    /// That is the same gate the module it names is under (rule #53: a capability gap is
    /// a runtime/build fact expressed once, not two parallel test bodies).
    ///
    /// # Why there are two spellings, and why this test is what keeps them together
    ///
    /// `crate::view` recognises the carrier so it does not write it as a property, but `crate::view`
    /// compiles on profiles where `crate::designer` does not (the generator is `designer_tooling`-gated
    /// because it needs `serde_json`). Reading the constant across that boundary broke the `tablet`
    /// and `mobile` builds, so `view` owns a literal of its own.
    ///
    /// Duplication is only safe with an assertion that the two agree, which is this test. Without it
    /// a change to one spelling would silently stop the engine from stripping the carrier, and the
    /// symptom would be the one this prefix exists to remove: a `PropertyRefused` for every wired
    /// node in a generated program.
    #[test]
    fn the_wire_prefix_is_the_same_string_in_view_and_designer() {
        assert_eq!(
            super::generator::WIRE_PROP_PREFIX,
            crate::view::GENERATED_WIRE_PROP_PREFIX,
            "`designer::generator::WIRE_PROP_PREFIX` and `view::GENERATED_WIRE_PROP_PREFIX` must \
             be the same string: the generator writes the carrier and the view layer strips it, so a \
             drift in either direction means one of the two stops recognising it"
        );
        assert!(
            crate::view::is_generated_wire_prop(super::generator::WIRE_PROP_PREFIX),
            "the predicate `view` uses must recognise the prefix `designer` writes"
        );
    }
}
