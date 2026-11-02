// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VirtualTable widget backed by an incremental table data source.
//!
//! Entry point only: the implementation lives in `widget.rs`, with rendering, events,
//! properties, the cell delegate and tests in `widget/`. Re-exporting the public surface here
//! keeps every historical path (`virtual_table::VirtualTable`, `virtual_table::VirtualCellDelegate`, …)
//! unchanged.

mod widget;

pub use widget::*;
