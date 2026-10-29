// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Terminal control family.
//!
//! [`terminal_view`] is the main control: a pseudo-terminal view with local line editing,
//! scrollback and a minimal ANSI parser. [`terminal_find_bar`] is its companion search strip,
//! which scans the terminal's scrollback through [`terminal_view::TerminalView::find_matches`].
//! Keeping both modules in one folder makes the family identifiable at a glance.

pub mod terminal_find_bar;
pub mod terminal_view;
