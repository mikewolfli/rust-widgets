// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SDI (single-document interface) workbench helper.
//!
//! The library already exposes every piece a desktop application needs to run several
//! independent top-level windows: [`crate::create_window`] creates one, [`crate::app::WindowHandle`]
//! addresses it, and the handle's `set_geometry` moves it. What was missing is the small
//! workbench object that owns the *set* of those windows and arranges them. This type is that
//! object; it deliberately adds no second window-creation mechanism, no second geometry path and
//! no `WindowKind` routing.
//!
//! # Why the cascade/tile arithmetic is not delegated to `MdiArea`
//!
//! [`crate::widget::MdiArea`] owns *sub-windows*: records inside one host window, addressed by a
//! child `ObjectId` and positioned in the area's own coordinate space. Its
//! `cascade_sub_windows`/`tile_sub_windows` mutate that private record list, so they cannot move
//! top-level windows. Calling them from here would require fabricating an `MdiArea` and copying
//! rectangles back out — a second, lossy representation of the same set. The offset/step math is
//! therefore implemented once in [`cascade_layout`] and [`tile_layout`], using the same formula
//! `MdiArea` uses, and applied through `WindowHandle::set_geometry`.
//!
//! # What the workbench is not
//!
//! It is not a window manager: it does not create a window's chrome, does not route input and
//! does not hide a window. A host builds each document window with the existing APIs and either
//! lets [`SdiWorkbench::open_window`] create it or hands it over with
//! [`SdiWorkbench::adopt_window`].

use crate::app::{WidgetHandle, WindowHandle};
use crate::core::Rect;

/// The offset between two cascaded windows when the host does not choose one.
pub const DEFAULT_CASCADE_OFFSET: i32 = 30;

/// A set of top-level windows with cascade/tile arrangement.
///
/// Every method that moves a window goes through the existing `WindowHandle::set_geometry`, so
/// the platform, the window-size mirror and the child layout all follow the same path they would
/// for a handle the host moved itself.
#[derive(Debug, Clone, Default)]
pub struct SdiWorkbench {
    bounds: Rect,
    windows: Vec<WindowHandle>,
    cascade_offset: i32,
}

impl SdiWorkbench {
    /// Creates an empty workbench whose arrangement area is `bounds`.
    ///
    /// `bounds` is expressed in the same screen coordinates the platform uses for window
    /// positions (`crate::create_window`'s `x`/`y`).
    pub fn new(bounds: Rect) -> Self {
        Self { bounds, windows: Vec::new(), cascade_offset: DEFAULT_CASCADE_OFFSET }
    }

    /// The arrangement area.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Moves the arrangement area. Existing windows are not moved until `cascade`/`tile` is called.
    pub fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    /// The offset between cascaded windows.
    pub fn cascade_offset(&self) -> i32 {
        self.cascade_offset
    }

    /// Sets the offset between cascaded windows; a negative value is clamped to zero.
    pub fn set_cascade_offset(&mut self, offset: i32) {
        self.cascade_offset = offset.max(0);
    }

    /// Creates a top-level window and adds it to the workbench.
    ///
    /// Creation uses [`crate::create_window`], the same entry point `App::new_window` uses, and
    /// records the initial geometry in the handle mirror so a layout applied immediately
    /// afterwards has a rectangle to work with. A platform refusal returns a handle with a zero
    /// id (the same failure signal the raw API uses) and does not add anything to the set.
    pub fn open_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> WindowHandle {
        let id = crate::create_window(title, x, y, width, height);
        if id == 0 {
            return WindowHandle::from_raw(0);
        }
        WindowHandle::record_created_geometry(id, x, y, width, height);
        let window = WindowHandle::from_raw(id);
        self.windows.push(window.clone());
        window
    }

    /// Adds an existing top-level window to the set.
    ///
    /// Returns `false` for a zero id or a window already in the set, so the workbench never keeps
    /// two handles for one window and `cascade`/`tile` cannot move it twice.
    pub fn adopt_window(&mut self, window: WindowHandle) -> bool {
        let id = window.raw_id();
        if id == 0 || self.windows.iter().any(|existing| existing.raw_id() == id) {
            return false;
        }
        self.windows.push(window);
        true
    }

    /// Removes a window from the set without closing or hiding it.
    ///
    /// Returns whether the window was in the set.
    pub fn release_window(&mut self, window: &WindowHandle) -> bool {
        let id = window.raw_id();
        let before = self.windows.len();
        self.windows.retain(|existing| existing.raw_id() != id);
        self.windows.len() != before
    }

    /// The windows the workbench manages, in adoption order.
    pub fn windows(&self) -> &[WindowHandle] {
        &self.windows
    }

    /// How many windows the workbench manages.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Arranges every window in a cascade, returning how many windows were moved.
    pub fn cascade(&mut self) -> usize {
        let layouts = cascade_layout(self.bounds, self.windows.len(), self.cascade_offset);
        for (window, rect) in self.windows.iter().zip(layouts) {
            window.set_geometry(rect.x, rect.y, rect.width, rect.height);
        }
        self.windows.len()
    }

    /// Arranges every window in a grid, returning how many windows were moved.
    pub fn tile(&mut self) -> usize {
        let layouts = tile_layout(self.bounds, self.windows.len());
        for (window, rect) in self.windows.iter().zip(layouts) {
            window.set_geometry(rect.x, rect.y, rect.width, rect.height);
        }
        self.windows.len()
    }
}

