// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The `devtools` group: the terminal view with its find strip, and the diff controls.
//!
//! # Why these are one group
//!
//! They are the controls a developer tool needs and an ordinary screen does not: a pseudo-terminal
//! with its scrollback search, a two-pane diff and the file list beside it. A host either ships a
//! developer surface or it does not.
//!
//! # Adding a group later
//!
//! A group is a module under `src/` with a `capability` submodule holding its property schemas,
//! event schemas and constructors, and an `install(factory)` function. A `mod.rs` only declares
//! and re-exports (principle #8).

mod capability;
/// The diff viewer and its file list.
pub mod diff_viewer;
/// The terminal family: the view and its find strip.
pub mod terminal;

pub use capability::{
    create_diff_file_list, create_diff_viewer, create_terminal_find_bar, create_terminal_view,
    default_property_value, diff_file_list_capability, diff_viewer_capability, install,
    published_event_names, published_property_names, terminal_find_bar_capability,
    terminal_view_capability, DIFF_FILE_LIST_PROPERTIES, DIFF_VIEWER_PROPERTIES,
    TERMINAL_FIND_BAR_PROPERTIES, TERMINAL_VIEW_PROPERTIES,
};
pub use diff_viewer::{DiffFileList, DiffKind, DiffLine, DiffViewer};
pub use terminal::terminal_find_bar::{TerminalFindBar, REGEX_SUPPORTED};
pub use terminal::terminal_view::TerminalView;
pub use terminal::{terminal_find_bar, terminal_view};
