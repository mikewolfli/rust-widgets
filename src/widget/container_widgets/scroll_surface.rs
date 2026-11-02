// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The generic scroll surface contract (RW-58).
//!
//! # Why this exists
//!
//! Each scrolling container owned its offset under a different name: `ScrollArea` publishes
//! `scroll_position` / `set_scroll_position` / `ensure_visible`, while `MdiArea` exposes a
//! `scroll_offset` point with `scroll_by`. A host that restores a workspace therefore had to
//! know which concrete control it was holding and write one branch per control.
//!
//! This trait is the common read/write primitive. The per-widget methods stay exactly as they
//! were; implementing the trait is additive and only names the three operations every scrolling
//! surface already performs.
//!
//! # Coordinate space
//!
//! Positions are **content** coordinates: `(0, 0)` is the top-left of the scrollable content,
//! and a positive offset moves the content up/left under the viewport. `ensure_visible` takes a
//! rectangle in the same space and is allowed to do nothing when the rectangle is already
//! inside the viewport.

use crate::core::Rect;

/// A container that can report and move its scroll offset.
pub trait ScrollableSurface {
    /// The current scroll offset as `(x, y)` in content coordinates.
    fn scroll_position(&self) -> (i32, i32);

    /// Moves the scroll offset to `(x, y)`.
    ///
    /// Implementations clamp against their own content and viewport extents, so a caller may
    /// pass an out-of-range request and read the accepted value back from
    /// [`Self::scroll_position`].
    fn set_scroll_position(&mut self, x: i32, y: i32);

    /// Scrolls the smallest distance that makes `rect` fully visible.
    ///
    /// `rect` is in content coordinates. An empty or already visible rectangle leaves the
    /// offset unchanged.
    fn ensure_visible(&mut self, rect: Rect);
}

#[cfg(widgets_unstripped)]
use super::mdiarea::MdiArea;
use super::scrollarea::ScrollArea;

impl ScrollableSurface for ScrollArea {
    fn scroll_position(&self) -> (i32, i32) {
        ScrollArea::scroll_position(self)
    }

    fn set_scroll_position(&mut self, x: i32, y: i32) {
        ScrollArea::set_scroll_position(self, x, y);
    }

    fn ensure_visible(&mut self, rect: Rect) {
        ScrollArea::ensure_visible(self, rect);
    }
}

#[cfg(widgets_unstripped)]
impl ScrollableSurface for MdiArea {
    fn scroll_position(&self) -> (i32, i32) {
        let offset = MdiArea::scroll_offset(self);
        (offset.x, offset.y)
    }

    fn set_scroll_position(&mut self, x: i32, y: i32) {
        MdiArea::set_scroll_offset(self, crate::core::Point::new(x, y));
    }

    /// Pans the work area by the smallest offset that brings `rect` into the viewport.
    ///
    /// The MDI pan is measured from the content's leading edge, and a stored sub-window frame
    /// is in content coordinates, so this is the same "subtract the offset, compare with the
    /// viewport" rule [`ScrollArea`] applies.
    fn ensure_visible(&mut self, rect: Rect) {
        let viewport = crate::widget::Widget::geometry(self);
        let current = MdiArea::scroll_offset(self);
        let mut x = current.x;
        let mut y = current.y;
        if rect.x - x < viewport.x {
            x = rect.x - viewport.x;
        }
        let right = rect.x.saturating_add(rect.width as i32) - x;
        let viewport_right = viewport.x.saturating_add(viewport.width as i32);
        if right > viewport_right {
            x = x.saturating_add(right - viewport_right);
        }
        if rect.y - y < viewport.y {
            y = rect.y - viewport.y;
        }
        let bottom = rect.y.saturating_add(rect.height as i32) - y;
        let viewport_bottom = viewport.y.saturating_add(viewport.height as i32);
        if bottom > viewport_bottom {
            y = y.saturating_add(bottom - viewport_bottom);
        }
        MdiArea::set_scroll_offset(self, crate::core::Point::new(x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scroll_area_round_trips_the_trait_offset() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 100, 100));
        area.set_content_size(crate::core::Size::new(400, 400));
        area.set_viewport(Rect::new(0, 0, 100, 100));
        ScrollableSurface::set_scroll_position(&mut area, 40, 60);
        assert_eq!(ScrollableSurface::scroll_position(&area), (40, 60));
        ScrollableSurface::ensure_visible(&mut area, Rect::new(0, 0, 10, 10));
        assert_eq!(
            ScrollableSurface::scroll_position(&area),
            (0, 0),
            "the leading edge is revealed"
        );
        // The per-widget entry and the trait entry name the same state.
        assert_eq!(area.scroll_position(), ScrollableSurface::scroll_position(&area));
    }

    #[cfg(widgets_unstripped)]
    #[test]
    fn an_mdi_area_round_trips_the_trait_offset() {
        let mut area = MdiArea::new(Rect::new(0, 0, 100, 100));
        // A pan needs content larger than the viewport to be meaningful; the MDI clamp keeps
        // an empty desktop at (0, 0).
        area.add_sub_window(1, Rect::new(0, 0, 400, 400));
        ScrollableSurface::set_scroll_position(&mut area, 15, 25);
        assert_eq!(ScrollableSurface::scroll_position(&area), (15, 25));
        // The MDI content bounds clamp the pan; the accepted value is what the trait reports.
        ScrollableSurface::set_scroll_position(&mut area, 9_999, 9_999);
        assert_eq!(ScrollableSurface::scroll_position(&area), (300, 300));
    }
}
