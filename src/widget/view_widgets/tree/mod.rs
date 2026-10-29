// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Tree widget family.
//!
//! Two controls that share one hierarchical model vocabulary: the plain outline
//! ([`TreeView`]) and the column-annotated outline ([`TreeTable`]). They are
//! separate modules because each owns its own paint, hit-test and editing surface;
//! what they have in common is the tree shape, not the widget implementation.

/// Hierarchical rows paired with table columns ([`TreeTable`]).
pub mod tree_table;
/// Outline of expandable nodes ([`TreeView`]).
pub mod tree_view;

// Flat item paths: `view_widgets::tree_table::X` / `view_widgets::tree_view::X`
// are re-exported by `view_widgets`, and the family keeps the same names
// available under `tree::`.
pub use tree_table::{TreeTable, TreeTableModel};
pub use tree_view::TreeView;
