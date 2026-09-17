// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Printing utilities and document layout support.
//!
//! # Reachability
//!
//! **State:** Reserved: a print/preview subsystem with no production consumer. Build it only under `--features print`. Removal condition: when print support is dropped from the product scope, or when a host wires `print_to_printer`.
mod print_impl;
pub use print_impl::*;
