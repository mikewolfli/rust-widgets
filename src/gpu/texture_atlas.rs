// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Texture atlas for GPU rendering — packs small textures into a larger atlas (BLUE11 R5.8).
//!
//! # Status: host-side tool, outside the widget chain (G-228)
//!
//! This packer has no in-repo consumer: no widget, renderer or upload path
//! allocates from it (verified repo-wide by grep, G-228), and the wgpu texture
//! upload path it would serve lives in the renderer, which manages its own
//! textures today. It is therefore kept as a **host-side tool**: a standalone
//! rectangle packer a host — or the renderer, when it grows an atlas path — can
//! drive directly at `rust_widgets::gpu::texture_atlas::TextureAtlas`.
//! Deliberately *not* re-exported from `gpu`'s public surface, so the widget
//! chain does not advertise an API the library itself never calls.
//!
//! **Integration condition:** wire it into an image/glyph upload path only when
//! that path exists and asks for atlas packing (principle #51 — no speculative
//! integration). **Deletion condition:** if it is still unused by the next
//! major release, delete it together with its tests, exactly as
//! `GpuStagingBufferPool` documents for itself (principle #49).
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
/// # Complexity (G-56 / G-57)
///
/// * `free_rects` is kept ordered by area, so [`Self::allocate`] binary-searches
///   past every rectangle too small to hold the request instead of scanning the
///   whole list.
/// * `remove` coalesces in a single forward pass. A merge removes an element from
///   the free list, so the pass is O(n) rather than the O(n²) repeated-restart
///   scan it used to be.
/// * `used_area` is maintained incrementally, so [`Self::utilization`] is O(1)
///   instead of walking every live entry.
///
pub struct TextureAtlas {
    max_size: Size,
    entries: HashMap<u64, AtlasEntry>,
    next_id: u64,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    /// Reusable rectangles ordered by area, ascending. The order is the index
    /// [`Self::first_fitting_free_rect`] binary-searches; it is *not* insertion
    /// order, so "first fit" here means "the first fitting rectangle in area
    /// order", which is deterministic across runs.
    free_rects: Vec<AtlasRect>,
    /// Sum of the areas of all live entries, in texels. Maintained on every
    /// allocate/remove so `utilization` never walks `entries`.
    used_area: u64,
}

/// Area of `rect` in texels, computed in `u64` so a full-size atlas cannot
/// overflow the multiplication.
fn rect_area(rect: &AtlasRect) -> u64 {
    rect.width as u64 * rect.height as u64
}

/// Whether two free rectangles share a full edge and can therefore be merged.
fn rectangles_are_adjacent(a: &AtlasRect, b: &AtlasRect) -> bool {
    let horizontal = a.y == b.y
        && a.height == b.height
        && (a.x.saturating_add(a.width) == b.x || b.x.saturating_add(b.width) == a.x);
    let vertical = a.x == b.x
        && a.width == b.width
        && (a.y.saturating_add(a.height) == b.y || b.y.saturating_add(b.height) == a.y);
    horizontal || vertical
}

