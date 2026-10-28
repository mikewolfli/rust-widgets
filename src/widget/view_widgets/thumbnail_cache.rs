// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `ThumbnailCache` — a bounded, LRU cache of down-sampled bitmaps (COMP-F04).
//!
//! `ImageGallery` used to hand the renderer the **whole source image** for every visible
//! thumbnail and for the preview, every frame. A strip of ten 12-megapixel photos therefore
//! resampled 120 megapixels per frame; the cost did not change while scrolling or hovering.
//!
//! This companion generates a target-sized bitmap once per `(key, target)` and returns the
//! cached copy afterwards. It is not a drawing control: it owns no ink and is meant to be
//! borrowed by the control that paints the thumbnails.
//!
//! # Why the key includes the target size
//!
//! A gallery needs two different targets from the same image — a square thumbnail and a
//! stage-fitting preview — and the preview target changes with the zoom level. A cache keyed on
//! the image alone could only hold one of them; the target is part of what identifies the bitmap.

use crate::compat::{BTreeMap, Rc};
use crate::core::{Rect, Size};
use crate::image::{resize, ImageData};
use crate::signal::Signal1;

/// The thumbnail cache key: `(image key, target width, target height)`.
type ThumbnailKey = (u64, u32, u32);
/// The LRU index key: `(stamp, thumbnail key)`.
type LruKey = (u64, ThumbnailKey);
/// The cache map.
type EntryMap = HashMap<ThumbnailKey, Entry>;
/// The LRU index.
type LruIndex = BTreeMap<LruKey, ()>;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// One cached bitmap and the order entry that keeps it alive.
struct Entry {
    /// The cached pixels. Shared with the draw path, so a hit is a refcount bump rather than a
    /// full-bitmap copy (VP-001).
    data: Rc<ImageData>,
    /// The logical last-use stamp; the smallest live stamp is the eviction candidate.
    stamp: u64,
}

/// A bounded LRU cache of down-sampled RGBA8 bitmaps.
pub struct ThumbnailCache {
    base: BaseWidget,
    /// Capacity; interior-mutable so `thumbnail_for` can evict through `&self`.
    max_entries: Cell<usize>,
    target_size: Cell<(u32, u32)>,
    entries: RefCell<EntryMap>,
    /// Live `(stamp, key)` pairs, kept in lockstep with `entries`. A `BTreeMap` gives O(log C)
    /// least-stamp lookup and removal, replacing the linear scan (VP-005).
    order: RefCell<LruIndex>,
    next_stamp: Cell<u64>,
    /// Emitted with the key whenever a bitmap is generated (a cache miss).
    pub thumbnail_ready: Signal1<u64>,
}

impl ThumbnailCache {
    /// Creates an empty cache.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Panel, geometry, "ThumbnailCache"),
            max_entries: Cell::new(256),
            target_size: Cell::new((96, 96)),
            entries: RefCell::new(HashMap::new()),
            order: RefCell::new(BTreeMap::new()),
            next_stamp: Cell::new(0),
            thumbnail_ready: Signal1::new(),
        }
    }

    /// The number of cached bitmaps.
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// Whether the cache holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// The maximum number of bitmaps kept.
    pub fn max_entries(&self) -> usize {
        self.max_entries.get()
    }

    /// Sets the maximum number of bitmaps kept, evicting the least recently used ones.
    pub fn set_max_entries(&self, max_entries: usize) {
        self.max_entries.set(max_entries.max(1));
        self.evict_to_capacity();
    }

    /// The default target used by [`Self::thumbnail_for`].
    pub fn target_size(&self) -> (u32, u32) {
        self.target_size.get()
    }

    /// Sets the default target used by [`Self::thumbnail_for`].
    pub fn set_target_size(&self, size: (u32, u32)) {
        self.target_size.set((size.0.max(1), size.1.max(1)));
    }

    /// Returns the bitmap for `key` fitted into [`Self::target_size`].
    ///
    /// `source_size` is the pixel size of `source`; `ImageData` carries no dimensions, so the
    /// caller supplies the two. A miss generates, caches and emits `thumbnail_ready`.
    pub fn thumbnail_for(
        &self,
        key: u64,
        source: &ImageData,
        source_size: (u32, u32),
    ) -> Rc<ImageData> {
        self.thumbnail_for_sized(key, source, source_size, self.target_size.get())
    }

    /// Returns the bitmap for `key` fitted into `target` (aspect ratio preserved).
    ///
    /// The result is `target`-bounded: one axis equals the target and the other is scaled down.
    /// A degenerate source or target returns `source` unchanged instead of panicking; a host can
    /// then still paint something rather than a hole.
    pub fn thumbnail_for_sized(
        &self,
        key: u64,
        source: &ImageData,
        source_size: (u32, u32),
        target: (u32, u32),
    ) -> Rc<ImageData> {
        let target = (target.0.max(1), target.1.max(1));
        let cache_key = (key, target.0, target.1);
        let stamp = self.tick_stamp();
        if let Some(entry) = self.entries.borrow_mut().get_mut(&cache_key) {
            let previous = entry.stamp;
            entry.stamp = stamp;
            let data = Rc::clone(&entry.data);
            // Move the key's order entry to the new stamp. A hit is therefore a refcount bump plus
            // one O(log C) index update, with no pixel copy (VP-001).
            let mut order = self.order.borrow_mut();
            order.remove(&(previous, cache_key));
            order.insert((stamp, cache_key), ());
            return data;
        }
        let data = Rc::new(fit_resample(source, source_size, target));
        self.entries.borrow_mut().insert(cache_key, Entry { data: Rc::clone(&data), stamp });
        self.order.borrow_mut().insert((stamp, cache_key), ());
        self.evict_to_capacity();
        self.thumbnail_ready.emit(key);
        data
    }

    /// Advances the monotonic use stamp.
    fn tick_stamp(&self) -> u64 {
        let stamp = self.next_stamp.get().wrapping_add(1);
        self.next_stamp.set(stamp);
        stamp
    }

    /// Drops every cached bitmap for `key`, across all target sizes.
    pub fn invalidate(&self, key: u64) {
        self.entries.borrow_mut().retain(|(cached_key, _, _), _| *cached_key != key);
        self.order.borrow_mut().retain(|(_, (cached_key, _, _)), _| *cached_key != key);
    }

    /// Drops every cached bitmap.
    pub fn clear(&self) {
        self.entries.borrow_mut().clear();
        self.order.borrow_mut().clear();
    }

    /// Evicts least-recently-used entries until the cache is within its bound.
    ///
    /// The order map names the smallest live stamp, so each eviction is one `BTreeMap` lookup and
    /// removal rather than a scan of every key (VP-005).
    fn evict_to_capacity(&self) {
        while self.entries.borrow().len() > self.max_entries.get() {
            let oldest = self.order.borrow().keys().next().copied();
            let Some(entry) = oldest else {
                break;
            };
            self.order.borrow_mut().remove(&entry);
            self.entries.borrow_mut().remove(&entry.1);
        }
    }
}

