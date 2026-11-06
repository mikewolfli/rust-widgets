// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The single placement entry for a container's content child.
//!
//! A container owns a *box* (its content rectangle) and the id of the child that fills it;
//! the runtime owns the child. Placing the child is therefore one write of that box into
//! the child's runtime geometry, and every single-content container routes through this
//! function so the rule lives in one place rather than seven.
//!
//! # Why the write is skipped when the rectangle is unchanged
//!
//! `set_widget_geometry` invalidates the child's layer cache and requests a repaint of its
//! subtree. Several containers re-derive their content box from `draw` (a tab view places
//! its selected page as it paints it), so an unconditional write made a *still* frame
//! invalidate itself: the layer cache never kept a frame and the window repainted forever.
//! Reading the current rectangle first keeps the steady state at one registry read and no
//! invalidation, which is the same guard `set_widget_geometry`'s backend entry documents
//! (`create_widgets_helpers.in.rs`, "unchanged" path).

use crate::core::{ObjectId, Rect};

/// Writes `rect` into the control mounted in `slot`, if any.
///
/// The rectangle is left alone when it already equals the child's runtime geometry, so a
/// container that re-places on every paint pays one read rather than a full invalidation.
pub(crate) fn place_content_child(slot: Option<ObjectId>, rect: Rect) {
    let Some(widget_id) = slot else {
        return;
    };
    #[cfg(not(alloc_frugal))]
    {
        if crate::widget_geometry(widget_id) == Some((rect.x, rect.y, rect.width, rect.height)) {
            return;
        }
        crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
        // Keep the tab order's spatial positions in step with layout, so a `RowMajor` or
        // `ColumnMajor` traversal strategy sorts by where the controls actually are rather than
        // by stale coordinates only a host that feeds them manually would ever update (RT2-04).
        // The call ignores ids that are not registered focusable, so a plain content child costs
        // one registration lookup and nothing else.
        crate::widget::runtime::set_focusable_position(widget_id, rect.x, rect.y);
    }
    // A sized profile has no mounted surfaces and no platform geometry entry. The rectangle
    // is still the container's model, so it is accepted and not turned into a write there.
    #[cfg(alloc_frugal)]
    let _ = (widget_id, rect);
}
