//! Editor commands driven by this demo's menu bar and tool bar.
//!
//! # Ownership
//!
//! This belongs to the **demo**, not the library. Every variant maps onto a
//! `CodeEditor` method, so the command list is demo business vocabulary — what
//! *this* application chose to expose. The library only supplies the generic
//! bridge ([`CustomWidgetHandle::update`] plus `runtime::with_widget_mut`); a
//! different host would pick a different command set without touching the crate.
//!
//! # How a command reaches the widget
//!
//! A custom-painted widget has no OS control of its own, so a menu item cannot
//! deliver an event to it directly. The path is:
//!
//! ```text
//! menu item → menu event queue → poll_menu_triggered()
//!           → CustomWidgetHandle::update(|widget| downcast + apply)
//!           → runtime::request_repaint → repaint
//! ```

use rust_widgets::app::CustomWidgetHandle;
use rust_widgets::shortcut::{Key, Modifiers, Shortcut};
use rust_widgets::widget::special_widgets::code_editor::CodeEditor;

/// An action this demo exposes in its menus and tool bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    // History
    Undo,
    Redo,
    // Clipboard
    Copy,
    Cut,
    Paste,
    SelectAll,
    // Lines
    DuplicateLine,
    DeleteLine,
    JoinLines,
    SortLines,
    TrimTrailingWhitespace,
    ToggleComment,
    Indent,
    Outdent,
    MoveLineUp,
    MoveLineDown,
    // Multi-cursor
    AddCursorAbove,
    AddCursorBelow,
    SelectNextOccurrence,
    SelectAllOccurrences,
    CollapseCursors,
    // Folding
    FoldBlock,
    UnfoldAll,
    // Find
    OpenFind,
    OpenReplace,
    CloseFind,
    FindNext,
    ReplaceAll,
    // Completion
    TriggerCompletion,
    // View
    PageUp,
    PageDown,
}

