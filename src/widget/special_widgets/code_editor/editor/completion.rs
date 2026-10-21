// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Completion popup and context menu.
//!
//! Split out of `editor.rs` (principle #9: one file, one responsibility).
//! The parent module owns the [`CodeEditor`](super::CodeEditor) type and its
//! fields; these methods are inherent, so moving them does not change the API.

use super::*;

impl CodeEditor {
    // ── Completion ──────────────────────────────────────────────────────────

    /// Returns the completion popup state.
    pub fn completion_state(&self) -> &CompletionState {
        &self.completion
    }

    /// Computes and shows completions for the prefix ending at the caret.
    ///
    /// Returns the number of candidates offered; 0 closes the popup.
    pub fn trigger_completion(&mut self) -> usize {
        let head = self.cursor.head;
        let prefix_start = self.prefix_start_column();
        let chars: Vec<char> = self.model.borrow().line(head.line).chars().collect();
        let prefix: String =
            chars[prefix_start.min(chars.len())..head.column.min(chars.len())].iter().collect();
        let text = self.text();
        let items = self.completion_source.completions(&text, &prefix, MAX_COMPLETIONS);
        if items.is_empty() {
            self.dismiss_completion();
            return 0;
        }
        self.completion.items = items;
        self.completion.selected = 0;
        self.completion.anchor_column = prefix_start;
        self.completion.visible = true;
        self.context_menu.visible = false;
        self.completion_changed.emit(true);
        self.base.request_layout();
        self.base.request_redraw();
        self.completion.items.len()
    }

