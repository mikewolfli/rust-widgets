// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A small thread-local cache for text measurements shared by the base, overlay and
//! navigation control families (G-A02).
//!
//! # Why a cache at all
//!
//! The same caption is measured repeatedly inside one frame: a bar measures its title
//! for centring, then again for the fitted draw; a fixed hint such as `"Skip"` or
//! `"Loading..."` is measured on every repaint even though neither the text nor the
//! font has changed. The backend has its own cache, but reaching it still costs a call
//! per site, and the navigation family's per-item labels are measured per item per
//! frame. This cache sits one level up and makes the steady-state cost a hash and a
//! compare.
//!
//! # Invalidation
//!
//! The key is the font's complete identity plus the text, so there is no revision to
//! bump and no invalidation list to keep in sync — the same shape as
//! [`crate::widget::nav_widgets::tab_view`]'s `TabWidthCache`. A theme, text-scale or
//! font change produces a different key and therefore a miss.
//!
//! # Bounded
//!
//! A frame that paints a table's worth of distinct cells would otherwise grow the map
//! without limit. The map is bounded, and eviction removes the least recently used
//! entry rather than clearing the whole table (the same policy the theme style cache
//! adopted for G-169): one new caption must not throw away every constant one.

use crate::compat::{HashMap, RefCell, String, ToString};
use crate::core::Font;
use crate::render::{RenderContext, TextMetrics};

/// The largest number of entries kept per cache.
const CACHE_LIMIT: usize = 768;

/// One cached measurement, with the inputs it was derived from.
///
/// The font and the text are stored so a hash collision is detected and treated as a
/// miss rather than answered with the wrong width.
struct MeasureEntry {
    font: Font,
    text: String,
    /// The device scale the measurement was taken at.
    dpi_bits: u64,
    metrics: TextMetrics,
}

/// One cached width estimate.
struct EstimateEntry {
    font: Font,
    text: String,
    width: u32,
}

/// A hash map plus an intrusive least-recently-used list over its keys.
///
/// The previous eviction scan read every entry on each insert to find the smallest
/// stamp, which made a steady state of distinct captions O(n) per miss. The list makes
/// both "mark this key as just used" and "drop the oldest key" constant-time: each entry
/// stores the key before and after it, and the cache stores the head (oldest) and tail
/// (newest).
struct Lru<V> {
    entries: HashMap<u64, LruEntry<V>>,
    /// Least recently used key.
    head: Option<u64>,
    /// Most recently used key.
    tail: Option<u64>,
}

/// One entry of [`Lru`], with the links that make the recency order intrusive.
struct LruEntry<V> {
    value: V,
    prev: Option<u64>,
    next: Option<u64>,
}

impl<V> Lru<V> {
    fn new() -> Self {
        Self { entries: HashMap::new(), head: None, tail: None }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    /// Looks `key` up and marks it as the most recently used entry.
    fn get_mut(&mut self, key: u64) -> Option<&mut V> {
        if !self.entries.contains_key(&key) {
            return None;
        }
        self.touch(key);
        self.entries.get_mut(&key).map(|entry| &mut entry.value)
    }

    /// Inserts or replaces `key`, always marking it as the most recently used entry.
    fn insert(&mut self, key: u64, value: V) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            self.touch(key);
            return;
        }
        let prev = self.tail;
        self.entries.insert(key, LruEntry { value, prev, next: None });
        if let Some(prev) = prev {
            if let Some(entry) = self.entries.get_mut(&prev) {
                entry.next = Some(key);
            }
        } else {
            self.head = Some(key);
        }
        self.tail = Some(key);
    }

    /// Removes the least recently used entry, if any.
    fn evict(&mut self) -> Option<u64> {
        let victim = self.head?;
        self.remove(victim);
        Some(victim)
    }

    fn remove(&mut self, key: u64) -> Option<V> {
        let entry = self.entries.remove(&key)?;
        match entry.prev {
            Some(prev) => {
                if let Some(previous) = self.entries.get_mut(&prev) {
                    previous.next = entry.next;
                }
            }
            None => self.head = entry.next,
        }
        match entry.next {
            Some(next) => {
                if let Some(following) = self.entries.get_mut(&next) {
                    following.prev = entry.prev;
                }
            }
            None => self.tail = entry.prev,
        }
        Some(entry.value)
    }

    /// Moves `key` to the tail, where the next eviction cannot reach it.
    fn touch(&mut self, key: u64) {
        if self.tail == Some(key) {
            return;
        }
        let (prev, next) = match self.entries.get(&key) {
            Some(entry) => (entry.prev, entry.next),
            None => return,
        };
        match prev {
            Some(prev) => {
                if let Some(entry) = self.entries.get_mut(&prev) {
                    entry.next = next;
                }
            }
            None => self.head = next,
        }
        if let Some(next) = next {
            if let Some(entry) = self.entries.get_mut(&next) {
                entry.prev = prev;
            }
        } else {
            self.tail = prev;
        }
        let old_tail = self.tail;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.prev = old_tail;
            entry.next = None;
        }
        match old_tail {
            Some(tail) => {
                if let Some(entry) = self.entries.get_mut(&tail) {
                    entry.next = Some(key);
                }
            }
            None => self.head = Some(key),
        }
        self.tail = Some(key);
    }
}