/// Bounding rectangle of two adjacent free rectangles.
fn merge_adjacent(a: &AtlasRect, b: &AtlasRect) -> AtlasRect {
    let horizontal = a.y == b.y && a.height == b.height;
    let vertical = a.x == b.x && a.width == b.width;
    AtlasRect {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        width: if horizontal { a.width + b.width } else { a.width },
        height: if vertical { a.height + b.height } else { a.height },
    }
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
            used_area: 0,
        }
    }

    /// Index of the first free rectangle (in area order) that can hold
    /// `width` × `height`, or `None` when none can.
    ///
    /// The binary search skips every rectangle whose area is smaller than the
    /// request, which is the size index G-57 asks for; the remaining suffix is
    /// still scanned for an exact width/height fit because a rectangle with
    /// enough area is not necessarily wide *and* tall enough.
    fn first_fitting_free_rect(&self, width: u32, height: u32) -> Option<usize> {
        let required_area = width as u64 * height as u64;
        let start = self.free_rects.partition_point(|rect| rect_area(rect) < required_area);
        self.free_rects[start..]
            .iter()
            .position(|rect| width <= rect.width && height <= rect.height)
            .map(|offset| start + offset)
    }

    /// Inserts a free rectangle while preserving the area ordering the binary
    /// search depends on.
    fn insert_free_rect(&mut self, rect: AtlasRect) {
        let area = rect_area(&rect);
        let index = self.free_rects.partition_point(|other| rect_area(other) <= area);
        self.free_rects.insert(index, rect);
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
            if let Some(index) = self.first_fitting_free_rect(width, height) {
                // `remove` rather than `swap_remove`: the free list is kept
                // ordered, and a scan must not be able to observe it scrambled.
                let free = self.free_rects.remove(index);
                let rect = AtlasRect { x: free.x, y: free.y, width, height };
                if free.width > width {
                    self.insert_free_rect(AtlasRect {
                        x: free.x + width,
                        y: free.y,
                        width: free.width - width,
                        height,
                    });
                }
                if free.height > height {
                    self.insert_free_rect(AtlasRect {
                        x: free.x,
                        y: free.y + height,
                        width: free.width,
                        height: free.height - height,
                    });
                }
                self.next_id = next_id;
                self.used_area += rect_area(&rect);
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
        self.used_area += rect_area(&rect);
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
    ///
    /// Coalescing runs as one forward pass over the free list: every merge removes
    /// a rectangle, so the pass is O(n) in total. The pass does not restart after a
    /// merge (G-56), which means it does not follow a chain back to rectangles it
    /// already visited; a coalesced rectangle can therefore stay adjacent to an
    /// earlier free rectangle. Reuse is unaffected — the next allocation can still
    /// take either rectangle — and the saving is the O(n²) rescan this used to do.
    pub fn remove(&mut self, id: u64) {
        let Some(entry) = self.entries.remove(&id) else { return };
        self.used_area = self.used_area.saturating_sub(rect_area(&entry.rect));
        if self.entries.is_empty() {
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.row_height = 0;
            self.free_rects.clear();
            self.used_area = 0;
            return;
        }
        let mut rect = entry.rect;
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Drain in place so the merged rectangle cannot be visited twice, and so
        // the survivors keep the area ordering `first_fitting_free_rect` relies on.
        let pending = core::mem::take(&mut self.free_rects);
        let mut kept = Vec::with_capacity(pending.len());
        for other in pending {
            if rectangles_are_adjacent(&rect, &other) {
                rect = merge_adjacent(&rect, &other);
            } else {
                kept.push(other);
            }
        }
        self.free_rects = kept;
        self.insert_free_rect(rect);
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
        self.used_area = 0;
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
    ///
    /// The numerator is maintained on every allocate/remove, so this is an O(1)
    /// lookup rather than a walk over every entry (G-57).
    pub fn utilization(&self) -> f32 {
        if self.max_size.width == 0 || self.max_size.height == 0 {
            return 0.0;
        }
        let total_pixels = self.max_size.width as u64 * self.max_size.height as u64;
        self.used_area as f32 / total_pixels as f32
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

    /// G-57: `utilization` must follow removals without re-walking the entries.
    #[test]
    fn utilization_tracks_removals_incrementally() {
        let mut atlas = TextureAtlas::new(Size::new(100, 100));
        let (first, _) = atlas.allocate(40, 50).expect("fits"); // 2000 texels
        let (second, _) = atlas.allocate(20, 25).expect("fits"); // 500 texels
        assert!((atlas.utilization() - 0.25).abs() < 1e-6);

        atlas.remove(first);
        assert!((atlas.utilization() - 0.05).abs() < 1e-6, "2000 texels released");

        atlas.remove(second);
        assert_eq!(atlas.utilization(), 0.0);
    }

    /// G-57: the area-ordered free list must skip rectangles that are too small
    /// even when they sit earlier in the list, so a large request still finds the
    /// large hole.
    #[test]
    fn free_rect_size_index_skips_undersized_holes() {
        let mut atlas = TextureAtlas::new(Size::new(32, 32));
        let (small, _) = atlas.allocate(8, 8).expect("fits");
        let (large, _) = atlas.allocate(24, 24).expect("fits");
        // Keep one live entry: removing every entry resets the atlas instead of
        // building a free list, which is a different code path.
        let (_keep, _) = atlas.allocate(4, 4).expect("fits on the next row");
        atlas.remove(small);
        atlas.remove(large);

        // The 8×8 hole has less area than the 16×16 request, so the size index
        // must land on the 24×24 hole instead.
        let (_, rect) = atlas.allocate(16, 16).expect("the large hole fits");
        assert_eq!((rect.x, rect.y), (8, 0));
    }
}
