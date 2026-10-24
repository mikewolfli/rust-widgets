// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A glyph atlas: rasterised glyph bitmaps remembered by face, character and size.
//!
//! # What this is
//!
//! The rasteriser is deterministic: the coverage bytes for a character depend only on the face,
//! the character and the device cell. A repaint asks for the same few hundred glyphs over and
//! over, so this module remembers the answer and hands the same buffer back.
//!
//! # What this is **not**
//!
//! It is not a second rasteriser. A miss calls the rasteriser's `rasterize_face_at_weight`, which
//! is the same code path a direct paint takes, with the same face cache and the same per-cell
//! coverage cache. The atlas therefore cannot change a glyph's bytes: it only decides whether to compute
//! them again.
//!
//! # Scope
//!
//! The atlas is owned by its caller and has no process-wide state. A renderer that paints on one
//! thread creates one and passes `&mut` to it; the slots hold [`Rc`](crate::compat::Rc) buffers,
//! so an [`AtlasSlot`] can be kept by a caller while the atlas continues to use the same bytes.
//!
//! # Frame epochs
//!
//! [`GlyphAtlas::begin_frame`] starts a new epoch and drops slots that have not been requested
//! for [`ATLAS_GC_FRAMES`] frames. A page of static text therefore keeps its glyphs resident, a
//! one-off label ages out, and a miss is never wrong — only slower.

use crate::compat::{HashMap, Rc};

use super::glyph_source::{paint_active, Cell, InkKind};
use super::raster::rasterize_face_at_weight;
use super::shaping::face_for_family;

/// How many frames a slot may go unrequested before [`GlyphAtlas::begin_frame`] recycles it.
///
/// Two frames covers the ordinary case where a piece of text is drawn every frame but a
/// transient (a tooltip, a hover label) is not. The value is a policy, not a correctness
/// requirement: an evicted slot is rasterised again on its next use.
pub const ATLAS_GC_FRAMES: u64 = 2;

/// What identifies one rasterised glyph.
///
/// # Why each field is part of the key
///
/// * `face` is the family name the caller asked for. It is the name `shaping::face_for_family`
///   resolves, so a host face and a shipped face with the same name are the same choice by
///   definition.
/// * `ch` is the character, not a shaped glyph id: the atlas is consulted from text layout,
///   where the character is what is known.
/// * `size_px` is the device pixel size, because coverage is rasterised at one size.
/// * `weight` is part of the key because a variable face is one face with a `wght` axis: the same
///   character at 400 and at 700 is different ink. A static face ignores it, so the key still
///   identifies exactly what was rasterised for every face this crate ships.
///
/// `Ord` is derived as well as `Hash`/`Eq` because this crate's [`HashMap`](crate::compat::HashMap)
/// alias becomes an ordered map on allocation-frugal builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlyphKey {
    /// The family name the caller asked for.
    pub face: &'static str,
    /// The character to rasterise.
    pub ch: char,
    /// The device pixel size the glyph is rasterised at.
    pub size_px: u8,
    /// The requested weight, in the conventional `100..=900` scale.
    pub weight: u16,
}

impl GlyphKey {
    /// A key for `ch` at `size_px` pixels and `weight`, drawn from `face`.
    pub const fn new(face: &'static str, ch: char, size_px: u8, weight: u16) -> Self {
        Self { face, ch, size_px, weight }
    }

    /// The one device cell this key rasterises into.
    ///
    /// The cell is the em square: `size_px` wide and `size_px` tall. Height is what the
    /// rasteriser uses as the pixel size, and the square width leaves room for a glyph whose
    /// advance is a full em. A caller that needs a different box asks the rasteriser directly
    /// rather than through the atlas, because the cell is part of what was cached.
    pub const fn cell(self) -> Cell {
        Cell::new(self.size_px as u32, self.size_px as u32)
    }
}

/// One cached glyph bitmap.
///
/// The bitmap is one coverage or bitmap byte per pixel, row-major, in the cell reported by
/// [`Self::width`] and [`Self::height`]. Colour ink is not represented: a colour face needs four
/// bytes per pixel, and the atlas reserves the one-byte coverage box. A colour-only character
/// therefore falls through the active stack to the next 1-bit face, exactly as a direct paint with
/// a coverage-sized buffer does.
#[derive(Debug, Clone)]
pub struct AtlasSlot {
    /// Cell width in device pixels.
    width: u32,
    /// Cell height in device pixels.
    height: u32,
    /// The glyph's bytes, shared with every other clone of this slot.
    bitmap: Rc<[u8]>,
    /// The source that produced the ink.
    source: &'static str,
    /// What shape of ink [`Self::bitmap`] holds.
    ink: InkKind,
    /// How many times this slot was served after it was rasterised.
    hits: u64,
    /// The epoch of the most recent request.
    last_epoch: u64,
}