thread_local! {
    static MEASUREMENTS: RefCell<Lru<MeasureEntry>> = RefCell::new(Lru::new());
    static ESTIMATES: RefCell<Lru<EstimateEntry>> = RefCell::new(Lru::new());
}

/// Hashes the complete font identity and the text into one key.
///
/// Deliberately allocation-free: this runs on every measurement read, and cloning the
/// family name into a key would cost exactly what the cache is meant to save.
fn fingerprint(text: &str, font: &Font, tag: u64, dpi_bits: u64) -> u64 {
    // FNV-1a, spelled out rather than pulled from a hasher type so this module stays
    // usable in the allocation-frugal profile as well.
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ tag;
    let mut mix = |value: u64| {
        hash = (hash ^ value).wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(dpi_bits);
    for byte in font.family().bytes() {
        mix(u64::from(byte));
    }
    mix(u64::from(font.size().to_bits()));
    mix(u64::from(font.weight()));
    mix(u64::from(font.is_italic()));
    mix(u64::from(font.letter_spacing().to_bits()));
    mix(u64::from(font.word_spacing().to_bits()));
    mix(u64::from(font.line_height().to_bits()));
    // A separator keeps `("ab", "c")` and `("a", "bc")` from sharing a key.
    mix(0xff);
    for byte in text.bytes() {
        mix(u64::from(byte));
    }
    hash
}

/// Measures `text` in `font`, reusing the previous answer when the inputs are unchanged.
pub(crate) fn measure_text(context: &RenderContext, text: &str, font: &Font) -> TextMetrics {
    // The measurement depends on the backend's device scale as well as on the font, so the
    // scale is part of the key: one logical font can be measured on two windows at different
    // DPI settings (G-A02).
    let key = fingerprint(text, font, 1, u64::from(context.dpi_scale().to_bits()));
    let dpi_bits = u64::from(context.dpi_scale().to_bits());
    let hit = MEASUREMENTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(key)?;
        if entry.font != *font || entry.text != text || entry.dpi_bits != dpi_bits {
            // A hash collision: the slot belongs to a different measurement. Treat it as
            // a miss rather than answering with the wrong width.
            return None;
        }
        Some(entry.metrics)
    });
    if let Some(metrics) = hit {
        return metrics;
    }

    let metrics = context.measure_text(text, font);
    MEASUREMENTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.evict();
        }
        cache.insert(
            key,
            MeasureEntry { font: font.clone(), text: text.to_string(), dpi_bits, metrics },
        );
    });
    metrics
}

/// Estimates the advance of `text` in `font` at unit scale, reusing the last answer.
///
/// This is the same quantity [`crate::widget::metrics::estimate_text_width`] reports
/// with a scale of `1.0`; it exists so a static caption's steady-state frame does not
/// re-walk the string.
pub(crate) fn estimate_text_width(text: &str, font: &Font) -> u32 {
    let key = fingerprint(text, font, 2, 0);
    let hit = ESTIMATES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache.get_mut(key)?;
        if entry.font != *font || entry.text != text {
            return None;
        }
        Some(entry.width)
    });
    if let Some(width) = hit {
        return width;
    }

    let width = crate::widget::metrics::estimate_text_width(text, font, 1.0);
    ESTIMATES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.evict();
        }
        cache.insert(key, EstimateEntry { font: font.clone(), text: text.to_string(), width });
    });
    width
}

#[cfg(test)]
mod tests {
    use super::Lru;

    #[test]
    fn eviction_drops_the_least_recently_touched_entry() {
        let mut cache: Lru<u32> = Lru::new();
        cache.insert(1, 10);
        cache.insert(2, 20);
        cache.insert(3, 30);

        // A hit reorders the list; the evicted key must be the one not touched since.
        assert_eq!(cache.get_mut(1).copied(), Some(10));
        assert_eq!(cache.evict(), Some(2));

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get_mut(1).copied(), Some(10));
        assert_eq!(cache.get_mut(3).copied(), Some(30));
    }

    #[test]
    fn removing_the_head_and_tail_keeps_the_links_consistent() {
        let mut cache: Lru<u32> = Lru::new();
        cache.insert(1, 10);
        cache.insert(2, 20);
        cache.insert(3, 30);

        assert_eq!(cache.remove(1), Some(10));
        assert_eq!(cache.evict(), Some(2));
        assert_eq!(cache.evict(), Some(3));
        assert_eq!(cache.evict(), None);
    }
}
