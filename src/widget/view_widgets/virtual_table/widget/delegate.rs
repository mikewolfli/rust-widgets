// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The virtual table's cell painter and its collection placeholder accessors.
//!
//! A child module so the placeholders' `CollectionState` impl can reach `VirtualTable`'s own
//! field, and so `virtual_table.rs`'s production part stays under the structure limit.

use super::*;
use crate::widget::view_widgets::collection_state::CollectionState;

/// Paint hook for a virtual table cell.
///
/// The table itself only knows how to draw text. A host that wants a checkbox column, a
/// progress bar or rich text installs a delegate, which owns a cell's ink when `paint`
/// returns `true`; returning `false` keeps the table's own text rendering.
pub trait VirtualCellDelegate: Send + Sync {
    /// Paints `cell`, returning whether the delegate took over the cell.
    fn paint(
        &self,
        context: &mut RenderContext,
        cell: Rect,
        row: usize,
        column: usize,
        text: Option<&str>,
    ) -> bool {
        let _ = (context, cell, row, column, text);
        false
    }
}

impl VirtualTable {
    /// The empty/error captions painted when the source has no rows (RW-17).
    pub fn placeholder(&self) -> &CollectionPlaceholder {
        &self.placeholder
    }
}

impl CollectionState for VirtualTable {
    fn collection_placeholder(&self) -> &CollectionPlaceholder {
        &self.placeholder
    }

    fn collection_placeholder_mut(&mut self) -> &mut CollectionPlaceholder {
        &mut self.placeholder
    }
}
