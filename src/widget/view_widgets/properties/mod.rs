// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Property editing widget family.
//!
//! The inspector vocabulary lives here: [`PropertyGrid`] is the two-column
//! name/value table, and [`PropertiesPanel`] is the categorized editor that
//! builds its rows on top of that grid. The grid is the lower layer and does
//! not depend on the panel, so the two stay in one folder without a cycle.

/// Categorized property editor panel built on [`PropertyGrid`].
pub mod properties_panel;
/// Inspector-style two-column name/value grid.
pub mod property_grid;

// Flat item paths: `view_widgets::properties_panel::X` /
// `view_widgets::property_grid::X` are re-exported by `view_widgets`, and the
// family keeps the same names available under `properties::`.
pub use properties_panel::{PropertiesPanel, PropertyEntry, PropertyValue};
pub use property_grid::{PropertyGrid, PropertyItem};
