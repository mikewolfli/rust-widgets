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
    /// Returns `None` when the entry does not fit in the remaining rows — that
    /// is, when the height would push past `max_size.height` after wrapping.
    /// A `width` larger than the atlas width is *not* rejected up front: the
    /// wrap test leaves the cursor at column 0 and the entry is allocated with
    /// an overflowing rectangle, so callers should ensure entries are no wider
    /// than the atlas. Zero-sized requests succeed and consume no space.
    ///
    /// Ids are handed out sequentially and never reused, even after removal.
    pub fn allocate(&mut self, width: u32, height: u32) -> Option<(u64, AtlasRect)> {
        // Simple row-based packing
        if self.cursor_x + width > self.max_size.width {
            // Start new row
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        if self.cursor_y + height > self.max_size.height {
            return None; // Atlas full
        }
        let id = self.next_id;
        self.next_id += 1;
        let rect = AtlasRect { x: self.cursor_x, y: self.cursor_y, width, height };
        self.entries.insert(id, AtlasEntry { rect, texture_id: id });
        self.cursor_x += width;
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