/// Fits `source` into `target` preserving aspect ratio, then resamples it.
///
/// `ImageData` has no dimensions, so the caller's `source_size` is authoritative. A non-RGBA8
/// source or a size mismatch returns the source unchanged — `resize` would otherwise be asked to
/// read past a buffer it did not size.
fn fit_resample(source: &ImageData, source_size: (u32, u32), target: (u32, u32)) -> ImageData {
    let (source_w, source_h) = (source_size.0.max(1), source_size.1.max(1));
    let scale = (target.0 as f32 / source_w as f32).min(target.1 as f32 / source_h as f32);
    let dst_w = (source_w as f32 * scale).round().max(1.0) as u32;
    let dst_h = (source_h as f32 * scale).round().max(1.0) as u32;
    if dst_w == source_w && dst_h == source_h {
        return source.clone();
    }
    resize(source, source_w, source_h, dst_w, dst_h).unwrap_or_else(|_| source.clone())
}

impl Widget for ThumbnailCache {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn size_hint(&self) -> Size {
        let (width, height) = self.target_size.get();
        Size::new(width, height)
    }
}

impl crate::event::EventHandler for ThumbnailCache {
    fn handle_event(&mut self, _event: &crate::event::Event) {
        // A cache has no gesture; it is maintained by the control that paints from it.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(width: u32, height: u32, value: u8) -> ImageData {
        ImageData::Rgba8(vec![value; (width * height * 4) as usize])
    }

    #[test]
    fn a_miss_generates_once_and_a_hit_reuses_it() {
        let cache = ThumbnailCache::new(Rect::new(0, 0, 100, 100));
        let source = rgba(64, 32, 120);
        let ready = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = std::sync::Arc::clone(&ready);
        cache.thumbnail_ready.connect(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        let first = cache.thumbnail_for(7, &source, (64, 32));
        assert_eq!(ready.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(cache.len(), 1);
        let second = cache.thumbnail_for(7, &source, (64, 32));
        assert_eq!(second, first, "the cached bitmap is returned");
        assert_eq!(
            ready.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "a cache hit does not regenerate"
        );
        match &*first {
            ImageData::Rgba8(bytes) => {
                // 64x32 fitted into 96x96 is 96x48 at most; the target is square, so 96x48.
                assert_eq!(bytes.len(), 96 * 48 * 4);
            }
            _ => panic!("the fixture is RGBA8"),
        }
    }

    #[test]
    fn invalidate_and_lru_eviction_drop_the_right_entries() {
        let cache = ThumbnailCache::new(Rect::new(0, 0, 100, 100));
        cache.set_max_entries(2);
        let source = rgba(8, 8, 10);
        let _ = cache.thumbnail_for(1, &source, (8, 8));
        let _ = cache.thumbnail_for(2, &source, (8, 8));
        assert_eq!(cache.len(), 2);
        // Touching 1 makes 2 the eviction candidate.
        let _ = cache.thumbnail_for(1, &source, (8, 8));
        let _ = cache.thumbnail_for(3, &source, (8, 8));
        assert_eq!(cache.len(), 2);
        assert_eq!(*cache.thumbnail_for(1, &source, (8, 8)), rgba(96, 96, 10));

        cache.invalidate(1);
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