impl Command {
    /// Menu / tool-bar label.
    pub fn label(self) -> &'static str {
        use Command::*;
        match self {
            Undo => "Undo",
            Redo => "Redo",
            Copy => "Copy",
            Cut => "Cut",
            Paste => "Paste",
            SelectAll => "Select All",
            DuplicateLine => "Duplicate Line",
            DeleteLine => "Delete Line",
            JoinLines => "Join Lines",
            SortLines => "Sort Lines",
            TrimTrailingWhitespace => "Trim Trailing Whitespace",
            ToggleComment => "Toggle Comment",
            Indent => "Indent",
            Outdent => "Outdent",
            MoveLineUp => "Move Line Up",
            MoveLineDown => "Move Line Down",
            AddCursorAbove => "Add Cursor Above",
            AddCursorBelow => "Add Cursor Below",
            SelectNextOccurrence => "Select Next Occurrence",
            SelectAllOccurrences => "Select All Occurrences",
            CollapseCursors => "Collapse Cursors",
            FoldBlock => "Fold Block",
            UnfoldAll => "Unfold All",
            OpenFind => "Find…",
            OpenReplace => "Replace…",
            CloseFind => "Close Find",
            FindNext => "Find Next",
            ReplaceAll => "Replace All",
            TriggerCompletion => "Complete",
            PageUp => "Page Up",
            PageDown => "Page Down",
        }
    }

    /// Keyboard shortcut shown next to a menu item, or `None` when there is no
    /// single conventional binding.
    ///
    /// Declared with [`Shortcut::primary`] rather than a spelled-out string, so
    /// **one** table produces the native notation on every desktop OS: macOS
    /// renders `⌘⇧Z` where Windows and Linux render `Ctrl+Shift+Z`. Writing
    /// `"Cmd+Z"` here would have shown a Command glyph on Windows too.
    pub fn shortcut(self) -> Option<Shortcut> {
        use Command::*;
        use Key::*;
        match self {
            Undo => Some(Shortcut::primary(Z)),
            // Redo is `⇧⌘Z` on macOS and `Ctrl+Shift+Z` on Windows/Linux — the
            // same declaration covers both.
            Redo => Some(Shortcut::primary_shift(Z)),
            Copy => Some(Shortcut::primary(C)),
            Cut => Some(Shortcut::primary(X)),
            Paste => Some(Shortcut::primary(V)),
            SelectAll => Some(Shortcut::primary(A)),
            DuplicateLine => Some(Shortcut::new(D, Modifiers::PRIMARY | Modifiers::SHIFT)),
            DeleteLine => Some(Shortcut::new(K, Modifiers::PRIMARY | Modifiers::SHIFT)),
            JoinLines => Some(Shortcut::primary(J)),
            ToggleComment => Some(Shortcut::primary(Slash)),
            Indent => Some(Shortcut::primary(I)),
            Outdent => Some(Shortcut::primary(LeftBracket)),
            MoveLineUp => Some(Shortcut::alt(Up)),
            MoveLineDown => Some(Shortcut::alt(Down)),
            AddCursorAbove => Some(Shortcut::new(Up, Modifiers::ALT | Modifiers::SHIFT)),
            AddCursorBelow => Some(Shortcut::new(Down, Modifiers::ALT | Modifiers::SHIFT)),
            SelectNextOccurrence => Some(Shortcut::primary(D)),
            SelectAllOccurrences => Some(Shortcut::new(L, Modifiers::PRIMARY | Modifiers::SHIFT)),
            CollapseCursors => Some(Shortcut::from_key(Escape)),
            FoldBlock => Some(Shortcut::new(LeftBracket, Modifiers::PRIMARY | Modifiers::ALT)),
            UnfoldAll => Some(Shortcut::new(RightBracket, Modifiers::PRIMARY | Modifiers::ALT)),
            OpenFind => Some(Shortcut::primary(F)),
            OpenReplace => Some(Shortcut::primary(H)),
            CloseFind => Some(Shortcut::from_key(Escape)),
            FindNext => Some(Shortcut::from_key(Enter)),
            // Ctrl+Space is the completion chord on every platform, so it stays
            // a physical Control binding rather than becoming the primary one
            // (which would read `⌘Space` on macOS and mean Spotlight).
            TriggerCompletion => Some(Shortcut::ctrl(Space)),
            // `PageUp`/`PageDown` exist on both `Command` and `Key`, so the arms
            // are qualified to keep the pattern unambiguous.
            Command::PageUp => Some(Shortcut::from_key(Key::PageUp)),
            Command::PageDown => Some(Shortcut::from_key(Key::PageDown)),
            // `None` must be spelled `std::option::Option::None`: the `Key::None`
            // variant is glob-imported into this scope and would shadow it.
            SortLines | TrimTrailingWhitespace | ReplaceAll => std::option::Option::None,
        }
    }

    /// Renders this command's shortcut for the host OS, or `None` when it has no
    /// shortcut. `Esc`/`Enter`-style hints are shown for all platforms.
    pub fn shortcut_label(self) -> Option<String> {
        self.shortcut().map(|shortcut| rust_widgets::format_shortcut(&shortcut))
    }

    /// Applies the command to `editor`, returning whether anything changed.
    ///
    /// `false` is meaningful: a read-only editor rejects mutations, an empty
    /// history rejects undo, and sorting an ordered block changes nothing. The
    /// caller uses it to skip a pointless repaint.
    pub fn apply(self, editor: &mut CodeEditor) -> bool {
        use Command::*;
        match self {
            Undo => editor.undo(),
            Redo => editor.redo(),

            Copy => editor.copy(),
            Cut => editor.cut(),
            Paste => editor.paste(),
            SelectAll => {
                editor.select_all();
                true
            }

            // The line commands return `()` and are legitimately no-ops in some
            // states, so compare the document to answer honestly.
            DuplicateLine => edit(editor, |e| e.duplicate_line()),
            DeleteLine => edit(editor, |e| e.delete_line()),
            JoinLines => edit(editor, |e| e.join_lines()),
            SortLines => edit(editor, |e| e.sort_lines()),
            TrimTrailingWhitespace => edit(editor, |e| e.trim_trailing_whitespace()),
            ToggleComment => edit(editor, |e| e.toggle_line_comment()),
            Indent => edit(editor, |e| e.indent_selection()),
            Outdent => edit(editor, |e| e.outdent_selection()),
            MoveLineUp => edit(editor, |e| e.move_line(-1)),
            MoveLineDown => edit(editor, |e| e.move_line(1)),

            AddCursorAbove => editor.add_cursor_above(),
            AddCursorBelow => editor.add_cursor_below(),
            SelectNextOccurrence => editor.select_next_occurrence(),
            SelectAllOccurrences => editor.select_all_occurrences() > 1,
            CollapseCursors => editor.collapse_cursors(),

            FoldBlock => {
                let line = editor.caret().line;
                editor.toggle_fold(line)
            }
            UnfoldAll => {
                let before = editor.folded_line_count();
                editor.unfold_all();
                before > 0
            }

            OpenFind => {
                editor.open_find(false);
                true
            }
            OpenReplace => {
                editor.open_find(true);
                true
            }
            CloseFind => {
                let was_open = editor.find_visible();
                editor.close_find();
                was_open
            }
            FindNext => editor.find_next(false),
            ReplaceAll => editor.replace_all() > 0,

            TriggerCompletion => editor.trigger_completion() > 0,

            PageUp => {
                let rows = editor.visible_rows() as isize;
                editor.scroll_by(-rows);
                true
            }
            PageDown => {
                let rows = editor.visible_rows() as isize;
                editor.scroll_by(rows);
                true
            }
        }
    }

    /// Sends the command to a mounted editor, repainting when it changed.
    ///
    /// Returns `false` when the widget is no longer mounted, is not a
    /// `CodeEditor`, or the command was a no-op.
    pub fn dispatch(self, editor: &CustomWidgetHandle) -> bool {
        editor
            .update(|widget| {
                let Some(editor) = (widget as &mut dyn std::any::Any).downcast_mut::<CodeEditor>()
                else {
                    return false;
                };
                self.apply(editor)
            })
            .unwrap_or(false)
    }
}

