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
    /// Monotonic use counter, for least-recently-used eviction.
    stamp: u64,
}

/// One cached width estimate.
struct EstimateEntry {
    font: Font,
    text: String,
    width: u32,
    stamp: u64,
}

thread_local! {
    // `HashMap::new` is const only under alloc_frugal (BTreeMap); the std profile cannot be const.
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static MEASUREMENTS: RefCell<(HashMap<u64, MeasureEntry>, u64)> =
        RefCell::new((HashMap::new(), 0));
    #[cfg_attr(alloc_frugal, allow(clippy::missing_const_for_thread_local))]
    static ESTIMATES: RefCell<(HashMap<u64, EstimateEntry>, u64)> =
        RefCell::new((HashMap::new(), 0));
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
        let (entries, stamp) = &mut *cache;
        let entry = entries.get_mut(&key)?;
        if entry.font != *font || entry.text != text || entry.dpi_bits != dpi_bits {
            // A hash collision: the slot belongs to a different measurement. Treat it as
            // a miss rather than answering with the wrong width.
            return None;
        }
        *stamp = stamp.wrapping_add(1);
        entry.stamp = *stamp;
        Some(entry.metrics)
    });
    if let Some(metrics) = hit {
        return metrics;
    }

    let metrics = context.measure_text(text, font);
    MEASUREMENTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let (entries, stamp) = &mut *cache;
        if entries.len() >= CACHE_LIMIT {
            evict_measure(entries);
        }
        *stamp = stamp.wrapping_add(1);
        entries.insert(
            key,
            MeasureEntry {
                font: font.clone(),
                text: text.to_string(),
                dpi_bits,
                metrics,
                stamp: *stamp,
            },
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
        let (entries, stamp) = &mut *cache;
        let entry = entries.get_mut(&key)?;
        if entry.font != *font || entry.text != text {
            return None;
        }
        *stamp = stamp.wrapping_add(1);
        entry.stamp = *stamp;
        Some(entry.width)
    });
    if let Some(width) = hit {
        return width;
    }

    let width = crate::widget::metrics::estimate_text_width(text, font, 1.0);
    ESTIMATES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let (entries, stamp) = &mut *cache;
        if entries.len() >= CACHE_LIMIT {
            evict_estimate(entries);
        }
        *stamp = stamp.wrapping_add(1);
        entries.insert(
            key,
            EstimateEntry { font: font.clone(), text: text.to_string(), width, stamp: *stamp },
        );
    });
    width
}

/// Removes the least recently used measurement.
fn evict_measure(entries: &mut HashMap<u64, MeasureEntry>) {
    let victim = entries.iter().min_by_key(|(_, entry)| entry.stamp).map(|(key, _)| *key);
    if let Some(victim) = victim {
        entries.remove(&victim);
    }
}

/// Removes the least recently used estimate.
fn evict_estimate(entries: &mut HashMap<u64, EstimateEntry>) {
    let victim = entries.iter().min_by_key(|(_, entry)| entry.stamp).map(|(key, _)| *key);
    if let Some(victim) = victim {
        entries.remove(&victim);
    }
}
