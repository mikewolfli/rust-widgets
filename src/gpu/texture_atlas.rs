// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Texture atlas for GPU rendering — packs small textures into a larger atlas (BLUE11 R5.8).
use crate::compat::HashMap;
use crate::core::Size;

/// A rectangle within the texture atlas.
///
/// Coordinates are in atlas texels with the origin at the atlas's top-left
/// corner; `x` and `y` are inclusive, and the far edges are at `x + width` and
/// `y + height`. All four values are zero-based and unsigned, so this type
/// cannot express a rectangle that lies outside the atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasRect {
    /// Left edge, in atlas texels.
    pub x: u32,
    /// Top edge, in atlas texels.
    pub y: u32,
    /// Width in texels; the right edge is `x + width`.
    pub width: u32,
    /// Height in texels; the bottom edge is `y + height`.
    pub height: u32,
}

/// A single entry in the texture atlas.
///
/// The id is a key into the atlas's entry map; it is not a GPU texture handle.
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    /// Where this entry's pixels live inside the atlas.
    pub rect: AtlasRect,
    /// Identifier assigned at allocation time. Always equal to the map key the
    /// entry is stored under, so it is redundant for lookup but useful to carry
    /// alongside the rectangle.
    pub texture_id: u64,
}

/// Simple texture atlas that packs textures into rows.
///
/// Allocation is a first-fit row packer: entries advance left to right along
/// the current row and, when the next entry would exceed the atlas width, a new
/// row is started below the tallest entry of the previous row. There is no
/// compaction and [`TextureAtlas::remove`] does **not** reclaim space — the free
/// rectangle is simply abandoned — so a long-lived atlas that churns entries
/// will eventually fill up and return `None`.
///
pub struct TextureAtlas {
    max_size: Size,
    entries: HashMap<u64, AtlasEntry>,
    next_id: u64,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl TextureAtlas {
    /// Creates an empty atlas whose usable area is `max_size` texels.
    ///
    /// The atlas allocates no GPU resources itself; `max_size` only bounds the
    /// packing arithmetic. Ids start at `1`, so `0` is never a valid entry id.
    pub fn new(max_size: Size) -> Self {
        Self {
            max_size,
            entries: HashMap::new(),
            next_id: 1,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// Allocate a slot in the atlas. Returns the texture ID and rect.
    ///
    /// Returns `None` when the entry does not fit — either because the height would push past
    /// `max_size.height` after wrapping, or because an entry is wider than the atlas (which
    /// previously wrapped to column 0 and then allocated an overflowing rectangle, so callers had to
    /// remember not to do it). All arithmetic is checked, so a huge `width`/`height` cannot wrap the
    /// cursor or panic in debug builds. Zero-sized requests succeed and consume no space.
    ///
    /// Ids are handed out sequentially and never reused, even after removal.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u64, AtlasRect)> {
        // An entry wider or taller than the atlas can never fit; reject it up front rather than
        // letting the wrap test produce an overflowing rectangle.
        if width > self.max_size.width || height > self.max_size.height {
            return None;
        }
        // Simple row-based packing. `checked_add` so a value near `u32::MAX` cannot wrap the cursor
        // (which would silently place the entry at a bogus origin) or panic in debug builds.
        if self.cursor_x.checked_add(width)? > self.max_size.width {
            // Start new row
            self.cursor_x = 0;
            self.cursor_y = self.cursor_y.checked_add(self.row_height)?;
            self.row_height = 0;
        }
        if self.cursor_y.checked_add(height)? > self.max_size.height {
            return None; // Atlas full or the new row overflows
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let rect = AtlasRect { x: self.cursor_x, y: self.cursor_y, width, height };
        self.entries.insert(id, AtlasEntry { rect, texture_id: id });
        self.cursor_x = self.cursor_x.checked_add(width)?;
        self.row_height = self.row_height.max(height);
        Some((id, rect))
    }

    /// Returns the entry with the given id, or `None` if it was never allocated
    /// or has been removed.
    pub fn get(&self, id: u64) -> Option<&AtlasEntry> {
        self.entries.get(&id)
    }

    /// Drops the entry's bookkeeping. No-op for an unknown id.
    ///
    /// The freed region is **not** returned to the packer: the cursors are left
    /// where they are, so the space stays occupied until
    /// [`TextureAtlas::clear`]. Removing an id also means a later `get` for it
    /// returns `None` even though its rectangle is still referenced by whatever
    /// was uploaded there.
    pub fn remove(&mut self, id: u64) {
        self.entries.remove(&id);
    }

    /// Forgets every entry and resets the packer, making the whole atlas
    /// available again. Ids continue from the previous value rather than
    /// restarting at `1`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
    }

    /// Returns the number of live entries — not the number of allocated slots,
    /// since removed entries are subtracted.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns `true` when no entries are registered. A cleared atlas is empty
    /// even if its packer has advanced.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Returns the fraction of the atlas area covered by live entries, in
    /// `0.0 ..= 1.0`.
    ///
    /// This is the sum of entry areas divided by the total area, so it can
    /// exceed a realistic packing efficiency only if rectangles overlap; it
    /// does not include padding. A zero-area atlas reports `0.0` rather than
    /// dividing by zero.
    pub fn utilization(&self) -> f32 {
        if self.max_size.width == 0 || self.max_size.height == 0 {
            return 0.0;
        }
        let total_pixels = self.max_size.width as u64 * self.max_size.height as u64;
        let used_pixels: u64 =
            self.entries.values().map(|e| e.rect.width as u64 * e.rect.height as u64).sum();
        used_pixels as f32 / total_pixels as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_pack_left_to_right_and_wrap_to_a_new_row() {
        let mut atlas = TextureAtlas::new(Size::new(100, 100));
        let (_, a) = atlas.allocate(60, 20).expect("first entry fits");
        assert_eq!((a.x, a.y), (0, 0));
        // The second 60-wide entry cannot follow the first on the row, so it wraps.
        let (_, b) = atlas.allocate(60, 20).expect("second entry wraps to a new row");
        assert_eq!((b.x, b.y), (0, 20), "it starts at column 0 on the row below");
        // A narrow entry now follows `b` on row 1 (the packer does not backfill row 0).
        let (_, c) = atlas.allocate(30, 20).expect("a narrow entry follows on the current row");
        assert_eq!((c.x, c.y), (60, 20));
    }

    /// An entry wider (or taller) than the atlas is refused.
    ///
    /// Pins the defect: the wrap test used unchecked `cursor_x + width`, so an entry wider
    /// than the atlas wrapped to column 0 and was allocated anyway, with a rectangle that
    /// overflowed the atlas — the doc even told callers to avoid it. All arithmetic is now
    /// checked and an oversize request is rejected up front.
    #[test]
    fn an_entry_larger_than_the_atlas_is_refused() {
        let mut atlas = TextureAtlas::new(Size::new(64, 64));
        assert!(atlas.allocate(128, 16).is_none(), "wider than the atlas");
        assert!(atlas.allocate(16, 128).is_none(), "taller than the atlas");
        assert!(atlas.allocate(64, 64).is_some(), "an exactly-atlas-sized entry still fits");
    }

    /// A `u32::MAX` request must not panic (debug) or wrap the cursor (release).
    #[test]
    fn a_huge_request_is_refused_without_panicking() {
        let mut atlas = TextureAtlas::new(Size::new(64, 64));
        assert!(atlas.allocate(u32::MAX, 1).is_none());
        assert!(atlas.allocate(1, u32::MAX).is_none());
        assert!(atlas.allocate(u32::MAX, u32::MAX).is_none());
        // The atlas is still usable afterwards.
        assert!(atlas.allocate(8, 8).is_some());
    }

    #[test]
    fn utilization_tracks_the_used_area() {
        let mut atlas = TextureAtlas::new(Size::new(100, 100));
        assert_eq!(atlas.utilization(), 0.0);
        let (_, rect) = atlas.allocate(50, 50).expect("fits");
        assert_eq!(rect.width, 50);
        assert!((atlas.utilization() - 0.25).abs() < 1e-6);
    }
}