impl AtlasSlot {
    /// Cell width in device pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Cell height in device pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// One byte per pixel, row-major, `width * height` bytes long.
    pub fn bitmap(&self) -> &[u8] {
        &self.bitmap
    }

    /// The source that produced the ink: a face name, or the fallback source's name.
    pub fn source(&self) -> &'static str {
        self.source
    }

    /// What shape of ink the bitmap holds.
    pub fn ink(&self) -> InkKind {
        self.ink
    }

    /// How many times the atlas served this slot from cache.
    pub fn hits(&self) -> u64 {
        self.hits
    }
}

/// A cache of rasterised glyphs, keyed by [`GlyphKey`].
///
/// See the module docs for the caching and eviction policy.
#[derive(Debug)]
pub struct GlyphAtlas {
    slots: HashMap<GlyphKey, AtlasSlot>,
    epoch: u64,
    /// Number of misses that reached the rasteriser. Test-only so the production struct keeps
    /// exactly the state the cache needs.
    #[cfg(test)]
    rasterisations: u64,
}

impl GlyphAtlas {
    /// An empty atlas at epoch zero.
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            epoch: 0,
            #[cfg(test)]
            rasterisations: 0,
        }
    }

    /// The current frame epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// How many glyphs are resident.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Whether `key` is resident.
    pub fn contains(&self, key: &GlyphKey) -> bool {
        self.slots.contains_key(key)
    }

    /// The slot for `key`, rasterising it through the text rasteriser on a miss.
    ///
    /// A hit returns the same shared bitmap and increments the slot's hit count. A miss paints
    /// with the key's named face when that face covers the character; otherwise it resolves
    /// through the active glyph stack, so a character the named face does not cover still renders
    /// its normal fallback — including tofu — with the bytes a direct paint would produce.
    pub fn get_or_raster(&mut self, key: GlyphKey) -> AtlasSlot {
        if let Some(slot) = self.slots.get_mut(&key) {
            slot.hits = slot.hits.saturating_add(1);
            slot.last_epoch = self.epoch;
            return slot.clone();
        }

        #[cfg(test)]
        {
            self.rasterisations = self.rasterisations.saturating_add(1);
        }

        let slot = rasterise(key, self.epoch);
        self.slots.insert(key, slot.clone());
        slot
    }

    /// Starts a new frame epoch and recycles slots that have not been requested for
    /// [`ATLAS_GC_FRAMES`] frames.
    ///
    /// A slot requested in the frame that just ended has an age of one and survives; a slot
    /// requested after this call carries the new epoch and cannot be collected until it ages out.
    pub fn begin_frame(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        let epoch = self.epoch;
        self.slots.retain(|_, slot| epoch.saturating_sub(slot.last_epoch) <= ATLAS_GC_FRAMES);
    }

    /// Number of glyphs that reached the rasteriser. Test-only.
    #[cfg(test)]
    fn rasterisations(&self) -> u64 {
        self.rasterisations
    }
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new()
    }
}

