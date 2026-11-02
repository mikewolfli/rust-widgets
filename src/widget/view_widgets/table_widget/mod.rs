// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Table widget.
//!
//! Entry point only: the implementation lives in `widget.rs`, with rendering, events,
//! properties and tests in `widget/`. Re-exporting the public surface here keeps every
//! historical path (`table_widget::TableWidget`, `table_widget::VecTableModel`, …) unchanged.

mod widget;

pub use widget::*;
