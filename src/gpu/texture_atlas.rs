// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Texture atlas for GPU rendering — packs small textures into a larger atlas (BLUE11 R5.8).
use crate::compat::{HashMap, Vec};
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
/// row is started below the tallest entry of the previous row. Removed rectangles
/// are reused before advancing the row packer. Adjacent free rectangles are coalesced;
/// live entries never move, so already uploaded texels and UV coordinates remain valid.
///
pub struct TextureAtlas {
    max_size: Size,
    entries: HashMap<u64, AtlasEntry>,
    next_id: u64,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    free_rects: Vec<AtlasRect>,
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
            free_rects: Vec::new(),
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
        // Exhaustion must not overwrite a live entry with a saturated identifier.
        let id = self.next_id;
        let next_id = id.checked_add(1)?;
        if width > 0 && height > 0 {
            if let Some(index) =
                self.free_rects.iter().position(|rect| width <= rect.width && height <= rect.height)
            {
                let free = self.free_rects.swap_remove(index);
                let rect = AtlasRect { x: free.x, y: free.y, width, height };
                if free.width > width {
                    self.free_rects.push(AtlasRect {
                        x: free.x + width,
                        y: free.y,
                        width: free.width - width,
                        height,
                    });
                }
                if free.height > height {
                    self.free_rects.push(AtlasRect {
                        x: free.x,
                        y: free.y + height,
                        width: free.width,
                        height: free.height - height,
                    });
                }
                self.next_id = next_id;
                self.entries.insert(id, AtlasEntry { rect, texture_id: id });
                return Some((id, rect));
            }
        }
        // Compute the prospective origin *without* mutating the packer, so a failed
        // allocation leaves `cursor_x`/`cursor_y`/`row_height` exactly where they were.
        let wrap = self.cursor_x.checked_add(width).map(|next| next > self.max_size.width)?;
        let x = if wrap { 0 } else { self.cursor_x };
        let y = if wrap { self.cursor_y.checked_add(self.row_height)? } else { self.cursor_y };
        // Validate the prospective row before committing any state: the entry must
        // fit vertically on the row it lands on, including after a wrap.
        if y.checked_add(height)? > self.max_size.height {
            return None; // Atlas full or the new row overflows
        }
        self.next_id = next_id;
        let rect = AtlasRect { x, y, width, height };
        self.entries.insert(id, AtlasEntry { rect, texture_id: id });
        self.cursor_x = x.checked_add(width)?;
        self.cursor_y = y;
        self.row_height = if wrap { height } else { self.row_height.max(height) };
        Some((id, rect))
    }

    /// Returns the entry with the given id, or `None` if it was never allocated
    /// or has been removed.
    pub fn get(&self, id: u64) -> Option<&AtlasEntry> {
        self.entries.get(&id)
    }

    /// Reclaims an entry's rectangle. No-op for an unknown id.
    ///
    /// A removed rectangle may be overwritten by the next allocation. Live entries
    /// retain their coordinates, and removed identifiers are never reused.
    pub fn remove(&mut self, id: u64) {
        let Some(entry) = self.entries.remove(&id) else { return };
        if self.entries.is_empty() {
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.row_height = 0;
            self.free_rects.clear();
            return;
        }
        let mut rect = entry.rect;
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let mut index = 0;
        while index < self.free_rects.len() {
            let other = self.free_rects[index];
            let horizontal = rect.y == other.y
                && rect.height == other.height
                && (rect.x + rect.width == other.x || other.x + other.width == rect.x);
            let vertical = rect.x == other.x
                && rect.width == other.width
                && (rect.y + rect.height == other.y || other.y + other.height == rect.y);
            if horizontal || vertical {
                self.free_rects.swap_remove(index);
                rect = AtlasRect {
                    x: rect.x.min(other.x),
                    y: rect.y.min(other.y),
                    width: if horizontal { rect.width + other.width } else { rect.width },
                    height: if vertical { rect.height + other.height } else { rect.height },
                };
                index = 0;
            } else {
                index += 1;
            }
        }
        self.free_rects.push(rect);
    }

    /// Forgets every entry and resets the packer, making the whole atlas
    /// available again. Ids continue from the previous value rather than
    /// restarting at `1`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.free_rects.clear();
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

    /// A failed allocation must not consume space or move the cursor — including
    /// when the failure is a too-tall entry on a wrapped row.
    #[test]
    fn a_failed_allocation_does_not_corrupt_packer_state() {
        let mut atlas = TextureAtlas::new(Size::new(10, 10));
        assert!(atlas.allocate(6, 6).is_some());
        // 5x5 would wrap to a second row but is too tall for the remaining
        // height: it must fail without moving the cursor or committing a new row.
        assert!(atlas.allocate(5, 5).is_none());
        // The 4x6 that fits beside the first entry is still allocatable.
        let (_, rect) = atlas.allocate(4, 6).expect("packer state must remain intact");
        assert_eq!((rect.x, rect.y), (6, 0));
    }

    #[test]
    fn removed_rectangles_are_split_and_coalesced_without_moving_live_entries() {
        let mut atlas = TextureAtlas::new(Size::new(16, 8));
        let (live, _) = atlas.allocate(8, 8).unwrap();
        let (removed, _) = atlas.allocate(8, 8).unwrap();
        for _ in 0..100 {
            atlas.remove(removed);
            let (a, first) = atlas.allocate(4, 8).unwrap();
            let (b, second) = atlas.allocate(4, 8).unwrap();
            assert_eq!((first.x, second.x), (8, 12));
            atlas.remove(a);
            atlas.remove(b);
            let (id, rect) = atlas.allocate(8, 8).unwrap();
            assert_eq!((rect.x, rect.y), (8, 0));
            atlas.remove(id);
        }
        assert_eq!(atlas.get(live).unwrap().rect.x, 0);
        assert!(atlas.get(removed).is_none());
    }

    #[test]
    fn exhausted_ids_do_not_replace_live_entries() {
        let mut atlas = TextureAtlas::new(Size::new(8, 8));
        atlas.next_id = u64::MAX;
        assert!(atlas.allocate(1, 1).is_none());
        assert!(atlas.is_empty());
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
