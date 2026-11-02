// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host-owned editor overlays and snippet-session data.
//!
//! These values are plain data like [`super::types`], but they describe what a
//! host pushes *into* the editor (annotations, lenses) or what the editor hands
//! back after expanding a template (a session). Keeping them in their own module
//! keeps `types` focused on the document/editor state and keeps both files within
//! the project's size ceiling.

use super::types::TextPosition;
use alloc::string::String;
use alloc::vec::Vec;

/// What an [`InlayHint`] annotates.
///
/// The kind is carried so a host can colour or filter hints without parsing the
/// label, and so a future painter can choose the inline name/type convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum InlayHintKind {
    /// A type annotation (`: i32`).
    Type,
    /// A parameter name (`name:`).
    Parameter,
    /// Anything else a host wants to show inline.
    #[default]
    Other,
}

/// A host-supplied annotation drawn inline at a document position.
///
/// The editor owns no language knowledge, so it cannot derive an inferred type
/// or a parameter name. This value is that seam: the host pushes hints and the
/// paint pass draws them without changing the document or the coordinate map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlayHint {
    /// Zero-based line the hint is anchored on.
    pub line: usize,
    /// Zero-based **character** column the hint follows.
    pub column: usize,
    /// The text drawn at the anchor.
    pub label: String,
    /// What the hint annotates.
    pub kind: InlayHintKind,
}

impl InlayHint {
    /// Creates a hint at `line`/`column` with an unspecified kind.
    pub fn new(line: usize, column: usize, label: impl Into<String>) -> Self {
        Self { line, column, label: label.into(), kind: InlayHintKind::Other }
    }

    /// Declares what the hint annotates.
    pub fn with_kind(mut self, kind: InlayHintKind) -> Self {
        self.kind = kind;
        self
    }
}

/// A host-supplied action offered above/after a line.
///
/// A code lens is deliberately just a label plus a command id: the widget cannot
/// know what a host's "2 references" action does, so it paints the affordance and
/// exposes the command for the host to dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeLens {
    /// Zero-based line the lens belongs to.
    pub line: usize,
    /// The text drawn as the affordance.
    pub label: String,
    /// The host command id that activating the lens should run.
    pub command: String,
}

impl CodeLens {
    /// Creates a lens on `line` with a label and a host command id.
    pub fn new(line: usize, label: impl Into<String>, command: impl Into<String>) -> Self {
        Self { line, label: label.into(), command: command.into() }
    }
}

/// One tab stop of a [`SnippetSession`].
///
/// `start == end` is a bare caret stop (`$1`); a non-empty range is a placeholder
/// (`${1:name}`) whose text the session selects so typing replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnippetStop {
    /// The marker number, `0..=9`. `0` is the final caret.
    pub index: u8,
    /// Inclusive start of the placeholder, or the caret position.
    pub start: TextPosition,
    /// Exclusive end of the placeholder (equal to `start` for a bare stop).
    pub end: TextPosition,
}

impl SnippetStop {
    /// Creates a bare caret stop.
    pub fn caret(index: u8, position: TextPosition) -> Self {
        Self { index, start: position, end: position }
    }

    /// Creates a placeholder stop selecting `[start, end)`.
    pub fn placeholder(index: u8, start: TextPosition, end: TextPosition) -> Self {
        Self { index, start, end }
    }

    /// Whether the stop selects placeholder text.
    pub fn is_placeholder(&self) -> bool {
        self.start != self.end
    }
}

/// An active snippet expansion: the stops and which one the caret is on.
///
/// The editor stores one of these while a snippet session is running. Editing
/// text translates the stops, so multiple placeholders keep their positions; a
/// structural change the translation cannot describe (an undo, a tab switch, a
/// whole-document replacement) ends the session instead.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SnippetSession {
    pub(crate) stops: Vec<SnippetStop>,
    pub(crate) current: usize,
}

impl SnippetSession {
    /// Creates a session over stops already sorted in navigation order.
    pub(crate) fn new(stops: Vec<SnippetStop>) -> Self {
        Self { stops, current: 0 }
    }

    /// Every stop, in navigation order (`$1`…`$9`, then `$0`).
    pub fn stops(&self) -> &[SnippetStop] {
        &self.stops
    }

    /// Index of the stop the caret is on.
    pub fn current(&self) -> usize {
        self.current
    }

    /// How many stops the snippet has.
    pub fn len(&self) -> usize {
        self.stops.len()
    }

    /// Whether the snippet has no stops at all.
    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// The stop the caret is on, if the index is in range.
    pub fn current_stop(&self) -> Option<SnippetStop> {
        self.stops.get(self.current).copied()
    }

    pub(crate) fn advance(&mut self) -> bool {
        if self.current + 1 >= self.stops.len() {
            return false;
        }
        self.current += 1;
        true
    }

    pub(crate) fn retreat(&mut self) -> bool {
        if self.current == 0 {
            return false;
        }
        self.current -= 1;
        true
    }
}