/// Rasterises `key` through the text layer and wraps the result as a slot.
fn rasterise(key: GlyphKey, epoch: u64) -> AtlasSlot {
    let cell = key.cell();

    // The key names the face, so try it first. A named face that this build does not carry, or
    // that has no glyph for the character, falls through to the active stack rather than
    // inventing ink.
    if let Some(face) = face_for_family(key.face) {
        if let Some(bitmap) = rasterize_face_at_weight(face, key.ch, cell, key.weight) {
            return AtlasSlot {
                width: cell.width,
                height: cell.height,
                bitmap: Rc::from(bitmap),
                source: face.name,
                ink: InkKind::Coverage,
                hits: 0,
                last_epoch: epoch,
            };
        }
    }

    // The named face could not answer. Paint through the active stack, which is the same path a
    // caller that did not name a face would take, so a fallback (including tofu) is unchanged.
    let mut out = crate::compat::vec![0u8; cell.area()];
    let painted = paint_active(key.ch, cell, &mut out);
    let (source, ink) = match painted {
        Some(painted) => (painted.source, painted.ink),
        None => (key.face, InkKind::Coverage),
    };
    AtlasSlot {
        width: cell.width,
        height: cell.height,
        bitmap: Rc::from(out),
        source,
        ink,
        hits: 0,
        last_epoch: epoch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{vec, Rc};
    use crate::render::text::{paint_active, Cell};

    /// The key a caller derives from the source the active stack reports for `ch`.
    ///
    /// A source that names an outline face resolves to that face, so the atlas paints the same
    /// outlines the stack does. A source that names a bitmap face does not resolve by name, so the
    /// atlas takes its documented fallback through the active stack — which is the same path and
    /// therefore the same bytes.
    fn key_for(ch: char, size_px: u8) -> GlyphKey {
        let cell = Cell::new(u32::from(size_px), u32::from(size_px));
        let mut scratch = vec![0u8; cell.area()];
        let face =
            paint_active(ch, cell, &mut scratch).map(|painted| painted.source).unwrap_or("font8x8");
        GlyphKey::new(face, ch, size_px, 400)
    }

    #[test]
    fn a_repeated_lookup_returns_the_same_bitmap_and_counts_the_hit() {
        let mut atlas = GlyphAtlas::new();
        let key = key_for('A', 16);

        let first = atlas.get_or_raster(key);
        let second = atlas.get_or_raster(key);

        assert_eq!(first.bitmap(), second.bitmap(), "a hit must return the cached bytes");
        assert!(
            Rc::ptr_eq(&first.bitmap, &second.bitmap),
            "a hit must share the buffer rather than copy it"
        );
        assert_eq!(first.hits(), 0, "the first request rasterises; it is not a hit");
        assert_eq!(second.hits(), 1, "the second request is one hit");
        assert_eq!(atlas.rasterisations(), 1, "only the first request reaches the rasteriser");
    }

    #[test]
    fn a_second_frame_of_the_same_text_rasterises_nothing() {
        let mut atlas = GlyphAtlas::new();
        let keys: crate::compat::Vec<GlyphKey> =
            "Hello 世界".chars().map(|ch| key_for(ch, 16)).collect();

        for key in keys.iter().copied() {
            atlas.get_or_raster(key);
        }
        let first_frame = atlas.rasterisations();
        assert_eq!(
            first_frame as usize,
            atlas.slot_count(),
            "one rasterisation per distinct glyph"
        );

        atlas.begin_frame();
        for key in keys.iter().copied() {
            atlas.get_or_raster(key);
        }

        assert_eq!(
            atlas.rasterisations(),
            first_frame,
            "the second frame must be served entirely from the atlas"
        );
    }

    #[test]
    fn garbage_collection_keeps_a_slot_hit_in_the_current_frame() {
        let mut atlas = GlyphAtlas::new();
        let key = key_for('A', 16);

        atlas.begin_frame();
        atlas.get_or_raster(key);
        assert!(atlas.contains(&key), "the request should have cached the slot");

        // A slot requested in the frame that just ended is one frame old and must survive.
        atlas.begin_frame();
        assert!(atlas.contains(&key), "a slot hit in the current frame must not be collected");

        // Without another request it eventually ages out.
        for _ in 0..=ATLAS_GC_FRAMES {
            atlas.begin_frame();
        }
        assert!(!atlas.contains(&key), "a slot untouched past the window must be recycled");
    }

    #[test]
    fn the_atlas_matches_the_uncached_path_for_mixed_text() {
        let mut atlas = GlyphAtlas::new();

        for ch in "Aé 中あ".chars() {
            let key = key_for(ch, 16);
            let cell = key.cell();

            // The uncached control: the active stack into a coverage-sized buffer.
            let mut expected = vec![0u8; cell.area()];
            let painted = paint_active(ch, cell, &mut expected);
            let slot = atlas.get_or_raster(key);

            assert_eq!(
                slot.bitmap(),
                &expected[..],
                "atlas bytes differ from the uncached path for U+{:04X} ({ch:?})",
                ch as u32
            );
            assert_eq!(
                Some(slot.source()),
                painted.map(|painted| painted.source),
                "atlas and uncached path disagree about the source for U+{:04X}",
                ch as u32
            );
        }
    }
}