/// The rectangles a cascade of `count` windows occupies inside `bounds`.
///
/// Each window is offset by `offset` from the previous one, and every window keeps the same size:
/// the largest size that still fits the last offset inside `bounds`. A degenerate area (or an
/// offset larger than the area) clamps to a zero-size rectangle rather than underflowing.
pub fn cascade_layout(bounds: Rect, count: usize, offset: i32) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let offset = offset.max(0);
    let max_width = (bounds.width as f32 - offset as f32 * (count as f32 - 1.0)).max(0.0) as u32;
    let max_height = (bounds.height as f32 - offset as f32 * (count as f32 - 1.0)).max(0.0) as u32;
    (0..count)
        .map(|index| {
            let x = bounds.x as f32 + offset as f32 * index as f32;
            let y = bounds.y as f32 + offset as f32 * index as f32;
            Rect::new(x as i32, y as i32, max_width, max_height)
        })
        .collect()
}

/// The rectangles a tile of `count` windows occupies inside `bounds`.
///
/// The grid is `ceil(sqrt(count))` columns wide, with rows derived from the same column count, so
/// the cells stay as square as the area allows. The last row may be partially filled; no window
/// is stretched across the remainder.
pub fn tile_layout(bounds: Rect, count: usize) -> Vec<Rect> {
    if count == 0 {
        return Vec::new();
    }
    let cols = (count as f32).sqrt().ceil() as usize;
    let rows = (count as f32 / cols as f32).ceil() as usize;
    let cell_width = (bounds.width as f32 / cols as f32).max(0.0) as u32;
    let cell_height = (bounds.height as f32 / rows as f32).max(0.0) as u32;
    (0..count)
        .map(|index| {
            let col = index % cols;
            let row = index / cols;
            let x = bounds.x as f32 + cell_width as f32 * col as f32;
            let y = bounds.y as f32 + cell_height as f32 * row as f32;
            Rect::new(x as i32, y as i32, cell_width, cell_height)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn adopting_a_window_is_idempotent_and_releasable() {
        let mut workbench = SdiWorkbench::new(Rect::new(0, 0, 1000, 800));
        let window = WindowHandle::from_raw(7);

        assert!(workbench.adopt_window(window.clone()));
        assert!(!workbench.adopt_window(window.clone()), "the same window is adopted once");
        assert_eq!(workbench.window_count(), 1);
        assert_eq!(workbench.windows()[0].raw_id(), 7);

        assert!(workbench.release_window(&window));
        assert!(!workbench.release_window(&window), "releasing twice is a no-op");
        assert!(workbench.windows().is_empty());
    }

    #[test]
    fn a_zero_id_window_is_not_adopted() {
        let mut workbench = SdiWorkbench::new(Rect::new(0, 0, 1000, 800));
        assert!(!workbench.adopt_window(WindowHandle::from_raw(0)));
        assert_eq!(workbench.window_count(), 0);
    }

    #[test]
    fn cascade_steps_each_window_by_the_offset() {
        let layouts = cascade_layout(Rect::new(10, 20, 1000, 800), 3, 30);
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0], Rect::new(10, 20, 940, 740));
        assert_eq!(layouts[1], Rect::new(40, 50, 940, 740));
        assert_eq!(layouts[2], Rect::new(70, 80, 940, 740));
        assert_eq!(
            layouts[2].x + layouts[2].width as i32,
            10 + 1000,
            "the last offset window still ends at the area's edge"
        );
    }

    #[test]
    fn cascade_clamps_a_degenerate_area() {
        let layouts = cascade_layout(Rect::new(0, 0, 20, 10), 3, 30);
        assert_eq!(layouts[0], Rect::new(0, 0, 0, 0));
        assert_eq!(layouts[2], Rect::new(60, 60, 0, 0));
    }

    #[test]
    fn tile_splits_into_a_square_as_possible_grid() {
        let layouts = tile_layout(Rect::new(10, 20, 1000, 800), 4);
        assert_eq!(layouts.len(), 4);
        assert_eq!(layouts[0], Rect::new(10, 20, 500, 400));
        assert_eq!(layouts[1], Rect::new(510, 20, 500, 400));
        assert_eq!(layouts[2], Rect::new(10, 420, 500, 400));
        assert_eq!(layouts[3], Rect::new(510, 420, 500, 400));
    }

    #[test]
    fn tile_derives_the_row_count_from_the_column_count() {
        let layouts = tile_layout(Rect::new(0, 0, 600, 600), 3);
        // ceil(sqrt(3)) = 2 columns, so 2 rows of 300x300 cells.
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0], Rect::new(0, 0, 300, 300));
        assert_eq!(layouts[1], Rect::new(300, 0, 300, 300));
        assert_eq!(layouts[2], Rect::new(0, 300, 300, 300));
    }

    #[test]
    fn empty_workbench_arrangements_are_noops() {
        let mut workbench = SdiWorkbench::new(Rect::new(0, 0, 1000, 800));
        assert_eq!(workbench.cascade(), 0);
        assert_eq!(workbench.tile(), 0);
        assert_eq!(workbench.bounds().position(), Point::new(0, 0));
    }

    #[test]
    fn bounds_and_offset_are_reported_back() {
        let mut workbench = SdiWorkbench::new(Rect::new(1, 2, 3, 4));
        assert_eq!(workbench.bounds(), Rect::new(1, 2, 3, 4));
        workbench.set_bounds(Rect::new(5, 6, 700, 500));
        assert_eq!(workbench.bounds(), Rect::new(5, 6, 700, 500));
        assert_eq!(workbench.cascade_offset(), DEFAULT_CASCADE_OFFSET);
        workbench.set_cascade_offset(-4);
        assert_eq!(workbench.cascade_offset(), 0);
    }
}
