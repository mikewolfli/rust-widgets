// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cut, copy and paste.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Clipboard ───────────────────────────────────────────────────────────

    /// Tells the editor whether the clipboard currently holds text.
    ///
    /// A host that subscribes to the platform's clipboard-change notification calls
    /// this, after which opening the context menu never touches the OS clipboard.
    /// Without the call the editor probes once and caches the answer, so the menu
    /// is still correct on the first open — it just pays one platform read.
    pub fn set_clipboard_has_text(&mut self, has_text: bool) {
        self.clipboard_has_text = Some(has_text);
    }

    /// Forgets the cached clipboard state, so the next menu re-probes it.
    ///
    /// For a host that cannot be notified of clipboard changes: call this when the
    /// application loses and regains focus, which is the point at which another
    /// program may have replaced the clipboard.
    pub fn invalidate_clipboard_has_text(&mut self) {
        self.clipboard_has_text = None;
    }

    /// Returns whether the clipboard holds text, probing the platform once.
    ///
    /// A host-injected answer (or one this widget recorded itself) wins; only an
    /// unknown state reads the platform. The read is cached, because the answer
    /// cannot change without either this widget or the host observing it — the menu
    /// is the wrong place to discover it and the wrong place to pay for it.
    pub fn clipboard_has_text(&mut self) -> bool {
        if let Some(known) = self.clipboard_has_text {
            return known;
        }
        let probe = !crate::clipboard::ClipboardManager::text().is_empty();
        self.clipboard_has_text = Some(probe);
        probe
    }

    /// Copies the selection to the system clipboard.
    pub fn copy(&mut self) -> bool {
        let Some(selected) = self.selected_text() else { return false };
        if selected.is_empty() {
            return false;
        }
        crate::clipboard::ClipboardManager::set_text(selected);
        // We just wrote text, so the cached answer is known without reading it back.
        self.clipboard_has_text = Some(true);
        true
    }

    /// Cuts the selection to the system clipboard.
    pub fn cut(&mut self) -> bool {
        if !self.is_editable() || !self.copy() {
            return false;
        }
        self.delete_selection();
        true
    }

    /// Pastes the system clipboard at the caret.
    pub fn paste(&mut self) -> bool {
        if !self.is_editable() {
            return false;
        }
        let text = crate::clipboard::ClipboardManager::text();
        // Record what the read saw, so the menu does not have to read it again.
        self.clipboard_has_text = Some(!text.is_empty());
        if text.is_empty() {
            return false;
        }
        // Zed re-indents a pasted block: only the first line continues the caret's line,
        // the rest adopt the current line's indentation after their common prefix is
        // removed. Pasting into an indented block otherwise drops raw column-0 code into
        // the middle of a function.
        let text = self.reindent_pasted(&text);
        self.insert(&text);
        true
    }

    /// Re-indents a multi-line paste to the caret line's indentation.
    ///
    /// # Algorithm
    ///
    /// 1. The **common leading whitespace** of the pasted lines is removed from every
    ///    line but the first (the first continues the line the caret is on, so it keeps
    ///    its own text).
    /// 2. The caret line's indentation is prepended to every line but the first.
    /// 3. Blank lines stay empty rather than gaining trailing whitespace.
    ///
    /// This is the behaviour Zed and VS Code share; without it a block copied from a
    /// nested function is pasted at column 0.
    pub(super) fn reindent_pasted(&self, text: &str) -> String {
        if !text.contains('\n') {
            return text.to_string();
        }
        let indent = self.line_indent(self.cursor.head.line);
        let common = common_leading_whitespace(text);
        let mut out = String::with_capacity(text.len() + indent.len() * 4);
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                out.push('\n');
            }
            if index == 0 {
                out.push_str(line);
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            out.push_str(&indent);
            out.push_str(line.strip_prefix(common.as_str()).unwrap_or(line));
        }
        out
    }
}