    /// Hides the completion popup.
    pub fn dismiss_completion(&mut self) {
        if !self.completion.visible {
            return;
        }
        self.completion.visible = false;
        self.completion.items.clear();
        self.completion_changed.emit(false);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Moves the completion highlight by `delta` rows, wrapping around.
    pub fn move_completion_selection(&mut self, delta: isize) -> bool {
        if !self.completion.visible || self.completion.items.is_empty() {
            return false;
        }
        let count = self.completion.items.len();
        let current = self.completion.selected.min(count - 1) as isize;
        self.completion.selected = (current + delta).rem_euclid(count as isize) as usize;
        self.base.request_redraw();
        true
    }

    /// Accepts the highlighted completion, replacing the typed prefix.
    pub fn accept_completion(&mut self) -> bool {
        if !self.completion.visible {
            return false;
        }
        let Some(item) = self.completion.selected_item().cloned() else {
            self.dismiss_completion();
            return false;
        };
        let line = self.cursor.head.line;
        let start = TextPosition::new(line, self.completion.anchor_column);
        let end = TextPosition::new(line, self.cursor.head.column);
        self.completion.visible = false;
        self.completion.items.clear();
        let head = TextPosition::new(line, start.column + item.label.chars().count());
        self.splice_range(start, end, &item.label, head);
        self.completion_changed.emit(false);
        true
    }

    pub(crate) fn prefix_start_column(&self) -> usize {
        let head = self.cursor.head;
        let model = self.model.borrow();
        let chars: Vec<char> = model.line(head.line).chars().collect();
        let mut start = head.column.min(chars.len());
        while start > 0 {
            let ch = chars[start - 1];
            if self.config.language.is_word_char(ch) {
                start -= 1;
            } else {
                break;
            }
        }
        start
    }

    // ── Context menu ────────────────────────────────────────────────────────

    /// Returns the context-menu state.
    pub fn context_menu(&self) -> &ContextMenuState {
        &self.context_menu
    }

    /// Opens the context menu at a widget-local position.
    ///
    /// The `paste` row's state comes from [`Self::clipboard_has_text`], which is
    /// cached rather than read here. Opening a menu is a pure UI action and must not
    /// touch the machine-global OS clipboard, whose handle is exclusive (it can
    /// block, and it reports empty while another process holds it — which would
    /// disable `paste` for a reason the user cannot see).
    pub fn open_context_menu(&mut self, position: Point) {
        let has_selection = self.has_selection();
        let clipboard_has_text = self.clipboard_has_text();
        let read_only = !self.is_editable();
        // A selection that overlaps a locked range cannot be mutated, so the
        // menu reports the refusal before the command runs. Without this every
        // actionable row would look enabled and then silently do nothing.
        let (selection_start, selection_end) = self.cursor.bounds();
        let selection_locked =
            has_selection && !self.is_range_editable(selection_start, selection_end);
        let caret_locked = !self.is_position_editable(self.cursor.head);
        let mutation_blocked = read_only || selection_locked || caret_locked;
        let selection_mutation_blocked = read_only || selection_locked;
        self.context_menu.items = vec![
            MenuItem::new("cut", "Cut")
                .with_shortcut("Cmd+X")
                .disabled(!has_selection || selection_mutation_blocked),
            MenuItem::new("copy", "Copy").with_shortcut("Cmd+C").disabled(!has_selection),
            MenuItem::new("paste", "Paste")
                .with_shortcut("Cmd+V")
                .disabled(mutation_blocked || !clipboard_has_text),
            MenuItem::new("select_all", "Select All").with_shortcut("Cmd+A"),
            MenuItem::new("find", "Find").with_shortcut("Cmd+F"),
            MenuItem::new("replace", "Replace").with_shortcut("Cmd+H"),
            MenuItem::new("toggle_comment", "Toggle Comment")
                .with_shortcut("Cmd+/")
                .disabled(mutation_blocked),
            MenuItem::new("fold", "Fold Block").with_shortcut("Cmd+["),
            MenuItem::new("unfold_all", "Unfold All"),
            MenuItem::new("indent", "Indent").with_shortcut("Cmd+I").disabled(mutation_blocked),
            MenuItem::new("outdent", "Outdent").with_shortcut("Cmd+[").disabled(mutation_blocked),
            MenuItem::new("duplicate", "Duplicate Line")
                .with_shortcut("Cmd+Shift+D")
                .disabled(mutation_blocked),
            MenuItem::new("delete_line", "Delete Line")
                .with_shortcut("Cmd+Shift+K")
                .disabled(mutation_blocked),
            MenuItem::new("sort_lines", "Sort Lines").disabled(selection_mutation_blocked),
            MenuItem::new("select_occurrences", "Select All Occurrences")
                .with_shortcut("Cmd+Shift+L"),
            MenuItem::new("add_cursor_below", "Add Cursor Below").with_shortcut("Alt+Down"),
            MenuItem::new("complete", "Complete").with_shortcut("Ctrl+Space"),
        ];
        self.context_menu.position = position;
        self.context_menu.selected =
            self.context_menu.items.iter().position(|item| !item.disabled).unwrap_or(0);
        self.context_menu.visible = true;
        self.completion.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Closes the context menu.
    pub fn close_context_menu(&mut self) {
        if !self.context_menu.visible {
            return;
        }
        self.context_menu.visible = false;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Activates a context-menu row by its stable id.
    pub fn activate_menu_item(&mut self, id: &str) -> bool {
        let Some(index) = self.context_menu.items.iter().position(|item| item.id == id) else {
            return false;
        };
        if self.context_menu.items.get(index).map(|item| item.disabled).unwrap_or(true) {
            return false;
        }
        self.context_menu.visible = false;
        let handled = match id {
            "cut" => self.cut(),
            "copy" => self.copy(),
            "paste" => self.paste(),
            "select_all" => {
                self.select_all();
                true
            }
            "find" => {
                self.open_find(false);
                true
            }
            "replace" => {
                self.open_find(true);
                true
            }
            "toggle_comment" => {
                self.toggle_line_comment();
                true
            }
            "fold" => {
                let line = self.cursor.head.line;
                self.toggle_fold(line)
            }
            "unfold_all" => {
                self.unfold_all();
                true
            }
            "indent" => {
                self.indent_selection();
                true
            }
            "outdent" => {
                self.outdent_selection();
                true
            }
            "duplicate" => {
                self.duplicate_line();
                true
            }
            "delete_line" => {
                self.delete_line();
                true
            }
            "sort_lines" => {
                self.sort_lines();
                true
            }
            "select_occurrences" => {
                self.select_all_occurrences();
                true
            }
            "add_cursor_below" => self.add_cursor_below(),
            "complete" => self.trigger_completion() > 0,
            _ => false,
        };
        self.base.request_layout();
        self.base.request_redraw();
        handled
    }

    /// Moves the context-menu highlight by `delta` rows, wrapping around.
    pub fn move_menu_selection(&mut self, delta: isize) -> bool {
        if !self.context_menu.visible || self.context_menu.items.is_empty() {
            return false;
        }
        let count = self.context_menu.items.len() as isize;
        let current = self.context_menu.selected.min(count as usize - 1) as isize;
        self.context_menu.selected = (current + delta).rem_euclid(count) as usize;
        self.base.request_redraw();
        true
    }
}