/// Runs a line-editing operation and reports whether the document changed.
fn edit(editor: &mut CodeEditor, operation: impl FnOnce(&mut CodeEditor)) -> bool {
    let before = editor.text();
    operation(editor);
    editor.text() != before
}

/// Renders the demo's status line for a mounted editor.
///
/// Returns `None` when the widget is not a `CodeEditor` (or was unmounted).
pub fn status_line(editor: &CustomWidgetHandle) -> Option<String> {
    editor.read(|widget| {
        let editor = (widget as &dyn std::any::Any).downcast_ref::<CodeEditor>()?;
        let (line, column) = editor.cursor();
        let modified = if editor.is_modified() { " •" } else { "" };
        Some(format!(
            "{}  |  Ln {}, Col {}  |  {} lines{}",
            editor.language_name(),
            line + 1,
            column + 1,
            editor.line_count(),
            modified
        ))
    })?
}

/// Returns `true` when the mounted widget is a `CodeEditor`.
pub fn is_editor(editor: &CustomWidgetHandle) -> bool {
    editor.read(|widget| (widget as &dyn std::any::Any).is::<CodeEditor>()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_widgets::core::Rect;
    use rust_widgets::widget::special_widgets::code_editor::{CodeEditorConfig, LanguageId};

    fn editor(text: &str) -> CodeEditor {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 400, 300));
        editor.set_text(text);
        editor
    }

    #[test]
    fn history_commands_round_trip() {
        let mut editor = editor("one");
        editor.append_line("two");
        assert!(Command::Undo.apply(&mut editor));
        assert_eq!(editor.text(), "one");
        assert!(Command::Redo.apply(&mut editor));
        assert_eq!(editor.text(), "one\ntwo");
    }

    #[test]
    fn undo_without_history_reports_no_change() {
        let mut editor = editor("only");
        // `set_text` records a checkpoint; clear it to reach empty history.
        editor.clear_history();
        assert!(!Command::Undo.apply(&mut editor));
        assert!(!Command::Redo.apply(&mut editor));
    }

    #[test]
    fn set_text_leaves_a_checkpoint() {
        // The complement: the first edit of a fresh editor must be undoable.
        let mut editor = editor("initial");
        assert!(editor.can_undo());
        assert!(Command::Undo.apply(&mut editor));
        assert_eq!(editor.text(), "");
    }

    #[test]
    fn line_commands_edit_the_document() {
        let mut editor = editor("beta\nalpha");
        editor.set_cursor(0, 0, false);
        assert!(Command::SortLines.apply(&mut editor));
        assert_eq!(editor.text(), "alpha\nbeta");
    }

    #[test]
    fn duplicate_then_delete_line_round_trips() {
        let mut editor = editor("a\nb");
        editor.set_cursor(0, 0, false);
        assert!(Command::DuplicateLine.apply(&mut editor));
        assert_eq!(editor.text(), "a\na\nb");
        editor.set_cursor(1, 0, false);
        assert!(Command::DeleteLine.apply(&mut editor));
        assert_eq!(editor.text(), "a\nb");
    }

    #[test]
    fn sorting_an_ordered_block_is_honestly_a_no_op() {
        let mut editor = editor("alpha\nbeta");
        editor.set_cursor(0, 0, false);
        editor.move_cursor_with_selection(1, 0, true);
        assert!(
            !Command::SortLines.apply(&mut editor),
            "an already-sorted block must not claim a change"
        );
    }

    #[test]
    fn multi_cursor_commands_change_the_caret_set() {
        let mut editor = editor("a\nb\nc");
        editor.set_cursor(0, 0, false);
        assert!(Command::AddCursorBelow.apply(&mut editor));
        assert_eq!(editor.cursors().len(), 2);
        assert!(Command::CollapseCursors.apply(&mut editor));
        assert_eq!(editor.cursors().len(), 1);
        assert!(!Command::CollapseCursors.apply(&mut editor));
    }

    #[test]
    fn find_commands_open_and_close_the_bar() {
        let mut editor = editor("needle");
        assert!(Command::OpenFind.apply(&mut editor));
        assert!(editor.find_visible());
        assert!(Command::CloseFind.apply(&mut editor));
        assert!(!editor.find_visible());
        // Closing an already-closed bar is a no-op.
        assert!(!Command::CloseFind.apply(&mut editor));
    }

    #[test]
    fn replace_all_requires_a_hit() {
        let mut editor = editor("a-b-c");
        editor.open_find(true);
        editor.set_search_query("-");
        editor.set_search_replacement("+");
        assert!(Command::ReplaceAll.apply(&mut editor));
        assert_eq!(editor.text(), "a+b+c");
        assert!(!Command::ReplaceAll.apply(&mut editor));
    }

    #[test]
    fn read_only_editor_rejects_mutating_commands() {
        let mut editor = CodeEditor::with_config(
            Rect::new(0, 0, 400, 300),
            CodeEditorConfig::new().read_only(true),
        )
        .expect("valid config");
        editor.set_text("locked");
        editor.clear_history();
        // View operations still work.
        assert!(Command::SelectAll.apply(&mut editor));
        assert!(editor.has_selection());
        // Mutations report no change.
        assert!(!Command::Undo.apply(&mut editor));
        assert!(!Command::DuplicateLine.apply(&mut editor));
        assert_eq!(editor.text(), "locked");
    }

    #[test]
    fn page_commands_move_the_viewport() {
        let mut editor = editor("a\nb\nc\nd\ne\nf\ng\nh");
        editor.set_cursor(0, 0, false);
        let before = editor.scroll_line();
        assert!(Command::PageDown.apply(&mut editor));
        assert!(editor.scroll_line() >= before);
        assert!(Command::PageUp.apply(&mut editor));
        assert_eq!(editor.scroll_line(), before);
    }

    #[test]
    fn every_command_has_a_label() {
        for command in ALL_COMMANDS {
            assert!(!command.label().is_empty(), "{command:?} needs a label");
        }
    }

    /// Two different commands must never claim the same chord, otherwise one of
    /// them is unreachable from the keyboard.
    #[test]
    fn shortcuts_do_not_collide() {
        let mut seen: Vec<(Key, Modifiers)> = Vec::new();
        for command in ALL_COMMANDS {
            let Some(shortcut) = command.shortcut() else {
                continue;
            };
            // `Esc` is legitimately shared (collapse cursors / close find), and
            // both are only reachable in their own mode.
            if shortcut.key == Key::Escape {
                continue;
            }
            let pair = (shortcut.key, shortcut.modifiers);
            assert!(!seen.contains(&pair), "{command:?} reuses the chord {:?}", shortcut);
            seen.push(pair);
        }
    }

    /// Application commands are declared with `PRIMARY`, not `CTRL`, so they
    /// render natively on every desktop OS.
    #[test]
    fn editing_commands_use_the_primary_modifier() {
        for command in [Command::Undo, Command::Copy, Command::Paste, Command::OpenFind] {
            let shortcut = command.shortcut().expect("command has a shortcut");
            assert!(
                shortcut.modifiers.contains(Modifiers::PRIMARY),
                "{command:?} should use PRIMARY so it renders natively per platform"
            );
        }
    }

    /// Rendered labels must match the host's notation, not a fixed convention.
    #[test]
    fn shortcut_labels_render_for_the_host_os() {
        use rust_widgets::shortcut::PlatformShortcutStyle;

        let undo = Command::Undo.shortcut_label().expect("Undo has a shortcut");
        // The expected notation is a runtime fact of the current platform, not a
        // compile-time `cfg!(target_os)` branch: the demo asks the library for the
        // active shortcut style, just as a real caller would.
        let expected = match rust_widgets::shortcut::PlatformShortcutStyle::current() {
            PlatformShortcutStyle::Mac => "⌘Z",
            PlatformShortcutStyle::Desktop => "Ctrl+Z",
        };
        assert_eq!(undo, expected);

        // Commands with no conventional binding must not invent one.
        assert!(Command::SortLines.shortcut_label().is_none());
    }

    #[test]
    fn toggle_comment_uses_the_language_prefix() {
        let mut editor = editor("let x = 1;");
        editor.set_language(LanguageId::Rust);
        editor.set_cursor(0, 0, false);
        assert!(Command::ToggleComment.apply(&mut editor));
        assert!(editor.text().starts_with("//"));
    }

    /// Every command the demo exposes, so tests stay exhaustive.
    const ALL_COMMANDS: [Command; 31] = [
        Command::Undo,
        Command::Redo,
        Command::Copy,
        Command::Cut,
        Command::Paste,
        Command::SelectAll,
        Command::DuplicateLine,
        Command::DeleteLine,
        Command::JoinLines,
        Command::SortLines,
        Command::TrimTrailingWhitespace,
        Command::ToggleComment,
        Command::Indent,
        Command::Outdent,
        Command::MoveLineUp,
        Command::MoveLineDown,
        Command::AddCursorAbove,
        Command::AddCursorBelow,
        Command::SelectNextOccurrence,
        Command::SelectAllOccurrences,
        Command::CollapseCursors,
        Command::FoldBlock,
        Command::UnfoldAll,
        Command::OpenFind,
        Command::OpenReplace,
        Command::CloseFind,
        Command::FindNext,
        Command::ReplaceAll,
        Command::TriggerCompletion,
        Command::PageUp,
        Command::PageDown,
    ];
}
